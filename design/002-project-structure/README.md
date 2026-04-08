# Project Structure

This document describes the repository layout to use before the first
implementation of `ox-jsdoc`.

The project needs two things at the same time:

1. a Rust implementation that can be developed and benchmarked independently
2. a JavaScript package that can expose the parser to JS users

The structure should keep these concerns separate without making the initial
repository too large.

## Goals

- Keep the parser core as a normal Rust library.
- Keep the JavaScript package as a workspace package.
- Keep NAPI / JS transfer code outside the core parser crate.
- Keep performance fixtures and benchmarks at repository level.
- Leave room for validator, analyzer, serializer, and future toolchain
  integration without restructuring the repository.

## Non-goals for the Initial Layout

- Do not introduce raw transfer as a core design requirement.
- Do not split the Rust parser into many crates before the first parser exists.
- Do not publish multiple npm packages until the binding strategy requires it.
- Do not make `refers/` part of the workspace.
- Do not put benchmark fixtures inside a package-specific directory.

## Options Considered

### Option A. Root package only

Shape:

```text
Cargo.toml
package.json
src/
```

Pros:

- smallest possible repository structure
- low initial setup cost

Cons:

- Rust parser, NAPI binding, JS package, and benchmarks become entangled
- hard to keep core parser independent from JS transfer concerns
- does not match the future validator / analyzer / serializer split

Decision:

- do not use this layout

### Option B. Public JavaScript package under `napi/`

Shape:

```text
crates/
  ox_jsdoc/
napi/
  ox-jsdoc/
```

Pros:

- close to the `oxc` layout for Node-facing native packages
- keeps Rust core under `crates/`
- keeps NAPI / JS transfer layer outside the core parser crate
- one public npm package can be managed from the NAPI package directory
- avoids adding an extra JS wrapper package before it is needed

Cons:

- the public JavaScript package lives under `napi/` rather than `packages/`
- if a pure JS wrapper grows substantially, it may later need a separate
  `packages/ox-jsdoc` package

Decision:

- use this layout for v1

### Option C. Separate `packages/ox-jsdoc` wrapper plus `napi/` binding package

Shape:

```text
packages/
  ox-jsdoc/
napi/
  ox-jsdoc/
```

Pros:

- clear `packages/` directory for JavaScript-facing packages
- leaves room for a pure JS wrapper independent from native build details
- useful if multiple binding packages or platform packages are introduced

Cons:

- adds an extra package boundary before it is necessary
- requires deciding how `packages/ox-jsdoc` consumes the native binding
- increases build and publish complexity for the initial parser

Decision:

- defer until the JS wrapper needs to grow beyond the NAPI package

## Recommended v1 Layout

```text
.
├── Cargo.toml
├── package.json
├── pnpm-workspace.yaml
├── rust-toolchain.toml
├── crates/
│   └── ox_jsdoc/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── ast.rs
│           ├── parser/
│           │   ├── mod.rs
│           │   ├── context.rs
│           │   ├── checkpoint.rs
│           │   ├── diagnostics.rs
│           │   └── scanner.rs
│           ├── validator/
│           │   └── mod.rs
│           ├── analyzer/
│           │   └── mod.rs
│           └── serializer/
│               ├── mod.rs
│               └── json.rs
├── napi/
│   └── ox-jsdoc/
│       ├── Cargo.toml
│       ├── package.json
│       ├── build.rs
│       ├── src/
│       │   └── lib.rs
│       └── src-js/
│           ├── index.js
│           └── index.d.ts
├── tasks/
│   └── benchmark/
│       ├── Cargo.toml
│       └── benches/
│           ├── parser.rs
│           ├── validator.rs
│           └── serializer.rs
├── fixtures/
│   └── perf/
│       ├── common/
│       ├── description-heavy/
│       ├── type-heavy/
│       ├── special-tag/
│       ├── malformed/
│       └── toolchain/
├── design/
├── .notes/
└── refers/
```

## Workspace Boundaries

### Cargo workspace

Root `Cargo.toml` should define a Rust workspace:

```toml
[workspace]
resolver = "3"
members = [
  "crates/*",
  "napi/*",
  "tasks/*",
]
exclude = [
  "refers/*",
]
```

The initial workspace crates should be:

- `crates/ox_jsdoc`
  - pure Rust core parser crate
  - owns AST, parser, validator stub, analyzer stub, JSON serializer
- `napi/ox-jsdoc`
  - NAPI binding crate and JavaScript package
  - depends on `ox_jsdoc`
  - owns JS transfer and package entrypoint
- `tasks/benchmark`
  - benchmark crate using `criterion2`
  - depends on `ox_jsdoc`

### pnpm workspace

Root `package.json` should be private.
Root `pnpm-workspace.yaml` should include JavaScript-facing package directories:

```yaml
packages:
  - "napi/*"
  - "packages/*"
```

`packages/*` is included from the beginning so a future pure JS wrapper can be
added without changing the workspace shape.
It does not need to exist in v1.

## Rust Core Crate

`crates/ox_jsdoc` is the core implementation crate.

It should not depend on NAPI.
It should expose:

- `parse_comment`
- `ParseOptions`
- `ParseOutput`
- AST types
- parser diagnostics helpers
- validator / analyzer / serializer stubs as they become available

Initial module responsibilities:

- `ast.rs`
  - Rust AST definitions aligned with `design/ast.md`
- `parser/context.rs`
  - `ParserContext<'a>`
- `parser/checkpoint.rs`
  - rollback checkpoint type
- `parser/diagnostics.rs`
  - v1 parser diagnostic constructors
- `parser/scanner.rs`
  - internal scanner helpers, not public API
- `parser/mod.rs`
  - `parse_comment` entrypoint
- `validator/mod.rs`
  - stub for tag-specific validation
- `analyzer/mod.rs`
  - stub for consumer-facing facts
- `serializer/json.rs`
  - JSON-oriented shape, not raw transfer

## JavaScript Package

`napi/ox-jsdoc` should be the initial JavaScript package.

It should own:

- `package.json`
- NAPI build configuration
- `src-js/index.js`
- `src-js/index.d.ts`
- Rust NAPI bridge in `src/lib.rs`

The public npm package can be named `ox-jsdoc` initially.
If a scoped package name is preferred later, that can be changed before the first
publish.

The NAPI package should call into `ox_jsdoc`.
It should not duplicate parser logic.

Initial JS API should be small:

```ts
export function parseComment(sourceText: string, options?: ParseOptions): ParseOutput
```

The JS package should remain JSON-first.
Raw transfer support should not affect the initial package layout.

## Fixtures

Performance fixtures should live at repository level:

```text
fixtures/perf/
```

They should not live under `crates/` or `napi/`.
Both Rust benchmarks and future JS/toolchain benchmarks should be able to reuse
the same fixture corpus.

Use sidecar JSON metadata:

```text
fixtures/perf/malformed/unclosed-inline-tag.jsdoc
fixtures/perf/malformed/unclosed-inline-tag.json
```

The `.jsdoc` file is exact parser input.
The `.json` file is metadata and expected behavior.

## Benchmarks

Benchmarks should start under:

```text
tasks/benchmark/
```

This keeps benchmarks out of the core crate while still allowing them to depend
on workspace crates.

Initial benchmark framework:

- `criterion2`

Initial benchmark targets:

- `parser.rs`
- `validator.rs`
- `serializer.rs`

CodSpeed integration should be deferred until benchmark names and fixture buckets
are stable.

## Relationship to `refers/`

`refers/` contains git submodules used for research and compatibility reference.
It should not be part of either workspace.

Reference sources may be used to derive fixtures, but the benchmark fixture
corpus should live under `fixtures/perf/` so it remains stable even if submodule
contents change.

## Decision Summary

Use this v1 structure:

- Rust core: `crates/ox_jsdoc`
- JS/NAPI package: `napi/ox-jsdoc`
- Benchmarks: `tasks/benchmark`
- Fixtures: `fixtures/perf`
- Keep `packages/*` reserved for future JS wrapper packages
- Keep `refers/*` outside Rust and pnpm workspaces

This layout keeps the core parser independent, keeps the JS package manageable,
and leaves enough room for the validator / analyzer / serializer pipeline
without over-splitting the repository before the first implementation exists.
