// @ts-check

import { describe, expect, it } from 'vitest'

import { jsdocVisitorKeys } from '../src/index.js'

describe('jsdocVisitorKeys', () => {
  it('is a frozen object', () => {
    expect(Object.isFrozen(jsdocVisitorKeys)).toBe(true)
  })

  it('covers every Comment AST kind (15)', () => {
    const expected = [
      'JsdocBlock',
      'JsdocDescriptionLine',
      'JsdocTag',
      'JsdocTagName',
      'JsdocTagNameValue',
      'JsdocTypeSource',
      'JsdocTypeLine',
      'JsdocInlineTag',
      'JsdocGenericTagBody',
      'JsdocBorrowsTagBody',
      'JsdocRawTagBody',
      'JsdocParameterName',
      'JsdocNamepathSource',
      'JsdocIdentifier',
      'JsdocText'
    ]
    for (const kind of expected) {
      expect(jsdocVisitorKeys, kind).toHaveProperty(kind)
      expect(Array.isArray(jsdocVisitorKeys[kind]), `${kind} is array`).toBe(true)
    }
  })

  it('covers every TypeNode kind (45)', () => {
    const expected = [
      'TypeName',
      'TypeNumber',
      'TypeStringValue',
      'TypeNull',
      'TypeUndefined',
      'TypeAny',
      'TypeUnknown',
      'TypeUnion',
      'TypeIntersection',
      'TypeGeneric',
      'TypeFunction',
      'TypeObject',
      'TypeTuple',
      'TypeParenthesis',
      'TypeNamePath',
      'TypeSpecialNamePath',
      'TypeNullable',
      'TypeNotNullable',
      'TypeOptional',
      'TypeVariadic',
      'TypeConditional',
      'TypeInfer',
      'TypeKeyOf',
      'TypeTypeOf',
      'TypeImport',
      'TypePredicate',
      'TypeAsserts',
      'TypeAssertsPlain',
      'TypeReadonlyArray',
      'TypeTemplateLiteral',
      'TypeUniqueSymbol',
      'TypeSymbol',
      'TypeObjectField',
      'TypeJsdocObjectField',
      'TypeKeyValue',
      'TypeProperty',
      'TypeIndexSignature',
      'TypeMappedType',
      'TypeTypeParameter',
      'TypeCallSignature',
      'TypeConstructorSignature',
      'TypeMethodSignature',
      'TypeIndexedAccessIndex',
      'TypeParameterList',
      'TypeReadonlyProperty'
    ]
    expect(expected).toHaveLength(45)
    for (const kind of expected) {
      expect(jsdocVisitorKeys, kind).toHaveProperty(kind)
      expect(Array.isArray(jsdocVisitorKeys[kind]), `${kind} is array`).toBe(true)
    }
  })

  it('matches the jsdoccomment-canonical shape on the 5 user-facing kinds', () => {
    expect(jsdocVisitorKeys.JsdocBlock).toEqual(['descriptionLines', 'tags', 'inlineTags'])
    expect(jsdocVisitorKeys.JsdocDescriptionLine).toEqual([])
    expect(jsdocVisitorKeys.JsdocTypeLine).toEqual([])
    expect(jsdocVisitorKeys.JsdocInlineTag).toEqual([])
    // ox-jsdoc adds tag/rawType/name/body as child nodes (jsdoccomment flattens
    // them); these extra keys come first, then the 4 canonical ones.
    expect(jsdocVisitorKeys.JsdocTag).toEqual([
      'tag',
      'rawType',
      'name',
      'parsedType',
      'body',
      'typeLines',
      'descriptionLines',
      'inlineTags'
    ])
  })
})
