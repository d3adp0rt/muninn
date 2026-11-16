#!/usr/bin/env python3
"""Tiny OpenAI-compatible mock for Muninn development.

Serves /v1/models and /v1/chat/completions (SSE) with reasoning, content,
usage and a delay between chunks. No dependencies beyond the stdlib.

Usage:
    python3 scripts/mock-server.py [port]
    adb reverse tcp:8765 tcp:8765
    # base URL in Muninn: http://127.0.0.1:8765/v1
"""
import json
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8765

MODELS = {
    "data": [
        {
            "id": "mock/free",
            "name": "Mock Free",
            "context_length": 128000,
            "pricing": {"prompt": "0", "completion": "0", "request": "0"},
            "architecture": {"input_modalities": ["text"], "output_modalities": ["text"]},
            "supported_parameters": ["reasoning", "tools", "max_tokens"],
            "top_provider": {"context_length": 128000, "max_completion_tokens": 8192},
        },
        {
            "id": "mock/smart",
            "name": "Mock Smart",
            "context_length": 200000,
            "pricing": {"prompt": "0.000003", "completion": "0.000015"},
            "architecture": {"input_modalities": ["text"], "output_modalities": ["text"]},
            "supported_parameters": ["reasoning", "tools", "max_tokens"],
            "top_provider": {"context_length": 200000, "max_completion_tokens": 8192},
        },
    ]
}


def chunk(delta, finish=None, usage=None):
    payload = {
        "id": "gen-mock",
        "model": "mock/free",
        "provider": "Mock",
        "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
    }
    if usage:
        payload["usage"] = usage
    return f"data: {json.dumps(payload, ensure_ascii=False)}\n\n"


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):
        sys.stderr.write("%s - %s\n" % (self.address_string(), fmt % args))

    def _json(self, obj, status=200):
        body = json.dumps(obj, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path.rstrip("/").endswith("/models"):
            self._json(MODELS)
            return
        if self.path.rstrip("/").endswith("/key"):
            self._json({"data": {"label": "mock", "usage": 0, "limit": None, "is_free_tier": True}})
            return
        self._json({"error": {"message": "not found"}}, 404)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length else b"{}"
        try:
            request = json.loads(raw)
        except json.JSONDecodeError:
            request = {}

        if not self.path.rstrip("/").endswith("/chat/completions"):
            self._json({"error": {"message": "not found"}}, 404)
            return

        last = ""
        for message in reversed(request.get("messages", [])):
            if message.get("role") == "user":
                last = message.get("content") or ""
                break
        if isinstance(last, list):
            last = " ".join(p.get("text", "") for p in last if isinstance(p, dict))

        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()

        def write(text: str):
            data = text.encode()
            self.wfile.write(f"{len(data):X}\r\n".encode() + data + b"\r\n")
            self.wfile.flush()

        write(": OPENROUTER PROCESSING\n\n")
        time.sleep(0.15)

        reasoning = [
            "Пользователь просит проверить стриминг. ",
            "Нужно показать reasoning, затем основной текст, ",
            "затем usage со стоимостью и finish_reason.",
        ]
        for part in reasoning:
            write(chunk({"reasoning": part}))
            time.sleep(0.2)

        answer = (
            f"Эхо: {last}\n\n"
            "Этот ответ пришёл по SSE из локального мока. "
            "Он проверяет инкрементальный рендер, перенос строк, "
            "стоимость и кнопку Stop.\n\n"
            "```rust\nfn main() { println!(\"Muninn\"); }\n```\n"
        )
        for i in range(0, len(answer), 7):
            write(chunk({"content": answer[i : i + 7]}))
            time.sleep(0.03)

        write(
            chunk(
                {},
                finish="stop",
                usage={
                    "prompt_tokens": 42,
                    "completion_tokens": 128,
                    "total_tokens": 170,
                    "cost": 0.000123,
                },
            )
        )
        write("data: [DONE]\n\n")
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()


if __name__ == "__main__":
    server = ThreadingHTTPServer(("0.0.0.0", PORT), Handler)
    print(f"mock OpenAI server on http://0.0.0.0:{PORT}/v1", flush=True)
    server.serve_forever()
