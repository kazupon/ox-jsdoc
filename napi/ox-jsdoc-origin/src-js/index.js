/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import {
  parse as parseBinding,
  parseTypeExpression as parseTypeExpressionBinding,
  parseTypeCheck as parseTypeCheckBinding
} from './bindings.js'

/**
 * Parse a complete `/** ... *​/` JSDoc block comment.
 *
 * @param {string} sourceText
 * @param {{
 *   fenceAware?: boolean,
 *   parseTypes?: boolean,
 *   typeParseMode?: 'jsdoc' | 'closure' | 'typescript',
 *   compatMode?: boolean,
 *   emptyStringForNull?: boolean,
 *   includePositions?: boolean,
 *   spacing?: 'compact' | 'preserve'
 * }} [options]
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
    if (options.compatMode !== undefined) {
      bindingOptions.compatMode = options.compatMode
    }
    if (options.emptyStringForNull !== undefined) {
      bindingOptions.emptyStringForNull = options.emptyStringForNull
    }
    if (options.includePositions !== undefined) {
      bindingOptions.includePositions = options.includePositions
    }
    if (options.spacing !== undefined) {
      bindingOptions.spacing = options.spacing
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

/**
 * Parse a standalone type expression (no comment parsing overhead).
 *
 * @param {string} typeText
 * @param {'jsdoc' | 'closure' | 'typescript'} [mode]
 * @returns {string | null}
 */
export function parseType(typeText, mode) {
  return parseTypeExpressionBinding(typeText, mode ?? 'jsdoc') ?? null
}

/**
 * Parse a type expression and return whether it succeeded (no stringify overhead).
 *
 * @param {string} typeText
 * @param {'jsdoc' | 'closure' | 'typescript'} [mode]
 * @returns {boolean}
 */
export function parseTypeCheck(typeText, mode) {
  return parseTypeCheckBinding(typeText, mode ?? 'jsdoc')
}
