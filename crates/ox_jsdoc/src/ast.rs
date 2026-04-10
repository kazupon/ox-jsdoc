// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use oxc_allocator::{Box as ArenaBox, Vec as ArenaVec};
use oxc_span::Span;

/// Root node for one `/** ... */` JSDoc block.
///
/// The AST borrows slices from the original source and stores nodes in the
/// caller-provided arena. Parser stages should avoid allocating owned strings
/// unless a value is normalized across multiple source lines.
#[derive(Debug)]
pub struct JSDocComment<'a> {
    /// Span covering the complete block comment, including `/**` and `*/`.
    pub span: Span,
    /// Description before the first block tag.
    pub description: Option<ArenaBox<'a, Description<'a>>>,
    /// Block tags in source order.
    pub tags: ArenaVec<'a, BlockTag<'a>>,
}

/// Top-level or tag-local prose, split into text and inline tags.
#[derive(Debug)]
pub struct Description<'a> {
    /// Span covering the normalized description text.
    pub span: Span,
    /// Ordered text and inline-tag fragments.
    pub parts: ArenaVec<'a, DescriptionPart<'a>>,
}

/// Ordered description fragment.
#[derive(Debug)]
pub enum DescriptionPart<'a> {
    Text(ArenaBox<'a, Text<'a>>),
    InlineTag(ArenaBox<'a, InlineTag<'a>>),
}

/// Plain source text with its byte span.
#[derive(Debug)]
pub struct Text<'a> {
    /// Span covering this text fragment.
    pub span: Span,
    /// Borrowed or arena-normalized text content.
    pub value: &'a str,
}

/// Inline tag such as `{@link Foo}` inside a description.
#[derive(Debug)]
pub struct InlineTag<'a> {
    /// Span covering the full inline tag, including `{@` and `}`.
    pub span: Span,
    /// Inline tag name without the leading `@`.
    pub tag_name: TagName<'a>,
    /// Raw payload after the inline tag name.
    pub body: Option<ArenaBox<'a, InlineTagBody<'a>>>,
}

/// Raw inline tag payload after the tag name.
#[derive(Debug)]
pub struct InlineTagBody<'a> {
    /// Span covering only the body text inside the inline tag.
    pub span: Span,
    /// Raw inline body text.
    pub raw: &'a str,
}

/// Block tag such as `@param {string} id - User id`.
#[derive(Debug)]
pub struct BlockTag<'a> {
    /// Span covering the tag name and body.
    pub span: Span,
    /// Block tag name without the leading `@`.
    pub tag_name: TagName<'a>,
    /// Structured body parsed from `raw_body`.
    pub body: Option<ArenaBox<'a, BlockTagBody<'a>>>,
    /// Raw body after the tag name, preserved for validators and consumers that
    /// need source-compatible reconstruction.
    pub raw_body: Option<ArenaBox<'a, Text<'a>>>,
}

/// Parsed block tag payload.
#[derive(Debug)]
pub enum BlockTagBody<'a> {
    Generic(ArenaBox<'a, GenericTagBody<'a>>),
    Borrows(ArenaBox<'a, BorrowsTagBody<'a>>),
}

/// Common JSDoc tag body layout: optional type, optional value, description.
#[derive(Debug)]
pub struct GenericTagBody<'a> {
    /// Span covering the normalized body text.
    pub span: Span,
    /// Optional `{...}` type expression.
    pub type_expression: Option<ArenaBox<'a, TypeExpression<'a>>>,
    /// Optional value token after the type expression.
    pub value: Option<ArenaBox<'a, TagValueToken<'a>>>,
    /// Optional prose after the value token.
    pub description: Option<ArenaBox<'a, Description<'a>>>,
}

/// Specialized shape for `@borrows source as target`.
#[derive(Debug)]
pub struct BorrowsTagBody<'a> {
    /// Span covering the normalized `@borrows` body.
    pub span: Span,
    /// Source side of the borrow relationship.
    pub source: TagValueToken<'a>,
    /// Target side of the borrow relationship.
    pub target: TagValueToken<'a>,
}

/// Raw text inside `{...}`.
#[derive(Debug)]
pub struct TypeExpression<'a> {
    /// Span covering the whole `{...}` expression.
    pub span: Span,
    /// Raw text inside the surrounding braces.
    pub raw: &'a str,
}

/// Value token after an optional type expression.
#[derive(Debug)]
pub enum TagValueToken<'a> {
    Raw(Text<'a>),
    Parameter(TagParameterName<'a>),
    NamePath(NamePathLike<'a>),
}

/// Parameter-like value, including optional/default syntax such as `[id=0]`.
#[derive(Debug)]
pub struct TagParameterName<'a> {
    /// Span covering the parameter token.
    pub span: Span,
    /// Parameter path without optional/default brackets.
    pub path: ParameterPath<'a>,
    /// Whether the parameter used optional bracket syntax.
    pub optional: bool,
    /// Default value from `[path=value]`, if present.
    pub default_value: Option<&'a str>,
}

/// Parameter path as written in source.
#[derive(Debug)]
pub struct ParameterPath<'a> {
    /// Span covering the parameter path token.
    pub span: Span,
    /// Raw parameter path text.
    pub raw: &'a str,
}

/// Name path-like value such as `module:foo/bar` or `Foo#bar`.
#[derive(Debug)]
pub struct NamePathLike<'a> {
    /// Span covering the name path-like token.
    pub span: Span,
    /// Raw name path-like text.
    pub raw: &'a str,
}

/// Tag name without the leading `@`.
#[derive(Debug)]
pub struct TagName<'a> {
    /// Span covering the tag name only.
    pub span: Span,
    /// Tag name text without `@`.
    pub value: &'a str,
}
