// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! The rules of jsdoc-type-pratt-parser 9.4.0 for jsdoc and closure: their
//! object types have no `[…]` keys, `readonly` or signatures, some keywords
//! are no keys, and `["…"]` follows only a name. A jsdoc field without a `:`
//! is a key. closure also has no arrays, no string types, no `?` before the
//! `:` of a field, and no `function` without parentheses. The neighbours
//! that it accepts stay accepted.

use ox_jsdoc::parser::parse_type_expression;
use ox_jsdoc::parser::type_data::ParseMode;

const JSDOC: ParseMode = ParseMode::Jsdoc;
const CLOSURE: ParseMode = ParseMode::Closure;
const TYPESCRIPT: ParseMode = ParseMode::Typescript;
const ALL: [ParseMode; 3] = [JSDOC, CLOSURE, TYPESCRIPT];

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

fn assert_accepted(modes: &[ParseMode], inputs: &[&str]) {
    for &mode in modes {
        for &input in inputs {
            assert!(
                parse_type_expression(input, mode).is_some(),
                "{input:?} ({mode:?})"
            );
        }
    }
}

#[test]
fn has_no_typescript_object_syntax_in_jsdoc_and_closure() {
    let inputs = [
        "{[someType]: string;}",
        "{[someType](): AnotherType;}",
        "{[someType]?<T>(a: T, b: number[]): AnotherType;}",
        "{[key: string]: number}",
        "{[key in Type]: number}",
        "{readonly [key: string]: number}",
        "{readonly [key in Type]?: number}",
        "{ readonly x: number }",
        "{(a: string, b: number): SomeType}",
        "{<T>(a: T, b: number): SomeType}",
    ];
    assert_rejected(&[JSDOC, CLOSURE], &inputs);
    assert_accepted(&[TYPESCRIPT], &inputs);
    // A parenthesized type as a key is jsdoc's, not closure's.
    assert_accepted(&[JSDOC], &["{(string): number}"]);
    assert_rejected(
        &[CLOSURE],
        &["{(string): number}", "{(number|boolean|string): number}"],
    );
}

#[test]
fn takes_the_keywords_of_each_mode_as_keys() {
    let jsdoc_only = ["{null: T}", "{undefined: T}", "{function: T}"];
    let closure_only = ["{event: T}", "{external: T}"];
    let neither = [
        "{extends: T}",
        "{readonly: T}",
        "{import: T}",
        "{infer: T}",
        "{is: T}",
        "{asserts: T}",
    ];
    let both = [
        "{this: T}",
        "{new: T}",
        "{module: T}",
        "{typeof: T}",
        "{keyof: T}",
        "{in: T}",
        "{unique: T}",
        "{symbol: T}",
        "{\"a\": T}",
        "{1: T}",
    ];
    assert_accepted(&[JSDOC], &jsdoc_only);
    assert_rejected(&[CLOSURE], &jsdoc_only);
    assert_accepted(&[CLOSURE], &closure_only);
    assert_rejected(&[JSDOC], &closure_only);
    assert_rejected(&[JSDOC, CLOSURE], &neither);
    assert_accepted(&[JSDOC, CLOSURE], &both);
    assert_accepted(&[TYPESCRIPT], &jsdoc_only);
    assert_accepted(&[TYPESCRIPT], &closure_only);
    assert_accepted(&[TYPESCRIPT], &neither);
    // In jsdoc, `event:` and `external:` start a name path, so the words are
    // keys without a `:`. `null` and `undefined` are keys only with a value.
    assert_accepted(
        &[JSDOC],
        &["{event}", "{event?: T}", "{external}", "{null?: T}"],
    );
    assert_rejected(&[JSDOC], &["{null}", "{undefined}", "{null?}", "{a, null}"]);
}

#[test]
fn takes_only_keys_as_fields_without_a_value_in_jsdoc() {
    assert_rejected(
        &[JSDOC],
        &[
            "{a=}",
            "{Array<T>}",
            "{(string)}",
            "{a.b}",
            "{a|b}",
            "{a[]}",
            "{!a}",
            "{?a?}",
            "{?null}",
            "{?Array<T>}",
        ],
    );
    assert_accepted(
        &[JSDOC],
        &[
            "{?a}",
            "{?1}",
            "{?\"a\"}",
            "{1}",
            "{\"a\"}",
            "{?a, b}",
            "{?function}",
            "{?event}",
            "{a\n  b}",
            // A type as the key, with a value
            "{(string): number}",
            "{Array<T>: U}",
        ],
    );
}

#[test]
fn takes_square_bracket_properties_only_after_a_name_in_jsdoc_and_closure() {
    assert_accepted(&ALL, &["Foo[\"a\"]"]);
    assert_rejected(
        &[JSDOC, CLOSURE],
        &["obj[\"level1\"][\"level2\"]", "Foo.bar[\"a\"]"],
    );
    assert_accepted(
        &[TYPESCRIPT],
        &["obj[\"level1\"][\"level2\"]", "Foo.bar[\"a\"]"],
    );
}

#[test]
fn has_no_arrays_string_types_or_optional_fields_in_closure() {
    let inputs = [
        "string[]",
        "number[][]",
        "Foo.bar[]",
        "{a: T}[]",
        "...string|string[]",
        "typeof N[]",
        "\"foo.bar.baz\"",
        "'foo.bar.baz'",
        "Array<\"x\">",
        "{a: \"x\"}",
        "function(\"x\")",
        "{ object?: string, key: string }",
    ];
    assert_rejected(&[CLOSURE], &inputs);
    assert_accepted(&[TYPESCRIPT], &inputs);
    // jsdoc has no `typeof`.
    let jsdoc: Vec<&str> = inputs
        .into_iter()
        .filter(|input| *input != "typeof N[]")
        .collect();
    assert_accepted(&[JSDOC], &jsdoc);
    // A field with `?` and no value, string keys and `module:"…"` stay.
    assert_accepted(
        &[CLOSURE],
        &["{a?}", "{\"a\": T}", "module:\"x\"", "Foo[\"a\"]"],
    );
}

#[test]
fn has_no_function_without_parentheses_in_closure() {
    let inputs = [
        "Array<function>",
        "{a: function}",
        "function(): function",
        "function[]",
    ];
    assert_rejected(&[CLOSURE], &inputs);
    assert_accepted(&[JSDOC], &inputs);
    assert_accepted(&[CLOSURE], &["(function())", "Array<function(): void>"]);
}
