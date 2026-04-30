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

import { COMMON_DATA_MASK, COMMON_DATA_OFFSET, STRING_FIELD_SIZE } from '../constants.ts'
import {
  absoluteRange,
  childNodeAtVisitorIndexChildren,
  extOffsetOf,
  extStringLeaf,
  firstChildIndex,
  stringPayloadOf,
  thisNode
} from '../helpers.ts'
import { inspectPayload, inspectSymbol } from '../inspect.ts'
import { nodeListAtSlotExtended } from '../node-list.ts'
import type {
  LazyNode,
  LazyNodeConstructor,
  RemoteInternal,
  RemoteJsonObject,
  RemoteSourceFileLike
} from '../types.ts'

/**
 * Single per-list metadata slot offset for TypeNode parents that own one
 * variable-length child list (TypeUnion, TypeIntersection, TypeTuple,
 * TypeObject, TypeGeneric, TypeTypeParameter, TypeParameterList). Mirrors
 * `crates/ox_jsdoc_binary/src/writer/nodes/type_node.rs::TYPE_LIST_PARENT_SLOT`.
 */
const TYPE_LIST_PARENT_SLOT = 0

function commonData(internal: RemoteInternal): number {
  return internal.view.getUint8(internal.byteIndex + COMMON_DATA_OFFSET) & COMMON_DATA_MASK
}

/** Optional callbacks used by the per-pattern factory helpers below. */
type ExtraJson = (internal: RemoteInternal, json: RemoteJsonObject) => void

/**
 * Build a Pattern 1 (string-only) class.
 *
 * `extraJson` lets variants append per-Kind metadata (quote / special_type)
 * to the JSON output without duplicating boilerplate.
 */
function defineStringPattern(typeName: string, extraJson?: ExtraJson): LazyNodeConstructor {
  return class implements LazyNode {
    readonly type = typeName
    readonly _internal: RemoteInternal

    constructor(
      view: DataView,
      byteIndex: number,
      index: number,
      rootIndex: number,
      parent: LazyNode | null,
      sourceFile: RemoteSourceFileLike
    ) {
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range(): readonly [number, number] {
      return absoluteRange(this._internal)
    }
    get parent(): LazyNode | null {
      return this._internal.parent
    }
    get value(): string {
      return stringPayloadOf(this._internal) ?? ''
    }
    toJSON(): RemoteJsonObject {
      const json: RemoteJsonObject = {
        type: this.type,
        range: [...this.range],
        value: this.value
      }
      if (extraJson !== undefined) {
        extraJson(this._internal, json)
      }
      return json
    }
    [inspectSymbol](): object {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}

/**
 * Build a "pure leaf" class (Children-type with zero payload — `TypeNull`,
 * `TypeUndefined`, `TypeAny`, `TypeUnknown`, `TypeUniqueSymbol`).
 */
function definePureLeaf(typeName: string): LazyNodeConstructor {
  return class implements LazyNode {
    readonly type = typeName
    readonly _internal: RemoteInternal

    constructor(
      view: DataView,
      byteIndex: number,
      index: number,
      rootIndex: number,
      parent: LazyNode | null,
      sourceFile: RemoteSourceFileLike
    ) {
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range(): readonly [number, number] {
      return absoluteRange(this._internal)
    }
    get parent(): LazyNode | null {
      return this._internal.parent
    }
    toJSON(): RemoteJsonObject {
      return { type: this.type, range: [...this.range] }
    }
    [inspectSymbol](): object {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}

// ===========================================================================
// Pattern 1 — String only (5 kinds)
// ===========================================================================

export class RemoteTypeName extends defineStringPattern('TypeName') {}
export class RemoteTypeNumber extends defineStringPattern('TypeNumber') {}
export class RemoteTypeStringValue extends defineStringPattern(
  'TypeStringValue',
  (internal, json) => {
    json.quote = commonData(internal) & 0b11
  }
) {}
export class RemoteTypeProperty extends defineStringPattern('TypeProperty', (internal, json) => {
  json.quote = commonData(internal) & 0b11
}) {}
export class RemoteTypeSpecialNamePath extends defineStringPattern(
  'TypeSpecialNamePath',
  (internal, json) => {
    const cd = commonData(internal)
    json.specialType = cd & 0b11
    json.quote = (cd >> 2) & 0b11
  }
) {}

// ===========================================================================
// Pattern 2 — Children only (29 kinds)
// ===========================================================================

/** Helper: build a class with one `elements` NodeList child. */
function defineElementsContainer(typeName: string): LazyNodeConstructor {
  return class implements LazyNode {
    readonly type = typeName
    readonly _internal: RemoteInternal

    constructor(
      view: DataView,
      byteIndex: number,
      index: number,
      rootIndex: number,
      parent: LazyNode | null,
      sourceFile: RemoteSourceFileLike
    ) {
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range(): readonly [number, number] {
      return absoluteRange(this._internal)
    }
    get parent(): LazyNode | null {
      return this._internal.parent
    }
    get elements(): readonly LazyNode[] {
      return nodeListAtSlotExtended(this._internal, TYPE_LIST_PARENT_SLOT)
    }
    toJSON(): RemoteJsonObject {
      return {
        type: this.type,
        range: [...this.range],
        elements: this.elements.map(n => n.toJSON())
      }
    }
    [inspectSymbol](): object {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}

/** Helper: build a class with a single `element` child. */
function defineSingleChildContainer(
  typeName: string,
  extraCommon?: ExtraJson
): LazyNodeConstructor {
  return class implements LazyNode {
    readonly type = typeName
    readonly _internal: RemoteInternal

    constructor(
      view: DataView,
      byteIndex: number,
      index: number,
      rootIndex: number,
      parent: LazyNode | null,
      sourceFile: RemoteSourceFileLike
    ) {
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range(): readonly [number, number] {
      return absoluteRange(this._internal)
    }
    get parent(): LazyNode | null {
      return this._internal.parent
    }
    get element(): LazyNode | null {
      return childNodeAtVisitorIndexChildren(this._internal, 0)
    }
    toJSON(): RemoteJsonObject {
      const json: RemoteJsonObject = {
        type: this.type,
        range: [...this.range],
        element: this.element?.toJSON() ?? null
      }
      if (extraCommon !== undefined) {
        extraCommon(this._internal, json)
      }
      return json
    }
    [inspectSymbol](): object {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}

/** Helper: build a class with `left` + `right` children. */
function defineLeftRightContainer(typeName: string, extraCommon?: ExtraJson): LazyNodeConstructor {
  return class implements LazyNode {
    readonly type = typeName
    readonly _internal: RemoteInternal

    constructor(
      view: DataView,
      byteIndex: number,
      index: number,
      rootIndex: number,
      parent: LazyNode | null,
      sourceFile: RemoteSourceFileLike
    ) {
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range(): readonly [number, number] {
      return absoluteRange(this._internal)
    }
    get parent(): LazyNode | null {
      return this._internal.parent
    }
    get left(): LazyNode | null {
      return childNodeAtVisitorIndexChildren(this._internal, 0)
    }
    get right(): LazyNode | null {
      return childNodeAtVisitorIndexChildren(this._internal, 1)
    }
    toJSON(): RemoteJsonObject {
      const json: RemoteJsonObject = {
        type: this.type,
        range: [...this.range],
        left: this.left?.toJSON() ?? null,
        right: this.right?.toJSON() ?? null
      }
      if (extraCommon !== undefined) {
        extraCommon(this._internal, json)
      }
      return json
    }
    [inspectSymbol](): object {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}

// --- elements containers ---
export class RemoteTypeUnion extends defineElementsContainer('TypeUnion') {}
export class RemoteTypeIntersection extends defineElementsContainer('TypeIntersection') {}

// `TypeObject` extends the elements container with a `separator` flag.
export class RemoteTypeObject implements LazyNode {
  readonly type = 'TypeObject'
  readonly _internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range(): readonly [number, number] {
    return absoluteRange(this._internal)
  }
  get parent(): LazyNode | null {
    return this._internal.parent
  }
  get elements(): readonly LazyNode[] {
    return nodeListAtSlotExtended(this._internal, TYPE_LIST_PARENT_SLOT)
  }
  /** `bits[0:2]` of Common Data — field separator style. */
  get separator(): number {
    return commonData(this._internal) & 0b111
  }
  toJSON(): RemoteJsonObject {
    return {
      type: this.type,
      range: [...this.range],
      elements: this.elements.map(n => n.toJSON()),
      separator: this.separator
    }
  }
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'TypeObject')
  }
}

export class RemoteTypeTuple extends defineElementsContainer('TypeTuple') {}
export class RemoteTypeTypeParameter extends defineElementsContainer('TypeTypeParameter') {}
export class RemoteTypeParameterList extends defineElementsContainer('TypeParameterList') {}

// --- single-child containers (with optional Common Data fields) ---
export class RemoteTypeParenthesis extends defineSingleChildContainer('TypeParenthesis') {}
export class RemoteTypeInfer extends defineSingleChildContainer('TypeInfer') {}
export class RemoteTypeKeyOf extends defineSingleChildContainer('TypeKeyOf') {}
export class RemoteTypeTypeOf extends defineSingleChildContainer('TypeTypeOf') {}
export class RemoteTypeImport extends defineSingleChildContainer('TypeImport') {}
export class RemoteTypeAssertsPlain extends defineSingleChildContainer('TypeAssertsPlain') {}
export class RemoteTypeReadonlyArray extends defineSingleChildContainer('TypeReadonlyArray') {}
export class RemoteTypeIndexedAccessIndex extends defineSingleChildContainer(
  'TypeIndexedAccessIndex'
) {}
export class RemoteTypeReadonlyProperty extends defineSingleChildContainer(
  'TypeReadonlyProperty'
) {}

/** Modifier types (Nullable / NotNullable / Optional) — single child + position flag. */
function defineModifier(typeName: string): LazyNodeConstructor {
  return defineSingleChildContainer(typeName, (internal, json) => {
    json.position = commonData(internal) & 1
  })
}
export class RemoteTypeNullable extends defineModifier('TypeNullable') {}
export class RemoteTypeNotNullable extends defineModifier('TypeNotNullable') {}
export class RemoteTypeOptional extends defineModifier('TypeOptional') {}

/** `TypeVariadic` — modifier + extra `square_brackets` flag. */
export class RemoteTypeVariadic extends defineSingleChildContainer(
  'TypeVariadic',
  (internal, json) => {
    const cd = commonData(internal)
    json.position = cd & 1
    json.squareBrackets = (cd & 0b10) !== 0
  }
) {}

// --- left + right containers ---
export class RemoteTypePredicate extends defineLeftRightContainer('TypePredicate') {}
export class RemoteTypeAsserts extends defineLeftRightContainer('TypeAsserts') {}
export class RemoteTypeNamePath extends defineLeftRightContainer(
  'TypeNamePath',
  (internal, json) => {
    json.pathType = commonData(internal) & 0b11
  }
) {}

/** `TypeGeneric` — `left` + `elements` NodeList + brackets/dot flags. */
export class RemoteTypeGeneric implements LazyNode {
  readonly type = 'TypeGeneric'
  readonly _internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range(): readonly [number, number] {
    return absoluteRange(this._internal)
  }
  get parent(): LazyNode | null {
    return this._internal.parent
  }
  get brackets(): number {
    return commonData(this._internal) & 1
  }
  get dot(): boolean {
    return (commonData(this._internal) & 0b10) !== 0
  }
  get left(): LazyNode | null {
    const internal = this._internal
    const childIdx = firstChildIndex(internal.sourceFile, internal.index)
    if (childIdx === 0) {
      return null
    }
    return internal.sourceFile.getNode(childIdx, thisNode(internal), internal.rootIndex)
  }
  get elements(): readonly LazyNode[] {
    return nodeListAtSlotExtended(this._internal, TYPE_LIST_PARENT_SLOT)
  }
  toJSON(): RemoteJsonObject {
    return {
      type: this.type,
      range: [...this.range],
      brackets: this.brackets,
      dot: this.dot,
      left: this.left?.toJSON() ?? null,
      elements: this.elements.map(n => n.toJSON())
    }
  }
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'TypeGeneric')
  }
}

/** `TypeFunction` — parameters + return + type_parameters. */
export class RemoteTypeFunction implements LazyNode {
  readonly type = 'TypeFunction'
  readonly _internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range(): readonly [number, number] {
    return absoluteRange(this._internal)
  }
  get parent(): LazyNode | null {
    return this._internal.parent
  }
  get constructor_(): boolean {
    return (commonData(this._internal) & 0b001) !== 0
  }
  get arrow(): boolean {
    return (commonData(this._internal) & 0b010) !== 0
  }
  get parenthesis(): boolean {
    return (commonData(this._internal) & 0b100) !== 0
  }
  get parameters(): LazyNode | null {
    return childNodeAtVisitorIndexChildren(this._internal, 0)
  }
  get returnType(): LazyNode | null {
    return childNodeAtVisitorIndexChildren(this._internal, 1)
  }
  get typeParameters(): LazyNode | null {
    return childNodeAtVisitorIndexChildren(this._internal, 2)
  }
  toJSON(): RemoteJsonObject {
    return {
      type: this.type,
      range: [...this.range],
      constructor: this.constructor_,
      arrow: this.arrow,
      parenthesis: this.parenthesis,
      parameters: this.parameters?.toJSON() ?? null,
      returnType: this.returnType?.toJSON() ?? null,
      typeParameters: this.typeParameters?.toJSON() ?? null
    }
  }
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'TypeFunction')
  }
}

/** `TypeConditional` — check / extends / true / false branches. */
export class RemoteTypeConditional implements LazyNode {
  readonly type = 'TypeConditional'
  readonly _internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range(): readonly [number, number] {
    return absoluteRange(this._internal)
  }
  get parent(): LazyNode | null {
    return this._internal.parent
  }
  get checkType(): LazyNode | null {
    return childNodeAtVisitorIndexChildren(this._internal, 0)
  }
  get extendsType(): LazyNode | null {
    return childNodeAtVisitorIndexChildren(this._internal, 1)
  }
  get trueType(): LazyNode | null {
    return childNodeAtVisitorIndexChildren(this._internal, 2)
  }
  get falseType(): LazyNode | null {
    return childNodeAtVisitorIndexChildren(this._internal, 3)
  }
  toJSON(): RemoteJsonObject {
    return {
      type: this.type,
      range: [...this.range],
      checkType: this.checkType?.toJSON() ?? null,
      extendsType: this.extendsType?.toJSON() ?? null,
      trueType: this.trueType?.toJSON() ?? null,
      falseType: this.falseType?.toJSON() ?? null
    }
  }
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'TypeConditional')
  }
}

/** `TypeObjectField` — key + right + flags. */
export class RemoteTypeObjectField implements LazyNode {
  readonly type = 'TypeObjectField'
  readonly _internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range(): readonly [number, number] {
    return absoluteRange(this._internal)
  }
  get parent(): LazyNode | null {
    return this._internal.parent
  }
  get optional(): boolean {
    return (commonData(this._internal) & 0b0001) !== 0
  }
  get readonly(): boolean {
    return (commonData(this._internal) & 0b0010) !== 0
  }
  get quote(): number {
    return (commonData(this._internal) >> 2) & 0b11
  }
  get key(): LazyNode | null {
    return childNodeAtVisitorIndexChildren(this._internal, 0)
  }
  get right(): LazyNode | null {
    return childNodeAtVisitorIndexChildren(this._internal, 1)
  }
  toJSON(): RemoteJsonObject {
    return {
      type: this.type,
      range: [...this.range],
      optional: this.optional,
      readonly: this.readonly,
      quote: this.quote,
      key: this.key?.toJSON() ?? null,
      right: this.right?.toJSON() ?? null
    }
  }
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'TypeObjectField')
  }
}

/** `TypeJsdocObjectField` — key + right (no flags). */
export class RemoteTypeJsdocObjectField implements LazyNode {
  readonly type = 'TypeJsdocObjectField'
  readonly _internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range(): readonly [number, number] {
    return absoluteRange(this._internal)
  }
  get parent(): LazyNode | null {
    return this._internal.parent
  }
  get key(): LazyNode | null {
    return childNodeAtVisitorIndexChildren(this._internal, 0)
  }
  get right(): LazyNode | null {
    return childNodeAtVisitorIndexChildren(this._internal, 1)
  }
  toJSON(): RemoteJsonObject {
    return {
      type: this.type,
      range: [...this.range],
      key: this.key?.toJSON() ?? null,
      right: this.right?.toJSON() ?? null
    }
  }
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'TypeJsdocObjectField')
  }
}

/** Signature container (CallSignature / ConstructorSignature). */
function defineSignature(typeName: string): LazyNodeConstructor {
  return class implements LazyNode {
    readonly type = typeName
    readonly _internal: RemoteInternal

    constructor(
      view: DataView,
      byteIndex: number,
      index: number,
      rootIndex: number,
      parent: LazyNode | null,
      sourceFile: RemoteSourceFileLike
    ) {
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range(): readonly [number, number] {
      return absoluteRange(this._internal)
    }
    get parent(): LazyNode | null {
      return this._internal.parent
    }
    get parameters(): LazyNode | null {
      return childNodeAtVisitorIndexChildren(this._internal, 0)
    }
    get returnType(): LazyNode | null {
      return childNodeAtVisitorIndexChildren(this._internal, 1)
    }
    get typeParameters(): LazyNode | null {
      return childNodeAtVisitorIndexChildren(this._internal, 2)
    }
    toJSON(): RemoteJsonObject {
      return {
        type: this.type,
        range: [...this.range],
        parameters: this.parameters?.toJSON() ?? null,
        returnType: this.returnType?.toJSON() ?? null,
        typeParameters: this.typeParameters?.toJSON() ?? null
      }
    }
    [inspectSymbol](): object {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}
export class RemoteTypeCallSignature extends defineSignature('TypeCallSignature') {}
export class RemoteTypeConstructorSignature extends defineSignature('TypeConstructorSignature') {}

// ===========================================================================
// Pattern 3 — Mixed string + child (6 kinds)
// ===========================================================================

/** `TypeKeyValue` — key string in Extended Data + first child as `right`. */
export class RemoteTypeKeyValue implements LazyNode {
  readonly type = 'TypeKeyValue'
  readonly _internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range(): readonly [number, number] {
    return absoluteRange(this._internal)
  }
  get parent(): LazyNode | null {
    return this._internal.parent
  }
  get optional(): boolean {
    return (commonData(this._internal) & 0b01) !== 0
  }
  get variadic(): boolean {
    return (commonData(this._internal) & 0b10) !== 0
  }
  get key(): string {
    return extStringLeaf(this._internal)
  }
  get right(): LazyNode | null {
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
  toJSON(): RemoteJsonObject {
    return {
      type: this.type,
      range: [...this.range],
      optional: this.optional,
      variadic: this.variadic,
      key: this.key,
      right: this.right?.toJSON() ?? null
    }
  }
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'TypeKeyValue')
  }
}

/** Helper for `TypeIndexSignature` / `TypeMappedType` — key + first child. */
function defineKeyAndChild(typeName: string): LazyNodeConstructor {
  return class implements LazyNode {
    readonly type = typeName
    readonly _internal: RemoteInternal

    constructor(
      view: DataView,
      byteIndex: number,
      index: number,
      rootIndex: number,
      parent: LazyNode | null,
      sourceFile: RemoteSourceFileLike
    ) {
      this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
    }
    get range(): readonly [number, number] {
      return absoluteRange(this._internal)
    }
    get parent(): LazyNode | null {
      return this._internal.parent
    }
    get key(): string {
      return extStringLeaf(this._internal)
    }
    get right(): LazyNode | null {
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
    toJSON(): RemoteJsonObject {
      return {
        type: this.type,
        range: [...this.range],
        key: this.key,
        right: this.right?.toJSON() ?? null
      }
    }
    [inspectSymbol](): object {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}
export class RemoteTypeIndexSignature extends defineKeyAndChild('TypeIndexSignature') {}
export class RemoteTypeMappedType extends defineKeyAndChild('TypeMappedType') {}

/** `TypeMethodSignature` — name string in Extended Data + Common Data flags. */
export class RemoteTypeMethodSignature implements LazyNode {
  readonly type = 'TypeMethodSignature'
  readonly _internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range(): readonly [number, number] {
    return absoluteRange(this._internal)
  }
  get parent(): LazyNode | null {
    return this._internal.parent
  }
  get quote(): number {
    return commonData(this._internal) & 0b11
  }
  get hasParameters(): boolean {
    return (commonData(this._internal) & 0b0100) !== 0
  }
  get hasTypeParameters(): boolean {
    return (commonData(this._internal) & 0b1000) !== 0
  }
  get name(): string {
    return extStringLeaf(this._internal)
  }
  toJSON(): RemoteJsonObject {
    return {
      type: this.type,
      range: [...this.range],
      quote: this.quote,
      hasParameters: this.hasParameters,
      hasTypeParameters: this.hasTypeParameters,
      name: this.name
    }
  }
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'TypeMethodSignature')
  }
}

/** `TypeTemplateLiteral` — literal-segment array in Extended Data. */
export class RemoteTypeTemplateLiteral implements LazyNode {
  readonly type = 'TypeTemplateLiteral'
  readonly _internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range(): readonly [number, number] {
    return absoluteRange(this._internal)
  }
  get parent(): LazyNode | null {
    return this._internal.parent
  }
  /** Number of literal segments stored at byte 0-1 of Extended Data. */
  get literalCount(): number {
    return this._internal.view.getUint16(extOffsetOf(this._internal), true)
  }
  /** Resolve the n-th literal segment. */
  literal(index: number): string {
    const off = extOffsetOf(this._internal) + 2 + index * STRING_FIELD_SIZE
    const offset = this._internal.view.getUint32(off, true)
    const length = this._internal.view.getUint16(off + 4, true)
    return this._internal.sourceFile.getStringByField(offset, length) ?? ''
  }
  /** All literal segments as an array. */
  get literals(): string[] {
    const count = this.literalCount
    const out: string[] = Array.from({ length: count })
    for (let i = 0; i < count; i++) {
      out[i] = this.literal(i)
    }
    return out
  }
  toJSON(): RemoteJsonObject {
    return { type: this.type, range: [...this.range], literals: this.literals }
  }
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'TypeTemplateLiteral')
  }
}

/** `TypeSymbol` — `Symbol(...)` callee value + optional element. */
export class RemoteTypeSymbol implements LazyNode {
  readonly type = 'TypeSymbol'
  readonly _internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this._internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }
  get range(): readonly [number, number] {
    return absoluteRange(this._internal)
  }
  get parent(): LazyNode | null {
    return this._internal.parent
  }
  get hasElement(): boolean {
    return (commonData(this._internal) & 1) !== 0
  }
  get value(): string {
    return extStringLeaf(this._internal)
  }
  get element(): LazyNode | null {
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
  toJSON(): RemoteJsonObject {
    return {
      type: this.type,
      range: [...this.range],
      hasElement: this.hasElement,
      value: this.value,
      element: this.element?.toJSON() ?? null
    }
  }
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'TypeSymbol')
  }
}

// ===========================================================================
// Pure leaves (no payload)
// ===========================================================================

export class RemoteTypeNull extends definePureLeaf('TypeNull') {}
export class RemoteTypeUndefined extends definePureLeaf('TypeUndefined') {}
export class RemoteTypeAny extends definePureLeaf('TypeAny') {}
export class RemoteTypeUnknown extends definePureLeaf('TypeUnknown') {}
export class RemoteTypeUniqueSymbol extends definePureLeaf('TypeUniqueSymbol') {}
