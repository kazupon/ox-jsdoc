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
use crate::format::node_record::{NODE_RECORD_SIZE, STRING_PAYLOAD_NONE_SENTINEL};
use crate::format::root_index::{
    BASE_OFFSET_FIELD, NODE_INDEX_OFFSET, ROOT_INDEX_ENTRY_SIZE, SOURCE_OFFSET_FIELD,
};
use crate::format::string_field::StringField;
use crate::format::string_table::STRING_OFFSET_ENTRY_SIZE;

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
///
/// Every string it gives is a slice of the String Data section, which is
/// checked to be UTF-8 when the file is made ([`Self::new`]), so no offset
/// read from the buffer or passed by the caller can give a `&str` that is
/// not UTF-8: an offset past the section or in the middle of a character
/// gives `None` (or `""`).
#[derive(Debug, Clone, Copy)]
pub struct LazySourceFile<'a> {
    pub(crate) bytes: &'a [u8],
    /// The String Data section (`string_data_offset..extended_data_offset`,
    /// with the padding before Extended Data), as UTF-8.
    string_data: &'a str,
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
    /// [`DecodeError::IncompatibleMajor`] when the buffer's major version
    /// disagrees with [`crate::format::header::SUPPORTED_MAJOR`],
    /// [`DecodeError::InvalidSectionLayout`] when the sections are out of
    /// order or do not fit in `bytes`, and [`DecodeError::InvalidStringData`]
    /// when the String Data section is not UTF-8. Decoders silently accept
    /// buffers with a newer minor version (forward compatibility). A newer
    /// minor version may add node kinds, which this decoder does not know.
    ///
    /// Checking the String Data takes time in proportion to its length. For
    /// a buffer that this crate's writer wrote, [`Self::new_unchecked`]
    /// skips that check.
    pub fn new(bytes: &'a [u8]) -> Result<Self, DecodeError> {
        let (mut file, string_data) = Self::read_header(bytes)?;
        file.string_data =
            core::str::from_utf8(string_data).map_err(|error| DecodeError::InvalidStringData {
                valid_up_to: error.valid_up_to(),
            })?;
        Ok(file)
    }

    /// [`Self::new`] without checking that the String Data section is UTF-8.
    /// The Header and the section layout are still checked.
    ///
    /// # Safety
    ///
    /// When the sections fit in `bytes`, the String Data section must be
    /// UTF-8. A buffer that this crate's writer wrote is: it builds the
    /// section from `&str` values only.
    pub unsafe fn new_unchecked(bytes: &'a [u8]) -> Result<Self, DecodeError> {
        let (mut file, string_data) = Self::read_header(bytes)?;
        // SAFETY: the caller guarantees that the String Data is UTF-8.
        file.string_data = unsafe { core::str::from_utf8_unchecked(string_data) };
        Ok(file)
    }

    /// Check the Header and the section layout, and give the file (with an
    /// empty `string_data`) and the bytes of the String Data section.
    fn read_header(bytes: &'a [u8]) -> Result<(Self, &'a [u8]), DecodeError> {
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
        let file = LazySourceFile {
            bytes,
            string_data: "",
            compat_mode: (flags & COMPAT_MODE_BIT) != 0,
            root_array_offset: read_u32(bytes, ROOT_ARRAY_OFFSET_FIELD),
            string_offsets_offset: read_u32(bytes, STRING_OFFSETS_OFFSET_FIELD),
            string_data_offset: read_u32(bytes, STRING_DATA_OFFSET_FIELD),
            extended_data_offset: read_u32(bytes, EXTENDED_DATA_OFFSET_FIELD),
            diagnostics_offset: read_u32(bytes, DIAGNOSTICS_OFFSET_FIELD),
            nodes_offset: read_u32(bytes, NODES_OFFSET_FIELD),
            node_count: read_u32(bytes, NODE_COUNT_FIELD),
            root_count: read_u32(bytes, ROOT_COUNT_FIELD),
        };
        file.check_layout()?;
        let string_data =
            &bytes[file.string_data_offset as usize..file.extended_data_offset as usize];
        Ok((file, string_data))
    }

    /// The sections come in the writer's order, each starts where the one
    /// before it may end, and the Root Index Array and the nodes fit.
    fn check_layout(&self) -> Result<(), DecodeError> {
        let roots_end = (self.root_count as usize)
            .checked_mul(ROOT_INDEX_ENTRY_SIZE)
            .and_then(|size| size.checked_add(self.root_array_offset as usize));
        let nodes_end = (self.node_count as usize)
            .checked_mul(NODE_RECORD_SIZE)
            .and_then(|size| size.checked_add(self.nodes_offset as usize));
        let at = |offset: u32| Some(offset as usize);
        // (section, where it starts or ends, the offset it must not pass)
        let checks = [
            (
                "Root Index Array",
                Some(HEADER_SIZE),
                self.root_array_offset,
            ),
            ("Root Index Array", roots_end, self.string_offsets_offset),
            (
                "String Offsets",
                at(self.string_offsets_offset),
                self.string_data_offset,
            ),
            (
                "String Data",
                at(self.string_data_offset),
                self.extended_data_offset,
            ),
            (
                "Extended Data",
                at(self.extended_data_offset),
                self.diagnostics_offset,
            ),
            (
                "Diagnostics",
                at(self.diagnostics_offset),
                self.nodes_offset,
            ),
        ];
        for (section, start, limit) in checks {
            if !start.is_some_and(|start| start <= limit as usize) {
                return Err(DecodeError::InvalidSectionLayout { section });
            }
        }
        if !nodes_end.is_some_and(|end| end <= self.bytes.len()) {
            return Err(DecodeError::InvalidSectionLayout { section: "Nodes" });
        }
        Ok(())
    }

    /// Borrow the underlying byte slice. Useful for advanced consumers that
    /// need raw access (e.g. exporting the buffer over IPC).
    #[inline]
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Resolve the string at `idx` (None when `idx` is the
    /// [`STRING_PAYLOAD_NONE_SENTINEL`] (`0x3FFF_FFFF`)). Used by
    /// string-leaf nodes (`TypeTag::String` payload) and the diagnostics
    /// section.
    ///
    /// Performs a zero-copy slice from String Data via the offsets table.
    /// `None` too when the offsets are past the section or in the middle of
    /// a character.
    #[must_use]
    pub fn get_string(&self, idx: u32) -> Option<&'a str> {
        if idx == STRING_PAYLOAD_NONE_SENTINEL {
            return None;
        }
        let so_offset =
            self.string_offsets_offset as usize + idx as usize * STRING_OFFSET_ENTRY_SIZE;
        let start = read_u32(self.bytes, so_offset) as usize;
        let end = read_u32(self.bytes, so_offset + 4) as usize;
        self.string_data.get(start..end)
    }

    /// Resolve a Path B-leaf inline `(offset, length)` pair into the
    /// underlying string slice (no NONE sentinel for inline payloads —
    /// encoders only emit StringInline when the string is real and short).
    /// `""` when the range is past the String Data or in the middle of a
    /// character.
    #[inline]
    #[must_use]
    pub fn get_inline_string(&self, offset: u32, length: u8) -> &'a str {
        self.string_at(offset, u32::from(length)).unwrap_or("")
    }

    /// Resolve a [`StringField`] into the underlying string (`None` when
    /// the field equals [`StringField::NONE`]).
    ///
    /// Performs a zero-copy slice from String Data; the returned `&str`
    /// borrows directly from the buffer. Used by Extended Data string
    /// slots which embed `(offset, length)` directly without going
    /// through the offsets table. `None` too when the range is past the
    /// section or in the middle of a character.
    #[inline]
    #[must_use]
    pub fn get_string_by_field(&self, field: StringField) -> Option<&'a str> {
        if field.is_none() {
            return None;
        }
        self.string_at(field.offset, u32::from(field.length))
    }

    /// Required-string variant of [`Self::get_string_by_field`]. Callers
    /// must guarantee the field is non-NONE (used for required-by-spec
    /// slots like `JsdocBlock.delimiter`); debug builds panic on the
    /// [`StringField::NONE`] sentinel. `""` when the range is past the
    /// String Data or in the middle of a character.
    #[inline]
    #[must_use]
    pub fn get_required_string_by_field(&self, field: StringField) -> &'a str {
        debug_assert!(
            !field.is_none(),
            "get_required_string_by_field called with StringField::NONE"
        );
        self.string_at(field.offset, u32::from(field.length))
            .unwrap_or("")
    }

    /// The `length` bytes at `offset` of the String Data, when they are in
    /// the section and start and end on character boundaries.
    #[inline]
    fn string_at(&self, offset: u32, length: u32) -> Option<&'a str> {
        let start = offset as usize;
        let end = start.checked_add(length as usize)?;
        self.string_data.get(start..end)
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

    /// Get the `source_offset_in_data` (byte offset where this root's
    /// source text starts inside the String Data section) for root index
    /// `root_index`. Used by `description_raw` getters that need to slice
    /// the source text by `(start, end)` byte offsets.
    #[must_use]
    pub fn get_root_source_offset_in_data(&self, root_index: u32) -> u32 {
        let off = self.root_array_offset as usize
            + root_index as usize * ROOT_INDEX_ENTRY_SIZE
            + SOURCE_OFFSET_FIELD;
        read_u32(self.bytes, off)
    }

    /// Slice the source text region for `root_index` at the given
    /// `(start, end)` source-text-relative UTF-8 byte offsets. Returns
    /// `None` for the `(0, 0)` sentinel, for `start > end`, or when the
    /// slice would extend past the String Data or cut a character.
    ///
    /// Used by `description_raw` getters on `LazyJsdocBlock` /
    /// `LazyJsdocTag` (compat-mode wire field per
    /// `design/008-oxlint-oxfmt-support/README.md` §4.3).
    #[must_use]
    pub fn slice_source_text(&self, root_index: u32, start: u32, end: u32) -> Option<&'a str> {
        if start == 0 && end == 0 {
            return None;
        }
        if start > end {
            return None;
        }
        let source_offset = self.get_root_source_offset_in_data(root_index) as usize;
        let start = source_offset.checked_add(start as usize)?;
        let end = source_offset.checked_add(end as usize)?;
        self.string_data.get(start..end)
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
        // fit in 80 bytes so it can sit on the stack with no heap pressure
        // (lazy nodes hold a reference to it, not a copy).
        assert!(size_of::<LazySourceFile<'static>>() <= 80);
    }
}
