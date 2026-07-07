<script setup lang="ts">
import type { clipboardRecord } from '../../../../bindings/changeme/models'
import { formatHistoryTime } from '../analyzer'

defineProps<{
  item: clipboardRecord
  selected: boolean
}>()

defineEmits<{
  select: [item: clipboardRecord]
  focusItem: [id: string]
  deleteItem: [id: string]
}>()
</script>

<template>
  <article
    class="history-item"
    :class="{ 'is-selected': selected }"
    tabindex="0"
    @click="$emit('select', item)"
    @focus="$emit('focusItem', item.id)"
    @keydown.enter.prevent="$emit('select', item)"
  >
    <div class="item-main">
      <div class="item-meta">
        <span v-if="item.pinned" class="pin">置顶</span>
        <span>{{ item.useCount }} 次</span>
      </div>
      <p class="item-preview">{{ item.preview || item.text }}</p>
    </div>
    <div class="item-side">
      <time>{{ formatHistoryTime(item.copiedAt) }}</time>
      <button type="button" class="delete-button" @click.stop="$emit('deleteItem', item.id)">删除</button>
    </div>
  </article>
</template>

<style scoped>
.history-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  border: 1px solid #252b38;
  border-radius: 8px;
  background: #141923;
  padding: 9px 9px 9px 11px;
  cursor: pointer;
  outline: none;
  transition: border-color 0.14s ease, background 0.14s ease;
  --wails-draggable: no-drag;
}

.history-item:hover,
.history-item:focus,
.history-item.is-selected {
  border-color: #4b5565;
  background: #1a202d;
}

.item-main {
  flex: 1 1 auto;
  min-width: 0;
}

.item-meta {
  display: flex;
  gap: 7px;
  margin-bottom: 4px;
  color: #98a2b3;
  font-size: 11px;
}

.pin {
  color: #fde68a;
}

.item-preview {
  display: -webkit-box;
  overflow: hidden;
  margin: 0;
  color: #f2f4f8;
  font-size: 13px;
  line-height: 1.36;
  word-break: break-word;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.item-side {
  display: grid;
  flex: 0 0 76px;
  justify-items: end;
  align-self: stretch;
  align-content: space-between;
  gap: 8px;
}

.item-side time {
  color: #98a2b3;
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.delete-button {
  min-width: 44px;
  border-color: #2a3140;
  padding: 5px 7px;
  color: #aeb8c8;
  font-size: 11px;
}
</style>
