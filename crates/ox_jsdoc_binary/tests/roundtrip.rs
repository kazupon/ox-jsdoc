//! End-to-end roundtrip tests for the encoder + decoder.
//!
//! These tests build a small Binary AST through the writer API, run it
//! through `LazySourceFile::new`, and inspect the on-wire bytes to make
//! sure encoder and decoder agree on the format.
//!
//! Per-Pattern coverage:
//!
//! - String type leaf (Pattern 1): `JsdocText`, `TypeName`
//! - Children type (Pattern 2): `TypeFunction` (with bitmask + Common Data)
//! - Extended type (Pattern 3): `JsdocBlock` (basic 18 bytes)
//! - Compat extension: `JsdocBlock` 40-byte tail
//! - Pure leaf: `TypeNull`
//! - NodeList wrapper: payload count
//! - Sibling backpatch: two children of the same parent

use ox_jsdoc_binary::decoder::helpers::{ext_offset, read_u16, read_u32};
use ox_jsdoc_binary::decoder::source_file::LazySourceFile;
use ox_jsdoc_binary::format::header::SUPPORTED_VERSION_BYTE;
use ox_jsdoc_binary::format::kind::Kind;
use ox_jsdoc_binary::format::node_record::{
    COMMON_DATA_MASK, END_OFFSET, KIND_OFFSET, NEXT_SIBLING_OFFSET, NODE_DATA_OFFSET,
    NODE_RECORD_SIZE, PARENT_INDEX_OFFSET, PAYLOAD_MASK, POS_OFFSET, TYPE_TAG_SHIFT, TypeTag,
};
use ox_jsdoc_binary::writer::nodes::comment_ast::{
    write_jsdoc_block, write_jsdoc_block_compat_tail, write_jsdoc_text, write_node_list,
};
use ox_jsdoc_binary::writer::nodes::type_node::{write_type_function, write_type_name, write_type_null};
use ox_jsdoc_binary::writer::BinaryWriter;
use oxc_allocator::Allocator;
use oxc_span::Span;

/// Read the byte for the given node's `Kind` field.
fn node_kind(sf: &LazySourceFile<'_>, node_index: u32) -> u8 {
    sf.bytes()[sf.nodes_offset as usize + node_index as usize * NODE_RECORD_SIZE + KIND_OFFSET]
}

fn node_common_data(sf: &LazySourceFile<'_>, node_index: u32) -> u8 {
    sf.bytes()[sf.nodes_offset as usize + node_index as usize * NODE_RECORD_SIZE + 1]
        & COMMON_DATA_MASK
}

fn node_pos(sf: &LazySourceFile<'_>, node_index: u32) -> u32 {
    read_u32(
        sf.bytes(),
        sf.nodes_offset as usize + node_index as usize * NODE_RECORD_SIZE + POS_OFFSET,
    )
}

fn node_end(sf: &LazySourceFile<'_>, node_index: u32) -> u32 {
    read_u32(
        sf.bytes(),
        sf.nodes_offset as usize + node_index as usize * NODE_RECORD_SIZE + END_OFFSET,
    )
}

fn node_data(sf: &LazySourceFile<'_>, node_index: u32) -> u32 {
    read_u32(
        sf.bytes(),
        sf.nodes_offset as usize + node_index as usize * NODE_RECORD_SIZE + NODE_DATA_OFFSET,
    )
}

fn node_parent(sf: &LazySourceFile<'_>, node_index: u32) -> u32 {
    read_u32(
        sf.bytes(),
        sf.nodes_offset as usize + node_index as usize * NODE_RECORD_SIZE + PARENT_INDEX_OFFSET,
    )
}

fn node_next_sibling(sf: &LazySourceFile<'_>, node_index: u32) -> u32 {
    read_u32(
        sf.bytes(),
        sf.nodes_offset as usize + node_index as usize * NODE_RECORD_SIZE + NEXT_SIBLING_OFFSET,
    )
}

fn type_tag(node_data: u32) -> TypeTag {
    TypeTag::from_u32((node_data >> TYPE_TAG_SHIFT) & 0b11).unwrap()
}

fn payload(node_data: u32) -> u32 {
    node_data & PAYLOAD_MASK
}

#[test]
fn header_carries_version_and_section_offsets() {
    let arena = Allocator::default();
    let writer = BinaryWriter::new(&arena);
    let bytes = writer.finish();

    assert_eq!(bytes[0], SUPPORTED_VERSION_BYTE);
    let sf = LazySourceFile::new(&bytes).unwrap();
    assert!(sf.nodes_offset >= 40, "Nodes section sits past the Header");
    assert_eq!(sf.node_count, 1, "sentinel node[0] is pre-emitted");
}

#[test]
fn write_jsdoc_text_leaf_roundtrips() {
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    let value = writer.intern_string("hello");
    let node = write_jsdoc_text(&mut writer, Span::new(0, 5), 0, value);
    assert_eq!(node.as_u32(), 1, "first non-sentinel node lands at index 1");

    let bytes = writer.finish();
    let sf = LazySourceFile::new(&bytes).unwrap();
    assert_eq!(sf.node_count, 2);

    assert_eq!(node_kind(&sf, 1), Kind::JsdocText.as_u8());
    assert_eq!(node_common_data(&sf, 1), 0);
    assert_eq!(node_pos(&sf, 1), 0);
    assert_eq!(node_end(&sf, 1), 5);
    assert_eq!(node_parent(&sf, 1), 0, "root parent = sentinel");
    assert_eq!(node_next_sibling(&sf, 1), 0, "no sibling");

    let nd = node_data(&sf, 1);
    assert_eq!(type_tag(nd), TypeTag::String);
    assert_eq!(sf.get_string(payload(nd)), Some("hello"));
}

#[test]
fn write_type_name_string_payload_round_trips() {
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    let value = writer.intern_string("Foo");
    let _ = write_type_name(&mut writer, Span::new(0, 3), 0, value);

    let bytes = writer.finish();
    let sf = LazySourceFile::new(&bytes).unwrap();
    assert_eq!(node_kind(&sf, 1), Kind::TypeName.as_u8());
    let nd = node_data(&sf, 1);
    assert_eq!(type_tag(nd), TypeTag::String);
    assert_eq!(sf.get_string(payload(nd)), Some("Foo"));
}

#[test]
fn write_type_function_packs_common_data_and_bitmask() {
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    // constructor=true, arrow=false, parenthesis=true → common = 0b101 = 5
    // Children bitmask: parameters present (bit0) + return_type (bit1) only
    let _ = write_type_function(&mut writer, Span::new(0, 10), 0, true, false, true, 0b011);

    let bytes = writer.finish();
    let sf = LazySourceFile::new(&bytes).unwrap();
    assert_eq!(node_kind(&sf, 1), Kind::TypeFunction.as_u8());
    assert_eq!(node_common_data(&sf, 1), 0b0000_0101);

    let nd = node_data(&sf, 1);
    assert_eq!(type_tag(nd), TypeTag::Children);
    assert_eq!(payload(nd), 0b011);
}

#[test]
fn write_type_null_leaf_has_zero_payload() {
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    let _ = write_type_null(&mut writer, Span::new(7, 11), 0);

    let bytes = writer.finish();
    let sf = LazySourceFile::new(&bytes).unwrap();
    assert_eq!(node_kind(&sf, 1), Kind::TypeNull.as_u8());
    assert_eq!(node_pos(&sf, 1), 7);
    assert_eq!(node_end(&sf, 1), 11);
    let nd = node_data(&sf, 1);
    assert_eq!(type_tag(nd), TypeTag::Children);
    assert_eq!(payload(nd), 0);
}

#[test]
fn write_jsdoc_block_basic_extended_data_layout() {
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);

    let desc = writer.intern_string("hello");
    let delim = writer.intern_string("*");
    let post_delim = writer.intern_string(" ");
    let terminal = writer.intern_string("*/");
    let line_end = writer.intern_string("\n");
    let initial = writer.intern_string("");
    let lbreak = writer.intern_string("\n");
    let pre_lbreak = writer.intern_string("");

    let _ = write_jsdoc_block(
        &mut writer,
        Span::new(0, 50),
        0,
        Some(desc),
        delim,
        post_delim,
        terminal,
        line_end,
        initial,
        lbreak,
        pre_lbreak,
        0b000, // bitmask: no children
    );

    let bytes = writer.finish();
    let sf = LazySourceFile::new(&bytes).unwrap();

    assert_eq!(node_kind(&sf, 1), Kind::JsdocBlock.as_u8());
    let nd = node_data(&sf, 1);
    assert_eq!(type_tag(nd), TypeTag::Extended);

    // Extended Data should resolve to the start of the section.
    let ext_off = ext_offset(&sf, 1);
    assert_eq!(ext_off, sf.extended_data_offset, "first record starts at offset 0 of the section");

    // Children bitmask byte 0 = 0
    assert_eq!(sf.bytes()[ext_off as usize], 0);
    // description string index byte 2-3 = desc.as_u16()
    let desc_idx = read_u16(sf.bytes(), ext_off as usize + 2);
    assert_eq!(sf.get_string(desc_idx as u32), Some("hello"));
    // delimiter byte 4-5
    let delim_idx = read_u16(sf.bytes(), ext_off as usize + 4);
    assert_eq!(sf.get_string(delim_idx as u32), Some("*"));
}

#[test]
fn compat_mode_emits_22_byte_jsdoc_block_tail() {
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    writer.set_compat_mode(true);

    let s = writer.intern_string("");
    let _ = write_jsdoc_block(
        &mut writer,
        Span::new(0, 3),
        0,
        None,
        s, s, s, s, s, s, s,
        0,
    );

    // Apply compat tail to the same record (offset 0 of Extended Data).
    use ox_jsdoc_binary::writer::ExtOffset;
    let off = ExtOffset::from_u32(0).unwrap();
    write_jsdoc_block_compat_tail(&mut writer, off, 12, Some(0), Some(2), Some(2), 1, None);

    let bytes = writer.finish();
    let sf = LazySourceFile::new(&bytes).unwrap();
    assert!(sf.compat_mode);

    let ext = sf.extended_data_offset as usize;
    // end_line at byte 20 = 12
    assert_eq!(read_u32(sf.bytes(), ext + 20), 12);
    // description_start_line at byte 24 = 0
    assert_eq!(read_u32(sf.bytes(), ext + 24), 0);
    // last_description_line at byte 32 = 2
    assert_eq!(read_u32(sf.bytes(), ext + 32), 2);
    // has_preterminal_description = 1
    assert_eq!(sf.bytes()[ext + 36], 1);
    // has_preterminal_tag_description = None → 0xFF sentinel
    assert_eq!(sf.bytes()[ext + 37], 0xFF);
}

#[test]
fn next_sibling_backpatch_links_two_children() {
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);

    // Parent: a TypeUnion at index 1.
    use ox_jsdoc_binary::writer::nodes::type_node::write_type_union;
    let parent = write_type_union(&mut writer, Span::new(0, 20), 0, 0b1);
    assert_eq!(parent.as_u32(), 1);

    // Two children of `parent` (parent_index = 1).
    let v1 = writer.intern_string("string");
    let v2 = writer.intern_string("number");
    let c1 = write_type_name(&mut writer, Span::new(0, 6), parent.as_u32(), v1);
    let c2 = write_type_name(&mut writer, Span::new(7, 13), parent.as_u32(), v2);
    assert_eq!(c1.as_u32(), 2);
    assert_eq!(c2.as_u32(), 3);

    let bytes = writer.finish();
    let sf = LazySourceFile::new(&bytes).unwrap();

    // Child 1's next_sibling must point to child 2 (= 3).
    assert_eq!(node_next_sibling(&sf, 2), 3);
    // Child 2 has no further sibling.
    assert_eq!(node_next_sibling(&sf, 3), 0);
    // Both children share parent_index = 1.
    assert_eq!(node_parent(&sf, 2), 1);
    assert_eq!(node_parent(&sf, 3), 1);
}

#[test]
fn node_list_wrapper_payload_counts_elements() {
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    let _ = write_node_list(&mut writer, Span::new(0, 0), 0, 7);

    let bytes = writer.finish();
    let sf = LazySourceFile::new(&bytes).unwrap();
    assert_eq!(node_kind(&sf, 1), Kind::NodeList.as_u8());
    let nd = node_data(&sf, 1);
    assert_eq!(type_tag(nd), TypeTag::Children);
    assert_eq!(payload(nd), 7);
}

#[test]
fn dedup_intern_returns_same_index() {
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    let a = writer.intern_string("param");
    let b = writer.intern_string("param");
    assert_eq!(a, b);
    // Encoded buffer must contain only one offsets entry for "param".
    let bytes = writer.finish();
    let sf = LazySourceFile::new(&bytes).unwrap();
    let offsets_size = sf.string_data_offset - sf.string_offsets_offset;
    assert_eq!(offsets_size, 8, "exactly one (start,end) entry");
}
