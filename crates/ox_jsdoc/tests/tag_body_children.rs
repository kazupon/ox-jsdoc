// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! The children of a tag body: the type source and the value of a
//! `JsdocGenericTagBody` (bit0 and bit1 of its Children bitmask), and the
//! source and the target of the reserved `JsdocBorrowsTagBody`, through
//! their accessors and the default walk of the visitor.

use std::collections::HashMap;

use ox_jsdoc::decoder::nodes::LazyNode;
use ox_jsdoc::decoder::nodes::comment_ast::{
    LazyJsdocBorrowsTagBody, LazyJsdocGenericTagBody, LazyJsdocIdentifier, LazyJsdocNamepathSource,
    LazyJsdocParameterName, LazyJsdocTagBody, LazyJsdocTagValue, LazyJsdocText,
    LazyJsdocTypeSource,
};
use ox_jsdoc::decoder::source_file::LazySourceFile;
use ox_jsdoc::decoder::visitor::LazyJsdocVisitor;
use ox_jsdoc::format::kind::Kind;
use ox_jsdoc::format::node_record::NODE_RECORD_SIZE;
use ox_jsdoc::parser::{ParseOptions, parse_to_bytes};
use ox_jsdoc::writer::BinaryWriter;
use ox_jsdoc::writer::nodes::comment_ast::{
    write_jsdoc_borrows_tag_body, write_jsdoc_identifier, write_jsdoc_namepath_source,
};
use oxc_allocator::Allocator;
use oxc_span::Span;

/// The body of the first tag of the first root.
fn generic_body<'a>(file: &'a LazySourceFile<'a>) -> LazyJsdocGenericTagBody<'a> {
    let block = file.asts().next().flatten().expect("a block");
    let tag = block.tags().next().expect("a tag");
    match tag.body() {
        Some(LazyJsdocTagBody::Generic(body)) => body,
        other => panic!("expected a generic tag body, got {other:?}"),
    }
}

#[test]
fn reads_the_type_source_and_a_parameter_name() {
    let bytes = parse_to_bytes(
        "/** @param {number} [count=1] How many. */",
        ParseOptions::default(),
    )
    .binary_bytes;
    let file = LazySourceFile::new(&bytes).unwrap();
    let body = generic_body(&file);
    assert_eq!(
        body.type_source().map(|source| source.raw()),
        Some("number")
    );
    let Some(LazyJsdocTagValue::Parameter(name)) = body.value() else {
        panic!("expected a parameter name, got {:?}", body.value());
    };
    assert_eq!(
        (name.path(), name.optional(), name.default_value()),
        ("count", true, Some("1"))
    );
}

#[test]
fn reads_a_namepath_and_a_text_value() {
    let bytes = parse_to_bytes("/** @see module:foo/bar */", ParseOptions::default()).binary_bytes;
    let file = LazySourceFile::new(&bytes).unwrap();
    let body = generic_body(&file);
    assert!(body.type_source().is_none());
    let Some(LazyJsdocTagValue::Namepath(namepath)) = body.value() else {
        panic!("expected a namepath, got {:?}", body.value());
    };
    assert_eq!(namepath.raw(), "module:foo/bar");

    let bytes = parse_to_bytes("/** @param 名前 説明 */", ParseOptions::default()).binary_bytes;
    let file = LazySourceFile::new(&bytes).unwrap();
    let Some(LazyJsdocTagValue::Raw(text)) = generic_body(&file).value() else {
        panic!("expected a text value");
    };
    assert_eq!(text.value(), "名前");
}

#[test]
fn reads_the_source_and_the_target_of_a_borrows_tag_body() {
    // `@borrows a as b`, as the writer would write it: a Children-type body
    // whose children are a namepath (bit0) and an identifier (bit1).
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    let _ = writer.append_source_text("a as b");
    let body = write_jsdoc_borrows_tag_body(&mut writer, Span::new(0, 6), 0, 0b11).as_u32();
    let source = writer.intern_string_payload("a");
    let _ = write_jsdoc_namepath_source(&mut writer, Span::new(0, 1), body, source);
    let target = writer.intern_string_payload("b");
    let _ = write_jsdoc_identifier(&mut writer, Span::new(5, 6), body, target);
    writer.push_root(body, 0, 0);
    let bytes = writer.finish();

    let file = LazySourceFile::new(&bytes).unwrap();
    let borrows = LazyJsdocBorrowsTagBody::from_index(&file, body, 0);
    let Some(LazyJsdocTagValue::Namepath(source)) = borrows.source() else {
        panic!("expected a namepath, got {:?}", borrows.source());
    };
    assert_eq!(source.raw(), "a");
    let Some(LazyJsdocTagValue::Identifier(target)) = borrows.target() else {
        panic!("expected an identifier, got {:?}", borrows.target());
    };
    assert_eq!(target.name(), "b");
}

/// Counts the leaves that sit under tag bodies (and the tags' own type
/// sources), with the default walk of every other node.
#[derive(Default)]
struct Leaves {
    counts: HashMap<Kind, usize>,
}

impl Leaves {
    fn bump(&mut self, kind: Kind) {
        *self.counts.entry(kind).or_insert(0) += 1;
    }
}

impl<'a> LazyJsdocVisitor<'a> for Leaves {
    fn visit_type_source(&mut self, _node: LazyJsdocTypeSource<'a>) {
        self.bump(Kind::JsdocTypeSource);
    }
    fn visit_parameter_name(&mut self, _node: LazyJsdocParameterName<'a>) {
        self.bump(Kind::JsdocParameterName);
    }
    fn visit_namepath_source(&mut self, _node: LazyJsdocNamepathSource<'a>) {
        self.bump(Kind::JsdocNamepathSource);
    }
    fn visit_identifier(&mut self, _node: LazyJsdocIdentifier<'a>) {
        self.bump(Kind::JsdocIdentifier);
    }
    fn visit_text(&mut self, _node: LazyJsdocText<'a>) {
        self.bump(Kind::JsdocText);
    }
}

#[test]
fn walks_the_children_of_tag_bodies() {
    let source = "/**\n * @param {string} id - The id.\n * @param {number} [count=1] How many.\n * @see module:foo/bar\n * @param 名前 説明\n */";
    let bytes = parse_to_bytes(source, ParseOptions::default()).binary_bytes;
    let file = LazySourceFile::new(&bytes).unwrap();
    let mut leaves = Leaves::default();
    for block in file.asts().flatten() {
        leaves.visit_block(block);
    }

    // Every such leaf of the buffer, from its node record.
    let mut expected: HashMap<Kind, usize> = HashMap::new();
    for index in 1..file.node_count {
        let record = file.nodes_offset as usize + index as usize * NODE_RECORD_SIZE;
        let kind = Kind::from_u8(file.bytes()[record]).unwrap();
        if matches!(
            kind,
            Kind::JsdocTypeSource
                | Kind::JsdocParameterName
                | Kind::JsdocNamepathSource
                | Kind::JsdocIdentifier
                | Kind::JsdocText
        ) {
            *expected.entry(kind).or_insert(0) += 1;
        }
    }
    // The tags' and the bodies' type sources, and a value of each kind.
    assert_eq!(expected.get(&Kind::JsdocTypeSource), Some(&4));
    assert_eq!(expected.get(&Kind::JsdocParameterName), Some(&2));
    assert_eq!(expected.get(&Kind::JsdocNamepathSource), Some(&1));
    assert_eq!(expected.get(&Kind::JsdocText), Some(&1));
    assert_eq!(leaves.counts, expected);
}
