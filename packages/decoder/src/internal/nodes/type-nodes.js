/**
 * Lazy classes for the 45 TypeNode kinds (`0x80 - 0xAC`).
 *
 * Mirrors `crates/ox_jsdoc_binary/src/decoder/nodes/type_node.rs`.
 *
 * Three structural patterns are at play:
 *
 * - **Pattern 1 — String only** (5 kinds): payload lives in the 30-bit
 *   String slot, optionally with quote/special-type flags in Common Data.
 * - **Pattern 2 — Children only** (29 kinds): Children-type Node Data
 *   carries the bitmask; child accessors use the Children-type helpers.
 * - **Pattern 3 — Mixed** (6 kinds): Extended type with a key/name string
 *   plus zero or one child node.
 *
 * Plus 5 pure leaves (`TypeNull` etc.) using Children-type with zero payload.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import { COMMON_DATA_MASK, COMMON_DATA_OFFSET, STRING_FIELD_SIZE } from '../constants.js'
import {
  absoluteRange,
  childNodeAtVisitorIndexChildren,
  extOffsetOf,
  extStringLeaf,
  firstChildIndex,
  stringPayloadOf,
  thisNode
} from '../helpers.js'
import { inspectPayload, inspectSymbol } from '../inspect.js'
import { nodeListAtSlotExtended } from '../node-list.js'

/**
 * Single per-list metadata slot offset for TypeNode parents that own one
 * variable-length child list (TypeUnion, TypeIntersection, TypeTuple,
 * TypeObject, TypeGeneric, TypeTypeParameter, TypeParameterList). Mirrors
 * `crates/ox_jsdoc_binary/src/writer/nodes/type_node.rs::TYPE_LIST_PARENT_SLOT`.
 */
const TYPE_LIST_PARENT_SLOT = 0

function commonData(internal) {
  return internal.view.getUint8(internal.byteIndex + COMMON_DATA_OFFSET) & COMMON_DATA_MASK
}

/**
 * Build a Pattern 1 (string-only) class.
 *
 * @param {string} typeName     The `type` field value.
 * @param {(internal: object, json: object) => void} [extraJson]
 *   Optional callback for adding extra fields (quote/special_type) to JSON.
 * @param {(internal: object, instance: object) => void} [extraGetters]
 *   Optional callback to install extra getters on the prototype/instance.
 */
function defineStringPattern(typeName, extraJson, extraGetters) {
  const cls = class {
    constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
      Object.defineProperty(this, 'type', { value: typeName, enumerable: true })
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
      if (extraGetters !== undefined) {
        extraGetters(this._internal, this)
      }
    }
    get range() {
      return absoluteRange(this._internal)
    }
    get parent() {
      return this._internal.parent
    }
    get value() {
      return stringPayloadOf(this._internal) ?? ''
    }
    toJSON() {
      const json = { type: this.type, range: this.range, value: this.value }
      if (extraJson !== undefined) {
        extraJson(this._internal, json)
      }
      return json
    }
    [inspectSymbol]() {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
  return cls
}

/**
 * Build a "pure leaf" class (Children-type with zero payload — `TypeNull`,
 * `TypeUndefined`, `TypeAny`, `TypeUnknown`, `TypeUniqueSymbol`).
 *
 * @param {string} typeName
 */
function definePureLeaf(typeName) {
  const cls = class {
    constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
      Object.defineProperty(this, 'type', { value: typeName, enumerable: true })
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range() {
      return absoluteRange(this._internal)
    }
    get parent() {
      return this._internal.parent
    }
    toJSON() {
      return { type: this.type, range: this.range }
    }
    [inspectSymbol]() {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
  return cls
}

// ===========================================================================
// Pattern 1 — String only (5 kinds)
// ===========================================================================

export const RemoteTypeName = defineStringPattern('TypeName')
export const RemoteTypeNumber = defineStringPattern('TypeNumber')
export const RemoteTypeStringValue = defineStringPattern('TypeStringValue', (internal, json) => {
  json.quote = commonData(internal) & 0b11
})
export const RemoteTypeProperty = defineStringPattern('TypeProperty', (internal, json) => {
  json.quote = commonData(internal) & 0b11
})
export const RemoteTypeSpecialNamePath = defineStringPattern(
  'TypeSpecialNamePath',
  (internal, json) => {
    const cd = commonData(internal)
    json.specialType = cd & 0b11
    json.quote = (cd >> 2) & 0b11
  }
)

// ===========================================================================
// Pattern 2 — Children only (29 kinds)
// ===========================================================================

/** Helper: build a class with one `elements` NodeList child. */
function defineElementsContainer(typeName) {
  return class {
    constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
      Object.defineProperty(this, 'type', { value: typeName, enumerable: true })
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range() {
      return absoluteRange(this._internal)
    }
    get parent() {
      return this._internal.parent
    }
    get elements() {
      return nodeListAtSlotExtended(this._internal, TYPE_LIST_PARENT_SLOT)
    }
    toJSON() {
      return {
        type: this.type,
        range: this.range,
        elements: this.elements.map(n => n.toJSON())
      }
    }
    [inspectSymbol]() {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}

/** Helper: build a class with a single `element` child. */
function defineSingleChildContainer(typeName, extraCommon) {
  return class {
    constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
      Object.defineProperty(this, 'type', { value: typeName, enumerable: true })
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range() {
      return absoluteRange(this._internal)
    }
    get parent() {
      return this._internal.parent
    }
    get element() {
      return childNodeAtVisitorIndexChildren(this._internal, 0)
    }
    toJSON() {
      const json = {
        type: this.type,
        range: this.range,
        element: this.element?.toJSON() ?? null
      }
      if (extraCommon !== undefined) {
        extraCommon(this._internal, json)
      }
      return json
    }
    [inspectSymbol]() {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}

/** Helper: build a class with `left` + `right` children. */
function defineLeftRightContainer(typeName, extraCommon) {
  return class {
    constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
      Object.defineProperty(this, 'type', { value: typeName, enumerable: true })
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range() {
      return absoluteRange(this._internal)
    }
    get parent() {
      return this._internal.parent
    }
    get left() {
      return childNodeAtVisitorIndexChildren(this._internal, 0)
    }
    get right() {
      return childNodeAtVisitorIndexChildren(this._internal, 1)
    }
    toJSON() {
      const json = {
        type: this.type,
        range: this.range,
        left: this.left?.toJSON() ?? null,
        right: this.right?.toJSON() ?? null
      }
      if (extraCommon !== undefined) {
        extraCommon(this._internal, json)
      }
      return json
    }
    [inspectSymbol]() {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}

// --- elements containers ---
export const RemoteTypeUnion = defineElementsContainer('TypeUnion')
export const RemoteTypeIntersection = defineElementsContainer('TypeIntersection')
export const RemoteTypeObject = (() => {
  const Base = defineElementsContainer('TypeObject')
  return class extends Base {
    /** `bits[0:2]` of Common Data — field separator style. */
    get separator() {
      return commonData(this._internal) & 0b111
    }
    toJSON() {
      const json = super.toJSON()
      json.separator = this.separator
      return json
    }
  }
})()
export const RemoteTypeTuple = defineElementsContainer('TypeTuple')
export const RemoteTypeTypeParameter = defineElementsContainer('TypeTypeParameter')
export const RemoteTypeParameterList = defineElementsContainer('TypeParameterList')

// --- single-child containers (with optional Common Data fields) ---
export const RemoteTypeParenthesis = defineSingleChildContainer('TypeParenthesis')
export const RemoteTypeInfer = defineSingleChildContainer('TypeInfer')
export const RemoteTypeKeyOf = defineSingleChildContainer('TypeKeyOf')
export const RemoteTypeTypeOf = defineSingleChildContainer('TypeTypeOf')
export const RemoteTypeImport = defineSingleChildContainer('TypeImport')
export const RemoteTypeAssertsPlain = defineSingleChildContainer('TypeAssertsPlain')
export const RemoteTypeReadonlyArray = defineSingleChildContainer('TypeReadonlyArray')
export const RemoteTypeIndexedAccessIndex = defineSingleChildContainer('TypeIndexedAccessIndex')
export const RemoteTypeReadonlyProperty = defineSingleChildContainer('TypeReadonlyProperty')

/** Modifier types (Nullable / NotNullable / Optional) — single child + position flag. */
function defineModifier(typeName) {
  return defineSingleChildContainer(typeName, (internal, json) => {
    json.position = commonData(internal) & 1
  })
}
export const RemoteTypeNullable = defineModifier('TypeNullable')
export const RemoteTypeNotNullable = defineModifier('TypeNotNullable')
export const RemoteTypeOptional = defineModifier('TypeOptional')

/** `TypeVariadic` — modifier + extra `square_brackets` flag. */
export const RemoteTypeVariadic = defineSingleChildContainer('TypeVariadic', (internal, json) => {
  const cd = commonData(internal)
  json.position = cd & 1
  json.squareBrackets = (cd & 0b10) !== 0
})

// --- left + right containers ---
export const RemoteTypePredicate = defineLeftRightContainer('TypePredicate')
export const RemoteTypeAsserts = defineLeftRightContainer('TypeAsserts')
export const RemoteTypeNamePath = defineLeftRightContainer('TypeNamePath', (internal, json) => {
  json.pathType = commonData(internal) & 0b11
})

/** `TypeGeneric` — `left` + `elements` NodeList + brackets/dot flags. */
export class RemoteTypeGeneric {
  type = 'TypeGeneric'
  _internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range() {
    return absoluteRange(this._internal)
  }
  get parent() {
    return this._internal.parent
  }
  get brackets() {
    return commonData(this._internal) & 1
  }
  get dot() {
    return (commonData(this._internal) & 0b10) !== 0
  }
  get left() {
    const internal = this._internal
    const childIdx = firstChildIndex(internal.sourceFile, internal.index)
    if (childIdx === 0) return null
    return internal.sourceFile.getNode(childIdx, thisNode(internal), internal.rootIndex)
  }
  get elements() {
    return nodeListAtSlotExtended(this._internal, TYPE_LIST_PARENT_SLOT)
  }
  toJSON() {
    return {
      type: this.type,
      range: this.range,
      brackets: this.brackets,
      dot: this.dot,
      left: this.left?.toJSON() ?? null,
      elements: this.elements.map(n => n.toJSON())
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'TypeGeneric')
  }
}

/** `TypeFunction` — parameters + return + type_parameters. */
export class RemoteTypeFunction {
  type = 'TypeFunction'
  _internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range() {
    return absoluteRange(this._internal)
  }
  get parent() {
    return this._internal.parent
  }
  get constructor_() {
    return (commonData(this._internal) & 0b001) !== 0
  }
  get arrow() {
    return (commonData(this._internal) & 0b010) !== 0
  }
  get parenthesis() {
    return (commonData(this._internal) & 0b100) !== 0
  }
  get parameters() {
    return childNodeAtVisitorIndexChildren(this._internal, 0)
  }
  get returnType() {
    return childNodeAtVisitorIndexChildren(this._internal, 1)
  }
  get typeParameters() {
    return childNodeAtVisitorIndexChildren(this._internal, 2)
  }
  toJSON() {
    return {
      type: this.type,
      range: this.range,
      constructor: this.constructor_,
      arrow: this.arrow,
      parenthesis: this.parenthesis,
      parameters: this.parameters?.toJSON() ?? null,
      returnType: this.returnType?.toJSON() ?? null,
      typeParameters: this.typeParameters?.toJSON() ?? null
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'TypeFunction')
  }
}

/** `TypeConditional` — check / extends / true / false branches. */
export class RemoteTypeConditional {
  type = 'TypeConditional'
  _internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range() {
    return absoluteRange(this._internal)
  }
  get parent() {
    return this._internal.parent
  }
  get checkType() {
    return childNodeAtVisitorIndexChildren(this._internal, 0)
  }
  get extendsType() {
    return childNodeAtVisitorIndexChildren(this._internal, 1)
  }
  get trueType() {
    return childNodeAtVisitorIndexChildren(this._internal, 2)
  }
  get falseType() {
    return childNodeAtVisitorIndexChildren(this._internal, 3)
  }
  toJSON() {
    return {
      type: this.type,
      range: this.range,
      checkType: this.checkType?.toJSON() ?? null,
      extendsType: this.extendsType?.toJSON() ?? null,
      trueType: this.trueType?.toJSON() ?? null,
      falseType: this.falseType?.toJSON() ?? null
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'TypeConditional')
  }
}

/** `TypeObjectField` — key + right + flags. */
export class RemoteTypeObjectField {
  type = 'TypeObjectField'
  _internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range() {
    return absoluteRange(this._internal)
  }
  get parent() {
    return this._internal.parent
  }
  get optional() {
    return (commonData(this._internal) & 0b0001) !== 0
  }
  get readonly() {
    return (commonData(this._internal) & 0b0010) !== 0
  }
  get quote() {
    return (commonData(this._internal) >> 2) & 0b11
  }
  get key() {
    return childNodeAtVisitorIndexChildren(this._internal, 0)
  }
  get right() {
    return childNodeAtVisitorIndexChildren(this._internal, 1)
  }
  toJSON() {
    return {
      type: this.type,
      range: this.range,
      optional: this.optional,
      readonly: this.readonly,
      quote: this.quote,
      key: this.key?.toJSON() ?? null,
      right: this.right?.toJSON() ?? null
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'TypeObjectField')
  }
}

/** `TypeJsdocObjectField` — key + right (no flags). */
export class RemoteTypeJsdocObjectField {
  type = 'TypeJsdocObjectField'
  _internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range() {
    return absoluteRange(this._internal)
  }
  get parent() {
    return this._internal.parent
  }
  get key() {
    return childNodeAtVisitorIndexChildren(this._internal, 0)
  }
  get right() {
    return childNodeAtVisitorIndexChildren(this._internal, 1)
  }
  toJSON() {
    return {
      type: this.type,
      range: this.range,
      key: this.key?.toJSON() ?? null,
      right: this.right?.toJSON() ?? null
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'TypeJsdocObjectField')
  }
}

/** Signature container (CallSignature / ConstructorSignature). */
function defineSignature(typeName) {
  return class {
    constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
      Object.defineProperty(this, 'type', { value: typeName, enumerable: true })
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range() {
      return absoluteRange(this._internal)
    }
    get parent() {
      return this._internal.parent
    }
    get parameters() {
      return childNodeAtVisitorIndexChildren(this._internal, 0)
    }
    get returnType() {
      return childNodeAtVisitorIndexChildren(this._internal, 1)
    }
    get typeParameters() {
      return childNodeAtVisitorIndexChildren(this._internal, 2)
    }
    toJSON() {
      return {
        type: this.type,
        range: this.range,
        parameters: this.parameters?.toJSON() ?? null,
        returnType: this.returnType?.toJSON() ?? null,
        typeParameters: this.typeParameters?.toJSON() ?? null
      }
    }
    [inspectSymbol]() {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}
export const RemoteTypeCallSignature = defineSignature('TypeCallSignature')
export const RemoteTypeConstructorSignature = defineSignature('TypeConstructorSignature')

// ===========================================================================
// Pattern 3 — Mixed string + child (6 kinds)
// ===========================================================================

/** `TypeKeyValue` — key string in Extended Data + first child as `right`. */
export class RemoteTypeKeyValue {
  type = 'TypeKeyValue'
  _internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range() {
    return absoluteRange(this._internal)
  }
  get parent() {
    return this._internal.parent
  }
  get optional() {
    return (commonData(this._internal) & 0b01) !== 0
  }
  get variadic() {
    return (commonData(this._internal) & 0b10) !== 0
  }
  get key() {
    return extStringLeaf(this._internal)
  }
  get right() {
    const head = firstChildIndex(this._internal.sourceFile, this._internal.index)
    if (head === 0) {
      return null
    }
    return this._internal.sourceFile.getNode(
      head,
      thisNode(this._internal),
      this._internal.rootIndex
    )
  }
  toJSON() {
    return {
      type: this.type,
      range: this.range,
      optional: this.optional,
      variadic: this.variadic,
      key: this.key,
      right: this.right?.toJSON() ?? null
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'TypeKeyValue')
  }
}

/** Helper for `TypeIndexSignature` / `TypeMappedType` — key + first child. */
function defineKeyAndChild(typeName) {
  return class {
    constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
      Object.defineProperty(this, 'type', { value: typeName, enumerable: true })
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range() {
      return absoluteRange(this._internal)
    }
    get parent() {
      return this._internal.parent
    }
    get key() {
      return extStringLeaf(this._internal)
    }
    get right() {
      const head = firstChildIndex(this._internal.sourceFile, this._internal.index)
      if (head === 0) {
        return null
      }
      return this._internal.sourceFile.getNode(
        head,
        thisNode(this._internal),
        this._internal.rootIndex
      )
    }
    toJSON() {
      return {
        type: this.type,
        range: this.range,
        key: this.key,
        right: this.right?.toJSON() ?? null
      }
    }
    [inspectSymbol]() {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}
export const RemoteTypeIndexSignature = defineKeyAndChild('TypeIndexSignature')
export const RemoteTypeMappedType = defineKeyAndChild('TypeMappedType')

/** `TypeMethodSignature` — name string in Extended Data + Common Data flags. */
export class RemoteTypeMethodSignature {
  type = 'TypeMethodSignature'
  _internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range() {
    return absoluteRange(this._internal)
  }
  get parent() {
    return this._internal.parent
  }
  get quote() {
    return commonData(this._internal) & 0b11
  }
  get hasParameters() {
    return (commonData(this._internal) & 0b0100) !== 0
  }
  get hasTypeParameters() {
    return (commonData(this._internal) & 0b1000) !== 0
  }
  get name() {
    return extStringLeaf(this._internal)
  }
  toJSON() {
    return {
      type: this.type,
      range: this.range,
      quote: this.quote,
      hasParameters: this.hasParameters,
      hasTypeParameters: this.hasTypeParameters,
      name: this.name
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'TypeMethodSignature')
  }
}

/** `TypeTemplateLiteral` — literal-segment array in Extended Data. */
export class RemoteTypeTemplateLiteral {
  type = 'TypeTemplateLiteral'
  _internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range() {
    return absoluteRange(this._internal)
  }
  get parent() {
    return this._internal.parent
  }
  /** Number of literal segments stored at byte 0-1 of Extended Data. */
  get literalCount() {
    return this._internal.view.getUint16(extOffsetOf(this._internal), true)
  }
  /**
   * Resolve the n-th literal segment.
   *
   * @param {number} index
   */
  literal(index) {
    const off = extOffsetOf(this._internal) + 2 + index * STRING_FIELD_SIZE
    const offset = this._internal.view.getUint32(off, true)
    const length = this._internal.view.getUint16(off + 4, true)
    return this._internal.sourceFile.getStringByField(offset, length) ?? ''
  }
  /** All literal segments as an array. */
  get literals() {
    const count = this.literalCount
    const out = new Array(count)
    for (let i = 0; i < count; i++) {
      out[i] = this.literal(i)
    }
    return out
  }
  toJSON() {
    return { type: this.type, range: this.range, literals: this.literals }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'TypeTemplateLiteral')
  }
}

/** `TypeSymbol` — `Symbol(...)` callee value + optional element. */
export class RemoteTypeSymbol {
  type = 'TypeSymbol'
  _internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range() {
    return absoluteRange(this._internal)
  }
  get parent() {
    return this._internal.parent
  }
  get hasElement() {
    return (commonData(this._internal) & 1) !== 0
  }
  get value() {
    return extStringLeaf(this._internal)
  }
  get element() {
    if (!this.hasElement) {
      return null
    }
    const head = firstChildIndex(this._internal.sourceFile, this._internal.index)
    if (head === 0) {
      return null
    }
    return this._internal.sourceFile.getNode(
      head,
      thisNode(this._internal),
      this._internal.rootIndex
    )
  }
  toJSON() {
    return {
      type: this.type,
      range: this.range,
      hasElement: this.hasElement,
      value: this.value,
      element: this.element?.toJSON() ?? null
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'TypeSymbol')
  }
}

// ===========================================================================
// Pure leaves (no payload)
// ===========================================================================

export const RemoteTypeNull = definePureLeaf('TypeNull')
export const RemoteTypeUndefined = definePureLeaf('TypeUndefined')
export const RemoteTypeAny = definePureLeaf('TypeAny')
export const RemoteTypeUnknown = definePureLeaf('TypeUnknown')
export const RemoteTypeUniqueSymbol = definePureLeaf('TypeUniqueSymbol')
