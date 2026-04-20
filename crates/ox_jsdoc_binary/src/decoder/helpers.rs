//! Common helpers shared by lazy node implementations.
//!
//! See `design/007-binary-ast/rust-impl.md#helper-functions-shared-parts-for-reading-binary-ast`.

use crate::format::node_record::{
    NEXT_SIBLING_OFFSET, NODE_DATA_OFFSET, NODE_RECORD_SIZE, PARENT_INDEX_OFFSET, PAYLOAD_MASK,
    TYPE_TAG_SHIFT, TypeTag,
};

use super::source_file::LazySourceFile;

/// Read a little-endian `u32` at `offset` from `bytes`.
///
/// The buffer is expected to be at least `offset + 4` bytes long; the
/// caller (via the lazy decoder) guarantees this through Header validation.
#[inline]
#[must_use]
pub fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

/// Read a little-endian `u16` at `offset` from `bytes`.
#[inline]
#[must_use]
pub fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

/// Resolve the Extended Data byte offset for the node at `node_index`.
///
/// The node must use Extended type Node Data (`0b10`); calling this on a
/// Children/String/Reserved node debug-asserts in development builds.
#[inline]
#[must_use]
pub fn ext_offset(sf: &LazySourceFile<'_>, node_index: u32) -> u32 {
    let nd = read_node_data(sf, node_index);
    let tag = TypeTag::from_u32((nd >> TYPE_TAG_SHIFT) & 0b11)
        .expect("Node Data type tag bits cover 0..=3 by construction");
    debug_assert_eq!(
        tag,
        TypeTag::Extended,
        "ext_offset called on a non-Extended node ({tag:?})"
    );
    sf.extended_data_offset + (nd & PAYLOAD_MASK)
}

/// Read the raw 32-bit Node Data field for the given node.
#[inline]
#[must_use]
pub fn read_node_data(sf: &LazySourceFile<'_>, node_index: u32) -> u32 {
    let off = sf.nodes_offset as usize + node_index as usize * NODE_RECORD_SIZE + NODE_DATA_OFFSET;
    read_u32(sf.bytes(), off)
}

/// Read the `next_sibling` field for the given node.
#[inline]
#[must_use]
pub fn read_next_sibling(sf: &LazySourceFile<'_>, node_index: u32) -> u32 {
    let off =
        sf.nodes_offset as usize + node_index as usize * NODE_RECORD_SIZE + NEXT_SIBLING_OFFSET;
    read_u32(sf.bytes(), off)
}

/// Read the `parent_index` field for the given node.
#[inline]
#[must_use]
pub fn read_parent_index(sf: &LazySourceFile<'_>, node_index: u32) -> u32 {
    let off =
        sf.nodes_offset as usize + node_index as usize * NODE_RECORD_SIZE + PARENT_INDEX_OFFSET;
    read_u32(sf.bytes(), off)
}

/// Return the first child of `parent_index` (= `parent_index + 1` if it
/// exists and its `parent_index` field equals `parent_index`).
///
/// Returns `None` when the parent has no child.
#[inline]
#[must_use]
pub fn first_child(sf: &LazySourceFile<'_>, parent_index: u32) -> Option<u32> {
    let candidate = parent_index + 1;
    if candidate >= sf.node_count {
        return None;
    }
    if read_parent_index(sf, candidate) == parent_index {
        Some(candidate)
    } else {
        None
    }
}

/// Resolve the 30-bit String payload of a String-type node into its
/// underlying string. `None` when the writer used the
/// [`crate::format::node_record::STRING_PAYLOD_NONE_SENTINEL`] sentinel.
#[inline]
#[must_use]
pub fn string_payload<'a>(sf: &LazySourceFile<'a>, node_index: u32) -> Option<&'a str> {
    let nd = read_node_data(sf, node_index);
    debug_assert_eq!(
        TypeTag::from_u32((nd >> TYPE_TAG_SHIFT) & 0b11),
        Ok(TypeTag::String),
        "string_payload called on a non-String node"
    );
    sf.get_string(nd & PAYLOAD_MASK)
}

/// Read the Children bitmask from the 30-bit Node Data payload of a
/// Children-type node.
#[inline]
#[must_use]
pub fn children_bitmask_payload(sf: &LazySourceFile<'_>, node_index: u32) -> u32 {
    let nd = read_node_data(sf, node_index);
    debug_assert_eq!(
        TypeTag::from_u32((nd >> TYPE_TAG_SHIFT) & 0b11),
        Ok(TypeTag::Children),
        "children_bitmask_payload called on a non-Children node"
    );
    nd & PAYLOAD_MASK
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
    sf: &LazySourceFile<'_>,
    parent_index: u32,
    bitmask: u8,
    visitor_index: u8,
) -> Option<u32> {
    if (bitmask & (1u8 << visitor_index)) == 0 {
        return None;
    }
    // Count the number of set bits below `visitor_index` to know how many
    // siblings to walk past.
    let mask_below = (1u8 << visitor_index).wrapping_sub(1);
    let skip = (bitmask & mask_below).count_ones();

    let mut child = parent_index + 1;
    for _ in 0..skip {
        let next = read_next_sibling(sf, child);
        if next == 0 {
            return None; // truncated sibling chain
        }
        child = next;
    }
    Some(child)
}
