import type { ParseSuggestion } from "../../core/types";
import type { ContentContext, ContentExtension } from "../types";
import { formatDateTime } from "../../utils/time";

function parseUnixTimestamp(text: string): { date: Date; unit: "s" | "ms" } | null {
  if (!/^-?\d{10}(\d{3})?$/.test(text)) return null;
  const raw = Number(text);
  const unit = text.replace(/^-/, "").length === 13 ? "ms" : "s";
  const ms = unit === "ms" ? raw : raw * 1000;
  const date = new Date(ms);
  if (Number.isNaN(date.getTime())) return null;
  if (date.getFullYear() < 2000 || date.getFullYear() > 2100) return null;
  return { date, unit };
}

function parseHumanDateTime(text: string): Date | null {
  const t = text.trim();
  const m =
    /^(\d{4})[-/](\d{1,2})[-/](\d{1,2})(?:[ T](\d{1,2}):(\d{1,2})(?::(\d{1,2}))?)?$/.exec(t);
  if (m) {
    const y = Number(m[1]);
    const mo = Number(m[2]) - 1;
    const d = Number(m[3]);
    const h = Number(m[4] ?? 0);
    const mi = Number(m[5] ?? 0);
    const s = Number(m[6] ?? 0);
    const date = new Date(y, mo, d, h, mi, s);
    if (!Number.isNaN(date.getTime())) return date;
  }
  if (/^\d{4}-\d{2}-\d{2}T/.test(t)) {
    const date = new Date(t);
    if (!Number.isNaN(date.getTime())) return date;
  }
  return null;
}

export const timestampExtension: ContentExtension = {
  id: "timestamp",

  suggest(ctx: ContentContext): ParseSuggestion[] {
    if (ctx.kind !== "text") return [];
    const text = ctx.rawText.trim();
    if (!text) return [];
    const out: ParseSuggestion[] = [];

    const unix = parseUnixTimestamp(text);
    if (unix) {
      const human = formatDateTime(unix.date);
      out.push({
        id: "ts:human",
        title: "时间戳 → 时间",
        preview: human,
        body: human,
        hint: unix.unit === "ms" ? "毫秒" : "秒",
        recommended: true,
      });
      const both = `${Math.floor(unix.date.getTime() / 1000)} (s)\n${unix.date.getTime()} (ms)`;
      out.push({
        id: "ts:both",
        title: "秒 / 毫秒",
        preview: both.replace(/\n/g, " · "),
        body: both,
      });
      return out;
    }

    const human = parseHumanDateTime(text);
    if (human) {
      const s = String(Math.floor(human.getTime() / 1000));
      const ms = String(human.getTime());
      out.push({
        id: "ts:to-s",
        title: "→ 秒时间戳",
        preview: s,
        body: s,
        recommended: true,
      });
      out.push({
        id: "ts:to-ms",
        title: "→ 毫秒时间戳",
        preview: ms,
        body: ms,
      });
    }
    return out;
  },
};
