// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Integration tests for the [`LazyJsdocVisitor`] trait.
//!
//! Builds a multi-tag JsdocBlock through the writer API, walks it with a
//! counter visitor, and asserts the per-Kind counts match the fixture's
//! known node inventory. Mirrors the pattern in
//! `design/007-binary-ast/testing.md#9-visitor-traversal-tests`.

use std::collections::HashMap;

use ox_jsdoc_binary::decoder::nodes::comment_ast::{
    LazyJsdocBlock, LazyJsdocDescriptionLine, LazyJsdocTag, LazyJsdocTagName, LazyJsdocTagNameValue,
};
use ox_jsdoc_binary::decoder::nodes::type_node::{
    LazyTypeFunction, LazyTypeName, LazyTypeNumber, LazyTypeParameterList,
};
use ox_jsdoc_binary::decoder::source_file::LazySourceFile;
use ox_jsdoc_binary::decoder::visitor::LazyJsdocVisitor;
use ox_jsdoc_binary::format::kind::Kind;
use ox_jsdoc_binary::writer::BinaryWriter;
use ox_jsdoc_binary::writer::nodes::comment_ast::{
    JSDOC_BLOCK_DESC_LINES_SLOT, JSDOC_BLOCK_TAGS_SLOT, write_jsdoc_block,
    write_jsdoc_description_line, write_jsdoc_tag, write_jsdoc_tag_name,
    write_jsdoc_tag_name_value,
};
use ox_jsdoc_binary::writer::nodes::type_node::{
    TYPE_LIST_PARENT_SLOT, write_type_function, write_type_name, write_type_number,
    write_type_parameter_list,
};
use oxc_allocator::Allocator;
use oxc_span::Span;

/// Counts every visit_* call bucketed by Kind.
struct CountVisitor {
    counts: HashMap<Kind, usize>,
}

impl CountVisitor {
    fn new() -> Self {
        CountVisitor {
            counts: HashMap::new(),
        }
    }
    fn bump(&mut self, k: Kind) {
        *self.counts.entry(k).or_insert(0) += 1;
    }
    fn get(&self, k: Kind) -> usize {
        self.counts.get(&k).copied().unwrap_or(0)
    }
}

impl<'a> LazyJsdocVisitor<'a> for CountVisitor {
    fn visit_block(&mut self, b: LazyJsdocBlock<'a>) {
        self.bump(Kind::JsdocBlock);
        self.visit_block_default(b);
    }
    fn visit_description_line(&mut self, _n: LazyJsdocDescriptionLine<'a>) {
        self.bump(Kind::JsdocDescriptionLine);
    }
    fn visit_tag(&mut self, t: LazyJsdocTag<'a>) {
        self.bump(Kind::JsdocTag);
        self.visit_tag_default(t);
    }
    fn visit_tag_name(&mut self, _n: LazyJsdocTagName<'a>) {
        self.bump(Kind::JsdocTagName);
    }
    fn visit_tag_name_value(&mut self, _n: LazyJsdocTagNameValue<'a>) {
        self.bump(Kind::JsdocTagNameValue);
    }
    fn visit_type_name(&mut self, _n: LazyTypeName<'a>) {
        self.bump(Kind::TypeName);
    }
    fn visit_type_number(&mut self, _n: LazyTypeNumber<'a>) {
        self.bump(Kind::TypeNumber);
    }
    fn visit_type_function(&mut self, n: LazyTypeFunction<'a>) {
        self.bump(Kind::TypeFunction);
        self.visit_type_function_default(n);
    }
    fn visit_type_parameter_list(&mut self, n: LazyTypeParameterList<'a>) {
        self.bump(Kind::TypeParameterList);
        self.visit_type_parameter_list_default(n);
    }
}

/// Build `/** description\n * @returns {Function(string): number} ok\n */`
/// as a Binary AST. The visitor must encounter:
///
/// - 1 `JsdocBlock`
/// - 1 `JsdocDescriptionLine`
/// - 1 `JsdocTag`
/// - 1 `JsdocTagName`         (the `@returns` text)
/// - 1 `JsdocTagNameValue`    (`ok`)
/// - 1 `TypeFunction`         (the `parsedType` of the tag)
/// - 1 `TypeParameterList`    (Function's parameters)
/// - 1 `TypeName`             (the `string` parameter)
/// - 1 `TypeNumber`           (the `number` return type literal)
fn build_fixture(arena: &Allocator) -> Vec<u8> {
    let mut w = BinaryWriter::new(arena);
    let _ = w.append_source_text("/** description\n * @returns {Function(string): number} ok\n */");
    let empty = w.intern_string("");
    let star = w.intern_string("*");
    let space = w.intern_string(" ");
    let close = w.intern_string("*/");
    let nl = w.intern_string("\n");
    // String-leaf nodes need a StringIndex (TypeTag::String payload).
    let desc_str = w.intern_string_payload("description");
    let returns_str = w.intern_string_payload("returns");
    let ok_str = w.intern_string_payload("ok");
    let string_str = w.intern_string_payload("string");
    let number_lit = w.intern_string_payload("number");

    // JsdocBlock with bit0 (descriptionLines) + bit1 (tags) = 0b011.
    let (block_idx, block_ext) = write_jsdoc_block(
        &mut w,
        Span::new(0, 60),
        0,
        None,
        star,
        space,
        close,
        nl,
        empty,
        nl,
        empty,
        0b011,
    );
    let block = block_idx.as_u32();

    // descriptionLines list (1 entry, direct child of block).
    let mut desc_list = w.begin_node_list_at(block_ext, JSDOC_BLOCK_DESC_LINES_SLOT);
    let dl = write_jsdoc_description_line(&mut w, Span::new(4, 15), block, desc_str);
    w.record_list_child(&mut desc_list, dl.as_u32());
    w.finalize_node_list(desc_list);

    // tags list (1 entry, direct child of block).
    let mut tags_list = w.begin_node_list_at(block_ext, JSDOC_BLOCK_TAGS_SLOT);

    // JsdocTag: bit0 (tag) + bit2 (name) + bit3 (parsedType) = 0b1101.
    let (tag_idx, _tag_ext) = write_jsdoc_tag(
        &mut w,
        Span::new(18, 56),
        block,
        false,
        None,
        None,
        None,
        0b0000_1101,
    );
    let tag = tag_idx.as_u32();
    w.record_list_child(&mut tags_list, tag);
    w.finalize_node_list(tags_list);
    // tag name "@returns" → JsdocTagName
    let _ = write_jsdoc_tag_name(&mut w, Span::new(19, 26), tag, returns_str);
    // tag name value "ok" → JsdocTagNameValue
    let _ = write_jsdoc_tag_name_value(&mut w, Span::new(54, 56), tag, ok_str);
    // parsedType = TypeFunction with parameters + return.
    // children_bitmask = 0b011 (bit0=parameters, bit1=return_type).
    let func = write_type_function(
        &mut w,
        Span::new(28, 51),
        tag,
        false,
        false,
        true, // constructor=false, arrow=false, parenthesis=true
        0b011,
    );
    // parameters: TypeParameterList containing 1 TypeName "string".
    let (params_idx, params_ext) =
        write_type_parameter_list(&mut w, Span::new(37, 43), func.as_u32());
    let params = params_idx.as_u32();
    let mut plist = w.begin_node_list_at(params_ext, TYPE_LIST_PARENT_SLOT);
    let pn = write_type_name(&mut w, Span::new(37, 43), params, string_str);
    w.record_list_child(&mut plist, pn.as_u32());
    w.finalize_node_list(plist);
    // return_type: TypeNumber "number".
    let _ = write_type_number(&mut w, Span::new(46, 52), func.as_u32(), number_lit);

    w.push_root(block, 0, 0);
    w.finish()
}

#[test]
fn visitor_visits_all_nodes_in_fixture() {
    let arena = Allocator::default();
    let bytes = build_fixture(&arena);
    let sf = LazySourceFile::new(&bytes).unwrap();

    let mut v = CountVisitor::new();
    for opt in sf.asts() {
        if let Some(block) = opt {
            v.visit_block(block);
        }
    }

    assert_eq!(v.get(Kind::JsdocBlock), 1);
    assert_eq!(v.get(Kind::JsdocDescriptionLine), 1);
    assert_eq!(v.get(Kind::JsdocTag), 1);
    assert_eq!(v.get(Kind::JsdocTagName), 1);
    assert_eq!(v.get(Kind::JsdocTagNameValue), 1);
    assert_eq!(v.get(Kind::TypeFunction), 1);
    assert_eq!(v.get(Kind::TypeParameterList), 1);
    assert_eq!(v.get(Kind::TypeName), 1);
    assert_eq!(v.get(Kind::TypeNumber), 1);
}
