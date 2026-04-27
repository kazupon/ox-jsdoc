# Consumer Integration Guide — oxlint, oxfmt, and beyond

This guide is for **consumers of ox-jsdoc** (lint engines, formatters,
toolchains) who need:

- **Tag aliasing** support (e.g. treat `@arg` as `@param`).
- **Description preserve-whitespace** access (e.g. for Markdown / mdast
  re-formatting in oxfmt).

For the parser-side design rationale see
[`README.md`](./README.md). This guide focuses on **how to wire ox-jsdoc
into a host application** correctly.

## 1. Three description-related APIs at a glance

ox-jsdoc exposes three description-related surfaces on `JsdocBlock` /
`JsdocTag`. Pick the right one for your use case:

| API                                                  | Type                   | When to use                                                                                                        | Allocation                                         |
| ---------------------------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------- |
| **`description`** (field)                            | `Option<&'a str>`      | Lint rules that just need the body as a flat string (default for plugin-jsdoc rules).                              | Zero (arena-borrowed)                              |
| **`description_raw`** (field)                        | `Option<&'a str>`      | Advanced consumers that want to apply their own algorithm to the source slice with `*` prefix and blanks intact.   | Zero (arena-borrowed)                              |
| **`description_text(preserve_whitespace)`** (method) | `Option<Cow<'a, str>>` | Format-aware consumers that want either compact or preserve-whitespace output without rolling their own algorithm. | Zero on `false`, one `String` allocation on `true` |

The same shape exists on the JS decoder (Binary AST) side:

| Rust API                                    | JS decoder API                                        |
| ------------------------------------------- | ----------------------------------------------------- |
| `block.description` (field)                 | `block.description` (getter)                          |
| `block.description_raw` (field)             | `block.descriptionRaw` (getter, **compat-mode only**) |
| `block.description_text(preserve)` (method) | `block.descriptionText(preserveWhitespace?)` (method) |

### How "absence" is represented

| Layer                                                              | "no description" expression                                                                  |
| ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------- |
| Rust struct                                                        | `description: None` / `description_raw: None`                                                |
| Rust serializer (typed AST → JSON, `compat_mode = false`)          | field omitted (`skip_serializing_if = "Option::is_none"`)                                    |
| Rust serializer (typed AST → JSON, `compat_mode = true`)           | field present as `null`                                                                      |
| JS decoder (Binary AST, parsed with `preserveWhitespace: true`)    | `block.description === null` / `block.descriptionRaw === null`                               |
| JS decoder (Binary AST, parsed without `preserveWhitespace: true`) | `block.descriptionRaw === null` always (the wire field is omitted, regardless of compatMode) |

> **JS subtlety**: `descriptionText(true)` returns `null` whenever the
> buffer was **not** parsed with `preserveWhitespace: true`. The flag is
> orthogonal to `compatMode`, so any of:
> `parse(src, { preserveWhitespace: true })` /
> `parse(src, { compatMode: true, preserveWhitespace: true })` works.
> There is no fallback to the compact view — if you want
> preserve-whitespace reliably, opt in at parse time.

## 2. Tag aliasing: implementing `tagNamePreference`

ox-jsdoc's parser is **tag-name-agnostic** — every tag, known or custom,
parses into the same `JsdocTag` shape. Only `tag.value` (Rust) /
`tag.tag` (JS) differs. Aliasing is the **consumer's** responsibility.

### 2.1 Rust consumer pattern (oxlint-style)

The reference implementation lives in
[`oxc_linter::JSDocPluginSettings`](../../refers/oxc/crates/oxc_linter/src/config/settings/jsdoc.rs).
The minimum scaffolding for your own lint engine:

```rust
use rustc_hash::FxHashMap;

pub struct JsdocSettings {
    /// Maps **canonical name** → **user-preferred name**.
    /// e.g. `"param" → "arg"` means "in this project, write @arg
    /// where @param would be expected".
    tag_name_preference: FxHashMap<String, String>,
}

impl JsdocSettings {
    /// Resolve the canonical name to the user-preferred one. If no
    /// preference is set, returns the canonical name unchanged.
    pub fn resolve_tag_name<'s>(&'s self, canonical: &'s str) -> &'s str {
        self.tag_name_preference
            .get(canonical)
            .map(String::as_str)
            .unwrap_or(canonical)
    }
}

// In a lint rule that wants to find @param tags:
let resolved = settings.resolve_tag_name("param");   // returns "arg" if user aliased
for tag in &block.tags {
    if tag.tag.value != resolved { continue; }
    // ... validate the tag (type, name, description, ...)
}
```

For a richer implementation (block / replace / message variants), see
the upstream `TagNamePreference` enum referenced in
[`README.md` §2.1](./README.md#21-tag-aliasing--fully-consumer-side).

### 2.2 JS consumer pattern (ESLint plugin-style)

ox-jsdoc's binary AST decoder exposes `tag.tag` (a string getter) on
`RemoteJsdocTag`. The shape mirrors the Rust contract:

```js
import { parse } from 'ox-jsdoc-binary' // (or @ox-jsdoc/wasm-binary)

function getPreferredTagName(canonical, settings) {
  return settings?.tagNamePreference?.[canonical] ?? canonical
}

const { ast } = parse(commentSource, { compatMode: true })
const resolved = getPreferredTagName('param', context.settings.jsdoc)
for (const tag of ast.tags) {
  if (tag.tag !== resolved) continue
  // ... validate
}
```

### 2.3 Default alias map (eslint-plugin-jsdoc parity)

eslint-plugin-jsdoc ships 16 default aliases. Both
`oxc_linter::JSDocPluginSettings` and any third-party host that wants
ESLint parity should hardcode these as fallbacks (i.e. apply them when
`tag_name_preference` has no entry for the canonical name):

```text
arg | argument → param
return         → returns
prop           → property
const          → constant
desc           → description
func | method  → function
var            → member
arg | argument → param
exception      → throws
yield          → yields
virtual        → abstract
extends        → augments
constructor    → class
defaultvalue   → default
host           → external
fileoverview | overview → file
emits          → fires
```

(Full list at
[`refers/oxc/crates/oxc_linter/src/config/settings/jsdoc.rs:126-143`](../../refers/oxc/crates/oxc_linter/src/config/settings/jsdoc.rs).)

## 3. Preserve-whitespace description: oxfmt integration

For format-side use cases (oxfmt, prettier-plugin-jsdoc, custom
re-flow tools), use the `description_text(preserve_whitespace=true)`
path.

### 3.1 What "preserve-whitespace" guarantees

| Property                                   | Compact (`preserve_whitespace = false`) | Preserve (`preserve_whitespace = true`)               |
| ------------------------------------------ | --------------------------------------- | ----------------------------------------------------- |
| `*` prefix stripped                        | ✓                                       | ✓                                                     |
| Blank lines (paragraph breaks) preserved   | ✗ (filtered out)                        | **✓**                                                 |
| Indentation past the `* ` prefix preserved | ✗ (collapsed)                           | **✓** (keeps Markdown indented code blocks intact)    |
| Markdown emphasis (`*foo*`) `*` retained   | partially                               | **✓** (algorithm distinguishes from continuation `*`) |

Algorithm details: [`README.md` §3](./README.md#3-the-parsed_preserving_whitespace-algorithm).

### 3.2 oxfmt-style consumer (Rust)

```rust
use ox_jsdoc::parse_comment;
use oxc_allocator::Allocator;

let arena = Allocator::default();
let block = parse_comment(&arena, comment_src, /* base_offset */ 0, options)
    .comment
    .expect("comment parsed");

// Compact view (default for lint rules):
let compact = block.description_text(false);
// → Some(Cow::Borrowed("First paragraph.\nSecond paragraph."))

// Preserve view (for formatters):
let preserved = block.description_text(true);
// → Some(Cow::Owned("First paragraph.\n\nSecond paragraph."))
//                                       ^^ paragraph break preserved

// Feed `preserved` into your Markdown / mdast pipeline:
let mdast = markdown_to_mdast(preserved.as_deref().unwrap_or(""));
// ...
```

### 3.3 oxfmt-style consumer (JS, Binary AST decoder)

```js
import { parse } from 'ox-jsdoc-binary'

// `preserveWhitespace: true` is required for descriptionRaw /
// descriptionText(true). It is fully orthogonal to compatMode — combine
// only when you also need the jsdoccomment AST shape (delimiters / line
// indices / source[]).
const { ast } = parse(commentSource, { preserveWhitespace: true })
if (!ast) return

// Compact view (works without any opt-in too):
const compact = ast.descriptionText(false)
// → "First paragraph.\nSecond paragraph."

// Preserve view (requires preserveWhitespace: true at parse time):
const preserved = ast.descriptionText(true)
// → "First paragraph.\n\nSecond paragraph."

// Same on each tag:
for (const tag of ast.tags) {
  const tagDesc = tag.descriptionText(true)
  // ...
}
```

## 4. Choosing parse modes

Three orthogonal flags control which APIs are available on the Binary AST:

| Flag                                                    | Default | Purpose                                                                                                                  |
| ------------------------------------------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------ |
| `parse_types: bool`                                     | `false` | Enable structured type-expression parsing (`tag.parsed_type`). Cost: ~1.1× parse time.                                   |
| `compat_mode: bool` (Binary AST)                        | `false` | Enable jsdoccomment-compat fields (delimiters, line indices, …) in the binary buffer.                                    |
| `preserve_whitespace: bool` (Binary AST)                | `false` | Emit `description_raw_span`, enabling `descriptionRaw` getter and `descriptionText(true)`. Adds 8 bytes per Block / Tag. |
| `SerializeOptions.compat_mode: bool` (typed AST → JSON) | `false` | Same idea on the JSON serialization side: emit compat fields including **`descriptionRaw`**.                             |

`compat_mode` and `preserve_whitespace` are **fully orthogonal** on the
Binary AST. The typed AST → JSON path uses `compat_mode` only (see
[`README.md` §4.4 "Asymmetry"](./README.md#asymmetry-vs-binary-ast-decoder-intentional)
for the rationale).

### 4.1 Decision matrix

| Use case                                     | `parse_types` | `compat_mode` | `preserve_whitespace` |
| -------------------------------------------- | :-----------: | :-----------: | :-------------------: |
| Basic lint rules (no type validation)        |       ✗       |       ✗       |           ✗           |
| Lint with `@param {Type}` validation         |       ✓       |       ✗       |           ✗           |
| jsdoccomment-compatible AST shape            |       ✗       |       ✓       |           ✗           |
| oxfmt formatting (preserve-whitespace only)  |       ✗       |       ✗       |         **✓**         |
| oxfmt + jsdoccomment shape together          |       ✗       |       ✓       |         **✓**         |
| Full feature parity with eslint-plugin-jsdoc |       ✓       |       ✓       |         **✓**         |

### 4.2 Cost summary (typescript-checker.ts fixture, 232 comments)

| Mode                                             | NAPI parseBatch median | Buffer size | Note                                                                   |
| ------------------------------------------------ | ---------------------: | ----------: | ---------------------------------------------------------------------- |
| `compatMode: false` (default)                    |                ~317 µs |     ~170 KB | Smallest binary buffer, leanest decoder API                            |
| `preserveWhitespace: true` (basic mode + opt-in) |      ~317 µs + ~3.6 KB |     ~174 KB | basic + 8 bytes per Block / Tag with description; descriptionRaw works |
| `compatMode: true`                               |                ~355 µs |     ~200 KB | Full compat tail (delimiter strings, line indices) — no descriptionRaw |
| `compatMode: true, preserveWhitespace: true`     |                ~360 µs |     ~204 KB | compat tail + 8 bytes per Block / Tag — full feature set               |
| `compatMode: true, emptyStringForNull: true`     |                ~370 µs |     ~200 KB | Same wire size; converts absent strings to `""` in `toJSON()`          |
| `parseTypes: true` (orthogonal)                  |                  +~10% |      +small | Type-expression sub-parser cost                                        |

`preserveWhitespace` adds 8 bytes per `JsdocBlock` and 8 bytes per
`JsdocTag` (≈ 3.6 KB on this fixture), independent of `compatMode`.
Basic-mode buffers without `preserveWhitespace` carry **zero** overhead
for this feature. The ~30 KB compat-mode growth (~170 KB → ~200 KB) is
the existing compat tail (delimiter `StringField`s + line indices), not
the `description_raw_span` slot.

## 5. Migration / wiring into existing toolchains

### 5.1 ESLint via `@es-joy/jsdoccomment`

`@es-joy/jsdoccomment` exposes `parseComment(text)` internally. Replacing
that with ox-jsdoc's compat-mode output preserves every eslint-plugin-jsdoc
rule's contract:

1. Build the JSON via `serialize_comment_json_with_options(comment,
None, None, &SerializeOptions { compat_mode: true, empty_string_for_null: true, .. })`.
2. The output's shape matches jsdoccomment's `commentParserToESTree`
   output (per [`design/005-jsdoccomment-compat`](../005-jsdoccomment-compat/README.md)).
3. eslint-plugin-jsdoc sees the same fields it always saw — including
   `descriptionRaw` if it ever needs the source-shape view.

No changes required in eslint-plugin-jsdoc itself.

### 5.2 oxlint plugin-jsdoc (in-tree consumer)

oxlint already consumes `oxc_jsdoc` (upstream) for parsing and
`JSDocPluginSettings::resolve_tag_name` for aliasing. To migrate to
ox-jsdoc:

1. Swap the parser dependency from `oxc_jsdoc` to `ox_jsdoc`.
2. Update `tag.kind.parsed()` call sites to `tag.tag.value`
   (semantically identical, naming difference only).
3. For description access, the `description` field works as a drop-in
   replacement for `tag.comment().parsed()`.

### 5.3 oxfmt jsdoc formatter

1. Parse with `compat_mode: true` (or use the Binary AST decoder).
2. Read description via `descriptionText(true)` on `JsdocBlock` and each
   `JsdocTag`.
3. Feed the result into your Markdown / mdast pipeline as the
   `* ` prefixes have already been stripped while preserving paragraph
   structure and indentation.

The upstream
[`refers/oxc/crates/oxc_formatter/src/formatter/jsdoc/serialize.rs`](../../refers/oxc/crates/oxc_formatter/src/formatter/jsdoc/serialize.rs)
implementation walks `JSDocCommentPart::parsed_preserving_whitespace`
exactly the same way.

## 6. FAQ

**Q. Can I check `tag.value === "param"` in a hot loop?**
A. Yes — `tag.value` is a borrow into the source text (`&'a str`), no
allocation. String comparison is O(name length). For sub-µs hot paths,
hoist the `resolve_tag_name(...)` call outside the loop.

**Q. Why two methods on JS (`description` getter + `descriptionText`)?**
A. The `description` getter mirrors today's compact behavior (always
available, zero-cost). `descriptionText(preserveWhitespace?)` is the new
API that lets formatters opt into the preserve-whitespace path. They
return identical strings when `preserveWhitespace = false`.

**Q. What if the user calls `descriptionText(true)` without opting in?**
A. Returns `null` (the per-node `description_raw_span` wire field is
absent). This is intentional — see
[`README.md` §4.3](./README.md#43-js-decoder-api). To get preserve-whitespace
reliably, parse with `preserveWhitespace: true` (orthogonal to
`compatMode`).

**Q. How do I serialize ox-jsdoc output to disk for cross-process use?**
A. Use the Binary AST API: `parse_to_bytes` / `parse_batch_to_bytes`
(Rust) or the `parseBatch` JS API. The resulting buffer contains
`descriptionRaw` only when `preserveWhitespace: true` was set at parse
time (regardless of `compatMode`).

## References

- Design rationale & API spec: [`./README.md`](./README.md)
- Algorithm details: [`./README.md` §3](./README.md#3-the-parsed_preserving_whitespace-algorithm)
- Upstream implementation references: [`./README.md` §9](./README.md#9-references)
- jsdoccomment AST shape baseline: [`design/005-jsdoccomment-compat/`](../005-jsdoccomment-compat/README.md)
- Binary AST format spec: [`design/007-binary-ast/format.md`](../007-binary-ast/format.md)
