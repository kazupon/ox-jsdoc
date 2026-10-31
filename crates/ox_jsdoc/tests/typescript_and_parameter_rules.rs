// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! The rules of jsdoc-type-pratt-parser 9.4.0 for typescript, and for the
//! named parameters of `function(…)` in each mode: no `#` / `~` name paths
//! outside `module:` paths, `readonly` only before an array or a tuple,
//! tuples without `?` and with all or no elements named, arrow functions with
//! names or named parameters, signatures whose only parameter has a name,
//! and `function(…)` with only `this`, `new` (and `args` in typescript) as
//! names. The neighbours that it accepts stay accepted.

use ox_jsdoc::parser::parse_type_expression;
use ox_jsdoc::parser::type_data::ParseMode;

const JSDOC: ParseMode = ParseMode::Jsdoc;
const CLOSURE: ParseMode = ParseMode::Closure;
const TYPESCRIPT: ParseMode = ParseMode::Typescript;

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
fn rejects_jsdoc_name_paths_outside_module_paths_in_typescript() {
    assert_rejected(
        &[TYPESCRIPT],
        &[
            "MyClass#myMember",
            "MyClass~myMember",
            "MyClass#myMember#yourMember~theirMember",
            "Foo.bar#baz",
            "Foo#event:bar",
        ],
    );
    assert_accepted(
        &[TYPESCRIPT],
        &[
            "module:a#b",
            "module:a~b",
            "module:foo.bar#baz",
            "module:a#b.c",
            "Foo.event:bar",
        ],
    );
    assert_accepted(&[JSDOC, CLOSURE], &["MyClass#myMember", "MyClass~myMember"]);
}

#[test]
fn takes_readonly_only_before_an_array_or_a_tuple() {
    assert_rejected(
        &[TYPESCRIPT],
        &[
            "readonly x",
            "readonly Array<string>",
            // The rest of the type is what `readonly` takes there.
            "readonly string[] | number",
            "{a: readonly string[] | null}",
            "readonly readonly string[]",
            "readonly (string[])",
        ],
    );
    assert_accepted(
        &[TYPESCRIPT],
        &[
            "readonly string[]",
            "readonly [a, b]",
            "(readonly string[]) | number",
            "{a: readonly string[]}",
            "Array<readonly string[]>",
        ],
    );
}

#[test]
fn takes_tuples_without_question_marks_and_with_all_or_no_names() {
    assert_rejected(
        &[TYPESCRIPT],
        &["[?,null]", "[?]", "[a: string, b]", "[string, b: number]"],
    );
    assert_accepted(
        &[TYPESCRIPT],
        &[
            "[a: string, ...b: number[]]",
            "[...a: string[]]",
            "[string, ...number[]]",
            "[a?: string, b: number]",
        ],
    );
}

#[test]
fn takes_names_or_named_parameters_in_arrow_functions() {
    assert_rejected(
        &[TYPESCRIPT],
        &[
            "(Array<string>) => void",
            "(string[]) => void",
            "(\"x\") => void",
            "(...a) => void",
            "(1) => void",
            "new (Array<string>) => T",
            "<T>(Array<T>) => T",
        ],
    );
    assert_accepted(
        &[TYPESCRIPT],
        &[
            "(string) => void",
            "(a, b) => void",
            "(a: string, b) => void",
            "(...a: T) => void",
            "(a?: T) => void",
            "new (string) => T",
            "<T>(a: T, b) => T",
        ],
    );
}

#[test]
fn rejects_a_single_unnamed_parameter_of_a_signature() {
    // jsdoc-type-pratt-parser reads `(T)` as a parenthesized type there.
    assert_rejected(
        &[TYPESCRIPT],
        &[
            "{(string): T}",
            "{(...a): T}",
            "{a(string): T}",
            "{new (string): T}",
            "{[a](string): T}",
            "{<T>(T): T}",
            "{a?(string): T}",
        ],
    );
    assert_accepted(
        &[TYPESCRIPT],
        &[
            // A trailing comma makes it a parameter list there.
            "{(string,): T}",
            "{(a: string): T}",
            "{(string, number): T}",
            "{a(x: T): T}",
            "{(): T}",
        ],
    );
}

#[test]
fn names_only_this_new_and_args_as_parameters_of_function_types() {
    assert_rejected(
        &[TYPESCRIPT],
        &[
            "function(a: string)",
            "function(this: Foo, a: string)",
            "function(a: string): void",
            "function(a?: string)",
            "new (a: string): T",
        ],
    );
    assert_accepted(
        &[TYPESCRIPT],
        &[
            "function(this: Foo)",
            "function(new: Foo)",
            "function(args: string)",
            "function(...args: string)",
            "function(this?: Foo)",
            "function(a)",
            "new (this: Foo): T",
            "new (args: Foo): T",
        ],
    );
    // jsdoc and closure: only `this` and `new`.
    assert_rejected(
        &[JSDOC, CLOSURE],
        &[
            "function(a: string)",
            "function(args: string)",
            "function(...args: any[])",
            "function(a?: string)",
        ],
    );
    assert_accepted(
        &[JSDOC, CLOSURE],
        &[
            "function(this: Foo)",
            "function(new: Foo, string)",
            "function(...string)",
        ],
    );
    // closure has no named rest parameter.
    assert_accepted(&[JSDOC], &["function(...this: T)", "function(this?: Foo)"]);
    assert_rejected(&[CLOSURE], &["function(...this: T)"]);
}
