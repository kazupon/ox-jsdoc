//! String table builder used by [`super::BinaryWriter`].
//!
//! Owns the **String Offsets** (`8K` bytes) and **String Data** buffers.
//! Strings are deduplicated by content so that `param`, `returns`, etc.
//! shared across a batch of comments are interned only once.
//!
//! See `design/007-binary-ast/format.md#string-table` for the section
//! layout, and `format::string_table` for the on-wire constants.

use core::num::NonZeroU32;

use oxc_allocator::{Allocator, Vec as ArenaVec};

/// Index into the **String Offsets** table.
///
/// Newtype wrapper so a string index cannot accidentally be confused with a
/// node index, an extended-data offset, or a raw `u32`. The maximum value
/// fits in a `u16` for Extended Data slots
/// (see `format::string_table::STRING_TABLE_MAX_INDEX`); String-type Node
/// Data leaves can reference up to 30 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StringIndex(NonZeroU32);

impl StringIndex {
    /// Construct a [`StringIndex`] from a raw `u32`.
    ///
    /// Returns `None` when the value is `0`. The all-zero slot in any
    /// Extended Data field is reserved for `0xFFFF` (None sentinel) handling
    /// and never refers to a real string at offset 0; index 0 itself is a
    /// valid string index, so the wrapper internally stores `index + 1` so
    /// it can use `NonZeroU32` for niche optimization.
    #[inline]
    #[must_use]
    pub const fn from_u32(value: u32) -> Option<Self> {
        match NonZeroU32::new(value.wrapping_add(1)) {
            Some(nz) => Some(StringIndex(nz)),
            None => None,
        }
    }

    /// Get the raw `u32` index.
    #[inline]
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0.get() - 1
    }

    /// Get the raw value as `u16`, panicking when it overflows. Used by
    /// Extended Data fields where the slot is u16-wide.
    #[inline]
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        // Phase 1.1a: switch to checked truncation that returns Result so
        // overflow can become a writer error instead of a panic.
        self.as_u32() as u16
    }
}

/// Builds the String Offsets + String Data buffers incrementally.
///
/// Internally:
/// - the offsets buffer accumulates `(start, end)` u32 pairs (8 bytes each),
/// - the data buffer accumulates UTF-8 bytes (zero-copy slice from the
///   source text whenever possible),
/// - a hash map from `&str` to existing [`StringIndex`] enables dedup.
// Phase 1.0b: fields are populated but not yet read because every public
// method is `unimplemented!()`. The `dead_code` allow is removed in 1.1a.
#[allow(dead_code)]
pub struct StringTableBuilder<'arena> {
    /// `8K` bytes of `(start, end)` u32 pairs, one per interned string.
    pub(crate) offsets_buffer: ArenaVec<'arena, u8>,
    /// Raw concatenated UTF-8 bytes for every interned string.
    pub(crate) data_buffer: ArenaVec<'arena, u8>,
    /// Number of strings interned so far. Equal to
    /// `offsets_buffer.len() / 8`.
    pub(crate) count: u32,
    // Phase 1.1a will add a `HashMap<&'arena str, StringIndex>` for dedup.
}

impl<'arena> StringTableBuilder<'arena> {
    /// Create an empty builder backed by the supplied arena.
    #[must_use]
    pub fn new(_arena: &'arena Allocator) -> Self {
        unimplemented!("Phase 1.1a: allocate offsets/data buffers in arena and seed dedup map")
    }

    /// Intern `value` and return its [`StringIndex`].
    ///
    /// Reuses the existing index when the same string was previously
    /// interned. Otherwise appends `(start, end)` to the offsets buffer and
    /// the bytes of `value` to the data buffer.
    pub fn intern(&mut self, _value: &str) -> StringIndex {
        unimplemented!("Phase 1.1a: dedup map lookup, append on miss")
    }

    /// Intern a sourceText prefix without adding a dedup map entry.
    ///
    /// Used by the writer to register each `BatchItem.sourceText` at the
    /// front of String Data so that `Pos`/`End` slicing is direct. Returns
    /// the byte offset where the text was written (used to populate
    /// `root[i].source_offset_in_data`).
    pub fn append_source_text(&mut self, _value: &str) -> u32 {
        unimplemented!("Phase 1.1a: append text bytes, return offset")
    }

    /// Number of strings interned so far.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.count
    }

    /// Whether nothing has been interned yet.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_index_round_trips() {
        for raw in [0u32, 1, 100, 0xFFFE, 0x3FFF_FFFE] {
            let idx = StringIndex::from_u32(raw).unwrap();
            assert_eq!(idx.as_u32(), raw);
        }
    }

    #[test]
    fn string_index_rejects_overflow() {
        assert!(StringIndex::from_u32(u32::MAX).is_none());
    }

    #[test]
    fn string_index_zero_is_representable() {
        let idx = StringIndex::from_u32(0).unwrap();
        assert_eq!(idx.as_u32(), 0);
    }
}
