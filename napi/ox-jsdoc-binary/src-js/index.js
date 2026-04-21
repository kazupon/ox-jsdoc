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

// `Buffer.byteLength` is a Node global; its types live in `@types/node`
// which is not wired into this file's `@ts-check` pass. Capture it once
// behind a typed cast so the rest of the file stays strictly typed.
const utf8ByteLength = /** @type {(s: string, e: 'utf8') => number} */ (
  /** @type {any} */ (globalThis).Buffer.byteLength
)

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
  // typed-array handles instead of an N-element `Vec<JsBatchItem>` (which
  // before this change was the call's hot path at ~213 µs / 30 %).
  //
  // Two-pass: `Buffer.byteLength` (V8 internal UTF-8 sizing, no
  // allocation) computes the exact concat length first so we can hand
  // `encodeInto` the final buffer and avoid the 226 intermediate
  // `Uint8Array` allocations a per-item `encoder.encode` would create.
  const n = items.length
  let totalBytes = 0
  for (let i = 0; i < n; i++) {
    totalBytes += utf8ByteLength(items[i].sourceText, 'utf8')
  }
  const concat = new Uint8Array(totalBytes)
  const offsets = new Uint32Array(n + 1)
  const baseOffsets = new Uint32Array(n)
  let pos = 0
  for (let i = 0; i < n; i++) {
    const { written } = utf8Encoder.encodeInto(items[i].sourceText, concat.subarray(pos))
    pos += written
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
