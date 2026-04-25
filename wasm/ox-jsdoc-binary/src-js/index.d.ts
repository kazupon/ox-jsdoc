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
  /**
   * Release the wasm-side bytes. After calling, accessing `ast` /
   * `sourceFile` is undefined behaviour.
   */
  free(): void
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

export interface BatchParseOptions {
  /** Suppress tag recognition inside fenced code blocks. Default: true. */
  fenceAware?: boolean
  /** Enable type expression parsing for `{...}` in tags. Default: false. */
  parseTypes?: boolean
  /** Parse mode for type expressions. Default: 'jsdoc'. */
  typeParseMode?: 'jsdoc' | 'closure' | 'typescript'
  /** Enable jsdoccomment-compat extension fields. Default: false. */
  compatMode?: boolean
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

/**
 * Initialize the WASM module. Must be called once before {@link parse}.
 */
export function initWasm(
  wasmUrl?: string | URL | Request | Response | WebAssembly.Module | ArrayBuffer | BufferSource
): Promise<void>

/**
 * Parse a complete `/** ... *​/` JSDoc block comment into a lazy decoder root.
 */
export function parse(sourceText: string, options?: ParseOptions): ParseResult

/**
 * Parse N JSDoc block comments at once into a single shared binary-AST
 * buffer. Common strings (`*`, `*​/`, tag names) are interned once across
 * all comments.
 */
export function parseBatch(items: BatchItem[], options?: BatchParseOptions): BatchParseResult

/**
 * Visitor keys for every Remote* node kind. Re-exported from
 * `@ox-jsdoc/decoder` for ergonomics (single import for the whole binding).
 */
export { jsdocVisitorKeys } from '@ox-jsdoc/decoder'
