# Benchmark Strategy

Binary AST is **primarily aimed at performance gains**, and the Phase 1.3 cutover
direction is also driven by performance comparisons. The benchmark strategy is
designed around the following five aspects.

## Design overview

The effects of the Binary AST migration are **distributed across multiple layers**,
so benchmarks measure each layer independently and then form an overall judgment
based on user-visible end-to-end metrics.

Key design goals:

- **Independent measurement across 5 layers**: parser standalone / Rust lazy decoder /
  JS lazy decoder / binding end-to-end / competitor comparison. Makes bottlenecks
  identifiable
- **Multi-dimensional metrics**: In addition to time (ns/us/ms), measure
  **memory, allocation count, buffer size, dedup rate, and cache hit rate**
  (verifying the core effects of the lazy decoder)
- **Separate batch scenarios**: Confirm scaling characteristics for single (N=1),
  medium (N=100), and large (N=1000) (verifying batch benefits such as String dedup
  and NodeList skipping)
- **Independently measure the performance delta of toggling compat_mode**: Capture in
  isolation how the 2x size of Extended Data affects performance
- **Objective Phase 1.3 cutover decision**: Quantitatively define required KPIs and
  decide cutover by attainment (eliminating subjective evaluation)

### Convenience names (function names inside the benchmark scripts)

To distinguish the typed AST version from the binary AST version inside the benchmarks,
we use the following convenience names (the production API uses the same `parse(text)`
signature in both):

| Name in benchmarks                   | Actual entity                                     | Source                                                                           |
| ------------------------------------ | ------------------------------------------------- | -------------------------------------------------------------------------------- |
| `parseTyped` / `parseTypedNapi`      | The current `parse` via `crates/ox_jsdoc`         | Until before the Phase 1.3 cutover                                               |
| `parseBinary` / `parseBinaryNapi`    | The new `parse` via `crates/ox_jsdoc_binary`      | Phase 1.1 onward                                                                 |
| `parseTypedWasm` / `parseBinaryWasm` | Same as above (WASM binding versions)             | Same as above                                                                    |
| `parseBatchBinary`                   | The new `parseBatch(items)` API (binary AST only) | [js-decoder.md "JS Public API"](./js-decoder.md#js-public-api-parse--parsebatch) |

These are defined as import aliases at the top of each script under
`tasks/benchmark/scripts/` (they are not function names in production code).

## 1. Measurement targets (5 layers)

### (a) Rust parser standalone

Directly compare the parse times of both parsers. Pure parser performance excluding
NAPI/WASM overhead.

```text
crates/ox_jsdoc/         (typed AST)         baseline
crates/ox_jsdoc_binary/  (binary AST)        improvement target
```

Tool: `criterion` (Rust's standard benchmarking framework)

### (b) Rust lazy decoder

Time to retrieve various properties from the Binary AST byte stream. Verifies the
**effectiveness of lazy access** (confirming that unaccessed parts are not materialized).

Scenarios:

- **Full walk**: Visit all nodes via a visitor (equivalent to ESLint)
- **Sparse access**: Retrieve only some nodes (e.g., tag names only)
- **Single property**: Just one property (e.g., `block.tags().count()`)

### (c) JS lazy decoder

Access a Binary AST received via NAPI/WASM through the JS lazy classes. Run the same
scenarios as (b) on the Rust side, but in JS.

### (d) JS bindings end-to-end

Total time of the `parse()` / `parseBatch()` calls users actually invoke:

- Rust parser -> binary output -> NAPI/WASM transfer -> JS lazy class construction ->
  one property access

This is the **primary practical metric**.

### (e) Competitor parser comparison

Side-by-side comparison with existing baselines:

- `comment-parser` (JS native)
- `@es-joy/jsdoccomment` (JS, comment-parser wrapper)
- `jsdoc-type-pratt-parser` (JS, type-only)
- `ox-jsdoc` (current typed AST version, NAPI/WASM)
- `ox-jsdoc-binary` (new binary AST version, NAPI/WASM)

## 2. Metrics

### Time

| Metric                                                     | Unit         | Method                        |
| ---------------------------------------------------------- | ------------ | ----------------------------- |
| **Parse time**                                             | ns / us / ms | criterion (Rust), mitata (JS) |
| **Encode time** (binary writer alone)                      | ns           | criterion                     |
| **Decode time** (lazy class construction)                  | ns           | criterion / mitata            |
| **JSON.parse time** (for comparison with the current path) | us           | mitata                        |
| **Throughput** (comments per second)                       | comments/sec | derived                       |

### Memory

| Metric                                                      | Unit  | Method                                                                             |
| ----------------------------------------------------------- | ----- | ---------------------------------------------------------------------------------- |
| **Peak memory**                                             | MB    | `dhat` (Rust), Node `process.memoryUsage()`                                        |
| **Allocation count**                                        | count | `dhat-rs`                                                                          |
| **Memory in lazy state** vs **memory after eager (toJSON)** | MB    | before/after diff with `process.memoryUsage` (the core effect of the lazy decoder) |

### Size (transfer efficiency)

| Metric                                           | Unit      | Method                                                 |
| ------------------------------------------------ | --------- | ------------------------------------------------------ |
| **Binary size** (buffer size)                    | byte      | encoder output length                                  |
| **JSON output size** (for comparison)            | byte      | serializer output length                               |
| **size reduction ratio** (binary / JSON)         | %         | derived (transfer reduction at the NAPI/WASM boundary) |
| **Extended Data average size** (basic vs compat) | byte/node | summary inside the encoder                             |

### Batch benefit effects (string dedup / NodeList skipping)

| Metric                                                                           | Unit  | Method                                                                                                 |
| -------------------------------------------------------------------------------- | ----- | ------------------------------------------------------------------------------------------------------ |
| **String dedup rate** (unique / total)                                           | %     | summary inside the encoder (sharing rate of identical tag names etc. during batch)                     |
| **String Data section size** vs **naive total**                                  | byte  | summary inside the encoder                                                                             |
| **Number of emitted NodeLists** vs **number of empty-array NodeList candidates** | count | summary inside the encoder (compared against the encoding.md estimate of "~26 KB saved per 100-batch") |

### Lazy decoder effect (cache hit rate)

| Metric                                                  | Unit  | Method                                                           |
| ------------------------------------------------------- | ----- | ---------------------------------------------------------------- |
| **Proxy construction count** (number of accessed nodes) | count | counter hook in the decoder (paired with testing.md category 15) |
| **Cache hit rate** (re-access / first access)           | %     | same as above                                                    |
| **Number of unvisited nodes** (lazy effect indicator)   | count | total node count - proxy construction count                      |

## 3. Benchmark fixtures

### Existing fixtures (all 7 buckets, reused)

```text
fixtures/perf/
|-- common/                <- typical JSDoc comments (basic-param etc.)
|-- description-heavy/     <- long descriptions (linkcode-description etc.)
|-- type-heavy/            <- complex type expressions (ts-import-record-type etc.)
|-- special-tag/           <- special tags (memberof-borrows etc.)
|-- malformed/             <- invalid input (unclosed-inline-tag etc.)
|-- source/                <- extracts from real code (typed-api-client.ts,
|                            vue-i18n-composer.ts, typescript-checker.ts etc.)
`-- toolchain/             <- tool-specific tags (vue-i18n-custom-tags etc.)
```

### New fixtures (for batch / scale comparisons)

```text
fixtures/bench_scale/
|-- single.jsdoc                     <- N=1 (single comment)
|-- small_batch_10.json              <- N=10
|-- medium_batch_100.json            <- N=100
`-- large_batch_1000.json            <- N=1000

fixtures/bench_files/                <- extracted directly from real JS/TS files
|-- small_file.ts                    <- <100 lines
|-- medium_file.ts                   <- 1000-line class
`-- typescript-checker.ts            <- the existing largest file (54K lines, 226 JSDocs)
```

## 4. Benchmark scenarios

### Scenario A: Single comment parse (pure performance)

```rust
// criterion
bench("parse/single/common-basic-param", || {
    let allocator = Allocator::default();
    typed::parse(&allocator, source, Default::default())
});
bench("parse/single/common-basic-param", || {
    let allocator = Allocator::default();
    binary::parse(&allocator, source, Default::default())
});
```

### Scenario B: Batch parse (NAPI/WASM overhead reduction)

```javascript
// mitata
bench(`napi typed batch ${N}`, () => {
  for (const item of items) {
    parseTyped(item.sourceText)
  }
})
bench(`napi binary batch ${N}`, () => {
  parseBatchBinary(items)
})
```

### Scenario C: Lazy access patterns (effectiveness of the lazy decoder)

```javascript
// "ESLint-like: walk all tags and access fields"
bench('lazy full walk', () => {
  const result = parseBinary(source)
  for (const tag of result.ast.tags) {
    visit(tag.tag, tag.range, tag.description) // all access
  }
})

// "AST type check only"
bench('lazy sparse access', () => {
  const result = parseBinary(source)
  result.ast.tags.length // single property only
})

// "JSON serialization (eager full materialization)"
bench('lazy full materialization', () => {
  const result = parseBinary(source)
  JSON.stringify(result.ast) // fully materialize via toJSON
})
```

### Scenario D: 4-way comparison (major competitors + own old/new)

```javascript
group(`comparison: ${fixture.name}`, () => {
  bench('comment-parser', () => commentParser.parse(source))
  bench('jsdoccomment', () => jsdoccomment.parse(source))
  bench('ox-jsdoc napi typed', () => parseTypedNapi(source))
  bench('ox-jsdoc napi binary', () => parseBinaryNapi(source))
  bench('ox-jsdoc wasm typed', () => parseTypedWasm(source))
  bench('ox-jsdoc wasm binary', () => parseBinaryWasm(source))
})
```

### Scenario E: Memory profile

```rust
// dhat-rs
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[test]
fn memory_profile_typescript_checker() {
    let _profiler = dhat::Profiler::new_heap();
    let allocator = Allocator::default();
    let source = include_str!("../../../fixtures/perf/source/typescript-checker.ts");
    binary::parse(&allocator, source, Default::default());
    // dhat report: total bytes, max bytes, allocation count
}
```

### Scenario F: Performance delta of toggling compat_mode

Pairs with testing.md category 16. With the same input, toggle `compat_mode` ON/OFF
and compare buffer size and processing time:

```javascript
// mitata
group('compat_mode toggle', () => {
  bench('encode basic', () => parseBinary(source, { compat_mode: false }))
  bench('encode compat', () => parseBinary(source, { compat_mode: true }))
})

// Also record buffer size deltas (encoder output length)
bench('size: basic', () => encode(source, { compat_mode: false }).byteLength)
bench('size: compat', () => encode(source, { compat_mode: true }).byteLength)
```

**Expected values** (see [the Extended Data section size table in format.md](./format.md#extended-data-section)):

- `JsdocBlock`: 18 -> 40 bytes (Extended Data adds 22 bytes)
- `JsdocTag`: 8 -> 22 bytes (adds 14 bytes)
- For large fixtures (typescript-checker.ts), expect a **+30-50% buffer size increase**

### Scenario G: Isolated measurement of batch dedup effects

Independently measure the **String dedup** and **NodeList skipping** effects, both
emphasized in format.md / encoding.md:

```text
fixtures/bench_batch/
|-- batch_unique.json           <- 100 comments, all tag names different (no dedup effect)
`-- batch_dedup_heavy.json      <- 100 comments, shared tag names (maximum dedup effect)
```

```javascript
// String dedup effect
group('String dedup', () => {
  bench('batch_unique (dedup 0%)', () => parseBatchBinary(batchUnique))
  bench('batch_dedup_heavy', () => parseBatchBinary(batchDedupHeavy))
})

// NodeList skipping effect (compare with a fixture that has many empty arrays)
bench('batch_with_empty_arrays', () => parseBatchBinary(batchEmptyArrays))
// -> verify the `number of emitted NodeLists` in the encoder summary
//   (encoding.md "NodeList optimization": measured against ~1100 / ~26 KB saved per 100-batch)
```

### Scenario H: Measuring lazy / sparse access

Pairs with testing.md category 15. Visualize the lazy effect by the ratio of
**proxy construction count vs total node count**:

```javascript
// Insert a counter hook to measure
let proxyConstructed = 0
const result = parseBinary(largeFixture, {
  decoder: { onProxyConstructed: () => proxyConstructed++ }
})

bench('sparse access: tag count only', () => {
  void result.ast.tags.length // only the root + tags NodeList proxies are constructed
})
// Expected: proxyConstructed << totalNodeCount (e.g., 2 / 5000)

bench('full walk: visitor', () => {
  visitor.visit(result.ast) // proxies constructed for all nodes
})
// Expected: proxyConstructed === totalNodeCount

bench('cache hit rate', () => {
  for (let i = 0; i < 10; i++) {
    void result.ast.tags // 2nd access onwards hits the #internal.$tags cache
  }
})
```

## 5. Comparison baseline and acceptance criteria

### Phase 1.3 cutover decision (primary KPIs)

#### Time (parse performance)

| KPI                        | Target                | Goal (Binary AST vs typed AST)     | Judgment     |
| -------------------------- | --------------------- | ---------------------------------- | ------------ |
| **Parse time (single)**    | typescript-checker.ts | **2x or more faster**              | Required     |
| **Parse time (batch 100)** | source/\* x 100       | **3x or more faster**              | Required     |
| **end-to-end (NAPI)**      | parse(text)           | **3x or more faster**              | Required     |
| **end-to-end (WASM)**      | parse(text)           | **2x or more faster**              | Recommended  |
| **lazy sparse access**     | tag count only        | **1/10 or less** of typed AST time | Recommended  |
| **vs comment-parser**      | all source fixtures   | **Close the gap to within 5x**     | Stretch goal |

#### Transfer efficiency (buffer size)

| KPI                                       | Target                                                                                                          | Goal                                           | Judgment    |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- | ----------- |
| **size reduction ratio** (binary / JSON)  | typescript-checker.ts                                                                                           | **40% or less** (= 60% smaller than JSON)      | Required    |
| **String dedup rate** (batch_dedup_heavy) | batch 100 with shared tag names                                                                                 | **50% or more reduction**                      | Recommended |
| **Empty NodeList reduction** (batch 100)  | [encoding.md "NodeList optimization"](./encoding.md#nodelist-optimization-skip-empty-arrays-option-a2) estimate | Verify a measured **~1100 / ~26 KB reduction** | Recommended |

#### Memory

| KPI                                             | Target                 | Goal                                   | Judgment    |
| ----------------------------------------------- | ---------------------- | -------------------------------------- | ----------- |
| **Peak memory**                                 | typescript-checker.ts  | **At or below** typed AST              | Recommended |
| **Memory in lazy state**                        | sparse access scenario | **1/3 or less** of typed AST           | Recommended |
| **Proxy construction count / total node count** | sparse access          | **5% or less** (lazy effect indicator) | Recommended |

-> Execute the Phase 1.3 cutover when **all required items** are met; otherwise
reconsider the design. Recommended items are continuous improvement targets
after the cutover.

### Regression detection criteria (after Phase 1.3)

| Metric      | Tolerance     | Action on detection |
| ----------- | ------------- | ------------------- |
| Parse time  | +/-5%         | Warning             |
| Parse time  | -10% or worse | CI fail             |
| Peak memory | +/-10%        | Warning             |
| Peak memory | -20% or worse | CI fail             |
| Binary size | +/-5%         | Warning             |

## 6. Measurement environment and noise reduction

```text
- Release build (cargo bench --release is the standard)
- LTO + codegen-units=1 (release profile in Cargo.toml)
- Host machine: Apple M1 Max (or M2/M3) etc. as a fixed reference
- CPU governor: performance mode (on Linux)
- Noise reduction: warm up before measurement (criterion default)
- Iterations: until criterion / mitata auto-converge (typically 100+ iter)
- Eliminate external factors: stop background processes during measurement (when local)
```

## 7. Report structure

### Per-PR benchmark report

When a PR makes changes that affect performance, paste the following into the PR
description:

```markdown
## Benchmark results

### parse/single/common-basic-param

| Implementation       |   Time |        Δ vs main |
| -------------------- | -----: | ---------------: |
| typed AST            | 422 ns |         baseline |
| binary AST (this PR) | 145 ns | **2.91x faster** |

### parse/batch/source-typescript-checker (226 comments)

| Implementation |  Time |        Δ vs main |
| -------------- | ----: | ---------------: |
| typed AST      | 95 µs |         baseline |
| binary AST     | 28 µs | **3.39x faster** |

Test environment: Apple M1 Max, Node.js v24.15, Rust 1.92 (release)
```

### Periodic report (per alpha release)

Save benchmark results as
`tasks/benchmark/results/benchmark-results-YYYY-MM-DD.md` so the history is trackable
(consolidated under the same `tasks/benchmark/` directory as the scripts).

## 8. CI integration

```yaml
# .github/workflows/benchmark.yml
on:
  pull_request:
    paths:
      - 'crates/**'
      - 'napi/**'
      - 'wasm/**'
  push:
    branches: [main]

jobs:
  rust-bench:
    runs-on: macos-latest
    steps:
      - run: cargo bench --bench parser_compare -- --save-baseline ${{ github.ref_name }}
      - run: cargo bench --bench lazy_decoder
      - if: github.event_name == 'pull_request'
        run: cargo bench -- --baseline main # compare against main

  js-bench:
    runs-on: macos-latest
    steps:
      - run: pnpm bench:full
      - run: pnpm bench:lazy-access
      - run: pnpm bench:competitor-comparison
```

## 9. Location of benchmark scripts

```text
tasks/benchmark/
|-- scripts/
|   |-- full-comparison.mjs              <- existing, extended for the 4-way comparison
|   |-- parser_compare_napi.mjs          <- new (typed vs binary, NAPI)
|   |-- parser_compare_wasm.mjs          <- new (typed vs binary, WASM)
|   |-- lazy_access_patterns.mjs         <- new (Scenario C)
|   |-- batch_scaling.mjs                <- new (single -> batch 1000)
|   |-- memory_profile.mjs               <- new (Scenario E)
|   `-- competitor_comparison.mjs        <- new (Scenario D)
`-- results/                             <- archive of measurement results
    `-- 2026-04-XX/...
```

## 10. Per-Phase benchmark addition schedule

The sub-phase numbers per Phase align with the Phase structure in
[phases.md](./phases.md) under "crate / package layout" (1.0a-d, 1.1a-d, 1.2a-d).

| Phase     | Benchmarks added                                                                                                                                                                                     |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1.0a-d    | (Not yet at the performance measurement stage; only skeleton verification)                                                                                                                           |
| 1.1a      | Add Rust encoder standalone benchmarks via criterion; add Scenario F (compat_mode toggle performance/size delta) alongside the encoder feature                                                       |
| 1.1b-c    | Add Rust decoder standalone benchmarks via criterion, with memory metrics (dhat: peak / allocation count) at the same time                                                                           |
| 1.1d      | (JS decoder standalone benchmarks are run after binding integration; skipped here)                                                                                                                   |
| 1.2a      | (Parser implementation only; no benchmarks added)                                                                                                                                                    |
| 1.2b      | Add benchmarks via NAPI binding (Scenario A: single parse, B: batch, G: batch dedup effects); also measure the transfer efficiency KPI (size reduction ratio)                                        |
| 1.2c      | Add benchmarks via WASM binding + Scenario H (lazy/sparse access, proxy construction count hook) + (alpha release) all scenarios including competitor comparison (Scenario D)                        |
| 1.2d      | benchmarks: **4-way comparison** (napi/wasm x typed/binary), **judgment: confirm KPI attainment** (required items across time + transfer efficiency + memory) -> Phase 1.3 cutover GO/NO-GO decision |
| After 1.3 | Integrate regression detection into CI (parse time / peak memory / binary size)                                                                                                                      |
