//! Common helpers shared by lazy node implementations.
//!
//! See `design/007-binary-ast/rust-impl.md#helper-functions-shared-parts-for-reading-binary-ast`.

use super::source_file::LazySourceFile;

/// Read a little-endian `u32` at `offset` from `bytes`.
///
/// The buffer is expected to be at least `offset + 4` bytes long; the
/// caller (via the lazy decoder) guarantees this through Header validation.
#[inline]
#[must_use]
pub fn read_u32(_bytes: &[u8], _offset: usize) -> u32 {
    todo!("Phase 1.1b: u32::from_le_bytes(bytes[offset..offset+4].try_into().unwrap())")
}

/// Resolve the Extended Data byte offset for the node at `node_index`.
///
/// The node must use Extended type Node Data (`0b10`); calling this on a
/// Children/String/Reserved node debug-asserts in development builds.
///
/// Per the format spec:
///
/// ```text
/// node_data       = u32 read at nodes_offset + node_index * 24 + 12
/// type_tag        = (node_data >> 30) & 0b11        // must equal 0b10
/// payload         = node_data & 0x3FFF_FFFF
/// ext_data_offset = sf.extended_data_offset + payload
/// ```
#[inline]
#[must_use]
pub fn ext_offset(_sf: &LazySourceFile<'_>, _node_index: u32) -> u32 {
    todo!("Phase 1.1b: read Node Data, validate type tag = 0b10, return absolute offset")
}

/// Find the `visitor_index`-th set bit in `bitmask` and return the
/// corresponding child node index relative to `parent_index`.
///
/// Children are placed contiguously starting at `parent_index + 1` in DFS
/// pre-order. The `visitor_index`-th set bit denotes the slot the parent
/// promised in its visitor key list; we walk `next_sibling` links between
/// emitted children to reach that slot.
///
/// Returns `None` when the requested visitor slot's bit is unset (the
/// caller's getter then yields `None` for an `Option`-typed field) or when
/// a sibling chain is truncated.
#[inline]
#[must_use]
pub fn child_at_visitor_index(
    _sf: &LazySourceFile<'_>,
    _parent_index: u32,
    _bitmask: u8,
    _visitor_index: u8,
) -> Option<u32> {
    todo!("Phase 1.1b: walk visitor bits, follow next_sibling links to the target child")
}
