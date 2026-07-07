<script setup lang="ts">
import { ref, watch } from 'vue'
import { ConfigService } from '../../../bindings/changeme'
import type { appConfig } from '../../../bindings/changeme/models'
import { PARSER_OPTIONS } from './config'
import HotkeyRecorder from './HotkeyRecorder.vue'

const props = defineProps<{
  open: boolean
}>()

defineEmits<{
  close: []
}>()

const config = ref<appConfig | null>(null)
const loading = ref(false)
const saving = ref(false)
const errorMessage = ref('')
const statusMessage = ref('')

async function loadConfig() {
  loading.value = true
  errorMessage.value = ''
  statusMessage.value = ''
  try {
    config.value = await ConfigService.GetConfig()
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error)
  } finally {
    loading.value = false
  }
}

async function saveConfig() {
  if (!config.value) {
    return
  }

  saving.value = true
  errorMessage.value = ''
  statusMessage.value = ''
  try {
    config.value = await ConfigService.SaveConfig(config.value)
    statusMessage.value = '设置已保存'
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error)
  } finally {
    saving.value = false
  }
}

function parserEnabled(id: string): boolean {
  return config.value?.parsers.enabled?.[id] ?? false
}

function toggleParser(id: string, event: Event) {
  if (!config.value) {
    return
  }
  if (!config.value.parsers.enabled) {
    config.value.parsers.enabled = {}
  }
  config.value.parsers.enabled[id] = (event.target as HTMLInputElement).checked
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      loadConfig()
    }
  },
)
</script>

<template>
  <div v-if="open" class="settings-backdrop" @click.self="$emit('close')">
    <aside class="settings-panel" aria-label="设置面板">
      <header class="settings-head">
        <div>
          <p class="eyebrow">settings</p>
          <h2>设置</h2>
        </div>
        <div class="settings-actions">
          <button type="button" :disabled="saving || !config" @click="saveConfig">保存</button>
          <button type="button" aria-label="关闭设置" @click="$emit('close')">关闭</button>
        </div>
      </header>

      <p v-if="errorMessage" class="message error-message">{{ errorMessage }}</p>
      <p v-else-if="statusMessage" class="message success-message">{{ statusMessage }}</p>
      <p v-else-if="loading" class="message">加载设置中...</p>

      <section v-if="config" class="settings-body">
        <article class="settings-section">
          <div>
            <h3>快捷键</h3>
            <p>点击输入框后按下目标组合键，保存后写入本地配置。</p>
          </div>
          <HotkeyRecorder v-model="config.hotkeys.openPanel" label="打开历史面板" />
          <HotkeyRecorder v-model="config.hotkeys.copyShortcut" label="监听复制快捷键" />
          <HotkeyRecorder v-model="config.hotkeys.pasteLatest" label="粘贴最近历史" />
        </article>

        <article class="settings-section">
          <div>
            <h3>历史记录</h3>
            <p>控制本地历史保留数量。保存后下次启动会应用到历史服务。</p>
          </div>
          <label>
            <span>最大历史条数</span>
            <input v-model.number="config.history.maxItems" min="20" max="5000" step="10" type="number" />
          </label>
        </article>

        <article class="settings-section">
          <div>
            <h3>内容解析</h3>
            <p>开启或关闭复制内容的本地解析规则。</p>
          </div>
          <div class="parser-grid">
            <label v-for="option in PARSER_OPTIONS" :key="option.id" class="check-row">
              <input type="checkbox" :checked="parserEnabled(option.id)" @change="toggleParser(option.id, $event)" />
              <span>{{ option.label }}</span>
            </label>
          </div>
        </article>

        <article class="settings-section">
          <div>
            <h3>翻译源</h3>
            <p>第一版默认关闭网络翻译，只保留配置入口。</p>
          </div>
          <label>
            <span>服务来源</span>
            <select v-model="config.translation.provider">
              <option value="disabled">关闭</option>
              <option value="local">本地词库</option>
            </select>
          </label>
          <label>
            <span>目标语言</span>
            <input v-model="config.translation.targetLanguage" type="text" />
          </label>
        </article>
      </section>
    </aside>
  </div>
</template>

<style scoped>
.settings-backdrop {
  position: fixed;
  inset: 0;
  z-index: 20;
  display: flex;
  justify-content: flex-end;
  background: rgba(7, 10, 16, 0.42);
  backdrop-filter: blur(4px);
  --wails-draggable: no-drag;
}

.settings-panel {
  width: min(420px, calc(100vw - 28px));
  height: 100%;
  border-left: 1px solid #293041;
  background: #121722;
  box-shadow: -18px 0 45px rgba(0, 0, 0, 0.34);
  color: #f6f7fb;
  padding: 18px;
  overflow: auto;
  scrollbar-width: none;
}

.settings-panel::-webkit-scrollbar {
  display: none;
}

.settings-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 18px;
}

.settings-actions {
  display: flex;
  gap: 8px;
}

.eyebrow {
  margin: 0 0 5px;
  color: #8c96aa;
  font-size: 12px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

h2,
h3,
p {
  margin: 0;
}

h2 {
  font-size: 24px;
}

button {
  border: 1px solid #323847;
  border-radius: 8px;
  color: #f6f7fb;
  background: #1b202b;
  padding: 8px 11px;
  font-size: 13px;
  cursor: pointer;
}

button:hover {
  border-color: #4b5565;
  background: #222838;
}

button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.message {
  margin: 0 0 12px;
  color: #98a2b3;
  font-size: 13px;
}

.error-message {
  color: #fecaca;
}

.success-message {
  color: #bbf7d0;
}

.settings-body {
  display: grid;
  gap: 10px;
}

.settings-section {
  display: grid;
  gap: 12px;
  border: 1px solid #252b38;
  border-radius: 8px;
  background: #151b27;
  padding: 13px;
}

label {
  display: grid;
  gap: 6px;
  color: #98a2b3;
  font-size: 12px;
}

input,
select {
  width: 100%;
  border: 1px solid #323847;
  border-radius: 8px;
  outline: none;
  color: #f6f7fb;
  background: #10141d;
  padding: 8px 9px;
  font-size: 13px;
}

input:focus,
select:focus {
  border-color: #64748b;
}

.parser-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.check-row {
  display: flex;
  align-items: center;
  gap: 8px;
  border: 1px solid #273044;
  border-radius: 7px;
  background: #10141d;
  padding: 8px 9px;
}

.check-row input {
  width: auto;
}

.settings-section h3 {
  margin-bottom: 5px;
  font-size: 15px;
}

.settings-section p {
  color: #98a2b3;
  font-size: 13px;
  line-height: 1.5;
}

</style>
