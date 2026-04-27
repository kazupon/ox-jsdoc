// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use oxc_allocator::{Allocator, Box as ArenaBox, Vec as ArenaVec};
use oxc_diagnostics::OxcDiagnostic;
use oxc_span::Span;

use crate::ast::{
    JsdocBlock, JsdocDescriptionLine, JsdocGenericTagBody, JsdocIdentifier, JsdocInlineTag,
    JsdocInlineTagFormat, JsdocNamepathSource, JsdocParameterName, JsdocSeparator, JsdocTag,
    JsdocTagBody, JsdocTagName, JsdocTagNameValue, JsdocTagValue, JsdocText, JsdocType,
    JsdocTypeLine, JsdocTypeSource,
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
    /// Logical lines that belong to the tag body (content only, no margin).
    body_lines: Vec<scanner::LogicalLine<'a>>,
    /// Absolute byte offset where the tag section ends.
    end: u32,
    /// Indentation before the `*` on the tag header line.
    header_initial: &'a str,
    /// The `*` delimiter on the tag header line.
    header_delimiter: &'a str,
    /// Whitespace after `*` on the tag header line.
    header_post_delimiter: &'a str,
    /// Line ending of the tag header line.
    header_line_end: &'a str,
}

/// Index range into the parallel lines/margins arrays for description lines.
#[derive(Debug, Clone, Copy)]
struct DescLineRange {
    /// Inclusive start index into ScanResult arrays.
    start: usize,
    /// Exclusive end index into ScanResult arrays.
    end: usize,
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
    pub(crate) options: ParseOptions,
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
            options,
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

        let scan = scanner::logical_lines(self.source_text, self.base_offset);
        let (desc_range, tag_sections) = self.partition_sections(&scan);
        let description = self.parse_description_lines(
            &scan.lines[desc_range.start..desc_range.end],
            &scan.margins[desc_range.start..desc_range.end],
        );
        let tags = self.parse_tag_sections(&tag_sections);

        // Line metadata
        let line_count = scan.lines.len() as u32;
        let end_line = if line_count > 0 { line_count - 1 } else { 0 };

        let delimiter_line_break = if scan.lines.len() <= 1 { "" } else { "\n" };

        let preterminal_line_break = if scan.lines.len() <= 1 {
            ""
        } else {
            if scan.margins[scan.lines.len() - 1].is_content_empty {
                "\n"
            } else {
                ""
            }
        };

        let block_line_end = if scan.margins.is_empty() {
            ""
        } else {
            scan.margins[0].line_end
        };

        // Description line indices
        let mut description_start_line: Option<u32> = None;
        let mut description_end_line: Option<u32> = None;
        let mut last_description_line: Option<u32> = None;
        let mut has_preterminal_description: u8 = 0;
        let mut has_preterminal_tag_description: Option<u8> = None;

        let has_tags = !tag_sections.is_empty();
        let desc_lines = &scan.margins[desc_range.start..desc_range.end];
        for (i, m) in desc_lines.iter().enumerate() {
            if !m.is_content_empty {
                let idx = i as u32;
                if description_start_line.is_none() {
                    description_start_line = Some(idx);
                }
                description_end_line = Some(idx);
            }
        }

        if has_tags {
            last_description_line = Some((desc_range.end - desc_range.start) as u32);
        } else if !scan.lines.is_empty() {
            last_description_line = Some(end_line);
        }

        if !scan.lines.is_empty() && !scan.margins[scan.lines.len() - 1].is_content_empty {
            if has_tags {
                has_preterminal_tag_description = Some(1);
            } else {
                has_preterminal_description = 1;
            }
        }

        let description_raw = self.slice_description_raw(&description.lines);

        let comment = ArenaBox::new_in(
            JsdocBlock {
                span: Span::new(self.base_offset, end),
                delimiter: "/**",
                post_delimiter: if delimiter_line_break.is_empty() && !scan.lines.is_empty() {
                    " "
                } else {
                    ""
                },
                terminal: "*/",
                line_end: block_line_end,
                initial: "",
                delimiter_line_break,
                preterminal_line_break,
                description: description.text,
                description_raw,
                description_lines: description.lines,
                tags,
                inline_tags: description.inline_tags,
                end_line,
                description_start_line,
                description_end_line,
                last_description_line,
                has_preterminal_description,
                has_preterminal_tag_description,
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

    /// Compute the `description_raw` slice for a node from its
    /// `description_lines`. Returns `None` when the line list is empty.
    ///
    /// The slice runs from the first description line's `span.start` to
    /// the last line's `span.end` — see
    /// `design/008-oxlint-oxfmt-support/README.md` §4.1 for the rationale
    /// (boundary covers blank intermediate lines + their `*` prefixes).
    fn slice_description_raw(
        &self,
        description_lines: &[crate::ast::JsdocDescriptionLine<'a>],
    ) -> Option<&'a str> {
        let first = description_lines.first()?;
        let last = description_lines.last()?;
        let start = first.span.start.checked_sub(self.base_offset)? as usize;
        let end = last.span.end.checked_sub(self.base_offset)? as usize;
        if start > end || end > self.source_text.len() {
            return None;
        }
        Some(&self.source_text[start..end])
    }

    /// Partition logical lines into description range + tag sections.
    /// Returns indices into scan arrays (not copies) for the description portion.
    fn partition_sections(
        &self,
        scan: &scanner::ScanResult<'a>,
    ) -> (DescLineRange, Vec<TagSection<'a>>) {
        let lines = &scan.lines;
        let margins = &scan.margins;
        let mut desc_end = 0usize; // exclusive end of description lines
        let mut tag_sections = Vec::new();
        let mut current_tag: Option<TagSection<'a>> = None;
        let mut in_fence = false;

        for (idx, line) in lines.iter().enumerate() {
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

                    let m = &margins[idx];
                    current_tag = Some(TagSection {
                        tag_name,
                        tag_name_start,
                        tag_name_end: tag_name_start + u32::try_from(tag_name.len()).unwrap(),
                        body_lines,
                        end: line.content_end,
                        header_initial: m.initial,
                        header_delimiter: m.delimiter,
                        header_post_delimiter: m.post_delimiter,
                        header_line_end: m.line_end,
                    });
                } else if let Some(section) = current_tag.as_mut() {
                    section.body_lines.push(*line);
                    section.end = line.content_end;
                } else {
                    desc_end = idx + 1;
                }
            } else if let Some(section) = current_tag.as_mut() {
                section.body_lines.push(*line);
                section.end = line.content_end;
            } else {
                desc_end = idx + 1;
            }

            if self.options.fence_aware && trimmed.starts_with("```") {
                in_fence = !in_fence;
            }
        }

        if let Some(section) = current_tag {
            tag_sections.push(section);
        }

        (
            DescLineRange {
                start: 0,
                end: desc_end,
            },
            tag_sections,
        )
    }

    fn parse_tag_sections(&mut self, sections: &[TagSection<'a>]) -> ArenaVec<'a, JsdocTag<'a>> {
        let mut tags = ArenaVec::new_in(self.allocator);
        for section in sections {
            tags.push(self.parse_jsdoc_tag(section));
        }
        tags
    }

    /// Parse a single block tag from its section, extracting type, name, and
    /// description into the arena-backed AST node.
    fn parse_jsdoc_tag(&mut self, section: &TagSection<'a>) -> JsdocTag<'a> {
        // Preserve raw body separately from the structured parse so validators
        // and downstream tools can inspect the exact post-tag text.
        let normalized = self.normalize_lines(&section.body_lines);
        let parsed_body = normalized.map(|normalized| self.parse_generic_tag_body(normalized));

        let (
            raw_type,
            name,
            optional,
            default_value,
            description,
            description_raw,
            type_lines,
            description_lines,
            inline_tags,
            body,
            raw_body,
        ) = if let Some(parsed_body) = parsed_body {
            // The synthetic `description.lines[0].span` (built via
            // `relative_span` in `parse_generic_tag_body`) has the correct
            // START offset but its END is short by `(line_breaks * margin_chars_lost)`
            // bytes for multi-line bodies (because `normalize_lines`
            // joins lines with single `\n`, dropping the original `\n  *  `
            // margin). For `description_raw` we therefore take START from
            // the synthetic line but END from the last non-blank
            // `body_line.content_end` to slice the correct original-source
            // range (boundary per design `008-…/README.md` §4.1).
            let description_raw = parsed_body.description.lines.first().and_then(|first| {
                let last_body_end = section
                    .body_lines
                    .iter()
                    .rev()
                    .find(|line| !line.content.bytes().all(|b| b == b' ' || b == b'\t'))?
                    .content_end;
                let s = first.span.start.checked_sub(self.base_offset)? as usize;
                let e = last_body_end.checked_sub(self.base_offset)? as usize;
                if s > e || e > self.source_text.len() {
                    return None;
                }
                Some(&self.source_text[s..e])
            });
            (
                parsed_body.raw_type,
                parsed_body.name,
                parsed_body.optional,
                parsed_body.default_value,
                parsed_body.description.text,
                description_raw,
                parsed_body.type_lines,
                parsed_body.description.lines,
                parsed_body.description.inline_tags,
                Some(ArenaBox::new_in(
                    JsdocTagBody::Generic(ArenaBox::new_in(parsed_body.body, self.allocator)),
                    self.allocator,
                )),
                Some(parsed_body.raw_body),
            )
        } else {
            (
                None,
                None,
                false,
                None,
                None,
                None,
                ArenaVec::new_in(self.allocator),
                ArenaVec::new_in(self.allocator),
                ArenaVec::new_in(self.allocator),
                None,
                None,
            )
        };

        // Parse type expression if enabled and raw_type is available
        let parsed_type = if self.options.parse_types {
            raw_type.and_then(|ts| {
                let mode = self.options.type_parse_mode;
                self.parse_type_expression(ts.raw, ts.span.start + 1, mode)
                    .map(|node| ArenaBox::new_in(JsdocType::Parsed(node), self.allocator))
            })
        } else {
            None
        };

        JsdocTag {
            span: Span::new(section.tag_name_start, section.end),
            tag: JsdocTagName {
                span: Span::new(section.tag_name_start, section.tag_name_end),
                value: section.tag_name,
            },
            raw_type,
            parsed_type,
            name,
            optional,
            default_value,
            description,
            description_raw,
            raw_body,
            delimiter: section.header_delimiter,
            post_delimiter: section.header_post_delimiter,
            initial: section.header_initial,
            line_end: section.header_line_end,
            post_tag: " ",
            post_type: " ",
            post_name: " ",
            type_lines,
            description_lines,
            inline_tags,
            body,
        }
    }

    /// Parse a generic tag body: optional `{type}`, optional value token,
    /// optional `-` separator, and remaining description text.
    fn parse_generic_tag_body(&mut self, normalized: NormalizedText<'a>) -> ParsedTagBody<'a> {
        let mut cursor = 0usize;
        let bytes = normalized.text.as_bytes();

        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        let type_source = if bytes.get(cursor) == Some(&b'{') {
            match find_matching_type_end(normalized.text, cursor) {
                Some(end) => {
                    let raw = &normalized.text[cursor + 1..end];
                    let span = relative_span(normalized.span, cursor as u32, (end + 1) as u32);
                    cursor = end + 1;
                    Some(JsdocTypeSource { span, raw })
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

        let mut type_lines = ArenaVec::new_in(self.allocator);
        if let Some(type_source) = type_source {
            type_lines.push(JsdocTypeLine {
                span: type_source.span,
                delimiter: "",
                post_delimiter: "",
                initial: "",
                raw_type: type_source.raw,
            });
        }

        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        let token_end = find_value_end(normalized.text, cursor);
        let value = if token_end > cursor {
            let token = &normalized.text[cursor..token_end];
            let span = relative_span(normalized.span, cursor as u32, token_end as u32);
            cursor = token_end;
            Some(parse_tag_value_token(token, span))
        } else {
            None
        };
        let (name, optional, default_value) = tag_value_name(value.as_ref());

        let mut separator = None;
        let mut remainder_start = cursor + leading_whitespace_len(&normalized.text[cursor..]);
        let mut remainder = &normalized.text[remainder_start..];
        if let Some(rest) = remainder.strip_prefix("- ") {
            separator = Some(JsdocSeparator::Dash);
            remainder_start += 2;
            remainder = rest;
        } else if remainder == "-" {
            separator = Some(JsdocSeparator::Dash);
            remainder_start = normalized.text.len();
            remainder = "";
        }

        let description_span = relative_span(
            normalized.span,
            remainder_start as u32,
            normalized.text.len() as u32,
        );
        let description = self.parse_description_text(remainder, description_span);
        let description_text = description.text;

        ParsedTagBody {
            raw_body: normalized.text,
            raw_type: type_source,
            name,
            optional,
            default_value,
            type_lines,
            description,
            body: JsdocGenericTagBody {
                span: normalized.span,
                type_source,
                value,
                separator,
                description: description_text,
            },
        }
    }

    /// Build description lines from scan results, using margin info for each line.
    fn parse_description_lines(
        &mut self,
        lines: &[scanner::LogicalLine<'a>],
        margins: &[scanner::MarginInfo<'a>],
    ) -> ParsedDescription<'a> {
        let mut description_lines = ArenaVec::new_in(self.allocator);
        for (line, margin) in lines.iter().zip(margins.iter()) {
            if margin.is_content_empty {
                continue;
            }
            description_lines.push(JsdocDescriptionLine {
                span: Span::new(line.content_start, line.content_end),
                delimiter: margin.delimiter,
                post_delimiter: margin.post_delimiter,
                initial: margin.initial,
                description: line.content.trim_end(),
            });
        }

        let Some(normalized) = self.normalize_lines(lines) else {
            return ParsedDescription {
                text: None,
                lines: description_lines,
                inline_tags: ArenaVec::new_in(self.allocator),
            };
        };
        let mut description = self.parse_description_text(normalized.text, normalized.span);
        description.lines = description_lines;
        description
    }

    /// Parse inline tags (`{@link ...}` etc.) from description text and wrap
    /// the result in a `ParsedDescription`.
    fn parse_description_text(&mut self, text: &'a str, span: Span) -> ParsedDescription<'a> {
        let mut lines = ArenaVec::new_in(self.allocator);
        let mut inline_tags = ArenaVec::new_in(self.allocator);

        if text
            .bytes()
            .all(|b| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r')
        {
            return ParsedDescription {
                text: None,
                lines,
                inline_tags,
            };
        }

        lines.push(JsdocDescriptionLine {
            span,
            delimiter: "",
            post_delimiter: "",
            initial: "",
            description: text,
        });
        let mut cursor = 0usize;

        while let Some(relative_start) = text[cursor..].find("{@") {
            let inline_start = cursor + relative_start;
            let Some(relative_end) = text[inline_start + 2..].find('}') else {
                self.diagnostics
                    .push(diagnostic(ParserDiagnosticKind::UnclosedInlineTag));
                break;
            };

            let inline_end = inline_start + 2 + relative_end;
            let inside = &text[inline_start + 2..inline_end];
            let Some((tag_name, body)) = parse_inline_tag_header(inside) else {
                self.diagnostics
                    .push(diagnostic(ParserDiagnosticKind::InvalidInlineTagStart));
                cursor = inline_start + 2;
                continue;
            };

            let inline_span = relative_span(span, inline_start as u32, (inline_end + 1) as u32);
            let tag_name_start = inline_start + 2;
            let tag_name_end = tag_name_start + tag_name.len();
            let (namepath_or_url, link_text, format) = parse_inline_tag_body(body);
            inline_tags.push(JsdocInlineTag {
                span: inline_span,
                tag: JsdocTagName {
                    span: relative_span(span, tag_name_start as u32, tag_name_end as u32),
                    value: self.allocator.alloc_str(tag_name),
                },
                namepath_or_url,
                text: link_text,
                format,
                raw_body: if body.is_empty() {
                    None
                } else {
                    Some(self.allocator.alloc_str(body))
                },
            });

            cursor = inline_end + 1;
        }

        ParsedDescription {
            text: Some(text),
            lines,
            inline_tags,
        }
    }

    /// Join logical lines into a single normalized text, dropping empty edge
    /// lines but keeping internal newlines. Returns `None` when all lines are
    /// empty or whitespace-only.
    fn normalize_lines(&self, lines: &[scanner::LogicalLine<'a>]) -> Option<NormalizedText<'a>> {
        let first_index = lines
            .iter()
            .position(|line| !line.content.bytes().all(|b| b == b' ' || b == b'\t'))?;
        let last_index = lines
            .iter()
            .rposition(|line| !line.content.bytes().all(|b| b == b' ' || b == b'\t'))?;
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

        // Pre-calculate capacity to avoid reallocations.
        let capacity: usize = lines.iter().map(|l| l.content.len() + 1).sum();
        let mut normalized = String::with_capacity(capacity);
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

/// Intermediate result from parsing description lines plus inline tags.
#[derive(Debug)]
struct ParsedDescription<'a> {
    /// Joined description text, or `None` when empty.
    text: Option<&'a str>,
    /// Source-preserving description line nodes.
    lines: ArenaVec<'a, JsdocDescriptionLine<'a>>,
    /// Inline tags found within the description text.
    inline_tags: ArenaVec<'a, JsdocInlineTag<'a>>,
}

/// Intermediate result from parsing a generic tag body.
#[derive(Debug)]
struct ParsedTagBody<'a> {
    /// Raw body text after the tag name.
    raw_body: &'a str,
    /// Extracted `{...}` type source, if present.
    raw_type: Option<JsdocTypeSource<'a>>,
    /// First value token interpreted as a name, if present.
    name: Option<JsdocTagNameValue<'a>>,
    /// Whether the name used optional bracket syntax.
    optional: bool,
    /// Default value from `[name=value]` syntax.
    default_value: Option<&'a str>,
    /// Source-preserving type lines.
    type_lines: ArenaVec<'a, JsdocTypeLine<'a>>,
    /// Parsed description (text + lines + inline tags).
    description: ParsedDescription<'a>,
    /// Structured body for the generic tag layout.
    body: JsdocGenericTagBody<'a>,
}

/// A normalized multi-line text span, either borrowed or arena-allocated.
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

fn parse_inline_tag_body(body: &str) -> (Option<&str>, Option<&str>, JsdocInlineTagFormat) {
    if body.is_empty() {
        return (None, None, JsdocInlineTagFormat::Plain);
    }

    if let Some((target, text)) = body.split_once('|') {
        return (
            non_empty_trimmed(target),
            non_empty_trimmed(text),
            JsdocInlineTagFormat::Pipe,
        );
    }

    if let Some((target, text)) = body.split_once(char::is_whitespace) {
        return (
            non_empty_trimmed(target),
            non_empty_trimmed(text),
            JsdocInlineTagFormat::Space,
        );
    }

    (Some(body), None, JsdocInlineTagFormat::Plain)
}

fn non_empty_trimmed(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn leading_whitespace_len(value: &str) -> usize {
    value.len() - value.trim_start().len()
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

fn parse_tag_value_token<'a>(token: &'a str, span: Span) -> JsdocTagValue<'a> {
    if token.starts_with('[') && token.ends_with(']') {
        let inner = &token[1..token.len() - 1];
        let (path, default_value) = inner
            .split_once('=')
            .map_or((inner, None), |(path, value)| (path, Some(value)));
        return JsdocTagValue::Parameter(JsdocParameterName {
            span,
            path,
            optional: true,
            default_value,
        });
    }

    if token
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '.' | '[' | ']'))
    {
        return JsdocTagValue::Parameter(JsdocParameterName {
            span,
            path: token,
            optional: false,
            default_value: None,
        });
    }

    if token.contains(['.', '#', '~', '/', ':', '"', '\'', '(']) {
        return JsdocTagValue::Namepath(JsdocNamepathSource { span, raw: token });
    }

    if token
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$'))
    {
        return JsdocTagValue::Identifier(JsdocIdentifier { span, name: token });
    }

    JsdocTagValue::Raw(JsdocText { span, value: token })
}

fn tag_value_name<'a>(
    value: Option<&JsdocTagValue<'a>>,
) -> (Option<JsdocTagNameValue<'a>>, bool, Option<&'a str>) {
    match value {
        Some(JsdocTagValue::Parameter(parameter)) => (
            Some(JsdocTagNameValue {
                span: parameter.span,
                raw: parameter.path,
            }),
            parameter.optional,
            parameter.default_value,
        ),
        Some(JsdocTagValue::Namepath(namepath)) => (
            Some(JsdocTagNameValue {
                span: namepath.span,
                raw: namepath.raw,
            }),
            false,
            None,
        ),
        Some(JsdocTagValue::Identifier(identifier)) => (
            Some(JsdocTagNameValue {
                span: identifier.span,
                raw: identifier.name,
            }),
            false,
            None,
        ),
        Some(JsdocTagValue::Raw(text)) => (
            Some(JsdocTagNameValue {
                span: text.span,
                raw: text.value,
            }),
            false,
            None,
        ),
        None => (None, false, None),
    }
}

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

    use crate::ast::JsdocTagValue;
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
        let scan = scanner::logical_lines(source, 100);

        let (desc_range, tag_sections) = context.partition_sections(&scan);

        assert_eq!(desc_range.end - desc_range.start, 2);
        assert_eq!(scan.lines[1].content, "Intro");

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
            JsdocTagValue::Parameter(parameter) => {
                assert_eq!(parameter.path, "name");
                assert!(parameter.optional);
                assert_eq!(parameter.default_value, Some("default"));
            }
            _ => panic!("expected optional parameter"),
        }

        match parse_tag_value_token("module:foo/bar", span) {
            JsdocTagValue::Namepath(name_path) => {
                assert_eq!(name_path.raw, "module:foo/bar");
            }
            _ => panic!("expected name path"),
        }

        match parse_tag_value_token("name-with-dash", span) {
            JsdocTagValue::Raw(text) => {
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
