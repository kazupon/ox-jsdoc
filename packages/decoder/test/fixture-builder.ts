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

const HEADER_SIZE = 40
const NODE_RECORD_SIZE = 24

const TYPE_TAG_CHILDREN = 0b00
const TYPE_TAG_STRING = 0b01
const TYPE_TAG_EXTENDED = 0b10

const NODE_LIST_KIND = 0x7f

type NodeDataTypeTag = typeof TYPE_TAG_CHILDREN | typeof TYPE_TAG_STRING | typeof TYPE_TAG_EXTENDED

interface EmitNodeArgs {
  parentIndex: number
  kind: number
  commonData?: number
  posStart?: number
  posEnd?: number
  nodeData: number
}

interface EmitStringNodeArgs {
  parentIndex: number
  kind: number
  value: string
  commonData?: number
  posStart?: number
  posEnd?: number
}

interface EmitChildrenNodeArgs {
  parentIndex: number
  kind: number
  commonData?: number
  posStart?: number
  posEnd?: number
  bitmask?: number
}

interface EmitNodeListArgs {
  parentIndex: number
  elementCount?: number
  posStart?: number
  posEnd?: number
}

interface EmitExtendedNodeArgs {
  parentIndex: number
  kind: number
  commonData?: number
  posStart?: number
  posEnd?: number
  extOffset: number
}

interface RootEntry {
  nodeIndex: number
  sourceOffset?: number
  baseOffset?: number
}

/**
 * Pack `(typeTag, payload)` into a Node Data u32.
 */
function packNodeData(typeTag: NodeDataTypeTag, payload: number): number {
  return ((typeTag & 0b11) << 30) | (payload & 0x3fff_ffff)
}

function align4(value: number): number {
  return (value + 3) & ~3
}

export class FixtureBuilder {
  compatMode: boolean
  stringOffsets: Array<[number, number]>
  stringData: Uint8Array
  stringIndex: Map<string, number>
  extendedData: Uint8Array
  diagnostics: Array<{ rootIndex: number; messageIndex: number }>
  nodes: Uint8Array[]
  rootEntries: Array<Required<RootEntry>>
  lastChildOfParent: Map<number, number>
  sourceTextLength: number

  constructor() {
    this.compatMode = false
    this.stringOffsets = []
    this.stringData = new Uint8Array(0)
    this.stringIndex = new Map()
    this.extendedData = new Uint8Array(0)
    this.diagnostics = []
    this.nodes = [new Uint8Array(NODE_RECORD_SIZE)] // sentinel node[0]
    this.rootEntries = []
    this.lastChildOfParent = new Map()
    this.sourceTextLength = 0
  }

  /**
   * Intern a UTF-8 string and return its String Offsets index.
   *
   */
  internString(value: string): number {
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
   */
  reserveExtended(byteSize: number): number {
    const start = this.extendedData.length
    const merged = new Uint8Array(start + byteSize)
    merged.set(this.extendedData)
    this.extendedData = merged
    return start
  }

  /**
   * Write into a previously-reserved Extended Data slot.
   *
   */
  writeExtended(offset: number, bytes: Uint8Array): void {
    this.extendedData.set(bytes, offset)
  }

  /**
   * Emit a node record.
   *
   */
  emitNode({
    parentIndex,
    kind,
    commonData = 0,
    posStart = 0,
    posEnd = 0,
    nodeData
  }: EmitNodeArgs): number {
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
   */
  emitStringNode(args: EmitStringNodeArgs): number {
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
  emitChildrenNode(args: EmitChildrenNodeArgs): number {
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
   */
  emitNodeList(args: EmitNodeListArgs): number {
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
  emitExtendedNode(args: EmitExtendedNodeArgs): number {
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
  pushRoot({ nodeIndex, sourceOffset = 0, baseOffset = 0 }: RootEntry): void {
    this.rootEntries.push({ nodeIndex, sourceOffset, baseOffset })
  }

  /**
   * Set the `sourceTextLength` Header field.
   *
   */
  setSourceTextLength(length: number): void {
    this.sourceTextLength = length
  }

  /**
   * Materialize the buffer.
   *
   */
  build(): Uint8Array {
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
