<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as api from "./api";
import { useHistory } from "./composables/useHistory";
import { usePlugins } from "./composables/usePlugins";
import { useSettings } from "./composables/useSettings";
import { useStatus } from "./composables/useStatus";
import { formatHistoryTime } from "./utils/time";
import "./extensions";

const { statusLine, statusTone, setStatus, disposeStatus } = useStatus();
const {
  query,
  items,
  loading,
  hasLoaded,
  selectedId,
  selectedItem,
  imageSrc,
  panelContent,
  suggestions,
  appliedSuggestionId,
  isShowingOriginal,
  itemCountLabel,
  formatBadge,
  loadHistory,
  deleteItem,
  copyItem,
  clearAll,
  applySuggestion,
  applyAndCopySuggestion,
  showOriginal,
  disposeHistory,
  refreshPluginSuggestions,
} = useHistory(setStatus);

const {
  settingsOpen,
  dataRoot,
  hotkeyRecording,
  config,
  loadConfig,
  onHotkeyKeydown,
  saveConfig,
  defaultToggleHotkey,
} = useSettings(setStatus);

const {
  pluginsOpen,
  pluginsTab,
  plugins,
  pluginsLoading,
  ecdict,
  ecdictBusy,
  ecdictProgress,
  protocolHelp,
  samples,
  samplesLoading,
  openPlugins,
  switchTab,
  togglePlugin,
  importPluginDir,
  removeUserPlugin,
  downloadEcdict,
  exportSample,
  exportAllSamples,
} = usePlugins(setStatus);

async function onTogglePlugin(p: (typeof plugins.value)[0]) {
  await togglePlugin(p);
  refreshPluginSuggestions();
}

function onGlobalKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === "w") {
    event.preventDefault();
    void api.hideMainWindow();
  }
}

const unlisteners: UnlistenFn[] = [];

onMounted(() => {
  window.addEventListener("keydown", onGlobalKeydown);

  // Register event listeners first and independently — never couple to data load success.
  void listen("clipboard-history-changed", () => {
    void loadHistory({ silent: true });
  }).then((u) => {
    unlisteners.push(u);
  });
  void listen("open-settings", () => {
    settingsOpen.value = true;
  }).then((u) => {
    unlisteners.push(u);
  });
  void listen<{ message?: string; phase?: string }>("ecdict-progress", (ev) => {
    const msg = ev.payload?.message;
    if (msg) setStatus(msg, "ok");
  }).then((u) => {
    unlisteners.push(u);
  });

  void loadConfig();
  void loadHistory();
});

onUnmounted(() => {
  window.removeEventListener("keydown", onGlobalKeydown);
  for (const u of unlisteners) u();
  disposeHistory();
  disposeStatus();
});
</script>

<template>
  <main class="shell">
    <header class="top" data-tauri-drag-region>
      <div class="brand" data-tauri-drag-region>
        <img class="mark" src="/appicon.png" width="32" height="32" alt="" draggable="false" />
        <div class="brand-text" data-tauri-drag-region>
          <h1>FunCV</h1>
          <p>本地剪贴板</p>
        </div>
      </div>
      <nav class="nav">
        <button type="button" class="link" :disabled="loading" @click="loadHistory()">刷新</button>
        <button type="button" class="link" @click="openPlugins">插件</button>
        <button type="button" class="link" :disabled="!items.length" @click="clearAll">清空</button>
        <button type="button" class="link" @click="settingsOpen = true">设置</button>
      </nav>
    </header>

    <div class="search-wrap">
      <input v-model="query" class="search" placeholder="搜索历史…" spellcheck="false" />
      <span class="count">{{ itemCountLabel }}</span>
    </div>

    <div class="body">
      <section class="list" aria-label="历史">
        <div v-if="!hasLoaded && loading" class="empty">加载中…</div>
        <div v-else-if="items.length === 0" class="empty">
          <strong>暂无记录</strong>
          <span>复制后自动出现 · 双击复制当前面板内容</span>
        </div>
        <ul v-else>
          <li
            v-for="item in items"
            :key="item.id"
            :class="{ on: item.id === selectedItem?.id }"
            @click="selectedId = item.id"
            @dblclick="copyItem(item)"
          >
            <span class="dot" :data-k="item.kind" />
            <div class="meta">
              <p class="preview">{{ item.preview }}</p>
              <time>{{ formatHistoryTime(item.copiedAt) }}</time>
            </div>
            <button type="button" class="icon-del" title="删除" @click.stop="deleteItem(item.id)">
              ×
            </button>
          </li>
        </ul>
      </section>

      <section class="detail" aria-label="详情">
        <template v-if="selectedItem">
          <div class="detail-head">
            <span class="label">{{ selectedItem.kind === "image" ? "预览" : "内容" }}</span>
            <span v-if="formatBadge" class="chip" :class="{ soft: !isShowingOriginal }">{{ formatBadge }}</span>
            <button
              v-if="!isShowingOriginal && selectedItem.kind === 'text'"
              type="button"
              class="link head-action"
              @click="showOriginal"
            >
              恢复原文
            </button>
          </div>

          <!-- Main panel: original by default; scrolls when long -->
          <div class="viewport">
            <div v-if="panelContent.mode === 'image'" class="img-wrap">
              <img v-if="imageSrc" :src="imageSrc" alt="" />
              <p v-else class="muted">图片不可用</p>
            </div>
            <pre v-else-if="panelContent.mode === 'plain'" class="code">{{ panelContent.body }}</pre>
            <div v-else class="muted">无内容</div>
          </div>

          <!-- 2×2 grid: show parse rule; click apply, double-click copy -->
          <div v-if="suggestions.length && selectedItem.kind === 'text'" class="suggest-bar">
            <div class="suggest-label">推荐解析 · 双击复制</div>
            <div class="suggest-grid" role="list">
              <button
                v-for="s in suggestions"
                :key="s.id"
                type="button"
                class="suggest-item"
                :class="{ on: appliedSuggestionId === s.id, rec: s.recommended }"
                role="listitem"
                :title="`${s.title}${s.preview ? ' · ' + s.preview : ''} · 双击复制`"
                @click="applySuggestion(s)"
                @dblclick.stop="applyAndCopySuggestion(s)"
              >
                <span class="suggest-title">
                  <span class="suggest-rule">{{ s.title }}</span>
                  <em v-if="s.recommended && appliedSuggestionId !== s.id">荐</em>
                </span>
                <span class="suggest-value">{{ s.preview }}</span>
              </button>
            </div>
          </div>
        </template>
        <div v-else class="empty full">选择左侧记录</div>
      </section>
    </div>

    <footer class="foot">
      <span class="status" :data-tone="statusTone">{{ statusLine }}</span>
      <span v-if="dataRoot" class="path" :title="dataRoot">{{ dataRoot }}</span>
    </footer>

    <div v-if="settingsOpen" class="scrim" @click.self="settingsOpen = false">
      <aside class="drawer" @click.stop>
        <header>
          <h2>设置</h2>
          <button type="button" class="link icon" @click="settingsOpen = false">×</button>
        </header>
        <label>
          最大历史条数
          <input v-model.number="config.history.maxItems" type="number" min="10" max="5000" />
        </label>
        <label>
          轮询间隔 (ms)
          <input v-model.number="config.pollIntervalMs" type="number" min="150" max="5000" step="50" />
        </label>
        <label class="hotkey-field">
          唤起 / 隐藏快捷键
          <input
            class="hotkey-input"
            :class="{ recording: hotkeyRecording }"
            type="text"
            readonly
            :value="hotkeyRecording ? '按下组合键…' : config.toggleHotkey"
            :placeholder="defaultToggleHotkey()"
            @focus="hotkeyRecording = true"
            @click="hotkeyRecording = true"
            @blur="hotkeyRecording = false"
            @keydown="onHotkeyKeydown"
          />
        </label>
        <p class="tip">
          右侧默认显示原文；底部「推荐解析」点击后才写入大面板。双击历史复制当前面板内容。
        </p>
        <div class="drawer-actions">
          <button type="button" class="link" @click="settingsOpen = false">取消</button>
          <button type="button" class="primary" @click="saveConfig">保存</button>
        </div>
      </aside>
    </div>

    <!-- Plugin drawer -->
    <div v-if="pluginsOpen" class="scrim" @click.self="pluginsOpen = false">
      <aside class="drawer drawer-wide" @click.stop>
        <header>
          <h2>插件扩展</h2>
          <button type="button" class="link icon" @click="pluginsOpen = false">×</button>
        </header>

        <div class="tabs" role="tablist">
          <button
            type="button"
            class="tab"
            :class="{ on: pluginsTab === 'list' }"
            role="tab"
            @click="switchTab('list')"
          >
            列表
          </button>
          <button
            type="button"
            class="tab"
            :class="{ on: pluginsTab === 'custom' }"
            role="tab"
            @click="switchTab('custom')"
          >
            自定义
          </button>
        </div>

        <!-- Tab: 列表 — installed plugins -->
        <template v-if="pluginsTab === 'list'">
          <div class="plugin-toolbar">
            <button type="button" class="primary" @click="importPluginDir">上传插件</button>
          </div>

          <div class="ecdict-box">
            <div class="ecdict-head">
              <strong>ECDICT 本地词典</strong>
              <span v-if="ecdict?.ready" class="chip soft">已就绪</span>
              <span v-else class="chip soft">未安装</span>
            </div>
            <p class="tip">
              内置「英汉互译」依赖
              <a class="ext" href="https://github.com/skywind3000/ECDICT" target="_blank" rel="noreferrer"
                >ECDICT</a
              >
              · 首次下载需几分钟
            </p>
            <p v-if="ecdict?.ready" class="tip">
              {{ ecdict.entries.toLocaleString() }} 词条 ·
              {{ Math.round((ecdict.sizeBytes || 0) / 1024 / 1024) }} MB
            </p>
            <p v-if="ecdictProgress" class="tip">{{ ecdictProgress }}</p>
            <button type="button" class="link" :disabled="ecdictBusy" @click="downloadEcdict">
              {{ ecdictBusy ? "下载 / 导入中…" : ecdict?.ready ? "重新下载词典" : "下载 ECDICT" }}
            </button>
          </div>

          <div class="plugin-list-label">已安装插件</div>
          <div v-if="pluginsLoading" class="tip">加载中…</div>
          <ul v-else class="plugin-list">
            <li v-for="p in plugins" :key="p.id" class="plugin-card">
              <div class="plugin-main">
                <div class="plugin-name">
                  {{ p.name }}
                  <span class="runtime">{{ p.runtime }}</span>
                  <span v-if="p.builtin" class="runtime">内置</span>
                  <span class="runtime" :class="{ off: !p.enabled }">{{ p.enabled ? "开" : "关" }}</span>
                </div>
                <p class="plugin-desc">{{ p.description || "—" }}</p>
                <p class="plugin-meta">
                  types: {{ (p.types || []).join(", ") || "text" }} · v{{ p.version }}
                </p>
              </div>
              <div class="plugin-actions">
                <button
                  type="button"
                  class="switch"
                  :class="{ on: p.enabled }"
                  :aria-pressed="p.enabled"
                  :title="p.enabled ? '关闭插件' : '启用插件'"
                  @click="onTogglePlugin(p)"
                >
                  <span class="switch-knob" />
                </button>
                <button
                  v-if="!p.builtin"
                  type="button"
                  class="link danger"
                  @click="removeUserPlugin(p)"
                >
                  删除
                </button>
              </div>
            </li>
          </ul>
          <p v-if="!pluginsLoading && !plugins.length" class="tip">暂无插件</p>
        </template>

        <!-- Tab: 自定义 — protocol + sample downloads -->
        <template v-else>
          <div class="plugin-list-label">接口规范</div>
          <div class="protocol-box">
            <pre class="protocol">{{ protocolHelp || "加载中…" }}</pre>
          </div>

          <div class="plugin-list-label">示例文件</div>
          <div class="plugin-toolbar">
            <button type="button" class="primary" :disabled="samplesLoading" @click="exportAllSamples">
              下载全部示例
            </button>
          </div>
          <p class="tip">选择保存目录后，会生成对应文件夹（含 plugin.json 与入口脚本）。</p>
          <div v-if="samplesLoading" class="tip">加载示例列表…</div>
          <ul v-else class="plugin-list">
            <li v-for="s in samples" :key="s.id" class="plugin-card">
              <div class="plugin-main">
                <div class="plugin-name">
                  {{ s.name }}
                  <span class="runtime">{{ s.runtime }}</span>
                </div>
                <p class="plugin-desc">{{ s.description }}</p>
                <p class="plugin-meta">文件: {{ (s.files || []).join(", ") }}</p>
              </div>
              <div class="plugin-actions">
                <button type="button" class="link" @click="exportSample(s.id, s.name)">下载</button>
              </div>
            </li>
          </ul>
        </template>
      </aside>
    </div>
  </main>
</template>

<style scoped>
.shell {
  --bg: #0b0f13;
  --panel: #12171d;
  --line: rgba(255, 255, 255, 0.07);
  --text: #e8edf2;
  --muted: #7a8792;
  --accent: #3ecfad;
  --accent-soft: rgba(62, 207, 173, 0.12);
  position: relative;
  height: 100%;
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr) 28px;
  gap: 12px;
  padding: 16px 18px 10px;
  color: var(--text);
  background: var(--bg);
}

.top {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
}

.mark {
  width: 32px;
  height: 32px;
  border-radius: 9px;
  display: block;
  object-fit: cover;
  flex-shrink: 0;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35);
}

.brand-text h1 {
  margin: 0;
  font-size: 1rem;
  font-weight: 720;
  letter-spacing: 0.02em;
}

.brand-text p {
  margin: 1px 0 0;
  color: var(--muted);
  font-size: 0.7rem;
}

.nav {
  display: flex;
  align-items: center;
  gap: 2px;
  -webkit-app-region: no-drag;
  app-region: no-drag;
}

.search-wrap,
.list,
.detail,
.scrim {
  -webkit-app-region: no-drag;
  app-region: no-drag;
}

button {
  font: inherit;
  cursor: pointer;
  border: 0;
  background: transparent;
  color: var(--text);
}

button:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

button.link {
  min-height: 28px;
  padding: 0 10px;
  border-radius: 7px;
  color: var(--muted);
  font-size: 0.78rem;
}

button.link:hover:not(:disabled) {
  color: var(--text);
  background: rgba(255, 255, 255, 0.05);
}

button.link.icon {
  width: 28px;
  padding: 0;
  font-size: 1.05rem;
}

button.primary {
  min-height: 30px;
  padding: 0 14px;
  border-radius: 8px;
  background: var(--accent);
  color: #042019;
  font-size: 0.8rem;
  font-weight: 700;
}

.search-wrap {
  display: grid;
  grid-template-columns: 1fr auto;
  align-items: center;
  gap: 10px;
}

.search {
  height: 34px;
  border: 1px solid var(--line);
  border-radius: 10px;
  padding: 0 12px;
  color: var(--text);
  background: #0e1318;
  outline: none;
  font-size: 0.86rem;
}

.search:focus {
  border-color: rgba(62, 207, 173, 0.45);
}

.count {
  color: var(--muted);
  font-size: 0.75rem;
  font-variant-numeric: tabular-nums;
  min-width: 1.5em;
  text-align: right;
}

.body {
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(240px, 0.85fr) minmax(0, 1.3fr);
  gap: 12px;
}

.list,
.detail {
  min-height: 0;
  border: 1px solid var(--line);
  border-radius: 12px;
  background: var(--panel);
  overflow: hidden;
}

.list {
  display: flex;
  flex-direction: column;
}

.list ul {
  list-style: none;
  margin: 0;
  padding: 6px;
  height: 100%;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
  /* Keep scroll; hide scrollbar chrome */
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.list ul::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}

.list li {
  display: grid;
  grid-template-columns: 8px minmax(0, 1fr) 26px;
  gap: 10px;
  align-items: start;
  padding: 10px 8px 10px 10px;
  border-radius: 9px;
  cursor: pointer;
  transition: background 0.12s ease;
}

.list li:hover {
  background: rgba(255, 255, 255, 0.03);
}

.list li.on {
  background: var(--accent-soft);
}

.dot {
  width: 7px;
  height: 7px;
  margin-top: 6px;
  border-radius: 50%;
  background: #6f8f85;
}

.dot[data-k="image"] {
  background: #6aa8e8;
}

.dot[data-k="text"] {
  background: var(--accent);
}

.meta {
  min-width: 0;
}

.preview {
  margin: 0 0 4px;
  font-size: 0.84rem;
  line-height: 1.4;
  color: #dce4eb;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

time {
  color: var(--muted);
  font-size: 0.68rem;
  font-variant-numeric: tabular-nums;
}

.icon-del {
  width: 26px;
  height: 26px;
  border-radius: 6px;
  color: var(--muted);
  font-size: 0.95rem;
  line-height: 1;
  opacity: 0;
  transition: opacity 0.12s ease, color 0.12s ease, background 0.12s ease;
}

.list li:hover .icon-del,
.list li.on .icon-del {
  opacity: 1;
}

.icon-del:hover {
  color: #f0b4b4;
  background: rgba(180, 70, 70, 0.14);
}

/* Detail: head + scrollable viewport + fixed suggest bar */
.detail {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.detail-head {
  flex: 0 0 34px;
  height: 34px;
  min-height: 34px;
  max-height: 34px;
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 8px;
  padding: 0 14px;
  border-bottom: 1px solid var(--line);
  overflow: hidden;
  box-sizing: border-box;
}

.label {
  font-size: 0.78rem;
  font-weight: 680;
  color: #c5d0d8;
  line-height: 1;
  flex-shrink: 0;
}

.chip {
  font-size: 0.62rem;
  font-weight: 720;
  line-height: 1;
  padding: 2px 7px;
  border-radius: 999px;
  color: #052018;
  background: var(--accent);
  flex-shrink: 0;
}

.chip.soft {
  color: var(--accent);
  background: var(--accent-soft);
}

/* Same visual scale as chip — never grow title bar height */
.head-action {
  margin-left: auto;
  flex-shrink: 0;
  min-height: 0 !important;
  height: auto;
  padding: 2px 7px !important;
  border-radius: 999px;
  font-size: 0.62rem;
  font-weight: 720;
  line-height: 1;
  color: var(--muted);
}

.viewport {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  padding: 12px 14px;
  scrollbar-width: thin;
  scrollbar-color: rgba(255, 255, 255, 0.14) transparent;
}

.viewport::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

.viewport::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.14);
  border-radius: 4px;
}

.code {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.84rem;
  line-height: 1.55;
  color: #e6edf3;
}

.img-wrap {
  display: grid;
  place-items: center;
  min-height: 100%;
}

.img-wrap img {
  max-width: 100%;
  max-height: min(420px, 55vh);
  object-fit: contain;
  border-radius: 8px;
}

/* Bottom suggestions: 2 columns × 2 rows; rule + truncated value */
.suggest-bar {
  --suggest-item-h: 46px;
  --suggest-gap: 6px;
  flex: 0 0 auto;
  border-top: 1px solid var(--line);
  background: rgba(0, 0, 0, 0.18);
  padding: 8px 10px 10px;
  min-height: 0;
}

.suggest-label {
  margin: 0 2px 6px;
  color: var(--muted);
  font-size: 0.66rem;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.suggest-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  grid-auto-rows: var(--suggest-item-h);
  gap: var(--suggest-gap);
  /* Exactly 2 rows visible */
  max-height: calc(var(--suggest-item-h) * 2 + var(--suggest-gap));
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.suggest-grid::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}

.suggest-item {
  min-width: 0;
  height: var(--suggest-item-h);
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 2px;
  padding: 0 10px;
  border-radius: 9px;
  border: 1px solid var(--line);
  background: rgba(255, 255, 255, 0.025);
  text-align: left;
  transition: border-color 0.12s ease, background 0.12s ease;
}

.suggest-item:hover {
  border-color: rgba(62, 207, 173, 0.35);
  background: rgba(62, 207, 173, 0.06);
}

.suggest-item.rec:not(.on) {
  border-color: rgba(62, 207, 173, 0.22);
}

.suggest-item.on {
  border-color: rgba(62, 207, 173, 0.55);
  background: var(--accent-soft);
}

.suggest-title {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
  font-size: 0.74rem;
  font-weight: 720;
  line-height: 1.2;
  color: var(--accent);
}

.suggest-rule {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.suggest-title em {
  font-style: normal;
  font-size: 0.58rem;
  font-weight: 700;
  padding: 1px 5px;
  border-radius: 999px;
  color: #052018;
  background: var(--accent);
  flex-shrink: 0;
}

/* Parsed value preview — single line, overflow ends with … */
.suggest-value {
  min-width: 0;
  font-size: 0.66rem;
  line-height: 1.25;
  color: var(--muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}

.empty {
  padding: 36px 14px;
  display: grid;
  place-content: center;
  gap: 6px;
  text-align: center;
  color: var(--muted);
  font-size: 0.82rem;
}

.empty.full {
  flex: 1;
  min-height: 140px;
}

.empty strong {
  color: #d4dde5;
  font-weight: 650;
}

.muted {
  color: var(--muted);
  font-size: 0.84rem;
}

.foot {
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
}

.status {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.72rem;
  line-height: 28px;
  color: var(--muted);
  transition: color 0.15s ease;
}

.status[data-tone="ok"] {
  color: var(--accent);
}

.status[data-tone="err"] {
  color: #f0a0a0;
}

.path {
  max-width: 48%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--muted);
  font-size: 0.68rem;
  opacity: 0.75;
}

.scrim {
  position: absolute;
  inset: 0;
  z-index: 30;
  display: flex;
  justify-content: flex-end;
  background: rgba(0, 0, 0, 0.38);
}

.drawer {
  width: min(300px, 92vw);
  height: 100%;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: #0f1419;
  border-left: 1px solid var(--line);
  min-height: 0;
  overflow: auto;
  /* Keep scroll; hide scrollbar chrome */
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.drawer::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}

.drawer-wide {
  width: min(380px, 94vw);
}

.drawer header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.drawer h2 {
  margin: 0;
  font-size: 0.95rem;
}

.drawer label {
  display: grid;
  gap: 6px;
  color: var(--muted);
  font-size: 0.78rem;
}

.drawer input {
  height: 34px;
  border-radius: 8px;
  border: 1px solid var(--line);
  padding: 0 10px;
  color: var(--text);
  background: #0b1014;
}

.hotkey-input {
  cursor: pointer;
  letter-spacing: 0.02em;
}

.hotkey-input.recording {
  color: var(--accent);
  border-color: rgba(62, 207, 173, 0.55);
  background: rgba(62, 207, 173, 0.08);
}

.tip {
  margin: 0;
  color: var(--muted);
  font-size: 0.74rem;
  line-height: 1.45;
}

.drawer-actions {
  margin-top: auto;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.tabs {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 4px;
  padding: 3px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid var(--line);
  flex: 0 0 auto;
}

.tab {
  min-height: 30px;
  border-radius: 8px;
  font-size: 0.78rem;
  font-weight: 650;
  color: var(--muted);
}

.tab:hover {
  color: var(--text);
  background: rgba(255, 255, 255, 0.04);
}

.tab.on {
  color: #052018;
  background: var(--accent);
}

.plugin-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
}

.switch {
  position: relative;
  width: 40px;
  height: 22px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.12);
  border: 1px solid var(--line);
  flex-shrink: 0;
  transition: background 0.15s ease, border-color 0.15s ease;
}

.switch.on {
  background: var(--accent);
  border-color: rgba(62, 207, 173, 0.55);
}

.switch-knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #e8edf2;
  transition: transform 0.15s ease;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.35);
}

.switch.on .switch-knob {
  transform: translateX(18px);
  background: #042019;
}

.runtime.off {
  color: var(--muted);
  background: rgba(255, 255, 255, 0.06);
}

.ecdict-box {
  border: 1px solid var(--line);
  border-radius: 10px;
  padding: 10px 12px;
  display: grid;
  gap: 6px;
  background: rgba(255, 255, 255, 0.02);
}

.ecdict-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ecdict-head strong {
  font-size: 0.82rem;
}

.plugin-list-label {
  margin-top: 4px;
  color: var(--muted);
  font-size: 0.66rem;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.plugin-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 8px;
}

.plugin-card {
  border: 1px solid var(--line);
  border-radius: 10px;
  padding: 10px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px 10px;
  align-items: center;
  background: rgba(255, 255, 255, 0.02);
}

.plugin-main {
  min-width: 0;
}

.plugin-name {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  font-size: 0.84rem;
  font-weight: 680;
}

.runtime {
  font-size: 0.6rem;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 999px;
  color: var(--accent);
  background: var(--accent-soft);
  text-transform: lowercase;
}

.plugin-desc {
  margin: 4px 0 0;
  color: var(--muted);
  font-size: 0.72rem;
  line-height: 1.4;
}

.plugin-meta {
  margin: 2px 0 0;
  color: var(--muted);
  font-size: 0.66rem;
  opacity: 0.85;
}

.plugin-actions {
  display: flex;
  gap: 4px;
}

.plugin-actions .danger {
  color: #f0a0a0;
}

.protocol-box {
  border: 1px solid var(--line);
  border-radius: 10px;
  padding: 10px;
  background: #0b1014;
}

.protocol {
  margin: 8px 0 0;
  max-height: 220px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 0.68rem;
  line-height: 1.45;
  color: #c5d0d8;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.protocol::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}

.tip code {
  font-size: 0.7rem;
  color: #d5f5e9;
}

.tip a.ext {
  color: var(--accent);
  text-decoration: none;
}

.tip a.ext:hover {
  text-decoration: underline;
}
</style>
