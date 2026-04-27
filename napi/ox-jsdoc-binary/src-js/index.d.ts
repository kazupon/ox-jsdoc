/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import type { RemoteJsdocBlock, RemoteSourceFile } from '@ox-jsdoc/decoder'

export interface ParseOptions {
  /** Suppress tag recognition inside fenced code blocks. Default: true. */
  fenceAware?: boolean
  /** Enable type expression parsing for `{...}` in tags. Default: false. */
  parseTypes?: boolean
  /** Parse mode for type expressions. Default: 'jsdoc'. */
  typeParseMode?: 'jsdoc' | 'closure' | 'typescript'
  /** Enable jsdoccomment-compat extension fields. Default: false. */
  compatMode?: boolean
  /** Emit per-node `description_raw_span` so the decoder's
   *  `descriptionRaw` getter and `descriptionText(true)` method work.
   *  Adds 8 bytes per `JsdocBlock` / `JsdocTag` ED record that has a
   *  description. Fully orthogonal to `compatMode`. Default: false.
   *  See `design/008-oxlint-oxfmt-support/README.md` §4.2. */
  preserveWhitespace?: boolean
  /** Convert absent optional strings (rawType, name, namepathOrURL, text)
   *  to `""` in `toJSON()` output. Only effective when `compatMode` is on.
   *  Mirrors the Rust serializer's `SerializeOptions.empty_string_for_null`
   *  for jsdoccomment parity. Default: false. */
  emptyStringForNull?: boolean
  /** Original-file absolute byte offset of `sourceText`. Default: 0. */
  baseOffset?: number
}

export interface Diagnostic {
  message: string
}

export interface ParseResult {
  /** Lazy root `RemoteJsdocBlock`, or `null` on parse failure. */
  ast: RemoteJsdocBlock | null
  /** Parser diagnostics. */
  diagnostics: Diagnostic[]
  /** Underlying `RemoteSourceFile` (held alive so `ast` getters keep working). */
  sourceFile: RemoteSourceFile
}

export interface BatchItem {
  /** `/** ... *​/` source text for this comment. */
  sourceText: string
  /** Original-file absolute byte offset (default 0). */
  baseOffset?: number
}

export interface BatchDiagnostic extends Diagnostic {
  /** Index of the input item this diagnostic belongs to. */
  rootIndex: number
}

export interface BatchParseResult {
  /** One entry per input item; `null` indicates parse failure. */
  asts: Array<RemoteJsdocBlock | null>
  /** All diagnostics produced during the batch. */
  diagnostics: BatchDiagnostic[]
  /** Underlying `RemoteSourceFile` (held alive so node getters keep working). */
  sourceFile: RemoteSourceFile
}

export interface BatchParseOptions {
  /** Suppress tag recognition inside fenced code blocks. Default: true. */
  fenceAware?: boolean
  /** Enable type expression parsing for `{...}` in tags. Default: false. */
  parseTypes?: boolean
  /** Parse mode for type expressions. Default: 'jsdoc'. */
  typeParseMode?: 'jsdoc' | 'closure' | 'typescript'
  /** Enable jsdoccomment-compat extension fields. Default: false. */
  compatMode?: boolean
  /** See `ParseOptions.preserveWhitespace`. */
  preserveWhitespace?: boolean
  /** See `ParseOptions.emptyStringForNull`. */
  emptyStringForNull?: boolean
}

/**
 * Parse a complete `/** ... *​/` JSDoc block comment into a lazy decoder root.
 */
export function parse(sourceText: string, options?: ParseOptions): ParseResult

/**
 * Parse N JSDoc block comments at once into a single shared Binary AST
 * buffer. Common strings (`*`, `*​/`, tag names) are interned once across
 * all comments.
 */
export function parseBatch(items: BatchItem[], options?: BatchParseOptions): BatchParseResult

/**
 * Visitor keys for every Remote* node kind. Re-exported from
 * `@ox-jsdoc/decoder` for ergonomics (single import for the whole binding).
 */
export { jsdocVisitorKeys } from '@ox-jsdoc/decoder'
