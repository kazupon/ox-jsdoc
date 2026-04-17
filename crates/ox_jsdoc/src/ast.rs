// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use oxc_allocator::{Box as ArenaBox, Vec as ArenaVec};
use oxc_span::Span;

/// Root node for one `/** ... */` JSDoc block.
///
/// Rust uses the concrete struct name as the node kind. JS-facing serializers
/// should emit this as `{ type: "JsdocBlock", ... }`.
#[derive(Debug)]
pub struct JsdocBlock<'a> {
    /// Span covering the complete block comment, including `/**` and `*/`.
    pub span: Span,
    /// Opening delimiter as source text.
    pub delimiter: &'a str,
    /// Whitespace after the opening delimiter, if represented by the parser.
    pub post_delimiter: &'a str,
    /// Closing delimiter as source text.
    pub terminal: &'a str,
    /// Line ending associated with the root delimiter, when known.
    pub line_end: &'a str,
    /// Indentation before the opening `/**` delimiter.
    pub initial: &'a str,
    /// Line break after the opening delimiter: `"\n"` for multi-line, `""` for single-line.
    pub delimiter_line_break: &'a str,
    /// Line break before the closing `*/`: `"\n"` for normal, `""` when content is on the `*/` line.
    pub preterminal_line_break: &'a str,
    /// Joined top-level description text before the first block tag.
    pub description: Option<&'a str>,
    /// Source-preserving top-level description lines.
    pub description_lines: ArenaVec<'a, JsdocDescriptionLine<'a>>,
    /// Block tags in source order.
    pub tags: ArenaVec<'a, JsdocTag<'a>>,
    /// Inline tags found in the top-level description.
    pub inline_tags: ArenaVec<'a, JsdocInlineTag<'a>>,
    /// 0-based line index of the closing `*/` line.
    pub end_line: u32,
    /// 0-based line index where block description starts (first non-empty description line).
    pub description_start_line: Option<u32>,
    /// 0-based line index where block description ends (last non-empty description line).
    pub description_end_line: Option<u32>,
    /// 0-based line index of the first tag or end line (description boundary).
    pub last_description_line: Option<u32>,
    /// 1 if block description text exists on the `*/` line, 0 otherwise.
    pub has_preterminal_description: u8,
    /// Some(1) if tag description exists on the `*/` line, None otherwise.
    pub has_preterminal_tag_description: Option<u8>,
}

/// One source-preserving description line.
#[derive(Debug)]
pub struct JsdocDescriptionLine<'a> {
    /// Span covering this logical description line.
    pub span: Span,
    /// Comment line delimiter, typically `*` for conventional JSDoc lines.
    pub delimiter: &'a str,
    /// Whitespace after `delimiter`, when known.
    pub post_delimiter: &'a str,
    /// Indentation before the delimiter, when known.
    pub initial: &'a str,
    /// Description content after stripping the JSDoc margin.
    pub description: &'a str,
}

/// A block tag such as `@param {string} id - User id`.
#[derive(Debug)]
pub struct JsdocTag<'a> {
    /// Span covering the tag name and body.
    pub span: Span,
    /// Block tag name without the leading `@`.
    pub tag: JsdocTagName<'a>,
    /// Raw `{...}` type source without the surrounding braces.
    pub raw_type: Option<JsdocTypeSource<'a>>,
    /// Future parsed type AST. The v1 parser leaves this as `None`.
    pub parsed_type: Option<ArenaBox<'a, JsdocType<'a>>>,
    /// First value token after the optional type, when interpreted as a name.
    pub name: Option<JsdocTagNameValue<'a>>,
    /// Whether `name` came from optional bracket syntax such as `[id]`.
    pub optional: bool,
    /// Default value from optional bracket syntax such as `[id=0]`.
    pub default_value: Option<&'a str>,
    /// Joined tag description text.
    pub description: Option<&'a str>,
    /// Raw body after the tag name, preserved for recovery and validators.
    pub raw_body: Option<&'a str>,
    /// Source line delimiter for the tag line.
    pub delimiter: &'a str,
    /// Whitespace after the tag line delimiter, when known.
    pub post_delimiter: &'a str,
    /// Indentation before the tag line delimiter.
    pub initial: &'a str,
    /// Line ending for the tag's first line.
    pub line_end: &'a str,
    /// Whitespace after the tag name.
    pub post_tag: &'a str,
    /// Whitespace after the type source.
    pub post_type: &'a str,
    /// Whitespace after the name token.
    pub post_name: &'a str,
    /// Source-preserving type lines.
    pub type_lines: ArenaVec<'a, JsdocTypeLine<'a>>,
    /// Source-preserving tag description lines.
    pub description_lines: ArenaVec<'a, JsdocDescriptionLine<'a>>,
    /// Inline tags found in the tag description.
    pub inline_tags: ArenaVec<'a, JsdocInlineTag<'a>>,
    /// Optional structured body for consumers that need more than convenience fields.
    pub body: Option<ArenaBox<'a, JsdocTagBody<'a>>>,
}

/// Tag name without the leading `@`.
#[derive(Debug, Clone, Copy)]
pub struct JsdocTagName<'a> {
    /// Span covering the tag name only.
    pub span: Span,
    /// Tag name text without `@`.
    pub value: &'a str,
}

/// Value token commonly used as a tag name or parameter name.
#[derive(Debug, Clone, Copy)]
pub struct JsdocTagNameValue<'a> {
    /// Span covering the raw value token.
    pub span: Span,
    /// Raw value text.
    pub raw: &'a str,
}

/// Raw text inside a `{...}` type expression.
#[derive(Debug, Clone, Copy)]
pub struct JsdocTypeSource<'a> {
    /// Span covering the whole `{...}` expression.
    pub span: Span,
    /// Raw text inside the surrounding braces.
    pub raw: &'a str,
}

/// One source-preserving type line.
#[derive(Debug, Clone, Copy)]
pub struct JsdocTypeLine<'a> {
    /// Span covering this type line.
    pub span: Span,
    /// Comment line delimiter, typically `*`.
    pub delimiter: &'a str,
    /// Whitespace after `delimiter`, when known.
    pub post_delimiter: &'a str,
    /// Indentation before the delimiter, when known.
    pub initial: &'a str,
    /// Raw type source for this line.
    pub raw_type: &'a str,
}

pub use crate::type_parser::ast::TypeNode;

/// Parsed JSDoc type AST.
#[derive(Debug)]
pub enum JsdocType<'a> {
    /// Structured type AST produced by the type parser.
    Parsed(ArenaBox<'a, TypeNode<'a>>),
    /// Raw fallback when type parsing is disabled or fails.
    Raw(JsdocTypeSource<'a>),
}

/// Inline tag such as `{@link Foo}` inside a description.
#[derive(Debug, Clone, Copy)]
pub struct JsdocInlineTag<'a> {
    /// Span covering the full inline tag, including `{@` and `}`.
    pub span: Span,
    /// Inline tag name without the leading `@`.
    pub tag: JsdocTagName<'a>,
    /// Link target, tutorial name, URL, or raw first payload token.
    pub namepath_or_url: Option<&'a str>,
    /// Display text for link-like inline tags.
    pub text: Option<&'a str>,
    /// Link-style body format.
    pub format: JsdocInlineTagFormat,
    /// Raw payload after the inline tag name.
    pub raw_body: Option<&'a str>,
}

/// Inline tag body format for link-like tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsdocInlineTagFormat {
    Plain,
    Pipe,
    Space,
    Prefix,
    Unknown,
}

/// Parsed block tag payload.
#[derive(Debug)]
pub enum JsdocTagBody<'a> {
    Generic(ArenaBox<'a, JsdocGenericTagBody<'a>>),
    Borrows(ArenaBox<'a, JsdocBorrowsTagBody<'a>>),
    Raw(ArenaBox<'a, JsdocRawTagBody<'a>>),
}

/// Common JSDoc tag body layout: optional type, optional value, description.
#[derive(Debug)]
pub struct JsdocGenericTagBody<'a> {
    /// Span covering the normalized body text.
    pub span: Span,
    /// Optional `{...}` type expression.
    pub type_source: Option<JsdocTypeSource<'a>>,
    /// Optional value token after the type expression.
    pub value: Option<JsdocTagValue<'a>>,
    /// Optional `-` separator between value and description.
    pub separator: Option<JsdocSeparator>,
    /// Optional prose after the value token.
    pub description: Option<&'a str>,
}

/// Specialized shape for `@borrows source as target`.
#[derive(Debug)]
pub struct JsdocBorrowsTagBody<'a> {
    /// Span covering the normalized `@borrows` body.
    pub span: Span,
    /// Source side of the borrow relationship.
    pub source: JsdocTagValue<'a>,
    /// Target side of the borrow relationship.
    pub target: JsdocTagValue<'a>,
}

/// Raw fallback for tag bodies that should not be interpreted by the parser.
#[derive(Debug)]
pub struct JsdocRawTagBody<'a> {
    /// Span covering the raw body.
    pub span: Span,
    /// Raw body text.
    pub raw: &'a str,
}

/// Value token after an optional type expression.
#[derive(Debug, Clone, Copy)]
pub enum JsdocTagValue<'a> {
    Parameter(JsdocParameterName<'a>),
    Namepath(JsdocNamepathSource<'a>),
    Identifier(JsdocIdentifier<'a>),
    Raw(JsdocText<'a>),
}

/// Optional separator between a value token and description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsdocSeparator {
    Dash,
}

/// Parameter-like value, including optional/default syntax such as `[id=0]`.
#[derive(Debug, Clone, Copy)]
pub struct JsdocParameterName<'a> {
    /// Span covering the parameter token.
    pub span: Span,
    /// Parameter path without optional/default brackets.
    pub path: &'a str,
    /// Whether the parameter used optional bracket syntax.
    pub optional: bool,
    /// Default value from `[path=value]`, if present.
    pub default_value: Option<&'a str>,
}

/// Raw namepath-like value such as `module:foo/bar` or `Foo#bar`.
#[derive(Debug, Clone, Copy)]
pub struct JsdocNamepathSource<'a> {
    /// Span covering the namepath-like token.
    pub span: Span,
    /// Raw namepath-like text.
    pub raw: &'a str,
}

/// Identifier-like value token.
#[derive(Debug, Clone, Copy)]
pub struct JsdocIdentifier<'a> {
    /// Span covering the identifier token.
    pub span: Span,
    /// Identifier text.
    pub name: &'a str,
}

/// Raw text leaf.
#[derive(Debug, Clone, Copy)]
pub struct JsdocText<'a> {
    /// Span covering this text fragment.
    pub span: Span,
    /// Borrowed or arena-normalized text content.
    pub value: &'a str,
}
