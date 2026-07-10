/**
 * Absolute-match dictionary (word / short phrase) → vertical copyable rows.
 * Driven by ECDICT translate plugin body when the entire panel text is a word-like query.
 */

import type { AbsParseRow } from "./timestampAbs";

export type AbsDictPanel = {
  title: string;
  source: string;
  direction: "en-zh" | "zh-en" | "unknown";
  rows: AbsParseRow[];
};

/** Entire clipboard/panel text looks like a dictionary query (not prose / code). */
export function isAbsoluteDictQuery(raw: string): boolean {
  const t = raw.trim();
  if (!t || t.length > 80) return false;
  // no URLs / paths / emails
  if (/:\/\//.test(t) || /@/.test(t) || /\\/.test(t) || /\//.test(t)) return false;
  // pure unix timestamps handled elsewhere
  if (/^-?\d{10}(\d{3})?$/.test(t)) return false;

  if (isMostlyAsciiWord(t)) return true;
  if (isShortCjk(t)) return true;
  return false;
}

function isMostlyAsciiWord(s: string): boolean {
  const words = s.split(/\s+/).filter(Boolean);
  if (words.length === 0 || words.length > 4) return false;
  return words.every((w) => /^[A-Za-z]+(?:['\-][A-Za-z]+)*$/.test(w));
}

function isShortCjk(s: string): boolean {
  // no latin letters mixed for absolute ZH query (allow digits rare)
  if (/[A-Za-z]/.test(s)) return false;
  const chars = [...s.replace(/\s+/g, "")];
  if (chars.length === 0 || chars.length > 12) return false;
  return chars.every((c) => {
    const u = c.codePointAt(0)!;
    return (
      (u >= 0x4e00 && u <= 0x9fff) ||
      (u >= 0x3400 && u <= 0x4dbf) ||
      (u >= 0xf900 && u <= 0xfaff) ||
      c === "·" ||
      c === "—" ||
      c === "-"
    );
  });
}

/**
 * Build stacked rows from translate plugin body.
 * EN→ZH body:
 *   word
 *   /phonetic/
 *   sense lines…
 * ZH→EN body:
 *   word /phonetic/
 *     translation
 *   (blank line between candidates)
 */
export function buildAbsDictPanel(
  source: string,
  body: string,
  title: string,
  hint?: string,
): AbsDictPanel {
  const cleaned = body.replace(/\\n/g, "\n").trim();
  // Prefer explicit arrows: 英→汉 vs 汉→英 (don't use bare 「汉」 which appears in both)
  let direction: AbsDictPanel["direction"] = "unknown";
  if (/汉\s*[→\->]/.test(title) || /zh\s*[-→]\s*en/i.test(title)) {
    direction = "zh-en";
  } else if (/英\s*[→\->]/.test(title) || /en\s*[-→]\s*zh/i.test(title)) {
    direction = "en-zh";
  } else if (cleaned.includes("\n\n") && !/^\/.+\/$/m.test(cleaned.split("\n")[1] || "")) {
    // multi-block candidates without phonetic-on-line-2 pattern
    direction = "zh-en";
  } else {
    direction = "en-zh";
  }

  // 原文在上方编辑区展示，下半区只放解析值
  const rows: AbsParseRow[] = [];

  if (direction === "zh-en") {
    // Multiple EN candidates
    const blocks = cleaned
      .split(/\n\s*\n/)
      .map((b) => b.trim())
      .filter(Boolean);
    blocks.forEach((block, i) => {
      const lines = block.split("\n").map((l) => l.trim()).filter(Boolean);
      if (!lines.length) return;
      const head = lines[0]!;
      // "word /phonetic/" or "word"
      const m = /^(.+?)\s+\/([^/]+)\/\s*$/.exec(head);
      const word = m ? m[1]!.trim() : head;
      const phonetic = m ? m[2]!.trim() : "";
      const gloss = lines
        .slice(1)
        .map((l) => l.replace(/^\s+/, ""))
        .join("\n")
        .trim();

      rows.push({
        id: `cand-${i}-word`,
        label: blocks.length > 1 ? `英文 ${i + 1}` : "英文",
        value: word,
        hint: phonetic ? `/${phonetic}/` : undefined,
      });
      if (phonetic) {
        rows.push({
          id: `cand-${i}-ph`,
          label: "音标",
          value: phonetic,
        });
      }
      if (gloss) {
        rows.push({
          id: `cand-${i}-tr`,
          label: "释义",
          value: gloss,
        });
      }
    });
  } else {
    // EN → ZH single entry
    const lines = cleaned.split("\n").map((l) => l.trimEnd());
    const nonempty = lines.map((l) => l.trim()).filter((l) => l.length > 0);
    let idx = 0;
    if (nonempty[0] && /^[A-Za-z]/.test(nonempty[0])) {
      // headword (may equal source)
      if (nonempty[0]!.toLowerCase() !== source.trim().toLowerCase()) {
        rows.push({
          id: "head",
          label: "词条",
          value: nonempty[0]!,
        });
      }
      idx = 1;
    }
    if (nonempty[idx] && /^\/.+\/$/.test(nonempty[idx]!)) {
      const ph = nonempty[idx]!.replace(/^\/|\/$/g, "");
      rows.push({
        id: "phonetic",
        label: "音标",
        value: ph,
        hint: `/${ph}/`,
      });
      idx += 1;
    }
    const senses = nonempty.slice(idx);
    if (senses.length === 1) {
      rows.push({
        id: "sense-0",
        label: "释义",
        value: senses[0]!,
      });
    } else if (senses.length > 1) {
      senses.forEach((s, i) => {
        rows.push({
          id: `sense-${i}`,
          label: `释义 ${i + 1}`,
          value: s,
        });
      });
    }
  }

  // Full result for one-shot copy
  if (cleaned) {
    rows.push({
      id: "all",
      label: "完整结果",
      value: cleaned,
      hint: hint || "全部",
    });
  }

  return { title: title || "词典", source: source.trim(), direction, rows };
}

/** Find translate plugin suggestion id. */
export function isTranslateSuggestionId(id: string): boolean {
  return id === "plugin:translate-en-zh" || id.includes("translate");
}
