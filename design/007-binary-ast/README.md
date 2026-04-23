# ox-jsdoc Binary AST Design

This directory contains the formal design documents for the ox-jsdoc
**Binary AST format**. By combining insights from the tsgo Binary AST and the
oxc raw transfer lazy decoder, it defines a single format that works across
NAPI / WASM / IPC environments.

## Overview

The Rust side of the JSDoc parser runs at nanosecond speeds, but transferring
the AST to JS through JSON serialization and `JSON.parse` introduces enough
overhead to slow it down to comment-parser levels. The Binary AST eliminates
this boundary cost and achieves the following:

1. Removes the `serde_json` dependency on the Rust side
2. Removes `JSON.parse` on the JS side
3. Lazy node materialization (only accessed nodes become JS objects)
4. Runs on a single JS decoder across all environments (NAPI / WASM / IPC)
5. Preserves the ESTree-compatible shape for lint rule authors
6. Transfers the complete AST including parsedType (TypeNode, 45 kinds) in a single format

## Design core

- **The parser builds the Binary AST directly (Approach c-1)**: removes the
  typed AST struct hierarchy and has the parser write the Binary AST directly
  into the arena
- **Same lazy decoder pattern in both languages**: both Rust and JS operate
  under the same model of "lazily expanding the Binary AST"
- **Zero-copy sharing**: the arena memory is viewed directly from JS through
  the NAPI Buffer / WASM `memory.buffer`
- **Batch support**: stores N comments in a single buffer (String dedup,
  reduced NAPI calls)
- **compat_mode is handled within the single format**: switched via Header bit0 flag

## Document structure

Each chapter is split into an independent file. We recommend reading them in
order during implementation:

### Design premises

1. [Architecture (Background & Architecture)](./architecture.md)
   - Problem and goals
   - Inspiration (tsgo + oxc)
   - Separation of concerns
   - Sharing strategy per environment (NAPI / WASM / IPC)
   - Layers of the Rust public API
   - Key design decisions

2. [AST Nodes (Catalog of target nodes)](./ast-nodes.md)
   - 15 comment AST kinds
   - 45 TypeNode AST kinds
   - Kind number space (single-instruction optimization)
   - Phase 4 dispatch table code generation

### Binary format details

3. [Binary Format](./format.md)
   - Conventions (LE / UTF-8 / UTF-16 Pos/End / bit ordering)
   - Section layout (7 sections)
   - Header (40 bytes) and Protocol Version
   - Root Index Array (12N bytes)
   - String table (String Offsets + String Data)
   - Extended Data section (variable length, 5 field types)
   - TypeNode Extended Data layout (6 mixed-type details)
   - Diagnostics section (4 + 8M bytes)
   - Nodes section (24 bytes/node, flat array)
   - Node Data bit packing (32 bit, 4 type tags)
   - Common Data (6-bit, small per-kind data)
   - Node catalog matrix (60 nodes + 1 Sentinel + 1 reserved-only `NodeList` discriminant = 62 entries; `NodeList` is reserved-only and never emitted by the encoder)

   Each section follows the unified structure of "Design overview / Layout /
   Field details / Implementation sketch / Size and performance / Differences
   from tsgo", accompanied by SVG diagrams.

4. [Encoding (Tree, Variant, compat_mode)](./encoding.md)
   - Handling nodes with variants
   - Relationship with compat_mode
   - Tree encoding (variable-length child lists via inline `(head_index, count)` metadata in Extended Data — no NodeList wrapper)

### Lazy decoder implementation

5. [JS Decoder (JS lazy decoder)](./js-decoder.md)
   - Design overview (lazy + zero-copy + cross-environment)
   - Lazy node classes (`#internal` pattern + RemoteJsdocTag as the main example)
   - Helper functions (`extOffsetOf`, `childAtVisitorIndex`)
   - `RemoteSourceFile` (the decoder's root class)
   - Kind dispatch (`KIND_TABLE` Phase 4 code generation)
   - Eagerization (`toJSON` / cross-environment support)
   - Array fields (`RemoteNodeList`)
   - Visitor Keys
   - JS Public API (`parse` / `parseBatch`)

6. [Rust Implementation](./rust-impl.md)
   - Design overview (Approach c-1 / typed AST removed / stack value type / JS decoder symmetry)
   - Parser-Integrated Binary Writer (Approach c-1)
   - Processing flow + Backpatching (parent / next_sibling)
   - PositionMap (UTF-8 → UTF-16 conversion, ASCII-only fast path)
   - Rust-side lazy decoder (`LazyJsdocBlock`, etc., stack value type Copy)
   - `LazySourceFile` (the decoder's root)
   - Helper functions (`ext_offset`, `child_at_visitor_index`)
   - Kind dispatch (`Kind` enum + `from_u8`)
   - Lazy Visitor trait
   - Sharing with NAPI/WASM
   - Code generation
   - Phase 2/3 public Rust API (`parse_to_binary`, `reserialize_binary`)
   - Performance expectations

### Process

7. [Testing Strategy](./testing.md)
   - Design overview (3-axis structure + per-Phase staged adoption)
   - **16 test categories** (unit / encoder / decoder / Roundtrip / compatibility /
     JS / cross-binding / edge cases / Visitor / memory safety / Fuzzing / Snapshot /
     performance / **bit-level** / **lazy/cache** / **compat switching**)
   - Per-Phase test addition schedule
   - Reuse of existing test assets (50-70% reduction)

8. [Benchmark Strategy](./benchmark.md)
   - Design overview + benchmark naming conventions (`parseTyped`, `parseBinary`, etc.)
   - 5 measurement layers
   - **5 categories of metrics** (time / memory / size / **batch benefits** / **lazy decoder**)
   - Benchmark fixtures (existing 7 buckets + new batch / scale)
   - 8 scenarios (A-H) — 4-way comparison / **compat switching** / **batch dedup** / **lazy/sparse**
   - **Phase 1.3 cutover decision: 3 KPI groups** (time / transfer efficiency / memory)
   - Per-Phase benchmark addition schedule

9. [Phases (Implementation phases)](./phases.md)
   - Phase 1.0a-d: Skeleton construction
   - Phase 1.1a-d: Rust + JS decoder implementation
   - Phase 1.2a-d: parser + binding + 4-way comparison
   - Phase 1.3: cutover (atomic, conditional)
   - Phase 2-4
   - Migration strategy
   - crate / package layout

### Detailed batch discussion

10. [Batch Processing (5 batch issues and decisions)](./batch-processing.md)
    - Diagnostic array, Encoder API, JS API naming, empty comments, source text handling

## Related documents

- [AST shape and memory model](../ast.md) — Description of the existing typed AST
- [Performance design](../001-performance/README.md) — Performance design premises
- [JS binding](../003-js-binding/) — JS binding design
- [WASM binding](../004-wasm/) — WASM binding design
- [jsdoccomment compatibility](../005-jsdoccomment-compat/README.md) — Background on compat_mode
- [parsedType](../006-parsed-type/README.md) — Design of the 45 TypeNode kinds

## References

The investigation history and external research are bundled under `./refs/`:

### Design alternatives (not adopted)

- [`./refs/construction-methods.md`](./refs/construction-methods.md) —
  Comparison of Binary AST construction alternatives (Approaches a / b / c-2 vs
  the adopted c-1, full breakdown of Layer 1-2 alternatives)

### External research and existing materials

- [`./refs/tsgo/tsgo-binary-ast.md`](./refs/tsgo/tsgo-binary-ast.md) — Detailed investigation of the tsgo Binary AST (with diagrams)
- [`./refs/tsgo-vs-oxc-ast-transfer.md`](./refs/tsgo-vs-oxc-ast-transfer.md) — tsgo vs oxc approach comparison
- [`./refs/js-rust-transfer.md`](./refs/js-rust-transfer.md) — Selection process for JSON / Raw Transfer / Binary AST
- [`./refs/benchmark-results.md`](./refs/benchmark-results.md) — Detailed benchmark figures
- [`./refs/binary-ast-draft-v2-deep-review.md`](./refs/binary-ast-draft-v2-deep-review.md) — Design review record
