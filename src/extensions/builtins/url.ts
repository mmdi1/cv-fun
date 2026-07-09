import type { ParseSuggestion } from "../../core/types";
import type { ContentContext, ContentExtension } from "../types";

export const urlExtension: ContentExtension = {
  id: "url",

  suggest(ctx: ContentContext): ParseSuggestion[] {
    if (ctx.kind !== "text") return [];
    const text = ctx.rawText.trim();
    if (!text) return [];
    try {
      const url = new URL(text);
      if (url.protocol !== "http:" && url.protocol !== "https:" && url.protocol !== "file:") {
        return [];
      }
      const out: ParseSuggestion[] = [
        {
          id: "url:href",
          title: "规范化 URL",
          preview: url.href,
          body: url.href,
          hint: url.host || url.protocol,
          recommended: true,
        },
      ];
      if (url.searchParams.toString()) {
        const lines = [...url.searchParams.entries()]
          .map(([k, v]) => `${k}=${v}`)
          .join("\n");
        out.push({
          id: "url:params",
          title: "查询参数",
          preview: lines.replace(/\n/g, " · ").slice(0, 72),
          body: lines,
        });
      }
      return out;
    } catch {
      return [];
    }
  },
};
