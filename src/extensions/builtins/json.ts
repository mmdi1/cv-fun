import type { ParseSuggestion } from "../../core/types";
import type { ContentContext, ContentExtension } from "../types";

/**
 * JSON smart suggestions:
 * 1. Minified one-liner → offer pretty
 * 2. Pretty multi-line → offer compact
 * 3. Escaped / log-style → offer clean + pretty
 * 4. Always offer alternate form when parse succeeds
 */

export type JsonRecover =
  | {
      ok: true;
      value: unknown;
      sourceForm: "pretty" | "compact" | "escaped";
      recoveredFromEscapes: boolean;
    }
  | { ok: false };

function looksLikeJsonShape(text: string): boolean {
  const t = text.trim();
  if (!t) return false;
  return (
    /^[{[]/.test(t) ||
    /^"\s*[{[]/.test(t) ||
    /^[{\[]\\/.test(t) ||
    /^\\?[{\[]/.test(t) ||
    /^{\\["nrt]/.test(t) ||
    /^"\{/.test(t)
  );
}

function isPrettySource(text: string): boolean {
  return /[\r\n]/.test(text.trim());
}

export function unescapeJsonNoise(input: string): string {
  let s = input.trim();
  if (
    (s.startsWith("'") && s.endsWith("'") && s.length >= 2) ||
    (s.startsWith("`") && s.endsWith("`") && s.length >= 2)
  ) {
    s = s.slice(1, -1);
  }

  for (let i = 0; i < 3; i++) {
    const next = s
      .replace(/\\r\\n/g, "\n")
      .replace(/\\n/g, "\n")
      .replace(/\\r/g, "\n")
      .replace(/\\t/g, "\t")
      .replace(/\\"/g, '"')
      .replace(/\\'/g, "'");
    const collapsed = next.replace(/\\\\/g, "\\");
    if (collapsed === s) break;
    s = collapsed;
  }
  return s;
}

function tryParse(text: string): unknown | undefined {
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

export function recoverJson(raw: string): JsonRecover {
  const trimmed = raw.trim();
  if (!trimmed || !looksLikeJsonShape(trimmed)) {
    return { ok: false };
  }

  let value = tryParse(trimmed);
  if (value !== undefined) {
    if (typeof value === "string") {
      const innerTrim = value.trim();
      if (/^[{[]/.test(innerTrim)) {
        const inner = tryParse(innerTrim);
        if (inner !== undefined) {
          return {
            ok: true,
            value: inner,
            sourceForm: isPrettySource(innerTrim) ? "pretty" : "compact",
            recoveredFromEscapes: true,
          };
        }
        const unescapedInner = unescapeJsonNoise(innerTrim);
        const inner2 = tryParse(unescapedInner);
        if (inner2 !== undefined) {
          return {
            ok: true,
            value: inner2,
            sourceForm: "escaped",
            recoveredFromEscapes: true,
          };
        }
      }
    }

    return {
      ok: true,
      value,
      sourceForm: isPrettySource(trimmed) ? "pretty" : "compact",
      recoveredFromEscapes: false,
    };
  }

  const unescaped = unescapeJsonNoise(trimmed);
  if (unescaped !== trimmed) {
    value = tryParse(unescaped);
    if (value !== undefined) {
      if (typeof value === "string" && /^[{[]/.test(value.trim())) {
        const inner = tryParse(value.trim());
        if (inner !== undefined) {
          return {
            ok: true,
            value: inner,
            sourceForm: "escaped",
            recoveredFromEscapes: true,
          };
        }
      }
      return {
        ok: true,
        value,
        sourceForm: "escaped",
        recoveredFromEscapes: true,
      };
    }
  }

  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    const stripped = unescapeJsonNoise(trimmed.slice(1, -1));
    value = tryParse(stripped);
    if (value !== undefined) {
      return {
        ok: true,
        value,
        sourceForm: "escaped",
        recoveredFromEscapes: true,
      };
    }
  }

  return { ok: false };
}

function oneLinePreview(text: string, max = 72): string {
  const line = text.replace(/\s+/g, " ").trim();
  return line.length > max ? `${line.slice(0, max)}…` : line;
}

function describeValue(value: unknown): string {
  if (Array.isArray(value)) return `数组 ${value.length} 项`;
  if (value !== null && typeof value === "object") {
    return `对象 ${Object.keys(value as object).length} 键`;
  }
  return String(typeof value);
}

export function inferJsonOutput(recovered: Extract<JsonRecover, { ok: true }>): {
  body: string;
  form: "pretty" | "compact";
  chip: string;
  hint: string;
} {
  const pretty = JSON.stringify(recovered.value, null, 2);
  const compact = JSON.stringify(recovered.value);

  if (recovered.recoveredFromEscapes || recovered.sourceForm === "escaped") {
    return {
      body: pretty,
      form: "pretty",
      chip: "清洗并格式化",
      hint: "去掉转义后格式化",
    };
  }
  if (recovered.sourceForm === "pretty") {
    return {
      body: compact,
      form: "compact",
      chip: "压缩为一行",
      hint: "多行 → 单行",
    };
  }
  return {
    body: pretty,
    form: "pretty",
    chip: "格式化",
    hint: "单行 → 缩进",
  };
}

export const jsonExtension: ContentExtension = {
  id: "json",

  suggest(ctx: ContentContext): ParseSuggestion[] {
    if (ctx.kind !== "text") return [];
    const recovered = recoverJson(ctx.rawText);
    if (!recovered.ok) return [];

    const pretty = JSON.stringify(recovered.value, null, 2);
    const compact = JSON.stringify(recovered.value);
    const meta = describeValue(recovered.value);
    const primary = inferJsonOutput(recovered);
    const out: ParseSuggestion[] = [];

    if (recovered.recoveredFromEscapes || recovered.sourceForm === "escaped") {
      out.push({
        id: "json:clean-pretty",
        title: "清洗并格式化",
        preview: oneLinePreview(pretty),
        body: pretty,
        hint: meta,
        recommended: true,
      });
      out.push({
        id: "json:clean-compact",
        title: "清洗并压缩",
        preview: oneLinePreview(compact),
        body: compact,
        hint: meta,
      });
      return out;
    }

    // Primary recommendation based on source form
    if (primary.form === "pretty") {
      out.push({
        id: "json:pretty",
        title: "格式化 JSON",
        preview: oneLinePreview(pretty),
        body: pretty,
        hint: meta,
        recommended: true,
      });
      out.push({
        id: "json:compact",
        title: "压缩为一行",
        preview: oneLinePreview(compact),
        body: compact,
        hint: meta,
      });
    } else {
      out.push({
        id: "json:compact",
        title: "压缩为一行",
        preview: oneLinePreview(compact),
        body: compact,
        hint: meta,
        recommended: true,
      });
      out.push({
        id: "json:pretty",
        title: "格式化 JSON",
        preview: oneLinePreview(pretty),
        body: pretty,
        hint: meta,
      });
    }

    return out;
  },
};
