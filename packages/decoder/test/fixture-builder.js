/**
 * Hand-rolled Binary AST builder used only by `@ox-jsdoc/decoder` unit tests.
 *
 * Mirrors the layout produced by `crates/ox_jsdoc_binary`'s `BinaryWriter`
 * but only implements what the decoder tests need (single-byte ASCII
 * strings, no compat-mode tail, no extended-data padding edge cases).
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

const HEADER_SIZE = 40
const NODE_RECORD_SIZE = 24

const TYPE_TAG_CHILDREN = 0b00
const TYPE_TAG_STRING = 0b01
const TYPE_TAG_EXTENDED = 0b10

const NODE_LIST_KIND = 0x7f

/**
 * Pack `(typeTag, payload)` into a Node Data u32.
 *
 * @param {number} typeTag 0=Children, 1=String, 2=Extended
 * @param {number} payload 30-bit value
 */
function packNodeData(typeTag, payload) {
  return ((typeTag & 0b11) << 30) | (payload & 0x3fff_ffff)
}

function align4(value) {
  return (value + 3) & ~3
}

export class FixtureBuilder {
  constructor() {
    /** @type {boolean} */
    this.compatMode = false
    /** @type {Array<[number, number]>} */
    this.stringOffsets = []
    /** @type {Uint8Array} */
    this.stringData = new Uint8Array(0)
    /** @type {Map<string, number>} */
    this.stringIndex = new Map()
    /** @type {Uint8Array} */
    this.extendedData = new Uint8Array(0)
    /** @type {Array<{ rootIndex: number, messageIndex: number }>} */
    this.diagnostics = []
    /** @type {Array<Uint8Array>} */
    this.nodes = [new Uint8Array(NODE_RECORD_SIZE)] // sentinel node[0]
    /** @type {Array<{ nodeIndex: number, sourceOffset: number, baseOffset: number }>} */
    this.rootEntries = []
    /** @type {Map<number, number>} parent_index → last child node_index */
    this.lastChildOfParent = new Map()
    /** @type {number} */
    this.sourceTextLength = 0
  }

  /**
   * Intern a UTF-8 string and return its String Offsets index.
   *
   * @param {string} value
   * @returns {number}
   */
  internString(value) {
    const cached = this.stringIndex.get(value)
    if (cached !== undefined) {
      return cached
    }
    const idx = this.stringOffsets.length
    const bytes = new TextEncoder().encode(value)
    const start = this.stringData.length
    const end = start + bytes.length
    this.stringOffsets.push([start, end])
    const merged = new Uint8Array(end)
    merged.set(this.stringData)
    merged.set(bytes, start)
    this.stringData = merged
    this.stringIndex.set(value, idx)
    return idx
  }

  /**
   * Reserve N bytes of Extended Data and return the byte offset within the
   * Extended Data section. Pads the final extended buffer to 8-byte
   * alignment when subsequent records are written.
   *
   * @param {number} byteSize
   * @returns {number}
   */
  reserveExtended(byteSize) {
    const start = this.extendedData.length
    const merged = new Uint8Array(start + byteSize)
    merged.set(this.extendedData)
    this.extendedData = merged
    return start
  }

  /**
   * Write into a previously-reserved Extended Data slot.
   *
   * @param {number} offset
   * @param {Uint8Array} bytes
   */
  writeExtended(offset, bytes) {
    this.extendedData.set(bytes, offset)
  }

  /**
   * Emit a node record.
   *
   * @param {object} args
   * @param {number} args.parentIndex
   * @param {number} args.kind
   * @param {number} [args.commonData] 6-bit value
   * @param {number} [args.posStart]
   * @param {number} [args.posEnd]
   * @param {number} args.nodeData Pre-packed Node Data u32.
   * @returns {number} Newly-assigned node_index.
   */
  emitNode({ parentIndex, kind, commonData = 0, posStart = 0, posEnd = 0, nodeData }) {
    const newIndex = this.nodes.length
    const buf = new Uint8Array(NODE_RECORD_SIZE)
    const view = new DataView(buf.buffer)
    view.setUint8(0, kind & 0xff)
    view.setUint8(1, commonData & 0x3f)
    view.setUint32(4, posStart, true)
    view.setUint32(8, posEnd, true)
    view.setUint32(12, nodeData >>> 0, true)
    view.setUint32(16, parentIndex, true)
    view.setUint32(20, 0, true)
    this.nodes.push(buf)

    const prev = this.lastChildOfParent.get(parentIndex)
    if (prev !== undefined) {
      const prevBuf = this.nodes[prev]
      new DataView(prevBuf.buffer).setUint32(20, newIndex, true)
    }
    this.lastChildOfParent.set(parentIndex, newIndex)
    return newIndex
  }

  /**
   * Emit a String-type leaf node.
   *
   * @param {object} args
   * @param {number} args.parentIndex
   * @param {number} args.kind
   * @param {string} args.value
   * @param {number} [args.commonData]
   * @param {number} [args.posStart]
   * @param {number} [args.posEnd]
   */
  emitStringNode(args) {
    const stringIdx = this.internString(args.value)
    return this.emitNode({
      parentIndex: args.parentIndex,
      kind: args.kind,
      commonData: args.commonData ?? 0,
      posStart: args.posStart ?? 0,
      posEnd: args.posEnd ?? 0,
      nodeData: packNodeData(TYPE_TAG_STRING, stringIdx)
    })
  }

  /**
   * Emit a Children-type node (bitmask in payload, no Extended Data).
   */
  emitChildrenNode(args) {
    return this.emitNode({
      parentIndex: args.parentIndex,
      kind: args.kind,
      commonData: args.commonData ?? 0,
      posStart: args.posStart ?? 0,
      posEnd: args.posEnd ?? 0,
      nodeData: packNodeData(TYPE_TAG_CHILDREN, args.bitmask ?? 0)
    })
  }

  /**
   * Emit a NodeList wrapper (Kind 0x7F).
   *
   * @param {object} args
   * @param {number} args.parentIndex
   * @param {number} [args.elementCount]
   * @param {number} [args.posStart]
   * @param {number} [args.posEnd]
   */
  emitNodeList(args) {
    return this.emitNode({
      parentIndex: args.parentIndex,
      kind: NODE_LIST_KIND,
      posStart: args.posStart ?? 0,
      posEnd: args.posEnd ?? 0,
      nodeData: packNodeData(TYPE_TAG_CHILDREN, args.elementCount ?? 0)
    })
  }

  /**
   * Emit an Extended-type node. The caller pre-reserves the Extended Data
   * record via {@link reserveExtended} and passes its offset.
   */
  emitExtendedNode(args) {
    return this.emitNode({
      parentIndex: args.parentIndex,
      kind: args.kind,
      commonData: args.commonData ?? 0,
      posStart: args.posStart ?? 0,
      posEnd: args.posEnd ?? 0,
      nodeData: packNodeData(TYPE_TAG_EXTENDED, args.extOffset)
    })
  }

  /**
   * Add a Root index entry.
   */
  pushRoot({ nodeIndex, sourceOffset = 0, baseOffset = 0 }) {
    this.rootEntries.push({ nodeIndex, sourceOffset, baseOffset })
  }

  /**
   * Set the `sourceTextLength` Header field.
   *
   * @param {number} length
   */
  setSourceTextLength(length) {
    this.sourceTextLength = length
  }

  /**
   * Materialize the buffer.
   *
   * @returns {Uint8Array}
   */
  build() {
    // Section boundaries are padded to 4-byte alignment so the decoder's
    // `Uint32Array` view can read u32 fields with a single MOV. Mirrors
    // `crates/ox_jsdoc_binary/src/writer/binary_writer.rs`'s `align_up_4`
    // calls.
    const rootArraySize = this.rootEntries.length * 12
    const stringOffsetsSize = this.stringOffsets.length * 8
    const stringDataSize = this.stringData.length
    const extendedDataSize = this.extendedData.length
    const diagnosticsSize = 4 + this.diagnostics.length * 8

    const rootArrayOffset = HEADER_SIZE
    const stringOffsetsOffset = rootArrayOffset + rootArraySize
    const stringDataOffset = stringOffsetsOffset + stringOffsetsSize
    const extendedDataOffset = align4(stringDataOffset + stringDataSize)
    const diagnosticsOffset = align4(extendedDataOffset + extendedDataSize)
    const nodesOffset = align4(diagnosticsOffset + diagnosticsSize)
    const nodesSize = this.nodes.length * NODE_RECORD_SIZE
    const total = nodesOffset + nodesSize

    const out = new Uint8Array(total)
    const view = new DataView(out.buffer)

    // Header (40 bytes)
    view.setUint8(0, 0x10) // version 1.0
    view.setUint8(1, this.compatMode ? 0x01 : 0)
    view.setUint32(4, rootArrayOffset, true)
    view.setUint32(8, stringOffsetsOffset, true)
    view.setUint32(12, stringDataOffset, true)
    view.setUint32(16, extendedDataOffset, true)
    view.setUint32(20, diagnosticsOffset, true)
    view.setUint32(24, nodesOffset, true)
    view.setUint32(28, this.nodes.length, true)
    view.setUint32(32, this.sourceTextLength, true)
    view.setUint32(36, this.rootEntries.length, true)

    let cursor = HEADER_SIZE

    // Root index array
    for (const r of this.rootEntries) {
      view.setUint32(cursor, r.nodeIndex, true)
      view.setUint32(cursor + 4, r.sourceOffset, true)
      view.setUint32(cursor + 8, r.baseOffset, true)
      cursor += 12
    }

    // String offsets
    for (const [start, end] of this.stringOffsets) {
      view.setUint32(cursor, start, true)
      view.setUint32(cursor + 4, end, true)
      cursor += 8
    }

    // String data + alignment pad
    out.set(this.stringData, cursor)
    cursor = extendedDataOffset

    // Extended data + alignment pad
    out.set(this.extendedData, cursor)
    cursor = diagnosticsOffset

    // Diagnostics
    view.setUint32(cursor, this.diagnostics.length, true)
    cursor += 4
    for (const d of this.diagnostics) {
      view.setUint32(cursor, d.rootIndex, true)
      view.setUint32(cursor + 4, d.messageIndex, true)
      cursor += 8
    }

    // Nodes (preceded by alignment pad to nodesOffset)
    cursor = nodesOffset
    for (const node of this.nodes) {
      out.set(node, cursor)
      cursor += NODE_RECORD_SIZE
    }

    return out
  }
}

// Re-export packing helper for tests that hand-build Node Data values.
export { packNodeData, TYPE_TAG_CHILDREN, TYPE_TAG_EXTENDED, TYPE_TAG_STRING }
