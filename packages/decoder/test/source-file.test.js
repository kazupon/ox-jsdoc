// @ts-check

import { describe, expect, it } from 'vite-plus/test'

import {
  RemoteJsdocBlock,
  RemoteJsdocText,
  RemoteSourceFile,
  RemoteTypeName
} from '../src/index.js'
import { FixtureBuilder } from './fixture-builder.js'

describe('RemoteSourceFile.constructor', () => {
  it('parses Header offsets and counts', () => {
    const b = new FixtureBuilder()
    b.setSourceTextLength(42)
    const sf = new RemoteSourceFile(b.build())
    expect(sf.compatMode).toBe(false)
    expect(sf.nodeCount).toBe(1) // sentinel only
    expect(sf.rootCount).toBe(0)
    expect(sf.nodesOffset).toBeGreaterThanOrEqual(40)
  })

  it('reads compat_mode flag bit', () => {
    const b = new FixtureBuilder()
    b.compatMode = true
    const sf = new RemoteSourceFile(b.build())
    expect(sf.compatMode).toBe(true)
  })

  it('rejects buffers shorter than the Header', () => {
    expect(() => new RemoteSourceFile(new Uint8Array(10))).toThrow(/too short/)
  })

  it('rejects incompatible major versions', () => {
    const b = new FixtureBuilder()
    const buf = b.build()
    buf[0] = 0xf0 // major 15
    expect(() => new RemoteSourceFile(buf)).toThrow(/incompatible/)
  })
})

describe('RemoteSourceFile.getString', () => {
  it('resolves interned strings via the offsets table', () => {
    const b = new FixtureBuilder()
    const idx = b.internString('hello world')
    const sf = new RemoteSourceFile(b.build())
    expect(sf.getString(idx)).toBe('hello world')
  })

  it('returns null for the u16 None sentinel', () => {
    const b = new FixtureBuilder()
    const sf = new RemoteSourceFile(b.build())
    expect(sf.getString(0xffff)).toBeNull()
  })

  it('returns null for the 30-bit None sentinel', () => {
    const b = new FixtureBuilder()
    const sf = new RemoteSourceFile(b.build())
    expect(sf.getString(0x3fff_ffff)).toBeNull()
  })

  it('caches resolved strings on repeat lookups', () => {
    const b = new FixtureBuilder()
    const idx = b.internString('cached')
    const sf = new RemoteSourceFile(b.build())
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
    const sf = new RemoteSourceFile(b.build())
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
    const sf = new RemoteSourceFile(b.build())
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
    const sf = new RemoteSourceFile(b.build())
    const text = /** @type {RemoteJsdocText} */ (sf.asts[0])
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
    const sf = new RemoteSourceFile(b.build())
    const tn = /** @type {RemoteTypeName} */ (sf.asts[0])
    expect(tn).toBeInstanceOf(RemoteTypeName)
    expect(tn.value).toBe('Foo')
  })

  it('throws on unknown Kind bytes', () => {
    const b = new FixtureBuilder()
    // Manually emit a node with kind 0x40 (reserved).
    b.emitNode({ parentIndex: 0, kind: 0x40, nodeData: 0 })
    b.pushRoot({ nodeIndex: 1, baseOffset: 0 })
    const sf = new RemoteSourceFile(b.build())
    expect(() => sf.asts).toThrow(/unknown Kind/)
  })
})

describe('JsdocBlock + tags NodeList', () => {
  /**
   * Build a JsdocBlock containing a 1-element tags NodeList wrapping a
   * minimal JsdocTag (only the mandatory tag-name child). Mirrors the
   * Rust visitor integration test.
   */
  function buildBlockWithTag() {
    const b = new FixtureBuilder()
    // Reserve JsdocBlock Extended Data record (basic 18 bytes).
    const blockExt = b.reserveExtended(18)
    const blockExtBytes = new Uint8Array(18)
    new DataView(blockExtBytes.buffer).setUint8(0, 0b010) // bit1 = tags
    // Required string fields default to None (0xFFFF).
    new DataView(blockExtBytes.buffer).setUint16(2, 0xffff, true) // description
    new DataView(blockExtBytes.buffer).setUint16(4, 0xffff, true) // delimiter
    new DataView(blockExtBytes.buffer).setUint16(6, 0xffff, true) // post_delimiter
    new DataView(blockExtBytes.buffer).setUint16(8, 0xffff, true) // terminal
    new DataView(blockExtBytes.buffer).setUint16(10, 0xffff, true) // line_end
    new DataView(blockExtBytes.buffer).setUint16(12, 0xffff, true) // initial
    new DataView(blockExtBytes.buffer).setUint16(14, 0xffff, true) // delimiter_line_break
    new DataView(blockExtBytes.buffer).setUint16(16, 0xffff, true) // preterminal_line_break
    b.writeExtended(blockExt, blockExtBytes)
    const block = b.emitExtendedNode({
      parentIndex: 0,
      kind: 0x01,
      extOffset: blockExt,
      posStart: 0,
      posEnd: 25
    })
    // tags NodeList wrapper, parent=block.
    const tagsList = b.emitNodeList({ parentIndex: block, elementCount: 1 })
    // JsdocTag with bit0 (tag-name only) bitmask.
    const tagExt = b.reserveExtended(8)
    const tagExtBytes = new Uint8Array(8)
    new DataView(tagExtBytes.buffer).setUint8(0, 0b0000_0001) // bit0 only
    new DataView(tagExtBytes.buffer).setUint16(2, 0xffff, true) // default_value
    new DataView(tagExtBytes.buffer).setUint16(4, 0xffff, true) // description
    new DataView(tagExtBytes.buffer).setUint16(6, 0xffff, true) // raw_body
    b.writeExtended(tagExt, tagExtBytes)
    const tag = b.emitExtendedNode({
      parentIndex: tagsList,
      kind: 0x03,
      extOffset: tagExt,
      posStart: 4,
      posEnd: 9
    })
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

  it('iterates tags via the NodeList wrapper', () => {
    const sf = new RemoteSourceFile(buildBlockWithTag())
    const block = /** @type {RemoteJsdocBlock} */ (sf.asts[0])
    expect(block).toBeInstanceOf(RemoteJsdocBlock)
    expect(block.tags).toHaveLength(1)
    expect(block.descriptionLines).toHaveLength(0)
    expect(block.inlineTags).toHaveLength(0)

    const tag = block.tags[0]
    expect(tag.type).toBe('JsdocTag')
    expect(tag.tag).not.toBeNull()
    expect(tag.tag.value).toBe('param')
    expect(tag.tag.parent).toBe(tag)
  })

  it('returns the same EMPTY_NODE_LIST singleton for empty arrays', () => {
    // Buffer with no children — descriptionLines, tags, inlineTags all empty.
    const b = new FixtureBuilder()
    const ext = b.reserveExtended(18)
    // Bitmask = 0 → all three NodeList slots unset.
    b.writeExtended(ext, new Uint8Array(18))
    const block = b.emitExtendedNode({
      parentIndex: 0,
      kind: 0x01,
      extOffset: ext
    })
    b.pushRoot({ nodeIndex: block })
    const sf = new RemoteSourceFile(b.build())
    const root = /** @type {RemoteJsdocBlock} */ (sf.asts[0])
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
    const sf = new RemoteSourceFile(b.build())
    const json = /** @type {RemoteJsdocText} */ (sf.asts[0]).toJSON()
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
    const sf = new RemoteSourceFile(b.build())
    const inspectSymbol = Symbol.for('nodejs.util.inspect.custom')
    const root = /** @type {RemoteJsdocText} */ (sf.asts[0])
    const payload = /** @type {{ [k: symbol]: () => object }} */ (/** @type {unknown} */ (root))[
      inspectSymbol
    ]()
    expect(payload.constructor.name).toBe('JsdocText')
  })
})
