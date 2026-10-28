from __future__ import annotations

import hashlib
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "mt5-macos-dev-pki.py"
REVIEW_SCRIPT = SCRIPT.with_name("mt5-macos-certificate-review.py")


@unittest.skipUnless(shutil.which("openssl"), "OpenSSL is required for PKI integration test")
class MacOsDevelopmentPkiTests(unittest.TestCase):
    def test_generates_and_verifies_local_mtls_pki_without_overwriting(self) -> None:
        self.assertTrue(REVIEW_SCRIPT.is_file(), "repository certificate-review helper must exist")
        with tempfile.TemporaryDirectory(prefix="quantos-mt5-pki-") as temporary:
            # A path with spaces mirrors the actual macOS Application Support path.
            output = Path(temporary) / "certs with spaces"
            result = subprocess.run(
                [sys.executable, str(SCRIPT), "--output-dir", str(output)],
                text=True, capture_output=True, timeout=90, check=False,
            )
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            self.assertIn("LOCAL DEVELOPMENT CERTIFICATES CREATED AND VERIFIED", result.stdout)
            self.assertIn("local development only", result.stdout)
            self.assertEqual(stat.S_IMODE(output.stat().st_mode), 0o700)
            for key in ("dev-ca.key.pem", "server.key.pem", "client.key.pem"):
                self.assertEqual(stat.S_IMODE((output / key).stat().st_mode), 0o600)

            for role in ("server", "client"):
                verify = subprocess.run(
                    ["openssl", "verify", "-crl_check", "-CRLfile", str(output / "dev-ca.crl.pem"),
                     "-CAfile", str(output / "dev-ca.pem"), str(output / f"{role}.pem")],
                    text=True, capture_output=True, check=False,
                )
                self.assertEqual(verify.returncode, 0, msg=verify.stdout + verify.stderr)

            san = subprocess.run(
                ["openssl", "x509", "-in", str(output / "server.pem"), "-noout", "-checkip", "127.0.0.1"],
                text=True, capture_output=True, check=False,
            )
            self.assertEqual(san.returncode, 0, msg=san.stdout + san.stderr)
            der = subprocess.run(
                ["openssl", "x509", "-in", str(output / "client.pem"), "-outform", "DER"],
                capture_output=True, check=True,
            ).stdout
            fingerprint = hashlib.sha256(der).hexdigest()
            self.assertIn(fingerprint, result.stdout)

            review = subprocess.run(
                [sys.executable, str(REVIEW_SCRIPT),
                 "--client-cert", str(output / "client.pem"),
                 "--client-key", str(output / "client.key.pem"),
                 "--server-cert", str(output / "server.pem"),
                 "--server-key", str(output / "server.key.pem"),
                 "--ca-file", str(output / "dev-ca.pem"),
                 "--expected-client-sha256", fingerprint,
                 "--crl-file", str(output / "dev-ca.crl.pem"),
                 "--evidence-out", str(Path(temporary) / "certificate-review.json")],
                text=True, capture_output=True, timeout=30, check=False,
            )
            self.assertEqual(review.returncode, 0, msg=review.stdout + review.stderr)
            self.assertIn("PARTIAL PASS", review.stdout)
            self.assertIn('"release_gate": "BLOCKED"', review.stdout)
            self.assertIn('"revocation_checked_for_both": true', review.stdout)

            second = subprocess.run(
                [sys.executable, str(SCRIPT), "--output-dir", str(output)],
                text=True, capture_output=True, timeout=10, check=False,
            )
            self.assertNotEqual(second.returncode, 0)
            self.assertIn("refusing to overwrite", second.stderr)


if __name__ == "__main__":
    unittest.main()
