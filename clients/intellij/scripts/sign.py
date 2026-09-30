"""Sign the tested ZIPs once; keep secret material outside the workspace."""

import os
import subprocess
import tempfile
from pathlib import Path

from release import CLIENT, config, find_archive, inspect_archive, manifest, version


def main():
    key = os.environ.get("INTELLIJ_PRIVATE_KEY", "")
    certificate = os.environ.get("INTELLIJ_CERTIFICATE_CHAIN", "")
    if not key.strip() or not certificate.strip():
        raise ValueError(
            "INTELLIJ_PRIVATE_KEY and INTELLIJ_CERTIFICATE_CHAIN are required"
        )
    unsigned = CLIENT / "build/distributions"
    signed = CLIENT / "build/signed"
    signed.mkdir(parents=True, exist_ok=True)
    if list(signed.iterdir()):
        raise ValueError(
            "Signed directory must be empty; recover existing artifacts instead of signing again"
        )
    with tempfile.TemporaryDirectory(prefix="intellij-sign-") as temporary:
        key_path = Path(temporary) / "key.pem"
        certificate_path = Path(temporary) / "chain.crt"
        key_path.write_text(key, encoding="utf-8")
        key_path.chmod(0o600)
        certificate_path.write_text(certificate, encoding="utf-8")
        env = dict(os.environ)
        env.pop("INTELLIJ_PRIVATE_KEY", None)
        env.pop("INTELLIJ_CERTIFICATE_CHAIN", None)
        env["INTELLIJ_PRIVATE_KEY_FILE"] = str(key_path)
        env["INTELLIJ_CERTIFICATE_CHAIN_FILE"] = str(certificate_path)
        for item in config()["platforms"]:
            source = find_archive(unsigned, item)
            before = inspect_archive(source, item, version())
            destination = signed / source.name
            subprocess.run(
                [
                    "./gradlew",
                    "signPlugin",
                    f"-PreleaseArchive={source}",
                    f"-PsignedArchive={destination}",
                    "--no-configuration-cache",
                    "--no-build-cache",
                ],
                cwd=CLIENT,
                env=env,
                check=True,
            )
            subprocess.run(
                [
                    "./gradlew",
                    "verifyPluginSignature",
                    f"-PreleaseArchive={destination}",
                    "--no-configuration-cache",
                    "--no-build-cache",
                ],
                cwd=CLIENT,
                env=env,
                check=True,
            )
            after = inspect_archive(destination, item, version())
            if before["payloadSha256"] != after["payloadSha256"]:
                raise ValueError("Signing changed the tested plugin contents")
        manifest(signed)


if __name__ == "__main__":
    main()
