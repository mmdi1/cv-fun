<script setup lang="ts">
import type { AnalysisCard } from '../analyzer'

defineProps<{
  cards: AnalysisCard[]
}>()
</script>

<template>
  <div class="analysis-panel">
    <div class="analysis-head">
      <strong>自动解析</strong>
      <span>{{ cards.length }} 项</span>
    </div>
    <article v-for="card in cards" :key="`${card.type}-${card.hint}`" class="analysis-card">
      <span class="analysis-title">{{ card.title }}</span>
      <pre>{{ card.value }}</pre>
      <small>{{ card.hint }}</small>
    </article>
  </div>
</template>

<style scoped>
.analysis-panel {
  display: grid;
  gap: 8px;
  margin-top: 12px;
  border-top: 1px solid #252b38;
  padding-top: 12px;
}

.analysis-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  color: #98a2b3;
  font-size: 13px;
}

.analysis-head strong {
  color: #dbeafe;
}

.analysis-card {
  display: grid;
  gap: 5px;
  border: 1px solid #252b38;
  border-radius: 8px;
  background: #111722;
  padding: 9px;
}

.analysis-title {
  color: #f6f7fb;
  font-size: 13px;
  font-weight: 600;
}

.analysis-card pre {
  max-height: 132px;
  margin: 0;
  overflow: auto;
  color: #dbeafe;
  white-space: pre-wrap;
  word-break: break-word;
  font: 12px/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

.analysis-card small {
  color: #98a2b3;
}
</style>
