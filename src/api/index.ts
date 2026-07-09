import { invoke } from "@tauri-apps/api/core";
import type { AppConfig, HistoryItem } from "../core/types";

export function listHistory(query: string) {
  return invoke<HistoryItem[]>("list_history", { query });
}

/** Full history item including text body (list endpoint omits text for speed). */
export function getHistoryItem(id: string) {
  return invoke<HistoryItem>("get_history_item", { id });
}

export function deleteHistory(id: string) {
  return invoke<void>("delete_history", { id });
}

export function clearHistory() {
  return invoke<void>("clear_history");
}

/** Copy item; optional textOverride writes inferred/transformed text instead of stored raw. */
export function copyHistory(id: string, textOverride?: string | null) {
  return invoke<void>("copy_history", {
    id,
    textOverride: textOverride ?? null,
  });
}

export function resolveImagePath(id: string) {
  return invoke<string>("resolve_image_path", { id });
}

export function getConfig() {
  return invoke<AppConfig>("get_config");
}

export function saveAppConfig(config: AppConfig) {
  return invoke<AppConfig>("save_app_config", { config });
}

export function dataRootPath() {
  return invoke<string>("data_root_path");
}

export function hideMainWindow() {
  return invoke<void>("hide_main_window");
}

export function showMainWindow() {
  return invoke<void>("show_main_window");
}
