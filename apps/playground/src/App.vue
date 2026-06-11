<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, shallowRef, watch } from 'vue'
import AstTree from './components/AstTree.vue'
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import * as monaco from 'monaco-editor/esm/vs/editor/editor.api'
import 'monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution'
import 'monaco-editor/esm/vs/basic-languages/typescript/typescript.contribution'
import { initWasm, parse } from '@ox-jsdoc/wasm'

type TypeParseMode = 'jsdoc' | 'closure' | 'typescript'

type PlaygroundSettings = {
  compatMode: boolean
  parseTypes: boolean
  preserveWhitespace: boolean
  theme: 'light' | 'dark'
  typeParseMode: TypeParseMode
}

type ExtractedJsdocBlock = {
  baseOffset: number
  sourceText: string
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
}`

const source = ref(sample)
const selectedAstPath = ref('')
const wasmReady = ref(false)
const wasmError = ref<string | null>(null)
const editorHost = ref<HTMLElement | null>(null)
const sourceEditor = shallowRef<monaco.editor.IStandaloneCodeEditor | null>(null)
let resizeObserver: ResizeObserver | null = null
const settingsKey = 'ox-jsdoc.playground.settings'
const defaultSettings: PlaygroundSettings = {
  compatMode: false,
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
      { token: 'keyword', foreground: 'c24b2c', fontStyle: 'bold' },
      { token: 'string', foreground: '28704f' },
      { token: 'number', foreground: '1f6f8b' },
      { token: 'regexp', foreground: 'a93678' },
      { token: 'type', foreground: '1f6f8b' },
      { token: 'delimiter', foreground: '8b6f4e' },
      { token: 'comment.jsdoc', foreground: '7a6653' },
      { token: 'comment.block.jsdoc', foreground: '9a8165' },
      { token: 'comment.marker.jsdoc', foreground: 'b9a88e' },
      { token: 'tag.jsdoc', foreground: 'c24b2c', fontStyle: 'bold' },
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
      'editorLineNumber.activeForeground': '#c24b2c',
      'editor.selectionBackground': '#f4d2bd',
      'editorCursor.foreground': '#c24b2c'
    }
  })

  monaco.editor.defineTheme('ox-jsdoc-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: 'b8a991' },
      { token: 'comment.doc', foreground: 'b8a991' },
      { token: 'keyword', foreground: 'ff8f70', fontStyle: 'bold' },
      { token: 'string', foreground: '97d988' },
      { token: 'number', foreground: '6fd1c7' },
      { token: 'regexp', foreground: 'f0a7d3' },
      { token: 'type', foreground: '6fd1c7' },
      { token: 'delimiter', foreground: 'b8a991' },
      { token: 'comment.jsdoc', foreground: 'b8a991' },
      { token: 'comment.block.jsdoc', foreground: '887b68' },
      { token: 'comment.marker.jsdoc', foreground: '6f6557' },
      { token: 'tag.jsdoc', foreground: 'ff8f70', fontStyle: 'bold' },
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
      'editorLineNumber.activeForeground': '#ff8f70',
      'editor.selectionBackground': '#623829',
      'editorCursor.foreground': '#ff8f70'
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
  compatMode: settings.compatMode
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

function extractFirstJsdocBlock(value: string): ExtractedJsdocBlock | null {
  const start = value.indexOf('/**')

  if (start === -1) {
    return null
  }

  let index = start + 3

  while (index < value.length) {
    if (value[index] === '*' && value[index + 1] === '/') {
      return {
        baseOffset: start,
        sourceText: value.slice(start, index + 2)
      }
    }

    index += 1
  }

  return {
    baseOffset: start,
    sourceText: value.slice(start)
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

    resizeObserver = new ResizeObserver(() => {
      sourceEditor.value?.layout()
    })
    resizeObserver.observe(editorHost.value)
  }

  try {
    await initWasm()
    wasmReady.value = true
  } catch (error) {
    wasmError.value = error instanceof Error ? error.message : String(error)
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
    () => options.parseTypes,
    () => options.preserveWhitespace,
    () => options.typeParseMode
  ],
  saveSettings
)

const parseView = computed<ParseView>(() => {
  if (!wasmReady.value) {
    const failed = wasmError.value !== null

    return {
      status: failed ? 'error' : 'loading',
      ast: null,
      diagnostics: [wasmError.value ?? 'Initializing WASM parser...'],
      duration: null,
      tagCount: 0,
      inlineTagCount: 0
    }
  }

  const jsdocBlock = extractFirstJsdocBlock(source.value)

  if (!jsdocBlock) {
    return {
      status: 'invalid',
      ast: null,
      diagnostics: ['No JSDoc block found. Add a /** ... */ block comment.'],
      duration: 0,
      tagCount: 0,
      inlineTagCount: 0
    }
  }

  const start = performance.now()
  const result = parse(jsdocBlock.sourceText, {
    baseOffset: jsdocBlock.baseOffset,
    compatMode: options.compatMode,
    parseTypes: options.parseTypes,
    preserveWhitespace: options.preserveWhitespace,
    typeParseMode: options.typeParseMode
  })

  try {
    const ast = result.ast?.toJSON() ?? null
    const diagnostics = result.diagnostics.map(diagnostic => diagnostic.message)

    return {
      status: result.ast && diagnostics.length === 0 ? 'ok' : 'invalid',
      ast,
      diagnostics,
      duration: performance.now() - start,
      tagCount: getArrayLength(ast, 'tags'),
      inlineTagCount: getArrayLength(ast, 'inlineTags')
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
})

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
</script>

<template>
  <main class="playground" :data-theme="theme">
    <section class="topbar" aria-labelledby="title">
      <div>
        <p class="eyebrow">@ox-jsdoc/wasm playground</p>
        <h1 id="title">JSDoc AST Explorer</h1>
      </div>

      <div class="top-actions">
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

h1 {
  margin: 0;
  font-size: clamp(34px, 6vw, 72px);
  line-height: 0.92;
  letter-spacing: -0.06em;
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
