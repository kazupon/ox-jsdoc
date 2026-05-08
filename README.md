# ox-jsdoc

High-performance JSDoc parser inspired by the `oxc` project.

## Motivation

- Parse JSDoc at native Rust speed when generating JSDoc documentation in `ox-content`, a potential alternative to TypeDoc.
- Speed up lint performance for `eslint-plugin-jsdoc` when it runs on Oxlint.

## Status

> [!WARNING] This project is still WIP, so don't use in production.

## Benchmark Results

Benchmarks were run on 2026-05-06. Treat these numbers as a snapshot for this workspace, not as a stable performance guarantee.

See [design/009-jsdoc-linter-benchmark/README.md](design/009-jsdoc-linter-benchmark/README.md) for the benchmark design, fixture selection, measurement methodology, and result interpretation notes.

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
- Parser-only Node.js timings use `mitata.measure()` via `tasks/benchmark/scripts/lib/measure.mjs`: 5 rounds, discard the first round, trim the fastest and slowest remaining rounds, then report the mean of the remaining round p50 values.
- Rust-direct parser references were run with `cargo bench` / Criterion and are reported separately from Node / NAPI / WASM timings.

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
| `eslint-jsdoc-upstream`  | 522.7 ms |                1.00x | 42.163 s |                1.00x |
| `oxlint-jsdoc-native`    |  81.8 ms |         6.39x faster | 287.9 ms |       146.47x faster |
| `eslint-ox-jsdoc-single` | 592.1 ms |                0.88x | 42.171 s |                1.00x |
| `eslint-ox-jsdoc-batch`  | 515.9 ms |         1.01x faster | 40.942 s |         1.03x faster |
| `oxlint-ox-jsdoc-batch`  | 420.9 ms |         1.24x faster |  8.076 s |         5.22x faster |

### Parser-only

In-process Node.js benchmark using the 226 JSDoc comments from `fixtures/perf/source/typescript-checker.ts`. These rows compare parser entry points and keep linter integration out of the timed path.

In the parser-only tables, `loop` means the benchmark calls the parser once per JSDoc comment in a JavaScript or Rust loop. It is the non-batch baseline used to show how much overhead `parseBatch` removes by parsing all comments in one call.

| Parser | Full file total | Per comment | vs binary NAPI parseBatch |
| --- | --: | --: | --: |
| `ox-jsdoc-binary NAPI (parseBatch)` | 293.625 us | 1.299 us | 1.00x |
| `ox-jsdoc-binary WASM (parseBatch)` | 443.125 us | 1.961 us | 1.51x slower |
| `comment-parser (loop)` | 545.188 us | 2.412 us | 1.86x slower |
| `jsdoccomment (loop)` | 836.604 us | 3.702 us | 2.85x slower |
| `@ox-jsdoc/jsdoccomment (parseCommentBatch)` | 1.115 ms | 4.932 us | 3.80x slower |
| `ox-jsdoc typed NAPI (loop)` | 1.667 ms | 7.376 us | 5.68x slower |
| `@ox-jsdoc/jsdoccomment (parseComment loop)` | 4.287 ms | 18.969 us | 14.60x slower |

Rust-direct parser references use Criterion, not Node / mitata. `oxc_jsdoc` is included as the public API used by Oxlint's native JSDoc support.

| Parser | Scenario | Estimate | Per comment | vs `oxc_jsdoc` |
| --- | --- | --: | --: | --: |
| `ox-jsdoc-binary` | `parse_block_into_data` phase 1 only, loop | 72.942 us | 323 ns | 0.36x |
| `ox_jsdoc` | typed AST `parse_comment`, loop | 117.54 us | 520 ns | 0.58x |
| `ox-jsdoc-binary` | `parse_batch_to_bytes`, single batch full pipeline | 175.72 us | 777 ns | 0.86x |
| `oxc_jsdoc` | `JSDoc::new(inner, span).tags()`, loop lazy parse | 203.89 us | 902 ns | 1.00x |

### Benchmark Takeaways

- `ox-jsdoc` is most useful when parsing can be batched. In the full-file parser-only benchmark, `ox-jsdoc-binary` NAPI `parseBatch` parses 226 comments in 293.625 us (1.299 us/comment), while `comment-parser` loop is 1.86x slower and `jsdoccomment` loop is 2.85x slower. The main win is avoiding per-comment JavaScript/native calls and repeated parser setup.
- For ESLint-only usage, replacing the parser does not produce a large end-to-end lint speedup by itself. These runs are still dominated by ESLint, config loading, AST traversal, and, for TypeScript, `@typescript-eslint/parser`. In this fixture set, `eslint-ox-jsdoc-batch` is only slightly faster than upstream ESLint (`1.01x` on JS, `1.03x` on TS).
- The larger linting benefit appears when the same JSDoc rules run on Oxlint. `oxlint-ox-jsdoc-batch` is 1.24x faster on JS and 5.22x faster on TS than upstream ESLint, even with the JS plugin bridge. Oxlint's built-in native JSDoc path is the fastest linter reference in this benchmark (`6.39x` on JS, `146.47x` on TS).
- For `ox-content` documentation generation, the parser-only numbers are the more relevant signal than the ESLint-only runs: a Rust-native batch parser can keep JSDoc parsing a small part of the total documentation pipeline when many comments are parsed from a file at once.
- The Rust-direct Criterion table is a reference, not a Node binding benchmark. It shows that the Rust parser core is competitive with the `oxc_jsdoc` public API used by Oxlint, while the Node / NAPI table captures the practical cost seen from JavaScript.

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
