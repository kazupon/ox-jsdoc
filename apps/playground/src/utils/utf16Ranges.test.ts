/**
 * Tests for converting ox-jsdoc's UTF-8 byte ranges to UTF-16 offsets.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { describe, expect, test } from 'vite-plus/test'
import { createUtf8ToUtf16Offset, toUtf16Ranges } from './utf16Ranges'

const encoder = new TextEncoder()

/** The UTF-8 byte range of the first `part` in `text`. */
function byteRange(text: string, part: string): [number, number] {
  const index = text.indexOf(part)
  const start = encoder.encode(text.slice(0, index)).length
  return [start, start + encoder.encode(part).length]
}

describe('createUtf8ToUtf16Offset', () => {
  test('keeps the offsets of ASCII text', () => {
    const toUtf16 = createUtf8ToUtf16Offset('/** @param a */')
    for (let offset = 0; offset <= 15; offset++) {
      expect(toUtf16(offset)).toBe(offset)
    }
  })

  test('maps the offsets after multi-byte characters', () => {
    // é: 2 bytes, 日 本: 3 bytes each, 🎉: 4 bytes and 2 UTF-16 code units.
    const text = 'aé日本🎉b'
    const toUtf16 = createUtf8ToUtf16Offset(text)
    expect([0, 1, 3, 6, 9, 13, 14].map(toUtf16)).toEqual([0, 1, 2, 3, 4, 6, 7])
  })

  test('finds each part of a text', () => {
    const text = '/**\r\n * 二つの数を足す 🎉。\r\n * @param {number} a 一つ目。\r\n */'
    const toUtf16 = createUtf8ToUtf16Offset(text)
    for (const part of ['二つの数', '🎉', '@param', '{number}', '一つ目。', '*/']) {
      const [start, end] = byteRange(text, part)
      expect(text.slice(toUtf16(start), toUtf16(end))).toBe(part)
    }
  })
})

describe('toUtf16Ranges', () => {
  const sourceText = '/**\n * 日本語の説明。\n * @param {string} a 名前。\n */'
  const range = (part: string) => byteRange(sourceText, part)

  test('converts every range of a toJSON() result and adds the start of the comment', () => {
    const ast = {
      type: 'JsdocBlock',
      range: range(sourceText),
      descriptionLines: [{ type: 'JsdocDescriptionLine', range: range('日本語の説明。') }],
      tags: [
        {
          type: 'JsdocTag',
          range: range('@param {string} a 名前。'),
          tag: { type: 'JsdocTagName', range: range('param') },
          parsedType: { type: 'TypeName', range: range('string'), value: 'string' },
          descriptionLines: [{ type: 'JsdocDescriptionLine', range: range('名前。') }],
          source: [{ number: 2 }]
        }
      ],
      descriptionStartLine: 1
    }
    // The comment starts after `// コメント\n` (8 UTF-16 code units) in the file.
    const file = `// コメント\n${sourceText}`
    const start = file.indexOf('/**')
    const converted = toUtf16Ranges(ast, sourceText, start)

    expect(file.slice(...converted.range)).toBe(sourceText)
    expect(file.slice(...converted.descriptionLines[0]!.range)).toBe('日本語の説明。')
    const tag = converted.tags[0]!
    expect(file.slice(...tag.range)).toBe('@param {string} a 名前。')
    expect(file.slice(...tag.tag.range)).toBe('param')
    expect(file.slice(...tag.parsedType.range)).toBe('string')
    expect(file.slice(...tag.descriptionLines[0]!.range)).toBe('名前。')
    // Other numbers are kept.
    expect(tag.source).toEqual([{ number: 2 }])
    expect(converted.descriptionStartLine).toBe(1)
  })

  test('does not change its input', () => {
    const ast = { type: 'JsdocBlock', range: range(sourceText), tags: [] }
    const copy = structuredClone(ast)
    toUtf16Ranges(ast, sourceText, 10)
    expect(ast).toEqual(copy)
  })

  test('keeps the ranges of an ASCII comment, moved by its start', () => {
    const text = '/** @param a */'
    const ast = { type: 'JsdocBlock', range: [0, 15], tags: [{ type: 'JsdocTag', range: [4, 12] }] }
    expect(toUtf16Ranges(ast, text, 100)).toEqual({
      type: 'JsdocBlock',
      range: [100, 115],
      tags: [{ type: 'JsdocTag', range: [104, 112] }]
    })
  })
})
