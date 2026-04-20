//! Rust-side lazy decoder.
//!
//! See `design/007-binary-ast/rust-impl.md#rust-side-lazy-decoder` and
//! `js-decoder.md` (the JS counterpart) for the design rationale.
//!
//! The decoder is **stateless** with respect to the input bytes: every
//! lazy node struct is `Copy` and just remembers `(source_file, node_index)`.
//! Reads happen on demand from the underlying `&[u8]`.
//!
//! Module layout (Phase 1.0c skeleton):
//!
//! - [`error`]: [`error::DecodeError`] enum.
//! - [`source_file`]: [`source_file::LazySourceFile`] (decoder root).
//! - [`helpers`]: shared low-level read helpers.
//! - [`nodes`]: lazy node structs (15 comment AST + 45 TypeNode = 60).
//!
//! All method bodies are `todo!()` placeholders. Phase 1.1b fills them in.

pub mod error;
pub mod helpers;
pub mod nodes;
pub mod source_file;

pub use error::DecodeError;
pub use nodes::{LazyNode, NodeListIter};
pub use source_file::LazySourceFile;
