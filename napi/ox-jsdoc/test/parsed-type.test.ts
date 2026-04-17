/**
 * L5: JS integration tests — Dynamic comparison with jsdoc-type-pratt-parser.
 *
 * For each type expression input, parses with both ox-jsdoc (napi) and
 * jsdoc-type-pratt-parser, then compares the AST output.
 */

import { describe, expect, it } from 'vite-plus/test'
import { parse } from '../src-js/index.js'
import { parse as jtpParse, tryParse as jtpTryParse } from 'jsdoc-type-pratt-parser'

type Mode = 'jsdoc' | 'closure' | 'typescript'

/**
 * Parse a type expression with ox-jsdoc and return the parsedType AST.
 */
function oxParse(typeExpr: string, mode: Mode): unknown | null {
  const source = `/** @param {${typeExpr}} x */`
  const result = parse(source, { parseTypes: true, typeParseMode: mode })
  if (!result.ast || result.ast.tags.length === 0) {
    return null
  }
  return result.ast.tags[0].parsedType ?? null
}

/**
 * Parse a type expression with jsdoc-type-pratt-parser.
 */
function refParse(typeExpr: string, mode: Mode): unknown | null {
  try {
    return jtpParse(typeExpr, mode)
  } catch {
    return null
  }
}

/**
 * Strip jsdoc-type-pratt-parser-specific fields from the reference output
 * so it matches ox-jsdoc's output format.
 *
 * Only removes fields that jsdoc-type-pratt-parser adds but ox-jsdoc does not:
 * - `range`, `loc` (position tracking)
 * - meta spacing fields (`parameterSpacing`, `elementSpacing`, etc.)
 */
function stripRefOnly(obj: unknown): unknown {
  if (obj === null || obj === undefined) {
    return obj
  }
  if (typeof obj !== 'object') {
    return obj
  }
  if (Array.isArray(obj)) {
    return obj.map(stripRefOnly)
  }

  const o = obj as Record<string, unknown>
  const result: Record<string, unknown> = {}

  for (const [key, value] of Object.entries(o)) {
    if (key === 'range' || key === 'loc') {
      continue
    }
    if (key === 'meta') {
      if (typeof value === 'object' && value !== null) {
        const meta: Record<string, unknown> = {}
        for (const [mk, mv] of Object.entries(value as Record<string, unknown>)) {
          // Remove spacing/formatting fields (jsdoc-type-pratt-parser only)
          if (mk.includes('Spacing') || mk.includes('spacing')) {
            continue
          }
          if (mk === 'propertyIndent' || mk === 'bracketSpacing') {
            continue
          }
          if (mk.includes('Punctuation') || mk === 'trailingPunctuation') {
            continue
          }
          if (mk === 'separatorForSingleObjectField') {
            continue
          }
          const stripped = stripRefOnly(mv)
          if (stripped !== undefined) {
            meta[mk] = stripped
          }
        }
        if (Object.keys(meta).length > 0) {
          result[key] = meta
        }
      }
      continue
    }
    const stripped = stripRefOnly(value)
    if (stripped !== undefined) {
      result[key] = stripped
    }
  }
  return result
}

/**
 * Compare ox-jsdoc output with jsdoc-type-pratt-parser output.
 */
function compareType(typeExpr: string, mode: Mode) {
  const ox = oxParse(typeExpr, mode)
  const ref = refParse(typeExpr, mode)

  if (ref === null) {
    // ref parser failed — ox-jsdoc should also fail (or we skip)
    return
  }

  expect(ox, `ox-jsdoc failed to parse "${typeExpr}" in ${mode} mode`).not.toBeNull()
  expect(ox).toEqual(stripRefOnly(ref))
}

// ============================================================================
// Test fixtures — organized by category
// ============================================================================

// Basic types that should work in all modes
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

// Types that work in jsdoc and closure (loose mode)
const BASIC_JSDOC_CLOSURE: string[] = ['My-1st-Class']

// Union types
const UNION_TYPES: string[] = [
  'string | number',
  'string | number | boolean',
  'number|boolean',
  'string | null',
  'string | undefined',
  'string | number | null | undefined',
  '!number | !string'
]

// Generic types
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

// Nullable / NotNullable / Optional
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
  '...Object!',
  'number=?',
  'number?=',
  'Object=!',
  'Object!='
]

// Function types (jsdoc/closure style)
const FUNCTION_TYPES_JSDOC: string[] = [
  'function()',
  'function(string)',
  'function(string, boolean)',
  'function(): number',
  'function(string): boolean',
  'function(string, boolean): boolean',
  'function(...foo)'
]

// Arrow function types (typescript)
const ARROW_FUNCTION_TYPES: string[] = [
  '() => void',
  '() => string',
  '(x: number) => string',
  '(x: number, y: string) => boolean'
]

// Object types
const OBJECT_TYPES: string[] = ['{}', '{a: string}', '{a: string, b: number}']

// `{a?: string}` has different semantics per mode:
// jsdoc: `?` is nullable on key → JsdocTypeJsdocObjectField with nullable
// typescript/closure: `?` is optional → JsdocTypeObjectField with optional=true
const OBJECT_OPTIONAL_TYPES: Array<{ input: string; modes: Mode[] }> = [
  { input: '{a?: string}', modes: ['typescript', 'closure'] }
]

// TypeScript-specific types
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

// Tuple types
const TUPLE_TYPES: string[] = ['[]', '[string]', '[string, number]', '[a: string, b: number]']

// Name paths
const NAME_PATH_TYPES: string[] = ['goog.ui.Menu']

const NAME_PATH_JSDOC: string[] = ['MyClass#myMember', 'MyClass~myMember']

// Array bracket shorthand
const ARRAY_BRACKET_TYPES: string[] = ['string[]', 'number[][]']

// Parenthesized
const PAREN_TYPES: string[] = ['(string)', '(string | number)']

// Number/String literals
const LITERAL_TYPES: string[] = ['42', '3.14', '-1', '"hello"', "'world'"]

// ============================================================================
// Test suites
// ============================================================================

describe('L5: parsedType comparison with jsdoc-type-pratt-parser', () => {
  describe('basic types — all modes', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of BASIC_ALL_MODES) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
  })

  describe('basic types — jsdoc/closure only', () => {
    for (const mode of ['jsdoc', 'closure'] as Mode[]) {
      for (const type of BASIC_JSDOC_CLOSURE) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
  })

  describe('union types', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of UNION_TYPES) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
  })

  describe('generic types — angle brackets', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of GENERIC_TYPES) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
  })

  describe('generic types — dot notation', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of GENERIC_DOT_TYPES) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
  })

  describe('modifier types (nullable, optional, variadic)', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of MODIFIER_TYPES) {
        it(`${type} (${mode})`, () => {
          // Some modifiers only work in certain modes — skip if ref fails
          const ref = refParse(type, mode)
          if (ref === null) {
            return
          }
          compareType(type, mode)
        })
      }
    }
  })

  describe('function types — jsdoc/closure', () => {
    for (const mode of ['jsdoc', 'closure'] as Mode[]) {
      for (const type of FUNCTION_TYPES_JSDOC) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
  })

  describe('arrow function types — typescript', () => {
    for (const type of ARROW_FUNCTION_TYPES) {
      it(`${type}`, () => compareType(type, 'typescript'))
    }
  })

  describe('object types', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of OBJECT_TYPES) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
    for (const { input, modes } of OBJECT_OPTIONAL_TYPES) {
      for (const mode of modes) {
        it(`${input} (${mode})`, () => compareType(input, mode))
      }
    }
  })

  describe('typescript-specific types', () => {
    for (const type of TS_TYPES) {
      it(`${type}`, () => compareType(type, 'typescript'))
    }
  })

  describe('tuple types', () => {
    for (const type of TUPLE_TYPES) {
      it(`${type}`, () => compareType(type, 'typescript'))
    }
  })

  describe('name paths — all modes', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of NAME_PATH_TYPES) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
  })

  describe('name paths — jsdoc/closure', () => {
    for (const mode of ['jsdoc', 'closure'] as Mode[]) {
      for (const type of NAME_PATH_JSDOC) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
  })

  describe('array bracket shorthand', () => {
    for (const mode of ['jsdoc', 'typescript'] as Mode[]) {
      for (const type of ARRAY_BRACKET_TYPES) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
  })

  describe('parenthesized types', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of PAREN_TYPES) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
  })

  describe('literal types', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of LITERAL_TYPES) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
  })

  // Combination / complex types
  describe('complex combinations', () => {
    const complexTypes: Array<{ input: string; modes: Mode[] }> = [
      { input: '(number | boolean)', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: '...(number | boolean)', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: '?(number | boolean)', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: '!(number | boolean)', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: '(number | boolean)=', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: 'Array<string> | Map<string, number>', modes: ['typescript'] },
      { input: '...Array.<string>', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: '...{myNum: number}', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: '?{myNum: number}', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: '!{myNum: number}', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: '{myNum: number}=', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: 'function(string)=', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: '...function(string, boolean)', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: 'function(string, boolean): boolean', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: 'function(): (number | string)', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: 'Object=', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: 'K extends keyof T ? T[K] : never', modes: ['typescript'] },
      { input: 'T extends Array<infer U> ? U : never', modes: ['typescript'] },
      { input: 'readonly [string, number]', modes: ['typescript'] },
      { input: '{myNum: number; myObject: string}', modes: ['jsdoc', 'closure', 'typescript'] },
      { input: '{myArray: Array.<string>}', modes: ['jsdoc', 'closure', 'typescript'] }
    ]

    for (const { input, modes } of complexTypes) {
      for (const mode of modes) {
        it(`${input} (${mode})`, () => compareType(input, mode))
      }
    }
  })

  // Symbol types (jsdoc/closure only)
  describe('symbol types', () => {
    const symbolTypes = ['MyClass()', 'MyClass(2)', 'MyClass(abc)']
    for (const mode of ['jsdoc', 'closure'] as Mode[]) {
      for (const type of symbolTypes) {
        it(`${type} (${mode})`, () => compareType(type, mode))
      }
    }
  })

  // Special name paths
  describe('special name paths', () => {
    it("module:'path' (jsdoc)", () => compareType("module:'some-path'", 'jsdoc'))
    it('module:"path" (jsdoc)', () => compareType('module:"some-path"', 'jsdoc'))
    it('event:click (jsdoc)', () => compareType('event:click', 'jsdoc'))
    it('external:jQuery (jsdoc)', () => compareType('external:jQuery', 'jsdoc'))
  })

  // Import types
  describe('import types', () => {
    it('import("x")', () => compareType('import("x")', 'typescript'))
    it('import("./x")', () => compareType('import("./x")', 'typescript'))
    it('import("x").T', () => compareType('import("x").T', 'typescript'))
  })
})
