// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Minor 4 of the Binary AST (`0x14`): a `TypeMethodSignature` wraps its
//! parameters and its type parameters in `TypeParameterList` children,
//! around its return type, as `format.md` says, so a decoder can tell them
//! apart. A buffer of an older minor version, whose writer wrote them one
//! after the other, gives no children.

use ox_jsdoc::decoder::nodes::type_node::{
    LazyTypeMethodSignature, LazyTypeName, LazyTypeNode, LazyTypeParameterList,
};
use ox_jsdoc::decoder::source_file::LazySourceFile;
use ox_jsdoc::decoder::visitor::LazyJsdocVisitor;
use ox_jsdoc::format::header::{SUPPORTED_VERSION_BYTE, VERSION_OFFSET};
use ox_jsdoc::parser::type_data::ParseMode;
use ox_jsdoc::parser::{ParseOptions, parse};
use oxc_allocator::Allocator;

/// The Binary AST of `/** @type {ty} */`, with the types in TypeScript mode.
fn bytes_of(ty: &str) -> Vec<u8> {
    let source = format!("/** @type {{{ty}}} */");
    let arena = Allocator::default();
    let options = ParseOptions {
        parse_types: true,
        type_parse_mode: ParseMode::Typescript,
        ..ParseOptions::default()
    };
    parse(&arena, &source, options).binary_bytes.to_vec()
}

/// Call `check` with the method signature that the object type in `bytes`
/// starts with.
fn with_method(bytes: &[u8], check: impl FnOnce(LazyTypeMethodSignature<'_>)) {
    let source_file = LazySourceFile::new(bytes).expect("valid buffer");
    let block = source_file.asts().next().flatten().expect("parsed");
    let tag = block.tags().next().expect("@type");
    let Some(LazyTypeNode::Object(object)) = tag.parsed_type() else {
        panic!("not an object");
    };
    let Some(LazyTypeNode::MethodSignature(method)) = object.elements().next() else {
        panic!("not a method signature");
    };
    check(method);
}

/// `T` for a name, `a: T` for a parameter, `<T>` for a type parameter.
fn describe(node: LazyTypeNode<'_>) -> String {
    match node {
        LazyTypeNode::Name(name) => name.value().to_owned(),
        LazyTypeNode::KeyValue(parameter) => {
            format!(
                "{}: {}",
                parameter.key(),
                parameter.right().map(describe).unwrap_or_default()
            )
        }
        LazyTypeNode::TypeParameter(parameter) => {
            format!("<{}>", parameter.name().map(describe).unwrap_or_default())
        }
        other => format!("{other:?}"),
    }
}

fn describe_list(list: Option<LazyTypeParameterList<'_>>) -> Option<Vec<String>> {
    list.map(|list| list.elements().map(describe).collect())
}

#[test]
fn writes_minor_version_4() {
    assert_eq!(SUPPORTED_VERSION_BYTE, 0x14);
    assert_eq!(bytes_of("{a(): void}")[VERSION_OFFSET], 0x14);
}

#[test]
fn reads_the_parameters_the_return_type_and_the_type_parameters() {
    let cases: [(&str, Option<&[&str]>, &str, Option<&[&str]>); 4] = [
        (
            "{foo<T, U>(a: T, b: number): U}",
            Some(&["a: T", "b: number"]),
            "U",
            Some(&["<T>", "<U>"]),
        ),
        ("{foo(a: string): R}", Some(&["a: string"]), "R", None),
        ("{foo<T>(): T}", None, "T", Some(&["<T>"])),
        ("{foo(): R}", None, "R", None),
    ];
    for (ty, parameters, return_type, type_parameters) in cases {
        with_method(&bytes_of(ty), |method| {
            assert_eq!(method.name(), "foo", "{ty}");
            let to_vec = |list: Option<&[&str]>| {
                list.map(|names| {
                    names
                        .iter()
                        .map(|name| (*name).to_owned())
                        .collect::<Vec<_>>()
                })
            };
            assert_eq!(
                describe_list(method.parameters()),
                to_vec(parameters),
                "{ty}"
            );
            assert_eq!(
                method.return_type().map(describe).as_deref(),
                Some(return_type),
                "{ty}"
            );
            assert_eq!(
                describe_list(method.type_parameters()),
                to_vec(type_parameters),
                "{ty}"
            );
        });
    }
}

#[test]
fn gives_no_children_in_a_buffer_of_minor_version_3() {
    let mut bytes = bytes_of("{foo<T>(a: T): T}");
    bytes[VERSION_OFFSET] = 0x13;
    with_method(&bytes, |method| {
        assert_eq!(method.name(), "foo");
        assert!(method.has_parameters() && method.has_type_parameters());
        assert!(method.parameters().is_none());
        assert!(method.return_type().is_none());
        assert!(method.type_parameters().is_none());
    });
}

/// The names of the `TypeName` nodes that the visitor reaches, in order.
#[derive(Default)]
struct Names(Vec<String>);

impl<'a> LazyJsdocVisitor<'a> for Names {
    fn visit_type_name(&mut self, node: LazyTypeName<'a>) {
        self.0.push(node.value().to_owned());
    }
}

#[test]
fn walks_the_children_of_a_method_signature() {
    let bytes = bytes_of("{foo<T>(a: A, b: B): R}");
    let source_file = LazySourceFile::new(&bytes).unwrap();
    let mut names = Names::default();
    for block in source_file.asts().flatten() {
        names.visit_block(block);
    }
    // The parameters, the return type, then the type parameters.
    assert_eq!(names.0, ["A", "B", "R", "T"]);
}
