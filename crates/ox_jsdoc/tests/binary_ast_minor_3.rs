// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Minor 3 of the Binary AST (`0x13`): optional methods (`{a?(): void}`,
//! the `optional` bit of `TypeMethodSignature`), BigInt literal types (`1n`,
//! the `TypeBigInt` kind) and the constraint or default of a type parameter
//! (`<T = U>`, two bits of `TypeTypeParameter`), as jsdoc-type-pratt-parser
//! 9.4.0 reads them. A buffer of an older minor version reads the type
//! parameters as before.

use ox_jsdoc::decoder::nodes::type_node::{LazyTypeNode, LazyTypeTypeParameter};
use ox_jsdoc::decoder::source_file::LazySourceFile;
use ox_jsdoc::format::header::{SUPPORTED_VERSION_BYTE, VERSION_OFFSET};
use ox_jsdoc::parser::type_data::ParseMode;
use ox_jsdoc::parser::{ParseOptions, parse, parse_type_expression};
use oxc_allocator::Allocator;

const JSDOC: ParseMode = ParseMode::Jsdoc;
const CLOSURE: ParseMode = ParseMode::Closure;
const TYPESCRIPT: ParseMode = ParseMode::Typescript;
const ALL: [ParseMode; 3] = [JSDOC, CLOSURE, TYPESCRIPT];

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

/// The Binary AST of `/** @type {ty} */` with `parseTypes`.
fn bytes_of(ty: &str, mode: ParseMode) -> Vec<u8> {
    let source = format!("/** @type {{{ty}}} */");
    let arena = Allocator::default();
    let options = ParseOptions {
        parse_types: true,
        type_parse_mode: mode,
        ..ParseOptions::default()
    };
    parse(&arena, &source, options).binary_bytes.to_vec()
}

/// Call `check` with the parsed type of the `@type` tag in `bytes`.
fn with_parsed_type(bytes: &[u8], check: impl FnOnce(LazyTypeNode<'_>)) {
    let source_file = LazySourceFile::new(bytes).expect("valid buffer");
    let block = source_file.asts().next().flatten().expect("parsed");
    let tag = block.tags().next().expect("@type");
    check(tag.parsed_type().expect("parsed type"));
}

/// The first type parameter of the call signature of `{<…>(…): …}`.
fn first_type_parameter<'a>(node: LazyTypeNode<'a>) -> LazyTypeTypeParameter<'a> {
    let LazyTypeNode::Object(object) = node else {
        panic!("not an object: {node:?}");
    };
    let Some(LazyTypeNode::CallSignature(signature)) = object.elements().next() else {
        panic!("not a call signature");
    };
    let list = signature.type_parameters().expect("type parameters");
    let Some(LazyTypeNode::TypeParameter(parameter)) = list.elements().next() else {
        panic!("not a type parameter");
    };
    parameter
}

/// The name of a `TypeName`, or a description of another node.
fn name(node: Option<LazyTypeNode<'_>>) -> String {
    match node {
        Some(LazyTypeNode::Name(name)) => name.value().to_owned(),
        other => format!("{other:?}"),
    }
}

#[test]
fn writes_minor_version_3() {
    assert_eq!(SUPPORTED_VERSION_BYTE, 0x13);
    assert_eq!(bytes_of("string", JSDOC)[VERSION_OFFSET], 0x13);
}

#[test]
fn reads_optional_methods() {
    assert_strings(
        &[TYPESCRIPT],
        &[
            ("{a?(): void}", "{a?(): void}"),
            ("{\"a\"?<T>(x: T): void}", "{\"a\"?<T>(x: T): void}"),
            ("{a?<T>(x: T): void}", "{a?<T>(x: T): void}"),
            ("{'a'?(): void}", "{'a'?(): void}"),
            ("{readonly?(): void}", "{readonly?(): void}"),
        ],
    );
    // jsdoc-type-pratt-parser: not for a number key, and no method
    // signatures in jsdoc and closure.
    assert_rejected(&[TYPESCRIPT], &["{1?(): void}"]);
    assert_rejected(&[JSDOC, CLOSURE], &["{a?(): void}"]);
}

#[test]
fn keeps_the_optional_flag_of_a_method() {
    for (ty, optional) in [("{a?(): void}", true), ("{a(): void}", false)] {
        with_parsed_type(&bytes_of(ty, TYPESCRIPT), |node| {
            let LazyTypeNode::Object(object) = node else {
                panic!("not an object: {node:?}");
            };
            let Some(LazyTypeNode::MethodSignature(method)) = object.elements().next() else {
                panic!("not a method signature");
            };
            assert_eq!(method.name(), "a");
            assert_eq!(method.optional(), optional, "{ty}");
        });
    }
}

#[test]
fn reads_bigint_literal_types() {
    assert_strings(
        &ALL,
        &[
            ("1n", "1n"),
            ("-1n", "-1n"),
            // As `BigInt(text).toString()` in jsdoc-type-pratt-parser.
            ("007n", "7n"),
            ("-007n", "-7n"),
            ("-0n", "0n"),
            ("1n | 2n", "1n | 2n"),
            ("Array<1n>", "Array<1n>"),
        ],
    );
    assert_strings(&[TYPESCRIPT], &[("{[1n]: T}", "{[1n]: T}")]);
    // Not a key or the right of a name path there, and `1.5n` is a number
    // and a name.
    assert_rejected(&ALL, &["{1n: T}", "Foo.1n", "1.5n"]);
}

#[test]
fn keeps_bigint_literal_types_as_their_own_kind() {
    with_parsed_type(&bytes_of("-007n", JSDOC), |node| {
        let LazyTypeNode::BigInt(bigint) = node else {
            panic!("not a BigInt: {node:?}");
        };
        // The digits as written, without the `n`, as `TypeNumber` keeps its
        // text.
        assert_eq!(bigint.value(), "-007");
    });
}

#[test]
fn tells_the_constraint_of_a_type_parameter_from_its_default() {
    let cases = [
        ("{<T = string>(x: T): T}", false, true),
        ("{<T extends string>(x: T): T}", true, false),
        ("{<T extends U = string>(x: T): T}", true, true),
        ("{<T>(x: T): T}", false, false),
    ];
    for (ty, has_constraint, has_default) in cases {
        with_parsed_type(&bytes_of(ty, TYPESCRIPT), |node| {
            let parameter = first_type_parameter(node);
            assert_eq!(parameter.has_constraint(), has_constraint, "{ty}");
            assert_eq!(parameter.has_default(), has_default, "{ty}");
            assert_eq!(name(parameter.name()), "T", "{ty}");
            let constraint = parameter.constraint();
            let default_value = parameter.default_value();
            match (has_constraint, has_default) {
                (false, true) => {
                    assert!(constraint.is_none(), "{ty}");
                    assert_eq!(name(default_value), "string", "{ty}");
                }
                (true, false) => {
                    assert_eq!(name(constraint), "string", "{ty}");
                    assert!(default_value.is_none(), "{ty}");
                }
                (true, true) => {
                    assert_eq!(name(constraint), "U", "{ty}");
                    assert_eq!(name(default_value), "string", "{ty}");
                }
                (false, false) => {
                    assert!(constraint.is_none() && default_value.is_none(), "{ty}");
                }
            }
        });
    }
}

#[test]
fn reads_the_type_parameters_of_an_older_buffer_by_their_order() {
    // Minor 2 has no flags; the second element was read as the constraint.
    let mut bytes = bytes_of("{<T = string>(x: T): T}", TYPESCRIPT);
    bytes[VERSION_OFFSET] = 0x12;
    with_parsed_type(&bytes, |node| {
        let parameter = first_type_parameter(node);
        assert!(parameter.has_constraint());
        assert!(!parameter.has_default());
        assert_eq!(name(parameter.constraint()), "string");
        assert!(parameter.default_value().is_none());
    });
}
