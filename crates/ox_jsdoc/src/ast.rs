use oxc_allocator::{Box as ArenaBox, Vec as ArenaVec};
use oxc_span::Span;

#[derive(Debug)]
pub struct JSDocComment<'a> {
    pub span: Span,
    pub description: Option<ArenaBox<'a, Description<'a>>>,
    pub tags: ArenaVec<'a, BlockTag<'a>>,
}

#[derive(Debug)]
pub struct Description<'a> {
    pub span: Span,
    pub parts: ArenaVec<'a, DescriptionPart<'a>>,
}

#[derive(Debug)]
pub enum DescriptionPart<'a> {
    Text(ArenaBox<'a, Text<'a>>),
    InlineTag(ArenaBox<'a, InlineTag<'a>>),
}

#[derive(Debug)]
pub struct Text<'a> {
    pub span: Span,
    pub value: &'a str,
}

#[derive(Debug)]
pub struct InlineTag<'a> {
    pub span: Span,
    pub tag_name: TagName<'a>,
    pub body: Option<ArenaBox<'a, InlineTagBody<'a>>>,
}

#[derive(Debug)]
pub struct InlineTagBody<'a> {
    pub span: Span,
    pub raw: &'a str,
}

#[derive(Debug)]
pub struct BlockTag<'a> {
    pub span: Span,
    pub tag_name: TagName<'a>,
    pub body: Option<ArenaBox<'a, BlockTagBody<'a>>>,
    pub raw_body: Option<ArenaBox<'a, Text<'a>>>,
}

#[derive(Debug)]
pub enum BlockTagBody<'a> {
    Generic(ArenaBox<'a, GenericTagBody<'a>>),
    Borrows(ArenaBox<'a, BorrowsTagBody<'a>>),
}

#[derive(Debug)]
pub struct GenericTagBody<'a> {
    pub span: Span,
    pub type_expression: Option<ArenaBox<'a, TypeExpression<'a>>>,
    pub value: Option<ArenaBox<'a, TagValueToken<'a>>>,
    pub description: Option<ArenaBox<'a, Description<'a>>>,
}

#[derive(Debug)]
pub struct BorrowsTagBody<'a> {
    pub span: Span,
    pub source: TagValueToken<'a>,
    pub target: TagValueToken<'a>,
}

#[derive(Debug)]
pub struct TypeExpression<'a> {
    pub span: Span,
    pub raw: &'a str,
}

#[derive(Debug)]
pub enum TagValueToken<'a> {
    Raw(Text<'a>),
    Parameter(TagParameterName<'a>),
    NamePath(NamePathLike<'a>),
}

#[derive(Debug)]
pub struct TagParameterName<'a> {
    pub span: Span,
    pub path: ParameterPath<'a>,
    pub optional: bool,
    pub default_value: Option<&'a str>,
}

#[derive(Debug)]
pub struct ParameterPath<'a> {
    pub span: Span,
    pub raw: &'a str,
}

#[derive(Debug)]
pub struct NamePathLike<'a> {
    pub span: Span,
    pub raw: &'a str,
}

#[derive(Debug)]
pub struct TagName<'a> {
    pub span: Span,
    pub value: &'a str,
}
