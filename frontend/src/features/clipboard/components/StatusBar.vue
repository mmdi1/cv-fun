<script setup lang="ts">
import { formatDateTime } from '../analyzer'

defineProps<{
  loading: boolean
  notice: string
  error: string
  lastLoadedAt: Date | null
}>()
</script>

<template>
  <footer class="statusbar">
    <span v-if="error" class="error-message">{{ error }}</span>
    <span v-else-if="notice" class="success-message">{{ notice }}</span>
    <span v-else-if="loading">加载中...</span>
    <span v-else-if="lastLoadedAt">最后刷新 {{ formatDateTime(lastLoadedAt.toISOString()) }}</span>
    <span v-else>等待剪贴板变化</span>
    <span class="storage-path">~/Library/Application Support/cv-fun/clipboard-history.json</span>
  </footer>
</template>

<style scoped>
.statusbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-height: 28px;
  margin-top: 11px;
  color: #98a2b3;
  font-size: 13px;
  flex: 0 0 auto;
}

.error-message {
  color: #fecaca;
}

.success-message {
  color: #bbf7d0;
}

.storage-path {
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
