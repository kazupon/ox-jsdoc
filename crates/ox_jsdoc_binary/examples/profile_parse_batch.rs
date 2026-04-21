//! Tight loop runner for `samply` / `cargo flamegraph` to profile
//! `parse_batch_to_bytes` on the typescript-checker.ts fixture.
//!
//! Usage: `samply record cargo run --release --example profile_parse_batch`

use std::fs;
use std::path::PathBuf;

use ox_jsdoc_binary::parser::{parse_batch_to_bytes, BatchItem, ParseOptions};

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
    let items: Vec<BatchItem<'_>> = blocks
        .iter()
        .map(|s| BatchItem {
            source_text: s.as_str(),
            base_offset: 0,
        })
        .collect();

    eprintln!("Loaded {} JSDoc blocks", blocks.len());
    eprintln!("Running parse_batch_to_bytes in a tight loop for samply…");

    // Aim for ~10 seconds at ~450 µs per iteration ≈ 22000 iterations.
    let iters = std::env::var("ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(22_000usize);

    let start = std::time::Instant::now();
    let mut total_bytes = 0usize;
    for _ in 0..iters {
        let r = parse_batch_to_bytes(&items, ParseOptions::default());
        total_bytes = total_bytes.wrapping_add(r.binary_bytes.len());
    }
    let elapsed = start.elapsed();
    eprintln!(
        "{} iters in {:?} → {:?}/iter",
        iters,
        elapsed,
        elapsed / iters as u32
    );
    // Anti-DCE
    std::process::exit((total_bytes & 0xff) as i32 ^ 0);
}
