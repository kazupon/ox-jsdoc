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

#[derive(Debug, Clone)]
struct TagSection<'a> {
    tag_name: &'a str,
    tag_name_start: u32,
    tag_name_end: u32,
    body_lines: Vec<scanner::LogicalLine<'a>>,
    end: u32,
}

pub struct ParserContext<'a> {
    pub(crate) allocator: &'a Allocator,
    pub(crate) source_text: &'a str,
    pub(crate) base_offset: u32,
    pub(crate) offset: u32,
    pub(crate) _options: ParseOptions,
    pub(crate) diagnostics: Vec<OxcDiagnostic>,
    pub(crate) brace_depth: u16,
    pub(crate) bracket_depth: u16,
    pub(crate) paren_depth: u16,
    pub(crate) quote: Option<QuoteKind>,
    pub(crate) fence: Option<FenceState>,
}

impl<'a> ParserContext<'a> {
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

    pub fn rewind(&mut self, checkpoint: Checkpoint) {
        self.offset = checkpoint.offset;
        self.brace_depth = checkpoint.brace_depth;
        self.bracket_depth = checkpoint.bracket_depth;
        self.paren_depth = checkpoint.paren_depth;
        self.quote = checkpoint.quote;
        self.fence = checkpoint.fence;
        self.diagnostics.truncate(checkpoint.diagnostics_len);
    }

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
    text: &'a str,
    span: Span,
}

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

fn relative_span(base: Span, relative_start: u32, relative_end: u32) -> Span {
    let start = base.start.saturating_add(relative_start);
    let end = base.start.saturating_add(relative_end);
    Span::new(
        start.min(base.end),
        end.min(base.end).max(start.min(base.end)),
    )
}
