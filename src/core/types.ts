/** Shared domain types for FunCV */

export type ItemKind = "text" | "image";

export type HistoryItem = {
  id: string;
  kind: ItemKind;
  text?: string | null;
  preview: string;
  hash: string;
  imagePath?: string | null;
  imageWidth?: number | null;
  imageHeight?: number | null;
  copiedAt: string;
  lastUsedAt: string;
  useCount: number;
  pinned: boolean;
};

export type AppConfig = {
  history: { maxItems: number };
  pollIntervalMs: number;
  /** Display form e.g. Option+Space */
  toggleHotkey: string;
  /**
   * 3-key combo: append selection to existing text clipboard with double-space.
   * Empty string disables. e.g. Cmd+Shift+C
   */
  appendCopyHotkey: string;
  /**
   * 3-key combo: copy selection then show FunCV for parse.
   * Empty string disables. e.g. Shift+Option+X
   */
  parseCopyHotkey: string;
};

/**
 * A recommended parse/transform shown under the main panel.
 * Click → apply `body` into the large content viewport.
 */
export type ParseSuggestion = {
  /** Stable id within an item, e.g. "json:pretty" or "plugin:translate-en-zh" */
  id: string;
  /** Short label, e.g. "格式化 JSON" */
  title: string;
  /** One-line preview for the suggestion row */
  preview: string;
  /** Full content applied to the main panel when selected */
  body: string;
  /** Extra hint shown on the chip */
  hint?: string;
  /** Recommended first (primary) */
  recommended?: boolean;
};

/** How the main detail pane should render body content. */
export type PanelContent =
  | { mode: "plain"; body: string }
  | { mode: "json"; body: string }
  | { mode: "image" }
  | { mode: "empty" };

/** Universal plugin content type */
export type PluginContentType = "text" | "img" | string;

export type PluginInfo = {
  id: string;
  name: string;
  version: string;
  runtime: string;
  description: string;
  types: string[];
  enabled: boolean;
  builtin: boolean;
  path?: string | null;
};

export type PluginOutput = {
  ok: boolean;
  title: string;
  body: string;
  preview: string;
  hint: string;
  error: string;
  pluginId: string;
};

export type EcdictStatus = {
  ready: boolean;
  path: string;
  entries: number;
  sizeBytes: number;
};

export type TopRepeat = {
  hash: string;
  preview: string;
  kind: string;
  count: number;
};

export type DailyStat = {
  day: string;
  total: number;
  duplicates: number;
  text: number;
  image: number;
};

export type StatsSnapshot = {
  totalCopies: number;
  totalDuplicates: number;
  textCopies: number;
  imageCopies: number;
  todayCopies: number;
  todayDuplicates: number;
  historyItems: number;
  duplicateRate: number;
  peakHour: number | null;
  peakHourCount: number;
  streakDays: number;
  topRepeats: TopRepeat[];
  daily: DailyStat[];
};
