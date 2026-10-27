"""Static contract checks for the Windows MT5 restart-verification script."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "adapters" / "mt5-python" / "scripts" / "mt5-demo-restart-verify.ps1"


class RestartVerificationContractTests(unittest.TestCase):
    def test_requires_existing_worker_and_daemon_and_only_restarts_worker(self) -> None:
        source = SCRIPT.read_text(encoding="utf-8")
        for required in (
            "WorkerServiceName", "DaemonServiceName",
            "must already be running before the first smoke pass",
            "will not start or stop the daemon",
            "Stop-Service -Name $WorkerServiceName",
            "Start-Service -Name $WorkerServiceName",
        ):
            with self.subTest(required=required):
                self.assertIn(required, source)
        self.assertNotIn("Stop-Service -Name $DaemonServiceName", source)
        self.assertNotIn("Restart-Service -Name $DaemonServiceName", source)

    def test_verifies_sequence_continuity_and_fresh_reconciliation(self) -> None:
        source = SCRIPT.read_text(encoding="utf-8")
        for required in (
            "pre_stop_market_sequence", "post_restart_market_sequence",
            "post_restart_reconciliation_sequence",
            "market sequence", "sequence_advanced",
            "post_restart_reconciliation_fresh", "PASS_RESTART_RECOVERY",
            "mt5-demo-smoke.ps1",
        ):
            with self.subTest(required=required):
                self.assertIn(required, source)

    def test_evidence_is_written_even_on_failure_and_no_order_calls(self) -> None:
        source = SCRIPT.read_text(encoding="utf-8")
        self.assertIn("finally", source)
        self.assertIn("Write-Evidence $record", source)
        for forbidden in ("mt5.order_send(", "mt5.order_delete(", "order_modify("):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, source)


if __name__ == "__main__":
    unittest.main()
