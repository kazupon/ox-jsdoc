// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Encoder microbenchmarks.
//!
//! Measures how long it takes to emit a small but non-trivial Binary AST
//! (one `JsdocBlock` containing a single `JsdocTag` with a parsed-type
//! child) using the `write_*` helpers directly. End-to-end parser timings
//! live in `parser.rs`; this bench isolates the writer's per-node cost.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use ox_jsdoc_binary::writer::BinaryWriter;
use ox_jsdoc_binary::writer::nodes::comment_ast::{
    JSDOC_BLOCK_TAGS_SLOT, write_jsdoc_block, write_jsdoc_tag, write_jsdoc_tag_name,
};
use ox_jsdoc_binary::writer::nodes::type_node::write_type_name;
use oxc_allocator::Allocator;
use oxc_span::Span;

/// Encode a `JsdocBlock` containing one tag and one parsed_type child.
fn encode_simple_param(arena: &Allocator) -> Vec<u8> {
    let mut writer = BinaryWriter::new(arena);
    let _ = writer.append_source_text("/** @param {string} id - User id */");

    // Extended-Data string slots: returned as `StringField`.
    let empty = writer.intern_string("");
    let star = writer.intern_string("*");
    let space = writer.intern_string(" ");
    let close = writer.intern_string("*/");
    let nl = writer.intern_string("\n");
    let desc = writer.intern_string("User id");

    // String-leaf node payloads: returned as `LeafStringPayload`.
    let tag_name = writer.intern_string_payload("param");
    let type_name = writer.intern_string_payload("string");
    let param_name = writer.intern_string_payload("id");

    // Root: JsdocBlock at index 1.
    let (block_idx, block_ext) = write_jsdoc_block(
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
        None,  // description_raw_span — Phase 5 opt-in, off here
    );
    let block = block_idx.as_u32();

    // tags list (1 entry, direct child of block).
    let mut tags_list = writer.begin_node_list_at(block_ext, JSDOC_BLOCK_TAGS_SLOT);

    // Child: JsdocTag (parent = block). bit0=tag (mandatory) + bit2=name +
    // bit3=parsedType = 0b1101.
    let (tag_idx, _tag_ext) = write_jsdoc_tag(
        &mut writer,
        Span::new(4, 33),
        block,
        false, // optional
        None,  // default_value
        Some(desc),
        None,
        0b0000_1101,
        None, // description_raw_span — Phase 5 opt-in, off here
    );
    let tag = tag_idx.as_u32();
    writer.record_list_child(&mut tags_list, tag);
    writer.finalize_node_list(tags_list);

    // Children of the tag — order matches visitor index.
    let _ = write_jsdoc_tag_name(&mut writer, Span::new(4, 9), tag, tag_name);
    let _ = write_type_name(&mut writer, Span::new(11, 17), tag, type_name);
    let _ = write_jsdoc_tag_name(&mut writer, Span::new(19, 21), tag, param_name);

    writer.push_root(block, 0, 0);
    writer.finish()
}

fn bench_encoder(c: &mut Criterion) {
    c.bench_function("encode_simple_param_block", |b| {
        // Allocate a fresh arena per iteration so the heap reuse pattern
        // matches the parser hot path.
        b.iter(|| {
            let arena = Allocator::default();
            let bytes = encode_simple_param(&arena);
            black_box(bytes)
        })
    });
}

criterion_group!(benches, bench_encoder);
criterion_main!(benches);
