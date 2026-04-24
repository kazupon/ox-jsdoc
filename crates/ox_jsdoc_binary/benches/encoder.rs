// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Encoder microbenchmarks.
//!
//! Phase 1.1a-vi seed benchmarks. The benches measure how long it takes to
//! emit a small but non-trivial Binary AST (one `JsdocBlock` containing a
//! single `JsdocTag` with a `parsed_type`). Real JSDoc fixture coverage
//! lands in Phase 1.2b once the parser is wired through the writer.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use ox_jsdoc_binary::writer::BinaryWriter;
use ox_jsdoc_binary::writer::nodes::comment_ast::{
    write_jsdoc_block, write_jsdoc_tag, write_jsdoc_tag_name,
};
use ox_jsdoc_binary::writer::nodes::type_node::write_type_name;
use oxc_allocator::Allocator;
use oxc_span::Span;

/// Encode a `JsdocBlock` containing one tag and one parsed_type child.
fn encode_simple_param(arena: &Allocator) -> Vec<u8> {
    let mut writer = BinaryWriter::new(arena);
    let _ = writer.append_source_text("/** @param {string} id - User id */");

    // Intern the strings the writer will need.
    let empty = writer.intern_string("");
    let star = writer.intern_string("*");
    let space = writer.intern_string(" ");
    let close = writer.intern_string("*/");
    let nl = writer.intern_string("\n");
    let tag_name = writer.intern_string("param");
    let type_name = writer.intern_string("string");
    let param_name = writer.intern_string("id");
    let desc = writer.intern_string("User id");

    // Root: JsdocBlock at index 1.
    let block = write_jsdoc_block(
        &mut writer,
        Span::new(0, 35),
        0,
        None,
        star,
        space,
        close,
        nl,
        empty,
        nl,
        empty,
        0b010, // bit1 = tags present
    );

    // Child: JsdocTag (parent = block).
    let tag = write_jsdoc_tag(
        &mut writer,
        Span::new(4, 33),
        block.as_u32(),
        false, // optional
        None,  // default_value
        Some(desc),
        None,
        0b0001_1101, // bit0=tag (required) + bit2=name + bit3=parsedType + bit4=body…
    );

    // Children of the tag — order matches visitor index.
    let _ = write_jsdoc_tag_name(&mut writer, Span::new(4, 9), tag.as_u32(), tag_name);
    let _ = write_type_name(&mut writer, Span::new(11, 17), tag.as_u32(), type_name);
    let _ = write_jsdoc_tag_name(&mut writer, Span::new(19, 21), tag.as_u32(), param_name);

    writer.push_root(block.as_u32(), 0, 0);
    writer.finish()
}

fn bench_encoder(c: &mut Criterion) {
    c.bench_function("encode_simple_param_block", |b| {
        // Allocate a fresh arena per iteration so the heap reuse pattern
        // matches the eventual Phase 1.2a parser hot path.
        b.iter(|| {
            let arena = Allocator::default();
            let bytes = encode_simple_param(&arena);
            black_box(bytes)
        })
    });
}

criterion_group!(benches, bench_encoder);
criterion_main!(benches);
