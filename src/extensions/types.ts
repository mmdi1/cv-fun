import type { HistoryItem, ItemKind, PanelContent, ParseSuggestion } from "../core/types";

/**
 * Immutable snapshot of clipboard history content for extensions.
 */
export type ContentContext = {
  kind: ItemKind;
  rawText: string;
  item: HistoryItem;
};

/**
 * Content extension plugin.
 *
 * - `suggest`: recommended transforms shown in the bottom row (click to apply)
 * - Optional `claimDisplay` is no longer used for auto main-panel override
 *   (main panel always shows original until user picks a suggestion)
 */
export interface ContentExtension {
  id: string;
  /**
   * Recommend parse/transform options for the suggestion row.
   */
  suggest?(ctx: ContentContext): ParseSuggestion[];
}

export type ResolveResult = {
  /** Always original content for the main panel (until UI applies a suggestion). */
  original: PanelContent;
  /** Clickable recommendations under the panel. */
  suggestions: ParseSuggestion[];
};
