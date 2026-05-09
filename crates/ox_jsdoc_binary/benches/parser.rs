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

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use ox_jsdoc_binary::parser::{
    BatchItem, ParseOptions,
    context::{emit_block, parse_block_into_data},
    parse, parse_batch, parse_batch_into, parse_batch_to_bytes, parse_into, parse_to_bytes,
};
use ox_jsdoc_binary::writer::BinaryWriter;
use oxc_allocator::Allocator;
use oxc_jsdoc::JSDoc;
use oxc_span::Span;

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

/// Arena-backed `parse()` loop (single shared arena) — the entry point that
/// returns a [`ParseResult`] with the lazy decoder root attached. Mirrors
/// the canonical Rust-side usage where one arena is reused across the file
/// and dropped at the end. Differs from `parse_to_bytes` in that it
/// preserves the `&'arena [u8]` view + lazy `LazyJsdocBlock` handle instead
/// of returning an owned `Vec<u8>`.
fn bench_parse_full(c: &mut Criterion) {
    let blocks = load_fixture();
    c.bench_function("parse (loop, full file, shared arena)", |b| {
        b.iter(|| {
            let arena = Allocator::default();
            for src in &blocks {
                let _ = black_box(parse(&arena, src.as_str(), ParseOptions::default()));
            }
            black_box(arena);
        });
    });
}

/// Same per-comment shape as `bench_parse_full` but routes through
/// `parse_into` with a single recycled `BinaryWriter` shared across all
/// 232 calls. Demonstrates the writer-reuse path's amortization of
/// `BinaryWriter::new` (`StringTableBuilder::new` prelude memcpy + the
/// per-buffer `ArenaVec::new_in`s) — the per-call overhead that dominates
/// `parse()`'s per-comment self time.
fn bench_parse_into_recycled(c: &mut Criterion) {
    let blocks = load_fixture();
    c.bench_function("parse_into (loop, full file, shared writer)", |b| {
        b.iter(|| {
            let arena = Allocator::default();
            let mut writer = BinaryWriter::new(&arena);
            for src in &blocks {
                let _ = black_box(parse_into(
                    &arena,
                    src.as_str(),
                    ParseOptions::default(),
                    &mut writer,
                ));
            }
            black_box(arena);
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

/// `parse_batch` — typed-result single-batch entry point. Same shape as
/// `parse_batch_to_bytes` but returns `BatchResult<'arena>` with lazy
/// roots + an arena-backed `LazySourceFile`, the input pattern Rust
/// callers use when they need to walk the parsed AST.
fn bench_parse_batch_full(c: &mut Criterion) {
    let blocks = load_fixture();
    let items: Vec<BatchItem<'_>> = blocks
        .iter()
        .map(|s| BatchItem {
            source_text: s.as_str(),
            base_offset: 0,
        })
        .collect();
    c.bench_function("parse_batch (single batch, full file, shared arena)", |b| {
        b.iter(|| {
            let arena = Allocator::default();
            let _ = black_box(parse_batch(&arena, &items, ParseOptions::default()));
            black_box(arena);
        });
    });
}

/// `oxc_jsdoc::JSDoc::new(...).tags()` per-comment loop — the Rust-direct
/// reference parser used by oxlint's native JSDoc plugin. Mirrors the
/// `tasks/benchmark/benches/oxc_jsdoc.rs` setup: strip `/**` / `*/`, build
/// `JSDoc::new(inner, span)`, materialize `tags()` to trigger the lazy
/// parse, accumulate `len()` for anti-DCE.
///
/// Lives next to the ox-jsdoc-binary entry-point benches so the per-comment
/// numbers are taken under the same machine state and one cargo bench run.
fn bench_oxc_jsdoc_loop(c: &mut Criterion) {
    let blocks = load_fixture();
    c.bench_function("oxc_jsdoc::JSDoc::new(...).tags() (loop, full file)", |b| {
        b.iter(|| {
            let mut tag_count = 0usize;
            for source_text in &blocks {
                let inner = source_text
                    .strip_prefix("/**")
                    .and_then(|s| s.strip_suffix("*/"))
                    .unwrap_or(black_box(source_text.as_str()));
                let span = Span::new(0, source_text.len() as u32);
                let jsdoc = JSDoc::new(inner, span);
                tag_count += jsdoc.tags().len();
            }
            black_box(tag_count);
        });
    });
}

/// `parse_batch_into` — typed-result single-batch entry point reusing a
/// caller-supplied `BinaryWriter`. Hot-loop callers that drive many
/// batches in a row can amortize the per-call writer construction
/// (`StringTableBuilder::new` prelude memcpy + ArenaVec init) here.
fn bench_parse_batch_into_recycled(c: &mut Criterion) {
    let blocks = load_fixture();
    let items: Vec<BatchItem<'_>> = blocks
        .iter()
        .map(|s| BatchItem {
            source_text: s.as_str(),
            base_offset: 0,
        })
        .collect();
    c.bench_function(
        "parse_batch_into (single batch, full file, shared writer)",
        |b| {
            b.iter(|| {
                let arena = Allocator::default();
                let mut writer = BinaryWriter::new(&arena);
                let _ = black_box(parse_batch_into(
                    &arena,
                    &items,
                    ParseOptions::default(),
                    &mut writer,
                ));
                black_box(arena);
            });
        },
    );
}

/// Phase 1 only: structural parse (no binary emission).
fn bench_parse_block_into_data(c: &mut Criterion) {
    let blocks = load_fixture();
    c.bench_function("phase 1 — parse_block_into_data only (full file)", |b| {
        b.iter(|| {
            let arena = Allocator::default();
            for src in &blocks {
                let _ = black_box(parse_block_into_data(
                    &arena,
                    src.as_str(),
                    0,
                    ParseOptions::default(),
                ));
            }
            black_box(arena);
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
                    parse_block_into_data(&arena, src.as_str(), 0, ParseOptions::default());
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

/// Batch-mode Phase 1 only: parses every block into `ParsedBlock` data,
/// no writer touched. Subtract this from `parse_batch_to_bytes` to bound
/// the emit + finish portion.
fn bench_batch_parse_only(c: &mut Criterion) {
    let blocks = load_fixture();
    c.bench_function(
        "batch phase 1 — parse_block_into_data only (full file)",
        |b| {
            b.iter(|| {
                let arena = Allocator::default();
                for src in &blocks {
                    let _ = black_box(parse_block_into_data(
                        &arena,
                        src.as_str(),
                        0,
                        ParseOptions::default(),
                    ));
                }
                black_box(arena);
            });
        },
    );
}

/// Batch-mode Phase 1+2: same loop body as `parse_batch_to_bytes` minus
/// the final `writer.finish()`. Reuses one arena + writer across all
/// items, matching the batch path exactly except for the trailing concat.
fn bench_batch_parse_plus_emit(c: &mut Criterion) {
    let blocks = load_fixture();
    let items: Vec<BatchItem<'_>> = blocks
        .iter()
        .map(|s| BatchItem {
            source_text: s.as_str(),
            base_offset: 0,
        })
        .collect();
    c.bench_function(
        "batch phase 1+2 — parse + emit (no finish, full file)",
        |b| {
            b.iter(|| {
                let arena = Allocator::default();
                let mut writer = BinaryWriter::new(&arena);
                for (index, item) in items.iter().enumerate() {
                    let source_offset = writer.append_source_text(item.source_text);
                    let parsed =
                        parse_block_into_data(&arena, item.source_text, 0, ParseOptions::default());
                    let root_node_index = if parsed.is_failure() {
                        0
                    } else {
                        emit_block(&mut writer, &parsed).unwrap_or(0)
                    };
                    writer.push_root(root_node_index, source_offset, item.base_offset);
                    for diag in parsed.diagnostics() {
                        writer.push_diagnostic(index as u32, diag.message());
                    }
                }
                // Drop without finish() to exclude the final concat cost.
                drop(writer);
                black_box(arena);
            });
        },
    );
}

criterion_group!(
    benches,
    bench_parse_to_bytes_full,
    bench_parse_full,
    bench_parse_into_recycled,
    bench_parse_batch_to_bytes,
    bench_parse_batch_full,
    bench_parse_batch_into_recycled,
    bench_oxc_jsdoc_loop,
    bench_parse_block_into_data,
    bench_parse_plus_emit,
    bench_batch_parse_only,
    bench_batch_parse_plus_emit,
);
criterion_main!(benches);
