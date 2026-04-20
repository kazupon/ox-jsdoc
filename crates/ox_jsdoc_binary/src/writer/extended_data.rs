//! Extended Data buffer builder used by [`super::BinaryWriter`].
//!
//! Manages a single byte buffer onto which per-Kind Extended Data records
//! are appended. Each new record is prefixed with zero-fill padding so its
//! starting byte offset is divisible by
//! [`crate::format::extended_data::EXTENDED_DATA_ALIGNMENT`] (8 bytes).
//!
//! See `design/007-binary-ast/format.md#extended-data-section` for the
//! per-Kind layouts.

use core::num::NonZeroU32;

use oxc_allocator::{Allocator, Vec as ArenaVec};

/// Byte offset into the Extended Data section.
///
/// Newtype wrapper to avoid mixing up Extended Data offsets with String
/// Offsets indices or node indices. Stored as `offset + 1` internally so the
/// type can use `NonZeroU32` for niche optimization (`Option<ExtOffset>` is
/// 4 bytes).
///
/// The wire representation in Node Data uses the *raw* offset packed into
/// 30 bits (see `format::node_record::PAYLOAD_MASK`); offset 0 is a valid
/// position because the very first record sits at byte 0 of the section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExtOffset(NonZeroU32);

impl ExtOffset {
    /// Construct an [`ExtOffset`] from the raw byte offset.
    ///
    /// Returns `None` only when `offset + 1` overflows `u32` (i.e. `offset`
    /// is `u32::MAX`).
    #[inline]
    #[must_use]
    pub const fn from_u32(offset: u32) -> Option<Self> {
        match NonZeroU32::new(offset.wrapping_add(1)) {
            Some(nz) => Some(ExtOffset(nz)),
            None => None,
        }
    }

    /// Get the raw byte offset.
    #[inline]
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0.get() - 1
    }
}

/// Builder that appends Extended Data records with the required 8-byte
/// alignment.
pub struct ExtendedDataBuilder<'arena> {
    /// Concatenated Extended Data records.
    pub(crate) buffer: ArenaVec<'arena, u8>,
}

impl<'arena> ExtendedDataBuilder<'arena> {
    /// Create an empty builder backed by the supplied arena.
    #[must_use]
    pub fn new(_arena: &'arena Allocator) -> Self {
        unimplemented!("Phase 1.1a: allocate buffer in arena")
    }

    /// Reserve `size` bytes for a new record, returning the resulting
    /// [`ExtOffset`].
    ///
    /// Inserts zero-fill padding before the record so the offset is
    /// 8-byte aligned. Caller is responsible for writing exactly `size`
    /// bytes after the call.
    pub fn reserve(&mut self, _size: usize) -> ExtOffset {
        unimplemented!("Phase 1.1a: align(buffer.len(), 8); push zeros; return offset")
    }

    /// Total Extended Data section size in bytes (includes padding).
    #[inline]
    #[must_use]
    pub fn size(&self) -> usize {
        self.buffer.len()
    }

    /// Whether the buffer is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext_offset_round_trips() {
        for raw in [0u32, 1, 7, 8, 0x3FFF_FFFE] {
            let off = ExtOffset::from_u32(raw).unwrap();
            assert_eq!(off.as_u32(), raw);
        }
    }

    #[test]
    fn ext_offset_zero_is_representable() {
        let off = ExtOffset::from_u32(0).unwrap();
        assert_eq!(off.as_u32(), 0);
    }

    #[test]
    fn ext_offset_rejects_u32_max() {
        assert!(ExtOffset::from_u32(u32::MAX).is_none());
    }
}
