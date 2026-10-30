// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! The name of an inline tag (`linkcode` in `{@linkcode Foo}`) is kept in the
//! Binary AST, and the lazy decoder reads it.

use ox_jsdoc::decoder::source_file::LazySourceFile;
use ox_jsdoc::parser::{ParseOptions, parse};
use oxc_allocator::Allocator;

/// The names of the inline tags in the block description and in the
/// description of each tag of `source`, in order.
fn names(source: &str, options: ParseOptions) -> Vec<Option<String>> {
    let arena = Allocator::default();
    let result = parse(&arena, source, options);
    let block = result.lazy_root.expect("parsed");
    let mut out: Vec<_> = block
        .inline_tags()
        .map(|inline| inline.tag().map(str::to_owned))
        .collect();
    for tag in block.tags() {
        out.extend(
            tag.inline_tags()
                .map(|inline| inline.tag().map(str::to_owned)),
        );
    }
    out
}

fn some(names: &[&str]) -> Vec<Option<String>> {
    names.iter().map(|name| Some((*name).to_owned())).collect()
}

#[test]
fn keeps_the_names_of_inline_tags() {
    for (source, expected) in [
        ("/** See {@link Foo} */", vec!["link"]),
        ("/** See {@linkcode Foo} */", vec!["linkcode"]),
        ("/** See {@linkplain Foo|the text} */", vec!["linkplain"]),
        ("/** See {@tutorial getting-started} */", vec!["tutorial"]),
        ("/** {@inheritDoc} */", vec!["inheritDoc"]),
        ("/** See {@link Foo the text} */", vec!["link"]),
        (
            "/** See {@link Foo} and {@linkcode Bar} */",
            vec!["link", "linkcode"],
        ),
        // In a tag description
        ("/** @param a See {@linkplain Foo} */", vec!["linkplain"]),
        // Both, across lines
        (
            "/**\n * See {@link Foo}\n * and {@linkcode\n * Bar}.\n * @param {number} a {@tutorial x}\n */",
            vec!["link", "linkcode", "tutorial"],
        ),
    ] {
        assert_eq!(
            names(source, ParseOptions::default()),
            some(&expected),
            "{source:?}"
        );
        let compat = ParseOptions {
            compat_mode: true,
            ..ParseOptions::default()
        };
        assert_eq!(
            names(source, compat),
            some(&expected),
            "{source:?} (compat)"
        );
    }
}

#[test]
fn gives_no_name_for_a_buffer_of_minor_version_0() {
    // Minor 0 buffers have no name field after the other fields, so the
    // decoder must not read it.
    let arena = Allocator::default();
    let result = parse(&arena, "/** See {@link Foo} */", ParseOptions::default());
    let mut bytes = result.binary_bytes.to_vec();
    bytes[0] = 0x10;
    let source_file = LazySourceFile::new(&bytes).expect("header");
    let block = source_file.asts().next().flatten().expect("parsed");
    let inline = block.inline_tags().next().expect("inline tag");
    assert_eq!(inline.tag(), None);
    assert_eq!(inline.namepath_or_url(), Some("Foo"));
}
