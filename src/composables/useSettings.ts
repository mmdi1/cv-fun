import { ref } from "vue";
import * as api from "../api";
import type { AppConfig } from "../core/types";
import {
  defaultAppendCopyHotkey,
  defaultParseCopyHotkey,
  defaultToggleHotkey,
  detectShortcutPlatform,
  shortcutDisplayFromEvent,
} from "../utils/hotkey";

export type HotkeyField = "toggle" | "append" | "parse";

export function useSettings(setStatus: (msg: string, tone?: "muted" | "ok" | "err") => void) {
  const settingsOpen = ref(false);
  const dataRoot = ref("");
  /** Which hotkey input is recording (null = none). */
  const hotkeyRecording = ref<HotkeyField | null>(null);
  const platform = detectShortcutPlatform();
  const config = ref<AppConfig>({
    history: { maxItems: 500 },
    pollIntervalMs: 400,
    toggleHotkey: defaultToggleHotkey(),
    appendCopyHotkey: defaultAppendCopyHotkey(),
    parseCopyHotkey: defaultParseCopyHotkey(),
  });

  async function loadConfig() {
    try {
      const loaded = await api.getConfig();
      config.value = {
        history: { maxItems: loaded.history?.maxItems ?? 500 },
        pollIntervalMs: loaded.pollIntervalMs ?? 400,
        toggleHotkey: loaded.toggleHotkey?.trim() || defaultToggleHotkey(),
        appendCopyHotkey:
          loaded.appendCopyHotkey !== undefined && loaded.appendCopyHotkey !== null
            ? String(loaded.appendCopyHotkey).trim()
            : defaultAppendCopyHotkey(),
        parseCopyHotkey:
          loaded.parseCopyHotkey !== undefined && loaded.parseCopyHotkey !== null
            ? String(loaded.parseCopyHotkey).trim()
            : defaultParseCopyHotkey(),
      };
      dataRoot.value = await api.dataRootPath();
    } catch {
      // defaults
    }
  }

  function onHotkeyKeydown(event: KeyboardEvent, field: HotkeyField) {
    if (hotkeyRecording.value !== field) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      hotkeyRecording.value = null;
      return;
    }
    // Backspace / Delete clears optional hotkeys (disable)
    if (
      (field === "append" || field === "parse") &&
      (event.key === "Backspace" || event.key === "Delete")
    ) {
      if (field === "append") config.value.appendCopyHotkey = "";
      else config.value.parseCopyHotkey = "";
      hotkeyRecording.value = null;
      return;
    }
    const minParts = field === "toggle" ? 2 : 3;
    const shortcut = shortcutDisplayFromEvent(event, platform, minParts);
    if (!shortcut) return;
    if (field === "toggle") config.value.toggleHotkey = shortcut;
    else if (field === "append") config.value.appendCopyHotkey = shortcut;
    else config.value.parseCopyHotkey = shortcut;
    hotkeyRecording.value = null;
  }

  async function saveConfig() {
    try {
      const append = config.value.appendCopyHotkey?.trim() || "";
      const parse = config.value.parseCopyHotkey?.trim() || "";
      if (append) {
        const n = append.split("+").filter(Boolean).length;
        if (n < 3) {
          setStatus("追加复制需 3 键组合，例如 Cmd+Shift+C", "err");
          return;
        }
      }
      if (parse) {
        const n = parse.split("+").filter(Boolean).length;
        if (n < 3) {
          setStatus("复制并解析需 3 键组合，例如 Shift+Option+X", "err");
          return;
        }
      }
      config.value = await api.saveAppConfig({
        history: { maxItems: Math.max(10, Number(config.value.history.maxItems) || 500) },
        pollIntervalMs: Math.max(150, Number(config.value.pollIntervalMs) || 400),
        toggleHotkey: config.value.toggleHotkey?.trim() || defaultToggleHotkey(),
        appendCopyHotkey: append,
        parseCopyHotkey: parse,
      });
      settingsOpen.value = false;
      hotkeyRecording.value = null;
      const bits = [`${config.value.toggleHotkey} 唤起/隐藏`];
      if (config.value.appendCopyHotkey) bits.push(`${config.value.appendCopyHotkey} 追加`);
      if (config.value.parseCopyHotkey) bits.push(`${config.value.parseCopyHotkey} 复制解析`);
      setStatus(`设置已保存 · ${bits.join(" · ")}`, "ok");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    }
  }

  return {
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
  };
}
