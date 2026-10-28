import importlib.util
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "mt5-macos-certificate-review.py"
SPEC = importlib.util.spec_from_file_location("macos_certificate_review", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
REVIEW = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REVIEW)


class MacOSCertificateReviewTests(unittest.TestCase):
    def test_private_key_permissions_accept_owner_only_file(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            root.chmod(0o700)
            key = root / "private.key"
            key.write_text("test-only-not-a-real-key", encoding="utf-8")
            key.chmod(0o600)
            result = REVIEW.check_private_key_permissions(key)
            self.assertTrue(result["owner_is_current_user"])
            self.assertTrue(result["permissions_pass"])

    def test_private_key_permissions_reject_group_or_other_access(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            root.chmod(0o700)
            key = root / "private.key"
            key.write_text("test-only-not-a-real-key", encoding="utf-8")
            key.chmod(0o644)
            result = REVIEW.check_private_key_permissions(key)
            self.assertFalse(result["permissions_pass"])
            self.assertTrue(any("group/other" in item for item in result["problems"]))

    @unittest.skipUnless(shutil.which("openssl"), "OpenSSL is required for certificate tests")
    def test_certificate_fingerprint_uses_binary_der(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            key = root / "test.key"
            cert = root / "test.crt"
            subprocess.run(
                [
                    "openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
                    "-keyout", str(key), "-out", str(cert), "-subj", "/CN=quant-os-test",
                    "-days", "1",
                ],
                check=True,
                capture_output=True,
            )
            fingerprint = REVIEW.sha256_certificate(cert)
            self.assertEqual(len(fingerprint), 64)
            self.assertTrue(all(char in "0123456789abcdef" for char in fingerprint))


if __name__ == "__main__":
    unittest.main()
