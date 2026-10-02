"""Check the extension manifest, download contract, and WASM build together."""

import json
import re
from pathlib import Path

import tomllib

CLIENT = Path(__file__).resolve().parents[1]
ROOT = CLIENT.parents[1]
manifest = tomllib.loads((CLIENT / "extension.toml").read_text())
cargo = tomllib.loads((CLIENT / "Cargo.toml").read_text())
assert manifest["version"] == cargo["package"]["version"]
assert manifest["lib"]["version"] == cargo["dependencies"][
    "zed_extension_api"
].removeprefix("=")
assert cargo["package"]["license"] == "MIT"
assert (CLIENT / "LICENSE").read_text().startswith("MIT License")
version = (CLIENT / "server-version").read_text().strip()
assert re.fullmatch(r"\d+\.\d+\.\d+", version), "Unsafe or non-stable server version"
mapping = manifest["language_servers"]["lspf-analysis"]["language_ids"]
assert set(mapping) == set(manifest["language_servers"]["lspf-analysis"]["languages"])
assert set(mapping.values()) == {
    "cpp",
    "java",
    "javascript",
    "python",
    "rust",
    "typescript",
    "typescriptreact",
}
source = (CLIENT / "src/server.rs").read_text()
for platform in json.loads((ROOT / "scripts/server-platforms.json").read_text()):
    assert f'"{platform["target"]}"' in source, platform
wasm = CLIENT / "target/wasm32-wasip2/release/lspf_analysis_zed.wasm"
assert wasm.read_bytes()[:8] == b"\x00asm\x0d\x00\x01\x00", "Expected a WASI component"
print(
    f"PASS: extension {manifest['version']}, server {version}, languages, six targets, WASI component"
)
