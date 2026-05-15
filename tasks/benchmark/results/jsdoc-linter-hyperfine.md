# 2026-05-15 — JSDoc linter hyperfine benchmark

End-to-end CLI measurement (shell driver + direct hyperfine, `oxc-project/bench-linter` style) based on `design/009-jsdoc-linter-benchmark/README.md`.

**2 fixtures × 5 patterns × 1 rule set (combined) = 10 measurement points**, covering both JS and TS against real multi-file projects.
`combined` rule set = `jsdoc/empty-tags` + `jsdoc/require-param-description` + `jsdoc/require-param-type` (representative of a practical lint set).

## Fixtures

| Fixture | Path | Files | Lines | JSDoc blocks | `@param` (with type / desc) |
|---|---|---:|---:|---:|---|
| `js` | `refers/eslint-plugin-jsdoc/src/` | 86 | 28,741 | 1,170 | 659 (639 / 31) |
| `ts` | `refers/vscode/src/` | 5,996 | 1,951,540 | 25,238 | 3471 (2 / 3411) |

- `js` fixture: lint the `eslint-plugin-jsdoc` source with the ESLint default parser (espree)
- `ts` fixture: linting VS Code's TS source with ESLint requires `@typescript-eslint/parser` (Oxlint is TS-native)

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
| 1 | `eslint-jsdoc-upstream` | 721.6 ms | 698.0 ms | 841.8 ms | 79.2 ms | 1.00x |
| 2 | `oxlint-jsdoc-native` | 231.8 ms | 228.0 ms | 253.2 ms | 11.5 ms | 0.32x |
| 3 | `eslint-ox-jsdoc-single` | 797.6 ms | 802.1 ms | 833.2 ms | 30.1 ms | 1.11x |
| 4 | `eslint-ox-jsdoc-batch` | 701.3 ms | 702.4 ms | 729.8 ms | 22.0 ms | 0.97x |
| 5 | `oxlint-ox-jsdoc-batch` | 592.6 ms | 587.9 ms | 612.6 ms | 13.7 ms | 0.82x |

## Fixture: `ts` — refers/vscode/src

| # | Name | Mean | Median | p95 | Stddev | vs baseline |
|---|---|---:|---:|---:|---:|---:|
| 1 | `eslint-jsdoc-upstream` | 40.316 s | 40.437 s | 41.732 s | 998.0 ms | 1.00x |
| 2 | `oxlint-jsdoc-native` | 430.0 ms | 407.3 ms | 494.1 ms | 41.0 ms | 0.01x |
| 3 | `eslint-ox-jsdoc-single` | 41.018 s | 41.124 s | 41.446 s | 372.5 ms | 1.02x |
| 4 | `eslint-ox-jsdoc-batch` | 39.994 s | 40.070 s | 41.156 s | 995.4 ms | 0.99x |
| 5 | `oxlint-ox-jsdoc-batch` | 7.766 s | 7.736 s | 7.968 s | 136.0 ms | 0.19x |

## Cross-fixture summary

| # | Pattern | js mean | ts mean | ts/js ratio |
|---|---|---:|---:|---:|
| 1 | `eslint-jsdoc-upstream` | 721.6 ms | 40.316 s | 55.87x |
| 2 | `oxlint-jsdoc-native` | 231.8 ms | 430.0 ms | 1.86x |
| 3 | `eslint-ox-jsdoc-single` | 797.6 ms | 41.018 s | 51.43x |
| 4 | `eslint-ox-jsdoc-batch` | 701.3 ms | 39.994 s | 57.02x |
| 5 | `oxlint-ox-jsdoc-batch` | 592.6 ms | 7.766 s | 13.11x |

