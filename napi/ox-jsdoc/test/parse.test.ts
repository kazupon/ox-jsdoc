import type {
  RemoteJsdocBlock,
  RemoteJsdocDescriptionLine,
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

  it('parses non-ASCII type names when parseTypes is enabled', () => {
    // `Foo.é` used to abort the process.
    const result = parse('/** @param {Foo.é} x desc */', { parseTypes: true })
    expect(result.diagnostics).toEqual([])
    const tag = result.ast!.tags[0] as RemoteJsdocTag
    expect(tag.parsedType?.toJSON()).toMatchObject({
      type: 'TypeNamePath',
      left: { type: 'TypeName', value: 'Foo' },
      right: { type: 'TypeProperty', value: 'é' }
    })
  })

  it('parses conditional types inside brackets when parseTypes is enabled', () => {
    // Used to make parse() loop forever.
    const result = parse('/** @type {A extends (B extends C ? D : E) ? F : G} */', {
      parseTypes: true,
      typeParseMode: 'typescript'
    })
    expect(result.diagnostics).toEqual([])
    const tag = result.ast!.tags[0] as RemoteJsdocTag
    expect(tag.parsedType?.toJSON()).toMatchObject({
      type: 'TypeConditional',
      checkType: { type: 'TypeName', value: 'A' },
      extendsType: {
        type: 'TypeParenthesis',
        element: {
          type: 'TypeConditional',
          checkType: { value: 'B' },
          extendsType: { value: 'C' },
          trueType: { value: 'D' },
          falseType: { value: 'E' }
        }
      },
      trueType: { value: 'F' },
      falseType: { value: 'G' }
    })
  })

  it('leaves no diagnostics for a type that parses without its question mark', () => {
    // Used to report "expected token" although the type parsed.
    const result = parse('/** @type {A extends B C : D} */', {
      parseTypes: true,
      typeParseMode: 'typescript'
    })
    expect(result.diagnostics).toEqual([])
    const tag = result.ast!.tags[0] as RemoteJsdocTag
    expect(tag.parsedType?.toJSON()).toMatchObject({
      type: 'TypeConditional',
      checkType: { value: 'A' },
      extendsType: { value: 'B' },
      trueType: { value: 'C' },
      falseType: { value: 'D' }
    })
  })

  it('matches oxc_jsdoc tag splitting for indented code blocks', () => {
    const source =
      '/**\n * @deprecated\n *     @myDecorator\n *     class Foo {}\n * @type {string}\n */'
    const result = parse(source)
    expect(result.diagnostics).toEqual([])
    const tags = result.ast!.tags as RemoteJsdocTag[]
    expect(tags.map(tag => (tag.tag as RemoteJsdocTagName).value)).toEqual(['deprecated', 'type'])
    expect(tags[0].rawBody).toBe('    @myDecorator\n    class Foo {}')
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

  it('can return jsdoccomment input blocks for the batch hot path', () => {
    const result = parseBatch([{ sourceText: '/**\n * @param {string} id Description.\n */' }], {
      compatMode: true,
      preserveWhitespace: true,
      emptyStringForNull: true,
      output: 'jsdoccomment-input'
    })

    expect(result.blocks).toHaveLength(1)
    const block = result.blocks[0]!
    expect(block.tags).toEqual([])
    expect(block.source[1].tokens.tag).toBe('@param')
    expect(block.source[1].tokens.type).toBe('{string}')
    expect(block.source[1].tokens.name).toBe('id')
    expect(block.source[1].tokens.description).toBe('Description.')
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

// The compat source lines used to be located in UTF-16 units and matched
// against the UTF-8 byte ranges of the tags. After non-ASCII text, a tag got
// the lines of the next tag, and a tag whose first line was not a tag line was
// dropped.
describe('compat toJSON() after non-ASCII text', () => {
  const tagLines = (sourceText: string) => {
    const result = parse(sourceText, {
      compatMode: true,
      emptyStringForNull: true,
      preserveWhitespace: true
    })
    const json = result.ast!.toJSON() as {
      tags: Array<{ tag: string; source: Array<{ number: number; tokens: { tag: string } }> }>
    }
    return json.tags.map(tag => [
      tag.tag,
      tag.source.map(line => line.number),
      tag.source[0]?.tokens.tag
    ])
  }

  it.each([
    ['ASCII', '/**\n * Desc.\n * @param a arg\n * @returns value\n */'],
    ['Japanese', '/**\n * 日本語の説明です。\n * @param a 引数\n * @returns 戻り値\n */'],
    ['emoji', '/**\n * Party 🎉🎉🎉 time.\n * @param a arg\n * @returns value\n */'],
    ['accented Latin', '/**\n * Größe und Maße.\n * @param a arg\n * @returns value\n */'],
    ['CRLF', '/**\r\n * 日本語の説明です。\r\n * @param a 引数\r\n * @returns 戻り値\r\n */']
  ])('gives each tag its own lines (%s)', (_, sourceText) => {
    expect(tagLines(sourceText)).toEqual([
      ['param', [2], '@param'],
      ['returns', [3, 4], '@returns']
    ])
  })

  it('keeps every tag after many lines of non-ASCII text', () => {
    const sourceText =
      '/**\n * 一行目の説明。\n * 二行目の説明。\n * 三行目の説明。\n' +
      ' * @param a 引数\n * @param b 引数\n * @throws {Error} 例外\n * @returns 戻り値\n */'
    expect(tagLines(sourceText)).toEqual([
      ['param', [4], '@param'],
      ['param', [5], '@param'],
      ['throws', [6], '@throws'],
      ['returns', [7, 8], '@returns']
    ])
  })

  it('keeps a tag with a multi-line type after non-ASCII text', () => {
    const sourceText =
      '/**\n * 説明。\n * @param {{\n *   a: string\n * }} options オプション\n * @returns 戻り値\n */'
    expect(tagLines(sourceText)).toEqual([
      ['param', [2, 3, 4], '@param'],
      ['returns', [5, 6], '@returns']
    ])
  })
})

// The decoder cached strings by their offset only, so a string that starts
// where another one starts came back as the one read first.
describe('string getters do not depend on the order of reads', () => {
  it('reads the name and the raw body of a tag in either order', () => {
    const source = '/** @param foo the foo */'
    const forward = parse(source).ast!.tags[0] as RemoteJsdocTag
    const forwardName = (forward.name as RemoteJsdocTagNameValue).raw
    expect([forwardName, forward.rawBody]).toEqual(['foo', 'foo the foo'])

    const reverse = parse(source).ast!.tags[0] as RemoteJsdocTag
    const reverseRawBody = reverse.rawBody
    expect([(reverse.name as RemoteJsdocTagNameValue).raw, reverseRawBody]).toEqual([
      'foo',
      'foo the foo'
    ])
  })

  it('reads the compat delimiters in either order', () => {
    const forward = parse('/** foo */', { compatMode: true }).ast as RemoteJsdocBlock
    const forwardPostDelimiter = forward.postDelimiter
    expect([forwardPostDelimiter, forward.initial, forward.lineEnd]).toEqual([' ', '', ''])

    const reverse = parse('/** foo */', { compatMode: true }).ast as RemoteJsdocBlock
    const reverseLineEnd = reverse.lineEnd
    const reverseInitial = reverse.initial
    expect([reverse.postDelimiter, reverseInitial, reverseLineEnd]).toEqual([' ', '', ''])
  })

  // Every string getter that toJSON() reports, read on a fresh AST each time
  // (an empty cache, so the value is right), and read in reverse order on one
  // AST, give the same values.
  const stringPaths = (
    node: unknown,
    json: unknown,
    path: Array<string | number>,
    out: Array<Array<string | number>>
  ) => {
    if (node === null || node === undefined || json === null || typeof json !== 'object') {
      return
    }
    if (Array.isArray(json)) {
      json.forEach((child, index) =>
        stringPaths((node as unknown[])[index], child, [...path, index], out)
      )
      return
    }
    for (const [key, value] of Object.entries(json)) {
      if (key === 'source' || key === 'range' || key === 'type') {
        continue
      }
      const child = (node as Record<string, unknown>)[key]
      if (typeof value === 'string' && typeof child === 'string') {
        out.push([...path, key])
      } else if (value !== null && typeof value === 'object') {
        stringPaths(child, value, [...path, key], out)
      }
    }
  }
  const read = (node: unknown, path: Array<string | number>) =>
    path.reduce<unknown>(
      (current, key) => (current as Record<string | number, unknown>)?.[key],
      node
    )

  it.each([
    ['/** @param foo the foo */', false],
    ['/** @param foo the foo */', true],
    ['/**\n * Desc.\n * @see {@link Foo|Bar} desc\n * @returns {string} the value\n */', false],
    ['/**\n * Desc.\n * @see {@link Foo|Bar} desc\n * @returns {string} the value\n */', true],
    ['/** @internal */', true]
  ])('gives the same strings in any order (%j, compatMode: %s)', (source, compatMode) => {
    const fresh = () => parse(source, { compatMode }).ast!
    const paths: Array<Array<string | number>> = []
    stringPaths(fresh(), fresh().toJSON(), [], paths)
    expect(paths.length).toBeGreaterThan(0)

    const truth = paths.map(path => read(fresh(), path))
    const ast = fresh()
    const reversed = [...paths]
      .reverse()
      .map(path => read(ast, path))
      .reverse()
    expect(reversed).toEqual(truth)
  })
})

// The parser joins the lines of a tag body without their margins (` * `), so
// the strings of such a tag are not slices of the source text.
describe('tags whose body spans several lines', () => {
  const descriptions = (tag: RemoteJsdocTag) =>
    tag.descriptionLines.map(line => (line as RemoteJsdocDescriptionLine).description)

  it('reads the name after a multi-line type', () => {
    const tag = parse('/**\n * @param {{\n *   a: string\n * }} options desc\n */').ast!
      .tags[0] as RemoteJsdocTag
    expect((tag.name as RemoteJsdocTagNameValue).raw).toBe('options')
    expect(descriptions(tag)).toEqual(['desc'])
  })

  it.each([false, true])(
    'reads the description line of a multi-line description (compatMode: %s)',
    compatMode => {
      const tag = parse('/**\n * @param foo the foo\n * second line\n */', { compatMode }).ast!
        .tags[0] as RemoteJsdocTag
      expect(descriptions(tag)).toEqual(['the foo\nsecond line'])
    }
  )

  it('reads the type names of a multi-line type when parseTypes is enabled', () => {
    const typeNames = (source: string) => {
      const names: string[] = []
      const walk = (node: unknown): void => {
        if (node === null || typeof node !== 'object') {
          return
        }
        const record = node as Record<string, unknown>
        if (record.type === 'TypeName') {
          names.push(record.value as string)
        }
        Object.entries(record).forEach(([key, child]) => key !== 'range' && walk(child))
      }
      const tag = parse(source, { parseTypes: true }).ast!.tags[0] as RemoteJsdocTag
      walk(tag.parsedType?.toJSON() ?? null)
      return names
    }
    const oneLine = typeNames('/** @param {{ a: Array<string>, b: number }} x */')
    expect(oneLine).toEqual(['a', 'Array', 'string', 'b', 'number'])
    expect(
      typeNames('/**\n * @param {{\n *   a: Array<string>,\n *   b: number\n * }} x\n */')
    ).toEqual(oneLine)
  })

  // The range of the first `needle` in an ASCII `source`.
  const at = (source: string, needle: string) => {
    const start = source.indexOf(needle)
    return [start, start + needle.length]
  }

  it('gives the source ranges of the nodes after a multi-line type', () => {
    const source = '/**\n * @param {{\n *   a: string\n * }} options desc\n */'
    const tag = parse(source).ast!.tags[0] as RemoteJsdocTag
    expect([...(tag.rawType as RemoteJsdocTypeSource).range]).toEqual([
      source.indexOf('{{'),
      source.indexOf('}}') + 2
    ])
    expect([...(tag.name as RemoteJsdocTagNameValue).range]).toEqual(at(source, 'options'))
    expect(tag.descriptionLines.map(line => [...line.range])).toEqual([at(source, 'desc')])
  })

  it('gives the source ranges of inline tags after a line break', () => {
    const source =
      '/**\n * Summary {@link Foo}\n * more {@link Bar} text\n * @param foo the foo\n *   see {@link Baz} here\n */'
    const ast = parse(source).ast!
    expect(ast.inlineTags.map(tag => [...tag.range])).toEqual([
      at(source, '{@link Foo}'),
      at(source, '{@link Bar}')
    ])
    expect((ast.tags[0] as RemoteJsdocTag).inlineTags.map(tag => [...tag.range])).toEqual([
      at(source, '{@link Baz}')
    ])
  })
})
