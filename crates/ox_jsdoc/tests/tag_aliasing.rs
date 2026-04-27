// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//
// L4: Tag aliasing — verify the parser is tag-name-agnostic so consumers
// (eslint-plugin-jsdoc / oxlint plugin-jsdoc / Biome / etc.) can implement
// `tagNamePreference`-style alias resolution at the rule layer.
//
// See `design/008-oxlint-oxfmt-support/README.md` §4.5.

use ox_jsdoc::ast::{JsdocBlock, JsdocTag};
use ox_jsdoc::parse_comment;
use ox_jsdoc::parser::ParseOptions;
use oxc_allocator::{Allocator, Box as ArenaBox};

// ============================================================================
// Helpers
// ============================================================================

/// Parse a single-tag JSDoc comment and return the parsed `JsdocTag`.
///
/// `comment_src` must be the full `/** … */` text with exactly one tag.
fn parse_single_tag<'a>(
    arena: &'a Allocator,
    comment_src: &'a str,
) -> ArenaBox<'a, JsdocBlock<'a>> {
    let output = parse_comment(arena, comment_src, 0, ParseOptions::default());
    let block = output.comment.expect("expected a parseable JsdocBlock");
    assert!(
        output.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        output.diagnostics
    );
    assert_eq!(block.tags.len(), 1, "expected exactly 1 tag");
    block
}

/// Snapshot the structural fields of a `JsdocTag` that should be invariant
/// across tag-name choices.
fn shape_of<'a>(tag: &'a JsdocTag<'a>) -> Shape<'a> {
    Shape {
        raw_type: tag.raw_type.as_ref().map(|t| t.raw),
        name: tag.name.as_ref().map(|n| n.raw),
        optional: tag.optional,
        default_value: tag.default_value,
        description: tag.description,
        type_lines_count: tag.type_lines.len(),
        description_lines_count: tag.description_lines.len(),
        inline_tags_count: tag.inline_tags.len(),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Shape<'a> {
    raw_type: Option<&'a str>,
    name: Option<&'a str>,
    optional: bool,
    default_value: Option<&'a str>,
    description: Option<&'a str>,
    type_lines_count: usize,
    description_lines_count: usize,
    inline_tags_count: usize,
}

// ============================================================================
// Tests
// ============================================================================

/// `@param`, `@arg`, `@argument` — the canonical eslint-plugin-jsdoc default
/// alias set — should all parse to the same shape, differing only in the
/// raw `tag.value` string.
#[test]
fn aliases_for_param_share_shape() {
    let arena = Allocator::default();

    let canonical = parse_single_tag(&arena, "/** @param {string} id The user id */");
    let arg_alias = parse_single_tag(&arena, "/** @arg {string} id The user id */");
    let argument_alias = parse_single_tag(&arena, "/** @argument {string} id The user id */");

    let canonical_tag = &canonical.tags[0];
    let arg_tag = &arg_alias.tags[0];
    let argument_tag = &argument_alias.tags[0];

    // Tag names differ — that's the whole point of aliasing.
    assert_eq!(canonical_tag.tag.value, "param");
    assert_eq!(arg_tag.tag.value, "arg");
    assert_eq!(argument_tag.tag.value, "argument");

    // But every other structural field is identical.
    let canonical_shape = shape_of(canonical_tag);
    assert_eq!(shape_of(arg_tag), canonical_shape);
    assert_eq!(shape_of(argument_tag), canonical_shape);
}

/// `@returns` and `@return` — same alias relationship, simpler shape (no name).
#[test]
fn aliases_for_returns_share_shape() {
    let arena = Allocator::default();

    let canonical = parse_single_tag(&arena, "/** @returns {boolean} True on success */");
    let alias = parse_single_tag(&arena, "/** @return {boolean} True on success */");

    assert_eq!(canonical.tags[0].tag.value, "returns");
    assert_eq!(alias.tags[0].tag.value, "return");
    assert_eq!(shape_of(&canonical.tags[0]), shape_of(&alias.tags[0]));
}

/// Project-defined custom tags (no eslint-plugin-jsdoc default alias for
/// them) parse with the same shape as a known tag.
#[test]
fn custom_tag_parses_like_a_known_tag() {
    let arena = Allocator::default();

    let known = parse_single_tag(&arena, "/** @param {T} foo bar */");
    let custom = parse_single_tag(&arena, "/** @whatever {T} foo bar */");

    assert_eq!(known.tags[0].tag.value, "param");
    assert_eq!(custom.tags[0].tag.value, "whatever");
    assert_eq!(shape_of(&known.tags[0]), shape_of(&custom.tags[0]));
}

/// kebab-case tag names are parsed without special handling — the parser
/// captures `tag.value` verbatim and lets the consumer decide what to do.
#[test]
fn kebab_case_tag_name_is_preserved_verbatim() {
    let arena = Allocator::default();

    let block = parse_single_tag(&arena, "/** @kebab-case {T} x */");
    assert_eq!(block.tags[0].tag.value, "kebab-case");

    // Same parse shape as a regular `@param`.
    let reference = parse_single_tag(&arena, "/** @param {T} x */");
    assert_eq!(shape_of(&block.tags[0]), shape_of(&reference.tags[0]));
}

/// Tag names with the supported set of non-letter characters (numbers,
/// underscore, hyphen, exclamation) survive intact. The ox-jsdoc tag-name
/// scanner accepts `[a-zA-Z0-9_-!]` (same set as upstream `oxc_jsdoc`);
/// other punctuation (e.g. `.`) terminates the name — this test fixes
/// that contract.
#[test]
fn punctuation_in_tag_name_is_preserved() {
    let arena = Allocator::default();

    for (source, expected_value) in [
        ("/** @since1 hello */", "since1"),         // trailing digit
        ("/** @x_under hello */", "x_under"),       // underscore
        ("/** @kebab-case hello */", "kebab-case"), // hyphen
        ("/** @ts! hello */", "ts!"),               // exclamation
    ] {
        let local_arena = Allocator::default();
        let block = parse_single_tag(&local_arena, source);
        assert_eq!(
            block.tags[0].tag.value, expected_value,
            "expected {expected_value:?} for {source:?}",
        );
    }

    // Reference: a known tag of the same body shape — confirm a custom
    // punctuated name produces the same structural fields.
    let reference = parse_single_tag(&arena, "/** @param hello */");
    let custom = parse_single_tag(&arena, "/** @ts! hello */");
    assert_eq!(shape_of(&custom.tags[0]), shape_of(&reference.tags[0]));

    // Period terminates the tag name (consumer must format as "@v1" if
    // they want a versioned tag).
    let dotted = parse_single_tag(&arena, "/** @v1.2.3 hello */");
    assert_eq!(
        dotted.tags[0].tag.value, "v1",
        "`.` is not part of the tag name; everything after is description text",
    );
}

/// The optional / defaultValue body parsing also works regardless of tag
/// name — the consumer can alias `@x` to mean `@param` and still get
/// `optional: true` etc.
#[test]
fn optional_default_value_parsing_is_tag_name_agnostic() {
    let arena = Allocator::default();

    let canonical = parse_single_tag(&arena, "/** @param {T} [name=fallback] desc */");
    let aliased = parse_single_tag(&arena, "/** @whatever {T} [name=fallback] desc */");

    let cn = &canonical.tags[0];
    let al = &aliased.tags[0];

    // Tag names differ.
    assert_eq!(cn.tag.value, "param");
    assert_eq!(al.tag.value, "whatever");

    // Both must parse the bracketed name + default value. `name.raw`
    // holds the parsed name (brackets stripped), while `optional` /
    // `default_value` capture the surrounding metadata.
    assert!(cn.optional, "canonical @param must be optional");
    assert_eq!(cn.default_value, Some("fallback"));
    assert_eq!(cn.name.as_ref().map(|n| n.raw), Some("name"));
    assert_eq!(shape_of(cn), shape_of(al));
}

/// Mixed: a single comment with both a known tag and a custom tag should
/// produce two structurally-equivalent tags differing only in name.
#[test]
fn known_and_custom_tag_in_same_comment_have_matching_shapes() {
    let arena = Allocator::default();
    let source = "/**\n * @param {T} x\n * @whatever {T} x\n */";

    let output = parse_comment(&arena, source, 0, ParseOptions::default());
    let block = output.comment.expect("expected a JsdocBlock");
    assert_eq!(block.tags.len(), 2);

    let known = &block.tags[0];
    let custom = &block.tags[1];

    assert_eq!(known.tag.value, "param");
    assert_eq!(custom.tag.value, "whatever");
    assert_eq!(shape_of(known), shape_of(custom));
}
