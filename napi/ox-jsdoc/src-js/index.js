/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import { parse as parseBinding } from './bindings.js'

/**
 * Parse a complete `/** ... *​/` JSDoc block comment.
 *
 * @param {string} sourceText
 * @param {{ fenceAware?: boolean, parseTypes?: boolean, typeParseMode?: string }} [options]
 * @returns {{ ast: any, diagnostics: Array<{ message: string }> }}
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
  }
  const result = parseBinding(sourceText, bindingOptions)
  return {
    get ast() {
      const value = result.astJson === 'null' ? null : JSON.parse(result.astJson)
      Object.defineProperty(this, 'ast', { value })
      return value
    },
    diagnostics: result.diagnostics
  }
}
