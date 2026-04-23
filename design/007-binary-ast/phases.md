# Phases (Implementation phases)

## Design overview

The transition of ox-jsdoc to the Binary AST proceeds via a **coexistence +
atomic cutover** strategy. We do not create an intermediate state where the
typed AST and the Binary AST coexist within the same binding; instead, we
migrate everything at once in Phase 1.3 based on the Phase 1.2d benchmark
results.

Key decisions:

- **Coexistence (add a new crate `crates/ox_jsdoc_binary/`)**: develop the
  Binary AST implementation without touching the existing `crates/ox_jsdoc/`,
  giving zero impact to existing users during development
- **Atomic cutover (single migration in Phase 1.3)**: no feature flag or config
  switch
  - Reasons: maintaining two paths long term is costly; since the API surface
    is identical, users need no modifications
  - Why we do not adopt the alternative (feature flag): at the pre-1.0 stage,
    breaking changes are acceptable, and if benchmark results are GO, switching
    in one shot is simpler
- **Decide GO/NO-GO using objective KPIs**: the cutover is decided by whether
  the required KPIs of 2x parse time / 3x batch / 3x end-to-end are met
  (excluding subjective evaluation; see
  [benchmark.md "Phase 1.3 cutover decision"](./benchmark.md#phase-13-cutover-decision-primary-kpis))
- **Mechanical progression of sub-phases a-d**: split each Phase into 4
  sub-phases and merge PRs small and frequently (Option 3: Stepping stones).
  Reduces review cost and keeps build/test passing at each point
- **Code generation isolated to Phase 4**: Phase 1-3 prioritize speed and
  correctness with hand-written implementations; Phase 4 transitions to
  auto-generation from the AST schema once stable

## Phase 1: Binary AST foundation + atomic cutover

The new and old parsers run side by side in `crates/ox_jsdoc/` (typed) and
`crates/ox_jsdoc_binary/` (new), with PRs merged incrementally per sub-phase
(Option 3: Stepping stones). Based on the Phase 1.2d benchmark results, the
atomic cutover is performed in Phase 1.3. We do not create an intermediate
state where the typed AST and the lazy decoder are mixed (everything migrates
at once within the cutover).

### Phase 1.0a-d: Skeleton construction (skeleton-only PRs)

Add only the skeleton (type definitions + signatures + `unimplemented!()`
stubs) for each module to `crates/ox_jsdoc_binary/`. The build/test (snapshot)
must pass.

- **Phase 1.0a**: `format/` module skeleton
  - Type definitions for the Header struct, Kind enum (including Common Data),
    NodeRecord layout
  - Each layout constant (offset, size) made `const`
- **Phase 1.0b**: `writer/` module skeleton
  - `BinaryWriter` struct, only signatures of `write_node_*()` functions
  - API design for String Table / Extended Data placement logic
- **Phase 1.0c**: `decoder/` module skeleton
  - Copy value-type struct definitions for Rust lazy decoder classes
    (`LazyJsdocBlock`, `LazyJsdocTag`, ...)
  - Methods can be `todo!()`
- **Phase 1.0d**: `parser/` module skeleton
  - Only the `parse()` signature; `unimplemented!()` is fine
  - Existing `crates/ox_jsdoc/` parser keeps working (untouched)

### Phase 1.1a-d: Rust encoder/decoder + JS shared decoder

- **Phase 1.1a**: Rust encoder implementation
  - Implement each `write_node_*()` of `BinaryWriter`, conforming to the Phase
    1.0a format spec
  - Cover all 60 emitted kinds at once (15 comment AST + 45 TypeNode); the
    62-discriminant total includes 1 Sentinel and 1 reserved-only `NodeList`
    that the encoder never emits — variable-length child lists use inline
    `(head_index, count)` metadata in Extended Data instead
  - Implement the String Table with zero-copy source slicing
  - Include compat_mode extension part (writing extra metadata when Header bit0 is ON)
  - Start encoder unit benchmarks with criterion
- **Phase 1.1b**: Rust lazy decoder implementation
  - Implement `LazySourceFile` (the decoder's root, orchestrating Header
    parsing + String table + Root array; see
    [rust-impl.md "LazySourceFile (the decoder's root)"](./rust-impl.md#lazysourcefile-root-of-the-decoder))
  - Implement `LazyJsdocBlock`, `LazyJsdocTag`, ..., `LazyTypeNode` family as
    Copy value types + getters
  - Implement common helpers (`ext_offset`, `child_at_visitor_index`; see
    [rust-impl.md "Helper functions"](./rust-impl.md#helper-functions-shared-parts-for-reading-binary-ast))
  - Hand-write `kind_table.rs` (Kind enum + `from_u8`; see
    [rust-impl.md "Kind dispatch"](./rust-impl.md#kind-dispatch-kind--type-selection))
    (code-generated at Phase 4)
  - Read the extension region by referencing `LazySourceFile.compat_mode`
  - Decoder unit benchmarks
- **Phase 1.1c**: Rust `LazyJsdocVisitor` trait implementation
  - Hand-write the visitor trait (planned for code generation at Phase 4)
  - Memory safety tests
- **Phase 1.1d**: First version of the `@ox-jsdoc/decoder` JS package
  - Implement `RemoteSourceFile` (the decoder's root, Header parsing +
    stringCache + nodeCache; see
    [js-decoder.md "RemoteSourceFile (the decoder's root class)"](./js-decoder.md#remotesourcefile-the-decoders-root-class))
  - Hand-written JS lazy classes for `RemoteJsdocBlock`, `RemoteJsdocTag`, ...,
    `RemoteTypeNode` family
  - Implement common helpers (`extOffsetOf`, `childAtVisitorIndex`; see
    [js-decoder.md "Helper functions"](./js-decoder.md#helper-functions-shared-parts-for-reading-the-binary-ast))
  - Hand-write `kind-dispatch.js` (`KIND_TABLE` flat table; see
    [js-decoder.md "Kind dispatch"](./js-decoder.md#kind-dispatch-kind--class-selection))
    (code-generated at Phase 4)
  - `RemoteNodeList extends Array`, `#internal` encapsulation, `EMPTY_NODE_LIST` singleton
  - Implement `toJSON()` + `Symbol.for("nodejs.util.inspect.custom")` (debug helpers)
  - JS unit tests (passing DataView directly)

### Phase 1.2a-d: Parser full implementation + binding + 4-way comparison

- **Phase 1.2a**: Parser full implementation (typed AST → binary writer)
  - Implement `crates/ox_jsdoc_binary/src/parser/`, calling the Phase 1.1a `BinaryWriter`
  - All 60 emitted kinds (15 comment AST + 45 TypeNode) + all parsedType
    TypeNodes, single comment (N=1) case (Sentinel and reserved-only
    `NodeList` are not emitted)
  - Copy existing typed AST Rust tests to the `ox_jsdoc_binary` side for
    regression verification (do not add new tests)
- **Phase 1.2b**: NAPI binding `ox-jsdoc-binary`
  - Add the `napi/ox-jsdoc-binary/` package (depends on `crates/ox_jsdoc_binary`)
  - Zero-copy NAPI Buffer sharing
  - Import `@ox-jsdoc/decoder` on the JS side
  - Reuse the existing `napi/ox-jsdoc/test/` by swapping imports
    (Roundtrip / compatibility / cross-binding)
  - Add NAPI binding benchmarks for scenario A (single parse) + B (batch)
- **Phase 1.2c**: WASM binding `ox-jsdoc-binary`
  - Add the `wasm/ox-jsdoc-binary/` package (sharing a view of `wasm.memory.buffer`)
  - Reuse the existing `wasm/ox-jsdoc/test/` by swapping imports
  - WASM binding benchmark + alpha release + all scenarios including
    competitor comparison (scenario D)
- **Phase 1.2d**: 4-way comparison + KPI decision
  - Benchmark: napi/wasm × typed/binary **4-way comparison**
  - Performance tests, peak memory measurement
  - **Decision**: confirm that key KPIs (2x or more parse time, 3x or more
    batch, 3x or more end-to-end NAPI) are met → **GO/NO-GO decision** for
    Phase 1.3 cutover

### Phase 1.3: cutover (atomic PR, conditional)

Based on the Phase 1.2d decision result, perform one of the following as an
atomic PR:

- **GO case** (Binary AST meets KPIs, expected case):
  - Delete `crates/ox_jsdoc/`, rename `crates/ox_jsdoc_binary/` to `crates/ox_jsdoc/`
  - Replace the contents of `napi/ox-jsdoc/` with the binary version, alias
    or delete `napi/ox-jsdoc-binary/`
  - Same for `wasm/ox-jsdoc/`
  - Make `@ox-jsdoc/decoder` permanent
  - **Completely rewrite the existing validator (321 lines) / analyzer (142 lines)
    to the lazy decoder API**
    (see "validator / analyzer migration guidelines" below for details)
  - Add **Fuzzing tests** before cutover
- **NO-GO case** (Binary AST misses KPIs, unexpected):
  - Delete `crates/ox_jsdoc_binary/`, `napi/ox-jsdoc-binary/`, `wasm/ox-jsdoc-binary/`
  - Also delete `@ox-jsdoc/decoder`
  - Reconsider the design

→ See the "crate / package layout" subsection below for details

### Realistic goals for Phase 1

Distinguish the design's "ideal target" from the Phase 1.3 cutover "minimum
line (required KPIs)":

| Layer                                        | Number                                                     | Source / Use                                                                                                                    |
| -------------------------------------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| **Required KPIs** (cutover decision minimum) | parse time 2x, 3x at batch 100, 3x or more end-to-end NAPI | [benchmark.md "Phase 1.3 cutover decision"](./benchmark.md#phase-13-cutover-decision-primary-kpis) — Phase 1.3 NO-GO if not met |
| **Realistic goal for Phase 1**               | **5-10x improvement** (829 µs → ~80-160 µs)                | Design-level expectation — we want to exceed the KPIs by this margin                                                            |
| **Ideal target**                             | 10-15x improvement (~50-80 µs)                             | Continued improvement target from Phase 2 onward                                                                                |

The "realistic goal" is a design-level estimate **assuming we exceed the
required KPIs**. We decide cutover only based on the required KPIs and tune
toward the realistic goal post-cutover.

Design rationale:

- **Rationale 1**: vs. the existing `parse_direct` (NAPI object direct return)
  measurement of ~30 µs and the JSON path's ~80 µs
- **Rationale 2**: with Approach c-1, the typed AST construction cost
  disappears too, making it faster than c-2
- **Rationale 3**: with lazy decoding, "only accessed nodes are materialized";
  if the JS walker does not use all nodes, it becomes even more efficient

### validator / analyzer migration guidelines (carried out within the Phase 1.3 GO case)

Rewrite pattern:

```rust
// Old (typed AST, direct field access)
pub fn validate_comment(comment: &JsdocBlock<'_>, options: ValidationOptions) -> ValidationOutput {
    for tag in &comment.tags {
        let tag_name = tag.tag.value;
        if let Some(body) = tag.body.as_ref() {
            match body.as_ref() {
                JsdocTagBody::Generic(g) => { /* g.value, g.description */ }
                JsdocTagBody::Borrows(_) => { /* */ }
                JsdocTagBody::Raw(_) => { /* */ }
            }
        }
    }
    // ...
}

// New (lazy decoder, method calls)
pub fn validate_comment(comment: LazyJsdocBlock<'_>, options: ValidationOptions) -> ValidationOutput {
    for tag in comment.tags() {
        let tag_name = tag.tag().name();
        if let Some(body) = tag.body() {
            match body {
                LazyJsdocTagBody::Generic(g) => { /* g.value(), g.description() */ }
                LazyJsdocTagBody::Borrows(_) => { /* */ }
                LazyJsdocTagBody::Raw(_) => { /* */ }
            }
        }
    }
    // ...
}
```

Main conversions:

- `&JsdocBlock<'_>` → `LazyJsdocBlock<'_>` (value type Copy, no reference needed)
- `.field` → `.method()`
- `Vec<T>` → `Iterator<Item=T>` (call `.collect()` if collect is required)
- `Option<&str>` is handled the same way
- enum match patterns are the same

Expected to complete with about 463 lines of mechanical rewrite.

## Phase 2: Batch support + public encoder API

Phase 1 only handles single comments (N=1). Phase 2 adds batch processing and
externally-public APIs.

- **Batch support**: Root Index Array, Diagnostics section, `parseBatch()`
  API, `BatchItem` struct
- **Public Rust API** (post-typed-AST API; see
  [rust-impl.md "Public Rust API (Phase 2 onward)"](./rust-impl.md#public-rust-api-phase-2-onward)):
  - `parse_to_binary(items: &[BatchItem<'_>], options) -> Vec<u8>` — for
    IPC/network use cases; input is `&str` (sourceText)
  - `reserialize_binary(bytes: &[u8]) -> Vec<u8>` — for persistent caches,
    etc. (typically a no-op)
  - No API to encode from the typed AST to binary (the source of truth is
    always the Binary AST)
- Full operation of batch benchmarks (scenarios B / G) (single → batch 1000
  scaling, isolated measurement of the String dedup effect)

## Phase 3: Environment integration & spec documentation

- **Document the format spec**: for implementers in other languages
  (Go/Python/Rust)
- **Make `decode_binary()` public** (under consideration): ox-jsdoc itself
  does not use it, but for interop use cases
- **Migration tool** at major version bumps (follow Header Major bit changes)

## Phase 4: Code generation

- Define the AST schema (node kind, fields, child properties, common data mapping)
- Generate Rust binary writer functions from the schema (called by the parser)
- Generate Rust lazy decoder classes (`LazyJsdocBlock`, etc.) from the schema
- Generate JS lazy decoder classes (`RemoteJsdocBlock`, etc.) and visitor keys from the schema
- CI for automatic regeneration on schema changes

## Migration strategy

Phase 1-2 contain breaking changes, so proceed in the following order:

1. **Phase 1.0a-d**: keep the existing typed AST path completely intact and
   add only skeletons to `crates/ox_jsdoc_binary/` (PRs are mechanically mergeable)
2. **Phase 1.1a-d**: implement the Rust + JS encoder/decoder (the parser is
   not yet started, so no impact on the existing typed AST path)
3. **Phase 1.2a-c**: full parser implementation + add the NAPI/WASM bindings
   in coexistence as new packages (alpha-release `ox-jsdoc-binary` as a
   separate npm package)
4. **Phase 1.2d**: KPI decision via 4-way benchmark → cutover GO/NO-GO decision
5. **Phase 1.3**: cutover atomic PR (GO case: completely remove the typed AST
   path and rewrite validator/analyzer to the lazy decoder API. NO-GO case:
   delete the binary crate)
6. **Phase 2-3**: add batch support, public APIs, format spec documentation,
   and other externally-public additions

## crate / package layout (coexistence + shared decoder)

The new and old parsers run side by side, with cutover at Phase 1.3 based on
benchmark results. They are split at the crate / npm package level so that
benchmarks can clearly measure performance differences:

```text
ox-jsdoc/
├── crates/
│   ├── ox_jsdoc/                  ← Existing typed AST parser (kept through Phase 1.0-1.2)
│   └── ox_jsdoc_binary/           ← New binary AST parser (added at Phase 1.0)
│       └── src/
│           ├── lib.rs
│           ├── format/             ← Binary AST format spec (Header, Kind, NodeRecord, StringTable)
│           ├── writer/             ← Binary writer (called by the parser)
│           ├── parser/             ← Parser that writes Binary AST directly
│           ├── decoder/            ← Rust lazy decoder
│           │   ├── source_file.rs  ← LazySourceFile (decoder root, Header parsing)
│           │   ├── helpers.rs      ← ext_offset, child_at_visitor_index, etc.
│           │   └── nodes/          ← LazyJsdocBlock, etc., Copy value types
│           ├── kind_table.rs       ← Kind enum + dispatch (code-generated at Phase 4)
│           ├── visitor.rs          ← LazyJsdocVisitor trait
│           ├── validator.rs        ← Lazy decoder based
│           └── analyzer.rs         ← Lazy decoder based
│
├── napi/
│   ├── ox-jsdoc/                  ← Existing (depends on crates/ox_jsdoc)
│   └── ox-jsdoc-binary/           ← New (depends on crates/ox_jsdoc_binary, Phase 1.2b)
│
├── wasm/
│   ├── ox-jsdoc/                  ← Existing
│   └── ox-jsdoc-binary/           ← New (Phase 1.2c)
│
└── npm/
    └── @ox-jsdoc/
        └── decoder/               ← New shared JS lazy decoder (Phase 1.1d)
                                      - Imported from napi/ox-jsdoc-binary/
                                      - Imported from wasm/ox-jsdoc-binary/
```

### Shared code policy

**Code duplication is allowed** between `crates/ox_jsdoc/` and `crates/ox_jsdoc_binary/`:

- It is OK if common ParseOptions struct or ValidationMode enum, etc., exist
  in both crates
- Splitting out a shared crate (`ox_jsdoc_format`, etc.) is a future decision
  (Phase 3 onward)
- Duplicated code is expected to be on the order of dozens of lines (mostly
  type definitions)

### Performance comparison via benchmark

In Phase 1.2d, conduct a **4-way comparison**:

```javascript
import { parse as parseNapiTyped } from 'ox-jsdoc'
import { parse as parseNapiBinary } from 'ox-jsdoc-binary'
import { parse as parseWasmTyped } from '@ox-jsdoc/wasm'
import { parse as parseWasmBinary } from '@ox-jsdoc/wasm-binary'
```

```rust
// crates/ox_jsdoc_binary/benches/parser_compare.rs
use ox_jsdoc as typed_ast;
use ox_jsdoc_binary as binary_ast;

bench("typed_ast", || typed_ast::parse(...));
bench("binary_ast", || binary_ast::parse(...));
```

### Phase 1.3 cutover (conditional based on benchmark results)

For the GO/NO-GO branch of cutover and the concrete steps for each case, see
the **"Phase 1.3: cutover (atomic PR, conditional)"** section above. The
crate / package layout rename / removal is also documented in that section.

For this reason, the Phase 1.3 atomic PR is **conditional** and waits for the
Phase 1.2d benchmark results.
