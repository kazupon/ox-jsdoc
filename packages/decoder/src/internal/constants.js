/**
 * Binary AST format constants — JS mirror of `crates/ox_jsdoc_binary/src/format/`.
 *
 * Keep these in sync with the Rust constants. Phase 4 will code-generate
 * both sides from a single schema.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

// ---------------------------------------------------------------------------
// Header (40 bytes)
// ---------------------------------------------------------------------------

export const HEADER_SIZE = 40
export const VERSION_OFFSET = 0
export const FLAGS_OFFSET = 1
export const ROOT_ARRAY_OFFSET_FIELD = 4
export const STRING_OFFSETS_OFFSET_FIELD = 8
export const STRING_DATA_OFFSET_FIELD = 12
export const EXTENDED_DATA_OFFSET_FIELD = 16
export const DIAGNOSTICS_OFFSET_FIELD = 20
export const NODES_OFFSET_FIELD = 24
export const NODE_COUNT_FIELD = 28
export const SOURCE_TEXT_LENGTH_FIELD = 32
export const ROOT_COUNT_FIELD = 36

export const SUPPORTED_MAJOR = 1
export const SUPPORTED_MINOR = 0
export const MAJOR_SHIFT = 4
export const MINOR_MASK = 0x0f

export const COMPAT_MODE_BIT = 0x01

// ---------------------------------------------------------------------------
// Node record (24 bytes / node)
// ---------------------------------------------------------------------------

export const NODE_RECORD_SIZE = 24
export const KIND_OFFSET = 0
export const COMMON_DATA_OFFSET = 1
export const POS_OFFSET = 4
export const END_OFFSET = 8
export const NODE_DATA_OFFSET = 12
export const PARENT_INDEX_OFFSET = 16
export const NEXT_SIBLING_OFFSET = 20

export const COMMON_DATA_MASK = 0b0011_1111

// ---------------------------------------------------------------------------
// Node Data 32-bit packing
// ---------------------------------------------------------------------------

export const TYPE_TAG_SHIFT = 30
export const TYPE_TAG_MASK = 0b11
export const PAYLOAD_MASK = 0x3fff_ffff

/** TypeTag: payload is a 30-bit Children bitmask (visitor order). */
export const TYPE_TAG_CHILDREN = 0b00
/** TypeTag: payload is a 30-bit String Offsets index (string-leaf nodes,
 *  fallback for length >= 256 or offset > 4 MB). */
export const TYPE_TAG_STRING = 0b01
/** TypeTag: payload is a 30-bit byte offset into the Extended Data section. */
export const TYPE_TAG_EXTENDED = 0b10
/** TypeTag: payload is a packed `(offset: u22, length: u8)` pointing into
 *  String Data directly (Path B-leaf inline path for short strings). */
export const TYPE_TAG_STRING_INLINE = 0b11

/** Sentinel for "absent" stored in a 30-bit String payload. */
export const STRING_PAYLOAD_NONE_SENTINEL = 0x3fff_ffff

/** Number of bits the inline-String payload reserves for the offset. */
export const STRING_INLINE_OFFSET_BITS = 22
/** Number of bits the inline-String payload reserves for the length (low bits). */
export const STRING_INLINE_LENGTH_BITS = 8
/** Mask isolating the length portion of an inline payload. */
export const STRING_INLINE_LENGTH_MASK = (1 << STRING_INLINE_LENGTH_BITS) - 1

// ---------------------------------------------------------------------------
// Root Index entry (12 bytes)
// ---------------------------------------------------------------------------

export const ROOT_INDEX_ENTRY_SIZE = 12
export const NODE_INDEX_OFFSET = 0
export const SOURCE_OFFSET_FIELD = 4
export const BASE_OFFSET_FIELD = 8

// ---------------------------------------------------------------------------
// String table (offsets table for string-leaf nodes + diagnostics)
// ---------------------------------------------------------------------------

export const STRING_OFFSET_ENTRY_SIZE = 8

// ---------------------------------------------------------------------------
// String field (6 bytes per slot, inlined in Extended Data)
// ---------------------------------------------------------------------------

/** Size of one inline `StringField` slot in bytes. */
export const STRING_FIELD_SIZE = 6
/** Offset value used by the `None` sentinel (matches Rust's u32::MAX). */
export const STRING_FIELD_NONE_OFFSET = 0xffff_ffff
/** Length value used by the `None` sentinel (always 0). */
export const STRING_FIELD_NONE_LENGTH = 0
