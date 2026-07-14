<script setup lang="ts">
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { nextTick, onMounted, onUnmounted, ref } from "vue";
import * as api from "./api";
import type { HistoryItem } from "./core/types";

const MAX_ITEMS = 50;
const PREVIEW_CHARS = 25;

const items = ref<HistoryItem[]>([]);
const thumbs = ref<Record<string, string>>({});
const busy = ref(false);
const empty = ref(false);
const listEl = ref<HTMLElement | null>(null);

const unlisteners: UnlistenFn[] = [];

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
  // macOS: ⌘1–9 · Windows: Ctrl+1–9
  return isMac() ? `⌘${index + 1}` : `^${index + 1}`;
}

/** Always start from the first history row when the popup opens. */
function resetScrollToTop() {
  void nextTick(() => {
    const el = listEl.value;
    if (el) el.scrollTop = 0;
  });
}

async function loadList() {
  try {
    const list = await api.listPasteHistory();
    items.value = list.slice(0, MAX_ITEMS);
    empty.value = items.value.length === 0;
    resetScrollToTop();
    // Resolve image thumbnails (best-effort)
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
    // Thumbs may reflow layout — keep list pinned to top.
    resetScrollToTop();
  } catch {
    items.value = [];
    empty.value = true;
  }
}

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

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    void dismiss();
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

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
  void loadList();
  void listen("paste-popup-show", () => {
    busy.value = false;
    resetScrollToTop();
    void loadList();
  }).then((u) => unlisteners.push(u));
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeydown);
  for (const u of unlisteners) u();
});
</script>

<template>
  <div class="paste-root" @contextmenu.prevent>
    <header class="paste-head">
      <span class="paste-title">快速粘贴</span>
      <span class="paste-hint">{{ isMac() ? "⌘1–9" : "Ctrl+1–9" }} · Esc</span>
    </header>

    <ul v-if="items.length" ref="listEl" class="paste-list">
      <li
        v-for="(item, i) in items"
        :key="item.id"
        class="paste-row"
        :class="{ image: item.kind === 'image' }"
        @click="pasteAt(i)"
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
    </ul>
    <div v-else class="paste-empty">
      {{ empty ? "暂无历史记录" : "加载中…" }}
    </div>

    <button type="button" class="paste-more" @click="onMore">查看更多 →</button>
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
  justify-content: space-between;
  gap: 8px;
  padding: 8px 10px 6px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  flex-shrink: 0;
}

.paste-title {
  font-weight: 650;
  font-size: 12px;
  letter-spacing: 0.02em;
  color: #c5d0d8;
}

.paste-hint {
  font-size: 11px;
  color: #6b7782;
}

.paste-list {
  list-style: none;
  margin: 0;
  padding: 4px;
  overflow-x: hidden;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
  /* Hide scrollbar, keep wheel/trackpad scroll */
  scrollbar-width: none; /* Firefox */
  -ms-overflow-style: none; /* legacy Edge */
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

.paste-row:hover {
  background: rgba(62, 207, 173, 0.12);
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

.kbd {
  flex-shrink: 0;
  font-size: 10px;
  color: #5a6570;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.02em;
}

.paste-empty {
  flex: 1;
  display: grid;
  place-content: center;
  color: #6b7782;
  font-size: 12px;
  padding: 16px;
}

.paste-more {
  flex-shrink: 0;
  border: none;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  background: transparent;
  color: #3ecfad;
  font-size: 12px;
  font-weight: 600;
  padding: 9px 12px;
  cursor: pointer;
  text-align: left;
}

.paste-more:hover {
  background: rgba(62, 207, 173, 0.08);
}
</style>
