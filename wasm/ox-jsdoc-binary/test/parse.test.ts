import type {
  RemoteJsdocBlock,
  RemoteJsdocInlineTag,
  RemoteJsdocTag,
  RemoteJsdocTagName,
  RemoteJsdocTagNameValue,
  RemoteJsdocTypeSource
} from '@ox-jsdoc/decoder'
import { beforeAll, describe, expect, it } from 'vite-plus/test'

import { initWasm, parse, parseType, parseTypeCheck } from '../src-js/index.js'

beforeAll(async () => {
  await initWasm()
})

describe('parse (binary WASM binding)', () => {
  it('parses a basic param tag through the zero-copy view', () => {
    const result = parse('/** @param {string} id - The user ID */')
    try {
      expect(result.diagnostics).toEqual([])
      expect(result.ast).not.toBeNull()
      const ast = result.ast!
      expect(ast.type).toBe('JsdocBlock')
      expect(ast.tags.length).toBe(1)
      const tag = ast.tags[0] as RemoteJsdocTag
      expect((tag.tag as RemoteJsdocTagName).value).toBe('param')
      expect((tag.rawType as RemoteJsdocTypeSource).raw).toBe('string')
      expect((tag.name as RemoteJsdocTagNameValue).raw).toBe('id')
      expect(tag.description).toBe('The user ID')
    } finally {
      result.free()
    }
  })

  it('parses description with inline tags', () => {
    const result = parse('/** See {@link Foo} for details. */')
    try {
      expect(result.diagnostics).toEqual([])
      expect(result.ast!.description).toContain('{@link Foo}')
      expect(result.ast!.inlineTags.length).toBe(1)
      const inline = result.ast!.inlineTags[0] as RemoteJsdocInlineTag
      expect(inline.format).toBe('plain')
      expect(inline.namepathOrURL).toBe('Foo')
    } finally {
      result.free()
    }
  })

  it('parses multiple tags', () => {
    const result = parse('/**\n * @param {string} id\n * @returns {User}\n */')
    try {
      expect(result.diagnostics).toEqual([])
      expect(result.ast!.tags.length).toBe(2)
      const first = result.ast!.tags[0] as RemoteJsdocTag
      const second = result.ast!.tags[1] as RemoteJsdocTag
      expect((first.tag as RemoteJsdocTagName).value).toBe('param')
      expect((second.tag as RemoteJsdocTagName).value).toBe('returns')
    } finally {
      result.free()
    }
  })

  it('returns diagnostics for malformed input', () => {
    const result = parse('/** {@link Foo */')
    try {
      expect(result.diagnostics.length).toBeGreaterThan(0)
      expect(result.diagnostics[0].message).toContain('inline tag')
    } finally {
      result.free()
    }
  })

  it('returns null ast and a diagnostic for non-JSDoc input', () => {
    const result = parse('/* plain */')
    try {
      expect(result.ast).toBeNull()
      expect(result.diagnostics.length).toBe(1)
      expect(result.diagnostics[0].message).toContain('not a JSDoc block')
    } finally {
      result.free()
    }
  })

  it('exposes parsedType when parseTypes is enabled', () => {
    const result = parse('/**\n * @param {string | number} id\n */', {
      parseTypes: true,
      typeParseMode: 'typescript'
    })
    try {
      expect(result.diagnostics).toEqual([])
      const tag = result.ast!.tags[0] as RemoteJsdocTag
      const parsed = tag.parsedType
      expect(parsed).not.toBeNull()
      expect(parsed!.type).toBe('TypeUnion')
    } finally {
      result.free()
    }
  })

  it('respects baseOffset when computing absolute ranges', () => {
    const result = parse('/** ok */', { baseOffset: 100 })
    try {
      expect(result.diagnostics).toEqual([])
      expect((result.ast as RemoteJsdocBlock).range).toEqual([100, 109])
    } finally {
      result.free()
    }
  })
})

describe('parseType / parseTypeCheck (binary WASM binding)', () => {
  it('parseType returns the stringified type for a valid input', () => {
    expect(parseType('string', 'jsdoc')).toBe('string')
    expect(parseType('string | number', 'typescript')).toBe('string | number')
    expect(parseType('Array.<string>', 'jsdoc')).toBe('Array.<string>')
  })

  it('parseType returns null for invalid input', () => {
    expect(parseType('@', 'jsdoc')).toBeNull()
  })

  it('parseTypeCheck returns true for valid types', () => {
    expect(parseTypeCheck('string', 'jsdoc')).toBe(true)
    expect(parseTypeCheck('string | number', 'typescript')).toBe(true)
    expect(parseTypeCheck('Array<T>', 'typescript')).toBe(true)
  })

  it('parseTypeCheck returns false for invalid input', () => {
    expect(parseTypeCheck('@', 'jsdoc')).toBe(false)
  })

  it('defaults to jsdoc mode when mode is omitted', () => {
    expect(parseTypeCheck('Array.<string>')).toBe(true)
  })

  it('roundtrip: parseType is idempotent', () => {
    const inputs: Array<{ source: string; mode: 'jsdoc' | 'closure' | 'typescript' }> = [
      { source: 'string | number', mode: 'typescript' },
      { source: 'Array<string>', mode: 'typescript' },
      { source: 'function(string): number', mode: 'jsdoc' }
    ]
    for (const { source, mode } of inputs) {
      const first = parseType(source, mode)
      expect(first, `${source} failed first parse`).not.toBeNull()
      expect(parseType(first!, mode)).toBe(first)
    }
  })
})
