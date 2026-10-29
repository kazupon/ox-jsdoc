// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Comments with CRLF line ends parse like the same comments with LF line
//! ends: the same strings, and ranges one byte later for each line break
//! before them.

use ox_jsdoc::decoder::nodes::LazyNode;
use ox_jsdoc::decoder::nodes::comment_ast::{LazyJsdocBlock, LazyJsdocTag};
use ox_jsdoc::parser::{ParseOptions, parse};
use oxc_allocator::Allocator;

fn block<'a>(arena: &'a Allocator, source: &'a str, options: ParseOptions) -> LazyJsdocBlock<'a> {
    let result = parse(arena, source, options);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.lazy_root.expect("root")
}

/// The range in the CRLF source of `range` in the LF source `lf`: one `\r`
/// more for each line break before each end.
fn crlf_range(lf: &str, [start, end]: [u32; 2]) -> [u32; 2] {
    let shift = |offset: u32| offset + lf[..offset as usize].matches('\n').count() as u32;
    [shift(start), shift(end)]
}

/// The strings and ranges of a block that do not depend on its line ends.
#[derive(Debug, PartialEq)]
struct Shape<'a> {
    description: Option<&'a str>,
    description_lines: Vec<(&'a str, [u32; 2])>,
    tags: Vec<TagShape<'a>>,
    end_line: Option<u32>,
    description_raw: Option<String>,
}

#[derive(Debug, PartialEq)]
struct TagShape<'a> {
    range: [u32; 2],
    name: Option<&'a str>,
    raw_type: Option<&'a str>,
    description: Option<&'a str>,
    description_lines: Vec<(&'a str, [u32; 2])>,
}

/// The shape of `block`, with its ranges moved by `range`.
fn shape<'a>(block: LazyJsdocBlock<'a>, range: &dyn Fn([u32; 2]) -> [u32; 2]) -> Shape<'a> {
    let tag = |tag: LazyJsdocTag<'a>| TagShape {
        range: range(tag.range()),
        name: tag.name().map(|name| name.raw()),
        raw_type: tag.raw_type().map(|t| t.raw()),
        description: tag.description(),
        description_lines: tag
            .description_lines()
            .map(|line| (line.description(), range(line.range())))
            .collect(),
    };
    Shape {
        description: block.description(),
        description_lines: block
            .description_lines()
            .map(|line| (line.description(), range(line.range())))
            .collect(),
        tags: block.tags().map(tag).collect(),
        end_line: block.end_line(),
        // The raw description is a slice of the source, `\r` included.
        description_raw: block.description_raw().map(|raw| raw.replace('\r', "")),
    }
}

#[test]
fn block_description_with_crlf_line_ends() {
    let arena = Allocator::default();
    let source = "/**\r\n * text\r\n */";
    let block = block(&arena, source, ParseOptions::default());
    assert_eq!(block.description(), Some("text"));
    let start = source.find("text").unwrap() as u32;
    let lines: Vec<_> = block
        .description_lines()
        .map(|line| (line.description(), line.range()))
        .collect();
    assert_eq!(lines, [("text", [start, start + 4])]);
}

#[test]
fn crlf_line_ends_parse_like_lf_line_ends() {
    let arena = Allocator::default();
    let compat = ParseOptions {
        compat_mode: true,
        preserve_whitespace: true,
        ..ParseOptions::default()
    };
    for lf in [
        "/**\n * text\n */",
        "/**\n * text\n *\n * more\n * @param {string} a first\n *   second\n */",
        "/**\n * Summary {@link Foo}\n *\n * @returns {number} the value\n * @see Bar\n */",
        "/** one line\n * @param {{\n *   a: string\n * }} options desc\n */",
    ] {
        let crlf = lf.replace('\n', "\r\n");
        for options in [ParseOptions::default(), compat] {
            let expected = shape(block(&arena, lf, options), &|r| crlf_range(lf, r));
            let actual = shape(block(&arena, &crlf, options), &|r| r);
            assert_eq!(actual, expected, "{lf:?}, compat: {}", options.compat_mode);
        }
    }
}
