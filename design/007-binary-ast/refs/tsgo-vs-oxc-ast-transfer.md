# tsgo vs oxc — Comparison of AST Sharing Strategies

Investigation date: 2026-04-13

---

## Differences in Design Philosophy

|                   | oxc (Raw Transfer)                  | tsgo (Binary AST)                                       |
| ----------------- | ----------------------------------- | ------------------------------------------------------- |
| **Core question** | "How do we eliminate the boundary?" | "How do we make data crossing the boundary compact?"    |
| **Process model** | **Same process** (NAPI binding)     | **Separate processes** (Go server + JS client over IPC) |

---

## Memory Architecture

### oxc: Direct Writes to a Shared Buffer

A Rust bump allocator is constructed directly on a **6 GiB ArrayBuffer** (of which 2 GiB is used) owned by the JS side. The parser allocates AST nodes, strings, and vectors all within this buffer. After parsing completes, the AST already lives in JS-side memory. **No data transfer**.

```
JS ArrayBuffer (6 GiB, 4 GiB-aligned)
┌──────────────────────────────────────────────┐
│ source text │ ...arena-allocated AST... │ meta │
└──────────────────────────────────────────────┘
  Rust writes directly → JS reads directly
```

- A bump allocator is constructed on the JS buffer via `Allocator::from_raw_parts` (`napi/parser/src/raw_transfer.rs`)
- Thanks to 4 GiB alignment, the upper 32 bits of 64-bit pointers are common, so the lower 32 bits alone can express offsets, fitting in V8's SMI (Small Integer) for high speed
- The source text is also placed at the start of the buffer

### tsgo: Encode + IPC Transfer

Build the AST on the Go heap → encode it into a custom binary format (28 bytes/node) → transfer via IPC (stdio) → the JS side receives it as a `Uint8Array`.

```
Go heap          IPC pipe        JS Uint8Array
┌─────────┐     ┌────────┐     ┌──────────────┐
│ AST tree │ --> │ encode │ --> │ binary data  │
└─────────┘     └────────┘     └──────────────┘
  serialize        copy          lazy decode
```

- The AST is arena-allocated on the Go heap (48 node types are targeted)
- The encoder traverses with `ForEachChild` in source order and converts to a flat array of 28 bytes/node
- IPC transfer uses msgpack (raw binary) or JSON (base64-encoded)

---

## Details of Boundary Crossing

| Aspect                 | oxc                                                                           | tsgo                                            |
| ---------------------- | ----------------------------------------------------------------------------- | ----------------------------------------------- |
| **Data copies**        | None (Rust writes directly to the JS buffer)                                  | Two (Go → binary encode, binary → JS receive)   |
| **Serialization**      | None (raw `repr(C)` struct layout)                                            | Yes (28 bytes/node flat format)                 |
| **JS object creation** | Eager: materialize all nodes as JS objects / Lazy: read on demand via getters | Always Lazy (`RemoteNode` reads via `DataView`) |

---

## String Handling

### oxc

- The source text is placed at the start of the buffer
- `Str = (ptr, len)` references locations within the buffer
- The JS side reads via several fast paths:
  1. ASCII-only → `sourceTextLatin.substr()` (fastest)
  2. Longer than 64 bytes → `Buffer.prototype.utf8Slice` (Node.js optimized)
  3. Short non-ASCII → `String.fromCharCode.apply`
  4. Fallback: `utf8Slice`

### tsgo

- The entire source text is stored as UTF-8 in the String Data section
- String Offsets (8 bytes/string) manage start/end pairs for each string
- Strings that can be referenced as slices within the source text are zero-copy (Go side)
- Escape-resolved strings and others are appended after the source text
- The JS side reads via `TextDecoder`
- The decoder converts all String Data into a single Go string, and substrings are zero-allocation

---

## Tree Navigation

### oxc

- Uses Rust's `repr(C)` struct layout as-is
- Field access via discriminant byte + fixed offsets
- Struct sizes for each node type **differ** (fixed per type, but not unified across node types)
- Reads the buffer through `Int32Array`/`Float64Array` typed array views

### tsgo

- A flat array with **fixed 28 bytes** (all nodes the same size)
- The tree structure is expressed via `parent` index + `next` sibling index + child bitmask
- The first child of `node[i]` is `node[i+1]` (when their parents match)
- Node Data field (32 bits): 2-bit type tag + 6-bit type-specific data + 24-bit payload

---

## API as Seen from JS

### oxc — Two Modes

**Eager mode** (`raw-transfer/eager.js`):

- A generated deserializer walks the entire buffer
- Materializes all nodes as plain JS objects
- After completion, returns the buffer to the cache

**Lazy mode** (`raw-transfer/lazy.js`):

- ES6 classes (`Program`, `Identifier`, etc.) wrap buffer positions
- Properties are getters that read from the buffer on access
- Stores `(pos, ast)` in a `#internal` private field
- Nodes are cached in a `Map` (keyed by buffer position)
- A `FinalizationRegistry` returns the buffer to the cache on GC

### tsgo — Always Lazy

- `RemoteNode`/`RemoteNodeList` classes wrap a `DataView`
- Property accesses are translated to `view.getUint32(byteIndex + offset, true)`
- Child nodes are looked up via parent/next indices and the child bitmask
- Nodes are cached in the SourceFile's `nodes` array
- A `childProperties` lookup table maps the bitmask to named properties

---

## Buffer Management

### oxc

Because it consumes 6 GiB of virtual memory, it is managed with a two-tier cache:

- **Tier 1**: Strong references, cleared after 10 seconds of inactivity
- **Tier 2**: `WeakRef`, GC-eligible but reusable while alive
- Async parsing is limited to `os.availableParallelism()` cores

### tsgo

- Buffers are disposable (new per request)
- `SourceFileCache` skips re-fetching files whose contentHash matches
- Cache entries are managed by ref-counting (per snapshot, per project)

---

## Span / Position Handling

|                         | oxc                                                               | tsgo                                                                       |
| ----------------------- | ----------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Internal representation | UTF-8                                                             | UTF-8                                                                      |
| Conversion for JS       | Convert UTF-8 → UTF-16 on the Rust side, then write to the buffer | Convert UTF-8 → UTF-16 on the Go side, then encode                         |
| Optimization            | Bulk conversion via `span_converter`                              | O(log n) binary search via `PositionMap`; ASCII-only files skip conversion |

---

## Platform Constraints

| Constraint      | oxc                                                               | tsgo                                                     |
| --------------- | ----------------------------------------------------------------- | -------------------------------------------------------- |
| Architecture    | **64-bit little-endian only**                                     | Cross-platform                                           |
| Node.js version | >= 22                                                             | No constraint                                            |
| Memory          | 6 GiB virtual memory per parse                                    | Proportional to AST size (small)                         |
| Concurrency     | Async parsing runs on threads; deserialization on the main thread | The Go server handles concurrency, the JS client is thin |

---

## Performance Characteristics

### Strengths of oxc

- **Zero data copies** from parsing to JS consumption
- No serialization/deserialization overhead
- No communication latency since it runs in the same process
- As shown by the `hybrid-type-aware-linting-performance` investigation, the FFI approach delivers top performance

### Strengths of tsgo

- **Process isolation** makes editor integration and multi-client support easy
- **Random access is O(1)** thanks to the fixed 28 bytes/node
- contentHash-based caching skips re-transfer of unchanged files
- No massive virtual memory consumption

---

## Implications for ox-jsdoc

### Current oxc_jsdoc Implementation

The current `oxc_jsdoc` crate does not depend on `oxc_allocator` and uses no arena allocation at all:

- The JSDoc "AST" consists of `&'a str` slice + `Span` wrappers (lightweight borrow-based design)
- Tags use `Vec<JSDocTag<'a>>` (the standard heap `Vec`), managed with `FxHashMap`
- Lazy parsing via `OnceCell<ParsedJSDoc<'a>>`
- `SemanticBuilder::build()` takes `&'a Program<'a>` but does not take `&'a Allocator`

Therefore, **it is not the case that "the ox-jsdoc AST is automatically placed in the same bump allocator as oxc"**.

### oxlint Integration (Primary Use Case)

Placing the ox-jsdoc AST inside oxc's bump allocator is **technically possible**, but requires the following API changes:

1. Add `&'a Allocator` to the signature of `SemanticBuilder::build()`
2. Design ox-jsdoc's parser to allocate nodes via `oxc_allocator::Box`/`Vec`
3. Modify the oxc-side API (an upstream PR is needed)

Since the bump allocator can allocate via `&Allocator` (no `&mut` needed; interior mutability), it can be held simultaneously with the parser AST (`&'a Program<'a>`), and the lifetime `'a` naturally matches. Additional allocations during the semantic analysis phase are also fine (bumpalo is append-only).

On the other hand, the current oxc_jsdoc's **allocator-free borrow-based design** is also fast enough, so whether arena allocation is truly necessary should be decided by measurement. If `&'a str` references into the source text are sufficient, it is simpler to avoid the arena's overhead.

### NAPI Integration (ESLint Compatibility, etc.)

To leverage oxc's Raw Transfer approach, the ox-jsdoc AST must be placed inside oxc's `ArrayBuffer` (i.e., bump allocator integration is a prerequisite). If not integrated, the JSDoc AST portion must be serialized separately.

### Future IPC

If a need arises to transfer the ox-jsdoc AST to a separate process, tsgo's 28 bytes/node flat format will be a useful reference. However, the need is low at this time.

### Lazy Deserialization

When used from ESLint plugins, rather than eagerly materializing the entire JSDoc AST as JS objects, the option of materializing only the necessary nodes via the tsgo-style Lazy `RemoteNode` pattern is also worth considering.

---

## Essential Differences Between Raw Transfer and Binary AST

oxc's Raw Transfer is not a "Binary AST". The two are fundamentally different approaches.

### Raw Transfer = Direct Sharing of Memory Layout

Raw Transfer exposes the memory representation of Rust's `repr(C)` structs directly to the JS side:

- **No format specification exists** — the struct layout decided by the Rust compiler is itself the "specification"
- **The JS side directly depends on Rust's memory representation** — it breaks if field order, padding, or pointer size changes
- `assert_layouts.rs` (159 KB) pins the size, alignment, and offsets of all structs in CI to ensure stability
- **No portability** — limited to 64-bit LE + same-process

### Binary AST = An Independent Format Designed for Transfer

tsgo's Binary AST is a protocol explicitly designed for transfer:

- **The specification is independent of the code** — Protocol Version, section structure, and node size are documented
- **Independent of the sender's memory layout** — the encoder handles conversion
- **Decodable anywhere** — same process, separate process, separate machine, WASM, browser

### Comparison

| Aspect                          | oxc Raw Transfer                              | tsgo Binary AST                                    |
| ------------------------------- | --------------------------------------------- | -------------------------------------------------- |
| **Essence**                     | Direct memory sharing                         | A serialization protocol                           |
| **Design intent**               | "Don't serialize"                             | "Serialize efficiently"                            |
| **Format specification**        | Decided by the Rust compiler (implicit)       | Explicit protocol specification                    |
| **Impact of Rust-side changes** | The JS-side deserializer can break            | Only the encoder updates; the JS side stays stable |
| **Portability**                 | 64-bit LE + NAPI only                         | Full support for NAPI / WASM / IPC / files         |
| **Per-environment JS code**     | Environment-dependent (Raw Transfer specific) | **The same decoder for all environments**          |

### Implications for ox-jsdoc

If ox-jsdoc designs a Binary AST, it is not a replacement for Raw Transfer but **a different approach**:

- Define an independent binary format that does not depend on Rust's memory layout
- Cover all environments (NAPI, WASM, IPC) with a single format specification + a single JS-side decoder
- Even if the Rust struct changes, only the encoder needs updating; the JS side stays stable
- Since `Uint8Array`/`DataView` are ECMAScript standards, the same reading code works on Node.js / browsers / Deno / Bun

---

## Reference Files

### oxc

- `napi/parser/src/raw_transfer.rs` — Raw Transfer implementation
- `napi/parser/src-js/raw-transfer/common.js` — Buffer management and caching
- `napi/parser/src-js/raw-transfer/eager.js` — Eager deserialization
- `napi/parser/src-js/raw-transfer/lazy.js` — Lazy deserialization
- `napi/parser/src-js/deserialize/js.js` — String decoding and fast paths

### tsgo

- `internal/api/encoder/encoder.go` — Binary AST encoder
- `internal/api/encoder/decoder.go` — Binary AST decoder
- `internal/api/encoder/stringtable.go` — String table
- `internal/api/session.go` — IPC integration
- `_packages/api/src/sync/api.ts` — JS sync client
- `_packages/api/src/protocol.ts` — Protocol constants
- `_packages/api/src/node.generated.ts` — RemoteNode generated code
- `_packages/api/src/sourceFileCache.ts` — Source file cache
