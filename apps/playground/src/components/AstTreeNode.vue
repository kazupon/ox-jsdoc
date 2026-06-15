<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { AstSelection, SourceRange } from '../types/playground'

defineOptions({
  name: 'AstTreeNode'
})

type AstChild = {
  name: string
  path: string
  value: unknown
}

const props = withDefaults(
  defineProps<{
    depth?: number
    name: string
    path: string
    revealPath?: string
    revealVersion?: number
    root?: boolean
    selectedPath?: string
    value: unknown
  }>(),
  {
    depth: 0,
    revealPath: '',
    revealVersion: 0,
    root: false,
    selectedPath: ''
  }
)

const emit = defineEmits<{
  select: [selection: AstSelection]
}>()

const children = computed(() => getAstChildren(props.value, props.path))
const openable = computed(() => children.value.length > 0)
const openManual = ref<boolean>()
const open = computed(() => openable.value && (openManual.value ?? props.root))
const valueCreated = ref(false)
const rowElement = ref<HTMLElement | null>(null)
const range = computed(() => getAstRange(props.value))
const title = computed(() => getAstTitle(props.value))
const preview = computed(() => getAstPreview(props.value))
const valueClass = computed(() => getAstValueClass(props.value))
const brackets = computed(() => getAstBrackets(props.value))
const rowStyle = computed(() => ({ marginLeft: `${props.depth * 28 + 22}px` }))

watch(
  open,
  value => {
    valueCreated.value ||= value
  },
  { immediate: true }
)

watch(
  () => [props.revealPath, props.revealVersion] as const,
  ([revealPath]) => {
    if (openable.value && isPathAncestorOrSelf(props.path, revealPath)) {
      openManual.value = true
    }

    if (props.path === revealPath) {
      rowElement.value?.scrollIntoView({
        behavior: 'smooth',
        block: 'nearest',
        inline: 'nearest'
      })
    }
  },
  { immediate: true, flush: 'post' }
)

function selectNode(): void {
  emit('select', {
    path: props.path,
    range: range.value,
    value: props.value
  })
}

function handleChildSelect(selection: unknown): void {
  emit('select', selection as AstSelection)
}

function toggleOpen(): void {
  if (!openable.value) {
    return
  }

  openManual.value = !open.value
}

function handleKeyClick(event: MouseEvent): void {
  if (!openable.value) {
    return
  }

  event.stopPropagation()
  selectNode()
  toggleOpen()
}

function handleSummaryClick(): void {
  selectNode()
  toggleOpen()
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    selectNode()
  }

  if (event.key === 'ArrowRight' && openable.value) {
    openManual.value = true
  }

  if (event.key === 'ArrowLeft' && openable.value) {
    openManual.value = false
  }
}

function isPathAncestorOrSelf(path: string, targetPath: string): boolean {
  return targetPath === path || targetPath.startsWith(`${path}.`)
}

function isAstRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function getAstChildren(value: unknown, path: string): AstChild[] {
  if (Array.isArray(value)) {
    return value.map((item, index) => ({
      name: String(index),
      path: `${path}.${index}`,
      value: item
    }))
  }

  if (!isAstRecord(value)) {
    return []
  }

  return Object.entries(value).map(([name, item]) => ({
    name,
    path: `${path}.${name}`,
    value: item
  }))
}

function getAstRange(value: unknown): SourceRange | null {
  if (Array.isArray(value)) {
    return value.length === 2 && typeof value[0] === 'number' && typeof value[1] === 'number'
      ? [value[0], value[1]]
      : null
  }

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
    return [sourceRange[0], sourceRange[1]]
  }

  return typeof value.start === 'number' && typeof value.end === 'number'
    ? [value.start, value.end]
    : null
}

function getAstPreview(value: unknown): string {
  if (Array.isArray(value) || isAstRecord(value)) {
    return getAstSummary(value)
  }

  return getAstLiteralPreview(value)
}

function getAstLiteralPreview(value: unknown): string {
  if (typeof value === 'string') {
    return JSON.stringify(value)
  }

  return value === null ? 'null' : String(value)
}

function getAstTitle(value: unknown): string {
  if (Array.isArray(value)) {
    return ''
  }

  if (isAstRecord(value)) {
    return typeof value.type === 'string' ? value.type : 'Object'
  }

  return ''
}

function getAstSummary(value: unknown): string {
  if (Array.isArray(value)) {
    return `${value.length} ${value.length === 1 ? 'element' : 'elements'}`
  }

  if (isAstRecord(value)) {
    const keys = Object.keys(value)
    return `${keys.slice(0, 5).join(', ')}${keys.length > 5 ? `, ... +${keys.length - 5}` : ''}`
  }

  return getAstLiteralPreview(value)
}

function getAstBrackets(value: unknown): [string, string] {
  return Array.isArray(value) ? ['[', ']'] : ['{', '}']
}

function getAstValueClass(value: unknown): string {
  if (Array.isArray(value)) {
    return 'is-array'
  }

  if (isAstRecord(value)) {
    return 'is-object'
  }

  if (value === null) {
    return 'is-null'
  }

  return `is-${typeof value}`
}
</script>

<template>
  <div class="ast-node">
    <div
      ref="rowElement"
      class="ast-node-row"
      :aria-expanded="openable ? String(open) : undefined"
      :class="{
        'has-range': range !== null,
        'is-selected': selectedPath === path
      }"
      :style="rowStyle"
      role="treeitem"
      tabindex="0"
      @click="selectNode"
      @keydown="handleKeydown"
    >
      <button
        v-if="openable"
        type="button"
        class="ast-toggle"
        :class="{ 'is-open': open }"
        :aria-label="`${open ? 'Collapse' : 'Expand'} ${name}`"
        @click.stop="toggleOpen"
      >
        {{ open ? '-' : '+' }}
      </button>

      <span
        v-if="!root"
        class="ast-key"
        :class="{ 'is-openable': openable }"
        @click="handleKeyClick"
      >
        {{ name }}
      </span>
      <span v-if="!root" class="ast-punctuation">:</span>

      <template v-if="openable">
        <span v-if="title && !root" class="ast-title">{{ title }}</span>
        <span class="ast-bracket">{{ brackets[0] }}</span>
        <button v-if="!open" type="button" class="ast-summary" @click.stop="handleSummaryClick">
          {{ preview }}
        </button>
        <span v-if="!open" class="ast-bracket">{{ brackets[1] }}</span>
      </template>

      <template v-else>
        <span class="ast-value" :class="valueClass">{{ preview }}</span>
        <span v-if="!root" class="ast-comma">,</span>
      </template>
    </div>

    <div v-if="openable && open && valueCreated" class="ast-children" role="group">
      <AstTreeNode
        v-for="child in children"
        :key="child.path"
        :depth="depth + 1"
        :name="child.name"
        :path="child.path"
        :reveal-path="revealPath"
        :reveal-version="revealVersion"
        :selected-path="selectedPath"
        :value="child.value"
        @select="handleChildSelect"
      />
      <div class="ast-bracket-row" :style="rowStyle">{{ brackets[1] }}</div>
    </div>
  </div>
</template>

<style scoped>
.ast-node-row {
  position: relative;
  display: flex;
  align-items: baseline;
  gap: 0;
  width: fit-content;
  min-height: 28px;
  padding: 0;
  cursor: pointer;
  font-family: var(--mono);
  font-size: 14px;
  line-height: 1.55;
  white-space: nowrap;
}

.ast-node-row:hover,
.ast-node-row:focus-visible {
  outline: none;
}

.ast-node-row.is-selected .ast-key,
.ast-node-row.is-selected .ast-title,
.ast-node-row.is-selected .ast-value,
.ast-node-row.is-selected .ast-summary {
  border-radius: 4px;
  background: rgba(111, 209, 199, 0.18);
}

.ast-toggle {
  position: absolute;
  top: 0;
  left: -22px;
  display: inline-grid;
  width: 16px;
  height: 28px;
  place-items: center;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  cursor: pointer;
  font: inherit;
  font-weight: 800;
  line-height: 1;
  opacity: 0.85;
}

.ast-key,
.ast-title,
.ast-value,
.ast-summary,
.ast-punctuation,
.ast-bracket,
.ast-comma,
.ast-bracket-row {
  font-family: var(--mono);
}

.ast-toggle:hover {
  color: var(--ast-title);
}

.ast-toggle.is-open {
  color: var(--ast-number);
}

.ast-toggle.is-open:hover {
  color: var(--ast-title);
}

.ast-key {
  color: var(--ast-key);
  font-weight: 700;
}

.ast-key.is-openable,
.ast-summary {
  cursor: pointer;
}

.ast-key.is-openable:hover,
.ast-summary:hover {
  text-decoration: underline;
  text-underline-offset: 4px;
}

.ast-punctuation,
.ast-bracket,
.ast-comma {
  color: var(--muted);
}

.ast-punctuation,
.ast-bracket,
.ast-title,
.ast-summary {
  margin-right: 0.35em;
}

.ast-title {
  color: var(--ast-title);
  font-weight: 700;
}

.ast-summary {
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--muted);
  font: inherit;
  font-style: italic;
}

.ast-bracket-row {
  min-height: 24px;
  color: var(--ink);
  white-space: nowrap;
}

.ast-value.is-object,
.ast-value.is-array {
  color: var(--ink);
}

.ast-value.is-string {
  color: var(--ast-string);
}

.ast-value.is-number {
  color: var(--ast-number);
}

.ast-value.is-boolean {
  color: var(--ast-boolean);
}

.ast-value.is-null,
.ast-value.is-undefined {
  color: var(--muted);
  font-style: italic;
}
</style>
