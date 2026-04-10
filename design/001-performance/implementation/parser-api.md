# Parser API and Allocation Contract

The initial parser API should make the allocation and source-lifetime model
explicit. Otherwise, later implementation work can accidentally introduce owned
strings, detached spans, or diagnostics that require extra source copying.

## Primary API shape

The first public parser entrypoint should parse one complete JSDoc block comment:

```rust
pub fn parse_comment<'a>(
    allocator: &'a Allocator,
    source_text: &'a str,
    base_offset: u32,
    options: ParseOptions,
) -> ParseOutput<'a>;
```

Recommended supporting shape:

```rust
pub struct ParseOptions {
    /// Whether Markdown-style fenced code blocks suppress tag parsing.
    pub fence_aware: bool,
    /// Whether to preserve inline-code boundaries for future description nodes.
    pub inline_code_aware: bool,
}

pub struct ParseOutput<'a> {
    /// Present when the parser can build a recoverable tree.
    pub comment: Option<Box<'a, JsdocBlock<'a>>>,
    /// Parser diagnostics. Recoverable diagnostics do not prevent `comment`
    /// from being returned.
    pub diagnostics: Vec<OxcDiagnostic>,
}
```

This is intentionally not `Result<JsdocBlock, Error>`.
Like `oxc`, parsing should be able to return a tree and diagnostics together
when recovery succeeds.

## Source lifetime contract

`source_text` and `allocator` should share the same lifetime `'a`.

That means the AST may contain borrowed slices into the original source:

```rust
JsdocDescriptionLine<'a> {
    span,
    description: &'a str,
}
```

The code that calls `parse_comment` must keep `source_text` alive for at least
as long as the returned AST. If that code owns the source as a `String`, that
`String` must outlive the `ParseOutput<'a>` and any AST consumers.
The same calling code must also keep `allocator` alive while the returned AST is
used; `ParseOutput<'a>` does not own the allocator.

This is the key contract that enables the borrowed-slice-first string policy.
If the JS-facing layer needs owned strings, it should create them during JSON
serialization, not during parsing.

## Span contract

All spans in the returned AST should use absolute byte offsets:

```text
absolute_span = base_offset + local_span_inside_source_text
```

This keeps the API usable both when parsing:

- a standalone JSDoc block
- a comment slice extracted from a larger JavaScript / TypeScript source file
- comments attached by a future `oxc` integration layer

The parser should not store separate line / column fields in the AST.
Line / column conversion belongs to diagnostic reporting or editor integration.

Because `Span` uses `u32` byte offsets, the parser should reject or report a
fatal diagnostic when `base_offset + source_text.len()` cannot fit in `u32`.

## Allocation contract

The parser should allocate only AST-owned node structure into `allocator`:

- `JsdocBlock`
- `JsdocDescriptionLine`
- `JsdocTag`
- `JsdocTagBody`
- `JsdocTypeLine`
- `JsdocInlineTag`
- `JsdocTypeSource`
- `JsdocNamepathSource`
- other typed AST nodes

The parser should not allocate owned strings for common-path text.
These should normally stay as borrowed source slices:

- `JsdocDescriptionLine.description`
- `JsdocTagName.value`
- `raw_body`
- inline tag body text
- raw type text
- parameter and name tokens

Arena string allocation is allowed only when parsing cannot represent the value
as a source slice. For v1, those cases should be rare and explicit.

Examples that should not allocate during parsing:

- `@param {string} userId - The user ID`
- `See {@linkcode Command | entry command}.`
- `@remarks Long text with no escape normalization`

Examples that may require later allocation outside the parser:

- a serializer needs owned JSON strings
- a formatter requests joined / normalized description text
- a validator or analyzer requests an unescaped value

## Diagnostics cost contract

Diagnostics should be cheap on the success path.

The parser should:

- keep successful parsing free of diagnostic message allocation
- create detailed diagnostic payloads only on malformed or recovered input
- prefer static diagnostic messages where possible
- attach diagnostics to `Span`, not to copied source snippets

The output uses a normal `Vec<OxcDiagnostic>` rather than an arena vector.
Diagnostics are not part of the AST memory layout and should be consumable
independently from traversal of the parsed tree.

## Fatal and recoverable outcomes

The parser should distinguish these cases:

1. **Successful parse**
   - `comment = Some(...)`
   - `diagnostics = []`
2. **Recoverable parse**
   - `comment = Some(...)`
   - `diagnostics` contains parser diagnostics
3. **Fatal parse**
   - `comment = None`
   - `diagnostics` explains why no usable tree could be produced

Examples:

- malformed inline tag: usually recoverable
- unclosed type expression: usually recoverable if the tag boundary remains clear
- input that is not a JSDoc block comment: fatal for `parse_comment`

## Minimal diagnostic model for v1

The v1 parser should keep the diagnostic set small.
The goal is to support recovery and useful spans, not to encode every semantic
JSDoc rule in the parser.

Recommended initial diagnostic kinds:

| Kind                       | Severity | Outcome              | Primary span              | Notes                                                                                                                           |
| -------------------------- | -------- | -------------------- | ------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `not_a_jsdoc_block`        | Error    | Fatal                | full input or first token | `parse_comment` received input that is not a `/** ... */` block                                                                 |
| `unclosed_block_comment`   | Error    | Fatal or recoverable | opening `/**`             | Fatal if no usable body can be bounded; recoverable if the code that calls `parse_comment` supplied a bounded comment slice     |
| `span_overflow`            | Error    | Fatal                | full input                | `base_offset + source_text.len()` does not fit in `u32`                                                                         |
| `unclosed_inline_tag`      | Error    | Recoverable          | opening `{@`              | Fallback should usually preserve the attempted inline tag as description text and avoid producing a misleading `JsdocInlineTag` |
| `unclosed_type_expression` | Error    | Recoverable          | opening `{` of the type   | Preserve `raw_body`; avoid committing a partial `JsdocTypeSource` unless the parser can do so safely                            |
| `unclosed_fence`           | Error    | Recoverable          | opening fence             | Treat the remaining content as fenced content or text, depending on the accepted description shape                              |
| `invalid_tag_start`        | Error    | Recoverable          | suspicious `@`            | Use when a line looks like a tag start but cannot produce a valid tag name                                                      |
| `invalid_inline_tag_start` | Error    | Recoverable          | suspicious `{@`           | Use when inline-tag scanning starts but the name/body boundary is invalid                                                       |

This list is intentionally parser-focused.
Do not include validator-level diagnostics such as:

- unknown tag
- invalid tag for the selected mode
- invalid namepath semantics
- invalid parameter path semantics
- type compatibility errors
- missing required description for a tag

Those belong to validator or analyzer phases.

## Diagnostic allocation rules

Parser diagnostics should follow these rules:

1. successful parsing should not allocate diagnostic messages
2. diagnostic messages should be static strings where possible
3. labels should be span-based and should not copy source snippets
4. diagnostics should be pushed only when an error path is taken
5. recovery should prefer one useful diagnostic over a cascade of follow-up diagnostics

For example, if an inline tag is unclosed:

```js
/**
 * See {@link UserService for details.
 */
```

The parser should usually emit one `unclosed_inline_tag` diagnostic on the
opening `{@` span and then keep the attempted inline tag in the affected
description line text instead of emitting a malformed `JsdocInlineTag`.
It should not emit additional noisy diagnostics for every later token in the
same description.

## Diagnostic rollback with checkpoints

Checkpoint rollback should also handle speculative diagnostics.

When a parser attempts an ambiguous interpretation, it should either:

1. avoid pushing diagnostics until the interpretation is accepted or finally fails, or
2. record `diagnostics.len()` in the checkpoint and truncate diagnostics on rollback

The first approach is preferred for v1.
It keeps checkpoints smaller and avoids making diagnostics part of the common
success path.

Use diagnostic rollback only if the implementation needs speculative diagnostics
to simplify a parser branch.

## Temporary parser state

Temporary scanner state should not leak into the AST.

Allowed implementation detail:

- stack-local counters for brace / bracket / paren depth
- stack-local quote and fence state
- small temporary standard-library vectors when recovery needs checkpoints

Not part of the core AST contract:

- scanner cursor state
- rollback checkpoints
- line-prefix stripping state
- detailed token streams that are not needed after AST construction

If later measurement shows that a separate event stream improves recovery or
benchmark behavior, it can be introduced internally. It should not be exposed as
the first public API.
