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
//! Phase **1.1c** complete: the `format`, `writer`, and `decoder` modules
//! ship working implementations including the [`decoder::LazyJsdocVisitor`]
//! depth-first walker. The `parser` module is still skeletal; real bodies
//! land in Phase 1.2a.

pub mod decoder;
pub mod format;
pub mod parser;
pub mod writer;
