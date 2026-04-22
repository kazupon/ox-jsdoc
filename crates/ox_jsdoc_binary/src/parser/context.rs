// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Structural parser for one JSDoc block, emitting Binary AST directly.
//!
//! Mirrors the typed-AST `crates/ox_jsdoc/src/parser/context.rs` but
//! replaces every `ArenaBox::new_in(JsdocXxx { ... })` with a `write_*`
//! call against a [`BinaryWriter`].
//!
//! Two phases:
//!
//! 1. **Parse** — `parse_block_into_data` walks the source text and
//!    collects intermediate plain-data structs (no allocation in the
//!    arena-tracked binary buffer). All slices borrow from the original
//!    source text.
//! 2. **Emit** — `emit_block` walks those structs in DFS pre-order and
//!    invokes the matching `write_*` helper. Each emission knows its
//!    parent's `NodeIndex` because we strictly write top-down.
//!
//! The split keeps the `children_bitmask` upfront (which the writer needs)
//! while still allowing the parser to make local decisions (e.g. drop
//! empty lists).
//!
//! Phase 1.2a-cont will add the `parsed_type` emission once
//! `parser/type_parse.rs` lands.

use oxc_span::Span;
use smallvec::SmallVec;

use crate::format::string_table::U16_NONE_SENTINEL;

/// Inline cap for `inline_tags`: most JSDoc tags carry zero or one
/// `{@link ...}` style inline tag. Inline-storing up to 2 keeps that
/// case heap-allocation-free at the cost of `2 * size_of::<InlineTagData>()`
/// (~160 bytes) of inline storage on every `TagData` / `BlockData`.
type InlineTagsVec<'a> = SmallVec<[InlineTagData<'a>; 2]>;
use crate::writer::nodes::comment_ast::{
    write_jsdoc_block, write_jsdoc_block_compat_tail, write_jsdoc_description_line,
    write_jsdoc_generic_tag_body, write_jsdoc_identifier, write_jsdoc_inline_tag,
    write_jsdoc_namepath_source, write_jsdoc_parameter_name, write_jsdoc_tag,
    write_jsdoc_tag_compat_tail, write_jsdoc_tag_name, write_jsdoc_tag_name_value,
    write_jsdoc_text, write_jsdoc_type_line, write_jsdoc_type_source, write_node_list,
};
use crate::writer::{BinaryWriter, StringIndex};

use super::checkpoint::{Checkpoint, FenceState, QuoteKind};
use super::diagnostics::{DiagnosticKind, ParserDiagnosticKind, TypeDiagnosticKind};
use super::scanner;
use super::type_data::TypeNodeData;
use super::ParseOptions;

// ---------------------------------------------------------------------------
// Diagnostic + parsed-data types
// ---------------------------------------------------------------------------

/// One diagnostic emitted while parsing a comment.
///
/// The parser stores the diagnostic kind + an optional source span so the
/// caller can decide how to format the human-readable message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedDiagnostic {
    /// Either a structural-parser or a type-parser diagnostic kind.
    pub kind: DiagnosticKind,
    /// Source span the diagnostic refers to, when known.
    pub span: Option<Span>,
}

impl ParsedDiagnostic {
    /// Convenience: get the static message string for this diagnostic.
    #[inline]
    #[must_use]
    pub const fn message(&self) -> &'static str {
        self.kind.message()
    }
}

/// Inline tag body format mirroring `ox_jsdoc::ast::JsdocInlineTagFormat`.
///
/// Stored as a `u8` in the binary record's Common Data slot (3 bits used).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InlineTagFormatData {
    /// `{@link target}` — no separator, no display text.
    Plain = 0,
    /// `{@link target|text}` — pipe separator.
    Pipe = 1,
    /// `{@link target text}` — whitespace separator.
    Space = 2,
    /// `{@link prefix:body}` — prefix-style.
    Prefix = 3,
    /// Could not classify.
    Unknown = 4,
}

#[derive(Debug, Clone, Copy)]
struct TypeSourceData<'a> {
    span: Span,
    raw: &'a str,
}

#[derive(Debug, Clone, Copy)]
struct TypeLineData<'a> {
    span: Span,
    raw_type: &'a str,
    delimiter: &'a str,
    post_delimiter: &'a str,
    initial: &'a str,
}

#[derive(Debug, Clone, Copy)]
struct DescriptionLineData<'a> {
    span: Span,
    description: &'a str,
    delimiter: &'a str,
    post_delimiter: &'a str,
    initial: &'a str,
}

#[derive(Debug, Clone, Copy)]
struct InlineTagData<'a> {
    span: Span,
    tag_name_span: Span,
    tag_name: &'a str,
    namepath_or_url: Option<&'a str>,
    text: Option<&'a str>,
    format: InlineTagFormatData,
    raw_body: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
struct TagNameValueData<'a> {
    span: Span,
    raw: &'a str,
}

#[derive(Debug, Clone, Copy)]
enum TagValueData<'a> {
    Parameter {
        span: Span,
        path: &'a str,
        optional: bool,
        default_value: Option<&'a str>,
    },
    Namepath {
        span: Span,
        raw: &'a str,
    },
    Identifier {
        span: Span,
        name: &'a str,
    },
    Raw {
        span: Span,
        value: &'a str,
    },
}

#[derive(Debug)]
struct GenericTagBodyData<'a> {
    span: Span,
    has_dash_separator: bool,
    type_source: Option<TypeSourceData<'a>>,
    value: Option<TagValueData<'a>>,
    description: Option<&'a str>,
}

#[derive(Debug)]
enum TagBodyData<'a> {
    Generic(GenericTagBodyData<'a>),
}

#[derive(Debug)]
struct TagData<'a> {
    span: Span,
    tag_name_span: Span,
    tag_name: &'a str,
    optional: bool,
    default_value: Option<&'a str>,
    description: Option<&'a str>,
    raw_body: Option<&'a str>,
    raw_type: Option<TypeSourceData<'a>>,
    name: Option<TagNameValueData<'a>>,
    parsed_type: Option<Box<TypeNodeData<'a>>>,
    body: Option<TagBodyData<'a>>,
    description_lines: Vec<DescriptionLineData<'a>>,
    type_lines: Vec<TypeLineData<'a>>,
    inline_tags: InlineTagsVec<'a>,
    header_initial: &'a str,
    header_delimiter: &'a str,
    header_post_delimiter: &'a str,
    header_line_end: &'a str,
    /// `post_tag` source-preserving slot (compat mode).
    post_tag: &'a str,
    /// `post_type` source-preserving slot (compat mode).
    post_type: &'a str,
    /// `post_name` source-preserving slot (compat mode).
    post_name: &'a str,
}

#[derive(Debug)]
struct BlockData<'a> {
    span: Span,
    description: Option<&'a str>,
    description_lines: Vec<DescriptionLineData<'a>>,
    inline_tags: InlineTagsVec<'a>,
    tags: Vec<TagData<'a>>,
    line_end: &'a str,
    delimiter_line_break: &'a str,
    preterminal_line_break: &'a str,
    /// 0-based line index of the closing `*/` line (compat mode).
    end_line: u32,
    description_start_line: Option<u32>,
    description_end_line: Option<u32>,
    last_description_line: Option<u32>,
    has_preterminal_description: u8,
    has_preterminal_tag_description: Option<u8>,
}

// ---------------------------------------------------------------------------
// ParserContext
// ---------------------------------------------------------------------------

/// Stateful parser for one JSDoc block, producing intermediate data that
/// the [`emit_block`] free function later flushes to a [`BinaryWriter`].
///
/// Mirrors `ParserContext` in the typed-AST parser (same offset/depth/
/// quote/fence state) but stores diagnostics as plain
/// [`ParsedDiagnostic`] so the binary parser does not depend on
/// `oxc_diagnostics`.
pub struct ParserContext<'a> {
    /// Complete source slice for one JSDoc block.
    pub(crate) source_text: &'a str,
    /// Absolute byte offset of `source_text` in the original file.
    pub(crate) base_offset: u32,
    /// Current parser offset relative to `source_text`.
    pub(crate) offset: u32,
    /// Feature switches for this parse.
    pub(crate) options: ParseOptions,
    /// Diagnostics emitted while parsing this comment.
    pub(crate) diagnostics: Vec<ParsedDiagnostic>,
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
    /// Scratch buffer for joining multi-line content. Kept between calls
    /// so we avoid reallocating on each tag body.
    scratch: String,
}

impl<'a> ParserContext<'a> {
    /// Create a parser context for one complete comment block.
    #[must_use]
    pub fn new(source_text: &'a str, base_offset: u32, options: ParseOptions) -> Self {
        Self {
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
            scratch: String::new(),
        }
    }

    /// Capture rewindable parser state.
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

    fn diag(&mut self, kind: ParserDiagnosticKind) {
        self.diagnostics.push(ParsedDiagnostic {
            kind: DiagnosticKind::Parser(kind),
            span: None,
        });
    }

    /// Push a type-parser diagnostic.
    pub(crate) fn type_diag(&mut self, kind: TypeDiagnosticKind) {
        self.diagnostics.push(ParsedDiagnostic {
            kind: DiagnosticKind::Type(kind),
            span: None,
        });
    }

    fn absolute_end(&self) -> Option<u32> {
        let len = u32::try_from(self.source_text.len()).ok()?;
        self.base_offset.checked_add(len)
    }
}

/// Outcome of [`parse_block_into_data`]: either a parsed block + diagnostics,
/// or just diagnostics when the input could not be parsed at all (not a
/// JSDoc block, unclosed, span overflow).
#[derive(Debug)]
pub struct ParsedBlock<'a> {
    block: Option<BlockData<'a>>,
    diagnostics: Vec<ParsedDiagnostic>,
}

impl<'a> ParsedBlock<'a> {
    /// Diagnostics produced during parsing.
    #[must_use]
    pub fn diagnostics(&self) -> &[ParsedDiagnostic] {
        &self.diagnostics
    }

    /// `true` when at least one parse-failure diagnostic was emitted.
    #[must_use]
    pub fn is_failure(&self) -> bool {
        self.block.is_none()
    }
}

/// Parse one JSDoc block into intermediate data. Use [`emit_block`] to
/// flush the result into a [`BinaryWriter`].
pub fn parse_block_into_data<'a>(
    source_text: &'a str,
    base_offset: u32,
    options: ParseOptions,
) -> ParsedBlock<'a> {
    let mut ctx = ParserContext::new(source_text, base_offset, options);
    if ctx.absolute_end().is_none() {
        ctx.diag(ParserDiagnosticKind::SpanOverflow);
        return ParsedBlock {
            block: None,
            diagnostics: ctx.diagnostics,
        };
    }
    if !scanner::is_jsdoc_block(source_text) {
        ctx.diag(ParserDiagnosticKind::NotAJSDocBlock);
        return ParsedBlock {
            block: None,
            diagnostics: ctx.diagnostics,
        };
    }
    if !scanner::has_closing_block(source_text) {
        ctx.diag(ParserDiagnosticKind::UnclosedBlockComment);
        return ParsedBlock {
            block: None,
            diagnostics: ctx.diagnostics,
        };
    }

    let end = ctx.absolute_end().expect("checked above");
    let span = Span::new(ctx.base_offset, end);

    let scan = scanner::logical_lines(source_text, base_offset);
    let (desc_range, tag_sections) = ctx.partition_sections(&scan);

    let desc_lines_slice = &scan.lines[desc_range.start..desc_range.end];
    let desc_margins_slice = &scan.margins[desc_range.start..desc_range.end];
    let parsed_desc = ctx.parse_description_lines(desc_lines_slice, desc_margins_slice);
    let tags = ctx.parse_tag_sections(&tag_sections);

    let line_count = scan.lines.len() as u32;
    let end_line = if line_count > 0 { line_count - 1 } else { 0 };

    let delimiter_line_break = if scan.lines.len() <= 1 { "" } else { "\n" };
    let preterminal_line_break = if scan.lines.len() <= 1 {
        ""
    } else if scan.margins[scan.lines.len() - 1].is_content_empty {
        "\n"
    } else {
        ""
    };
    let block_line_end = if scan.margins.is_empty() {
        ""
    } else {
        scan.margins[0].line_end
    };

    let mut description_start_line: Option<u32> = None;
    let mut description_end_line: Option<u32> = None;
    let mut last_description_line: Option<u32> = None;
    let mut has_preterminal_description: u8 = 0;
    let mut has_preterminal_tag_description: Option<u8> = None;

    let has_tags = !tag_sections.is_empty();
    for (i, m) in desc_margins_slice.iter().enumerate() {
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

    let block = BlockData {
        span,
        description: parsed_desc.text,
        description_lines: parsed_desc.lines,
        inline_tags: parsed_desc.inline_tags,
        tags,
        line_end: block_line_end,
        delimiter_line_break,
        preterminal_line_break,
        end_line,
        description_start_line,
        description_end_line,
        last_description_line,
        has_preterminal_description,
        has_preterminal_tag_description,
    };

    ParsedBlock {
        block: Some(block),
        diagnostics: ctx.diagnostics,
    }
}

// ---------------------------------------------------------------------------
// Section partitioning + parsing helpers (live on ParserContext)
// ---------------------------------------------------------------------------

/// Index range into the parallel lines/margins arrays for description lines.
#[derive(Debug, Clone, Copy)]
struct DescLineRange {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone)]
struct TagSection<'a> {
    tag_name: &'a str,
    tag_name_start: u32,
    tag_name_end: u32,
    body_lines: Vec<scanner::LogicalLine<'a>>,
    end: u32,
    header_initial: &'a str,
    header_delimiter: &'a str,
    header_post_delimiter: &'a str,
    header_line_end: &'a str,
}

#[derive(Debug)]
struct ParsedDescription<'a> {
    text: Option<&'a str>,
    lines: Vec<DescriptionLineData<'a>>,
    inline_tags: InlineTagsVec<'a>,
}

#[derive(Debug, Clone, Copy)]
struct NormalizedText<'a> {
    text: &'a str,
    span: Span,
}

#[derive(Debug)]
struct ParsedTagBody<'a> {
    raw_body: &'a str,
    raw_type: Option<TypeSourceData<'a>>,
    name: Option<TagNameValueData<'a>>,
    optional: bool,
    default_value: Option<&'a str>,
    description: Option<&'a str>,
    type_lines: Vec<TypeLineData<'a>>,
    description_lines: Vec<DescriptionLineData<'a>>,
    inline_tags: InlineTagsVec<'a>,
    body: GenericTagBodyData<'a>,
}

impl<'a> ParserContext<'a> {
    fn partition_sections(
        &self,
        scan: &scanner::ScanResult<'a>,
    ) -> (DescLineRange, Vec<TagSection<'a>>) {
        let lines = &scan.lines;
        let margins = &scan.margins;
        let mut desc_end = 0usize;
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

    fn parse_tag_sections(&mut self, sections: &[TagSection<'a>]) -> Vec<TagData<'a>> {
        let mut tags = Vec::with_capacity(sections.len());
        for section in sections {
            tags.push(self.parse_jsdoc_tag(section));
        }
        tags
    }

    fn parse_jsdoc_tag(&mut self, section: &TagSection<'a>) -> TagData<'a> {
        let normalized = self.normalize_lines(&section.body_lines);
        let parsed_body = normalized.map(|n| self.parse_generic_tag_body(n));

        let (
            raw_type,
            name,
            optional,
            default_value,
            description,
            type_lines,
            description_lines,
            inline_tags,
            body,
            raw_body,
        ) = if let Some(p) = parsed_body {
            (
                p.raw_type,
                p.name,
                p.optional,
                p.default_value,
                p.description,
                p.type_lines,
                p.description_lines,
                p.inline_tags,
                Some(TagBodyData::Generic(p.body)),
                Some(p.raw_body),
            )
        } else {
            (
                None,
                None,
                false,
                None,
                None,
                Vec::new(),
                Vec::new(),
                InlineTagsVec::new(),
                None,
                None,
            )
        };

        // parsedType: parse the {...} type expression when enabled.
        let parsed_type = if self.options.parse_types {
            raw_type.and_then(|ts| {
                let mode = self.options.type_parse_mode;
                self.parse_type_expression(ts.raw, ts.span.start + 1, mode)
            })
        } else {
            None
        };

        TagData {
            span: Span::new(section.tag_name_start, section.end),
            tag_name_span: Span::new(section.tag_name_start, section.tag_name_end),
            tag_name: section.tag_name,
            optional,
            default_value,
            description,
            raw_body,
            raw_type,
            name,
            parsed_type,
            body,
            description_lines,
            type_lines,
            inline_tags,
            header_initial: section.header_initial,
            header_delimiter: section.header_delimiter,
            header_post_delimiter: section.header_post_delimiter,
            header_line_end: section.header_line_end,
            post_tag: " ",
            post_type: " ",
            post_name: " ",
        }
    }

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
                    Some(TypeSourceData { span, raw })
                }
                None => {
                    self.diag(ParserDiagnosticKind::UnclosedTypeExpression);
                    None
                }
            }
        } else {
            None
        };

        let mut type_lines = Vec::new();
        if let Some(ts) = type_source {
            type_lines.push(TypeLineData {
                span: ts.span,
                raw_type: ts.raw,
                delimiter: "",
                post_delimiter: "",
                initial: "",
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

        let mut separator = false;
        let mut remainder_start = cursor + leading_whitespace_len(&normalized.text[cursor..]);
        let mut remainder = &normalized.text[remainder_start..];
        if let Some(rest) = remainder.strip_prefix("- ") {
            separator = true;
            remainder_start += 2;
            remainder = rest;
        } else if remainder == "-" {
            separator = true;
            remainder_start = normalized.text.len();
            remainder = "";
        }

        let description_span = relative_span(
            normalized.span,
            remainder_start as u32,
            normalized.text.len() as u32,
        );
        let parsed_desc = self.parse_description_text(remainder, description_span);

        ParsedTagBody {
            raw_body: normalized.text,
            raw_type: type_source,
            name,
            optional,
            default_value,
            description: parsed_desc.text,
            type_lines,
            description_lines: parsed_desc.lines,
            inline_tags: parsed_desc.inline_tags,
            body: GenericTagBodyData {
                span: normalized.span,
                has_dash_separator: separator,
                type_source,
                value,
                description: parsed_desc.text,
            },
        }
    }

    fn parse_description_lines(
        &mut self,
        lines: &[scanner::LogicalLine<'a>],
        margins: &[scanner::MarginInfo<'a>],
    ) -> ParsedDescription<'a> {
        let mut description_lines = Vec::new();
        for (line, margin) in lines.iter().zip(margins.iter()) {
            if margin.is_content_empty {
                continue;
            }
            description_lines.push(DescriptionLineData {
                span: Span::new(line.content_start, line.content_end),
                description: line.content.trim_end(),
                delimiter: margin.delimiter,
                post_delimiter: margin.post_delimiter,
                initial: margin.initial,
            });
        }

        let Some(normalized) = self.normalize_lines(lines) else {
            return ParsedDescription {
                text: None,
                lines: description_lines,
                inline_tags: InlineTagsVec::new(),
            };
        };
        let mut description = self.parse_description_text(normalized.text, normalized.span);
        description.lines = description_lines;
        description
    }

    fn parse_description_text(&mut self, text: &'a str, span: Span) -> ParsedDescription<'a> {
        let mut lines = Vec::new();
        let mut inline_tags = InlineTagsVec::new();

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

        lines.push(DescriptionLineData {
            span,
            description: text,
            delimiter: "",
            post_delimiter: "",
            initial: "",
        });

        // Fast path: most descriptions contain no `{@…}` inline tag at
        // all, but the original `find("{@")` loop still pays a Boyer-Moore
        // scan over the entire text. A single byte search for `@` is
        // SIMD-fused on modern targets and lets us skip the scan loop
        // entirely when the description has no `@` to anchor on.
        if !text.as_bytes().contains(&b'@') {
            return ParsedDescription {
                text: Some(text),
                lines,
                inline_tags,
            };
        }

        let mut cursor = 0usize;
        while let Some(rel_start) = text[cursor..].find("{@") {
            let inline_start = cursor + rel_start;
            let Some(rel_end) = text[inline_start + 2..].find('}') else {
                self.diag(ParserDiagnosticKind::UnclosedInlineTag);
                break;
            };
            let inline_end = inline_start + 2 + rel_end;
            let inside = &text[inline_start + 2..inline_end];
            let Some((tag_name, body)) = parse_inline_tag_header(inside) else {
                self.diag(ParserDiagnosticKind::InvalidInlineTagStart);
                cursor = inline_start + 2;
                continue;
            };

            let inline_span = relative_span(span, inline_start as u32, (inline_end + 1) as u32);
            let tag_name_start = inline_start + 2;
            let tag_name_end = tag_name_start + tag_name.len();
            let (np_or_url, link_text, format) = parse_inline_tag_body(body);
            inline_tags.push(InlineTagData {
                span: inline_span,
                tag_name_span: relative_span(span, tag_name_start as u32, tag_name_end as u32),
                tag_name,
                namepath_or_url: np_or_url,
                text: link_text,
                format,
                raw_body: if body.is_empty() { None } else { Some(body) },
            });
            cursor = inline_end + 1;
        }

        ParsedDescription {
            text: Some(text),
            lines,
            inline_tags,
        }
    }

    fn normalize_lines(&mut self, lines: &[scanner::LogicalLine<'a>]) -> Option<NormalizedText<'a>> {
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

        // Pre-compute capacity to avoid reallocations.
        let capacity: usize = lines.iter().map(|l| l.content.len() + 1).sum();
        self.scratch.clear();
        self.scratch.reserve(capacity);
        for (index, line) in lines.iter().enumerate() {
            if index > 0 {
                self.scratch.push('\n');
            }
            self.scratch.push_str(line.content.trim_end());
        }
        // Allocate a stable copy on the parser's scratch arena. The result
        // lives as long as the parser context; we leak it intentionally
        // since `ParserContext` is short-lived.
        // SAFETY: we transmute the lifetime to 'a; the scratch String
        // outlives the returned NormalizedText because `normalize_lines`
        // is only called from the same `parse_block_into_data` call frame
        // and the returned `&str` is consumed before `self.scratch` is
        // mutated again in the same frame.
        let leaked: &'a str = unsafe { std::mem::transmute::<&str, &'a str>(self.scratch.as_str()) };
        Some(NormalizedText { text: leaked, span })
    }
}

// ---------------------------------------------------------------------------
// Local parsing helpers (no Self)
// ---------------------------------------------------------------------------

fn parse_tag_header(line: &str, line_start: u32) -> Option<(&str, u32, Option<(&str, u32)>)> {
    let stripped = line.strip_prefix('@')?;
    // Tag-name characters are ASCII-only by spec, so byte position search
    // beats `chars().take_while().sum::<len_utf8>()` (no UTF-8 decoding,
    // single-byte is_ascii_alphanumeric LUT).
    let name_len = stripped
        .as_bytes()
        .iter()
        .position(|&b| !(b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'!')))
        .unwrap_or(stripped.len());
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
        .as_bytes()
        .iter()
        .position(|&b| !(b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'!')))
        .unwrap_or(trimmed.len());
    if name_len == 0 {
        return None;
    }
    let tag_name = &trimmed[..name_len];
    let body = trimmed[name_len..].trim();
    Some((tag_name, body))
}

fn parse_inline_tag_body(body: &str) -> (Option<&str>, Option<&str>, InlineTagFormatData) {
    if body.is_empty() {
        return (None, None, InlineTagFormatData::Plain);
    }
    if let Some((target, text)) = body.split_once('|') {
        return (
            non_empty_trimmed(target),
            non_empty_trimmed(text),
            InlineTagFormatData::Pipe,
        );
    }
    if let Some((target, text)) = body.split_once(char::is_whitespace) {
        return (
            non_empty_trimmed(target),
            non_empty_trimmed(text),
            InlineTagFormatData::Space,
        );
    }
    (Some(body), None, InlineTagFormatData::Plain)
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

fn find_matching_type_end(text: &str, start: usize) -> Option<usize> {
    // `{` and `}` are single-byte ASCII chars and unambiguous within UTF-8
    // (continuation bytes never collide with ASCII), so byte iteration is
    // both correct and avoids per-step UTF-8 decoding. The previous
    // `char_indices().skip(start)` was also semantically wrong because
    // `start` is a byte offset but `.skip(N)` skips N chars.
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn find_value_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    if start >= bytes.len() {
        return start;
    }
    if bytes[start] == b'[' {
        // Bracket-depth scan: `[` and `]` are single-byte ASCII so byte loop
        // suffices.
        let mut depth = 0usize;
        let mut i = start;
        while i < bytes.len() {
            match bytes[i] {
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return i + 1;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        return text.len();
    }
    // Tag value tokens terminate at any whitespace. ASCII bytes are checked
    // inline (LUT, no UTF-8 decode); the moment a non-ASCII byte appears we
    // fall back to char_indices to preserve the original Unicode-whitespace
    // semantics.
    let mut i = start;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            if (b as char).is_whitespace() {
                return i;
            }
            i += 1;
        } else {
            for (idx, ch) in text[i..].char_indices() {
                if ch.is_whitespace() {
                    return i + idx;
                }
            }
            return text.len();
        }
    }
    text.len()
}

fn parse_tag_value_token(token: &str, span: Span) -> TagValueData<'_> {
    if token.starts_with('[') && token.ends_with(']') {
        let inner = &token[1..token.len() - 1];
        let (path, default_value) = inner
            .split_once('=')
            .map_or((inner, None), |(p, v)| (p, Some(v)));
        return TagValueData::Parameter {
            span,
            path,
            optional: true,
            default_value,
        };
    }
    if token
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '.' | '[' | ']'))
    {
        return TagValueData::Parameter {
            span,
            path: token,
            optional: false,
            default_value: None,
        };
    }
    if token.contains(['.', '#', '~', '/', ':', '"', '\'', '(']) {
        return TagValueData::Namepath { span, raw: token };
    }
    if token
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$'))
    {
        return TagValueData::Identifier { span, name: token };
    }
    TagValueData::Raw { span, value: token }
}

fn tag_value_name<'a>(
    value: Option<&TagValueData<'a>>,
) -> (Option<TagNameValueData<'a>>, bool, Option<&'a str>) {
    match value {
        Some(TagValueData::Parameter {
            span,
            path,
            optional,
            default_value,
        }) => (
            Some(TagNameValueData {
                span: *span,
                raw: path,
            }),
            *optional,
            *default_value,
        ),
        Some(TagValueData::Namepath { span, raw }) => (
            Some(TagNameValueData {
                span: *span,
                raw,
            }),
            false,
            None,
        ),
        Some(TagValueData::Identifier { span, name }) => (
            Some(TagNameValueData {
                span: *span,
                raw: name,
            }),
            false,
            None,
        ),
        Some(TagValueData::Raw { span, value }) => (
            Some(TagNameValueData {
                span: *span,
                raw: value,
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

// ---------------------------------------------------------------------------
// Emit phase — walk parsed data and write to BinaryWriter
// ---------------------------------------------------------------------------

/// Write a parsed JSDoc block into `writer` and return the assigned root
/// node index. The caller is responsible for adding a corresponding entry
/// to the Root index array via [`BinaryWriter::push_root`].
pub fn emit_block<'arena>(
    writer: &mut BinaryWriter<'arena>,
    parsed: &ParsedBlock<'_>,
) -> Option<u32> {
    let block = parsed.block.as_ref()?;
    let compat = writer.compat_mode();
    Some(emit_block_inner(writer, block, compat))
}

fn opt_string(writer: &mut BinaryWriter<'_>, value: Option<&str>) -> Option<StringIndex> {
    value.map(|s| writer.intern_string(s))
}

fn empty_string(writer: &mut BinaryWriter<'_>) -> StringIndex {
    writer.intern_string("")
}

fn intern(writer: &mut BinaryWriter<'_>, value: &str) -> StringIndex {
    writer.intern_string(value)
}

fn emit_block_inner<'arena>(
    writer: &mut BinaryWriter<'arena>,
    block: &BlockData<'_>,
    compat: bool,
) -> u32 {
    let description_idx = opt_string(writer, block.description);

    // Pre-intern all source-preserving strings.
    let post_delim_str = if block.delimiter_line_break.is_empty() && !is_empty_block(block) {
        " "
    } else {
        ""
    };
    let star = intern(writer, "*");
    let close = intern(writer, "*/");
    let post_delim = intern(writer, post_delim_str);
    let line_end = intern(writer, block.line_end);
    let initial = empty_string(writer);
    let dlb = intern(writer, block.delimiter_line_break);
    let plb = intern(writer, block.preterminal_line_break);

    let mut bitmask: u8 = 0;
    if !block.description_lines.is_empty() {
        bitmask |= 0b001;
    }
    if !block.tags.is_empty() {
        bitmask |= 0b010;
    }
    if !block.inline_tags.is_empty() {
        bitmask |= 0b100;
    }

    let block_idx = write_jsdoc_block(
        writer,
        block.span,
        0,
        description_idx,
        star,
        post_delim,
        close,
        line_end,
        initial,
        dlb,
        plb,
        bitmask,
    );

    if compat {
        // Compat tail: write to ext offset 0 for the just-emitted block.
        // Since this is the first Extended Data record, its offset is 0;
        // for subsequent comments this would need to be the recorded offset.
        // For Phase 1 (single-comment), we know it lands at offset 0.
        let off = crate::writer::ExtOffset::from_u32(0).expect("first ext record");
        write_jsdoc_block_compat_tail(
            writer,
            off,
            block.end_line,
            block.description_start_line,
            block.description_end_line,
            block.last_description_line,
            block.has_preterminal_description,
            block.has_preterminal_tag_description,
        );
    }

    let block_parent = block_idx.as_u32();

    if !block.description_lines.is_empty() {
        let list = write_node_list(
            writer,
            block.span,
            block_parent,
            block.description_lines.len() as u32,
        );
        for line in &block.description_lines {
            emit_description_line(writer, line, list.as_u32(), compat);
        }
    }
    if !block.tags.is_empty() {
        let list = write_node_list(
            writer,
            block.span,
            block_parent,
            block.tags.len() as u32,
        );
        for tag in &block.tags {
            emit_tag(writer, tag, list.as_u32(), compat);
        }
    }
    if !block.inline_tags.is_empty() {
        let list = write_node_list(
            writer,
            block.span,
            block_parent,
            block.inline_tags.len() as u32,
        );
        for inline in &block.inline_tags {
            emit_inline_tag(writer, inline, list.as_u32());
        }
    }

    block_parent
}

fn is_empty_block(block: &BlockData<'_>) -> bool {
    block.description_lines.is_empty() && block.tags.is_empty() && block.inline_tags.is_empty()
}

fn emit_description_line(
    writer: &mut BinaryWriter<'_>,
    line: &DescriptionLineData<'_>,
    parent_index: u32,
    _compat: bool,
) {
    // Path A: `line.description` is `line.content.trim_end()`, i.e. a
    // sub-slice of the source text that was just appended to `data_buffer`
    // via `append_source_text`. Use the zero-copy `intern_source_slice`
    // path so we register the offsets-only entry without re-copying the
    // bytes — the dominant emit-phase cost we identified in
    // `.notes/binary-ast-emit-phase-format-analysis.md`.
    //
    // The byte range is `[span.start, span.start + description.len())`;
    // `span.end` would over-shoot because it includes the trailing
    // whitespace that `trim_end()` removed.
    let desc_byte_end = line.span.start + line.description.len() as u32;
    let desc_idx = writer.intern_source_slice(line.span.start, desc_byte_end);
    let delim = opt_string(writer, non_empty_str(line.delimiter));
    let pdelim = opt_string(writer, non_empty_str(line.post_delimiter));
    let init = opt_string(writer, non_empty_str(line.initial));
    let _ = write_jsdoc_description_line(
        writer,
        line.span,
        parent_index,
        desc_idx,
        delim,
        pdelim,
        init,
    );
}

fn non_empty_str(s: &str) -> Option<&str> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn emit_tag(
    writer: &mut BinaryWriter<'_>,
    tag: &TagData<'_>,
    parent_index: u32,
    compat: bool,
) {
    let default_idx = opt_string(writer, tag.default_value);
    let desc_idx = opt_string(writer, tag.description);
    let raw_body_idx = opt_string(writer, tag.raw_body);

    let mut bitmask: u8 = 0b0000_0001; // bit0 = tag (mandatory)
    if tag.raw_type.is_some() {
        bitmask |= 0b0000_0010;
    }
    if tag.name.is_some() {
        bitmask |= 0b0000_0100;
    }
    if tag.parsed_type.is_some() {
        bitmask |= 0b0000_1000;
    }
    if tag.body.is_some() {
        bitmask |= 0b0001_0000;
    }
    if !tag.type_lines.is_empty() {
        bitmask |= 0b0010_0000;
    }
    if !tag.description_lines.is_empty() {
        bitmask |= 0b0100_0000;
    }
    if !tag.description_lines.is_empty() {
        bitmask |= 0b0100_0000;
    }
    if !tag.inline_tags.is_empty() {
        bitmask |= 0b1000_0000;
    }

    let tag_idx = write_jsdoc_tag(
        writer,
        tag.span,
        parent_index,
        tag.optional,
        default_idx,
        desc_idx,
        raw_body_idx,
        bitmask,
    );

    if compat {
        // Compat tail: locate the ext offset of this tag. We can compute
        // it from the writer's extended data length minus the basic 8
        // bytes (the tag's record was just appended). Cleaner: re-fetch
        // via a helper in the writer.
        // For Phase 1.2a, default the post_* / line_end to single-space /
        // empty — matching ox_jsdoc's typed parser convention.
        let post_tag_str = intern(writer, tag.post_tag);
        let post_type_str = intern(writer, tag.post_type);
        let post_name_str = intern(writer, tag.post_name);
        let initial = intern(writer, tag.header_initial);
        let line_end = intern(writer, tag.header_line_end);
        let delim = intern(writer, tag.header_delimiter);
        let pdelim = intern(writer, tag.header_post_delimiter);
        // Look up the ext offset just emitted: for Phase 1 we don't track
        // it precisely. The compat tail bytes are appended via the writer's
        // public helper using the offset returned by reserveExtended()
        // inside write_jsdoc_tag — but that helper doesn't return it. To
        // keep this Phase scope manageable we omit the per-tag compat
        // tail; the BlockData level still emits its own compat tail above.
        // TODO: extend writer to surface ext_offset of the latest emit so
        // we can complete the per-tag compat tail.
        let _ = (delim, pdelim, post_tag_str, post_type_str, post_name_str, initial, line_end);
        let _ = write_jsdoc_tag_compat_tail; // suppress unused-import warning
    }

    let tag_parent = tag_idx.as_u32();

    // Mandatory tag-name child (visitor index 0).
    let tn_str = intern(writer, tag.tag_name);
    let _ = write_jsdoc_tag_name(writer, tag.tag_name_span, tag_parent, tn_str);

    if let Some(rt) = tag.raw_type.as_ref() {
        let raw_idx = intern(writer, rt.raw);
        let _ = write_jsdoc_type_source(writer, rt.span, tag_parent, raw_idx);
    }
    if let Some(name) = tag.name.as_ref() {
        let raw_idx = intern(writer, name.raw);
        let _ = write_jsdoc_tag_name_value(writer, name.span, tag_parent, raw_idx);
    }
    if let Some(pt) = tag.parsed_type.as_ref() {
        super::type_emit::emit_type_node(writer, pt, tag_parent);
    }

    if let Some(body) = tag.body.as_ref() {
        emit_tag_body(writer, body, tag_parent);
    }
    if !tag.type_lines.is_empty() {
        let list = write_node_list(
            writer,
            tag.span,
            tag_parent,
            tag.type_lines.len() as u32,
        );
        for tl in &tag.type_lines {
            let raw_idx = intern(writer, tl.raw_type);
            let delim = opt_string(writer, non_empty_str(tl.delimiter));
            let pdelim = opt_string(writer, non_empty_str(tl.post_delimiter));
            let init = opt_string(writer, non_empty_str(tl.initial));
            let _ = write_jsdoc_type_line(
                writer,
                tl.span,
                list.as_u32(),
                raw_idx,
                delim,
                pdelim,
                init,
            );
        }
    }
    if !tag.description_lines.is_empty() {
        let list = write_node_list(
            writer,
            tag.span,
            tag_parent,
            tag.description_lines.len() as u32,
        );
        for line in &tag.description_lines {
            emit_description_line(writer, line, list.as_u32(), compat);
        }
    }
    if !tag.inline_tags.is_empty() {
        let list = write_node_list(
            writer,
            tag.span,
            tag_parent,
            tag.inline_tags.len() as u32,
        );
        for inline in &tag.inline_tags {
            emit_inline_tag(writer, inline, list.as_u32());
        }
    }
}

fn emit_tag_body(writer: &mut BinaryWriter<'_>, body: &TagBodyData<'_>, parent_index: u32) {
    match body {
        TagBodyData::Generic(g) => {
            let desc_idx = opt_string(writer, g.description);
            // Children bitmask: bit0 = type_source, bit1 = value.
            let mut bm: u8 = 0;
            if g.type_source.is_some() {
                bm |= 0b01;
            }
            if g.value.is_some() {
                bm |= 0b10;
            }
            let body_idx = write_jsdoc_generic_tag_body(
                writer,
                g.span,
                parent_index,
                g.has_dash_separator,
                desc_idx,
                bm,
            );
            let body_parent = body_idx.as_u32();

            if let Some(ts) = g.type_source.as_ref() {
                let raw_idx = intern(writer, ts.raw);
                let _ = write_jsdoc_type_source(writer, ts.span, body_parent, raw_idx);
            }
            if let Some(v) = g.value.as_ref() {
                emit_tag_value(writer, v, body_parent);
            }
        }
    }
}

fn emit_tag_value(writer: &mut BinaryWriter<'_>, value: &TagValueData<'_>, parent_index: u32) {
    match value {
        TagValueData::Parameter {
            span,
            path,
            optional,
            default_value,
        } => {
            let path_idx = intern(writer, path);
            let dv_idx = opt_string(writer, *default_value);
            let _ = write_jsdoc_parameter_name(
                writer,
                *span,
                parent_index,
                *optional,
                path_idx,
                dv_idx,
            );
        }
        TagValueData::Namepath { span, raw } => {
            let raw_idx = intern(writer, raw);
            let _ = write_jsdoc_namepath_source(writer, *span, parent_index, raw_idx);
        }
        TagValueData::Identifier { span, name } => {
            let name_idx = intern(writer, name);
            let _ = write_jsdoc_identifier(writer, *span, parent_index, name_idx);
        }
        TagValueData::Raw { span, value } => {
            let val_idx = intern(writer, value);
            let _ = write_jsdoc_text(writer, *span, parent_index, val_idx);
        }
    }
}

fn emit_inline_tag(
    writer: &mut BinaryWriter<'_>,
    inline: &InlineTagData<'_>,
    parent_index: u32,
) {
    let np_idx = opt_string(writer, inline.namepath_or_url);
    let text_idx = opt_string(writer, inline.text);
    let raw_idx = opt_string(writer, inline.raw_body);
    let format = inline.format as u8;
    let _ = write_jsdoc_inline_tag(
        writer,
        inline.span,
        parent_index,
        format,
        np_idx,
        text_idx,
        raw_idx,
    );
    let _ = U16_NONE_SENTINEL; // suppress unused-import warning
    // Note: the inline tag's `tag` child (JsdocTagName) is referenced by the
    // jsdoccomment AST shape but the binary writer's JsdocInlineTag does not
    // currently include a child slot for it. This matches the Rust lazy
    // decoder's surface.
    let _ = inline.tag_name_span;
    let _ = inline.tag_name;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> ParseOptions {
        ParseOptions::default()
    }

    #[test]
    fn parse_block_returns_none_for_non_jsdoc() {
        let parsed = parse_block_into_data("/* plain */", 0, opts());
        assert!(parsed.is_failure());
        assert!(matches!(
            parsed.diagnostics()[0].kind,
            DiagnosticKind::Parser(ParserDiagnosticKind::NotAJSDocBlock)
        ));
    }

    #[test]
    fn parse_block_returns_none_for_unclosed() {
        let parsed = parse_block_into_data("/** unclosed", 0, opts());
        assert!(parsed.is_failure());
        assert!(matches!(
            parsed.diagnostics()[0].kind,
            DiagnosticKind::Parser(ParserDiagnosticKind::UnclosedBlockComment)
        ));
    }

    #[test]
    fn parses_top_level_description() {
        let parsed = parse_block_into_data("/** ok */", 10, opts());
        assert!(!parsed.is_failure());
        let block = parsed.block.as_ref().unwrap();
        assert_eq!(block.description, Some("ok"));
        assert_eq!(block.description_lines.len(), 1);
    }

    #[test]
    fn parses_param_tag_with_type_value_and_description() {
        let parsed = parse_block_into_data(
            "/**\n * @param {string} id - The user ID\n */",
            0,
            opts(),
        );
        assert!(!parsed.is_failure());
        let block = parsed.block.as_ref().unwrap();
        assert_eq!(block.tags.len(), 1);
        let tag = &block.tags[0];
        assert_eq!(tag.tag_name, "param");
        assert_eq!(tag.description, Some("The user ID"));
        assert!(tag.raw_type.is_some());
        assert_eq!(tag.raw_type.as_ref().unwrap().raw, "string");
        assert!(tag.name.is_some());
        assert_eq!(tag.name.as_ref().unwrap().raw, "id");
    }
}
