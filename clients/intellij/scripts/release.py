"""Validate and inspect IntelliJ release artifacts using Python's standard library."""

import argparse
import hashlib
import io
import json
import os
import platform
import re
import shutil
import stat
import struct
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile
from fnmatch import fnmatchcase
from pathlib import Path, PurePosixPath

import tomllib

CLIENT = Path(__file__).resolve().parents[1]
ROOT = CLIENT.parent.parent
PLUGIN_ID = "com.github.meymchen.lspfanalysis"
VERSION_PATTERN = r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-rc\.[1-9]\d*)?"


def config():
    return json.loads((CLIENT / "release.json").read_text(encoding="utf-8"))


def version():
    value = re.search(
        r"(?m)^version\s*=\s*(\S+)\s*$", (CLIENT / "gradle.properties").read_text()
    )[1]
    if not re.fullmatch(VERSION_PATTERN, value):
        raise ValueError(f"Invalid release version: {value}")
    return value


def target_config(target):
    return next(item for item in config()["platforms"] if item["target"] == target)


def executable(item):
    return (
        "lspf-analysis.exe"
        if item["variant"].startswith("windows-")
        else "lspf-analysis"
    )


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def notes(release_version):
    text = (CLIENT / "CHANGELOG.md").read_text(encoding="utf-8")
    match = re.search(
        rf"(?m)^## {re.escape(release_version)}\s*\n([\s\S]*?)(?=^## |\Z)", text
    )
    if not match or not match[1].strip() or "unreleased" in match[1].lower():
        raise ValueError(f"CHANGELOG.md needs a completed entry for {release_version}")
    return match[1].strip()


def validate_tag(tag, release_version, commit):
    if tag != f"intellij-v{release_version}":
        raise ValueError("Tag does not match gradle.properties")
    if git("rev-parse", f"refs/tags/{tag}^{{commit}}") != commit:
        raise ValueError("Tag no longer points to the release commit")
    subprocess.run(
        ["git", "merge-base", "--is-ancestor", commit, "origin/main"],
        cwd=ROOT,
        check=True,
    )


def verification_ides(data):
    """Use representative IDEs for ordinary PRs and the full matrix otherwise."""
    full = True
    if os.environ.get("GITHUB_EVENT_NAME") == "pull_request":
        event = json.loads(
            Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8")
        )
        pr = event["pull_request"]
        changed = git(
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            f"{pr['base']['sha']}...{pr['head']['sha']}",
            "--",
        ).split("\0")
        compatibility_paths = (
            ".github/workflows/release-intellij.yml",
            "clients/intellij/release.json",
            "clients/intellij/*.gradle.kts",
            "clients/intellij/gradle.properties",
            "clients/intellij/gradle/*",
            "clients/intellij/gradlew*",
            "clients/intellij/src/main/resources/META-INF/*.xml",
            "clients/intellij/scripts/*",
        )
        full = any(
            fnmatchcase(path, pattern)
            for path in changed
            for pattern in compatibility_paths
        )
    if full:
        return [
            {"type": ide["type"], "version": value}
            for ide in data["ides"]
            for value in ide["versions"]
        ]

    versions = {
        ide["type"]: sorted(
            set(ide["versions"]), key=lambda value: tuple(map(int, value.split(".")))
        )
        for ide in data["ides"]
    }
    return [
        {"type": "IntellijIdea", "version": value}
        for value in dict.fromkeys(
            (versions["IntellijIdea"][0], versions["IntellijIdea"][-1])
        )
    ] + [{"type": "PyCharm", "version": versions["PyCharm"][-1]}]


def metadata():
    release_version = version()
    commit = git("rev-parse", "HEAD")
    publish = (
        os.environ.get("GITHUB_EVENT_NAME") == "push"
        and os.environ.get("GITHUB_REF_TYPE") == "tag"
    )
    if publish:
        validate_tag(os.environ["GITHUB_REF_NAME"], release_version, commit)
    notes(release_version)
    data = config()
    result = {
        "version": release_version,
        "commit": commit,
        "publish": str(publish).lower(),
        "prerelease": str("-rc." in release_version).lower(),
        "platforms": {"include": data["platforms"]},
        "ides": {"include": verification_ides(data)},
    }
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as stream:
            stream.writelines(
                f"{key}={json.dumps(value, separators=(',', ':')) if isinstance(value, dict) else value}\n"
                for key, value in result.items()
            )
    print(json.dumps(result, indent=2))


def binary_architecture(data, variant):
    """Reject accidental cross-target/stale binaries before making a ZIP."""
    os_name, arch = variant.split("-")
    if os_name == "windows":
        if data[:2] != b"MZ" or len(data) < 64:
            raise ValueError("Expected a PE executable")
        offset = struct.unpack_from("<I", data, 60)[0]
        if data[offset : offset + 4] != b"PE\0\0":
            raise ValueError("Invalid PE header")
        machine = struct.unpack_from("<H", data, offset + 4)[0]
        expected = 0x8664 if arch == "x86_64" else 0xAA64
    elif os_name == "linux":
        if data[:6] != b"\x7fELF\x02\x01":
            raise ValueError("Expected a 64-bit little-endian ELF executable")
        machine = struct.unpack_from("<H", data, 18)[0]
        expected = 62 if arch == "x86_64" else 183
    else:
        if data[:4] != b"\xcf\xfa\xed\xfe":
            raise ValueError("Expected a 64-bit Mach-O executable")
        machine = struct.unpack_from("<I", data, 4)[0]
        expected = 0x01000007 if arch == "x86_64" else 0x0100000C
    if machine != expected:
        raise ValueError(f"Executable architecture does not match {variant}")


def stage(target):
    item = target_config(target)
    source = ROOT / "target" / item["rust"] / "release" / executable(item)
    binary_architecture(source.read_bytes(), item["variant"])
    destination = CLIENT / "build/native" / item["variant"] / "server" / source.name
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)
    destination.chmod(0o755)


def safe_members(archive):
    names = set()
    total = 0
    for entry in archive.infolist():
        name = entry.orig_filename
        path = PurePosixPath(name)
        if (
            name in names
            or "\\" in name
            or ":" in name
            or path.is_absolute()
            or ".." in path.parts
            or not path.parts
            or stat.S_ISLNK(entry.external_attr >> 16)
        ):
            raise ValueError(f"Unsafe or duplicate archive member: {name}")
        names.add(name)
        total += entry.file_size
        if total > 1024 * 1024 * 1024:
            raise ValueError("Unpacked archive exceeds 1 GiB")
    return archive.infolist()


def extract(archive_path, destination):
    destination = Path(destination).resolve()
    with zipfile.ZipFile(archive_path) as archive:
        for entry in safe_members(archive):
            path = destination.joinpath(*PurePosixPath(entry.filename).parts)
            if not path.resolve().is_relative_to(destination):
                raise ValueError("Archive escapes extraction directory")
            if entry.is_dir():
                path.mkdir(parents=True, exist_ok=True)
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                with archive.open(entry) as source, path.open("xb") as output:
                    shutil.copyfileobj(source, output)
                path.chmod((entry.external_attr >> 16) & 0o777 or 0o644)


def inspect_archive(path, item, release_version):
    with zipfile.ZipFile(path) as archive:
        entries = safe_members(archive)
        payload = hashlib.sha256()
        jvm_payload = hashlib.sha256()
        for entry in sorted(entries, key=lambda e: e.filename):
            if not entry.is_dir():
                payload.update(entry.filename.encode() + b"\0")
                payload.update(struct.pack("<I", (entry.external_attr >> 16) & 0o777))
                payload.update(hashlib.sha256(archive.read(entry)).digest())
        roots = {PurePosixPath(entry.filename).parts[0] for entry in entries}
        if len(roots) != 1:
            raise ValueError("Plugin ZIP must have exactly one root directory")
        root = roots.pop()
        binary_name = f"{root}/server/{executable(item)}"
        binary = archive.read(binary_name)
        binary_architecture(binary, item["variant"])
        if (
            not item["variant"].startswith("windows-")
            and not (archive.getinfo(binary_name).external_attr >> 16) & 0o111
        ):
            raise ValueError("Packaged server is not executable")
        servers = [
            e.filename for e in entries if "/server/" in e.filename and not e.is_dir()
        ]
        if servers != [binary_name]:
            raise ValueError("Plugin must contain exactly one matching server")
        descriptors = []
        for entry in sorted(entries, key=lambda e: e.filename):
            if re.fullmatch(re.escape(root) + r"/lib/[^/]+\.jar", entry.filename):
                with zipfile.ZipFile(io.BytesIO(archive.read(entry))) as jar:
                    for member in sorted(safe_members(jar), key=lambda e: e.filename):
                        if (
                            not member.is_dir()
                            and member.filename != "META-INF/plugin.xml"
                        ):
                            jvm_payload.update(
                                entry.filename.encode()
                                + b"\0"
                                + member.filename.encode()
                                + b"\0"
                            )
                            jvm_payload.update(
                                hashlib.sha256(jar.read(member)).digest()
                            )
                    if "META-INF/plugin.xml" in jar.namelist():
                        descriptors.append(
                            ET.fromstring(jar.read("META-INF/plugin.xml"))
                        )
        descriptors = [d for d in descriptors if d.findtext("id") == PLUGIN_ID]
        if len(descriptors) != 1:
            raise ValueError("Expected one LSPF Analysis plugin descriptor")
        descriptor = descriptors[0]
        variant_version = f"{release_version}-{item['variant']}"
        if descriptor.findtext("version") != variant_version:
            raise ValueError("Plugin descriptor version does not match release/variant")
        os_name, arch = item["variant"].split("-")
        dependencies = {d.text for d in descriptor.findall("depends")}
        required = {
            "com.intellij.modules.lsp",
            f"com.intellij.modules.os.{os_name}",
            f"com.intellij.modules.arch.{arch}",
        }
        if not required.issubset(dependencies):
            raise ValueError("Missing LSP, OS or architecture dependency")
        bounds = descriptor.find("idea-version")
        if (
            bounds is None
            or bounds.get("since-build") != config()["sinceBuild"]
            or bounds.get("until-build") != config()["untilBuild"]
        ):
            raise ValueError("IDE compatibility range differs from release.json")
    return {
        "target": item["target"],
        "variant": item["variant"],
        "version": variant_version,
        "file": Path(path).name,
        "sha256": sha256(path),
        "serverSha256": hashlib.sha256(binary).hexdigest(),
        "payloadSha256": payload.hexdigest(),
        "jvmSha256": jvm_payload.hexdigest(),
    }


def find_archive(directory, item):
    paths = list(Path(directory).glob(f"*-{item['variant']}.zip"))
    if len(paths) != 1:
        raise ValueError(
            f"Expected exactly one ZIP for {item['variant']}, got {len(paths)}"
        )
    return paths[0]


def manifest(directory):
    directory = Path(directory)
    archives = [
        inspect_archive(find_archive(directory, item), item, version())
        for item in config()["platforms"]
    ]
    if len(list(directory.glob("*.zip"))) != len(archives):
        raise ValueError("Unexpected ZIPs in release bundle")
    if len({item["jvmSha256"] for item in archives}) != 1:
        raise ValueError("Native variants must contain the same JVM code and resources")
    with (ROOT / "Cargo.toml").open("rb") as stream:
        server_version = tomllib.load(stream)["workspace"]["package"]["version"]
    result = {
        "schema": 1,
        "pluginId": PLUGIN_ID,
        "version": version(),
        "tag": f"intellij-v{version()}",
        "commit": git("rev-parse", "HEAD"),
        "serverVersion": server_version,
        "compatibility": config(),
        "archives": archives,
    }
    (directory / "manifest.json").write_text(
        json.dumps(result, indent=2) + "\n", encoding="utf-8"
    )
    (directory / "SHA256SUMS").write_text(
        "".join(f"{a['sha256']}  {a['file']}\n" for a in archives)
        + f"{sha256(directory / 'manifest.json')}  manifest.json\n",
        encoding="utf-8",
    )
    (directory / "release-notes.md").write_text(
        notes(version()) + "\n", encoding="utf-8"
    )


def verify_bundle(directory):
    directory = Path(directory)
    data = json.loads((directory / "manifest.json").read_text(encoding="utf-8"))
    if (
        data["schema"] != 1
        or data["version"] != version()
        or data["commit"] != git("rev-parse", "HEAD")
    ):
        raise ValueError("Release bundle does not belong to this source commit/version")
    if (
        data["pluginId"] != PLUGIN_ID
        or data["tag"] != f"intellij-v{version()}"
        or data["compatibility"] != config()
    ):
        raise ValueError("Release manifest identity/compatibility mismatch")
    expected = config()["platforms"]
    if len(data["archives"]) != len(expected):
        raise ValueError("Incomplete release bundle")
    for item, recorded in zip(expected, data["archives"], strict=True):
        actual = inspect_archive(find_archive(directory, item), item, data["version"])
        if actual != recorded:
            raise ValueError(f"Changed release archive: {item['variant']}")
    sums = "".join(f"{a['sha256']}  {a['file']}\n" for a in data["archives"])
    sums += f"{sha256(directory / 'manifest.json')}  manifest.json\n"
    if (directory / "SHA256SUMS").read_text(encoding="utf-8") != sums:
        raise ValueError("Checksum manifest mismatch")
    with (ROOT / "Cargo.toml").open("rb") as stream:
        if (
            data["serverVersion"]
            != tomllib.load(stream)["workspace"]["package"]["version"]
        ):
            raise ValueError("Server version mismatch")
    if (directory / "release-notes.md").read_text(encoding="utf-8") != notes(
        version()
    ) + "\n":
        raise ValueError("Release notes differ from the tagged changelog")
    return data


def smoke(directory, target):
    from lsp_smoke import smoke_server

    item = target_config(target)
    host_os = {"Windows": "windows", "Darwin": "mac", "Linux": "linux"}[
        platform.system()
    ]
    host_arch = {
        "AMD64": "x86_64",
        "x86_64": "x86_64",
        "aarch64": "arm64",
        "arm64": "arm64",
        "ARM64": "arm64",
    }[platform.machine()]
    if item["variant"] != f"{host_os}-{host_arch}":
        raise ValueError("Smoke test must run on the native target, without emulation")
    archive = find_archive(directory, item)
    inspect_archive(archive, item, version())
    with tempfile.TemporaryDirectory(prefix="lspf-package-") as temp:
        extract(archive, temp)
        candidates = list(Path(temp).glob(f"*/server/{executable(item)}"))
        smoke_server(candidates[0])
    print(f"Packaged LSP smoke passed: {target}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command",
        choices=[
            "metadata",
            "stage",
            "inspect",
            "manifest",
            "verify",
            "smoke",
            "install-smoke",
        ],
    )
    parser.add_argument(
        "--directory", type=Path, default=CLIENT / "build/distributions"
    )
    parser.add_argument(
        "--target", choices=[p["target"] for p in config()["platforms"]]
    )
    args = parser.parse_args()
    if args.command == "metadata":
        metadata()
    elif args.command == "stage":
        stage(args.target)
    elif args.command == "manifest":
        manifest(args.directory)
    elif args.command == "verify":
        verify_bundle(args.directory)
    elif args.command == "smoke":
        smoke(args.directory, args.target)
    elif args.command == "install-smoke":
        item = target_config(args.target)
        archive = find_archive(args.directory, item)
        inspect_archive(archive, item, version())
        extract(archive, CLIENT / "build/release-smoke-plugin")
    else:
        jvm_hashes = set()
        for item in config()["platforms"]:
            inspected = inspect_archive(
                find_archive(args.directory, item), item, version()
            )
            jvm_hashes.add(inspected["jvmSha256"])
            print(json.dumps(inspected))
        if len(jvm_hashes) != 1:
            raise ValueError("Native variants contain different JVM code or resources")


if __name__ == "__main__":
    main()
