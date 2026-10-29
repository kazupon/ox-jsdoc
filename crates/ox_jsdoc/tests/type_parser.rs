// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Type expressions that used to crash, hang or be silently cut short.
//!
//! The expected results follow jsdoc-type-pratt-parser 9.4.0, the de facto
//! spec of the type syntax (`design/006-parsed-type/README.md`).
//!
//! Each input is parsed on its own thread with a time limit, so a regression
//! that panics or loops fails one test with the input in the message,
//! instead of aborting or stalling the whole run.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use ox_jsdoc::parser::type_data::ParseMode::{self, Closure, Jsdoc, Typescript};
use ox_jsdoc::parser::{ParseOptions, parse, parse_type_check, parse_type_expression};
use oxc_allocator::Allocator;

const TIME_LIMIT: Duration = Duration::from_secs(10);

/// Run `f` on another thread. Fail with `what` in the message if it panics
/// or does not return within [`TIME_LIMIT`].
fn run_isolated<T: Send + 'static>(what: String, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(f());
    });
    match receiver.recv_timeout(TIME_LIMIT) {
        Ok(value) => value,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("{what} did not return within {TIME_LIMIT:?}")
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => panic!("{what} panicked"),
    }
}

/// Parse a type expression with both entry points, which must agree.
fn parse_type(input: &str, mode: ParseMode) -> Option<String> {
    let owned = input.to_owned();
    let (stringified, accepted) = run_isolated(format!("{input:?} ({mode:?})"), move || {
        (
            parse_type_expression(&owned, mode),
            parse_type_check(&owned, mode),
        )
    });
    assert_eq!(
        stringified.is_some(),
        accepted,
        "parse_type_expression and parse_type_check disagree on {input:?} ({mode:?})"
    );
    stringified
}

#[track_caller]
fn assert_parsed(input: &str, mode: ParseMode, expected: &str) {
    assert_eq!(
        parse_type(input, mode).as_deref(),
        Some(expected),
        "{input:?} ({mode:?})"
    );
}

#[track_caller]
fn assert_accepted(input: &str, mode: ParseMode) {
    assert!(
        parse_type(input, mode).is_some(),
        "{input:?} ({mode:?}) was rejected"
    );
}

#[track_caller]
fn assert_rejected(input: &str, mode: ParseMode) {
    let result = parse_type(input, mode);
    assert!(
        result.is_none(),
        "{input:?} ({mode:?}) was accepted as {result:?}"
    );
}

// Non-ASCII names used to make the type parser panic.

#[test]
fn accepts_non_ascii_names() {
    for (input, mode) in [
        ("é", Jsdoc),
        ("Größe", Jsdoc),
        ("Größe", Typescript),
        ("日本語", Typescript),
        ("café", Closure),
        ("fooébar", Typescript),
        ("Array<日本語>", Typescript),
        ("Foo.é", Jsdoc),
        ("Foo.é", Typescript),
        ("Foo#é", Jsdoc),
        ("Foo~é", Jsdoc),
        ("module:é", Jsdoc),
        ("{é: string}", Typescript),
        ("T extends infer é ? 1 : 2", Typescript),
        // The loose lexer (jsdoc, closure) also takes `-` in names.
        ("Größe-Klasse", Jsdoc),
        // ZWNJ (U+200C) continues an identifier.
        ("a\u{200c}b", Typescript),
    ] {
        assert_parsed(input, mode, input);
    }
    // Accepted. The stringified form drops the key type (`{[é: number]}`),
    // as it does with ASCII names; that is a separate bug.
    assert_accepted("{[é: string]: number}", Typescript);
}

#[test]
fn accepts_non_ascii_names_in_comments_with_parse_types() {
    let diagnostics = run_isolated("parse(…, parse_types: true)".to_owned(), || {
        let arena = Allocator::default();
        let options = ParseOptions {
            parse_types: true,
            ..ParseOptions::default()
        };
        let result = parse(&arena, "/** @param {Foo.é} x desc */", options);
        result.diagnostics.len()
    });
    assert_eq!(diagnostics, 0);
}

// Characters that the lexer did not know used to end the type without an
// error.

#[test]
fn rejects_unknown_characters() {
    for (input, mode) in [
        ("%", Jsdoc),
        ("string%x", Jsdoc),
        ("string%x", Typescript),
        // `-` is not part of a name in the strict lexer (typescript).
        ("my-type", Typescript),
        // ZWSP (U+200B) is neither whitespace nor part of a name.
        ("a\u{200b}b", Typescript),
        ("Array<%>", Typescript),
        ("string | %", Typescript),
        ("{a: %}", Typescript),
        ("[%]", Typescript),
    ] {
        assert_rejected(input, mode);
    }
}

#[test]
fn rejects_unknown_characters_where_a_name_or_key_is_expected() {
    for (input, mode) in [
        ("Foo.%", Jsdoc),
        ("Foo.", Jsdoc),
        ("module:%", Jsdoc),
        ("{%: string}", Typescript),
        ("<%>() => void", Typescript),
        ("T extends infer % ? 1 : 2", Typescript),
        ("{[%: string]: number}", Typescript),
    ] {
        assert_rejected(input, mode);
    }
}

#[test]
fn rejects_unterminated_strings_and_template_literals() {
    for (input, mode) in [
        ("\"abc", Typescript),
        ("'abc", Jsdoc),
        ("`abc", Typescript),
        ("\"é", Typescript),
        ("'é", Jsdoc),
        ("`é", Typescript),
        // It used to lose its last character and come out as a closed string.
        ("module:\"foo", Jsdoc),
    ] {
        assert_rejected(input, mode);
    }
}

#[test]
fn rejects_invalid_template_literal_interpolations() {
    for input in [
        "`x${foo%bar}`",
        "`x${foo bar}`",
        "`x${foo`",
        // jsdoc-type-pratt-parser 9.4.0 does not return on these two;
        // TypeScript rejects them.
        "`x${%}`",
        "`x${}`",
    ] {
        assert_rejected(input, Typescript);
    }
    // Accepted. The stringified form drops the backquotes, which is a
    // separate bug.
    for input in [
        "`x${foo}`",
        "`x${foo}y${bar}`",
        "`x${foo | bar}`",
        "`x${Größe}`",
    ] {
        assert_accepted(input, Typescript);
    }
}

#[test]
fn skips_unicode_whitespace() {
    for (input, mode, expected) in [
        ("Array<\u{a0}string>", Typescript, "Array<string>"),
        ("\u{3000}string", Jsdoc, "string"),
        ("string\u{feff}", Jsdoc, "string"),
        ("string\u{b}", Jsdoc, "string"),
    ] {
        assert_parsed(input, mode, expected);
    }
    // NBSP separates two names, so the second one is left over.
    assert_rejected("a\u{a0}b", Typescript);
}

#[test]
fn keeps_accepting_names_and_paths() {
    for (input, mode) in [
        ("$foo", Typescript),
        ("_bar", Typescript),
        ("foo$", Jsdoc),
        ("My-1st-Class", Jsdoc),
        ("module:", Jsdoc),
        ("module:foo/bar.baz", Jsdoc),
        ("module:\"foo/bar\"", Jsdoc),
        ("Foo.123", Jsdoc),
        ("Foo.\"bar\"", Jsdoc),
        ("{\"é\": string}", Typescript),
    ] {
        assert_parsed(input, mode, input);
    }
}

// Nested `extends` in the `extends` clause of a conditional type used to make
// the parser loop forever, also for valid conditional types inside brackets.

#[test]
fn rejects_nested_extends_without_looping() {
    for input in [
        "A extends B extends C ? D : E",
        // A constraint on `infer` is not supported, as in
        // jsdoc-type-pratt-parser.
        "T extends infer U extends string ? U : never",
        "T extends [infer U extends string] ? U : never",
        "T extends (infer U extends string) ? U : never",
    ] {
        assert_rejected(input, Typescript);
    }
}

#[test]
fn accepts_conditional_types_inside_brackets_in_an_extends_clause() {
    for input in [
        "A extends (B extends C ? D : E) ? F : G",
        "A extends Foo<B extends C ? D : E> ? F : G",
        "A extends [B extends C ? D : E] ? F : G",
        "A extends {a: B extends C ? D : E} ? F : G",
        "A extends (x: B extends C ? D : E) => void ? F : G",
        "A extends B[C extends D ? E : F] ? G : H",
    ] {
        assert_parsed(input, Typescript, input);
    }
}

#[test]
fn keeps_accepting_conditional_types() {
    for input in [
        "A extends B ? C : D",
        "A extends B ? C extends D ? 1 : 2 : 3",
        "A extends B ? C : D extends E ? F : G",
        "A extends B ? C | D : E",
        "Foo<A extends B ? C : D>",
        "(A extends B ? C : D)[]",
        "A extends B[] ? 1 : 2",
        "A extends Foo<B> ? 1 : 2",
        "A extends keyof B ? 1 : 2",
        "A extends (infer U) ? U : never",
        // The return type is outside the parameter list, so `?` ends the
        // `extends` clause.
        "A extends () => void ? F : G",
        "A extends (x: string) => void ? F : G",
        // jsdoc-type-pratt-parser rejects these two, but they are valid
        // TypeScript.
        "A extends B | C ? D : E",
        "A extends B & C ? D : E",
    ] {
        assert_parsed(input, Typescript, input);
    }
}

// A missing bracket, parenthesis or colon used to be accepted as if it were
// there, and the type was reported as valid.

#[test]
fn rejects_a_missing_bracket_parenthesis_or_colon() {
    for (input, mode) in [
        // `>` after type parameters
        ("{<T(x: T): T}", Typescript),
        ("{<T, U(x: T): U}", Typescript),
        // `(` and `)` of call, constructor and method signatures
        ("{(x: string: number}", Typescript),
        ("{(x: string, y: number: number}", Typescript),
        ("{<T> x: T): T}", Typescript),
        ("{<T>(x: T: T}", Typescript),
        ("{new <T> x: T): Foo}", Typescript),
        ("{new (x: string: Foo}", Typescript),
        ("{foo<T> x: T): T}", Typescript),
        ("{foo(x: string: number}", Typescript),
        ("{[k]<T> x: T): T}", Typescript),
        ("{[k](x: string: number}", Typescript),
        // `]` and `:` of mapped types, index signatures and computed keys
        ("{[K in T: V}", Typescript),
        ("{[K in T] V}", Typescript),
        ("{[k: string: number}", Typescript),
        ("{[k: string] number}", Typescript),
        ("{[k string}", Typescript),
        ("{[k] string}", Typescript),
        // `)` of function types
        ("function(string: boolean", Jsdoc),
        ("function(string, number", Jsdoc),
        ("function(string: boolean", Typescript),
        ("function(string, number", Closure),
        ("new (x: string => Foo", Typescript),
        ("new (x: string, y: number => Foo", Typescript),
        // `(` after `import`, and `]` after a quoted property
        ("import \"foo\")", Typescript),
        ("Foo[\"bar\".baz", Jsdoc),
    ] {
        assert_rejected(input, mode);
    }
}

#[test]
fn accepts_a_missing_colon_or_question_mark_where_jsdoc_type_pratt_parser_does() {
    for (input, expected) in [
        // The `:` before the return type of a signature
        ("{(x: string) number}", "{(x: string): number}"),
        ("{<T>(x: T) T}", "{<T>(x: T): T}"),
        ("{new (x: string) Foo}", "{new (x: string): Foo}"),
        ("{foo(x: string) number}", "{foo(x: string): number}"),
        // The `?` and `:` of a conditional type
        ("A extends B C : D", "A extends B ? C : D"),
        ("A extends B ? C D", "A extends B ? C : D"),
    ] {
        assert_parsed(input, Typescript, expected);
    }
    // Accepted. The stringified form drops the brackets of the computed key,
    // which is a separate bug.
    assert_accepted("{[k](x: string) number}", Typescript);
}

#[test]
fn leaves_no_diagnostics_for_a_type_that_parses() {
    for source in [
        "/** @type {A extends B C : D} */",
        "/** @type {{(x: string) number}} */",
    ] {
        let diagnostics = run_isolated(format!("parse({source:?})"), move || {
            let arena = Allocator::default();
            let options = ParseOptions {
                parse_types: true,
                type_parse_mode: Typescript,
                ..ParseOptions::default()
            };
            parse(&arena, source, options).diagnostics.len()
        });
        assert_eq!(diagnostics, 0, "{source:?}");
    }
}

#[test]
fn keeps_accepting_signatures_and_brackets() {
    for (input, mode) in [
        ("{<T>(x: T): T}", Typescript),
        ("{(x: string): number}", Typescript),
        ("{new (x: string): Foo}", Typescript),
        ("{new <T>(x: T): Foo}", Typescript),
        ("{foo(x: string): number}", Typescript),
        ("{foo<T>(x: T): T}", Typescript),
        ("function(string): boolean", Jsdoc),
        ("function(string, number): boolean", Closure),
        ("import(\"foo\")", Typescript),
        ("A[B]", Typescript),
        ("A extends B ? C : D", Typescript),
    ] {
        assert_parsed(input, mode, input);
    }
    // Accepted. Their stringified forms are lossy, which is a separate bug.
    for (input, mode) in [
        ("{[K in T]: V}", Typescript),
        ("{[K in T]?: V}", Typescript),
        ("{[k: string]: number}", Typescript),
        ("{[k]: string}", Typescript),
        ("{[k](x: string): number}", Typescript),
        ("{[k]<T>(x: T): T}", Typescript),
        ("new (x: string) => Foo", Typescript),
        ("Foo[\"bar\"]", Jsdoc),
    ] {
        assert_accepted(input, mode);
    }
}

// Punctuation where a name or key is expected used to be taken as the name.

#[test]
fn rejects_punctuation_where_a_name_or_key_is_expected() {
    for (input, mode) in [
        // The right side of a name path
        ("Foo.(", Jsdoc),
        ("Foo.)", Jsdoc),
        ("Foo.>", Jsdoc),
        ("Foo.|", Jsdoc),
        ("Foo.:", Jsdoc),
        ("Foo#(", Jsdoc),
        ("Foo~)", Jsdoc),
        ("Foo.`a`", Typescript),
        // Object keys
        ("{): string}", Typescript),
        ("{,}", Typescript),
        ("{;}", Typescript),
        ("{a: string,,}", Typescript),
        ("{a: string, ;}", Typescript),
        ("{`a`: string}", Typescript),
        // Index signature and mapped type keys; a mapped type needs a name
        ("{[(: string]: number}", Typescript),
        ("{[): string]: number}", Typescript),
        ("{[1 in T]: V}", Typescript),
        ("{[\"a\" in T]: V}", Typescript),
        // Type parameters and `infer` need a name
        ("{<1>(x: T): T}", Typescript),
        ("{<\"a\">(x: T): T}", Typescript),
        ("T extends infer ( ? 1 : 2", Typescript),
        ("T extends infer ) ? 1 : 2", Typescript),
        ("T extends infer 1 ? 1 : 2", Typescript),
        ("T extends infer \"a\" ? 1 : 2", Typescript),
        // The value of `module:` and the like is empty here, so the token is
        // left over
        ("module:(", Jsdoc),
        ("module:)", Jsdoc),
        ("module:,", Jsdoc),
        ("module:{", Jsdoc),
        ("module:<", Jsdoc),
        ("module:(", Closure),
        ("module:(", Typescript),
        ("module:null", Jsdoc),
        ("module:1", Jsdoc),
        ("module:*", Jsdoc),
        ("module:./foo", Jsdoc),
    ] {
        assert_rejected(input, mode);
    }
}

#[test]
fn accepts_names_numbers_and_strings_where_jsdoc_type_pratt_parser_does() {
    for (input, mode) in [
        ("Foo.1", Jsdoc),
        ("Foo.\"a\"", Jsdoc),
        ("Foo.null", Jsdoc),
        ("Foo.function", Jsdoc),
        ("Foo.module", Jsdoc),
        ("{1: string}", Typescript),
        ("{\"a\": string}", Typescript),
        ("{null: string}", Typescript),
        ("{function: string}", Typescript),
        ("{in: string}", Typescript),
        ("{<null>(x: T): T}", Typescript),
        ("{<in>(x: T): T}", Typescript),
        ("T extends infer in ? 1 : 2", Typescript),
        ("module:", Jsdoc),
        ("module:foo", Jsdoc),
        ("module:\"a\"", Jsdoc),
        ("module:/foo", Jsdoc),
        ("module:@foo", Jsdoc),
        ("module:#foo", Jsdoc),
        ("module:~foo", Jsdoc),
        ("module:foo/bar", Jsdoc),
        ("module:foo.bar~baz#qux", Jsdoc),
        ("module:foo-bar", Jsdoc),
        ("module:@scope/pkg", Jsdoc),
        ("event:", Jsdoc),
        ("event:foo", Jsdoc),
        ("external:foo", Jsdoc),
        // An empty value leaves the closing token to the outer type
        ("(module:)", Jsdoc),
        ("Array<module:>", Jsdoc),
        ("Array<module:foo>", Jsdoc),
    ] {
        assert_parsed(input, mode, input);
    }
    // A union of an empty `module:` and `string`
    assert_parsed("module:|string", Jsdoc, "module: | string");
    // Accepted. Their stringified forms drop the key type, which is a
    // separate bug.
    for input in [
        "{[1: string]: number}",
        "{[\"a\": string]: number}",
        "{[null: string]: number}",
        "{[in in T]: V}",
    ] {
        assert_accepted(input, Typescript);
    }
}

// Valid types with a trailing separator used to be rejected.

#[test]
fn accepts_trailing_separators() {
    for (input, mode, expected) in [
        ("{a: string, }", Typescript, "{a: string}"),
        ("{a: string,}", Typescript, "{a: string}"),
        ("{a: string; }", Typescript, "{a: string}"),
        ("{a: string, b: number,}", Typescript, "{a: string, b: number}"),
        ("{a: string, }", Jsdoc, "{a: string}"),
        ("{a, }", Jsdoc, "{a}"),
        ("{a: string, }", Closure, "{a: string}"),
        ("{(a: string, ): void}", Typescript, "{(a: string): void}"),
        ("{<T>(a: T, ): T}", Typescript, "{<T>(a: T): T}"),
        ("{foo(a: string, ): void}", Typescript, "{foo(a: string): void}"),
        ("{new (a: string, ): Foo}", Typescript, "{new (a: string): Foo}"),
    ] {
        assert_parsed(input, mode, expected);
    }
    // Accepted. The stringified form drops `new`, which is a separate bug.
    assert_accepted("new (a: string, ) => Foo", Typescript);
}
