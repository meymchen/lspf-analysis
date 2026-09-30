"""Exercise the actual packaged server over stdio, including a document and hover."""

import json
import os
import queue
import subprocess
import threading
import time
from pathlib import Path


def smoke_server(binary, timeout=30):
    env = {
        key: value
        for key, value in os.environ.items()
        if key not in {"LSPF_ANALYSIS_LOG_FILE", "LSPF_ANALYSIS_DEBUG_PORT"}
    }
    env["RUST_LOG"] = "info"
    process = subprocess.Popen(
        [str(Path(binary).resolve()), "serve", "--stdio"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    messages = queue.Queue()
    logs = bytearray()

    def reader():
        try:
            while True:
                headers = {}
                while line := process.stdout.readline():
                    if line == b"\r\n":
                        break
                    key, value = line.decode("ascii").strip().split(":", 1)
                    headers[key.lower()] = value.strip()
                if not line:
                    raise EOFError("Server stdout closed")
                length = int(headers["content-length"])
                if not 0 < length <= 8 * 1024 * 1024:
                    raise ValueError("Invalid LSP Content-Length")
                body = process.stdout.read(length)
                if len(body) != length:
                    raise EOFError("Incomplete LSP frame")
                messages.put(json.loads(body))
        except (OSError, EOFError, ValueError, KeyError) as error:
            messages.put(error)

    def stderr():
        while chunk := process.stderr.read(1024):
            logs.extend(chunk)
            del logs[:-65536]

    threading.Thread(target=reader, daemon=True).start()
    threading.Thread(target=stderr, daemon=True).start()
    deadline = time.monotonic() + timeout

    def send(method, params=None, request_id=None):
        message = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            message["params"] = params
        if request_id is not None:
            message["id"] = request_id
        payload = json.dumps(message).encode("utf-8")
        process.stdin.write(
            f"Content-Length: {len(payload)}\r\n\r\n".encode("ascii") + payload
        )
        process.stdin.flush()

    def receive(predicate):
        while True:
            message = messages.get(timeout=max(0.01, deadline - time.monotonic()))
            if isinstance(message, Exception):
                raise message
            if "method" in message and "id" in message:
                raise ValueError(f"Unexpected server request: {message['method']}")
            if predicate(message):
                if "error" in message:
                    raise ValueError(f"LSP error: {message['error']}")
                return message

    try:
        send(
            "initialize",
            {
                "processId": os.getpid(),
                "rootUri": None,
                "capabilities": {"general": {"positionEncodings": ["utf-16"]}},
                "initializationOptions": {"lspfAnalysis": {"locale": "en"}},
            },
            1,
        )
        result = receive(lambda m: m.get("id") == 1)["result"]
        if not isinstance(result.get("capabilities"), dict):
            raise TypeError("Missing server capabilities")
        send("initialized", {})
        uri = (Path(binary).resolve().parent / "release-smoke.rs").as_uri()
        send(
            "textDocument/didOpen",
            {
                "textDocument": {
                    "uri": uri,
                    "languageId": "rust",
                    "version": 1,
                    "text": "fn wide(a: u32, b: u32, c: u32, d: u32, e: u32) -> u32 { a + b + c + d + e }\n",
                }
            },
        )
        health = receive(lambda m: m.get("method") == "lspfAnalysis/fileHealth")[
            "params"
        ]
        if health.get("uri") != uri or health.get("functions") != 1:
            raise ValueError(f"Invalid document health: {health}")
        send(
            "textDocument/hover",
            {"textDocument": {"uri": uri}, "position": {"line": 0, "character": 4}},
            2,
        )
        hover = receive(lambda m: m.get("id") == 2)["result"]
        if not hover or "wide" not in json.dumps(hover):
            raise ValueError("Packaged server did not return a function hover")
        send("shutdown", request_id=3)
        if receive(lambda m: m.get("id") == 3)["result"] is not None:
            raise ValueError("Invalid shutdown response")
        send("exit")
        if process.wait(timeout=max(0.01, deadline - time.monotonic())) != 0:
            raise ValueError("Server exited unsuccessfully")
    except Exception as error:
        raise RuntimeError(
            f"Packaged LSP smoke failed: {error}\n{logs.decode('utf-8', errors='replace')}"
        ) from error
    finally:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=5)
        for stream in (process.stdin, process.stdout, process.stderr):
            stream.close()
