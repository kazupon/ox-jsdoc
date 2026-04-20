//! [`BinaryWriter`] — the top-level entry point for emitting Binary AST.

use oxc_allocator::{Allocator, Vec as ArenaVec};

use crate::format::header::Header;

use super::extended_data::ExtendedDataBuilder;
use super::string_table::StringTableBuilder;

/// Top-level writer that owns one buffer per Binary AST section.
///
/// Construction: [`BinaryWriter::new`] pre-writes the 24-byte sentinel
/// `node[0]` so that real nodes start at index 1.
///
/// Lifecycle: parser code drives [`BinaryWriter`] through the
/// `write_*` helpers (see the [`super::nodes`] module). When all roots and
/// diagnostics have been written, [`BinaryWriter::finish`] concatenates the
/// per-section buffers and patches the [`Header`] with the resolved offsets,
/// returning the final Binary AST byte stream.
///
/// All buffers are arena-allocated against the borrow-checker-tracked
/// `'arena` lifetime, so the resulting bytes can be shared zero-copy with
/// NAPI/WASM bindings as long as the arena outlives the consumer.
// Phase 1.0b: fields are populated but not yet read because every public
// method is `unimplemented!()`. The `dead_code` allow is removed in 1.1a.
#[allow(dead_code)]
pub struct BinaryWriter<'arena> {
    /// In-memory header; field offsets are patched in at [`Self::finish`].
    pub(crate) header: Header,
    /// Root index array buffer (`12N` bytes, see `format::root_index`).
    pub(crate) root_index_buffer: ArenaVec<'arena, u8>,
    /// Diagnostics section buffer (`4 + 8M` bytes).
    pub(crate) diagnostics_buffer: ArenaVec<'arena, u8>,
    /// Nodes section buffer (`24P` bytes), starting with the sentinel.
    pub(crate) nodes_buffer: ArenaVec<'arena, u8>,
    /// String table builder (handles dedup + offsets/data buffers).
    pub(crate) strings: StringTableBuilder<'arena>,
    /// Extended Data builder (handles 8-byte alignment).
    pub(crate) extended: ExtendedDataBuilder<'arena>,
    /// Reference to the underlying arena, used by the per-node helpers when
    /// they need to allocate scratch space.
    pub(crate) arena: &'arena Allocator,
}

impl<'arena> BinaryWriter<'arena> {
    /// Create a fresh writer bound to the supplied arena.
    ///
    /// Pre-allocates the per-section buffers and writes the all-zero
    /// `node[0]` sentinel. After construction, calling [`Self::finish`]
    /// without writing any roots yields a valid empty Binary AST buffer.
    #[must_use]
    pub fn new(_arena: &'arena Allocator) -> Self {
        unimplemented!("Phase 1.1a: implement BinaryWriter::new (allocates per-section buffers and writes node[0] sentinel)")
    }

    /// Set the `compat_mode` flag bit on the header.
    ///
    /// Must be called before any node is written, since the bit affects the
    /// per-Kind Extended Data layouts emitted by `write_*` helpers.
    pub fn set_compat_mode(&mut self, _enabled: bool) {
        unimplemented!("Phase 1.1a: flip COMPAT_MODE_BIT on self.header.flags")
    }

    /// Append one root entry to the Root Index Array.
    ///
    /// `node_index = 0` indicates parse failure (per
    /// `format::root_index::PARSE_FAILURE_SENTINEL`); when used, at least one
    /// matching diagnostic must subsequently be emitted via
    /// [`Self::push_diagnostic`].
    pub fn push_root(
        &mut self,
        _node_index: u32,
        _source_offset_in_data: u32,
        _base_offset: u32,
    ) {
        unimplemented!("Phase 1.1a: append a 12-byte entry to self.root_index_buffer")
    }

    /// Append one diagnostic entry. The buffer is sorted by `root_index`
    /// ascending at [`Self::finish`], not on insertion.
    pub fn push_diagnostic(&mut self, _root_index: u32, _message: &str) {
        unimplemented!("Phase 1.1a: intern message + append (root_index, message_index) entry")
    }

    /// Borrow the underlying string table builder. Used by per-Kind
    /// `write_*` helpers to intern delimiter / description strings.
    pub fn strings(&mut self) -> &mut StringTableBuilder<'arena> {
        &mut self.strings
    }

    /// Borrow the underlying Extended Data builder.
    pub fn extended(&mut self) -> &mut ExtendedDataBuilder<'arena> {
        &mut self.extended
    }

    /// Finish writing and produce the concatenated Binary AST byte stream.
    ///
    /// At this point the writer:
    /// - sorts the diagnostic buffer by `root_index` ascending,
    /// - resolves each section's start offset and patches the [`Header`],
    /// - writes Header (40 bytes) + all section buffers in canonical order
    ///   (Root index array → String Offsets → String Data → Extended Data
    ///   → Diagnostics → Nodes).
    ///
    /// The returned `Vec<u8>` is owned (not arena-backed) so it can be sent
    /// across NAPI/WASM boundaries without lifetime concerns.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        unimplemented!("Phase 1.1a: concatenate sections + patch header offsets")
    }
}
