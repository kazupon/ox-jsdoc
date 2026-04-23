// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Binary AST writer (parser-integrated, approach c-1).
//!
//! See `design/007-binary-ast/rust-impl.md#parser-integrated-binary-writer`
//! for the design rationale.
//!
//! The writer is invoked from inside the parser as it produces nodes. It
//! emits Binary AST bytes directly into an arena-backed buffer per section
//! ([`BinaryWriter`]), then concatenates them into the final
//! `Vec<u8>` at [`BinaryWriter::finish`]. There is no separate encoder pass;
//! parsing and encoding happen in lockstep.
//!
//! This module is currently a **Phase 1.0b skeleton**: the public surface
//! (struct, types, signatures) is in place but every implementation body is
//! `unimplemented!()`. The first real bodies land in Phase 1.1a.

mod binary_writer;
mod extended_data;
pub mod nodes;
mod string_table;

pub use crate::format::string_field::StringField;
pub use binary_writer::{BinaryWriter, ListInProgress};
pub use extended_data::{ExtOffset, ExtendedDataBuilder};
pub use nodes::NodeIndex;
pub use string_table::{
    COMMON_CRLF, COMMON_EMPTY, COMMON_LF, COMMON_SLASH_STAR, COMMON_SLASH_STAR_STAR, COMMON_SPACE,
    COMMON_STAR, COMMON_TAB, LeafStringPayload, StringIndex, StringTableBuilder,
    common_string_field,
};
