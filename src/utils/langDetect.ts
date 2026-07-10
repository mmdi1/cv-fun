import { looksLikeSql } from "../extensions/builtins/sql";
import { recoverJson } from "../extensions/builtins/json";

export type CodeLang =
  | "json"
  | "sql"
  | "go"
  | "javascript"
  | "typescript"
  | "python"
  | "plaintext";

export const CODE_LANG_LABEL: Record<CodeLang, string> = {
  json: "JSON",
  sql: "SQL",
  go: "Go",
  javascript: "JavaScript",
  typescript: "TypeScript",
  python: "Python",
  plaintext: "文本",
};

/**
 * Detect programming / structured language for highlight.
 * `hint` can be suggestion id like "sql:pretty" or "json:clean-pretty".
 */
export function detectCodeLang(text: string, hint?: string | null): CodeLang {
  const t = text.trim();
  if (!t) return "plaintext";

  const h = (hint || "").toLowerCase();
  if (h.includes("json")) return "json";
  if (h.includes("sql")) return "sql";
  if (h.includes("python") || h.includes("py")) return "python";
  if (h.includes("typescript") || h.includes("ts")) return "typescript";
  if (h.includes("javascript") || h.includes("node") || /\bjs\b/.test(h)) return "javascript";
  if (h.includes("golang") || /\bgo\b/.test(h)) return "go";

  // JSON first (strict structure)
  if (recoverJson(t).ok) return "json";
  if (/^[{[]/.test(t) && /[}\]]$/.test(t) && /["']?\w+["']?\s*:/.test(t)) {
    try {
      JSON.parse(t);
      return "json";
    } catch {
      // fall through
    }
  }

  if (looksLikeSql(t)) return "sql";

  // Go
  if (
    /^\s*package\s+\w+/m.test(t) ||
    (/\bfunc\s+(\(\w+\s+\*?\w+\)\s+)?\w+\s*\(/.test(t) &&
      (/\bimport\s+(?:\(|")/.test(t) || /\bfmt\.|\berr\s*!=\s*nil\b/.test(t)))
  ) {
    return "go";
  }

  // TypeScript (before JS)
  if (
    /\b(interface|type)\s+\w+\s*[=<{]/.test(t) ||
    /:\s*(string|number|boolean|any|void|unknown)\b/.test(t) ||
    /\bimport\s+type\b/.test(t) ||
    /\bas\s+const\b/.test(t)
  ) {
    return "typescript";
  }

  // JavaScript / TSX-ish
  if (
    /\b(function|const|let|var|=>|import\s+|export\s+|require\s*\()/.test(t) &&
    /[{};()]/.test(t) &&
    !/^\s*def\s+\w+\s*\(/m.test(t)
  ) {
    return "javascript";
  }

  // Python
  if (
    /^\s*(def|class|async\s+def)\s+\w+/m.test(t) ||
    (/^\s*import\s+\w+/m.test(t) && /^\s*(from\s+\w+\s+import|print\s*\()/m.test(t)) ||
    /^\s*if\s+__name__\s*==\s*['"]__main__['"]/m.test(t)
  ) {
    return "python";
  }

  return "plaintext";
}
