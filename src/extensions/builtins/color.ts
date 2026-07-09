import type { ParseSuggestion } from "../../core/types";
import type { ContentContext, ContentExtension } from "../types";

function isColor(text: string): boolean {
  const t = text.trim();
  return (
    /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(t) ||
    /^rgba?\(\s*\d+\s*,\s*\d+\s*,\s*\d+(?:\s*,\s*[\d.]+\s*)?\)$/i.test(t)
  );
}

export const colorExtension: ContentExtension = {
  id: "color",

  suggest(ctx: ContentContext): ParseSuggestion[] {
    if (ctx.kind !== "text") return [];
    const text = ctx.rawText.trim();
    if (!isColor(text)) return [];
    return [
      {
        id: "color:raw",
        title: "颜色值",
        preview: text,
        body: text,
        recommended: true,
      },
    ];
  },
};
