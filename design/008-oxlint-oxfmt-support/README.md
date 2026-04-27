# oxlint / oxfmt Support — JSDoc Tag Aliasing & Preserve-Whitespace Description

## Goals

This document covers two related consumer-driven requirements for ox-jsdoc:

1. **JSDoc tag aliasing for any linter** — let consumers configure arbitrary
   strings as synonyms for standard tag names (`@arg` ≡ `@param`, `@return` ≡
   `@returns`, project-defined `@kebab-tag`, …). The original feature ships
   in [`eslint-plugin-jsdoc`'s `tagNamePreference` setting][epj-settings];
   `oxlint plugin-jsdoc` is a port that retains the same semantics. ox-jsdoc
   must therefore work for **both ESLint (via `@es-joy/jsdoccomment`) and
   oxlint** as the parser they consume.
2. **oxfmt `jsdoc` format (preserve-whitespace description)** — surface
   description text with paragraph breaks, indented code blocks (4+ spaces),
   and other vertical structure intact, so downstream Markdown / mdast
   processors can re-flow it without losing semantic information.

Both features pivot on the same principle: **the parser should stay neutral;
shape-preservation and per-tag policy belong on the consumer side**.

[epj-settings]: https://github.com/gajus/eslint-plugin-jsdoc/blob/v50.5.0/docs/settings.md#tagnamepreference

## Strategy

### 1. Parser stays tag-name-agnostic (uniform parse)

Every tag — known or custom — is parsed into the same `JsdocTag` shape:

```text
{ tag.value: "param" / "arg" / "kebab-tag",
  raw_type:    Option<...>,
  name:        Option<...>,
  description: Option<&str>, ... }
```

The parser **does not branch on `tag.value`**. Lint rules and formatters
resolve aliases / dispatch behavior at consumption time.

This mirrors `oxc_jsdoc::JSDocTag` (`refers/oxc/crates/oxc_jsdoc/src/parser/jsdoc_tag.rs:9-31`):

> However, I discovered that some use cases, like `eslint-plugin-jsdoc`,
> provide an option to create an alias for the tag kind. … We cannot assume
> that `@param` will always map to `JSDocParameterTag`.

ox-jsdoc inherits this design (`crates/ox_jsdoc/src/parser/parse.rs::parse_jsdoc_tag`
contains no tag-name dispatch).

### 2. Description text has two reader contracts

The same description body needs two consumer views:

| Reader contract                                   | Use case                                                             | Output                                                                                                                         |
| ------------------------------------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| **Compact** (today's `description: Option<&str>`) | linters reading the body as flat text                                | `\n`-joined non-empty lines, `*` prefixes stripped                                                                             |
| **Preserve-whitespace** (new)                     | formatters re-emitting Markdown / code fences / indented code blocks | All lines kept (including blanks); `*` prefix stripped at most once with one trailing space; markdown emphasis `*word*` exempt |

Both views must be derivable from the **same parser output** (no second
parse, no AST mutation). This drives the API choice in [§4](#4-api-design).

## 1. Background

### 1.1 Why aliasing matters (any JSDoc-aware linter)

The original feature lives in [`eslint-plugin-jsdoc`][epj]: the
`tagNamePreference` setting lets users say "in this project, write `@arg`
instead of `@param`":

```jsonc
// .eslintrc settings.jsdoc
{
  "tagNamePreference": {
    "param": "arg", // simple replacement
    "qux": { "message": "...", "replacement": "quux" }, // replacement + custom message
    "bar": { "message": "do not use bar" }, // block-only
    "foo": false // banned (default message)
  }
}
```

Two known consumers inherit this contract today:

| Consumer                           | Path                                                  | Notes                                                                                               |
| ---------------------------------- | ----------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| **ESLint** + `eslint-plugin-jsdoc` | runs on top of [`@es-joy/jsdoccomment`][jsdoccomment] | If ox-jsdoc replaces jsdoccomment's internal `parseComment()`, every plugin rule must keep working. |
| **oxlint** + `plugin-jsdoc`        | `refers/oxc/crates/oxc_linter/src/rules/jsdoc/`       | Direct Rust port of eslint-plugin-jsdoc; same settings shape, same alias defaults.                  |

In both cases the lint rule cannot hardcode "look for `@param`" — it must
consult its host's settings to discover the user's preferred spelling, then
iterate parser output filtering by that string. **ox-jsdoc's job is to make
the raw tag name (`tag.value`) available unfiltered**; the host (ESLint
plugin code or oxlint rule) does the alias resolution.

Future linter integrations (e.g. Biome, custom in-house tooling) follow the
same pattern. Anywhere the consumer wants to honor `tagNamePreference`-style
config, ox-jsdoc must not pre-empt that decision.

[epj]: https://github.com/gajus/eslint-plugin-jsdoc
[jsdoccomment]: https://github.com/es-joy/jsdoccomment

### 1.2 Why preserve-whitespace matters (oxfmt)

oxfmt formats JSDoc descriptions as Markdown. Consider:

```js
/**
 * Some intro.
 *
 *     code block (4-space indent)
 *
 * Second paragraph.
 *
 * @param x
 */
```

A formatter walking `description = "Some intro.\ncode block...\nSecond paragraph."`
loses both the **blank-line paragraph breaks** and the **4-space indent
distinguishing code from prose**. It cannot re-flow the body without
corrupting the code block.

The reader needs to see the text **with the comment chrome (`*` prefixes)
stripped but the line shape intact**.

## 2. Reference: How upstream `oxc` solves these

### 2.1 Tag aliasing — fully consumer-side

Two reference implementations exist; both keep the parser tag-name-agnostic
and resolve aliases on the consumer side.

#### eslint-plugin-jsdoc (the original)

The `tagNamePreference` setting is read by every plugin rule (see e.g.
`require-param`, `check-tag-names` in
[gajus/eslint-plugin-jsdoc][epj]). The plugin uses helper functions like
`getPreferredTagName(name, settings)` and walks `@es-joy/jsdoccomment`'s
parse output to find tags by the resolved name. Defaults include `arg →
param`, `return → returns`, `prop → property`, etc.

#### oxc_linter (Rust port of the same contract)

`oxc_linter::JSDocPluginSettings` (`refers/oxc/crates/oxc_linter/src/config/settings/jsdoc.rs`)
ports the eslint-plugin-jsdoc contract field-for-field:

```rust
pub struct JSDocPluginSettings {
    tag_name_preference: FxHashMap<String, TagNamePreference>,
    // ... other settings
}

#[serde(untagged)]
enum TagNamePreference {
    TagNameOnly(String),                                    // "param": "arg"
    ObjectWithMessageAndReplacement { message, replacement }, // detailed
    ObjectWithMessage { message: String },                  // block-only
    FalseOnly(bool),                                        // "foo": false  (banned)
}
```

`#[serde(untagged)]` auto-routes the four JSON shapes into one enum.

Plus a hardcoded **default alias map** for eslint-plugin-jsdoc parity
(`check_preferred_tag_name`):

```rust
"virtual" → "abstract", "extends" → "augments", "constructor" → "class",
"const" → "constant", "desc" → "description", "func" | "method" → "function",
"arg" | "argument" → "param", "prop" → "property", "return" → "returns",
"exception" → "throws", "yield" → "yields", ... // (16 entries total)
```

Four public methods cover every aliasing operation:

| Method                                               | Purpose                                                         |
| ---------------------------------------------------- | --------------------------------------------------------------- |
| `resolve_tag_name(orig) -> &str`                     | "given the canonical name, what should I look for in the file?" |
| `check_blocked_tag_name(name) -> Option<Cow<str>>`   | block diagnostics                                               |
| `check_preferred_tag_name(orig) -> Option<Cow<str>>` | replacement diagnostics                                         |
| `list_user_defined_tag_names() -> Vec<&str>`         | whitelist for `check-tag-names`                                 |

A typical rule (`require-param-type`):

```rust
let resolved = settings.resolve_tag_name("param");   // returns "arg" if user aliased
for tag in jsdoc.tags() {
    if tag.kind.parsed() != resolved { continue; }
    let (type_part, name_part, _) = tag.type_name_comment();
    // ... validate type
}
```

The parser provides the raw `tag.kind.parsed()` string; the rule does the
alias lookup and equality check.

### 2.2 Preserve-whitespace — post-parse method on raw slice

`oxc_jsdoc::JSDocCommentPart` (`refers/oxc/crates/oxc_jsdoc/src/parser/jsdoc_parts.rs:97-124`)
exposes two methods over the same raw `&str`:

| Method                                     | Output                                                                                                       |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------ |
| `parsed() -> String`                       | Compact view (existing): trim, drop empty lines, strip `* ` prefix                                           |
| `parsed_preserving_whitespace() -> String` | Preserve view (new since #19828): keep blank lines, preserve indentation past `* `, exempt markdown emphasis |

oxfmt heavily depends on both
(`refers/oxc/crates/oxc_formatter/src/formatter/jsdoc/serialize.rs:117, 296`):

```rust
let description = comment_part.parsed_preserving_whitespace();
// ...
let raw_ws = tag.comment().parsed_preserving_whitespace();
let source_has_trailing_blank = raw_ws.ends_with("\n\n");
```

The key design choice: **operate on a raw `&str` post-parse**, not on
already-split-and-filtered description lines. No AST changes, no second
parse, just a different walk over the same text.

## 3. The `parsed_preserving_whitespace` Algorithm

Adapted from `refers/oxc/crates/oxc_jsdoc/src/parser/jsdoc_parts.rs:97-124`:

```rust
fn parsed_preserving_whitespace(raw: &str) -> String {
    if !raw.contains('\n') {
        return raw.trim().to_string();
    }
    let mut result = String::with_capacity(raw.len());
    for (i, line) in raw.lines().enumerate() {
        if i > 0 { result.push('\n'); }
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('*') {
            // markdown emphasis (`*word*`) is NOT a comment-continuation prefix
            let is_emphasis =
                rest.starts_with(|c: char| c.is_alphanumeric() || c == '_');
            if !is_emphasis {
                // Strip at most ONE leading space after `*` —
                // any extra indent (e.g. for indented code blocks) is preserved.
                result.push_str(rest.strip_prefix(' ').unwrap_or(rest));
                continue;
            }
        }
        result.push_str(trimmed);
    }
    result
}
```

### 3.1 Step-by-step

1. **Single-line fast path**: no newline → return `raw.trim()`.
2. For each line (zero-based index):
   - Push `\n` between lines (so the join is exactly `lines().count() - 1` newlines).
   - `line.trim()` to remove both-end whitespace.
   - If the trimmed line starts with `*`:
     - Inspect the character right after `*`:
       - **Alphanumeric or `_`** → markdown emphasis (`*foo*`, `*bold*`). Do NOT strip the `*`. Push the trimmed line as-is.
       - **Anything else** (space / EOL / `\``  / punctuation) → comment-continuation prefix. Strip the `\*`, then strip **at most one leading space**. Preserve any remaining indentation.
   - Otherwise push the trimmed line.

### 3.2 Why "exactly one space"

JSDoc convention is `* ` (asterisk + one space) before content. Stripping
exactly that pair preserves the user's actual indentation:

```text
 * normal text          → "normal text"        (1 space after *, stripped)
 *     indented code    → "    indented code"  (1 stripped, 4 remain)
 *no_space              → "no_space"           (no space to strip; passthrough)
 *foo*                  → "*foo*"              (markdown emphasis, * kept)
```

The 4-space remnant is what Markdown / CommonMark renderers interpret as an
indented code block.

### 3.3 Edge cases tested upstream

`refers/oxc/crates/oxc_jsdoc/src/parser/jsdoc_parts.rs:298-367` covers
(reusable as ox-jsdoc fixtures):

| Input                            | `parsed()`        | `parsed_preserving_whitespace()`   |
| -------------------------------- | ----------------- | ---------------------------------- |
| `""`                             | `""`              | `""`                               |
| `"hello  "`                      | `"hello"`         | `"hello"`                          |
| `"  * single line"`              | `"* single line"` | `"* single line"`                  |
| `" * "`                          | `"*"`             | `"*"`                              |
| `" * * "`                        | `"* *"`           | `"* *"`                            |
| `"\n     * asterisk\n    "`      | `"asterisk"`      | (preserved with leading newline)   |
| `"\n     * * li\n     * * li\n"` | `"* li\n* li"`    | (same shape)                       |
| `"\n    1\n\n    2\n\n    3\n"`  | `"1\n2\n3"`       | `"1\n\n2\n\n3"` (blank lines kept) |

## 4. API Design

### 4.1 Rust side (typed AST)

`JsdocBlock` / `JsdocTag` carry a new **raw description slice** field,
**populated unconditionally at parse time** (always present when the node
has a description, `None` otherwise). Storage cost is one fat pointer
(16 bytes / node) borrowed from the arena — no extra allocation.

```rust
pub struct JsdocBlock<'a> {
    // ... existing fields, including:
    //     pub description: Option<&'a str>,   // compact view (unchanged)
    pub description_raw: Option<&'a str>,      // NEW: raw slice with `*` prefix + newlines
}

pub struct JsdocTag<'a> {
    // ... existing fields, including:
    //     pub description: Option<&'a str>,   // compact view (unchanged)
    pub description_raw: Option<&'a str>,      // NEW
}
```

#### Boundary definition

`description_raw` is the **byte-exact source slice covering the AST's
`description_lines` range** — concretely:

```rust
description_raw = source_text[
    description_lines.first().span.start .. description_lines.last().span.end
]
```

Where `span` is the existing per-`JsdocDescriptionLine` field (absolute
UTF-8 byte offsets into the original source text).

#### Two `description_lines` shapes (parser internals)

The current parser (`crates/ox_jsdoc/src/parser/context.rs`) builds
`description_lines` differently for `JsdocBlock` vs `JsdocTag`:

| Node         | Builder                          | Shape                                                                                                                                                                                                 |
| ------------ | -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `JsdocBlock` | `parse_description_lines` (L533) | One entry **per non-empty `LogicalLine`**. Each `span` covers that line's content range in the original source.                                                                                       |
| `JsdocTag`   | `parse_description_text` (L566)  | A **single synthetic entry** spanning the post-header description region (after `@tag {type} name`). `span` is computed via `relative_span(normalized.span, remainder_start, normalized.text.len())`. |

The boundary rule above works for both because:

- For `JsdocBlock` (multi-entry): `first.span.start` is the first line's
  content start, `last.span.end` is the last non-blank line's content
  end. **Intermediate blank lines that were filtered out of
  `description_lines` are still included in the slice** (since the slice
  spans across them, picking up their `*` prefixes verbatim). The
  algorithm in [§3](#3-the-parsed_preserving_whitespace-algorithm) handles
  the prefixes.
- For `JsdocTag` (single-entry): `first.span == last.span`, so the slice
  is exactly that single span. Blank lines within the tag's description
  are picked up by the same span-spans-across-source mechanism (the
  `relative_span` mapping preserves source byte positions even when
  `normalized.text` was joined / trimmed).

#### `None` cases

- `description_raw == None` ⇔ `description_lines.is_empty()` (no
  description content at all — only whitespace, or no description text
  before the first `@tag` / closing `*/`).
- Trailing or leading blank lines outside the first/last non-empty
  description lines are **not** part of the slice, matching the existing
  parser's "drop empty lines" semantics.

#### Public method

A single method exposes both reader contracts via a boolean flag:

```rust
impl<'a> JsdocBlock<'a> {
    /// Description text. When `preserve_whitespace` is `true`, blank lines
    /// and indentation past the `* ` prefix are preserved (see [§3](#3-the-parsed_preserving_whitespace-algorithm)).
    /// When `false`, returns the compact view (existing `description` field
    /// behavior). `None` when no description present.
    pub fn description_text(
        &self,
        preserve_whitespace: bool,
    ) -> Option<Cow<'a, str>> {
        if preserve_whitespace {
            self.description_raw.map(|raw| Cow::Owned(parsed_preserving_whitespace(raw)))
        } else {
            self.description.map(Cow::Borrowed)
        }
    }
}

// Same shape on JsdocTag.
```

`Cow` lets the compact path stay zero-allocation while the preserve path
materializes a `String`.

The existing `description: Option<&'a str>` field stays public for backward
compatibility and zero-cost direct access.

### 4.2 Binary AST wire format (opt-in via `preserve_whitespace` parse option)

Binary AST gates `description_raw_span` on a **per-node Common Data bit
plus a `preserve_whitespace` parse option** — both basic-mode and
compat-mode buffers can opt in (or out), so consumers can pick the wire
size vs feature trade-off independently of the jsdoccomment-shape choice.

#### Gating

| `compat_mode` | `preserve_whitespace` | Effect                                                                                                     |
| ------------- | --------------------- | ---------------------------------------------------------------------------------------------------------- |
| `false`       | `false` (default)     | basic mode, **no** `description_raw_span` in ED — `descriptionRaw` / `descriptionText(true)` return `null` |
| `false`       | `true`                | basic mode + 8-byte span appended to ED tail — `descriptionRaw` works                                      |
| `true`        | `false`               | compat tail without `description_raw_span` (jsdoccomment-shape only)                                       |
| `true`        | `true`                | compat tail + 8-byte span (full jsdoccomment-shape **and** preserve-whitespace API)                        |

`compat_mode = true` does **not** auto-imply `preserve_whitespace = true`;
the two flags are fully orthogonal so consumers that want jsdoccomment
shape only (without descriptionRaw) can save the 8 bytes/node.

#### Common Data presence bit

A single bit in the Node Record's Common Data byte signals presence of
the trailing 8-byte span:

| Node         | Common Data layout                                                                     |
| ------------ | -------------------------------------------------------------------------------------- |
| `JsdocBlock` | bit 0 = `has_description_raw_span`. (bits 1–5 reserved)                                |
| `JsdocTag`   | bit 0 = `optional` (existing); bit 1 = `has_description_raw_span`. (bits 2–5 reserved) |

When the bit is set, the parent's Extended Data record is **8 bytes
larger**. The span sits at the very end of the ED record:

```text
description_raw_span (8 bytes, appended to ED tail when the bit is set):
  byte X..X+4   : description_raw_start (u32, UTF-8 byte offset in source text)
  byte X+4..X+8 : description_raw_end   (u32, UTF-8 byte offset in source text)

  X = ED base size (basic / compat) — see §5.2 for the per-mode numbers.
```

Empty descriptions are represented by **omitting the span entirely**
(bit cleared, no 8 bytes reserved). The wire format does not need an
in-band sentinel: the bit answers presence directly.

#### Encoding unit (UTF-8)

`description_raw_span` is **UTF-8 bytes**, _not_ UTF-16 code units. This
differs intentionally from `Pos` / `End` (which are UTF-16 code units,
converted at emit time for ESLint / LSP compatibility). The contrast:

| Field                  | Unit        | Purpose                                                       |
| ---------------------- | ----------- | ------------------------------------------------------------- |
| `Pos` / `End`          | UTF-16 cu   | ESLint `range`, LSP positions — JS-facing                     |
| `description_raw_span` | **UTF-8 b** | Decoder slices `String Data` bytes directly — internal helper |

Choosing UTF-8 here keeps the decoder's slice operation a single integer
range over the existing UTF-8 String Data section — no `Utf16PositionMap`
round-trip at emit or decode. This matches `oxc_jsdoc::JSDocCommentPart::span`,
which is also UTF-8 bytes.

ox-jsdoc is **pre-release**, so the opt-in layout is shipped **without
a format version bump**. The compat tail spec in
`007-binary-ast/format.md` is amended in place. After 1.0 the format
version stability rules from `007-binary-ast/phases.md` apply.

### 4.3 JS decoder API

The decoder mirrors the Rust API exactly:

```ts
class RemoteJsdocBlock {
  /** Raw description slice (with `*` prefix and newlines).
   *  Returns null when the buffer was not parsed with
   *  `preserveWhitespace: true`, or when the block has no description. */
  get descriptionRaw(): string | null

  /** Description text. When `preserveWhitespace` is true, blank lines and
   *  indentation past `* ` are preserved (algorithm: see §3).
   *  When false (default), returns the compact view (same as `description`).
   *  Returns null when no description present, or when
   *  `preserveWhitespace=true` is requested on a buffer that wasn't
   *  parsed with the matching `preserveWhitespace: true` parse option. */
  descriptionText(preserveWhitespace?: boolean): string | null
}

class RemoteJsdocTag {
  get descriptionRaw(): string | null
  descriptionText(preserveWhitespace?: boolean): string | null
}
```

`descriptionText(false)` (or omitted argument) returns the compact view via
the existing `description` getter — works in **any** buffer regardless of
parse options. `descriptionText(true)` requires the buffer to have been
parsed with `preserveWhitespace: true` so the per-node
`description_raw_span` is present in the Extended Data record.

The parse option matrix (oxlint / oxfmt API):

```ts
parse(src, {}) // descriptionRaw = null
parse(src, { compatMode: true }) // descriptionRaw = null
parse(src, { preserveWhitespace: true }) // descriptionRaw = string ✓
parse(src, { compatMode: true, preserveWhitespace: true }) // descriptionRaw = string ✓
```

#### Slice resolution

The `descriptionRaw` getter materializes the string by:

1. Reading the node's Common Data byte and checking the
   `has_description_raw_span` bit (bit 0 for `JsdocBlock`, bit 1 for
   `JsdocTag`). If clear → return `null`.
2. Reading `description_raw_span` (two `u32` values, **UTF-8 byte offsets**)
   from the **last 8 bytes** of the parent's Extended Data record.
3. Resolving the source text region: `String Data[root.source_offset_in_data + start .. + end]`.
4. Decoding via `TextDecoder('utf-8')` → JS string.

No `Utf16PositionMap` lookup needed — UTF-8 → JS string conversion is the
single hot step.

### 4.4 JSON serialization (typed AST path)

The typed AST JSON serializer (`crates/ox_jsdoc/src/serializer/json.rs`)
emits `descriptionRaw` **only when `SerializeOptions.compat_mode` is `true`**:

```jsonc
// SerializeOptions { compat_mode: true, ... }
{
  "type": "JsdocBlock",
  "description": "First paragraph.\nSecond paragraph.",
  "descriptionRaw": "First paragraph.\n *\n * Second paragraph.",  // present
  "delimiter": "/**",
  // ... other compat-only fields
}

// SerializeOptions { compat_mode: false, ... }  (default)
{
  "type": "JsdocBlock",
  "description": "First paragraph.\nSecond paragraph.",
  // descriptionRaw is omitted
  // ... no compat-only fields
}
```

**Rationale**: matches the existing pattern for compat-only fields
(`delimiter`, `endLine`, line indices, …) — non-compat consumers see no
JSON shape change, compat consumers opt into the full extended payload.

#### Asymmetry vs Binary AST decoder (intentional)

Typed AST JSON gates `descriptionRaw` on `SerializeOptions.compat_mode`,
**not** on a separate `preserve_whitespace` flag. The Binary AST decoder
takes the opposite approach and uses the orthogonal `preserve_whitespace`
parse option (see [§4.2](#42-binary-ast-wire-format-opt-in-via-preserve_whitespace-parse-option)).
The asymmetry is deliberate:

- **Typed AST JSON path**: the natural consumer is jsdoccomment-shape
  tooling (eslint-plugin-jsdoc, IDE integrations) that reads the full
  compat payload anyway — bundling `descriptionRaw` with the rest of the
  compat fields keeps the JSON shape coherent and avoids a third option
  knob with negligible payload savings (one extra string per node).
- **Binary AST decoder path**: the typical perf-sensitive consumer
  (oxlint, batch tools) wants the smallest possible buffer and the
  leanest decoder API. A separate opt-in for the 8-byte
  `description_raw_span` keeps basic-mode buffers truly minimal even
  when compat shape is enabled.

`descriptionText` is a Rust method — not serialized to JSON. JS consumers
wanting the preserve-whitespace view either call `descriptionText(true)`
on the lazy decoder (Binary AST path, requires `preserveWhitespace: true`
at parse time) or apply the algorithm in
[§3](#3-the-parsed_preserving_whitespace-algorithm) themselves to
`descriptionRaw` (typed AST path, requires `compat_mode: true` at
serialize time).

### 4.5 Tag aliasing — no parser changes

ox-jsdoc keeps `tag.value: &str` as-is. Aliasing belongs in the
**consumer host**, regardless of which linter is consuming the parse output:

| Consumer host                          | Where aliasing lives                                                              |
| -------------------------------------- | --------------------------------------------------------------------------------- |
| ESLint + `eslint-plugin-jsdoc`         | Existing `getPreferredTagName(name, settings)` JS helpers (no change required)    |
| oxlint + `plugin-jsdoc`                | Existing Rust `JSDocPluginSettings::resolve_tag_name` (no change required)        |
| Future linters (Biome, custom tooling) | Must implement an equivalent `resolve_tag_name` step before iterating tag results |

The contract is the same in every case:

```rust
// Rust shape (oxlint-style)
let resolved = settings.resolve_tag_name("param");   // returns "arg" if user aliased
for tag in &block.tags {
    if tag.value != resolved { continue; }
    // ... type / name validation
}
```

```js
// JS shape (ESLint plugin-style; ox-jsdoc returns RemoteJsdocBlock)
const resolved = getPreferredTagName('param', context.settings.jsdoc)
for (const tag of block.tags) {
  if (tag.tag !== resolved) continue
  // ... type / name validation
}
```

ox-jsdoc ships **only test fixtures + documentation** for this pattern; the
actual settings struct lives in whichever lint engine consumes ox-jsdoc.

## 5. Compatibility Considerations

### 5.1 jsdoccomment AST shape

Tag aliasing requires zero changes here — `JsdocTag.value` is already the
raw tag name as a string.

Preserve-whitespace surfaces use **three different gating mechanisms** —
intentional asymmetry justified by each surface's typical consumer
(see [§4.4 "Asymmetry vs Binary AST decoder"](#asymmetry-vs-binary-ast-decoder-intentional)
for the rationale):

- **Typed AST struct**: new `description_raw: Option<&'a str>` field
  **unconditional** (always populated when present). Direct Rust
  consumers pay the 16-byte fat-pointer cost regardless.
- **Typed AST JSON**: `descriptionRaw` key emitted **only when
  `SerializeOptions.compat_mode = true`** — bundles with other compat
  fields (`delimiter`, `endLine`, …) for jsdoccomment-shape JSON
  consumers. See [§4.4](#44-json-serialization-typed-ast-path).
- **Binary AST decoder**: `descriptionRaw` getter + `descriptionText(true)`
  method available **only when the buffer was parsed with
  `preserve_whitespace: true`** (fully orthogonal to `compat_mode` so
  basic-mode + compat-mode buffers can both opt in or out). See
  [§4.2](#42-binary-ast-wire-format-opt-in-via-preserve_whitespace-parse-option)
  for the gating matrix.

Existing consumers reading `description` see no behavioral change in any
of these surfaces.

### 5.2 Binary AST size

`description_raw_span` is **opt-in per parse call** (see [§4.2](#42-binary-ast-wire-format-opt-in-via-preserve_whitespace-parse-option)).
When `preserve_whitespace = true`, the `JsdocBlock` and `JsdocTag` Extended
Data records each grow by **8 bytes** at the tail, signalled by the
`has_description_raw_span` Common Data bit:

| Node type    | basic ED (preserve = false) | basic ED (preserve = true) | compat ED (preserve = false) | compat ED (preserve = true) |
| ------------ | --------------------------: | -------------------------: | ---------------------------: | --------------------------: |
| `JsdocBlock` |                          68 |                **76** (+8) |                           90 |                 **98** (+8) |
| `JsdocTag`   |                          38 |                **46** (+8) |                           80 |                 **88** (+8) |

For the typescript-checker.ts fixture (226 `JsdocBlock`s, similar order of
`JsdocTag`s), the increment when `preserve_whitespace = true` is roughly:

```
226 blocks × 8 bytes + N_tags × 8 bytes ≈ 3.6 KB
```

(Exact `N_tags` depends on the fixture; ~226 yields ~3.6 KB.) — independent
of `compat_mode`. When `preserve_whitespace = false`, basic-mode and
compat-mode buffers carry **zero** overhead for this feature.

Wire-format compatibility: ox-jsdoc is pre-release, so the opt-in layout
ships **without a format minor version bump** — the compat tail spec in
`007-binary-ast/format.md` is amended in place. After 1.0 the version
stability rules from `007-binary-ast/phases.md` apply.

### 5.3 Typed AST memory cost

`description_raw: Option<&'a str>` is **always populated** on `JsdocBlock`
and `JsdocTag`. Storage is one fat pointer per node (16 bytes on 64-bit),
borrowed from the arena (`oxc_allocator::Allocator`) — zero additional
allocation. For the typescript-checker.ts fixture, this adds ~3.6 KB to
the typed AST footprint.

### 5.4 Performance

`description_text(true)` is `O(n)` over the raw slice and allocates a
`String` of similar size. Hot lint paths using `tag.description` directly
or `descriptionText(false)` (which returns `Cow::Borrowed`) are unaffected.
Formatter call paths see one extra allocation per tag description, matching
upstream's cost.

## 6. Implementation Roadmap

| Phase       | Scope                                                                                                                                                         | Risk                                          |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| **Phase 1** | Aliasing test fixtures + draft of [`consumer-guide.md`](./consumer-guide.md)                                                                                  | Low (no code change)                          |
| **Phase 2** | Rust: `description_raw` field + algorithm port + JSON serializer + tests                                                                                      | Medium (parser plumbing)                      |
| **Phase 3** | Binary AST: `description_raw_span` in compat tail (always-on, no version bump) + decoder API + bench                                                          | Medium (wire format)                          |
| **Phase 4** | Finalize [`consumer-guide.md`](./consumer-guide.md) + API reference                                                                                           | Low                                           |
| **Phase 5** | Opt-in `preserve_whitespace` ParseOption: gate `description_raw_span` on Common Data bit so basic-mode buffers can also opt in (instead of being compat-only) | Medium (wire format change to Phase 3 layout) |

Phase 2's typed-AST scope:

- `crates/ox_jsdoc/src/parser/scanner.rs` — no change (existing `LogicalLine.content_start/end` suffices)
- `crates/ox_jsdoc/src/parser/context.rs` — compute `description_raw` from `description_lines.first().span.start .. last().span.end`
- `crates/ox_jsdoc/src/ast.rs` — add `description_raw: Option<&'a str>` to `JsdocBlock` / `JsdocTag`
- `crates/ox_jsdoc/src/serializer/json.rs` — add `descriptionRaw` field gated by `SerializeOptions.compat_mode`
- `crates/ox_jsdoc/src/parser/text.rs` (new module) — `parsed_preserving_whitespace` algorithm + unit tests
- `crates/ox_jsdoc/src/lib.rs` — wire the new helper module
- **NAPI binding type defs**: `napi/ox-jsdoc/src-js/index.d.ts` — `JsdocBlock` / `JsdocTag` に `descriptionRaw?: string` (compat-mode-only) を追加
- **WASM binding type defs**: `wasm/ox-jsdoc/src-js/index.d.ts` — 同上 (NAPI 側と shape 揃える)

Phase 3's binary-AST scope:

- `crates/ox_jsdoc_binary/src/writer/nodes/comment_ast.rs` — extend compat tail layouts (JsdocBlock 22 → 30 + 2 padding, JsdocTag 42 → 50 + 6 padding) and emit `description_raw_span` (UTF-8 byte offsets) when `compat_mode = ON`
- `crates/ox_jsdoc_binary/src/decoder/nodes/comment_ast.rs` — `description_raw()` getter returning `Option<&'a str>` + `description_text(preserve_whitespace)` method
- `packages/decoder/src/internal/preserve-whitespace.js` (new) — JS port of the `parsed_preserving_whitespace` algorithm (mirrors the Rust `crates/ox_jsdoc/src/parser/text.rs`)
- `packages/decoder/test/preserve-whitespace.test.js` (new) — JS test suite, fixtures shared with Rust side
- `packages/decoder/src/internal/nodes/jsdoc.js` — `descriptionRaw` getter + `descriptionText(preserveWhitespace?)` method on `RemoteJsdocBlock` / `RemoteJsdocTag`
- **Binary AST type defs (decoder)**: `packages/decoder/src/index.d.ts` — `RemoteJsdocBlock` / `RemoteJsdocTag` に `descriptionRaw: string | null` getter + `descriptionText(preserveWhitespace?: boolean): string | null` method を追加
- **Binary AST type defs (NAPI)**: `napi/ox-jsdoc-binary/src-js/index.d.ts` — re-export shape を更新 (decoder と同期)
- **Binary AST type defs (WASM)**: `wasm/ox-jsdoc-binary/src-js/index.d.ts` — 同上
- `design/007-binary-ast/format.md` — amend `JsdocBlock complete byte-level layout` / `JsdocTag complete byte-level layout` sections to include `description_raw_span` in the compat tail

Phase 5's binary-AST scope (revising the Phase 3 wire layout to support
basic-mode opt-in):

- `crates/ox_jsdoc_binary/src/parser/mod.rs` — extend `ParseOptions` with `preserve_whitespace: bool` (default `false`); flows through every entry point (`parse` / `parse_to_bytes` / `parse_batch_to_bytes`) since they all take `ParseOptions` as the global option struct (`BatchItem` carries only per-item `source_text` + `base_offset`, not parse options)
- `crates/ox_jsdoc_binary/src/parser/context.rs` — emit `description_raw_span` regardless of compat mode when `preserve_whitespace = true`; otherwise leave the bit clear and skip the 8-byte span entirely
- `crates/ox_jsdoc_binary/src/writer/nodes/comment_ast.rs` — set the `has_description_raw_span` bit in Common Data; size the ED record dynamically (basic +0 / +8, compat +0 / +8); the `description_raw_span` slot moves to the **last 8 bytes** of the ED record
- `crates/ox_jsdoc_binary/src/decoder/nodes/comment_ast.rs` — read the bit; compute span offset as "ED end − 8" instead of the fixed Phase 3 compat-tail offset
- `packages/decoder/src/internal/constants.js` — replace the fixed `JSDOC_BLOCK_DESCRIPTION_RAW_SPAN_OFFSET` / `JSDOC_TAG_DESCRIPTION_RAW_SPAN_OFFSET` with the corresponding Common Data bit-mask constants (e.g. `JSDOC_BLOCK_HAS_DESCRIPTION_RAW_SPAN_BIT = 1 << 0`, `JSDOC_TAG_HAS_DESCRIPTION_RAW_SPAN_BIT = 1 << 1`); the dynamic offset is computed in the node class
- `packages/decoder/src/internal/nodes/jsdoc.js` — `descriptionRaw` getter checks the bit, returns `null` if clear; otherwise reads the last 8 bytes of the ED record
- **NAPI binding** `napi/ox-jsdoc-binary/src/lib.rs` + `src-js/index.d.ts` — add `preserveWhitespace?: boolean` to `ParseOptions` / `BatchParseOptions`
- **WASM binding** `wasm/ox-jsdoc-binary/src/lib.rs` + `src-js/index.d.ts` — same
- `design/007-binary-ast/format.md` — amend `JsdocBlock complete byte-level layout` / `JsdocTag complete byte-level layout` to describe the `has_description_raw_span` bit and the dynamic ED tail
- **Test + fixture updates** (drives §7.3 validation matrix):
  - `fixtures/cross-language/description-text.json` — extend each fixture to declare expected `descriptionRaw` / `descriptionText(true)` outputs across the 4 `compat_mode × preserve_whitespace` combinations (or document why a sub-set is sufficient)
  - `crates/ox_jsdoc/tests/cross_language_parity.rs` — extend to iterate the 4 combinations
  - `napi/ox-jsdoc-binary/test/description-text-parity.test.ts` — same on the JS side
  - `napi/ox-jsdoc-binary/test/description-raw.test.ts` — add basic-mode + `preserveWhitespace: true` cases
- **Consumer guide follow-up** `design/008-oxlint-oxfmt-support/consumer-guide.md` — drop the legacy "case A: descriptionText(true) returns null on basic-mode buffer" subtlety; refresh §1 / §4 to mention the new opt-in matrix

Phase narrative recap:

- **Phase 2** (typed AST): JSON serializer keeps `descriptionRaw` gated on `SerializeOptions.compat_mode = true`. The struct field is unconditional (Rust consumers always see it). Unaffected by Phase 5.
- **Phase 3** (binary AST, original): shipped `description_raw_span` always-on inside the compat tail, gated solely on `compat_mode`. **Superseded by Phase 5**.
- **Phase 5** (binary AST, current): replaces the Phase 3 wire with a bit-gated "ED end + 8 bytes" layout. `preserve_whitespace` becomes a separate parse option, fully orthogonal to `compat_mode` so basic-mode and compat-mode buffers can both opt in or out. Non-opt-in consumers (in either mode) carry zero overhead for this feature.

## 7. Validation Strategy

### 7.1 Functional checks

| Check                                  | Tooling                                                                                   |
| -------------------------------------- | ----------------------------------------------------------------------------------------- |
| Aliasing parser shape stable           | Unit tests in `crates/ox_jsdoc/tests/` (1 known + 1 unknown + 1 punctuation tag)          |
| Algorithm parity with upstream         | Port `comment_part_parsed` + (new) `parsed_preserving_whitespace` test fixtures           |
| Markdown emphasis preserved            | `*foo*`, `**bold**`, `*_underscore_*` test cases                                          |
| Indented code block preserved          | Fixture with 4-space indent after `* `, verify 4-space indent in output                   |
| Blank line preservation (`JsdocBlock`) | Multi-paragraph fixture, verify `\n\n` survives                                           |
| Blank line preservation (`JsdocTag`)   | Multi-paragraph tag description fixture (e.g. `@param x` followed by `*\n * Second para`) |
| No regression on `description`         | Existing serializer snapshot tests                                                        |

### 7.2 JSON serializer (typed AST) checks

| Check                                                                              | Tooling                                                          |
| ---------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `descriptionRaw` is **present** in JSON when `SerializeOptions.compat_mode = true` | Snapshot test in `crates/ox_jsdoc/src/serializer/json.rs::tests` |
| `descriptionRaw` is **absent** in JSON when `compat_mode = false` (default)        | Same                                                             |
| Compact `description` field is unchanged in both modes                             | Same (existing snapshots)                                        |

### 7.3 Binary AST decoder checks

| Check                                                                                                               | Tooling                                                                 |
| ------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `descriptionRaw` getter returns **string** on `preserve_whitespace = true` buffer                                   | Vitest in `napi/ox-jsdoc-binary/test/` + `packages/decoder/test/`       |
| `descriptionRaw` getter returns **null** on `preserve_whitespace = false` buffer (regardless of compat mode)        | Same                                                                    |
| `descriptionText(true)` returns **null** on `preserve_whitespace = false` buffer                                    | Same                                                                    |
| `descriptionText(false)` returns the compact string in any mode                                                     | Same                                                                    |
| `descriptionText(true)` output matches Rust `description_text(true)` exact bytes                                    | Cross-language parity test (Rust → bytes → JS decoder → string compare) |
| Opt-in matrix: 4 combinations of `compat_mode` × `preserve_whitespace` produce expected wire size + getter behavior | Cross-language parity test extended to cover every combination          |

### 7.4 Performance checks

| Path                                         | Tooling                                                                         |
| -------------------------------------------- | ------------------------------------------------------------------------------- |
| Rust parser bench                            | `cargo bench --bench parser`                                                    |
| Rust JSON serialize timing                   | criterion micro-bench (added)                                                   |
| JS decoder bench (basic mode)                | `pnpm benchmark:napi-binary-vs-typed`                                           |
| JS decoder bench (compat mode, parseBatch)   | same script with `compatMode: true`                                             |
| JS decoder bench (preserve mode, parseBatch) | same script with `preserveWhitespace: true` (Phase 5)                           |
| Opt-in buffer size growth                    | Measure `description_raw_span` cost on typescript-checker.ts (~3.6 KB per §5.2) |

### 7.5 Out-of-scope sanity checks

| Check                                             | Reason out-of-scope                                             |
| ------------------------------------------------- | --------------------------------------------------------------- |
| `JsdocTag.body.*.description` preserve-whitespace | Per §8 — lower-layer description is not exposed via this design |
| `@example` code-fence-aware output                | Per §8 — defer to format-side use cases                         |

## 8. Out of Scope / Future Work

- **Tag-name normalization built into ox-jsdoc** — explicitly delegated to
  consumers; ox-jsdoc only documents the pattern.
- **Rich Markdown / mdast representation in the AST** — oxfmt parses
  description text into mdast itself; ox-jsdoc only provides the string.
- **Full eslint-plugin-jsdoc rule parity** — the lint engine that consumes
  ox-jsdoc is responsible for rule logic; this design only covers the parser
  surface.
- **`@example` code-fence-aware highlighting** — would require a second
  parse pass; defer until the format-side use cases stabilize.
- **`JsdocTag.body.*.description` (lower-layer description)** — `JsdocTag` has
  two distinct "description" surfaces: the upper `JsdocTag.description`
  (covered by this design) and the lower `JsdocTag.body.{Generic,Borrows,Raw}.description`
  on the `JsdocTagBody` enum variants. The lower one is **out of scope**:
  `Borrows` / `Raw` variants are reserved (not emitted by the current parser),
  and the `Generic` variant's `description` is a derived field that already
  mirrors the upper `description`. Preserve-whitespace is wired only on the
  upper `JsdocBlock.description` and `JsdocTag.description`.

## 9. References

| Document                       | Path                                                                                                   |
| ------------------------------ | ------------------------------------------------------------------------------------------------------ |
| Upstream JSDoc parser          | `refers/oxc/crates/oxc_jsdoc/src/parser/jsdoc_parts.rs`                                                |
| Upstream tag-aliasing settings | `refers/oxc/crates/oxc_linter/src/config/settings/jsdoc.rs`                                            |
| Upstream JSDoc formatter       | `refers/oxc/crates/oxc_formatter/src/formatter/jsdoc/serialize.rs`                                     |
| eslint-plugin-jsdoc settings   | https://github.com/gajus/eslint-plugin-jsdoc/blob/v50.5.0/docs/settings.md                             |
| ox-jsdoc parser entry point    | `crates/ox_jsdoc/src/parser/parse.rs`                                                                  |
| ox-jsdoc description splitter  | `crates/ox_jsdoc/src/parser/context.rs::parse_description_lines`                                       |
| Binary AST compat-tail layout  | [`design/007-binary-ast/format.md`](../007-binary-ast/format.md#jsdocblock-complete-byte-level-layout) |
| Consumer integration guide     | [`./consumer-guide.md`](./consumer-guide.md) (drafted in Phase 1, finalized in Phase 4)                |
