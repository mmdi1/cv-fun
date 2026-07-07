<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { Events } from '@wailsio/runtime'
import { ClipboardService } from '../../../bindings/changeme'
import type { clipboardRecord } from '../../../bindings/changeme/models'
import AnalysisPanel from './components/AnalysisPanel.vue'
import HistoryList from './components/HistoryList.vue'
import StatusBar from './components/StatusBar.vue'
import SettingsPanel from '../settings/SettingsPanel.vue'
import { analyzeClipboardText, formatDateTime } from './analyzer'
import { resolveSelectedHistoryId } from './selection'

const query = ref('')
const items = ref<clipboardRecord[]>([])
const loading = ref(false)
const hasLoaded = ref(false)
const errorMessage = ref('')
const statusNotice = ref('')
const selectedId = ref('')
const latestTopId = ref('')
const settingsOpen = ref(false)
const lastLoadedAt = ref<Date | null>(null)
let refreshTimer: ReturnType<typeof setInterval> | undefined
let searchTimer: ReturnType<typeof setTimeout> | undefined
let noticeTimer: ReturnType<typeof setTimeout> | undefined
let unlistenHistoryChanged: (() => void) | undefined

const itemCountLabel = computed(() => `${items.value.length} 条记录`)
const selectedItem = computed(() => items.value.find((item) => item.id === selectedId.value) ?? items.value[0])
const analysisCards = computed(() => analyzeClipboardText(selectedItem.value?.text ?? ''))

function setNotice(message: string) {
  statusNotice.value = message
  clearTimeout(noticeTimer)
  noticeTimer = setTimeout(() => {
    statusNotice.value = ''
  }, 1400)
}

function applySelection(records: clipboardRecord[]) {
  const result = resolveSelectedHistoryId({
    records,
    currentSelectedId: selectedId.value,
    lastTopId: latestTopId.value,
  })
  selectedId.value = result.selectedId
  latestTopId.value = result.lastTopId
}

async function loadHistory(options: { silent?: boolean } = {}) {
  const silent = options.silent === true
  if (!silent) {
    loading.value = true
    errorMessage.value = ''
  }

  try {
    const records = await ClipboardService.ListHistory(query.value)
    items.value = records ?? []
    applySelection(items.value)
    lastLoadedAt.value = new Date()
    hasLoaded.value = true
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error)
  } finally {
    if (!silent) {
      loading.value = false
    }
  }
}

async function deleteItem(id: string) {
  errorMessage.value = ''
  try {
    await ClipboardService.DeleteHistory(id)
    await loadHistory({ silent: true })
    setNotice('已删除历史记录')
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error)
  }
}

async function copyItem(item: clipboardRecord) {
  selectedId.value = item.id
  errorMessage.value = ''
  try {
    await ClipboardService.CopyHistory(item.id)
    setNotice('已复制到剪贴板')
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error)
  }
}

async function clearHistory() {
  if (items.value.length === 0) {
    return
  }

  errorMessage.value = ''
  try {
    await ClipboardService.ClearHistory()
    await loadHistory({ silent: true })
    setNotice('已清空历史记录')
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error)
  }
}

watch(query, () => {
  clearTimeout(searchTimer)
  searchTimer = setTimeout(() => loadHistory({ silent: true }), 180)
})

onMounted(() => {
  loadHistory()
  unlistenHistoryChanged = Events.On('clipboard-history-changed', () => {
    loadHistory({ silent: true })
  })
  refreshTimer = setInterval(() => loadHistory({ silent: true }), 5000)
})

onUnmounted(() => {
  if (refreshTimer) {
    clearInterval(refreshTimer)
  }
  if (unlistenHistoryChanged) {
    unlistenHistoryChanged()
  }
  clearTimeout(searchTimer)
  clearTimeout(noticeTimer)
})
</script>

<template>
  <main class="clipboard-app">
    <section class="workspace">
      <header class="topbar">
        <div>
          <p class="eyebrow">ntools clipboard</p>
          <h1>剪贴板历史</h1>
        </div>
        <div class="topbar-actions">
          <button type="button" @click="settingsOpen = true">设置</button>
          <button type="button" :disabled="loading" @click="loadHistory()">刷新</button>
          <button type="button" class="danger-button" :disabled="items.length === 0" @click="clearHistory">清空</button>
        </div>
      </header>

      <div class="main-grid">
        <section class="history-pane" aria-label="剪贴板历史列表">
          <div class="search-row">
            <input
              v-model="query"
              class="search-input"
              type="search"
              placeholder="搜索复制历史"
              autocomplete="off"
              aria-label="搜索复制历史"
            />
            <span class="count-pill">{{ itemCountLabel }}</span>
          </div>

          <HistoryList
            :items="items"
            :selected-id="selectedId"
            :has-loaded="hasLoaded"
            @select="copyItem"
            @focus-item="selectedId = $event"
            @delete-item="deleteItem"
          />
        </section>

        <aside class="detail-pane" aria-label="剪贴板详情">
          <template v-if="selectedItem">
            <div class="detail-head">
              <span class="detail-label">当前选中</span>
              <span>{{ formatDateTime(selectedItem.copiedAt) }}</span>
            </div>
            <pre class="detail-text">{{ selectedItem.text }}</pre>
            <div class="detail-actions">
              <button type="button" @click="copyItem(selectedItem)">复制这条</button>
            </div>
            <AnalysisPanel :cards="analysisCards" />
          </template>
          <template v-else>
            <div class="empty-detail">
              <h2>等待复制</h2>
              <p>后台会静默刷新历史，不再每秒闪烁。</p>
            </div>
          </template>
        </aside>
      </div>

      <StatusBar :loading="loading" :notice="statusNotice" :error="errorMessage" :last-loaded-at="lastLoadedAt" />
    </section>
    <SettingsPanel :open="settingsOpen" @close="settingsOpen = false" />
  </main>
</template>

<style scoped>
:global(body) {
  display: block;
  min-height: 100vh;
  margin: 0;
  padding: 0;
  overflow: hidden;
  background: #101218;
  color: #f6f7fb;
  font-family:
    -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial,
    "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
  color-scheme: dark;
  user-select: none;
  -webkit-user-select: none;
  --wails-draggable: drag;
}

:global(#app) {
  display: block;
  width: 100vw;
  height: 100vh;
}

:global(*) {
  box-sizing: border-box;
  scrollbar-width: none;
}

:global(*::-webkit-scrollbar) {
  display: none;
}

:global(button),
:global(input),
:global(textarea),
:global(select) {
  font: inherit;
}

.clipboard-app {
  height: 100vh;
  min-height: 0;
  width: 100vw;
  padding: 22px;
  overflow: hidden;
  color: #f6f7fb;
  background:
    radial-gradient(circle at 12% 8%, rgba(52, 211, 153, 0.12), transparent 28%),
    linear-gradient(180deg, #151821 0%, #0f1117 100%);
  --wails-draggable: drag;
}

.workspace {
  display: flex;
  flex-direction: column;
  width: min(1080px, 100%);
  height: 100%;
  margin: 0 auto;
}

.topbar,
.search-row,
.detail-head,
.detail-actions {
  display: flex;
  align-items: center;
}

.topbar {
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
  flex: 0 0 auto;
}

.eyebrow {
  margin: 0 0 5px;
  color: #8c96aa;
  font-size: 12px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

h1,
h2,
p {
  margin: 0;
}

h1 {
  font-size: 26px;
  letter-spacing: 0;
}

.topbar-actions {
  display: flex;
  gap: 8px;
}

button,
.search-input,
.detail-pane {
  --wails-draggable: no-drag;
}

button {
  border: 1px solid #323847;
  border-radius: 8px;
  color: #f6f7fb;
  background: #1b202b;
  padding: 7px 10px;
  font-size: 13px;
  cursor: pointer;
}

button:hover:not(:disabled) {
  border-color: #4b5565;
  background: #222838;
}

button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.danger-button {
  border-color: rgba(239, 68, 68, 0.38);
  color: #fecaca;
}

.main-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.35fr) minmax(300px, 0.75fr);
  gap: 14px;
  min-height: 0;
  flex: 1 1 auto;
}

.history-pane,
.detail-pane {
  min-height: 0;
  border: 1px solid #252b38;
  border-radius: 8px;
  background: rgba(18, 22, 31, 0.92);
}

.history-pane {
  display: flex;
  flex-direction: column;
  padding: 10px;
}

.search-row {
  gap: 10px;
  flex: 0 0 auto;
}

.search-input {
  flex: 1;
  min-width: 0;
  border: 1px solid #323847;
  border-radius: 8px;
  outline: none;
  color: #f6f7fb;
  background: #10141d;
  padding: 9px 10px;
  font-size: 13px;
  user-select: text;
  -webkit-user-select: text;
}

.search-input:focus {
  border-color: #64748b;
}

.count-pill {
  flex: 0 0 auto;
  color: #98a2b3;
  font-size: 12px;
  border: 1px solid #2b3242;
  border-radius: 999px;
  padding: 5px 8px;
}

.detail-pane {
  display: flex;
  flex-direction: column;
  padding: 12px;
  overflow: hidden;
}

.detail-head {
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 10px;
  color: #98a2b3;
  font-size: 13px;
  flex: 0 0 auto;
}

.detail-label {
  color: #f6f7fb;
  font-weight: 600;
}

.detail-text {
  flex: 1 1 auto;
  min-height: 0;
  margin: 0;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  color: #eef2f7;
  background: #0f131b;
  border: 1px solid #242b38;
  border-radius: 8px;
  padding: 12px;
  font: 13px/1.55 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  user-select: text;
  -webkit-user-select: text;
}

.detail-actions {
  gap: 8px;
  margin-top: 10px;
}

.empty-detail {
  padding: 28px;
  text-align: center;
}

.empty-detail h2 {
  margin-bottom: 8px;
  font-size: 17px;
}

.empty-detail p {
  color: #98a2b3;
  font-size: 13px;
}

@media (max-width: 780px) {
  .clipboard-app {
    padding: 16px;
  }

  .workspace {
    height: calc(100vh - 32px);
  }

  .main-grid {
    grid-template-columns: 1fr;
  }

  .detail-pane {
    min-height: 240px;
  }
}
</style>
