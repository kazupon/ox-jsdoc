import { describe, expect, it } from 'vite-plus/test'

import {
  RemoteJsdocBlock,
  RemoteJsdocText,
  RemoteSourceFile,
  RemoteTypeName
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
  // `crates/ox_jsdoc_binary/src/writer/nodes/comment_ast.rs`.
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
