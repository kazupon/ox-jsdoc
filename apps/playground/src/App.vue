<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, shallowRef, watch } from 'vue'
import AstTree from './components/AstTree.vue'
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import * as monaco from 'monaco-editor/esm/vs/editor/editor.api'
import 'monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution'
import 'monaco-editor/esm/vs/basic-languages/typescript/typescript.contribution'
import { initWasm, parse, parseBatch } from '@ox-jsdoc/wasm'
import oxJsdocWasmPackage from '../../../wasm/ox-jsdoc/package.json' with { type: 'json' }

type TypeParseMode = 'jsdoc' | 'closure' | 'typescript'

type PlaygroundSettings = {
  compatMode: boolean
  parseBatch: boolean
  parseTypes: boolean
  preserveWhitespace: boolean
  theme: 'light' | 'dark'
  typeParseMode: TypeParseMode
}

type OxcComment = {
  end?: number
  range?: SourceRange
  start?: number
}

type OxcParserModule = {
  parseSync(
    filename: string,
    sourceText: string,
    options?: {
      range?: boolean
      sourceType?: 'module' | 'script' | 'commonjs' | 'unambiguous'
    }
  ): {
    comments?: OxcComment[]
    errors?: Array<{ message?: string }>
  }
}

const dynamicImport = new Function('specifier', 'return import(specifier)') as (
  specifier: string
) => Promise<unknown>

type ParsedJsdocComment = {
  ast: unknown
  baseOffset: number
  range: SourceRange
  sourceText: string
  type: 'JsdocComment'
}

type ParsedJsdocSourceFile = {
  comments: ParsedJsdocComment[]
  range: SourceRange
  type: 'JsdocSourceFile'
}

type SourceRange = [number, number]

type ParseView =
  | {
      status: 'loading' | 'error'
      ast: null
      diagnostics: string[]
      duration: null
      tagCount: 0
      inlineTagCount: 0
    }
  | {
      status: 'ok' | 'invalid' | 'error'
      ast: unknown
      diagnostics: string[]
      duration: number
      tagCount: number
      inlineTagCount: number
    }

const sample = `/**
 * Parse a JSDoc block with ox-jsdoc.
 *
 * Supports inline links like {@link https://github.com/kazupon/ox-jsdoc}.
 *
 * @template T
 * @param {Array<T>} values - Input values.
 * @param {(value: T) => string} format - Value formatter.
 * @returns {string[]} Formatted values.
 */
export function formatValues<T>(
  values: Array<T>,
  format: (value: T) => string,
): string[] {
  return values.map(format)
}

/**
 * Return the first value when it exists.
 *
 * @param {T[]} values - Candidate values.
 * @returns {T | undefined} First value.
 */
export function firstValue<T>(values: T[]): T | undefined {
  return values[0]
}`

const oxJsdocWasmVersion = oxJsdocWasmPackage.version
const source = ref(sample)
const selectedAstPath = ref('')
const revealAstPath = ref('')
const revealAstVersion = ref(0)
const wasmReady = ref(false)
const wasmError = ref<string | null>(null)
const oxcParser = shallowRef<OxcParserModule | null>(null)
const oxcError = ref<string | null>(null)
const editorHost = ref<HTMLElement | null>(null)
const sourceEditor = shallowRef<monaco.editor.IStandaloneCodeEditor | null>(null)
let resizeObserver: ResizeObserver | null = null
const settingsKey = 'ox-jsdoc.playground.settings'
const defaultSettings: PlaygroundSettings = {
  compatMode: false,
  parseBatch: true,
  parseTypes: true,
  preserveWhitespace: true,
  theme: 'light',
  typeParseMode: 'typescript'
}
const settings = loadSettings()
const theme = ref<'light' | 'dark'>(settings.theme)

const globalSelf = self as typeof self & {
  MonacoEnvironment?: {
    getWorker(): Worker
  }
}

globalSelf.MonacoEnvironment = {
  getWorker() {
    return new editorWorker()
  }
}

const defineEditorThemes = () => {
  monaco.editor.defineTheme('ox-jsdoc-light', {
    base: 'vs',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '7a6653' },
      { token: 'comment.doc', foreground: '7a6653' },
      { token: 'keyword', foreground: '0891b2', fontStyle: 'bold' },
      { token: 'string', foreground: '28704f' },
      { token: 'number', foreground: '1f6f8b' },
      { token: 'regexp', foreground: 'a93678' },
      { token: 'type', foreground: '1f6f8b' },
      { token: 'delimiter', foreground: '8b6f4e' },
      { token: 'comment.jsdoc', foreground: '7a6653' },
      { token: 'comment.block.jsdoc', foreground: '9a8165' },
      { token: 'comment.marker.jsdoc', foreground: 'b9a88e' },
      { token: 'tag.jsdoc', foreground: '0891b2', fontStyle: 'bold' },
      { token: 'inline.tag.jsdoc', foreground: 'a93678', fontStyle: 'bold' },
      { token: 'inline.punctuation.jsdoc', foreground: 'a93678' },
      { token: 'type.jsdoc', foreground: '1f6f8b' },
      { token: 'name.jsdoc', foreground: '28704f', fontStyle: 'bold' },
      { token: 'name.optional.jsdoc', foreground: '28704f' },
      { token: 'dash.jsdoc', foreground: 'b08b54' }
    ],
    colors: {
      'editor.background': '#fffaf0',
      'editor.foreground': '#1d1b16',
      'editor.lineHighlightBackground': '#f3e5cf',
      'editorLineNumber.foreground': '#aa9c89',
      'editorLineNumber.activeForeground': '#0891b2',
      'editor.selectionBackground': '#a5f3fc',
      'editorCursor.foreground': '#0891b2'
    }
  })

  monaco.editor.defineTheme('ox-jsdoc-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: 'b8a991' },
      { token: 'comment.doc', foreground: 'b8a991' },
      { token: 'keyword', foreground: '22d3ee', fontStyle: 'bold' },
      { token: 'string', foreground: '97d988' },
      { token: 'number', foreground: '6fd1c7' },
      { token: 'regexp', foreground: 'f0a7d3' },
      { token: 'type', foreground: '6fd1c7' },
      { token: 'delimiter', foreground: 'b8a991' },
      { token: 'comment.jsdoc', foreground: 'b8a991' },
      { token: 'comment.block.jsdoc', foreground: '887b68' },
      { token: 'comment.marker.jsdoc', foreground: '6f6557' },
      { token: 'tag.jsdoc', foreground: '22d3ee', fontStyle: 'bold' },
      { token: 'inline.tag.jsdoc', foreground: 'f0a7d3', fontStyle: 'bold' },
      { token: 'inline.punctuation.jsdoc', foreground: 'f0a7d3' },
      { token: 'type.jsdoc', foreground: '6fd1c7' },
      { token: 'name.jsdoc', foreground: '97d988', fontStyle: 'bold' },
      { token: 'name.optional.jsdoc', foreground: '97d988' },
      { token: 'dash.jsdoc', foreground: 'dfb35c' }
    ],
    colors: {
      'editor.background': '#161310',
      'editor.foreground': '#f4eadc',
      'editor.lineHighlightBackground': '#211c17',
      'editorLineNumber.foreground': '#6f6557',
      'editorLineNumber.activeForeground': '#22d3ee',
      'editor.selectionBackground': '#164e63',
      'editorCursor.foreground': '#22d3ee'
    }
  })
}

const editorTheme = computed(() => `ox-jsdoc-${theme.value}`)

const toggleTheme = () => {
  theme.value = theme.value === 'light' ? 'dark' : 'light'
}

const options = reactive({
  parseTypes: settings.parseTypes,
  typeParseMode: settings.typeParseMode,
  preserveWhitespace: settings.preserveWhitespace,
  compatMode: settings.compatMode,
  parseBatch: settings.parseBatch
})

const sourceLanguage = computed(() =>
  options.typeParseMode === 'typescript' ? 'typescript' : 'javascript'
)

function loadSettings(): PlaygroundSettings {
  if (typeof localStorage === 'undefined') {
    return { ...defaultSettings }
  }

  try {
    const raw = localStorage.getItem(settingsKey)
    if (!raw) {
      return { ...defaultSettings }
    }

    const parsed = JSON.parse(raw) as Partial<PlaygroundSettings>

    return {
      compatMode:
        typeof parsed.compatMode === 'boolean' ? parsed.compatMode : defaultSettings.compatMode,
      parseBatch:
        typeof parsed.parseBatch === 'boolean' ? parsed.parseBatch : defaultSettings.parseBatch,
      parseTypes:
        typeof parsed.parseTypes === 'boolean' ? parsed.parseTypes : defaultSettings.parseTypes,
      preserveWhitespace:
        typeof parsed.preserveWhitespace === 'boolean'
          ? parsed.preserveWhitespace
          : defaultSettings.preserveWhitespace,
      theme:
        parsed.theme === 'dark' || parsed.theme === 'light' ? parsed.theme : defaultSettings.theme,
      typeParseMode:
        parsed.typeParseMode === 'jsdoc' ||
        parsed.typeParseMode === 'closure' ||
        parsed.typeParseMode === 'typescript'
          ? parsed.typeParseMode
          : defaultSettings.typeParseMode
    }
  } catch {
    return { ...defaultSettings }
  }
}

function saveSettings() {
  if (typeof localStorage === 'undefined') {
    return
  }

  try {
    localStorage.setItem(
      settingsKey,
      JSON.stringify({
        compatMode: options.compatMode,
        parseBatch: options.parseBatch,
        parseTypes: options.parseTypes,
        preserveWhitespace: options.preserveWhitespace,
        theme: theme.value,
        typeParseMode: options.typeParseMode
      } satisfies PlaygroundSettings)
    )
  } catch {
    // Ignore unavailable storage; parser settings still work for the current session.
  }
}

onMounted(async () => {
  defineEditorThemes()
  document.documentElement.dataset.theme = theme.value

  if (editorHost.value) {
    sourceEditor.value = monaco.editor.create(editorHost.value, {
      automaticLayout: true,
      fontFamily: '"SFMono-Regular", Consolas, "Liberation Mono", monospace',
      fontSize: 14,
      language: sourceLanguage.value,
      lineNumbersMinChars: 3,
      minimap: { enabled: false },
      padding: { bottom: 18, top: 18 },
      scrollBeyondLastLine: false,
      smoothScrolling: true,
      tabSize: 2,
      theme: editorTheme.value,
      value: source.value,
      wordWrap: 'on'
    })

    sourceEditor.value.onDidChangeModelContent(() => {
      const value = sourceEditor.value?.getValue() ?? ''
      if (value !== source.value) {
        source.value = value
      }
    })

    sourceEditor.value.onMouseDown(event => {
      const model = sourceEditor.value?.getModel()
      const position = event.target.position

      if (!model || !position) {
        return
      }

      revealAstNodeAtOffset(model.getOffsetAt(position))
    })

    resizeObserver = new ResizeObserver(() => {
      sourceEditor.value?.layout()
    })
    resizeObserver.observe(editorHost.value)
  }

  try {
    await initWasm('/vendor/ox-jsdoc/ox_jsdoc_wasm_bg.wasm')
    wasmReady.value = true
  } catch (error) {
    wasmError.value = error instanceof Error ? error.message : String(error)
  }

  try {
    oxcParser.value = (await dynamicImport(
      '/vendor/oxc-parser/browser-bundle.js'
    )) as OxcParserModule
  } catch (error) {
    oxcError.value = error instanceof Error ? error.message : String(error)
  }
})

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  sourceEditor.value?.dispose()
})

watch(source, value => {
  const editor = sourceEditor.value

  if (editor && editor.getValue() !== value) {
    editor.setValue(value)
  }
})

watch(theme, value => {
  document.documentElement.dataset.theme = value
  monaco.editor.setTheme(editorTheme.value)
})

watch(sourceLanguage, value => {
  const model = sourceEditor.value?.getModel()

  if (model) {
    monaco.editor.setModelLanguage(model, value)
  }
})

watch(
  [
    theme,
    () => options.compatMode,
    () => options.parseBatch,
    () => options.parseTypes,
    () => options.preserveWhitespace,
    () => options.typeParseMode
  ],
  saveSettings
)

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
    const diagnostics = (result?.errors ?? []).map(error => error.message ?? String(error))

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

function revealAstNodeAtOffset(offset: number): void {
  const path = findAstPathAtOffset(parseView.value.ast, offset)

  if (!path) {
    return
  }

  selectedAstPath.value = path
  revealAstPath.value = path
  revealAstVersion.value += 1
}

function findAstPathAtOffset(value: unknown, offset: number): string | null {
  let match: { depth: number; path: string; span: number } | null = null

  function visit(item: unknown, path: string, depth: number): void {
    const range = getAstObjectRange(item)

    if (range && path !== 'root' && range[0] <= offset && offset <= range[1]) {
      const span = range[1] - range[0]

      if (!match || span < match.span || (span === match.span && depth > match.depth)) {
        match = { depth, path, span }
      }
    }

    if (Array.isArray(item)) {
      item.forEach((child, index) => visit(child, `${path}.${index}`, depth + 1))
      return
    }

    if (isAstRecord(item)) {
      Object.entries(item).forEach(([key, child]) => {
        visit(child, `${path}.${key}`, depth + 1)
      })
    }
  }

  visit(value, 'root', 0)

  return match?.path ?? null
}

function getAstObjectRange(value: unknown): SourceRange | null {
  if (!isAstRecord(value)) {
    return null
  }

  const sourceRange = value.range

  if (
    Array.isArray(sourceRange) &&
    sourceRange.length === 2 &&
    typeof sourceRange[0] === 'number' &&
    typeof sourceRange[1] === 'number'
  ) {
    return sourceRange
  }

  return typeof value.start === 'number' && typeof value.end === 'number'
    ? [value.start, value.end]
    : null
}

function isAstRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

const selectSourceRange = (range: SourceRange) => {
  const editor = sourceEditor.value
  const model = editor?.getModel()

  if (!editor || !model) {
    return
  }

  const sourceLength = source.value.length
  const startOffset = Math.max(0, Math.min(range[0], sourceLength))
  const endOffset = Math.max(startOffset, Math.min(range[1], sourceLength))
  const start = model.getPositionAt(startOffset)
  const end = model.getPositionAt(endOffset)
  const selection = new monaco.Range(start.lineNumber, start.column, end.lineNumber, end.column)

  editor.setSelection(selection)
  editor.revealRangeInCenter(selection, monaco.editor.ScrollType.Smooth)
  editor.focus()
}

const handleAstNodeSelect = (selection: { path: string; range: SourceRange | null }) => {
  selectedAstPath.value = selection.path

  if (selection.range) {
    selectSourceRange(selection.range)
  }
}

const statusLabel = computed(() => {
  if (parseView.value.status === 'loading') {
    return 'Loading'
  }
  if (parseView.value.status === 'ok') {
    return 'Parsed'
  }
  if (parseView.value.status === 'invalid') {
    return 'Diagnostics'
  }
  return 'Error'
})

const statusTone = computed(() => ({
  'is-loading': parseView.value.status === 'loading',
  'is-ok': parseView.value.status === 'ok',
  'is-warn': parseView.value.status === 'invalid',
  'is-error': parseView.value.status === 'error'
}))

const resetSample = () => {
  source.value = sample
}

const getArrayLength = (value: unknown, key: string) => {
  if (!value || typeof value !== 'object') {
    return 0
  }
  const item = (value as Record<string, unknown>)[key]
  return Array.isArray(item) ? item.length : 0
}

const sumArrayLength = (values: unknown[], key: string) =>
  values.reduce((total, value) => total + getArrayLength(value, key), 0)
</script>

<template>
  <main class="playground" :data-theme="theme">
    <section class="topbar" aria-labelledby="title">
      <div>
        <p class="eyebrow">ox-jsdoc playground (wasm) v{{ oxJsdocWasmVersion }}</p>
        <h1 id="title" class="product-logo-text">JSDoc AST Explorer</h1>
        <p class="tagline">High performance jsdoc parser</p>
      </div>

      <div class="top-actions">
        <a
          class="github-link"
          href="https://github.com/kazupon/ox-jsdoc"
          target="_blank"
          rel="noreferrer"
          aria-label="Open ox-jsdoc on GitHub"
        >
          <svg aria-hidden="true" viewBox="0 0 16 16">
            <path
              fill="currentColor"
              d="M8 0C3.58 0 0 3.67 0 8.2c0 3.63 2.29 6.7 5.47 7.79.4.08.55-.18.55-.4 0-.2-.01-.86-.01-1.56-2.01.38-2.53-.5-2.69-.97-.09-.24-.48-.97-.82-1.17-.28-.16-.68-.55-.01-.56.63-.01 1.08.59 1.23.84.72 1.24 1.87.89 2.33.68.07-.53.28-.89.51-1.09-1.78-.21-3.64-.91-3.64-4.04 0-.89.31-1.62.82-2.19-.08-.21-.36-1.04.08-2.16 0 0 .67-.22 2.2.84A7.42 7.42 0 0 1 8 3.94c.68 0 1.36.09 2 .28 1.53-1.06 2.2-.84 2.2-.84.44 1.12.16 1.95.08 2.16.51.57.82 1.3.82 2.19 0 3.14-1.87 3.83-3.65 4.04.29.26.54.75.54 1.52 0 1.09-.01 1.97-.01 2.24 0 .22.15.48.55.4A8.09 8.09 0 0 0 16 8.2C16 3.67 12.42 0 8 0Z"
            />
          </svg>
          GitHub
        </a>
        <button
          type="button"
          class="theme-toggle"
          :aria-pressed="theme === 'dark'"
          @click="toggleTheme"
        >
          <span class="toggle-track">
            <span class="toggle-thumb" />
          </span>
          {{ theme === 'light' ? 'Light' : 'Dark' }}
        </button>

        <div class="status" :class="statusTone">
          <span>{{ statusLabel }}</span>
          <strong v-if="parseView.duration !== null">{{ parseView.duration.toFixed(2) }} ms</strong>
        </div>
      </div>
    </section>

    <section class="toolbar" aria-label="Parser options">
      <label>
        <input v-model="options.parseTypes" type="checkbox" />
        Parse types
      </label>
      <label>
        <input v-model="options.preserveWhitespace" type="checkbox" />
        Preserve whitespace
      </label>
      <label>
        <input v-model="options.compatMode" type="checkbox" />
        Compat mode
      </label>
      <label>
        <input v-model="options.parseBatch" type="checkbox" />
        Batch parse
      </label>
      <label>
        Type mode
        <select v-model="options.typeParseMode">
          <option value="jsdoc">JSDoc</option>
          <option value="closure">Closure</option>
          <option value="typescript">TypeScript</option>
        </select>
      </label>
      <button type="button" @click="resetSample">Reset sample</button>
    </section>

    <section class="workspace" aria-label="JSDoc AST explorer">
      <section class="pane source-pane">
        <div class="pane-title">
          <span>Source</span>
          <strong>{{ source.length }} chars</strong>
        </div>
        <div ref="editorHost" class="monaco-host" aria-label="JSDoc source" role="textbox" />
      </section>

      <section class="pane ast-pane">
        <div class="pane-title">
          <span>AST</span>
          <strong>{{ parseView.tagCount }} tags / {{ parseView.inlineTagCount }} inline</strong>
        </div>

        <div v-if="parseView.diagnostics.length > 0" class="diagnostics">
          <p v-for="diagnostic in parseView.diagnostics" :key="diagnostic">
            {{ diagnostic }}
          </p>
        </div>

        <AstTree
          :ast="parseView.ast"
          :reveal-path="revealAstPath"
          :reveal-version="revealAstVersion"
          :selected-path="selectedAstPath"
          @select="handleAstNodeSelect"
        />
      </section>
    </section>
  </main>
</template>

<style scoped>
.playground {
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr);
  gap: 14px;
  width: min(1440px, 100%);
  min-height: 100vh;
  margin: 0 auto;
  padding: 18px;
}

.topbar,
.toolbar,
.pane {
  border: 1px solid var(--line);
  background: var(--panel);
  box-shadow: 0 20px 60px rgba(35, 29, 20, 0.1);
}

.topbar {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 20px;
  padding: 22px 24px;
  border-radius: 26px;
}

.top-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 10px;
}

.eyebrow,
.pane-title,
.toolbar,
.status {
  font: 700 12px/1 var(--mono);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.eyebrow {
  margin: 0 0 10px;
  color: var(--accent);
}

.github-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 38px;
  padding: 8px 12px;
  border: 1px solid var(--line);
  border-radius: 999px;
  background: var(--panel-strong);
  color: var(--muted);
  font: 700 11px/1 var(--mono);
  letter-spacing: 0.1em;
  text-decoration: none;
  text-transform: uppercase;
}

.github-link:hover {
  border-color: var(--accent);
  color: var(--accent);
}

.github-link svg {
  width: 15px;
  height: 15px;
}

h1 {
  margin: 0;
  font-size: clamp(32px, 5vw, 58px);
  line-height: 0.96;
  letter-spacing: -0.055em;
  white-space: nowrap;
}

.product-logo-text {
  font-family: 'Montserrat', 'Arial Black', 'Helvetica Neue', sans-serif;
  font-weight: 900;
  letter-spacing: -0.04em;
  text-transform: uppercase;
}

.tagline {
  margin: 14px 0 0;
  color: var(--muted);
  font: 700 14px/1.2 var(--mono);
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.status {
  display: inline-flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border-radius: 999px;
  background: var(--panel-strong);
  color: var(--muted);
  white-space: nowrap;
}

.theme-toggle {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  min-height: 38px;
  padding: 8px 12px;
  border: 1px solid var(--line);
  border-radius: 999px;
  background: var(--panel-strong);
  color: var(--ink);
  font: 700 12px/1 var(--mono);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.toggle-track {
  position: relative;
  width: 38px;
  height: 20px;
  border-radius: 999px;
  background: var(--line);
}

.toggle-thumb {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--accent);
  transition: transform 0.2s ease;
}

.theme-toggle[aria-pressed='true'] .toggle-thumb {
  transform: translateX(18px);
}

.status::before {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  content: '';
}

.status.is-loading::before {
  background: #b08b54;
}

.status.is-ok::before {
  background: #27734c;
}

.status.is-warn::before {
  background: #c08222;
}

.status.is-error::before {
  background: #b5312e;
}

.status strong {
  color: var(--ink);
}

.toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  padding: 12px;
  border-radius: 18px;
  color: var(--muted);
}

.toolbar label,
.toolbar button {
  min-height: 38px;
  border: 1px solid var(--line);
  border-radius: 999px;
  background: var(--panel-strong);
}

.toolbar label {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 9px 12px;
}

.toolbar input,
.toolbar select {
  accent-color: var(--accent);
}

.toolbar select {
  border: none;
  background: transparent;
  color: var(--ink);
  font: inherit;
}

.toolbar button {
  padding: 9px 14px;
}

button {
  color: var(--ink);
  cursor: pointer;
  font: inherit;
}

button:hover {
  border-color: var(--accent);
  color: var(--accent);
}

button:focus-visible {
  outline: 3px solid var(--accent-soft);
  outline-offset: 3px;
}

.workspace {
  display: grid;
  grid-template-columns: minmax(0, 0.95fr) minmax(0, 1.05fr);
  gap: 14px;
  min-height: 0;
}

.pane {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  min-width: 0;
  min-height: 660px;
  overflow: hidden;
  border-radius: 24px;
}

.pane-title {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--line);
  color: var(--muted);
}

.pane-title strong {
  color: var(--ink);
}

.ast-title {
  color: var(--ast-title);
}

.ast-title strong,
.ast-title span {
  color: var(--ast-title);
}

.monaco-host {
  width: 100%;
  height: 100%;
  margin: 0;
  border: 0;
  background: var(--panel-strong);
  color: var(--ink);
  font: 14px/1.55 var(--mono);
}

.monaco-host {
  min-height: 0;
  overflow: hidden;
}

.ast-pane {
  position: relative;
}

.diagnostics {
  border-bottom: 1px solid var(--line);
  background: rgba(255, 226, 190, 0.6);
  color: #7e2f1e;
  font: 13px/1.5 var(--mono);
}

.diagnostics p {
  margin: 0;
  padding: 10px 16px;
}

@media (max-width: 980px) {
  .playground {
    padding: 12px;
  }

  .topbar {
    align-items: start;
    flex-direction: column;
  }

  .workspace {
    grid-template-columns: 1fr;
  }

  .pane {
    min-height: 460px;
  }
}
</style>
