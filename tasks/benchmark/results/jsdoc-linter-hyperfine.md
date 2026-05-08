# 2026-05-06 — JSDoc linter hyperfine ベンチマーク

`design/009-jsdoc-linter-benchmark/README.md` に基づく end-to-end CLI 計測 (shell driver + hyperfine 直接実行、`oxc-project/bench-linter` 形式)。

**2 fixtures × 5 patterns × 1 rule set (combined) = 10 計測点**で、JS / TS の両方を実 multi-file project に対して計測。
`combined` rule set = `jsdoc/empty-tags` + `jsdoc/require-param-description` + `jsdoc/require-param-type` (実用 lint 一式の代表値)。

## Fixtures

| Fixture | Path | Files | Lines | JSDoc blocks | `@param` (with type / desc) |
|---|---|---:|---:|---:|---|
| `js` | `refers/eslint-plugin-jsdoc/src/` | 86 | 28,741 | 1,170 | 659 (639 / 31) |
| `ts` | `refers/vscode/src/` | 5,996 | 1,951,540 | 25,238 | 3471 (2 / 3411) |

- `js` fixture: `eslint-plugin-jsdoc` のソースを ESLint default parser (espree) で lint
- `ts` fixture: VS Code TS source を ESLint で lint する場合は `@typescript-eslint/parser` 必須 (Oxlint は TS native)

## Patterns

| # | Name | Linter | JSDoc parser / strategy |
|---|---|---|---|
| 1 | `eslint-jsdoc-upstream` | ESLint | upstream `eslint-plugin-jsdoc` (`@es-joy/jsdoccomment`) |
| 2 | `oxlint-jsdoc-native` | Oxlint | built-in JSDoc plugin (Rust) |
| 3 | `eslint-ox-jsdoc-single` | ESLint | `@ox-jsdoc/eslint-plugin-jsdoc` + `oxParseStrategy: 'single'` |
| 4 | `eslint-ox-jsdoc-batch` | ESLint | `@ox-jsdoc/eslint-plugin-jsdoc` + `oxParseStrategy: 'batch'` |
| 5 | `oxlint-ox-jsdoc-batch` | Oxlint (JS plugin bridge, alias `jsdoc-js`) | `@ox-jsdoc/eslint-plugin-jsdoc` + `oxParseStrategy: 'batch'` |

## Fixture: `js` — refers/eslint-plugin-jsdoc/src

| # | Name | Mean | Median | p95 | Stddev | vs baseline |
|---|---|---:|---:|---:|---:|---:|
| 1 | `eslint-jsdoc-upstream` | 522.7 ms | 522.9 ms | 537.9 ms | 12.1 ms | 1.00x |
| 2 | `oxlint-jsdoc-native` | 81.8 ms | 81.0 ms | 84.9 ms | 2.3 ms | 0.16x |
| 3 | `eslint-ox-jsdoc-single` | 592.1 ms | 588.8 ms | 614.2 ms | 12.9 ms | 1.13x |
| 4 | `eslint-ox-jsdoc-batch` | 515.9 ms | 514.5 ms | 530.6 ms | 9.5 ms | 0.99x |
| 5 | `oxlint-ox-jsdoc-batch` | 420.9 ms | 417.9 ms | 440.6 ms | 11.9 ms | 0.81x |

## Fixture: `ts` — refers/vscode/src

| # | Name | Mean | Median | p95 | Stddev | vs baseline |
|---|---|---:|---:|---:|---:|---:|
| 1 | `eslint-jsdoc-upstream` | 42.163 s | 43.448 s | 44.066 s | 2.327 s | 1.00x |
| 2 | `oxlint-jsdoc-native` | 287.9 ms | 288.4 ms | 304.8 ms | 11.4 ms | 0.01x |
| 3 | `eslint-ox-jsdoc-single` | 42.171 s | 43.032 s | 44.164 s | 2.023 s | 1.00x |
| 4 | `eslint-ox-jsdoc-batch` | 40.942 s | 39.094 s | 46.499 s | 3.382 s | 0.97x |
| 5 | `oxlint-ox-jsdoc-batch` | 8.076 s | 8.078 s | 8.330 s | 182.2 ms | 0.19x |

## Cross-fixture summary

| # | Pattern | js mean | ts mean | ts/js ratio |
|---|---|---:|---:|---:|
| 1 | `eslint-jsdoc-upstream` | 522.7 ms | 42.163 s | 80.66x |
| 2 | `oxlint-jsdoc-native` | 81.8 ms | 287.9 ms | 3.52x |
| 3 | `eslint-ox-jsdoc-single` | 592.1 ms | 42.171 s | 71.22x |
| 4 | `eslint-ox-jsdoc-batch` | 515.9 ms | 40.942 s | 79.36x |
| 5 | `oxlint-ox-jsdoc-batch` | 420.9 ms | 8.076 s | 19.19x |

## Parser-only auxiliary measurements

Command:

```sh
pnpm --filter @ox-jsdoc/benchmark benchmark:parse-batch
```

In-process Node.js measurement using `node --expose-gc` and
`tasks/benchmark/scripts/lib/measure.mjs` defaults. Fixture is
`fixtures/perf/source/typescript-checker.ts` with 226 JSDoc comments.
`Batch 100` uses the first 100 comments (25,333 cumulative bytes).

The `vs parseBatch` column uses `ox-jsdoc-binary NAPI (parseBatch)` as the
reference lower-bound row.

### Batch 100

| Parser | Total (spread) | Per comment | vs parseBatch |
|---|---:|---:|---:|
| `ox-jsdoc-binary NAPI (parseBatch)` | 118.792 µs (±0.7%) | 1.188 µs | 1.00x |
| `ox-jsdoc-binary WASM (parseBatch)` | 175.271 µs (±0.6%) | 1.753 µs | 1.48x |
| `comment-parser (loop)` | 207.583 µs (±1.7%) | 2.076 µs | 1.75x |
| `ox-jsdoc-binary WASM (loop)` | 307.625 µs (±1.5%) | 3.076 µs | 2.59x |
| `jsdoccomment (loop)` | 320.041 µs (±0.8%) | 3.200 µs | 2.69x |
| `ox-jsdoc-binary NAPI (loop)` | 375.729 µs (±2.7%) | 3.757 µs | 3.16x |
| `@ox-jsdoc/jsdoccomment (parseCommentBatch)` | 427.875 µs (±1.7%) | 4.279 µs | 3.60x |
| `ox-jsdoc typed NAPI (loop)` | 657.000 µs (±0.5%) | 6.570 µs | 5.53x |
| `@ox-jsdoc/jsdoccomment (parseComment loop)` | 1.655 ms (±3.1%) | 16.549 µs | 13.93x |

### Full file (226 comments)

| Parser | Total (spread) | Per comment | vs parseBatch |
|---|---:|---:|---:|
| `ox-jsdoc-binary NAPI (parseBatch)` | 293.625 µs (±3.0%) | 1.299 µs | 1.00x |
| `ox-jsdoc-binary WASM (parseBatch)` | 443.125 µs (±0.7%) | 1.961 µs | 1.51x |
| `comment-parser (loop)` | 545.188 µs (±1.8%) | 2.412 µs | 1.86x |
| `ox-jsdoc-binary WASM (loop)` | 758.667 µs (±0.8%) | 3.357 µs | 2.58x |
| `jsdoccomment (loop)` | 836.604 µs (±0.1%) | 3.702 µs | 2.85x |
| `ox-jsdoc-binary NAPI (loop)` | 903.917 µs (±1.7%) | 4.000 µs | 3.08x |
| `@ox-jsdoc/jsdoccomment (parseCommentBatch)` | 1.115 ms (±0.8%) | 4.932 µs | 3.80x |
| `ox-jsdoc typed NAPI (loop)` | 1.667 ms (±0.6%) | 7.376 µs | 5.68x |
| `@ox-jsdoc/jsdoccomment (parseComment loop)` | 4.287 ms (±1.0%) | 18.969 µs | 14.60x |

### String dedup effect (50x identical comments)

| Mode | Bytes | Per item |
|---|---:|---:|
| `50x parse()` | 41,400 | 828 |
| `1x parseBatch x50` | 20,084 | 401.7 |
| Reduction | 51.5% smaller | |

### Rust-direct parser references (criterion)

These rows use criterion in Rust, not Node / mitata. They are included to
compare `oxc_jsdoc`, which has no Node binding in this workspace. All rows use
the same `typescript-checker.ts` fixture with 226 JSDoc comments.

Commands:

```sh
cargo bench --bench oxc_jsdoc -p ox_jsdoc_benchmark -- "source/typescript-checker"
cargo bench --bench parser -p ox_jsdoc_benchmark -- "source/typescript-checker"
cargo bench -p ox_jsdoc_binary --bench parser -- parse_batch_to_bytes
cargo bench -p ox_jsdoc_binary --bench parser -- "phase 1"
```

| Parser | Scenario | Estimate | Per comment | vs `oxc_jsdoc` |
|---|---|---:|---:|---:|
| `ox-jsdoc-binary` | `parse_block_into_data` phase 1 only, loop | 72.942 µs | 323 ns | 0.36x |
| `ox_jsdoc` | typed AST `parse_comment`, loop | 117.54 µs | 520 ns | 0.58x |
| `ox-jsdoc-binary` | `parse_batch_to_bytes`, single batch full pipeline | 175.72 µs | 777 ns | 0.86x |
| `oxc_jsdoc` | `JSDoc::new(inner, span).tags()`, loop lazy parse | 203.89 µs | 902 ns | 1.00x |

`oxc_jsdoc` bench details: strips the comment delimiters before constructing
`JSDoc`, creates a `Span` for the original comment length, and calls `.tags()`
so the lazy parse is included.
