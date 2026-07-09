import { ref } from "vue";
import * as api from "../api";
import type { AppConfig } from "../core/types";
import {
  defaultToggleHotkey,
  detectShortcutPlatform,
  shortcutDisplayFromEvent,
} from "../utils/hotkey";

export function useSettings(setStatus: (msg: string, tone?: "muted" | "ok" | "err") => void) {
  const settingsOpen = ref(false);
  const dataRoot = ref("");
  const hotkeyRecording = ref(false);
  const platform = detectShortcutPlatform();
  const config = ref<AppConfig>({
    history: { maxItems: 500 },
    pollIntervalMs: 400,
    toggleHotkey: defaultToggleHotkey(),
  });

  async function loadConfig() {
    try {
      const loaded = await api.getConfig();
      config.value = {
        history: { maxItems: loaded.history?.maxItems ?? 500 },
        pollIntervalMs: loaded.pollIntervalMs ?? 400,
        toggleHotkey: loaded.toggleHotkey?.trim() || defaultToggleHotkey(),
      };
      dataRoot.value = await api.dataRootPath();
    } catch {
      // defaults
    }
  }

  function onHotkeyKeydown(event: KeyboardEvent) {
    if (!hotkeyRecording.value) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      hotkeyRecording.value = false;
      return;
    }
    const shortcut = shortcutDisplayFromEvent(event, platform);
    if (!shortcut) return;
    config.value.toggleHotkey = shortcut;
    hotkeyRecording.value = false;
  }

  async function saveConfig() {
    try {
      config.value = await api.saveAppConfig({
        history: { maxItems: Math.max(10, Number(config.value.history.maxItems) || 500) },
        pollIntervalMs: Math.max(150, Number(config.value.pollIntervalMs) || 400),
        toggleHotkey: config.value.toggleHotkey?.trim() || defaultToggleHotkey(),
      });
      settingsOpen.value = false;
      hotkeyRecording.value = false;
      setStatus(`设置已保存 · ${config.value.toggleHotkey} 唤起/隐藏`, "ok");
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
  };
}
