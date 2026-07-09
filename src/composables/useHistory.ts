import { computed, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import * as api from "../api";
import type { HistoryItem, PanelContent, ParseSuggestion } from "../core/types";
import { resolveContent } from "../extensions";

export function useHistory(setStatus: (msg: string, tone?: "muted" | "ok" | "err") => void) {
  const query = ref("");
  const items = ref<HistoryItem[]>([]);
  const loading = ref(false);
  const hasLoaded = ref(false);
  const selectedId = ref("");
  const imageSrc = ref("");
  /** True while fetching full text body for the selected text item. */
  const hydrating = ref(false);

  const appliedSuggestionId = ref<string | null>(null);
  const appliedBody = ref<string | null>(null);

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let hydrateToken = 0;

  const selectedItem = computed(
    () => items.value.find((i) => i.id === selectedId.value) ?? items.value[0] ?? null,
  );

  const resolved = computed(() => resolveContent(selectedItem.value));
  const suggestions = computed(() => resolved.value.suggestions);
  const itemCountLabel = computed(() => `${items.value.length}`);

  const panelContent = computed((): PanelContent => {
    const item = selectedItem.value;
    if (!item) return { mode: "empty" };
    if (item.kind === "image") return { mode: "image" };

    if (appliedBody.value != null) {
      return { mode: "plain", body: appliedBody.value };
    }
    // List summaries omit text until hydrated
    if (item.text == null) {
      return { mode: "plain", body: hydrating.value ? "加载中…" : "" };
    }
    return resolved.value.original;
  });

  const isShowingOriginal = computed(() => appliedSuggestionId.value == null);

  const formatBadge = computed(() => {
    if (!selectedItem.value) return "";
    if (selectedItem.value.kind === "image") return "图片";
    if (!isShowingOriginal.value) return "解析";
    return "原文";
  });

  function resetApplied() {
    appliedSuggestionId.value = null;
    appliedBody.value = null;
  }

  function applySuggestion(s: ParseSuggestion) {
    appliedSuggestionId.value = s.id;
    appliedBody.value = s.body;
    setStatus(`已应用 · ${s.title}`, "ok");
  }

  /** Double-click a recommendation: show in panel and write result to clipboard. */
  async function applyAndCopySuggestion(s: ParseSuggestion) {
    applySuggestion(s);
    const item = selectedItem.value;
    if (!item || item.kind !== "text") {
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
    setStatus("已恢复原文", "ok");
  }

  function mergeItem(full: HistoryItem) {
    const idx = items.value.findIndex((i) => i.id === full.id);
    if (idx >= 0) {
      items.value[idx] = full;
    }
  }

  /** Load full text body for selected text item (list omits text). */
  async function hydrateSelected() {
    const id = selectedId.value;
    if (!id) return;
    const item = items.value.find((i) => i.id === id);
    if (!item || item.kind !== "text") return;
    // Already has body
    if (item.text != null) return;

    const token = ++hydrateToken;
    hydrating.value = true;
    try {
      const full = await api.getHistoryItem(id);
      if (token !== hydrateToken || selectedId.value !== id) return;
      mergeItem(full);
    } catch (error) {
      if (token === hydrateToken) {
        setStatus(error instanceof Error ? error.message : String(error), "err");
      }
    } finally {
      if (token === hydrateToken) hydrating.value = false;
    }
  }

  async function loadHistory(options: { silent?: boolean } = {}) {
    const silent = options.silent === true;
    if (!silent) loading.value = true;
    try {
      // Preserve already-hydrated text bodies across list refresh
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

      // Auto-select newest when a new item lands at the top (clipboard capture).
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
      let override: string | null = null;
      if (item.kind === "text") {
        if (item.id === selectedItem.value?.id && appliedBody.value != null) {
          override = appliedBody.value;
        }
      }
      // Backend loads full text from DB when override is null
      await api.copyHistory(item.id, override);
      setStatus(
        override && override !== (item.text ?? "") ? "已复制解析结果" : "已复制到剪贴板",
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
    resetApplied();
    void hydrateSelected();
  });

  watch(selectedItem, () => {
    void refreshImagePreview();
  });

  function disposeHistory() {
    clearTimeout(searchTimer);
    hydrateToken += 1;
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
  };
}
