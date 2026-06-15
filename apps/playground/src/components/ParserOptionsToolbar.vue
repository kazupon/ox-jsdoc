<script setup lang="ts">
import { useId } from 'vue'

import type { TypeParseMode } from '../types/playground'

const id = useId()
const titleId = `${id}-parser-options-title`
const typeModeId = `${id}-parser-type-mode`

defineProps<{
  compatMode: boolean
  parseBatch: boolean
  parseTypes: boolean
  preserveWhitespace: boolean
  typeParseMode: TypeParseMode
}>()

const emit = defineEmits<{
  'update:compatMode': [value: boolean]
  'update:parseBatch': [value: boolean]
  'update:parseTypes': [value: boolean]
  'update:preserveWhitespace': [value: boolean]
  'update:typeParseMode': [value: TypeParseMode]
}>()

function getChecked(event: Event): boolean {
  return (event.target as HTMLInputElement).checked
}

function getTypeParseMode(event: Event): TypeParseMode {
  return (event.target as HTMLSelectElement).value as TypeParseMode
}

function handleParseTypesChange(event: Event): void {
  emit('update:parseTypes', getChecked(event))
}

function handlePreserveWhitespaceChange(event: Event): void {
  emit('update:preserveWhitespace', getChecked(event))
}

function handleCompatModeChange(event: Event): void {
  emit('update:compatMode', getChecked(event))
}

function handleParseBatchChange(event: Event): void {
  emit('update:parseBatch', getChecked(event))
}

function handleTypeParseModeChange(event: Event): void {
  emit('update:typeParseMode', getTypeParseMode(event))
}
</script>

<template>
  <section class="toolbar" :aria-labelledby="titleId">
    <div :id="titleId" class="toolbar-label">
      <span>parser options</span>
      <small>parse / parseBatch settings</small>
    </div>
    <label>
      <input :checked="parseTypes" type="checkbox" @change="handleParseTypesChange" />
      Parse types
    </label>
    <label>
      <input
        :checked="preserveWhitespace"
        type="checkbox"
        @change="handlePreserveWhitespaceChange"
      />
      Preserve whitespace
    </label>
    <label>
      <input :checked="compatMode" type="checkbox" @change="handleCompatModeChange" />
      Compat mode
    </label>
    <label>
      <input :checked="parseBatch" type="checkbox" @change="handleParseBatchChange" />
      Batch parse
    </label>
    <label :for="typeModeId">Type mode</label>
    <select :id="typeModeId" :value="typeParseMode" @change="handleTypeParseModeChange">
      <option value="jsdoc">JSDoc</option>
      <option value="closure">Closure</option>
      <option value="typescript">TypeScript</option>
    </select>
  </section>
</template>

<style scoped>
.toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  padding: 12px;
  border: 1px solid var(--line);
  border-radius: 18px;
  background: var(--panel);
  box-shadow: 0 20px 60px rgba(35, 29, 20, 0.1);
  color: var(--muted);
  font: 700 12px/1 var(--sans);
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.toolbar-label {
  display: inline-flex;
  flex-direction: column;
  gap: 4px;
  min-height: 38px;
  justify-content: center;
  padding: 0 8px 0 4px;
  color: var(--accent);
}

.toolbar-label span {
  color: var(--accent);
  white-space: nowrap;
}

.toolbar-label small {
  color: var(--muted);
  font: 600 10px/1 var(--sans);
  letter-spacing: 0.08em;
  text-transform: uppercase;
  white-space: nowrap;
}

.toolbar label {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 38px;
  padding: 9px 12px;
  border: 1px solid var(--line);
  border-radius: 999px;
  background: var(--panel-strong);
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
</style>
