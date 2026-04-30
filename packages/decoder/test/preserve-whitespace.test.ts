import { describe, expect, it } from 'vite-plus/test'

import { parsedPreservingWhitespace } from '../src/internal/preserve-whitespace.ts'

describe('parsedPreservingWhitespace', () => {
  it('returns trimmed single-line input as-is', () => {
    for (const [input, expected] of [
      ['', ''],
      ['hello  ', 'hello'],
      ['  * single line', '* single line'],
      [' * ', '*'],
      [' * * ', '* *'],
      ['***', '***']
    ]) {
      expect(parsedPreservingWhitespace(input), `input: ${JSON.stringify(input)}`).toBe(expected)
    }
  })

  it('collapses empty multi-line input to blank lines', () => {
    expect(parsedPreservingWhitespace('\n\n    ')).toBe('\n\n')
  })

  it('strips the `*` continuation prefix', () => {
    const input = '\n     * asterisk\n    '
    expect(parsedPreservingWhitespace(input)).toBe('\nasterisk\n')
  })

  it('preserves nested star lists', () => {
    // The outer `*` is the continuation prefix and is stripped; the
    // inner `* li` survives as content.
    const input = '\n     * * li\n     * * li\n'
    expect(parsedPreservingWhitespace(input)).toBe('\n* li\n* li')
  })

  it('handles mixed star layouts consistently', () => {
    const input = '\n     * * 1\n     ** 2\n'
    expect(parsedPreservingWhitespace(input)).toBe('\n* 1\n* 2')
  })

  it('preserves paragraph breaks (blank lines)', () => {
    const input = '\n    1\n\n    2\n\n    3\n                '
    expect(parsedPreservingWhitespace(input)).toBe('\n1\n\n2\n\n3\n')
  })

  it('preserves indented code blocks past the `* ` prefix', () => {
    const input = ' * some intro.\n *\n *     code()\n *\n * outro.\n'
    expect(parsedPreservingWhitespace(input)).toBe('some intro.\n\n    code()\n\noutro.')
  })

  it('keeps the leading star on markdown emphasis', () => {
    const input = ' * normal\n * *foo* is emphasis\n * *bold* word\n'
    expect(parsedPreservingWhitespace(input)).toBe('normal\n*foo* is emphasis\n*bold* word')
  })

  it('treats underscore after star as emphasis', () => {
    const input = ' * normal\n * *_underscore_* example\n'
    expect(parsedPreservingWhitespace(input)).toBe('normal\n*_underscore_* example')
  })

  it('treats punctuation after star as a continuation prefix', () => {
    expect(parsedPreservingWhitespace(' * abc\n * .punct\n')).toBe('abc\n.punct')
    expect(parsedPreservingWhitespace(' * abc\n * \n * def\n')).toBe('abc\n\ndef')
  })

  it('classifies `*<alnum>` (no space) as emphasis (algorithm quirk)', () => {
    // Matches the Rust port — `*no_space` is kept verbatim because the
    // algorithm cannot distinguish `*foo` (incomplete emphasis) from
    // `*foo*` (completed) without scanning ahead.
    expect(parsedPreservingWhitespace(' * normal\n *no_space\n')).toBe('normal\n*no_space')
  })

  it('passes lines without `*` prefix through trimmed', () => {
    expect(parsedPreservingWhitespace('first line\nsecond line\n  third line  ')).toBe(
      'first line\nsecond line\nthird line'
    )
  })
})
