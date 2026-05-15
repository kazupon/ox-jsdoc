# Binary AST Mainstream Migration Design

**Status:** Draft  
**Related designs:** [`../007-binary-ast/`](../007-binary-ast/), [`../002-project-structure/README.md`](../002-project-structure/README.md), [`../003-js-binding/README.md`](../003-js-binding/README.md), [`../004-wasm/README.md`](../004-wasm/README.md)

## Overview

The Binary AST path has met the performance bar required to become the main implementation of `ox-jsdoc`.

This document concretizes the Phase 1.3 cutover plan defined in [`007-binary-ast/phases.md`](../007-binary-ast/phases.md) as a repository-wide migration design.

1. Promote the Binary AST implementation to the canonical `ox-jsdoc` name
2. Move the original typed AST implementation aside into `origin`-suffixed packages reserved for benchmark / reference use
3. Reorganize public package names around their role rather than their implementation detail
4. Preserve historical design / benchmark assets while keeping them off the main path

The top-level repository split remains appropriate.

```text
crates/
napi/
wasm/
packages/
tasks/benchmark/
design/
```

What this migration changes is which implementation owns the canonical name within that layout.

## Background

The current repository reflects a coexistence phase between the typed AST and the Binary AST.

```text
crates/
  ox_jsdoc/                  # typed AST core
  ox_jsdoc_binary/           # Binary AST core

napi/
  ox-jsdoc/                  # typed AST package
  ox-jsdoc-binary/           # Binary AST package

wasm/
  ox-jsdoc/                  # typed AST package
  ox-jsdoc-binary/           # Binary AST package
```

This shape was intentional during benchmarking and validation. It made the typed-vs-binary comparison explicit and avoided impacting existing users while the Binary AST implementation matured.

If the Binary AST is going to be the product direction from here on, that rationale no longer applies. Keeping both implementations as first-class product paths long-term locks in the following duplication:

- parser maintenance
- binding maintenance
- documentation
- benchmark interpretation
- package naming

The original Binary AST design already assumed this destination as well:

- Coexist during development
- Atomically switch over after the GO decision
- Do not maintain both implementations as equally first-class product paths permanently

At the same time, the original typed AST implementation is still valuable as a comparison baseline against the Binary AST. This document defines the post-GO repository state while also preserving that implementation under the `origin` name as benchmark / reference only.

## Decisions

The Binary AST implementation becomes the canonical implementation of `ox-jsdoc`.

### Canonical names after migration

| Target | canonical name | post-migration implementation |
| --- | --- | --- |
| Rust core crate | `ox_jsdoc` | Binary AST parser / writer / decoder / public Rust API |
| Node.js npm package | `ox-jsdoc` | Binary AST NAPI binding |
| WASM npm package | `@ox-jsdoc/wasm` | Binary AST WASM binding |
| Shared JS decoder | `@ox-jsdoc/decoder` | Maintained as the shared public package |

The binding-side Rust crates under NAPI / WASM are aligned to the same canonical names. They are internal crates that exist to produce binding artifacts (`.node` / `.wasm`) and are not published to crates.io.

| Target | canonical name | Role |
| --- | --- | --- |
| NAPI binding Rust crate | `ox_jsdoc_napi` | Internal crate inside `napi/ox-jsdoc/`. Produces the Binary AST NAPI binding. `publish = false` |
| WASM binding Rust crate | `ox_jsdoc_wasm` | Internal crate inside `wasm/ox-jsdoc/`. Produces the Binary AST WASM binding. `publish = false` |

### Transitional names

| Current name | Goal |
| --- | --- |
| `ox_jsdoc_binary` | Rename to canonical `ox_jsdoc` |
| `ox-jsdoc-binary` | Keep as a thin deprecated re-export package for one deprecation cycle only |
| `@ox-jsdoc/wasm-binary` | Same as `ox-jsdoc-binary` — keep as a thin alias for one deprecation cycle only |
| The typed AST `ox_jsdoc` implementation | Rename into the `origin`-line benchmark / reference-only implementation |

The `-binary` public packages remain for one deprecation cycle for the benefit of users who have already installed the published versions. They do not carry a separate implementation; they shrink to thin aliases that re-export the canonical package, and after the deprecation window ends, new use is consolidated onto the canonical package.

### Reference-implementation names

| Target | Name | Role |
| --- | --- | --- |
| Rust core crate | `ox_jsdoc_origin` | The original typed AST implementation. Benchmark / reference only |
| Node.js npm package | `ox-jsdoc-origin` | The original typed AST NAPI binding. Benchmark / reference only |
| WASM npm package | `@ox-jsdoc/wasm-origin` | The original typed AST WASM binding. Benchmark / reference only |
| NAPI binding Rust crate | `ox_jsdoc_origin_napi` | Internal crate inside `napi/ox-jsdoc-origin/`. `publish = false` |
| WASM binding Rust crate | `ox_jsdoc_origin_wasm` | Internal crate inside `wasm/ox-jsdoc-origin/`. `publish = false` |

`origin` is a name that denotes "the first implementation" and carries no judgment about its current recommendation level or quality. Unlike `typed`, it does not imply rigor; unlike `legacy`, it is less likely to collide with future generational shifts. That makes it a fitting name for the benchmark reference implementation.

`origin`-line packages are not a product surface. The Rust crates are `publish = false` and the npm packages are `"private": true` by default; the canonical packages do not depend on them. Rust crates under `napi/*` and `wasm/*` — whether canonical or `origin` — are internal crates that exist to produce binding artifacts, and they all keep `publish = false` so that nothing is published to crates.io.

## Goals

- Make the Binary AST path the single main implementation
- Keep the original typed AST implementation isolated under the `origin` name as benchmark / reference only
- Drop implementation-detail suffixes from canonical package names
- Preserve the existing boundaries between core / bindings / shared JS / benchmark
- Maintain `@ox-jsdoc/decoder` as the decoder shared across NAPI / WASM / future IPC
- Preserve historically necessary benchmark evidence and design documents
- Bring the current docs and benchmark commands into a state where they describe the post-cutover product, not the migration experiment

## Non-Goals

- Maintaining the typed AST and Binary AST as equally first-class implementations long-term
- Maintaining `origin`-line packages as product packages for general use
- Keeping every transition-only benchmark as a first-class default command
- Retroactively rewriting past benchmark result files to match the new names
- Changing the Binary AST wire format as part of the repository reorganization
- Redesigning `@ox-jsdoc/jsdoccomment` or `@ox-jsdoc/eslint-plugin-jsdoc` beyond what is needed to follow the canonical parser package

## Target repository structure

```text
.
├── crates/
│   ├── ox_jsdoc/                  # Binary AST core crate
│   │   ├── decoder/
│   │   ├── format/
│   │   ├── parser/
│   │   └── writer/
│   └── ox_jsdoc_origin/           # first typed AST implementation, benchmark / reference only
├── napi/
│   ├── ox-jsdoc/                  # Binary AST NAPI package
│   └── ox-jsdoc-origin/           # first typed AST NAPI package, benchmark / reference only
├── wasm/
│   ├── ox-jsdoc/                  # Binary AST WASM package
│   └── ox-jsdoc-origin/           # first typed AST WASM package, benchmark / reference only
├── packages/
│   ├── decoder/                   # shared public lazy decoder
│   ├── jsdoccomment/              # private compat integration
│   └── eslint-plugin-jsdoc/       # private benchmark / integration fork
├── tasks/
│   └── benchmark/
├── design/
└── fixtures/
```

When compatibility-aliases are needed, keep the structure shallow.

```text
napi/
  ox-jsdoc-binary/                 # thin re-export only, no separate Rust crate

wasm/
  ox-jsdoc-binary/                 # thin re-export only, no separate Rust crate
```

An alias package must not become a second implementation path.

## Public API policy

A package rename can only be a mechanical exercise if the public API contract has been settled first.

### Decisions to make before cutover

1. Return value of `parse()`
   - The recommended canonical behavior is the Binary AST lazy result: `{ ast, diagnostics, sourceFile }`
   - When plain object materialization is needed it is done explicitly, and the JSON-compat cost is not put back on the default path

2. `parseBatch()`
   - Make it part of the canonical Node.js / WASM API
   - Batch parsing is a product feature, not an implementation experiment

3. `parseType()` / `parseTypeCheck()`
   - The current canonical `ox-jsdoc` package (typed AST NAPI / WASM) exposes standalone type parsing as `parseType(text, mode): string | null` and `parseTypeCheck(text, mode): boolean`
   - This surface is preserved post-cutover
   - The Rust implementation of the type parser already exists on the binary AST side as well (`crates/ox_jsdoc_binary/src/parser/type_parse.rs`) and is reachable through the `parseTypes: true` option of `parse()`. What Phase B adds is just the binding-layer work to expose standalone `parseType` / `parseTypeCheck` functions on the binary side via NAPI / WASM; reimplementing the core is unnecessary
   - Keep the return shape fully compatible with the typed AST side (`string | null` / `boolean`). This minimizes user migration cost
   - Expose it on the post-cutover canonical package before retiring the old typed package into `origin`

4. Rust-side validator / analyzer
   - The current typed AST crate exposes `validate_comment()` and `analyze_comment()`
   - If they are to remain, port them onto the lazy Binary AST API before the rename completes

A repository rename must not silently lose public surface area.

`origin`-line packages are not a compatibility target for the canonical API. Their role is to preserve the original typed AST implementation as a comparative reference, not to gain new features or maintain compatibility for general users. Changes to them are limited to what is necessary for benchmark validity, build health, and fixture compatibility.

## Package / crate migration

### Rust crates

Current:

```text
crates/ox_jsdoc/          # typed AST
crates/ox_jsdoc_binary/   # Binary AST
```

Goal:

```text
crates/ox_jsdoc/          # Binary AST, canonical
crates/ox_jsdoc_origin/   # first typed AST implementation, benchmark / reference only
```

Migration rules:

- Rename the current typed AST implementation to `crates/ox_jsdoc_origin/`
- Move the Binary AST implementation under `crates/ox_jsdoc/`
- Port any public API kept post-cutover ahead of time
- Move the canonical Rust benches under the canonical crate
- Typed-AST-comparison benches refer to `ox_jsdoc_origin` explicitly
- `ox_jsdoc_origin` is `publish = false` and remains benchmark / reference only
- Do not create a shared compatibility crate that keeps the old implementation alive for product compatibility

### NAPI packages

Current:

```text
napi/ox-jsdoc/            # typed AST + JSON transfer (internal Rust crate: ox_jsdoc_napi)
napi/ox-jsdoc-binary/     # Binary AST + lazy decoder (internal Rust crate: ox_jsdoc_binary_napi)
```

Goal:

```text
napi/ox-jsdoc/            # Binary AST + lazy decoder, canonical (internal Rust crate: ox_jsdoc_napi)
napi/ox-jsdoc-origin/     # typed AST + JSON transfer, benchmark / reference only (internal Rust crate: ox_jsdoc_origin_napi)
```

Migration rules:

- Rename the current typed AST binding to `napi/ox-jsdoc-origin/`
- Move the Binary AST binding to the canonical package name
- Apply the same rename to the internal Rust crates
  - Current `ox_jsdoc_napi` (typed AST) → `ox_jsdoc_origin_napi`
  - Current `ox_jsdoc_binary_napi` (Binary AST) → `ox_jsdoc_napi` (canonical)
  - Both crates keep `publish = false` and are not published to crates.io
- Keep `parseBatch()` on the canonical package
- Update tests / READMEs / internal imports to the canonical package
- `ox-jsdoc-origin` is `"private": true` and used only from benchmarks
- `ox-jsdoc-binary` remains as a re-export package for one deprecation cycle

### WASM packages

Current:

```text
wasm/ox-jsdoc/            # typed AST + JSON transfer (internal Rust crate: ox_jsdoc_wasm)
wasm/ox-jsdoc-binary/     # Binary AST + lazy decoder (internal Rust crate: ox_jsdoc_binary_wasm)
```

Goal:

```text
wasm/ox-jsdoc/            # Binary AST + lazy decoder, canonical (internal Rust crate: ox_jsdoc_wasm)
wasm/ox-jsdoc-origin/     # typed AST + JSON transfer, benchmark / reference only (internal Rust crate: ox_jsdoc_origin_wasm)
```

Migration rules are the same as for NAPI.

- Rename the current typed AST binding to `wasm/ox-jsdoc-origin/`
- Make `@ox-jsdoc/wasm` canonical
- Apply the same rename to the internal Rust crates
  - Current `ox_jsdoc_wasm` (typed AST) → `ox_jsdoc_origin_wasm`
  - Current `ox_jsdoc_binary_wasm` (Binary AST) → `ox_jsdoc_wasm` (canonical)
  - Both crates keep `publish = false` and are not published to crates.io
- Keep `parseBatch()`
- `@ox-jsdoc/wasm-origin` is `"private": true` and used only from benchmarks
- `@ox-jsdoc/wasm-binary` remains as a temporary alias only when compatibility requires it

### Shared JavaScript packages

`@ox-jsdoc/decoder` is shared by all Binary AST transports, so it remains part of the main architecture.

Post-cutover, `@ox-jsdoc/jsdoccomment` depends on the canonical `ox-jsdoc` rather than `ox-jsdoc-binary`.

`@ox-jsdoc/eslint-plugin-jsdoc` remains a private integration / benchmark fork; it does not add a separate parser dependency and follows `@ox-jsdoc/jsdoccomment`.

## npm registry / publish policy

Making the Binary AST canonical involves not just directory renames inside the repository but also a decision about how to migrate the public packages on the npm registry.

### Basic policy

- Do not introduce new public NAPI / WASM packages
- `origin`-line packages are benchmark / reference-only private workspace packages and are not published to the npm registry
- Canonical public packages reuse their existing names
  - NAPI: `ox-jsdoc`
  - WASM: `@ox-jsdoc/wasm`
  - shared decoder: `@ox-jsdoc/decoder`
- The `-binary`-line packages, if needed, are treated as thin aliases for the migration window only and are then deprecated
- NAPI platform packages are also consolidated to the canonical line; the `binary-binding` line is not used on the canonical path

### semver and dist-tags

- The current version series is `0.0.x` (unstable / pre-1.0). Because publishes assume that the public API contract is not yet finalized, a cutover release that includes breaking changes is also handled with **a patch-number bump (`0.0.12 → 0.0.13`)**
  - Since the official guidance is "do not use 0.0.x in production," the impact on typical users is limited
  - A semver-major bump (the promotion to `1.0.0`) is reserved for a separate release that declares the API surface stable
- The cutover release **switches the `latest` dist-tag immediately**
  - There is no staged `next` release and no `legacy` dist-tag fallback for the old typed AST users
  - This is consistent with the policy of not providing a formal migration path for old typed AST users
- The `-binary` alias packages (`ox-jsdoc-binary` / `@ox-jsdoc/wasm-binary`) are also published as thin alias releases on the same `0.0.13` series
- `@ox-jsdoc/binary-binding-*` stay at `0.0.12` and only have `npm deprecate` applied to them (no new versions are published)

### Treatment of public packages

| Package | Current role | Post-cutover role | Treatment on the registry |
| --- | --- | --- | --- |
| `ox-jsdoc` | typed AST NAPI package | canonical Binary AST NAPI package | Continues under the same name. A breaking release that switches to Binary AST is published |
| `@ox-jsdoc/binding-*` | platform binding for `ox-jsdoc` | canonical Binary AST platform binding | Continues under the same name and is updated in the same release series as `ox-jsdoc` |
| `@ox-jsdoc/wasm` | typed AST WASM package | canonical Binary AST WASM package | Continues under the same name. A breaking release that switches to Binary AST is published |
| `@ox-jsdoc/decoder` | Binary AST shared decoder | Binary AST shared decoder | Continues under the same name |
| `ox-jsdoc-binary` | Binary AST NAPI package | Alias kept for one deprecation cycle only | Publish a thin alias release that re-exports `ox-jsdoc`, then deprecate it |
| `@ox-jsdoc/binary-binding-*` | platform binding for `ox-jsdoc-binary` | Not needed on the canonical path | At the same time as the cutover release, run `npm deprecate` on the existing versions. Do not publish new versions |
| `@ox-jsdoc/wasm-binary` | Binary AST WASM package | Alias kept for one deprecation cycle only | Publish a thin alias release that re-exports `@ox-jsdoc/wasm`, then deprecate it |

`ox-jsdoc` and `@ox-jsdoc/wasm` keep their package names, but their implementation and return-value contract change from typed AST to Binary AST lazy result, so the cutover release is treated as a breaking change.

### Packages not published

| Package | Treatment |
| --- | --- |
| `ox-jsdoc-origin` | `"private": true`. For local benchmarks. Not published to the registry |
| `@ox-jsdoc/wasm-origin` | `"private": true`. For local benchmarks. Not published to the registry |
| `ox_jsdoc_origin` | Rust core crate is `publish = false`. Not published to crates.io |
| `ox_jsdoc_napi` / `ox_jsdoc_origin_napi` | NAPI binding internal crates. `publish = false`. Not published to crates.io |
| `ox_jsdoc_wasm` / `ox_jsdoc_origin_wasm` | WASM binding internal crates. `publish = false`. Not published to crates.io |

`ox-jsdoc-origin` assumes a local workspace build; no new public platform packages such as `@ox-jsdoc/origin-binding-*` are created.

Rust crates under `napi/*` and `wasm/*` are internal crates that exist to produce binding artifacts (`.node` / `.wasm`); none of them — canonical or `origin` — are ever published to crates.io. The only thing exposed for Rust users is the core crate at `crates/ox_jsdoc/` (whether or not it is publishable is a separate discussion).

### Release sequence

1. Update `@ox-jsdoc/decoder` if needed
2. Publish the canonical NAPI platform packages `@ox-jsdoc/binding-*` and the wrapper package `ox-jsdoc` in the same release series
3. Publish the canonical WASM package `@ox-jsdoc/wasm`
4. For the one deprecation cycle, publish thin alias releases of `ox-jsdoc-binary` and `@ox-jsdoc/wasm-binary`
5. At the same time as the cutover release, run `npm deprecate` on the existing versions of `@ox-jsdoc/binary-binding-*`. Do not publish new versions
6. After the migration window, run `npm deprecate` on `ox-jsdoc-binary` and `@ox-jsdoc/wasm-binary`

Published versions are not assumed to be deletable; the migration target is signaled on the registry through deprecation. The README, deprecation message, and release notes of `-binary` alias releases must clearly state the migration target on the canonical package. `@ox-jsdoc/binary-binding-*` lose their role the moment the alias `ox-jsdoc-binary` becomes a JS-only re-export, so the intent is to deprecate them without waiting for the cutover release.

### Publish pipeline

The publish pipeline switches from "mechanically publish whatever package exists in the workspace" to an explicit allowlist of packages that go to the registry.

#### Package categories

| Category | Package | Treatment |
| --- | --- | --- |
| Always publish (canonical) | `@ox-jsdoc/decoder`, `ox-jsdoc`, `@ox-jsdoc/binding-*`, `@ox-jsdoc/wasm` | The main subject of normal releases |
| Publish for one deprecation cycle (alias) | `ox-jsdoc-binary`, `@ox-jsdoc/wasm-binary` | Published as thin aliases |
| Do not publish new versions, only deprecate | `@ox-jsdoc/binary-binding-*` | At the same time as the cutover release, run `npm deprecate` on the existing versions. Once the alias `ox-jsdoc-binary` becomes JS-only there is no role left for them, so no new versions are published |
| Do not publish | `ox-jsdoc-origin`, `@ox-jsdoc/wasm-origin` | Benchmark / reference only. Not on the registry |

#### Execution order

1. Build / test `@ox-jsdoc/decoder`
2. Native-build only the canonical NAPI package `napi/ox-jsdoc` for each target
3. Release-build only the canonical WASM package `@ox-jsdoc/wasm`
4. Publish the canonical packages
   - `@ox-jsdoc/decoder`
   - `@ox-jsdoc/binding-*`
   - `ox-jsdoc`
   - `@ox-jsdoc/wasm`
5. Publish the one-deprecation-cycle alias packages
   - `ox-jsdoc-binary`
   - `@ox-jsdoc/wasm-binary`
6. Run `npm deprecate` on the existing versions of `@ox-jsdoc/binary-binding-*`. Do not publish new versions
7. Run smoke tests
   - Always check: `@ox-jsdoc/decoder`, `ox-jsdoc`, `@ox-jsdoc/wasm`
   - Additionally check on alias releases: `ox-jsdoc-binary`, `@ox-jsdoc/wasm-binary`
   - Confirm deprecation: when fetching the latest version of `@ox-jsdoc/binary-binding-*`, the deprecation message is shown

#### Implementation rules

- `origin`-line packages remain for local workspace build / benchmark, but are not part of the release job
- The retained `ox-jsdoc-binary` does not carry a separate native binding build
- Alias packages are JS-only packages that re-export the canonical package; they do not create a second artifact series
- `@ox-jsdoc/binary-binding-*` are not part of publish targets after cutover. They are addressed on the registry only via `npm deprecate`
- The list of publish targets is not duplicated across workflows or scripts; it is consolidated into a single allowlist definition
- The version-bump targets in the root `package.json`, `release.yml`, `scripts/release.sh`, `scripts/npm-trust.sh`, and the smoke tests are all updated against the same allowlist

The allowlist can be expressed in JSON or as a JavaScript module, but at minimum the canonical / alias / non-publish categories must be readable in one place. This prevents the accidental publish of `origin` packages and the accident of leaving retired alias packages in the release workflow indefinitely.

## Toolchain / automation policy

Making the Binary AST canonical does not finish at repository-layout renames. Workspace auto-detection, the release workflow, publish scripts, and benchmark dependency assumptions all need to be updated at the same time.

### Settings that can stay as-is

- `members = ["crates/*", ..., "napi/*", "wasm/*"]` in the root `Cargo.toml`
- `napi/*` / `wasm/*` / `packages/*` / `tasks/*` in `pnpm-workspace.yaml`
- `build` / `test` / `check` / `fmt` in the root `package.json`
- The whole-workspace build / test in `.github/workflows/ci.yml`

These are glob-based, so adding `origin`-line packages is picked up automatically. Even though `origin` implementations are benchmark / reference only, it matters that they do not break as a comparison baseline, so they remain in the normal CI build / test set.

### Settings that need explicit updates

| Target | Current | Post-cutover policy |
| --- | --- | --- |
| `release` script in the root `package.json` | Bumps versions of `wasm/**` / `napi/*` together | Move to a public-package allowlist. Do not implicitly include `origin` in release targets |
| NAPI build matrix in `.github/workflows/release.yml` | Native-builds both `ox-jsdoc` and `ox-jsdoc-binary` | Native-build only the canonical `ox-jsdoc`. The retained alias is a JS-only package; do not give it a separate native build |
| WASM build in `.github/workflows/release.yml` | `vpr -F './wasm/*' build --release` | Build the canonical public WASM package explicitly. Do not mix the private `@ox-jsdoc/wasm-origin` into release jobs |
| publish / smoke tests in `.github/workflows/release.yml` | Treats `ox-jsdoc-binary` as a first-class publish target | Switch the main subject to the canonical package; on alias releases, additionally smoke-test the thin alias |
| `scripts/release.sh` | Publishes all of `wasm/*` | Publish per the public WASM package allowlist. Do not publish `origin` |
| `scripts/npm-trust.sh` | Includes `-binary` / `binary-binding-*` in trusted-publishing targets | Reorganize around canonical packages and do not add `origin`. After cutover, `@ox-jsdoc/binary-binding-*` are not published, so remove them from trusted-publishing targets. `-binary` aliases are removed in stages following the deprecation plan |
| `tasks/benchmark/package.json` | References `ox-jsdoc-binary` / `@ox-jsdoc/wasm-binary` | The binary side references the canonical package; the typed side references the `origin` package |
| `tasks/benchmark/Cargo.toml` | References `ox_jsdoc` as the typed AST side | Reference the Binary AST side as canonical `ox_jsdoc` and the typed AST side as `ox_jsdoc_origin` |

### Generated files and lint / format

Generated NAPI bindings appear in multiple directories after the package rename. The ignore configuration in the root `vite.config.ts` is better off normalizing the generated-file pattern rather than pointing at a specific package.

It currently says `napi/ox-jsdoc/src-js/binding.*`, but the actual generated files are `bindings.js` / `bindings.d.ts`. If `origin` packages are added during the cutover, instead of growing additional one-off paths around the existing inconsistency, update the configuration so that generated bindings are ignored consistently.

### Lockfiles

`Cargo.lock` and `pnpm-lock.yaml` are updated as a result of directory renames and dependency renames. Include them as ordinary migration artifacts, but treat them separately from the publish-script allowlist work.

## Implementation change policy

The cutover is not just directory renames. Currently, the typed AST side and the Binary AST side expose different public surfaces, so the canonical API has to be settled first and the resulting gap has to be closed in code.

### Current differences

| Surface | Typed AST side | Binary AST side | Treatment at cutover |
| --- | --- | --- | --- |
| Rust core | `parse_comment`, `parse_type`, `validate_comment`, `analyze_comment` | `parse`, `parse_into`, `parse_batch`, `parse_batch_into`, `parse_*_to_bytes` | Decide upfront which Rust surface stays as the canonical API |
| NAPI / WASM `parse()` | JSON materialization path | Lazy Binary AST decoder path | Canonical switches to the Binary AST lazy result — a breaking change |
| NAPI / WASM `parseBatch()` | None | Present | Promote to the canonical package |
| `parseType()` / `parseTypeCheck()` | Exported as standalone functions on NAPI / WASM | The Rust core implementation exists (`crates/ox_jsdoc_binary/src/parser/type_parse.rs`); only the NAPI / WASM standalone exports are missing | Add NAPI / WASM standalone exports on the binary main side and keep the return shape fully compatible with the typed AST side (`string \| null` / `boolean`) |
| `@ox-jsdoc/jsdoccomment` integration | Depends on `ox-jsdoc-binary` | Uses `parseBatch(..., { output: "jsdoccomment-input" })` | Switch the dependency to the canonical `ox-jsdoc` and keep the batch fast path |

In particular, `parseType()` / `parseTypeCheck()` exist only on the current canonical `ox-jsdoc` / `@ox-jsdoc/wasm` and are missing from the Binary AST package. Because they are preserved post-cutover, add the implementation and tests on the Binary AST main side before the rename.

### Required code changes

1. Move the original typed AST implementation into the `origin`-line
   - `crates/ox_jsdoc` → `crates/ox_jsdoc_origin`
   - `napi/ox-jsdoc` → `napi/ox-jsdoc-origin` (internal Rust crate also `ox_jsdoc_napi` → `ox_jsdoc_origin_napi`)
   - `wasm/ox-jsdoc` → `wasm/ox-jsdoc-origin` (internal Rust crate also `ox_jsdoc_wasm` → `ox_jsdoc_origin_wasm`)
   - The above NAPI / WASM internal Rust crates keep `publish = false`

2. Promote the Binary AST implementation to the canonical name
   - `crates/ox_jsdoc_binary` → `crates/ox_jsdoc`
   - `napi/ox-jsdoc-binary` → `napi/ox-jsdoc` (internal Rust crate also `ox_jsdoc_binary_napi` → `ox_jsdoc_napi`)
   - `wasm/ox-jsdoc-binary` → `wasm/ox-jsdoc` (internal Rust crate also `ox_jsdoc_binary_wasm` → `ox_jsdoc_wasm`)
   - The above NAPI / WASM internal Rust crates keep `publish = false`

3. Settle the public surface of the canonical package
   - `parse()` returns the Binary AST lazy result
   - Make `parseBatch()` part of the canonical API
   - Expose `parseType()` / `parseTypeCheck()` as standalone functions on the canonical NAPI / WASM. Because the Rust core implementation already exists on the binary side, only the binding layer (NAPI export / wasm-bindgen export), the `index.d.ts` export declaration, and tests need to be added. Keep the return shape fully compatible with the typed AST side (`string | null` / `boolean`)
   - If `validate_comment()` / `analyze_comment()` are kept on the Rust side, redesign them on the canonical Binary AST API

4. Switch internal users to the canonical package
   - `@ox-jsdoc/jsdoccomment` imports `ox-jsdoc` rather than `ox-jsdoc-binary`
   - `@ox-jsdoc/eslint-plugin-jsdoc` keeps using its `@ox-jsdoc/jsdoccomment`-mediated path
   - Benchmark / sanity scripts rebind the binary side to the canonical package and the typed side to the `origin` package

5. Build the thin wrappers for the one deprecation cycle
   - `ox-jsdoc-binary` / `@ox-jsdoc/wasm-binary` are re-exports of the canonical package only
   - The aliases do not carry a separate Rust crate, a separate NAPI binding, or a separate WASM artifact

### Implementation order

If the canonical API disappears mid-rename, it becomes hard to tell intentional breaking changes apart from accidental regressions. Fix the order as follows.

1. Settle the public APIs that remain after cutover
2. Implement what is missing on the Binary AST side among the APIs that remain
3. Move the typed AST implementation aside under the `origin` name
4. Promote the Binary AST implementation to the canonical name
5. Update `jsdoccomment` / benchmarks / docs / publish pipeline to canonical + `origin` names
6. Add the thin wrappers for the one deprecation cycle last

## Design document policy

The design documents from `001` through `010` are all treated as still-valid design documents post-cutover. Rather than retiring them as historical, they are kept consistent with the current package / crate names through ongoing updates.

### Naming principles

- When referring to the current product path, use the canonical name
  - Rust: `ox_jsdoc`
  - NAPI: `ox-jsdoc`
  - WASM: `@ox-jsdoc/wasm`
- When referring to the original typed AST implementation, use the `origin` name
  - Rust: `ox_jsdoc_origin`
  - NAPI: `ox-jsdoc-origin`
  - WASM: `@ox-jsdoc/wasm-origin`
- `ox_jsdoc_binary` / `ox-jsdoc-binary` / `@ox-jsdoc/wasm-binary` remain only in places that describe the pre-cutover transitional structure
- Where transitional names appear, make it clear from context that they are pre-cutover names

### How to update documents

- Maintain `001`–`010` as design documents that remain referenceable
- Keep each document's subject, but update its content to match the current repository structure and naming
- When earlier implementation paths need to be described, do not leave them as "old documents" — describe them using the current name `origin`
- Where the present-tense post-cutover behavior is described, do not leave the `-binary` names
- Where the cutover decision-making and migration history are described, the contemporaneous `-binary` name may be used, but make sure not to confuse it with the current package topology

### Main update targets

- `002-project-structure/`
  - Update the current repository layout to the canonical + `origin` structure
- `003-js-binding/`
  - Update so that the JSON-first typed AST binding design can be read as the design of `ox-jsdoc-origin`
- `004-wasm/`
  - Update so that the JSON-first typed AST WASM binding design can be read as the design of `@ox-jsdoc/wasm-origin`
- `005-jsdoccomment-compat/`
  - Clearly separate the responsibilities of the canonical binary path and the `origin` typed path
- `007-binary-ast/`
  - Update the description of the main implementation to the canonical name; keep the transitional names only in the description of the coexistence phase
- `008-oxlint-oxfmt-support/`
  - Express the canonical binary path as the primary path and the typed path as `origin`
- `009-jsdoc-linter-benchmark/`
  - Align benchmark display names and dependency package names with the canonical / `origin` policy

`design/index.md` is also maintained as a subject-based table of contents rather than an active / historical split. The priority is that a new reader can read it without surprise, and naming drift is not left around.

### Order in which to perform the naming-alignment pass

1. Update `002-project-structure/` to make the post-cutover canonical + `origin` structure the baseline for the repository layout
2. Update `003-js-binding/` and `004-wasm/` so that the JSON-first typed AST binding designs read as designs of the `origin` implementations
3. Update `007-binary-ast/` so that the main implementation description shifts to the canonical name and the transitional `-binary` names are confined to coexistence-phase explanations
4. Update `005-jsdoccomment-compat/`, `008-oxlint-oxfmt-support/`, and `009-jsdoc-linter-benchmark/` so that the canonical and `origin` paths are described with consistent vocabulary
5. Re-scan `001`–`010` as a whole and confirm that no naming drift remains in package / crate names, links, benchmark display names, or code examples

`001-performance/` and `006-parsed-type/` are less directly affected by package renames, but include them in the final pass for the same kind of check.

## Benchmark policy

Even after the Binary AST becomes canonical, the benchmark measurement design itself does not change. Because the original typed AST implementation remains as `origin`-line packages, the existing typed AST vs. Binary AST comparisons remain valid.

What changes is just making sure that package imports / dependencies / display names do not contradict the new repository structure. Fixtures, scenarios, measurement layers, and comparison axes are preserved.

### Benchmarks that are kept

- Parser-only NAPI / WASM measurement
- `parseBatch()` measurement
- Rust-direct canonical parser measurement
- JSDoc linter benchmark
- Typed AST vs. Binary AST NAPI comparison
- Typed AST vs. Binary AST WASM comparison

`origin` is a package name, not a benchmark comparison axis itself. In comparison tables and script names, `typed AST` and `Binary AST` may continue to be used. Post-cutover, only the import / dependency on the typed AST side changes to `ox-jsdoc-origin` / `@ox-jsdoc/wasm-origin`.

### Directory structure

```text
tasks/benchmark/
  scripts/
  results/
```

`scripts/` is not split. Because the `origin` implementation remains on the mainline, comparison benchmarks do not need to be moved into a separate directory.

The existing `binary-vs-typed-*` scripts and package scripts can be kept because they describe what is being measured. All that is needed is to update the package names they reference to follow the canonical / `origin` policy.

Historical result files are not rewritten retroactively. They remain as a record of the migration period using the package names that existed at that time.

### Display policy in the root README

The structure of benchmarks shown in the root `README.md` does not change.

- TL;DR
- Measurement environment
- Measurement method
- JSDoc linter
- parser-only Node.js
- Rust-direct parser reference

What changes is only the naming in row labels and explanatory text.

- Package / crate names match the post-cutover reality
  - canonical Binary AST: `ox-jsdoc` / `@ox-jsdoc/wasm` / `ox_jsdoc`
  - original typed AST reference: `ox-jsdoc-origin` / `@ox-jsdoc/wasm-origin` / `ox_jsdoc_origin`
- The implementation-method comparison axis is still expressed with `Binary AST` and `typed AST`
- `binary` is used as a word describing the implementation method, not as a package name
- `origin` is used as a package name and is not the comparison axis itself

For example, the parser-only table post-cutover is laid out by role as follows.

| Row label                                | Meaning                                              |
| ---------------------------------------- | ---------------------------------------------------- |
| `ox-jsdoc NAPI (parseBatch)`             | Binary AST batch path via the canonical package      |
| `@ox-jsdoc/wasm (parseBatch)`            | Binary AST batch path via the canonical WASM package |
| `ox-jsdoc-origin NAPI (typed AST, loop)` | The original typed AST reference package             |

In the Rust-direct table, the Binary AST row is `ox_jsdoc` and the typed AST reference row is `ox_jsdoc_origin`.

The comparison-baseline column also matches the package rename. For example, `vs canonical NAPI parseBatch` describes the post-cutover role more accurately than `vs binary NAPI parseBatch`.

If the README body shows pre-migration benchmark snapshots verbatim, add a one-line note that the names have been read against the current package names where applicable. If you rerun and update the results, however, you may write them in current names only as usual.

## Migration phases

### Phase A: Settle the canonical API

- Decide the return contract of `parse()`
- Maintain `parseType()` / `parseTypeCheck()` as part of the canonical API
- Decide whether to keep the Rust validator / analyzer public
- The `-binary` packages remain as thin aliases for one deprecation cycle only
- Settle the benchmark / reference-only policy for the `origin` packages

### Phase B: Close the API gap on the Binary AST side

- Expose standalone type-parsing APIs (`parseType` / `parseTypeCheck`) as standalone functions on the Binary AST main NAPI / WASM
  - The Rust core implementation already exists at `crates/ox_jsdoc_binary/src/parser/type_parse.rs`, so it is enough to add a thin wrapper to the binding layer (`napi/ox-jsdoc-binary/src/lib.rs`, `wasm/ox-jsdoc-binary/src/lib.rs`)
  - Keep the return shape fully compatible with the typed AST side (`string | null` / `boolean`)
- If they are kept, port the validator / analyzer
- Add tests that prove the canonical API is in place before the rename
  - Add tests on the binary AST side that mirror `napi/ox-jsdoc/test/parsed-type.test.ts` on the typed AST side

### Phase C: Promote the Binary AST to the canonical name

- Rename the current typed AST implementation to the `origin` name
- Rename / move `crates/ox_jsdoc_binary` to `crates/ox_jsdoc`
- Replace `napi/ox-jsdoc` with the Binary AST implementation
- Replace `wasm/ox-jsdoc` with the Binary AST implementation
- Update workspace-internal dependencies to the canonical name
- Update `@ox-jsdoc/jsdoccomment` to depend on `ox-jsdoc`
- Keep the canonical NAPI package on `@ox-jsdoc/binding-*` and remove `@ox-jsdoc/binary-binding-*` from the canonical path

### Phase D: Retire migration-only public surface

- Pin `origin`-line packages to a private / publish=false benchmark / reference-only surface
- Shrink the `-binary` packages to thin deprecated aliases
- Publish the alias releases on the registry, then enter the deprecation window
- Delete or rename the default benchmark commands that existed only for the cutover

### Phase E: Update the entry points for docs and benchmarks

- Align package / crate names in `001`–`010` to the canonical / `origin` policy
- Perform the naming-alignment pass in the order `002` → `003` / `004` → `007` → `005` / `008` / `009` → final whole-of-`001`–`010` check
- Update `design/index.md` to a subject-based table of contents
- Update the root README and package READMEs to canonical names
- Keep the benchmark measurement design as-is and update only imports / dependencies / display names to the canonical / `origin` policy

## Verification plan

Verification post-cutover is not satisfied by "the build passes after the rename." Confirm canonical implementation, `origin` reference implementation, integration packages, and public packages as four separate facets.

### Migration / preservation of automated tests

| Current test | Post-cutover role |
| --- | --- |
| `crates/ox_jsdoc/tests/*` | Move to `crates/ox_jsdoc_origin/tests/*` and watch for regressions of the typed AST reference implementation |
| `crates/ox_jsdoc_binary/tests/*` | Maintain as canonical `crates/ox_jsdoc/tests/*` |
| `napi/ox-jsdoc/test/*` | Move to `napi/ox-jsdoc-origin/test/*`. Add corresponding tests for `parseType()` / `parseTypeCheck()` on the canonical side as well |
| `napi/ox-jsdoc-binary/test/*` | Maintain as canonical `napi/ox-jsdoc/test/*`. Move compat tests to the canonical side as well |
| `wasm/ox-jsdoc/test/*` | Move to `wasm/ox-jsdoc-origin/test/*`. Add corresponding tests for `parseType()` / `parseTypeCheck()` on the canonical side as well |
| `wasm/ox-jsdoc-binary/test/*` | Maintain as canonical `wasm/ox-jsdoc/test/*` |
| `packages/decoder/test/*` | Maintain as-is |
| `packages/jsdoccomment/test/*` | Pass all tests after the dependency switch |
| `packages/eslint-plugin-jsdoc/test/*` | Pass all tests as a regression check on the linter integration |

### Tests that need to be added

- Contract test that the canonical NAPI / WASM `parse()` returns the lazy Binary AST result
- Contract test for the canonical NAPI / WASM `parseBatch()` batch / `jsdoccomment-input` paths
- Integration test that `@ox-jsdoc/jsdoccomment` uses the batch fast path via the canonical `ox-jsdoc`
- NAPI / WASM contract tests for `parseType()` / `parseTypeCheck()`
- Smoke test confirming that alias packages are nothing but re-exports of the canonical package
- Confirmation in the publish dry-run / smoke test that `origin` packages are not in the release targets

### Commands

The same checks as normal CI:

```bash
cargo test
pnpm -r test
pnpm -r build
```

Cutover-specific additional checks:

```bash
pnpm --filter ox-jsdoc test
pnpm --filter @ox-jsdoc/wasm test
pnpm --filter @ox-jsdoc/jsdoccomment test
pnpm --filter @ox-jsdoc/eslint-plugin-jsdoc test
node tasks/benchmark/scripts/sanity-check.mjs
```

For alias releases, the release workflow's smoke test installs / imports the alias packages in addition to the canonical packages.

## Verification checklist

- `cargo test`
- `cargo check`
- `pnpm -r test`
- `pnpm -r build`
- The canonical NAPI tests pass from `napi/ox-jsdoc`
- The canonical WASM tests pass from `wasm/ox-jsdoc`
- The canonical contract test for `parseBatch()` passes on both NAPI and WASM
- The canonical contract test for `parseType()` / `parseTypeCheck()` passes on both NAPI and WASM
- `@ox-jsdoc/jsdoccomment` works through the canonical `ox-jsdoc`
- The `@ox-jsdoc/eslint-plugin-jsdoc` test suite passes via the canonical parser
- `ox_jsdoc_origin` / `ox-jsdoc-origin` / `@ox-jsdoc/wasm-origin` build as benchmark / reference only
- `ox-jsdoc-origin` / `@ox-jsdoc/wasm-origin` are not in the publish targets
- The canonical `ox-jsdoc` uses `@ox-jsdoc/binding-*` and the canonical path does not depend on `@ox-jsdoc/binary-binding-*`
- The release workflow native-builds / publishes only canonical public packages
- `scripts/release.sh` does not publish private `origin` packages
- `scripts/npm-trust.sh` only handles canonical public packages and aliases kept for the one deprecation cycle
- The root `vite.config.ts` ignores generated NAPI bindings under a consistent pattern
- The parser-only benchmark commands reference canonical names
- The typed AST vs. Binary AST benchmarks explicitly reference the `origin` package on the typed side
- The JSDoc linter benchmark continues to pass and reports the canonical name
- `tasks/benchmark/scripts/sanity-check.mjs` passes with canonical / `origin` imports
- Aside from the thin alias packages themselves, no mainline source imports `ox-jsdoc-binary` or `@ox-jsdoc/wasm-binary`
- No package / crate name drift in the design documents `001`–`010`
- Every design document is reachable from `design/index.md` without surprise

## Risks

### Hidden API regressions in rename work

The biggest practical risk is dropping existing public functionality during the rename. Standalone type parsing and Rust validator / analyzer entry points are particularly easy to overlook. Treat the API contract as a precondition for the move, not as cleanup after the move.

### Ambiguity in benchmark notation

While package names change to canonical / `origin`, the measurement axis remains typed AST vs. Binary AST. If documents, scripts, or result tables conflate package names with the comparison axis, what is being measured becomes hard to read. Update imports to `origin` names while keeping comparison names in the vocabulary that describes the AST style.

### Permanent forking of alias packages

Allow compatibility packages only when they re-export the canonical implementation. Once you start maintaining a second binding path, you reintroduce the duplication this migration is trying to eliminate.

### `origin` packages drifting into the product path

`origin`-line packages are kept only for benchmarks / reference, not as a compatibility surface for general users. If new features or product dependencies start being added back, you end up with a substantive double-implementation maintenance load even after canonicalization.

## Open questions

None at this time.
