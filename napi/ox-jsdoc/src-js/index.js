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
 * @param {{ fenceAware?: boolean }} [options]
 * @returns {{ ast: any, diagnostics: Array<{ message: string }> }}
 */
export function parse(sourceText, options) {
  const result = parseBinding(sourceText, options ?? {})
  return {
    get ast() {
      const value = result.astJson === 'null' ? null : JSON.parse(result.astJson)
      Object.defineProperty(this, 'ast', { value })
      return value
    },
    diagnostics: result.diagnostics
  }
}
