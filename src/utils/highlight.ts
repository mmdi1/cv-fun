import hljs from "highlight.js/lib/core";
import go from "highlight.js/lib/languages/go";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import python from "highlight.js/lib/languages/python";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import type { CodeLang } from "./langDetect";

let registered = false;

function ensureLangs() {
  if (registered) return;
  hljs.registerLanguage("json", json);
  hljs.registerLanguage("sql", sql);
  hljs.registerLanguage("go", go);
  hljs.registerLanguage("javascript", javascript);
  hljs.registerLanguage("typescript", typescript);
  hljs.registerLanguage("python", python);
  registered = true;
}

/** Escape HTML for plaintext fallback. */
export function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/**
 * Highlight source for overlay display.
 * Always returns safe HTML (escaped if highlight fails).
 */
export function highlightCode(source: string, lang: CodeLang): string {
  if (!source) return "";
  ensureLangs();
  if (lang === "plaintext") {
    return escapeHtml(source);
  }
  try {
    return hljs.highlight(source, { language: lang, ignoreIllegals: true }).value;
  } catch {
    try {
      return hljs.highlightAuto(source, [
        "json",
        "sql",
        "go",
        "javascript",
        "typescript",
        "python",
      ]).value;
    } catch {
      return escapeHtml(source);
    }
  }
}
