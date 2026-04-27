// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//
// L4: description_raw + description_text(preserve_whitespace) integration
// tests — end-to-end coverage of the typed AST API and JSON serializer.
//
// See `design/008-oxlint-oxfmt-support/README.md` §4.1 (boundary
// definition) + §4.4 (JSON serializer behavior) + §7 (validation matrix).

use ox_jsdoc::ast::{JsdocBlock, JsdocTag};
use ox_jsdoc::parser::ParseOptions;
use ox_jsdoc::{
    SerializeOptions, parse_comment, parsed_preserving_whitespace,
    serialize_comment_json_with_options,
};
use oxc_allocator::{Allocator, Box as ArenaBox};

fn parse<'a>(arena: &'a Allocator, src: &'a str) -> ArenaBox<'a, JsdocBlock<'a>> {
    let output = parse_comment(arena, src, 0, ParseOptions::default());
    output.comment.expect("expected a parseable JsdocBlock")
}

fn first_tag<'a, 'b>(block: &'b JsdocBlock<'a>) -> &'b JsdocTag<'a> {
    block.tags.first().expect("expected at least one tag")
}

// ============================================================================
// JsdocBlock.description_raw — boundary fidelity
// ============================================================================

#[test]
fn block_description_raw_is_none_when_no_description() {
    let arena = Allocator::default();
    let block = parse(&arena, "/** @param x */");
    assert_eq!(block.description, None);
    assert_eq!(block.description_raw, None);
}

#[test]
fn block_description_raw_single_line() {
    let arena = Allocator::default();
    let block = parse(&arena, "/** Just one line */");
    let raw = block.description_raw.expect("description_raw expected");
    // Single-line: slice is exactly the description content **as it appears
    // in source**, including trailing whitespace before the closing `*/`.
    // The compact `description` field is trim_end'd ("Just one line"); the
    // raw slice is byte-exact per §4.1.
    assert_eq!(raw, "Just one line ");
    // The preserve algorithm trims each line, normalizing the output:
    assert_eq!(parsed_preserving_whitespace(raw), "Just one line");
}

#[test]
fn block_description_raw_multi_line_preserves_intermediate_margins() {
    let arena = Allocator::default();
    let src = "/**\n * First line.\n * Second line.\n */";
    let block = parse(&arena, src);
    // Compact view: blank-stripped joined text.
    assert_eq!(
        block.description.as_deref(),
        Some("First line.\nSecond line.")
    );
    // Raw slice spans across the intermediate `* ` margin verbatim.
    assert_eq!(block.description_raw, Some("First line.\n * Second line."));
}

#[test]
fn block_description_raw_includes_blank_line_margins() {
    let arena = Allocator::default();
    let src = "/**\n * First.\n *\n * Second.\n */";
    let block = parse(&arena, src);
    let raw = block.description_raw.expect("description_raw expected");
    assert_eq!(raw, "First.\n *\n * Second.");
    // Algorithm output preserves the paragraph break.
    assert_eq!(parsed_preserving_whitespace(raw), "First.\n\nSecond.");
}

#[test]
fn block_description_raw_indented_code_block_survives_algorithm() {
    let arena = Allocator::default();
    let src = "/**\n * Intro.\n *\n *     code()\n *\n * Outro.\n */";
    let block = parse(&arena, src);
    let preserved = block
        .description_text(true)
        .expect("preserve-whitespace text expected");
    assert_eq!(preserved.as_ref(), "Intro.\n\n    code()\n\nOutro.");
}

// ============================================================================
// JsdocTag.description_raw — boundary fidelity
// ============================================================================

#[test]
fn tag_description_raw_single_line() {
    let arena = Allocator::default();
    let block = parse(&arena, "/** @param {T} id A short description */");
    let tag = first_tag(&block);
    assert_eq!(tag.description.as_deref(), Some("A short description"));
    // Trailing space before `*/` is preserved in the raw slice.
    assert_eq!(tag.description_raw, Some("A short description "));
    assert_eq!(
        tag.description_text(true).as_deref(),
        Some("A short description")
    );
}

#[test]
fn tag_description_raw_none_when_no_description() {
    let arena = Allocator::default();
    let block = parse(&arena, "/** @param {T} id */");
    let tag = first_tag(&block);
    assert_eq!(tag.description, None);
    assert_eq!(tag.description_raw, None);
}

#[test]
fn tag_description_raw_multi_line_uses_correct_end_offset() {
    let arena = Allocator::default();
    // Regression test for the multi-line span bug: relative_span's END is
    // short by `n_line_breaks * margin_chars_lost` bytes for multi-line
    // bodies. Parser fixes this by using the last body_line.content_end.
    let src = "/**\n * @param {T} x first line of desc\n *   continuation here\n */";
    let block = parse(&arena, src);
    let tag = first_tag(&block);

    // Compact: joined non-blank lines.
    assert_eq!(
        tag.description.as_deref(),
        Some("first line of desc\n  continuation here")
    );

    // Raw: slice covers from "first" through the end of "continuation here",
    // INCLUDING the `\n *   ` margin between lines.
    let raw = tag.description_raw.expect("description_raw expected");
    assert!(
        raw.starts_with("first line of desc"),
        "raw should start with first description text: {raw:?}",
    );
    assert!(
        raw.ends_with("continuation here"),
        "raw should end at last description content end: {raw:?}",
    );
    assert!(
        raw.contains("\n *   "),
        "raw should preserve the inter-line margin: {raw:?}",
    );

    // Algorithm reflows it to the compact-with-indent shape.
    let preserved = tag
        .description_text(true)
        .expect("preserve-whitespace expected");
    assert_eq!(
        preserved.as_ref(),
        "first line of desc\n  continuation here"
    );
}

#[test]
fn tag_description_raw_with_blank_paragraph_break() {
    let arena = Allocator::default();
    let src = "/**\n * @param x first paragraph.\n *\n * second paragraph.\n */";
    let block = parse(&arena, src);
    let tag = first_tag(&block);
    let preserved = tag
        .description_text(true)
        .expect("preserve-whitespace expected");
    // Paragraph break must survive into the algorithm output.
    assert_eq!(preserved.as_ref(), "first paragraph.\n\nsecond paragraph.");
}

// ============================================================================
// description_text(preserve) public API
// ============================================================================

#[test]
fn description_text_compact_returns_borrowed_cow() {
    let arena = Allocator::default();
    let block = parse(&arena, "/** Hello world */");
    let cow = block
        .description_text(false)
        .expect("description_text expected");
    // Non-allocating path: Cow::Borrowed reuses block.description's slice.
    assert_eq!(cow.as_ref(), "Hello world");
    // Verify it's truly borrowed (zero-alloc path).
    assert!(matches!(cow, std::borrow::Cow::Borrowed(_)));
}

#[test]
fn description_text_preserve_returns_owned_cow() {
    let arena = Allocator::default();
    let src = "/**\n * First.\n *\n * Second.\n */";
    let block = parse(&arena, src);
    let cow = block
        .description_text(true)
        .expect("description_text expected");
    assert_eq!(cow.as_ref(), "First.\n\nSecond.");
    // Allocates a String — Cow::Owned.
    assert!(matches!(cow, std::borrow::Cow::Owned(_)));
}

#[test]
fn description_text_returns_none_when_no_description() {
    let arena = Allocator::default();
    let block = parse(&arena, "/** @param x */");
    assert!(block.description_text(false).is_none());
    assert!(block.description_text(true).is_none());
}

// ============================================================================
// JSON serializer (compat_mode-only descriptionRaw output)
// ============================================================================

#[test]
fn json_serializer_omits_description_raw_in_basic_mode() {
    let arena = Allocator::default();
    let src = "/**\n * Multi-line\n * description.\n */";
    let block = parse(&arena, src);
    let json =
        serialize_comment_json_with_options(&block, None, None, &SerializeOptions::default());
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    // Default (compat_mode = false) → descriptionRaw must NOT appear.
    assert!(
        value.get("descriptionRaw").is_none(),
        "descriptionRaw should be omitted in basic mode: {value:#?}",
    );
    // But the compact `description` field is unchanged.
    assert_eq!(
        value["description"].as_str(),
        Some("Multi-line\ndescription.")
    );
}

#[test]
fn json_serializer_emits_description_raw_in_compat_mode() {
    let arena = Allocator::default();
    let src = "/**\n * Multi-line\n * description.\n */";
    let block = parse(&arena, src);
    let opts = SerializeOptions {
        compat_mode: true,
        ..SerializeOptions::default()
    };
    let json = serialize_comment_json_with_options(&block, None, None, &opts);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        value["descriptionRaw"].as_str(),
        Some("Multi-line\n * description.")
    );
}

#[test]
fn json_serializer_descriptionRaw_on_jsdoctag_in_compat_mode() {
    let arena = Allocator::default();
    let src = "/**\n * @param {T} x first\n *   continuation\n */";
    let block = parse(&arena, src);
    let opts = SerializeOptions {
        compat_mode: true,
        ..SerializeOptions::default()
    };
    let json = serialize_comment_json_with_options(&block, None, None, &opts);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let tags = value["tags"].as_array().expect("tags array");
    assert_eq!(tags.len(), 1);
    let raw = tags[0]["descriptionRaw"]
        .as_str()
        .expect("tag descriptionRaw expected");
    assert!(raw.starts_with("first"));
    assert!(raw.ends_with("continuation"));
}

#[test]
fn json_serializer_omits_description_raw_when_block_has_no_description() {
    let arena = Allocator::default();
    let block = parse(&arena, "/** @param x */");
    let opts = SerializeOptions {
        compat_mode: true,
        ..SerializeOptions::default()
    };
    let json = serialize_comment_json_with_options(&block, None, None, &opts);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    // Even in compat mode, no descriptionRaw key when there's no description.
    assert!(
        value.get("descriptionRaw").is_none(),
        "descriptionRaw should be omitted when block has no description",
    );
}
