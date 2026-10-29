/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import { RemoteSourceFile } from '@ox-jsdoc/decoder'

import init, {
  parse_jsdoc as parseJsdocWasm,
  parse_jsdoc_batch_raw as parseJsdocBatchRawWasm,
  parse_type_check as parseTypeCheckWasm,
  parse_type_expression as parseTypeExpressionWasm
} from '../pkg/ox_jsdoc_wasm.js'

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

/**
 * @typedef {string | URL | Request | Response | ArrayBuffer | ArrayBufferView | WebAssembly.Module} WasmSource
 * @typedef {WasmSource | Promise<WasmSource> | { module_or_path: WasmSource | Promise<WasmSource> }} InitWasmInput
 */

/** @type {WebAssembly.Memory | null} */
let wasmMemory = null
/** @type {Promise<void> | null} */
let initPromise = null

/**
 * Wrap the input in the object form that wasm-bindgen expects (0.2.93+); it
 * prints a deprecation warning for the other forms. An object that has
 * `module_or_path` is already that form. It is copied, so the glue always
 * gets a plain object of this realm.
 *
 * @param {InitWasmInput | undefined} input
 */
function toInitOptions(input) {
  if (input === undefined) {
    return undefined
  }
  const isOptions = typeof input === 'object' && input !== null && 'module_or_path' in input
  return { module_or_path: isOptions ? input.module_or_path : input }
}

/**
 * The error for a failed initialization without an input: the `.wasm` file
 * next to the generated glue could not be loaded. In Node.js that always
 * happens, because `fetch` cannot read its `file:` URL.
 *
 * @param {unknown} cause
 */
function defaultLoadError(cause) {
  return new Error(
    'Could not load the default ox_jsdoc_wasm_bg.wasm. Pass the .wasm source to initWasm(): ' +
      'its URL, a Response, or its bytes (in Node.js, read the file with fs). ' +
      'In Node.js, the native `ox-jsdoc` package needs no initWasm().',
    { cause }
  )
}

/**
 * Initialize the WASM module. Must be called once before {@link parse}.
 *
 * Concurrent and repeated calls share one initialization; the input of later
 * calls is ignored. After a failure, the next call starts over.
 *
 * @param {InitWasmInput} [input] The `.wasm` file: a URL, a `Response`, its
 *   bytes, a compiled module, a promise of one of them, or
 *   `{ module_or_path }`. When omitted, the file next to the generated glue
 *   is fetched. If that fails (always in Node.js), the error says what to
 *   pass instead, with the original error as its `cause`.
 * @returns {Promise<void>}
 */
export function initWasm(input) {
  initPromise ??= init(toInitOptions(input)).then(
    exports => {
      wasmMemory = exports.memory
    },
    error => {
      initPromise = null
      throw input === undefined ? defaultLoadError(error) : error
    }
  )
  return initPromise
}

/**
 * Copy the Binary AST of `handle` out of wasm memory, read its diagnostics
 * and free it.
 *
 * The bytes live in wasm memory and belong to the wasm-bindgen handle. A
 * view into them would go stale: growing the memory (any later call may)
 * detaches its `ArrayBuffer`, and the bytes are freed by `handle.free()` or
 * once the handle is collected (the glue registers it with a
 * `FinalizationRegistry`). The copy lives in JS memory for as long as the
 * AST does, like the `Buffer` of the NAPI binding, and starts at offset 0,
 * which meets the 4-byte alignment that the decoder needs.
 *
 * @param {{ bufferPtr(): number, bufferLen(): number, diagnostics(): any[], free(): void }} handle
 * @returns {{ sourceFile: RemoteSourceFile, diagnostics: unknown[] }}
 */
function takeResult(handle) {
  try {
    const ptr = handle.bufferPtr()
    const memory = /** @type {WebAssembly.Memory} */ (wasmMemory)
    const bytes = memory.buffer.slice(ptr, ptr + handle.bufferLen())
    return { sourceFile: new RemoteSourceFile(bytes), diagnostics: handle.diagnostics() }
  } finally {
    handle.free()
  }
}

/**
 * The `free()` of a result. The wasm-side bytes are freed before `parse()`
 * and `parseBatch()` return, so there is nothing left to release; it is kept
 * so that existing calls keep working.
 */
const free = () => {}

/**
 * Parse a complete `/** ... *​/` JSDoc block comment.
 *
 * The Rust side writes the Binary AST into wasm memory. The wrapper copies it
 * into a JS `ArrayBuffer` for `@ox-jsdoc/decoder`'s `RemoteSourceFile`, and
 * frees the wasm-side bytes before returning. So the AST stays readable for
 * as long as it is referenced, whatever later calls do to wasm memory.
 *
 * **Lifecycle**: nothing to release. The returned object's `free()` does
 * nothing; it is kept for compatibility.
 *
 * @param {string} sourceText
 * @param {{
 *   fenceAware?: boolean,
 *   parseTypes?: boolean,
 *   typeParseMode?: 'jsdoc' | 'closure' | 'typescript',
 *   compatMode?: boolean,
 *   preserveWhitespace?: boolean,
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
    options?.preserveWhitespace ?? null,
    options?.baseOffset ?? null
  )

  const result = takeResult(handle)
  const sourceFile = result.sourceFile
  const diagnostics = /** @type {Array<{ message: string }>} */ (result.diagnostics)
  const root = sourceFile.asts[0] ?? null
  return {
    ast: /** @type {import('@ox-jsdoc/decoder').RemoteJsdocBlock | null} */ (root),
    diagnostics,
    sourceFile,
    free
  }
}

/**
 * Parse N JSDoc block comments at once into a single shared binary-AST
 * buffer. Common strings (`*`, `*​/`, tag names) are interned once across
 * all comments.
 *
 * As with {@link parse}, the buffer is copied out of wasm memory, so the ASTs
 * stay readable for as long as they are referenced, and `free()` does
 * nothing.
 *
 * @param {Array<{ sourceText: string, baseOffset?: number }>} items
 * @param {{
 *   fenceAware?: boolean,
 *   parseTypes?: boolean,
 *   typeParseMode?: 'jsdoc' | 'closure' | 'typescript',
 *   compatMode?: boolean,
 *   preserveWhitespace?: boolean,
 *   output?: 'ast' | 'jsdoccomment-input',
 * }} [options]
 * @returns {{
 *   asts: Array<import('@ox-jsdoc/decoder').RemoteJsdocBlock | null>,
 *   blocks?: Array<import('@ox-jsdoc/decoder').JsdocCommentInput | null>,
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
    options?.compatMode ?? null,
    options?.preserveWhitespace ?? null
  )

  const result = takeResult(handle)
  const sourceFile = result.sourceFile
  const diagnostics = /** @type {Array<{ message: string, rootIndex: number }>} */ (
    result.diagnostics
  )
  const asts = /** @type {Array<import('@ox-jsdoc/decoder').RemoteJsdocBlock | null>} */ (
    sourceFile.asts
  )
  if (options?.output === 'jsdoccomment-input') {
    const blocks = asts.map(ast => {
      return ast?.toJsdocCommentInput() ?? null
    })
    return {
      asts,
      blocks,
      diagnostics,
      sourceFile,
      free
    }
  }

  return {
    asts,
    diagnostics,
    sourceFile,
    free
  }
}

/**
 * Parse a standalone type expression (no comment parsing overhead).
 *
 * @param {string} typeText
 * @param {'jsdoc' | 'closure' | 'typescript'} [mode]
 * @returns {string | null}
 */
export function parseType(typeText, mode) {
  if (wasmMemory === null) {
    throw new Error('Call initWasm() before parseType()')
  }
  return parseTypeExpressionWasm(typeText, mode ?? 'jsdoc') ?? null
}

/**
 * Parse a type expression and return whether it succeeded (no stringify overhead).
 *
 * @param {string} typeText
 * @param {'jsdoc' | 'closure' | 'typescript'} [mode]
 * @returns {boolean}
 */
export function parseTypeCheck(typeText, mode) {
  if (wasmMemory === null) {
    throw new Error('Call initWasm() before parseTypeCheck()')
  }
  return parseTypeCheckWasm(typeText, mode ?? 'jsdoc')
}
