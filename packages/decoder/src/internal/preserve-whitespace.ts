/**
 * Description-text post-processing helpers.
 *
 * JS port of `crates/ox_jsdoc/src/parser/text.rs::parsed_preserving_whitespace`
 * — kept byte-for-byte equivalent so `RemoteJsdocBlock.descriptionText(true)`
 * (Binary AST decoder) and `JsdocBlock::description_text(true)` (typed AST
 * Rust) produce identical output for any given raw description slice.
 *
 * See `design/008-oxlint-oxfmt-support/README.md` §3 for the algorithm
 * design + §4.3 for the JS API contract this powers.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

const ALNUM_OR_UNDERSCORE = /^[\p{L}\p{N}_]/u

/**
 * Reflow a raw description slice into preserve-whitespace form:
 *
 * - Strip the leading `* ` margin from each comment-continuation line
 *   (at most ONE space after `*` is consumed; extra indentation is kept
 *   so Markdown indented code blocks survive).
 * - Preserve blank `*` lines as empty lines (paragraph structure).
 * - Preserve markdown emphasis: `*foo*` / `*_bold_*` keep the leading
 *   `*` because the character right after is alphanumeric or `_`.
 *
 * Single-line input takes a fast path that just returns `raw.trim()`.
 *
 */
export function parsedPreservingWhitespace(raw: string): string {
  if (!raw.includes('\n')) {
    return raw.trim()
  }

  // Mirror Rust's `str::lines()`: drop a single trailing newline so
  // `"a\nb\n"` splits to ["a", "b"] (not ["a", "b", ""]).
  const trimmedTrailing = raw.endsWith('\n') ? raw.slice(0, -1) : raw
  const lines = trimmedTrailing.split('\n')
  let result = ''
  for (let i = 0; i < lines.length; i++) {
    if (i > 0) {
      result += '\n'
    }
    const trimmed = lines[i].trim()
    if (trimmed.startsWith('*')) {
      const rest = trimmed.slice(1)
      // Markdown emphasis (`*word*`) is NOT a comment-continuation prefix —
      // leave the `*` in place.
      const isEmphasis = ALNUM_OR_UNDERSCORE.test(rest)
      if (!isEmphasis) {
        // Strip at most ONE leading space after `*` so any extra
        // indentation (e.g. for indented code blocks) is preserved.
        result += rest.startsWith(' ') ? rest.slice(1) : rest
        continue
      }
    }
    result += trimmed
  }
  return result
}
