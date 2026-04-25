/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import { RemoteSourceFile } from '@ox-jsdoc/decoder'

import init, {
  parse_jsdoc as parseJsdocWasm,
  parse_jsdoc_batch_raw as parseJsdocBatchRawWasm
} from '../pkg/ox_jsdoc_binary_wasm.js'

export { jsdocVisitorKeys } from '@ox-jsdoc/decoder'

// Reused encoder cuts allocation churn on hot-loop callers (lint runners).
const utf8Encoder = new TextEncoder()

// ---------------------------------------------------------------------------
// Module-level buffer pool for parseBatch (mirrors NAPI's pool — same
// rationale: avoids the per-call zero-fill of the worst-case Uint8Array
// + two small Uint32Arrays. Hot-loop callers keep the pool warm; one-shot
// callers pay the same alloc once and never benefit again — additive.)
//
// SAFETY: WASM calls below are synchronous, so the pool cannot be observed
// mid-write by another `parseBatch` invocation. The buffers we hand to
// `parseJsdocBatchRawWasm` are `subarray(0, pos)` views; wasm-bindgen
// copies them into linear memory for the duration of the call.

/** Cap to avoid pinning huge buffers from one outlier call. 8 MiB covers
 *  ~30 × the typescript-checker.ts batch (80 KB). Larger inputs bypass the
 *  pool entirely so memory is GC-eligible right after the call. */
const POOL_CONCAT_CAP = 8 * 1024 * 1024
/** Cap on offsets/baseOffsets buffers (entries, not bytes). 1M × 4 byte = 4 MiB. */
const POOL_INDEX_CAP = 1 << 20

/** @type {Uint8Array | null}  Reused concat buffer, monotonically grows up to POOL_CONCAT_CAP. */
let _concatPool = null
/** @type {Uint32Array | null} Reused offsets buffer (length = items.length + 1). */
let _offsetsPool = null
/** @type {Uint32Array | null} Reused baseOffsets buffer (length = items.length). */
let _baseOffsetsPool = null

/**
 * Grow `pool` to at least `need` slots with a 1.5x growth factor.
 *
 * @param {Uint8Array | null} pool
 * @param {number} need
 * @returns {Uint8Array}
 */
function _growU8(pool, need) {
  const current = pool?.length ?? 0
  const cap = Math.max(need, (current * 3) >>> 1)
  return new Uint8Array(cap)
}

/**
 * Grow `pool` to at least `need` slots with a 1.5x growth factor.
 *
 * @param {Uint32Array | null} pool
 * @param {number} need
 * @returns {Uint32Array}
 */
function _growU32(pool, need) {
  const current = pool?.length ?? 0
  const cap = Math.max(need, (current * 3) >>> 1)
  return new Uint32Array(cap)
}

/** @type {WebAssembly.Memory | null} */
let wasmMemory = null

/**
 * Initialize the WASM module. Must be called once before {@link parse}.
 *
 * @param {string | URL | Request | Response | WebAssembly.Module | ArrayBuffer | BufferSource} [wasmUrl]
 * @returns {Promise<void>}
 */
export async function initWasm(wasmUrl) {
  if (wasmMemory === null) {
    const exports = await init(wasmUrl)
    wasmMemory = exports.memory
  }
}

/**
 * Parse a complete `/** ... *​/` JSDoc block comment.
 *
 * The Rust side returns a handle whose bytes live inside `wasm.memory.buffer`.
 * The wrapper wraps those bytes as a `Uint8Array` view (no copy) and feeds
 * them into `@ox-jsdoc/decoder`'s `RemoteSourceFile`. Subsequent lazy-getter
 * reads pull from wasm memory directly.
 *
 * **Lifecycle**: the returned object's `free()` releases the wasm-side
 * bytes. After `free()`, accessing `ast` / `sourceFile` is undefined
 * behaviour. Callers that need the bytes to outlive `free()` should call
 * `toPlainObject(ast)` first to materialize the lazy tree.
 *
 * @param {string} sourceText
 * @param {{
 *   fenceAware?: boolean,
 *   parseTypes?: boolean,
 *   typeParseMode?: 'jsdoc' | 'closure' | 'typescript',
 *   compatMode?: boolean,
 *   baseOffset?: number,
 * }} [options]
 * @returns {{
 *   ast: import('@ox-jsdoc/decoder').RemoteJsdocBlock | null,
 *   diagnostics: Array<{ message: string }>,
 *   sourceFile: import('@ox-jsdoc/decoder').RemoteSourceFile,
 *   free: () => void,
 * }}
 */
export function parse(sourceText, options) {
  if (wasmMemory === null) {
    throw new Error('Call initWasm() before parse()')
  }
  const handle = parseJsdocWasm(
    sourceText,
    options?.fenceAware ?? null,
    options?.parseTypes ?? null,
    options?.typeParseMode ?? null,
    options?.compatMode ?? null,
    options?.baseOffset ?? null
  )

  const view = new Uint8Array(wasmMemory.buffer, handle.bufferPtr(), handle.bufferLen())
  const sourceFile = new RemoteSourceFile(view)
  const diagnostics =
    /** @type {Array<{ message: string }>} */ (/** @type {unknown} */ (handle.diagnostics()))
  const root = sourceFile.asts[0] ?? null
  return {
    ast: /** @type {import('@ox-jsdoc/decoder').RemoteJsdocBlock | null} */ (root),
    diagnostics,
    sourceFile,
    free: () => handle.free()
  }
}

/**
 * Parse N JSDoc block comments at once into a single shared binary-AST
 * buffer. Common strings (`*`, `*​/`, tag names) are interned once across
 * all comments.
 *
 * @param {Array<{ sourceText: string, baseOffset?: number }>} items
 * @param {{
 *   fenceAware?: boolean,
 *   parseTypes?: boolean,
 *   typeParseMode?: 'jsdoc' | 'closure' | 'typescript',
 *   compatMode?: boolean,
 * }} [options]
 * @returns {{
 *   asts: Array<import('@ox-jsdoc/decoder').RemoteJsdocBlock | null>,
 *   diagnostics: Array<{ message: string, rootIndex: number }>,
 *   sourceFile: import('@ox-jsdoc/decoder').RemoteSourceFile,
 *   free: () => void,
 * }}
 */
export function parseBatch(items, options) {
  if (wasmMemory === null) {
    throw new Error('Call initWasm() before parseBatch()')
  }
  // Concatenate every `source_text` into a single UTF-8 buffer + offsets
  // table so the WASM call sees three slice handles instead of an
  // N-element `Vec<String>`. Each `String` element of the prior path paid
  // a separate JS string → wasm linear-memory copy + `String` wrapper
  // allocation; for the typescript-checker.ts fixture (226 strings,
  // ~25 KB total) this dominated the cross-boundary cost.
  //
  // Single-pass: over-allocate the concat buffer to the worst-case UTF-8
  // size (3 bytes per UTF-16 unit covers BMP, and 2 surrogate units
  // contribute 4 bytes ≤ 2 × 3) so we can hand `encodeInto` the final
  // buffer in one loop. The wasted tail is freed when the concat buffer
  // (or its `subarray` view) is collected.
  const n = items.length
  let totalChars = 0
  for (let i = 0; i < n; i++) {
    totalChars += items[i].sourceText.length
  }
  const needConcat = totalChars * 3
  const needOffsets = n + 1
  const needBaseOffsets = n

  // Pool acquisition. Outliers larger than the cap bypass pooling so we
  // don't pin a huge buffer for the rest of the process lifetime.
  let concat
  if (needConcat > POOL_CONCAT_CAP) {
    concat = new Uint8Array(needConcat)
  } else {
    if (!_concatPool || _concatPool.length < needConcat) {
      _concatPool = _growU8(_concatPool, needConcat)
    }
    concat = _concatPool
  }
  let offsets
  if (needOffsets > POOL_INDEX_CAP) {
    offsets = new Uint32Array(needOffsets)
  } else {
    if (!_offsetsPool || _offsetsPool.length < needOffsets) {
      _offsetsPool = _growU32(_offsetsPool, needOffsets)
    }
    offsets = _offsetsPool
  }
  let baseOffsets
  if (needBaseOffsets > POOL_INDEX_CAP) {
    baseOffsets = new Uint32Array(needBaseOffsets)
  } else {
    if (!_baseOffsetsPool || _baseOffsetsPool.length < needBaseOffsets) {
      _baseOffsetsPool = _growU32(_baseOffsetsPool, needBaseOffsets)
    }
    baseOffsets = _baseOffsetsPool
  }

  // `offsets[0]` must be 0 — when the pool is reused, a previous call may
  // have left a different value in slot 0 (from `pos` at iteration 0 of
  // the prior batch). Explicit zero is cheaper than a full clear.
  offsets[0] = 0

  let pos = 0
  for (let i = 0; i < n; i++) {
    const { written } = utf8Encoder.encodeInto(items[i].sourceText, concat.subarray(pos))
    pos += written
    offsets[i + 1] = pos
    baseOffsets[i] = items[i].baseOffset ?? 0
  }

  const handle = parseJsdocBatchRawWasm(
    concat.subarray(0, pos),
    offsets.subarray(0, needOffsets),
    baseOffsets.subarray(0, needBaseOffsets),
    options?.fenceAware ?? null,
    options?.parseTypes ?? null,
    options?.typeParseMode ?? null,
    options?.compatMode ?? null
  )

  const view = new Uint8Array(wasmMemory.buffer, handle.bufferPtr(), handle.bufferLen())
  const sourceFile = new RemoteSourceFile(view)
  const diagnostics =
    /** @type {Array<{ message: string, rootIndex: number }>} */ (
      /** @type {unknown} */ (handle.diagnostics())
    )
  const asts = /** @type {Array<import('@ox-jsdoc/decoder').RemoteJsdocBlock | null>} */ (
    sourceFile.asts
  )
  return {
    asts,
    diagnostics,
    sourceFile,
    free: () => handle.free()
  }
}
