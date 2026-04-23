# Batch Processing (5 points and decisions)

This document summarizes the five points of discussion and their decisions for the
**batch processing** of Binary AST (a design that stores N comments in a single buffer).

## Design overview

ox-jsdoc primarily targets use cases such as oxlint that **process N comments at once**,
so we design Binary AST as a batch-capable format from the start.
N=1 (a single comment) is treated as a special case of the same format; we do not provide
a separate API.

Key decisions:

- **Single buffer with N roots (add a root index array to the Header)**: Fit everything
  into one NAPI call and reap all batch benefits, including string dedup, memory locality,
  and Header overhead reduction (Options alpha and gamma are not adopted)
- **Store Diagnostics inside the Binary AST as well** (Option 1A): Avoid V8 object
  construction at the NAPI boundary by adding a Diagnostics section to the Header.
  Minimize each entry to **8 bytes fixed** as `{root_index, message_index}` (the
  eslint-plugin-jsdoc investigation confirmed that severity / range are unnecessary)
- **Public API splits `parse()` and `parseBatch()`** (Option 3A): To avoid TypeScript
  return-type unions. Maintain backward compatibility with the existing `parse()` API
- **Parse failure is represented by `roots[i] = 0` sentinel** (Option 4A): Convention
  that on failure, at least one diagnostic is always attached. The failure location is
  reconstructed from the input
- **Each BatchItem owns an independent sourceText** (Option 5C): In typical oxlint code,
  each comment is an independent slice, so we do not force the caller to perform any
  pre-concatenation
- **Pos/End are relative to sourceText**, plus **`base_offset` stored as root metadata**:
  The JS side computes the absolute position as `baseOffset + pos` (for ESLint reporting)

The remainder of the document explains the five points in detail.

## Use case

The batch scenario for ox-jsdoc is typically driven via oxlint:

```text
1. oxc_parser parses a JS/TS file
2. Extract all comments (`/** ... */`) -> array of N comment texts
3. ox-jsdoc receives N items and returns N JsdocBlock ASTs
4. The ESLint plugin walks each AST in turn
```

Benefits of batching:

1. **Reduced NAPI call overhead** (140ns x N -> 140ns x 1)
2. **Reduced buffer allocation cost** (eliminating N duplicated Headers)
3. **String dedup**: Tag names such as `param`, `returns`, and `throws` recur across
   all comments, so sharing them through the string table saves several KB on large files
4. **Cache locality**: Sequential walking over contiguous memory

## Adoption policy: a single buffer with N roots

Add a root index array to the Header and lay multiple trees side-by-side within the
Nodes section:

```text
Nodes section:
  node[0]: Sentinel (all fields zero)
  node[1]: root[0] (root JsdocBlock for the 1st comment)
  node[2..k1]: descendants of root[0]
  node[k1+1]: root[1] (root for the 2nd comment)
  ...
  parent of each root = 0 (= sentinel)

Reasons:
- Captures all of batching benefits 1-4
- Compatible with N=1 (single comment): the same format works with a root count of 1
- Extending the Header by 8 bytes is acceptable
- The decoder is straightforward: read the root index array and construct N
  `RemoteJsdocBlock`s
```

### Other options considered (not adopted)

**Option alpha: Return N items as separate buffers (Vec<Vec<u8>>)**

- Pros: Zero design changes
- Cons: Loses benefits 2-4 (N duplicated Headers, duplicated strings, scattered memory)

**Option gamma: Wrap N roots as children of a top-level virtual NodeList**

- Pros: No Header change required
- Cons: Single comments must go through the virtual NodeList -> unnatural in ESTree
  terms, plus 24 bytes overhead
- Note: NodeList wrappers are no longer emitted anywhere in the live design
  (variable-length child lists use inline `(head_index, count)` metadata in
  the parent's Extended Data block); this option is preserved here as
  historical context only

---

## Point 1: Where to store the Diagnostic array

### Decisions

**Adopt Option 1A** (Diagnostics section in the Header, linked by root_index)

| Item                                    | Decision                                                     | Rationale                                                                                    |
| --------------------------------------- | ------------------------------------------------------------ | -------------------------------------------------------------------------------------------- |
| Diagnostic structure size               | **8 bytes fixed** (`root_index: u32` + `message_index: u32`) | The eslint-plugin-jsdoc investigation confirmed that severity / source_range are unnecessary |
| Global diagnostics (not tied to a root) | **Not needed**                                               | ox-jsdoc Diagnostics are always attached to a specific root                                  |
| Diagnostic ordering                     | **Ascending by root_index**                                  | Speeds up retrieval of diagnostics for a specific root via binary search                     |

### Summary of the eslint-plugin-jsdoc investigation

Investigating all sources under `refers/eslint-plugin-jsdoc/` showed that:

- The only Diagnostic field actually used is **`message`** (one location at
  `validTypes.js:253-258`)
- `code`, `line`, and `critical` (originating from comment-parser) are **never used**
- The block-level `problems` array is never read
- **No rule** reports "JSDoc parsing failed"
- `severity` is determined by the **ESLint rule configuration** (not needed on the
  parser side)
- Error positions are reconstructed from the **AST node spans**
  (`iterateJsdoc.js:1916-1918`)

-> There is no value in storing severity / source_range in the Binary AST.
`{ root_index, message_index }` is sufficient.

### Final format

```text
Diagnostics section (4 + 8M bytes):
  | 0-3   | u32 | Diagnostic count M                    |
  -- M diagnostics stored in ascending root_index order --
  | 4-7   | u32 | diagnostic[0].root_index              |
  | 8-11  | u32 | diagnostic[0].message_index           |
  | 12-15 | u32 | diagnostic[1].root_index              |
  | 16-19 | u32 | diagnostic[1].message_index           |
  ...
```

### Other options considered (not adopted)

**Option 1B: Fully separate from the AST (Diagnostics as a separate NAPI return value)**

Return `{ binary: Vec<u8>, diagnostics: Diagnostic[] }` as a 2-tuple via NAPI.

- Pros: Simpler Binary AST format
- Cons: Causes V8 object construction at the NAPI boundary -> reduces batching benefit
- Reason for non-adoption: Undermines the batching benefit (fitting everything into one
  NAPI call)

**Option 1C: Nodelize Diagnostics inside the binary as well**

Add a `JsdocDiagnostic` Kind and place it as a virtual child of each root.

- Pros: Uniform handling
- Cons: Visitors leak into the AST, affecting ESLint compatibility
- Reason for non-adoption: ESLint visitors would see them and trigger unintended node
  traversal

---

## Point 2: Encoder API (Rust side)

### Design direction

**Current design (approach c-1)**: The parser builds the Binary AST directly into the
arena (the typed AST is removed). The JS side shares the binary on the arena via
zero-copy (NAPI Buffer / WASM memory.buffer). The Rust walker also reads the binary
through the lazy decoder. The encoder is a secondary use case for IPC/network and
ships in Phase 2 onward.

### Decisions

| Item                             | Decision                                                                                                                                                     |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **How to build the Binary AST**  | **Approach c-1**: Modify the parser to write Binary AST directly into the arena without going through a typed AST. The typed AST struct hierarchy is removed |
| **Rust public API**              | `parse()` returns `binary_bytes: &[u8]` (Binary AST) and `lazy_root: LazyJsdocBlock<'_>` (for the Rust walker). No typed AST is provided                     |
| **NAPI sharing**                 | (c) zero-copy: pass the binary on the arena to JS via NAPI Buffer (no memcpy)                                                                                |
| **WASM sharing**                 | JS views the arena region directly with `new Uint8Array(wasm.memory.buffer, offset, length)`                                                                 |
| **Rust walker**                  | Walks the Binary AST **via the lazy decoder** (Copy-able value structs such as `LazyJsdocBlock`). There is no typed AST                                      |
| **JS walker (linters, etc.)**    | Reads the Uint8Array (binary on the arena) lazily and only materializes nodes as JS objects on demand (allocated on the JS heap, not the arena)              |
| **`parse_to_binary` public API** | Implemented in **Phase 2 onward**. Targeted at IPC/network use (no API encodes from a typed AST; the input is `&str`)                                        |
| **`decode_binary` public API**   | For access from other languages (Go/Python/Rust). ox-jsdoc itself does not use it                                                                            |

### API shape if exposed in Phase 2 onward (Option 2B adopted)

Because the typed AST is removed (approach c-1), the API takes `&str` (sourceText)
directly, runs the parser internally, and produces the Binary AST byte stream
(see [rust-impl.md "Public Rust API (Phase 2 onward)"](./rust-impl.md#public-rust-api-phase-2-onward)):

```rust
pub struct BatchItem<'a> {
    pub source_text: &'a str,    // sourceText for each comment (required)
    pub base_offset: u32,         // absolute offset within the original file (default 0)
}

/// Produce a Binary AST in batch (internally calls parser_into_binary)
pub fn parse_to_binary<'a>(
    items: &[BatchItem<'a>],
    options: SerializeOptions,
) -> Vec<u8>

/// Re-serialize an existing Binary AST byte stream (for persistent caches; usually a no-op)
pub fn reserialize_binary(bytes: &[u8]) -> Vec<u8>
```

Note: The "encode from typed AST to Binary AST" use case **does not exist**
(after removing the typed AST, the source of truth is always the Binary AST byte stream).

Reason for adoption (rationale for rejecting Option 2A/2C):

- **Room for extension**: We can add fields to `BatchItem` later (e.g., `language_mode`)
- **Readability**: The struct name `BatchItem` makes intent obvious
- **API unification**: A single comment can be passed as `&[item][..]`, so one
  `parse_to_binary` function handles both cases
- **Smooth napi integration**: It is easy to map directly to a JS-side `BatchItem` type
  later via `#[napi(object)]`

### Other options considered (not adopted)

**Option 2A: Take a tuple array**

```rust
pub fn parse_to_binary<'a>(
    items: &[(&'a str, u32)],  // (source_text, base_offset)
    options: SerializeOptions,
) -> Vec<u8>
```

Reason for non-adoption: Tuples make intent unclear, and adding fields would be a
breaking change.

**Option 2C: Separate functions for single and batch**

```rust
pub fn parse_to_binary_single<'a>(...) -> Vec<u8>
pub fn parse_to_binary_batch<'a>(...) -> Vec<u8>
```

Reason for non-adoption: There is no need to split the API into two; a single comment
can be expressed as `&[item; 1]`.

---

## Point 3: JS API naming

### Decisions

**Adopt Option 3A** (split `parse` and `parseBatch`), maintain backward compatibility
with the existing `parse()` API, and provide `BatchItem.baseOffset` from the start.

### Final API

```typescript
// Existing API (compatibility maintained)
parse(text: string, options?: ParseOptions): ParseResult

interface ParseResult {
  ast: RemoteJsdocBlock | null  // internal implementation switches to the lazy decoder
  diagnostics: Diagnostic[]      // message only (root_index unnecessary in single mode)
}

// New batch API
parseBatch(items: BatchItem[], options?: ParseOptions): BatchResult

interface BatchItem {
  sourceText: string
  baseOffset?: number  // offset within the original file (for ESLint, default 0)
                       // added to each node's Pos/End
}

interface BatchResult {
  asts: (RemoteJsdocBlock | null)[]  // one per item; null = parse failed (root_index = 0)
  diagnostics: BatchDiagnostic[]     // across all items
}

interface BatchDiagnostic extends Diagnostic {
  rootIndex: number  // which item it is attached to
}
```

### Internal implementation unification

`parse()` internally calls `parseBatchInternal([{sourceText: text}])` and converts
the result into the single-item form:

```typescript
function parse(text: string, options?: ParseOptions): ParseResult {
  const result = parseBatchInternal([{ sourceText: text }], options)
  return {
    ast: result.asts[0],
    diagnostics: result.diagnostics.map(d => ({ message: d.message })) // strip rootIndex
  }
}
```

### Usage example for baseOffset (oxlint scenario)

```typescript
const comments = oxc.parse(jsCode).comments
const result = parseBatch(comments.map(c => ({ sourceText: c.text, baseOffset: c.start })))
// Node ranges in the AST are absolute offsets within jsCode
result.asts[0]?.tags[0].range // -> [142, 158] etc.
```

### Other options considered (not adopted)

**Option 3B: Overload**

```typescript
function parse(input: string | BatchItem[], options?): ParseResult | BatchResult
```

Reason for non-adoption: TypeScript would union the return type, forcing users to write
`Array.isArray(result.asts)` checks every time, and IDE completions would be unclear.

**Option 3C: Unified BatchResult**

```typescript
parse(input: string | BatchItem | BatchItem[], options): BatchResult
```

Reason for non-adoption: Breaks backward compatibility with the existing API
(PR #5 has already shipped), and always writing `result.asts[0]` is verbose.

---

## Point 4: Handling empty comments (parse failures)

### Decisions

**Adopt Option 4A** (root index = 0 sentinel) plus related conventions:

| Item                           | Decision                                                                                                                                                                                           |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Root representation on failure | `roots[i] = 0` (points to the sentinel node index)                                                                                                                                                 |
| Diagnostic required            | **On failure, at least one diagnostic is always attached** (the parser guarantees this; if it does not currently, we adopt the convention of always attaching a minimal "parse failed" diagnostic) |
| Failure location               | Not included in the Binary AST. Reconstructed on the JS side from `BatchItem.baseOffset + sourceText.length`                                                                                       |

### Example behavior

```typescript
Input: 5 comments (i=1 and i=3 fail)
roots = [1, 0, 47, 0, 92]
         |  |  |   |  |
        OK NG  OK  NG  OK

// Behavior visible to JS users
const result = parseBatch(items)
result.asts[0]   // RemoteJsdocBlock (AST for i=0)
result.asts[1]   // null (parse failed)

// Retrieve the failure reason (at least one diagnostic is guaranteed)
const errors = result.diagnostics.filter(d => d.rootIndex === 1)
errors[0].message  // "expected '}' for inline tag" etc.

// Retrieve the position of the failed comment (reconstructed from the input)
const item = items[1]
const start = item.baseOffset
const end = start + item.sourceText.length
```

### Other options considered (not adopted)

**Option 4B: Skip failure entries (output count < input count)**

Reason for non-adoption: `asts.length < items.length` breaks the input correspondence,
causing position misreporting in ESLint.

**Option 4C: Emit a placeholder empty JsdocBlock**

Reason for non-adoption: Wastes 24 bytes, forces rule authors to consider "empty nodes",
and produces an unnatural empty JsdocBlock in the ESTree.

---

## Point 5: Handling per-item source text

### Decisions

**Adopt Option 5C** (each BatchItem owns an independent sourceText)

Note: Option 5B was the initial recommendation, but during discussion we discovered that
**typical oxlint code provides each sourceText as an independent slice** (the entire
jsCode is not passed to ox-jsdoc), so we switched to Option 5C.

### Decisions

| Item                                 | Decision                                                                             |
| ------------------------------------ | ------------------------------------------------------------------------------------ |
| **String Data**                      | Concatenate each sourceText in order (the zero-copy slice optimization is preserved) |
| **Root index array**                 | **12 bytes/root** (`node_index + source_offset_in_data + base_offset`)               |
| **Pos/End on nodes**                 | **Relative byte offsets within sourceText** (parser output as-is, no rewriting)      |
| **Absolute range computation on JS** | `[baseOffset + pos, baseOffset + end]` (added on the decoder side)                   |

### Binary AST format details

```text
Root index array (12N bytes):
  | 0-3   | u32 | root[0].node_index            |  <- root node index within the Nodes section
  | 4-7   | u32 | root[0].source_offset_in_data |  <- starting position of sourceText within String Data
  | 8-11  | u32 | root[0].base_offset           |  <- BatchItem.baseOffset (for absolute offset computation)
  | 12-15 | u32 | root[1].node_index            |
  | 16-19 | u32 | root[1].source_offset_in_data |
  | 20-23 | u32 | root[1].base_offset           |
  ...

String Data:
  [sourceText[0]] [sourceText[1]] ... [sourceText[N-1]] [unique strings...]
       ^               ^                    ^
   referenced by    referenced by         referenced by
   root[0] via      root[1] via           root[N-1] via
   source_offset_in_data
```

### Usage example

```typescript
// oxlint scenario
const program = oxc.parse(jsCode)
const items = program.comments.map(c => ({
  sourceText: jsCode.slice(c.start, c.end),
  baseOffset: c.start
}))

const result = parseBatch(items)

// Node ranges in the AST are absolute offsets within jsCode
result.asts[0]?.tags[0].range // -> [142, 158]

// Internal implementation (lazy decoder):
class RemoteJsdocTag {
  get range() {
    const pos = this.view.getUint32(this.byteIndex + 4, true) // relative offset
    const end = this.view.getUint32(this.byteIndex + 8, true)
    const baseOffset = this.sourceFile.getRootBaseOffset(this.rootIndex)
    return [baseOffset + pos, baseOffset + end] // absolute offsets
  }
}
```

### Other options considered (not adopted)

**Option 5A: Concatenate all sources into a single String Data (general purpose)**

Each root holds (source_start, source_end) in its Extended Data.

Reason for non-adoption: Functionally equivalent to 5C, but the per-root metadata is
scattered across two locations (the root index array and the Extended Data). 5C is
more consistent.

**Option 5B: The encoder takes a single `source` (oxlint-specialized)**

Reason for non-adoption: In typical oxlint code, each comment is passed as an
independent slice. Forcing the caller to concatenate everything into one `source`
hurts usability.

---

## Summary of decisions

| Point               | Decision                                                                                                                                              |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1: Diagnostic array | Option 1A (Diagnostics section in the Header, ordered by root_index, 8 bytes fixed)                                                                   |
| 2: Rust encoder API | **Approach c-1** (parser writes Binary AST directly into the arena, typed AST removed); `parse_to_binary` exposed in Phase 2 onward (Option 2B shape) |
| 3: JS API naming    | Option 3A (split `parse` and `parseBatch`, maintain API compatibility, provide `baseOffset`)                                                          |
| 4: Empty comments   | Option 4A (root index = 0 sentinel) plus a required diagnostic on failure; failure location is reconstructed from the input                           |
| 5: Source text      | Option 5C (each BatchItem owns its sourceText, root array 12B/root, Pos/End relative)                                                                 |
