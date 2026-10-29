// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Tags whose body spans several lines.
//!
//! The parser joins the lines of a tag body without their margins (` * `)
//! before it reads the type, the name and the description, so the strings
//! of such a tag are not slices of the source text, and their positions in
//! the joined text are not source positions.

use ox_jsdoc::decoder::nodes::LazyNode;
use ox_jsdoc::decoder::nodes::comment_ast::LazyJsdocTag;
use ox_jsdoc::decoder::nodes::type_node::{LazyTypeName, LazyTypeObject};
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

/// The range of the first `needle` in `source`.
fn at(source: &str, needle: &str) -> [u32; 2] {
    let start = source.find(needle).expect("needle");
    [start as u32, (start + needle.len()) as u32]
}

/// The range from the first `from` to the end of the first `to` after it.
fn from_to(source: &str, from: &str, to: &str) -> [u32; 2] {
    let start = source.find(from).expect("from");
    let end = start + source[start..].find(to).expect("to") + to.len();
    [start as u32, end as u32]
}

#[test]
fn ranges_after_a_multiline_type() {
    let arena = Allocator::default();
    for source in [
        "/**\n * @param {{\n *   a: string\n * }} options desc\n */",
        "/**\r\n * @param {{\r\n *   a: string\r\n * }} options desc\r\n */",
    ] {
        let tag = tags(&arena, source, ParseOptions::default())[0];
        let raw_type = from_to(source, "{{", "}}");
        assert_eq!(
            tag.raw_type().map(|t| t.range()),
            Some(raw_type),
            "{source:?}"
        );
        let type_lines: Vec<_> = tag.type_lines().map(|line| line.range()).collect();
        assert_eq!(type_lines, [raw_type], "{source:?}");
        let name = tag.name().map(|name| name.range());
        assert_eq!(name, Some(at(source, "options")), "{source:?}");
        let lines: Vec<_> = tag.description_lines().map(|line| line.range()).collect();
        assert_eq!(lines, [at(source, "desc")], "{source:?}");
    }
}

#[test]
fn ranges_of_a_name_on_the_second_line() {
    let arena = Allocator::default();
    let source = "/**\n * @param {string}\n *   foo the foo\n */";
    let tag = tags(&arena, source, ParseOptions::default())[0];
    assert_eq!(tag.name().map(|name| name.range()), Some(at(source, "foo")));
    let lines: Vec<_> = tag.description_lines().map(|line| line.range()).collect();
    assert_eq!(lines, [at(source, "the foo")]);
}

#[test]
fn range_of_a_multiline_description() {
    let arena = Allocator::default();
    let source = "/**\n * @param foo the foo\n *\n * second line\n */";
    for options in [ParseOptions::default(), compat()] {
        let tag = tags(&arena, source, options)[0];
        assert_eq!(tag.description(), Some("the foo\n\nsecond line"));
        let lines: Vec<_> = tag.description_lines().map(|line| line.range()).collect();
        assert_eq!(lines, [from_to(source, "the foo", "second line")]);
    }
}

#[test]
fn ranges_of_inline_tags_after_a_line_break() {
    let arena = Allocator::default();
    let source = "/**\n * Summary {@link Foo}\n * more {@link Bar} text\n * @param foo the foo\n *   see {@link Baz} here\n */";
    let result = parse(&arena, source, ParseOptions::default());
    let block = result.lazy_root.expect("root");
    let ranges: Vec<_> = block.inline_tags().map(|tag| tag.range()).collect();
    assert_eq!(
        ranges,
        [at(source, "{@link Foo}"), at(source, "{@link Bar}")]
    );
    let tag = block.tags().next().expect("tag");
    let ranges: Vec<_> = tag.inline_tags().map(|tag| tag.range()).collect();
    assert_eq!(ranges, [at(source, "{@link Baz}")]);
}

/// Values and ranges of the `TypeName` nodes and ranges of the
/// `TypeObject` nodes of a parsed type.
#[derive(Default)]
struct TypeRanges<'a> {
    names: Vec<(&'a str, [u32; 2])>,
    objects: Vec<[u32; 2]>,
}

impl<'a> LazyJsdocVisitor<'a> for TypeRanges<'a> {
    fn visit_type_name(&mut self, node: LazyTypeName<'a>) {
        self.names.push((node.value(), node.range()));
    }
    fn visit_type_object(&mut self, node: LazyTypeObject<'a>) {
        self.objects.push(node.range());
        self.visit_type_object_default(node);
    }
}

#[test]
fn ranges_of_type_nodes_in_a_multiline_type() {
    let arena = Allocator::default();
    let options = ParseOptions {
        parse_types: true,
        ..ParseOptions::default()
    };
    for source in [
        "/**\n * @param {{\n *   a: Array<string>,\n *   b: number\n * }} x\n */",
        "/**\r\n * @param {{\r\n *\ta: Array<string>,\r\n *\tb: number\r\n * }} x\r\n */",
    ] {
        let tag = tags(&arena, source, options)[0];
        let mut ranges = TypeRanges::default();
        ranges.visit_type_node(tag.parsed_type().expect("parsed type"));
        let names: Vec<_> = ranges.names.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["a", "Array", "string", "b", "number"], "{source:?}");
        for (name, [start, end]) in ranges.names {
            assert_eq!(&source[start as usize..end as usize], name, "{source:?}");
        }
        // The object type is the inner `{ … }` of `{{ … }}`.
        let [start, end] = from_to(source, "{{", "}}");
        assert_eq!(ranges.objects, [[start + 1, end - 1]], "{source:?}");
    }
}

#[test]
fn description_raw_of_a_description_after_a_line_break() {
    let arena = Allocator::default();
    let options = ParseOptions {
        compat_mode: true,
        preserve_whitespace: true,
        ..ParseOptions::default()
    };
    for (source, raw) in [
        (
            "/**\n * @param {string} foo\n *   The description.\n */",
            "The description.",
        ),
        (
            "/**\n * @param {{\n *   a: string\n * }} options desc\n * more\n */",
            "desc\n * more",
        ),
    ] {
        let tag = tags(&arena, source, options)[0];
        assert_eq!(tag.description_raw(), Some(raw), "{source:?}");
    }
}
