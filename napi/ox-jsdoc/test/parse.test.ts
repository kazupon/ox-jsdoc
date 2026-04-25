import { describe, expect, it } from 'vite-plus/test'
import { parse } from '../src-js/index.js'

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

describe('parse with serialize options', () => {
  it('emits compat-mode delimiter / line metadata when compatMode is true', () => {
    const source = '/**\n * @param {string} id\n */'
    const result = parse(source, { compatMode: true })
    expect(result.diagnostics).toEqual([])
    const block = result.ast!
    expect(block.delimiter).toBe('/**')
    expect(block.terminal).toBe('*/')
    expect(typeof block.endLine).toBe('number')
    expect(typeof block.hasPreterminalDescription).toBe('number')
    const tag = block.tags[0]
    expect(tag.tag).toBe('param')
    expect(tag).not.toHaveProperty('optional')
    expect(tag).not.toHaveProperty('defaultValue')
    expect(tag).not.toHaveProperty('rawBody')
    expect(tag).not.toHaveProperty('body')
    expect(tag.delimiter).toBe('*')
    expect(typeof tag.postDelimiter).toBe('string')
  })

  it('omits compat-only fields by default', () => {
    const result = parse('/**\n * @param {string} id\n */')
    const block = result.ast!
    expect(block).not.toHaveProperty('delimiter')
    expect(block).not.toHaveProperty('endLine')
    const tag = block.tags[0]
    expect(tag).toHaveProperty('optional', false)
    expect(tag).not.toHaveProperty('postDelimiter')
  })

  it('converts null optional strings to "" when emptyStringForNull is true', () => {
    const result = parse('/** @author */', {
      compatMode: true,
      emptyStringForNull: true
    })
    const tag = result.ast!.tags[0]
    expect(tag.tag).toBe('author')
    expect(tag.rawType).toBe('') // would be null without emptyStringForNull
    expect(tag.name).toBe('') // ditto
  })

  it('keeps null optional strings as null when emptyStringForNull is false', () => {
    const result = parse('/** @author */', { compatMode: true })
    const tag = result.ast!.tags[0]
    expect(tag.rawType).toBeNull()
    expect(tag.name).toBeNull()
  })

  it('drops position fields when includePositions is false', () => {
    const result = parse('/** @param x */', { includePositions: false })
    const block = result.ast!
    expect(block).not.toHaveProperty('start')
    expect(block).not.toHaveProperty('end')
    expect(block).not.toHaveProperty('range')
  })
})
