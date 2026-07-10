import { computed, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import * as api from "../api";
import type { HistoryItem, PanelContent, ParseSuggestion } from "../core/types";
import { resolveContent } from "../extensions";

const CACHE_MAX = 100;

/** Simple FIFO-ish cache trim (Map preserves insertion order). */
function cacheSet<V>(map: Map<string, V>, key: string, value: V) {
  if (map.has(key)) map.delete(key);
  map.set(key, value);
  while (map.size > CACHE_MAX) {
    const oldest = map.keys().next().value;
    if (oldest === undefined) break;
    map.delete(oldest);
  }
}

/**
 * Parse cache policy:
 * - Built-in extensions (JSON/SQL/…): once per content hash.
 * - Plugins: once per (content hash + enabled plugin set); re-run only when plugins change.
 */
export function useHistory(setStatus: (msg: string, tone?: "muted" | "ok" | "err") => void) {
  const query = ref("");
  const items = ref<HistoryItem[]>([]);
  const loading = ref(false);
  const hasLoaded = ref(false);
  const selectedId = ref("");
  const imageSrc = ref("");
  const hydrating = ref(false);

  const appliedSuggestionId = ref<string | null>(null);
  const appliedBody = ref<string | null>(null);
  /**
   * Right-panel draft text (text items only).
   * Editing is ephemeral — does NOT write back to history DB.
   */
  const editorText = ref("");
  /** True when panel draft differs from stored original. */
  const editorDirty = ref(false);

  const builtinSuggestions = ref<ParseSuggestion[]>([]);
  const pluginSuggestions = ref<ParseSuggestion[]>([]);

  /** contentHash → builtin suggestions */
  const builtinCache = new Map<string, ParseSuggestion[]>();
  /** `${contentHash}::${pluginSig}` → plugin suggestions */
  const pluginCache = new Map<string, ParseSuggestion[]>();
  /** Enabled plugin fingerprint; null = not loaded yet */
  let pluginSig: string | null = null;

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let hydrateToken = 0;
  let pluginToken = 0;

  const selectedItem = computed(
    () => items.value.find((i) => i.id === selectedId.value) ?? items.value[0] ?? null,
  );

  const suggestions = computed(() => [
    ...builtinSuggestions.value,
    ...pluginSuggestions.value,
  ]);
  const itemCountLabel = computed(() => `${items.value.length}`);

  const panelContent = computed((): PanelContent => {
    const item = selectedItem.value;
    if (!item) return { mode: "empty" };
    if (item.kind === "image") return { mode: "image" };
    if (hydrating.value && item.text == null) {
      return { mode: "plain", body: "加载中…" };
    }
    // Editor is source of truth while viewing text
    return { mode: "plain", body: editorText.value };
  });

  const isShowingOriginal = computed(() => appliedSuggestionId.value == null);
  const isTextEditor = computed(
    () => selectedItem.value?.kind === "text" && !hydrating.value,
  );

  const formatBadge = computed(() => {
    if (!selectedItem.value) return "";
    if (selectedItem.value.kind === "image") return "图片";
    if (editorDirty.value) return "草稿";
    if (!isShowingOriginal.value) return "解析";
    return "原文";
  });

  function syncEditorFromItem(item: HistoryItem | null) {
    if (!item || item.kind !== "text") {
      editorText.value = "";
      editorDirty.value = false;
      return;
    }
    if (appliedBody.value != null) {
      editorText.value = appliedBody.value;
      editorDirty.value = appliedBody.value !== (item.text ?? "");
    } else if (item.text != null) {
      editorText.value = item.text;
      editorDirty.value = false;
    } else {
      editorText.value = "";
      editorDirty.value = false;
    }
  }

  function resetApplied() {
    appliedSuggestionId.value = null;
    appliedBody.value = null;
  }

  function applySuggestion(s: ParseSuggestion) {
    appliedSuggestionId.value = s.id;
    appliedBody.value = s.body;
    editorText.value = s.body;
    const orig = selectedItem.value?.text ?? "";
    editorDirty.value = s.body !== orig;
    setStatus(`已应用 · ${s.title}（不写入历史）`, "ok");
  }

  /** Local edit only — history row stays unchanged. */
  function onEditorInput(value: string) {
    editorText.value = value;
    appliedSuggestionId.value = null;
    appliedBody.value = null;
    const orig = selectedItem.value?.text ?? "";
    editorDirty.value = value !== orig;
  }

  async function applyAndCopySuggestion(s: ParseSuggestion) {
    applySuggestion(s);
    const item = selectedItem.value;
    if (!item) {
      setStatus("无法复制", "err");
      return;
    }
    try {
      await api.copyHistory(item.id, s.body);
      setStatus(`已复制 · ${s.title}`, "ok");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    }
  }

  function showOriginal() {
    resetApplied();
    const item = selectedItem.value;
    if (item?.kind === "text" && item.text != null) {
      editorText.value = item.text;
      editorDirty.value = false;
    }
    setStatus("已恢复原文", "ok");
  }

  function mergeItem(full: HistoryItem) {
    const idx = items.value.findIndex((i) => i.id === full.id);
    if (idx >= 0) {
      items.value[idx] = full;
    }
  }

  function contentKey(item: HistoryItem): string {
    return item.hash || item.id;
  }

  function pluginCacheKey(item: HistoryItem, sig: string): string {
    return `${contentKey(item)}::${sig}`;
  }

  /** Built-in analyzers: parse once per content hash. */
  function updateBuiltinForItem(item: HistoryItem | null) {
    if (!item) {
      builtinSuggestions.value = [];
      return;
    }
    // Need full text for text analysis
    if (item.kind === "text" && item.text == null) {
      builtinSuggestions.value = [];
      return;
    }
    const key = contentKey(item);
    const hit = builtinCache.get(key);
    if (hit) {
      builtinSuggestions.value = hit;
      return;
    }
    const resolved = resolveContent(item);
    cacheSet(builtinCache, key, resolved.suggestions);
    builtinSuggestions.value = resolved.suggestions;
  }

  async function ensurePluginSig(): Promise<string> {
    if (pluginSig !== null) return pluginSig;
    try {
      const list = (await api.listPlugins()) || [];
      pluginSig = list
        .filter((p) => p.enabled)
        .map((p) => `${p.id}@${p.version || "0"}`)
        .sort()
        .join("|");
    } catch {
      pluginSig = "";
    }
    return pluginSig;
  }

  /** Restore plugin suggestions from cache if present; returns true on hit. */
  function tryRestorePluginCache(item: HistoryItem): boolean {
    if (pluginSig === null) return false;
    const key = pluginCacheKey(item, pluginSig);
    const hit = pluginCache.get(key);
    if (!hit) return false;
    pluginSuggestions.value = hit;
    return true;
  }

  async function hydrateSelected() {
    const id = selectedId.value;
    if (!id) return;
    const item = items.value.find((i) => i.id === id);
    if (!item) return;

    if (item.kind === "image") {
      editorText.value = "";
      editorDirty.value = false;
      updateBuiltinForItem(item);
      void runPluginsForSelected();
      return;
    }

    if (item.kind === "text" && item.text != null) {
      if (!editorDirty.value) syncEditorFromItem(item);
      updateBuiltinForItem(item);
      void runPluginsForSelected();
      return;
    }

    const token = ++hydrateToken;
    hydrating.value = true;
    try {
      const full = await api.getHistoryItem(id);
      if (token !== hydrateToken || selectedId.value !== id) return;
      mergeItem(full);
      if (!editorDirty.value) syncEditorFromItem(full);
      updateBuiltinForItem(full);
      void runPluginsForSelected();
    } catch (error) {
      if (token === hydrateToken) {
        setStatus(error instanceof Error ? error.message : String(error), "err");
      }
    } finally {
      if (token === hydrateToken) hydrating.value = false;
    }
  }

  /**
   * Plugins: cache by content hash + enabled plugin set.
   * Only re-invoke when cache miss (new content or plugins changed).
   */
  async function runPluginsForSelected() {
    const item = selectedItem.value;
    if (!item) {
      pluginSuggestions.value = [];
      return;
    }

    // Image plugins need path; text needs body
    if (item.kind === "text" && item.text == null) {
      return;
    }

    const token = ++pluginToken;
    const sig = await ensurePluginSig();
    if (token !== pluginToken) return;

    const key = pluginCacheKey(item, sig);
    const cached = pluginCache.get(key);
    if (cached) {
      pluginSuggestions.value = cached;
      return;
    }

    // Brief yield so selection paint stays smooth on first miss only
    await new Promise<void>((r) => setTimeout(r, 50));
    if (token !== pluginToken) return;

    const type = item.kind === "image" ? "img" : "text";
    const content =
      item.kind === "text" ? (item.text ?? "") : (item.imagePath ?? item.id);
    if (item.kind === "text" && !content) {
      pluginSuggestions.value = [];
      cacheSet(pluginCache, key, []);
      return;
    }

    try {
      const outs = (await api.runEnabledPlugins(content, type)) || [];
      if (token !== pluginToken) return;
      const mapped: ParseSuggestion[] = outs
        .filter((o) => o.ok && o.body)
        .map((o) => ({
          id: `plugin:${o.pluginId}`,
          title: o.title || o.pluginId,
          preview: o.preview || o.body.replace(/\s+/g, " ").slice(0, 72),
          body: o.body,
          hint: o.hint || "插件",
          recommended: o.pluginId === "translate-en-zh",
        }));
      cacheSet(pluginCache, key, mapped);
      pluginSuggestions.value = mapped;
    } catch {
      if (token === pluginToken) {
        pluginSuggestions.value = [];
      }
    }
  }

  async function loadHistory(options: { silent?: boolean } = {}) {
    const silent = options.silent === true;
    if (!silent) loading.value = true;
    try {
      const hydratedText = new Map<string, string>();
      for (const it of items.value) {
        if (it.kind === "text" && it.text != null) {
          hydratedText.set(it.id, it.text);
        }
      }

      const prevFirstId = items.value[0]?.id;
      const records = (await api.listHistory(query.value)) || [];
      items.value = records.map((r) => {
        const cached = hydratedText.get(r.id);
        return cached != null ? { ...r, text: cached } : r;
      });

      const topId = records[0]?.id ?? "";
      if (topId && topId !== prevFirstId && (!selectedId.value || silent)) {
        selectedId.value = topId;
        resetApplied();
      } else if (!records.some((r) => r.id === selectedId.value)) {
        selectedId.value = topId;
        resetApplied();
      }
      hasLoaded.value = true;
      void hydrateSelected();
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    } finally {
      if (!silent) loading.value = false;
    }
  }

  async function refreshImagePreview() {
    imageSrc.value = "";
    const item = selectedItem.value;
    if (!item || item.kind !== "image") return;
    // Cache image src by id lightly via... always resolve is cheap enough; could cache later
    try {
      const path = await api.resolveImagePath(item.id);
      imageSrc.value = convertFileSrc(path);
    } catch {
      imageSrc.value = "";
    }
  }

  async function deleteItem(id: string) {
    try {
      await api.deleteHistory(id);
      await loadHistory({ silent: true });
      setStatus("已删除", "ok");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    }
  }

  async function copyItem(item: HistoryItem) {
    selectedId.value = item.id;
    try {
      // Prefer current panel draft when copying the selected text item
      let override: string | null = null;
      if (item.kind === "text" && item.id === selectedId.value) {
        override = editorText.value;
      }
      await api.copyHistory(item.id, override);
      setStatus(
        override != null && override !== (item.text ?? "")
          ? "已复制面板内容"
          : "已复制到剪贴板",
        "ok",
      );
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    }
  }

  async function clearAll() {
    if (items.value.length === 0) return;
    try {
      await api.clearHistory();
      builtinCache.clear();
      pluginCache.clear();
      await loadHistory({ silent: true });
      setStatus("已清空历史", "ok");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    }
  }

  watch(query, () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => loadHistory({ silent: true }), 180);
  });

  watch(selectedId, () => {
    // Discard local draft — history is never written by the editor
    resetApplied();
    editorDirty.value = false;

    const item = items.value.find((i) => i.id === selectedId.value) ?? null;
    syncEditorFromItem(item);

    if (item && !(item.kind === "text" && item.text == null)) {
      updateBuiltinForItem(item);
      if (!tryRestorePluginCache(item)) {
        pluginSuggestions.value = [];
      }
    } else {
      builtinSuggestions.value = [];
      pluginSuggestions.value = [];
    }

    void hydrateSelected();
  });

  watch(selectedItem, () => {
    void refreshImagePreview();
  });

  function disposeHistory() {
    clearTimeout(searchTimer);
    hydrateToken += 1;
    pluginToken += 1;
  }

  /**
   * Call after plugin enable/disable/import/remove.
   * @param enabledIds optional sorted list of enabled plugin ids (with optional @version)
   */
  function refreshPluginSuggestions(enabledIds?: string[]) {
    if (enabledIds) {
      pluginSig = [...enabledIds].sort().join("|");
    } else {
      pluginSig = null;
    }
    pluginCache.clear();
    void runPluginsForSelected();
  }

  return {
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
  };
}
