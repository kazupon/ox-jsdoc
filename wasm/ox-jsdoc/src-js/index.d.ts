/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

export interface ParseOptions {
  /** Suppress tag recognition inside fenced code blocks. Default: true. */
  fenceAware?: boolean
  /** Enable type expression parsing for `{...}` in tags. Default: false. */
  parseTypes?: boolean
  /** Parse mode for type expressions: "jsdoc", "closure", or "typescript". Default: "jsdoc". */
  typeParseMode?: 'jsdoc' | 'closure' | 'typescript'
  /** Output jsdoccomment-compatible fields (delimiter, postDelimiter,
   *  initial, line indices, …) and exclude ox-jsdoc-specific fields
   *  (optional, defaultValue, rawBody, body). Default: false. */
  compatMode?: boolean
  /** Convert absent optional strings (rawType, name, namepathOrURL, text)
   *  to `""` instead of `null`. Mirrors jsdoccomment serialization.
   *  Default: false. */
  emptyStringForNull?: boolean
  /** Include ESTree position fields (start, end, range). Default: true. */
  includePositions?: boolean
  /** Spacing mode for compat output: "compact" (default, drops empty
   *  description lines like jsdoccomment) or "preserve" (keeps every
   *  scanned line verbatim). Only effective when `compatMode` is true. */
  spacing?: 'compact' | 'preserve'
}

export interface Diagnostic {
  message: string
}

export interface ParseResult {
  /** Parsed JSDoc AST as a JSON object (ESTree-like shape), or null on fatal error. */
  ast: JsdocBlock | null
  /** Parser diagnostics. Empty array on successful parse. */
  diagnostics: Diagnostic[]
}

export interface JsdocBlock {
  type: 'JsdocBlock'
  start: number
  end: number
  range: [number, number]
  description: string
  descriptionLines: JsdocDescriptionLine[]
  tags: JsdocTag[]
  inlineTags: JsdocInlineTag[]
  // ── compat_mode: true ──────────────────────────────────────────────
  /** Raw description slice (with `*` prefix and blank lines intact).
   *  Source-preserving view used by oxfmt-style formatters that need
   *  paragraph breaks and indented code blocks intact.
   *  See `design/008-oxlint-oxfmt-support/README.md` §4.4. compat_mode only. */
  descriptionRaw?: string
  /** Source-preserved opening delimiter ("/**"). compat_mode only. */
  delimiter?: string
  /** Whitespace after the opening delimiter. compat_mode only. */
  postDelimiter?: string
  /** Indent before the opening delimiter on the first line. compat_mode only. */
  initial?: string
  /** Closing delimiter ("*​/"). compat_mode only. */
  terminal?: string
  /** Newline characters after the block (e.g. "\n"). compat_mode only. */
  lineEnd?: string
  /** Line break right after `/**` (or "" when the block is single-line). compat_mode only. */
  delimiterLineBreak?: string
  /** Line break right before `*​/` (or ""). compat_mode only. */
  preterminalLineBreak?: string
  /** Zero-based logical line index of the closing `*​/`. compat_mode only. */
  endLine?: number
  /** First description line index, or undefined when none. compat_mode only. */
  descriptionStartLine?: number
  /** Last description line index, or undefined when none. compat_mode only. */
  descriptionEndLine?: number
  /** Last logical line that contributed to a description, or undefined. compat_mode only. */
  lastDescriptionLine?: number
  /** 1 when the description shares the closing-line text, else 0. compat_mode only. */
  hasPreterminalDescription?: number
  /** 1 when the last tag's description shares the closing line, else 0/undefined. compat_mode only. */
  hasPreterminalTagDescription?: number
}

export interface JsdocDescriptionLine {
  type: 'JsdocDescriptionLine'
  start: number
  end: number
  range: [number, number]
  delimiter: string
  postDelimiter: string
  initial: string
  description: string
}

export interface JsdocTag {
  type: 'JsdocTag'
  start: number
  end: number
  range: [number, number]
  tag: string
  rawType: string | null
  parsedType?: JsdocParsedType
  name: string | null
  /** ox-jsdoc-specific. Excluded from output when `compatMode: true`. */
  optional?: boolean
  /** ox-jsdoc-specific. Excluded from output when `compatMode: true`. */
  defaultValue?: string | null
  description: string
  /** Raw description slice (with `*` prefix and blank lines intact).
   *  Same shape as `JsdocBlock.descriptionRaw`. compat_mode only.
   *  See `design/008-oxlint-oxfmt-support/README.md` §4.4. */
  descriptionRaw?: string
  /** ox-jsdoc-specific. Excluded from output when `compatMode: true`. */
  rawBody?: string | null
  typeLines: JsdocTypeLine[]
  descriptionLines: JsdocDescriptionLine[]
  inlineTags: JsdocInlineTag[]
  /** ox-jsdoc-specific. Excluded from output when `compatMode: true`. */
  body?: JsdocTagBody | null
  // ── compat_mode: true ──────────────────────────────────────────────
  /** Whitespace before the leading `*` on the tag line. compat_mode only. */
  delimiter?: string
  /** Whitespace after the leading `*`. compat_mode only. */
  postDelimiter?: string
  /** Whitespace immediately after `@tag`. compat_mode only. */
  postTag?: string
  /** Whitespace immediately after `{type}`. compat_mode only. */
  postType?: string
  /** Whitespace immediately after the parameter name. compat_mode only. */
  postName?: string
  /** Indent before the leading `*` (== JsdocBlock.initial). compat_mode only. */
  initial?: string
  /** Newline characters terminating the tag's first line. compat_mode only. */
  lineEnd?: string
}

export interface JsdocTypeLine {
  type: 'JsdocTypeLine'
  start: number
  end: number
  range: [number, number]
  delimiter: string
  postDelimiter: string
  initial: string
  rawType: string
}

export interface JsdocInlineTag {
  type: 'JsdocInlineTag'
  start: number
  end: number
  range: [number, number]
  tag: string
  namepathOrURL: string | null
  text: string | null
  /** In `compatMode: true`, "unknown" is mapped to "plain" for jsdoccomment parity. */
  format: 'plain' | 'pipe' | 'space' | 'prefix' | 'unknown'
  /** ox-jsdoc-specific. Excluded from output when `compatMode: true`. */
  rawBody?: string | null
}

export type JsdocTagBody = JsdocGenericTagBody | JsdocBorrowsTagBody | JsdocRawTagBody

export interface JsdocGenericTagBody {
  kind: 'generic'
  typeSource: string | null
  value: JsdocTagValue | null
  separator: '-' | null
  description: string | null
}

export interface JsdocBorrowsTagBody {
  kind: 'borrows'
  source: JsdocTagValue
  target: JsdocTagValue
}

export interface JsdocRawTagBody {
  kind: 'raw'
  raw: string
}

export type JsdocTagValue =
  | { kind: 'parameter'; path: string; optional: boolean; defaultValue: string | null }
  | { kind: 'namepath'; raw: string }
  | { kind: 'identifier'; name: string }
  | { kind: 'raw'; value: string }

/**
 * Initialize the WASM module. Must be called once before `parse()`.
 *
 * @param wasmUrl Custom URL or source for the `.wasm` file.
 */
export function initWasm(
  wasmUrl?: string | URL | Request | Response | WebAssembly.Module | ArrayBuffer | BufferSource
): Promise<void>

/** Parsed JSDoc type expression AST (jsdoc-type-pratt-parser compatible). */
export type JsdocParsedType = {
  type: string
  [key: string]: unknown
}

/**
 * Parse a complete `/** ... *​/` JSDoc block comment.
 */
export function parse(sourceText: string, options?: ParseOptions): ParseResult

/**
 * Parse a standalone type expression (no comment parsing overhead).
 * Returns the stringified type, or null if parsing fails.
 */
export function parseType(
  typeText: string,
  mode?: 'jsdoc' | 'closure' | 'typescript'
): string | null

/**
 * Parse a type expression and return whether it succeeded (no stringify overhead).
 */
export function parseTypeCheck(typeText: string, mode?: 'jsdoc' | 'closure' | 'typescript'): boolean
