//! Extended Data section constants.
//!
//! See `design/007-binary-ast/format.md#extended-data-section` for the full
//! specification. Per-Kind layouts (e.g. `JsdocBlock` 18 / 40 bytes,
//! `JsdocTag` 8 / 22 bytes) live in the per-Kind layout helpers shipped in
//! later sub-phases (Phase 1.1a writer / 1.1b decoder).

/// Alignment requirement for each Extended Data record start (8 bytes).
///
/// The encoder inserts zero-fill padding before each new record so that
/// every offset reserved in the 30-bit Node Data payload is divisible by
/// [`EXTENDED_DATA_ALIGNMENT`]. This guarantees that u32 fields can be read
/// without unaligned-access penalties on every supported target.
pub const EXTENDED_DATA_ALIGNMENT: usize = 8;

/// Maximum byte offset that fits in the 30-bit Node Data payload
/// (`2^30 - 1`). Cross-checked against
/// [`super::node_record::PAYLOAD_MAX`].
pub const EXTENDED_DATA_MAX_OFFSET: u32 = (1u32 << 30) - 1;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::node_record::PAYLOAD_MAX;

    #[test]
    fn alignment_is_8_bytes() {
        assert_eq!(EXTENDED_DATA_ALIGNMENT, 8);
    }

    #[test]
    fn max_offset_matches_node_data_payload_max() {
        assert_eq!(EXTENDED_DATA_MAX_OFFSET, PAYLOAD_MAX);
    }
}
