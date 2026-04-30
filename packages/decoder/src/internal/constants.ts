/**
 * Binary AST format constants — JS mirror of `crates/ox_jsdoc_binary/src/format/`.
 *
 * Keep these in sync with the Rust constants. Phase 4 will code-generate
 * both sides from a single schema.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

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

// ---------------------------------------------------------------------------
// description_raw_span (Phase 5: opt-in via per-node Common Data bit)
// ---------------------------------------------------------------------------
// See `design/008-oxlint-oxfmt-support/README.md` §4.2 for the wire layout.
//
// `description_raw_span` is `(u32 start, u32 end)` UTF-8 byte offsets
// relative to the root's source text region (resolve via
// `root.source_offset_in_data + start`). When the per-node
// `has_description_raw_span` Common Data bit is set, the 8-byte span sits
// at the **end** of the Extended Data record (basic ED tail or compat ED
// tail, depending on whether compat_mode is on).
//
// Per-mode total ED sizes (matrix from §5.2):
//   Block | preserve=false | preserve=true
//   basic | 68             | 76 (= basic 68 + 8)
//   compat| 90 (basic 68 + compat tail 22) | 98 (compat 90 + 8)
//   Tag   | preserve=false | preserve=true
//   basic | 38             | 46 (= basic 38 + 8)
//   compat| 80 (basic 38 + compat tail 42) | 88 (compat 80 + 8)
//
// span offset within the ED record = the corresponding base size:
//   Block: compat ? 90 : 68     Tag: compat ? 80 : 38

/** Basic-mode Extended Data size for JsdocBlock (also the span offset
 *  within ED when compat_mode = false). */
export const JSDOC_BLOCK_BASIC_SIZE = 68
/** Compat-mode Extended Data size for JsdocBlock (also the span offset
 *  within ED when compat_mode = true). */
export const JSDOC_BLOCK_COMPAT_SIZE = 90
/** Common Data bit signalling presence of `description_raw_span` on a
 *  JsdocBlock ED record. Bit 0 of the 6-bit Common Data field. */
export const JSDOC_BLOCK_HAS_DESCRIPTION_RAW_SPAN_BIT = 1 << 0

/** Basic-mode Extended Data size for JsdocTag (also the span offset
 *  within ED when compat_mode = false). */
export const JSDOC_TAG_BASIC_SIZE = 38
/** Compat-mode Extended Data size for JsdocTag (also the span offset
 *  within ED when compat_mode = true). */
export const JSDOC_TAG_COMPAT_SIZE = 80
/** Common Data bit signalling presence of `description_raw_span` on a
 *  JsdocTag ED record. Bit 1 of the 6-bit Common Data field
 *  (bit 0 = optional). */
export const JSDOC_TAG_HAS_DESCRIPTION_RAW_SPAN_BIT = 1 << 1
