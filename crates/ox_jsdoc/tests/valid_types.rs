// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Types that jsdoc-type-pratt-parser 9.4.0 accepts, and the strings it gives
//! them: variadic types in square brackets in parameter lists (jsdoc), the
//! values of special name paths and `event:` on the right of a name path, a
//! prefix `=` (typescript), generic arrow functions, `readonly?` keys, unions
//! in the return types of signatures, and `?` / `=` around computed keys. The
//! neighbours that jsdoc-type-pratt-parser rejects stay rejected.

use ox_jsdoc::decoder::nodes::type_node::LazyTypeNode;
use ox_jsdoc::parser::type_data::ParseMode;
use ox_jsdoc::parser::{ParseOptions, parse, parse_type_expression};
use oxc_allocator::Allocator;

const JSDOC: ParseMode = ParseMode::Jsdoc;
const CLOSURE: ParseMode = ParseMode::Closure;
const TYPESCRIPT: ParseMode = ParseMode::Typescript;
const ALL: [ParseMode; 3] = [JSDOC, CLOSURE, TYPESCRIPT];

/// Each `(input, expected string)` parses in each mode and gives the string
/// that jsdoc-type-pratt-parser's `stringify` gives.
fn assert_strings(modes: &[ParseMode], cases: &[(&str, &str)]) {
    for &mode in modes {
        for &(input, expected) in cases {
            assert_eq!(
                parse_type_expression(input, mode).as_deref(),
                Some(expected),
                "{input:?} ({mode:?})"
            );
        }
    }
}

/// Each input fails to parse in each mode, as in jsdoc-type-pratt-parser.
fn assert_rejected(modes: &[ParseMode], inputs: &[&str]) {
    for &mode in modes {
        for &input in inputs {
            assert_eq!(
                parse_type_expression(input, mode),
                None,
                "{input:?} ({mode:?})"
            );
        }
    }
}

/// Parse `/** @type {ty} */` with `parseTypes` and call `check` with the
/// parsed type of the Binary AST.
fn with_parsed_type(ty: &str, mode: ParseMode, check: impl FnOnce(LazyTypeNode<'_>)) {
    let source = format!("/** @type {{{ty}}} */");
    let arena = Allocator::default();
    let options = ParseOptions {
        parse_types: true,
        type_parse_mode: mode,
        ..ParseOptions::default()
    };
    let result = parse(&arena, &source, options);
    let root = result.lazy_root.expect("parsed");
    let tag = root.tags().next().expect("@type");
    check(tag.parsed_type().expect("parsed type"));
}

#[test]
fn reads_variadic_types_in_square_brackets_as_parameters() {
    assert_strings(
        &[JSDOC],
        &[
            (
                "function(string, ...[number]): number",
                "function(string, ...number): number",
            ),
            ("function(...[null])", "function(...null)"),
            ("function(...[*])", "function(...*)"),
            (
                "function(new:Array, ...[*]): Array",
                "function(new: Array, ...*): Array",
            ),
            (
                "function(...[number]): boolean",
                "function(...number): boolean",
            ),
            ("function(...[number]=)", "function(...number=)"),
            (
                "function(...[number]|string)",
                "function(...number | string)",
            ),
            ("function(...[number[]])", "function(...number[])"),
        ],
    );
}

#[test]
fn prints_variadic_types_without_their_square_brackets() {
    assert_strings(
        &[JSDOC],
        &[
            ("...[number]", "...number"),
            ("Array<...[number]>", "Array<...number>"),
            ("(...[number])", "(...number)"),
            ("{a: ...[number]}", "{a: ...number}"),
        ],
    );
    // In typescript, the brackets are a tuple.
    assert_strings(&[TYPESCRIPT], &[("...[number]", "...[number]")]);
}

#[test]
fn rejects_a_union_in_the_square_brackets_of_a_variadic_type() {
    assert_rejected(
        &[JSDOC],
        &["...[number|string]", "function(...[number|string])"],
    );
}

#[test]
fn reads_event_name_paths_on_the_right_of_a_name_path() {
    assert_strings(
        &ALL,
        &[
            ("module:foo.event:bar", "module:foo.event:bar"),
            ("Foo.event:bar", "Foo.event:bar"),
            ("module:a#event:b", "module:a#event:b"),
            ("module:a~event:b", "module:a~event:b"),
        ],
    );
    assert_strings(
        &[JSDOC, CLOSURE],
        &[
            // A `-` in a name is for the loose names of jsdoc and closure.
            (
                "module:some-module.event:some-event",
                "module:some-module.event:some-event",
            ),
            (
                "module:some-module#some-where.event:some-event",
                "module:some-module#some-where.event:some-event",
            ),
            ("Foo#event:bar", "Foo#event:bar"),
        ],
    );
    assert_strings(&[JSDOC], &[("event:foo.event:bar", "event:foo.event:bar")]);
}

#[test]
fn reads_the_rest_of_a_special_name_path_as_a_name_path() {
    assert_strings(
        &ALL,
        &[
            ("module:foo.1", "module:foo.1"),
            ("module:foo.\"bar\"", "module:foo.\"bar\""),
            ("module:\"foo\".bar", "module:\"foo\".bar"),
            ("module:#foo", "module:#foo"),
            ("module:@a/b.c~d", "module:@a/b.c~d"),
        ],
    );
}

#[test]
fn rejects_name_paths_that_jsdoc_type_pratt_parser_rejects() {
    assert_rejected(
        &ALL,
        &[
            // A name path after an event name path on the right.
            "module:a.event:b.c",
            "module:a.event:b#c",
            "module:a.module:b",
            // A keyword in the value of a special name path.
            "module:foo/function",
        ],
    );
}

#[test]
fn keeps_a_special_name_path_as_the_left_of_a_name_path() {
    with_parsed_type("module:foo.bar", JSDOC, |node| {
        let LazyTypeNode::NamePath(path) = node else {
            panic!("not a name path: {node:?}");
        };
        let Some(LazyTypeNode::SpecialNamePath(left)) = path.left() else {
            panic!("not a special name path: {:?}", path.left());
        };
        assert_eq!(left.value(), "foo");
        let Some(LazyTypeNode::Property(right)) = path.right() else {
            panic!("not a property: {:?}", path.right());
        };
        assert_eq!(right.value(), "bar");
    });
    with_parsed_type("Foo.event:bar", TYPESCRIPT, |node| {
        let LazyTypeNode::NamePath(path) = node else {
            panic!("not a name path: {node:?}");
        };
        let Some(LazyTypeNode::SpecialNamePath(right)) = path.right() else {
            panic!("not a special name path: {:?}", path.right());
        };
        // 1 = event
        assert_eq!(right.special_type(), 1);
        assert_eq!(right.value(), "bar");
    });
}

#[test]
fn reads_a_prefix_equals_sign_in_every_mode() {
    assert_strings(&ALL, &[("=abc", "=abc"), ("{a: =b}", "{a: =b}")]);
}

#[test]
fn reads_generic_arrow_functions() {
    assert_strings(
        &[TYPESCRIPT],
        &[
            ("<T>(x: T) => T", "<T>(x: T) => T"),
            (
                "<T, U extends V = string, W = string>(x: T) => U",
                "<T, U extends V = string, W = string>(x: T) => U",
            ),
            ("<T>(x: T) => T | U", "<T>(x: T) => T | U"),
            ("<T>new (x: T) => T", "new <T>(x: T) => T"),
            ("<T>() => void", "<T>() => void"),
        ],
    );
    with_parsed_type("<T>(x: T) => T", TYPESCRIPT, |node| {
        let LazyTypeNode::Function(function) = node else {
            panic!("not a function: {node:?}");
        };
        assert!(function.arrow());
        let type_parameters = function.type_parameters().expect("type parameters");
        assert_eq!(type_parameters.elements().count(), 1);
    });
}

#[test]
fn rejects_type_parameters_without_an_arrow_function() {
    assert_rejected(&[TYPESCRIPT], &["<T>(x)", "<T>string", "<T,>(x: T) => T"]);
    assert_rejected(&[JSDOC, CLOSURE], &["<T>(x: T) => T"]);
}

#[test]
fn reads_readonly_as_an_optional_key_in_typescript() {
    assert_strings(
        &[TYPESCRIPT],
        &[("{readonly?: boolean}", "{readonly?: boolean}")],
    );
    // jsdoc-type-pratt-parser does not read `readonly` as a key in these.
    assert_rejected(&[JSDOC, CLOSURE], &["{readonly?: boolean}"]);
}

#[test]
fn reads_unions_and_intersections_in_the_return_types_of_signatures() {
    assert_strings(
        &[TYPESCRIPT],
        &[
            ("{a(): U | V}", "{a(): U | V}"),
            ("{a(): U | V, b: W}", "{a(): U | V, b: W}"),
            ("{(): U & V}", "{(): U & V}"),
            ("{<T>(): U | V}", "{<T>(): U | V}"),
            ("{new (): U | V}", "{new (): U | V}"),
            ("{[a](): U | V}", "{[a](): U | V}"),
        ],
    );
    // jsdoc-type-pratt-parser has no call signatures in these.
    assert_rejected(&[JSDOC, CLOSURE], &["{(): U | V}"]);
}

#[test]
fn reads_question_marks_and_equals_signs_around_computed_keys() {
    assert_strings(
        &[TYPESCRIPT],
        &[
            ("{[a?]: T}", "{[a?]: T}"),
            ("{[a=]: T}", "{[a=]: T}"),
            ("{[?a]: T}", "{[?a]: T}"),
            ("{[=a]: T}", "{[=a]: T}"),
            ("{[a?=]: T}", "{[a?=]: T}"),
            ("{[=a?]: T}", "{[=a?]: T}"),
            ("{[a?](): T}", "{[a?](): T}"),
            ("{[\"a\"?]: T}", "{[\"a\"?]: T}"),
            ("{[1?]: T}", "{[1?]: T}"),
        ],
    );
    assert_rejected(&[TYPESCRIPT], &["{[?]: T}", "{[a?: string]: T}"]);
    with_parsed_type("{[=a?]: T}", TYPESCRIPT, |node| {
        let LazyTypeNode::Object(object) = node else {
            panic!("not an object: {node:?}");
        };
        let Some(LazyTypeNode::ObjectField(field)) = object.elements().next() else {
            panic!("no object field");
        };
        let Some(LazyTypeNode::ComputedProperty(key)) = field.key() else {
            panic!("not a computed property: {:?}", field.key());
        };
        // `=` (prefix) around `a?` (suffix), as in jsdoc-type-pratt-parser.
        let Some(LazyTypeNode::Optional(optional)) = key.value() else {
            panic!("not an optional type: {:?}", key.value());
        };
        assert_eq!(optional.position(), 0);
        let Some(LazyTypeNode::Nullable(nullable)) = optional.element() else {
            panic!("not a nullable type: {:?}", optional.element());
        };
        assert_eq!(nullable.position(), 1);
    });
}
