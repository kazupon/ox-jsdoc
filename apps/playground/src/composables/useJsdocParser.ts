/**
 * Parser orchestration for the ox-jsdoc playground.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import { computed, onMounted, shallowRef, ref, type ComputedRef, type Ref } from 'vue'
import { initWasm, parse, parseBatch } from '@ox-jsdoc/wasm'
import type {
  OxcComment,
  OxcParserModule,
  ParsedJsdocComment,
  ParsedJsdocSourceFile,
  ParserOptions,
  ParseView,
  SourceRange
} from '../types/playground'

async function importBrowserModule(specifier: string): Promise<unknown> {
  return import(/* -ignore */ specifier)
}

type UseJsdocParserOptions = {
  options: ParserOptions
  source: Ref<string>
  sourceLanguage: ComputedRef<'javascript' | 'typescript'>
}

export function useJsdocParser({ options, source, sourceLanguage }: UseJsdocParserOptions) {
  const wasmReady = ref(false)
  const wasmError = ref<string | null>(null)
  const oxcParser = shallowRef<OxcParserModule | null>(null)
  const oxcError = ref<string | null>(null)

  onMounted(async () => {
    try {
      await initWasm('/vendor/ox-jsdoc/ox_jsdoc_wasm_bg.wasm')
      wasmReady.value = true
    } catch (error) {
      wasmError.value = error instanceof Error ? error.message : String(error)
    }

    try {
      oxcParser.value = (await importBrowserModule(
        '/__oxc-parser/browser-bundle.js'
      )) as OxcParserModule
    } catch (error) {
      oxcError.value = error instanceof Error ? error.message : String(error)
    }
  })

  const parseView = computed<ParseView>(() => {
    if (!wasmReady.value || !oxcParser.value) {
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
    const result = parseBatch(parseResult.comments, getJsdocParseOptions())

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
      const result = parse(comment.sourceText, {
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
