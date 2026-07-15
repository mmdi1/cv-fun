<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import * as api from "./api";
import type { HistoryItem } from "./core/types";

const MAX_ITEMS = 50;
const PREVIEW_CHARS = 25;

const items = ref<HistoryItem[]>([]);
const thumbs = ref<Record<string, string>>({});
const busy = ref(false);
const empty = ref(false);
const listEl = ref<HTMLElement | null>(null);
const searchInputEl = ref<HTMLInputElement | null>(null);
const query = ref("");
/** Highlighted row: 0..items-1, or items.length for 「查看更多」 */
const activeIndex = ref(0);

const unlisteners: UnlistenFn[] = [];
let searchTimer: ReturnType<typeof setTimeout> | null = null;

function truncatePreview(text: string): string {
  const t = (text || "").replace(/\s+/g, " ").trim();
  if (!t) return "(空)";
  const chars = [...t];
  if (chars.length <= PREVIEW_CHARS) return t;
  return chars.slice(0, PREVIEW_CHARS).join("") + "…";
}

function isMac(): boolean {
  const p = (navigator.platform || "").toLowerCase();
  return p.includes("mac");
}

function shortcutHint(index: number): string {
  if (index >= 9) return "";
  return isMac() ? `⌘${index + 1}` : `^${index + 1}`;
}

/** Max selectable index: last item, or 「查看更多」 row after items. */
function maxSelectIndex(): number {
  // Always have "查看更多" as last selectable row
  return items.value.length;
}

function clampActive() {
  const max = maxSelectIndex();
  if (activeIndex.value < 0) activeIndex.value = 0;
  if (activeIndex.value > max) activeIndex.value = max;
}

function scrollActiveIntoView() {
  void nextTick(() => {
    const list = listEl.value;
    if (!list) return;
    const row = list.querySelector<HTMLElement>(`[data-idx="${activeIndex.value}"]`);
    row?.scrollIntoView({ block: "nearest" });
  });
}

function focusSearch() {
  void nextTick(() => {
    searchInputEl.value?.focus();
    searchInputEl.value?.select();
  });
}

async function loadList() {
  try {
    const list = await api.listPasteHistory(query.value.trim());
    items.value = list.slice(0, MAX_ITEMS);
    empty.value = items.value.length === 0;
    activeIndex.value = 0;
    clampActive();
    void nextTick(() => {
      if (listEl.value) listEl.value.scrollTop = 0;
    });
    const next: Record<string, string> = {};
    await Promise.all(
      items.value
        .filter((it) => it.kind === "image")
        .map(async (it) => {
          try {
            const path = await api.resolveImagePath(it.id);
            next[it.id] = convertFileSrc(path);
          } catch {
            /* ignore */
          }
        }),
    );
    thumbs.value = next;
    scrollActiveIntoView();
  } catch {
    items.value = [];
    empty.value = true;
    activeIndex.value = 0;
  }
}

function scheduleSearch() {
  if (searchTimer) clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    searchTimer = null;
    void loadList();
  }, 80);
}

watch(query, () => {
  scheduleSearch();
});

async function pasteAt(index: number) {
  if (busy.value) return;
  const item = items.value[index];
  if (!item) return;
  busy.value = true;
  try {
    await api.pasteHistoryItem(item.id);
  } catch {
    busy.value = false;
  }
}

async function activateSelection() {
  if (activeIndex.value >= items.value.length) {
    await onMore();
    return;
  }
  await pasteAt(activeIndex.value);
}

async function onMore() {
  try {
    await api.openMainFromPaste();
  } catch {
    /* ignore */
  }
}

async function dismiss() {
  try {
    await api.hidePastePopup();
  } catch {
    /* ignore */
  }
}

function moveActive(delta: number) {
  const max = maxSelectIndex();
  if (max < 0) return;
  let next = activeIndex.value + delta;
  if (next < 0) next = max;
  if (next > max) next = 0;
  activeIndex.value = next;
  scrollActiveIntoView();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    void dismiss();
    return;
  }

  if (e.key === "ArrowDown") {
    e.preventDefault();
    moveActive(1);
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    moveActive(-1);
    return;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    void activateSelection();
    return;
  }

  // Cmd/Ctrl + 1..9 → paste that row
  const mod = e.metaKey || e.ctrlKey;
  if (!mod || e.altKey || e.shiftKey) return;
  const digit = e.key >= "1" && e.key <= "9" ? Number(e.key) : -1;
  if (digit >= 1 && digit <= 9) {
    e.preventDefault();
    e.stopPropagation();
    void pasteAt(digit - 1);
  }
}

function onShow() {
  busy.value = false;
  query.value = "";
  activeIndex.value = 0;
  void loadList().then(() => focusSearch());
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  void loadList().then(() => focusSearch());
  void listen("paste-popup-show", () => {
    onShow();
  }).then((u) => unlisteners.push(u));
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  if (searchTimer) clearTimeout(searchTimer);
  for (const u of unlisteners) u();
});
</script>

<template>
  <div class="paste-root" @contextmenu.prevent>
    <header class="paste-head">
      <input
        ref="searchInputEl"
        v-model="query"
        class="paste-search"
        type="search"
        enterkeyhint="go"
        autocomplete="off"
        spellcheck="false"
        placeholder="快速搜索…"
        @keydown.stop="onKeydown"
      />
      <span class="paste-hint">↑↓ · Enter · Esc</span>
    </header>

    <ul v-if="items.length" ref="listEl" class="paste-list">
      <li
        v-for="(item, i) in items"
        :key="item.id"
        class="paste-row"
        :class="{ image: item.kind === 'image', active: activeIndex === i }"
        :data-idx="i"
        @click="pasteAt(i)"
        @mouseenter="activeIndex = i"
      >
        <span class="idx">{{ i + 1 }}</span>
        <template v-if="item.kind === 'image'">
          <img
            v-if="thumbs[item.id]"
            class="thumb"
            :src="thumbs[item.id]"
            alt=""
            draggable="false"
          />
          <span v-else class="thumb placeholder">图</span>
          <span class="label">图片…</span>
        </template>
        <template v-else>
          <span class="label">{{ truncatePreview(item.preview || item.text || "") }}</span>
        </template>
        <span v-if="shortcutHint(i)" class="kbd">{{ shortcutHint(i) }}</span>
      </li>
      <li
        class="paste-row more-row"
        :class="{ active: activeIndex === items.length }"
        :data-idx="items.length"
        @click="onMore"
        @mouseenter="activeIndex = items.length"
      >
        <span class="idx" />
        <span class="label more-label">查看更多 →</span>
      </li>
    </ul>
    <div v-else class="paste-empty">
      {{ empty ? (query.trim() ? "无匹配记录" : "暂无历史记录") : "加载中…" }}
      <button
        v-if="empty"
        type="button"
        class="paste-more-inline"
        :class="{ active: activeIndex === 0 }"
        data-idx="0"
        @click="onMore"
        @mouseenter="activeIndex = 0"
      >
        查看更多 →
      </button>
    </div>
  </div>
</template>

<style>
/* Transparent frameless window host */
html,
body,
#app {
  background: transparent !important;
  overflow: hidden;
}
</style>

<style scoped>
.paste-root {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: #12181e;
  color: #e8edf2;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  overflow: hidden;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial,
    sans-serif;
  font-size: 13px;
  user-select: none;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.45);
}

.paste-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}

.paste-search {
  flex: 1;
  min-width: 0;
  height: 28px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.28);
  color: #e8edf2;
  font: inherit;
  font-size: 13px;
  padding: 0 10px;
  outline: none;
}

.paste-search::placeholder {
  color: #6b7782;
}

.paste-search:focus {
  border-color: rgba(62, 207, 173, 0.55);
  box-shadow: 0 0 0 2px rgba(62, 207, 173, 0.12);
}

/* Hide native search clear on some browsers for cleaner look */
.paste-search::-webkit-search-cancel-button {
  -webkit-appearance: none;
}

.paste-hint {
  flex-shrink: 0;
  font-size: 10px;
  color: #5a6570;
  white-space: nowrap;
}

.paste-list {
  list-style: none;
  margin: 0;
  padding: 4px;
  overflow-x: hidden;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.paste-list::-webkit-scrollbar {
  width: 0;
  height: 0;
  display: none;
}

.paste-row {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 32px;
  padding: 0 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.08s ease;
}

.paste-row:hover,
.paste-row.active {
  background: rgba(62, 207, 173, 0.14);
}

.paste-row.active {
  outline: 1px solid rgba(62, 207, 173, 0.35);
  outline-offset: -1px;
}

.idx {
  flex: 0 0 22px;
  width: 22px;
  text-align: right;
  font-size: 11px;
  font-weight: 600;
  color: #7a8792;
  font-variant-numeric: tabular-nums;
}

.thumb {
  width: 22px;
  height: 22px;
  border-radius: 4px;
  object-fit: cover;
  background: #1a222a;
  flex-shrink: 0;
}

.thumb.placeholder {
  display: grid;
  place-content: center;
  font-size: 10px;
  color: #7a8792;
}

.label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: #e8edf2;
}

.more-label {
  color: #3ecfad;
  font-weight: 600;
}

.kbd {
  flex-shrink: 0;
  font-size: 10px;
  color: #5a6570;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.02em;
}

.paste-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: #6b7782;
  font-size: 12px;
  padding: 16px;
}

.paste-more-inline {
  border: none;
  background: transparent;
  color: #3ecfad;
  font: inherit;
  font-weight: 600;
  font-size: 12px;
  cursor: pointer;
  padding: 6px 10px;
  border-radius: 6px;
}

.paste-more-inline:hover,
.paste-more-inline.active {
  background: rgba(62, 207, 173, 0.12);
}
</style>
