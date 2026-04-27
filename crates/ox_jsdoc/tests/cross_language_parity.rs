// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//
// L4: cross-language parity for `description_text(preserve_whitespace)` —
// ensures the Rust typed-AST API and the JS Binary AST decoder API stay
// byte-identical for the same source input.
//
// The fixture file `fixtures/cross-language/description-text.json` is the
// shared ground truth. This test asserts the Rust side. The matching JS
// side lives in `napi/ox-jsdoc-binary/test/description-text-parity.test.ts`
// and asserts against the **same JSON file**, so any divergence between
// the two implementations surfaces as a CI failure on one (or both) sides.
//
// See `design/008-oxlint-oxfmt-support/README.md` §7.3 for the validation
// strategy that motivates this test.

use std::fs;
use std::path::PathBuf;

use ox_jsdoc::ast::{JsdocBlock, JsdocTag};
use ox_jsdoc::parse_comment;
use ox_jsdoc::parser::ParseOptions;
use oxc_allocator::{Allocator, Box as ArenaBox};
use serde::Deserialize;

#[derive(Deserialize)]
struct Expected {
    compact: Option<String>,
    preserve: Option<String>,
}

#[derive(Deserialize)]
struct Fixture {
    name: String,
    source: String,
    block: Expected,
    tag: Option<Expected>,
}

#[derive(Deserialize)]
struct FixtureFile {
    fixtures: Vec<Fixture>,
}

fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures/cross-language/description-text.json")
}

fn parse<'a>(arena: &'a Allocator, src: &'a str) -> ArenaBox<'a, JsdocBlock<'a>> {
    parse_comment(arena, src, 0, ParseOptions::default())
        .comment
        .expect("expected a parseable JsdocBlock")
}

fn first_tag<'a, 'b>(block: &'b JsdocBlock<'a>) -> Option<&'b JsdocTag<'a>> {
    block.tags.first()
}

#[test]
fn rust_description_text_matches_shared_fixture_expected_outputs() {
    let path = fixtures_path();
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read fixture {path:?}: {e}"));
    let file: FixtureFile = serde_json::from_str(&raw).expect("parse fixture JSON");
    assert!(!file.fixtures.is_empty(), "fixture file has no entries");

    for fx in &file.fixtures {
        let arena = Allocator::default();
        let block = parse(&arena, &fx.source);

        let block_compact = block.description_text(false).map(|c| c.into_owned());
        let block_preserve = block.description_text(true).map(|c| c.into_owned());
        assert_eq!(
            block_compact, fx.block.compact,
            "fixture `{}`: block.description_text(false) mismatch",
            fx.name
        );
        assert_eq!(
            block_preserve, fx.block.preserve,
            "fixture `{}`: block.description_text(true) mismatch",
            fx.name
        );

        if let Some(expected_tag) = fx.tag.as_ref() {
            let tag = first_tag(&block)
                .unwrap_or_else(|| panic!("fixture `{}`: expected at least one tag", fx.name));
            let tag_compact = tag.description_text(false).map(|c| c.into_owned());
            let tag_preserve = tag.description_text(true).map(|c| c.into_owned());
            assert_eq!(
                tag_compact, expected_tag.compact,
                "fixture `{}`: tag.description_text(false) mismatch",
                fx.name
            );
            assert_eq!(
                tag_preserve, expected_tag.preserve,
                "fixture `{}`: tag.description_text(true) mismatch",
                fx.name
            );
        }
    }
}
