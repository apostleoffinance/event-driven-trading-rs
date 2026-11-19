"""Static safety checks for the Windows DEMO artifact manifest collector."""
from pathlib import Path
import unittest

SCRIPT = Path(__file__).parents[1] / "scripts" / "capture-demo-acceptance-manifest.ps1"


class DemoManifestCollectorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = SCRIPT.read_text(encoding="utf-8")

    def test_collector_is_windows_only_and_hashes_pinned_artifacts(self):
        self.assertIn("approved target Windows MT5 host", self.source)
        for field in (
            "worker_sha256", "daemon_sha256", "terminal_sha256",
            "worker_certificate_sha256", "source_commit_sha",
            "configuration_revision",
        ):
            with self.subTest(field=field):
                self.assertIn(field, self.source)
        self.assertIn("Get-FileHash", self.source)
        self.assertIn("git -C $SourceRoot rev-parse HEAD", self.source)

    def test_collector_does_not_read_or_export_broker_password_or_private_keys(self):
        for forbidden in (
            "MT5_PASSWORD", "MT5_PASSWORD_DPAPI_FILE",
            "MT5_BRIDGE_CLIENT_KEY", "MT5_BRIDGE_SERVER_KEY",
            "DATABASE_URL", "Get-ChildItem Env:",
        ):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, self.source)

    def test_manifest_defaults_to_demo_paper_and_execution_disabled(self):
        self.assertIn('broker_environment = "DEMO"', self.source)
        self.assertIn('quant_environment = "PAPER"', self.source)
        self.assertIn("execution_enabled = $false", self.source)
        self.assertIn('result = "NOT_RUN"', self.source)
        self.assertIn("1..21", self.source)

    def test_uses_only_public_worker_certificate_for_fingerprint(self):
        self.assertIn("WorkerCertificatePath", self.source)
        self.assertIn("MT5_MANIFEST_PUBLIC_CERT", self.source)
        self.assertNotIn("WorkerPrivateKeyPath", self.source)


if __name__ == "__main__":
    unittest.main()
