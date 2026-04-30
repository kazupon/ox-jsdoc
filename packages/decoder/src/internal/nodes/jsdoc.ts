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

import {
  COMMON_DATA_MASK,
  COMMON_DATA_OFFSET,
  JSDOC_BLOCK_BASIC_SIZE as JSDOC_BLOCK_BASIC_SIZE_CONST,
  JSDOC_BLOCK_COMPAT_SIZE as JSDOC_BLOCK_COMPAT_SIZE_CONST,
  JSDOC_BLOCK_HAS_DESCRIPTION_RAW_SPAN_BIT,
  JSDOC_TAG_BASIC_SIZE as JSDOC_TAG_BASIC_SIZE_CONST,
  JSDOC_TAG_COMPAT_SIZE as JSDOC_TAG_COMPAT_SIZE_CONST,
  JSDOC_TAG_HAS_DESCRIPTION_RAW_SPAN_BIT,
  STRING_FIELD_SIZE
} from '../constants.ts'
import {
  absoluteRange,
  childNodeAtVisitorIndex,
  extStringField,
  extStringFieldRequired,
  extU32,
  extU8,
  stringPayloadOf
} from '../helpers.ts'
import { inspectPayload, inspectSymbol } from '../inspect.ts'
import { nodeListAtSlotExtended, RemoteNodeList } from '../node-list.ts'
import { parsedPreservingWhitespace } from '../preserve-whitespace.ts'
import type {
  LazyNode,
  LazyNodeConstructor,
  RemoteInternal,
  RemoteJsonObject,
  RemoteJsonValue,
  RemoteSourceFileLike
} from '../types.ts'

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
// `description_raw_span` is **opt-in** in Phase 5: presence is gated on
// `JSDOC_BLOCK_HAS_DESCRIPTION_RAW_SPAN_BIT` in Common Data. When set, the
// 8-byte span sits at the **end** of the ED record at offset
// `compatMode ? JSDOC_BLOCK_COMPAT_SIZE_CONST : JSDOC_BLOCK_BASIC_SIZE_CONST`.
// See `design/008-oxlint-oxfmt-support/README.md` §4.2.
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
// `description_raw_span` is **opt-in** in Phase 5: presence is gated on
// `JSDOC_TAG_HAS_DESCRIPTION_RAW_SPAN_BIT` in Common Data. When set, the
// 8-byte span sits at the **end** of the ED record at offset
// `compatMode ? JSDOC_TAG_COMPAT_SIZE_CONST : JSDOC_TAG_BASIC_SIZE_CONST`.
// See `design/008-oxlint-oxfmt-support/README.md` §4.2.

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

interface LineTokens {
  start: string
  delimiter: string
  postDelimiter: string
  tag: string
  postTag: string
  name: string
  postName: string
  type: string
  postType: string
  description: string
  end: string
  lineEnd: string
}

interface PhysicalLine {
  source: string
  lineEnd: string
  startOffset: number
  endOffset: number
}

interface SourceEntry {
  number: number
  source: string
  tokens: LineTokens
  startOffset: number
  endOffset: number
}

interface PublicSourceEntry {
  number: number
  source: string
  tokens: LineTokens
}

/** Empty `tokens` template — every key present so consumers can index by
 * field name without truthy checks. Mirrors comment-parser's
 * `seedTokens()`. */
function emptyTokens(): LineTokens {
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
function tokensToSource(t: LineTokens): string {
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

const DEFAULT_NO_TYPES = new Set([
  'default',
  'defaultvalue',
  'description',
  'example',
  'file',
  'fileoverview',
  'license',
  'overview',
  'see',
  'summary'
])

const DEFAULT_NO_NAMES = new Set([
  'access',
  'author',
  'default',
  'defaultvalue',
  'description',
  'example',
  'exception',
  'file',
  'fileoverview',
  'kind',
  'license',
  'overview',
  'return',
  'returns',
  'since',
  'summary',
  'throws',
  'version',
  'variation'
])

const TAG_HEADER_RE = /^@(?<tag>[^\s{]+)(?<postTag>\s*)/u

interface ConsumedType {
  type: string
  postType: string
  rest: string
}

function consumeBalancedType(text: string): ConsumedType | null {
  if (!text.startsWith('{')) {
    return null
  }
  let depth = 0
  for (let i = 0; i < text.length; i++) {
    const ch = text[i]
    if (ch === '{') {
      depth++
    }
    if (ch === '}') {
      depth--
      if (depth === 0) {
        const type = text.slice(0, i + 1)
        const rest = text.slice(i + 1)
        const ws = rest.match(/^\s*/u)?.[0] ?? ''
        return { type, postType: ws, rest: rest.slice(ws.length) }
      }
    }
  }
  return null
}

interface BraceState {
  depth: number
  closeIndex: number
}

function braceDepthDelta(text: string, initialDepth = 0): BraceState {
  let depth = initialDepth
  let closeIndex = -1
  for (let i = 0; i < text.length; i++) {
    const ch = text[i]
    if (ch === '{') {
      depth++
    }
    if (ch === '}') {
      depth--
      if (depth === 0) {
        closeIndex = i
        break
      }
    }
  }
  return { depth, closeIndex }
}

interface SplitName {
  name: string
  postName: string
  rest: string
}

function splitName(text: string): SplitName {
  if (text === '') {
    return { name: '', postName: '', rest: '' }
  }
  const match = text.match(/^(?<name>\S+)(?<postName>\s*)(?<rest>.*)$/su)
  return {
    name: match?.groups?.name ?? '',
    postName: match?.groups?.postName ?? '',
    rest: match?.groups?.rest ?? ''
  }
}

function splitTemplateName(text: string): SplitName {
  let pos: number
  if (text.startsWith('[') && text.includes(']')) {
    const endingBracketPos = text.lastIndexOf(']')
    pos = text.slice(endingBracketPos).search(/(?<![\s,])\s/u)
    if (pos > -1) {
      pos += endingBracketPos
    }
  } else {
    pos = text.search(/(?<![\s,])\s/u)
  }
  const name = pos === -1 ? text : text.slice(0, pos)
  const extra = pos === -1 ? '' : text.slice(pos)
  const match = extra.match(/^(?<postName>\s*)(?<rest>[^\r]*)(?<lineEnd>\r)?$/u)
  return {
    name,
    postName: match?.groups?.postName ?? '',
    rest: match?.groups?.rest ?? ''
  }
}

function applyTagTokens(tokens: LineTokens): void {
  const header = tokens.description.match(TAG_HEADER_RE)
  if (!header?.groups) {
    return
  }

  const tagName = header.groups.tag!
  tokens.tag = '@' + tagName
  tokens.postTag = header.groups.postTag ?? ''
  let rest = tokens.description.slice(header[0].length)
  tokens.description = ''

  if (!DEFAULT_NO_TYPES.has(tagName)) {
    const parsedType = consumeBalancedType(rest)
    if (parsedType) {
      tokens.type = parsedType.type
      tokens.postType = parsedType.postType
      rest = parsedType.rest
    } else if (rest.startsWith('{')) {
      tokens.type = rest
      return
    }
  }

  if (tagName === 'template') {
    const parsedName = splitTemplateName(rest)
    tokens.name = parsedName.name
    tokens.postName = parsedName.postName
    tokens.description = parsedName.rest
    return
  }

  if (
    DEFAULT_NO_NAMES.has(tagName) ||
    (tagName === 'see' && /\{@link.+?\}/u.test(tokensToSource(tokens) + rest))
  ) {
    tokens.description = rest
    return
  }

  const parsedName = splitName(rest)
  tokens.name = parsedName.name
  tokens.postName = parsedName.postName
  tokens.description = parsedName.rest
}

function splitPhysicalLines(sourceText: string, baseOffset: number): PhysicalLine[] {
  const lines: PhysicalLine[] = []
  let offset = 0
  const pattern = /.*(?:\r\n|\n|\r|$)/gu
  for (const match of sourceText.matchAll(pattern)) {
    const raw = match[0]
    if (raw === '') {
      break
    }
    let lineEnd = ''
    let source = raw
    if (raw.endsWith('\r\n')) {
      lineEnd = '\r\n'
      source = raw.slice(0, -2)
    } else if (raw.endsWith('\n') || raw.endsWith('\r')) {
      lineEnd = raw.at(-1) ?? ''
      source = raw.slice(0, -1)
    }
    lines.push({
      source,
      lineEnd,
      startOffset: baseOffset + offset,
      endOffset: baseOffset + offset + source.length
    })
    offset += raw.length
  }
  return lines
}

function lineToSourceEntry(line: PhysicalLine, number: number): SourceEntry {
  const tokens = emptyTokens()
  tokens.lineEnd = ''

  let rest = line.source
  const opening = rest.indexOf('/**')
  if (opening !== -1) {
    tokens.start = rest.slice(0, opening)
    tokens.delimiter = '/**'
    rest = rest.slice(opening + 3)
  } else {
    const initial = rest.match(/^\s*/u)?.[0] ?? ''
    tokens.start = initial
    rest = rest.slice(initial.length)
    if (rest.startsWith('*') && !rest.startsWith('*/')) {
      tokens.delimiter = '*'
      rest = rest.slice(1)
    }
  }

  if (tokens.delimiter) {
    const postDelimiter = rest.match(/^[ \t]*/u)?.[0] ?? ''
    tokens.postDelimiter = postDelimiter
    rest = rest.slice(postDelimiter.length)
  }

  if (rest.endsWith('*/')) {
    tokens.end = '*/'
    rest = rest.slice(0, -2)
  }

  tokens.description = rest
  applyTagTokens(tokens)

  return {
    number,
    source: tokensToSource(tokens),
    tokens,
    startOffset: line.startOffset,
    endOffset: line.endOffset
  }
}

function applyMultilineTypeTokens(entries: SourceEntry[]): void {
  let depth = 0
  for (const entry of entries) {
    const { tokens } = entry
    if (depth === 0) {
      if (!tokens.tag) {
        continue
      }
      const typeText = tokens.type || tokens.description
      if (!typeText.startsWith('{')) {
        continue
      }
      const typeState = braceDepthDelta(typeText)
      if (typeState.closeIndex !== -1) {
        continue
      }
      tokens.type = typeText
      tokens.description = ''
      depth = typeState.depth
      entry.source = tokensToSource(tokens)
      continue
    }

    let text = tokens.description
    if (tokens.delimiter && tokens.postDelimiter.length > 1) {
      text = tokens.postDelimiter.slice(1) + text
      tokens.postDelimiter = tokens.postDelimiter[0] ?? ''
    }
    const typeState = braceDepthDelta(text, depth)
    if (typeState.closeIndex === -1) {
      tokens.type = text
      tokens.description = ''
      depth = typeState.depth
      entry.source = tokensToSource(tokens)
      continue
    }

    tokens.type = text.slice(0, typeState.closeIndex + 1)
    const rest = text.slice(typeState.closeIndex + 1)
    const postType = rest.match(/^\s*/u)?.[0] ?? ''
    tokens.postType = postType
    const afterType = rest.slice(postType.length)
    const parsedName = splitName(afterType)
    tokens.name = parsedName.name
    tokens.postName = parsedName.postName
    tokens.description = parsedName.rest
    depth = 0
    entry.source = tokensToSource(tokens)
  }
}

function stripSourceEntryMeta(entry: SourceEntry, number = entry.number): PublicSourceEntry {
  return {
    number,
    source: entry.source,
    tokens: entry.tokens
  }
}

/**
 * Minimal interface needed by the source[] reconstruction helpers — every
 * lazy class that owns a `range` / `parent` / `rootIndex` / `sourceFile`
 * tuple satisfies it. `parent` is the generic `LazyNode | null` (we only
 * need `.range` from it, which every LazyNode exposes).
 */
interface SourceLikeNode {
  readonly range: readonly [number, number]
  readonly parent: LazyNode | null
  readonly rootIndex: number
  readonly sourceFile: RemoteSourceFileLike
}

function buildSourceEntriesForNode(node: SourceLikeNode): SourceEntry[] {
  const baseOffset = node.sourceFile.getRootBaseOffset(node.rootIndex)
  // Use the parent's range when available (the parent is the JsdocBlock
  // root for tag-side reconstruction); otherwise fall back to the node's
  // own range. Only `.range` is consumed so we don't need the parent to
  // be a SourceLikeNode itself.
  const [rootStart, rootEnd] = node.parent !== null ? node.parent.range : node.range
  const sourceText =
    node.sourceFile.sliceSourceText(node.rootIndex, rootStart - baseOffset, rootEnd - baseOffset) ??
    ''

  const entries = splitPhysicalLines(sourceText, rootStart).map((line, number) =>
    lineToSourceEntry(line, number)
  )
  applyMultilineTypeTokens(entries)
  return entries
}

function buildBlockSource(block: SourceLikeNode): PublicSourceEntry[] {
  return buildSourceEntriesForNode(block).map(entry => stripSourceEntryMeta(entry))
}

function buildTagSourceFromRoot(tag: SourceLikeNode): PublicSourceEntry[] {
  const entries = buildSourceEntriesForNode(tag)
  const [tagStart, tagEnd] = tag.range
  return entries
    .filter(entry => entry.endOffset > tagStart && entry.startOffset < tagEnd)
    .map(entry => stripSourceEntryMeta(entry))
}

function stripTypeBraces(type: string): string {
  return type.replace(/^\{/u, '').replace(/\}$/u, '')
}

function compactDescriptionFromEntries(entries: PublicSourceEntry[]): string {
  return entries
    .filter(entry => entry.tokens.tag === '')
    .map(entry => entry.tokens.description)
    .map(description => description.replace(/^\s*/u, ''))
    .filter(Boolean)
    .join(' ')
}

// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------

/** Read the 6-bit Common Data byte for a node. */
function commonData(internal: RemoteInternal): number {
  return internal.view.getUint8(internal.byteIndex + COMMON_DATA_OFFSET) & COMMON_DATA_MASK
}

/**
 * `JsdocInlineTagFormat` numeric → string label.
 * Mirrors Rust's `JsdocInlineTagFormat` enum order.
 */
const INLINE_TAG_FORMATS = ['plain', 'pipe', 'space', 'prefix', 'unknown'] as const
type InlineTagFormat = (typeof INLINE_TAG_FORMATS)[number]

/**
 * Per-class internal cache shape. Each Remote* class extends this with
 * `$<getter>` slots for its own lazily-computed values. Defined as the
 * intersection between the immutable wiring pieces (view/byteIndex/...)
 * and an open-ended record of cache slots so subclasses can add fields
 * without re-declaring the wiring fields each time.
 */
type LazyInternal<TCache> = RemoteInternal & TCache

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

interface JsdocBlockCache {
  $range: readonly [number, number] | undefined
  $description: string | null | undefined
  $delimiter: string | undefined
  $postDelimiter: string | undefined
  $terminal: string | undefined
  $lineEnd: string | undefined
  $initial: string | undefined
  $delimiterLineBreak: string | undefined
  $preterminalLineBreak: string | undefined
  $descriptionLines: RemoteNodeList | undefined
  $tags: RemoteNodeList | undefined
  $inlineTags: RemoteNodeList | undefined
}

/**
 * `JsdocBlock` (Kind 0x01) — root of every parsed `/** ... *​/` comment.
 */
export class RemoteJsdocBlock implements LazyNode, SourceLikeNode {
  readonly type = 'JsdocBlock'
  readonly #internal: LazyInternal<JsdocBlockCache>

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
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

  get range(): readonly [number, number] {
    const internal = this.#internal
    return internal.$range !== undefined
      ? internal.$range
      : (internal.$range = absoluteRange(internal))
  }
  get parent(): LazyNode | null {
    return this.#internal.parent
  }
  get sourceFile(): RemoteSourceFileLike {
    return this.#internal.sourceFile
  }
  get rootIndex(): number {
    return this.#internal.rootIndex
  }

  /** Top-level description string (`null` when absent). The
   * `emptyStringForNull` option only affects `toJSON()` output. */
  get description(): string | null {
    const internal = this.#internal
    const cached = internal.$description
    if (cached !== undefined) {
      return cached
    }
    return (internal.$description = extStringField(internal, 2))
  }
  /** Source-preserving `*` line-prefix delimiter. */
  get delimiter(): string {
    const internal = this.#internal
    const cached = internal.$delimiter
    if (cached !== undefined) {
      return cached
    }
    return (internal.$delimiter = extStringFieldRequired(internal, 8))
  }
  /** Source-preserving space after `*`. */
  get postDelimiter(): string {
    const internal = this.#internal
    const cached = internal.$postDelimiter
    if (cached !== undefined) {
      return cached
    }
    return (internal.$postDelimiter = extStringFieldRequired(internal, 14))
  }
  /** Source-preserving `*​/` terminal. */
  get terminal(): string {
    const internal = this.#internal
    const cached = internal.$terminal
    if (cached !== undefined) {
      return cached
    }
    return (internal.$terminal = extStringFieldRequired(internal, 20))
  }
  /** Source-preserving line-end characters. */
  get lineEnd(): string {
    const internal = this.#internal
    const cached = internal.$lineEnd
    if (cached !== undefined) {
      return cached
    }
    return (internal.$lineEnd = extStringFieldRequired(internal, 26))
  }
  /** Indentation before the leading `*`. */
  get initial(): string {
    const internal = this.#internal
    const cached = internal.$initial
    if (cached !== undefined) {
      return cached
    }
    return (internal.$initial = extStringFieldRequired(internal, 32))
  }
  /** Line-break right after `/**`. */
  get delimiterLineBreak(): string {
    const internal = this.#internal
    const cached = internal.$delimiterLineBreak
    if (cached !== undefined) {
      return cached
    }
    return (internal.$delimiterLineBreak = extStringFieldRequired(internal, 38))
  }
  /** Line-break right before `*​/`. */
  get preterminalLineBreak(): string {
    const internal = this.#internal
    const cached = internal.$preterminalLineBreak
    if (cached !== undefined) {
      return cached
    }
    return (internal.$preterminalLineBreak = extStringFieldRequired(internal, 44))
  }

  // -- compat-only line metadata -------------------------------------------
  // The following 6 fields exist only when `sourceFile.compatMode` is true
  // (basic-mode ED records stop at byte 68; reading further would walk into
  // the next node's bytes). Each getter returns `null` outside compat mode.

  /** Total number of LogicalLines in this comment (compat-mode only). */
  get endLine(): number | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extU32(internal, JSDOC_BLOCK_END_LINE_OFFSET)
  }
  /** Index of the first description line, or `null` when absent. */
  get descriptionStartLine(): number | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    const v = extU32(internal, JSDOC_BLOCK_DESC_START_LINE_OFFSET)
    return v === COMPAT_LINE_NONE ? null : v
  }
  /** Index of the last description line, or `null` when absent. */
  get descriptionEndLine(): number | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    const v = extU32(internal, JSDOC_BLOCK_DESC_END_LINE_OFFSET)
    return v === COMPAT_LINE_NONE ? null : v
  }
  /** Description-boundary index (jsdoccomment's `lastDescriptionLine` —
   * actually the index of the first tag/end line). `null` when absent. */
  get lastDescriptionLine(): number | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    const v = extU32(internal, JSDOC_BLOCK_LAST_DESC_LINE_OFFSET)
    return v === COMPAT_LINE_NONE ? null : v
  }
  /** `1` when block description text exists on the `*​/` line. */
  get hasPreterminalDescription(): number | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extU8(internal, JSDOC_BLOCK_HAS_PRETERMINAL_DESC_OFFSET)
  }
  /** `1` when tag description text exists on the `*​/` line; `null` when not
   * applicable (no active lastTag at end). */
  get hasPreterminalTagDescription(): number | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    const v = extU8(internal, JSDOC_BLOCK_HAS_PRETERMINAL_TAG_DESC_OFFSET)
    return v === COMPAT_U8_NONE ? null : v
  }

  /**
   * Raw description slice (with `*` prefix and blank lines intact).
   * Returns `null` when the buffer was not parsed with
   * `preserveWhitespace: true` (the per-node
   * `has_description_raw_span` Common Data bit is clear), or when the
   * block has no description.
   *
   * Phase 5 layout: the span sits at the **last 8 bytes** of the ED
   * record (offset = `compatMode ? 90 : 68` = the basic / compat ED size).
   * See `design/008-oxlint-oxfmt-support/README.md` §4.2 / §4.3.
   */
  get descriptionRaw(): string | null {
    const internal = this.#internal
    if ((commonData(internal) & JSDOC_BLOCK_HAS_DESCRIPTION_RAW_SPAN_BIT) === 0) {
      return null
    }
    const spanOff = internal.sourceFile.compatMode
      ? JSDOC_BLOCK_COMPAT_SIZE_CONST
      : JSDOC_BLOCK_BASIC_SIZE_CONST
    const start = extU32(internal, spanOff)
    const end = extU32(internal, spanOff + 4)
    return internal.sourceFile.sliceSourceText(internal.rootIndex, start, end)
  }

  /**
   * Description text. When `preserveWhitespace` is `true`, blank lines
   * and indentation past the `* ` prefix are preserved (algorithm: see
   * `parsedPreservingWhitespace` / design §3). When `false` or omitted,
   * returns the compact view (`description` getter).
   *
   * Returns `null` when no description is present, or when
   * `preserveWhitespace=true` is requested on a buffer that wasn't
   * parsed with the matching `preserveWhitespace: true` parse option.
   */
  descriptionText(preserveWhitespace?: boolean): string | null {
    if (preserveWhitespace) {
      const raw = this.descriptionRaw
      return raw === null ? null : parsedPreservingWhitespace(raw)
    }
    return this.description
  }

  /** Top-level description lines. */
  get descriptionLines(): RemoteNodeList {
    const internal = this.#internal
    const cached = internal.$descriptionLines
    if (cached !== undefined) {
      return cached
    }
    return (internal.$descriptionLines = nodeListAtSlotExtended(
      internal,
      JSDOC_BLOCK_DESC_LINES_SLOT
    ))
  }
  /** Block tags. */
  get tags(): RemoteNodeList {
    const internal = this.#internal
    const cached = internal.$tags
    if (cached !== undefined) {
      return cached
    }
    return (internal.$tags = nodeListAtSlotExtended(internal, JSDOC_BLOCK_TAGS_SLOT))
  }
  /** Inline tags found inside the top-level description. */
  get inlineTags(): RemoteNodeList {
    const internal = this.#internal
    const cached = internal.$inlineTags
    if (cached !== undefined) {
      return cached
    }
    return (internal.$inlineTags = nodeListAtSlotExtended(internal, JSDOC_BLOCK_INLINE_TAGS_SLOT))
  }

  toJSON(): RemoteJsonObject {
    const internal = this.#internal
    const nullToEmpty = internal.sourceFile.emptyStringForNull
    const json: RemoteJsonObject = {
      type: this.type,
      range: [...this.range],
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
      const source = buildBlockSource(this)
      const sourceDescription = compactDescriptionFromEntries(source)
      if (sourceDescription) {
        json.description = sourceDescription
      }
      // Match jsdoccomment's optional-field serialization: omit absent
      // line indices (Rust serializer uses `skip_serializing_if = "Option::is_none"`).
      json.endLine = this.endLine
      const dsl = this.descriptionStartLine
      if (dsl !== null) {
        json.descriptionStartLine = dsl
      }
      const del = this.descriptionEndLine
      if (del !== null) {
        json.descriptionEndLine = del
      }
      const ldl = this.lastDescriptionLine
      if (ldl !== null) {
        json.lastDescriptionLine = ldl
      }
      json.hasPreterminalDescription = this.hasPreterminalDescription
      const hptd = this.hasPreterminalTagDescription
      if (hptd !== null) {
        json.hasPreterminalTagDescription = hptd
      }
      // descriptionRaw — design 008 §4.4: emit only when present (parity
      // with the Rust JSON serializer's `skip_serializing_if = "Option::is_none"`).
      const raw = this.descriptionRaw
      if (raw !== null) {
        json.descriptionRaw = raw
      }
      // Phase 6: comment-parser-shape source[] array. Cast each entry to
      // RemoteJsonObject (LineTokens is a fixed string-record, JSON-safe).
      json.source = source as unknown as RemoteJsonObject[]
      const tagsArr = json.tags as RemoteJsonObject[]
      json.tags = tagsArr.filter(tag => {
        const tagSource = tag.source as RemoteJsonObject[] | undefined
        const firstTokens = tagSource?.[0]?.tokens as LineTokens | undefined
        return Boolean(firstTokens?.tag)
      })
    }
    return json
  }

  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'JsdocBlock')
  }
}

// ===========================================================================
// 0x02 RemoteJsdocDescriptionLine
//
// Extended Data layout: byte 0-5 description (always required); compat
// adds 3 × 6-byte StringField slots after it.
// ===========================================================================

interface JsdocDescriptionLineCache {
  $range: readonly [number, number] | undefined
  $description: string | undefined
}

/**
 * `JsdocDescriptionLine` (Kind 0x02). Both basic and compat modes store
 * `description` as the leading StringField of the Extended Data record.
 */
export class RemoteJsdocDescriptionLine implements LazyNode, SourceLikeNode {
  readonly type = 'JsdocDescriptionLine'
  readonly #internal: LazyInternal<JsdocDescriptionLineCache>

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
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

  get range(): readonly [number, number] {
    const internal = this.#internal
    return internal.$range !== undefined
      ? internal.$range
      : (internal.$range = absoluteRange(internal))
  }
  get parent(): LazyNode | null {
    return this.#internal.parent
  }
  get sourceFile(): RemoteSourceFileLike {
    return this.#internal.sourceFile
  }
  get rootIndex(): number {
    return this.#internal.rootIndex
  }

  /** Description content. Basic mode reads the String payload (Node Data);
   * compat mode reads byte 0-5 of the Extended Data record. */
  get description(): string {
    const internal = this.#internal
    const cached = internal.$description
    if (cached !== undefined) {
      return cached
    }
    const value = internal.sourceFile.compatMode
      ? extStringFieldRequired(internal, 0)
      : (stringPayloadOf(internal) ?? '')
    return (internal.$description = value)
  }

  // -- compat-only delimiter trio ------------------------------------------
  // Stored in ED bytes 6-23 only when sourceFile.compatMode is true.

  /** Source-preserving `*` line-prefix (compat-mode only; `null` otherwise). */
  get delimiter(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringField(internal, COMPAT_LINE_DELIMITER)
  }
  /** Source-preserving space after `*` (compat-mode only). */
  get postDelimiter(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringField(internal, COMPAT_LINE_POST_DELIMITER)
  }
  /** Indentation before the leading `*` (compat-mode only). */
  get initial(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringField(internal, COMPAT_LINE_INITIAL)
  }

  toJSON(): RemoteJsonObject {
    const json: RemoteJsonObject = {
      type: this.type,
      range: [...this.range],
      description: this.description
    }
    if (this.#internal.sourceFile.compatMode) {
      json.delimiter = this.delimiter ?? ''
      json.postDelimiter = this.postDelimiter ?? ''
      json.initial = this.initial ?? ''
    }
    return json
  }
  [inspectSymbol](): object {
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

interface JsdocTagCache {
  $range: readonly [number, number] | undefined
  $defaultValue: string | null | undefined
  $description: string | null | undefined
  $rawBody: string | null | undefined
  $tag: LazyNode | null | undefined
  $rawType: LazyNode | null | undefined
  $name: LazyNode | null | undefined
  $parsedType: LazyNode | null | undefined
  $body: LazyNode | null | undefined
  $typeLines: RemoteNodeList | undefined
  $descriptionLines: RemoteNodeList | undefined
  $inlineTags: RemoteNodeList | undefined
}

/**
 * `JsdocTag` (Kind 0x03) — one block tag (e.g. `@param`).
 */
export class RemoteJsdocTag implements LazyNode, SourceLikeNode {
  readonly type = 'JsdocTag'
  readonly #internal: LazyInternal<JsdocTagCache>

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
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

  get range(): readonly [number, number] {
    const internal = this.#internal
    return internal.$range !== undefined
      ? internal.$range
      : (internal.$range = absoluteRange(internal))
  }
  get parent(): LazyNode | null {
    return this.#internal.parent
  }
  get sourceFile(): RemoteSourceFileLike {
    return this.#internal.sourceFile
  }
  get rootIndex(): number {
    return this.#internal.rootIndex
  }

  /** `bit0` of Common Data — was the tag wrapped in `[...]`? */
  get optional(): boolean {
    return (commonData(this.#internal) & 0b0000_0001) !== 0
  }
  /** Default value parsed from `[id=foo]` syntax. */
  get defaultValue(): string | null {
    const internal = this.#internal
    const cached = internal.$defaultValue
    if (cached !== undefined) {
      return cached
    }
    return (internal.$defaultValue = extStringField(internal, 2))
  }
  /** Joined description text. */
  get description(): string | null {
    const internal = this.#internal
    const cached = internal.$description
    if (cached !== undefined) {
      return cached
    }
    return (internal.$description = extStringField(internal, 8))
  }
  /**
   * Raw description slice (with `*` prefix and blank lines intact).
   * Returns `null` when the buffer was not parsed with
   * `preserveWhitespace: true` (the per-node
   * `has_description_raw_span` Common Data bit is clear), or when the
   * tag has no description.
   *
   * Phase 5 layout: the span sits at the **last 8 bytes** of the ED
   * record (offset = `compatMode ? 80 : 38` = the basic / compat ED size).
   * See `design/008-oxlint-oxfmt-support/README.md` §4.2 / §4.3.
   */
  get descriptionRaw(): string | null {
    const internal = this.#internal
    if ((commonData(internal) & JSDOC_TAG_HAS_DESCRIPTION_RAW_SPAN_BIT) === 0) {
      return null
    }
    const spanOff = internal.sourceFile.compatMode
      ? JSDOC_TAG_COMPAT_SIZE_CONST
      : JSDOC_TAG_BASIC_SIZE_CONST
    const start = extU32(internal, spanOff)
    const end = extU32(internal, spanOff + 4)
    return internal.sourceFile.sliceSourceText(internal.rootIndex, start, end)
  }
  /**
   * Description text. Identical contract to
   * `RemoteJsdocBlock.descriptionText`.
   */
  descriptionText(preserveWhitespace?: boolean): string | null {
    if (preserveWhitespace) {
      const raw = this.descriptionRaw
      return raw === null ? null : parsedPreservingWhitespace(raw)
    }
    return this.description
  }
  /** Raw body when the tag uses the `Raw` body variant. */
  get rawBody(): string | null {
    const internal = this.#internal
    const cached = internal.$rawBody
    if (cached !== undefined) {
      return cached
    }
    return (internal.$rawBody = extStringField(internal, 14))
  }

  /** Mandatory tag-name child (visitor index 0 — the `@name` token). */
  get tag(): LazyNode | null {
    const internal = this.#internal
    const cached = internal.$tag
    if (cached !== undefined) {
      return cached
    }
    return (internal.$tag = childNodeAtVisitorIndex(internal, 0))
  }
  /** Raw `{...}` type source (visitor index 1). */
  get rawType(): LazyNode | null {
    const internal = this.#internal
    const cached = internal.$rawType
    if (cached !== undefined) {
      return cached
    }
    return (internal.$rawType = childNodeAtVisitorIndex(internal, 1))
  }
  /** Tag-name value (visitor index 2). */
  get name(): LazyNode | null {
    const internal = this.#internal
    const cached = internal.$name
    if (cached !== undefined) {
      return cached
    }
    return (internal.$name = childNodeAtVisitorIndex(internal, 2))
  }
  /** `parsedType` child (visitor index 3) — any TypeNode variant. */
  get parsedType(): LazyNode | null {
    const internal = this.#internal
    const cached = internal.$parsedType
    if (cached !== undefined) {
      return cached
    }
    return (internal.$parsedType = childNodeAtVisitorIndex(internal, 3))
  }
  /** Body child (visitor index 4) — Generic / Borrows / Raw variant. */
  get body(): LazyNode | null {
    const internal = this.#internal
    const cached = internal.$body
    if (cached !== undefined) {
      return cached
    }
    return (internal.$body = childNodeAtVisitorIndex(internal, 4))
  }
  /** Source-preserving type lines. */
  get typeLines(): RemoteNodeList {
    const internal = this.#internal
    const cached = internal.$typeLines
    if (cached !== undefined) {
      return cached
    }
    return (internal.$typeLines = nodeListAtSlotExtended(internal, JSDOC_TAG_TYPE_LINES_SLOT))
  }
  /** Source-preserving description lines. */
  get descriptionLines(): RemoteNodeList {
    const internal = this.#internal
    const cached = internal.$descriptionLines
    if (cached !== undefined) {
      return cached
    }
    return (internal.$descriptionLines = nodeListAtSlotExtended(
      internal,
      JSDOC_TAG_DESC_LINES_SLOT
    ))
  }
  /** Inline tags found in this tag's description. */
  get inlineTags(): RemoteNodeList {
    const internal = this.#internal
    const cached = internal.$inlineTags
    if (cached !== undefined) {
      return cached
    }
    return (internal.$inlineTags = nodeListAtSlotExtended(internal, JSDOC_TAG_INLINE_TAGS_SLOT))
  }

  // -- compat-only delimiter strings ---------------------------------------
  // Stored in ED bytes 38-79 (7 StringFields) only when compatMode is true.

  /** Source-preserving `*` line-prefix (compat-mode only). */
  get delimiter(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_DELIMITER)
  }
  /** Source-preserving space after `*` (compat-mode only). */
  get postDelimiter(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_POST_DELIMITER)
  }
  /** Whitespace after the `@name` token (compat-mode only). */
  get postTag(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_POST_TAG)
  }
  /** Whitespace after the `{type}` source (compat-mode only). */
  get postType(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_POST_TYPE)
  }
  /** Whitespace after the name token (compat-mode only). */
  get postName(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_POST_NAME)
  }
  /** Indentation before the line's `*` (compat-mode only). */
  get initial(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_INITIAL)
  }
  /** Line ending of the tag's first line (compat-mode only). */
  get lineEnd(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringFieldRequired(internal, JSDOC_TAG_COMPAT_LINE_END)
  }

  toJSON(): RemoteJsonObject {
    const internal = this.#internal
    const compat = internal.sourceFile.compatMode
    const nullToEmpty = internal.sourceFile.emptyStringForNull

    if (compat) {
      // jsdoccomment-shape: omit ox-jsdoc-specific fields (optional,
      // defaultValue, rawBody, body) and surface delimiter strings + lineEnd.
      const source = buildTagSourceFromRoot(this)
      const headTokens: LineTokens = source[0]?.tokens ?? emptyTokens()
      const tagNode = this.tag?.toJSON() ?? null
      const tagName = (tagNode?.value as string | undefined) ?? ''
      const rawTypeNode = this.rawType?.toJSON() ?? null
      const hasSource = source.length > 0
      const rawTypeRaw: string | null = hasSource
        ? stripTypeBraces(headTokens.type)
        : ((rawTypeNode?.raw as string | undefined) ?? null)
      const nameNode = this.name?.toJSON() ?? null
      const nameValue: string | null = hasSource
        ? headTokens.name
        : ((nameNode?.raw as string | undefined) ?? null)
      const description = hasSource ? headTokens.description : this.description
      const json: RemoteJsonObject = {
        type: this.type,
        range: [...this.range],
        tag: tagName,
        rawType: nullToEmpty ? (rawTypeRaw ?? '') : rawTypeRaw,
        name: nullToEmpty ? (nameValue ?? '') : nameValue,
        description: description ?? (nullToEmpty ? '' : null),
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
        // Phase 6: per-tag source[] subset (the physical lines that
        // belong to this tag in the original comment).
        source: source as unknown as RemoteJsonObject[]
      }
      // descriptionRaw — design 008 §4.4: emit only when present.
      const raw = this.descriptionRaw
      if (raw !== null) {
        json.descriptionRaw = raw
      }
      return json
    }
    return {
      type: this.type,
      range: [...this.range],
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
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'JsdocTag')
  }
}

// ===========================================================================
// 0x04-0x06, 0x0B, 0x0D-0x0F: Extended-type string-leaf nodes
// (each carries a single 6-byte StringField in Extended Data)
// ===========================================================================

interface StringLeafCache {
  $range: readonly [number, number] | undefined
  $value: string | undefined
}

/**
 * Build a class for a single-string-leaf node. Captures `accessorName` so
 * the resolved value is exposed under the right property name (`value`,
 * `raw`, or `name`) per the Rust enum's variant.
 */
function defineStringLeaf(typeName: string, accessorName: string): LazyNodeConstructor {
  return class implements LazyNode {
    readonly type = typeName
    readonly _internal: LazyInternal<StringLeafCache>;
    // Index signature lets the dynamic accessor (`this[accessorName]`)
    // type-check while keeping consumers free to use `.value` / `.raw` /
    // `.name` per the public d.ts.
    [key: string]: unknown

    constructor(
      view: DataView,
      byteIndex: number,
      index: number,
      rootIndex: number,
      parent: LazyNode | null,
      sourceFile: RemoteSourceFileLike
    ) {
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
      // Define the dynamic accessor on the instance: keeps the name
      // closure-bound while preserving the V8 hidden class shape.
      const internal = this._internal
      Object.defineProperty(this, accessorName, {
        get() {
          const cached = internal.$value
          if (cached !== undefined) {
            return cached
          }
          return (internal.$value = stringPayloadOf(internal) ?? '')
        },
        enumerable: true,
        configurable: false
      })
    }
    get range(): readonly [number, number] {
      const internal = this._internal
      return internal.$range !== undefined
        ? internal.$range
        : (internal.$range = absoluteRange(internal))
    }
    get parent(): LazyNode | null {
      return this._internal.parent
    }
    toJSON(): RemoteJsonObject {
      const value: RemoteJsonValue = (this[accessorName] as string | undefined) ?? ''
      return {
        type: this.type,
        range: [...this.range],
        [accessorName]: value
      }
    }
    [inspectSymbol](): object {
      return inspectPayload(this.toJSON(), typeName)
    }
  }
}

// The interface declarations below intentionally merge with their classes to
// surface the dynamic accessor (`Object.defineProperty(this, accessorName, ...)`
// inside `defineStringLeaf`'s constructor) on the public type. This pattern
// keeps the per-Kind accessor name (`value`/`raw`/`name`) distinct while
// sharing one runtime class body — declaration merging is the only way to
// reflect that on the type side.

/** `JsdocTagName` (Kind 0x04) — the `@name` token text. */
// eslint-disable-next-line typescript-eslint/no-unsafe-declaration-merging -- dynamic accessor; see comment above
export class RemoteJsdocTagName extends defineStringLeaf('JsdocTagName', 'value') {}
export interface RemoteJsdocTagName {
  readonly value: string
}
/** `JsdocTagNameValue` (Kind 0x05) — value after the type in `@param`. */
// eslint-disable-next-line typescript-eslint/no-unsafe-declaration-merging -- dynamic accessor; see comment above
export class RemoteJsdocTagNameValue extends defineStringLeaf('JsdocTagNameValue', 'raw') {}
export interface RemoteJsdocTagNameValue {
  readonly raw: string
}
/** `JsdocTypeSource` (Kind 0x06) — raw `{...}` text inside a tag. */
// eslint-disable-next-line typescript-eslint/no-unsafe-declaration-merging -- dynamic accessor; see comment above
export class RemoteJsdocTypeSource extends defineStringLeaf('JsdocTypeSource', 'raw') {}
export interface RemoteJsdocTypeSource {
  readonly raw: string
}
/** `JsdocRawTagBody` (Kind 0x0B) — raw text body fallback. */
// eslint-disable-next-line typescript-eslint/no-unsafe-declaration-merging -- dynamic accessor; see comment above
export class RemoteJsdocRawTagBody extends defineStringLeaf('JsdocRawTagBody', 'raw') {}
export interface RemoteJsdocRawTagBody {
  readonly raw: string
}
/** `JsdocNamepathSource` (Kind 0x0D) — namepath token. */
// eslint-disable-next-line typescript-eslint/no-unsafe-declaration-merging -- dynamic accessor; see comment above
export class RemoteJsdocNamepathSource extends defineStringLeaf('JsdocNamepathSource', 'raw') {}
export interface RemoteJsdocNamepathSource {
  readonly raw: string
}
/** `JsdocIdentifier` (Kind 0x0E) — bare identifier. */
// eslint-disable-next-line typescript-eslint/no-unsafe-declaration-merging -- dynamic accessor; see comment above
export class RemoteJsdocIdentifier extends defineStringLeaf('JsdocIdentifier', 'name') {}
export interface RemoteJsdocIdentifier {
  readonly name: string
}
/** `JsdocText` (Kind 0x0F) — raw text. */
// eslint-disable-next-line typescript-eslint/no-unsafe-declaration-merging -- dynamic accessor; see comment above
export class RemoteJsdocText extends defineStringLeaf('JsdocText', 'value') {}
export interface RemoteJsdocText {
  readonly value: string
}

// ===========================================================================
// 0x07 RemoteJsdocTypeLine
//
// Extended Data layout: byte 0-5 raw_type (always required); compat adds
// 3 × 6-byte StringField slots after it.
// ===========================================================================

interface JsdocTypeLineCache {
  $range: readonly [number, number] | undefined
  $rawType: string | undefined
}

/**
 * `JsdocTypeLine` (Kind 0x07).
 */
export class RemoteJsdocTypeLine implements LazyNode {
  readonly type = 'JsdocTypeLine'
  readonly #internal: LazyInternal<JsdocTypeLineCache>

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
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

  get range(): readonly [number, number] {
    const internal = this.#internal
    return internal.$range !== undefined
      ? internal.$range
      : (internal.$range = absoluteRange(internal))
  }
  get parent(): LazyNode | null {
    return this.#internal.parent
  }

  /** Raw `{...}` line content. Basic mode reads the String payload;
   * compat mode reads byte 0-5 of the Extended Data record. */
  get rawType(): string {
    const internal = this.#internal
    const cached = internal.$rawType
    if (cached !== undefined) {
      return cached
    }
    const value = internal.sourceFile.compatMode
      ? extStringFieldRequired(internal, 0)
      : (stringPayloadOf(internal) ?? '')
    return (internal.$rawType = value)
  }

  // -- compat-only delimiter trio (same layout as JsdocDescriptionLine) ----

  /** Source-preserving `*` line-prefix (compat-mode only; `null` otherwise). */
  get delimiter(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringField(internal, COMPAT_LINE_DELIMITER)
  }
  /** Source-preserving space after `*` (compat-mode only). */
  get postDelimiter(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringField(internal, COMPAT_LINE_POST_DELIMITER)
  }
  /** Indentation before the leading `*` (compat-mode only). */
  get initial(): string | null {
    const internal = this.#internal
    if (!internal.sourceFile.compatMode) {
      return null
    }
    return extStringField(internal, COMPAT_LINE_INITIAL)
  }

  toJSON(): RemoteJsonObject {
    const json: RemoteJsonObject = {
      type: this.type,
      range: [...this.range],
      rawType: this.rawType
    }
    if (this.#internal.sourceFile.compatMode) {
      json.delimiter = this.delimiter ?? ''
      json.postDelimiter = this.postDelimiter ?? ''
      json.initial = this.initial ?? ''
    }
    return json
  }
  [inspectSymbol](): object {
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

interface JsdocInlineTagCache {
  $range: readonly [number, number] | undefined
  $namepathOrURL: string | null | undefined
  $text: string | null | undefined
  $rawBody: string | null | undefined
}

/**
 * `JsdocInlineTag` (Kind 0x08) — e.g. `{@link Foo}`.
 */
export class RemoteJsdocInlineTag implements LazyNode {
  readonly type = 'JsdocInlineTag'
  readonly #internal: LazyInternal<JsdocInlineTagCache>

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
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

  get range(): readonly [number, number] {
    const internal = this.#internal
    return internal.$range !== undefined
      ? internal.$range
      : (internal.$range = absoluteRange(internal))
  }
  get parent(): LazyNode | null {
    return this.#internal.parent
  }

  /** Inline tag format string. In compat mode the `'unknown'` variant is
   * mapped to `'plain'` to mirror jsdoccomment's behavior. */
  get format(): InlineTagFormat {
    const internal = this.#internal
    const raw = INLINE_TAG_FORMATS[commonData(internal) & 0b0000_0111] ?? 'unknown'
    return raw === 'unknown' && internal.sourceFile.compatMode ? 'plain' : raw
  }
  /** Optional name path or URL portion. */
  get namepathOrURL(): string | null {
    const internal = this.#internal
    const cached = internal.$namepathOrURL
    if (cached !== undefined) {
      return cached
    }
    return (internal.$namepathOrURL = extStringField(internal, 0))
  }
  /** Optional display text portion. */
  get text(): string | null {
    const internal = this.#internal
    const cached = internal.$text
    if (cached !== undefined) {
      return cached
    }
    return (internal.$text = extStringField(internal, STRING_FIELD_SIZE))
  }
  /** Raw body text fallback. */
  get rawBody(): string | null {
    const internal = this.#internal
    const cached = internal.$rawBody
    if (cached !== undefined) {
      return cached
    }
    return (internal.$rawBody = extStringField(internal, 2 * STRING_FIELD_SIZE))
  }

  toJSON(): RemoteJsonObject {
    const internal = this.#internal
    const compat = internal.sourceFile.compatMode
    const nullToEmpty = internal.sourceFile.emptyStringForNull
    // NOTE(jsdoccomment): `JsdocInlineTag` exposes a `tag` field (the inline
    // tag name without `@`, e.g. "link"). The binary writer does not currently
    // serialize this value (`emit_inline_tag` discards `inline.tag_name`), so
    // we cannot reproduce it here. Track as a future binary-format extension.
    const json: RemoteJsonObject = {
      type: this.type,
      range: [...this.range],
      format: this.format,
      namepathOrURL: nullToEmpty ? (this.namepathOrURL ?? '') : this.namepathOrURL,
      text: nullToEmpty ? (this.text ?? '') : this.text
    }
    // jsdoccomment excludes rawBody from inline-tag output.
    if (!compat) {
      json.rawBody = this.rawBody
    }
    return json
  }
  [inspectSymbol](): object {
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
export class RemoteJsdocGenericTagBody implements LazyNode {
  readonly type = 'JsdocGenericTagBody'
  readonly #internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this.#internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }

  get range(): readonly [number, number] {
    return absoluteRange(this.#internal)
  }
  get parent(): LazyNode | null {
    return this.#internal.parent
  }

  /** `true` when the tag separator was `-`. */
  get hasDashSeparator(): boolean {
    return (commonData(this.#internal) & 0b0000_0001) !== 0
  }
  /** Description text after the dash separator. */
  get description(): string | null {
    return extStringField(this.#internal, 2)
  }

  toJSON(): RemoteJsonObject {
    return {
      type: this.type,
      range: [...this.range],
      hasDashSeparator: this.hasDashSeparator,
      description: this.description
    }
  }
  [inspectSymbol](): object {
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
export class RemoteJsdocBorrowsTagBody implements LazyNode {
  readonly type = 'JsdocBorrowsTagBody'
  readonly #internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this.#internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }

  get range(): readonly [number, number] {
    return absoluteRange(this.#internal)
  }
  get parent(): LazyNode | null {
    return this.#internal.parent
  }

  toJSON(): RemoteJsonObject {
    return { type: this.type, range: [...this.range] }
  }
  [inspectSymbol](): object {
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
export class RemoteJsdocParameterName implements LazyNode {
  readonly type = 'JsdocParameterName'
  readonly #internal: RemoteInternal

  constructor(
    view: DataView,
    byteIndex: number,
    index: number,
    rootIndex: number,
    parent: LazyNode | null,
    sourceFile: RemoteSourceFileLike
  ) {
    this.#internal = { view, byteIndex, index, rootIndex, parent, sourceFile }
  }

  get range(): readonly [number, number] {
    return absoluteRange(this.#internal)
  }
  get parent(): LazyNode | null {
    return this.#internal.parent
  }

  /** `true` when the parameter was wrapped in `[id]` brackets. */
  get optional(): boolean {
    return (commonData(this.#internal) & 0b0000_0001) !== 0
  }
  /** Path text. */
  get path(): string {
    return extStringFieldRequired(this.#internal, 0)
  }
  /** Default value parsed from `[id=foo]` syntax. */
  get defaultValue(): string | null {
    return extStringField(this.#internal, STRING_FIELD_SIZE)
  }

  toJSON(): RemoteJsonObject {
    return {
      type: this.type,
      range: [...this.range],
      optional: this.optional,
      path: this.path,
      defaultValue: this.defaultValue
    }
  }
  [inspectSymbol](): object {
    return inspectPayload(this.toJSON(), 'JsdocParameterName')
  }
}
