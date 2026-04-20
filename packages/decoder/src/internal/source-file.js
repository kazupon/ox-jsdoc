/**
 * `RemoteSourceFile` — root of the JS lazy decoder.
 *
 * Mirrors the Rust `LazySourceFile`: parses the 40-byte Header at
 * construction so every Remote* instance can reach the String table /
 * Root array / Nodes section in O(1).
 *
 * Per js-decoder.md, all per-instance state lives in a single `#internal`
 * object (V8 hidden-class friendly), and this is the only class that
 * actually allocates caches (stringCache, nodeCache).
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import { decodeKindToClass } from './kind-dispatch.js'
import {
  BASE_OFFSET_FIELD,
  COMPAT_MODE_BIT,
  DIAGNOSTICS_OFFSET_FIELD,
  EXTENDED_DATA_OFFSET_FIELD,
  FLAGS_OFFSET,
  HEADER_SIZE,
  KIND_OFFSET,
  MAJOR_SHIFT,
  NODE_COUNT_FIELD,
  NODE_INDEX_OFFSET,
  NODES_OFFSET_FIELD,
  NODE_RECORD_SIZE,
  ROOT_ARRAY_OFFSET_FIELD,
  ROOT_COUNT_FIELD,
  ROOT_INDEX_ENTRY_SIZE,
  SOURCE_TEXT_LENGTH_FIELD,
  STRING_DATA_OFFSET_FIELD,
  STRING_OFFSET_ENTRY_SIZE,
  STRING_OFFSETS_OFFSET_FIELD,
  STRING_PAYLOAD_NONE_SENTINEL,
  SUPPORTED_MAJOR,
  U16_NONE_SENTINEL,
  VERSION_OFFSET
} from './constants.js'

const utf8Decoder = new TextDecoder('utf-8')

/**
 * Root of the lazy decoder. Construct one per Binary AST buffer.
 *
 * Public surface (used by Remote* node classes):
 * - `view`, `extendedDataOffset`, `nodesOffset`, `nodeCount`, `rootCount`,
 *   `compatMode` getters
 * - `getString(idx)` — String Offsets[idx] → resolved string (cached)
 * - `getRootBaseOffset(rootIndex)`
 * - `getNode(nodeIndex, parent, rootIndex)` — lazy class instance (cached)
 * - `asts` getter — array of root Remote* instances (or `null` for failures)
 */
export class RemoteSourceFile {
  #internal

  /**
   * Construct from a binary buffer (any `BufferSource`-compatible value).
   *
   * Throws when:
   * - the buffer is shorter than {@link HEADER_SIZE} bytes
   * - the major version disagrees with {@link SUPPORTED_MAJOR}
   *
   * @param {ArrayBuffer | ArrayBufferView} buffer
   */
  constructor(buffer) {
    const view =
      buffer instanceof ArrayBuffer
        ? new DataView(buffer)
        : new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength)
    if (view.byteLength < HEADER_SIZE) {
      throw new Error(`buffer too short: ${view.byteLength} bytes (need at least ${HEADER_SIZE})`)
    }
    const versionByte = view.getUint8(VERSION_OFFSET)
    const major = versionByte >>> MAJOR_SHIFT
    if (major !== SUPPORTED_MAJOR) {
      throw new Error(
        `incompatible Binary AST major version: buffer=${major}, decoder=${SUPPORTED_MAJOR}`
      )
    }
    const flags = view.getUint8(FLAGS_OFFSET)
    const nodeCount = view.getUint32(NODE_COUNT_FIELD, true)
    this.#internal = {
      view,
      version: versionByte,
      compatMode: (flags & COMPAT_MODE_BIT) !== 0,
      rootArrayOffset: view.getUint32(ROOT_ARRAY_OFFSET_FIELD, true),
      stringOffsetsOffset: view.getUint32(STRING_OFFSETS_OFFSET_FIELD, true),
      stringDataOffset: view.getUint32(STRING_DATA_OFFSET_FIELD, true),
      extendedDataOffset: view.getUint32(EXTENDED_DATA_OFFSET_FIELD, true),
      diagnosticsOffset: view.getUint32(DIAGNOSTICS_OFFSET_FIELD, true),
      nodesOffset: view.getUint32(NODES_OFFSET_FIELD, true),
      nodeCount,
      sourceTextLength: view.getUint32(SOURCE_TEXT_LENGTH_FIELD, true),
      rootCount: view.getUint32(ROOT_COUNT_FIELD, true),
      stringCache: new Map(),
      nodeCache: new Array(nodeCount),
      $asts: undefined
    }
  }

  /** Underlying DataView. */
  get view() {
    return this.#internal.view
  }
  /** Whether the buffer's `compat_mode` flag bit is set. */
  get compatMode() {
    return this.#internal.compatMode
  }
  /** Byte offset of the Extended Data section. */
  get extendedDataOffset() {
    return this.#internal.extendedDataOffset
  }
  /** Byte offset of the Nodes section. */
  get nodesOffset() {
    return this.#internal.nodesOffset
  }
  /** Total number of node records (including the `node[0]` sentinel). */
  get nodeCount() {
    return this.#internal.nodeCount
  }
  /** Number of roots N. */
  get rootCount() {
    return this.#internal.rootCount
  }

  /**
   * Resolve the string at `idx` (returns `null` for the None sentinels).
   * Cached on first lookup so repeated reads are O(1).
   *
   * @param {number} idx
   * @returns {string | null}
   */
  getString(idx) {
    if (idx === U16_NONE_SENTINEL || idx === STRING_PAYLOAD_NONE_SENTINEL) {
      return null
    }
    const cached = this.#internal.stringCache.get(idx)
    if (cached !== undefined) {
      return cached
    }
    const { view, stringOffsetsOffset, stringDataOffset } = this.#internal
    const start = view.getUint32(stringOffsetsOffset + idx * STRING_OFFSET_ENTRY_SIZE, true)
    const end = view.getUint32(stringOffsetsOffset + idx * STRING_OFFSET_ENTRY_SIZE + 4, true)
    const bytes = new Uint8Array(
      view.buffer,
      view.byteOffset + stringDataOffset + start,
      end - start
    )
    const str = utf8Decoder.decode(bytes)
    this.#internal.stringCache.set(idx, str)
    return str
  }

  /**
   * Get the `base_offset` for the i-th root (used to compute absolute ranges).
   *
   * @param {number} rootIndex
   * @returns {number}
   */
  getRootBaseOffset(rootIndex) {
    const off =
      this.#internal.rootArrayOffset + rootIndex * ROOT_INDEX_ENTRY_SIZE + BASE_OFFSET_FIELD
    return this.#internal.view.getUint32(off, true)
  }

  /**
   * Build (or fetch from cache) the lazy class instance for a node.
   *
   * Returns `null` for the sentinel (node index 0).
   *
   * @param {number} nodeIndex
   * @param {object | null} parent
   * @param {number} [rootIndex]
   * @returns {object | null}
   */
  getNode(nodeIndex, parent, rootIndex = -1) {
    if (nodeIndex === 0) {
      return null
    }
    const cached = this.#internal.nodeCache[nodeIndex]
    if (cached !== undefined) {
      return cached
    }
    const byteIndex = this.#internal.nodesOffset + nodeIndex * NODE_RECORD_SIZE
    const kind = this.#internal.view.getUint8(byteIndex + KIND_OFFSET)
    const Class = decodeKindToClass(kind)
    const node = new Class(this.#internal.view, byteIndex, nodeIndex, rootIndex, parent, this)
    this.#internal.nodeCache[nodeIndex] = node
    return node
  }

  /**
   * AST root for each entry in the Root Index array. Yields `null` for
   * entries with `node_index === 0` (parse failure sentinel) and the
   * matching lazy class instance otherwise.
   *
   * @returns {(object | null)[]}
   */
  get asts() {
    if (this.#internal.$asts !== undefined) {
      return this.#internal.$asts
    }
    const { view, rootArrayOffset, rootCount } = this.#internal
    const result = new Array(rootCount)
    for (let i = 0; i < rootCount; i++) {
      const nodeIdx = view.getUint32(
        rootArrayOffset + i * ROOT_INDEX_ENTRY_SIZE + NODE_INDEX_OFFSET,
        true
      )
      result[i] = nodeIdx === 0 ? null : this.getNode(nodeIdx, null, i)
    }
    this.#internal.$asts = result
    return result
  }
}
