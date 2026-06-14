/**
 * Parser orchestration for the ox-jsdoc playground.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { RemoteSourceFile } from '@ox-jsdoc/decoder'
import { computed, onMounted, shallowRef, ref, type ComputedRef, type Ref } from 'vue'
import type {
  OxcComment,
  OxcParserModule,
  ParsedJsdocComment,
  ParsedJsdocSourceFile,
  ParserOptions,
  ParseView,
  SourceRange
} from '../types/playground'

const oxJsdocWasmModuleUrl = '/vendor/ox-jsdoc/ox_jsdoc_wasm.js'
const oxJsdocWasmBinaryUrl = '/vendor/ox-jsdoc/ox_jsdoc_wasm_bg.wasm'
const oxcParserModuleUrl = '/vendor/oxc-parser/browser-bundle.js'
const utf8Encoder = new TextEncoder()

type RemoteAst = {
  toJSON(): unknown
}

type WasmDiagnostic = {
  message: string
}

type WasmBatchDiagnostic = WasmDiagnostic & {
  rootIndex: number
}

type WasmParseHandle = {
  bufferLen(): number
  bufferPtr(): number
  diagnostics(): unknown
  free(): void
}

type OxJsdocWasmModule = {
  default(moduleOrPath?: unknown): Promise<{ memory: WebAssembly.Memory }>
  parse_jsdoc(
    sourceText: string,
    fenceAware: boolean | null,
    parseTypes: boolean | null,
    typeParseMode: ParserOptions['typeParseMode'] | null,
    compatMode: boolean | null,
    preserveWhitespace: boolean | null,
    baseOffset: number | null
  ): WasmParseHandle
  parse_jsdoc_batch_raw(
    sourceText: Uint8Array,
    offsets: Uint32Array,
    baseOffsets: Uint32Array,
    fenceAware: boolean | null,
    parseTypes: boolean | null,
    typeParseMode: ParserOptions['typeParseMode'] | null,
    compatMode: boolean | null,
    preserveWhitespace: boolean | null
  ): WasmParseHandle
}

async function importOxJsdocWasmModule(specifier: string): Promise<OxJsdocWasmModule> {
  return import(/* @vite-ignore */ specifier) as Promise<OxJsdocWasmModule>
}

async function importBrowserModule(specifier: string): Promise<unknown> {
  const response = await fetch(specifier)

  if (!response.ok) {
    throw new Error(`Failed to load ${specifier}: ${response.status} ${response.statusText}`)
  }

  const assetBaseUrl = new URL('.', new URL(specifier, globalThis.location.href)).href
  const wasmUrl = new URL('parser.wasm32-wasi.wasm', assetBaseUrl).href
  const workerUrl = new URL('wasi-worker-browser.mjs', assetBaseUrl).href
  const source = (await response.text())
    .replaceAll(
      "new URL('./parser.wasm32-wasi.wasm', import.meta.url)",
      `new URL(${JSON.stringify(wasmUrl)})`
    )
    .replaceAll(
      "new URL('./wasi-worker-browser.mjs', import.meta.url)",
      `new URL(${JSON.stringify(workerUrl)})`
    )
  const moduleUrl = URL.createObjectURL(new Blob([source], { type: 'text/javascript' }))

  try {
    return await import(/* @vite-ignore */ moduleUrl)
  } finally {
    URL.revokeObjectURL(moduleUrl)
  }
}

type UseJsdocParserOptions = {
  options: ParserOptions
  source: Ref<string>
  sourceLanguage: ComputedRef<'javascript' | 'typescript'>
}

export function useJsdocParser({ options, source, sourceLanguage }: UseJsdocParserOptions) {
  const wasmReady = ref(false)
  const wasmError = ref<string | null>(null)
  const oxJsdocWasm = shallowRef<OxJsdocWasmModule | null>(null)
  const wasmMemory = shallowRef<WebAssembly.Memory | null>(null)
  const oxcParser = shallowRef<OxcParserModule | null>(null)
  const oxcError = ref<string | null>(null)

  onMounted(async () => {
    try {
      const wasmModule = await importOxJsdocWasmModule(oxJsdocWasmModuleUrl)
      const wasmExports = await wasmModule.default(oxJsdocWasmBinaryUrl)

      oxJsdocWasm.value = wasmModule
      wasmMemory.value = wasmExports.memory
      wasmReady.value = true
    } catch (error) {
      wasmError.value = error instanceof Error ? error.message : String(error)
    }

    try {
      oxcParser.value = (await importBrowserModule(oxcParserModuleUrl)) as OxcParserModule
    } catch (error) {
      oxcError.value = error instanceof Error ? error.message : String(error)
    }
  })

  const parseView = computed<ParseView>(() => {
    if (!wasmReady.value || !oxJsdocWasm.value || !wasmMemory.value || !oxcParser.value) {
      const failed = wasmError.value !== null || oxcError.value !== null

      return {
        status: failed ? 'error' : 'loading',
        ast: null,
        diagnostics: [wasmError.value ?? oxcError.value ?? 'Initializing parsers...'],
        duration: null,
        tagCount: 0,
        inlineTagCount: 0
      }
    }

    const start = performance.now()
    const parseResult = parseJavaScriptComments(source.value)

    if (parseResult.comments.length === 0) {
      return {
        status: 'invalid',
        ast: null,
        diagnostics: [
          ...parseResult.diagnostics,
          'No JSDoc block found. Add a /** ... */ block comment.'
        ],
        duration: performance.now() - start,
        tagCount: 0,
        inlineTagCount: 0
      }
    }

    return options.parseBatch
      ? parseJsdocCommentsWithBatch(parseResult, start)
      : parseJsdocCommentsIndividually(parseResult, start)
  })

  function parseJsdocCommentsWithBatch(
    parseResult: {
      comments: Array<{ baseOffset: number; sourceText: string }>
      diagnostics: string[]
    },
    start: number
  ): ParseView {
    const result = parseJsdocBatch(parseResult.comments, getJsdocParseOptions())

    try {
      const comments = result.asts
        .map((ast, index): ParsedJsdocComment | null => {
          const comment = parseResult.comments[index]

          if (!ast || !comment) {
            return null
          }

          return {
            type: 'JsdocComment',
            range: [comment.baseOffset, comment.baseOffset + comment.sourceText.length],
            baseOffset: comment.baseOffset,
            sourceText: comment.sourceText,
            ast: ast.toJSON()
          }
        })
        .filter((comment): comment is ParsedJsdocComment => comment !== null)
      const ast = createJsdocSourceFileAst(comments)
      const diagnostics = [
        ...parseResult.diagnostics,
        ...result.diagnostics.map(diagnostic => {
          const comment = parseResult.comments[diagnostic.rootIndex]
          const prefix = comment ? `Comment ${diagnostic.rootIndex + 1}: ` : ''
          return `${prefix}${diagnostic.message}`
        })
      ]

      return {
        status: ast && diagnostics.length === 0 ? 'ok' : 'invalid',
        ast,
        diagnostics,
        duration: performance.now() - start,
        tagCount: sumArrayLength(
          comments.map(comment => comment.ast),
          'tags'
        ),
        inlineTagCount: sumArrayLength(
          comments.map(comment => comment.ast),
          'inlineTags'
        )
      }
    } catch (error) {
      return {
        status: 'error',
        ast: null,
        diagnostics: [error instanceof Error ? error.message : String(error)],
        duration: performance.now() - start,
        tagCount: 0,
        inlineTagCount: 0
      }
    } finally {
      result.free()
    }
  }

  function parseJsdocCommentsIndividually(
    parseResult: {
      comments: Array<{ baseOffset: number; sourceText: string }>
      diagnostics: string[]
    },
    start: number
  ): ParseView {
    const comments: ParsedJsdocComment[] = []
    const diagnostics = [...parseResult.diagnostics]

    for (const [index, comment] of parseResult.comments.entries()) {
      const result = parseJsdoc(comment.sourceText, {
        ...getJsdocParseOptions(),
        baseOffset: comment.baseOffset
      })

      try {
        const ast = result.ast?.toJSON() ?? null

        if (ast) {
          comments.push({
            type: 'JsdocComment',
            range: [comment.baseOffset, comment.baseOffset + comment.sourceText.length],
            baseOffset: comment.baseOffset,
            sourceText: comment.sourceText,
            ast
          })
        }

        diagnostics.push(
          ...result.diagnostics.map(diagnostic => `Comment ${index + 1}: ${diagnostic.message}`)
        )
      } finally {
        result.free()
      }
    }

    const ast = createJsdocSourceFileAst(comments)

    return {
      status: ast && diagnostics.length === 0 ? 'ok' : 'invalid',
      ast,
      diagnostics,
      duration: performance.now() - start,
      tagCount: sumArrayLength(
        comments.map(comment => comment.ast),
        'tags'
      ),
      inlineTagCount: sumArrayLength(
        comments.map(comment => comment.ast),
        'inlineTags'
      )
    }
  }

  function getJsdocParseOptions() {
    return {
      compatMode: options.compatMode,
      parseTypes: options.parseTypes,
      preserveWhitespace: options.preserveWhitespace,
      typeParseMode: options.typeParseMode
    }
  }

  function parseJsdoc(
    sourceText: string,
    parseOptions: ReturnType<typeof getJsdocParseOptions> & { baseOffset?: number }
  ): {
    ast: RemoteAst | null
    diagnostics: WasmDiagnostic[]
    free: () => void
  } {
    const { memory, wasm } = getWasmParser()
    const handle = wasm.parse_jsdoc(
      sourceText,
      null,
      parseOptions.parseTypes,
      parseOptions.typeParseMode,
      parseOptions.compatMode,
      parseOptions.preserveWhitespace,
      parseOptions.baseOffset ?? null
    )
    const sourceFile = decodeSourceFile(handle, memory)

    return {
      ast: (sourceFile.asts[0] ?? null) as RemoteAst | null,
      diagnostics: handle.diagnostics() as WasmDiagnostic[],
      free: () => handle.free()
    }
  }

  function parseJsdocBatch(
    comments: Array<{ baseOffset: number; sourceText: string }>,
    parseOptions: ReturnType<typeof getJsdocParseOptions>
  ): {
    asts: Array<RemoteAst | null>
    diagnostics: WasmBatchDiagnostic[]
    free: () => void
  } {
    const { memory, wasm } = getWasmParser()
    const commentCount = comments.length
    let totalChars = 0

    for (const comment of comments) {
      totalChars += comment.sourceText.length
    }

    const concat = new Uint8Array(totalChars * 3)
    const offsets = new Uint32Array(commentCount + 1)
    const baseOffsets = new Uint32Array(commentCount)
    let position = 0

    for (const [index, comment] of comments.entries()) {
      const { written } = utf8Encoder.encodeInto(comment.sourceText, concat.subarray(position))

      position += written
      offsets[index + 1] = position
      baseOffsets[index] = comment.baseOffset
    }

    const handle = wasm.parse_jsdoc_batch_raw(
      concat.subarray(0, position),
      offsets,
      baseOffsets,
      null,
      parseOptions.parseTypes,
      parseOptions.typeParseMode,
      parseOptions.compatMode,
      parseOptions.preserveWhitespace
    )
    const sourceFile = decodeSourceFile(handle, memory)

    return {
      asts: sourceFile.asts as Array<RemoteAst | null>,
      diagnostics: handle.diagnostics() as WasmBatchDiagnostic[],
      free: () => handle.free()
    }
  }

  function getWasmParser(): {
    memory: WebAssembly.Memory
    wasm: OxJsdocWasmModule
  } {
    if (!oxJsdocWasm.value || !wasmMemory.value) {
      throw new Error('WASM parser is not initialized')
    }

    return {
      memory: wasmMemory.value,
      wasm: oxJsdocWasm.value
    }
  }

  function decodeSourceFile(handle: WasmParseHandle, memory: WebAssembly.Memory): RemoteSourceFile {
    const view = new Uint8Array(memory.buffer, handle.bufferPtr(), handle.bufferLen())

    return new RemoteSourceFile(view)
  }

  function createJsdocSourceFileAst(comments: ParsedJsdocComment[]): ParsedJsdocSourceFile | null {
    const firstComment = comments[0]
    const lastComment = comments.at(-1)

    return firstComment && lastComment
      ? {
          type: 'JsdocSourceFile',
          range: [firstComment.baseOffset, lastComment.baseOffset + lastComment.sourceText.length],
          comments
        }
      : null
  }

  function parseJavaScriptComments(value: string): {
    comments: Array<{ baseOffset: number; sourceText: string }>
    diagnostics: string[]
  } {
    try {
      const filename = sourceLanguage.value === 'typescript' ? 'playground.ts' : 'playground.js'
      const result = oxcParser.value?.parseSync(filename, value, {
        range: true,
        sourceType: 'module'
      })
      const comments = (result?.comments ?? [])
        .map(comment => {
          const range = getOxcCommentRange(comment)

          return range
            ? {
                baseOffset: range[0],
                sourceText: value.slice(range[0], range[1])
              }
            : null
        })
        .filter(comment => comment !== null)
        .filter(comment => comment.sourceText.startsWith('/**'))
      const diagnostics = (result?.errors ?? []).map(
        error => error.message ?? JSON.stringify(error)
      )

      return { comments, diagnostics }
    } catch (error) {
      return {
        comments: [],
        diagnostics: [error instanceof Error ? error.message : String(error)]
      }
    }
  }

  function getOxcCommentRange(comment: OxcComment): SourceRange | null {
    if (typeof comment.start === 'number' && typeof comment.end === 'number') {
      return [comment.start, comment.end]
    }

    if (
      Array.isArray(comment.range) &&
      comment.range.length === 2 &&
      typeof comment.range[0] === 'number' &&
      typeof comment.range[1] === 'number'
    ) {
      return comment.range
    }

    return null
  }

  function getArrayLength(value: unknown, key: string): number {
    if (!value || typeof value !== 'object') {
      return 0
    }

    const item = (value as Record<string, unknown>)[key]
    return Array.isArray(item) ? item.length : 0
  }

  function sumArrayLength(values: unknown[], key: string): number {
    return values.reduce<number>((total, value) => total + getArrayLength(value, key), 0)
  }

  return {
    parseView
  }
}
