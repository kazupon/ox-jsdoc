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
        // It used to come out as `module:"fo"`.
        ("module:\"foo", Jsdoc),
    ] {
        assert_rejected(input, mode);
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
