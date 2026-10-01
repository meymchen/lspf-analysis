import tempfile
import unittest
from pathlib import Path

import release_server as release


class ServerReleaseTests(unittest.TestCase):
    def test_only_matching_push_tags_publish(self):
        ref = "refs/tags/server-v0.1.0"
        self.assertTrue(release.publish_allowed("push", ref, "0.1.0"))
        for event, candidate in [
            ("workflow_dispatch", ref),
            ("pull_request", ref),
            ("push", "refs/heads/main"),
            ("push", "refs/tags/intellij-v0.1.0"),
        ]:
            self.assertFalse(release.publish_allowed(event, candidate, "0.1.0"))
        with self.assertRaises(ValueError):
            release.publish_allowed("push", "refs/tags/server-v0.2.0", "0.1.0")

    def test_incomplete_or_mixed_version_assets_cannot_publish(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(ValueError):
                release.verify_assets(root, "0.1.0")
            for platform in release.PLATFORMS:
                (root / release.asset_name(platform["target"], "0.1.0")).write_bytes(
                    b"server"
                )
            self.assertEqual(len(release.verify_assets(root, "0.1.0")), 6)
            extra = root / "lspf-analysis-0.0.9-old"
            extra.write_bytes(b"old")
            with self.assertRaises(ValueError):
                release.verify_assets(root, "0.1.0")
            extra.unlink()
            next(root.iterdir()).write_bytes(b"")
            with self.assertRaises(ValueError):
                release.verify_assets(root, "0.1.0")

    def test_target_is_an_allowlisted_identifier(self):
        with self.assertRaises(ValueError):
            release.asset_name("../../unexpected", "0.1.0")


if __name__ == "__main__":
    unittest.main()
