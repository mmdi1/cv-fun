import type { ParseSuggestion } from "../../core/types";
import type { ContentContext, ContentExtension } from "../types";

function isUuid(text: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
    text.trim(),
  );
}

export const uuidExtension: ContentExtension = {
  id: "uuid",

  suggest(ctx: ContentContext): ParseSuggestion[] {
    if (ctx.kind !== "text") return [];
    const text = ctx.rawText.trim();
    if (!isUuid(text)) return [];
    const lower = text.toLowerCase();
    const upper = text.toUpperCase();
    return [
      {
        id: "uuid:lower",
        title: "UUID 小写",
        preview: lower,
        body: lower,
        recommended: true,
      },
      {
        id: "uuid:upper",
        title: "UUID 大写",
        preview: upper,
        body: upper,
      },
    ];
  },
};
