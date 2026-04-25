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
  STRING_FIELD_NONE_OFFSET,
  STRING_OFFSET_ENTRY_SIZE,
  STRING_OFFSETS_OFFSET_FIELD,
  STRING_PAYLOAD_NONE_SENTINEL,
  SUPPORTED_MAJOR,
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
   * @param {{ emptyStringForNull?: boolean }} [options]
   *   `emptyStringForNull`: only effective when the buffer's `compat_mode`
   *   flag is set. Switches `toJSON()` and compat-mode field accessors to
   *   emit `""` instead of `null` for absent optional strings (rawType,
   *   name, namepathOrURL, text). Mirrors the Rust serializer's
   *   `SerializeOptions.empty_string_for_null` for jsdoccomment parity.
   */
  constructor(buffer, options) {
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
    // `Uint32Array[idx]` is 5–10× faster than `DataView.getUint32` in
    // V8 hot paths because the typed-array element load compiles down to
    // a single MOV instruction rather than a runtime stub call. The
    // writer pads every section that contains u32 reads to a 4-byte
    // boundary, so we can use the typed view across the whole buffer.
    //
    // Both NAPI's `Uint8Array::from(Vec<u8>)` and WASM's
    // `Box<[u8]>::as_ptr()` produce 4-byte aligned `byteOffset`s in
    // practice; the assertion below catches the (very unlikely) day
    // someone wraps a misaligned slice manually.
    if ((view.byteOffset & 3) !== 0) {
      throw new Error(
        `Binary AST buffer must be 4-byte aligned (byteOffset=${view.byteOffset})`
      )
    }
    const uint32View = new Uint32Array(view.buffer, view.byteOffset, view.byteLength >>> 2)
    const flags = view.getUint8(FLAGS_OFFSET)
    const nodeCount = uint32View[NODE_COUNT_FIELD >>> 2]
    const compatMode = (flags & COMPAT_MODE_BIT) !== 0
    this.#internal = {
      view,
      uint32View,
      version: versionByte,
      compatMode,
      // Only effective when compatMode; basic-mode buffers have no
      // jsdoccomment-shape consumers so the toggle is meaningless there.
      emptyStringForNull: compatMode && options !== undefined && options.emptyStringForNull === true,
      rootArrayOffset: uint32View[ROOT_ARRAY_OFFSET_FIELD >>> 2],
      stringOffsetsOffset: uint32View[STRING_OFFSETS_OFFSET_FIELD >>> 2],
      stringDataOffset: uint32View[STRING_DATA_OFFSET_FIELD >>> 2],
      extendedDataOffset: uint32View[EXTENDED_DATA_OFFSET_FIELD >>> 2],
      diagnosticsOffset: uint32View[DIAGNOSTICS_OFFSET_FIELD >>> 2],
      nodesOffset: uint32View[NODES_OFFSET_FIELD >>> 2],
      nodeCount,
      sourceTextLength: uint32View[SOURCE_TEXT_LENGTH_FIELD >>> 2],
      rootCount: uint32View[ROOT_COUNT_FIELD >>> 2],
      stringCache: new Map(),
      nodeCache: new Array(nodeCount),
      $asts: undefined
    }
  }

  /** Underlying DataView. */
  get view() {
    return this.#internal.view
  }
  /**
   * Underlying typed `Uint32Array` view aligned to the buffer start.
   * Index by `byteOffset >>> 2` for any 4-byte aligned u32 read; this is
   * 5–10× faster than `DataView.getUint32` in V8 hot paths.
   */
  get uint32View() {
    return this.#internal.uint32View
  }
  /** Whether the buffer's `compat_mode` flag bit is set. */
  get compatMode() {
    return this.#internal.compatMode
  }
  /** Whether `null` optional strings are emitted as `""` in compat-mode. */
  get emptyStringForNull() {
    return this.#internal.emptyStringForNull
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
   * Resolve the string at `idx` (returns `null` for the
   * `STRING_PAYLOAD_NONE_SENTINEL` (`0x3FFF_FFFF`) sentinel). Used by
   * string-leaf nodes (TypeTag::String payload) and the diagnostics
   * section.
   *
   * Cached on first lookup so repeated reads are O(1).
   *
   * @param {number} idx
   * @returns {string | null}
   */
  getString(idx) {
    if (idx === STRING_PAYLOAD_NONE_SENTINEL) {
      return null
    }
    const cached = this.#internal.stringCache.get(idx)
    if (cached !== undefined) {
      return cached
    }
    const { view, uint32View, stringOffsetsOffset, stringDataOffset } = this.#internal
    const entryWordIndex = (stringOffsetsOffset + idx * STRING_OFFSET_ENTRY_SIZE) >>> 2
    const start = uint32View[entryWordIndex]
    const end = uint32View[entryWordIndex + 1]
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
   * Resolve a `StringField` `(offset, length)` pair into the underlying
   * string. Returns `null` when the field is the `NONE` sentinel
   * (`offset === STRING_FIELD_NONE_OFFSET`). Used by Extended Data string
   * slots which embed `(offset, length)` directly.
   *
   * Cache key uses a high-bit-set form of `offset` so it never collides
   * with `getString(idx)` cache entries (string-leaf path uses small
   * indices, ED path uses byte offsets — both fit in u32 and overlap).
   *
   * @param {number} offset Byte offset within the String Data section.
   * @param {number} length UTF-8 byte length.
   * @returns {string | null}
   */
  getStringByField(offset, length) {
    if (offset === STRING_FIELD_NONE_OFFSET) {
      return null
    }
    // Cache key disambiguation: ED-path keys are tagged with a sentinel
    // bit so they never collide with index-path keys.
    const cacheKey = -(offset + 1)
    const cached = this.#internal.stringCache.get(cacheKey)
    if (cached !== undefined) {
      return cached
    }
    const { view, stringDataOffset } = this.#internal
    const bytes = new Uint8Array(
      view.buffer,
      view.byteOffset + stringDataOffset + offset,
      length
    )
    const str = utf8Decoder.decode(bytes)
    this.#internal.stringCache.set(cacheKey, str)
    return str
  }

  /**
   * Resolve a Path B-leaf inline `(offset, length)` pair into the underlying
   * string. Always returns a real `&str` (never `null`) — encoders only
   * emit `TypeTag::StringInline` for present, non-empty short strings.
   *
   * Reuses the same cache-key disambiguation as `getStringByField` (offset
   * is tagged with the sign bit) so inline-path lookups never collide with
   * String-Offsets-table lookups.
   *
   * @param {number} offset Byte offset within the String Data section (≤ 4 MB).
   * @param {number} length UTF-8 byte length (≤ 255).
   * @returns {string}
   */
  getStringByOffsetAndLength(offset, length) {
    const cacheKey = -(offset + 1)
    const cached = this.#internal.stringCache.get(cacheKey)
    if (cached !== undefined) {
      return cached
    }
    const { view, stringDataOffset } = this.#internal
    const bytes = new Uint8Array(
      view.buffer,
      view.byteOffset + stringDataOffset + offset,
      length
    )
    const str = utf8Decoder.decode(bytes)
    this.#internal.stringCache.set(cacheKey, str)
    return str
  }

  /**
   * Get the `base_offset` for the i-th root (used to compute absolute ranges).
   *
   * @param {number} rootIndex
   * @returns {number}
   */
  getRootBaseOffset(rootIndex) {
    // Root index array is 4-byte aligned (starts at HEADER_SIZE = 40)
    // and each entry is 12 bytes (3 × u32) so every field lands on a
    // 4-byte boundary — safe for `Uint32Array[idx]`.
    const off =
      this.#internal.rootArrayOffset + rootIndex * ROOT_INDEX_ENTRY_SIZE + BASE_OFFSET_FIELD
    return this.#internal.uint32View[off >>> 2]
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
