/**
 * Standalone `parseType` / `parseTypeCheck` API tests for the binary NAPI
 * binding.
 *
 * Mirrors the categories covered by `napi/ox-jsdoc/test/parsed-type.test.ts`
 * (typed-side AST equivalence with jsdoc-type-pratt-parser) but exercises
 * the standalone parseType / parseTypeCheck functions which return a
 * stringified type or a boolean — they bypass comment parsing and Binary AST
 * emission entirely.
 *
 * AST-level equivalence (parsedType through `parse(..., { parseTypes: true })`)
 * is covered separately by `parse.test.ts`.
 */

import { describe, expect, it } from 'vite-plus/test'
import { parse as jtpParse } from 'jsdoc-type-pratt-parser'

import { parseType, parseTypeCheck } from '../src-js/index.js'

type Mode = 'jsdoc' | 'closure' | 'typescript'

/**
 * Verify that the binary parser accepts the same inputs as
 * jsdoc-type-pratt-parser. We compare acceptance, not stringified shape,
 * because the two parsers use different formatting conventions
 * (whitespace, separator style) that are tested separately in the typed
 * side AST test.
 */
function expectAccepts(typeExpr: string, mode: Mode) {
  let refAccepts = false
  try {
    jtpParse(typeExpr, mode)
    refAccepts = true
  } catch {
    refAccepts = false
  }
  if (!refAccepts) {
    return
  }
  expect(parseTypeCheck(typeExpr, mode), `parseTypeCheck rejected "${typeExpr}" in ${mode}`).toBe(
    true
  )
  expect(
    parseType(typeExpr, mode),
    `parseType returned null for "${typeExpr}" in ${mode}`
  ).not.toBe(null)
}

// ============================================================================
// Test fixtures — organized by category, mirror napi/ox-jsdoc/test/parsed-type.test.ts
// ============================================================================

const BASIC_ALL_MODES: string[] = [
  'boolean',
  'string',
  'number',
  'Window',
  'MyClass',
  'null',
  'undefined',
  '*',
  '?'
]

const BASIC_JSDOC_CLOSURE: string[] = ['My-1st-Class']

const UNION_TYPES: string[] = [
  'string | number',
  'string | number | boolean',
  'number|boolean',
  'string | null',
  'string | undefined',
  'string | number | null | undefined',
  '!number | !string'
]

const GENERIC_TYPES: string[] = [
  'Array<string>',
  'Map<string, number>',
  'Promise<void>',
  'Array<string | number>'
]

const GENERIC_DOT_TYPES: string[] = [
  'Array.<string>',
  'Object.<string, number>',
  'Array.<{length}>',
  'Array.<?>',
  'Promise.<string>'
]

const MODIFIER_TYPES: string[] = [
  '?number',
  'number?',
  '!Object',
  'Object!',
  'number=',
  '...number',
  '...*',
  '...null',
  '...undefined',
  '...?',
  '...?number',
  '...number?',
  '...!Object',
  '...Object!'
]

const FUNCTION_TYPES_JSDOC: string[] = [
  'function()',
  'function(string)',
  'function(string, boolean)',
  'function(): number',
  'function(string): boolean',
  'function(string, boolean): boolean',
  'function(...foo)'
]

const ARROW_FUNCTION_TYPES: string[] = [
  '() => void',
  '() => string',
  '(x: number) => string',
  '(x: number, y: string) => boolean'
]

const OBJECT_TYPES: string[] = ['{}', '{a: string}', '{a: string, b: number}']

const TS_TYPES: string[] = [
  'A & B',
  'A & B & C',
  'keyof T',
  'typeof x',
  'infer T',
  'asserts x',
  'asserts x is string',
  'x is string',
  'unique symbol',
  'readonly string[]',
  'T extends U ? X : Y'
]

const TUPLE_TYPES: string[] = ['[]', '[string]', '[string, number]', '[a: string, b: number]']

const NAME_PATH_TYPES: string[] = ['goog.ui.Menu']

const NAME_PATH_JSDOC: string[] = ['MyClass#myMember', 'MyClass~myMember']

const ARRAY_BRACKET_TYPES: string[] = ['string[]', 'number[][]']

const PAREN_TYPES: string[] = ['(string)', '(string | number)']

const LITERAL_TYPES: string[] = ['42', '3.14', '-1', '"hello"', "'world'"]

// ============================================================================
// Test suites
// ============================================================================

describe('parseType / parseTypeCheck — standalone type expression API', () => {
  describe('basic types — all modes', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of BASIC_ALL_MODES) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('basic types — jsdoc/closure only', () => {
    for (const mode of ['jsdoc', 'closure'] as Mode[]) {
      for (const type of BASIC_JSDOC_CLOSURE) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('union types', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of UNION_TYPES) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('generic types — angle brackets', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of GENERIC_TYPES) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('generic types — dot notation', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of GENERIC_DOT_TYPES) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('modifier types (nullable, optional, variadic)', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of MODIFIER_TYPES) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('function types — jsdoc/closure', () => {
    for (const mode of ['jsdoc', 'closure'] as Mode[]) {
      for (const type of FUNCTION_TYPES_JSDOC) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('arrow function types — typescript', () => {
    for (const type of ARROW_FUNCTION_TYPES) {
      it(`${type}`, () => expectAccepts(type, 'typescript'))
    }
  })

  describe('object types', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of OBJECT_TYPES) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('typescript-specific types', () => {
    for (const type of TS_TYPES) {
      it(`${type}`, () => expectAccepts(type, 'typescript'))
    }
  })

  describe('tuple types', () => {
    for (const type of TUPLE_TYPES) {
      it(`${type}`, () => expectAccepts(type, 'typescript'))
    }
  })

  describe('name paths — all modes', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of NAME_PATH_TYPES) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('name paths — jsdoc/closure', () => {
    for (const mode of ['jsdoc', 'closure'] as Mode[]) {
      for (const type of NAME_PATH_JSDOC) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('array bracket shorthand', () => {
    for (const mode of ['jsdoc', 'typescript'] as Mode[]) {
      for (const type of ARRAY_BRACKET_TYPES) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('parenthesized types', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of PAREN_TYPES) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('literal types', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of LITERAL_TYPES) {
        it(`${type} (${mode})`, () => expectAccepts(type, mode))
      }
    }
  })

  describe('return value contract', () => {
    it('parseType returns a non-empty string for a valid type', () => {
      const result = parseType('string', 'jsdoc')
      expect(typeof result).toBe('string')
      expect(result).toBe('string')
    })

    it('parseType returns null for clearly invalid input', () => {
      expect(parseType('@', 'jsdoc')).toBeNull()
    })

    it('parseTypeCheck returns true for a valid type', () => {
      expect(parseTypeCheck('string', 'jsdoc')).toBe(true)
    })

    it('parseTypeCheck returns false for clearly invalid input', () => {
      expect(parseTypeCheck('@', 'jsdoc')).toBe(false)
    })

    it('defaults to jsdoc mode when mode is omitted', () => {
      // `Array.<string>` is jsdoc-specific syntax — should parse without explicit mode.
      expect(parseTypeCheck('Array.<string>')).toBe(true)
    })

    it('roundtrip: parseType(parseType(x)) === parseType(x)', () => {
      const inputs: Array<{ source: string; mode: Mode }> = [
        { source: 'string | number', mode: 'typescript' },
        { source: 'Array<string>', mode: 'typescript' },
        { source: 'function(string): number', mode: 'jsdoc' },
        { source: 'Array.<string>', mode: 'jsdoc' }
      ]
      for (const { source, mode } of inputs) {
        const first = parseType(source, mode)
        expect(first, `${source} failed first parse`).not.toBeNull()
        const second = parseType(first!, mode)
        expect(second, `${source} → ${first} failed second parse`).toBe(first)
      }
    })
  })
})
