// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! The source texts of a batch sit together in String Data, in input order,
//! after the common strings (`design/007-binary-ast/format.md`, "String
//! Data section"). The strings that are not slices of a source come after
//! them, so a decoder can tell where each source text ends: where the next
//! one starts, and the last one at the first one's start plus the total
//! length.

use ox_jsdoc::decoder::source_file::LazySourceFile;
use ox_jsdoc::parser::{BatchItem, ParseOptions, parse_batch, parse_batch_to_bytes};
use oxc_allocator::Allocator;

/// Tags with types and names (strings that are not slices of the source),
/// a failed item (its diagnostic message), and non-ASCII text.
const SOURCES: [&str; 5] = [
    "/** @param {string} alpha The first. */",
    "not a comment",
    "/** 日本語の説明 */",
    "/** @returns {number} beta */",
    "/** c */",
];

fn options() -> [ParseOptions; 3] {
    [
        ParseOptions::default(),
        ParseOptions {
            compat_mode: true,
            preserve_whitespace: true,
            ..ParseOptions::default()
        },
        ParseOptions {
            parse_types: true,
            ..ParseOptions::default()
        },
    ]
}

fn items() -> Vec<BatchItem<'static>> {
    SOURCES
        .iter()
        .enumerate()
        .map(|(index, source_text)| BatchItem {
            source_text,
            base_offset: index as u32 * 100,
        })
        .collect()
}

#[track_caller]
fn assert_sources_together(file: &LazySourceFile<'_>) {
    let mut offset = file.get_root_source_offset_in_data(0);
    for (index, source) in SOURCES.iter().enumerate() {
        let root = index as u32;
        assert_eq!(
            file.get_root_source_offset_in_data(root),
            offset,
            "the start of root {root}"
        );
        assert_eq!(
            file.slice_source_text(root, 0, source.len() as u32),
            Some(*source),
            "the source text of root {root}"
        );
        offset += source.len() as u32;
    }
    let total: u32 = SOURCES.iter().map(|source| source.len() as u32).sum();
    assert_eq!(
        file.slice_source_text(0, 0, total),
        Some(SOURCES.concat().as_str()),
        "all the source texts, one after the other"
    );
}

#[test]
fn keeps_the_source_texts_together_in_parse_batch_to_bytes() {
    for options in options() {
        let result = parse_batch_to_bytes(&items(), options);
        let file = LazySourceFile::new(&result.binary_bytes).unwrap();
        assert_sources_together(&file);
    }
}

#[test]
fn keeps_the_source_texts_together_in_parse_batch() {
    for options in options() {
        let arena = Allocator::default();
        let result = parse_batch(&arena, &items(), options);
        assert_sources_together(&result.source_file);
    }
}
