/**
 * Absolute-match timestamp → vertical copyable parse rows for the content panel.
 * Matches pure Unix s/ms or a single datetime string (entire text).
 */

import {
  FAMOUS_TIME_ZONES,
  formatDateTime,
  formatInTimeZone,
  localUtcOffsetLabel,
} from "./time";

export type AbsParseRow = {
  id: string;
  /** Short label above the value */
  label: string;
  /** Value shown / copied on click */
  value: string;
  /** Optional secondary hint */
  hint?: string;
};

export type AbsTimestampPanel = {
  kind: "unix" | "datetime";
  unit?: "s" | "ms";
  source: string;
  date: Date;
  rows: AbsParseRow[];
};

function parseUnixAbsolute(text: string): { date: Date; unit: "s" | "ms" } | null {
  // pure integer: 10 digits (sec) or 13 (ms); allow optional sign
  if (!/^-?\d{10}(\d{3})?$/.test(text)) return null;
  const raw = Number(text);
  if (!Number.isFinite(raw)) return null;
  const digits = text.replace(/^-/, "").length;
  const unit: "s" | "ms" = digits === 13 ? "ms" : "s";
  const ms = unit === "ms" ? raw : raw * 1000;
  const date = new Date(ms);
  if (Number.isNaN(date.getTime())) return null;
  const y = date.getFullYear();
  // sensible clipboard range
  if (y < 1970 || y > 2200) return null;
  return { date, unit };
}

function parseHumanAbsolute(text: string): Date | null {
  const t = text.trim();
  const m =
    /^(\d{4})[-/](\d{1,2})[-/](\d{1,2})(?:[ T](\d{1,2}):(\d{1,2})(?::(\d{1,2}))?)?$/.exec(t);
  if (m) {
    const date = new Date(
      Number(m[1]),
      Number(m[2]) - 1,
      Number(m[3]),
      Number(m[4] ?? 0),
      Number(m[5] ?? 0),
      Number(m[6] ?? 0),
    );
    if (!Number.isNaN(date.getTime())) return date;
  }
  if (/^\d{4}-\d{2}-\d{2}T/.test(t)) {
    const date = new Date(t);
    if (!Number.isNaN(date.getTime())) return date;
  }
  return null;
}

function buildRows(date: Date): AbsParseRow[] {
  const sec = String(Math.floor(date.getTime() / 1000));
  const ms = String(date.getTime());
  const local = formatDateTime(date);
  // 原文在上方编辑区展示，下半区只放解析值
  const rows: AbsParseRow[] = [
    {
      id: "local",
      label: "本地时间",
      value: local,
      hint: localUtcOffsetLabel(date),
    },
    {
      id: "unix-s",
      label: "Unix 秒",
      value: sec,
    },
    {
      id: "unix-ms",
      label: "Unix 毫秒",
      value: ms,
    },
  ];

  for (const z of FAMOUS_TIME_ZONES) {
    rows.push({
      id: `tz:${z.id}`,
      label: z.label,
      value: formatInTimeZone(date, z.id),
      hint: z.id,
    });
  }
  return rows;
}

/**
 * If `raw` is entirely a timestamp / single datetime, return a panel model.
 * Returns null for mixed text (not absolute match).
 */
export function tryAbsoluteTimestampPanel(raw: string): AbsTimestampPanel | null {
  const source = raw.trim();
  if (!source || source.length > 64) return null;

  const unix = parseUnixAbsolute(source);
  if (unix) {
    return {
      kind: "unix",
      unit: unix.unit,
      source,
      date: unix.date,
      rows: buildRows(unix.date),
    };
  }

  const human = parseHumanAbsolute(source);
  if (human) {
    return {
      kind: "datetime",
      source,
      date: human,
      rows: buildRows(human),
    };
  }

  return null;
}
