"""Static contract checks for the Windows MT5 conformance operator kit."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "adapters" / "mt5-python" / "scripts" / "mt5-certificate-acl-review.ps1"
RUNBOOK = ROOT / "docs" / "MT5_DEMO_SMOKE_TEST.md"
EVIDENCE = ROOT / "docs" / "MT5_DEMO_CONFORMANCE_EVIDENCE_TEMPLATE.md"


class ConformanceOperatorKitTests(unittest.TestCase):
    def test_certificate_review_checks_pin_pairing_expiry_and_acl(self) -> None:
        source = SCRIPT.read_text(encoding="utf-8")
        for required in (
            "MT5_BRIDGE_CLIENT_CERT", "MT5_BRIDGE_CLIENT_KEY",
            "MT5_BRIDGE_SERVER_CERT", "MT5_BRIDGE_SERVER_KEY",
            "MT5_BRIDGE_PRINCIPAL_SHA256", "load_cert_chain",
            "not_after_utc", "Everyone", "NT AUTHORITY\\Authenticated Users",
            "BUILTIN\\Users", "ExpectedServiceIdentity",
        ):
            with self.subTest(required=required):
                self.assertIn(required, source)
        for forbidden in ("BEGIN PRIVATE KEY", "mt5.order_send(", "mt5.order_delete("):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, source)

    def test_runbook_requires_worker_restart_and_sequence_continuity(self) -> None:
        source = RUNBOOK.read_text(encoding="utf-8")
        for required in (
            "Keep the daemon running", "same MT5_SEQUENCE_FILE",
            "post-restart market-data sequence is greater",
            "reconciliation was persisted after reconnect",
            "BLOCKED", "Do not proceed to execution",
        ):
            with self.subTest(required=required):
                self.assertIn(required, source)

    def test_evidence_template_captures_results_without_secrets(self) -> None:
        source = EVIDENCE.read_text(encoding="utf-8")
        for required in (
            "First smoke pass", "Worker restart and recovery",
            "Certificate lifecycle", "Final decision",
            "No venue execution was enabled",
        ):
            with self.subTest(required=required):
                self.assertIn(required, source)


if __name__ == "__main__":
    unittest.main()
