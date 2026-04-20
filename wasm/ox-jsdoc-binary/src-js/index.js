/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import { RemoteSourceFile } from '@ox-jsdoc/decoder'

import init, {
  parse_jsdoc as parseJsdocWasm,
  parse_jsdoc_batch as parseJsdocBatchWasm
} from '../pkg/ox_jsdoc_binary_wasm.js'

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
  const sourceTexts = items.map(item => item.sourceText)
  const baseOffsets = new Uint32Array(items.length)
  for (let i = 0; i < items.length; i++) {
    baseOffsets[i] = items[i].baseOffset ?? 0
  }
  const handle = parseJsdocBatchWasm(
    sourceTexts,
    baseOffsets,
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
