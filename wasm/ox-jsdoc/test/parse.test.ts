import { describe, expect, it, beforeAll } from 'vite-plus/test'
import { initWasm, parse } from '../src-js/index.js'

beforeAll(async () => {
  await initWasm()
})

describe('parse', () => {
  it('parses a basic param tag', () => {
    const result = parse('/** @param {string} id - The user ID */')
    expect(result.diagnostics).toEqual([])
    expect(result.ast).not.toBeNull()
    expect(result.ast!.type).toBe('JsdocBlock')
    expect(result.ast!.tags.length).toBe(1)
    expect(result.ast!.tags[0].tag).toBe('param')
    expect(result.ast!.tags[0].rawType).toBe('string')
    expect(result.ast!.tags[0].name).toBe('id')
    expect(result.ast!.tags[0].description).toBe('The user ID')
  })

  it('parses description with inline tags', () => {
    const result = parse('/** See {@link Foo} for details. */')
    expect(result.diagnostics).toEqual([])
    expect(result.ast).not.toBeNull()
    expect(result.ast!.description).toContain('{@link Foo}')
    expect(result.ast!.inlineTags.length).toBe(1)
    expect(result.ast!.inlineTags[0].tag).toBe('link')
    expect(result.ast!.inlineTags[0].namepathOrURL).toBe('Foo')
  })

  it('parses multiple tags', () => {
    const result = parse('/**\n * @param {string} id\n * @returns {User}\n */')
    expect(result.diagnostics).toEqual([])
    expect(result.ast!.tags.length).toBe(2)
    expect(result.ast!.tags[0].tag).toBe('param')
    expect(result.ast!.tags[1].tag).toBe('returns')
  })

  it('returns diagnostics for malformed input', () => {
    const result = parse('/** {@link Foo */')
    expect(result.diagnostics.length).toBeGreaterThan(0)
    expect(result.diagnostics[0].message).toContain('inline tag')
  })

  it('rejects non-jsdoc input', () => {
    const result = parse('/* plain */')
    expect(result.ast).toBeNull()
    expect(result.diagnostics.length).toBeGreaterThan(0)
  })

  it('supports fenceAware option', () => {
    const source = '/**\n * @example\n * ```ts\n * @decorator()\n * ```\n * @returns {void}\n */'
    const result = parse(source, { fenceAware: true })
    expect(result.diagnostics).toEqual([])
    expect(result.ast!.tags.length).toBe(2)
    expect(result.ast!.tags[0].tag).toBe('example')
    expect(result.ast!.tags[1].tag).toBe('returns')
  })

  it('returns ast with correct span', () => {
    const source = '/** @param x */'
    const result = parse(source)
    expect(result.ast!.start).toBe(0)
    expect(result.ast!.end).toBe(source.length)
    expect(result.ast!.range).toEqual([0, source.length])
  })

  it('handles optional parameter syntax', () => {
    const result = parse('/** @param {string} [name=default] - desc */')
    expect(result.diagnostics).toEqual([])
    const tag = result.ast!.tags[0]
    expect(tag.optional).toBe(true)
    expect(tag.defaultValue).toBe('default')
    expect(tag.name).toBe('name')
  })

  it('handles empty comment', () => {
    const result = parse('/** */')
    expect(result.diagnostics).toEqual([])
    expect(result.ast).not.toBeNull()
    expect(result.ast!.tags.length).toBe(0)
  })
})

describe('initWasm', () => {
  it('can be called multiple times safely', async () => {
    await initWasm()
    await initWasm()
    const result = parse('/** @param x */')
    expect(result.ast).not.toBeNull()
  })
})
