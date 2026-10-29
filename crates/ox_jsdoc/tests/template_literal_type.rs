// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! The interpolations of a template literal type are the children of its
//! node in the Binary AST; the lazy decoder and the visitor read them.

use ox_jsdoc::decoder::nodes::type_node::{LazyTypeName, LazyTypeNode};
use ox_jsdoc::decoder::visitor::LazyJsdocVisitor;
use ox_jsdoc::parser::type_data::ParseMode;
use ox_jsdoc::parser::{ParseOptions, parse};
use oxc_allocator::Allocator;

const SOURCE: &str = "/** @type {`a${b}c${d | e}`} */";

fn options() -> ParseOptions {
    ParseOptions {
        parse_types: true,
        type_parse_mode: ParseMode::Typescript,
        ..ParseOptions::default()
    }
}

#[test]
fn reads_the_interpolations_of_a_template_literal() {
    let arena = Allocator::default();
    let result = parse(&arena, SOURCE, options());
    let root = result.lazy_root.expect("parsed");
    let tag = root.tags().next().expect("@type");
    let Some(LazyTypeNode::TemplateLiteral(literal)) = tag.parsed_type() else {
        panic!("not a template literal: {:?}", tag.parsed_type());
    };
    let literals: Vec<_> = (0..literal.literal_count())
        .map(|i| literal.literal(i))
        .collect();
    assert_eq!(literals, ["a", "c", ""]);
    let kinds: Vec<_> = literal
        .interpolations()
        .map(|node| match node {
            LazyTypeNode::Name(name) => name.value().to_owned(),
            LazyTypeNode::Union(_) => "union".to_owned(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(kinds, ["b", "union"]);
}

/// Collects the names it visits.
struct Names(Vec<String>);

impl<'a> LazyJsdocVisitor<'a> for Names {
    fn visit_type_name(&mut self, node: LazyTypeName<'a>) {
        self.0.push(node.value().to_owned());
    }
}

#[test]
fn visits_the_interpolations_of_a_template_literal() {
    let arena = Allocator::default();
    let result = parse(&arena, SOURCE, options());
    let mut names = Names(Vec::new());
    names.visit_block(result.lazy_root.expect("parsed"));
    assert_eq!(names.0, ["b", "d", "e"]);
}
