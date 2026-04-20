//! Binary AST for ox_jsdoc.
//!
//! This crate hosts the Binary AST format specification (`format`), the
//! parser-integrated binary writer (`writer`, Phase 1.1a), the parser entry
//! point (`parser`, Phase 1.2a), and the Rust-side lazy decoder
//! (`decoder` + [`visitor`], Phase 1.1b/c).
//!
//! The Binary AST replaces the previous JSON serialization path between the
//! Rust parser and JS bindings. The full design lives under
//! `design/007-binary-ast/`. The format specification itself is in
//! `design/007-binary-ast/format.md`; this crate aims to be the single Rust
//! reference implementation for that spec.
//!
//! The crate is currently in **Phase 1.0b** (skeleton construction): the
//! `format` module is fully populated with layout constants, and the
//! `writer` module exposes the public surface (struct, function signatures)
//! with `unimplemented!()` bodies. Decoder, parser, and visitor modules will
//! land in subsequent sub-phases.

pub mod format;
pub mod writer;
