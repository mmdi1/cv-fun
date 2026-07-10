/**
 * Safe arithmetic evaluator for the history search box.
 * Supports + - * / % ^, parentheses, decimals, and a few functions/constants.
 * Never uses eval / Function.
 */

export type MathEvalResult = {
  /** Normalized expression text. */
  expression: string;
  value: number;
  /** Human-readable result (or error label). */
  display: string;
  ok: boolean;
};

const MAX_LEN = 120;

const FUNCS: Record<string, (x: number) => number> = {
  sqrt: Math.sqrt,
  abs: Math.abs,
  sin: Math.sin,
  cos: Math.cos,
  tan: Math.tan,
  ln: Math.log,
  log: Math.log10,
  log10: Math.log10,
  floor: Math.floor,
  ceil: Math.ceil,
  round: Math.round,
  exp: Math.exp,
};

const CONSTS: Record<string, number> = {
  pi: Math.PI,
  e: Math.E,
};

/** Operators / structure that make text look like a formula (not a bare number). */
function hasMathStructure(s: string): boolean {
  // at least one binary-ish operator, or function call, or percent-like
  if (/[+\-*/%^]/.test(s.replace(/^[+\-]/, ""))) return true;
  if (/[（(]/.test(s) && /[）)]/.test(s)) return true;
  if (/\b(sqrt|abs|sin|cos|tan|ln|log|log10|floor|ceil|round|exp)\s*\(/i.test(s)) {
    return true;
  }
  return false;
}

/** Normalize common unicode / fullwidth math symbols. */
export function normalizeMathInput(raw: string): string {
  return raw
    .trim()
    .replace(/[＋]/g, "+")
    .replace(/[－−–—]/g, "-")
    .replace(/[×✕✖･·]/g, "*")
    .replace(/[÷／]/g, "/")
    .replace(/[（]/g, "(")
    .replace(/[）]/g, ")")
    .replace(/[，]/g, ",")
    .replace(/\s+/g, "");
}

/**
 * Quick gate: only attempt parse when input looks like a math formula.
 * Pure numbers / ordinary search keywords return null.
 */
export function looksLikeMathFormula(raw: string): boolean {
  const t = raw.trim();
  if (!t || t.length > MAX_LEN) return false;
  // avoid paths / URLs / emails
  if (/:\/\//.test(t) || /@/.test(t) || /\\/.test(t)) return false;
  // avoid SQL-ish or word search with spaces + letters only
  if (/[a-zA-Z\u4e00-\u9fff]{3,}/.test(t) && !/\b(sqrt|abs|sin|cos|tan|ln|log|floor|ceil|round|exp|pi)\b/i.test(t)) {
    return false;
  }
  const n = normalizeMathInput(t);
  if (!n) return false;
  // only allowed charset after normalize
  if (!/^[0-9a-zA-Z_+\-*/%^().,]+$/.test(n)) return false;
  if (!hasMathStructure(n)) return false;
  return true;
}

export function formatMathResult(n: number): string {
  if (!Number.isFinite(n)) {
    if (Number.isNaN(n)) return "无效";
    return n > 0 ? "∞" : "-∞";
  }
  if (Object.is(n, -0)) return "0";
  if (Number.isInteger(n) && Math.abs(n) < 1e15) return String(n);
  // trim floating noise
  const fixed = Number(n.toPrecision(12));
  if (Number.isInteger(fixed) && Math.abs(fixed) < 1e15) return String(fixed);
  let s = String(fixed);
  if (s.includes("e") || s.includes("E")) return s;
  if (s.includes(".")) {
    s = s.replace(/(\.\d*?)0+$/, "$1").replace(/\.$/, "");
  }
  return s;
}

class Parser {
  private s: string;
  private i = 0;

  constructor(s: string) {
    this.s = s;
  }

  parse(): number {
    const v = this.parseExpr();
    this.skipWs();
    if (this.i < this.s.length) {
      throw new Error(`unexpected '${this.s[this.i]}'`);
    }
    return v;
  }

  private skipWs() {
    while (this.i < this.s.length && /\s/.test(this.s[this.i]!)) this.i++;
  }

  private peek(): string {
    return this.s[this.i] ?? "";
  }

  private eat(ch: string): boolean {
    if (this.peek() === ch) {
      this.i++;
      return true;
    }
    return false;
  }

  private parseExpr(): number {
    let v = this.parseTerm();
    for (;;) {
      this.skipWs();
      if (this.eat("+")) v += this.parseTerm();
      else if (this.eat("-")) v -= this.parseTerm();
      else break;
    }
    return v;
  }

  private parseTerm(): number {
    let v = this.parsePower();
    for (;;) {
      this.skipWs();
      if (this.eat("*")) v *= this.parsePower();
      else if (this.eat("/")) {
        const r = this.parsePower();
        v /= r;
      } else if (this.eat("%")) {
        const r = this.parsePower();
        v %= r;
      } else break;
    }
    return v;
  }

  /** Right-associative ^ */
  private parsePower(): number {
    const base = this.parseUnary();
    this.skipWs();
    if (this.eat("^")) {
      const exp = this.parsePower();
      return base ** exp;
    }
    return base;
  }

  private parseUnary(): number {
    this.skipWs();
    if (this.eat("+")) return this.parseUnary();
    if (this.eat("-")) return -this.parseUnary();
    return this.parsePrimary();
  }

  private parsePrimary(): number {
    this.skipWs();
    const ch = this.peek();

    if (ch === "(") {
      this.i++;
      const v = this.parseExpr();
      this.skipWs();
      if (!this.eat(")")) throw new Error("missing )");
      return v;
    }

    // number
    if (/\d/.test(ch) || (ch === "." && /\d/.test(this.s[this.i + 1] ?? ""))) {
      return this.parseNumber();
    }

    // identifier: function or constant
    if (/[a-zA-Z_]/.test(ch)) {
      const start = this.i;
      this.i++;
      while (/[a-zA-Z_0-9]/.test(this.peek())) this.i++;
      const name = this.s.slice(start, this.i).toLowerCase();
      this.skipWs();
      if (this.eat("(")) {
        const fn = FUNCS[name];
        if (!fn) throw new Error(`unknown function ${name}`);
        const arg = this.parseExpr();
        this.skipWs();
        if (!this.eat(")")) throw new Error("missing )");
        return fn(arg);
      }
      const c = CONSTS[name];
      if (c !== undefined) return c;
      throw new Error(`unknown ${name}`);
    }

    throw new Error("expected number");
  }

  private parseNumber(): number {
    const start = this.i;
    while (/\d/.test(this.peek())) this.i++;
    if (this.peek() === ".") {
      this.i++;
      while (/\d/.test(this.peek())) this.i++;
    }
    // scientific notation 1e-3
    if (this.peek() === "e" || this.peek() === "E") {
      const ePos = this.i;
      this.i++;
      if (this.peek() === "+" || this.peek() === "-") this.i++;
      if (!/\d/.test(this.peek())) {
        this.i = ePos;
      } else {
        while (/\d/.test(this.peek())) this.i++;
      }
    }
    const raw = this.s.slice(start, this.i);
    const n = Number(raw);
    if (!Number.isFinite(n) && raw !== "Infinity") {
      // Number("1e999") can be Infinity — allow only if finite or we re-check
      if (Number.isNaN(n)) throw new Error("bad number");
    }
    return n;
  }
}

/**
 * Try to evaluate `raw` as a math formula.
 * Returns null when input is not a formula (fall through to history search).
 */
export function tryEvalMath(raw: string): MathEvalResult | null {
  if (!looksLikeMathFormula(raw)) return null;
  const expression = normalizeMathInput(raw);
  try {
    const value = new Parser(expression).parse();
    const ok = Number.isFinite(value);
    return {
      expression,
      value,
      display: formatMathResult(value),
      ok,
    };
  } catch {
    return null;
  }
}
