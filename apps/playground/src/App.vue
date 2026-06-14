<script setup lang="ts">
import { computed, ref } from 'vue'
import AstPane from './components/AstPane.vue'
import ParserOptionsToolbar from './components/ParserOptionsToolbar.vue'
import PlaygroundHeader from './components/PlaygroundHeader.vue'
import SourcePane from './components/SourcePane.vue'
import { useJsdocParser } from './composables/useJsdocParser'
import { useMonacoSourceEditor } from './composables/useMonacoSourceEditor'
import { usePlaygroundSettings } from './composables/usePlaygroundSettings'
import type { AstSelection, SourceRange } from './types/playground'
import oxJsdocWasmPackage from '../../../wasm/ox-jsdoc/package.json' with { type: 'json' }

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
const { options, theme, toggleTheme } = usePlaygroundSettings()
const sourceLanguage = computed(() =>
  options.typeParseMode === 'typescript' ? 'typescript' : 'javascript'
)
const { parseView } = useJsdocParser({ options, source, sourceLanguage })
const { highlightSourceRange, setEditorHost } = useMonacoSourceEditor({
  onSourceOffsetClick: revealAstNodeAtOffset,
  source,
  sourceLanguage,
  theme
})

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

const handleAstNodeSelect = (selection: AstSelection) => {
  selectedAstPath.value = selection.path

  if (selection.range) {
    highlightSourceRange(selection.range)
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
</script>

<template>
  <main class="playground" :data-theme="theme">
    <PlaygroundHeader
      :duration="parseView.duration"
      :status-label="statusLabel"
      :status-tone="statusTone"
      :theme="theme"
      :version="oxJsdocWasmVersion"
      @toggle-theme="toggleTheme"
    />

    <ParserOptionsToolbar
      v-model:compat-mode="options.compatMode"
      v-model:parse-batch="options.parseBatch"
      v-model:parse-types="options.parseTypes"
      v-model:preserve-whitespace="options.preserveWhitespace"
      v-model:type-parse-mode="options.typeParseMode"
    />

    <section class="workspace" aria-label="JSDoc AST explorer">
      <SourcePane
        :set-editor-host="setEditorHost"
        :source-length="source.length"
        @reset="resetSample"
      />

      <AstPane
        :parse-view="parseView"
        :reveal-path="revealAstPath"
        :reveal-version="revealAstVersion"
        :selected-path="selectedAstPath"
        @select="handleAstNodeSelect"
      />
    </section>
  </main>
</template>

<style scoped>
.playground {
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr);
  gap: 14px;
  width: min(1440px, 100%);
  height: 100dvh;
  min-height: 0;
  margin: 0 auto;
  overflow: hidden;
  padding: 18px;
}

.workspace {
  display: grid;
  grid-template-columns: minmax(0, 0.95fr) minmax(0, 1.05fr);
  gap: 14px;
  min-height: 0;
  overflow: hidden;
}

@media (max-width: 980px) {
  .playground {
    padding: 12px;
  }

  .workspace {
    grid-template-columns: 1fr;
  }
}
</style>
