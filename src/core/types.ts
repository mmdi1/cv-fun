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
};

/**
 * A recommended parse/transform shown as a compact row under the main panel.
 * Click → apply `body` into the large content viewport.
 */
export type ParseSuggestion = {
  /** Stable id within an item, e.g. "json:pretty" */
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
