// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! `write_*` helpers for the 15 comment AST kinds (`0x01 - 0x0F`).
//!
//! Convention: every helper takes the writer first, then the node's
//! `Span`, then its `parent_index` (`0` = sentinel parent = root),
//! followed by the per-Kind payload parameters. Each helper returns the
//! [`NodeIndex`] of the freshly written node so that the parser can wire
//! it as a child of its parent (via backpatched `next_sibling`).

use oxc_span::Span;

use crate::format::kind::Kind;
use crate::format::node_record::{TypeTag, pack_node_data};

use super::super::{BinaryWriter, ExtOffset, StringIndex};
use super::NodeIndex;

// ---------------------------------------------------------------------------
// 0x01 JsdocBlock (Extended, root)
// ---------------------------------------------------------------------------

/// Write a `JsdocBlock` (Kind `0x01`, Extended type).
///
/// Extended Data layout (basic 18 bytes; compat extends to 40 bytes via
/// [`write_jsdoc_block_compat_tail`]):
///
/// ```text
/// byte 0     : Children bitmask (u8)
/// byte 1     : padding (u8)
/// byte 2-3   : description string index           (u16)
/// byte 4-5   : delimiter                          (u16)
/// byte 6-7   : post_delimiter                     (u16)
/// byte 8-9   : terminal                           (u16)
/// byte 10-11 : line_end                           (u16)
/// byte 12-13 : initial                            (u16)
/// byte 14-15 : delimiter_line_break               (u16)
/// byte 16-17 : preterminal_line_break             (u16)
/// ```
///
/// The caller supplies `description = None` as `0xFFFF`. All other slots
/// are mandatory u16 indices written as-is.
#[allow(clippy::too_many_arguments)]
pub fn write_jsdoc_block(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    description: Option<StringIndex>,
    delimiter: StringIndex,
    post_delimiter: StringIndex,
    terminal: StringIndex,
    line_end: StringIndex,
    initial: StringIndex,
    delimiter_line_break: StringIndex,
    preterminal_line_break: StringIndex,
    children_bitmask: u8,
) -> NodeIndex {
    let basic_size = 18;
    let total_size = if writer.compat_mode() { 40 } else { basic_size };
    // Build the 18-byte basic record on the stack so the buffer write is
    // a single 18-byte memcpy instead of nine `write_u16` dispatches +
    // their per-call slice bounds checks. The compat tail (bytes 18..40)
    // is left as the zero fill `reserve_mut` already laid down; the
    // `_compat_tail` helper patches it later.
    let desc = opt_string_index(description).to_le_bytes();
    let delim = delimiter.as_u16().to_le_bytes();
    let pdelim = post_delimiter.as_u16().to_le_bytes();
    let term = terminal.as_u16().to_le_bytes();
    let le = line_end.as_u16().to_le_bytes();
    let init = initial.as_u16().to_le_bytes();
    let dlb = delimiter_line_break.as_u16().to_le_bytes();
    let plb = preterminal_line_break.as_u16().to_le_bytes();
    let record: [u8; 18] = [
        children_bitmask, 0,
        desc[0], desc[1],
        delim[0], delim[1],
        pdelim[0], pdelim[1],
        term[0], term[1],
        le[0], le[1],
        init[0], init[1],
        dlb[0], dlb[1],
        plb[0], plb[1],
    ];
    let (off, dst) = writer.extended.reserve_mut(total_size);
    dst[..basic_size].copy_from_slice(&record);
    writer.emit_extended_node(parent_index, Kind::JsdocBlock, 0, span, off)
}

/// Patch the compat-mode tail (`bytes 18..=39`) on a previously written
/// `JsdocBlock` Extended Data record. Only call this when
/// [`BinaryWriter::compat_mode`] is `true` (the basic write helper already
/// reserved the extra bytes).
pub fn write_jsdoc_block_compat_tail(
    writer: &mut BinaryWriter<'_>,
    ext_offset: ExtOffset,
    end_line: u32,
    description_start_line: Option<u32>,
    description_end_line: Option<u32>,
    last_description_line: Option<u32>,
    has_preterminal_description: u8,
    has_preterminal_tag_description: Option<u8>,
) {
    debug_assert!(
        writer.compat_mode(),
        "write_jsdoc_block_compat_tail called but compat_mode is off"
    );
    let dst = writer.extended.slice_mut(ext_offset, 40);
    // bytes 18-19 are u32 alignment padding (already zero)
    dst[20..24].copy_from_slice(&end_line.to_le_bytes());
    dst[24..28].copy_from_slice(&opt_u32_sentinel(description_start_line).to_le_bytes());
    dst[28..32].copy_from_slice(&opt_u32_sentinel(description_end_line).to_le_bytes());
    dst[32..36].copy_from_slice(&opt_u32_sentinel(last_description_line).to_le_bytes());
    dst[36] = has_preterminal_description;
    dst[37] = has_preterminal_tag_description.unwrap_or(0xFF);
    // bytes 38-39 trailing alignment (already zero)
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
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    description: StringIndex,
    delimiter: Option<StringIndex>,
    post_delimiter: Option<StringIndex>,
    initial: Option<StringIndex>,
) -> NodeIndex {
    if writer.compat_mode() {
        // Extended Data: byte 0-1 description, 2-3 delimiter, 4-5 post_delimiter, 6-7 initial
        let d = description.as_u16().to_le_bytes();
        let dl = opt_string_index(delimiter).to_le_bytes();
        let pd = opt_string_index(post_delimiter).to_le_bytes();
        let init = opt_string_index(initial).to_le_bytes();
        let record: [u8; 8] = [d[0], d[1], dl[0], dl[1], pd[0], pd[1], init[0], init[1]];
        let (off, dst) = writer.extended.reserve_mut(8);
        dst.copy_from_slice(&record);
        writer.emit_extended_node(parent_index, Kind::JsdocDescriptionLine, 0, span, off)
    } else {
        writer.emit_string_node(parent_index, Kind::JsdocDescriptionLine, 0, span, description)
    }
}

// ---------------------------------------------------------------------------
// 0x03 JsdocTag (Extended)
// ---------------------------------------------------------------------------

/// Write a `JsdocTag` (Kind `0x03`, Extended type).
///
/// Common Data: `bit0 = optional`. Extended Data layout (basic 8 bytes):
///
/// ```text
/// byte 0     : Children bitmask (u8)
/// byte 1     : padding (u8)
/// byte 2-3   : default_value (u16, 0xFFFF=None)
/// byte 4-5   : description    (u16, 0xFFFF=None)
/// byte 6-7   : raw_body       (u16, 0xFFFF=None)
/// ```
///
/// In compat mode the 7 delimiter string indices follow at byte 8..=21
/// (total 22 bytes); use [`write_jsdoc_tag_compat_tail`] to patch them.
#[allow(clippy::too_many_arguments)]
pub fn write_jsdoc_tag(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    optional: bool,
    default_value: Option<StringIndex>,
    description: Option<StringIndex>,
    raw_body: Option<StringIndex>,
    children_bitmask: u8,
) -> NodeIndex {
    let basic_size = 8;
    let total_size = if writer.compat_mode() { 22 } else { basic_size };
    let dv = opt_string_index(default_value).to_le_bytes();
    let desc = opt_string_index(description).to_le_bytes();
    let rb = opt_string_index(raw_body).to_le_bytes();
    let record: [u8; 8] = [
        children_bitmask, 0,
        dv[0], dv[1],
        desc[0], desc[1],
        rb[0], rb[1],
    ];
    let (off, dst) = writer.extended.reserve_mut(total_size);
    dst[..basic_size].copy_from_slice(&record);
    writer.emit_extended_node(parent_index, Kind::JsdocTag, optional as u8, span, off)
}

/// Write the 7 delimiter string indices (14 bytes) at the compat tail of a
/// previously written `JsdocTag` Extended Data record.
#[allow(clippy::too_many_arguments)]
pub fn write_jsdoc_tag_compat_tail(
    writer: &mut BinaryWriter<'_>,
    ext_offset: ExtOffset,
    delimiter: StringIndex,
    post_delimiter: StringIndex,
    post_tag: StringIndex,
    post_type: StringIndex,
    post_name: StringIndex,
    initial: StringIndex,
    line_end: StringIndex,
) {
    debug_assert!(writer.compat_mode());
    let dst = writer.extended.slice_mut(ext_offset, 22);
    write_u16(dst, 8, delimiter.as_u16());
    write_u16(dst, 10, post_delimiter.as_u16());
    write_u16(dst, 12, post_tag.as_u16());
    write_u16(dst, 14, post_type.as_u16());
    write_u16(dst, 16, post_name.as_u16());
    write_u16(dst, 18, initial.as_u16());
    write_u16(dst, 20, line_end.as_u16());
}

// ---------------------------------------------------------------------------
// 0x04 JsdocTagName (String leaf)
// ---------------------------------------------------------------------------

/// Write a `JsdocTagName` leaf (Kind `0x04`, String type).
pub fn write_jsdoc_tag_name(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    value: StringIndex,
) -> NodeIndex {
    writer.emit_string_node(parent_index, Kind::JsdocTagName, 0, span, value)
}

// ---------------------------------------------------------------------------
// 0x05 JsdocTagNameValue (String leaf)
// ---------------------------------------------------------------------------

/// Write a `JsdocTagNameValue` leaf (Kind `0x05`, String type).
pub fn write_jsdoc_tag_name_value(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    raw: StringIndex,
) -> NodeIndex {
    writer.emit_string_node(parent_index, Kind::JsdocTagNameValue, 0, span, raw)
}

// ---------------------------------------------------------------------------
// 0x06 JsdocTypeSource (String leaf)
// ---------------------------------------------------------------------------

/// Write a `JsdocTypeSource` leaf (Kind `0x06`, String type).
pub fn write_jsdoc_type_source(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    raw: StringIndex,
) -> NodeIndex {
    writer.emit_string_node(parent_index, Kind::JsdocTypeSource, 0, span, raw)
}

// ---------------------------------------------------------------------------
// 0x07 JsdocTypeLine (String / Extended in compat)
// ---------------------------------------------------------------------------

/// Write a `JsdocTypeLine` (Kind `0x07`).
///
/// Mirrors [`write_jsdoc_description_line`]: basic = String type, compat =
/// Extended with `raw_type` + 3 delimiter u16 fields.
pub fn write_jsdoc_type_line(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    raw_type: StringIndex,
    delimiter: Option<StringIndex>,
    post_delimiter: Option<StringIndex>,
    initial: Option<StringIndex>,
) -> NodeIndex {
    if writer.compat_mode() {
        let off = writer.extended.reserve(8);
        let dst = writer.extended.slice_mut(off, 8);
        write_u16(dst, 0, raw_type.as_u16());
        write_u16(dst, 2, opt_string_index(delimiter));
        write_u16(dst, 4, opt_string_index(post_delimiter));
        write_u16(dst, 6, opt_string_index(initial));
        writer.emit_extended_node(parent_index, Kind::JsdocTypeLine, 0, span, off)
    } else {
        writer.emit_string_node(parent_index, Kind::JsdocTypeLine, 0, span, raw_type)
    }
}

// ---------------------------------------------------------------------------
// 0x08 JsdocInlineTag (Extended)
// ---------------------------------------------------------------------------

/// Write a `JsdocInlineTag` (Kind `0x08`, Extended type).
///
/// Common Data: `bits[0:2] = format`. Extended Data: 6 bytes
/// (`namepath_or_url` + `text` + `raw_body`, each `u16`).
pub fn write_jsdoc_inline_tag(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    format: u8,
    namepath_or_url: Option<StringIndex>,
    text: Option<StringIndex>,
    raw_body: Option<StringIndex>,
) -> NodeIndex {
    let off = writer.extended.reserve(6);
    let dst = writer.extended.slice_mut(off, 6);
    write_u16(dst, 0, opt_string_index(namepath_or_url));
    write_u16(dst, 2, opt_string_index(text));
    write_u16(dst, 4, opt_string_index(raw_body));
    writer.emit_extended_node(
        parent_index,
        Kind::JsdocInlineTag,
        format & 0b111,
        span,
        off,
    )
}

// ---------------------------------------------------------------------------
// 0x09 JsdocGenericTagBody (Extended)
// ---------------------------------------------------------------------------

/// Write a `JsdocGenericTagBody` (Kind `0x09`, Extended type).
///
/// Common Data: `bit0 = has_dash_separator`. Extended Data: 4 bytes
/// (Children bitmask u8 + padding u8 + description u16).
pub fn write_jsdoc_generic_tag_body(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    has_dash_separator: bool,
    description: Option<StringIndex>,
    children_bitmask: u8,
) -> NodeIndex {
    let off = writer.extended.reserve(4);
    {
        let dst = writer.extended.slice_mut(off, 4);
        dst[0] = children_bitmask;
        dst[1] = 0;
        write_u16(dst, 2, opt_string_index(description));
    }
    writer.emit_extended_node(
        parent_index,
        Kind::JsdocGenericTagBody,
        has_dash_separator as u8,
        span,
        off,
    )
}

// ---------------------------------------------------------------------------
// 0x0A JsdocBorrowsTagBody (Children)
// ---------------------------------------------------------------------------

/// Write a `JsdocBorrowsTagBody` (Kind `0x0A`, Children type; 2 children:
/// `source` + `target`).
pub fn write_jsdoc_borrows_tag_body(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    children_bitmask: u32,
) -> NodeIndex {
    writer.emit_children_node(
        parent_index,
        Kind::JsdocBorrowsTagBody,
        0,
        span,
        children_bitmask,
    )
}

// ---------------------------------------------------------------------------
// 0x0B JsdocRawTagBody (String leaf)
// ---------------------------------------------------------------------------

/// Write a `JsdocRawTagBody` leaf (Kind `0x0B`, String type).
pub fn write_jsdoc_raw_tag_body(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    raw: StringIndex,
) -> NodeIndex {
    writer.emit_string_node(parent_index, Kind::JsdocRawTagBody, 0, span, raw)
}

// ---------------------------------------------------------------------------
// 0x0C JsdocParameterName (Extended)
// ---------------------------------------------------------------------------

/// Write a `JsdocParameterName` (Kind `0x0C`, Extended type).
///
/// Common Data: `bit0 = optional`. Extended Data: 4 bytes (`path` u16 +
/// `default_value` u16).
pub fn write_jsdoc_parameter_name(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    optional: bool,
    path: StringIndex,
    default_value: Option<StringIndex>,
) -> NodeIndex {
    let off = writer.extended.reserve(4);
    {
        let dst = writer.extended.slice_mut(off, 4);
        write_u16(dst, 0, path.as_u16());
        write_u16(dst, 2, opt_string_index(default_value));
    }
    writer.emit_extended_node(
        parent_index,
        Kind::JsdocParameterName,
        optional as u8,
        span,
        off,
    )
}

// ---------------------------------------------------------------------------
// 0x0D JsdocNamepathSource (String leaf)
// ---------------------------------------------------------------------------

/// Write a `JsdocNamepathSource` leaf (Kind `0x0D`, String type).
pub fn write_jsdoc_namepath_source(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    raw: StringIndex,
) -> NodeIndex {
    writer.emit_string_node(parent_index, Kind::JsdocNamepathSource, 0, span, raw)
}

// ---------------------------------------------------------------------------
// 0x0E JsdocIdentifier (String leaf)
// ---------------------------------------------------------------------------

/// Write a `JsdocIdentifier` leaf (Kind `0x0E`, String type).
pub fn write_jsdoc_identifier(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    name: StringIndex,
) -> NodeIndex {
    writer.emit_string_node(parent_index, Kind::JsdocIdentifier, 0, span, name)
}

// ---------------------------------------------------------------------------
// 0x0F JsdocText (String leaf)
// ---------------------------------------------------------------------------

/// Write a `JsdocText` leaf (Kind `0x0F`, String type).
pub fn write_jsdoc_text(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    value: StringIndex,
) -> NodeIndex {
    writer.emit_string_node(parent_index, Kind::JsdocText, 0, span, value)
}

// ---------------------------------------------------------------------------
// 0x7F NodeList wrapper
// ---------------------------------------------------------------------------

/// Write a `NodeList` wrapper (Kind `0x7F`).
///
/// Per `format.md` "Special nodes", the 30-bit Node Data payload stores the
/// element count. Children follow the wrapper in DFS pre-order.
pub fn write_node_list(
    writer: &mut BinaryWriter<'_>,
    span: Span,
    parent_index: u32,
    element_count: u32,
) -> NodeIndex {
    let node_data = pack_node_data(TypeTag::Children, element_count);
    writer.emit_node_record(parent_index, Kind::NodeList, 0, span, node_data)
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

#[inline]
fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

#[inline]
fn opt_string_index(opt: Option<StringIndex>) -> u16 {
    use crate::format::string_table::U16_NONE_SENTINEL;
    opt.map(StringIndex::as_u16).unwrap_or(U16_NONE_SENTINEL)
}

#[inline]
fn opt_u32_sentinel(opt: Option<u32>) -> u32 {
    opt.unwrap_or(0xFFFF_FFFF)
}
