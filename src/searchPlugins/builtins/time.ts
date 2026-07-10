import type { SearchPlugin, SearchPluginResult } from "../types";
import {
  FAMOUS_TIME_ZONES,
  formatDateTime,
  formatInTimeZone,
  localUtcOffsetLabel,
} from "../../utils/time";

/** Triggers: 时间 / time / now / timestamp / 时间戳 / ts … */
const TRIGGERS = [
  "时间",
  "时间戳",
  "当前时间",
  "现在",
  "time",
  "now",
  "timestamp",
  "ts",
  "datetime",
  "date",
];

const TRIGGER_SET = new Set(TRIGGERS.map((t) => t.toLowerCase()));

export function buildTimeResult(_query: string, now = new Date()): SearchPluginResult {
  const sec = Math.floor(now.getTime() / 1000);
  const ms = now.getTime();
  const local = formatDateTime(now);

  const zoneLines = FAMOUS_TIME_ZONES.map((z) => {
    const t = formatInTimeZone(now, z.id);
    return `${z.label.padEnd(22, " ")}  ${t}`;
  }).join("\n");

  const body = [
    "当前时间",
    "────────",
    `本地时间     ${local}  (${localUtcOffsetLabel(now)})`,
    `Unix 秒     ${sec}`,
    `Unix 毫秒   ${ms}`,
    "",
    "主要时区",
    "────────",
    zoneLines,
  ].join("\n");

  // Prefer copying both timestamps for paste into code/logs
  const copyText = [
    `unix_s=${sec}`,
    `unix_ms=${ms}`,
    `local=${local}`,
    ...FAMOUS_TIME_ZONES.map((z) => `${z.id}=${formatInTimeZone(now, z.id)}`),
  ].join("\n");

  return {
    pluginId: "search-time",
    title: "时间",
    body,
    copyText,
    hint: "实时",
    live: true,
  };
}

export const timeSearchPlugin: SearchPlugin = {
  id: "search-time",
  name: "时间",
  description:
    "搜索框输入「时间 / time / now / 时间戳」等，在内容区显示当前 Unix 秒/毫秒与主要时区时间。",
  version: "1.0.0",
  triggers: TRIGGERS,
  defaultInstalled: false,
  match(query: string): boolean {
    const t = query.trim().toLowerCase();
    if (!t || t.length > 32) return false;
    if (TRIGGER_SET.has(t)) return true;
    // allow light variants: "time now", "now()", "时间 "
    const compact = t.replace(/[\s()（）]+/g, "");
    if (TRIGGER_SET.has(compact)) return true;
    // bare "时间戳秒" / "timestamp ms" style
    if (/^(时间|time|timestamp|ts|now)/i.test(t) && t.length <= 16) {
      if (/^(时间|time|timestamp|ts|now|datetime|date)([:：\s_-]*(now|stamp|戳|秒|毫秒|ms|s))?$/i.test(t)) {
        return true;
      }
    }
    return false;
  },
  run(query: string): SearchPluginResult | null {
    if (!this.match(query)) return null;
    return buildTimeResult(query);
  },
};
