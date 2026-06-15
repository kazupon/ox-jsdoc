/**
 * L5: JS integration tests — Dynamic comparison with jsdoc-type-pratt-parser.
 *
 * For each type expression input, parses with both ox-jsdoc (napi) and
 * jsdoc-type-pratt-parser, then compares the AST output.
 */

import { describe, expect, it } from 'vite-plus/test'
import { parse } from '../src-js/index.js'
import { parse as jtpParse } from 'jsdoc-type-pratt-parser'

type Mode = 'jsdoc' | 'closure' | 'typescript'
type TypeCase = { input: string; mode: Mode }
const PARSER_MODES: Mode[] = ['jsdoc', 'closure', 'typescript']

/**
 * Parse a type expression with ox-jsdoc and return the parsedType AST.
 */
function oxParse(typeExpr: string, mode: Mode): unknown {
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
function refParse(typeExpr: string, mode: Mode): unknown {
  try {
    return jtpParse(typeExpr, mode)
  } catch {
    return null
  }
}

function casesFor(inputs: readonly string[], modes: readonly Mode[]): TypeCase[] {
  return inputs.flatMap(input => modes.map(mode => ({ input, mode })))
}

function splitByReferenceSupport(cases: readonly TypeCase[]): {
  comparable: TypeCase[]
  oxOnly: TypeCase[]
} {
  const comparable: TypeCase[] = []
  const oxOnly: TypeCase[] = []

  for (const typeCase of cases) {
    if (refParse(typeCase.input, typeCase.mode) === null) {
      oxOnly.push(typeCase)
    } else {
      comparable.push(typeCase)
    }
  }

  return { comparable, oxOnly }
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

function normalizeReferenceType(obj: unknown): unknown {
  if (obj === null || obj === undefined) {
    return obj
  }
  if (typeof obj !== 'object') {
    return obj
  }
  if (Array.isArray(obj)) {
    return obj.map(normalizeReferenceType)
  }

  const o = obj as Record<string, unknown>
  const result: Record<string, unknown> = {}
  const toRecord = (value: unknown): Record<string, unknown> | null =>
    value && typeof value === 'object' && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : null

  for (const [key, value] of Object.entries(o)) {
    const normalized = normalizeReferenceType(value)
    if (normalized !== undefined) {
      result[key] = normalized
    }
  }

  if (o.type === 'JsdocTypeConditional') {
    const extendsType = toRecord(result.extendsType)
    const meta = toRecord(extendsType?.meta)
    const element = toRecord(extendsType?.element)

    if (
      extendsType &&
      extendsType.type === 'JsdocTypeNullable' &&
      meta &&
      meta.position === 'suffix' &&
      element &&
      element.type === 'JsdocTypeName'
    ) {
      result.extendsType = {
        type: 'JsdocTypeKeyof',
        element
      }
    }

    const trueType = toRecord(result.trueType)
    const trueTypeLeft = toRecord(trueType?.left)
    const trueTypeRight = toRecord(trueType?.right)
    const indexFromNamePathRight = (
      right: Record<string, unknown> | null
    ): Record<string, unknown> | null => {
      if (!right) {
        return null
      }
      if (right.type === 'JsdocTypeProperty' && typeof right.value === 'string') {
        return {
          type: 'JsdocTypeName',
          value: right.value
        }
      }
      if (right.type === 'JsdocTypeIndexedAccessIndex') {
        return (
          indexFromNamePathRight(toRecord(right.right) as Record<string, unknown> | null) ??
          indexFromNamePathRight(toRecord(right.left) as Record<string, unknown> | null) ??
          indexFromNamePathRight(toRecord(right.element) as Record<string, unknown> | null) ??
          right
        )
      }
      if (right.type === 'JsdocTypeName') {
        return right
      }
      return right
    }

    if (
      trueType &&
      trueType.type === 'JsdocTypeNamePath' &&
      trueType.pathType === 'property-brackets' &&
      trueTypeLeft &&
      trueTypeRight &&
      (trueTypeRight.type === 'JsdocTypeProperty' ||
        trueTypeRight.type === 'JsdocTypeIndexedAccessIndex' ||
        trueTypeRight.type === 'JsdocTypeName')
    ) {
      const indexValue = indexFromNamePathRight(trueTypeRight)
      result.trueType = {
        type: 'JsdocTypeIndexedAccessIndex',
        left: trueTypeLeft,
        index: indexValue ?? trueTypeRight
      }
    }
  }

  if (o.type === 'JsdocTypeGeneric' && o.infer === true) {
    const elements = o.elements
    if (Array.isArray(elements)) {
      result.elements = elements.map(element => {
        if (element && typeof element === 'object' && !Array.isArray(element)) {
          return {
            type: 'JsdocTypeInfer',
            element: normalizeReferenceType(element)
          }
        }
        return normalizeReferenceType(element)
      })
    }
    delete result.infer
  }

  return result
}

/**
 * Compare ox-jsdoc output with jsdoc-type-pratt-parser output.
 */
function compareType(typeExpr: string, mode: Mode): boolean {
  const ox = oxParse(typeExpr, mode)
  const ref = refParse(typeExpr, mode)

  expect(
    ref,
    `jsdoc-type-pratt-parser failed to parse "${typeExpr}" in ${mode} mode`
  ).not.toBeNull()

  expect(ox, `ox-jsdoc failed to parse "${typeExpr}" in ${mode} mode`).not.toBeNull()
  expect(normalizeReferenceType(ox)).toEqual(normalizeReferenceType(stripRefOnly(ref)))

  return true
}

function parseTypeWithOx(typeExpr: string, mode: Mode): boolean {
  expect(
    oxParse(typeExpr, mode),
    `ox-jsdoc failed to parse "${typeExpr}" in ${mode} mode`
  ).not.toBeNull()

  return true
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

const { comparable: MODIFIER_COMPARABLE_CASES } = splitByReferenceSupport(
  casesFor(MODIFIER_TYPES, PARSER_MODES)
)

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
const { comparable: OBJECT_OPTIONAL_COMPARABLE_CASES, oxOnly: OBJECT_OPTIONAL_OX_ONLY_CASES } =
  splitByReferenceSupport(casesFor(['{a?: string}'], ['typescript', 'closure']))

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

const { comparable: TS_COMPARABLE_CASES, oxOnly: TS_OX_ONLY_CASES } = splitByReferenceSupport(
  casesFor(TS_TYPES, ['typescript'])
)

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

const { comparable: LITERAL_COMPARABLE_CASES, oxOnly: LITERAL_OX_ONLY_CASES } =
  splitByReferenceSupport(casesFor(LITERAL_TYPES, PARSER_MODES))

const COMPLEX_TYPES: TypeCase[] = [
  ...casesFor(['(number | boolean)'], PARSER_MODES),
  ...casesFor(['...(number | boolean)'], PARSER_MODES),
  ...casesFor(['?(number | boolean)'], PARSER_MODES),
  ...casesFor(['!(number | boolean)'], PARSER_MODES),
  ...casesFor(['(number | boolean)='], PARSER_MODES),
  ...casesFor(['Array<string> | Map<string, number>'], ['typescript']),
  ...casesFor(['...Array.<string>'], PARSER_MODES),
  ...casesFor(['...{myNum: number}'], PARSER_MODES),
  ...casesFor(['?{myNum: number}'], PARSER_MODES),
  ...casesFor(['!{myNum: number}'], PARSER_MODES),
  ...casesFor(['{myNum: number}='], PARSER_MODES),
  ...casesFor(['function(string)='], PARSER_MODES),
  ...casesFor(['...function(string, boolean)'], PARSER_MODES),
  ...casesFor(['function(string, boolean): boolean'], PARSER_MODES),
  ...casesFor(['function(): (number | string)'], PARSER_MODES),
  ...casesFor(['Object='], PARSER_MODES),
  ...casesFor(['K extends keyof T ? T[K] : never'], ['typescript']),
  ...casesFor(['T extends Array<infer U> ? U : never'], ['typescript']),
  ...casesFor(['readonly [string, number]'], ['typescript']),
  ...casesFor(['{myNum: number; myObject: string}'], PARSER_MODES),
  ...casesFor(['{myArray: Array.<string>}'], PARSER_MODES)
]

const { comparable: COMPLEX_COMPARABLE_CASES, oxOnly: COMPLEX_OX_ONLY_CASES } =
  splitByReferenceSupport(COMPLEX_TYPES)

// ============================================================================
// Test suites
// ============================================================================

describe('L5: parsedType comparison with jsdoc-type-pratt-parser', () => {
  describe('basic types — all modes', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of BASIC_ALL_MODES) {
        it(`${type} (${mode})`, () => {
          expect(compareType(type, mode)).toBe(true)
        })
      }
    }
  })

  describe('basic types — jsdoc/closure only', () => {
    for (const mode of ['jsdoc', 'closure'] as Mode[]) {
      for (const type of BASIC_JSDOC_CLOSURE) {
        it(`${type} (${mode})`, () => {
          expect(compareType(type, mode)).toBe(true)
        })
      }
    }
  })

  describe('union types', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of UNION_TYPES) {
        it(`${type} (${mode})`, () => {
          expect(compareType(type, mode)).toBe(true)
        })
      }
    }
  })

  describe('generic types — angle brackets', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of GENERIC_TYPES) {
        it(`${type} (${mode})`, () => {
          expect(compareType(type, mode)).toBe(true)
        })
      }
    }
  })

  describe('generic types — dot notation', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of GENERIC_DOT_TYPES) {
        it(`${type} (${mode})`, () => {
          expect(compareType(type, mode)).toBe(true)
        })
      }
    }
  })

  describe('modifier types (nullable, optional, variadic)', () => {
    for (const { input, mode } of MODIFIER_COMPARABLE_CASES) {
      it(`${input} (${mode})`, () => {
        expect(compareType(input, mode)).toBe(true)
      })
    }
  })

  describe('function types — jsdoc/closure', () => {
    for (const mode of ['jsdoc', 'closure'] as Mode[]) {
      for (const type of FUNCTION_TYPES_JSDOC) {
        it(`${type} (${mode})`, () => {
          expect(compareType(type, mode)).toBe(true)
        })
      }
    }
  })

  describe('arrow function types — typescript', () => {
    for (const type of ARROW_FUNCTION_TYPES) {
      it(`${type}`, () => {
        expect(compareType(type, 'typescript')).toBe(true)
      })
    }
  })

  describe('object types', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of OBJECT_TYPES) {
        it(`${type} (${mode})`, () => {
          expect(compareType(type, mode)).toBe(true)
        })
      }
    }
    for (const { input, mode } of OBJECT_OPTIONAL_COMPARABLE_CASES) {
      it(`${input} (${mode})`, () => {
        expect(compareType(input, mode)).toBe(true)
      })
    }
  })

  describe('object types — ox-jsdoc only', () => {
    for (const { input, mode } of OBJECT_OPTIONAL_OX_ONLY_CASES) {
      it(`${input} (${mode})`, () => {
        expect(parseTypeWithOx(input, mode)).toBe(true)
      })
    }
  })

  describe('typescript-specific types', () => {
    for (const { input, mode } of TS_COMPARABLE_CASES) {
      it(`${input}`, () => {
        expect(compareType(input, mode)).toBe(true)
      })
    }
  })

  describe('typescript-specific types — ox-jsdoc only', () => {
    for (const { input, mode } of TS_OX_ONLY_CASES) {
      it(`${input}`, () => {
        expect(parseTypeWithOx(input, mode)).toBe(true)
      })
    }
  })

  describe('tuple types', () => {
    for (const type of TUPLE_TYPES) {
      it(`${type}`, () => {
        expect(compareType(type, 'typescript')).toBe(true)
      })
    }
  })

  describe('name paths — all modes', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of NAME_PATH_TYPES) {
        it(`${type} (${mode})`, () => {
          expect(compareType(type, mode)).toBe(true)
        })
      }
    }
  })

  describe('name paths — jsdoc/closure', () => {
    for (const mode of ['jsdoc', 'closure'] as Mode[]) {
      for (const type of NAME_PATH_JSDOC) {
        it(`${type} (${mode})`, () => {
          expect(compareType(type, mode)).toBe(true)
        })
      }
    }
  })

  describe('array bracket shorthand', () => {
    for (const mode of ['jsdoc', 'typescript'] as Mode[]) {
      for (const type of ARRAY_BRACKET_TYPES) {
        it(`${type} (${mode})`, () => {
          expect(compareType(type, mode)).toBe(true)
        })
      }
    }
  })

  describe('parenthesized types', () => {
    for (const mode of ['jsdoc', 'closure', 'typescript'] as Mode[]) {
      for (const type of PAREN_TYPES) {
        it(`${type} (${mode})`, () => {
          expect(compareType(type, mode)).toBe(true)
        })
      }
    }
  })

  describe('literal types', () => {
    for (const { input, mode } of LITERAL_COMPARABLE_CASES) {
      it(`${input} (${mode})`, () => {
        expect(compareType(input, mode)).toBe(true)
      })
    }
  })

  describe('literal types — ox-jsdoc only', () => {
    for (const { input, mode } of LITERAL_OX_ONLY_CASES) {
      it(`${input} (${mode})`, () => {
        expect(parseTypeWithOx(input, mode)).toBe(true)
      })
    }
  })

  // Combination / complex types
  describe('complex combinations', () => {
    for (const { input, mode } of COMPLEX_COMPARABLE_CASES) {
      it(`${input} (${mode})`, () => {
        expect(compareType(input, mode)).toBe(true)
      })
    }
  })

  describe('complex combinations — ox-jsdoc only', () => {
    for (const { input, mode } of COMPLEX_OX_ONLY_CASES) {
      it(`${input} (${mode})`, () => {
        expect(parseTypeWithOx(input, mode)).toBe(true)
      })
    }
  })

  // Symbol types (jsdoc/closure only)
  describe('symbol types', () => {
    const symbolTypes = ['MyClass()', 'MyClass(2)', 'MyClass(abc)']
    for (const mode of ['jsdoc', 'closure'] as Mode[]) {
      for (const type of symbolTypes) {
        it(`${type} (${mode})`, () => {
          expect(compareType(type, mode)).toBe(true)
        })
      }
    }
  })

  // Special name paths
  describe('special name paths', () => {
    it("module:'path' (jsdoc)", () => {
      expect(compareType("module:'some-path'", 'jsdoc')).toBe(true)
    })
    it('module:"path" (jsdoc)', () => {
      expect(compareType('module:"some-path"', 'jsdoc')).toBe(true)
    })
    it('event:click (jsdoc)', () => {
      expect(compareType('event:click', 'jsdoc')).toBe(true)
    })
    it('external:jQuery (jsdoc)', () => {
      expect(compareType('external:jQuery', 'jsdoc')).toBe(true)
    })
  })

  // Import types
  describe('import types', () => {
    it('import("x")', () => {
      expect(compareType('import("x")', 'typescript')).toBe(true)
    })
    it('import("./x")', () => {
      expect(compareType('import("./x")', 'typescript')).toBe(true)
    })
    it('import("x").T', () => {
      expect(compareType('import("x").T', 'typescript')).toBe(true)
    })
  })
})
