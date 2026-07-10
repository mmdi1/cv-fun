#!/bin/sh
# FunCV shell plugin: read JSON from stdin, write JSON to stdout
# Requires python3 for JSON parse (portable enough on macOS/Linux)
python3 - <<'PY'
import json, sys
raw = sys.stdin.read()
try:
    data = json.loads(raw or "{}")
except Exception as e:
    print(json.dumps({"ok": False, "error": str(e)}))
    raise SystemExit(0)
content = str(data.get("content", ""))
n = len(content)
chars = len(list(content))
lines = content.count("\n") + (1 if content else 0)
body = f"字符(bytes) {n}\nUnicode 字符 {chars}\n行数 {lines}"
print(json.dumps({
    "ok": True,
    "title": "字数",
    "body": body,
    "preview": f"{chars} 字符 · {lines} 行",
    "hint": "shell",
}, ensure_ascii=False))
PY
