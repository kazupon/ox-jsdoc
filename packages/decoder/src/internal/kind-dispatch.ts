/**
 * Kind → class dispatch table.
 *
 * Mirrors the Rust `Kind::from_u8` mapping. Phase 4 will code-generate
 * this file from a single schema. Until then, every Kind discriminant
 * (0x01-0x0F, 0x7F, 0x80-0xAC) is wired by hand.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import {
  RemoteJsdocBlock,
  RemoteJsdocBorrowsTagBody,
  RemoteJsdocDescriptionLine,
  RemoteJsdocGenericTagBody,
  RemoteJsdocIdentifier,
  RemoteJsdocInlineTag,
  RemoteJsdocNamepathSource,
  RemoteJsdocParameterName,
  RemoteJsdocRawTagBody,
  RemoteJsdocTag,
  RemoteJsdocTagName,
  RemoteJsdocTagNameValue,
  RemoteJsdocText,
  RemoteJsdocTypeLine,
  RemoteJsdocTypeSource
} from './nodes/jsdoc.ts'
import { RemoteNodeListNode } from './nodes/node-list-node.ts'
import {
  RemoteTypeAny,
  RemoteTypeAsserts,
  RemoteTypeAssertsPlain,
  RemoteTypeCallSignature,
  RemoteTypeConditional,
  RemoteTypeConstructorSignature,
  RemoteTypeFunction,
  RemoteTypeGeneric,
  RemoteTypeImport,
  RemoteTypeIndexedAccessIndex,
  RemoteTypeIndexSignature,
  RemoteTypeInfer,
  RemoteTypeIntersection,
  RemoteTypeJsdocObjectField,
  RemoteTypeKeyOf,
  RemoteTypeKeyValue,
  RemoteTypeMappedType,
  RemoteTypeMethodSignature,
  RemoteTypeName,
  RemoteTypeNamePath,
  RemoteTypeNotNullable,
  RemoteTypeNull,
  RemoteTypeNullable,
  RemoteTypeNumber,
  RemoteTypeObject,
  RemoteTypeObjectField,
  RemoteTypeOptional,
  RemoteTypeParameterList,
  RemoteTypeParenthesis,
  RemoteTypePredicate,
  RemoteTypeProperty,
  RemoteTypeReadonlyArray,
  RemoteTypeReadonlyProperty,
  RemoteTypeSpecialNamePath,
  RemoteTypeStringValue,
  RemoteTypeSymbol,
  RemoteTypeTemplateLiteral,
  RemoteTypeTuple,
  RemoteTypeTypeOf,
  RemoteTypeTypeParameter,
  RemoteTypeUndefined,
  RemoteTypeUnion,
  RemoteTypeUniqueSymbol,
  RemoteTypeUnknown,
  RemoteTypeVariadic
} from './nodes/type-nodes.ts'

import type { LazyNodeConstructor } from './types.ts'

/**
 * Flat 256-entry table indexed by the Kind byte. `undefined` entries fall
 * into the reserved space and trip an explicit error in `decodeKindToClass`.
 */
const KIND_TABLE: Array<LazyNodeConstructor | undefined> = Array.from({ length: 256 })

// Comment AST (0x01 - 0x0F)
KIND_TABLE[0x01] = RemoteJsdocBlock
KIND_TABLE[0x02] = RemoteJsdocDescriptionLine
KIND_TABLE[0x03] = RemoteJsdocTag
KIND_TABLE[0x04] = RemoteJsdocTagName
KIND_TABLE[0x05] = RemoteJsdocTagNameValue
KIND_TABLE[0x06] = RemoteJsdocTypeSource
KIND_TABLE[0x07] = RemoteJsdocTypeLine
KIND_TABLE[0x08] = RemoteJsdocInlineTag
KIND_TABLE[0x09] = RemoteJsdocGenericTagBody
KIND_TABLE[0x0a] = RemoteJsdocBorrowsTagBody
KIND_TABLE[0x0b] = RemoteJsdocRawTagBody
KIND_TABLE[0x0c] = RemoteJsdocParameterName
KIND_TABLE[0x0d] = RemoteJsdocNamepathSource
KIND_TABLE[0x0e] = RemoteJsdocIdentifier
KIND_TABLE[0x0f] = RemoteJsdocText

// NodeList wrapper
KIND_TABLE[0x7f] = RemoteNodeListNode

// TypeNode (0x80 - 0xAC)
KIND_TABLE[0x80] = RemoteTypeName
KIND_TABLE[0x81] = RemoteTypeNumber
KIND_TABLE[0x82] = RemoteTypeStringValue
KIND_TABLE[0x83] = RemoteTypeNull
KIND_TABLE[0x84] = RemoteTypeUndefined
KIND_TABLE[0x85] = RemoteTypeAny
KIND_TABLE[0x86] = RemoteTypeUnknown
KIND_TABLE[0x87] = RemoteTypeUnion
KIND_TABLE[0x88] = RemoteTypeIntersection
KIND_TABLE[0x89] = RemoteTypeGeneric
KIND_TABLE[0x8a] = RemoteTypeFunction
KIND_TABLE[0x8b] = RemoteTypeObject
KIND_TABLE[0x8c] = RemoteTypeTuple
KIND_TABLE[0x8d] = RemoteTypeParenthesis
KIND_TABLE[0x8e] = RemoteTypeNamePath
KIND_TABLE[0x8f] = RemoteTypeSpecialNamePath
KIND_TABLE[0x90] = RemoteTypeNullable
KIND_TABLE[0x91] = RemoteTypeNotNullable
KIND_TABLE[0x92] = RemoteTypeOptional
KIND_TABLE[0x93] = RemoteTypeVariadic
KIND_TABLE[0x94] = RemoteTypeConditional
KIND_TABLE[0x95] = RemoteTypeInfer
KIND_TABLE[0x96] = RemoteTypeKeyOf
KIND_TABLE[0x97] = RemoteTypeTypeOf
KIND_TABLE[0x98] = RemoteTypeImport
KIND_TABLE[0x99] = RemoteTypePredicate
KIND_TABLE[0x9a] = RemoteTypeAsserts
KIND_TABLE[0x9b] = RemoteTypeAssertsPlain
KIND_TABLE[0x9c] = RemoteTypeReadonlyArray
KIND_TABLE[0x9d] = RemoteTypeTemplateLiteral
KIND_TABLE[0x9e] = RemoteTypeUniqueSymbol
KIND_TABLE[0x9f] = RemoteTypeSymbol
KIND_TABLE[0xa0] = RemoteTypeObjectField
KIND_TABLE[0xa1] = RemoteTypeJsdocObjectField
KIND_TABLE[0xa2] = RemoteTypeKeyValue
KIND_TABLE[0xa3] = RemoteTypeProperty
KIND_TABLE[0xa4] = RemoteTypeIndexSignature
KIND_TABLE[0xa5] = RemoteTypeMappedType
KIND_TABLE[0xa6] = RemoteTypeTypeParameter
KIND_TABLE[0xa7] = RemoteTypeCallSignature
KIND_TABLE[0xa8] = RemoteTypeConstructorSignature
KIND_TABLE[0xa9] = RemoteTypeMethodSignature
KIND_TABLE[0xaa] = RemoteTypeIndexedAccessIndex
KIND_TABLE[0xab] = RemoteTypeParameterList
KIND_TABLE[0xac] = RemoteTypeReadonlyProperty

/**
 * Look up the lazy class for a given Kind byte.
 */
export function decodeKindToClass(kind: number): LazyNodeConstructor {
  const Class = KIND_TABLE[kind]
  if (Class === undefined) {
    throw new Error(`unknown Kind: 0x${kind.toString(16).padStart(2, '0')}`)
  }
  return Class
}
