#!/usr/bin/env node
/**
 * FunCV plugin contract:
 *   stdin  → { "content": string, "type": "text" | "img" }
 *   stdout → { "ok": true, "title", "body", "preview", "hint" }
 *         or { "ok": false, "error": "..." }
 */
const chunks = [];
process.stdin.setEncoding("utf8");
process.stdin.on("data", (c) => chunks.push(c));
process.stdin.on("end", () => {
  try {
    const input = JSON.parse(chunks.join("") || "{}");
    const content = String(input.content ?? "");
    const type = String(input.type ?? "text");
    const body = `[node-echo] type=${type}\n${content}`;
    process.stdout.write(
      JSON.stringify({
        ok: true,
        title: "Node 回显",
        body,
        preview: content.slice(0, 72),
        hint: "example",
      }),
    );
  } catch (e) {
    process.stdout.write(JSON.stringify({ ok: false, error: String(e) }));
    process.exitCode = 1;
  }
});
