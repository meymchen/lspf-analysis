"""Stage standalone server assets; publishing requires a matching pushed tag."""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[1]
DIST = ROOT / "target" / "server-dist"
PLATFORMS = json.loads((ROOT / "scripts/server-platforms.json").read_text())


def version():
    value = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"][
        "version"
    ]
    if not re.fullmatch(r"\d+\.\d+\.\d+", value):
        raise ValueError("Standalone releases require a stable X.Y.Z version")
    return value


def publish_allowed(event, ref, release_version):
    if event != "push" or not ref.startswith("refs/tags/server-v"):
        return False
    if ref != f"refs/tags/server-v{release_version}":
        raise ValueError("Server tag does not match workspace.package.version")
    return True


def asset_name(target, release_version):
    if target not in {item["target"] for item in PLATFORMS}:
        raise ValueError(f"Unsupported release target: {target}")
    suffix = ".exe" if target.endswith("windows-msvc") else ""
    return f"lspf-analysis-{release_version}-{target}{suffix}"


def metadata():
    release_version = version()
    publish = publish_allowed(
        os.environ.get("GITHUB_EVENT_NAME", ""),
        os.environ.get("GITHUB_REF", ""),
        release_version,
    )
    if publish:
        # A tag from an unrelated branch must not publish an unreviewed server.
        subprocess.run(
            ["git", "merge-base", "--is-ancestor", "HEAD", "origin/main"],
            cwd=ROOT,
            check=True,
        )
    values = {
        "version": release_version,
        "publish": str(publish).lower(),
        "platforms": json.dumps({"include": PLATFORMS}, separators=(",", ":")),
    }
    lines = "".join(f"{key}={value}\n" for key, value in values.items())
    print(lines, end="")
    if output := os.environ.get("GITHUB_OUTPUT"):
        with Path(output).open("a", encoding="utf-8") as stream:
            stream.write(lines)


def stage(target):
    name = asset_name(target, version())
    executable = (
        "lspf-analysis.exe" if target.endswith("windows-msvc") else "lspf-analysis"
    )
    source = ROOT / "target" / target / "release" / executable
    if not source.is_file() or source.stat().st_size == 0:
        raise ValueError(f"Missing server executable: {source}")
    DIST.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, DIST / name)
    print(DIST / name)


def verify_assets(directory, release_version):
    expected = {asset_name(item["target"], release_version) for item in PLATFORMS}
    actual = {path.name for path in directory.glob("lspf-analysis-*")}
    if actual != expected:
        raise ValueError(
            f"Release assets mismatch: missing={expected - actual}, extra={actual - expected}"
        )
    paths = [directory / name for name in sorted(expected)]
    if any(not path.is_file() or path.stat().st_size == 0 for path in paths):
        raise ValueError("A release executable is missing or empty")
    return paths


def checksums():
    paths = verify_assets(DIST, version())
    shutil.copy2(ROOT / "LICENSE", DIST / "LICENSE")
    commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    (DIST / "SOURCE.txt").write_text(
        f"Source and build scripts: https://github.com/meymchen/lspf-analysis/tree/{commit}\n"
        "The language server is licensed under MPL-2.0. See LICENSE.\n",
        encoding="utf-8",
    )
    paths.extend([DIST / "LICENSE", DIST / "SOURCE.txt"])
    lines = []
    for path in paths:
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        lines.append(f"{digest}  {path.name}\n")
    (DIST / "SHA256SUMS").write_text("".join(lines), encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("metadata")
    staging = commands.add_parser("stage")
    staging.add_argument("--target", required=True)
    commands.add_parser("checksums")
    args = parser.parse_args()
    if args.command == "metadata":
        metadata()
    elif args.command == "stage":
        stage(args.target)
    else:
        checksums()
