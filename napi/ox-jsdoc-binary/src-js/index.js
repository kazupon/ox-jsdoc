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

// Reused encoder + parallel-array buffers cut allocation churn when the
// same caller fires `parseBatch` repeatedly (lint loops, watch mode).
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
  }
  // Concatenate every `source_text` into a single UTF-8 buffer + offsets
  // table. The native binding's `parse_jsdoc_batch_raw` then sees three
  // typed-array handles instead of an N-element `Vec<JsBatchItem>`, which
  // is the input-marshalling hot path (~213 µs / 30 % of the call before
  // this change for a 226-comment fixture).
  const n = items.length
  const encoded = new Array(n)
  let totalBytes = 0
  for (let i = 0; i < n; i++) {
    const bytes = utf8Encoder.encode(items[i].sourceText)
    encoded[i] = bytes
    totalBytes += bytes.length
  }
  const concat = new Uint8Array(totalBytes)
  const offsets = new Uint32Array(n + 1)
  const baseOffsets = new Uint32Array(n)
  let pos = 0
  for (let i = 0; i < n; i++) {
    concat.set(encoded[i], pos)
    pos += encoded[i].length
    offsets[i + 1] = pos
    baseOffsets[i] = items[i].baseOffset ?? 0
  }
  const result = parseJsdocBatchRawBinding(concat, offsets, baseOffsets, bindingOptions)
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
