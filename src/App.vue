<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as api from "./api";
import CodeEditor from "./components/CodeEditor.vue";
import { useHistory } from "./composables/useHistory";
import { usePlugins } from "./composables/usePlugins";
import { useSearchPlugins } from "./composables/useSearchPlugins";
import { useSettings } from "./composables/useSettings";
import { useStats } from "./composables/useStats";
import { useStatus } from "./composables/useStatus";
import { CODE_LANG_LABEL, detectCodeLang } from "./utils/langDetect";
import { tryEvalMath } from "./utils/mathEval";
import { formatHistoryTime } from "./utils/time";
import { tryAbsoluteTimestampPanel } from "./utils/timestampAbs";
import {
  buildAbsDictPanel,
  isAbsoluteDictQuery,
  isTranslateSuggestionId,
} from "./utils/wordAbs";
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
  editorText,
  editorDirty,
  hydrating,
  isTextEditor,
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
  onEditorInput,
  disposeHistory,
  refreshPluginSuggestions,
} = useHistory(setStatus);

/** Recognized language chip for syntax-highlighted panel text. */
const detectedLangLabel = computed(() => {
  if (!selectedItem.value || selectedItem.value.kind !== "text") return null;
  // Absolute timestamp / dict panels are not code highlight
  if (tryAbsoluteTimestampPanel(editorText.value)) return null;
  if (isAbsoluteDictQuery(editorText.value)) return null;
  const lang = detectCodeLang(editorText.value, appliedSuggestionId.value);
  return lang === "plaintext" ? null : CODE_LANG_LABEL[lang];
});

/**
 * Absolute-match timestamp (e.g. 1783660159738): vertical parse rows in content area.
 * Uses current panel draft; skips when user applied a non-source suggestion that breaks match.
 */
const absTimestampPanel = computed(() => {
  if (!selectedItem.value || selectedItem.value.kind !== "text") return null;
  if (hydrating.value && selectedItem.value.text == null) return null;
  return tryAbsoluteTimestampPanel(editorText.value);
});

/**
 * Absolute-match word / short phrase + ECDICT translate suggestion → content stack.
 * Only when entire text is a dict query (not long prose).
 */
const absDictPanel = computed(() => {
  if (absTimestampPanel.value) return null;
  if (!selectedItem.value || selectedItem.value.kind !== "text") return null;
  if (hydrating.value && selectedItem.value.text == null) return null;
  // Prefer original history word; if user is editing draft that still looks like a word, allow it
  const src = editorText.value;
  if (!isAbsoluteDictQuery(src)) return null;
  const dictSug = suggestions.value.find(
    (s) => isTranslateSuggestionId(s.id) && s.body?.trim(),
  );
  if (!dictSug) return null;
  return buildAbsDictPanel(src, dictSug.body, dictSug.title, dictSug.hint);
});

/** Either absolute panel currently filling the content area. */
const absParsePanel = computed(() => {
  if (absTimestampPanel.value) {
    return {
      kind: "timestamp" as const,
      label: "时间戳",
      chip:
        absTimestampPanel.value.unit === "ms"
          ? "毫秒"
          : absTimestampPanel.value.unit === "s"
            ? "秒"
            : "时间",
      rows: absTimestampPanel.value.rows,
    };
  }
  if (absDictPanel.value) {
    return {
      kind: "dict" as const,
      label: absDictPanel.value.title || "词典",
      chip: absDictPanel.value.direction === "zh-en" ? "汉→英" : "英→汉",
      rows: absDictPanel.value.rows,
    };
  }
  return null;
});

/** Viewport size for “content ≤ half of content area” layout. */
const detailViewportEl = ref<HTMLElement | null>(null);
const viewportSize = ref({ w: 360, h: 320 });
let viewportRo: ResizeObserver | null = null;

function bindViewportMeasure(el: HTMLElement | null) {
  detailViewportEl.value = el;
  viewportRo?.disconnect();
  viewportRo = null;
  if (!el || typeof ResizeObserver === "undefined") return;
  const apply = () => {
    viewportSize.value = {
      w: Math.max(120, el.clientWidth),
      h: Math.max(120, el.clientHeight),
    };
  };
  apply();
  viewportRo = new ResizeObserver(apply);
  viewportRo.observe(el);
}

/**
 * Estimate natural height of current panel content (text draft or image),
 * independent of the split layout, so we can decide half-screen mode.
 */
function estimatePanelContentHeight(): number {
  const item = selectedItem.value;
  if (!item) return 0;
  const { w: vw, h: vh } = viewportSize.value;

  if (item.kind === "image") {
    const iw = item.imageWidth && item.imageWidth > 0 ? item.imageWidth : 480;
    const ih = item.imageHeight && item.imageHeight > 0 ? item.imageHeight : 320;
    const maxW = Math.max(80, vw - 8);
    const maxH = Math.min(420, Math.max(120, vh * 0.9));
    const scale = Math.min(1, maxW / iw, maxH / ih);
    return Math.max(48, ih * scale);
  }

  const text = editorText.value || "";
  if (!text) return 24;
  // Match CodeEditor: 0.84rem · line-height 1.55 · mono ≈ 0.55em per char
  const fontSize = 0.84 * 16;
  const lineHeight = fontSize * 1.55;
  const charW = fontSize * 0.55;
  const cpl = Math.max(16, Math.floor(Math.max(80, vw - 8) / charW));
  let lines = 0;
  for (const line of text.split("\n")) {
    const len = [...line].length;
    lines += Math.max(1, Math.ceil(len / cpl));
  }
  return Math.max(lineHeight, lines * lineHeight) + 4;
}

/** Content (text/image) natural height ≤ half of the content viewport. */
const contentFitsHalf = computed(() => {
  // Depend on size + content
  void viewportSize.value.w;
  void viewportSize.value.h;
  void editorText.value;
  void selectedItem.value?.id;
  void selectedItem.value?.imageWidth;
  void selectedItem.value?.imageHeight;
  const half = viewportSize.value.h * 0.5;
  return estimatePanelContentHeight() <= half + 4;
});

/**
 * Show recommended parse in the lower half (like abs timestamp/dict),
 * when content is short enough or absolute panel applies.
 */
const showLowerParse = computed(() => {
  if (!selectedItem.value) return false;
  if (absParsePanel.value) return true;
  if (!suggestions.value.length) return false;
  return contentFitsHalf.value;
});

const lowerParseTitle = computed(() => {
  if (absParsePanel.value) return absParsePanel.value.label;
  return "推荐解析";
});

const lowerParseHint = computed(() => {
  if (absParsePanel.value) return "点击复制";
  return "单击应用 · 双击复制";
});

async function copyAbsParseRow(value: string, label?: string) {
  try {
    await navigator.clipboard.writeText(value);
    setStatus(label ? `已复制 · ${label}` : "已复制", "ok");
  } catch {
    setStatus("复制失败", "err");
  }
}

function suggestDisplayBody(s: { preview?: string; body: string }): string {
  const p = (s.preview || "").trim();
  if (p) return p;
  const b = s.body.replace(/\s+/g, " ").trim();
  return b.length > 160 ? `${b.slice(0, 160)}…` : b;
}

/** Search box as mini calculator: e.g. `1+1` → 2 (history search still runs). */
const calcResult = computed(() => tryEvalMath(query.value));

const {
  searchPluginCatalog,
  searchPluginResult,
  installSearchPlugin,
  uninstallSearchPlugin,
  toggleSearchPlugin,
} = useSearchPlugins(query, setStatus);

/** Content-area panel driven by search (math first, then search plugins). */
const searchPanel = computed(() => {
  const calc = calcResult.value;
  if (calc) {
    return {
      kind: "calc" as const,
      title: "计算",
      chip: "公式",
      body: `${calc.expression}\n=\n${calc.display}`,
      copyText: calc.ok ? calc.display : "",
      ok: calc.ok,
      expression: calc.expression,
      display: calc.display,
    };
  }
  const sp = searchPluginResult.value;
  if (sp) {
    return {
      kind: "plugin" as const,
      title: sp.title,
      chip: sp.hint || "搜索插件",
      body: sp.body,
      copyText: sp.copyText || sp.body,
      ok: true,
    };
  }
  return null;
});

async function copySearchPanelResult() {
  const p = searchPanel.value;
  if (!p?.ok || !p.copyText) return;
  try {
    await navigator.clipboard.writeText(p.copyText);
    setStatus(`已复制 · ${p.title}`, "ok");
  } catch {
    setStatus("复制失败", "err");
  }
}

const {
  settingsOpen,
  dataRoot,
  hotkeyRecording,
  config,
  loadConfig,
  onHotkeyKeydown,
  saveConfig,
  defaultToggleHotkey,
  defaultAppendCopyHotkey,
  defaultParseCopyHotkey,
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

const {
  statsOpen,
  stats,
  statsLoading,
  duplicateRateLabel,
  peakHourLabel,
  dailyBarWidth,
  openStats,
  refreshStats,
} = useStats(setStatus);

async function onTogglePlugin(p: (typeof plugins.value)[0]) {
  await togglePlugin(p);
  const enabled = plugins.value
    .filter((x) => x.enabled)
    .map((x) => `${x.id}@${x.version || "0"}`);
  refreshPluginSuggestions(enabled);
}

async function onImportPlugin() {
  await importPluginDir();
  const enabled = plugins.value
    .filter((x) => x.enabled)
    .map((x) => `${x.id}@${x.version || "0"}`);
  refreshPluginSuggestions(enabled);
}

async function onRemovePlugin(p: (typeof plugins.value)[0]) {
  await removeUserPlugin(p);
  const enabled = plugins.value
    .filter((x) => x.enabled)
    .map((x) => `${x.id}@${x.version || "0"}`);
  refreshPluginSuggestions(enabled);
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
  void listen("clipboard-stats-changed", () => {
    if (statsOpen.value) void refreshStats();
  }).then((u) => {
    unlisteners.push(u);
  });
  void listen<{ ok?: boolean; message?: string }>("append-copy-result", (ev) => {
    const msg = ev.payload?.message || (ev.payload?.ok ? "已追加复制" : "追加复制失败");
    setStatus(msg, ev.payload?.ok ? "ok" : "err");
  }).then((u) => {
    unlisteners.push(u);
  });
  void listen<{ ok?: boolean; message?: string }>("parse-copy-result", (ev) => {
    const msg = ev.payload?.message || (ev.payload?.ok ? "已复制并打开" : "复制并解析失败");
    setStatus(msg, ev.payload?.ok ? "ok" : "err");
    // Watcher will also fire; force a silent refresh so the new item is selected soon.
    if (ev.payload?.ok) void loadHistory({ silent: true });
  }).then((u) => {
    unlisteners.push(u);
  });

  // Yield a frame so the shell is interactive before IPC-heavy bootstrap.
  requestAnimationFrame(() => {
    void loadConfig();
    // Slight delay: list + hydrate + plugins should not compete with first drag/click.
    setTimeout(() => {
      void loadHistory();
    }, 50);
  });
});

onUnmounted(() => {
  window.removeEventListener("keydown", onGlobalKeydown);
  for (const u of unlisteners) u();
  viewportRo?.disconnect();
  viewportRo = null;
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
        </div>
      </div>
      <nav class="nav">
        <button type="button" class="link" :disabled="loading" @click="loadHistory()">刷新</button>
        <button type="button" class="link" @click="openPlugins">插件</button>
        <button type="button" class="link" @click="openStats">统计</button>
        <button type="button" class="link" :disabled="!items.length" @click="clearAll">清空</button>
        <button type="button" class="link" @click="settingsOpen = true">设置</button>
      </nav>
    </header>

    <div class="search-wrap">
      <div class="search-field">
        <input
          v-model="query"
          class="search"
          :class="{ 'has-clear': !!query }"
          placeholder="搜索历史… 1+1 / 时间"
          spellcheck="false"
        />
        <button
          v-if="query"
          type="button"
          class="search-clear"
          title="清空搜索"
          aria-label="清空搜索"
          @click="query = ''"
        >
          ×
        </button>
      </div>
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
        <!-- Search formula / search plugins → content area (no extra search height) -->
        <template v-if="searchPanel">
          <div class="detail-head">
            <span class="head-title">{{ searchPanel.title }}</span>
            <span class="head-tag tag-mode">{{ searchPanel.chip }}</span>
            <button
              v-if="searchPanel.ok && searchPanel.copyText"
              type="button"
              class="link head-action"
              title="复制结果"
              @click="copySearchPanelResult"
            >
              复制结果
            </button>
          </div>
          <div
            class="viewport"
            :class="searchPanel.kind === 'calc' ? 'calc-viewport' : 'search-plugin-viewport'"
            role="status"
          >
            <div v-if="searchPanel.kind === 'calc'" class="calc-panel">
              <div class="calc-expr" :title="searchPanel.expression">{{ searchPanel.expression }}</div>
              <div class="calc-eq">=</div>
              <div class="calc-val" :class="{ bad: !searchPanel.ok }">{{ searchPanel.display }}</div>
            </div>
            <pre v-else class="search-plugin-body">{{ searchPanel.body }}</pre>
          </div>
        </template>

        <template v-else-if="selectedItem">
          <div class="detail-head">
            <span class="head-title">{{ selectedItem.kind === "image" ? "预览" : "内容" }}</span>
            <span
              v-if="formatBadge"
              class="head-tag"
              :class="
                formatBadge === '原文'
                  ? 'tag-origin'
                  : formatBadge === '草稿'
                    ? 'tag-draft'
                    : formatBadge === '解析'
                      ? 'tag-parse'
                      : 'tag-meta'
              "
            >{{ formatBadge }}</span>
            <span
              v-if="absParsePanel"
              class="head-tag tag-mode"
              :title="absParsePanel.label"
            >{{ absParsePanel.chip }}</span>
            <span
              v-else-if="showLowerParse && suggestions.length"
              class="head-tag tag-mode"
            >解析</span>
            <span
              v-if="detectedLangLabel"
              class="head-tag tag-lang"
              :title="`识别为 ${detectedLangLabel}`"
            >{{ detectedLangLabel }}</span>
            <button
              v-if="(editorDirty || !isShowingOriginal) && selectedItem.kind === 'text'"
              type="button"
              class="link head-action"
              @click="showOriginal"
            >
              恢复原文
            </button>
          </div>

          <!--
            Content ≤ half viewport (or abs timestamp/dict): upper = text/image,
            lower = recommended parse cards. Tall content keeps bottom suggest-bar.
          -->
          <div
            class="viewport"
            :class="{ 'viewport-split': showLowerParse }"
            :ref="(el) => bindViewportMeasure(el as HTMLElement | null)"
          >
            <div
              v-if="panelContent.mode === 'image'"
              class="editor-pane content-pane"
            >
              <div class="img-wrap">
                <img v-if="imageSrc" :src="imageSrc" alt="" />
                <p v-else class="muted">图片不可用</p>
              </div>
            </div>
            <div
              v-else-if="isTextEditor || panelContent.mode === 'plain'"
              class="editor-pane content-pane"
            >
              <CodeEditor
                :model-value="editorText"
                :disabled="hydrating && selectedItem.kind === 'text' && selectedItem.text == null"
                :lang-hint="appliedSuggestionId"
                placeholder="可编辑面板内容（草稿，不改历史）；双击左侧或复制建议写回剪贴板"
                @update:model-value="onEditorInput"
              />
            </div>
            <div v-else class="editor-pane content-pane">
              <p class="muted">无内容</p>
            </div>

            <!-- Lower half: absolute rows or general recommendations -->
            <div v-if="showLowerParse" class="abs-parse-pane" role="list" aria-label="解析结果">
              <div class="abs-parse-pane-head">
                <span>{{ lowerParseTitle }}</span>
                <span class="abs-parse-pane-hint">{{ lowerParseHint }}</span>
              </div>
              <div class="abs-parse-stack">
                <template v-if="absParsePanel">
                  <button
                    v-for="row in absParsePanel.rows"
                    :key="row.id"
                    type="button"
                    class="abs-parse-row"
                    role="listitem"
                    :title="`复制 ${row.label}`"
                    @click="copyAbsParseRow(row.value, row.label)"
                  >
                    <span class="abs-parse-label">
                      {{ row.label }}
                      <em v-if="row.hint">{{ row.hint }}</em>
                    </span>
                    <span class="abs-parse-value">{{ row.value }}</span>
                  </button>
                </template>
                <template v-else>
                  <button
                    v-for="s in suggestions"
                    :key="s.id"
                    type="button"
                    class="abs-parse-row"
                    :class="{ on: appliedSuggestionId === s.id, rec: s.recommended }"
                    role="listitem"
                    :title="`${s.title}${s.preview ? ' · ' + s.preview : ''} · 单击应用 · 双击复制`"
                    @click="applySuggestion(s)"
                    @dblclick.stop="applyAndCopySuggestion(s)"
                  >
                    <span class="abs-parse-label">
                      {{ s.title }}
                      <em v-if="s.recommended && appliedSuggestionId !== s.id">荐</em>
                      <em v-else-if="s.hint">{{ s.hint }}</em>
                    </span>
                    <span class="abs-parse-value">{{ suggestDisplayBody(s) }}</span>
                  </button>
                </template>
              </div>
            </div>
          </div>

          <!-- Tall content: keep compact bottom bar -->
          <div
            v-if="suggestions.length && !showLowerParse"
            class="suggest-bar"
          >
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

    <!-- Stats drawer -->
    <div v-if="statsOpen" class="scrim" @click.self="statsOpen = false">
      <aside class="drawer drawer-wide" @click.stop>
        <header>
          <h2>趣味统计</h2>
          <button type="button" class="link icon" @click="statsOpen = false">×</button>
        </header>
        <div v-if="statsLoading" class="tip">加载中…</div>
        <template v-else>
          <div class="stats-grid">
            <div class="stat-card">
              <div class="stat-n">{{ stats.totalCopies }}</div>
              <div class="stat-l">累计复制</div>
            </div>
            <div class="stat-card">
              <div class="stat-n">{{ stats.todayCopies }}</div>
              <div class="stat-l">今日复制</div>
            </div>
            <div class="stat-card">
              <div class="stat-n">{{ stats.totalDuplicates }}</div>
              <div class="stat-l">重复复制</div>
            </div>
            <div class="stat-card">
              <div class="stat-n">{{ duplicateRateLabel }}</div>
              <div class="stat-l">重复率</div>
            </div>
            <div class="stat-card">
              <div class="stat-n">{{ stats.textCopies }}</div>
              <div class="stat-l">文本</div>
            </div>
            <div class="stat-card">
              <div class="stat-n">{{ stats.imageCopies }}</div>
              <div class="stat-l">图片</div>
            </div>
            <div class="stat-card">
              <div class="stat-n">{{ stats.streakDays }}</div>
              <div class="stat-l">连续活跃天</div>
            </div>
            <div class="stat-card">
              <div class="stat-n">{{ stats.historyItems }}</div>
              <div class="stat-l">历史条数</div>
            </div>
          </div>
          <p class="tip">高峰时段：{{ peakHourLabel }} · 今日重复 {{ stats.todayDuplicates }} 次</p>

          <div class="plugin-list-label">重复复制 Top</div>
          <ul v-if="stats.topRepeats?.length" class="plugin-list">
            <li v-for="t in stats.topRepeats" :key="t.hash" class="plugin-card">
              <div class="plugin-main">
                <p class="plugin-desc">{{ t.preview || t.hash }}</p>
                <p class="plugin-meta">{{ t.kind }} · 复制 {{ t.count }} 次</p>
              </div>
            </li>
          </ul>
          <p v-else class="tip">再多复制一些，这里会出现「最爱拷贝」榜单～</p>

          <div class="plugin-list-label">近 14 天</div>
          <ul v-if="stats.daily?.length" class="daily-list">
            <li v-for="d in stats.daily" :key="d.day">
              <span>{{ d.day.slice(5) }}</span>
              <span class="daily-bar-wrap">
                <span class="daily-bar" :style="{ width: dailyBarWidth(d.total) }" />
              </span>
              <span class="daily-n">{{ d.total }}</span>
            </li>
          </ul>
          <p v-else class="tip">暂无日统计</p>
          <button type="button" class="link" @click="refreshStats">刷新统计</button>
        </template>
      </aside>
    </div>

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
            :class="{ recording: hotkeyRecording === 'toggle' }"
            type="text"
            readonly
            :value="hotkeyRecording === 'toggle' ? '按下组合键…' : config.toggleHotkey"
            :placeholder="defaultToggleHotkey()"
            @focus="hotkeyRecording = 'toggle'"
            @click="hotkeyRecording = 'toggle'"
            @blur="hotkeyRecording = null"
            @keydown="onHotkeyKeydown($event, 'toggle')"
          />
        </label>
        <label class="hotkey-field">
          追加复制快捷键（3 键）
          <input
            class="hotkey-input"
            :class="{ recording: hotkeyRecording === 'append' }"
            type="text"
            readonly
            :value="
              hotkeyRecording === 'append'
                ? '按下三键组合…（Delete 清空）'
                : config.appendCopyHotkey || '未设置（已关闭）'
            "
            :placeholder="defaultAppendCopyHotkey()"
            @focus="hotkeyRecording = 'append'"
            @click="hotkeyRecording = 'append'"
            @blur="hotkeyRecording = null"
            @keydown="onHotkeyKeydown($event, 'append')"
          />
        </label>
        <label class="hotkey-field">
          复制并解析快捷键（3 键）
          <input
            class="hotkey-input"
            :class="{ recording: hotkeyRecording === 'parse' }"
            type="text"
            readonly
            :value="
              hotkeyRecording === 'parse'
                ? '按下三键组合…（Delete 清空）'
                : config.parseCopyHotkey || '未设置（已关闭）'
            "
            :placeholder="defaultParseCopyHotkey()"
            @focus="hotkeyRecording = 'parse'"
            @click="hotkeyRecording = 'parse'"
            @blur="hotkeyRecording = null"
            @keydown="onHotkeyKeydown($event, 'parse')"
          />
        </label>
        <p class="tip">
          <strong>复制并解析</strong>：在任意应用选中文本后按快捷键 → 自动复制并弹出 FunCV 解析（默认
          Shift+Option+X）。<br />
          <strong>追加复制</strong>：剪贴板已是文本时，把选中内容用双空格接在后面；重复追加同一段只生效一次。macOS
          需在「辅助功能」中允许 FunCV。
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
            <button type="button" class="primary" @click="onImportPlugin">上传插件</button>
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

          <div v-if="pluginsLoading" class="tip">加载中…</div>
          <ul v-else class="plugin-list">
            <!-- 搜索插件（可选安装） -->
            <li v-for="sp in searchPluginCatalog" :key="'search:' + sp.id" class="plugin-card">
              <div class="plugin-main">
                <div class="plugin-name">
                  {{ sp.name }}
                  <span class="runtime">搜索</span>
                  <span class="runtime">内置</span>
                  <span v-if="!sp.installed" class="runtime off">未安装</span>
                  <span v-else class="runtime" :class="{ off: !sp.enabled }">
                    {{ sp.enabled ? "开" : "关" }}
                  </span>
                </div>
                <p class="plugin-desc">{{ sp.description }}</p>
                <p class="plugin-meta">
                  触发词: {{ sp.triggers.slice(0, 6).join(" · ")
                  }}{{ sp.triggers.length > 6 ? " …" : "" }} · v{{ sp.version }}
                </p>
              </div>
              <div class="plugin-actions">
                <template v-if="!sp.installed">
                  <button type="button" class="link" @click="installSearchPlugin(sp.id)">安装</button>
                </template>
                <template v-else>
                  <button
                    type="button"
                    class="switch"
                    :class="{ on: sp.enabled }"
                    :aria-pressed="sp.enabled"
                    :title="sp.enabled ? '关闭' : '启用'"
                    @click="toggleSearchPlugin(sp.id)"
                  >
                    <span class="switch-knob" />
                  </button>
                  <button type="button" class="link danger" @click="uninstallSearchPlugin(sp.id)">
                    卸载
                  </button>
                </template>
              </div>
            </li>

            <!-- 内容 / 用户插件 -->
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
                  @click="onRemovePlugin(p)"
                >
                  删除
                </button>
              </div>
            </li>
          </ul>
          <p
            v-if="!pluginsLoading && !searchPluginCatalog.length && !plugins.length"
            class="tip"
          >
            暂无插件
          </p>
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

.search-field {
  position: relative;
  min-width: 0;
  display: flex;
  align-items: center;
}

.search {
  width: 100%;
  height: 34px;
  border: 1px solid var(--line);
  border-radius: 10px;
  padding: 0 12px;
  color: var(--text);
  background: #0e1318;
  outline: none;
  font-size: 0.86rem;
  box-sizing: border-box;
}

.search.has-clear {
  padding-right: 32px;
}

.search:focus {
  border-color: rgba(62, 207, 173, 0.45);
}

.search-clear {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  width: 22px;
  height: 22px;
  display: grid;
  place-items: center;
  margin: 0;
  padding: 0;
  border: 0;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
  color: var(--muted);
  font-size: 0.95rem;
  font-weight: 500;
  line-height: 1;
  cursor: pointer;
  transition: background 0.12s ease, color 0.12s ease;
}

.search-clear:hover {
  background: rgba(255, 255, 255, 0.14);
  color: var(--text);
}

.count {
  color: var(--muted);
  font-size: 0.75rem;
  font-variant-numeric: tabular-nums;
  min-width: 1.5em;
  text-align: right;
}

/* Formula result fills the right content panel */
.calc-viewport {
  display: flex;
  align-items: center;
  justify-content: center;
}

.calc-panel {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  width: 100%;
  max-width: 100%;
  padding: 12px 8px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

.calc-panel .calc-expr {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.95rem;
  color: var(--muted);
  word-break: break-all;
  white-space: pre-wrap;
}

.calc-panel .calc-eq {
  font-size: 0.9rem;
  color: var(--accent);
  opacity: 0.85;
}

.calc-panel .calc-val {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: clamp(1.6rem, 4vw, 2.4rem);
  font-weight: 750;
  line-height: 1.2;
  color: #e8fff7;
  word-break: break-all;
}

.calc-panel .calc-val.bad {
  color: #f0b4b4;
  font-size: 1.2rem;
  font-weight: 650;
}

.search-plugin-viewport {
  overflow: auto;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.search-plugin-viewport::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}

.search-plugin-body {
  margin: 0;
  width: 100%;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.84rem;
  line-height: 1.55;
  color: #e6edf3;
}

/* Upper: editor; lower: absolute parse (timestamp / dict) */
.viewport-split {
  display: flex;
  flex-direction: column;
  gap: 0;
  /* keep horizontal room so child card borders are not clipped */
  overflow-x: hidden;
  overflow-y: hidden;
  min-width: 0;
  max-width: 100%;
  box-sizing: border-box;
  padding-bottom: 10px;
}

.editor-pane,
.content-pane {
  flex: 1 1 auto;
  min-height: 72px;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
}

.viewport-split .editor-pane,
.viewport-split .content-pane {
  flex: 1 1 45%;
  max-height: 50%;
  min-height: 64px;
}

.viewport-split .content-pane .img-wrap {
  min-height: 0;
  height: 100%;
  overflow: auto;
  scrollbar-width: none;
}

.viewport-split .content-pane .img-wrap::-webkit-scrollbar {
  display: none;
}

.viewport-split .content-pane .img-wrap img {
  max-height: 100%;
}

.abs-parse-row.on {
  border-color: rgba(62, 207, 173, 0.5);
  background: rgba(62, 207, 173, 0.1);
}

.abs-parse-row.rec:not(.on) {
  border-color: rgba(62, 207, 173, 0.22);
}

.abs-parse-pane {
  flex: 1 1 55%;
  min-height: 96px;
  min-width: 0;
  max-width: 100%;
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 8px;
  padding-top: 8px;
  /* no side margin that would push past viewport padding */
  border-top: 1px solid var(--line);
  overflow: hidden;
  box-sizing: border-box;
}

.abs-parse-pane-head {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 2px 2px 4px;
  font-size: 0.78rem;
  font-weight: 760;
  color: #b8f5e4;
  letter-spacing: 0.04em;
  box-sizing: border-box;
  text-shadow: 0 1px 0 rgba(0, 0, 0, 0.25);
}

.abs-parse-pane-hint {
  font-size: 0.7rem;
  font-weight: 680;
  color: #a8b4be;
  letter-spacing: 0.02em;
  opacity: 1;
}

.abs-parse-stack {
  flex: 1 1 auto;
  min-height: 0;
  min-width: 0;
  max-width: 100%;
  /* inset so 1px borders of cards never sit under overflow clip */
  padding: 2px 3px 8px 2px;
  overflow-x: hidden;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
  box-sizing: border-box;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.abs-parse-stack::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}

.abs-parse-row {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 3px;
  /* auto width inside padded stack — avoids 100% + border clipping */
  width: auto;
  max-width: 100%;
  margin: 0;
  padding: 8px 10px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.03);
  text-align: left;
  cursor: pointer;
  transition: border-color 0.12s ease, background 0.12s ease;
  box-sizing: border-box;
  flex: 0 0 auto;
  align-self: stretch;
}

.abs-parse-row:hover {
  border-color: rgba(62, 207, 173, 0.45);
  background: rgba(62, 207, 173, 0.07);
}

.abs-parse-row:active {
  background: rgba(62, 207, 173, 0.12);
}

.abs-parse-label {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 8px;
  max-width: 100%;
  font-size: 0.72rem;
  font-weight: 740;
  color: #c5d0d8;
  letter-spacing: 0.03em;
  box-sizing: border-box;
}

.abs-parse-label em {
  font-style: normal;
  font-weight: 720;
  font-size: 0.66rem;
  color: #9ff0d8;
  opacity: 1;
}

.abs-parse-value {
  max-width: 100%;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.86rem;
  font-weight: 650;
  line-height: 1.4;
  color: #e8fff7;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
  box-sizing: border-box;
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
  flex: 0 0 40px;
  height: 40px;
  min-height: 40px;
  max-height: 40px;
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 8px;
  padding: 0 14px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.04), rgba(255, 255, 255, 0.01));
  overflow: hidden;
  box-sizing: border-box;
}

/* Primary title: 内容 / 预览 */
.head-title {
  font-size: 0.92rem;
  font-weight: 760;
  letter-spacing: 0.04em;
  color: #f2f6fa;
  line-height: 1;
  flex-shrink: 0;
  text-shadow: 0 1px 0 rgba(0, 0, 0, 0.35);
}

/* Shared badge for 原文 / 解析 / 汉→英 / JSON … */
.head-tag {
  display: inline-flex;
  align-items: center;
  flex-shrink: 0;
  max-width: 9em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.72rem;
  font-weight: 740;
  line-height: 1.15;
  letter-spacing: 0.03em;
  padding: 4px 9px;
  border-radius: 999px;
  border: 1px solid transparent;
  box-sizing: border-box;
}

/* 原文 — solid accent, highest contrast */
.head-tag.tag-origin {
  color: #042019;
  background: var(--accent);
  border-color: rgba(62, 207, 173, 0.85);
  box-shadow: 0 0 0 1px rgba(62, 207, 173, 0.15);
}

/* 解析 — bright outline */
.head-tag.tag-parse {
  color: #9ff0d8;
  background: rgba(62, 207, 173, 0.16);
  border-color: rgba(62, 207, 173, 0.45);
}

/* 草稿 */
.head-tag.tag-draft {
  color: #ffe6a8;
  background: rgba(255, 200, 80, 0.14);
  border-color: rgba(255, 200, 80, 0.4);
}

/* 图片 等 */
.head-tag.tag-meta {
  color: #e8edf2;
  background: rgba(255, 255, 255, 0.1);
  border-color: rgba(255, 255, 255, 0.16);
}

/* 汉→英 / 英→汉 / 毫秒 / 解析 mode */
.head-tag.tag-mode {
  color: #b8f5e4;
  background: rgba(62, 207, 173, 0.18);
  border-color: rgba(62, 207, 173, 0.5);
}

/* JSON / SQL / Go … */
.head-tag.tag-lang {
  color: #c9ddff;
  background: rgba(110, 160, 255, 0.16);
  border-color: rgba(130, 170, 255, 0.42);
  letter-spacing: 0.04em;
}

/* Legacy .label / .chip kept for other drawers */
.label {
  font-size: 0.78rem;
  font-weight: 680;
  color: #c5d0d8;
  line-height: 1;
  flex-shrink: 0;
}

.chip {
  font-size: 0.68rem;
  font-weight: 740;
  line-height: 1.15;
  padding: 3px 8px;
  border-radius: 999px;
  color: #042019;
  background: var(--accent);
  flex-shrink: 0;
  border: 1px solid rgba(62, 207, 173, 0.7);
}

.chip.soft {
  color: #9ff0d8;
  background: rgba(62, 207, 173, 0.16);
  border-color: rgba(62, 207, 173, 0.4);
}

.head-action {
  margin-left: auto;
  flex-shrink: 0;
  min-height: 0 !important;
  height: auto;
  padding: 4px 10px !important;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 720;
  line-height: 1.15;
  color: #c5d0d8 !important;
  border: 1px solid rgba(255, 255, 255, 0.12);
  background: rgba(255, 255, 255, 0.04);
}

.head-action:hover {
  color: #f2f6fa !important;
  border-color: rgba(62, 207, 173, 0.4);
  background: rgba(62, 207, 173, 0.1);
}

.viewport {
  flex: 1 1 auto;
  min-height: 0;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  /* Keep scroll capability on children; hide scrollbar chrome */
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.viewport::-webkit-scrollbar {
  display: none;
  width: 0;
  height: 0;
}

.lang-chip {
  letter-spacing: 0.02em;
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

.stats-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.stat-card {
  border: 1px solid var(--line);
  border-radius: 10px;
  padding: 12px 10px;
  background: rgba(255, 255, 255, 0.02);
  text-align: center;
}

.stat-n {
  font-size: 1.25rem;
  font-weight: 720;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}

.stat-l {
  margin-top: 4px;
  font-size: 0.68rem;
  color: var(--muted);
}

.daily-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 6px;
  font-size: 0.72rem;
  color: var(--muted);
}

.daily-list li {
  display: grid;
  grid-template-columns: 42px 1fr 28px;
  gap: 8px;
  align-items: center;
}

.daily-bar-wrap {
  height: 6px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.06);
  overflow: hidden;
}

.daily-bar {
  display: block;
  height: 100%;
  border-radius: 999px;
  background: var(--accent);
  min-width: 2px;
}

.daily-n {
  text-align: right;
  font-variant-numeric: tabular-nums;
  color: var(--text);
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
  padding: 10px 12px;
  display: grid;
  /* 介绍 | 操作：左右一排；操作列内部再上下 */
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 10px 14px;
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
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  flex-shrink: 0;
  min-width: 2.75rem;
}

.plugin-actions > .switch,
.plugin-actions > .link,
.plugin-actions > button {
  margin: 0;
}

.plugin-actions .danger {
  color: #f0a0a0;
  font-size: 0.72rem;
  line-height: 1.2;
  text-align: center;
  white-space: nowrap;
}

.plugin-actions .link {
  font-size: 0.72rem;
  line-height: 1.2;
  text-align: center;
  white-space: nowrap;
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
