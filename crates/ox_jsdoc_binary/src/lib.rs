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
//! The crate is currently in **Phase 1.0a** (skeleton construction): only the
//! `format` module is populated, and even that only provides type definitions
//! and layout constants. Encoder, decoder, parser, and visitor modules will
//! land in subsequent sub-phases.

#![cfg_attr(not(test), no_std)]

pub mod format;
