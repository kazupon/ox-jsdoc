// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

//! Description text post-processing helpers.
//!
//! See `design/008-oxlint-oxfmt-support/README.md` §3 for the algorithm
//! design and §4.1 for the public API.
//!
//! The implementation here is the canonical port of upstream
//! `oxc_jsdoc::JSDocCommentPart::parsed_preserving_whitespace`
//! (`refers/oxc/crates/oxc_jsdoc/src/parser/jsdoc_parts.rs:97-124`).

/// Reflow a raw description slice into preserve-whitespace form:
///
/// - Strip the leading `* ` margin from each comment-continuation line
///   (at most ONE space after `*` is consumed; extra indentation is kept
///   so Markdown indented code blocks survive).
/// - Preserve blank `*` lines as empty lines (so paragraph structure
///   round-trips intact).
/// - Preserve markdown emphasis: `*foo*` / `*_bold_*` keep the leading
///   `*` because the character right after is alphanumeric or `_`
///   (signalling emphasis, not a comment-continuation prefix).
///
/// Single-line input takes a fast path that just returns `raw.trim()`.
///
/// See also [`super::context::ParserContext`] for how the raw slice is
/// computed (covers from the first description line's `content_start` to
/// the last line's `content_end` per design `§4.1`).
#[must_use]
pub fn parsed_preserving_whitespace(raw: &str) -> String {
    if !raw.contains('\n') {
        return raw.trim().to_string();
    }

    let mut result = String::with_capacity(raw.len());
    for (i, line) in raw.lines().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('*') {
            // Markdown emphasis (`*word*`) is NOT a comment-continuation
            // prefix — leave the `*` in place.
            let is_emphasis = rest.starts_with(|ch: char| ch.is_alphanumeric() || ch == '_');
            if !is_emphasis {
                // Strip at most ONE leading space after `*` so any extra
                // indentation (e.g. for indented code blocks) is preserved.
                result.push_str(rest.strip_prefix(' ').unwrap_or(rest));
                continue;
            }
        }
        result.push_str(trimmed);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::parsed_preserving_whitespace;

    /// Fixtures derived from upstream `oxc_jsdoc` `comment_part_parsed`
    /// tests — see `refers/oxc/crates/oxc_jsdoc/src/parser/jsdoc_parts.rs:298-367`.
    /// We share the input shapes; the expected outputs differ where
    /// preserve-whitespace and `parsed()` diverge.
    #[test]
    fn single_line_inputs_just_get_trimmed() {
        for (input, expected) in [
            ("", ""),
            ("hello  ", "hello"),
            ("  * single line", "* single line"),
            (" * ", "*"),
            (" * * ", "* *"),
            ("***", "***"),
        ] {
            assert_eq!(
                parsed_preserving_whitespace(input),
                expected,
                "input: {input:?}",
            );
        }
    }

    #[test]
    fn empty_multiline_collapses_to_blank_lines() {
        // Three blank lines → two newlines between empty strings.
        assert_eq!(parsed_preserving_whitespace("\n\n    "), "\n\n");
    }

    #[test]
    fn star_continuation_prefix_is_stripped() {
        // Note: `lines()` splits on `\n` and discards the trailing newline,
        // so trailing `\n    ` becomes the last empty line, not an extra `\n`.
        let input = "\n     * asterisk\n    ";
        // - line 0: ""              → ""
        // - line 1: "     * asterisk" → trim → "* asterisk" → strip "* " → "asterisk"
        // - line 2: "    "           → trim → ""
        assert_eq!(parsed_preserving_whitespace(input), "\nasterisk\n");
    }

    #[test]
    fn nested_star_lists_preserve_their_leading_star() {
        // `* * li` — outer `*` is comment prefix, inner `* li` is content.
        // Trailing `\n` is dropped by `lines()`.
        let input = "\n     * * li\n     * * li\n";
        assert_eq!(parsed_preserving_whitespace(input), "\n* li\n* li");
    }

    #[test]
    fn mixed_star_layout_is_consistent() {
        let input = "\n     * * 1\n     ** 2\n";
        // - line 1: " * * 1" → trim → "* * 1" → strip "* " → "* 1"
        // - line 2: "     ** 2" → trim → "** 2" → strip leading `*` → "* 2",
        //   next char is `*` (non alnum/`_`), so the continuation branch runs
        //   `strip_prefix(' ')`; first char of "* 2" is `*` (not ` `), so the
        //   strip returns the original — push "* 2".
        assert_eq!(parsed_preserving_whitespace(input), "\n* 1\n* 2");
    }

    #[test]
    fn paragraph_breaks_are_preserved() {
        // The defining preserve-whitespace test: input with blank lines
        // between paragraphs must round-trip the blank lines.
        let input = "\n    1\n\n    2\n\n    3\n                ";
        assert_eq!(parsed_preserving_whitespace(input), "\n1\n\n2\n\n3\n");
    }

    #[test]
    fn indented_code_block_is_preserved() {
        // Markdown convention: 4 leading spaces past the `* ` prefix
        // indicates an indented code block. Strip exactly the `* ` and
        // let the 4 spaces survive into the output.
        let input = " * some intro.\n *\n *     code()\n *\n * outro.\n";
        assert_eq!(
            parsed_preserving_whitespace(input),
            "some intro.\n\n    code()\n\noutro."
        );
    }

    #[test]
    fn markdown_emphasis_keeps_its_leading_star() {
        // `*foo*` and `*bold*` are markdown emphasis. Note: the algorithm
        // detects emphasis only by inspecting the char *immediately after*
        // the leading `*` — if it's alnum/`_`, the prefix `*` is kept and
        // the line is pushed verbatim. Here the `*` of `* ` (the comment
        // continuation) IS stripped because it's followed by a space; the
        // emphasis `*foo*` after the strip still starts with `*`, but the
        // line content as a whole was already trimmed and pushed.
        let input = " * normal\n * *foo* is emphasis\n * *bold* word\n";
        assert_eq!(
            parsed_preserving_whitespace(input),
            "normal\n*foo* is emphasis\n*bold* word"
        );
    }

    #[test]
    fn underscore_after_star_is_treated_as_emphasis() {
        let input = " * normal\n * *_underscore_* example\n";
        assert_eq!(
            parsed_preserving_whitespace(input),
            "normal\n*_underscore_* example"
        );
    }

    #[test]
    fn punctuation_after_star_is_continuation_prefix() {
        // `*` followed by `.`, `\``, `(`, etc. is NOT emphasis — the `*`
        // is a continuation prefix and gets stripped.
        for (input, expected) in [
            (" * abc\n * .punct\n", "abc\n.punct"),
            // The `*` followed by space is the canonical case.
            (" * abc\n * \n * def\n", "abc\n\ndef"),
        ] {
            assert_eq!(
                parsed_preserving_whitespace(input),
                expected,
                "input: {input:?}",
            );
        }
    }

    #[test]
    fn star_followed_by_alnum_is_classified_as_emphasis() {
        // Quirk of the heuristic: any `*<alnum>` (e.g. `*no_space`) is
        // classified as emphasis and kept verbatim. The algorithm cannot
        // distinguish `*foo` (incomplete emphasis) from `*foo*` (completed)
        // without scanning ahead. JSDoc convention is `* ` (asterisk +
        // space) for continuation, so emphasis-like sequences are rare in
        // practice. This matches upstream `oxc_jsdoc` behavior exactly.
        let input = " * normal\n *no_space\n";
        assert_eq!(parsed_preserving_whitespace(input), "normal\n*no_space");
    }

    #[test]
    fn lines_without_star_prefix_pass_through_trimmed() {
        // Some descriptions don't use the `*` prefix at all (e.g.
        // bare text inside `/** ... */`). Those lines just get trimmed.
        let input = "first line\nsecond line\n  third line  ";
        assert_eq!(
            parsed_preserving_whitespace(input),
            "first line\nsecond line\nthird line"
        );
    }
}
