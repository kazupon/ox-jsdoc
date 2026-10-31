// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! JS reserved words are not types, as jsdoc-type-pratt-parser 9.4.0 checks
//! them the way eslint-plugin-jsdoc calls it (`classContext: true`; module,
//! strict mode and async function bodies, as by default): the whole type and
//! each member of a `|` / `&` may not be such a name, also in parentheses.
//! Elsewhere (`Array<continue>`, `{continue: T}`, `foo.continue`) they are
//! names as before, and `this`, `null`, `true`, `false` and `void` are types.

use ox_jsdoc::parser::parse_type_expression;
use ox_jsdoc::parser::type_data::ParseMode;

const ALL: [ParseMode; 3] = [ParseMode::Jsdoc, ParseMode::Closure, ParseMode::Typescript];

/// The words that jsdoc-type-pratt-parser rejects as the whole type.
const WORDS: [&str; 43] = [
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "return",
    "super",
    "switch",
    "throw",
    "try",
    "typeof",
    "var",
    "while",
    "with",
    "let",
    "static",
    "yield",
    "await",
    "enum",
    "implements",
    "interface",
    "package",
    "private",
    "protected",
    "public",
    "arguments",
    "eval",
];

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
fn rejects_reserved_words_as_the_whole_type() {
    assert_rejected(&ALL, &WORDS);
}

#[test]
fn rejects_reserved_words_as_members_of_unions_and_intersections() {
    assert_rejected(
        &ALL,
        &[
            "abc | continue",
            "continue | abc",
            "((continue))",
            "(string|continue)",
            "Array<continue|string>",
            "object|function",
            "(function)",
            "function|string",
        ],
    );
    assert_rejected(&[ParseMode::Typescript], &["abc & continue"]);
}

#[test]
fn accepts_reserved_words_elsewhere_and_the_types_among_them() {
    assert_accepted(
        &ALL,
        &[
            // `classContext: true`, as eslint-plugin-jsdoc calls it.
            "this",
            "this.foo",
            "Array<this>",
            "{a: this}",
            "{continue: string}",
            "foo.continue",
            "Array<continue>",
            "Promise<let>",
            "null",
            "true",
            "false",
            "void",
            "undefined",
            "Function",
            "function(): void",
            "function(string)",
        ],
    );
    assert_accepted(
        &[ParseMode::Jsdoc, ParseMode::Typescript],
        &["Array<function>", "let[]", "continue[]"],
    );
}
