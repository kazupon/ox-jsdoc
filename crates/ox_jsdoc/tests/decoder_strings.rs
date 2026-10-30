// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! The safe API of the lazy decoder never gives a `&str` that is not UTF-8,
//! whatever the offsets passed to it: an offset in the middle of a character
//! or past the String Data gives nothing.

use ox_jsdoc::decoder::nodes::type_node::LazyTypeNode;
use ox_jsdoc::decoder::source_file::LazySourceFile;
use ox_jsdoc::format::string_field::StringField;
use ox_jsdoc::parser::type_data::{ParseMode, TypeNodeData, TypeTemplateLiteral};
use ox_jsdoc::parser::type_emit::emit_type_node;
use ox_jsdoc::parser::{ParseOptions, parse};
use ox_jsdoc::writer::BinaryWriter;
use oxc_allocator::Allocator;
use oxc_span::Span;

/// `é` and `ü` are two bytes each, so some offsets cut a character in half.
const SOURCE: &str = "/** é and ü */";

#[track_caller]
fn assert_utf8(what: &str, s: &str) {
    assert!(
        std::str::from_utf8(s.as_bytes()).is_ok(),
        "{what} gave a str that is not UTF-8: {:?}",
        s.as_bytes()
    );
}

#[test]
fn slices_of_the_source_text_are_utf8() {
    let arena = Allocator::default();
    let result = parse(&arena, SOURCE, ParseOptions::default());
    let source_file = result.source_file;
    let len = SOURCE.len() as u32;
    for start in 0..=len + 2 {
        for end in start..=len + 2 {
            if let Some(s) = source_file.slice_source_text(0, start, end) {
                assert_utf8(&format!("slice_source_text(0, {start}, {end})"), s);
            }
        }
    }
    // On character boundaries, the text; in the middle of `é`, nothing.
    assert_eq!(source_file.slice_source_text(0, 4, 6), Some("é"));
    assert_eq!(source_file.slice_source_text(0, 4, 5), None);
    assert_eq!(source_file.slice_source_text(0, 5, 6), None);
}

#[test]
fn strings_at_any_offset_are_utf8() {
    let arena = Allocator::default();
    let result = parse(&arena, SOURCE, ParseOptions::default());
    let source_file = result.source_file;
    // The String Data section and a little past it (Extended Data and the
    // nodes follow it in the buffer).
    let span = source_file.extended_data_offset - source_file.string_data_offset + 16;
    for offset in 0..span {
        for length in 0..8u16 {
            let field = StringField { offset, length };
            if let Some(s) = source_file.get_string_by_field(field) {
                assert_utf8(&format!("get_string_by_field({offset}, {length})"), s);
            }
            assert_utf8(
                &format!("get_required_string_by_field({offset}, {length})"),
                source_file.get_required_string_by_field(field),
            );
            assert_utf8(
                &format!("get_inline_string({offset}, {length})"),
                source_file.get_inline_string(offset, length as u8),
            );
        }
    }
    // Indexes into the String Offsets table read any pair of offsets from
    // the bytes after it.
    let entries = (source_file.bytes().len() as u32 - source_file.string_offsets_offset) / 8;
    for index in 0..entries {
        if let Some(s) = source_file.get_string(index) {
            assert_utf8(&format!("get_string({index})"), s);
        }
    }
}

#[test]
fn strings_in_the_middle_of_a_character_are_empty() {
    let arena = Allocator::default();
    let result = parse(&arena, SOURCE, ParseOptions::default());
    let source_file = result.source_file;
    let source = source_file.get_root_source_offset_in_data(0);
    let whole = StringField {
        offset: source + 4,
        length: 2,
    };
    let half = StringField {
        offset: source + 4,
        length: 1,
    };
    assert_eq!(source_file.get_string_by_field(whole), Some("é"));
    assert_eq!(source_file.get_string_by_field(half), None);
    assert_eq!(source_file.get_required_string_by_field(half), "");
    assert_eq!(source_file.get_inline_string(source + 4, 2), "é");
    assert_eq!(source_file.get_inline_string(source + 4, 1), "");
}

#[test]
#[should_panic(expected = "template literal segment 2 out of range (count 2)")]
fn panics_for_a_template_literal_segment_past_the_count() {
    let arena = Allocator::default();
    let options = ParseOptions {
        parse_types: true,
        type_parse_mode: ParseMode::Typescript,
        ..ParseOptions::default()
    };
    let result = parse(&arena, "/** @type {`a${b}`} */", options);
    let tag = result
        .lazy_root
        .expect("parsed")
        .tags()
        .next()
        .expect("@type");
    let Some(LazyTypeNode::TemplateLiteral(literal)) = tag.parsed_type() else {
        panic!("not a template literal: {:?}", tag.parsed_type());
    };
    assert_eq!(literal.literal_count(), 2);
    let _ = literal.literal(2);
}

#[test]
fn interns_new_contents_of_the_same_buffer_as_another_string() {
    // The writer remembers the last string by its address and length; the
    // same `String` with other contents is another string.
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    let mut s = String::from("zq");
    let first = writer.intern_string(&s);
    s.clear();
    s.push_str("xw");
    let second = writer.intern_string(&s);
    s.clear();
    s.push_str("vu");
    let first_index = writer.intern_string_index(&s);
    s.clear();
    s.push_str("ts");
    let second_index = writer.intern_string_index(&s);
    let bytes = writer.finish();
    let source_file = LazySourceFile::new(&bytes).expect("header");
    assert_eq!(source_file.get_string_by_field(first), Some("zq"));
    assert_eq!(source_file.get_string_by_field(second), Some("xw"));
    assert_eq!(source_file.get_string(first_index.as_u32()), Some("vu"));
    assert_eq!(source_file.get_string(second_index.as_u32()), Some("ts"));
}

/// A template literal type with one literal.
fn template_literal(text: &str) -> TypeNodeData<'_> {
    TypeNodeData::TemplateLiteral(TypeTemplateLiteral {
        span: Span::new(0, 0),
        literals: vec![text],
        interpolations: Vec::new(),
    })
}

/// The literal of the template literal type that `emit_type_node` wrote at
/// `index` in `bytes`.
fn literal_at(bytes: &[u8], index: u32) -> String {
    let source_file = LazySourceFile::new(bytes).expect("header");
    match LazyTypeNode::from_index(&source_file, index, 0) {
        Some(LazyTypeNode::TemplateLiteral(literal)) => literal.literal(0).to_owned(),
        other => panic!("not a template literal: {other:?}"),
    }
}

#[test]
fn emits_new_contents_of_a_string_that_the_public_interner_saw() {
    // A public intern, then the same `String` with other contents to an
    // emitter, which takes strings from the parser and from any caller.
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    let mut s = String::from("foo");
    let _ = writer.intern_string(&s);
    s.make_ascii_uppercase();
    let index = emit_type_node(&mut writer, &template_literal(&s), 0);
    assert_eq!(literal_at(&writer.finish(), index), "FOO");
}

#[test]
fn emits_new_contents_of_a_source_text_registered_again() {
    // The same memory registered as the source text twice, with other
    // contents the second time.
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    let mut s = String::from("foo");
    writer.append_source_text(&s);
    let first = emit_type_node(&mut writer, &template_literal(&s), 0);
    s.make_ascii_uppercase();
    writer.append_source_text(&s);
    let second = emit_type_node(&mut writer, &template_literal(&s), 0);
    let bytes = writer.finish();
    assert_eq!(literal_at(&bytes, first), "foo");
    assert_eq!(literal_at(&bytes, second), "FOO");
}
