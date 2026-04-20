/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import { RemoteSourceFile } from '@ox-jsdoc/decoder'

import { parseJsdoc as parseJsdocBinding } from './bindings.js'

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
