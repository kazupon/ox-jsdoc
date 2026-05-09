// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Tight loop runner for `samply` to profile the public `parse()` entry point.
//!
//! Mirrors `profile_parse_batch.rs` but exercises the per-comment, arena-backed
//! `parse()` path so we can localize what differentiates `parse()` from
//! `parse_batch_to_bytes()` / `parse_to_bytes()`.
//!
//! Usage:
//!   cargo run --release --example profile_parse
//!   ITERS=20000 samply record --save-only -o /tmp/profile-out/profile-parse.json \
//!     --unstable-presymbolicate ./target/release/examples/profile_parse

use std::fs;
use std::path::PathBuf;

use ox_jsdoc_binary::parser::{ParseOptions, parse};
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
    eprintln!("Running parse() in a tight loop for samply…");

    let iters = std::env::var("ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000usize);

    let start = std::time::Instant::now();
    let mut sink = 0usize;
    for _ in 0..iters {
        // Mirror the canonical "one arena per file pass" usage pattern used in
        // production wrappers — `parse()` is invoked per-comment but each
        // returned `ParseResult` borrows from the same shared arena until the
        // arena is dropped at the end of the iteration.
        let arena = Allocator::default();
        for src in &blocks {
            let r = parse(&arena, src.as_str(), ParseOptions::default());
            sink = sink.wrapping_add(r.binary_bytes.len());
            sink = sink.wrapping_add(r.diagnostics.len());
        }
        std::mem::drop(arena);
    }
    let elapsed = start.elapsed();
    eprintln!(
        "{} iters in {:?} → {:?}/iter (≈ {:?}/comment)",
        iters,
        elapsed,
        elapsed / iters as u32,
        elapsed / (iters as u32 * blocks.len() as u32),
    );
    // Anti-DCE
    std::process::exit((sink & 0xff) as i32 ^ 0);
}
