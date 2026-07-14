<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ask, open } from "@tauri-apps/plugin-dialog";
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
import { tryAbsoluteTimestampPanel, type AbsParseRow } from "./utils/timestampAbs";
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
  formatBadge,
  loadHistory,
  deleteItem,
  togglePinned,
  toggleFavorited,
  saveNote,
  copyItem,
  clearHistory,
  applySuggestion,
  applyAndCopySuggestion,
  showOriginal,
  onEditorInput,
  disposeHistory,
  refreshPluginSuggestions,
} = useHistory(setStatus);

/**
 * Left list tabs: 历史 keeps all records (pin/fav marks stay visible);
 * 钉住 / 收藏 are quick filters only — items do not leave the history feed.
 */
type HistTab = "history" | "pinned" | "favorites";
const histTab = ref<HistTab>("history");

const histTabCounts = computed(() => {
  let pinned = 0;
  let favorites = 0;
  for (const i of items.value) {
    if (i.pinned) pinned += 1;
    if (i.favorited) favorites += 1;
  }
  return {
    history: items.value.length,
    pinned,
    favorites,
  };
});

const tabItems = computed(() => {
  if (histTab.value === "pinned") {
    return items.value.filter((i) => i.pinned);
  }
  if (histTab.value === "favorites") {
    return items.value.filter((i) => i.favorited);
  }
  // 历史：全部记录（含钉住/收藏），按时间排序
  return items.value;
});

const tabCountLabel = computed(() => `${tabItems.value.length}`);

const clearMenuOpen = ref(false);

const clearStats = computed(() => {
  const all = items.value.length;
  const fav = items.value.filter((i) => i.favorited).length;
  const saved = items.value.filter((i) => i.pinned || i.favorited).length;
  return {
    all,
    fav,
    plain: Math.max(0, all - saved),
  };
});

function toggleClearMenu() {
  if (!items.value.length) return;
  clearMenuOpen.value = !clearMenuOpen.value;
}

async function onClearMode(mode: "keep_saved" | "all") {
  clearMenuOpen.value = false;
  if (!items.value.length) return;

  if (mode === "all") {
    const fav = clearStats.value.fav;
    const msg =
      fav > 0
        ? `将删除全部 ${clearStats.value.all} 条记录（含 ${fav} 条收藏），确定？`
        : `将删除全部 ${clearStats.value.all} 条记录，确定？`;
    let ok = false;
    try {
      ok = await ask(msg, {
        title: "全部清空",
        kind: "warning",
        okLabel: "全部删除",
        cancelLabel: "取消",
      });
    } catch {
      ok = false;
    }
    if (!ok) {
      setStatus("已取消清空", "muted");
      return;
    }
  }

  await clearHistory(mode);
}

function onGlobalPointerDown(ev: MouseEvent) {
  const t = ev.target as HTMLElement | null;
  if (!t?.closest?.(".clear-menu-wrap")) {
    clearMenuOpen.value = false;
  }
}

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

/** Favorite note editor (shown under detail head when favorited). */
const noteDraft = ref("");
const noteSaving = ref(false);
const noteInputEl = ref<HTMLInputElement | null>(null);
const noteFocusToken = ref(0);

watch(
  () => selectedItem.value?.id,
  () => {
    noteDraft.value = selectedItem.value?.note?.trim() || "";
  },
);

watch(
  () => selectedItem.value?.note,
  (n) => {
    // Sync when patch updates note without id change
    if (!noteSaving.value) {
      noteDraft.value = n?.trim() || "";
    }
  },
);

async function onToggleFavorite() {
  const item = selectedItem.value;
  if (!item) return;
  const wasFav = !!item.favorited;
  const nowFav = await toggleFavorited(item);
  // After first favorite: focus note field so user can tag it for search
  if (nowFav && !wasFav) {
    noteDraft.value = selectedItem.value?.note?.trim() || "";
    noteFocusToken.value += 1;
    await nextTick();
    noteInputEl.value?.focus();
  }
}

async function onSaveNote() {
  const item = selectedItem.value;
  if (!item?.favorited) return;
  noteSaving.value = true;
  try {
    await saveNote(item, noteDraft.value);
  } finally {
    noteSaving.value = false;
  }
}

/** Image OCR result (local Vision / Windows OCR). */
const ocrBusy = ref(false);
const ocrText = ref("");
const ocrError = ref("");
const ocrForId = ref<string | null>(null);

watch(
  () => selectedItem.value?.id,
  () => {
    ocrText.value = "";
    ocrError.value = "";
    ocrForId.value = null;
    ocrBusy.value = false;
  },
);

const imageOcrRows = computed<AbsParseRow[] | null>(() => {
  if (!selectedItem.value || selectedItem.value.kind !== "image") return null;
  if (ocrForId.value !== selectedItem.value.id) return null;
  const text = ocrText.value.trim();
  if (!text) return null;
  const lines = text
    .split(/\n/)
    .map((l) => l.trim())
    .filter(Boolean);
  // First row = full OCR text (one click copies everything); then per-line rows.
  const rows: AbsParseRow[] = [
    {
      id: "ocr-all",
      label: "全部结果",
      value: text,
      hint: "点击复制",
    },
  ];
  if (lines.length > 1) {
    for (let i = 0; i < lines.length; i++) {
      rows.push({
        id: `ocr-line-${i}`,
        label: `行 ${i + 1}`,
        value: lines[i],
      });
    }
  }
  return rows;
});

async function runImageOcr() {
  const item = selectedItem.value;
  if (!item || item.kind !== "image") return;
  ocrBusy.value = true;
  ocrError.value = "";
  try {
    const text = await api.ocrHistoryImage(item.id);
    ocrText.value = text;
    ocrForId.value = item.id;
    setStatus("识别完成", "ok");
  } catch (error) {
    ocrText.value = "";
    ocrForId.value = item.id;
    ocrError.value = error instanceof Error ? error.message : String(error);
    setStatus(ocrError.value, "err");
  } finally {
    ocrBusy.value = false;
  }
}

/**
 * Show recommended parse in the lower half (like abs timestamp/dict),
 * when content is short enough or absolute panel applies.
 */
const showLowerParse = computed(() => {
  if (!selectedItem.value) return false;
  if (absParsePanel.value) return true;
  if (imageOcrRows.value || (selectedItem.value.kind === "image" && (ocrBusy.value || ocrError.value))) {
    return true;
  }
  if (!suggestions.value.length) return false;
  return contentFitsHalf.value;
});

const lowerParseTitle = computed(() => {
  if (imageOcrRows.value || (selectedItem.value?.kind === "image" && (ocrBusy.value || ocrError.value))) {
    return "图片文字";
  }
  if (absParsePanel.value) return absParsePanel.value.label;
  return "推荐解析";
});

const lowerParseHint = computed(() => {
  if (imageOcrRows.value) return "点击复制";
  if (selectedItem.value?.kind === "image" && ocrBusy.value) return "识别中…";
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
  isMac,
  loadConfig,
  onHotkeyKeydown,
  saveConfig,
  defaultToggleHotkey,
  defaultAppendCopyHotkey,
  defaultParseCopyHotkey,
  defaultPasteHotkey,
} = useSettings(setStatus);

/** App version from Tauri package (tauri.conf.json / Cargo). */
const appVersion = ref("0.1.0");

/** Settings → export options */
const exportIncludeImages = ref(true);
const exportIncludeConfig = ref(true);
const exportBusy = ref(false);

async function pickExportDir(title: string): Promise<string | null> {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title,
    });
    if (!selected || Array.isArray(selected)) return null;
    return selected;
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), "err");
    return null;
  }
}

async function onExportSaved() {
  if (exportBusy.value) return;
  const dest = await pickExportDir("选择导出目录（收藏与钉住）");
  if (!dest) return;
  exportBusy.value = true;
  try {
    const r = await api.exportData("saved", dest, {
      includeImages: exportIncludeImages.value,
    });
    setStatus(`已导出收藏/钉住 ${r.count} 条 → ${r.path}`, "ok");
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), "err");
  } finally {
    exportBusy.value = false;
  }
}

async function onExportFull() {
  if (exportBusy.value) return;
  const dest = await pickExportDir("选择导出目录（完整库备份）");
  if (!dest) return;
  exportBusy.value = true;
  try {
    const r = await api.exportData("full", dest, {
      includeConfig: exportIncludeConfig.value,
    });
    setStatus(`已导出完整库 ${r.count} 条 → ${r.path}`, "ok");
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), "err");
  } finally {
    exportBusy.value = false;
  }
}

async function pickImportDir(title: string): Promise<string | null> {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title,
    });
    if (!selected || Array.isArray(selected)) return null;
    return selected;
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), "err");
    return null;
  }
}

async function onImportSaved() {
  if (exportBusy.value) return;
  const src = await pickImportDir("选择 FunCV-saved-… 导出目录");
  if (!src) return;
  exportBusy.value = true;
  try {
    const r = await api.importData("saved", src);
    await loadHistory({ silent: true });
    const skip =
      r.skipped > 0 ? `，跳过 ${r.skipped}` : "";
    setStatus(`已导入收藏/钉住 ${r.count} 条${skip}`, "ok");
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), "err");
  } finally {
    exportBusy.value = false;
  }
}

async function onImportFull() {
  if (exportBusy.value) return;
  const src = await pickImportDir("选择 FunCV-full-… 完整备份目录");
  if (!src) return;

  let ok = false;
  try {
    ok = await ask(
      "导入完整库将覆盖当前全部历史记录与图片，确定继续？建议先导出备份。",
      {
        title: "导入完整库",
        kind: "warning",
        okLabel: "覆盖导入",
        cancelLabel: "取消",
      },
    );
  } catch {
    ok = false;
  }
  if (!ok) {
    setStatus("已取消导入", "muted");
    return;
  }

  exportBusy.value = true;
  try {
    const r = await api.importData("full", src, {
      includeConfig: exportIncludeConfig.value,
    });
    // Config may have changed; reload UI state
    await loadConfig();
    await loadHistory({ silent: true });
    setStatus(`已导入完整库 · ${r.count} 条历史`, "ok");
  } catch (error) {
    setStatus(error instanceof Error ? error.message : String(error), "err");
  } finally {
    exportBusy.value = false;
  }
}

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

const searchInputEl = ref<HTMLInputElement | null>(null);

/** Focus search when main UI is shown (hotkey / tray). Skip if a drawer is open. */
function focusSearchInput() {
  if (settingsOpen.value || pluginsOpen.value || statsOpen.value) return;
  if (hotkeyRecording.value) return;
  void nextTick(() => {
    const el = searchInputEl.value;
    if (!el) return;
    el.focus();
    // Place caret at end so typing continues the query
    const len = el.value.length;
    try {
      el.setSelectionRange(len, len);
    } catch {
      /* ignore */
    }
  });
}

const unlisteners: UnlistenFn[] = [];

onMounted(() => {
  window.addEventListener("keydown", onGlobalKeydown);
  window.addEventListener("pointerdown", onGlobalPointerDown, true);

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
  void listen("focus-search", () => {
    // Delay slightly so window focus settles (esp. macOS raise path).
    setTimeout(() => focusSearchInput(), 30);
  }).then((u) => {
    unlisteners.push(u);
  });

  // Yield a frame so the shell is interactive before IPC-heavy bootstrap.
  requestAnimationFrame(() => {
    void loadConfig();
    void getVersion()
      .then((v) => {
        if (v) appVersion.value = v;
      })
      .catch(() => {
        /* browser preview: keep package default */
      });
    // Slight delay: list + hydrate + plugins should not compete with first drag/click.
    setTimeout(() => {
      void loadHistory();
      focusSearchInput();
    }, 50);
  });
});

onUnmounted(() => {
  window.removeEventListener("keydown", onGlobalKeydown);
  window.removeEventListener("pointerdown", onGlobalPointerDown, true);
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
        <button type="button" class="link" @click="openPlugins">插件</button>
        <button type="button" class="link" @click="openStats">统计</button>
        <div class="clear-menu-wrap">
          <button
            type="button"
            class="link"
            :disabled="!items.length"
            :aria-expanded="clearMenuOpen"
            aria-haspopup="menu"
            @click="toggleClearMenu"
          >
            清空
          </button>
          <div v-if="clearMenuOpen" class="clear-menu" role="menu">
            <button
              type="button"
              class="clear-menu-item"
              role="menuitem"
              :disabled="clearStats.plain === 0"
              @click="onClearMode('keep_saved')"
            >
              <span class="clear-menu-title">普通</span>
              <span class="clear-menu-desc">保留钉住与收藏 · {{ clearStats.plain }} 条</span>
            </button>
            <button
              type="button"
              class="clear-menu-item danger"
              role="menuitem"
              @click="onClearMode('all')"
            >
              <span class="clear-menu-title">全部</span>
              <span class="clear-menu-desc">含收藏 / 钉住 · {{ clearStats.all }} 条</span>
            </button>
          </div>
        </div>
        <button type="button" class="link" @click="settingsOpen = true">设置</button>
      </nav>
    </header>

    <div class="search-wrap">
      <div class="search-field">
        <input
          ref="searchInputEl"
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
      <span class="count">{{ tabCountLabel }}</span>
    </div>

    <div class="body">
      <section class="list" aria-label="历史">
        <div class="hist-tabs" role="tablist" aria-label="历史分类">
          <button
            type="button"
            class="hist-tab"
            role="tab"
            :class="{ on: histTab === 'history' }"
            :aria-selected="histTab === 'history'"
            @click="histTab = 'history'"
          >
            历史<span class="hist-tab-n">{{ histTabCounts.history }}</span>
          </button>
          <button
            type="button"
            class="hist-tab"
            role="tab"
            :class="{ on: histTab === 'pinned' }"
            :aria-selected="histTab === 'pinned'"
            @click="histTab = 'pinned'"
          >
            钉住<span class="hist-tab-n">{{ histTabCounts.pinned }}</span>
          </button>
          <button
            type="button"
            class="hist-tab"
            role="tab"
            :class="{ on: histTab === 'favorites' }"
            :aria-selected="histTab === 'favorites'"
            @click="histTab = 'favorites'"
          >
            收藏<span class="hist-tab-n">{{ histTabCounts.favorites }}</span>
          </button>
        </div>
        <div v-if="!hasLoaded && loading" class="empty">加载中…</div>
        <div v-else-if="tabItems.length === 0" class="empty">
          <template v-if="histTab === 'pinned'">
            <strong>暂无钉住</strong>
            <span>在内容区点图钉可钉住条目</span>
          </template>
          <template v-else-if="histTab === 'favorites'">
            <strong>暂无收藏</strong>
            <span>在内容区点星标可收藏并写备注</span>
          </template>
          <template v-else>
            <strong>暂无记录</strong>
            <span>复制后自动出现 · 双击复制当前面板内容</span>
          </template>
        </div>
        <ul v-else>
          <li
            v-for="item in tabItems"
            :key="item.id"
            :class="{ on: item.id === selectedItem?.id, pinned: item.pinned, favorited: item.favorited }"
            @click="selectedId = item.id"
            @dblclick="copyItem(item)"
          >
            <span class="dot" :data-k="item.kind" />
            <div class="meta">
              <p class="preview">
                <span v-if="item.pinned" class="list-mark pin" title="已钉住">⊙</span>
                <span v-if="item.favorited" class="list-mark fav" title="已收藏">★</span>
                {{ item.preview }}
              </p>
              <p v-if="item.note" class="list-note" :title="item.note">{{ item.note }}</p>
              <time>{{ formatHistoryTime(item.copiedAt) }}</time>
            </div>
            <button type="button" class="icon-del" title="删除" @click.stop="deleteItem(item)">
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
              v-if="selectedItem.kind === 'image'"
              type="button"
              class="link head-action ocr-action"
              :disabled="ocrBusy"
              title="本地识别图片中的文字"
              @click="runImageOcr"
            >
              {{ ocrBusy ? "识别中…" : "识别文字" }}
            </button>
            <button
              v-if="(editorDirty || !isShowingOriginal) && selectedItem.kind === 'text'"
              type="button"
              class="link head-action"
              @click="showOriginal"
            >
              恢复原文
            </button>
            <!-- Far right, flush to panel edge -->
            <div class="head-icons" role="group" aria-label="钉住与收藏">
              <button
                type="button"
                class="head-icon-btn"
                :class="{ on: selectedItem.pinned }"
                :title="selectedItem.pinned ? '取消钉住' : '钉住（列表置顶，不易被清理）'"
                :aria-pressed="selectedItem.pinned"
                @click="togglePinned(selectedItem)"
              >
                <svg class="head-icon-svg" viewBox="0 0 24 24" aria-hidden="true">
                  <!-- thumbtack / push-pin -->
                  <path
                    fill="currentColor"
                    d="M16 4V3a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v1H7a1 1 0 1 0 0 2h.3l.9 4.3A4 4 0 0 0 7 14v1h4v6a1 1 0 1 0 2 0v-6h4v-1a4 4 0 0 0-1.2-2.7L16.7 6H17a1 1 0 1 0 0-2h-1zm-5.05 2h2.1l.85 4.1.2.9.7.6c.4.35.7.9.7 1.4H9.5c0-.5.3-1.05.7-1.4l.7-.6.2-.9L10.95 6z"
                  />
                </svg>
              </button>
              <button
                type="button"
                class="head-icon-btn"
                :class="{ on: !!selectedItem.favorited }"
                :title="selectedItem.favorited ? '取消收藏' : '收藏（可写备注并搜索）'"
                :aria-pressed="!!selectedItem.favorited"
                @click="onToggleFavorite"
              >
                <svg class="head-icon-svg" viewBox="0 0 24 24" aria-hidden="true">
                  <path
                    v-if="selectedItem.favorited"
                    fill="currentColor"
                    d="M12 2.5l2.9 6.2 6.8.7-5.1 4.6 1.5 6.6L12 17.3 5.9 20.6l1.5-6.6L2.3 9.4l6.8-.7L12 2.5z"
                  />
                  <path
                    v-else
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.8"
                    stroke-linejoin="round"
                    d="M12 3.2l2.5 5.4 5.9.6-4.4 4 1.3 5.8L12 16.2 6.7 19l1.3-5.8-4.4-4 5.9-.6L12 3.2z"
                  />
                </svg>
              </button>
            </div>
          </div>

          <!-- Favorite note: under title bar; searchable from top search -->
          <div v-if="selectedItem.favorited" class="note-bar">
            <span class="note-label">备注</span>
            <input
              :key="`note-${selectedItem.id}-${noteFocusToken}`"
              ref="noteInputEl"
              v-model="noteDraft"
              class="note-input"
              type="text"
              maxlength="200"
              placeholder="给收藏写备注，可在上方搜索…"
              spellcheck="false"
              @keydown.enter.prevent="onSaveNote"
            />
            <button
              type="button"
              class="link note-save"
              :disabled="noteSaving || noteDraft.trim() === (selectedItem.note || '').trim()"
              @click="onSaveNote"
            >
              {{ noteSaving ? "…" : "保存" }}
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

            <!-- Lower half: OCR / absolute rows / general recommendations -->
            <div v-if="showLowerParse" class="abs-parse-pane" role="list" aria-label="解析结果">
              <div class="abs-parse-pane-head">
                <span>{{ lowerParseTitle }}</span>
                <span class="abs-parse-pane-hint">{{ lowerParseHint }}</span>
              </div>
              <div class="abs-parse-stack">
                <template v-if="selectedItem.kind === 'image' && ocrBusy">
                  <p class="ocr-status muted">正在本地识别文字…</p>
                </template>
                <template v-else-if="selectedItem.kind === 'image' && ocrError && !imageOcrRows">
                  <p class="ocr-status ocr-err">{{ ocrError }}</p>
                </template>
                <template v-else-if="imageOcrRows">
                  <button
                    v-for="row in imageOcrRows"
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
                <template v-else-if="absParsePanel">
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
          <h2>设置 <span class="settings-ver">v{{ appVersion }}</span></h2>
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
        <div class="watch-types">
          <span class="watch-types-label">监听类型</span>
          <label class="check-row">
            <input v-model="config.watchText" type="checkbox" />
            <span>文本</span>
          </label>
          <label class="check-row">
            <input v-model="config.watchImage" type="checkbox" />
            <span>图片</span>
          </label>
        </div>
        <p class="tip">仅勾选的类型会写入历史；默认文本 + 图片都监听。至少保留一项。</p>
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
        <label class="hotkey-field">
          快速粘贴快捷键
          <input
            class="hotkey-input"
            :class="{ recording: hotkeyRecording === 'paste' }"
            type="text"
            readonly
            :value="
              hotkeyRecording === 'paste'
                ? '按下组合键…（Delete 清空）'
                : config.pasteHotkey || '未设置（已关闭）'
            "
            :placeholder="defaultPasteHotkey()"
            @focus="hotkeyRecording = 'paste'"
            @click="hotkeyRecording = 'paste'"
            @blur="hotkeyRecording = null"
            @keydown="onHotkeyKeydown($event, 'paste')"
          />
        </label>
        <p class="tip">
          <template v-if="isMac">
            <strong>快速粘贴</strong>：在任意应用按快捷键（默认 Cmd+F2）→ 在光标旁弹出精简列表（最近 50
            条，可滚动），⌘1–9 或点击即粘贴到原光标处；「查看更多」才打开主界面。<br />
            <strong>复制并解析</strong>：在任意应用选中文本后按快捷键 → 自动复制并弹出 FunCV
            解析（默认 Shift+Option+X）。<br />
            <strong>追加复制</strong>：剪贴板已是文本时，把选中内容用双空格接在后面；重复追加同一段只生效一次。需在「系统设置
            → 隐私与安全性 → 辅助功能」中允许 FunCV。
          </template>
          <template v-else>
            <strong>快速粘贴</strong>：在任意应用按快捷键（默认 Ctrl+F2）→ 在光标旁弹出精简列表（最近 50
            条，可滚动），Ctrl+1–9 或点击即粘贴到原光标处；「查看更多」才打开主界面。<br />
            <strong>复制并解析</strong>：在任意应用选中文本后按快捷键 → 自动复制并弹出 FunCV
            解析（默认 Shift+Alt+X）。<br />
            <strong>追加复制</strong>：剪贴板已是文本时，把选中内容用双空格接在后面；重复追加同一段只生效一次。若模拟按键失败，请检查系统是否拦截后台注入。
          </template>
        </p>

        <div class="export-box">
          <div class="export-head">
            <strong>数据导出 / 导入</strong>
          </div>
          <div class="watch-types export-opts">
            <label class="check-row">
              <input v-model="exportIncludeImages" type="checkbox" />
              <span>收藏导出含图片</span>
            </label>
            <label class="check-row">
              <input v-model="exportIncludeConfig" type="checkbox" />
              <span>完整导出/导入含配置</span>
            </label>
          </div>
          <div class="export-actions">
            <button
              type="button"
              class="link export-btn"
              :disabled="exportBusy"
              title="导出 JSON + 可选图片，便于备份收藏"
              @click="onExportSaved"
            >
              导出收藏和钉住
            </button>
            <button
              type="button"
              class="primary export-btn"
              :disabled="exportBusy"
              title="复制 history.db 与 images，便于整库迁移"
              @click="onExportFull"
            >
              导出全部
            </button>
          </div>
          <div class="export-actions">
            <button
              type="button"
              class="link export-btn"
              :disabled="exportBusy"
              title="选择 FunCV-saved-… 目录，合并收藏与钉住"
              @click="onImportSaved"
            >
              导入收藏和钉住
            </button>
            <button
              type="button"
              class="link export-btn danger-text"
              :disabled="exportBusy"
              title="选择 FunCV-full-… 目录，覆盖当前库"
              @click="onImportFull"
            >
              导入全部
            </button>
          </div>
          <p class="tip">
            <strong>收藏和钉住</strong>：导出/导入 <code>export.json</code> + 可选
            <code>images/</code>；导入按内容合并（同 hash 合并钉住/收藏/备注）。<br />
            <strong>全部</strong>：导出复制库文件；导入将<strong>覆盖</strong>当前历史与图片，请先备份。
          </p>
        </div>

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

.clear-menu-wrap {
  position: relative;
  display: inline-flex;
}

.clear-menu {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  z-index: 40;
  min-width: 200px;
  padding: 6px;
  border-radius: 12px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  background: #151b22;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45);
  display: grid;
  gap: 2px;
}

.clear-menu-item {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  width: 100%;
  margin: 0;
  padding: 8px 10px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--text);
  text-align: left;
  cursor: pointer;
}

.clear-menu-item:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.06);
}

.clear-menu-item:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.clear-menu-title {
  font-size: 0.8rem;
  font-weight: 700;
  line-height: 1.25;
}

.clear-menu-desc {
  font-size: 0.68rem;
  color: var(--muted);
  line-height: 1.3;
}

.clear-menu-item.danger .clear-menu-title {
  color: #f0b4b4;
}

.clear-menu-item.danger:hover:not(:disabled) {
  background: rgba(180, 70, 70, 0.14);
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

/* Shared panel header height (tabs ↔ content title stay aligned) */
/* hist-tabs + detail-head both 36px */

/* Compact history tabs — pin/fav no longer bury the main feed */
.hist-tabs {
  flex: 0 0 36px;
  height: 36px;
  min-height: 36px;
  max-height: 36px;
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  align-items: center;
  gap: 3px;
  padding: 0 6px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.04), rgba(255, 255, 255, 0.01));
  box-sizing: border-box;
}

.hist-tab {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  height: 26px;
  margin: 0;
  padding: 0 6px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--muted);
  font-size: 0.72rem;
  font-weight: 680;
  letter-spacing: 0.02em;
  line-height: 1;
  cursor: pointer;
  transition: color 0.12s ease, background 0.12s ease;
}

.hist-tab:hover {
  color: #dce4eb;
  background: rgba(255, 255, 255, 0.05);
}

.hist-tab.on {
  color: #042019;
  background: var(--accent);
  font-weight: 740;
}

.hist-tab-n {
  font-size: 0.64rem;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  opacity: 0.75;
  min-width: 0.9em;
  text-align: center;
}

.hist-tab.on .hist-tab-n {
  opacity: 0.85;
}

.list ul {
  list-style: none;
  margin: 0;
  padding: 6px;
  flex: 1 1 auto;
  min-height: 0;
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
  /* Match .hist-tabs row height (36px) for horizontal align with left panel */
  flex: 0 0 36px;
  height: 36px;
  min-height: 36px;
  max-height: 36px;
  display: flex;
  align-items: center;
  flex-wrap: nowrap;
  gap: 6px;
  padding: 0 6px 0 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.04), rgba(255, 255, 255, 0.01));
  overflow: hidden;
  box-sizing: border-box;
}

/* Primary title: 内容 / 预览 — same scale as hist-tab labels */
.head-title {
  font-size: 0.78rem;
  font-weight: 740;
  letter-spacing: 0.04em;
  color: #f2f6fa;
  line-height: 1;
  flex-shrink: 0;
}

/* Pin / favorite — always far right, flush to edge */
.head-icons {
  display: inline-flex;
  align-items: center;
  gap: 0;
  flex-shrink: 0;
  margin-left: auto;
  margin-right: 0;
}

.head-icon-btn {
  display: grid;
  place-items: center;
  width: 26px;
  height: 26px;
  margin: 0;
  padding: 0;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: #8a969f;
  cursor: pointer;
  transition: color 0.12s ease, background 0.12s ease, opacity 0.12s ease;
}

.head-icon-btn:hover {
  color: #e8edf2;
  background: rgba(255, 255, 255, 0.06);
}

.head-icon-btn.on {
  color: var(--accent);
  background: transparent;
}

.head-icon-btn.on:hover {
  color: #7aebc8;
  background: rgba(62, 207, 173, 0.1);
}

.head-icon-svg {
  width: 14px;
  height: 14px;
  display: block;
}

.list-mark {
  display: inline-block;
  margin-right: 4px;
  font-size: 0.72rem;
  vertical-align: baseline;
  opacity: 0.95;
}

.list-mark.fav {
  color: #f0c45a;
}

.list li.pinned .preview {
  font-weight: 650;
}

.list-note {
  margin: 0 0 2px;
  font-size: 0.68rem;
  line-height: 1.3;
  color: #f0c45a;
  opacity: 0.92;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.note-bar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 36px;
  padding: 6px 12px 6px 14px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  background: rgba(240, 196, 90, 0.06);
  box-sizing: border-box;
}

.note-label {
  flex-shrink: 0;
  font-size: 0.72rem;
  font-weight: 740;
  color: #f0c45a;
  letter-spacing: 0.04em;
}

.note-input {
  flex: 1 1 auto;
  min-width: 0;
  height: 28px;
  margin: 0;
  padding: 0 10px;
  border: 1px solid rgba(240, 196, 90, 0.28);
  border-radius: 8px;
  background: #0b1014;
  color: #e8edf2;
  font: inherit;
  font-size: 0.8rem;
  outline: none;
  box-sizing: border-box;
}

.note-input::placeholder {
  color: #7a8792;
  opacity: 0.85;
}

.note-input:focus {
  border-color: rgba(240, 196, 90, 0.55);
  box-shadow: 0 0 0 2px rgba(240, 196, 90, 0.12);
}

.note-save {
  flex-shrink: 0;
  font-size: 0.72rem !important;
  font-weight: 720;
  color: #f0c45a !important;
  padding: 4px 8px !important;
}

.note-save:disabled {
  opacity: 0.35;
}

/* Shared badge for 原文 / 解析 / 汉→英 / JSON … */
.head-tag {
  display: inline-flex;
  align-items: center;
  flex-shrink: 0;
  max-width: 8em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.66rem;
  font-weight: 740;
  line-height: 1;
  letter-spacing: 0.02em;
  padding: 3px 8px;
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
  /* stay with tags; pin/fav use margin-left:auto on .head-icons */
  margin-left: 0;
  flex-shrink: 0;
  min-height: 0 !important;
  height: 24px !important;
  padding: 0 9px !important;
  border-radius: 999px;
  font-size: 0.66rem;
  font-weight: 720;
  line-height: 1;
  color: #c5d0d8 !important;
  border: 1px solid rgba(255, 255, 255, 0.12);
  background: rgba(255, 255, 255, 0.04);
}

.head-action:hover {
  color: #f2f6fa !important;
  border-color: rgba(62, 207, 173, 0.4);
  background: rgba(62, 207, 173, 0.1);
}

.head-action.ocr-action {
  color: #b8f5e4 !important;
  border-color: rgba(62, 207, 173, 0.45);
  background: rgba(62, 207, 173, 0.12);
  font-weight: 740;
}

.head-action.ocr-action:disabled {
  opacity: 0.55;
}

.ocr-status {
  margin: 4px 2px;
  font-size: 0.8rem;
  line-height: 1.45;
}

.ocr-status.ocr-err {
  color: #f0b4b4;
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

.settings-ver {
  margin-left: 8px;
  font-size: 0.72rem;
  font-weight: 650;
  color: var(--muted);
  letter-spacing: 0.03em;
  vertical-align: middle;
}

.export-box {
  border: 1px solid var(--line);
  border-radius: 10px;
  padding: 10px 12px;
  display: grid;
  gap: 8px;
  background: rgba(255, 255, 255, 0.02);
}

.export-head {
  font-size: 0.84rem;
  color: #e8edf2;
}

.export-opts {
  padding: 0;
}

.export-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.export-btn {
  flex: 1 1 auto;
  min-width: 7.5rem;
  justify-content: center;
}

.export-box .tip code {
  font-size: 0.7rem;
  color: var(--accent);
}

.export-btn.danger-text {
  color: #f0b4b4 !important;
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

.watch-types {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px 16px;
  padding: 4px 0;
}

.watch-types-label {
  font-size: 0.78rem;
  font-weight: 700;
  color: #c5d0d8;
  margin-right: 4px;
}

.check-row {
  display: inline-flex !important;
  align-items: center;
  gap: 8px;
  grid-template-columns: none !important;
  color: #e8edf2 !important;
  font-size: 0.84rem !important;
  font-weight: 650;
  cursor: pointer;
  user-select: none;
}

.check-row input[type="checkbox"] {
  width: 16px;
  height: 16px;
  margin: 0;
  padding: 0;
  border-radius: 4px;
  accent-color: var(--accent);
  cursor: pointer;
  flex-shrink: 0;
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
