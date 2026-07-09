import { format as formatSql } from "sql-formatter";
import type { ParseSuggestion } from "../../core/types";
import type { ContentContext, ContentExtension } from "../types";
import { recoverJson } from "./json";

const SQL_START =
  /^\s*(?:WITH|SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP|TRUNCATE|REPLACE|MERGE|EXPLAIN|SHOW|DESCRIBE|DESC|USE|SET|CALL|BEGIN|COMMIT|ROLLBACK|GRANT|REVOKE)\b/i;

const SQL_KEYWORDS =
  /\b(?:SELECT|FROM|WHERE|JOIN|INNER|LEFT|RIGHT|OUTER|FULL|ON|GROUP\s+BY|ORDER\s+BY|HAVING|LIMIT|OFFSET|INSERT\s+INTO|VALUES|UPDATE|SET|DELETE\s+FROM|CREATE\s+TABLE|ALTER\s+TABLE|DROP\s+TABLE|UNION(?:\s+ALL)?|AS|AND|OR|NOT|IN|EXISTS|BETWEEN|LIKE|IS\s+NULL|DISTINCT|COUNT|SUM|AVG|MAX|MIN|CASE|WHEN|THEN|ELSE|END)\b/i;

function oneLinePreview(text: string, max = 72): string {
  const line = text.replace(/\s+/g, " ").trim();
  return line.length > max ? `${line.slice(0, max)}…` : line;
}

function isPrettySource(text: string): boolean {
  return /[\r\n]/.test(text.trim());
}

/** Heuristic: looks like SQL, not JSON / pure noise. */
export function looksLikeSql(raw: string): boolean {
  const text = raw.trim();
  if (!text || text.length < 8) return false;
  // Prefer JSON extension when content is valid JSON
  if (recoverJson(raw).ok) return false;
  if (/^[{[]/.test(text)) return false;

  if (SQL_START.test(text)) return true;

  // Multi-keyword body (e.g. fragment without leading SELECT after paste noise)
  const matches = text.match(
    /\b(?:SELECT|FROM|WHERE|JOIN|INSERT|UPDATE|DELETE|CREATE|GROUP|ORDER|LIMIT|VALUES)\b/gi,
  );
  if (matches && matches.length >= 2 && SQL_KEYWORDS.test(text)) {
    // Avoid English prose: require some SQL punctuation / structure
    if (/[(),=*<>`]/.test(text) || /\bFROM\b/i.test(text)) return true;
  }
  return false;
}

function tryFormat(sql: string): string | null {
  try {
    return formatSql(sql, {
      language: "sql",
      tabWidth: 2,
      keywordCase: "upper",
      linesBetweenQueries: 1,
    });
  } catch {
    // Dialect guess fallbacks
    for (const language of ["mysql", "postgresql", "sqlite", "mariadb", "tsql", "bigquery"] as const) {
      try {
        return formatSql(sql, {
          language,
          tabWidth: 2,
          keywordCase: "upper",
          linesBetweenQueries: 1,
        });
      } catch {
        // try next
      }
    }
    return null;
  }
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

    const pretty = tryFormat(raw.trim());
    if (!pretty) return [];

    // If formatter barely changed a non-SQL-looking result, still offer if looksLikeSql passed
    const compact = compactSql(pretty);
    const sourcePretty = isPrettySource(raw);
    const out: ParseSuggestion[] = [];

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

    // If source had messy whitespace but formatter differs from both, already covered
    return out;
  },
};
