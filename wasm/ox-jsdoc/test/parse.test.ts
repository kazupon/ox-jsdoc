import { describe, expect, it, beforeAll } from 'vite-plus/test'
import { initWasm, parse, parseType, parseTypeCheck } from '../src-js/index.js'

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

describe('parse with parseTypes', () => {
  it('does not include parsedType when parseTypes is disabled (default)', () => {
    const result = parse('/** @param {string} id */')
    expect(result.diagnostics).toEqual([])
    const tag = result.ast!.tags[0]
    expect(tag.rawType).toBe('string')
    // parsedType is omitted from JSON when parseTypes is disabled
    expect(tag.parsedType).toBeUndefined()
  })

  it('populates parsedType when parseTypes is enabled', () => {
    const result = parse('/** @param {string} id */', {
      parseTypes: true,
      typeParseMode: 'jsdoc'
    })
    expect(result.diagnostics).toEqual([])
    const tag = result.ast!.tags[0]
    expect(tag.rawType).toBe('string')
    expect(tag.parsedType).not.toBeNull()
    expect(tag.parsedType!.type).toBe('JsdocTypeName')
    expect(tag.parsedType!.value).toBe('string')
  })

  it('parses union types in typescript mode', () => {
    const result = parse('/** @param {string | number} id */', {
      parseTypes: true,
      typeParseMode: 'typescript'
    })
    expect(result.diagnostics).toEqual([])
    const tag = result.ast!.tags[0]
    expect(tag.parsedType!.type).toBe('JsdocTypeUnion')
    const elements = tag.parsedType!.elements as Array<{ type: string; value: string }>
    expect(elements.length).toBe(2)
    expect(elements[0].value).toBe('string')
    expect(elements[1].value).toBe('number')
  })

  it('parses generic types', () => {
    const result = parse('/** @returns {Array<string>} */', {
      parseTypes: true,
      typeParseMode: 'typescript'
    })
    expect(result.diagnostics).toEqual([])
    const tag = result.ast!.tags[0]
    expect(tag.parsedType!.type).toBe('JsdocTypeGeneric')
  })
})

describe('parseType', () => {
  it('parses a simple name', () => {
    const result = parseType('string', 'jsdoc')
    expect(result).not.toBeNull()
    expect(result).toContain('string')
  })

  it('parses union types', () => {
    const result = parseType('string | number', 'typescript')
    expect(result).not.toBeNull()
    expect(result).toBe('string | number')
  })

  it('parses generic types', () => {
    const result = parseType('Array<string>', 'typescript')
    expect(result).toBe('Array<string>')
  })

  it('parses dot notation generic in jsdoc mode', () => {
    const result = parseType('Array.<string>', 'jsdoc')
    expect(result).toBe('Array.<string>')
  })

  it('parses nullable types', () => {
    const result = parseType('?string', 'jsdoc')
    expect(result).toBe('?string')
  })

  it('parses optional types', () => {
    const result = parseType('string=', 'jsdoc')
    expect(result).toBe('string=')
  })

  it('parses variadic types', () => {
    const result = parseType('...string', 'jsdoc')
    expect(result).toBe('...string')
  })

  it('parses function types in jsdoc mode', () => {
    const result = parseType('function(string): number', 'jsdoc')
    expect(result).toBe('function(string): number')
  })

  it('parses arrow function in typescript mode', () => {
    const result = parseType('(x: number) => string', 'typescript')
    expect(result).toBe('(x: number) => string')
  })

  it('parses conditional types in typescript mode', () => {
    const result = parseType('T extends U ? X : Y', 'typescript')
    expect(result).toBe('T extends U ? X : Y')
  })

  it('returns null for invalid input', () => {
    const result = parseType('!!!', 'jsdoc')
    expect(result).toBeNull()
  })
})

describe('parseTypeCheck', () => {
  it('returns true for valid types', () => {
    expect(parseTypeCheck('string', 'jsdoc')).toBe(true)
    expect(parseTypeCheck('string | number', 'typescript')).toBe(true)
    expect(parseTypeCheck('Array<string>', 'typescript')).toBe(true)
    expect(parseTypeCheck('{a: string, b: number}', 'typescript')).toBe(true)
  })

  it('returns false for invalid types', () => {
    expect(parseTypeCheck('!!!', 'jsdoc')).toBe(false)
    expect(parseTypeCheck('', 'jsdoc')).toBe(false)
  })

  it('respects parse mode', () => {
    // intersection only works in typescript mode
    expect(parseTypeCheck('A & B', 'typescript')).toBe(true)
    expect(parseTypeCheck('A & B', 'jsdoc')).toBe(false)
  })

  it('defaults to jsdoc mode when no mode specified', () => {
    expect(parseTypeCheck('string')).toBe(true)
  })
})
