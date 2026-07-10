#!/usr/bin/env python3
# FunCV plugin: stdin JSON {content, type} → stdout JSON
import json
import sys

def main():
    raw = sys.stdin.read()
    try:
        data = json.loads(raw or "{}")
    except Exception as e:
        print(json.dumps({"ok": False, "error": f"invalid json: {e}"}))
        sys.exit(1)

    content = str(data.get("content", ""))
    typ = str(data.get("type", "text"))
    if typ != "text":
        print(json.dumps({"ok": False, "error": "only text supported"}))
        return

    body = content.upper()
    print(
        json.dumps(
            {
                "ok": True,
                "title": "转大写",
                "body": body,
                "preview": body[:72],
                "hint": "python",
            },
            ensure_ascii=False,
        )
    )


if __name__ == "__main__":
    main()
