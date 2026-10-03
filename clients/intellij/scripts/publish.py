"""Publish a verified bundle; resume uploads without replacing release content."""

import argparse
import json
import os
import re
import tempfile
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path

from release import (
    PLUGIN_ID,
    git,
    inspect_archive,
    release_tag,
    sha256,
    target_config,
    validate_tag,
    verify_bundle,
    version,
)


class Redirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if urllib.parse.urlsplit(newurl).scheme != "https":
            raise ValueError("Refusing a non-HTTPS redirect")
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        if (
            urllib.parse.urlsplit(req.full_url).netloc
            != urllib.parse.urlsplit(newurl).netloc
        ):
            redirected.remove_header("Authorization")
        return redirected


def request(
    url,
    token=None,
    data=None,
    content_type=None,
    method=None,
    missing_ok=False,
    accept="application/json",
):
    headers = {"Accept": accept, "User-Agent": "lspf-analysis-release"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    if isinstance(data, dict):
        data = json.dumps(data).encode("utf-8")
        content_type = "application/json"
    if content_type:
        headers["Content-Type"] = content_type
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.build_opener(Redirect).open(req, timeout=180) as response:
            return response.read()
    except urllib.error.HTTPError as error:
        if missing_ok and error.code == 404:
            return None
        # Do not echo request headers or credential-bearing responses.
        raise RuntimeError(
            f"{req.get_method()} {urllib.parse.urlsplit(url).path}: HTTP {error.code}"
        ) from None


class GitHub:
    def __init__(self):
        self.repo = os.environ["GITHUB_REPOSITORY"]
        if not re.fullmatch(r"[\w.-]+/[\w.-]+", self.repo):
            raise ValueError("Invalid repository")
        self.token = os.environ["GH_TOKEN"]
        self.base = f"https://api.github.com/repos/{self.repo}"

    def api(self, path, **kwargs):
        value = request(self.base + path, self.token, **kwargs)
        return json.loads(value) if value else None

    def release(self, tag):
        return self.api(
            f"/releases/tags/{urllib.parse.quote(tag, safe='')}", missing_ok=True
        )

    def assets(self, release):
        result = []
        page = 1
        while True:
            items = self.api(
                f"/releases/{release['id']}/assets?per_page=100&page={page}"
            )
            result.extend(items)
            if len(items) < 100:
                return {item["name"]: item for item in result}
            page += 1

    def download(self, asset):
        return request(
            self.base + f"/releases/assets/{asset['id']}",
            self.token,
            accept="application/octet-stream",
        )

    def upload(self, release, path):
        assets = self.assets(release)
        if path.name in assets:
            import hashlib

            if hashlib.sha256(self.download(assets[path.name])).hexdigest() != sha256(
                path
            ):
                raise ValueError(
                    f"Refusing to replace different published content: {path.name}"
                )
            return
        request(
            f"https://uploads.github.com/repos/{self.repo}/releases/{release['id']}/assets?"
            + urllib.parse.urlencode({"name": path.name}),
            self.token,
            path.read_bytes(),
            "application/octet-stream",
        )


def canonical_bundle(github, directory):
    tag = release_tag()
    release = github.release(tag)
    if not release:
        raise ValueError("No saved GitHub release exists")
    assets = github.assets(release)
    manifest_asset = assets.get("manifest.json")
    if not manifest_asset:
        raise ValueError(
            "Incomplete saved release; recover the original signed Actions artifact"
        )
    manifest_bytes = github.download(manifest_asset)
    manifest = json.loads(manifest_bytes)
    filenames = [a["file"] for a in manifest["archives"]] + [
        "manifest.json",
        "SHA256SUMS",
        "release-notes.md",
    ]
    if any(
        Path(name).name != name or "/" in name or "\\" in name or ":" in name
        for name in filenames
    ):
        raise ValueError("Unsafe release asset name")
    directory.mkdir(parents=True, exist_ok=True)
    for name in filenames:
        if name not in assets:
            raise ValueError(
                f"Missing saved asset {name}; recover the original signed Actions artifact"
            )
        (directory / name).write_bytes(
            manifest_bytes if name == "manifest.json" else github.download(assets[name])
        )
    verify_bundle(directory)
    return release


def probe(github):
    """Prefer an immutable artifact from this run, then the durable GitHub copy."""
    reuse = "none"
    run_id = os.environ["GITHUB_RUN_ID"]
    artifacts = github.api(f"/actions/runs/{run_id}/artifacts?per_page=100")[
        "artifacts"
    ]
    signed = [
        item
        for item in artifacts
        if item["name"] == "intellij-signed" and not item["expired"]
    ]
    if signed:
        reuse = "artifact"
    elif github.release(release_tag()):
        with tempfile.TemporaryDirectory() as temp:
            canonical_bundle(github, Path(temp))
        reuse = "github"
    original_tag = f"intellij-v{version()}"
    if reuse == "none" and release_tag() != original_tag:
        original = github.release(original_tag)
        if (
            not original
            or original.get("draft", True)
            or original.get("prerelease") != ("-rc." in version())
        ):
            raise ValueError("A rebuild requires the original published release")
    elif reuse == "none" and version() == "0.1.0":
        # The first public version must follow a published RC rehearsal.
        page = 1
        while True:
            releases = github.api(f"/releases?per_page=100&page={page}")
            if any(
                not item["draft"]
                and item["prerelease"]
                and re.fullmatch(r"intellij-v0\.1\.0-rc\.[1-9]\d*", item["tag_name"])
                for item in releases
            ):
                break
            if len(releases) < 100:
                raise ValueError(
                    "Publish and validate a 0.1.0 RC before the first stable release"
                )
            page += 1
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as stream:
        stream.write(f"reuse={reuse}\n")
    print(f"Saved release bundle: {reuse}")


def summary(text):
    print(text)
    if path := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(path, "a", encoding="utf-8") as stream:
            stream.write(text + "\n\n")


def publish_github(github, directory):
    data = verify_bundle(directory)
    release = github.release(data["tag"])
    if not release:
        release = github.api(
            "/releases",
            data={
                "tag_name": data["tag"],
                "target_commitish": data["commit"],
                "name": f"IntelliJ client {data['version']}",
                "draft": True,
                "prerelease": "-rc." in data["version"],
                "body": (directory / "release-notes.md").read_text(encoding="utf-8")
                + f"\n\nSource: `{data['commit']}`. Server: `{data['serverVersion']}`.\n\n"
                + (
                    "RC: GitHub only.\n"
                    if "-rc." in data["version"]
                    else "Marketplace submission and review are tracked separately in the publishing run. "
                    "GitHub availability does not imply Marketplace approval.\n"
                ),
            },
        )
    # Upload checks every existing file, including the manifest. Never clobber.
    for name in [a["file"] for a in data["archives"]] + [
        "manifest.json",
        "SHA256SUMS",
        "release-notes.md",
    ]:
        github.upload(release, directory / name)
    if release["draft"]:
        github.api(f"/releases/{release['id']}", method="PATCH", data={"draft": False})
    summary(
        f"GitHub published: https://github.com/{github.repo}/releases/tag/{data['tag']}"
    )


def multipart(archive):
    boundary = "lspf-" + uuid.uuid4().hex
    chunks = []
    for key, value in (("xmlId", PLUGIN_ID), ("channel", "")):
        chunks.append(
            f'--{boundary}\r\nContent-Disposition: form-data; name="{key}"\r\n\r\n{value}\r\n'.encode()
        )
    chunks.append(
        f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{archive.name}"\r\nContent-Type: application/zip\r\n\r\n'.encode()
    )
    chunks.extend([archive.read_bytes(), f"\r\n--{boundary}--\r\n".encode()])
    return b"".join(chunks), f"multipart/form-data; boundary={boundary}"


def publish_marketplace(github, directory):
    data = verify_bundle(directory)
    if "-rc." in data["version"]:
        raise ValueError("RC versions are never uploaded to Marketplace")
    if os.environ.get("INTELLIJ_MARKETPLACE_READY") != "true":
        if data["version"] != "0.1.0":
            raise ValueError(
                "Marketplace must be configured before subsequent stable releases"
            )
        summary(
            "Marketplace NOT submitted: bootstrap 0.1.0 requires the first manual upload. "
            "Use these exact signed GitHub assets, then set INTELLIJ_MARKETPLACE_READY=true and rerun failed/all jobs."
        )
        return
    token = os.environ["INTELLIJ_PUBLISH_TOKEN"]
    if not token:
        raise ValueError("Marketplace token is missing")
    release = github.release(data["tag"])
    if not release or release["draft"]:
        raise ValueError(
            "Persist the complete signed GitHub release before Marketplace submission"
        )
    for archive in data["archives"]:
        release = github.release(data["tag"])
        body = release.get("body") or ""
        receipts = [
            json.loads(value)
            for value in re.findall(r"<!-- intellij-marketplace:(\{[^\n]*\}) -->", body)
        ]
        receipt = {
            "version": archive["version"],
            "sha256": archive["sha256"],
            "status": "submitted",
        }
        previous = [
            item for item in receipts if item.get("version") == archive["version"]
        ]
        if previous:
            if previous != [receipt]:
                raise ValueError("Marketplace receipt differs from release bundle")
            continue
        query = urllib.parse.urlencode(
            {"pluginId": PLUGIN_ID, "version": archive["version"]}
        )
        existing = request(
            f"https://plugins.jetbrains.com/plugin/download?{query}",
            token,
            missing_ok=True,
            accept="application/octet-stream",
        )
        if existing is not None:
            # Marketplace adds its own ZIP signature. Compare the complete
            # payload, including descriptor and executable, not the envelope.
            with tempfile.TemporaryDirectory(prefix="intellij-existing-") as temp:
                downloaded = Path(temp) / archive["file"]
                downloaded.write_bytes(existing)
                actual = inspect_archive(
                    downloaded, target_config(archive["target"]), data["version"]
                )
            if actual["payloadSha256"] != archive["payloadSha256"]:
                raise ValueError(
                    f"Marketplace already has different contents for {archive['version']}"
                )
        else:
            upload_body, content_type = multipart(directory / archive["file"])
            request(
                "https://plugins.jetbrains.com/api/updates/upload",
                token,
                upload_body,
                content_type,
            )
        # Notes can be updated even when GitHub's immutable releases are enabled.
        # No release asset is added or changed after the draft becomes public.
        github.api(
            f"/releases/{release['id']}",
            method="PATCH",
            data={
                "body": body
                + "\n<!-- intellij-marketplace:"
                + json.dumps(receipt, separators=(",", ":"))
                + " -->\n",
            },
        )
    summary(
        "Marketplace submitted: all six variants. Review/availability is pending; "
        "check https://plugins.jetbrains.com/author/me . This is not an approval result."
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "command", choices=["probe", "download", "github", "marketplace"]
    )
    parser.add_argument(
        "--directory", type=Path, default=Path("clients/intellij/build/signed")
    )
    args = parser.parse_args()
    # Every privileged invocation checks the immutable tag and ancestry again.
    validate_tag(release_tag(), version(), git("rev-parse", "HEAD"))
    github = GitHub()
    if args.command == "probe":
        probe(github)
    elif args.command == "download":
        canonical_bundle(github, args.directory)
    elif args.command == "github":
        publish_github(github, args.directory)
    else:
        publish_marketplace(github, args.directory)


if __name__ == "__main__":
    main()
