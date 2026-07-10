import { format as formatSql } from "sql-formatter";
import type { ParseSuggestion } from "../../core/types";
import type { ContentContext, ContentExtension } from "../types";
import { recoverJson } from "./json";

const SQL_START =
  /^\s*(?:WITH|SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP|TRUNCATE|REPLACE|MERGE|EXPLAIN|SHOW|DESCRIBE|DESC|USE|SET|CALL|BEGIN|COMMIT|ROLLBACK|GRANT|REVOKE)\b/i;

const SQL_KEYWORDS =
  /\b(?:SELECT|FROM|WHERE|JOIN|INNER|LEFT|RIGHT|OUTER|FULL|ON|GROUP\s+BY|ORDER\s+BY|HAVING|LIMIT|OFFSET|INSERT\s+INTO|VALUES|UPDATE|SET|DELETE\s+FROM|CREATE\s+TABLE|ALTER\s+TABLE|DROP\s+TABLE|UNION(?:\s+ALL)?|AS|AND|OR|NOT|IN|EXISTS|BETWEEN|LIKE|IS\s+NULL|DISTINCT|COUNT|SUM|AVG|MAX|MIN|CASE|WHEN|THEN|ELSE|END|DATE_FORMAT|COALESCE|NULLIF|ROUND|MAX|MIN)\b/i;

const DIALECTS = [
  "mysql",
  "sql",
  "postgresql",
  "mariadb",
  "sqlite",
  "tsql",
  "bigquery",
] as const;

function oneLinePreview(text: string, max = 72): string {
  const line = text.replace(/\s+/g, " ").trim();
  return line.length > max ? `${line.slice(0, max)}…` : line;
}

function isPrettySource(text: string): boolean {
  return /[\r\n]/.test(text.trim());
}

/** True if text still has common string-escape noise. */
function hasEscapeNoise(text: string): boolean {
  return /\\n|\\r|\\t|\\"|\\'/.test(text);
}

/**
 * Smart normalize for SQL pasted as escaped strings, e.g.
 * `\nSELECT\n  a\nFROM t\n`
 * or quoted: `"SELECT ..."` / `'SELECT ...'`
 */
export function normalizeSqlSource(raw: string): { sql: string; cleaned: boolean } {
  let s = raw.trim();
  if (!s) return { sql: s, cleaned: false };
  let cleaned = false;

  // Strip wrapping quotes (JSON string / log copy)
  for (let i = 0; i < 2; i++) {
    if (
      (s.startsWith('"') && s.endsWith('"') && s.length >= 2) ||
      (s.startsWith("'") && s.endsWith("'") && s.length >= 2) ||
      (s.startsWith("`") && s.endsWith("`") && s.length >= 2)
    ) {
      s = s.slice(1, -1).trim();
      cleaned = true;
    }
  }

  // Strip common prefixes: sql:, query=, SQL =
  const prefix = s.match(/^(?:sql|query|statement)\s*[=:：]\s*/i);
  if (prefix) {
    s = s.slice(prefix[0].length).trim();
    cleaned = true;
  }

  // Unescape literal \n \r \t \" \' (often from logs / Java / JSON)
  if (hasEscapeNoise(s)) {
    for (let i = 0; i < 3; i++) {
      const next = s
        .replace(/\\r\\n/g, "\n")
        .replace(/\\n/g, "\n")
        .replace(/\\r/g, "\n")
        .replace(/\\t/g, "\t")
        .replace(/\\"/g, '"')
        .replace(/\\'/g, "'");
      // Collapse remaining double backslashes carefully
      const collapsed = next.replace(/\\\\/g, "\\");
      if (collapsed === s) break;
      s = collapsed;
      cleaned = true;
    }
  }

  // Leading/trailing junk: leading "\n" already handled; trim whitespace
  const trimmed = s.trim();
  if (trimmed !== s) {
    s = trimmed;
    cleaned = true;
  }

  // If still starts with literal backslash-n after partial clean
  s = s.replace(/^(?:\\n|\s)+/i, "").replace(/(?:\\n|\s)+$/i, "").trim();

  return { sql: s, cleaned };
}

/** Heuristic: looks like SQL (after smart normalize). */
export function looksLikeSql(raw: string): boolean {
  if (!raw.trim() || raw.trim().length < 6) return false;
  // Prefer JSON extension when content is valid JSON object/array
  if (recoverJson(raw).ok) {
    const r = recoverJson(raw);
    if (r.ok && typeof r.value !== "string") return false;
  }
  if (/^[{[]/.test(raw.trim()) && !hasEscapeNoise(raw)) return false;

  const { sql } = normalizeSqlSource(raw);
  if (!sql || sql.length < 6) return false;

  if (SQL_START.test(sql)) return true;

  const matches = sql.match(
    /\b(?:SELECT|FROM|WHERE|JOIN|INSERT|UPDATE|DELETE|CREATE|GROUP|ORDER|LIMIT|VALUES|DATE_FORMAT)\b/gi,
  );
  if (matches && matches.length >= 2 && SQL_KEYWORDS.test(sql)) {
    if (/[(),=*<>`]/.test(sql) || /\bFROM\b/i.test(sql)) return true;
  }
  return false;
}

function tryFormat(sql: string): string | null {
  // Prefer mysql first: DATE_FORMAT / common analytics SQL
  for (const language of DIALECTS) {
    try {
      return formatSql(sql, {
        language,
        tabWidth: 2,
        keywordCase: "upper",
        linesBetweenQueries: 1,
      });
    } catch {
      // try next dialect
    }
  }
  return null;
}

function compactSql(sql: string): string {
  return sql
    .replace(/\/\*[\s\S]*?\*\//g, " ")
    .replace(/--[^\n\r]*/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

export const sqlExtension: ContentExtension = {
  id: "sql",

  suggest(ctx: ContentContext): ParseSuggestion[] {
    if (ctx.kind !== "text") return [];
    const raw = ctx.rawText;
    if (!looksLikeSql(raw)) return [];

    const { sql: normalized, cleaned } = normalizeSqlSource(raw);
    const pretty = tryFormat(normalized);
    if (!pretty) return [];

    const compact = compactSql(pretty);
    // Source is "pretty" only if it already had real newlines (not only \n escapes)
    const sourcePretty = isPrettySource(normalized) && !cleaned;
    const out: ParseSuggestion[] = [];

    if (cleaned) {
      // Escaped / log-style paste → always recommend clean + format
      out.push({
        id: "sql:clean-pretty",
        title: "清洗并格式化 SQL",
        preview: oneLinePreview(pretty),
        body: pretty,
        hint: "去掉 \\n 转义 · 缩进",
        recommended: true,
      });
      out.push({
        id: "sql:clean-compact",
        title: "清洗并压缩 SQL",
        preview: oneLinePreview(compact),
        body: compact,
        hint: "单行",
      });
      return out;
    }

    if (!sourcePretty) {
      out.push({
        id: "sql:pretty",
        title: "格式化 SQL",
        preview: oneLinePreview(pretty),
        body: pretty,
        hint: "缩进 · 关键字大写",
        recommended: true,
      });
      out.push({
        id: "sql:compact",
        title: "压缩 SQL",
        preview: oneLinePreview(compact),
        body: compact,
        hint: "单行",
      });
    } else {
      out.push({
        id: "sql:compact",
        title: "压缩 SQL",
        preview: oneLinePreview(compact),
        body: compact,
        hint: "单行",
        recommended: true,
      });
      out.push({
        id: "sql:pretty",
        title: "格式化 SQL",
        preview: oneLinePreview(pretty),
        body: pretty,
        hint: "缩进 · 关键字大写",
      });
    }

    return out;
  },
};
