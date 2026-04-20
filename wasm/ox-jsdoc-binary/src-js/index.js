/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import { RemoteSourceFile } from '@ox-jsdoc/decoder'

import init, { parse_jsdoc as parseJsdocWasm } from '../pkg/ox_jsdoc_binary_wasm.js'

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
