# ox_jsdoc

[![Crates.io](https://img.shields.io/crates/v/ox_jsdoc.svg)](https://crates.io/crates/ox_jsdoc)
[![Docs.rs](https://docs.rs/ox_jsdoc/badge.svg)](https://docs.rs/ox_jsdoc)
[![License](https://img.shields.io/crates/l/ox_jsdoc.svg)](https://opensource.org/licenses/MIT)

High-performance JSDoc parser with a parser-integrated Binary AST writer and a lazy decoder, inspired by the [`oxc`](https://github.com/oxc-project/oxc) project.

`ox_jsdoc` parses `/** ... */` comment blocks on the Rust side and emits a compact byte stream (the Binary AST). The same crate provides a Rust-side lazy decoder so callers can walk the tree without materializing every node up front. The byte stream is also the format consumed by the JS-side `@ox-jsdoc/decoder` package, making the same parser usable from Rust, Node.js (via `ox-jsdoc`), and the browser (via `@ox-jsdoc/wasm`).

## Features

- **Binary AST** — parser-integrated writer that produces a self-contained, 8-byte-aligned byte stream with shared string interning across batched comments
- **Lazy Rust-side decoder** — walk the AST on demand (`LazyJsdocBlock`, `LazySourceFile`, …) without eagerly building intermediate node objects
- **Batch parsing** — amortize parser setup, intern common strings (`*`, `*/`, tag names) once across many comments
- **Standalone type expression parser** — `parse_type_expression` / `parse_type_check` for parsing JSDoc type strings without a surrounding comment
- **`compat_mode`** — opt-in `@es-joy/jsdoccomment`-compatible output for downstream tooling

## Installation

```toml
[dependencies]
ox_jsdoc = "0.0.13"
oxc_allocator = "0.123"
```

## Quick start

### Parse a single block

```rust
use oxc_allocator::Allocator;
use ox_jsdoc::parser::{parse, ParseOptions};

let arena = Allocator::default();
let result = parse(
    &arena,
    "/** @param {string} id - The user ID */",
    ParseOptions::default(),
);

let root = result.lazy_root.expect("parsed");
let tag = root.tags().next().expect("tag");
assert_eq!(tag.tag().value(), "param");
assert_eq!(tag.description(), Some("The user ID"));
assert!(result.diagnostics.is_empty());
```

### Parse a batch of blocks

```rust
use oxc_allocator::Allocator;
use ox_jsdoc::parser::{parse_batch, BatchItem, ParseOptions};

let arena = Allocator::default();
let items = [
    BatchItem { source_text: "/** @param {string} a */", base_offset: 0 },
    BatchItem { source_text: "/** @returns {void} */",   base_offset: 100 },
];
let result = parse_batch(&arena, &items, ParseOptions::default());

for (i, root) in result.lazy_roots.iter().enumerate() {
    let Some(root) = root else {
        // parse failure for items[i]; check `result.diagnostics`.
        continue;
    };
    if let Some(tag) = root.tags().next() {
        println!("item {i}: @{}", tag.tag().value());
    }
}
```

### Reuse a `BinaryWriter`

For hot loops (lint runners, watch mode), construct a `BinaryWriter` once and reuse it across calls so the per-call writer setup cost (string-table prelude, arena buffer init) is amortized:

```rust
use oxc_allocator::Allocator;
use ox_jsdoc::parser::{parse_into, ParseOptions};
use ox_jsdoc::writer::BinaryWriter;

let arena = Allocator::default();
let mut writer = BinaryWriter::new(&arena);

for src in ["/** ok */", "/** @param {string} id */"] {
    let result = parse_into(&arena, src, ParseOptions::default(), &mut writer);
    let _ = result.lazy_root;
}
```

The matching batch entry is `parse_batch_into`.

### Bytes-only API (binding-friendly)

`parse_to_bytes` and `parse_batch_to_bytes` skip the arena round-trip and return an owned `Vec<u8>` directly. This is the API the NAPI / WASM bindings consume so the bytes can be moved into a `Uint8Array` without an extra copy:

```rust
use ox_jsdoc::parser::{parse_to_bytes, ParseOptions};

let result = parse_to_bytes("/** ok */", ParseOptions::default());
let _bytes: Vec<u8> = result.binary_bytes;
// `bytes` is the canonical Binary AST byte stream — the JS-side
// `@ox-jsdoc/decoder` package can lazily read it.
```

### Standalone type expressions

```rust
use ox_jsdoc::parser::{parse_type_check, parse_type_expression, type_data::ParseMode};

assert_eq!(
    parse_type_expression("string | number", ParseMode::Typescript),
    Some("string | number".to_string()),
);
assert!(parse_type_check("Array<string>", ParseMode::Typescript));
assert!(!parse_type_check("not a type {{", ParseMode::Jsdoc));
```

## Public API map

| Module / item | Purpose |
| --- | --- |
| `parser::parse` / `parse_into` | Parse one comment, return a `ParseResult` (lazy root + diagnostics + source file). `parse_into` reuses a caller-supplied `BinaryWriter`. |
| `parser::parse_to_bytes` | Per-comment bytes-only entry point (`Vec<u8>` + diagnostics). Used by NAPI / WASM bindings. |
| `parser::parse_batch` / `parse_batch_into` | Parse N comments into one shared Binary AST buffer + lazy roots. |
| `parser::parse_batch_to_bytes` | Per-batch bytes-only entry point. |
| `parser::parse_type_expression` / `parse_type_check` | Parse a standalone type expression (no surrounding comment). |
| `parser::ParseOptions` | `compat_mode`, `parse_types`, `type_parse_mode`, `preserve_whitespace`, `fence_aware`, `base_offset`. |
| `decoder::source_file::LazySourceFile` | Wraps the Binary AST byte buffer and exposes the cached header. |
| `decoder::nodes::comment_ast::LazyJsdocBlock` | Lazy root accessors (`tags()`, `description()`, `inline_tags()`, …). |
| `writer::BinaryWriter` | Re-usable writer; arena-backed, shared by `parse_into` / `parse_batch_into`. |
| `format::*` | Wire-format constants (header, node record layout, kind tags, extended-data). |

See the full [API documentation on docs.rs](https://docs.rs/ox_jsdoc) for the lazy decoder node hierarchy and per-node accessors.

## Design

`ox_jsdoc` is the canonical Rust core of the [ox-jsdoc](https://github.com/kazupon/ox-jsdoc) workspace, which also publishes the corresponding NAPI binding (`ox-jsdoc` on npm) and WASM binding (`@ox-jsdoc/wasm` on npm). All three share this crate's parser, byte format, and decoder.

The design rationale (Binary AST format, lazy decoder, batch sharing, NAPI / WASM transport, jsdoccomment compatibility) lives under [`design/`](https://github.com/kazupon/ox-jsdoc/tree/main/design):

- [`design/007-binary-ast/`](https://github.com/kazupon/ox-jsdoc/tree/main/design/007-binary-ast) — Binary AST format and decoder design
- [`design/008-oxlint-oxfmt-support/`](https://github.com/kazupon/ox-jsdoc/tree/main/design/008-oxlint-oxfmt-support) — `compat_mode` / `preserve_whitespace` for downstream tooling
- [`design/010-main-stream-binary/`](https://github.com/kazupon/ox-jsdoc/tree/main/design/010-main-stream-binary) — post-cutover migration that promoted the Binary AST to canonical
- [`design/009-jsdoc-linter-benchmark/`](https://github.com/kazupon/ox-jsdoc/tree/main/design/009-jsdoc-linter-benchmark) — benchmark methodology and the Rust-direct measurement layer

The original typed AST + JSON implementation is preserved as the workspace-internal `ox_jsdoc_origin` crate (`publish = false`) for benchmark / reference comparison only.

## Minimum Supported Rust Version (MSRV)

Tracks the workspace `rust-version`. See the workspace [`Cargo.toml`](https://github.com/kazupon/ox-jsdoc/blob/main/Cargo.toml).

## License

[MIT](https://opensource.org/licenses/MIT)
