"""Test Zed's standard LSP journey against a real server, without custom requests."""

import argparse
import json
import os
import queue
import subprocess
import threading
from pathlib import Path

SAMPLES = [
    ("cpp", "cpp", "int wide(int a, int b, int c, int d, int e) { return a+b+c+d+e; }"),
    (
        "java",
        "java",
        "class Sample { int wide(int a, int b, int c, int d, int e) { return a+b+c+d+e; } }",
    ),
    ("javascript", "js", "function wide(a, b, c, d, e) { return a+b+c+d+e; }"),
    (
        "javascript",
        "jsx",
        "function wide(a, b, c, d, e) { return <div>{a+b+c+d+e}</div>; }",
    ),
    ("python", "py", "def wide(a, b, c, d, e):\n    return a+b+c+d+e\n"),
    (
        "rust",
        "rs",
        "fn wide(a: u32, b: u32, c: u32, d: u32, e: u32) -> u32 { a+b+c+d+e }",
    ),
    (
        "typescript",
        "ts",
        "function wide(a: number, b: number, c: number, d: number, e: number) { return a+b+c+d+e; }",
    ),
    (
        "typescriptreact",
        "tsx",
        "function wide(a: number, b: number, c: number, d: number, e: number) { return <div>{a+b+c+d+e}</div>; }",
    ),
]


def smoke(binary):
    environment = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith("LSPF_ANALYSIS_")
    }
    process = subprocess.Popen(
        [str(binary.resolve()), "serve", "--stdio"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=environment,
    )
    messages = queue.Queue()
    logs = bytearray()

    def read_messages():
        try:
            while True:
                headers = {}
                while (line := process.stdout.readline()) not in (b"\r\n", b""):
                    key, value = line.decode("ascii").split(":", 1)
                    headers[key.lower()] = value.strip()
                if not line:
                    raise EOFError("Server stdout closed")
                size = int(headers["content-length"])
                if not 0 < size <= 8 * 1024 * 1024:
                    raise ValueError("Invalid LSP frame length")
                payload = process.stdout.read(size)
                if len(payload) != size:
                    raise EOFError("Incomplete LSP frame")
                messages.put(json.loads(payload))
        except (OSError, EOFError, ValueError, KeyError) as error:
            messages.put(error)

    def read_logs():
        while chunk := process.stderr.read(1024):
            logs.extend(chunk)
            del logs[:-65536]

    reader = threading.Thread(target=read_messages, daemon=True)
    logger = threading.Thread(target=read_logs, daemon=True)
    reader.start()
    logger.start()

    def send(method, params=None, request_id=None):
        message = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            message["params"] = params
        if request_id is not None:
            message["id"] = request_id
        body = json.dumps(message).encode("utf-8")
        process.stdin.write(
            f"Content-Length: {len(body)}\r\n\r\n".encode("ascii") + body
        )
        process.stdin.flush()

    def receive(predicate):
        while True:
            message = messages.get(timeout=15)
            if isinstance(message, Exception):
                raise message
            if predicate(message):
                if "error" in message:
                    raise ValueError(message["error"])
                return message

    def diagnostics(uri, nonempty):
        return receive(
            lambda m: (
                m.get("method") == "textDocument/publishDiagnostics"
                and m["params"]["uri"] == uri
                and bool(m["params"]["diagnostics"]) == nonempty
            )
        )

    settings = {
        "lspfAnalysis": {
            "health": {"qualityWarn": 100, "qualityError": 0},
            "locale": "en",
        }
    }
    try:
        send(
            "initialize",
            {
                "processId": os.getpid(),
                "rootUri": None,
                "capabilities": {
                    "textDocument": {"hover": {"contentFormat": ["markdown"]}}
                },
                "initializationOptions": settings,
            },
            1,
        )
        result = receive(lambda m: m.get("id") == 1)["result"]
        assert result["capabilities"]["hoverProvider"]
        send("initialized", {})
        # Zed sends this immediately after initialization, and on settings edits.
        send("workspace/didChangeConfiguration", {"settings": settings})
        for index, (language_id, suffix, source) in enumerate(SAMPLES):
            uri = (binary.resolve().parent / f"zed-smoke.{suffix}").as_uri()
            send(
                "textDocument/didOpen",
                {
                    "textDocument": {
                        "uri": uri,
                        "languageId": language_id,
                        "version": 1,
                        "text": source,
                    }
                },
            )
            diagnostics(uri, True)
            request_id = 10 + index
            send(
                "textDocument/hover",
                {
                    "textDocument": {"uri": uri},
                    "position": {"line": 0, "character": source.index("wide")},
                },
                request_id,
            )
            hover = receive(lambda m, expected=request_id: m.get("id") == expected)[
                "result"
            ]
            assert hover and "wide" in json.dumps(hover), (language_id, hover)
            assert "<span" not in json.dumps(hover), (
                "Undeclared HTML must not reach Zed"
            )
            send("textDocument/didClose", {"textDocument": {"uri": uri}})
            diagnostics(uri, False)

        uri = (binary.resolve().parent / "zed-config-smoke.rs").as_uri()
        send(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": SAMPLES[5][2],
                }
            },
        )
        diagnostics(uri, True)
        settings["lspfAnalysis"]["locale"] = "zh-cn"
        send("workspace/didChangeConfiguration", {"settings": settings})
        diagnostics(uri, True)
        send(
            "textDocument/hover",
            {"textDocument": {"uri": uri}, "position": {"line": 0, "character": 3}},
            30,
        )
        hover = receive(lambda m: m.get("id") == 30)["result"]
        assert any(
            "\u4e00" <= char <= "\u9fff"
            for char in json.dumps(hover, ensure_ascii=False)
        )
        settings["lspfAnalysis"]["diagnostics"] = {"enabled": False}
        send("workspace/didChangeConfiguration", {"settings": settings})
        diagnostics(uri, False)
        send("shutdown", request_id=99)
        assert receive(lambda m: m.get("id") == 99)["result"] is None
        send("exit")
        assert process.wait(timeout=10) == 0
        print(
            "PASS: 8 language IDs/file types, diagnostics, Markdown hover, live settings, Chinese, shutdown"
        )
    except Exception as error:
        raise RuntimeError(
            f"LSP smoke failed: {error}\n{logs.decode('utf-8', errors='replace')}"
        ) from error
    finally:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=5)
        reader.join(timeout=5)
        logger.join(timeout=5)
        for stream in (process.stdin, process.stdout, process.stderr):
            stream.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    smoke(parser.parse_args().binary)
