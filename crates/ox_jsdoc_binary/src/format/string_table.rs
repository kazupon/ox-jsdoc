//! String table (String Offsets + String Data) constants.
//!
//! See `design/007-binary-ast/format.md#string-table` for the full layout.
//!
//! - **String Offsets**: 8 bytes per string (`u32 start, u32 end`).
//! - **String Data**: contiguous UTF-8 bytes; strings are referenced by an
//!   index into the String Offsets table.
//!
//! String indices stored inside Extended Data fields are u16 (capped at the
//! [`STRING_TABLE_MAX_INDEX`] below). String indices stored in the 30-bit
//! Node Data payload (String-type leaves) use the wider range up to
//! [`super::node_record::PAYLOAD_MAX`].

/// Size of one String Offsets entry in bytes (`u32 start` + `u32 end`).
pub const STRING_OFFSET_ENTRY_SIZE: usize = 8;

/// Sentinel for `Option<&str> = None` stored as a u16 string index in
/// Extended Data (`0xFFFF`).
pub const U16_NONE_SENTINEL: u16 = 0xFFFF;

/// Maximum *valid* string index when the slot is u16 (`0xFFFE`).
///
/// `0xFFFF` is reserved as the [`U16_NONE_SENTINEL`].
pub const STRING_TABLE_MAX_INDEX: u16 = 0xFFFE;

/// Maximum number of unique strings the encoder may write before it must
/// either drop dedup, error out, or use a wider slot. This matches
/// [`STRING_TABLE_MAX_INDEX`] + 1 (since indices are zero-based).
pub const STRING_TABLE_MAX_COUNT: usize = STRING_TABLE_MAX_INDEX as usize + 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_size_is_8() {
        assert_eq!(STRING_OFFSET_ENTRY_SIZE, 8);
    }

    #[test]
    fn sentinels_do_not_overlap_valid_indices() {
        assert!(STRING_TABLE_MAX_INDEX < U16_NONE_SENTINEL);
        assert_eq!(STRING_TABLE_MAX_INDEX, U16_NONE_SENTINEL - 1);
    }

    #[test]
    fn max_count_matches_max_index() {
        assert_eq!(STRING_TABLE_MAX_COUNT, STRING_TABLE_MAX_INDEX as usize + 1);
    }
}
