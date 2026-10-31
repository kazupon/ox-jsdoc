// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Line breaks between the fields of an object type, as jsdoc-type-pratt-parser
//! 9.4.0 reads and prints them: a line break separates fields as `,` and `;`
//! do, the separator of the object tells which (`Linebreak`,
//! `CommaAndLinebreak`, `SemicolonAndLinebreak`), and the string puts each
//! field on its own line.

use ox_jsdoc::decoder::nodes::type_node::LazyTypeNode;
use ox_jsdoc::parser::type_data::ParseMode;
use ox_jsdoc::parser::{ParseOptions, parse, parse_type_expression};
use oxc_allocator::Allocator;

const ALL: [ParseMode; 3] = [ParseMode::Jsdoc, ParseMode::Closure, ParseMode::Typescript];

fn assert_strings(cases: &[(&str, &str)]) {
    for mode in ALL {
        for &(input, expected) in cases {
            assert_eq!(
                parse_type_expression(input, mode).as_deref(),
                Some(expected),
                "{input:?} ({mode:?})"
            );
        }
    }
}

/// The separator of the object type of the first tag of `source`, as the
/// Binary AST keeps it (Common Data of `TypeObject`).
fn separator_in_comment(source: &str) -> u8 {
    let arena = Allocator::default();
    let options = ParseOptions {
        parse_types: true,
        type_parse_mode: ParseMode::Typescript,
        ..ParseOptions::default()
    };
    let result = parse(&arena, source, options);
    let root = result.lazy_root.expect("parsed");
    let tag = root.tags().next().expect("a tag");
    match tag.parsed_type() {
        Some(LazyTypeNode::Object(object)) => object.separator(),
        other => panic!("not an object type: {other:?}"),
    }
}

/// The separator of `ty` as the type of `@type`.
fn separator(ty: &str) -> u8 {
    separator_in_comment(&format!("/** @type {{{ty}}} */"))
}

#[test]
fn reads_fields_separated_by_line_breaks() {
    assert_strings(&[
        (
            "{\n  range: boolean\n  loc: boolean\n}",
            "{\n  range: boolean\n  loc: boolean\n}",
        ),
        // `\r\n` is a line break; the string has `\n`.
        ("{a: b\r\n c: d}", "{\n  a: b\n  c: d\n}"),
        // A stray separator at the start of a line goes with the line break.
        ("{a: b\n, c: d}", "{\n  a: b\n  c: d\n}"),
    ]);
}

#[test]
fn prints_each_field_on_its_own_line() {
    assert_strings(&[
        (
            "{\n  range: boolean,\n  loc: boolean\n}",
            "{\n  range: boolean,\n  loc: boolean\n}",
        ),
        (
            "{\n  range: boolean;\n  loc: boolean\n}",
            "{\n  range: boolean;\n  loc: boolean\n}",
        ),
        // The last separator wins, and the indent is two spaces.
        ("{a: b,\n c: d;\n e: f}", "{\n  a: b;\n  c: d;\n  e: f\n}"),
        // Not deeper in a nested object, as there.
        (
            "{\n  a: {\n    b: c,\n    d: e\n  },\n  f: g\n}",
            "{\n  a: {\n  b: c,\n  d: e\n},\n  f: g\n}",
        ),
    ]);
}

#[test]
fn keeps_one_field_or_a_line_break_after_the_last_one_on_one_line() {
    assert_strings(&[
        ("{a: b\n}", "{a: b}"),
        ("{\n  a: b\n}", "{a: b}"),
        ("{a: b, c: d\n}", "{a: b, c: d}"),
        ("{a: b,\n}", "{a: b}"),
    ]);
}

#[test]
fn rejects_fields_separated_by_a_carriage_return() {
    // A lone `\r` is no line break for jsdoc-type-pratt-parser.
    for mode in ALL {
        assert_eq!(
            parse_type_expression("{a: b\r c: d}", mode),
            None,
            "{mode:?}"
        );
    }
}

#[test]
fn keeps_the_kind_of_separator() {
    // 1 = Comma, 2 = Semicolon, 3 = Linebreak, 4 = CommaAndLinebreak,
    // 5 = SemicolonAndLinebreak
    assert_eq!(separator("{a: b, c: d}"), 1);
    assert_eq!(separator("{a: b; c: d}"), 2);
    assert_eq!(separator("{\n a: b\n c: d\n}"), 3);
    assert_eq!(separator("{\n a: b,\n c: d\n}"), 4);
    assert_eq!(separator("{\n a: b;\n c: d\n}"), 5);
    assert_eq!(separator("{\n a: b\n}"), 3);
}

#[test]
fn reads_the_line_breaks_of_a_type_over_several_lines_of_a_comment() {
    let source = "/**\n * @param {{\n *   a: string,\n *   b: number\n * }} x\n */";
    assert_eq!(separator_in_comment(source), 4);
    let source = "/**\n * @param {{\n *   a: string\n *   b: number\n * }} x\n */";
    assert_eq!(separator_in_comment(source), 3);
}
