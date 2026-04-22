//! Tight loop runner for `samply` to profile `parse_block_into_data`
//! plus manual sub-phase timing (scanner-only, full-parse) to localize
//! which part of the parse pipeline dominates.
//!
//! Usage:
//!   cargo run --release --example profile_parse_block
//!   ITERS=20000 samply record --save-only -o /tmp/p.json.gz \
//!     ./target/release/examples/profile_parse_block

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use ox_jsdoc_binary::parser::{
    context::parse_block_into_data, scanner::logical_lines, ParseOptions,
};
use oxc_allocator::Allocator;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures/perf/source/typescript-checker.ts")
}

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

fn main() {
    let source = fs::read_to_string(fixture_path()).expect("read fixture");
    let blocks = extract_jsdoc_blocks(&source);

    eprintln!("Loaded {} JSDoc blocks", blocks.len());

    let iters = std::env::var("ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000usize);

    // ---- Sub-phase 1: scanner::logical_lines only (no parser/lexer) ----
    let mut acc_scanner = 0usize;
    let start = Instant::now();
    for _ in 0..iters {
        for src in &blocks {
            let result = logical_lines(src.as_str(), 0);
            acc_scanner = acc_scanner.wrapping_add(result.lines.len());
        }
    }
    let t_scanner = start.elapsed();

    // ---- Sub-phase 2: full parse_block_into_data ----
    let mut acc_full = 0usize;
    let start = Instant::now();
    for _ in 0..iters {
        let arena = Allocator::default();
        for src in &blocks {
            let parsed =
                parse_block_into_data(&arena, src.as_str(), 0, ParseOptions::default());
            acc_full = acc_full.wrapping_add(parsed.diagnostics().len());
            if parsed.is_failure() {
                acc_full = acc_full.wrapping_add(1);
            }
        }
        std::mem::drop(arena);
    }
    let t_full = start.elapsed();

    let per_iter_scanner = t_scanner / iters as u32;
    let per_iter_full = t_full / iters as u32;
    let per_iter_rest = per_iter_full
        .checked_sub(per_iter_scanner)
        .unwrap_or_default();

    eprintln!();
    eprintln!("=== Sub-phase breakdown over {} iters ===", iters);
    eprintln!(
        "  scanner only      : {:>10?}/iter  ({:>5.1}%)",
        per_iter_scanner,
        100.0 * per_iter_scanner.as_nanos() as f64 / per_iter_full.as_nanos() as f64
    );
    eprintln!(
        "  full parse        : {:>10?}/iter  (100.0%)",
        per_iter_full
    );
    eprintln!(
        "  rest (lexer+ctx+type): {:>10?}/iter  ({:>5.1}%)",
        per_iter_rest,
        100.0 * per_iter_rest.as_nanos() as f64 / per_iter_full.as_nanos() as f64
    );

    // Anti-DCE
    std::process::exit(((acc_scanner ^ acc_full) & 0xff) as i32 ^ 0);
}
