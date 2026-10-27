"""Static safety checks for the Windows-only MT5 demo smoke harness.

These checks do not replace executing the PowerShell script on the real Windows VPS.
"""
from pathlib import Path
import unittest


SCRIPT = (
    Path(__file__).resolve().parents[1]
    / "scripts"
    / "mt5-demo-smoke.ps1"
)


class SmokeScriptContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SCRIPT.read_text(encoding="utf-8")

    def test_smoke_script_exists_and_is_windows_gated(self) -> None:
        self.assertIn("This smoke test must run on the target Windows MT5 host", self.source)
        self.assertIn("MT5_EXPECTED_ACCOUNT_CURRENCY", self.source)

    def test_probe_does_not_submit_or_cancel_orders(self) -> None:
        # Guard against accidentally introducing trading calls into the smoke probe.
        for forbidden in (
            "mt5.order_send(",
            "mt5.order_modify(",
            "mt5.order_delete(",
            "mt5.positions_close(",
        ):
            with self.subTest(forbidden=forbidden):
                self.assertNotIn(forbidden, self.source)

    def test_probe_does_not_mutate_market_watch(self) -> None:
        # The live worker owns symbol selection. The smoke test should observe only.
        self.assertNotIn("mt5.symbol_select(", self.source)
        self.assertIn("Do not call symbol_select here", self.source)

    def test_probe_checks_identity_freshness_and_persistence(self) -> None:
        for required in (
            "MT5_BRIDGE_PRINCIPAL_SHA256",
            "MT5_LOGIN",
            "MT5_SERVER",
            "market_data_snapshots",
            "reconciliation_snapshots",
            "observedSequences.Count -ge 2",
            "quotes_fresh",
        ):
            with self.subTest(required=required):
                self.assertIn(required, self.source)


if __name__ == "__main__":
    unittest.main()
