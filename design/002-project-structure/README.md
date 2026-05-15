# Project Structure

**Status:** Implemented (post-cutover, see [`../010-main-stream-binary/README.md`](../010-main-stream-binary/README.md))

This document describes the post-cutover repository layout of `ox-jsdoc`. The layout supports two implementations side by side: the canonical Binary AST implementation (the product main path) and the original typed AST implementation kept under the `origin` name for benchmark / reference use only.

The structure keeps these concerns separate without making the repository too large.

## Goals

- Keep the parser core as a normal Rust library.
- Keep the JavaScript packages as workspace packages.
- Keep NAPI / WASM transfer code outside the core parser crate.
- Keep performance fixtures and benchmarks at repository level.
- Allow the canonical Binary AST implementation and the `origin`-line typed AST reference to coexist as parallel workspace members without duplication.
- Leave room for future toolchain integration without restructuring the repository.

## Non-Goals

- Do not introduce raw transfer as a core design requirement (the canonical Binary AST already provides zero-copy bytes).
- Do not split the Rust parser into many crates beyond the canonical / `origin` pair.
- Do not publish more than the canonical public packages plus the documented `-binary` thin aliases.
- Do not make `refers/` part of the workspace.
- Do not put benchmark fixtures inside a package-specific directory.

## Layout

```text
.
├── Cargo.toml
├── package.json
├── pnpm-workspace.yaml
├── rust-toolchain.toml
├── crates/
│   ├── ox_jsdoc/                      # canonical (Binary AST) Rust core crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── decoder/               # Rust-side lazy decoder
│   │       ├── format/                # binary AST wire format
│   │       ├── parser/                # parser-integrated binary writer
│   │       └── writer/                # BinaryWriter implementation
│   └── ox_jsdoc_origin/               # original typed AST Rust core (benchmark / reference only)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── ast.rs
│           ├── analyzer/
│           ├── parser/                # context / scanner / type_parse / etc.
│           ├── serializer/            # JSON serializer
│           ├── type_parser/
│           └── validator/
├── napi/
│   ├── ox-jsdoc/                      # canonical NAPI binding (npm: ox-jsdoc)
│   │   ├── Cargo.toml                 # internal Rust crate: ox_jsdoc_napi (publish = false)
│   │   ├── package.json               # napi.packageName: @ox-jsdoc/binding
│   │   ├── src/lib.rs
│   │   └── src-js/
│   ├── ox-jsdoc-origin/               # typed AST NAPI binding (private)
│   │   ├── Cargo.toml                 # internal crate: ox_jsdoc_origin_napi (publish = false)
│   │   ├── package.json               # "private": true
│   │   ├── src/lib.rs
│   │   └── src-js/
│   └── ox-jsdoc-binary/               # JS-only thin alias re-exporting `ox-jsdoc` (deprecated)
│       └── src-js/
├── wasm/
│   ├── ox-jsdoc/                      # canonical WASM binding (npm: @ox-jsdoc/wasm)
│   ├── ox-jsdoc-origin/               # typed AST WASM binding (private)
│   └── ox-jsdoc-binary/               # JS-only thin alias re-exporting `@ox-jsdoc/wasm` (deprecated)
├── packages/
│   ├── decoder/                       # shared lazy decoder package (@ox-jsdoc/decoder)
│   ├── jsdoccomment/                  # private jsdoccomment-compat integration
│   └── eslint-plugin-jsdoc/           # private eslint plugin fork
├── tasks/
│   ├── benchmark/                     # criterion + Node.js benchmark suite
│   └── xtask/                         # repo automation tasks (license header check, etc.)
├── fixtures/
│   └── perf/                          # shared benchmark fixture corpus
├── design/
└── refers/                            # research / reference submodules (not in workspace)
```

## Workspace Boundaries

### Cargo workspace

Root `Cargo.toml` defines the Rust workspace:

```toml
[workspace]
resolver = "3"
members = [
  "crates/*",
  "napi/*",
  "wasm/*",
  "tasks/*",
]
exclude = [
  "refers/*",
]
```

The workspace crates are:

- `crates/ox_jsdoc`
  - canonical Binary AST core crate
  - owns the binary format, parser-integrated writer, Rust-side lazy decoder, and the public Rust API
- `crates/ox_jsdoc_origin`
  - original typed AST core crate (benchmark / reference only)
  - owns the typed AST, parser, validator, analyzer, type parser, and JSON serializer
  - `publish = false`
- `napi/ox-jsdoc` — canonical NAPI binding crate (`ox_jsdoc_napi`), depends on `ox_jsdoc`
- `napi/ox-jsdoc-origin` — typed AST NAPI binding crate (`ox_jsdoc_origin_napi`), depends on `ox_jsdoc_origin`
- `wasm/ox-jsdoc` — canonical WASM binding crate (`ox_jsdoc_wasm`)
- `wasm/ox-jsdoc-origin` — typed AST WASM binding crate (`ox_jsdoc_origin_wasm`)
- `tasks/benchmark` — benchmark crate using `criterion2`, depends on both `ox_jsdoc` and `ox_jsdoc_origin`
- `tasks/xtask` — repository automation crate

All NAPI / WASM internal Rust crates are `publish = false`; they exist only to produce binding artifacts (`.node` / `.wasm`).

The thin alias directories `napi/ox-jsdoc-binary/` and `wasm/ox-jsdoc-binary/` are JS-only and do not contain Rust crates. They are not Cargo workspace members.

### pnpm workspace

Root `pnpm-workspace.yaml` includes:

```yaml
packages:
  - 'napi/*'
  - 'packages/*'
  - 'tasks/*'
  - 'wasm/*'
```

The published npm packages are:

- `ox-jsdoc` — canonical Binary AST NAPI binding
- `@ox-jsdoc/wasm` — canonical Binary AST WASM binding
- `@ox-jsdoc/decoder` — shared lazy decoder
- `@ox-jsdoc/binding-*` — canonical NAPI platform binding (auto-generated)
- `ox-jsdoc-binary` / `@ox-jsdoc/wasm-binary` — JS-only thin aliases (one deprecation cycle)

The `origin`-line packages (`ox-jsdoc-origin`, `@ox-jsdoc/wasm-origin`) carry `"private": true` and are not published.

## Rust Core Crates

### `crates/ox_jsdoc` — canonical (Binary AST)

- Public Rust API: `parse`, `parse_into`, `parse_batch`, `parse_batch_into`, `parse_to_bytes`, `parse_batch_to_bytes`, `parse_type_expression`, `parse_type_check`
- Module layout: `decoder/`, `format/`, `parser/`, `writer/`
- Exposes the Rust-side lazy decoder API (`LazyJsdocBlock`, `LazySourceFile`, etc.) for Rust walkers
- Emits the wire-compatible Binary AST byte stream consumed by `@ox-jsdoc/decoder` and the NAPI / WASM bindings

### `crates/ox_jsdoc_origin` — typed AST reference

- Public Rust API: `parse_comment`, `parse_type`, `validate_comment`, `analyze_comment`, `serialize_comment_json_with_options`
- Module layout: `ast.rs`, `parser/`, `analyzer/`, `serializer/`, `type_parser/`, `validator/`
- Preserved as the original v1 implementation for benchmark / reference comparison only
- Not depended on by canonical NAPI / WASM bindings or by `@ox-jsdoc/jsdoccomment`

## JavaScript Packages

### Canonical NAPI / WASM

- `napi/ox-jsdoc` (npm: `ox-jsdoc`) — Binary AST NAPI binding using `@ox-jsdoc/binding-*` platform packages
- `wasm/ox-jsdoc` (npm: `@ox-jsdoc/wasm`) — Binary AST WASM binding via `wasm-pack`
- Both call into `ox_jsdoc` (canonical Rust crate) and emit Binary AST bytes that `@ox-jsdoc/decoder` lazily decodes

### Origin NAPI / WASM (private)

- `napi/ox-jsdoc-origin` (npm: `ox-jsdoc-origin`, private) — typed AST NAPI binding for benchmark / reference
- `wasm/ox-jsdoc-origin` (npm: `@ox-jsdoc/wasm-origin`, private) — typed AST WASM binding
- Both call into `ox_jsdoc_origin` and emit JSON-serialized AST

### Deprecated JS-only aliases

- `napi/ox-jsdoc-binary` (npm: `ox-jsdoc-binary`) — re-exports `ox-jsdoc`
- `wasm/ox-jsdoc-binary` (npm: `@ox-jsdoc/wasm-binary`) — re-exports `@ox-jsdoc/wasm`
- Both are pure JS re-exports without separate native artifacts; kept for one deprecation cycle to preserve a migration path for previously-published `0.0.12` consumers

## Fixtures

Performance fixtures live at repository level:

```text
fixtures/perf/
```

Both Rust benchmarks and Node.js benchmarks reuse the same fixture corpus. Sidecar JSON metadata describes each fixture:

```text
fixtures/perf/malformed/unclosed-inline-tag.jsdoc
fixtures/perf/malformed/unclosed-inline-tag.json
```

The `.jsdoc` file is exact parser input. The `.json` file is metadata and expected behavior.

## Benchmarks

Benchmarks live under `tasks/benchmark/` and use:

- `criterion2` for Rust-direct measurements (per-comment parse / batch parse / writer-reuse / typed AST vs Binary AST cost split)
- `mitata` for in-process Node.js parser-only measurements
- `hyperfine` for end-to-end CLI linter measurements

Benchmark scripts depend on both canonical (`ox-jsdoc`, `@ox-jsdoc/wasm`) and origin (`ox-jsdoc-origin`, `@ox-jsdoc/wasm-origin`) packages so the typed AST vs Binary AST comparison stays measurable.

## Relationship to `refers/`

`refers/` contains git submodules used for research and compatibility reference. It is not part of either workspace.

Reference sources may be used to derive fixtures, but the benchmark fixture corpus lives under `fixtures/perf/` so it remains stable even if submodule contents change.

## Decision Summary

Use this layout:

- Canonical Rust core: `crates/ox_jsdoc` (Binary AST)
- Origin Rust core: `crates/ox_jsdoc_origin` (typed AST, benchmark / reference only)
- Canonical NAPI: `napi/ox-jsdoc` (npm `ox-jsdoc`)
- Canonical WASM: `wasm/ox-jsdoc` (npm `@ox-jsdoc/wasm`)
- Origin NAPI / WASM: `napi/ox-jsdoc-origin` / `wasm/ox-jsdoc-origin` (private)
- Deprecated JS-only aliases: `napi/ox-jsdoc-binary` / `wasm/ox-jsdoc-binary` (one deprecation cycle)
- Shared decoder: `packages/decoder` (npm `@ox-jsdoc/decoder`)
- Internal integrations: `packages/jsdoccomment`, `packages/eslint-plugin-jsdoc` (private)
- Benchmarks: `tasks/benchmark`
- Fixtures: `fixtures/perf`
- `refers/*` outside Rust and pnpm workspaces

This layout keeps the canonical Binary AST core independent, preserves the typed AST reference under `origin`, and provides a one-deprecation-cycle alias surface for users migrating from the previous `ox-jsdoc-binary` / `@ox-jsdoc/wasm-binary` packages.
