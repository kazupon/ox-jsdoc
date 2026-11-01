import { describe, expect, it } from 'vite-plus/test'
import * as decoder from '../src/index.ts'
import { jsdocVisitorKeys, RemoteSourceFile } from '../src/index.ts'
import type { RemoteJsdocBorrowsTagBody } from '../src/index.ts'
import { FixtureBuilder } from './fixture-builder.ts'

const visitorKeys = jsdocVisitorKeys as Readonly<Record<string, readonly string[]>>

/** Whether `name` is a getter of `prototype` or of a prototype that it inherits. */
function hasGetter(prototype: object, name: string): boolean {
  for (
    let p: object | null = prototype;
    p !== null && p !== Object.prototype;
    p = Object.getPrototypeOf(p)
  ) {
    if (Object.getOwnPropertyDescriptor(p, name)?.get !== undefined) {
      return true
    }
  }
  return false
}

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
      expect(visitorKeys).toHaveProperty(kind)
      expect(Array.isArray(visitorKeys[kind])).toBe(true)
    }
  })

  it('covers every TypeNode kind (48)', () => {
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
      'TypeReadonlyProperty',
      'TypeComputedProperty',
      'TypeComputedMethod',
      'TypeBigInt'
    ]
    expect(expected).toHaveLength(48)
    for (const kind of expected) {
      expect(visitorKeys).toHaveProperty(kind)
      expect(Array.isArray(visitorKeys[kind])).toBe(true)
    }
  })

  it('has no children for a BigInt literal type', () => {
    expect(jsdocVisitorKeys.TypeBigInt).toEqual([])
  })

  it('walks the parts of computed keys', () => {
    expect(jsdocVisitorKeys.TypeComputedProperty).toEqual(['value'])
    // In the order of the other function-like kinds (TypeFunction etc.),
    // after the key itself.
    expect(jsdocVisitorKeys.TypeComputedMethod).toEqual([
      'value',
      'parameters',
      'returnType',
      'typeParameters'
    ])
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

describe('the classes of the visitor keys', () => {
  it('have a getter for every key, also the reserved kinds', () => {
    const classes = decoder as unknown as Record<string, { prototype: object } | undefined>
    const missing: string[] = []
    for (const [kind, keys] of Object.entries(visitorKeys)) {
      const Class = classes[`Remote${kind}`]
      if (Class === undefined) {
        missing.push(`${kind} (no class)`)
        continue
      }
      for (const key of keys) {
        if (!hasGetter(Class.prototype, key)) {
          missing.push(`${kind}.${key}`)
        }
      }
    }
    expect(missing).toEqual([])
  })

  it('read the source and the target of a JsdocBorrowsTagBody', () => {
    // `@borrows a as b`: a JsdocBorrowsTagBody (Kind 0x0A, Children type)
    // whose two children are JsdocNamepathSource (Kind 0x0D) leaves.
    const b = new FixtureBuilder()
    const body = b.emitChildrenNode({ parentIndex: 0, kind: 0x0a, bitmask: 0b11 })
    b.emitStringNode({ parentIndex: body, kind: 0x0d, value: 'a' })
    b.emitStringNode({ parentIndex: body, kind: 0x0d, value: 'b' })
    b.pushRoot({ nodeIndex: body })
    const borrows = new RemoteSourceFile(b.build()).asts[0] as RemoteJsdocBorrowsTagBody
    const leaf = (node: unknown) => node as { type: string; raw: string; parent: unknown }
    expect(leaf(borrows.source)).toMatchObject({ type: 'JsdocNamepathSource', raw: 'a' })
    expect(leaf(borrows.target)).toMatchObject({ type: 'JsdocNamepathSource', raw: 'b' })
    expect(leaf(borrows.target).parent).toBe(borrows)
  })
})
