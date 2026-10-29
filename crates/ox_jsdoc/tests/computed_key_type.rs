// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! The computed keys of object types (`{[k]: T}`, `{[k]<U>(a: A): R}`) are
//! the keys of their object fields, as `TypeComputedProperty` and
//! `TypeComputedMethod`, as in jsdoc-type-pratt-parser. The lazy decoder and
//! the visitor read them.

use ox_jsdoc::decoder::nodes::type_node::{LazyTypeName, LazyTypeNode, LazyTypeObjectField};
use ox_jsdoc::decoder::visitor::LazyJsdocVisitor;
use ox_jsdoc::parser::type_data::ParseMode;
use ox_jsdoc::parser::{ParseOptions, parse};
use oxc_allocator::Allocator;

fn options() -> ParseOptions {
    ParseOptions {
        parse_types: true,
        type_parse_mode: ParseMode::Typescript,
        ..ParseOptions::default()
    }
}

/// Parse `/** @type {ty} */` and call `check` with the fields of the object
/// type.
fn with_fields(ty: &str, check: impl FnOnce(Vec<LazyTypeObjectField<'_>>)) {
    let source = format!("/** @type {{{ty}}} */");
    let arena = Allocator::default();
    let result = parse(&arena, &source, options());
    let root = result.lazy_root.expect("parsed");
    let tag = root.tags().next().expect("@type");
    let Some(LazyTypeNode::Object(object)) = tag.parsed_type() else {
        panic!("not an object type: {:?}", tag.parsed_type());
    };
    let fields = object
        .elements()
        .map(|node| match node {
            LazyTypeNode::ObjectField(field) => field,
            other => panic!("not an object field: {other:?}"),
        })
        .collect();
    check(fields);
}

/// The name of a `TypeName`, or a description of another node.
fn name(node: Option<LazyTypeNode<'_>>) -> String {
    match node {
        Some(LazyTypeNode::Name(name)) => name.value().to_owned(),
        other => format!("{other:?}"),
    }
}

#[test]
fn reads_a_computed_property() {
    with_fields("{readonly [k]?: string}", |fields| {
        let field = fields[0];
        assert!(field.optional());
        assert!(field.readonly());
        let Some(LazyTypeNode::ComputedProperty(key)) = field.key() else {
            panic!("not a computed property: {:?}", field.key());
        };
        assert_eq!(name(key.value()), "k");
        assert_eq!(name(field.right()), "string");
    });
}

#[test]
fn reads_strings_and_numbers_as_computed_keys() {
    with_fields("{[\"a\"]: T, [1](): U}", |fields| {
        let Some(LazyTypeNode::ComputedProperty(key)) = fields[0].key() else {
            panic!("not a computed property: {:?}", fields[0].key());
        };
        let Some(LazyTypeNode::StringValue(value)) = key.value() else {
            panic!("not a string: {:?}", key.value());
        };
        assert_eq!(value.value(), "\"a\"");
        assert_eq!(value.quote(), 2);
        let Some(LazyTypeNode::ComputedMethod(key)) = fields[1].key() else {
            panic!("not a computed method: {:?}", fields[1].key());
        };
        let Some(LazyTypeNode::Number(value)) = key.value() else {
            panic!("not a number: {:?}", key.value());
        };
        assert_eq!(value.value(), "1");
    });
}

#[test]
fn reads_a_computed_method() {
    with_fields("{[k]?<T>(a: T, b): R}", |fields| {
        let field = fields[0];
        // The `?` belongs to the method, as in jsdoc-type-pratt-parser.
        assert!(!field.optional());
        assert!(!field.readonly());
        assert!(field.right().is_none());
        let Some(LazyTypeNode::ComputedMethod(key)) = field.key() else {
            panic!("not a computed method: {:?}", field.key());
        };
        assert!(key.optional());
        assert_eq!(name(key.value()), "k");
        let parameters: Vec<_> = key
            .parameters()
            .expect("parameters")
            .elements()
            .map(|node| match node {
                LazyTypeNode::KeyValue(kv) => format!("{}: {}", kv.key(), name(kv.right())),
                other => name(Some(other)),
            })
            .collect();
        assert_eq!(parameters, ["a: T", "b"]);
        assert_eq!(name(key.return_type()), "R");
        let type_parameters: Vec<_> = key
            .type_parameters()
            .expect("type parameters")
            .elements()
            .map(|node| match node {
                LazyTypeNode::TypeParameter(tp) => name(tp.elements().next()),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(type_parameters, ["T"]);
    });
}

#[test]
fn reads_a_computed_method_without_parameters() {
    with_fields("{[k](): R}", |fields| {
        let Some(LazyTypeNode::ComputedMethod(key)) = fields[0].key() else {
            panic!("not a computed method: {:?}", fields[0].key());
        };
        assert!(!key.optional());
        assert_eq!(name(key.value()), "k");
        assert!(key.parameters().is_none());
        assert_eq!(name(key.return_type()), "R");
        assert!(key.type_parameters().is_none());
    });
}

/// Collects the names it visits.
struct Names(Vec<String>);

impl<'a> LazyJsdocVisitor<'a> for Names {
    fn visit_type_name(&mut self, node: LazyTypeName<'a>) {
        self.0.push(node.value().to_owned());
    }
}

#[test]
fn visits_the_parts_of_computed_keys() {
    let arena = Allocator::default();
    let source = "/** @type {{[a]: B, [c]<D>(e: F): G}} */";
    let result = parse(&arena, source, options());
    let mut names = Names(Vec::new());
    names.visit_block(result.lazy_root.expect("parsed"));
    // The key and the value of each field; a method's value, parameters,
    // return type and type parameters, in that order.
    assert_eq!(names.0, ["a", "B", "c", "F", "G", "D"]);
}
