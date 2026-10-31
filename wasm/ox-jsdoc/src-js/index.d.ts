/**
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import type { JsdocCommentInput, RemoteJsdocBlock, RemoteSourceFile } from '@ox-jsdoc/decoder'

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
  /** Original-file absolute UTF-8 byte offset of `sourceText` (not a
   *  JavaScript string index; node ranges use the same unit). Default: 0. */
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
  /**
   * Release the wasm-side bytes. After calling, accessing `ast` /
   * `sourceFile` is undefined behaviour.
   */
  free(): void
}

export interface BatchItem {
  /** `/** ... *​/` source text for this comment. */
  sourceText: string
  /** Original-file absolute UTF-8 byte offset (not a JavaScript string
   *  index; node ranges use the same unit). Default: 0. */
  baseOffset?: number
}

export interface BatchDiagnostic extends Diagnostic {
  /** Index of the input item this diagnostic belongs to. */
  rootIndex: number
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
  /** Select the materialized output. Default: 'ast'. */
  output?: 'ast'
}

export interface BatchParseJsdocCommentInputOptions extends Omit<BatchParseOptions, 'output'> {
  /** Return decoder-created input for @ox-jsdoc/jsdoccomment's normalizer. */
  output: 'jsdoccomment-input'
}

export interface BatchParseResult {
  /** One entry per input item; `null` indicates parse failure. */
  asts: Array<RemoteJsdocBlock | null>
  /** All diagnostics produced during the batch. */
  diagnostics: BatchDiagnostic[]
  /** Underlying `RemoteSourceFile` (held alive so node getters keep working). */
  sourceFile: RemoteSourceFile
  /**
   * Release the wasm-side bytes. After calling, accessing `asts` /
   * `sourceFile` is undefined behaviour.
   */
  free(): void
}

export interface BatchParseJsdocCommentInputResult extends BatchParseResult {
  /** One jsdoccomment input block per input item; `null` indicates parse failure. */
  blocks: Array<JsdocCommentInput | null>
}

/**
 * The `.wasm` file: a URL, a `Request` or `Response`, its bytes, or a
 * compiled module. Same as wasm-bindgen's `InitInput`, but spelled with types
 * that also exist without the DOM lib (Node.js projects), where `RequestInfo`
 * and `BufferSource` would be `any`.
 */
type WasmSource =
  | string
  | URL
  | Request
  | Response
  | ArrayBuffer
  | ArrayBufferView
  | WebAssembly.Module

/**
 * What {@link initWasm} accepts: the `.wasm` file, a promise of it, or
 * wasm-bindgen's object form `{ module_or_path }`.
 */
export type InitWasmInput =
  | WasmSource
  | Promise<WasmSource>
  | { module_or_path: WasmSource | Promise<WasmSource> }

/**
 * Initialize the WASM module. Must be called once before {@link parse}.
 *
 * Concurrent and repeated calls share one initialization; the input of later
 * calls is ignored. After a failure, the next call starts over.
 *
 * @param input The `.wasm` file. When omitted, the file next to the generated
 *   glue is fetched (browsers and bundlers). If that fails (always in Node.js,
 *   where `fetch` cannot read its `file:` URL), the error says what to pass
 *   instead, with the original error as its `cause`.
 */
export function initWasm(input?: InitWasmInput): Promise<void>

/**
 * Parse a complete `/** ... *​/` JSDoc block comment into a lazy decoder root.
 */
export function parse(sourceText: string, options?: ParseOptions): ParseResult

/**
 * Parse N JSDoc block comments at once into a single shared binary-AST
 * buffer. Common strings (`*`, `*​/`, tag names) are interned once across
 * all comments.
 */
export function parseBatch(
  items: BatchItem[],
  options: BatchParseJsdocCommentInputOptions
): BatchParseJsdocCommentInputResult
export function parseBatch(items: BatchItem[], options?: BatchParseOptions): BatchParseResult

/**
 * Parse a standalone type expression (no comment parsing overhead).
 * Returns the stringified type or `null` if parsing fails.
 */
export function parseType(
  typeText: string,
  mode?: 'jsdoc' | 'closure' | 'typescript'
): string | null

/**
 * Parse a type expression and return whether it succeeded.
 * No stringify overhead — used for benchmarks.
 */
export function parseTypeCheck(typeText: string, mode?: 'jsdoc' | 'closure' | 'typescript'): boolean

/**
 * Visitor keys for every Remote* node kind. Re-exported from
 * `@ox-jsdoc/decoder` for ergonomics (single import for the whole binding).
 */
export { jsdocVisitorKeys } from '@ox-jsdoc/decoder'
