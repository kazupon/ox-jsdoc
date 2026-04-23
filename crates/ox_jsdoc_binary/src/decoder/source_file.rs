// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! [`LazySourceFile`] — root of the lazy decoder.
//!
//! See `design/007-binary-ast/rust-impl.md#lazysourcefile-root-of-the-decoder`.
//!
//! The struct caches the Header offsets/counts at construction so every
//! lazy node can reach the String table or Root array in O(1) without
//! re-parsing the Header.

use crate::format::header::{
    self, COMPAT_MODE_BIT, DIAGNOSTICS_OFFSET_FIELD, EXTENDED_DATA_OFFSET_FIELD, FLAGS_OFFSET,
    HEADER_SIZE, NODE_COUNT_FIELD, NODES_OFFSET_FIELD, ROOT_ARRAY_OFFSET_FIELD, ROOT_COUNT_FIELD,
    STRING_DATA_OFFSET_FIELD, STRING_OFFSETS_OFFSET_FIELD,
};
use crate::format::node_record::STRING_PAYLOAD_NONE_SENTINEL;
use crate::format::root_index::{BASE_OFFSET_FIELD, NODE_INDEX_OFFSET, ROOT_INDEX_ENTRY_SIZE};
use crate::format::string_field::StringField;
use crate::format::string_table::{STRING_OFFSET_ENTRY_SIZE, U16_NONE_SENTINEL};

use super::error::DecodeError;
use super::helpers::read_u32;
use super::nodes::LazyNode;
use super::nodes::comment_ast::LazyJsdocBlock;

/// Lazy decoder root. Holds the underlying byte slice plus all Header-derived
/// offsets/counts.
///
/// `#[derive(Copy, Clone)]` so that lazy node structs can store
/// `&'a LazySourceFile<'a>` (already a stack value) and pass it around by
/// value without cost.
#[derive(Debug, Clone, Copy)]
pub struct LazySourceFile<'a> {
    pub(crate) bytes: &'a [u8],
    /// Whether the buffer's `compat_mode` flag bit is set.
    pub compat_mode: bool,
    /// Byte offset of the Root index array within `bytes`.
    pub root_array_offset: u32,
    /// Byte offset of the String Offsets section. Indexed by
    /// `StringIndex` for string-leaf nodes and the diagnostics section's
    /// `message_index`.
    pub string_offsets_offset: u32,
    /// Byte offset of the String Data section. Both string-leaf
    /// (offsets-table indirection) and Extended Data
    /// [`crate::format::string_field::StringField`] slots resolve into
    /// this section's raw UTF-8 bytes.
    pub string_data_offset: u32,
    /// Byte offset of the Extended Data section.
    pub extended_data_offset: u32,
    /// Byte offset of the Diagnostics section.
    pub diagnostics_offset: u32,
    /// Byte offset of the Nodes section.
    pub nodes_offset: u32,
    /// Total node count (including the `node[0]` sentinel).
    pub node_count: u32,
    /// Number of roots N stored in this batch buffer.
    pub root_count: u32,
}

impl<'a> LazySourceFile<'a> {
    /// Parse the 40-byte Header from `bytes` and construct a [`LazySourceFile`].
    ///
    /// Returns [`DecodeError::TooShort`] when the slice cannot fit a Header,
    /// and [`DecodeError::IncompatibleMajor`] when the buffer's major version
    /// disagrees with [`crate::format::header::SUPPORTED_MAJOR`]. Decoders
    /// silently accept buffers with a newer minor version (forward
    /// compatibility) — Phase 1.1a is the first version, so the only valid
    /// value is `0`.
    pub fn new(bytes: &'a [u8]) -> Result<Self, DecodeError> {
        if bytes.len() < HEADER_SIZE {
            return Err(DecodeError::TooShort {
                actual: bytes.len(),
                required: HEADER_SIZE,
            });
        }
        let version_byte = bytes[0];
        let major = header::major(version_byte);
        if major != header::SUPPORTED_MAJOR {
            return Err(DecodeError::IncompatibleMajor {
                buffer_major: major,
                decoder_major: header::SUPPORTED_MAJOR,
            });
        }
        let flags = bytes[FLAGS_OFFSET];
        Ok(LazySourceFile {
            bytes,
            compat_mode: (flags & COMPAT_MODE_BIT) != 0,
            root_array_offset: read_u32(bytes, ROOT_ARRAY_OFFSET_FIELD),
            string_offsets_offset: read_u32(bytes, STRING_OFFSETS_OFFSET_FIELD),
            string_data_offset: read_u32(bytes, STRING_DATA_OFFSET_FIELD),
            extended_data_offset: read_u32(bytes, EXTENDED_DATA_OFFSET_FIELD),
            diagnostics_offset: read_u32(bytes, DIAGNOSTICS_OFFSET_FIELD),
            nodes_offset: read_u32(bytes, NODES_OFFSET_FIELD),
            node_count: read_u32(bytes, NODE_COUNT_FIELD),
            root_count: read_u32(bytes, ROOT_COUNT_FIELD),
        })
    }

    /// Borrow the underlying byte slice. Useful for advanced consumers that
    /// need raw access (e.g. exporting the buffer over IPC).
    #[inline]
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Resolve the string at `idx` (None when `idx` is the u16 None
    /// sentinel `0xFFFF` or the 30-bit `0x3FFF_FFFF`). Used by
    /// string-leaf nodes (`TypeTag::String` payload) and the diagnostics
    /// section.
    ///
    /// Performs a zero-copy slice from String Data via the offsets table.
    #[must_use]
    pub fn get_string(&self, idx: u32) -> Option<&'a str> {
        if idx == STRING_PAYLOAD_NONE_SENTINEL || idx == U16_NONE_SENTINEL as u32 {
            return None;
        }
        let so_offset =
            self.string_offsets_offset as usize + idx as usize * STRING_OFFSET_ENTRY_SIZE;
        let start = read_u32(self.bytes, so_offset) as usize;
        let end = read_u32(self.bytes, so_offset + 4) as usize;
        let sd_offset = self.string_data_offset as usize;
        let slice = &self.bytes[sd_offset + start..sd_offset + end];
        // SAFETY: Phase 1 writers only accept `&str` inputs and feed them
        // verbatim into String Data, so the slice is guaranteed UTF-8.
        Some(unsafe { core::str::from_utf8_unchecked(slice) })
    }

    /// Resolve a [`StringField`] into the underlying string (`None` when
    /// the field equals [`StringField::NONE`]).
    ///
    /// Performs a zero-copy slice from String Data; the returned `&str`
    /// borrows directly from the buffer. Used by Extended Data string
    /// slots which embed `(offset, length)` directly without going
    /// through the offsets table.
    #[inline]
    #[must_use]
    pub fn get_string_by_field(&self, field: StringField) -> Option<&'a str> {
        if field.is_none() {
            return None;
        }
        let sd_offset = self.string_data_offset as usize;
        let start = sd_offset + field.offset as usize;
        let end = start + field.length as usize;
        let slice = &self.bytes[start..end];
        // SAFETY: writers only accept `&str` inputs and feed them
        // verbatim into String Data, so the slice is guaranteed UTF-8.
        Some(unsafe { core::str::from_utf8_unchecked(slice) })
    }

    /// Required-string variant of [`Self::get_string_by_field`]. Panics on
    /// the [`StringField::NONE`] sentinel — callers must guarantee the
    /// field is non-NONE (used for required-by-spec slots like
    /// `JsdocBlock.delimiter`).
    #[inline]
    #[must_use]
    pub fn get_required_string_by_field(&self, field: StringField) -> &'a str {
        debug_assert!(
            !field.is_none(),
            "get_required_string_by_field called with StringField::NONE"
        );
        let sd_offset = self.string_data_offset as usize;
        let start = sd_offset + field.offset as usize;
        let end = start + field.length as usize;
        let slice = &self.bytes[start..end];
        // SAFETY: see `get_string_by_field`.
        unsafe { core::str::from_utf8_unchecked(slice) }
    }

    /// Get the `base_offset` (original-file absolute byte position) for
    /// root index `root_index`. Used by lazy nodes when computing the
    /// `range` getter.
    #[must_use]
    pub fn get_root_base_offset(&self, root_index: u32) -> u32 {
        let off = self.root_array_offset as usize
            + root_index as usize * ROOT_INDEX_ENTRY_SIZE
            + BASE_OFFSET_FIELD;
        read_u32(self.bytes, off)
    }

    /// Iterate over the AST root for each entry in the Root index array.
    ///
    /// Yields `None` for entries whose `node_index = 0` (parse failure
    /// sentinel) and `Some(LazyJsdocBlock)` for successful parses.
    pub fn asts(&'a self) -> AstsIter<'a> {
        AstsIter {
            source_file: self,
            cursor: 0,
        }
    }
}

/// Iterator returned by [`LazySourceFile::asts`].
#[derive(Debug)]
pub struct AstsIter<'a> {
    source_file: &'a LazySourceFile<'a>,
    cursor: u32,
}

impl<'a> Iterator for AstsIter<'a> {
    type Item = Option<LazyJsdocBlock<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.source_file.root_count {
            return None;
        }
        let root_index = self.cursor;
        let off = self.source_file.root_array_offset as usize
            + root_index as usize * ROOT_INDEX_ENTRY_SIZE
            + NODE_INDEX_OFFSET;
        let node_index = read_u32(self.source_file.bytes, off);
        self.cursor += 1;
        if node_index == 0 {
            // Parse failure sentinel.
            Some(None)
        } else {
            Some(Some(LazyJsdocBlock::from_index(
                self.source_file,
                node_index,
                root_index,
            )))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.source_file.root_count - self.cursor) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for AstsIter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn lazy_source_file_is_compact() {
        // Concrete size depends on the field layout, but it must comfortably
        // fit in 64 bytes so it can sit on the stack with no heap pressure.
        assert!(size_of::<LazySourceFile<'static>>() <= 64);
    }
}
