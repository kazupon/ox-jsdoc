import type {
  RemoteJsdocBlock,
  RemoteJsdocInlineTag,
  RemoteJsdocTag,
  RemoteJsdocTagName,
  RemoteJsdocTagNameValue,
  RemoteJsdocTypeSource
} from '@ox-jsdoc/decoder'
import { describe, expect, it } from 'vite-plus/test'

import { parse, parseBatch } from '../src-js/index.js'

describe('parse (binary NAPI binding)', () => {
  it('parses a basic param tag and exposes lazy getters', () => {
    const result = parse('/** @param {string} id - The user ID */')
    expect(result.diagnostics).toEqual([])
    expect(result.ast).not.toBeNull()
    const ast = result.ast!
    expect(ast.type).toBe('JsdocBlock')
    expect(ast.tags.length).toBe(1)
    const tag = ast.tags[0] as RemoteJsdocTag
    expect(tag.type).toBe('JsdocTag')
    expect((tag.tag as RemoteJsdocTagName).value).toBe('param')
    expect((tag.rawType as RemoteJsdocTypeSource).raw).toBe('string')
    expect((tag.name as RemoteJsdocTagNameValue).raw).toBe('id')
    expect(tag.description).toBe('The user ID')
  })

  it('parses description with inline tags', () => {
    const result = parse('/** See {@link Foo} for details. */')
    expect(result.diagnostics).toEqual([])
    expect(result.ast).not.toBeNull()
    expect(result.ast!.description).toContain('{@link Foo}')
    expect(result.ast!.inlineTags.length).toBe(1)
    const inline = result.ast!.inlineTags[0] as RemoteJsdocInlineTag
    expect(inline.format).toBe('plain')
    expect(inline.namepathOrURL).toBe('Foo')
  })

  it('parses multiple tags', () => {
    const result = parse('/**\n * @param {string} id\n * @returns {User}\n */')
    expect(result.diagnostics).toEqual([])
    expect(result.ast!.tags.length).toBe(2)
    const first = result.ast!.tags[0] as RemoteJsdocTag
    const second = result.ast!.tags[1] as RemoteJsdocTag
    expect((first.tag as RemoteJsdocTagName).value).toBe('param')
    expect((second.tag as RemoteJsdocTagName).value).toBe('returns')
  })

  it('returns diagnostics for malformed input', () => {
    const result = parse('/** {@link Foo */')
    expect(result.diagnostics.length).toBeGreaterThan(0)
    expect(result.diagnostics[0].message).toContain('inline tag')
  })

  it('returns null ast and a diagnostic for non-JSDoc input', () => {
    const result = parse('/* plain */')
    expect(result.ast).toBeNull()
    expect(result.diagnostics.length).toBe(1)
    expect(result.diagnostics[0].message).toContain('not a JSDoc block')
  })

  it('exposes parsed_type when parseTypes is enabled', () => {
    const result = parse('/**\n * @param {string | number} id\n */', {
      parseTypes: true,
      typeParseMode: 'typescript'
    })
    expect(result.diagnostics).toEqual([])
    const tag = result.ast!.tags[0] as RemoteJsdocTag
    const parsed = tag.parsedType
    expect(parsed).not.toBeNull()
    expect(parsed!.type).toBe('TypeUnion')
  })

  it('respects baseOffset when computing absolute ranges', () => {
    const result = parse('/** ok */', { baseOffset: 100 })
    expect(result.diagnostics).toEqual([])
    expect((result.ast as RemoteJsdocBlock).range).toEqual([100, 109])
  })
})

describe('parseBatch (binary NAPI binding)', () => {
  it('returns empty result for an empty input', () => {
    const result = parseBatch([])
    expect(result.asts).toEqual([])
    expect(result.diagnostics).toEqual([])
    expect(result.sourceFile.rootCount).toBe(0)
  })

  it('parses N comments and yields one root per item', () => {
    const result = parseBatch([
      { sourceText: '/** first */' },
      { sourceText: '/**\n * @param {string} id\n */' },
      { sourceText: '/** third */' }
    ])
    expect(result.diagnostics).toEqual([])
    expect(result.asts.length).toBe(3)
    expect(result.asts[0]!.description).toBe('first')
    const tag = result.asts[1]!.tags[0] as RemoteJsdocTag
    expect((tag.tag as RemoteJsdocTagName).value).toBe('param')
    expect(result.asts[2]!.description).toBe('third')
  })

  it('marks failed items with null and emits diagnostic with rootIndex', () => {
    const result = parseBatch([
      { sourceText: '/** good */' },
      { sourceText: '/* not jsdoc */' },
      { sourceText: '/** also good */' }
    ])
    expect(result.asts[0]).not.toBeNull()
    expect(result.asts[1]).toBeNull()
    expect(result.asts[2]).not.toBeNull()
    const failureDiag = result.diagnostics.find(d => d.rootIndex === 1)
    expect(failureDiag).toBeDefined()
    expect(failureDiag!.message).toContain('not a JSDoc block')
  })

  it('respects per-item baseOffset for absolute range computation', () => {
    const result = parseBatch([
      { sourceText: '/** a */', baseOffset: 1000 },
      { sourceText: '/** b */', baseOffset: 2000 }
    ])
    expect((result.asts[0] as RemoteJsdocBlock).range[0]).toBe(1000)
    expect((result.asts[1] as RemoteJsdocBlock).range[0]).toBe(2000)
  })

  it('shares a single buffer across N roots (string dedup engaged)', () => {
    const single = parseBatch([{ sourceText: '/**\n * @param {string} id\n */' }])
    const singleBytes = (single.sourceFile as unknown as { view: DataView }).view.byteLength
    const batch50 = parseBatch(
      Array.from({ length: 50 }, () => ({ sourceText: '/**\n * @param {string} id\n */' }))
    )
    const batchBytes = (batch50.sourceFile as unknown as { view: DataView }).view.byteLength
    const perItem = batchBytes / 50
    // Per-item amortised size must be smaller than the standalone size
    // (header + dedup table amortise away).
    expect(perItem).toBeLessThan(singleBytes)
  })
})
