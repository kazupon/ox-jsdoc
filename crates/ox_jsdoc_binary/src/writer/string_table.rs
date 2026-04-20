// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! String table builder used by [`super::BinaryWriter`].
//!
//! Owns the **String Offsets** (`8K` bytes) and **String Data** buffers.
//! Strings are deduplicated by content so that `param`, `returns`, etc.
//! shared across a batch of comments are interned only once.
//!
//! See `design/007-binary-ast/format.md#string-table` for the section
//! layout, and `format::string_table` for the on-wire constants.

use core::num::NonZeroU32;
use std::collections::HashMap;

use oxc_allocator::{Allocator, Vec as ArenaVec};

use crate::format::string_table::{STRING_TABLE_MAX_INDEX, U16_NONE_SENTINEL};

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
    /// Returns `None` only when `value == u32::MAX` (so `value + 1`
    /// overflows the `NonZeroU32` storage). Index 0 itself is a valid
    /// string index, so the wrapper internally stores `index + 1`.
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

    /// Get the raw value as `u16`. Returns [`U16_NONE_SENTINEL`] when the
    /// index exceeds [`STRING_TABLE_MAX_INDEX`] — Extended Data slots use
    /// `0xFFFF` as the "None" marker, so that mapping is what the
    /// per-Kind helpers want even when interpretation is ambiguous.
    ///
    /// Realistic encoders never overflow because the writer caps the
    /// string table at [`STRING_TABLE_MAX_INDEX`] entries; this fallback
    /// merely guarantees memory safety if a bug causes overflow.
    #[inline]
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        let raw = self.as_u32();
        if raw <= STRING_TABLE_MAX_INDEX as u32 {
            raw as u16
        } else {
            U16_NONE_SENTINEL
        }
    }
}

/// Builds the String Offsets + String Data buffers incrementally.
///
/// Internally:
/// - the offsets buffer accumulates `(start, end)` u32 pairs (8 bytes each),
/// - the data buffer accumulates UTF-8 bytes (zero-copy slice from the
///   source text whenever possible),
/// - a hash map from `&'arena str` to existing [`StringIndex`] enables dedup.
pub struct StringTableBuilder<'arena> {
    /// `8K` bytes of `(start, end)` u32 pairs, one per interned string.
    pub(crate) offsets_buffer: ArenaVec<'arena, u8>,
    /// Raw concatenated UTF-8 bytes for every interned string and every
    /// appended sourceText.
    pub(crate) data_buffer: ArenaVec<'arena, u8>,
    /// Number of strings interned so far. Equal to
    /// `offsets_buffer.len() / 8`.
    pub(crate) count: u32,
    /// Reference to the underlying arena, used to allocate dedup keys with
    /// `'arena` lifetime.
    arena: &'arena Allocator,
    /// Dedup map: arena-allocated string → its `StringIndex`. Stored on
    /// the `std` heap (not the arena) because `HashMap` cannot live inside
    /// `oxc_allocator` without a custom allocator binding.
    dedup: HashMap<&'arena str, StringIndex>,
}

impl<'arena> StringTableBuilder<'arena> {
    /// Create an empty builder backed by the supplied arena.
    #[must_use]
    pub fn new(arena: &'arena Allocator) -> Self {
        StringTableBuilder {
            offsets_buffer: ArenaVec::new_in(arena),
            data_buffer: ArenaVec::new_in(arena),
            count: 0,
            arena,
            dedup: HashMap::new(),
        }
    }

    /// Intern `value` and return its [`StringIndex`].
    ///
    /// Reuses the existing index when the same string was previously
    /// interned. Otherwise appends `(start, end)` to the offsets buffer and
    /// the bytes of `value` to the data buffer.
    pub fn intern(&mut self, value: &str) -> StringIndex {
        if let Some(&existing) = self.dedup.get(value) {
            return existing;
        }
        let start = self.data_buffer.len() as u32;
        self.data_buffer.extend_from_slice(value.as_bytes());
        let end = self.data_buffer.len() as u32;
        self.offsets_buffer.extend_from_slice(&start.to_le_bytes());
        self.offsets_buffer.extend_from_slice(&end.to_le_bytes());

        let idx = StringIndex::from_u32(self.count).expect("string index overflow");
        self.count = self.count.checked_add(1).expect("string table overflow");

        // Allocate a stable &'arena str so the dedup key outlives every
        // future call.
        let key: &'arena str = self.arena.alloc_str(value);
        self.dedup.insert(key, idx);
        idx
    }

    /// Append a sourceText prefix without registering it in the offsets
    /// table.
    ///
    /// Used by the writer to register each `BatchItem.sourceText` at the
    /// front of String Data so that `Pos`/`End` slicing is direct. Returns
    /// the byte offset where the text was written (used to populate
    /// `root[i].source_offset_in_data`).
    ///
    /// Caller's contract: must be invoked **before** any [`Self::intern`]
    /// call so source-text bytes occupy the front of the String Data
    /// section, matching the format spec ("[sourceText[0]] [sourceText[1]]
    /// ... [unique strings...]").
    pub fn append_source_text(&mut self, value: &str) -> u32 {
        let offset = self.data_buffer.len() as u32;
        self.data_buffer.extend_from_slice(value.as_bytes());
        offset
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

    #[test]
    fn intern_dedups_repeated_values() {
        let arena = Allocator::default();
        let mut builder = StringTableBuilder::new(&arena);
        let a = builder.intern("param");
        let b = builder.intern("returns");
        let a_again = builder.intern("param");
        assert_eq!(a, a_again, "intern must dedup identical strings");
        assert_ne!(a, b);
        assert_eq!(builder.len(), 2, "exactly 2 unique entries");
    }

    #[test]
    fn intern_appends_to_offsets_and_data_buffers() {
        let arena = Allocator::default();
        let mut builder = StringTableBuilder::new(&arena);
        let _ = builder.intern("ab");
        let _ = builder.intern("cd");
        assert_eq!(builder.offsets_buffer.len(), 16, "two 8-byte entries");
        assert_eq!(builder.data_buffer.len(), 4, "concatenated bytes");
    }

    #[test]
    fn append_source_text_does_not_register_an_index() {
        let arena = Allocator::default();
        let mut builder = StringTableBuilder::new(&arena);
        let off = builder.append_source_text("/** @x */");
        assert_eq!(off, 0, "first source text starts at byte 0");
        assert_eq!(builder.len(), 0, "source text does not count as an interned entry");
        assert_eq!(builder.data_buffer.len(), "/** @x */".len());
    }
}
