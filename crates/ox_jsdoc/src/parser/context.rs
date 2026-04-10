// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use oxc_allocator::{Allocator, Box as ArenaBox, Vec as ArenaVec};
use oxc_diagnostics::OxcDiagnostic;
use oxc_span::Span;

use crate::ast::{
    BlockTag, BlockTagBody, Description, DescriptionPart, GenericTagBody, InlineTag, InlineTagBody,
    JSDocComment, NamePathLike, ParameterPath, TagName, TagParameterName, TagValueToken, Text,
    TypeExpression,
};

use super::{
    Checkpoint, FenceState, ParseOptions, ParseOutput, ParserDiagnosticKind, QuoteKind, diagnostic,
    scanner,
};

/// Temporary representation for one block tag section.
///
/// The scanner first partitions logical lines into top-level description lines
/// and tag sections. Each section keeps the tag header plus all continuation
/// lines until the next block tag.
#[derive(Debug, Clone)]
struct TagSection<'a> {
    /// Tag name without the leading `@`.
    tag_name: &'a str,
    /// Absolute byte offset of the tag name start.
    tag_name_start: u32,
    /// Absolute byte offset of the tag name end.
    tag_name_end: u32,
    /// Logical lines that belong to the tag body.
    body_lines: Vec<scanner::LogicalLine<'a>>,
    /// Absolute byte offset where the tag section ends.
    end: u32,
}

/// Stateful parser for one JSDoc block.
///
/// The parser owns no source text. It borrows input, writes AST nodes into the
/// arena, and accumulates diagnostics for recoverable malformed syntax.
pub struct ParserContext<'a> {
    /// Arena used for all AST allocations.
    pub(crate) allocator: &'a Allocator,
    /// Complete source slice for one JSDoc block.
    pub(crate) source_text: &'a str,
    /// Absolute byte offset of `source_text` in the original file.
    pub(crate) base_offset: u32,
    /// Current parser offset relative to `source_text`.
    pub(crate) offset: u32,
    /// Feature switches for this parse.
    pub(crate) _options: ParseOptions,
    /// Diagnostics emitted while parsing this comment.
    pub(crate) diagnostics: Vec<OxcDiagnostic>,
    /// Current nested `{...}` depth for speculative scanners.
    pub(crate) brace_depth: u16,
    /// Current nested `[...]` depth for speculative scanners.
    pub(crate) bracket_depth: u16,
    /// Current nested `(...)` depth for speculative scanners.
    pub(crate) paren_depth: u16,
    /// Active quote context for speculative scanners.
    pub(crate) quote: Option<QuoteKind>,
    /// Active fenced code context for speculative scanners.
    pub(crate) fence: Option<FenceState>,
}

impl<'a> ParserContext<'a> {
    /// Create a parser context for one complete comment block.
    pub fn new(
        allocator: &'a Allocator,
        source_text: &'a str,
        base_offset: u32,
        options: ParseOptions,
    ) -> Self {
        Self {
            allocator,
            source_text,
            base_offset,
            offset: 0,
            _options: options,
            diagnostics: Vec::new(),
            brace_depth: 0,
            bracket_depth: 0,
            paren_depth: 0,
            quote: None,
            fence: None,
        }
    }

    #[must_use]
    /// Capture rewindable parser state.
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            offset: self.offset,
            brace_depth: self.brace_depth,
            bracket_depth: self.bracket_depth,
            paren_depth: self.paren_depth,
            quote: self.quote,
            fence: self.fence,
            diagnostics_len: self.diagnostics.len(),
        }
    }

    /// Restore a previous checkpoint and discard diagnostics emitted after it.
    pub fn rewind(&mut self, checkpoint: Checkpoint) {
        self.offset = checkpoint.offset;
        self.brace_depth = checkpoint.brace_depth;
        self.bracket_depth = checkpoint.bracket_depth;
        self.paren_depth = checkpoint.paren_depth;
        self.quote = checkpoint.quote;
        self.fence = checkpoint.fence;
        self.diagnostics.truncate(checkpoint.diagnostics_len);
    }

    /// Parse the full JSDoc comment into the arena-backed AST.
    pub fn parse_comment(mut self) -> ParseOutput<'a> {
        let Some(end) = self.absolute_end() else {
            self.diagnostics
                .push(diagnostic(ParserDiagnosticKind::SpanOverflow));
            return ParseOutput {
                comment: None,
                diagnostics: self.diagnostics,
            };
        };

        if !scanner::is_jsdoc_block(self.source_text) {
            self.diagnostics
                .push(diagnostic(ParserDiagnosticKind::NotAJSDocBlock));
            return ParseOutput {
                comment: None,
                diagnostics: self.diagnostics,
            };
        }

        if !scanner::has_closing_block(self.source_text) {
            self.diagnostics
                .push(diagnostic(ParserDiagnosticKind::UnclosedBlockComment));
            return ParseOutput {
                comment: None,
                diagnostics: self.diagnostics,
            };
        }

        // Once the block shell is known-good, parsing works on logical content
        // lines with comment margins removed.
        let logical_lines = scanner::logical_lines(self.source_text, self.base_offset);
        let (description_lines, tag_sections) = self.partition_sections(&logical_lines);
        let description = self.parse_description_lines(&description_lines);
        let tags = self.parse_tag_sections(&tag_sections);

        let comment = ArenaBox::new_in(
            JSDocComment {
                span: Span::new(self.base_offset, end),
                description,
                tags,
            },
            self.allocator,
        );

        ParseOutput {
            comment: Some(comment),
            diagnostics: self.diagnostics,
        }
    }

    fn absolute_end(&self) -> Option<u32> {
        let len = u32::try_from(self.source_text.len()).ok()?;
        self.base_offset.checked_add(len)
    }

    fn partition_sections(
        &self,
        lines: &[scanner::LogicalLine<'a>],
    ) -> (Vec<scanner::LogicalLine<'a>>, Vec<TagSection<'a>>) {
        let mut description_lines = Vec::new();
        let mut tag_sections = Vec::new();
        let mut current_tag: Option<TagSection<'a>> = None;
        let mut in_fence = false;

        for line in lines {
            let trimmed = line.content.trim_start();
            let trimmed_delta = line.content.len() - trimmed.len();
            let trimmed_start = line.content_start + u32::try_from(trimmed_delta).unwrap();

            if !in_fence {
                if let Some((tag_name, tag_name_start, body_start)) =
                    parse_tag_header(trimmed, trimmed_start)
                {
                    if let Some(section) = current_tag.take() {
                        tag_sections.push(section);
                    }

                    let mut body_lines = Vec::new();
                    if let Some((body, body_abs_start)) = body_start {
                        body_lines.push(scanner::LogicalLine {
                            content: body,
                            content_start: body_abs_start,
                            content_end: line.content_end,
                        });
                    }

                    current_tag = Some(TagSection {
                        tag_name,
                        tag_name_start,
                        tag_name_end: tag_name_start + u32::try_from(tag_name.len()).unwrap(),
                        body_lines,
                        end: line.content_end,
                    });
                } else if let Some(section) = current_tag.as_mut() {
                    // Non-tag lines after a tag are continuation text for that
                    // tag body.
                    section.body_lines.push(*line);
                    section.end = line.content_end;
                } else {
                    description_lines.push(*line);
                }
            } else if let Some(section) = current_tag.as_mut() {
                section.body_lines.push(*line);
                section.end = line.content_end;
            } else {
                description_lines.push(*line);
            }

            // Fenced examples often contain lines like `@decorator`; those
            // should remain body text instead of creating new block tags.
            if self._options.fence_aware && trimmed.starts_with("```") {
                in_fence = !in_fence;
            }
        }

        if let Some(section) = current_tag {
            tag_sections.push(section);
        }

        (description_lines, tag_sections)
    }

    fn parse_tag_sections(&mut self, sections: &[TagSection<'a>]) -> ArenaVec<'a, BlockTag<'a>> {
        let mut tags = ArenaVec::new_in(self.allocator);
        for section in sections {
            tags.push(self.parse_block_tag(section));
        }
        tags
    }

    fn parse_block_tag(&mut self, section: &TagSection<'a>) -> BlockTag<'a> {
        // Preserve raw body separately from the structured parse so validators
        // and downstream tools can inspect the exact post-tag text.
        let raw_body = self.normalize_lines(&section.body_lines).map(|normalized| {
            ArenaBox::new_in(
                Text {
                    span: normalized.span,
                    value: normalized.text,
                },
                self.allocator,
            )
        });

        let body = self
            .normalize_lines(&section.body_lines)
            .map(|normalized| self.parse_generic_tag_body(normalized));

        BlockTag {
            span: Span::new(section.tag_name_start, section.end),
            tag_name: TagName {
                span: Span::new(section.tag_name_start, section.tag_name_end),
                value: section.tag_name,
            },
            body: body.map(|body| {
                ArenaBox::new_in(
                    BlockTagBody::Generic(ArenaBox::new_in(body, self.allocator)),
                    self.allocator,
                )
            }),
            raw_body,
        }
    }

    fn parse_generic_tag_body(&mut self, normalized: NormalizedText<'a>) -> GenericTagBody<'a> {
        let mut cursor = 0usize;
        let bytes = normalized.text.as_bytes();

        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        // `{...}` is optional and may contain nested braces. If it is malformed,
        // report the diagnostic and continue parsing the rest as value/text.
        let type_expression = if bytes.get(cursor) == Some(&b'{') {
            match find_matching_type_end(normalized.text, cursor) {
                Some(end) => {
                    let raw = &normalized.text[cursor + 1..end];
                    let span = relative_span(normalized.span, cursor as u32, (end + 1) as u32);
                    cursor = end + 1;
                    Some(ArenaBox::new_in(
                        TypeExpression { span, raw },
                        self.allocator,
                    ))
                }
                None => {
                    self.diagnostics
                        .push(diagnostic(ParserDiagnosticKind::UnclosedTypeExpression));
                    None
                }
            }
        } else {
            None
        };

        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        // The first token after the type is treated as the tag value. Optional
        // parameters use bracket syntax and may contain whitespace-free `=`.
        let token_end = find_value_end(normalized.text, cursor);
        let value = if token_end > cursor {
            let token = &normalized.text[cursor..token_end];
            let span = relative_span(normalized.span, cursor as u32, token_end as u32);
            cursor = token_end;
            Some(ArenaBox::new_in(
                parse_tag_value_token(token, span),
                self.allocator,
            ))
        } else {
            None
        };

        // JSDoc descriptions commonly use `-` as a separator after the value.
        let mut remainder = normalized.text[cursor..].trim_start();
        if let Some(rest) = remainder.strip_prefix("- ") {
            remainder = rest;
        } else if remainder == "-" {
            remainder = "";
        }

        let description = self.parse_description_text(remainder, normalized.span);

        GenericTagBody {
            span: normalized.span,
            type_expression,
            value,
            description,
        }
    }

    fn parse_description_lines(
        &mut self,
        lines: &[scanner::LogicalLine<'a>],
    ) -> Option<ArenaBox<'a, Description<'a>>> {
        let normalized = self.normalize_lines(lines)?;
        self.parse_description_text(normalized.text, normalized.span)
    }

    fn parse_description_text(
        &mut self,
        text: &'a str,
        span: Span,
    ) -> Option<ArenaBox<'a, Description<'a>>> {
        if text.trim().is_empty() {
            return None;
        }

        let mut parts = ArenaVec::new_in(self.allocator);
        let mut cursor = 0usize;

        while let Some(relative_start) = text[cursor..].find("{@") {
            let inline_start = cursor + relative_start;
            if inline_start > cursor {
                let value = self.allocator.alloc_str(&text[cursor..inline_start]);
                parts.push(DescriptionPart::Text(ArenaBox::new_in(
                    Text {
                        span: relative_span(span, cursor as u32, inline_start as u32),
                        value,
                    },
                    self.allocator,
                )));
            }

            let Some(relative_end) = text[inline_start + 2..].find('}') else {
                // Recovery strategy: keep the unterminated inline tag as text
                // so consumers do not lose source content.
                self.diagnostics
                    .push(diagnostic(ParserDiagnosticKind::UnclosedInlineTag));
                let value = self.allocator.alloc_str(&text[inline_start..]);
                parts.push(DescriptionPart::Text(ArenaBox::new_in(
                    Text {
                        span: relative_span(span, inline_start as u32, text.len() as u32),
                        value,
                    },
                    self.allocator,
                )));
                cursor = text.len();
                break;
            };

            let inline_end = inline_start + 2 + relative_end;
            let inside = &text[inline_start + 2..inline_end];
            let Some((tag_name, body)) = parse_inline_tag_header(inside) else {
                // Invalid inline tag start is also recovered as literal text,
                // then parsing resumes after `{@`.
                self.diagnostics
                    .push(diagnostic(ParserDiagnosticKind::InvalidInlineTagStart));
                let value = self.allocator.alloc_str("{@");
                parts.push(DescriptionPart::Text(ArenaBox::new_in(
                    Text {
                        span: relative_span(span, inline_start as u32, (inline_start + 2) as u32),
                        value,
                    },
                    self.allocator,
                )));
                cursor = inline_start + 2;
                continue;
            };

            let inline_span = relative_span(span, inline_start as u32, (inline_end + 1) as u32);
            let tag_name_start = inline_start + 2;
            let tag_name_end = tag_name_start + tag_name.len();
            let body_node = if body.is_empty() {
                None
            } else {
                let body_start = inline_end + 1 - body.len();
                Some(ArenaBox::new_in(
                    InlineTagBody {
                        span: relative_span(span, body_start as u32, inline_end as u32),
                        raw: self.allocator.alloc_str(body),
                    },
                    self.allocator,
                ))
            };

            parts.push(DescriptionPart::InlineTag(ArenaBox::new_in(
                InlineTag {
                    span: inline_span,
                    tag_name: TagName {
                        span: relative_span(span, tag_name_start as u32, tag_name_end as u32),
                        value: self.allocator.alloc_str(tag_name),
                    },
                    body: body_node,
                },
                self.allocator,
            )));

            cursor = inline_end + 1;
        }

        if cursor < text.len() {
            let value = self.allocator.alloc_str(&text[cursor..]);
            parts.push(DescriptionPart::Text(ArenaBox::new_in(
                Text {
                    span: relative_span(span, cursor as u32, text.len() as u32),
                    value,
                },
                self.allocator,
            )));
        }

        if parts.is_empty() {
            return None;
        }

        Some(ArenaBox::new_in(
            Description { span, parts },
            self.allocator,
        ))
    }

    fn normalize_lines(&self, lines: &[scanner::LogicalLine<'a>]) -> Option<NormalizedText<'a>> {
        // Drop empty edge lines but keep internal newlines. This gives consumers
        // compact text while retaining enough shape for multi-line descriptions.
        let first_index = lines
            .iter()
            .position(|line| !line.content.trim().is_empty())?;
        let last_index = lines
            .iter()
            .rposition(|line| !line.content.trim().is_empty())?;
        let lines = &lines[first_index..=last_index];
        let first = &lines[0];
        let last = &lines[lines.len() - 1];
        let span = Span::new(first.content_start, last.content_end);

        if lines.len() == 1 {
            return Some(NormalizedText {
                text: lines[0].content.trim_end(),
                span,
            });
        }

        let mut normalized = String::new();
        for (index, line) in lines.iter().enumerate() {
            if index > 0 {
                normalized.push('\n');
            }
            normalized.push_str(line.content.trim_end());
        }

        Some(NormalizedText {
            text: self.allocator.alloc_str(&normalized),
            span,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct NormalizedText<'a> {
    /// Normalized text, either borrowed from source or allocated in the arena.
    text: &'a str,
    /// Span covering the source range used to build `text`.
    span: Span,
}

/// Parse a block tag header from a logical line.
fn parse_tag_header(line: &str, line_start: u32) -> Option<(&str, u32, Option<(&str, u32)>)> {
    let stripped = line.strip_prefix('@')?;
    let name_len = stripped
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '!'))
        .map(char::len_utf8)
        .sum::<usize>();
    if name_len == 0 {
        return None;
    }

    let tag_name = &stripped[..name_len];
    let body = stripped[name_len..].trim_start();
    let body_start = if body.is_empty() {
        None
    } else {
        let body_delta = line.len() - body.len();
        Some((body, line_start + u32::try_from(body_delta).unwrap()))
    };

    Some((tag_name, line_start + 1, body_start))
}

/// Parse the `tagName body` content inside `{@...}`.
fn parse_inline_tag_header(inside: &str) -> Option<(&str, &str)> {
    let trimmed = inside.trim();
    let name_len = trimmed
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '!'))
        .map(char::len_utf8)
        .sum::<usize>();
    if name_len == 0 {
        return None;
    }

    let tag_name = &trimmed[..name_len];
    let body = trimmed[name_len..].trim();
    Some((tag_name, body))
}

/// Find the closing `}` for a type expression, accounting for nested braces.
fn find_matching_type_end(text: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in text.char_indices().skip(start) {
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

/// Find the end of the value token that follows an optional type expression.
fn find_value_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    if start >= bytes.len() {
        return start;
    }

    if bytes[start] == b'[' {
        let mut depth = 0usize;
        for (index, ch) in text[start..].char_indices() {
            if ch == '[' {
                depth += 1;
            } else if ch == ']' {
                depth -= 1;
                if depth == 0 {
                    return start + index + 1;
                }
            }
        }
        return text.len();
    }

    for (index, ch) in text[start..].char_indices() {
        if ch.is_whitespace() {
            return start + index;
        }
    }

    text.len()
}

/// Classify a tag value into the most useful AST token.
fn parse_tag_value_token<'a>(token: &'a str, span: Span) -> TagValueToken<'a> {
    if token.starts_with('[') && token.ends_with(']') {
        let inner = &token[1..token.len() - 1];
        let (path, default_value) = inner
            .split_once('=')
            .map_or((inner, None), |(path, value)| (path, Some(value)));
        return TagValueToken::Parameter(TagParameterName {
            span,
            path: ParameterPath { span, raw: path },
            optional: true,
            default_value,
        });
    }

    if token
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '.' | '[' | ']'))
    {
        return TagValueToken::Parameter(TagParameterName {
            span,
            path: ParameterPath { span, raw: token },
            optional: false,
            default_value: None,
        });
    }

    if token.contains(['.', '#', '~', '/', ':', '"', '\'', '(']) {
        return TagValueToken::NamePath(NamePathLike { span, raw: token });
    }

    TagValueToken::Raw(Text { span, value: token })
}

/// Convert offsets inside a normalized text span back to absolute spans.
fn relative_span(base: Span, relative_start: u32, relative_end: u32) -> Span {
    let start = base.start.saturating_add(relative_start);
    let end = base.start.saturating_add(relative_end);
    Span::new(
        start.min(base.end),
        end.min(base.end).max(start.min(base.end)),
    )
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;
    use oxc_span::Span;

    use crate::ast::TagValueToken;
    use crate::parser::diagnostic;

    use super::{
        ParseOptions, ParserContext, ParserDiagnosticKind, QuoteKind, find_matching_type_end,
        find_value_end, parse_inline_tag_header, parse_tag_header, parse_tag_value_token,
        relative_span, scanner,
    };

    fn context<'a>(allocator: &'a Allocator, source_text: &'a str) -> ParserContext<'a> {
        ParserContext::new(allocator, source_text, 100, ParseOptions::default())
    }

    #[test]
    fn checkpoint_and_rewind_restore_parser_state() {
        let allocator = Allocator::default();
        let mut context = context(&allocator, "/** ok */");

        context.offset = 4;
        context.brace_depth = 1;
        context.bracket_depth = 2;
        context.paren_depth = 3;
        context.quote = Some(QuoteKind::Double);
        context
            .diagnostics
            .push(diagnostic(ParserDiagnosticKind::UnclosedInlineTag));

        let checkpoint = context.checkpoint();

        context.offset = 9;
        context.brace_depth = 0;
        context.bracket_depth = 0;
        context.paren_depth = 0;
        context.quote = None;
        context
            .diagnostics
            .push(diagnostic(ParserDiagnosticKind::UnclosedTypeExpression));

        context.rewind(checkpoint);

        assert_eq!(context.offset, 4);
        assert_eq!(context.brace_depth, 1);
        assert_eq!(context.bracket_depth, 2);
        assert_eq!(context.paren_depth, 3);
        assert_eq!(context.quote, Some(QuoteKind::Double));
        assert_eq!(context.diagnostics.len(), 1);
        assert!(
            context.diagnostics[0]
                .to_string()
                .contains("inline tag is not closed")
        );
    }

    #[test]
    fn partitions_description_and_tag_sections() {
        let allocator = Allocator::default();
        let source = "/**\n * Intro\n * @example\n * ```ts\n * @decorator()\n * ```\n * @returns {void}\n */";
        let context = context(&allocator, source);
        let lines = scanner::logical_lines(source, 100);

        let (description_lines, tag_sections) = context.partition_sections(&lines);

        assert_eq!(description_lines.len(), 2);
        assert_eq!(description_lines[1].content, "Intro");

        assert_eq!(tag_sections.len(), 2);
        assert_eq!(tag_sections[0].tag_name, "example");
        assert_eq!(
            tag_sections[0]
                .body_lines
                .iter()
                .map(|line| line.content)
                .collect::<Vec<_>>(),
            vec!["```ts", "@decorator()", "```"]
        );
        assert_eq!(tag_sections[1].tag_name, "returns");
        assert_eq!(tag_sections[1].body_lines[0].content, "{void}");
    }

    #[test]
    fn parses_tag_headers_with_absolute_offsets() {
        let (tag_name, tag_start, body) =
            parse_tag_header("@param {string} id", 10).expect("expected tag header");

        assert_eq!(tag_name, "param");
        assert_eq!(tag_start, 11);
        assert_eq!(body, Some(("{string} id", 17)));

        assert_eq!(
            parse_inline_tag_header(" link   UserService "),
            Some(("link", "UserService"))
        );
        assert_eq!(parse_tag_header("not-a-tag", 10), None);
        assert_eq!(parse_inline_tag_header("   "), None);
    }

    #[test]
    fn finds_type_and_value_boundaries() {
        let text = "{Record<string, {id: number}>} options - desc";
        let type_end = find_matching_type_end(text, 0).expect("expected closing brace");

        assert_eq!(&text[1..type_end], "Record<string, {id: number}>");

        let value_start = type_end + 2;
        let value_end = find_value_end(text, value_start);
        assert_eq!(&text[value_start..value_end], "options");

        let optional = "[name=default] - desc";
        let optional_end = find_value_end(optional, 0);
        assert_eq!(&optional[..optional_end], "[name=default]");

        assert_eq!(find_matching_type_end("{Unclosed", 0), None);
    }

    #[test]
    fn classifies_tag_value_tokens() {
        let span = Span::new(10, 20);

        match parse_tag_value_token("[name=default]", span) {
            TagValueToken::Parameter(parameter) => {
                assert_eq!(parameter.path.raw, "name");
                assert!(parameter.optional);
                assert_eq!(parameter.default_value, Some("default"));
            }
            _ => panic!("expected optional parameter"),
        }

        match parse_tag_value_token("module:foo/bar", span) {
            TagValueToken::NamePath(name_path) => {
                assert_eq!(name_path.raw, "module:foo/bar");
            }
            _ => panic!("expected name path"),
        }

        match parse_tag_value_token("name-with-dash", span) {
            TagValueToken::Raw(text) => {
                assert_eq!(text.value, "name-with-dash");
            }
            _ => panic!("expected raw token"),
        }
    }

    #[test]
    fn clamps_relative_spans_to_base_span() {
        let base = Span::new(100, 110);

        assert_eq!(relative_span(base, 2, 6), Span::new(102, 106));
        assert_eq!(relative_span(base, 8, 50), Span::new(108, 110));
        assert_eq!(relative_span(base, 50, 60), Span::new(110, 110));
    }
}
