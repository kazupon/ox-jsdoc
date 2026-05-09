# JSDoc Linter Benchmark Design

## Purpose

Measure the performance impact of using `@ox-jsdoc/jsdoccomment` for JSDoc linting. In particular, compare calling `parseComment` per comment against calling `parseCommentBatch` once per ESLint `SourceCode` / indent pair.

The current `batch` path is the tuned path used by `@ox-jsdoc/eslint-plugin-jsdoc`: it collects all JSDoc comments from `SourceCode#getAllComments()`, caches the parsed blocks per source file, and uses the `ox-jsdoc-binary` batch fast path that can return a lightweight `jsdoccomment` input shape without materializing duplicate per-tag JSON.

This benchmark does not aim to re-prove the general linter performance gap between ESLint and Oxlint. Oxlint's linter-level performance advantage is treated as already established by official and community measurements.

## Fixtures

We measure **two fixtures**: JS and TS. Whether the lint target is JavaScript or TypeScript changes the presence of `@typescript-eslint/parser`, which significantly shifts ESLint's wall-clock composition. Measuring both covers a wider range of real-world use cases.

| Fixture | Path | Files | Lines | JSDoc blocks | Characteristics |
| --- | --- | --: | --: | --: | --- |
| `js` | `refers/eslint-plugin-jsdoc/src/` | 86 | 28,741 | 1,170 | JSDoc-as-types style (heavy `@param {Type}`), 5% have descriptions |
| `ts` | `refers/vscode/src/` | 5,996 | 1,951,540 | 25,238 | Uses TS types so no JSDoc types, 98% have descriptions |

Rationale:

- **JS fixture (`refers/eslint-plugin-jsdoc/src`)**: A mid-sized project of 86 files. Uses the JSDoc-as-types style where `@param {Type}` is always attached. Being the source of `eslint-plugin-jsdoc` itself, it is also empirically well-suited to its lint rules.
- **TS fixture (`refers/vscode/src`)**: Roughly 6,000 TS files of the entire VS Code codebase. Uses real TypeScript types so JSDoc types are omitted and only descriptions are present. Reflects the lint experience of a real-world large TS project.

The two fixtures have symmetric characteristics (the JS side is typed, the TS side is description-heavy), so together they cover the variety of lint rule behaviour.

Before interpreting results, `setup.mjs` writes the following fixture stats to `tasks/benchmark/.tmp/jsdoc-linter/fixture-stats.json`:

- File count / total line count
- Total JSDoc block count
- Total `@param` tag count
- `@param` tag count with / without description
- `@param` tag count with / without type
- Empty tag count (no name / type / description)

## Target Rules

The first benchmark set enables the following 3 rules **simultaneously** as a single `combined` measurement point. A per-rule breakdown is not measured (parser cost is not the dominant factor in wall-clock time, so per-rule differences are buried in noise).

| Rule                              | Purpose                                                     |
| --------------------------------- | ----------------------------------------------------------- |
| `jsdoc/empty-tags`                | Lightweight JSDoc block scan + tag scan                     |
| `jsdoc/require-param-description` | Function ↔ JSDoc correspondence and param description shape |
| `jsdoc/require-param-type`        | Function ↔ JSDoc correspondence and param type shape        |

`jsdoc/check-tag-names` is avoided in this benchmark. It is closely tied to tag aliasing and `settings.jsdoc.tagNamePreference`, which we do not want as a primary independent variable in this measurement.

`jsdoc/no-defaults` is also excluded from the first benchmark set. It is useful for verifying optional/default name parsing, but for this round we focus on the combination of the 3 rules above.

## Measurement Patterns Including the Linter

The following 5 patterns are measured.

| # | Pattern | Purpose |
| --: | --- | --- |
| 1 | `eslint-jsdoc-upstream` | `eslint` + `eslint-plugin-jsdoc` (`@es-joy/jsdoccomment`). **Baseline** |
| 2 | `oxlint-jsdoc-native` | `oxlint` + built-in JSDoc plugin enabled by config. Comparison value for the Oxlint native (Rust) implementation |
| 3 | `eslint-ox-jsdoc-single` | `eslint` + `@ox-jsdoc/eslint-plugin-jsdoc` + `oxParseStrategy: 'single'`. Effect of swapping the parser without batching |
| 4 | `eslint-ox-jsdoc-batch` | `eslint` + `@ox-jsdoc/eslint-plugin-jsdoc` + `oxParseStrategy: 'batch'`. Effect of using batch parse on top of ESLint |
| 5 | `oxlint-ox-jsdoc-batch` | `oxlint` + JS plugin alias `jsdoc-js` + `@ox-jsdoc/eslint-plugin-jsdoc` + `oxParseStrategy: 'batch'`. Practical comparison of Oxlint runtime + JS plugin bridge + batch parser |

The `batch` variants are toggled via `settings.jsdoc.oxParseStrategy` on the fork side. `eslint-plugin-jsdoc` does not call `parseCommentBatch` as shipped, so result names explicitly include the `oxParseStrategy` value to avoid confusion with `eslint-plugin-jsdoc` behaviour.

**2 fixtures × 5 patterns × 1 rule set (combined) = 10 measurement points**.

### Cost We Want to Isolate per Pair

The numbers below refer to the `#` column in the measurement pattern table above. For example, `1 vs. 3` compares `eslint-jsdoc-upstream` against `eslint-ox-jsdoc-single`.

- **1 vs. 3**: Parser swap effect alone (both ESLint + per-comment, `@es-joy` → `@ox-jsdoc/jsdoccomment`)
- **3 vs. 4**: Batch parse amortization plus the current batch fast path (both ESLint + `@ox-jsdoc`)
- **1 vs. 4**: Combined effect of fully adopting `@ox-jsdoc` on ESLint
- **4 vs. 5**: **Linter runtime difference** under the same JS plugin / parser strategy (ESLint vs Oxlint + JS plugin bridge cost)
- **2 vs. 5**: Oxlint native (Rust) vs Oxlint + JS plugin bridge + batch (practical comparison)

## How to Run the Linter Benchmark

The linter measurements **invoke `hyperfine` directly from a shell script** rather than going through the mitata scripts in `tasks/benchmark` (the same shape as `oxc-project/bench-linter`).

Reasons:

- Like `bench-linter`, this measures end-to-end wall-clock time including actual CLI startup, configuration loading, file reading, and linter execution.
- It enables comparison that includes the runtime difference between ESLint and Oxlint, the cost of the Oxlint JS plugin bridge, and the cost of configuration loading.
- `mitata` is well-suited to parser-only or in-process micro benchmarks, but `hyperfine` is easier to handle for linter comparisons that involve a CLI subprocess.
- Calling hyperfine directly from a shell driver avoids wrapper overhead such as Node.js `spawnSync` (minimising the per-command startup path).

### Pipeline Composition

Split into 3 stages:

| Script | Role |
| --- | --- |
| `tasks/benchmark/scripts/jsdoc-linter-setup.mjs` | Auto-generates configs (2 × 5 × 1 = 10) + computes fixture stats → outputs to `.tmp/jsdoc-linter/` |
| `tasks/benchmark/scripts/jsdoc-linter-hyperfine.sh` | Shell script that calls hyperfine directly (per-fixture × per-rule-set, outputs `.json` / `.md`) |
| `tasks/benchmark/scripts/jsdoc-linter-report.mjs` | Aggregates `.json` files and generates `tasks/benchmark/results/jsdoc-linter-hyperfine.md` |

Run:

```sh
node tasks/benchmark/scripts/jsdoc-linter-setup.mjs && \
  bash tasks/benchmark/scripts/jsdoc-linter-hyperfine.sh && \
  node tasks/benchmark/scripts/jsdoc-linter-report.mjs
```

Or `pnpm --filter @ox-jsdoc/benchmark benchmark:jsdoc-linter`.

### Hyperfine Options

```sh
hyperfine \
  --warmup 1 \
  --runs 10 \
  --ignore-failure \
  --export-markdown <results>.md \
  --export-json <results>.json \
  --command-name <pattern> "cd <fixture> && <linter cmd>" \
  ...
```

- **`--ignore-failure`**: The target rules emit lint diagnostics against the fixtures, so the exit code is non-zero. Since we only compare execution time, ignore it.
- **`--warmup 1`**: Reduces first-run variance from CLI startup, JIT, and config loading.
- **`--runs 10`**: Suppresses stddev in results (with 5 runs, some patterns showed stddev/mean > 10%).
- **The `cd <fixture> && <cmd>` wrap is required**: hyperfine has no per-command cwd flag, so a shell wrap is needed to evaluate `.` under the fixture. Skipping it makes hyperfine interpret `.` from its own cwd (= the invocation directory), causing the wrong tree to be linted.

### CLI Command Matrix

Each command targets the fixture directory.

| Pattern | Command policy |
| --- | --- |
| `eslint-jsdoc-upstream` | `eslint` + `eslint-plugin-jsdoc` |
| `eslint-ox-jsdoc-single` | `eslint` + `@ox-jsdoc/eslint-plugin-jsdoc` + `settings.jsdoc.oxParseStrategy: "single"` |
| `eslint-ox-jsdoc-batch` | `eslint` + `@ox-jsdoc/eslint-plugin-jsdoc` + `settings.jsdoc.oxParseStrategy: "batch"` |
| `oxlint-jsdoc-native` | `oxlint` + config `plugins: ["jsdoc"]` + built-in JSDoc rules |
| `oxlint-ox-jsdoc-batch` | `oxlint` + JS plugin alias `jsdoc-js` + `@ox-jsdoc/eslint-plugin-jsdoc` + `oxParseStrategy: "batch"` |

Because `jsdoc` is a reserved plugin name in Oxlint, when loading `@ox-jsdoc/eslint-plugin-jsdoc` as a JS plugin we use the `jsdoc-js` alias. Rule ids therefore look like `jsdoc-js/empty-tags`.

### ESLint / Oxlint Configuration to Suppress Measurement Noise

For a fair comparison, **both linters run only the specified rules**:

#### ESLint side

```js
{
  files: ['**/*.{js,ts}'],
  plugins: { jsdoc },
  // The TypeScript fixture must specify a parser (espree cannot read TS syntax)
  languageOptions: { parser: tsParser },  // ts fixture only
  // Eliminate noise from `// eslint-disable-*` directives in the source
  // (a flood of disable comments for undefined rules adds time during
  //  inline-config processing and hides the pure cost of JSDoc rules)
  linterOptions: {
    noInlineConfig: true,
    reportUnusedDisableDirectives: 'off'
  },
  settings: { jsdoc: { oxParseStrategy: 'batch' } },
  rules: { 'jsdoc/empty-tags': 'error', /* ... */ }
}
```

The ESLint CLI command also passes `--no-config-lookup --no-warn-ignored` to avoid fixture-local config discovery and ignored-file warning noise.

#### Oxlint side

```json
{
  "categories": {
    "correctness": "off",
    "nursery": "off",
    "pedantic": "off",
    "perf": "off",
    "restriction": "off",
    "style": "off",
    "suspicious": "off"
  },
  "rules": { "jsdoc/empty-tags": "error" }
}
```

Additionally, pass `--disable-nested-config --disable-unicorn-plugin --disable-oxc-plugin --disable-typescript-plugin` on the CLI. `--disable-nested-config` prevents fixture-local config discovery, and the plugin flags prevent Oxlint's default plugin rules from being enabled.

### Config Generation

Hand-managing multiple configs makes configuration drift likely, so `setup.mjs` generates them under `tasks/benchmark/.tmp/jsdoc-linter/<fixture>/<pattern>/<rule-set>/` (under `.gitignore`, not committed).

## Benchmark eslint-plugin-jsdoc Fork

To validate `@ox-jsdoc/jsdoccomment`'s `parseComment` compatibility on ESLint, `refers/eslint-plugin-jsdoc` was copied as the following workspace package.

| Package | Purpose |
| --- | --- |
| `packages/eslint-plugin-jsdoc` | Fork that swaps `@es-joy/jsdoccomment` imports for `@ox-jsdoc/jsdoccomment` and toggles single/batch via `oxParseStrategy` |

The package name is `@ox-jsdoc/eslint-plugin-jsdoc`. In flat config it is imported explicitly and assigned to the same `jsdoc` plugin key as the regular `eslint-plugin-jsdoc`.

```js
import jsdoc from '@ox-jsdoc/eslint-plugin-jsdoc'

export default [
  {
    plugins: { jsdoc },
    rules: {
      'jsdoc/empty-tags': 'error',
      'jsdoc/require-param-description': 'error',
      'jsdoc/require-param-type': 'error'
    },
    settings: { jsdoc: { oxParseStrategy: 'batch' } }
  }
]
```

### Batch Strategy and Fast Path

`oxParseStrategy: 'batch'` is not only a native parser batching switch. The fork caches the `parseCommentBatch` result in a `WeakMap` keyed by ESLint `SourceCode`, with a nested key for `indent`. With the combined rule set, this means all enabled rules reuse the same parsed JSDoc blocks for a file instead of paying parser and normalisation cost once per rule.

The underlying `ox-jsdoc-binary.parseBatch` path has also been tuned for this usage:

- Multiple comment sources are encoded into one UTF-8 buffer plus offset / base-offset typed arrays, reducing per-item NAPI marshalling overhead.
- Reused typed-array pools reduce allocation churn for repeated batch calls.
- `output: 'jsdoccomment-input'` lets `@ox-jsdoc/jsdoccomment` normalise from block-level `source[]` data without eagerly materializing duplicate per-tag `source[]` and child JSON.

Because of this, `eslint-ox-jsdoc-batch` should be interpreted as the deployed batch strategy effect, not as a raw parser-only `parseBatch` measurement.

## Parser-only Auxiliary Measurements

Parser-only measurements are kept on a separate chart from the linter measurements. Existing scripts under `tasks/benchmark` are reused here.

| Pattern | Layer | Purpose |
| --- | --- | --- |
| `@es-joy/jsdoccomment parseComment loop` | JS | Current JS parser baseline |
| `@ox-jsdoc/jsdoccomment parseComment loop` | NAPI / WASM | Cost of binary-backed single-comment parse |
| `@ox-jsdoc/jsdoccomment parseCommentBatch` | NAPI / WASM | Cost of batch parse + tuned `jsdoccomment` input normalisation |
| `ox-jsdoc-binary parseBatch` | NAPI / WASM | Lower bound without jsdoccomment shape normalisation |
| `ox_jsdoc_binary::parse` (loop) / `parse_into` (loop) | Rust-direct | ox-jsdoc-binary per-comment entry points without NAPI/WASM crossing — isolates writer-reuse savings |
| `ox_jsdoc_binary::parse_batch` / `parse_batch_into` | Rust-direct | ox-jsdoc-binary batch entry points without NAPI/WASM crossing — Rust-direct lower bound for the binary AST emit path |
| `ox_jsdoc_binary::parse_block_into_data` (phase 1 only) | Rust-direct | Structural parse only (no binary emission) — theoretical lower bound for the structural pass |
| `ox_jsdoc::parse_comment` (typed AST loop) | Rust-direct | Typed AST entry point — pays no binary serialization cost; the gap against `parse_into` is the binary-emit overhead |
| `oxc_jsdoc JSDoc::new(...).tags()` | Rust-direct | Rust-direct reference for the parser used by Oxlint's native JSDoc |

The Rust-direct rows are mandatory: omitting them hides the writer-reuse / batch-amortization / binary-emit costs and makes the NAPI/WASM "crossing" overhead unattributable. Always populate these rows together with the JS / NAPI / WASM rows so the per-comment numbers in the cross-comparison table are decomposable.

These measurements isolate the following costs:

- Cost of the binary parser
- Cost of normalising into a `comment-parser` compatible shape
- Amortization effect of batching
- Cost of linter integration

Mapping to existing scripts:

| Script | Purpose |
| --- | --- |
| `tasks/benchmark/scripts/parse-batch-vs-loop.mjs` | Foundation for comment extraction from `typescript-checker.ts` and `parseBatch` vs loop |
| `tasks/benchmark/scripts/parsers-comparison.mjs` | Reference values for raw parser comparison |
| `tasks/benchmark/scripts/lib/measure.mjs` | Robust aggregation helper for parser-only |
| `crates/ox_jsdoc_binary/benches/parser.rs` | Rust criterion bench for ox-jsdoc-binary entry points (`parse` / `parse_into` / `parse_batch` / `parse_batch_into`) + internal phase breakdown + same-run `oxc_jsdoc` reference |
| `tasks/benchmark/benches/parser.rs` | Rust criterion bench for the typed AST entry point `ox_jsdoc::parse_comment` |
| `tasks/benchmark/benches/oxc_jsdoc.rs` | Standalone Rust criterion reference for `oxc_jsdoc` |

The linter benchmark uses `hyperfine` (shell driver), the parser-only benchmark uses `mitata` / `measure.mjs` — the roles are split.

### Parser-only Measurement Method

Parser-only scripts are in-process Node.js measurements. They should be run through the package scripts, which use `node --expose-gc`, for example:

```sh
pnpm --filter @ox-jsdoc/benchmark benchmark:parse-batch
```

`tasks/benchmark/scripts/lib/measure.mjs` wraps `mitata.measure()` with a multi-round aggregation layer. Unless a script passes explicit options, the defaults are:

| Setting | Default | Meaning |
| --- | --: | --- |
| `rounds` | 10 | Number of outer measurements per benchmark case |
| `discardFirst` | `true` | Drop the first round as cold-start / JIT / inline-cache warmup |
| `trim` | 1 | After discarding the first round, drop the fastest and slowest usable round |
| `minSamples` | 15 | `mitata.measure()` minimum sample count per round |
| `minCpuTimeMs` | 800 ms | `mitata.measure()` minimum CPU time per round |
| `warmupSamples` | 5 | `mitata.measure()` warmup sample count per round |
| `gc` | `true` | Ask mitata to call `globalThis.gc` when available |

The reported `p50` is therefore not a single `mitata` run. Each round records `stats.p50`; the first round is discarded, the remaining round p50 values are sorted, the fastest and slowest are trimmed, and the mean of the remaining round p50 values is reported as `p50`. With the defaults this means 10 raw rounds → 9 usable rounds → trim best/worst → average the middle 7 round p50s.

`p50_min`, `p50_max`, and `spread_pct` are computed from the usable rounds after the first-round discard. `spread_pct` is the best-to-worst round spread relative to the reported `p50`; it is a stability signal, not a confidence interval or hyperfine-style standard deviation.

The workload setup is outside the timed closure. For `parse-batch-vs-loop.mjs`, comments are extracted once from `fixtures/perf/source/typescript-checker.ts`, the first 100 comments and the full-file comment list are prepared up front, and `parseBatch` item arrays are pre-built before measurement. One measured sample is the whole scenario (for example "parse these 100 comments" or "parse the full file"), and the per-comment value printed by the script is derived after measurement.

### Rust-direct Measurement Method

The Rust-direct rows above (ox-jsdoc-binary entry points, ox-jsdoc typed AST, oxc_jsdoc) are taken via `cargo bench` (criterion). They share the same fixture as the `mitata` runs (`fixtures/perf/source/typescript-checker.ts`, 226 JSDoc comments) so the per-comment numbers are directly comparable to the NAPI / WASM rows.

Run all three:

```sh
# 1. ox-jsdoc-binary entry points + internal phase breakdown + same-run oxc_jsdoc reference
cargo bench --bench parser -p ox_jsdoc_binary

# 2. ox-jsdoc typed AST (ox_jsdoc::parse_comment)
cargo bench --bench parser -p ox_jsdoc_benchmark -- "source/typescript-checker"

# 3. Standalone oxc_jsdoc reference (kept for cross-checking against bench (1))
cargo bench --bench oxc_jsdoc -p ox_jsdoc_benchmark -- "source/typescript-checker"
```

What each bench covers:

- `crates/ox_jsdoc_binary/benches/parser.rs` exercises the public Rust entry points (`parse` / `parse_into` / `parse_batch` / `parse_batch_into` / `parse_to_bytes` / `parse_batch_to_bytes`) plus the internal phase breakdown (`parse_block_into_data` only, `parse + emit` no finish) and includes a same-run `oxc_jsdoc` row so machine-state drift between (1) and (3) can be spotted.
- `tasks/benchmark/benches/parser.rs` exercises `ox_jsdoc::parse_comment` against every fixture under `fixtures/perf/`. Filter with `-- "source/typescript-checker"` to keep the per-comment numbers comparable to the `mitata` table.
- `tasks/benchmark/benches/oxc_jsdoc.rs` is the standalone reference: it strips `/**` / `*/`, constructs `JSDoc::new(inner, span)`, and calls `.tags()` so the lazy parse is included. Keep it as an independent run so a regression in the binary AST bench harness does not silently move the reference value.

When reporting Rust-direct numbers, divide criterion's reported time by the comment count (226 for `typescript-checker.ts`) to get the per-comment value, and place those rows in the same cross-comparison table as the NAPI / WASM rows so the binding overhead is decomposable: NAPI batch − Rust-direct `parse_batch_into` ≈ NAPI marshalling + decoder shape cost; `parse_into` − typed AST `parse_comment` ≈ binary emit cost; `parse` − `parse_into` ≈ writer construction cost.

## Notes on Result Interpretation

- Treat the single combined rule set as a representative value for "a practical lint suite". A per-rule breakdown carries little meaning unless parser cost dominates wall-clock.
- Treat the `batch` variants as measuring the tuned integration path: native batch parse, per-`SourceCode` cache reuse, and JS-side normalisation fast path together.
- Compare linter results across the **10 cells of fixture × pattern**.
- For linter results, report median, p95, stddev, and the relative speedup against the `eslint + eslint-plugin-jsdoc (@es-joy/jsdoccomment)` baseline.
- For parser-only results, report the `measure.mjs` trimmed `p50`, per-comment derived value, relative speedup, and `spread_pct`.
- For Rust-direct parser references such as `oxc_jsdoc`, report criterion's estimate and keep the table separate from Node / NAPI / WASM measurements.
- If **stddev** of any linter measurement exceeds 10% of the mean, or `spread_pct` of any parser-only measurement is too large to support the conclusion, **increase the number of runs or lower the system load and re-measure**.
- **Do not include observations / interpretation in the report generator script**. Append them manually to the report file (`tasks/benchmark/results/...`) or write them up in a separate doc.
