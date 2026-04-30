/**
 * `@ox-jsdoc/decoder` — shared JS lazy decoder for the ox-jsdoc Binary AST.
 *
 * Phase 1.1d: hand-written first version. Phase 4 will code-generate every
 * Remote* class and the Kind dispatch table from a single schema.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

export { RemoteSourceFile } from './internal/source-file.ts'
export { EMPTY_NODE_LIST, RemoteNodeList } from './internal/node-list.ts'

// Comment AST classes — exported so consumers can `instanceof` check them.
export {
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
} from './internal/nodes/jsdoc.ts'

// TypeNode classes (45).
export {
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
} from './internal/nodes/type-nodes.ts'

export { RemoteNodeListNode } from './internal/nodes/node-list-node.ts'

/**
 * Visitor keys for every Remote* node kind (60 = 15 Comment AST + 45 TypeNode).
 *
 * Each entry maps a node `type` name to the **traversable child property
 * names** in canonical visit order. Mirrors the jsdoccomment / ESLint
 * `visitorKeys` convention — frameworks that depend on it (`estraverse`,
 * `eslint-visitor-keys`, etc.) can spread this object directly into their
 * own key map.
 *
 * **Differences from jsdoccomment**: ox-jsdoc emits `JsdocTag.tag` /
 * `rawType` / `name` / `body` as actual child nodes (`RemoteJsdocTagName`,
 * `RemoteJsdocTypeSource`, `RemoteJsdocTagNameValue`, `RemoteJsdocTagBody`)
 * instead of flattening them to strings, so those property names appear in
 * `JsdocTag`'s key list. Strict-jsdoccomment consumers can filter the list
 * down to `['parsedType', 'typeLines', 'descriptionLines', 'inlineTags']`.
 *
 * **Reserved kinds** (`JsdocBorrowsTagBody`, `JsdocRawTagBody`) are listed
 * for future use; the parser does not currently emit them
 * (see `design/007-binary-ast/ast-nodes.md` "Reserved Kinds").
 */
export const jsdocVisitorKeys = Object.freeze({
  // Comment AST (15)
  JsdocBlock: ['descriptionLines', 'tags', 'inlineTags'],
  JsdocDescriptionLine: [],
  JsdocTag: [
    'tag',
    'rawType',
    'name',
    'parsedType',
    'body',
    'typeLines',
    'descriptionLines',
    'inlineTags'
  ],
  JsdocTagName: [],
  JsdocTagNameValue: [],
  JsdocTypeSource: [],
  JsdocTypeLine: [],
  JsdocInlineTag: [],
  JsdocGenericTagBody: ['typeSource', 'value'],
  JsdocBorrowsTagBody: ['source', 'target'],
  JsdocRawTagBody: [],
  JsdocParameterName: [],
  JsdocNamepathSource: [],
  JsdocIdentifier: [],
  JsdocText: [],

  // TypeNode — leaves (10)
  TypeName: [],
  TypeNumber: [],
  TypeStringValue: [],
  TypeProperty: [],
  TypeSpecialNamePath: [],
  TypeNull: [],
  TypeUndefined: [],
  TypeAny: [],
  TypeUnknown: [],
  TypeUniqueSymbol: [],

  // TypeNode — elements containers (6)
  TypeUnion: ['elements'],
  TypeIntersection: ['elements'],
  TypeObject: ['elements'],
  TypeTuple: ['elements'],
  TypeTypeParameter: ['elements'],
  TypeParameterList: ['elements'],

  // TypeNode — single-child containers (13)
  TypeParenthesis: ['element'],
  TypeInfer: ['element'],
  TypeKeyOf: ['element'],
  TypeTypeOf: ['element'],
  TypeImport: ['element'],
  TypeAssertsPlain: ['element'],
  TypeReadonlyArray: ['element'],
  TypeIndexedAccessIndex: ['element'],
  TypeReadonlyProperty: ['element'],
  TypeNullable: ['element'],
  TypeNotNullable: ['element'],
  TypeOptional: ['element'],
  TypeVariadic: ['element'],

  // TypeNode — left+right (3)
  TypePredicate: ['left', 'right'],
  TypeAsserts: ['left', 'right'],
  TypeNamePath: ['left', 'right'],

  // TypeNode — mixed shapes (13)
  TypeGeneric: ['left', 'elements'],
  TypeFunction: ['parameters', 'returnType', 'typeParameters'],
  TypeConditional: ['checkType', 'extendsType', 'trueType', 'falseType'],
  TypeObjectField: ['key', 'right'],
  TypeJsdocObjectField: ['key', 'right'],
  TypeKeyValue: ['right'],
  TypeIndexSignature: ['right'],
  TypeMappedType: ['right'],
  TypeMethodSignature: ['parameters', 'returnType', 'typeParameters'],
  TypeCallSignature: ['parameters', 'returnType', 'typeParameters'],
  TypeConstructorSignature: ['parameters', 'returnType', 'typeParameters'],
  // TypeTemplateLiteral interpolations are stored as direct children but
  // are not exposed via a named property on the lazy class today; consumers
  // that need to walk them should iterate the node's children directly.
  TypeTemplateLiteral: [],
  TypeSymbol: ['element']
})

/**
 * Recursively convert a Remote* lazy node into a plain JSON object.
 * Handy for browser DevTools (where `Symbol.for('nodejs.util.inspect.custom')`
 * has no effect) and for general logging.
 */
export function toPlainObject(node: unknown): unknown {
  if (node === null || node === undefined) {
    return node
  }
  if (typeof node !== 'object') {
    return node
  }
  if (Array.isArray(node)) {
    return node.map(toPlainObject)
  }
  const candidate = node as { toJSON?: () => unknown }
  if (typeof candidate.toJSON === 'function') {
    return candidate.toJSON()
  }
  return node
}
