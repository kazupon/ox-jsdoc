/**
 * Level 2 dynamic compatibility test against `@es-joy/jsdoccomment`.
 *
 * Strategy: parse the same fixture comment with both libraries and compare
 * the resulting ESTree-style AST field-by-field (see `helpers.ts`).
 * Field paths listed in `KNOWN_DIFFERENCES` are skipped so this test
 * surfaces *new* divergences without forcing every legacy gap to be fixed
 * before merging the test infrastructure itself.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { commentParserToESTree, parseComment } from '@es-joy/jsdoccomment'
import { describe, expect, it } from 'vite-plus/test'

import { parse } from '../../src-js/index.js'

import { FIXTURES } from './fixtures.js'
import { assertCompatible, type Mismatch } from './helpers.js'

/** Strip the surrounding `/** ... *​/` markers — jsdoccomment expects the
 * inner body only via `parseComment({ value })`. */
function innerBody(source: string): string {
  return source.slice(2, -2)
}

function formatMismatches(name: string, mismatches: Mismatch[]): string {
  const header = `\n[${name}] ${mismatches.length} mismatch(es) vs jsdoccomment:`
  const lines = mismatches.map(
    m =>
      `  - ${m.path}\n    expected: ${JSON.stringify(m.expected)}\n    actual:   ${JSON.stringify(m.actual)}`
  )
  return [header, ...lines].join('\n')
}

describe('jsdoccomment compatibility (Level 2 — dynamic comparison)', () => {
  for (const fixture of FIXTURES) {
    it(`matches jsdoccomment for: ${fixture.name}`, () => {
      // jsdoccomment's API
      const parsed = parseComment({ value: innerBody(fixture.source), type: 'Block' } as never)
      const expected = commentParserToESTree(parsed, 'jsdoc')

      // ox-jsdoc compat path
      const result = parse(fixture.source, {
        compatMode: true,
        emptyStringForNull: true
      })
      expect(result.ast).not.toBeNull()
      const actual = result.ast!.toJSON()

      const mismatches = assertCompatible(actual, expected)
      if (mismatches.length > 0) {
        throw new Error(formatMismatches(fixture.name, mismatches))
      }
    })
  }
})
