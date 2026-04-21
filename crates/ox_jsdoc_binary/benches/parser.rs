// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Per-phase benchmarks for the parse → emit → finish pipeline.
//!
//! Splits the public `parse_to_bytes` body into its three measurable stages
//! so we can spot which one dominates and where future optimisation work
//! should focus. Run with `cargo bench --bench parser`.

use std::fs;
use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use ox_jsdoc_binary::parser::{
    context::{emit_block, parse_block_into_data},
    parse_batch_to_bytes, parse_to_bytes, BatchItem, ParseOptions,
};
use ox_jsdoc_binary::writer::BinaryWriter;
use oxc_allocator::Allocator;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures/perf/source/typescript-checker.ts")
}

/// Pull every `/** ... */` block out of the fixture so the benchmarks see
/// the same comment corpus the JS-side `parse-batch-vs-loop` script uses.
fn extract_jsdoc_blocks(source: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0usize;
    while i + 4 < bytes.len() {
        if &bytes[i..i + 3] == b"/**" {
            let mut end = i + 3;
            while end + 1 < bytes.len() {
                if bytes[end] == b'*' && bytes[end + 1] == b'/' {
                    break;
                }
                end += 1;
            }
            if end + 1 < bytes.len() {
                end += 2;
                blocks.push(source[i..end].to_string());
                i = end;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    blocks
}

fn load_fixture() -> Vec<String> {
    let source = fs::read_to_string(fixture_path()).expect("read fixture");
    extract_jsdoc_blocks(&source)
}

fn bench_parse_to_bytes_full(c: &mut Criterion) {
    let blocks = load_fixture();
    c.bench_function("parse_to_bytes (loop, full file)", |b| {
        b.iter(|| {
            for src in &blocks {
                let _ = black_box(parse_to_bytes(src.as_str(), ParseOptions::default()));
            }
        });
    });
}

fn bench_parse_batch_to_bytes(c: &mut Criterion) {
    let blocks = load_fixture();
    let items: Vec<BatchItem<'_>> = blocks
        .iter()
        .map(|s| BatchItem {
            source_text: s.as_str(),
            base_offset: 0,
        })
        .collect();
    c.bench_function("parse_batch_to_bytes (single batch, full file)", |b| {
        b.iter(|| {
            let _ = black_box(parse_batch_to_bytes(&items, ParseOptions::default()));
        });
    });
}

/// Phase 1 only: structural parse (no binary emission).
fn bench_parse_block_into_data(c: &mut Criterion) {
    let blocks = load_fixture();
    c.bench_function("phase 1 — parse_block_into_data only (full file)", |b| {
        b.iter(|| {
            for src in &blocks {
                let _ = black_box(parse_block_into_data(
                    src.as_str(),
                    0,
                    ParseOptions::default(),
                ));
            }
        });
    });
}

/// Phase 1 + 2 only: skips `writer.finish()` (header + section concat).
/// Combined with `parse_to_bytes (full)` lets us infer the finish cost as
/// `full - (parse + emit)`. We can't isolate `emit_block` alone because
/// `ParsedBlock<'a>` borrows from the source string, so it cannot be
/// stashed in `iter_batched`'s setup output without lifetime gymnastics.
fn bench_parse_plus_emit(c: &mut Criterion) {
    let blocks = load_fixture();
    c.bench_function("phase 1+2 — parse + emit (no finish, full file)", |b| {
        b.iter(|| {
            let arena = Allocator::default();
            for src in &blocks {
                let parsed =
                    parse_block_into_data(src.as_str(), 0, ParseOptions::default());
                let mut writer = BinaryWriter::new(&arena);
                let _ = writer.append_source_text(src.as_str());
                let _ = black_box(emit_block(&mut writer, &parsed));
                writer.push_root(1, 0, 0);
                // Drop `writer` here without calling `finish()` so the
                // measurement excludes header + section concatenation.
                drop(writer);
            }
            black_box(arena);
        });
    });
}

criterion_group!(
    benches,
    bench_parse_to_bytes_full,
    bench_parse_batch_to_bytes,
    bench_parse_block_into_data,
    bench_parse_plus_emit,
);
criterion_main!(benches);
