<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./assets/oxjsdoc-light.svg">
    <source media="(prefers-color-scheme: light)" srcset="./assets/oxjsdoc-dark.svg">
    <img alt="Ox JSDOC logo" src="./assets/oxjsdoc-dark.svg" height="60">
  </picture>
</p>

## ox-jsdoc

High-performance JSDoc parser inspired by the `oxc` project.

## Motivation

- Parse JSDoc at native Rust speed when generating JSDoc documentation in `ox-content`, a potential alternative to TypeDoc.
- Speed up lint performance for `eslint-plugin-jsdoc` when it runs on Oxlint.

## Benchmark Results

Benchmarks were run on 2026-05-15 against the post-cutover canonical Binary AST package (`ox-jsdoc` / `@ox-jsdoc/wasm`) and the typed AST reference (`ox-jsdoc-origin` / `@ox-jsdoc/wasm-origin`). All Rust-direct rows were re-measured in the same run on the same machine. Treat these numbers as a snapshot for this workspace, not as a stable performance guarantee.

See [design/009-jsdoc-linter-benchmark/README.md](design/009-jsdoc-linter-benchmark/README.md) for the benchmark design, fixture selection, measurement methodology, and result interpretation notes.

### TL;DR

- `ox-jsdoc` is most useful when parsing can be batched. In the full-file parser-only benchmark, `ox-jsdoc` NAPI `parseBatch` parses 226 comments in 300.720 us (1.331 us/comment), while `comment-parser` loop is 1.86x slower and `jsdoccomment` loop is 2.82x slower. The main win is avoiding per-comment JavaScript/native calls and repeated parser setup.
- For ESLint-only usage, replacing the parser does not produce a large end-to-end lint speedup by itself. These runs are still dominated by ESLint, config loading, AST traversal, and, for TypeScript, `@typescript-eslint/parser`. In this fixture set, `eslint-ox-jsdoc-batch` is essentially the same as upstream ESLint on JS (`1.03x faster`) and TS (`1.01x faster`).
- The larger linting benefit appears when the same JSDoc rules run on Oxlint. `oxlint-ox-jsdoc-batch` is 1.22x faster on JS and 5.19x faster on TS than upstream ESLint, even with the JS plugin bridge. Oxlint's built-in native JSDoc path is the fastest linter reference in this benchmark (`3.11x` on JS, `93.76x` on TS).
- The Rust-direct table shows that the canonical batch path (`parse_batch_to_bytes` at 885 ns/comment, `parse_batch_into` at 978 ns/comment) is in the same neighbourhood as `oxc_jsdoc`'s lazy parse (952 ns/comment), even though `ox-jsdoc` eagerly emits a full Binary AST while `oxc_jsdoc` only walks tags lazily. Per-comment usage (`parse_into`, `parse`) is 1.25x–1.39x slower than `oxc_jsdoc` because the writer setup cost is paid for every comment instead of being amortized across a batch.
- For `ox-content` documentation generation, the parser-only numbers are the more relevant signal than the ESLint-only runs: a Rust-native batch parser can keep JSDoc parsing a small part of the total documentation pipeline when many comments are parsed from a file at once.
- The Rust-direct Criterion table is a Rust-side reference, not an end-to-end JavaScript benchmark. Use it to separate parser-core lower bounds, typed-AST vs Binary-AST emission cost, batch vs per-comment writer-setup cost, and bytes-output cost; use the Node / NAPI table for the practical cost seen from JavaScript.

### Measurement Environment

| Item         | Value                                                 |
| ------------ | ----------------------------------------------------- |
| Machine      | MacBook Pro                                           |
| CPU          | Apple M1 Max, 10 cores (8 performance + 2 efficiency) |
| Memory       | 64 GB                                                 |
| Architecture | arm64                                                 |
| OS           | macOS 15.5 (24F74), Darwin 24.5.0                     |
| Node.js      | v24.15.0                                              |
| pnpm         | 10.33.0                                               |
| Rust / Cargo | 1.94.1                                                |
| Hyperfine    | 1.19.0                                                |
| Mitata       | 1.0.34                                                |

### Measurement Method

- Linter benchmarks were run through `pnpm --filter @ox-jsdoc/benchmark benchmark:jsdoc-linter`.
- The linter script generates configs, then invokes `hyperfine` directly from a shell script with `--warmup 1`, `--runs 10`, and `--ignore-failure`.
- Each linter command is executed from inside the fixture directory so `.` is resolved against the intended project tree.
- Parser-only Node.js benchmarks were run through `pnpm --filter @ox-jsdoc/benchmark benchmark:parse-batch`, which uses `node --expose-gc`.
- Parser-only Node.js timings use `mitata.measure()` via `tasks/benchmark/scripts/lib/measure.mjs`: 10 rounds, discard the first round, trim the fastest and slowest remaining rounds, then report the mean of the remaining 7 round p50 values.
- Rust-direct parser references were run with `cargo bench` / Criterion and are reported separately from Node / NAPI / WASM timings. The runs cover the canonical `ox_jsdoc` (Binary AST) entry points (`parse` / `parse_into` / `parse_batch` / `parse_batch_into` / `parse_to_bytes` / `parse_batch_to_bytes`) plus `parse_block_into_data` (phase 1 only), the typed AST entry point `ox_jsdoc_origin::parse_comment`, and `oxc_jsdoc` as the Oxlint-native reference.

### JSDoc Linter

End-to-end CLI benchmark using Hyperfine. The `combined` rule set enables:

- `jsdoc/empty-tags`
- `jsdoc/require-param-description`
- `jsdoc/require-param-type`

Fixtures:

- JS: `refers/eslint-plugin-jsdoc/src/` - 86 files, 28,741 lines, 1,170 JSDoc blocks
- TS: `refers/vscode/src/` - 5,996 files, 1,951,540 lines, 25,238 JSDoc blocks

This README summarizes mean timings. Full median, p95, and standard deviation values are in `tasks/benchmark/results/jsdoc-linter-hyperfine.md`.

| Pattern                  |  JS mean | JS speed vs baseline |  TS mean | TS speed vs baseline |
| ------------------------ | -------: | -------------------: | -------: | -------------------: |
| `eslint-jsdoc-upstream`  | 721.6 ms |     1.00x (baseline) | 40.316 s |     1.00x (baseline) |
| `oxlint-jsdoc-native`    | 231.8 ms |         3.11x faster | 430.0 ms |        93.76x faster |
| `eslint-ox-jsdoc-single` | 797.6 ms |         1.11x slower | 41.018 s |         1.02x slower |
| `eslint-ox-jsdoc-batch`  | 701.3 ms |         1.03x faster | 39.994 s |         1.01x faster |
| `oxlint-ox-jsdoc-batch`  | 592.6 ms |         1.22x faster |  7.766 s |         5.19x faster |

### Parser-only

In-process Node.js benchmark using the 226 JSDoc comments from `fixtures/perf/source/typescript-checker.ts`. These rows compare parser entry points and keep linter integration out of the timed path.

In the parser-only tables, `loop` means the benchmark calls the parser once per JSDoc comment in a JavaScript or Rust loop. It is the non-batch baseline used to show how much overhead `parseBatch` removes by parsing all comments in one call.

| Parser | Full file total | Per comment | vs canonical NAPI parseBatch |
| --- | --: | --: | --: |
| `ox-jsdoc NAPI (parseBatch)` | 300.720 us | 1.331 us | 1.00x |
| `ox-jsdoc WASM (parseBatch)` | 462.214 us | 2.045 us | 1.54x slower |
| `comment-parser (loop)` | 559.584 us | 2.476 us | 1.86x slower |
| `jsdoccomment (loop)` | 847.494 us | 3.750 us | 2.82x slower |
| `@ox-jsdoc/jsdoccomment (parseCommentBatch)` | 1.142 ms | 5.055 us | 3.80x slower |
| `ox-jsdoc-origin NAPI (typed AST, loop)` | 1.684 ms | 7.452 us | 5.60x slower |
| `@ox-jsdoc/jsdoccomment (parseComment loop)` | 4.613 ms | 20.412 us | 15.34x slower |

Rust-direct parser references are separate Criterion measurements. They do not include Node.js, NAPI / WASM boundary cost, or `@ox-jsdoc/jsdoccomment` normalization. `oxc_jsdoc` is included as a Rust reference because it is the public API used by Oxlint's native JSDoc support.

The last column compares each row against `oxc_jsdoc` using `faster` / `slower` wording. Rows with different scenarios are not interchangeable feature-for-feature comparisons.

- `parse_block_into_data` is the canonical crate's internal phase-1 parser. It walks one JSDoc block and builds intermediate `BlockData` / `TagData` plus diagnostics, but does not emit Binary AST bytes. The benchmark loops over all 226 comments, so this is a parser-core lower-bound measurement.
- `ox_jsdoc_origin::parse_comment` is the typed AST entry point — it parses one block into an arena-allocated typed AST and skips Binary AST emission entirely. The gap between this and the canonical `parse_into` is the cost of binary serialization.
- `parse_batch_into` and `parse_batch` are the public batch APIs returning a typed `BatchResult<'arena>`. `parse_batch_into` reuses a caller-supplied `BinaryWriter` so per-call writer setup (`StringTableBuilder` prelude memcpy + arena buffer init) is amortized over the whole batch. `parse_batch` constructs a fresh writer per call.
- `parse_batch_to_bytes` is the public bytes API used by the JS bindings. It accepts all comments as one batch, emits each parsed block through `BinaryWriter`, and returns one owned Binary AST byte buffer with shared string data. This is the full Rust-side pipeline behind `parseBatch`, before Node / NAPI / WASM overhead.
- `parse_into` and `parse` are the public per-comment APIs returning `ParseResult<'arena>`. `parse_into` reuses a caller-supplied `BinaryWriter` so the writer setup cost is amortized across the loop. `parse` constructs a fresh writer per call.
- `parse_to_bytes` is the per-comment bytes-only sibling of `parse_into` / `parse`; it allocates a fresh writer and returns the owned `Vec<u8>`.

| Parser | Scenario | Estimate | Per comment | vs `oxc_jsdoc` |
| --- | --- | --: | --: | --: |
| `ox_jsdoc` | `parse_block_into_data`, phase 1 block parse only, loop | 78.081 us | 345 ns | 2.76x faster |
| `ox_jsdoc_origin` | typed AST `parse_comment`, loop, shared arena | 121.21 us | 536 ns | 1.78x faster |
| `oxc_jsdoc` | `JSDoc::new(inner, span).tags()`, loop lazy parse | 215.21 us | 952 ns | 1.00x (baseline) |
| `ox_jsdoc` | `parse_batch_to_bytes`, single batch full pipeline | 199.93 us | 885 ns | 1.08x faster |
| `ox_jsdoc` | `parse_batch`, single batch, shared arena | 221.10 us | 978 ns | 1.03x slower |
| `ox_jsdoc` | `parse_batch_into`, single batch, shared writer | 221.11 us | 978 ns | 1.03x slower |
| `ox_jsdoc` | `parse_into`, public single-comment API, loop, shared writer | 269.65 us | 1.193 us | 1.25x slower |
| `ox_jsdoc` | `parse`, public single-comment API, loop, shared arena | 298.61 us | 1.321 us | 1.39x slower |
| `ox_jsdoc` | `parse_to_bytes`, public single-comment bytes API, loop | 303.54 us | 1.343 us | 1.41x slower |

## Development

This repository uses Vite+ as the task runner. Install `vp` before running the project tasks:

```sh
curl -fsSL https://vite.plus | bash
```

Common commands:

```sh
vpr build     # or `vp run build`, build for JavaScript codes
vpr fmt       # or `vp run fmt`, format for Rust and JavaScript codes
vpr check     # or `vp run check`, lint for Rust and JavaScript codes
vpr test      # or `vp run test`, test for Rust and JavaScript codes
```

`vpr check` runs the Rust license-header task and `cargo check`. The header task checks Rust sources for:

- non-empty `@author`
- `@license MIT`

The first run builds the local `xtask` crate automatically through Cargo. You can also run the task directly:

```sh
cargo run -p xtask -- headers:check
```

Rust commands can be run directly as well:

```sh
cargo fmt --check
cargo check
cargo test
```

## Sponsors

The development of ox-jsdoc is supported by my OSS sponsors!

<p align="center">
  <a href="https://cdn.jsdelivr.net/gh/kazupon/sponsors/sponsors.svg">
    <img alt="sponsor" src="https://cdn.jsdelivr.net/gh/kazupon/sponsors/sponsors.svg">
  </a>
</p>

## License

[MIT](http://opensource.org/licenses/MIT)
