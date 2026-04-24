/**
 * `@ox-jsdoc/decoder` — shared JS lazy decoder for the ox-jsdoc Binary AST.
 *
 * Phase 1.1d: hand-written first version. Phase 4 will code-generate every
 * Remote* class and the Kind dispatch table from a single schema.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

export { RemoteSourceFile } from './internal/source-file.js'
export { EMPTY_NODE_LIST, RemoteNodeList } from './internal/node-list.js'

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
} from './internal/nodes/jsdoc.js'

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
} from './internal/nodes/type-nodes.js'

export { RemoteNodeListNode } from './internal/nodes/node-list-node.js'

/**
 * Recursively convert a Remote* lazy node into a plain JSON object.
 * Handy for browser DevTools (where `Symbol.for('nodejs.util.inspect.custom')`
 * has no effect) and for general logging.
 *
 * @param {unknown} node
 * @returns {unknown}
 */
export function toPlainObject(node) {
  if (node === null || node === undefined) {
    return node
  }
  if (typeof node !== 'object') {
    return node
  }
  if (Array.isArray(node)) {
    return node.map(toPlainObject)
  }
  if (typeof (/** @type {{ toJSON?: () => unknown }} */ (node).toJSON) === 'function') {
    return /** @type {{ toJSON: () => unknown }} */ (node).toJSON()
  }
  return node
}
