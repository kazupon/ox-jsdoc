/**
 * Monaco source editor integration for the ox-jsdoc playground.
 *
 * @author kazuya kawaguchi (a.k.a. kazupon)
 * @license MIT
 */

import {
  computed,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  watch,
  type ComputedRef,
  type Ref
} from 'vue'
// oxlint-disable-next-line eslint-plugin-import/default -- NOTE(monaco): This import is for side effects (registering the worker), so the default export is not used.
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import * as monaco from 'monaco-editor/esm/vs/editor/editor.api'
import 'monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution'
import 'monaco-editor/esm/vs/basic-languages/typescript/typescript.contribution'
import type { PlaygroundTheme, SourceRange } from '../types/playground'

const globalSelf = globalThis as typeof globalThis & {
  MonacoEnvironment?: {
    getWorker(): Worker
  }
}

globalSelf.MonacoEnvironment = {
  getWorker() {
    return new editorWorker()
  }
}

let editorThemesDefined = false

function defineEditorThemes(): void {
  if (editorThemesDefined) {
    return
  }

  editorThemesDefined = true

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

type UseMonacoSourceEditorOptions = {
  onSourceOffsetClick: (offset: number) => void
  handleSourceOffsetClick(offset: number): void
  source: Ref<string>
  sourceLanguage: ComputedRef<'javascript' | 'typescript'>
  theme: Ref<PlaygroundTheme>
}

export function useMonacoSourceEditor({
  source,
  sourceLanguage,
  theme,
  onSourceOffsetClick: handleSourceOffsetClick
}: UseMonacoSourceEditorOptions) {
  const editorHost = ref<HTMLElement | null>(null)
  const sourceEditor = shallowRef<monaco.editor.IStandaloneCodeEditor | null>(null)
  const sourceHighlightDecorations = shallowRef<monaco.editor.IEditorDecorationsCollection | null>(
    null
  )
  let resizeObserver: ResizeObserver | null = null

  const editorTheme = computed(() => `ox-jsdoc-${theme.value}`)

  const setEditorHost = (element: Element | null): void => {
    editorHost.value = element instanceof HTMLElement ? element : null
  }

  const highlightSourceRange = (range: SourceRange): void => {
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

    sourceHighlightDecorations.value?.clear()
    sourceHighlightDecorations.value = editor.createDecorationsCollection([
      {
        range: new monaco.Range(start.lineNumber, start.column, end.lineNumber, end.column),
        options: {
          inlineClassName: 'source-range-highlight',
          stickiness: monaco.editor.TrackedRangeStickiness.NeverGrowsWhenTypingAtEdges
        }
      }
    ])
  }

  onMounted(() => {
    defineEditorThemes()
    document.documentElement.dataset.theme = theme.value

    if (editorHost.value) {
      sourceEditor.value = monaco.editor.create(editorHost.value, {
        automaticLayout: true,
        fontFamily: 'Menlo, Monaco, "SFMono-Regular", Consolas, "Liberation Mono", monospace',
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

        handleSourceOffsetClick(model.getOffsetAt(position))
      })

      resizeObserver = new ResizeObserver(() => {
        sourceEditor.value?.layout()
      })
      resizeObserver.observe(editorHost.value)
    }
  })

  onBeforeUnmount(() => {
    resizeObserver?.disconnect()
    sourceHighlightDecorations.value?.clear()
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

  return {
    highlightSourceRange,
    setEditorHost
  }
}
