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

import { COMMON_DATA_MASK, COMMON_DATA_OFFSET, STRING_FIELD_SIZE } from '../constants.js'
import {
  absoluteRange,
  childNodeAtVisitorIndex,
  extStringField,
  extStringFieldRequired,
  extU32,
  extU8,
  stringPayloadOf
} from '../helpers.js'
import { inspectPayload, inspectSymbol } from '../inspect.js'
import { nodeListAtSlotExtended } from '../node-list.js'

// ---------------------------------------------------------------------------
// Per-Kind ED list-metadata slot offsets (see
// `crates/ox_jsdoc_binary/src/writer/nodes/comment_ast.rs`).
// ---------------------------------------------------------------------------

/** Size of one `(head: u32, count: u16)` list metadata slot. */
const LIST_METADATA_SIZE = 6

/** `JsdocBlock.descriptionLines` slot offset (right after 8 StringFields). */
const JSDOC_BLOCK_DESC_LINES_SLOT = 1 + 1 + 8 * STRING_FIELD_SIZE
/** `JsdocBlock.tags` slot offset. */
const JSDOC_BLOCK_TAGS_SLOT = JSDOC_BLOCK_DESC_LINES_SLOT + LIST_METADATA_SIZE
/** `JsdocBlock.inlineTags` slot offset. */
const JSDOC_BLOCK_INLINE_TAGS_SLOT = JSDOC_BLOCK_DESC_LINES_SLOT + 2 * LIST_METADATA_SIZE
/** `JsdocBlock` basic ED size (= start of compat tail). */
const JSDOC_BLOCK_BASIC_SIZE = JSDOC_BLOCK_DESC_LINES_SLOT + 3 * LIST_METADATA_SIZE

// JsdocBlock compat tail (only present when sourceFile.compatMode is true).
// Layout mirrors `crates/ox_jsdoc_binary/src/writer/nodes/comment_ast.rs`:
//   byte 68-69 : padding
//   byte 70-73 : end_line (u32)
//   byte 74-77 : description_start_line (u32, 0xFFFFFFFF = None)
//   byte 78-81 : description_end_line   (u32, 0xFFFFFFFF = None)
//   byte 82-85 : last_description_line  (u32, 0xFFFFFFFF = None)
//   byte 86    : has_preterminal_description (u8)
//   byte 87    : has_preterminal_tag_description (u8, 0xFF = None)
const JSDOC_BLOCK_END_LINE_OFFSET = JSDOC_BLOCK_BASIC_SIZE + 2
const JSDOC_BLOCK_DESC_START_LINE_OFFSET = JSDOC_BLOCK_END_LINE_OFFSET + 4
const JSDOC_BLOCK_DESC_END_LINE_OFFSET = JSDOC_BLOCK_DESC_START_LINE_OFFSET + 4
const JSDOC_BLOCK_LAST_DESC_LINE_OFFSET = JSDOC_BLOCK_DESC_END_LINE_OFFSET + 4
const JSDOC_BLOCK_HAS_PRETERMINAL_DESC_OFFSET = JSDOC_BLOCK_LAST_DESC_LINE_OFFSET + 4
const JSDOC_BLOCK_HAS_PRETERMINAL_TAG_DESC_OFFSET = JSDOC_BLOCK_HAS_PRETERMINAL_DESC_OFFSET + 1
/** `0xFFFFFFFF` sentinel for `Option<u32>` line indices in compat mode. */
const COMPAT_LINE_NONE = 0xff_ff_ff_ff
/** `0xFF` sentinel for `Option<u8>` flags in compat mode. */
const COMPAT_U8_NONE = 0xff

/** `JsdocTag.typeLines` slot offset (right after 3 StringFields). */
const JSDOC_TAG_TYPE_LINES_SLOT = 1 + 1 + 3 * STRING_FIELD_SIZE
/** `JsdocTag.descriptionLines` slot offset. */
const JSDOC_TAG_DESC_LINES_SLOT = JSDOC_TAG_TYPE_LINES_SLOT + LIST_METADATA_SIZE
/** `JsdocTag.inlineTags` slot offset. */
const JSDOC_TAG_INLINE_TAGS_SLOT = JSDOC_TAG_TYPE_LINES_SLOT + 2 * LIST_METADATA_SIZE
/** `JsdocTag` basic ED size (= start of compat tail). */
const JSDOC_TAG_BASIC_SIZE = JSDOC_TAG_TYPE_LINES_SLOT + 3 * LIST_METADATA_SIZE

// JsdocTag compat tail (7 StringFields, 42 bytes, basic+38..=basic+79):
//   delimiter, post_delimiter, post_tag, post_type, post_name, initial, line_end
const JSDOC_TAG_COMPAT_DELIMITER = JSDOC_TAG_BASIC_SIZE
const JSDOC_TAG_COMPAT_POST_DELIMITER = JSDOC_TAG_COMPAT_DELIMITER + STRING_FIELD_SIZE
const JSDOC_TAG_COMPAT_POST_TAG = JSDOC_TAG_COMPAT_POST_DELIMITER + STRING_FIELD_SIZE
const JSDOC_TAG_COMPAT_POST_TYPE = JSDOC_TAG_COMPAT_POST_TAG + STRING_FIELD_SIZE
const JSDOC_TAG_COMPAT_POST_NAME = JSDOC_TAG_COMPAT_POST_TYPE + STRING_FIELD_SIZE
const JSDOC_TAG_COMPAT_INITIAL = JSDOC_TAG_COMPAT_POST_NAME + STRING_FIELD_SIZE
const JSDOC_TAG_COMPAT_LINE_END = JSDOC_TAG_COMPAT_INITIAL + STRING_FIELD_SIZE

// JsdocDescriptionLine / JsdocTypeLine compat tail (3 optional StringFields,
// after the leading `description` / `raw_type` StringField at byte 0-5).
const COMPAT_LINE_DELIMITER = STRING_FIELD_SIZE
const COMPAT_LINE_POST_DELIMITER = 2 * STRING_FIELD_SIZE
const COMPAT_LINE_INITIAL = 3 * STRING_FIELD_SIZE

// ---------------------------------------------------------------------------
// Phase 6: source[]/tokens[] reconstruction (compat-mode only)
// ---------------------------------------------------------------------------
//
// jsdoccomment's `JsdocBlock.source[]` mirrors comment-parser's `Block.source`:
// one entry per source line, each with a `tokens` object holding 12 fields
// (start, delimiter, postDelimiter, tag, postTag, type, postType, name,
// postName, description, end, lineEnd). Many eslint-plugin-jsdoc fixer
// rules mutate these fields directly, so faithful reconstruction is on
// the critical path for using this AST as a jsdoccomment drop-in.
//
// The reconstruction here is a *skeleton* (Phase 6 v1): it walks the
// existing JsdocBlock fields + descriptionLines + tags + per-tag
// descriptionLines and stitches an entry per logical source line. Edge
// cases that depend on parser/writer fixes (block.delimiter currently
// stores "*" instead of "/**", empty descriptionLines are dropped at
// parse time, content on opening/closing line) produce close-but-imperfect
// output and are tracked alongside the Level 2 dynamic comparison test.

/** Empty `tokens` template — every key present so consumers can index by
 * field name without truthy checks. Mirrors comment-parser's
 * `seedTokens()`. */
function emptyTokens() {
  return {
    start: '',
    delimiter: '',
    postDelimiter: '',
    tag: '',
    postTag: '',
    name: '',
    postName: '',
    type: '',
    postType: '',
    description: '',
    end: '',
    lineEnd: ''
  }
}

/** Concatenate every token field (in jsdoccomment order) to rebuild the
 * `source` string for one line. Mirrors `comment-parser.stringify()` for
 * a single Line. */
function tokensToSource(t) {
  return (
    t.start +
    t.delimiter +
    t.postDelimiter +
    t.tag +
    t.postTag +
    t.type +
    t.postType +
    t.name +
    t.postName +
    t.description +
    t.end +
    t.lineEnd
  )
}

/** Build one source-array entry from a `JsdocDescriptionLine`. The entry
 * carries no tag/type/name segments; only the line's prose. */
function descriptionLineToSourceEntry(descLine, number) {
  const tokens = emptyTokens()
  tokens.start = descLine.initial ?? ''
  tokens.delimiter = descLine.delimiter ?? ''
  tokens.postDelimiter = descLine.postDelimiter ?? ''
  tokens.description = descLine.description ?? ''
  return { number, source: tokensToSource(tokens), tokens }
}

/** Build the per-tag header entry (the line carrying `@name {type} value …`). */
function tagHeaderToSourceEntry(tag, number) {
  const tokens = emptyTokens()
  tokens.start = tag.initial ?? ''
  tokens.delimiter = tag.delimiter ?? ''
  tokens.postDelimiter = tag.postDelimiter ?? ''
  tokens.tag = tag.tag ? '@' + tag.tag.value : ''
  tokens.postTag = tag.postTag ?? ''
  // jsdoccomment keeps the surrounding braces in `type`; ox-jsdoc's
  // RemoteJsdocTypeSource strips them, so wrap them back.
  if (tag.rawType) {
    tokens.type = '{' + tag.rawType.raw + '}'
  }
  tokens.postType = tag.postType ?? ''
  if (tag.name) {
    tokens.name = tag.name.raw
  }
  tokens.postName = tag.postName ?? ''
  tokens.description = tag.description ?? ''
  tokens.lineEnd = tag.lineEnd ?? ''
  return { number, source: tokensToSource(tokens), tokens }
}

/** Build the `source[]` array for a JsdocBlock. */
function buildBlockSource(block) {
  const out = []
  let number = 0

  // Opening `/**` line. ox-jsdoc currently stores `block.delimiter` as the
  // per-line `*` marker (see KNOWN_DIFFERENCES), so we override here with
  // the literal `/**` to match jsdoccomment's expectation.
  const openingTokens = emptyTokens()
  openingTokens.start = block.initial ?? ''
  openingTokens.delimiter = '/**'
  openingTokens.postDelimiter = block.postDelimiter ?? ''
  out.push({ number: number++, source: tokensToSource(openingTokens), tokens: openingTokens })

  // Block-level description lines.
  for (const descLine of block.descriptionLines) {
    out.push(descriptionLineToSourceEntry(descLine, number++))
  }

  // Tags. Each tag contributes a header line + one continuation entry
  // per *additional* descriptionLine. The first descriptionLine shares
  // the same source line as the tag header (jsdoccomment treats them as
  // one source row), so we skip index 0. (typeLines beyond the first
  // are an edge case we collapse into the description for v1; revisit
  // alongside the parser multi-line type fix.)
  for (const tag of block.tags) {
    out.push(tagHeaderToSourceEntry(tag, number++))
    for (let i = 1; i < tag.descriptionLines.length; i++) {
      out.push(descriptionLineToSourceEntry(tag.descriptionLines[i], number++))
    }
  }

  // Closing `*/` line.
  const closingTokens = emptyTokens()
  closingTokens.start = block.initial ?? ''
  closingTokens.end = block.terminal ?? '*/'
  out.push({ number, source: tokensToSource(closingTokens), tokens: closingTokens })

  return out
}

/** Build the per-tag `source[]` subset (every line that belongs to this
 * tag — header + descriptionLines beyond the first). The `number` field
 * is left tag-local (as if the tag stood alone). */
function buildTagSource(tag) {
  const out = []
  let number = 0
  out.push(tagHeaderToSourceEntry(tag, number++))
  // Skip descriptionLines[0] — same source row as the tag header.
  for (let i = 1; i < tag.descriptionLines.length; i++) {
    out.push(descriptionLineToSourceEntry(tag.descriptionLines[i], number++))
  }
  return out
}

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
//
// Extended Data layout (basic 50 bytes; compat extends to 72 bytes):
//   byte 0      : Children bitmask (u8)
//   byte 1      : padding (u8)
//   byte 2-7    : description     (StringField, NONE if absent)
//   byte 8-13   : delimiter
//   byte 14-19  : post_delimiter
//   byte 20-25  : terminal
//   byte 26-31  : line_end
//   byte 32-37  : initial
//   byte 38-43  : delimiter_line_break
//   byte 44-49  : preterminal_line_break
// ===========================================================================

/**
 * `JsdocBlock` (Kind 0x01) — root of every parsed `/** ... *​/` comment.
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

  /** Top-level description string (`null` when absent). The
   * `emptyStringForNull` option only affects `toJSON()` output. */
  get description() {
    const internal = this.#internal
    const cached = internal.$description
    if (cached !== undefined) return cached
    return (internal.$description = extStringField(internal, 2))
  }
  /** Source-preserving `*` line-prefix delimiter. */
  get delimiter() {
    const internal = this.#internal
    const cached = internal.$delimiter
    if (cached !== undefined) return cached
    return (internal.$delimiter = extStringFieldRequired(internal, 8))
  }
  /** Source-preserving space after `*`. */
  get postDelimiter() {
    const internal = this.#internal
    const cached = internal.$postDelimiter
    if (cached !== undefined) return cached
    return (internal.$postDelimiter = extStringFieldRequired(internal, 14))
  }
  /** Source-preserving `*​/` terminal. */
  get terminal() {
    const internal = this.#internal
    const cached = internal.$terminal
    if (cached !== undefined) return cached
    return (internal.$terminal = extStringFieldRequired(internal, 20))
  }
  /** Source-preserving line-end characters. */
  get lineEnd() {
    const internal = this.#internal
    const cached = internal.$lineEnd
    if (cached !== undefined) return cached
    return (internal.$lineEnd = extStringFieldRequired(internal, 26))
  }
  /** Indentation before the leading `*`. */
  get initial() {
    const internal = this.#internal
    const cached = internal.$initial
    if (cached !== undefined) return cached
    return (internal.$initial = extStringFieldRequired(internal, 32))
  }
  /** Line-break right after `/**`. */
  get delimiterLineBreak() {
    const internal = this.#internal
    const cached = internal.$delimiterLineBreak
    if (cached !== undefined) return cached
    return (internal.$delimiterLineBreak = extStringFieldRequired(internal, 38))
  }
  /** Line-break right before `*​/`. */
  get preterminalLineBreak() {
    const internal = this.#internal
    const cached = internal.$preterminalLineBreak
    if (cached !== undefined) return cached
    return (internal.$preterminalLineBreak = extStringFieldRequired(internal, 44))
  }

  // -- compat-only line metadata -------------------------------------------
  // The following 6 fields exist only when `sourceFile.compatMode` is true
  // (basic-mode ED records stop at byte 68; reading further would walk into
  // the next node's bytes). Each getter returns `null` outside compat mode.

  /** Total number of LogicalLines in this comment (compat-mode only). */
  get endLine() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extU32(internal, JSDOC_BLOCK_END_LINE_OFFSET)
  }
  /** Index of the first description line, or `null` when absent. */
  get descriptionStartLine() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    const v = extU32(internal, JSDOC_BLOCK_DESC_START_LINE_OFFSET)
    return v === COMPAT_LINE_NONE ? null : v
  }
  /** Index of the last description line, or `null` when absent. */
  get descriptionEndLine() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    const v = extU32(internal, JSDOC_BLOCK_DESC_END_LINE_OFFSET)
    return v === COMPAT_LINE_NONE ? null : v
  }
  /** Description-boundary index (jsdoccomment's `lastDescriptionLine` —
   * actually the index of the first tag/end line). `null` when absent. */
  get lastDescriptionLine() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    const v = extU32(internal, JSDOC_BLOCK_LAST_DESC_LINE_OFFSET)
    return v === COMPAT_LINE_NONE ? null : v
  }
  /** `1` when block description text exists on the `*​/` line. */
  get hasPreterminalDescription() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extU8(internal, JSDOC_BLOCK_HAS_PRETERMINAL_DESC_OFFSET)
  }
  /** `1` when tag description text exists on the `*​/` line; `null` when not
   * applicable (no active lastTag at end). */
  get hasPreterminalTagDescription() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    const v = extU8(internal, JSDOC_BLOCK_HAS_PRETERMINAL_TAG_DESC_OFFSET)
    return v === COMPAT_U8_NONE ? null : v
  }

  /** Top-level description lines. */
  get descriptionLines() {
    const internal = this.#internal
    const cached = internal.$descriptionLines
    if (cached !== undefined) return cached
    return (internal.$descriptionLines = nodeListAtSlotExtended(
      internal,
      JSDOC_BLOCK_DESC_LINES_SLOT
    ))
  }
  /** Block tags. */
  get tags() {
    const internal = this.#internal
    const cached = internal.$tags
    if (cached !== undefined) return cached
    return (internal.$tags = nodeListAtSlotExtended(internal, JSDOC_BLOCK_TAGS_SLOT))
  }
  /** Inline tags found inside the top-level description. */
  get inlineTags() {
    const internal = this.#internal
    const cached = internal.$inlineTags
    if (cached !== undefined) return cached
    return (internal.$inlineTags = nodeListAtSlotExtended(
      internal,
      JSDOC_BLOCK_INLINE_TAGS_SLOT
    ))
  }

  toJSON() {
    const internal = this.#internal
    const nullToEmpty = internal.sourceFile.emptyStringForNull
    const json = {
      type: this.type,
      range: this.range,
      description: nullToEmpty ? (this.description ?? '') : this.description,
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
    if (internal.sourceFile.compatMode) {
      // Match jsdoccomment's optional-field serialization: omit absent
      // line indices (Rust serializer uses `skip_serializing_if = "Option::is_none"`).
      json.endLine = this.endLine
      const dsl = this.descriptionStartLine
      if (dsl !== null) json.descriptionStartLine = dsl
      const del = this.descriptionEndLine
      if (del !== null) json.descriptionEndLine = del
      const ldl = this.lastDescriptionLine
      if (ldl !== null) json.lastDescriptionLine = ldl
      json.hasPreterminalDescription = this.hasPreterminalDescription
      const hptd = this.hasPreterminalTagDescription
      if (hptd !== null) json.hasPreterminalTagDescription = hptd
      // Phase 6: comment-parser-shape source[] array.
      json.source = buildBlockSource(this)
    }
    return json
  }

  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'JsdocBlock')
  }
}

// ===========================================================================
// 0x02 RemoteJsdocDescriptionLine
//
// Extended Data layout: byte 0-5 description (always required); compat
// adds 3 × 6-byte StringField slots after it.
// ===========================================================================

/**
 * `JsdocDescriptionLine` (Kind 0x02). Both basic and compat modes store
 * `description` as the leading StringField of the Extended Data record.
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

  /** Description content. Basic mode reads the String payload (Node Data);
   * compat mode reads byte 0-5 of the Extended Data record. */
  get description() {
    const internal = this.#internal
    const cached = internal.$description
    if (cached !== undefined) return cached
    const value = internal.sourceFile.compatMode
      ? extStringFieldRequired(internal, 0)
      : (stringPayloadOf(internal) ?? '')
    return (internal.$description = value)
  }

  // -- compat-only delimiter trio ------------------------------------------
  // Stored in ED bytes 6-23 only when sourceFile.compatMode is true.

  /** Source-preserving `*` line-prefix (compat-mode only; `null` otherwise). */
  get delimiter() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringField(internal, COMPAT_LINE_DELIMITER)
  }
  /** Source-preserving space after `*` (compat-mode only). */
  get postDelimiter() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringField(internal, COMPAT_LINE_POST_DELIMITER)
  }
  /** Indentation before the leading `*` (compat-mode only). */
  get initial() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringField(internal, COMPAT_LINE_INITIAL)
  }

  toJSON() {
    const json = { type: this.type, range: this.range, description: this.description }
    if (this.#internal.sourceFile.compatMode) {
      json.delimiter = this.delimiter ?? ''
      json.postDelimiter = this.postDelimiter ?? ''
      json.initial = this.initial ?? ''
    }
    return json
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'JsdocDescriptionLine')
  }
}

// ===========================================================================
// 0x03 RemoteJsdocTag
//
// Extended Data layout (basic 20 bytes; compat extends to 62 bytes):
//   byte 0      : Children bitmask (u8)
//   byte 1      : padding (u8)
//   byte 2-7    : default_value (StringField, NONE if absent)
//   byte 8-13   : description    (StringField, NONE if absent)
//   byte 14-19  : raw_body       (StringField, NONE if absent)
// ===========================================================================

/**
 * `JsdocTag` (Kind 0x03) — one block tag (e.g. `@param`).
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
    return (internal.$defaultValue = extStringField(internal, 2))
  }
  /** Joined description text. */
  get description() {
    const internal = this.#internal
    const cached = internal.$description
    if (cached !== undefined) return cached
    return (internal.$description = extStringField(internal, 8))
  }
  /** Raw body when the tag uses the `Raw` body variant. */
  get rawBody() {
    const internal = this.#internal
    const cached = internal.$rawBody
    if (cached !== undefined) return cached
    return (internal.$rawBody = extStringField(internal, 14))
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
  /** Source-preserving type lines. */
  get typeLines() {
    const internal = this.#internal
    const cached = internal.$typeLines
    if (cached !== undefined) return cached
    return (internal.$typeLines = nodeListAtSlotExtended(internal, JSDOC_TAG_TYPE_LINES_SLOT))
  }
  /** Source-preserving description lines. */
  get descriptionLines() {
    const internal = this.#internal
    const cached = internal.$descriptionLines
    if (cached !== undefined) return cached
    return (internal.$descriptionLines = nodeListAtSlotExtended(
      internal,
      JSDOC_TAG_DESC_LINES_SLOT
    ))
  }
  /** Inline tags found in this tag's description. */
  get inlineTags() {
    const internal = this.#internal
    const cached = internal.$inlineTags
    if (cached !== undefined) return cached
    return (internal.$inlineTags = nodeListAtSlotExtended(internal, JSDOC_TAG_INLINE_TAGS_SLOT))
  }

  // -- compat-only delimiter strings ---------------------------------------
  // Stored in ED bytes 38-79 (7 StringFields) only when compatMode is true.

  /** Source-preserving `*` line-prefix (compat-mode only). */
  get delimiter() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_DELIMITER)
  }
  /** Source-preserving space after `*` (compat-mode only). */
  get postDelimiter() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_POST_DELIMITER)
  }
  /** Whitespace after the `@name` token (compat-mode only). */
  get postTag() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_POST_TAG)
  }
  /** Whitespace after the `{type}` source (compat-mode only). */
  get postType() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_POST_TYPE)
  }
  /** Whitespace after the name token (compat-mode only). */
  get postName() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_POST_NAME)
  }
  /** Indentation before the line's `*` (compat-mode only). */
  get initial() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_INITIAL)
  }
  /** Line ending of the tag's first line (compat-mode only). */
  get lineEnd() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_LINE_END)
  }

  toJSON() {
    const internal = this.#internal
    const compat = internal.sourceFile.compatMode
    const nullToEmpty = internal.sourceFile.emptyStringForNull

    if (compat) {
      // jsdoccomment-shape: omit ox-jsdoc-specific fields (optional,
      // defaultValue, rawBody, body) and surface delimiter strings + lineEnd.
      const tagNode = this.tag?.toJSON() ?? null
      const tagName = tagNode?.value ?? ''
      const rawTypeNode = this.rawType?.toJSON() ?? null
      const rawTypeRaw = rawTypeNode?.raw ?? null
      const nameNode = this.name?.toJSON() ?? null
      const nameValue = nameNode?.raw ?? null
      // Build a synthetic facade so buildTagSource() can read camelCase
      // fields without re-rolling the StringField lookups for every tag
      // entry. Only the keys consumed by tagHeaderToSourceEntry need to
      // be present.
      const tagFacade = {
        tag: tagNode ? { value: tagName } : null,
        rawType: rawTypeNode ? { raw: rawTypeRaw ?? '' } : null,
        name: nameNode ? { raw: nameValue ?? '' } : null,
        description: this.description,
        initial: this.initial,
        delimiter: this.delimiter,
        postDelimiter: this.postDelimiter,
        postTag: this.postTag,
        postType: this.postType,
        postName: this.postName,
        lineEnd: this.lineEnd,
        descriptionLines: this.descriptionLines
      }
      return {
        type: this.type,
        range: this.range,
        tag: tagName,
        rawType: nullToEmpty ? (rawTypeRaw ?? '') : rawTypeRaw,
        name: nullToEmpty ? (nameValue ?? '') : nameValue,
        description: this.description ?? (nullToEmpty ? '' : null),
        delimiter: this.delimiter,
        postDelimiter: this.postDelimiter,
        postTag: this.postTag,
        postType: this.postType,
        postName: this.postName,
        initial: this.initial,
        lineEnd: this.lineEnd,
        parsedType: this.parsedType?.toJSON() ?? null,
        typeLines: this.typeLines.map(n => n.toJSON()),
        descriptionLines: this.descriptionLines.map(n => n.toJSON()),
        inlineTags: this.inlineTags.map(n => n.toJSON()),
        // Phase 6: per-tag source[] subset (the lines that belong to this
        // tag — header + description continuation).
        source: buildTagSource(tagFacade)
      }
    }
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
// 0x04-0x06, 0x0B, 0x0D-0x0F: Extended-type string-leaf nodes
// (each carries a single 6-byte StringField in Extended Data)
// ===========================================================================

/**
 * Build a class for a single-string-leaf node.
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
//
// Extended Data layout: byte 0-5 raw_type (always required); compat adds
// 3 × 6-byte StringField slots after it.
// ===========================================================================

/**
 * `JsdocTypeLine` (Kind 0x07).
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

  /** Raw `{...}` line content. Basic mode reads the String payload;
   * compat mode reads byte 0-5 of the Extended Data record. */
  get rawType() {
    const internal = this.#internal
    const cached = internal.$rawType
    if (cached !== undefined) return cached
    const value = internal.sourceFile.compatMode
      ? extStringFieldRequired(internal, 0)
      : (stringPayloadOf(internal) ?? '')
    return (internal.$rawType = value)
  }

  // -- compat-only delimiter trio (same layout as JsdocDescriptionLine) ----

  /** Source-preserving `*` line-prefix (compat-mode only; `null` otherwise). */
  get delimiter() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringField(internal, COMPAT_LINE_DELIMITER)
  }
  /** Source-preserving space after `*` (compat-mode only). */
  get postDelimiter() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringField(internal, COMPAT_LINE_POST_DELIMITER)
  }
  /** Indentation before the leading `*` (compat-mode only). */
  get initial() {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) return null
    return extStringField(internal, COMPAT_LINE_INITIAL)
  }

  toJSON() {
    const json = { type: this.type, range: this.range, rawType: this.rawType }
    if (this.#internal.sourceFile.compatMode) {
      json.delimiter = this.delimiter ?? ''
      json.postDelimiter = this.postDelimiter ?? ''
      json.initial = this.initial ?? ''
    }
    return json
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'JsdocTypeLine')
  }
}

// ===========================================================================
// 0x08 RemoteJsdocInlineTag
//
// Extended Data layout (18 bytes):
//   byte 0-5    : namepath_or_url (StringField, NONE if absent)
//   byte 6-11   : text             (StringField, NONE if absent)
//   byte 12-17  : raw_body         (StringField, NONE if absent)
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

  /** Inline tag format string. In compat mode the `'unknown'` variant is
   * mapped to `'plain'` to mirror jsdoccomment's behavior. */
  get format() {
    const internal = this.#internal
    const raw = INLINE_TAG_FORMATS[commonData(internal) & 0b0000_0111] ?? 'unknown'
    return raw === 'unknown' && internal.sourceFile.compatMode ? 'plain' : raw
  }
  /** Optional name path or URL portion. */
  get namepathOrURL() {
    const internal = this.#internal
    const cached = internal.$namepathOrURL
    if (cached !== undefined) return cached
    return (internal.$namepathOrURL = extStringField(internal, 0))
  }
  /** Optional display text portion. */
  get text() {
    const internal = this.#internal
    const cached = internal.$text
    if (cached !== undefined) return cached
    return (internal.$text = extStringField(internal, STRING_FIELD_SIZE))
  }
  /** Raw body text fallback. */
  get rawBody() {
    const internal = this.#internal
    const cached = internal.$rawBody
    if (cached !== undefined) return cached
    return (internal.$rawBody = extStringField(internal, 2 * STRING_FIELD_SIZE))
  }

  toJSON() {
    const internal = this.#internal
    const compat = internal.sourceFile.compatMode
    const nullToEmpty = internal.sourceFile.emptyStringForNull
    // NOTE: jsdoccomment's `JsdocInlineTag` exposes a `tag` field (the inline
    // tag name without `@`, e.g. "link"). The binary writer does not currently
    // serialize this value (`emit_inline_tag` discards `inline.tag_name`), so
    // we cannot reproduce it here. Track as a future binary-format extension.
    const json = {
      type: this.type,
      range: this.range,
      format: this.format,
      namepathOrURL: nullToEmpty ? (this.namepathOrURL ?? '') : this.namepathOrURL,
      text: nullToEmpty ? (this.text ?? '') : this.text
    }
    // jsdoccomment excludes rawBody from inline-tag output.
    if (!compat) json.rawBody = this.rawBody
    return json
  }
  [inspectSymbol]() {
    return inspectPayload(this.toJSON(), 'JsdocInlineTag')
  }
}

// ===========================================================================
// 0x09 RemoteJsdocGenericTagBody
//
// Extended Data layout (8 bytes):
//   byte 0      : Children bitmask (u8)
//   byte 1      : padding (u8)
//   byte 2-7    : description (StringField, NONE if absent)
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
    return extStringField(this.#internal, 2)
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
 * emitting them; for now the class exposes the standard range/parent/toJSON
 * surface.
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
//
// Extended Data layout (12 bytes):
//   byte 0-5    : path           (StringField, required)
//   byte 6-11   : default_value  (StringField, NONE if absent)
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
    return extStringFieldRequired(this.#internal, 0)
  }
  /** Default value parsed from `[id=foo]` syntax. */
  get defaultValue() {
    return extStringField(this.#internal, STRING_FIELD_SIZE)
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
