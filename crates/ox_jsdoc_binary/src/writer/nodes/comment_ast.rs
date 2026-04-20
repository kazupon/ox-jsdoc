//! `write_*` helpers for the 15 comment AST kinds (`0x01 - 0x0F`).
//!
//! Phase 1.0b: signatures only; bodies are `unimplemented!()`.
//!
//! Convention: every helper takes the writer first, then the node's
//! `Span`, then its `parent_index`, followed by the per-Kind payload
//! parameters. Each helper returns the [`NodeIndex`] of the freshly
//! written node so that the parser can wire it as a child of its parent
//! (via backpatched `next_sibling`).

use oxc_span::Span;

use super::super::{BinaryWriter, ExtOffset, StringIndex};
use super::NodeIndex;

// ---------------------------------------------------------------------------
// 0x01 JsdocBlock (Extended, root)
// ---------------------------------------------------------------------------

/// Write a `JsdocBlock` (Kind `0x01`, Extended type).
///
/// Extended Data layout (basic 18 bytes, compat extends to 40 bytes):
/// see `design/007-binary-ast/format.md` "JsdocBlock Children bitmask"
/// and "JsdocBlock Extended Data complete field layout".
///
/// Common Data: unused. The parser must subsequently emit the
/// `descriptionLines`, `tags`, and `inlineTags` NodeLists in visitor order
/// so the bitmask written here remains accurate.
pub fn write_jsdoc_block(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _description: Option<StringIndex>,
    _delimiter: StringIndex,
    _post_delimiter: StringIndex,
    _terminal: StringIndex,
    _line_end: StringIndex,
    _initial: StringIndex,
    _delimiter_line_break: StringIndex,
    _preterminal_line_break: StringIndex,
    _children_bitmask: u8,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x01 + 18/40-byte Extended Data; backpatch siblings")
}

/// Allocate the compat-mode tail (`bytes 18..=39`) on a previously written
/// `JsdocBlock` Extended Data record. Phase 1.0b helper exposed for the
/// parser; bodies will be filled in Phase 1.1a once compat layout lands.
pub fn write_jsdoc_block_compat_tail(
    _writer: &mut BinaryWriter<'_>,
    _ext_offset: ExtOffset,
    _end_line: u32,
    _description_start_line: Option<u32>,
    _description_end_line: Option<u32>,
    _last_description_line: Option<u32>,
    _has_preterminal_description: u8,
    _has_preterminal_tag_description: Option<u8>,
) {
    unimplemented!("Phase 1.1a: write 22-byte compat tail at ext_offset + 18")
}

// ---------------------------------------------------------------------------
// 0x02 JsdocDescriptionLine (String / Extended in compat)
// ---------------------------------------------------------------------------

/// Write a `JsdocDescriptionLine` (Kind `0x02`).
///
/// Node Data type depends on `compat_mode`:
/// - basic: `String` type — payload = description string index
/// - compat: `Extended` type — Extended Data holds 4 string indices
///   (description + 3 delimiters)
pub fn write_jsdoc_description_line(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _description: StringIndex,
    _delimiter: Option<StringIndex>,
    _post_delimiter: Option<StringIndex>,
    _initial: Option<StringIndex>,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: branch on compat_mode for Node Data type")
}

// ---------------------------------------------------------------------------
// 0x03 JsdocTag (Extended)
// ---------------------------------------------------------------------------

/// Write a `JsdocTag` (Kind `0x03`, Extended type).
///
/// Common Data: `bit0 = optional`.
/// Extended Data layout (basic 8 bytes, compat 22 bytes):
/// see "JsdocTag Children bitmask" in the format spec.
pub fn write_jsdoc_tag(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _optional: bool,
    _default_value: Option<StringIndex>,
    _description: Option<StringIndex>,
    _raw_body: Option<StringIndex>,
    _children_bitmask: u8,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x03 + 8/22-byte Extended Data")
}

// ---------------------------------------------------------------------------
// 0x04 JsdocTagName (String)
// ---------------------------------------------------------------------------

/// Write a `JsdocTagName` leaf (Kind `0x04`, String type).
pub fn write_jsdoc_tag_name(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _value: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x04 + 30-bit string payload")
}

// ---------------------------------------------------------------------------
// 0x05 JsdocTagNameValue (String)
// ---------------------------------------------------------------------------

/// Write a `JsdocTagNameValue` leaf (Kind `0x05`, String type).
pub fn write_jsdoc_tag_name_value(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _raw: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x05")
}

// ---------------------------------------------------------------------------
// 0x06 JsdocTypeSource (String)
// ---------------------------------------------------------------------------

/// Write a `JsdocTypeSource` leaf (Kind `0x06`, String type) — raw `{...}`
/// text.
pub fn write_jsdoc_type_source(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _raw: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x06")
}

// ---------------------------------------------------------------------------
// 0x07 JsdocTypeLine (String / Extended in compat)
// ---------------------------------------------------------------------------

/// Write a `JsdocTypeLine` (Kind `0x07`).
///
/// Mirrors `JsdocDescriptionLine`: basic = String type, compat = Extended.
pub fn write_jsdoc_type_line(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _raw_type: StringIndex,
    _delimiter: Option<StringIndex>,
    _post_delimiter: Option<StringIndex>,
    _initial: Option<StringIndex>,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: branch on compat_mode")
}

// ---------------------------------------------------------------------------
// 0x08 JsdocInlineTag (Extended)
// ---------------------------------------------------------------------------

/// Write a `JsdocInlineTag` (Kind `0x08`, Extended type).
///
/// Common Data: `bits[0:2] = format` (5 variants).
/// Extended Data: 6 bytes (`namepath_or_url` + `text` + `raw_body` u16 each).
pub fn write_jsdoc_inline_tag(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _format: u8,
    _namepath_or_url: Option<StringIndex>,
    _text: Option<StringIndex>,
    _raw_body: Option<StringIndex>,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x08")
}

// ---------------------------------------------------------------------------
// 0x09 JsdocGenericTagBody (Extended)
// ---------------------------------------------------------------------------

/// Write a `JsdocGenericTagBody` (Kind `0x09`, Extended type).
///
/// Common Data: `bit0 = has_dash_separator`.
/// Extended Data: 4 bytes (Children bitmask + reserved + description u16).
pub fn write_jsdoc_generic_tag_body(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _has_dash_separator: bool,
    _description: Option<StringIndex>,
    _children_bitmask: u8,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x09")
}

// ---------------------------------------------------------------------------
// 0x0A JsdocBorrowsTagBody (Children)
// ---------------------------------------------------------------------------

/// Write a `JsdocBorrowsTagBody` (Kind `0x0A`, Children type).
///
/// 30-bit payload = Children bitmask. Subsequently the parser must emit
/// `source` and `target` child nodes in visitor order.
pub fn write_jsdoc_borrows_tag_body(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _children_bitmask: u32,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x0A")
}

// ---------------------------------------------------------------------------
// 0x0B JsdocRawTagBody (String)
// ---------------------------------------------------------------------------

/// Write a `JsdocRawTagBody` leaf (Kind `0x0B`, String type).
pub fn write_jsdoc_raw_tag_body(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _raw: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x0B")
}

// ---------------------------------------------------------------------------
// 0x0C JsdocParameterName (Extended)
// ---------------------------------------------------------------------------

/// Write a `JsdocParameterName` (Kind `0x0C`, Extended type).
///
/// Common Data: `bit0 = optional`.
/// Extended Data: 4 bytes (`path` + `default_value` u16 each).
pub fn write_jsdoc_parameter_name(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _optional: bool,
    _path: StringIndex,
    _default_value: Option<StringIndex>,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x0C")
}

// ---------------------------------------------------------------------------
// 0x0D JsdocNamepathSource (String)
// ---------------------------------------------------------------------------

/// Write a `JsdocNamepathSource` leaf (Kind `0x0D`, String type).
pub fn write_jsdoc_namepath_source(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _raw: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x0D")
}

// ---------------------------------------------------------------------------
// 0x0E JsdocIdentifier (String)
// ---------------------------------------------------------------------------

/// Write a `JsdocIdentifier` leaf (Kind `0x0E`, String type).
pub fn write_jsdoc_identifier(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _name: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x0E")
}

// ---------------------------------------------------------------------------
// 0x0F JsdocText (String)
// ---------------------------------------------------------------------------

/// Write a `JsdocText` leaf (Kind `0x0F`, String type).
pub fn write_jsdoc_text(
    _writer: &mut BinaryWriter<'_>,
    _span: Span,
    _parent_index: NodeIndex,
    _value: StringIndex,
) -> NodeIndex {
    unimplemented!("Phase 1.1a: emit Kind 0x0F")
}
