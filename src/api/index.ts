import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  EcdictStatus,
  HistoryItem,
  PluginInfo,
  PluginOutput,
  StatsSnapshot,
} from "../core/types";

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

/** Clear history. mode: keep_saved | keep_favorites | all. Returns deleted count. */
export function clearHistory(mode: "keep_saved" | "keep_favorites" | "all" = "all") {
  return invoke<number>("clear_history", { mode });
}

export function setHistoryPinned(id: string, pinned: boolean) {
  return invoke<HistoryItem>("set_history_pinned", { id, pinned });
}

export function setHistoryFavorited(id: string, favorited: boolean) {
  return invoke<HistoryItem>("set_history_favorited", { id, favorited });
}

export function setHistoryNote(id: string, note: string) {
  return invoke<HistoryItem>("set_history_note", { id, note });
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

/** Local OCR for a history image (macOS Vision / Windows Media.Ocr). */
export function ocrHistoryImage(id: string) {
  return invoke<string>("ocr_history_image", { id });
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

export type ExportResult = {
  path: string;
  count: number;
  mode: string;
};

export type ImportResult = {
  mode: string;
  path: string;
  count: number;
  skipped: number;
};

/** Export history package. mode: saved | full */
export function exportData(
  mode: "saved" | "full",
  destDir: string,
  opts?: { includeImages?: boolean; includeConfig?: boolean },
) {
  return invoke<ExportResult>("export_data", {
    mode,
    destDir,
    includeImages: opts?.includeImages ?? true,
    includeConfig: opts?.includeConfig ?? true,
  });
}

/** Import package. mode: saved | full | auto */
export function importData(
  mode: "saved" | "full" | "auto",
  sourcePath: string,
  opts?: { includeConfig?: boolean },
) {
  return invoke<ImportResult>("import_data", {
    mode,
    sourcePath,
    includeConfig: opts?.includeConfig ?? true,
  });
}

export function hideMainWindow() {
  return invoke<void>("hide_main_window");
}

export function showMainWindow() {
  return invoke<void>("show_main_window");
}

/** Recent history for quick-paste popup (max 12). */
export function listPasteHistory() {
  return invoke<HistoryItem[]>("list_paste_history");
}

export function hidePastePopup() {
  return invoke<void>("hide_paste_popup");
}

/** Copy item to clipboard and simulate paste into previous app. */
export function pasteHistoryItem(id: string) {
  return invoke<void>("paste_history_item", { id });
}

/** From paste popup: open main UI. */
export function openMainFromPaste() {
  return invoke<void>("open_main_from_paste");
}

export function getFunStats() {
  return invoke<StatsSnapshot>("get_fun_stats");
}

// ----- Plugins -----

export function listPlugins() {
  return invoke<PluginInfo[]>("list_plugins");
}

export function setPluginEnabled(id: string, enabled: boolean) {
  return invoke<void>("set_plugin_enabled", { id, enabled });
}

export function importPlugin(path: string) {
  return invoke<PluginInfo>("import_plugin", { path });
}

export function removePlugin(id: string) {
  return invoke<void>("remove_plugin", { id });
}

export function runPlugin(id: string, content: string, type: string) {
  return invoke<PluginOutput>("run_plugin", { id, content, type });
}

export function runEnabledPlugins(content: string, type: string) {
  return invoke<PluginOutput[]>("run_enabled_plugins", { content, type });
}

export function ecdictStatus() {
  return invoke<EcdictStatus>("ecdict_status");
}

export function installEcdict() {
  return invoke<EcdictStatus>("install_ecdict");
}

export function pluginProtocolHelp() {
  return invoke<string>("plugin_protocol_help");
}

export type SamplePack = {
  id: string;
  name: string;
  runtime: string;
  description: string;
  files: string[];
};

export function listPluginSamples() {
  return invoke<SamplePack[]>("list_plugin_samples");
}

export function exportPluginSample(id: string, destDir: string) {
  return invoke<string>("export_plugin_sample", { id, destDir });
}

export function exportAllPluginSamples(destDir: string) {
  return invoke<string[]>("export_all_plugin_samples", { destDir });
}
