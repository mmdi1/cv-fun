import type { ParseSuggestion } from "../../core/types";
import type { ContentContext, ContentExtension } from "../types";
import { recoverJson } from "./json";

function looksLikeBase64(text: string): boolean {
  const t = text.trim().replace(/\s+/g, "");
  if (t.length < 16 || t.length % 4 !== 0) return false;
  if (!/^[A-Za-z0-9+/]+={0,2}$/.test(t)) return false;
  if (/^[A-Za-z]+$/.test(t) && t.length < 40) return false;
  try {
    atob(t);
    return true;
  } catch {
    return false;
  }
}

export const base64Extension: ContentExtension = {
  id: "base64",

  suggest(ctx: ContentContext): ParseSuggestion[] {
    if (ctx.kind !== "text") return [];
    const text = ctx.rawText.trim();
    if (!text) return [];
    if (recoverJson(ctx.rawText).ok) return [];
    if (!looksLikeBase64(text)) return [];
    try {
      const decoded = atob(text.replace(/\s+/g, ""));
      // Prefer UTF-8-ish text
      const preview = decoded.length > 72 ? `${decoded.slice(0, 72)}…` : decoded;
      return [
        {
          id: "base64:decode",
          title: "Base64 解码",
          preview: preview.replace(/\s+/g, " "),
          body: decoded,
          hint: `${decoded.length} 字节`,
          recommended: true,
        },
      ];
    } catch {
      return [];
    }
  },
};
