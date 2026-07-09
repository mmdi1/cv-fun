import type { HistoryItem, PanelContent, ParseSuggestion } from "../core/types";
import { extensionRegistry } from "./registry";
import type { ContentContext, ResolveResult } from "./types";

function toContext(item: HistoryItem | null | undefined): ContentContext | null {
  if (!item) return null;
  return {
    kind: item.kind,
    rawText: item.kind === "text" ? (item.text ?? "") : "",
    item,
  };
}

function originalPanel(ctx: ContentContext): PanelContent {
  if (ctx.kind === "image") return { mode: "image" };
  const text = ctx.rawText;
  if (!text.trim()) return { mode: "empty" };
  // Original as plain always — no auto pretty
  return { mode: "plain", body: text };
}

/**
 * Pipeline:
 * 1. Build original panel content (never auto-transformed)
 * 2. Collect parse suggestions from all extensions (recommended first)
 */
export function resolveContent(item: HistoryItem | null | undefined): ResolveResult {
  const ctx = toContext(item);
  if (!ctx) {
    return { original: { mode: "empty" }, suggestions: [] };
  }

  const suggestions: ParseSuggestion[] = [];
  for (const ext of extensionRegistry.list()) {
    if (!ext.suggest) continue;
    try {
      const part = ext.suggest(ctx);
      if (part?.length) suggestions.push(...part);
    } catch (err) {
      console.warn(`[extensions] ${ext.id}.suggest failed`, err);
    }
  }

  // Recommended first, then registration order
  suggestions.sort((a, b) => Number(!!b.recommended) - Number(!!a.recommended));

  return {
    original: originalPanel(ctx),
    suggestions,
  };
}
