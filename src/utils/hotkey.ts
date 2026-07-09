export type ShortcutPlatform = "mac" | "windows" | "other";

export function detectShortcutPlatform(): ShortcutPlatform {
  const nav = window.navigator as Navigator & { userAgentData?: { platform?: string } };
  const platform = (nav.userAgentData?.platform || nav.platform || "").toLowerCase();
  if (platform.includes("mac")) return "mac";
  if (platform.includes("win")) return "windows";
  return "other";
}

export function defaultToggleHotkey(platform: ShortcutPlatform = detectShortcutPlatform()): string {
  return platform === "mac" ? "Option+Space" : "Alt+Space";
}

function keyFromEvent(
  event: Pick<KeyboardEvent, "key"> & Partial<Pick<KeyboardEvent, "code">>,
) {
  const code = event.code || "";
  if (code === "Space") return "Space";
  if (code === "Comma") return ",";
  if (code === "Period") return ".";
  if (code === "Slash") return "/";
  if (code === "Semicolon") return ";";
  if (code === "Quote") return "'";
  if (code === "BracketLeft") return "[";
  if (code === "BracketRight") return "]";
  if (code === "Backslash") return "\\";
  if (code === "Minus") return "-";
  if (code === "Equal") return "=";
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (code.startsWith("Arrow")) return code.replace("Arrow", "");
  if (event.key === " ") return "Space";
  return event.key.length === 1 ? event.key.toUpperCase() : event.key;
}

/** Display string e.g. Option+Space, Ctrl+Shift+V */
export function shortcutDisplayFromEvent(
  event: Pick<KeyboardEvent, "altKey" | "ctrlKey" | "metaKey" | "shiftKey" | "key"> &
    Partial<Pick<KeyboardEvent, "code">>,
  platform: ShortcutPlatform = detectShortcutPlatform(),
): string {
  const key = keyFromEvent(event);
  const lower = key.toLowerCase();
  if (["alt", "option", "control", "ctrl", "meta", "command", "shift"].includes(lower)) {
    return "";
  }

  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push(platform === "mac" ? "Option" : "Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push(platform === "mac" ? "Cmd" : "Meta");

  const displayKey =
    lower === " " || lower === "space" ? "Space" : key.length === 1 ? key.toUpperCase() : key;
  parts.push(displayKey);

  if (parts.length < 2) return "";
  return parts.join("+");
}
