/**
 * Lazy classes for the 15 comment AST kinds (`0x01 - 0x0F`).
 *
 * Each class follows the `#internal` pattern from `js-decoder.md`:
 * private state lives in a single object so the V8 hidden class stays
 * stable across all instances, and lazily constructed children are cached
 * inside the same object.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

// @ts-check

import { COMMON_DATA_MASK, COMMON_DATA_OFFSET } from '../constants.js'
import {
  absoluteRange,
  childNodeAtVisitorIndex,
  extOffsetOf,
  extU16String,
  extU16StringRequired,
  stringPayloadOf
} from '../helpers.js'
import { inspectPayload, inspectSymbol } from '../inspect.js'
import { nodeListAtVisitorIndexExtended } from '../node-list.js'

// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------

/**
 * Read the 6-bit Common Data byte for a node.
 *
 * @param {import('../helpers.js').RemoteInternal} internal
 * @returns {number}
 */
function commonData(internal) {
  return internal.view.getUint8(internal.byteIndex + COMMON_DATA_OFFSET) & COMMON_DATA_MASK
}

/**
 * `JsdocInlineTagFormat` numeric → string label.
 * Mirrors Rust's `JsdocInlineTagFormat` enum order.
 */
const INLINE_TAG_FORMATS = ['plain', 'pipe', 'space', 'prefix', 'unknown']

// ===========================================================================
// 0x01 RemoteJsdocBlock
// ===========================================================================

/**
 * `JsdocBlock` (Kind 0x01) — root of every parsed `/** ... *​/` comment.
 *
 * Getters self-cache into `#internal.$field` slots. The slots are
 * pre-allocated in the constructor so the V8 hidden class is fixed before
 * any getter writes; repeated access (toJSON, multi-pass visitors) returns
 * the cached value with one property read.
 */
export class RemoteJsdocBlock {
  type = 'JsdocBlock'
  #internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this.#internal = {
      view,
      byteIndex,
      index,
      rootIndex,
      parent,
      sourceFile,
      $range: undefined,
      $description: undefined,
      $delimiter: undefined,
      $postDelimiter: undefined,
      $terminal: undefined,
      $lineEnd: undefined,
      $initial: undefined,
      $delimiterLineBreak: undefined,
      $preterminalLineBreak: undefined,
      $descriptionLines: undefined,
      $tags: undefined,
      $inlineTags: undefined
    }
  }

  get range() {
    const internal = this.#internal
    return internal.$range !== undefined
      ? internal.$range
      : (internal.$range = absoluteRange(internal))
  }
  get parent() {
    return this.#internal.parent
  }

  /** Top-level description string (`null` when absent). */
  get description() {
    const internal = this.#internal
    const cached = internal.$description
    if (cached !== undefined) return cached
    return (internal.$description = extU16String(internal, 2))
  }
  /** Source-preserving `*` line-prefix delimiter. */
  get delimiter() {
    const internal = this.#internal
    const cached = internal.$delimiter
    if (cached !== undefined) return cached
    return (internal.$delimiter = extU16StringRequired(internal, 4))
  }
  /** Source-preserving space after `*`. */
  get postDelimiter() {
    const internal = this.#internal
    const cached = internal.$postDelimiter
    if (cached !== undefined) return cached
    return (internal.$postDelimiter = extU16StringRequired(internal, 6))
  }
  /** Source-preserving `*​/` terminal. */
  get terminal() {
    const internal = this.#internal
    const cached = internal.$terminal
    if (cached !== undefined) return cached
    return (internal.$terminal = extU16StringRequired(internal, 8))
  }
  /** Source-preserving line-end characters. */
  get lineEnd() {
    const internal = this.#internal
    const cached = internal.$lineEnd
    if (cached !== undefined) return cached
    return (internal.$lineEnd = extU16StringRequired(internal, 10))
  }
  /** Indentation before the leading `*`. */
  get initial() {
    const internal = this.#internal
    const cached = internal.$initial
    if (cached !== undefined) return cached
    return (internal.$initial = extU16StringRequired(internal, 12))
  }
  /** Line-break right after `/**`. */
  get delimiterLineBreak() {
    const internal = this.#internal
    const cached = internal.$delimiterLineBreak
    if (cached !== undefined) return cached
    return (internal.$delimiterLineBreak = extU16StringRequired(internal, 14))
  }
  /** Line-break right before `*​/`. */
  get preterminalLineBreak() {
    const internal = this.#internal
    const cached = internal.$preterminalLineBreak
    if (cached !== undefined) return cached
    return (internal.$preterminalLineBreak = extU16StringRequired(internal, 16))
  }

  /** Top-level description lines. */
  get descriptionLines() {
    const internal = this.#internal
    const cached = internal.$descriptionLines
    if (cached !== undefined) return cached
    return (internal.$descriptionLines = nodeListAtVisitorIndexExtended(internal, 0))
  }
  /** Block tags. */
  get tags() {
    const internal = this.#internal
    const cached = internal.$tags
    if (cached !== undefined) return cached
    return (internal.$tags = nodeListAtVisitorIndexExtended(internal, 1))
  }
  /** Inline tags found inside the top-level description. */
  get inlineTags() {
    const internal = this.#internal
    const cached = internal.$inlineTags
    if (cached !== undefined) return cached
    return (internal.$inlineTags = nodeListAtVisitorIndexExtended(internal, 2))
  }

  toJSON() {
    return {
      type: this.type,
      range: this.range,
      description: this.description,
      delimiter: this.delimiter,
      postDelimiter: this.postDelimiter,
      terminal: this.terminal,
      lineEnd: this.lineEnd,
      initial: this.initial,
      delimiterLineBreak: this.delimiterLineBreak,
      preterminalLineBreak: this.preterminalLineBreak,
      descriptionLines: this.descriptionLines.map(n => n.toJSON()),
      tags: this.tags.map(n => n.toJSON()),
      inlineTags: this.inlineTags.map(n => n.toJSON())
    }
  }

  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'JsdocBlock')
  }
}

// ===========================================================================
// 0x02 RemoteJsdocDescriptionLine
// ===========================================================================

/**
 * `JsdocDescriptionLine` (Kind 0x02). Basic mode stores `description`
 * as a String payload; compat mode promotes it (plus delimiters) into
 * Extended Data.
 */
export class RemoteJsdocDescriptionLine {
  type = 'JsdocDescriptionLine'
  #internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this.#internal = {
      view,
      byteIndex,
      index,
      rootIndex,
      parent,
      sourceFile,
      $range: undefined,
      $description: undefined
    }
  }

  get range() {
    const internal = this.#internal
    return internal.$range !== undefined
      ? internal.$range
      : (internal.$range = absoluteRange(internal))
  }
  get parent() {
    return this.#internal.parent
  }

  /** Description content. */
  get description() {
    const internal = this.#internal
    const cached = internal.$description
    if (cached !== undefined) return cached
    const value = internal.sourceFile.compatMode
      ? extU16StringRequired(internal, 0)
      : (stringPayloadOf(internal) ?? '')
    return (internal.$description = value)
  }

  toJSON() {
    return { type: this.type, range: this.range, description: this.description }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'JsdocDescriptionLine')
  }
}

// ===========================================================================
// 0x03 RemoteJsdocTag
// ===========================================================================

/**
 * `JsdocTag` (Kind 0x03) — one block tag (e.g. `@param`). Self-caches
 * structural fields; cheap bit checks like {@link RemoteJsdocTag.optional}
 * stay uncached because the cache check would cost more than recomputing.
 */
export class RemoteJsdocTag {
  type = 'JsdocTag'
  #internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this.#internal = {
      view,
      byteIndex,
      index,
      rootIndex,
      parent,
      sourceFile,
      $range: undefined,
      $defaultValue: undefined,
      $description: undefined,
      $rawBody: undefined,
      $tag: undefined,
      $rawType: undefined,
      $name: undefined,
      $parsedType: undefined,
      $body: undefined,
      $typeLines: undefined,
      $descriptionLines: undefined,
      $inlineTags: undefined
    }
  }

  get range() {
    const internal = this.#internal
    return internal.$range !== undefined
      ? internal.$range
      : (internal.$range = absoluteRange(internal))
  }
  get parent() {
    return this.#internal.parent
  }

  /** `bit0` of Common Data — was the tag wrapped in `[...]`? */
  get optional() {
    return (commonData(this.#internal) & 0b0000_0001) !== 0
  }
  /** Default value parsed from `[id=foo]` syntax. */
  get defaultValue() {
    const internal = this.#internal
    const cached = internal.$defaultValue
    if (cached !== undefined) return cached
    return (internal.$defaultValue = extU16String(internal, 2))
  }
  /** Joined description text. */
  get description() {
    const internal = this.#internal
    const cached = internal.$description
    if (cached !== undefined) return cached
    return (internal.$description = extU16String(internal, 4))
  }
  /** Raw body when the tag uses the `Raw` body variant. */
  get rawBody() {
    const internal = this.#internal
    const cached = internal.$rawBody
    if (cached !== undefined) return cached
    return (internal.$rawBody = extU16String(internal, 6))
  }

  /** Mandatory tag-name child (visitor index 0 — the `@name` token). */
  get tag() {
    const internal = this.#internal
    const cached = internal.$tag
    if (cached !== undefined) return cached
    return (internal.$tag = childNodeAtVisitorIndex(internal, 0))
  }
  /** Raw `{...}` type source (visitor index 1). */
  get rawType() {
    const internal = this.#internal
    const cached = internal.$rawType
    if (cached !== undefined) return cached
    return (internal.$rawType = childNodeAtVisitorIndex(internal, 1))
  }
  /** Tag-name value (visitor index 2). */
  get name() {
    const internal = this.#internal
    const cached = internal.$name
    if (cached !== undefined) return cached
    return (internal.$name = childNodeAtVisitorIndex(internal, 2))
  }
  /** `parsedType` child (visitor index 3) — any TypeNode variant. */
  get parsedType() {
    const internal = this.#internal
    const cached = internal.$parsedType
    if (cached !== undefined) return cached
    return (internal.$parsedType = childNodeAtVisitorIndex(internal, 3))
  }
  /** Body child (visitor index 4) — Generic / Borrows / Raw variant. */
  get body() {
    const internal = this.#internal
    const cached = internal.$body
    if (cached !== undefined) return cached
    return (internal.$body = childNodeAtVisitorIndex(internal, 4))
  }
  /** Source-preserving type lines (visitor index 5). */
  get typeLines() {
    const internal = this.#internal
    const cached = internal.$typeLines
    if (cached !== undefined) return cached
    return (internal.$typeLines = nodeListAtVisitorIndexExtended(internal, 5))
  }
  /** Source-preserving description lines (visitor index 6). */
  get descriptionLines() {
    const internal = this.#internal
    const cached = internal.$descriptionLines
    if (cached !== undefined) return cached
    return (internal.$descriptionLines = nodeListAtVisitorIndexExtended(internal, 6))
  }
  /** Inline tags found in this tag's description (visitor index 7). */
  get inlineTags() {
    const internal = this.#internal
    const cached = internal.$inlineTags
    if (cached !== undefined) return cached
    return (internal.$inlineTags = nodeListAtVisitorIndexExtended(internal, 7))
  }

  toJSON() {
    return {
      type: this.type,
      range: this.range,
      optional: this.optional,
      defaultValue: this.defaultValue,
      description: this.description,
      rawBody: this.rawBody,
      tag: this.tag?.toJSON() ?? null,
      rawType: this.rawType?.toJSON() ?? null,
      name: this.name?.toJSON() ?? null,
      parsedType: this.parsedType?.toJSON() ?? null,
      body: this.body?.toJSON() ?? null,
      typeLines: this.typeLines.map(n => n.toJSON()),
      descriptionLines: this.descriptionLines.map(n => n.toJSON()),
      inlineTags: this.inlineTags.map(n => n.toJSON())
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'JsdocTag')
  }
}

// ===========================================================================
// 0x04-0x06, 0x0B, 0x0D-0x0F: String-type leaves
// ===========================================================================

/**
 * Build a class for a single-string-payload leaf node.
 *
 * @param {string} typeName     The `type` field value.
 * @param {string} accessorName The accessor that returns the resolved string.
 * @returns {Function}
 */
function defineStringLeaf(typeName, accessorName) {
  return class {
    constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
      Object.defineProperty(this, 'type', { value: typeName, enumerable: true })
      this._internal = {
        view,
        byteIndex,
        index,
        rootIndex,
        parent,
        sourceFile,
        $range: undefined,
        $value: undefined
      }
    }
    get range() {
      const internal = this._internal
      return internal.$range !== undefined
        ? internal.$range
        : (internal.$range = absoluteRange(internal))
    }
    get parent() {
      return this._internal.parent
    }
    get [accessorName]() {
      const internal = this._internal
      const cached = internal.$value
      if (cached !== undefined) return cached
      return (internal.$value = stringPayloadOf(internal) ?? '')
    }
    toJSON() {
      return {
        type: this.type,
        range: this.range,
        [accessorName]: this[accessorName]
      }
    }
    [inspectSymbol]() {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}

/** `JsdocTagName` (Kind 0x04) — the `@name` token text. */
export const RemoteJsdocTagName = defineStringLeaf('JsdocTagName', 'value')
/** `JsdocTagNameValue` (Kind 0x05) — value after the type in `@param`. */
export const RemoteJsdocTagNameValue = defineStringLeaf('JsdocTagNameValue', 'raw')
/** `JsdocTypeSource` (Kind 0x06) — raw `{...}` text inside a tag. */
export const RemoteJsdocTypeSource = defineStringLeaf('JsdocTypeSource', 'raw')
/** `JsdocRawTagBody` (Kind 0x0B) — raw text body fallback. */
export const RemoteJsdocRawTagBody = defineStringLeaf('JsdocRawTagBody', 'raw')
/** `JsdocNamepathSource` (Kind 0x0D) — namepath token. */
export const RemoteJsdocNamepathSource = defineStringLeaf('JsdocNamepathSource', 'raw')
/** `JsdocIdentifier` (Kind 0x0E) — bare identifier. */
export const RemoteJsdocIdentifier = defineStringLeaf('JsdocIdentifier', 'name')
/** `JsdocText` (Kind 0x0F) — raw text. */
export const RemoteJsdocText = defineStringLeaf('JsdocText', 'value')

// ===========================================================================
// 0x07 RemoteJsdocTypeLine
// ===========================================================================

/**
 * `JsdocTypeLine` (Kind 0x07). Basic mode = String payload; compat mode
 * promotes `rawType` + delimiters into Extended Data.
 */
export class RemoteJsdocTypeLine {
  type = 'JsdocTypeLine'
  #internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this.#internal = {
      view,
      byteIndex,
      index,
      rootIndex,
      parent,
      sourceFile,
      $range: undefined,
      $rawType: undefined
    }
  }

  get range() {
    const internal = this.#internal
    return internal.$range !== undefined
      ? internal.$range
      : (internal.$range = absoluteRange(internal))
  }
  get parent() {
    return this.#internal.parent
  }

  /** Raw `{...}` line content. */
  get rawType() {
    const internal = this.#internal
    const cached = internal.$rawType
    if (cached !== undefined) return cached
    const value = internal.sourceFile.compatMode
      ? extU16StringRequired(internal, 0)
      : (stringPayloadOf(internal) ?? '')
    return (internal.$rawType = value)
  }

  toJSON() {
    return { type: this.type, range: this.range, rawType: this.rawType }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'JsdocTypeLine')
  }
}

// ===========================================================================
// 0x08 RemoteJsdocInlineTag
// ===========================================================================

/**
 * `JsdocInlineTag` (Kind 0x08) — e.g. `{@link Foo}`.
 */
export class RemoteJsdocInlineTag {
  type = 'JsdocInlineTag'
  #internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this.#internal = {
      view,
      byteIndex,
      index,
      rootIndex,
      parent,
      sourceFile,
      $range: undefined,
      $namepathOrURL: undefined,
      $text: undefined,
      $rawBody: undefined
    }
  }

  get range() {
    const internal = this.#internal
    return internal.$range !== undefined
      ? internal.$range
      : (internal.$range = absoluteRange(internal))
  }
  get parent() {
    return this.#internal.parent
  }

  /** Inline tag format string (`'plain' | 'pipe' | 'space' | 'prefix' | 'unknown'`). */
  get format() {
    return INLINE_TAG_FORMATS[commonData(this.#internal) & 0b0000_0111] ?? 'unknown'
  }
  /** Optional name path or URL portion. */
  get namepathOrURL() {
    const internal = this.#internal
    const cached = internal.$namepathOrURL
    if (cached !== undefined) return cached
    return (internal.$namepathOrURL = extU16String(internal, 0))
  }
  /** Optional display text portion. */
  get text() {
    const internal = this.#internal
    const cached = internal.$text
    if (cached !== undefined) return cached
    return (internal.$text = extU16String(internal, 2))
  }
  /** Raw body text fallback. */
  get rawBody() {
    const internal = this.#internal
    const cached = internal.$rawBody
    if (cached !== undefined) return cached
    return (internal.$rawBody = extU16String(internal, 4))
  }

  toJSON() {
    return {
      type: this.type,
      range: this.range,
      format: this.format,
      namepathOrURL: this.namepathOrURL,
      text: this.text,
      rawBody: this.rawBody
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'JsdocInlineTag')
  }
}

// ===========================================================================
// 0x09 RemoteJsdocGenericTagBody
// ===========================================================================

/**
 * `JsdocGenericTagBody` (Kind 0x09).
 */
export class RemoteJsdocGenericTagBody {
  type = 'JsdocGenericTagBody'
  #internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this.#internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }

  get range() {
    return absoluteRange(this.#internal)
  }
  get parent() {
    return this.#internal.parent
  }

  /** `true` when the tag separator was `-`. */
  get hasDashSeparator() {
    return (commonData(this.#internal) & 0b0000_0001) !== 0
  }
  /** Description text after the dash separator. */
  get description() {
    return extU16String(this.#internal, 2)
  }

  toJSON() {
    return {
      type: this.type,
      range: this.range,
      hasDashSeparator: this.hasDashSeparator,
      description: this.description
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'JsdocGenericTagBody')
  }
}

// ===========================================================================
// 0x0A RemoteJsdocBorrowsTagBody
// ===========================================================================

/**
 * `JsdocBorrowsTagBody` (Kind 0x0A) — Children type with `source` + `target`
 * children. The child accessors will be filled in once the parser starts
 * emitting them (Phase 1.2a); for now the class exposes the standard
 * range/parent/toJSON surface.
 */
export class RemoteJsdocBorrowsTagBody {
  type = 'JsdocBorrowsTagBody'
  #internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this.#internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }

  get range() {
    return absoluteRange(this.#internal)
  }
  get parent() {
    return this.#internal.parent
  }

  toJSON() {
    return { type: this.type, range: this.range }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'JsdocBorrowsTagBody')
  }
}

// ===========================================================================
// 0x0C RemoteJsdocParameterName
// ===========================================================================

/**
 * `JsdocParameterName` (Kind 0x0C) — `JsdocTagValue::Parameter` variant.
 */
export class RemoteJsdocParameterName {
  type = 'JsdocParameterName'
  #internal

  constructor(view, byteIndex, index, rootIndex, parent, sourceFile) {
    this.#internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }

  get range() {
    return absoluteRange(this.#internal)
  }
  get parent() {
    return this.#internal.parent
  }

  /** `true` when the parameter was wrapped in `[id]` brackets. */
  get optional() {
    return (commonData(this.#internal) & 0b0000_0001) !== 0
  }
  /** Path text. */
  get path() {
    return extU16StringRequired(this.#internal, 0)
  }
  /** Default value parsed from `[id=foo]` syntax. */
  get defaultValue() {
    return extU16String(this.#internal, 2)
  }

  toJSON() {
    return {
      type: this.type,
      range: this.range,
      optional: this.optional,
      path: this.path,
      defaultValue: this.defaultValue
    }
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'JsdocParameterName')
  }
}
