// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Binary AST for ox_jsdoc.
//!
//! This crate hosts the Binary AST format specification ([`format`]), the
//! parser-integrated binary writer ([`writer`]), the parser entry point
//! ([`parser`]) and the Rust-side lazy decoder ([`decoder`]).
//!
//! Node positions (Pos / End) are UTF-8 byte offsets relative to the root's
//! source text, the parser's spans as they are. Hosts that need UTF-16 code
//! units (JavaScript string indices, ESLint, LSP) convert them with the
//! original source text.
//!
//! The Binary AST replaces the previous JSON serialization path between the
//! Rust parser and JS bindings. The full design lives under
//! `design/007-binary-ast/`. The format specification itself is in
//! `design/007-binary-ast/format.md`; this crate is the Rust reference
//! implementation for that spec.

pub mod decoder;
pub mod format;
pub mod parser;
pub mod writer;
