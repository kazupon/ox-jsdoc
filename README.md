# ox-jsdoc

High-performance JSDoc parser inspired by the `oxc` project.

## Status

This repository is in the initial implementation phase.

Current scope:

- Rust workspace scaffold
- `ox_jsdoc` core parser crate skeleton
- `xtask` development tasks
- performance fixture buckets

The parser currently exposes the initial API shape and only implements minimal
JSDoc block boundary checks. Full tag parsing, validation, analysis, serializer,
and JavaScript / NAPI bindings are still planned work.

## Repository Layout

```text
crates/ox_jsdoc      Rust core parser crate
tasks/xtask          Repository development tasks
fixtures/perf        Performance and parser fixture buckets
design/              Design documents
refers/              Reference implementations managed as git submodules
```

Start from [design/index.md](design/index.md) for the design document table of
contents.

## Development

This repository uses Vite+ as the task runner. Install `vp` before running the
project tasks:

```sh
curl -fsSL https://vite.plus | bash
```

Common commands:

```sh
vp run fmt
vp run check
vp run test
```

`vp run check` runs the Rust license-header task and `cargo check`.
The header task checks Rust sources for:

- non-empty `@author`
- `@license MIT`

The first run builds the local `xtask` crate automatically through Cargo. You
can also run the task directly:

```sh
cargo run -p xtask -- headers:check
```

Rust commands can be run directly as well:

```sh
cargo fmt --check
cargo check
cargo test
```
