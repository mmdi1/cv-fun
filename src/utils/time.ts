const pad = (n: number) => String(n).padStart(2, "0");

export function formatDateTime(value: unknown): string {
  if (!value) return "-";
  const date = value instanceof Date ? value : new Date(String(value));
  if (Number.isNaN(date.getTime())) return String(value);
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

/** Format date in a named IANA timezone (local wall clock). */
export function formatInTimeZone(date: Date, timeZone: string): string {
  try {
    const parts = new Intl.DateTimeFormat("en-CA", {
      timeZone,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
      hourCycle: "h23",
    }).formatToParts(date);
    const get = (type: string) => parts.find((p) => p.type === type)?.value ?? "";
    const h = get("hour");
    const hh = h === "24" ? "00" : h;
    return `${get("year")}-${get("month")}-${get("day")} ${hh}:${get("minute")}:${get("second")}`;
  } catch {
    return "—";
  }
}

export function localUtcOffsetLabel(date: Date): string {
  const offsetMin = -date.getTimezoneOffset();
  const sign = offsetMin >= 0 ? "+" : "-";
  const abs = Math.abs(offsetMin);
  return `UTC${sign}${pad(Math.floor(abs / 60))}:${pad(abs % 60)}`;
}

/** Zones used for absolute timestamp panels / time plugin. */
export const FAMOUS_TIME_ZONES: { id: string; label: string }[] = [
  { id: "UTC", label: "UTC" },
  { id: "America/Los_Angeles", label: "洛杉矶 (US Pacific)" },
  { id: "America/New_York", label: "纽约 (US Eastern)" },
  { id: "Europe/London", label: "伦敦" },
  { id: "Europe/Paris", label: "巴黎" },
  { id: "Asia/Shanghai", label: "北京 / 上海" },
  { id: "Asia/Tokyo", label: "东京" },
  { id: "Asia/Singapore", label: "新加坡" },
  { id: "Australia/Sydney", label: "悉尼" },
];

export function formatHistoryTime(value: unknown, now = new Date()): string {
  if (!value) return "-";
  const date = new Date(String(value));
  if (Number.isNaN(date.getTime())) return String(value);
  const sameDay =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();
  if (sameDay) {
    return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
  }
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}
