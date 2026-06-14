<script setup lang="ts">
import AstTree from './AstTree.vue'
import type { AstSelection, ParseView } from '../types/playground'

defineProps<{
  parseView: ParseView
  revealPath: string
  revealVersion: number
  selectedPath: string
}>()

const emit = defineEmits<{
  select: [selection: AstSelection]
}>()
</script>

<template>
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
      :reveal-path="revealPath"
      :reveal-version="revealVersion"
      :selected-path="selectedPath"
      @select="emit('select', $event)"
    />
  </section>
</template>

<style scoped>
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
</style>
