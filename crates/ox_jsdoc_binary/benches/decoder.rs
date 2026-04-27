// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Decoder microbenchmarks.
//!
//! Measures three lazy-access patterns against the same encoded buffer:
//!
//! - **`decode_full_walk`**: visit every property of every node (worst case).
//! - **`decode_sparse_access`**: read only the root tag count (best case for lazy).
//! - **`decode_construct_only`**: build `LazySourceFile` without touching nodes.
//!
//! Together they characterize how much overhead the lazy decoder pays per
//! accessed node; full vs sparse should differ by ~10x in line with the
//! design KPI for sparse-access patterns.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use ox_jsdoc_binary::decoder::nodes::LazyNode;
use ox_jsdoc_binary::decoder::source_file::LazySourceFile;
use ox_jsdoc_binary::writer::BinaryWriter;
use ox_jsdoc_binary::writer::nodes::comment_ast::{
    JSDOC_BLOCK_TAGS_SLOT, write_jsdoc_block, write_jsdoc_tag, write_jsdoc_tag_name,
};
use ox_jsdoc_binary::writer::nodes::type_node::write_type_name;
use oxc_allocator::Allocator;
use oxc_span::Span;

/// Build the same simple buffer used by the encoder benchmark.
fn build_buffer() -> Vec<u8> {
    let arena = Allocator::default();
    let mut writer = BinaryWriter::new(&arena);
    let _ = writer.append_source_text("/** @param {string} id - User id */");

    let empty = writer.intern_string("");
    let star = writer.intern_string("*");
    let space = writer.intern_string(" ");
    let close = writer.intern_string("*/");
    let nl = writer.intern_string("\n");
    let desc = writer.intern_string("User id");

    let tag_name = writer.intern_string_payload("param");
    let type_name = writer.intern_string_payload("string");
    let param_name = writer.intern_string_payload("id");

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
        0b010,
        None, // description_raw_span — Phase 5 opt-in, off here
    );
    let block = block_idx.as_u32();

    let mut tags_list = writer.begin_node_list_at(block_ext, JSDOC_BLOCK_TAGS_SLOT);
    let (tag_idx, _tag_ext) = write_jsdoc_tag(
        &mut writer,
        Span::new(4, 33),
        block,
        false,
        None,
        Some(desc),
        None,
        0b0000_1101,
        None, // description_raw_span — Phase 5 opt-in, off here
    );
    let tag = tag_idx.as_u32();
    writer.record_list_child(&mut tags_list, tag);
    writer.finalize_node_list(tags_list);

    let _ = write_jsdoc_tag_name(&mut writer, Span::new(4, 9), tag, tag_name);
    let _ = write_type_name(&mut writer, Span::new(11, 17), tag, type_name);
    let _ = write_jsdoc_tag_name(&mut writer, Span::new(19, 21), tag, param_name);

    writer.push_root(block, 0, 0);
    writer.finish()
}

fn bench_decoder(c: &mut Criterion) {
    let bytes = build_buffer();

    c.bench_function("decode_construct_only", |b| {
        b.iter(|| {
            let sf = LazySourceFile::new(&bytes).unwrap();
            black_box(sf);
        })
    });

    c.bench_function("decode_sparse_access", |b| {
        b.iter(|| {
            let sf = LazySourceFile::new(&bytes).unwrap();
            // Only count tags — the rest of the tree never materializes.
            let count: usize = sf
                .asts()
                .filter_map(|opt| opt)
                .map(|block| block.tags().count())
                .sum();
            black_box(count)
        })
    });

    c.bench_function("decode_full_walk", |b| {
        b.iter(|| {
            let sf = LazySourceFile::new(&bytes).unwrap();
            let mut total_str_len = 0usize;
            for block in sf.asts().filter_map(|opt| opt) {
                if let Some(d) = block.description() {
                    total_str_len += d.len();
                }
                total_str_len += block.delimiter().len();
                total_str_len += block.terminal().len();
                for tag in block.tags() {
                    total_str_len += tag.tag().value().len();
                    if let Some(desc) = tag.description() {
                        total_str_len += desc.len();
                    }
                    total_str_len += tag.range()[1] as usize;
                }
                total_str_len += block.range()[1] as usize;
            }
            black_box(total_str_len)
        })
    });
}

criterion_group!(benches, bench_decoder);
criterion_main!(benches);
