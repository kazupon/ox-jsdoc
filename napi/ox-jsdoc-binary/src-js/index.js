/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import { RemoteSourceFile } from '@ox-jsdoc/decoder'

import {
  parseJsdoc as parseJsdocBinding,
  parseJsdocBatchRaw as parseJsdocBatchRawBinding
} from './bindings.js'

export { jsdocVisitorKeys } from '@ox-jsdoc/decoder'

// Reused encoder cuts allocation churn when the same caller fires
// `parseBatch` repeatedly (lint loops, watch mode).
const utf8Encoder = new TextEncoder()

// ---------------------------------------------------------------------------
// Module-level buffer pool for parseBatch
// ---------------------------------------------------------------------------
//
// Saves the per-call zero-fill of the worst-case Uint8Array (~80 KB for the
// typescript-checker.ts fixture) plus the two small Uint32Array allocs.
// Hot-loop callers (lint runners, watch mode) keep the pool warm; one-shot
// callers pay the same alloc once and never benefit again — pool is purely
// additive (never regresses).
//
// SAFETY: NAPI calls below are synchronous, so the pool cannot be observed
// mid-write by another `parseBatch` invocation. The buffers we hand to NAPI
// are `subarray(0, pos)` views; the Rust side borrows them for the duration
// of the synchronous call only.

/** Cap to avoid pinning huge buffers from one outlier call. 8 MiB covers
 *  ~30 × the typescript-checker.ts batch (80 KB). Larger inputs bypass the
 *  pool entirely so memory is GC-eligible right after the call. */
const POOL_CONCAT_CAP = 8 * 1024 * 1024
/** Cap on offsets/baseOffsets buffers (entries, not bytes). Same rationale
 *  but smaller absolute bound — 1M entries × 4 byte = 4 MiB. */
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

/**
 * Parse a complete `/** ... *​/` JSDoc block comment.
 *
 * The Rust side returns a Binary AST byte buffer that the JS-side
 * `@ox-jsdoc/decoder` lazy decoder wraps as a `RemoteSourceFile`. The
 * lazy `ast` getter materializes the root `RemoteJsdocBlock` only on
 * first access (cached afterwards).
 *
 * @param {string} sourceText
 * @param {{
 *   fenceAware?: boolean,
 *   parseTypes?: boolean,
 *   typeParseMode?: 'jsdoc' | 'closure' | 'typescript',
 *   compatMode?: boolean,
 *   preserveWhitespace?: boolean,
 *   emptyStringForNull?: boolean,
 *   baseOffset?: number,
 * }} [options]
 * @returns {{
 *   ast: import('@ox-jsdoc/decoder').RemoteJsdocBlock | null,
 *   diagnostics: Array<{ message: string }>,
 *   sourceFile: import('@ox-jsdoc/decoder').RemoteSourceFile,
 * }}
 */
export function parse(sourceText, options) {
  const bindingOptions = {}
  if (options) {
    if (options.fenceAware !== undefined) {
      bindingOptions.fenceAware = options.fenceAware
    }
    if (options.parseTypes !== undefined) {
      bindingOptions.parseTypes = options.parseTypes
    }
    if (options.typeParseMode !== undefined) {
      bindingOptions.typeParseMode = options.typeParseMode
    }
    if (options.compatMode !== undefined) {
      bindingOptions.compatMode = options.compatMode
    }
    if (options.preserveWhitespace !== undefined) {
      bindingOptions.preserveWhitespace = options.preserveWhitespace
    }
    if (options.baseOffset !== undefined) {
      bindingOptions.baseOffset = options.baseOffset
    }
  }
  const result = parseJsdocBinding(sourceText, bindingOptions)
  const sourceFile = new RemoteSourceFile(result.buffer, {
    emptyStringForNull: options?.emptyStringForNull
  })
  const root = sourceFile.asts[0] ?? null
  return {
    ast: /** @type {import('@ox-jsdoc/decoder').RemoteJsdocBlock | null} */ (root),
    diagnostics: result.diagnostics,
    sourceFile
  }
}

/**
 * Parse N JSDoc block comments at once into a single shared Binary AST
 * buffer. Common strings (`*`, `* /`, tag names) are interned once across
 * all comments.
 *
 * @param {Array<{ sourceText: string, baseOffset?: number }>} items
 * @param {{
 *   fenceAware?: boolean,
 *   parseTypes?: boolean,
 *   typeParseMode?: 'jsdoc' | 'closure' | 'typescript',
 *   compatMode?: boolean,
 *   preserveWhitespace?: boolean,
 *   emptyStringForNull?: boolean,
 * }} [options]
 * @returns {{
 *   asts: Array<import('@ox-jsdoc/decoder').RemoteJsdocBlock | null>,
 *   diagnostics: Array<{ message: string, rootIndex: number }>,
 *   sourceFile: import('@ox-jsdoc/decoder').RemoteSourceFile,
 * }}
 */
export function parseBatch(items, options) {
  // Concatenate every `source_text` into a single UTF-8 buffer + offsets
  // table. The native binding's `parse_jsdoc_batch_raw` then sees three
  // typed-array handles instead of an N-element `Vec<JsBatchItem>` (which
  // before this change was the call's hot path at ~213 µs / 30 %).
  //
  // Single-pass: over-allocate the concat buffer to the worst-case UTF-8
  // size (3 bytes per UTF-16 unit covers BMP, and 2 surrogate units
  // contribute 4 bytes ≤ 2 × 3) so we can skip the `Buffer.byteLength`
  // pre-pass and hand `encodeInto` the final buffer in one loop. The
  // wasted tail (up to 2× total chars on ASCII-heavy input) is freed
  // when both the concat and the `subarray` view we hand to NAPI are
  // collected — never larger than ~3 × total source bytes per call.
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
  const result = parseJsdocBatchRawBinding(
    concat.subarray(0, pos),
    offsets.subarray(0, needOffsets),
    baseOffsets.subarray(0, needBaseOffsets),
    options ?? {}
  )
  const sourceFile = new RemoteSourceFile(result.buffer, {
    emptyStringForNull: options?.emptyStringForNull
  })
  const asts = /** @type {Array<import('@ox-jsdoc/decoder').RemoteJsdocBlock | null>} */ (
    sourceFile.asts
  )
  return {
    asts,
    diagnostics: result.diagnostics,
    sourceFile
  }
}
