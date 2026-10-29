// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Tags whose body spans several lines.
//!
//! The parser joins the lines of a tag body without their margins (` * `)
//! before it reads the type, the name and the description, so the strings
//! of such a tag are not slices of the source text.

use ox_jsdoc::decoder::nodes::comment_ast::LazyJsdocTag;
use ox_jsdoc::decoder::nodes::type_node::LazyTypeName;
use ox_jsdoc::decoder::visitor::LazyJsdocVisitor;
use ox_jsdoc::parser::{ParseOptions, parse};
use oxc_allocator::Allocator;

fn tags<'a>(arena: &'a Allocator, source: &'a str, options: ParseOptions) -> Vec<LazyJsdocTag<'a>> {
    let result = parse(arena, source, options);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.lazy_root.expect("root").tags().collect()
}

fn description_lines<'a>(tag: LazyJsdocTag<'a>) -> Vec<&'a str> {
    tag.description_lines()
        .map(|line| line.description())
        .collect()
}

fn compat() -> ParseOptions {
    ParseOptions {
        compat_mode: true,
        ..ParseOptions::default()
    }
}

/// Values of the `TypeName` nodes of a parsed type, in visiting order.
#[derive(Default)]
struct TypeNames<'a>(Vec<&'a str>);

impl<'a> LazyJsdocVisitor<'a> for TypeNames<'a> {
    fn visit_type_name(&mut self, node: LazyTypeName<'a>) {
        self.0.push(node.value());
    }
}

fn type_names<'a>(tag: LazyJsdocTag<'a>) -> Vec<&'a str> {
    let mut names = TypeNames::default();
    names.visit_type_node(tag.parsed_type().expect("parsed type"));
    names.0
}

#[test]
fn name_after_a_multiline_type() {
    let arena = Allocator::default();
    for source in [
        "/**\n * @param {{\n *   a: string\n * }} options desc\n */",
        "/**\r\n * @param {{\r\n *   a: string\r\n * }} options desc\r\n */",
    ] {
        let tag = tags(&arena, source, ParseOptions::default())[0];
        assert_eq!(
            tag.name().map(|name| name.raw()),
            Some("options"),
            "{source:?}"
        );
        assert_eq!(tag.raw_type().map(|t| t.raw()), Some("{\n  a: string\n}"));
        assert_eq!(description_lines(tag), ["desc"], "{source:?}");
    }
}

#[test]
fn name_on_the_second_line() {
    let arena = Allocator::default();
    let tag = tags(
        &arena,
        "/**\n * @param {string}\n *   foo the foo\n */",
        ParseOptions::default(),
    )[0];
    assert_eq!(tag.name().map(|name| name.raw()), Some("foo"));
    assert_eq!(description_lines(tag), ["the foo"]);
}

#[test]
fn description_line_of_a_multiline_description() {
    let arena = Allocator::default();
    for options in [ParseOptions::default(), compat()] {
        let tag = tags(
            &arena,
            "/**\n * @param foo the foo\n * second line\n */",
            options,
        )[0];
        assert_eq!(tag.description(), Some("the foo\nsecond line"));
        // The one description line of a tag holds the whole description.
        assert_eq!(description_lines(tag), ["the foo\nsecond line"]);
    }
}

#[test]
fn description_line_of_a_multiline_example() {
    let arena = Allocator::default();
    // `@example` has no name in compat mode, so the code is the description.
    let tag = tags(
        &arena,
        "/** @example\n * console.log(\"foo\");\n * console.log(\"bar\");\n */",
        compat(),
    )[0];
    assert_eq!(
        description_lines(tag),
        ["console.log(\"foo\");\nconsole.log(\"bar\");"]
    );
}

#[test]
fn type_names_in_a_multiline_type() {
    let arena = Allocator::default();
    let options = ParseOptions {
        parse_types: true,
        ..ParseOptions::default()
    };
    let multiline = tags(
        &arena,
        "/**\n * @param {{\n *   a: Array<string>,\n *   b: number\n * }} x\n */",
        options,
    )[0];
    let one_line = tags(
        &arena,
        "/** @param {{ a: Array<string>, b: number }} x */",
        options,
    )[0];
    assert_eq!(
        type_names(one_line),
        ["a", "Array", "string", "b", "number"]
    );
    assert_eq!(type_names(multiline), type_names(one_line));
}
