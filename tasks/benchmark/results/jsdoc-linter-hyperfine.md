# 2026-05-09 — JSDoc linter hyperfine benchmark

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
| 1 | `eslint-jsdoc-upstream` | 684.6 ms | 684.1 ms | 701.3 ms | 10.4 ms | 1.00x |
| 2 | `oxlint-jsdoc-native` | 223.4 ms | 221.1 ms | 235.7 ms | 7.2 ms | 0.33x |
| 3 | `eslint-ox-jsdoc-single` | 765.6 ms | 771.0 ms | 790.8 ms | 18.9 ms | 1.12x |
| 4 | `eslint-ox-jsdoc-batch` | 685.2 ms | 677.9 ms | 710.3 ms | 19.2 ms | 1.00x |
| 5 | `oxlint-ox-jsdoc-batch` | 577.8 ms | 573.7 ms | 597.8 ms | 11.7 ms | 0.84x |

## Fixture: `ts` — refers/vscode/src

| # | Name | Mean | Median | p95 | Stddev | vs baseline |
|---|---|---:|---:|---:|---:|---:|
| 1 | `eslint-jsdoc-upstream` | 38.890 s | 39.167 s | 40.149 s | 1.131 s | 1.00x |
| 2 | `oxlint-jsdoc-native` | 389.0 ms | 387.7 ms | 400.7 ms | 7.6 ms | 0.01x |
| 3 | `eslint-ox-jsdoc-single` | 39.912 s | 39.902 s | 40.349 s | 294.9 ms | 1.03x |
| 4 | `eslint-ox-jsdoc-batch` | 38.514 s | 38.396 s | 40.000 s | 1.100 s | 0.99x |
| 5 | `oxlint-ox-jsdoc-batch` | 7.695 s | 7.707 s | 7.848 s | 140.7 ms | 0.20x |

## Cross-fixture summary

| # | Pattern | js mean | ts mean | ts/js ratio |
|---|---|---:|---:|---:|
| 1 | `eslint-jsdoc-upstream` | 684.6 ms | 38.890 s | 56.80x |
| 2 | `oxlint-jsdoc-native` | 223.4 ms | 389.0 ms | 1.74x |
| 3 | `eslint-ox-jsdoc-single` | 765.6 ms | 39.912 s | 52.13x |
| 4 | `eslint-ox-jsdoc-batch` | 685.2 ms | 38.514 s | 56.21x |
| 5 | `oxlint-ox-jsdoc-batch` | 577.8 ms | 7.695 s | 13.32x |

