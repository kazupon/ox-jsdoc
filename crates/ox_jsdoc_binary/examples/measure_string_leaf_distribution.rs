// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Measure the byte-length distribution of `TypeTag::String` payload nodes
//! (string-leaf nodes) in `typescript-checker.ts`. Used to size the
//! `Path B-leaf` design where short strings get inlined into Node Data
//! `(offset: u22, length: u8)` and only long ones (>= 256 bytes) keep
//! using the String Offsets table.
//!
//! Usage: `cargo run --release --example measure_string_leaf_distribution`

use std::fs;
use std::path::PathBuf;

use ox_jsdoc_binary::format::header::{
    NODES_OFFSET_FIELD, NODE_COUNT_FIELD, STRING_DATA_OFFSET_FIELD, STRING_OFFSETS_OFFSET_FIELD,
};
use ox_jsdoc_binary::format::node_record::{
    NODE_DATA_OFFSET, NODE_RECORD_SIZE, PAYLOAD_MASK, TYPE_TAG_SHIFT, TypeTag,
};
use ox_jsdoc_binary::parser::{BatchItem, ParseOptions, parse_batch_to_bytes};

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

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn main() {
    // -- 1. parse the fixture into Binary AST bytes ----------------------
    let source = fs::read_to_string(fixture_path()).expect("read fixture");
    let blocks = extract_jsdoc_blocks(&source);
    let items: Vec<BatchItem<'_>> = blocks
        .iter()
        .map(|s| BatchItem {
            source_text: s.as_str(),
            base_offset: 0,
        })
        .collect();
    let result = parse_batch_to_bytes(&items, ParseOptions::default());
    let bytes = &result.binary_bytes[..];

    // -- 2. read Header ---------------------------------------------------
    let nodes_offset = read_u32_le(bytes, NODES_OFFSET_FIELD) as usize;
    let node_count = read_u32_le(bytes, NODE_COUNT_FIELD) as usize;
    let string_offsets_offset = read_u32_le(bytes, STRING_OFFSETS_OFFSET_FIELD) as usize;
    let string_data_offset = read_u32_le(bytes, STRING_DATA_OFFSET_FIELD) as usize;

    eprintln!(
        "Loaded {} JSDoc blocks → {} nodes ({} bytes total binary)",
        blocks.len(),
        node_count,
        bytes.len()
    );

    // -- 3. iterate nodes, collect lengths of TypeTag::String -------------
    let mut lengths: Vec<usize> = Vec::new();
    let mut by_kind: std::collections::BTreeMap<u8, Vec<usize>> =
        std::collections::BTreeMap::new();

    for i in 1..node_count {
        // skip sentinel node 0
        let node_offset = nodes_offset + i * NODE_RECORD_SIZE;
        let kind = bytes[node_offset]; // KIND_OFFSET = 0
        let nd = read_u32_le(bytes, node_offset + NODE_DATA_OFFSET);
        let tag = TypeTag::from_u32((nd >> TYPE_TAG_SHIFT) & 0b11).expect("valid tag");
        if tag != TypeTag::String {
            continue;
        }
        let payload = nd & PAYLOAD_MASK;
        // Skip None sentinel
        if payload == PAYLOAD_MASK {
            continue;
        }
        let so_off = string_offsets_offset + (payload as usize) * 8;
        let start = read_u32_le(bytes, so_off) as usize;
        let end = read_u32_le(bytes, so_off + 4) as usize;
        let len = end - start;
        let _ = string_data_offset; // (only need len)
        lengths.push(len);
        by_kind.entry(kind).or_default().push(len);
    }

    eprintln!("");
    eprintln!("=== String-leaf length distribution ===");
    eprintln!("");

    let total = lengths.len();
    eprintln!("Total string-leaf nodes: {}", total);
    if total == 0 {
        return;
    }
    let mut sorted = lengths.clone();
    sorted.sort_unstable();
    let pct = |p: f64| -> usize {
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx]
    };
    eprintln!("min: {}, p25: {}, p50: {}, p75: {}, p90: {}, p95: {}, p99: {}, max: {}",
        sorted[0], pct(25.0), pct(50.0), pct(75.0), pct(90.0), pct(95.0), pct(99.0), sorted[sorted.len() - 1]);
    let total_bytes: usize = lengths.iter().sum();
    eprintln!(
        "avg: {:.1} byte, total: {} byte",
        total_bytes as f64 / total as f64,
        total_bytes
    );
    eprintln!("");

    eprintln!("=== Bucket histogram ===");
    let buckets: &[(&str, usize, usize)] = &[
        ("[0..16)", 0, 16),
        ("[16..32)", 16, 32),
        ("[32..64)", 32, 64),
        ("[64..128)", 64, 128),
        ("[128..256)", 128, 256),
        ("[256..512)", 256, 512),
        ("[512..1024)", 512, 1024),
        ("[1024..)", 1024, usize::MAX),
    ];
    for (label, lo, hi) in buckets {
        let count = lengths.iter().filter(|&&n| n >= *lo && n < *hi).count();
        let pct = (count as f64 / total as f64) * 100.0;
        let bar = "█".repeat((pct / 2.0) as usize);
        eprintln!("{:>14}: {:>5} ({:>5.1}%) {}", label, count, pct, bar);
    }

    eprintln!("");
    eprintln!("=== Inline-fit summary (Path B-leaf with length cap) ===");
    for &cap in &[64usize, 128, 256, 512] {
        let inline_count = lengths.iter().filter(|&&n| n < cap).count();
        let inline_pct = (inline_count as f64 / total as f64) * 100.0;
        eprintln!(
            "length < {:>4}: {:>5}/{} ({:>5.1}%) inline-fit",
            cap, inline_count, total, inline_pct
        );
    }

    eprintln!("");
    eprintln!("=== Top 10 string-leaf Kinds by count ===");
    let mut kinds: Vec<(u8, usize, &Vec<usize>)> = by_kind
        .iter()
        .map(|(k, v)| (*k, v.len(), v))
        .collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1));
    eprintln!("{:>6} | {:>6} | {:>6} | {:>6} | {:>6} | desc", "kind", "count", "p50", "p95", "max");
    for (k, c, lens) in kinds.iter().take(10) {
        let mut s = (*lens).clone();
        s.sort_unstable();
        let p50 = s[s.len() / 2];
        let p95 = s[(s.len() as f64 * 0.95) as usize];
        let max = s[s.len() - 1];
        let kind_name = match k {
            0x02 => "JsdocDescriptionLine",
            0x04 => "JsdocTagName",
            0x06 => "JsdocText",
            0x07 => "JsdocTypeSource",
            0x08 => "JsdocTypeLine",
            0x09 => "JsdocTagNameValue",
            0x0A => "JsdocNamepathSource",
            0x0B => "JsdocIdentifier",
            0x0C => "JsdocRawTagBody",
            0x80 => "TypeName",
            0x81 => "TypeNumber",
            0x82 => "TypeStringValue",
            0xA3 => "TypeProperty",
            0x8F => "TypeSpecialNamePath",
            _ => "?",
        };
        eprintln!("0x{:02X} | {:>6} | {:>6} | {:>6} | {:>6} | {}", k, c, p50, p95, max, kind_name);
    }
}
