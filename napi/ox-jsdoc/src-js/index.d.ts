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
  parsedType: JsdocParsedType | null
  name: string | null
  optional: boolean
  defaultValue: string | null
  description: string
  rawBody: string | null
  typeLines: JsdocTypeLine[]
  descriptionLines: JsdocDescriptionLine[]
  inlineTags: JsdocInlineTag[]
  body: JsdocTagBody | null
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
  format: 'plain' | 'pipe' | 'space' | 'prefix' | 'unknown'
  rawBody: string | null
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
