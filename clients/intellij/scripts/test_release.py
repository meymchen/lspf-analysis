import hashlib
import io
import json
import os
import stat
import struct
import tempfile
import unittest
import urllib.request
import zipfile
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import Mock, patch

import publish
import release


def binary(variant):
    os_name, architecture = variant.split("-")
    data = bytearray(128)
    if os_name == "windows":
        data[:2] = b"MZ"
        struct.pack_into("<I", data, 60, 64)
        data[64:68] = b"PE\0\0"
        struct.pack_into("<H", data, 68, 0x8664 if architecture == "x86_64" else 0xAA64)
    elif os_name == "linux":
        data[:6] = b"\x7fELF\x02\x01"
        struct.pack_into("<H", data, 18, 62 if architecture == "x86_64" else 183)
    else:
        data[:4] = b"\xcf\xfa\xed\xfe"
        struct.pack_into(
            "<I", data, 4, 0x01000007 if architecture == "x86_64" else 0x0100000C
        )
    return bytes(data)


def archive(
    directory,
    item,
    *,
    wrong_binary=False,
    executable=True,
    wrong_version=False,
    code=b"shared JVM code",
):
    variant = item["variant"]
    release_version = release.version()
    name = f"lspf-analysis-{release_version}-{variant}.zip"
    path = directory / name
    os_name, architecture = variant.split("-")
    descriptor = f"""<idea-plugin><id>{release.PLUGIN_ID}</id>
      <version>{release_version if wrong_version else release_version + "-" + variant}</version>
      <idea-version since-build="261.26222" until-build="262.*"/>
      <depends>com.intellij.modules.lsp</depends>
      <depends>com.intellij.modules.os.{os_name}</depends>
      <depends>com.intellij.modules.arch.{architecture}</depends></idea-plugin>"""
    jar = io.BytesIO()
    with zipfile.ZipFile(jar, "w") as contents:
        contents.writestr("META-INF/plugin.xml", descriptor)
        contents.writestr("plugin.class", code)
    with zipfile.ZipFile(path, "w") as contents:
        contents.writestr("lspf-analysis/lib/plugin.jar", jar.getvalue())
        server = zipfile.ZipInfo(f"lspf-analysis/server/{release.executable(item)}")
        server.external_attr = (stat.S_IFREG | (0o755 if executable else 0o644)) << 16
        contents.writestr(server, binary("windows-arm64" if wrong_binary else variant))
    return path


class CompatibilityTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.repository = self.directory / "repository"
        self.repository.mkdir()
        self.enterContext(patch.object(release, "ROOT", self.repository))
        self.event = self.directory / "event.json"
        self.enterContext(
            patch.dict(
                os.environ,
                {
                    "GITHUB_EVENT_NAME": "pull_request",
                    "GITHUB_EVENT_PATH": str(self.event),
                    "GITHUB_OUTPUT": "",
                },
            )
        )
        release.git("init", "--quiet")
        self.descriptor = (
            self.repository / "clients/intellij/src/main/resources/META-INF/plugin.xml"
        )
        self.descriptor.parent.mkdir(parents=True)
        self.descriptor.write_text("<idea-plugin/>\n")
        self.base = self.commit()
        self.data = {
            "platforms": release.config()["platforms"],
            "ides": [
                {
                    "type": "IntellijIdea",
                    "versions": ["2026.2.3", "2026.1.4", "2026.2.10"],
                },
                {"type": "PyCharm", "versions": ["2026.2.3", "2026.1.4"]},
                {"type": "WebStorm", "versions": ["2026.1.4", "2026.2.3"]},
            ],
        }

    def commit(self):
        release.git("add", ".")
        release.git(
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "Test change",
        )
        return release.git("rev-parse", "HEAD")

    def select(self, *, base=None, head=None):
        self.event.write_text(
            json.dumps(
                {
                    "pull_request": {
                        "base": {"sha": base or self.base},
                        "head": {"sha": head or release.git("rev-parse", "HEAD")},
                    }
                }
            )
        )
        return release.verification_ides(self.data)

    def test_ordinary_pr_metadata_uses_three_ide_builds_and_all_native_platforms(self):
        (self.repository / "clients/intellij/Health.kt").write_text("// UI change\n")
        self.commit()
        self.select()
        output = io.StringIO()
        with (
            patch.object(release, "config", return_value=self.data),
            redirect_stdout(output),
        ):
            release.metadata()
        metadata = json.loads(output.getvalue())
        self.assertEqual(
            [
                {"type": "IntellijIdea", "version": "2026.1.4"},
                {"type": "IntellijIdea", "version": "2026.2.10"},
                {"type": "PyCharm", "version": "2026.2.3"},
            ],
            metadata["ides"]["include"],
        )
        self.assertEqual(self.data["platforms"], metadata["platforms"]["include"])
        self.assertEqual("false", metadata["publish"])

    def test_compatibility_changes_use_all_ide_builds(self):
        for path in (
            "clients/intellij/build.gradle.kts",
            "clients/intellij/release.json",
            "clients/intellij/gradle/wrapper/gradle-wrapper.properties",
            "clients/intellij/scripts/release.py",
            ".github/workflows/release-intellij.yml",
        ):
            with self.subTest(path=path):
                base = release.git("rev-parse", "HEAD")
                changed = self.repository / path
                changed.parent.mkdir(parents=True, exist_ok=True)
                changed.write_text("Compatibility change\n")
                self.commit()
                self.assertEqual(7, len(self.select(base=base)))

    def test_renamed_descriptor_uses_full_matrix(self):
        self.descriptor.rename(self.descriptor.with_suffix(".txt"))
        self.commit()
        self.assertEqual(7, len(self.select()))

    def test_base_branch_changes_do_not_expand_an_ordinary_pr(self):
        (self.repository / "clients/intellij/Health.kt").write_text("// UI change\n")
        head = self.commit()
        release.git("checkout", "--quiet", "--detach", self.base)
        (self.repository / "clients/intellij/build.gradle.kts").write_text("// build\n")
        base = self.commit()
        self.assertEqual(3, len(self.select(base=base, head=head)))

    def test_tag_manual_and_local_runs_use_full_matrix(self):
        for event in ("push", "workflow_dispatch", ""):
            with (
                self.subTest(event=event),
                patch.dict(os.environ, {"GITHUB_EVENT_NAME": event}),
            ):
                self.assertEqual(7, len(release.verification_ides(self.data)))

    def test_single_idea_version_does_not_create_duplicate_jobs(self):
        self.data["ides"][0]["versions"] = ["2026.1.4"]
        self.assertEqual(2, len(self.select()))


class BundleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.items = release.config()["platforms"]
        self.git = patch.object(release, "git", return_value="a" * 40).start()
        self.addCleanup(patch.stopall)

    def complete_bundle(self):
        for item in self.items:
            archive(self.directory, item)
        release.manifest(self.directory)

    def test_complete_bundle_round_trip(self):
        self.complete_bundle()
        data = release.verify_bundle(self.directory)
        self.assertEqual(6, len(data["archives"]))
        self.assertEqual("0.1.0", data["serverVersion"])
        self.assertEqual("a" * 40, data["commit"])

    def test_missing_platform_prevents_manifest(self):
        for item in self.items[:-1]:
            archive(self.directory, item)
        with self.assertRaisesRegex(ValueError, "exactly one ZIP"):
            release.manifest(self.directory)

    def test_different_jvm_code_between_platforms_prevents_manifest(self):
        for item in self.items:
            archive(self.directory, item)
        archive(self.directory, self.items[-1], code=b"untested variant code")
        with self.assertRaisesRegex(ValueError, "same JVM code"):
            release.manifest(self.directory)

    def test_changed_archive_is_not_reusable(self):
        self.complete_bundle()
        path = release.find_archive(self.directory, self.items[0])
        with zipfile.ZipFile(path, "a") as contents:
            contents.writestr("lspf-analysis/changed.txt", "tampered")
        with self.assertRaisesRegex(ValueError, "Changed release archive"):
            release.verify_bundle(self.directory)

    def test_other_commit_is_not_reusable(self):
        self.complete_bundle()
        self.git.return_value = "b" * 40
        with self.assertRaisesRegex(ValueError, "source commit"):
            release.verify_bundle(self.directory)

    def test_wrong_architecture_is_rejected(self):
        item = self.items[0]
        path = archive(self.directory, item, wrong_binary=True)
        with self.assertRaisesRegex(ValueError, "architecture"):
            release.inspect_archive(path, item, release.version())

    def test_unix_executable_bit_is_required(self):
        item = self.items[-1]
        path = archive(self.directory, item, executable=False)
        with self.assertRaisesRegex(ValueError, "not executable"):
            release.inspect_archive(path, item, release.version())

    def test_variant_version_is_required(self):
        item = self.items[0]
        path = archive(self.directory, item, wrong_version=True)
        with self.assertRaisesRegex(ValueError, "descriptor version"):
            release.inspect_archive(path, item, release.version())

    def test_unsafe_zip_paths_are_rejected(self):
        for name in ("../outside", "/absolute", "C:/absolute", "folder\\escape"):
            with self.subTest(name=name):
                path = self.directory / "unsafe.zip"
                with zipfile.ZipFile(path, "w") as contents:
                    entry = zipfile.ZipInfo("placeholder")
                    entry.filename = name
                    contents.writestr(entry, "unsafe")
                with self.assertRaisesRegex(ValueError, "Unsafe"):
                    release.extract(path, self.directory / "unpacked")

    def test_zip_symlink_is_rejected(self):
        path = self.directory / "symlink.zip"
        entry = zipfile.ZipInfo("link")
        entry.external_attr = (stat.S_IFLNK | 0o777) << 16
        with zipfile.ZipFile(path, "w") as contents:
            contents.writestr(entry, "../outside")
        with self.assertRaisesRegex(ValueError, "Unsafe"):
            release.extract(path, self.directory / "unpacked")

    def test_extract_preserves_executable(self):
        item = self.items[-1]
        path = archive(self.directory, item)
        release.extract(path, self.directory / "unpacked")
        server = self.directory / "unpacked/lspf-analysis/server/lspf-analysis"
        self.assertEqual(binary(item["variant"]), server.read_bytes())
        if os.name != "nt":
            self.assertTrue(server.stat().st_mode & 0o111)


class TagTests(unittest.TestCase):
    def test_wrong_prefix_or_version_is_rejected(self):
        for tag in ("vscode-v0.1.0", "intellij-v0.2.0", "intellij-v0.1.0;echo unsafe"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.validate_tag(tag, "0.1.0", "a" * 40)

    def test_moved_tag_is_rejected(self):
        with (
            patch.object(release, "git", return_value="b" * 40),
            self.assertRaisesRegex(ValueError, "no longer points"),
        ):
            release.validate_tag("intellij-v0.1.0", "0.1.0", "a" * 40)

    def test_tag_must_belong_to_main(self):
        with (
            patch.object(release, "git", return_value="a" * 40),
            patch.object(release.subprocess, "run") as run,
        ):
            release.validate_tag("intellij-v0.1.0", "0.1.0", "a" * 40)
            self.assertEqual(
                ["git", "merge-base", "--is-ancestor", "a" * 40, "origin/main"],
                run.call_args.args[0],
            )
            self.assertTrue(run.call_args.kwargs["check"])


class PublishingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)

    def github(self):
        github = object.__new__(publish.GitHub)
        github.repo = "owner/repository"
        github.token = "test-token"
        github.assets = Mock(return_value={"archive.zip": {"id": 1}})
        return github

    def test_existing_identical_upload_is_skipped(self):
        github = self.github()
        github.download = Mock(return_value=b"signed bytes")
        path = self.directory / "archive.zip"
        path.write_bytes(b"signed bytes")
        with patch.object(publish, "request") as request:
            github.upload({"id": 7}, path)
            request.assert_not_called()

    def test_existing_different_upload_is_never_replaced(self):
        github = self.github()
        github.download = Mock(return_value=b"old signed bytes")
        path = self.directory / "archive.zip"
        path.write_bytes(b"rebuilt bytes")
        with (
            patch.object(publish, "request") as request,
            self.assertRaisesRegex(ValueError, "Refusing to replace"),
        ):
            github.upload({"id": 7}, path)
        request.assert_not_called()

    def test_redirect_does_not_send_token_to_download_host(self):
        request = urllib.request.Request(
            "https://api.github.com/asset", headers={"Authorization": "Bearer test"}
        )
        redirected = publish.Redirect().redirect_request(
            request,
            None,
            302,
            "Found",
            {},
            "https://release-assets.githubusercontent.com/file",
        )
        self.assertIsNone(redirected.get_header("Authorization"))

    def test_redirect_refuses_plain_http(self):
        request = urllib.request.Request("https://api.github.com/asset")
        with self.assertRaises(ValueError):
            publish.Redirect().redirect_request(
                request, None, 302, "Found", {}, "http://example.com/file"
            )

    def test_rc_cannot_reach_marketplace(self):
        with (
            patch.object(
                publish, "verify_bundle", return_value={"version": "0.1.0-rc.1"}
            ),
            self.assertRaisesRegex(ValueError, "RC"),
        ):
            publish.publish_marketplace(Mock(), self.directory)

    def test_bootstrap_does_not_claim_submission(self):
        with (
            patch.object(publish, "verify_bundle", return_value={"version": "0.1.0"}),
            patch.dict(os.environ, {"INTELLIJ_MARKETPLACE_READY": "false"}),
            patch.object(publish, "summary") as summary,
        ):
            publish.publish_marketplace(Mock(), self.directory)
            self.assertIn("NOT submitted", summary.call_args.args[0])

    def test_later_stable_release_fails_if_unconfigured(self):
        with (
            patch.object(publish, "verify_bundle", return_value={"version": "0.1.1"}),
            patch.dict(os.environ, {"INTELLIJ_MARKETPLACE_READY": "false"}),
            self.assertRaisesRegex(ValueError, "configured"),
        ):
            publish.publish_marketplace(Mock(), self.directory)

    def test_marketplace_recovery_compares_payload_despite_changed_signature(self):
        self.check_marketplace_recovery(tampered=False)

    def test_marketplace_recovery_rejects_changed_payload(self):
        self.check_marketplace_recovery(tampered=True)

    def check_marketplace_recovery(self, *, tampered):
        github = Mock()
        github.release.return_value = {"id": 1, "draft": False, "body": "Notes"}
        item = release.config()["platforms"][0]
        with patch.object(release, "version", return_value="0.1.0"):
            path = archive(self.directory, item)
        recorded = release.inspect_archive(path, item, "0.1.0")
        with zipfile.ZipFile(path, "a") as contents:
            contents.comment = b"different signature envelope"
            if tampered:
                contents.writestr("lspf-analysis/changed.txt", "tampered")
        data = {"version": "0.1.0", "tag": "intellij-v0.1.0", "archives": [recorded]}
        with (
            patch.object(publish, "verify_bundle", return_value=data),
            patch.dict(
                os.environ,
                {
                    "INTELLIJ_MARKETPLACE_READY": "true",
                    "INTELLIJ_PUBLISH_TOKEN": "test",
                },
            ),
            patch.object(publish, "request", return_value=path.read_bytes()) as request,
            patch.object(publish, "summary"),
        ):
            if tampered:
                with self.assertRaisesRegex(ValueError, "different contents"):
                    publish.publish_marketplace(github, self.directory)
                github.api.assert_not_called()
            else:
                publish.publish_marketplace(github, self.directory)
                github.api.assert_called_once()
            request.assert_called_once()

    def test_first_stable_requires_an_existing_rc(self):
        github = Mock()
        github.api.side_effect = [{"artifacts": []}, []]
        github.release.return_value = None
        with (
            patch.object(publish, "version", return_value="0.1.0"),
            patch.dict(os.environ, {"GITHUB_RUN_ID": "42"}),
            self.assertRaisesRegex(ValueError, "RC before"),
        ):
            publish.probe(github)

    def test_marketplace_receipts_resume_only_missing_uploads(self):
        github = Mock()
        saved_release = {"id": 1, "draft": False, "body": "Release notes"}
        github.release.side_effect = lambda _: saved_release.copy()
        github.api.side_effect = lambda _, **kwargs: saved_release.update(
            kwargs["data"]
        )
        archives = []
        for variant in ("windows-x86_64", "linux-arm64"):
            path = self.directory / f"plugin-{variant}.zip"
            path.write_bytes(variant.encode())
            archives.append(
                {
                    "variant": variant,
                    "version": f"0.1.0-{variant}",
                    "file": path.name,
                    "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                }
            )
        data = {"version": "0.1.0", "tag": "intellij-v0.1.0", "archives": archives}
        requests = []

        def request(url, *args, **kwargs):
            requests.append(url)
            if url.endswith("/upload") and requests.count(url) == 2:
                raise RuntimeError("interrupted")

        with (
            patch.object(publish, "verify_bundle", return_value=data),
            patch.dict(
                os.environ,
                {
                    "INTELLIJ_MARKETPLACE_READY": "true",
                    "INTELLIJ_PUBLISH_TOKEN": "test",
                },
            ),
            patch.object(publish, "request", side_effect=request),
            patch.object(publish, "summary"),
        ):
            with self.assertRaisesRegex(RuntimeError, "interrupted"):
                publish.publish_marketplace(github, self.directory)
            self.assertEqual(
                1, saved_release["body"].count("<!-- intellij-marketplace:")
            )
            publish.publish_marketplace(github, self.directory)
        self.assertEqual(2, saved_release["body"].count("<!-- intellij-marketplace:"))
        self.assertTrue(saved_release["body"].startswith("Release notes\n"))
        # The first platform was uploaded once; only the interrupted one retried.
        self.assertEqual(3, sum(url.endswith("/upload") for url in requests))


if __name__ == "__main__":
    unittest.main()
