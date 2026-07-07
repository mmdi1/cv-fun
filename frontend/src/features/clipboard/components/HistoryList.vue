<script setup lang="ts">
import type { clipboardRecord } from '../../../../bindings/changeme/models'
import HistoryItem from './HistoryItem.vue'

defineProps<{
  items: clipboardRecord[]
  selectedId: string
  hasLoaded: boolean
}>()

defineEmits<{
  select: [item: clipboardRecord]
  focusItem: [id: string]
  deleteItem: [id: string]
}>()
</script>

<template>
  <div class="history-list" aria-live="polite">
    <article v-if="items.length === 0 && hasLoaded" class="empty-state">
      <h2>还没有历史</h2>
      <p>复制任意文本后，这里会自动出现记录。</p>
    </article>

    <HistoryItem
      v-for="item in items"
      :key="item.id"
      :item="item"
      :selected="item.id === selectedId"
      @select="$emit('select', $event)"
      @focus-item="$emit('focusItem', $event)"
      @delete-item="$emit('deleteItem', $event)"
    />
  </div>
</template>

<style scoped>
.history-list {
  display: grid;
  gap: 6px;
  min-height: 0;
  margin-top: 10px;
  overflow: auto;
}

.empty-state {
  border: 1px solid #252b38;
  border-radius: 8px;
  background: #151a24;
  padding: 24px;
  text-align: center;
}

.empty-state h2 {
  margin: 0 0 8px;
  font-size: 17px;
}

.empty-state p {
  margin: 0;
  color: #98a2b3;
  font-size: 13px;
}
</style>
