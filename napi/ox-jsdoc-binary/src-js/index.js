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

// Reused encoder cuts allocation churn when the same caller fires
// `parseBatch` repeatedly (lint loops, watch mode).
const utf8Encoder = new TextEncoder()

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
    if (options.baseOffset !== undefined) {
      bindingOptions.baseOffset = options.baseOffset
    }
  }
  const result = parseJsdocBinding(sourceText, bindingOptions)
  const sourceFile = new RemoteSourceFile(result.buffer)
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
  const concat = new Uint8Array(totalChars * 3)
  const offsets = new Uint32Array(n + 1)
  const baseOffsets = new Uint32Array(n)
  let pos = 0
  for (let i = 0; i < n; i++) {
    const { written } = utf8Encoder.encodeInto(items[i].sourceText, concat.subarray(pos))
    pos += written
    offsets[i + 1] = pos
    baseOffsets[i] = items[i].baseOffset ?? 0
  }
  const result = parseJsdocBatchRawBinding(
    concat.subarray(0, pos),
    offsets,
    baseOffsets,
    options ?? {}
  )
  const sourceFile = new RemoteSourceFile(result.buffer)
  const asts = /** @type {Array<import('@ox-jsdoc/decoder').RemoteJsdocBlock | null>} */ (
    sourceFile.asts
  )
  return {
    asts,
    diagnostics: result.diagnostics,
    sourceFile
  }
}
