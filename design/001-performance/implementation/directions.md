# Concrete Directions for `ox-jsdoc`

## Direction 1. The parser builds ASTs, but does not decide final meaning

The parser should stop after producing a syntax-informed tree.

Examples:

- `@variation 3` can remain generic at parse time
- `@memberof! Foo.` can remain generic with preserved raw structure
- custom tags should be parsed as far as possible instead of being rejected early

## Direction 2. Keep expensive normalization for later phases

Tag-specific, mode-specific, and context-sensitive rules are often better handled
after parsing. The parser should preserve enough information for later analysis
instead of eagerly normalizing everything.

Examples:

- `raw_body`
- `TagValueToken`
- broad `NamePathLike`
- broad `ParameterPath`

## Direction 3. Split nodes when their semantic roles differ

If two syntactic forms play different semantic roles, they should not be merged
just because their surface syntax looks similar.

Examples:

- `NamePathLike` vs `ParameterPath`
- `TypeName` as a wrapper around `NamePathLike`
- a dedicated `BorrowsTagBody`

This is close in spirit to how `oxc` distinguishes identifier roles.

## Direction 4. Mechanically verify layout-sensitive parts

Once the AST stabilizes, at least representative nodes should be checked for:

- struct size
- enum size
- performance-sensitive field ordering

The goal is not to freeze an ABI too early.
The goal is to detect accidental regressions.

## Direction 5. Treat serializer shape as part of the design, even with JSON transfer

Even if JS transfer starts with JSON, serialization should not be an afterthought.

Desired properties:

- a serializer shape that is easy to reason about
  - it should be easy to see which node exposes which fields
- predictable field ordering
- limited temporary allocation

Even before raw transfer exists, the serializer is part of the performance story.

## Direction 6. Use borrowed slices for parser strings by default

The parser should follow a **borrowed slice first, normalized string later** policy.

In the parser hot path, these values should normally stay as source-backed
`Span` + `&'a str` data instead of owned strings:

- description text
- block tag `raw_body`
- inline tag body
- raw type text
- parameter and name tokens

Normalized strings should be produced only when a later phase needs them, for example:

- escape interpretation
- whitespace-normalized descriptions
- joined description strings
- default value quote handling
- rendered link labels
- serializer-owned JS output strings

This keeps the parser cheap while preserving enough information for validator,
analyzer, formatter, and serializer layers.

Descriptions should not be flattened into one string at parse time.
They should preserve inline-tag boundaries:

```text
Description.parts = [
  Text(slice),
  InlineTag(slice),
  Text(slice)
]
```

This is important for real-world API documentation where `{@link}` and
`{@linkcode}` occur frequently, and where later consumers need accurate spans
for lint diagnostics, formatting, and rendering.

Fenced code blocks should also be tracked as scanner state.
Inside a fence, `@foo` and `{@link ...}`-looking text should not be eagerly
treated as normal block or inline tags.

The scanner can be byte-oriented in v1, but it should keep shallow state rather
than relying on naive whitespace splitting.
Important markers and states include:

- ASCII markers: `@`, `{`, `}`, `[`, `]`, `(`, `)`, `<`, `>`, `|`, `=`, `-`, quotes, backticks, and line breaks
- `brace_depth`
- `bracket_depth`
- `paren_depth`
- quote state
- inline-tag state
- fence state
- line-start / after-star-prefix state

This prevents common real-world forms from being split at the wrong boundary,
including:

- `[opts.maxLength=Infinity]`
- `[subject="world"]`
- `{import('typedoc').TypeDocOptions & { docsRoot?: string }}`
- `{@linkcode Command | entry command}`
- fenced `@example` blocks

The v1 rule should be:

1. do not allocate in the parser hot path when a source slice is enough
2. always preserve `raw_body`
3. preserve inline-tag and fenced-code boundaries
4. delay description joining and whitespace normalization
5. move tag-specific string normalization to validator or later phases
6. keep long or TypeScript-like type text as a raw range until a type parser is required

Tag names and other frequently compared small tokens can start as borrowed strings.
If measurement later shows lookup or comparison cost is significant, an
`Ident`-like pre-hashed representation can be considered.
