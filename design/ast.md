# ox-jsdoc AST Design

This document defines the intended AST direction for `ox-jsdoc`.

The AST should support two primary consumers:

1. `oxlint`, where JSDoc comments are parsed from comments collected by `oxc_parser`.
2. ESLint-compatible tooling, where rule authors expect an ESTree-like shape.

The design must also preserve the performance principles in
`design/001-performance`:

- keep the parser hot path small
- use arena allocation and borrowed source slices
- preserve enough syntax for later validation, formatting, and analysis
- avoid doing final tag semantics during parsing
- keep parser, validator, analyzer, and serializer concerns separate

The core direction is:

```text
JSDoc source text
  -> parser
  -> ESTree-like, source-preserving JSDoc AST
  -> validator / analyzer / type parser
  -> lint rules, formatter rules, JSON / JS binding
```

## Goals

### 1. ESTree-like public shape

The public AST should look familiar to ESLint rule authors.

Every visitable node should have:

- a node kind equivalent to ESTree's `type`
- a `span` in Rust
- optional `range` / `loc` only in JS-facing serialization
- child fields that can be described by visitor keys

For Rust implementation, node structs should not use a field named `type`.
The node kind is represented by the Rust struct or enum variant.
When serialized to JS or JSON, the node kind should be emitted as `"type"`.

Example JS-facing shape:

```ts
{
  type: "JsdocBlock",
  range: [0, 42],
  description: "Find a user.",
  descriptionLines: [...],
  tags: [...],
  inlineTags: [...]
}
```

### 2. Source-preserving syntax layer

Lint rules need more than semantic data.
They often inspect whitespace, delimiter lines, raw type text, tag order,
description lines, and exact spans for fixes.

The AST should preserve:

- complete comment span
- block tag spans
- description line spans
- tag name spans
- raw type spans
- raw body spans
- delimiter and post-delimiter text
- inline tag spans and raw syntax
- enough line-level structure to write formatter/linter rules

This is intentionally stronger than a doclet-style semantic model.

### 3. Parser does not finalize semantics

The parser should extract syntax and recover from malformed input.
It should not decide every tag's final meaning.

The following belong to later phases:

- built-in vs custom tag policy
- JSDoc / Closure / TypeScript mode differences
- complete type-expression parsing
- complete namepath validation
- complete parameter-path validation
- rule-specific diagnostics

The parser may classify obvious common syntax, but it must preserve raw forms so
later phases can reinterpret them.

### 4. Future extension without AST churn

The AST should allow adding richer structures later without breaking the basic
rule-facing shape.

Planned extension points:

- `parsedType` for JSDoc type AST
- richer `namepath` AST
- richer parameter path AST
- markdown/fenced-code nodes
- inline code nodes
- tag dictionaries and mode-specific validation facts
- comment attachment metadata for `oxlint` / ESLint integrations

The first version should not eagerly implement every extension, but it should
reserve stable locations for them.

## Relationship to Existing Ecosystem

`comment-parser` is a low-level JSDoc block parser.

`@es-joy/jsdoccomment` builds a JSDoc-specific ESTree-like layer on top of it:

- `JsdocBlock`
- `JsdocTag`
- `JsdocDescriptionLine`
- `JsdocTypeLine`
- `JsdocInlineTag`
- visitor keys
- optional `jsdoc-type-pratt-parser` type AST
- utilities for locating comments attached to ESLint AST nodes

`ox-jsdoc` should be closer to the `@es-joy/jsdoccomment` AST shape than to
`comment-parser`'s raw result shape.

The goal is not byte-for-byte compatibility.
The goal is to provide a rule-friendly AST with stable node kinds and visitor
keys, while keeping Rust-side performance and arena allocation.

## Layered Architecture

### Parser Layer

Input:

- one complete `/** ... */` block
- `base_offset` from the original source
- parser options

Output:

- `JsdocBlock<'a>`
- parser diagnostics

Responsibilities:

- recognize the JSDoc block
- strip line prefixes while preserving line syntax
- split block description from block tags
- parse block tag names
- extract raw tag body
- extract common type/name/description slots when unambiguous
- parse inline tags in descriptions
- preserve raw text and spans
- recover when possible

Non-responsibilities:

- final tag validity
- rule-level lint diagnostics
- full type compatibility
- complete namepath validation

### Validator Layer

Input:

- `JsdocBlock<'a>`
- validation mode
- tag dictionary

Output:

- diagnostics
- optional semantic facts

Responsibilities:

- unknown tag policy
- required type/name checks
- mode-sensitive tag checks
- parameter/namepath/type validity checks
- validation diagnostics with precise spans

### Analyzer Layer

Input:

- `JsdocBlock<'a>`
- optional validator output

Output:

- consumer-oriented facts

Examples:

- tag names
- parameter names
- custom tags
- documented return presence
- inline tag usage
- relationships such as `@borrows`

### Serializer / Binding Layer

Input:

- AST
- diagnostics
- optional validator/analyzer/type-parser output

Output:

- JSON or JS object shape

Responsibilities:

- emit ESTree-like `"type"` fields
- emit `range` and optionally `loc`
- convert borrowed Rust slices to JS strings
- optionally include parser-only, validator, analyzer, and type information

## Core Node Set

The initial public AST should be built from these node kinds:

```text
JsdocBlock
JsdocDescriptionLine
JsdocTag
JsdocTypeLine
JsdocInlineTag
JsdocText
```

Future node kinds may include:

```text
JsdocType*
JsdocNamepath*
JsdocParameterPath*
JsdocFencedCodeBlock
JsdocInlineCode
```

## Visitor Keys

The default visitor keys should match rule-author expectations:

```ts
const jsdocVisitorKeys = {
  JsdocBlock: ["descriptionLines", "tags", "inlineTags"],
  JsdocDescriptionLine: [],
  JsdocTag: ["parsedType", "typeLines", "descriptionLines", "inlineTags"],
  JsdocTypeLine: [],
  JsdocInlineTag: [],
  JsdocText: []
}
```

If `parsedType` is absent or `null`, traversal skips it.
When type parsing is added, `JsdocType*` visitor keys should be merged in the
same way `@es-joy/jsdoccomment` merges `jsdoc-type-pratt-parser` visitor keys.

## Rust AST Shape

The Rust AST should keep arena allocation and borrowed strings.

Recommended v1 shape:

```rust
pub struct JsdocBlock<'a> {
    pub span: Span,
    pub delimiter: &'a str,
    pub post_delimiter: &'a str,
    pub terminal: &'a str,
    pub line_end: &'a str,
    pub description: Option<&'a str>,
    pub description_lines: Vec<'a, JsdocDescriptionLine<'a>>,
    pub tags: Vec<'a, JsdocTag<'a>>,
    pub inline_tags: Vec<'a, JsdocInlineTag<'a>>,
}
```

`description` is a convenience string slice or arena-normalized string for the
top-level description.
`description_lines` is the source-preserving representation used by formatter
and lint rules.

```rust
pub struct JsdocDescriptionLine<'a> {
    pub span: Span,
    pub delimiter: &'a str,
    pub post_delimiter: &'a str,
    pub initial: &'a str,
    pub description: &'a str,
}
```

```rust
pub struct JsdocTag<'a> {
    pub span: Span,
    pub tag: JsdocTagName<'a>,
    pub raw_type: Option<JsdocTypeSource<'a>>,
    pub parsed_type: Option<Box<'a, JsdocType<'a>>>,
    pub name: Option<JsdocTagNameValue<'a>>,
    pub optional: bool,
    pub default_value: Option<&'a str>,
    pub description: Option<&'a str>,
    pub raw_body: Option<&'a str>,
    pub delimiter: &'a str,
    pub post_delimiter: &'a str,
    pub post_tag: &'a str,
    pub post_type: &'a str,
    pub post_name: &'a str,
    pub type_lines: Vec<'a, JsdocTypeLine<'a>>,
    pub description_lines: Vec<'a, JsdocDescriptionLine<'a>>,
    pub inline_tags: Vec<'a, JsdocInlineTag<'a>>,
    pub body: Option<Box<'a, JsdocTagBody<'a>>>,
}
```

`JsdocTag` intentionally has both ESTree-like convenience fields and an optional
structured `body`.

The convenience fields are for lint rules:

- `tag`
- `raw_type`
- `name`
- `description`
- `optional`
- `default_value`

The structured `body` is for future richer analysis without forcing every rule
to understand low-level variants.

```rust
pub struct JsdocTagName<'a> {
    pub span: Span,
    pub value: &'a str,
}

pub struct JsdocTagNameValue<'a> {
    pub span: Span,
    pub raw: &'a str,
}

pub struct JsdocTypeSource<'a> {
    pub span: Span,
    pub raw: &'a str,
}

pub struct JsdocTypeLine<'a> {
    pub span: Span,
    pub delimiter: &'a str,
    pub post_delimiter: &'a str,
    pub initial: &'a str,
    pub raw_type: &'a str,
}
```

```rust
pub struct JsdocInlineTag<'a> {
    pub span: Span,
    pub tag: JsdocTagName<'a>,
    pub namepath_or_url: Option<&'a str>,
    pub text: Option<&'a str>,
    pub format: JsdocInlineTagFormat,
    pub raw_body: Option<&'a str>,
}

pub enum JsdocInlineTagFormat {
    Plain,
    Pipe,
    Space,
    Prefix,
    Unknown,
}
```

The inline tag shape follows the practical rule-facing model used by
`@es-joy/jsdoccomment` while keeping raw body text for custom inline tags and
future re-parsing.

## Structured Tag Body

`JsdocTag.body` should remain optional and extensible.

Recommended initial variants:

```rust
pub enum JsdocTagBody<'a> {
    Generic(Box<'a, JsdocGenericTagBody<'a>>),
    Borrows(Box<'a, JsdocBorrowsTagBody<'a>>),
    Raw(Box<'a, JsdocRawTagBody<'a>>),
}
```

```rust
pub struct JsdocGenericTagBody<'a> {
    pub span: Span,
    pub type_source: Option<JsdocTypeSource<'a>>,
    pub value: Option<JsdocTagValue<'a>>,
    pub separator: Option<JsdocSeparator>,
    pub description: Option<&'a str>,
}

pub struct JsdocBorrowsTagBody<'a> {
    pub span: Span,
    pub source: JsdocTagValue<'a>,
    pub target: JsdocTagValue<'a>,
}

pub struct JsdocRawTagBody<'a> {
    pub span: Span,
    pub raw: &'a str,
}
```

```rust
pub enum JsdocTagValue<'a> {
    Parameter(JsdocParameterName<'a>),
    Namepath(JsdocNamepathSource<'a>),
    Identifier(JsdocIdentifier<'a>),
    Raw(JsdocText<'a>),
}

pub enum JsdocSeparator {
    Dash,
}
```

This keeps the parser flexible:

- common tags get useful shape
- `@borrows source as target` can be represented explicitly
- unknown or ambiguous bodies can remain raw
- future tag-specific variants can be added without changing `JsdocTag`

## Type Expressions

Type expressions should be split into two layers:

1. `JsdocTypeSource`
2. `JsdocType`

`JsdocTypeSource` is parser-owned and cheap:

```rust
pub struct JsdocTypeSource<'a> {
    pub span: Span,
    pub raw: &'a str,
}
```

`JsdocType` is a future typed AST:

```rust
pub enum JsdocType<'a> {
    Name(Box<'a, JsdocTypeName<'a>>),
    Union(Box<'a, JsdocTypeUnion<'a>>),
    Function(Box<'a, JsdocTypeFunction<'a>>),
    Record(Box<'a, JsdocTypeRecord<'a>>),
    TypeApplication(Box<'a, JsdocTypeApplication<'a>>),
    Nullable(Box<'a, JsdocTypeNullable<'a>>),
    NonNullable(Box<'a, JsdocTypeNonNullable<'a>>),
    Optional(Box<'a, JsdocTypeOptional<'a>>),
    Variadic(Box<'a, JsdocTypeVariadic<'a>>),
    Unknown(Box<'a, JsdocTypeUnknown<'a>>),
}
```

The v1 parser should not be blocked on implementing `JsdocType`.
It should fill `raw_type` / `type_source` and leave `parsed_type = None`.

Later, a type parser can run as:

```text
JsdocTypeSource.raw
  -> JSDoc type parser
  -> JsdocType
  -> validator / analyzer / lint rules
```

This preserves future extensibility without making the parse hot path pay for
type parsing.

## Namepaths and Parameter Paths

Namepaths and parameter paths follow the same staged approach as types.

The parser should initially preserve raw source:

```rust
pub struct JsdocNamepathSource<'a> {
    pub span: Span,
    pub raw: &'a str,
}

pub struct JsdocParameterName<'a> {
    pub span: Span,
    pub path: &'a str,
    pub optional: bool,
    pub default_value: Option<&'a str>,
}
```

Future richer ASTs can be attached later:

```rust
pub struct JsdocParameterName<'a> {
    pub span: Span,
    pub path: &'a str,
    pub optional: bool,
    pub default_value: Option<&'a str>,
    pub parsed_path: Option<Box<'a, JsdocParameterPath<'a>>>,
}
```

This lets v1 support common lint rules quickly while still leaving room for
strict path validation and precise fixes.

## Comment Attachment

Comment attachment should not live inside `JsdocBlock`.

For `oxlint`, attachment belongs to the integration layer that has access to
the JavaScript / TypeScript AST and the comment list from `oxc_parser`.

Recommended shape:

```rust
pub struct JsdocAttachment<'a> {
    pub comment: Box<'a, JsdocBlock<'a>>,
    pub target_span: Span,
    pub target_kind: JsdocAttachmentTargetKind,
}
```

This keeps the core parser reusable:

- standalone comment parsing
- source comment benchmarks
- oxlint integration
- ESLint-compatible binding

The parser should not know whether a comment documents a function, class,
variable declaration, overload, or export declaration.

## JS / JSON Shape

The JS-facing AST should use ESTree-style field names.

Example:

```ts
interface JsdocBlock {
  type: "JsdocBlock"
  range: [number, number]
  description: string
  descriptionLines: JsdocDescriptionLine[]
  tags: JsdocTag[]
  inlineTags: JsdocInlineTag[]
}
```

```ts
interface JsdocTag {
  type: "JsdocTag"
  range: [number, number]
  tag: string
  rawType: string | null
  parsedType: JsdocType | null
  name: string | null
  optional: boolean
  defaultValue: string | null
  description: string
  rawBody: string | null
  typeLines: JsdocTypeLine[]
  descriptionLines: JsdocDescriptionLine[]
  inlineTags: JsdocInlineTag[]
}
```

```ts
interface JsdocDescriptionLine {
  type: "JsdocDescriptionLine"
  range: [number, number]
  delimiter: string
  postDelimiter: string
  initial: string
  description: string
}
```

```ts
interface JsdocTypeLine {
  type: "JsdocTypeLine"
  range: [number, number]
  delimiter: string
  postDelimiter: string
  initial: string
  rawType: string
}
```

```ts
interface JsdocInlineTag {
  type: "JsdocInlineTag"
  range: [number, number]
  tag: string
  namepathOrURL: string | null
  text: string | null
  format: "plain" | "pipe" | "space" | "prefix" | "unknown"
  rawBody: string | null
}
```

The JS shape can include `loc` as an option, but Rust AST nodes should keep only
byte spans.

## Migration From Current AST

Current implementation names:

```text
JSDocComment
Description
DescriptionPart
BlockTag
BlockTagBody
GenericTagBody
TypeExpression
TagValueToken
```

Recommended direction:

```text
JSDocComment     -> JsdocBlock
BlockTag         -> JsdocTag
Description      -> description + descriptionLines + inlineTags
DescriptionPart  -> JsdocDescriptionLine / JsdocInlineTag / future text nodes
TypeExpression   -> JsdocTypeSource first, JsdocType later
TagValueToken    -> JsdocTagValue
```

The biggest structural change is moving from an interleaved description tree to
an ESTree-like line-oriented shape.

However, the design should not lose the ability to represent text and inline
tags structurally.
The recommended compromise is:

- `descriptionLines` preserves formatter/linter line structure
- `inlineTags` exposes all inline tags as direct visitable children
- future `JsdocText` / `JsdocInlineCode` / `JsdocFencedCodeBlock` nodes can be
  added if rules need deeper description traversal

## Compatibility Policy

The AST should optimize for stable rule-facing fields:

- `type`
- `range`
- `tag`
- `rawType`
- `name`
- `description`
- `tags`
- `descriptionLines`
- `typeLines`
- `inlineTags`

Internal Rust representation may evolve as long as the JS-facing shape remains
compatible.

Fields likely to remain stable:

- node kinds
- visitor keys
- raw text fields
- spans/ranges
- direct tag/name/type/description convenience fields

Fields allowed to evolve:

- `body`
- `parsedType`
- parsed namepath structure
- parsed parameter-path structure
- analyzer output
- validation metadata

## Open Questions

1. Should `JsdocBlock.description` be `""` or `null` when no description exists?

   Recommendation: JS-facing shape should use `""`; Rust can use
   `Option<&str>` internally.

2. Should malformed type expressions produce `rawType`?

   Recommendation: yes. `parsedType` can be `null`, and parser diagnostics can
   describe the malformed range.

3. Should unknown inline tags be parsed into `JsdocInlineTag`?

   Recommendation: yes. Use `format: "unknown"` when link-style parsing cannot
   classify the body.

4. Should comment attachment be part of the AST?

   Recommendation: no. Keep it in the oxlint / ESLint integration layer.

5. Should type parsing run by default?

   Recommendation: no for the parser hot path. Make it an option or a later
   phase.

## Summary

`ox-jsdoc` should expose an ESTree-like JSDoc AST for lint tooling while keeping
the Rust parser fast and source-preserving.

The public shape should be close to `@es-joy/jsdoccomment` because that model is
already proven in ESLint JSDoc rules.

The internal design should still follow the performance documents:

- arena allocation
- borrowed strings
- absolute spans
- parser/validator/analyzer separation
- raw syntax preservation
- delayed type/namepath/semantic interpretation

This gives `ox-jsdoc` a practical v1 AST for oxlint and ESLint, without closing
the door on richer JSDoc structure later.
