import type {
  RemoteJsdocBlock,
  RemoteJsdocInlineTag,
  RemoteJsdocTag,
  RemoteJsdocTagName,
  RemoteJsdocTagNameValue,
  RemoteJsdocTypeSource
} from '@ox-jsdoc/decoder'
import { beforeAll, describe, expect, it } from 'vite-plus/test'

import { initWasm, parse } from '../src-js/index.js'

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
