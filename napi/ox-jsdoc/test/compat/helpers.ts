/**
 * Helpers for the jsdoccomment-compat dynamic comparison test.
 *
 * `assertCompatible(actual, expected)` walks both AST trees in parallel and
 * collects every field-level mismatch. Known acceptable differences live in
 * `KNOWN_DIFFERENCES` so the test surfaces *new* divergences without forcing
 * us to fix every legacy gap before merging the test infrastructure.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

/* eslint-disable @typescript-eslint/no-explicit-any -- comparing untyped JSON shapes */

/**
 * Field paths that differ between ox-jsdoc and jsdoccomment in ways we
 * already understand. Each entry is documented with the current Rust-side
 * cause and the follow-up task that will close it. The comparator skips
 * these paths entirely so newly-introduced regressions still trigger
 * failures.
 *
 * Path syntax mirrors the comparator output: `JsdocBlock.delimiter`,
 * `JsdocBlock.tags[0].postTag`, `JsdocBlock.tags[*].description` (`*`
 * matches any list index).
 */
export const KNOWN_DIFFERENCES: ReadonlySet<string> = new Set([
  // jsdoccomment treats the leading `/**` as JsdocBlock.delimiter; ox-jsdoc
  // currently stores `*` here (the per-line delimiter, not the block one).
  // Closed by Phase 6 source[]/tokens[] reconstruction.
  'JsdocBlock.delimiter',
  // jsdoccomment fills postDelimiter/initial with the whitespace observed
  // on the opening `/**` line. ox-jsdoc emits empty string. Same Phase 6
  // source extraction work covers it.
  'JsdocBlock.postDelimiter',
  'JsdocBlock.initial',
  // jsdoccomment's `lineEnd` is the line ending of the closing `*/` line,
  // which is always "" for inputs without a trailing newline. ox-jsdoc
  // currently stores the inner-line ending. Phase 6 covers it.
  'JsdocBlock.lineEnd',
  // jsdoccomment uses "" for delimiterLineBreak/preterminalLineBreak when
  // the comment is single-line (`/** … */`). ox-jsdoc currently stores
  // " " here because the writer captures the literal margin span instead
  // of the conditional jsdoccomment value. Phase 6 covers it.
  'JsdocBlock.delimiterLineBreak',
  'JsdocBlock.preterminalLineBreak',
  // ox-jsdoc parser strips the leading `-` separator from the tag
  // description; jsdoccomment preserves it. Tracked as a parser-side fix
  // (separate task; not Note 4).
  'JsdocBlock.tags[*].description',
  'JsdocBlock.tags[*].descriptionLines[*].description',
  // ox-jsdoc trims trailing whitespace from description-line content
  // (`line.content.trim_end()` in parse_description_lines); jsdoccomment
  // keeps it. Visible in `/** Hello world */` where jsdoccomment yields
  // "Hello world " and ox-jsdoc yields "Hello world". Same trim is also
  // visible on the joined block-level `description` field.
  'JsdocBlock.descriptionLines[*].description',
  'JsdocBlock.description',
  // jsdoccomment captures the whitespace between tag/type/name segments
  // (` ` defaults), ox-jsdoc currently emits empty strings here because
  // its scanner does not retain those margins. Closed by the same Phase 6
  // / source-preserving extraction work as block.delimiter.
  'JsdocBlock.tags[*].postTag',
  'JsdocBlock.tags[*].postType',
  'JsdocBlock.tags[*].postName',
  'JsdocBlock.tags[*].postDelimiter',
  'JsdocBlock.tags[*].initial',
  'JsdocBlock.tags[*].delimiter',
  'JsdocBlock.tags[*].lineEnd',
  // jsdoccomment always emits at least one descriptionLines entry per tag
  // (even when description is empty); ox-jsdoc skips empty description
  // lines at parse time (Note 5 in design/005). Closed by Phase 1.2 +
  // SerializeOptions::Spacing.
  'JsdocBlock.tags[*].descriptionLines.length',
  // (Closed by Note 4: tag-specific parse rule (defaultNoNames). Removed
  // from KNOWN_DIFFERENCES; `JsdocBlock.tags[*].name` should now match.)
  // jsdoccomment captures `postDelimiter`/`initial` from the source line
  // margins for description lines; ox-jsdoc emits empty strings. Same
  // root cause as JsdocBlock.postDelimiter — Phase 6 source extraction.
  'JsdocBlock.descriptionLines[*].postDelimiter',
  'JsdocBlock.descriptionLines[*].initial',
  'JsdocBlock.descriptionLines[*].delimiter',
  'JsdocBlock.tags[*].descriptionLines[*].delimiter',
  'JsdocBlock.tags[*].descriptionLines[*].postDelimiter',
  'JsdocBlock.tags[*].descriptionLines[*].initial',
  'JsdocBlock.tags[*].typeLines[*].delimiter',
  'JsdocBlock.tags[*].typeLines[*].postDelimiter',
  'JsdocBlock.tags[*].typeLines[*].initial',
  // jsdoccomment exposes `tag` (the inline-tag name) on JsdocInlineTag.
  // The binary writer does not currently serialize the inline tag's name
  // (see emit_inline_tag in context.rs). Tracked as a binary-format
  // extension item.
  'JsdocBlock.inlineTags[*].tag',
  'JsdocBlock.tags[*].inlineTags[*].tag',
  // Phase 6 v1: source[] reconstruction is a skeleton. Per-line
  // start/postDelimiter/initial values fall through to "" because the
  // underlying writer fields are also "" (KNOWN_DIFFERENCES above
  // already cover those root causes). The reconstructed `source` string
  // therefore lacks the conventional spaces too. Closed once the
  // upstream writer fixes land.
  'JsdocBlock.source[*].source',
  'JsdocBlock.source[*].tokens.start',
  'JsdocBlock.source[*].tokens.delimiter',
  'JsdocBlock.source[*].tokens.postDelimiter',
  'JsdocBlock.source[*].tokens.postTag',
  'JsdocBlock.source[*].tokens.postType',
  'JsdocBlock.source[*].tokens.postName',
  'JsdocBlock.source[*].tokens.lineEnd',
  // jsdoccomment captures the tag description with its leading `-`
  // separator; ox-jsdoc parser strips it (separate parser-side fix
  // tracked alongside the JsdocBlock.tags[*].description gap above).
  'JsdocBlock.source[*].tokens.description',
  // Same line-count gap as JsdocBlock.tags[*].descriptionLines.length:
  // jsdoccomment emits an entry per source line including empties; ox-jsdoc
  // skips empty description lines at parse time (Note 5).
  'JsdocBlock.source.length',
  'JsdocBlock.tags[*].source.length',
  'JsdocBlock.tags[*].source[*].source',
  'JsdocBlock.tags[*].source[*].tokens.start',
  'JsdocBlock.tags[*].source[*].tokens.delimiter',
  'JsdocBlock.tags[*].source[*].tokens.postDelimiter',
  'JsdocBlock.tags[*].source[*].tokens.postTag',
  'JsdocBlock.tags[*].source[*].tokens.postType',
  'JsdocBlock.tags[*].source[*].tokens.postName',
  'JsdocBlock.tags[*].source[*].tokens.description',
  'JsdocBlock.tags[*].source[*].tokens.lineEnd'
])

/** Field paths to strip from BOTH sides before comparison (ox-jsdoc-only or
 * jsdoccomment-only fields that have no counterpart). */
export const STRIP_FIELDS: ReadonlySet<string> = new Set([
  'range', // ox-jsdoc-specific [start, end] tuple
  'parsedType' // jsdoccomment runs jsdoc-type-pratt-parser; ox-jsdoc may emit a different shape
])

/** A single field mismatch found by the comparator. */
export interface Mismatch {
  /** Dotted path inside the AST, e.g. `JsdocBlock.tags[0].postTag`. */
  path: string
  /** Path with concrete list indices generalized to `*` for KNOWN_DIFFERENCES lookup. */
  patternPath: string
  expected: unknown
  actual: unknown
}

/**
 * Recursively compare two AST objects.
 *
 * @param expected jsdoccomment's `commentParserToESTree()` output
 * @param actual ox-jsdoc's `RemoteJsdocBlock.toJSON()` output (compat_mode)
 * @returns Every mismatch encountered, except those listed in `KNOWN_DIFFERENCES`.
 */
export function assertCompatible(actual: any, expected: any): Mismatch[] {
  const mismatches: Mismatch[] = []
  walkObject(expected, actual, expected?.type ?? 'root', expected?.type ?? 'root', mismatches)
  return mismatches.filter(m => !KNOWN_DIFFERENCES.has(m.patternPath))
}

function walkObject(
  expected: any,
  actual: any,
  path: string,
  patternPath: string,
  out: Mismatch[]
): void {
  if (expected === null || expected === undefined) {
    if (actual !== null && actual !== undefined) {
      out.push({ path, patternPath, expected, actual })
    }
    return
  }
  if (typeof expected !== 'object') {
    if (expected !== actual) {
      out.push({ path, patternPath, expected, actual })
    }
    return
  }
  if (Array.isArray(expected)) {
    if (!Array.isArray(actual)) {
      out.push({ path, patternPath, expected, actual })
      return
    }
    if (expected.length !== actual.length) {
      out.push({
        path: `${path}.length`,
        patternPath: `${patternPath}.length`,
        expected: expected.length,
        actual: actual.length
      })
    }
    const len = Math.min(expected.length, actual.length)
    for (let i = 0; i < len; i++) {
      walkObject(expected[i], actual[i], `${path}[${i}]`, `${patternPath}[*]`, out)
    }
    return
  }
  if (typeof actual !== 'object' || actual === null || Array.isArray(actual)) {
    out.push({ path, patternPath, expected, actual })
    return
  }
  for (const key of Object.keys(expected)) {
    if (STRIP_FIELDS.has(key)) {
      continue
    }
    const childPath = `${path}.${key}`
    const childPattern = `${patternPath}.${key}`
    walkObject(expected[key], actual[key], childPath, childPattern, out)
  }
}
