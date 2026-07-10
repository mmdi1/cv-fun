import type { ParseSuggestion } from "../../core/types";
import type { ContentContext, ContentExtension } from "../types";
import { formatDateTime } from "../../utils/time";
import { tryAbsoluteTimestampPanel } from "../../utils/timestampAbs";

export const timestampExtension: ContentExtension = {
  id: "timestamp",

  suggest(ctx: ContentContext): ParseSuggestion[] {
    if (ctx.kind !== "text") return [];
    const text = ctx.rawText.trim();
    if (!text) return [];

    // Absolute match: full panel is shown in content area; keep light suggestions for bar.
    const abs = tryAbsoluteTimestampPanel(text);
    if (abs) {
      const local = formatDateTime(abs.date);
      const sec = String(Math.floor(abs.date.getTime() / 1000));
      const ms = String(abs.date.getTime());
      if (abs.kind === "unix") {
        return [
          {
            id: "ts:human",
            title: "时间戳 → 本地时间",
            preview: local,
            body: local,
            hint: abs.unit === "ms" ? "毫秒" : "秒",
            recommended: true,
          },
          {
            id: "ts:sec",
            title: "Unix 秒",
            preview: sec,
            body: sec,
          },
          {
            id: "ts:ms",
            title: "Unix 毫秒",
            preview: ms,
            body: ms,
          },
        ];
      }
      return [
        {
          id: "ts:to-s",
          title: "→ 秒时间戳",
          preview: sec,
          body: sec,
          recommended: true,
        },
        {
          id: "ts:to-ms",
          title: "→ 毫秒时间戳",
          preview: ms,
          body: ms,
        },
        {
          id: "ts:local",
          title: "本地时间",
          preview: local,
          body: local,
        },
      ];
    }

    return [];
  },
};
