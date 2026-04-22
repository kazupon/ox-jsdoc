/**
 * Low-level helpers shared by every Remote* class.
 *
 * Mirrors `crates/ox_jsdoc_binary/src/decoder/helpers.rs`.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import {
  END_OFFSET,
  NEXT_SIBLING_OFFSET,
  NODE_DATA_OFFSET,
  NODE_RECORD_SIZE,
  PAYLOAD_MASK,
  POS_OFFSET,
  STRING_PAYLOAD_NONE_SENTINEL,
  TYPE_TAG_EXTENDED,
  TYPE_TAG_SHIFT
} from './constants.js'

/**
 * @typedef {object} RemoteInternal
 * @property {DataView} view
 * @property {number} byteIndex     Byte offset of this node record (= nodesOffset + index*24).
 * @property {number} index         Node index (0 = sentinel).
 * @property {number} rootIndex     Index of the root this node belongs to (used for absolute range).
 * @property {object | null} parent The parent Remote* instance (null for roots).
 * @property {import('./source-file.js').RemoteSourceFile} sourceFile
 */

/**
 * Read a 4-byte aligned u32 from the source file's `Uint32Array` view.
 *
 * 5–10× faster than `DataView.getUint32` because the typed-array element
 * load compiles to a single CPU instruction in V8's TurboFan, whereas
 * `getUint32` goes through a runtime stub. Caller MUST guarantee
 * `byteOffset` is 4-byte aligned (writer pads section boundaries to keep
 * every node record's u32 fields aligned).
 *
 * @param {import('./source-file.js').RemoteSourceFile} sourceFile
 * @param {number} byteOffset 4-byte aligned absolute offset within the buffer.
 * @returns {number}
 */
function readU32Aligned(sourceFile, byteOffset) {
  return sourceFile.uint32View[byteOffset >>> 2]
}

/**
 * Resolve the Extended Data byte offset for a node.
 *
 * Throws if the node's TypeTag is not `Extended` (matches the Rust
 * `debug_assert!`). Used by classes whose Extended Data carries the
 * Children bitmask + per-kind fields.
 *
 * @param {RemoteInternal} internal
 * @returns {number}
 */
export function extOffsetOf(internal) {
  const { byteIndex, sourceFile } = internal
  const nodeData = readU32Aligned(sourceFile, byteIndex + NODE_DATA_OFFSET)
  const typeTag = (nodeData >>> TYPE_TAG_SHIFT) & 0b11
  if (typeTag !== TYPE_TAG_EXTENDED) {
    throw new Error(
      `Node at index ${internal.index} is not Extended type (got tag 0b${typeTag.toString(2)})`
    )
  }
  return sourceFile.extendedDataOffset + (nodeData & PAYLOAD_MASK)
}

/**
 * Read the 30-bit String payload of a String-type node, returning the
 * resolved string or `null` if the writer used the None sentinel. Used by
 * string-leaf Kinds whose Node Data carries a `TypeTag::String` payload.
 *
 * @param {RemoteInternal} internal
 * @returns {string | null}
 */
export function stringPayloadOf(internal) {
  const { byteIndex, sourceFile } = internal
  const nodeData = readU32Aligned(sourceFile, byteIndex + NODE_DATA_OFFSET)
  const payload = nodeData & PAYLOAD_MASK
  if (payload === STRING_PAYLOAD_NONE_SENTINEL) {
    return null
  }
  return sourceFile.getString(payload)
}

/**
 * Resolve the leading `StringField` (6 bytes at offset 0 of the record)
 * of an Extended-type node whose record begins with a StringField slot
 * (Pattern 3 TypeNodes such as `TypeKeyValue.key`, `TypeMethodSignature.name`,
 * `TypeSymbol.value`).
 *
 * Returns `""` when the field equals the NONE sentinel.
 *
 * @param {RemoteInternal} internal
 * @returns {string}
 */
export function extStringLeaf(internal) {
  return extStringFieldRequired(internal, 0)
}

/**
 * Read the 30-bit Children bitmask payload of a Children-type node.
 *
 * @param {RemoteInternal} internal
 * @returns {number}
 */
export function childrenBitmaskPayloadOf(internal) {
  const { byteIndex, sourceFile } = internal
  return readU32Aligned(sourceFile, byteIndex + NODE_DATA_OFFSET) & PAYLOAD_MASK
}

/**
 * Read the `next_sibling` field for the given node index.
 *
 * @param {import('./source-file.js').RemoteSourceFile} sourceFile
 * @param {number} nodeIndex
 * @returns {number}
 */
export function readNextSibling(sourceFile, nodeIndex) {
  const off = sourceFile.nodesOffset + nodeIndex * NODE_RECORD_SIZE + NEXT_SIBLING_OFFSET
  return readU32Aligned(sourceFile, off)
}

/**
 * Return the first child of the parent at `parentIndex` (= `parentIndex + 1`
 * if its `parent_index` field equals `parentIndex`). Returns `0` when the
 * parent has no child.
 *
 * @param {import('./source-file.js').RemoteSourceFile} sourceFile
 * @param {number} parentIndex
 * @returns {number}
 */
export function firstChildIndex(sourceFile, parentIndex) {
  const candidate = parentIndex + 1
  if (candidate >= sourceFile.nodeCount) {
    return 0
  }
  const off = sourceFile.nodesOffset + candidate * NODE_RECORD_SIZE + /* PARENT_INDEX_OFFSET */ 16
  if (readU32Aligned(sourceFile, off) !== parentIndex) {
    return 0
  }
  return candidate
}

/**
 * Find the `visitorIndex`-th set bit in `bitmask` and return the
 * corresponding child node index. Returns `0` when the slot is unset
 * or the sibling chain is truncated.
 *
 * @param {RemoteInternal} internal
 * @param {number} bitmask     The Children bitmask (8 bits).
 * @param {number} visitorIndex
 * @returns {number}
 */
export function childIndexAtVisitorIndex(internal, bitmask, visitorIndex) {
  if ((bitmask & (1 << visitorIndex)) === 0) {
    return 0
  }
  const maskBelow = (1 << visitorIndex) - 1
  const skip = popcount(bitmask & maskBelow)

  let child = internal.index + 1
  for (let i = 0; i < skip; i++) {
    const next = readNextSibling(internal.sourceFile, child)
    if (next === 0) {
      return 0
    }
    child = next
  }
  return child
}

/**
 * Build a Remote* instance for the child at `visitorIndex` under the parent
 * described by `internal`. Reads the parent's bitmask from Extended Data
 * (so the parent must be Extended type).
 *
 * @template T
 * @param {RemoteInternal} internal
 * @param {number} visitorIndex
 * @returns {object | null}
 */
export function childNodeAtVisitorIndex(internal, visitorIndex) {
  const bitmask = internal.view.getUint8(extOffsetOf(internal))
  const childIdx = childIndexAtVisitorIndex(internal, bitmask, visitorIndex)
  if (childIdx === 0) {
    return null
  }
  return internal.sourceFile.getNode(childIdx, /* parent */ thisNode(internal), internal.rootIndex)
}

/**
 * Same as `childNodeAtVisitorIndex` but reads the bitmask from the 30-bit
 * Node Data payload (Children-type parents).
 *
 * @param {RemoteInternal} internal
 * @param {number} visitorIndex
 * @returns {object | null}
 */
export function childNodeAtVisitorIndexChildren(internal, visitorIndex) {
  const bitmask = childrenBitmaskPayloadOf(internal) & 0xff
  const childIdx = childIndexAtVisitorIndex(internal, bitmask, visitorIndex)
  if (childIdx === 0) {
    return null
  }
  return internal.sourceFile.getNode(childIdx, thisNode(internal), internal.rootIndex)
}

/**
 * Resolve an Optional `StringField` slot at `fieldOffset` inside this
 * node's Extended Data record (`null` when the slot equals the NONE
 * sentinel).
 *
 * The 6-byte slot is read as `(offset: u32 LE, length: u16 LE)` and then
 * passed to {@link import('./source-file.js').RemoteSourceFile.getStringByField}.
 *
 * @param {RemoteInternal} internal
 * @param {number} fieldOffset Byte offset within the Extended Data record.
 * @returns {string | null}
 */
export function extStringField(internal, fieldOffset) {
  const ext = extOffsetOf(internal) + fieldOffset
  const offset = internal.view.getUint32(ext, true)
  const length = internal.view.getUint16(ext + 4, true)
  return internal.sourceFile.getStringByField(offset, length)
}

/**
 * Resolve a Required `StringField` slot at `fieldOffset` (returns `""` for
 * the NONE sentinel).
 *
 * @param {RemoteInternal} internal
 * @param {number} fieldOffset
 * @returns {string}
 */
export function extStringFieldRequired(internal, fieldOffset) {
  const ext = extOffsetOf(internal) + fieldOffset
  const offset = internal.view.getUint32(ext, true)
  const length = internal.view.getUint16(ext + 4, true)
  return internal.sourceFile.getStringByField(offset, length) ?? ''
}

/**
 * Compute the absolute `[start, end]` range of a node by adding the root's
 * `base_offset` to the relative Pos/End fields.
 *
 * @param {RemoteInternal} internal
 * @returns {[number, number]}
 */
export function absoluteRange(internal) {
  const { byteIndex, rootIndex, sourceFile } = internal
  const pos = readU32Aligned(sourceFile, byteIndex + POS_OFFSET)
  const end = readU32Aligned(sourceFile, byteIndex + END_OFFSET)
  const baseOffset = sourceFile.getRootBaseOffset(rootIndex)
  return [baseOffset + pos, baseOffset + end]
}

/**
 * Look up the lazy node instance described by `internal` (used as the
 * `parent` argument when constructing children). Goes through the
 * sourceFile's nodeCache to keep instances stable.
 *
 * @param {RemoteInternal} internal
 * @returns {object}
 */
export function thisNode(internal) {
  return internal.sourceFile.getNode(internal.index, internal.parent, internal.rootIndex)
}

/**
 * Population count for a u8.
 *
 * @param {number} byte
 * @returns {number}
 */
function popcount(byte) {
  let n = byte & 0xff
  n -= (n >> 1) & 0x55
  n = (n & 0x33) + ((n >> 2) & 0x33)
  return (n + (n >> 4)) & 0x0f
}
