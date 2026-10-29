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
import type { Fixture } from './fixtures.js'
import { assertCompatible } from './helpers.js'
import type { Mismatch } from './helpers.js'

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

// The compat `toJSON()` block description is the one of comment-parser's
// block (`parseComment().description`): the lines before the first tag, each
// trimmed, the empty ones dropped, joined with a space. The ESTree
// `JsdocBlock.description` joins them with `\n`, so the comparison above
// skips it.
describe('compat block description (comment-parser)', () => {
  const samples: ReadonlyArray<Fixture> = [
    ...FIXTURES,
    {
      name: 'tag with a continuation line',
      source: '/**\n * Summary\n * @param a first\n * second line\n */'
    },
    {
      name: 'two tags with continuation lines',
      source:
        '/**\n * Summary\n * more\n *\n * @param a first\n *   second\n * @returns {number} the value\n *   and more\n */'
    },
    { name: 'tags only', source: '/**\n * @param a first\n *   second\n */' },
    {
      name: 'description on the /** line',
      source: '/** Summary\n * @param a first\n * second\n */'
    },
    { name: 'single line with an @', source: '/** Summary @param a */' },
    {
      name: '@ lines in a fence',
      source:
        '/**\n * Summary\n * ```\n * @example inside\n * ```\n * after\n * @param a first\n *   second\n */'
    },
    {
      name: 'fence in a tag',
      source: '/**\n * Summary\n * @example\n * ```\n * @foo bar\n * ```\n * @param a first\n */'
    },
    {
      name: '@ in a line',
      source: '/**\n * Summary with a @mention\n * @param a first\n * second\n */'
    },
    {
      name: '@/path line',
      source: '/**\n * Summary\n * @/path not a tag\n * @param a first\n * second\n */'
    },
    {
      name: 'blank lines between tags',
      source: '/**\n * Summary\n *\n * @param a first\n *\n * after a blank\n * @param b\n */'
    }
  ]

  for (const sample of samples) {
    it(`matches comment-parser for: ${sample.name}`, () => {
      const expected = parseComment({ value: innerBody(sample.source), type: 'Block' } as never)
      const result = parse(sample.source, { compatMode: true, emptyStringForNull: true })
      const json = result.ast!.toJSON() as Record<string, unknown>
      expect(json.description).toBe(expected.description)
    })
  }

  it('leaves the continuation lines of a tag out', () => {
    const result = parse('/**\n * Summary\n * @param a first\n * second line\n */', {
      compatMode: true
    })
    const json = result.ast!.toJSON() as Record<string, unknown>
    expect(json.description).toBe('Summary')
  })
})
