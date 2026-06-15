<script setup lang="ts">
const { setEditorHost } = defineProps<{
  setEditorHost(element: Element | null): void
  sourceLength: number
}>()

const emit = defineEmits<{
  reset: []
}>()

function handleReset(): void {
  emit('reset')
}
</script>

<template>
  <section class="pane source-pane">
    <div class="pane-title">
      <span>Source</span>
      <div class="pane-title-actions">
        <strong>{{ sourceLength }} chars</strong>
        <button type="button" class="pane-action" @click="handleReset">Reset sample</button>
      </div>
    </div>
    <div :ref="setEditorHost" class="monaco-host" aria-label="JSDoc source" role="textbox" />
  </section>
</template>

<style scoped>
.monaco-host {
  width: 100%;
  height: 100%;
  min-height: 0;
  margin: 0;
  overflow: hidden;
  border: 0;
  background: var(--panel-strong);
  color: var(--ink);
  font: 14px/1.55 var(--mono);
}
</style>
