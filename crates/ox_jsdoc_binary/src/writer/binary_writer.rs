// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! [`BinaryWriter`] — the top-level entry point for emitting Binary AST.

use oxc_allocator::{Allocator, Vec as ArenaVec};
use oxc_span::Span;

use crate::format::diagnostics;
use crate::format::header::{
    self, COMPAT_MODE_BIT, Header, SUPPORTED_VERSION_BYTE,
    DIAGNOSTICS_OFFSET_FIELD, EXTENDED_DATA_OFFSET_FIELD, FLAGS_OFFSET, HEADER_SIZE,
    NODES_OFFSET_FIELD, NODE_COUNT_FIELD, ROOT_ARRAY_OFFSET_FIELD, ROOT_COUNT_FIELD,
    SOURCE_TEXT_LENGTH_FIELD, STRING_DATA_OFFSET_FIELD, STRING_OFFSETS_OFFSET_FIELD,
    VERSION_OFFSET,
};
use crate::format::kind::Kind;
use crate::format::node_record::{
    COMMON_DATA_MASK, NEXT_SIBLING_OFFSET, NODE_RECORD_SIZE, TypeTag, pack_node_data,
};
use crate::format::root_index::ROOT_INDEX_ENTRY_SIZE;

use super::extended_data::{ExtOffset, ExtendedDataBuilder};
use super::nodes::NodeIndex;
use super::string_table::{StringIndex, StringTableBuilder};

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
pub struct BinaryWriter<'arena> {
    /// In-memory header; field offsets are patched in at [`Self::finish`].
    pub(crate) header: Header,
    /// Root index array buffer (`12N` bytes, see `format::root_index`).
    pub(crate) root_index_buffer: ArenaVec<'arena, u8>,
    /// Diagnostics entries (`(root_index, message_index)`); sorted at
    /// [`Self::finish`] before being serialized.
    pub(crate) diagnostics: ArenaVec<'arena, (u32, u32)>,
    /// Nodes section buffer (`24P` bytes), starting with the sentinel.
    pub(crate) nodes_buffer: ArenaVec<'arena, u8>,
    /// String table builder (handles dedup + offsets/data buffers).
    pub(crate) strings: StringTableBuilder<'arena>,
    /// Extended Data builder (handles 8-byte alignment).
    pub(crate) extended: ExtendedDataBuilder<'arena>,
    /// Total length of source-text bytes appended via
    /// [`StringTableBuilder::append_source_text`]. Stored separately from
    /// `strings.data_buffer.len()` because that buffer also contains
    /// interned strings.
    pub(crate) source_text_length: u32,
    /// Per-parent backpatch table: `next_sibling_patch[parent_index]`
    /// stores the byte offset of the most recent child of `parent_index`
    /// (so the next call to [`Self::emit_node_record`] can patch its
    /// `next_sibling` field). `0` means "no previous sibling".
    pub(crate) next_sibling_patch: ArenaVec<'arena, u32>,
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
    pub fn new(arena: &'arena Allocator) -> Self {
        let mut nodes_buffer = ArenaVec::new_in(arena);
        // Pre-write the all-zero sentinel `node[0]` so real nodes start at
        // index 1 and `parent_index = 0` / `next_sibling = 0` mean
        // "no link" without a special case.
        nodes_buffer.extend(core::iter::repeat_n(0u8, NODE_RECORD_SIZE));

        let mut header = Header::default();
        header.version = SUPPORTED_VERSION_BYTE;

        BinaryWriter {
            header,
            root_index_buffer: ArenaVec::new_in(arena),
            diagnostics: ArenaVec::new_in(arena),
            nodes_buffer,
            strings: StringTableBuilder::new(arena),
            extended: ExtendedDataBuilder::new(arena),
            source_text_length: 0,
            next_sibling_patch: ArenaVec::new_in(arena),
            arena,
        }
    }

    /// Emit one 24-byte node record into the Nodes section and return its
    /// new [`NodeIndex`].
    ///
    /// Side effect: updates `next_sibling_patch` so the next sibling of
    /// `parent_index` will be backpatched to point at the freshly-emitted
    /// node. `parent_index = 0` means "child of the sentinel" (i.e. a
    /// root).
    ///
    /// `common_data` is masked to its lower 6 bits before being stored, so
    /// callers can pass the raw bit field without worrying about the
    /// reserved upper 2 bits.
    pub(crate) fn emit_node_record(
        &mut self,
        parent_index: u32,
        kind: Kind,
        common_data: u8,
        span: Span,
        node_data: u32,
    ) -> NodeIndex {
        let new_index = self.node_count();
        let new_byte_offset = self.nodes_buffer.len() as u32;

        // Write the 24-byte record (next_sibling temporarily 0).
        self.nodes_buffer.push(kind.as_u8());
        self.nodes_buffer.push(common_data & COMMON_DATA_MASK);
        self.nodes_buffer.extend_from_slice(&[0u8, 0u8]); // padding (byte 2-3)
        self.nodes_buffer.extend_from_slice(&span.start.to_le_bytes());
        self.nodes_buffer.extend_from_slice(&span.end.to_le_bytes());
        self.nodes_buffer.extend_from_slice(&node_data.to_le_bytes());
        self.nodes_buffer.extend_from_slice(&parent_index.to_le_bytes());
        self.nodes_buffer.extend_from_slice(&0u32.to_le_bytes());

        // Backpatch the previous sibling's `next_sibling` to this node, if
        // any.
        let parent_idx = parent_index as usize;
        if parent_idx >= self.next_sibling_patch.len() {
            self.next_sibling_patch.resize(parent_idx + 1, 0);
        }
        let prev_byte_offset = self.next_sibling_patch[parent_idx];
        if prev_byte_offset != 0 {
            let patch_at = prev_byte_offset as usize + NEXT_SIBLING_OFFSET;
            let bytes = new_index.to_le_bytes();
            self.nodes_buffer[patch_at..patch_at + 4].copy_from_slice(&bytes);
        }
        self.next_sibling_patch[parent_idx] = new_byte_offset;

        NodeIndex::new(new_index).expect("node_index 0 is reserved for the sentinel")
    }

    /// Convenience for **String-type** leaves: emit a node whose Node Data
    /// payload is a 30-bit String Offsets index.
    #[inline]
    pub(crate) fn emit_string_node(
        &mut self,
        parent_index: u32,
        kind: Kind,
        common_data: u8,
        span: Span,
        string_index: StringIndex,
    ) -> NodeIndex {
        let node_data = pack_node_data(TypeTag::String, string_index.as_u32());
        self.emit_node_record(parent_index, kind, common_data, span, node_data)
    }

    /// Convenience for **Children-type** nodes: emit a node whose Node Data
    /// payload is a 30-bit visitor-order Children bitmask.
    #[inline]
    pub(crate) fn emit_children_node(
        &mut self,
        parent_index: u32,
        kind: Kind,
        common_data: u8,
        span: Span,
        children_bitmask: u32,
    ) -> NodeIndex {
        let node_data = pack_node_data(TypeTag::Children, children_bitmask);
        self.emit_node_record(parent_index, kind, common_data, span, node_data)
    }

    /// Convenience for **Extended-type** nodes: emit a node whose Node Data
    /// payload is the supplied Extended Data byte offset.
    #[inline]
    pub(crate) fn emit_extended_node(
        &mut self,
        parent_index: u32,
        kind: Kind,
        common_data: u8,
        span: Span,
        ext_offset: ExtOffset,
    ) -> NodeIndex {
        let node_data = pack_node_data(TypeTag::Extended, ext_offset.as_u32());
        self.emit_node_record(parent_index, kind, common_data, span, node_data)
    }

    /// Set the `compat_mode` flag bit on the header.
    ///
    /// Must be called before any node is written, since the bit affects the
    /// per-Kind Extended Data layouts emitted by `write_*` helpers.
    pub fn set_compat_mode(&mut self, enabled: bool) {
        if enabled {
            self.header.flags |= COMPAT_MODE_BIT;
        } else {
            self.header.flags &= !COMPAT_MODE_BIT;
        }
    }

    /// Whether `compat_mode` is currently enabled. `write_*` helpers consult
    /// this to decide whether to emit the compat extension region.
    #[inline]
    #[must_use]
    pub const fn compat_mode(&self) -> bool {
        self.header.compat_mode()
    }

    /// Append one root entry to the Root Index Array.
    ///
    /// `node_index = 0` indicates parse failure (per
    /// [`crate::format::root_index::PARSE_FAILURE_SENTINEL`]); when used,
    /// at least one matching diagnostic must subsequently be emitted via
    /// [`Self::push_diagnostic`].
    pub fn push_root(
        &mut self,
        node_index: u32,
        source_offset_in_data: u32,
        base_offset: u32,
    ) {
        self.root_index_buffer.extend_from_slice(&node_index.to_le_bytes());
        self.root_index_buffer
            .extend_from_slice(&source_offset_in_data.to_le_bytes());
        self.root_index_buffer.extend_from_slice(&base_offset.to_le_bytes());
    }

    /// Append one diagnostic entry. The entries are sorted by `root_index`
    /// ascending at [`Self::finish`] (so callers may insert them in any
    /// order).
    pub fn push_diagnostic(&mut self, root_index: u32, message: &str) {
        let message_index = self.strings.intern(message);
        self.diagnostics.push((root_index, message_index.as_u32()));
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

    /// Convenience: intern a string into the table. Returns the index that
    /// can be packed into a Node Data String payload or an Extended Data
    /// `u16` slot.
    pub fn intern_string(&mut self, value: &str) -> StringIndex {
        self.strings.intern(value)
    }

    /// Convenience: append a sourceText prefix and remember its byte length
    /// so [`Header.source_text_length`] is set correctly at [`Self::finish`].
    pub fn append_source_text(&mut self, value: &str) -> u32 {
        let offset = self.strings.append_source_text(value);
        self.source_text_length = self
            .source_text_length
            .saturating_add(value.len() as u32);
        offset
    }

    /// Number of node records currently in the Nodes section (including the
    /// `node[0]` sentinel).
    #[inline]
    #[must_use]
    pub fn node_count(&self) -> u32 {
        (self.nodes_buffer.len() / NODE_RECORD_SIZE) as u32
    }

    /// Number of roots currently in the Root Index Array.
    #[inline]
    #[must_use]
    pub fn root_count(&self) -> u32 {
        (self.root_index_buffer.len() / ROOT_INDEX_ENTRY_SIZE) as u32
    }

    /// Reference to the arena passed at [`Self::new`]. Useful when
    /// per-Kind helpers need scratch allocations.
    #[inline]
    #[must_use]
    pub fn arena(&self) -> &'arena Allocator {
        self.arena
    }

    /// Finish writing and produce the concatenated Binary AST byte stream.
    ///
    /// At this point the writer:
    /// - sorts the diagnostic entries by `root_index` ascending,
    /// - resolves each section's start offset and patches the [`Header`],
    /// - writes Header (40 bytes) + all section buffers in canonical order
    ///   (Root index array → String Offsets → String Data → Extended Data
    ///   → Diagnostics → Nodes).
    ///
    /// The returned `Vec<u8>` is owned (not arena-backed) so it can be sent
    /// across NAPI/WASM boundaries without lifetime concerns.
    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        // -- 1. sort diagnostics by root_index ascending --------------------
        self.diagnostics.sort_by_key(|(root_index, _)| *root_index);

        // -- 2. compute counts and section sizes ----------------------------
        let node_count = self.node_count();
        let root_count = self.root_count();
        let diagnostic_count = self.diagnostics.len() as u32;

        let root_array_size = self.root_index_buffer.len();
        let string_offsets_size = self.strings.offsets_buffer.len();
        let string_data_size = self.strings.data_buffer.len();
        let extended_data_size = self.extended.buffer.len();
        let diagnostics_size = diagnostics::section_size(self.diagnostics.len());
        let nodes_size = self.nodes_buffer.len();

        // -- 3. compute absolute section offsets ----------------------------
        let root_array_offset = HEADER_SIZE as u32;
        let string_offsets_offset = root_array_offset + root_array_size as u32;
        let string_data_offset = string_offsets_offset + string_offsets_size as u32;
        let extended_data_offset = string_data_offset + string_data_size as u32;
        let diagnostics_offset = extended_data_offset + extended_data_size as u32;
        let nodes_offset = diagnostics_offset + diagnostics_size as u32;

        // -- 4. build the output buffer -------------------------------------
        let total_size = HEADER_SIZE
            + root_array_size
            + string_offsets_size
            + string_data_size
            + extended_data_size
            + diagnostics_size
            + nodes_size;
        let mut out: Vec<u8> = Vec::with_capacity(total_size);
        out.resize(HEADER_SIZE, 0);

        // -- 4a. Header -----------------------------------------------------
        out[VERSION_OFFSET] = self.header.version;
        out[FLAGS_OFFSET] = self.header.flags;
        // bytes 2-3 already zero (reserved)
        write_u32(&mut out, ROOT_ARRAY_OFFSET_FIELD, root_array_offset);
        write_u32(&mut out, STRING_OFFSETS_OFFSET_FIELD, string_offsets_offset);
        write_u32(&mut out, STRING_DATA_OFFSET_FIELD, string_data_offset);
        write_u32(&mut out, EXTENDED_DATA_OFFSET_FIELD, extended_data_offset);
        write_u32(&mut out, DIAGNOSTICS_OFFSET_FIELD, diagnostics_offset);
        write_u32(&mut out, NODES_OFFSET_FIELD, nodes_offset);
        write_u32(&mut out, NODE_COUNT_FIELD, node_count);
        write_u32(&mut out, SOURCE_TEXT_LENGTH_FIELD, self.source_text_length);
        write_u32(&mut out, ROOT_COUNT_FIELD, root_count);
        debug_assert_eq!(header::HEADER_SIZE, out.len());

        // -- 4b. Root index array ------------------------------------------
        out.extend_from_slice(&self.root_index_buffer);

        // -- 4c. String Offsets / Data --------------------------------------
        out.extend_from_slice(&self.strings.offsets_buffer);
        out.extend_from_slice(&self.strings.data_buffer);

        // -- 4d. Extended Data ----------------------------------------------
        out.extend_from_slice(&self.extended.buffer);

        // -- 4e. Diagnostics: count header + entries ------------------------
        out.extend_from_slice(&diagnostic_count.to_le_bytes());
        for (root_index, message_index) in &self.diagnostics {
            out.extend_from_slice(&root_index.to_le_bytes());
            out.extend_from_slice(&message_index.to_le_bytes());
        }

        // -- 4f. Nodes ------------------------------------------------------
        out.extend_from_slice(&self.nodes_buffer);

        debug_assert_eq!(total_size, out.len(), "section sizes must match capacity");
        out
    }
}

/// Write a little-endian u32 at the given byte offset.
#[inline]
fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::source_file::LazySourceFile;

    #[test]
    fn empty_buffer_roundtrips_through_lazy_source_file() {
        use crate::writer::string_table::COMMON_STRING_COUNT;

        let arena = Allocator::default();
        let writer = BinaryWriter::new(&arena);
        assert_eq!(writer.node_count(), 1, "sentinel node[0] is pre-written");
        assert_eq!(writer.root_count(), 0);
        assert!(!writer.compat_mode());
        // The string table is seeded with `COMMON_STRING_COUNT` entries
        // (delimiters, whitespace, common tag names) that the writer
        // pre-interns so per-call `intern()` can skip the HashMap.
        assert_eq!(writer.strings.len(), COMMON_STRING_COUNT);

        let bytes = writer.finish();

        let sf = LazySourceFile::new(&bytes).expect("empty buffer must parse");
        assert_eq!(sf.node_count, 1);
        assert_eq!(sf.root_count, 0);
        assert!(!sf.compat_mode);
        // Sections sit in canonical order; offsets shift by the size of
        // the common-string prelude.
        assert_eq!(sf.root_array_offset, 40);
        assert_eq!(sf.string_offsets_offset, 40);
        // Each interned entry occupies 8 bytes in the offsets table.
        let prelude_offsets_bytes = COMMON_STRING_COUNT * 8;
        assert_eq!(sf.string_data_offset, 40 + prelude_offsets_bytes);
    }

    #[test]
    fn set_compat_mode_round_trips() {
        let arena = Allocator::default();
        let mut writer = BinaryWriter::new(&arena);
        writer.set_compat_mode(true);
        let bytes = writer.finish();
        assert_eq!(bytes[FLAGS_OFFSET] & COMPAT_MODE_BIT, COMPAT_MODE_BIT);

        let sf = LazySourceFile::new(&bytes).unwrap();
        assert!(sf.compat_mode);
    }

    #[test]
    fn push_root_writes_12_byte_entry_in_canonical_order() {
        let arena = Allocator::default();
        let mut writer = BinaryWriter::new(&arena);
        writer.push_root(1, 0, 100);
        writer.push_root(0, 7, 200); // parse failure sentinel
        assert_eq!(writer.root_count(), 2);

        let bytes = writer.finish();
        let sf = LazySourceFile::new(&bytes).unwrap();
        assert_eq!(sf.root_count, 2);
        // Each entry is 12 bytes; the first one starts at root_array_offset.
        let root0 = sf.root_array_offset as usize;
        assert_eq!(read_u32_at(&bytes, root0), 1, "node_index of root 0");
        assert_eq!(read_u32_at(&bytes, root0 + 4), 0, "source_offset_in_data");
        assert_eq!(read_u32_at(&bytes, root0 + 8), 100, "base_offset");
        assert_eq!(read_u32_at(&bytes, root0 + 12), 0, "node_index of root 1 (failure)");
        assert_eq!(read_u32_at(&bytes, root0 + 20), 200);
    }

    #[test]
    fn push_diagnostic_sorts_by_root_index() {
        let arena = Allocator::default();
        let mut writer = BinaryWriter::new(&arena);
        // Insert out of order; finish() must sort ascending by root_index.
        writer.push_diagnostic(2, "second");
        writer.push_diagnostic(0, "zero");
        writer.push_diagnostic(1, "one");

        let bytes = writer.finish();
        let sf = LazySourceFile::new(&bytes).unwrap();
        let diag_offset = sf.diagnostics_offset as usize;
        assert_eq!(read_u32_at(&bytes, diag_offset), 3, "diagnostic count");

        // First entry: root_index = 0
        assert_eq!(read_u32_at(&bytes, diag_offset + 4), 0);
        // Second entry: root_index = 1
        assert_eq!(read_u32_at(&bytes, diag_offset + 4 + 8), 1);
        // Third entry: root_index = 2
        assert_eq!(read_u32_at(&bytes, diag_offset + 4 + 16), 2);
    }

    #[test]
    fn finish_records_source_text_length() {
        let arena = Allocator::default();
        let mut writer = BinaryWriter::new(&arena);
        let _ = writer.append_source_text("/** @param x */");
        let bytes = writer.finish();
        let sf = LazySourceFile::new(&bytes).unwrap();
        // sourceText length is in bytes, not chars; ASCII-only here so they match.
        let expected = "/** @param x */".len() as u32;
        assert_eq!(
            read_u32_at(&bytes, header::SOURCE_TEXT_LENGTH_FIELD),
            expected
        );
        // Spot-check the LazySourceFile path doesn't panic on the same buffer.
        assert_eq!(sf.node_count, 1);
    }

    fn read_u32_at(buf: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
    }
}
