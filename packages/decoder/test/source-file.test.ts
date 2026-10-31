import { describe, expect, it } from 'vite-plus/test'

import {
  RemoteJsdocBlock,
  RemoteJsdocText,
  RemoteSourceFile,
  RemoteTypeName
} from '../src/index.js'
import type {
  RemoteJsdocInlineTag,
  RemoteTypeBigInt,
  RemoteTypeComputedMethod,
  RemoteTypeComputedProperty,
  RemoteTypeMethodSignature,
  RemoteTypeTemplateLiteral,
  RemoteTypeTypeParameter
} from '../src/index.js'
import { FixtureBuilder } from './fixture-builder.ts'

interface RemoteSourceFileInstance {
  readonly compatMode: boolean
  readonly nodeCount: number
  readonly rootCount: number
  readonly nodesOffset: number
  readonly asts: Array<unknown>
  getString(index: number): string | null
}

interface RemoteJsdocTextNode {
  readonly type: string
  readonly value: string
  readonly range: [number, number]
  readonly parent: unknown
  toJSON(): unknown
}

interface RemoteTypeNameNode {
  readonly value: string
}

interface RemoteJsdocTagNameNode {
  readonly value: string
  readonly parent: unknown
}

interface RemoteJsdocTagNode {
  readonly type: string
  readonly tag: RemoteJsdocTagNameNode | null
}

interface RemoteJsdocBlockNode {
  readonly tags: RemoteJsdocTagNode[]
  readonly descriptionLines: unknown[]
  readonly inlineTags: unknown[]
}

const RemoteSourceFileCtor = RemoteSourceFile as unknown as new (
  buffer: Uint8Array,
  options?: { emptyStringForNull?: boolean }
) => RemoteSourceFileInstance

describe('RemoteSourceFile.constructor', () => {
  it('parses Header offsets and counts', () => {
    const b = new FixtureBuilder()
    b.setSourceTextLength(42)
    const sf = new RemoteSourceFileCtor(b.build())
    expect(sf.compatMode).toBe(false)
    expect(sf.nodeCount).toBe(1) // sentinel only
    expect(sf.rootCount).toBe(0)
    expect(sf.nodesOffset).toBeGreaterThanOrEqual(40)
  })

  it('reads compat_mode flag bit', () => {
    const b = new FixtureBuilder()
    b.compatMode = true
    const sf = new RemoteSourceFileCtor(b.build())
    expect(sf.compatMode).toBe(true)
  })

  it('rejects buffers shorter than the Header', () => {
    expect(() => new RemoteSourceFileCtor(new Uint8Array(10))).toThrow(/too short/)
  })

  it('rejects incompatible major versions', () => {
    const b = new FixtureBuilder()
    const buf = b.build()
    buf[0] = 0xf0 // major 15
    expect(() => new RemoteSourceFileCtor(buf)).toThrow(/incompatible/)
  })
})

describe('RemoteSourceFile.getString', () => {
  it('resolves interned strings via the offsets table', () => {
    const b = new FixtureBuilder()
    const idx = b.internString('hello world')
    const sf = new RemoteSourceFileCtor(b.build())
    expect(sf.getString(idx)).toBe('hello world')
  })

  it('returns null for the 30-bit None sentinel', () => {
    const b = new FixtureBuilder()
    const sf = new RemoteSourceFileCtor(b.build())
    expect(sf.getString(0x3fff_ffff)).toBeNull()
  })

  it('caches resolved strings on repeat lookups', () => {
    const b = new FixtureBuilder()
    const idx = b.internString('cached')
    const sf = new RemoteSourceFileCtor(b.build())
    const a = sf.getString(idx)
    const c = sf.getString(idx)
    expect(a).toBe('cached')
    expect(c).toBe(a) // string interning gives === identity
  })
})

describe('RemoteSourceFile.asts', () => {
  it('yields null for parse-failure roots (node_index === 0)', () => {
    const b = new FixtureBuilder()
    b.pushRoot({ nodeIndex: 0, baseOffset: 100 })
    const sf = new RemoteSourceFileCtor(b.build())
    expect(sf.asts).toHaveLength(1)
    expect(sf.asts[0]).toBeNull()
  })

  it('yields a lazy class instance for non-zero roots', () => {
    const b = new FixtureBuilder()
    const node = b.emitStringNode({
      parentIndex: 0,
      kind: 0x0f, // JsdocText
      value: 'hello',
      posStart: 0,
      posEnd: 5
    })
    b.pushRoot({ nodeIndex: node, baseOffset: 100 })
    const sf = new RemoteSourceFileCtor(b.build())
    const asts = sf.asts
    expect(asts).toHaveLength(1)
    expect(asts[0]).toBeInstanceOf(RemoteJsdocText)
    // The same call twice should hit the cache.
    expect(sf.asts[0]).toBe(asts[0])
  })
})

describe('Lazy node dispatch', () => {
  it('dispatches Kind 0x0F to RemoteJsdocText with the right value/range', () => {
    const b = new FixtureBuilder()
    const node = b.emitStringNode({
      parentIndex: 0,
      kind: 0x0f,
      value: 'hello world',
      posStart: 0,
      posEnd: 11
    })
    b.pushRoot({ nodeIndex: node, baseOffset: 100 })
    const sf = new RemoteSourceFileCtor(b.build())
    const text = sf.asts[0] as RemoteJsdocTextNode
    expect(text.type).toBe('JsdocText')
    expect(text.value).toBe('hello world')
    expect(text.range).toEqual([100, 111])
    expect(text.parent).toBeNull() // root
  })

  it('dispatches Kind 0x80 to RemoteTypeName', () => {
    const b = new FixtureBuilder()
    const node = b.emitStringNode({
      parentIndex: 0,
      kind: 0x80, // TypeName
      value: 'Foo',
      posStart: 0,
      posEnd: 3
    })
    b.pushRoot({ nodeIndex: node, baseOffset: 0 })
    const sf = new RemoteSourceFileCtor(b.build())
    const tn = sf.asts[0] as RemoteTypeNameNode
    expect(tn).toBeInstanceOf(RemoteTypeName)
    expect(tn.value).toBe('Foo')
  })

  it('throws on unknown Kind bytes', () => {
    const b = new FixtureBuilder()
    // Manually emit a node with kind 0x40 (reserved).
    b.emitNode({ parentIndex: 0, kind: 0x40, nodeData: 0 })
    b.pushRoot({ nodeIndex: 1, baseOffset: 0 })
    const sf = new RemoteSourceFileCtor(b.build())
    expect(() => sf.asts).toThrow(/unknown Kind/)
  })
})

describe('JsdocBlock + tags list metadata', () => {
  // Per-Kind Extended Data sizes / list-metadata slot offsets. Mirrors
  // `crates/ox_jsdoc/src/writer/nodes/comment_ast.rs`.
  const JSDOC_BLOCK_BASIC_SIZE = 68
  const JSDOC_BLOCK_TAGS_SLOT = 56
  const JSDOC_TAG_BASIC_SIZE = 38

  /** Pre-fill an Extended Data block with NONE StringFields. */
  function fillNoneStringFields(bytes: Uint8Array, startByte: number, count: number): void {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
    for (let i = 0; i < count; i++) {
      view.setUint32(startByte + i * 6, 0xffff_ffff, true) // offset = NONE
      view.setUint16(startByte + i * 6 + 4, 0, true) // length
    }
  }

  /**
   * Build a JsdocBlock with a single JsdocTag (no NodeList wrapper). The
   * tag is a direct sibling of the block's other children; its index is
   * patched into the block's `tags` list-metadata slot as `(head, count=1)`.
   * Mirrors the Rust visitor integration test post-format-change.
   */
  function buildBlockWithTag(): Uint8Array {
    const b = new FixtureBuilder()
    // -- JsdocBlock Extended Data (basic 68 bytes) -----------------------
    const blockExt = b.reserveExtended(JSDOC_BLOCK_BASIC_SIZE)
    const blockExtBytes = new Uint8Array(JSDOC_BLOCK_BASIC_SIZE)
    blockExtBytes[0] = 0b010 // bitmask: bit1 = tags (kept for visitor framework)
    fillNoneStringFields(blockExtBytes, 2, 8) // 8 StringFields, all NONE
    // List metadata slots stay zero (head=0, count=0); tags slot is patched
    // after we know the tag's NodeIndex.
    b.writeExtended(blockExt, blockExtBytes)
    const block = b.emitExtendedNode({
      parentIndex: 0,
      kind: 0x01,
      extOffset: blockExt,
      posStart: 0,
      posEnd: 25
    })
    // -- JsdocTag (parent = block directly, no NodeList wrapper) ---------
    const tagExt = b.reserveExtended(JSDOC_TAG_BASIC_SIZE)
    const tagExtBytes = new Uint8Array(JSDOC_TAG_BASIC_SIZE)
    tagExtBytes[0] = 0b0000_0001 // bit0 = tag-name child
    fillNoneStringFields(tagExtBytes, 2, 3) // 3 StringFields, all NONE
    b.writeExtended(tagExt, tagExtBytes)
    const tag = b.emitExtendedNode({
      parentIndex: block,
      kind: 0x03,
      extOffset: tagExt,
      posStart: 4,
      posEnd: 9
    })
    // Patch tags list metadata `(head=tag, count=1)` into block's ED.
    const tagsView = new DataView(blockExtBytes.buffer)
    tagsView.setUint32(JSDOC_BLOCK_TAGS_SLOT, tag, true)
    tagsView.setUint16(JSDOC_BLOCK_TAGS_SLOT + 4, 1, true)
    b.writeExtended(blockExt, blockExtBytes)
    // Tag-name child (visitor index 0, mandatory).
    b.emitStringNode({
      parentIndex: tag,
      kind: 0x04,
      value: 'param',
      posStart: 4,
      posEnd: 9
    })
    b.pushRoot({ nodeIndex: block, baseOffset: 0 })
    return b.build()
  }

  it('iterates tags via the per-list ED metadata slot', () => {
    const sf = new RemoteSourceFileCtor(buildBlockWithTag())
    const block = sf.asts[0] as RemoteJsdocBlockNode
    expect(block).toBeInstanceOf(RemoteJsdocBlock)
    expect(block.tags).toHaveLength(1)
    expect(block.descriptionLines).toHaveLength(0)
    expect(block.inlineTags).toHaveLength(0)

    const tag = block.tags[0]
    expect(tag.type).toBe('JsdocTag')
    expect(tag.tag).not.toBeNull()
    const tagName = tag.tag
    if (tagName === null) {
      throw new Error('expected JsdocTag.tag to be present')
    }
    expect(tagName.value).toBe('param')
    expect(tagName.parent).toBe(tag)
  })

  it('returns the same EMPTY_NODE_LIST singleton for empty arrays', () => {
    // Buffer with no children — all three list-metadata slots are
    // (head=0, count=0) → every list yields the EMPTY_NODE_LIST singleton.
    const b = new FixtureBuilder()
    const ext = b.reserveExtended(JSDOC_BLOCK_BASIC_SIZE)
    const bytes = new Uint8Array(JSDOC_BLOCK_BASIC_SIZE)
    fillNoneStringFields(bytes, 2, 8)
    b.writeExtended(ext, bytes)
    const block = b.emitExtendedNode({
      parentIndex: 0,
      kind: 0x01,
      extOffset: ext
    })
    b.pushRoot({ nodeIndex: block })
    const sf = new RemoteSourceFileCtor(b.build())
    const root = sf.asts[0] as RemoteJsdocBlockNode
    expect(root.tags).toBe(root.descriptionLines)
    expect(root.tags).toBe(root.inlineTags)
    expect(root.tags.length).toBe(0)
  })
})

describe('toJSON / Symbol.for("nodejs.util.inspect.custom")', () => {
  it('JsdocText.toJSON returns a plain object with type/range/value', () => {
    const b = new FixtureBuilder()
    const node = b.emitStringNode({
      parentIndex: 0,
      kind: 0x0f,
      value: 'hi',
      posStart: 0,
      posEnd: 2
    })
    b.pushRoot({ nodeIndex: node, baseOffset: 50 })
    const sf = new RemoteSourceFileCtor(b.build())
    const json = (sf.asts[0] as RemoteJsdocTextNode).toJSON()
    expect(json).toEqual({ type: 'JsdocText', range: [50, 52], value: 'hi' })
  })

  it('inspect-symbol payload sets the prototype to a same-named class', () => {
    const b = new FixtureBuilder()
    const node = b.emitStringNode({
      parentIndex: 0,
      kind: 0x0f,
      value: 'x',
      posStart: 0,
      posEnd: 1
    })
    b.pushRoot({ nodeIndex: node })
    const sf = new RemoteSourceFileCtor(b.build())
    const inspectSymbol = Symbol.for('nodejs.util.inspect.custom')
    const root = sf.asts[0] as RemoteJsdocTextNode
    const payload = (root as unknown as { [k: symbol]: () => object })[inspectSymbol]()
    expect(payload.constructor.name).toBe('JsdocText')
  })
})

interface RemoteSourceFileStrings {
  getStringByField(offset: number, length: number): string | null
  getStringByOffsetAndLength(offset: number, length: number): string
}

// These strings used to be cached by their offset only, so the string read
// first at an offset came back for every other length at that offset.
describe('RemoteSourceFile strings by offset and length', () => {
  const build = () => {
    const b = new FixtureBuilder()
    b.internString('foo the foo')
    return new RemoteSourceFileCtor(b.build()) as unknown as RemoteSourceFileStrings
  }

  it('keeps strings of different lengths at the same offset apart (StringField)', () => {
    const short = build()
    expect(short.getStringByField(0, 3)).toBe('foo')
    expect(short.getStringByField(0, 11)).toBe('foo the foo')
    expect(short.getStringByField(0, 0)).toBe('')

    const long = build()
    expect(long.getStringByField(0, 11)).toBe('foo the foo')
    expect(long.getStringByField(0, 0)).toBe('')
    expect(long.getStringByField(0, 3)).toBe('foo')
  })

  it('keeps strings of different lengths at the same offset apart (inline)', () => {
    const short = build()
    expect(short.getStringByOffsetAndLength(0, 3)).toBe('foo')
    expect(short.getStringByOffsetAndLength(0, 11)).toBe('foo the foo')

    const long = build()
    expect(long.getStringByOffsetAndLength(0, 11)).toBe('foo the foo')
    expect(long.getStringByOffsetAndLength(0, 3)).toBe('foo')
  })

  it('keeps long and short strings at the same offset apart', () => {
    const b = new FixtureBuilder()
    const text = 'x'.repeat(300)
    b.internString(text)
    const sf = new RemoteSourceFileCtor(b.build()) as unknown as RemoteSourceFileStrings
    expect(sf.getStringByField(0, 300)).toBe(text)
    expect(sf.getStringByField(0, 256)).toBe(text.slice(0, 256))
    expect(sf.getStringByField(0, 255)).toBe(text.slice(0, 255))
    expect(sf.getStringByOffsetAndLength(1, 0)).toBe('')
    expect(sf.getStringByField(0, 3)).toBe('xxx')
    expect(sf.getStringByField(0, 300)).toBe(text)
  })

  it('keeps them apart across the two kinds of reads', () => {
    const sf = build()
    expect(sf.getStringByField(0, 3)).toBe('foo')
    expect(sf.getStringByOffsetAndLength(0, 11)).toBe('foo the foo')
    expect(sf.getStringByOffsetAndLength(0, 3)).toBe('foo')
    expect(sf.getStringByField(0, 11)).toBe('foo the foo')
  })
})

describe('RemoteTypeTemplateLiteral.interpolations', () => {
  /**
   * A buffer whose root is a `TypeTemplateLiteral` (Kind 0x9D) with the given
   * literal segments and a `TypeName` (Kind 0x80) child for each name.
   */
  const build = (literals: string[], names: string[]) => {
    const b = new FixtureBuilder()
    const size = 2 + literals.length * 6
    const ext = b.reserveExtended(size)
    const bytes = new Uint8Array(size)
    const view = new DataView(bytes.buffer)
    view.setUint16(0, literals.length, true)
    for (const [i, literal] of literals.entries()) {
      const [start, end] = b.stringOffsets[b.internString(literal)]!
      view.setUint32(2 + i * 6, start, true)
      view.setUint16(2 + i * 6 + 4, end - start, true)
    }
    b.writeExtended(ext, bytes)
    const node = b.emitExtendedNode({ parentIndex: 0, kind: 0x9d, extOffset: ext })
    const children = names.map(value => b.emitStringNode({ parentIndex: node, kind: 0x80, value }))
    b.pushRoot({ nodeIndex: node })
    return { b, children }
  }
  const names = (sf: RemoteSourceFileInstance) =>
    (sf.asts[0] as RemoteTypeTemplateLiteral).interpolations.map(
      node => (node as unknown as RemoteTypeNameNode).value
    )

  it('reads the children between the literal segments', () => {
    const { b } = build(['a', 'c', ''], ['b', 'd'])
    const sf = new RemoteSourceFileCtor(b.build())
    expect((sf.asts[0] as RemoteTypeTemplateLiteral).literals).toEqual(['a', 'c', ''])
    expect(names(sf)).toEqual(['b', 'd'])
  })

  it('stops at the literal count when the sibling links loop', () => {
    // A malformed buffer: the child is its own next sibling.
    const { b, children } = build(['a', ''], ['b'])
    new DataView(b.nodes[children[0]!]!.buffer).setUint32(20, children[0]!, true)
    const sf = new RemoteSourceFileCtor(b.build())
    expect(names(sf)).toEqual(['b'])
  })
})

describe('RemoteTypeComputedProperty / RemoteTypeComputedMethod', () => {
  const nameOf = (node: unknown) => (node as RemoteTypeNameNode).value

  it('reads the value of a computed property', () => {
    // `[k]`: a TypeComputedProperty (Kind 0xAD) with a TypeName child.
    const b = new FixtureBuilder()
    const node = b.emitChildrenNode({ parentIndex: 0, kind: 0xad, bitmask: 0b1 })
    b.emitStringNode({ parentIndex: node, kind: 0x80, value: 'k' })
    b.pushRoot({ nodeIndex: node })
    const sf = new RemoteSourceFileCtor(b.build())
    const key = sf.asts[0] as RemoteTypeComputedProperty
    expect(key.type).toBe('TypeComputedProperty')
    expect(nameOf(key.value)).toBe('k')
  })

  it('reads the parts of a computed method from its children bitmask', () => {
    // `[k]?(): R`: a TypeComputedMethod (Kind 0xAE) with the value and the
    // return type, but no parameters or type parameters.
    const b = new FixtureBuilder()
    const node = b.emitChildrenNode({
      parentIndex: 0,
      kind: 0xae,
      commonData: 0b1,
      bitmask: 0b0101
    })
    b.emitStringNode({ parentIndex: node, kind: 0x80, value: 'k' })
    b.emitStringNode({ parentIndex: node, kind: 0x80, value: 'R' })
    b.pushRoot({ nodeIndex: node })
    const sf = new RemoteSourceFileCtor(b.build())
    const key = sf.asts[0] as RemoteTypeComputedMethod
    expect(key.type).toBe('TypeComputedMethod')
    expect(key.optional).toBe(true)
    expect(nameOf(key.value)).toBe('k')
    expect(key.parameters).toBeNull()
    expect(nameOf(key.returnType)).toBe('R')
    expect(key.typeParameters).toBeNull()
  })
})

describe('RemoteTypeBigInt', () => {
  it('reads the digits of a BigInt literal type', () => {
    // `-1n`: a TypeBigInt (Kind 0xAF) with the digits, without the `n`.
    const b = new FixtureBuilder()
    const node = b.emitStringNode({ parentIndex: 0, kind: 0xaf, value: '-1' })
    b.pushRoot({ nodeIndex: node })
    const bigint = new RemoteSourceFileCtor(b.build()).asts[0] as RemoteTypeBigInt
    expect(bigint.type).toBe('TypeBigInt')
    expect((bigint as unknown as RemoteTypeNameNode).value).toBe('-1')
    expect(bigint.toJSON()).toMatchObject({ type: 'TypeBigInt', value: '-1' })
  })
})

describe('RemoteTypeMethodSignature.optional', () => {
  /** `a(): R` or `a?(): R`: a TypeMethodSignature (Kind 0xA9) with its name. */
  const build = (commonData: number) => {
    const b = new FixtureBuilder()
    const bytes = new Uint8Array(6)
    const view = new DataView(bytes.buffer)
    const [start, end] = b.stringOffsets[b.internString('a')]!
    view.setUint32(0, start, true)
    view.setUint16(4, end - start, true)
    const ext = b.reserveExtended(bytes.length)
    b.writeExtended(ext, bytes)
    const node = b.emitExtendedNode({ parentIndex: 0, kind: 0xa9, commonData, extOffset: ext })
    b.emitStringNode({ parentIndex: node, kind: 0x80, value: 'R' })
    b.pushRoot({ nodeIndex: node })
    return new RemoteSourceFileCtor(b.build()).asts[0] as RemoteTypeMethodSignature
  }

  it('reads bit4 of the Common Data', () => {
    const method = build(0b1_0000)
    expect(method.name).toBe('a')
    expect(method.optional).toBe(true)
    expect(method.toJSON()).toMatchObject({ name: 'a', optional: true })
    expect(build(0).optional).toBe(false)
  })
})

describe('RemoteTypeTypeParameter', () => {
  const nameOf = (node: unknown) => (node as RemoteTypeNameNode | null)?.value ?? null

  /**
   * `<T = string>` or `<T extends string>`: a TypeTypeParameter (Kind 0xA6)
   * whose elements are the names `T` and `string`, with the given Common Data
   * and version byte.
   */
  const build = (commonData: number, version: number) => {
    const b = new FixtureBuilder()
    const ext = b.reserveExtended(6)
    const node = b.emitExtendedNode({ parentIndex: 0, kind: 0xa6, commonData, extOffset: ext })
    const head = b.emitStringNode({ parentIndex: node, kind: 0x80, value: 'T' })
    b.emitStringNode({ parentIndex: node, kind: 0x80, value: 'string' })
    const bytes = new Uint8Array(6)
    const view = new DataView(bytes.buffer)
    view.setUint32(0, head, true)
    view.setUint16(4, 2, true)
    b.writeExtended(ext, bytes)
    b.pushRoot({ nodeIndex: node })
    const buffer = b.build()
    buffer[0] = version
    return new RemoteSourceFileCtor(buffer).asts[0] as RemoteTypeTypeParameter
  }

  it('tells a default from a constraint by the Common Data of minor version 3', () => {
    // bit1 = a default
    const withDefault = build(0b10, 0x13)
    expect(withDefault.hasConstraint).toBe(false)
    expect(withDefault.hasDefault).toBe(true)
    expect(nameOf(withDefault.name)).toBe('T')
    expect(withDefault.constraint).toBeNull()
    expect(nameOf(withDefault.defaultValue)).toBe('string')
    expect(withDefault.toJSON()).toMatchObject({ hasConstraint: false, hasDefault: true })
    // bit0 = a constraint
    const withConstraint = build(0b01, 0x13)
    expect(withConstraint.hasConstraint).toBe(true)
    expect(withConstraint.hasDefault).toBe(false)
    expect(nameOf(withConstraint.constraint)).toBe('string')
    expect(withConstraint.defaultValue).toBeNull()
  })

  it('reads the elements of an older buffer by their order', () => {
    // Minor 2 has no flags; the second element was read as the constraint.
    const parameter = build(0, 0x12)
    expect(parameter.hasConstraint).toBe(true)
    expect(parameter.hasDefault).toBe(false)
    expect(nameOf(parameter.constraint)).toBe('string')
    expect(parameter.defaultValue).toBeNull()
  })
})

describe('RemoteJsdocInlineTag.tag', () => {
  /**
   * A buffer whose root is the `JsdocInlineTag` (Kind 0x08) of `{@link Foo}`,
   * with the name field after the other three, and the given version byte.
   */
  const build = (version: number) => {
    const b = new FixtureBuilder()
    const bytes = new Uint8Array(24)
    const view = new DataView(bytes.buffer)
    const field = (at: number, value: string | null) => {
      if (value === null) {
        view.setUint32(at, 0xffff_ffff, true)
        view.setUint16(at + 4, 0, true)
        return
      }
      const [start, end] = b.stringOffsets[b.internString(value)]!
      view.setUint32(at, start, true)
      view.setUint16(at + 4, end - start, true)
    }
    field(0, 'Foo')
    field(6, null)
    field(12, null)
    field(18, 'link')
    const ext = b.reserveExtended(bytes.length)
    b.writeExtended(ext, bytes)
    const node = b.emitExtendedNode({ parentIndex: 0, kind: 0x08, extOffset: ext })
    b.pushRoot({ nodeIndex: node })
    const buffer = b.build()
    buffer[0] = version
    return new RemoteSourceFileCtor(buffer).asts[0] as RemoteJsdocInlineTag
  }

  it('reads the name in a buffer of minor version 2', () => {
    const inline = build(0x12)
    expect(inline.tag).toBe('link')
    expect(inline.namepathOrURL).toBe('Foo')
    expect(inline.toJSON()).toMatchObject({ tag: 'link', namepathOrURL: 'Foo' })
  })

  it('gives no name in a buffer of minor version 0 or 1, which has no name field', () => {
    for (const version of [0x10, 0x11]) {
      const inline = build(version)
      expect(inline.tag).toBeNull()
      expect(inline.namepathOrURL).toBe('Foo')
    }
  })
})
