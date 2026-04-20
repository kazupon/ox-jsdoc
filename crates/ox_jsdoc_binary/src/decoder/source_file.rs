//! [`LazySourceFile`] — root of the lazy decoder.
//!
//! See `design/007-binary-ast/rust-impl.md#lazysourcefile-root-of-the-decoder`.
//!
//! The struct caches the Header offsets/counts at construction so every
//! lazy node can reach the String table or Root array in O(1) without
//! re-parsing the Header.

use core::marker::PhantomData;

use super::error::DecodeError;

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
    /// Byte offset of the String Offsets section.
    pub string_offsets_offset: u32,
    /// Byte offset of the String Data section.
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
    /// log a warning (but accept the input) when the buffer's minor version
    /// is newer than the decoder's supported minor.
    pub fn new(_bytes: &'a [u8]) -> Result<Self, DecodeError> {
        todo!("Phase 1.1b: parse Header (40 bytes), validate major, populate fields")
    }

    /// Borrow the underlying byte slice. Useful for advanced consumers that
    /// need raw access (e.g. exporting the buffer over IPC).
    #[inline]
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Resolve the string at `idx` (None when `idx` is the u16 None sentinel
    /// `0xFFFF` or the 30-bit `0x3FFF_FFFF`).
    ///
    /// Performs an in-place UTF-8 slice from String Data; the returned `&str`
    /// borrows directly from the buffer (zero-copy). Phase 1.1b will use
    /// `from_utf8_unchecked` after the writer guarantees valid UTF-8.
    pub fn get_string(&self, _idx: u32) -> Option<&'a str> {
        todo!("Phase 1.1b: lookup (start, end) in String Offsets, slice String Data")
    }

    /// Get the `base_offset` (original-file absolute byte position) for
    /// root index `root_index`. Used by lazy nodes when computing the
    /// `range` getter.
    pub fn get_root_base_offset(&self, _root_index: u32) -> u32 {
        todo!("Phase 1.1b: read u32 at root_array_offset + root_index * 12 + 8")
    }

    /// Iterate over the AST root for each entry in the Root index array.
    ///
    /// Yields `None` for entries whose `node_index = 0` (parse failure
    /// sentinel) and `Some(LazyJsdocBlock)` for successful parses. The
    /// generic parameter ties the iterator's lifetime to `self`.
    pub fn asts(&'a self) -> AstsIter<'a> {
        AstsIter {
            source_file: self,
            cursor: 0,
            _marker: PhantomData,
        }
    }
}

/// Iterator returned by [`LazySourceFile::asts`].
#[derive(Debug)]
pub struct AstsIter<'a> {
    source_file: &'a LazySourceFile<'a>,
    cursor: u32,
    // Reserved for future generic refinements (e.g. LazyJsdocBlock once
    // comment_ast.rs is fully populated). Held here to keep the iterator
    // signature stable across phases.
    _marker: PhantomData<&'a u8>,
}

impl<'a> Iterator for AstsIter<'a> {
    /// `None` when the root parsed successfully (placeholder until Phase
    /// 1.1b wires up the real `Option<LazyJsdocBlock<'a>>` return type).
    type Item = Option<u32>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.source_file.root_count {
            return None;
        }
        // Phase 1.1b: read u32 at root_array_offset + cursor * 12, return
        // None for sentinel (0) and Some(LazyJsdocBlock::from_index(...))
        // otherwise. Today we just advance the cursor so callers can verify
        // the iterator yields exactly `root_count` entries.
        self.cursor += 1;
        Some(None)
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
