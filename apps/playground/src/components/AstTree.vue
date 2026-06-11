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
  background: var(--panel-strong);
  color: var(--ink);
  font: 16px/1.65 var(--mono);
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
