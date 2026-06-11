<script setup lang="ts">
import AstTreeNode from './AstTreeNode.vue'

type SourceRange = [number, number]

type AstSelection = {
  path: string
  range: SourceRange | null
  value: unknown
}

defineProps<{
  ast: unknown
  revealPath: string
  revealVersion: number
  selectedPath: string
}>()

const emit = defineEmits<{
  select: [selection: AstSelection]
}>()
</script>

<template>
  <div v-if="ast" class="ast-tree" role="tree" aria-label="JSDoc AST tree">
    <AstTreeNode
      name="root"
      path="root"
      root
      :reveal-path="revealPath"
      :reveal-version="revealVersion"
      :selected-path="selectedPath"
      :value="ast"
      @select="emit('select', $event)"
    />
  </div>
  <div v-else class="ast-empty">No AST available.</div>
</template>

<style scoped>
.ast-tree,
.ast-empty {
  width: 100%;
  height: 100%;
  background: var(--editor-bg);
  color: var(--ink);
  font-family: var(--mono);
  font-size: 14px;
  line-height: 1.55;
}

.ast-tree {
  min-height: 420px;
  overflow: auto;
  padding: 26px 28px 34px;
}

.ast-empty {
  display: grid;
  min-height: 420px;
  place-items: center;
  color: var(--muted);
}
</style>
