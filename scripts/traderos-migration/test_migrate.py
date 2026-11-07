import importlib.util
import json
import tempfile
import unittest
from datetime import datetime, timezone
from decimal import Decimal
from pathlib import Path
from unittest.mock import patch

MODULE_PATH = Path(__file__).with_name("migrate.py")
spec = importlib.util.spec_from_file_location("traderos_migrate", MODULE_PATH)
migrate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(migrate)


class MigrationTransformTests(unittest.TestCase):
    def test_decimal_is_exact_string(self):
        self.assertEqual(migrate.decimal_str("0.010000000000000001"), "0.010000000000000001")

    def test_invalid_decimal_fails(self):
        with self.assertRaises(ValueError):
            migrate.decimal_str("not-a-number")

    def test_naive_timestamp_fails_closed(self):
        with self.assertRaises(ValueError):
            migrate.parse_time("2026-10-01T09:30:00")

    def test_timezone_timestamp_preserves_offset(self):
        self.assertTrue(migrate.parse_time("2026-10-01T09:30:00+01:00").endswith("+01:00"))

    def test_secret_fields_are_removed_recursively(self):
        self.assertEqual(migrate.json_value({"api_token":"nope","ok":1,"nested":{"refresh_token":"x","v":2}}),
                         {"ok":1,"nested":{"v":2}})

    def test_closed_trade_requires_close_timestamp_and_exit_price(self):
        row = {"id":"trade-1","trade_timestamp":"2026-10-01T09:30:00+00:00",
               "exit_timestamp":None,"status":"closed","direction":"long","account_id":"a1",
               "lot_size":"0.01","entry_price":"1.10000","exit_price":None}
        with self.assertRaises(ValueError):
            migrate.make_trade(row, "journal-account", {}, "legacy-user")

    def test_trade_keeps_exact_financial_values_and_source_id(self):
        row = {"id":"trade-42","trade_timestamp":"2026-10-01T09:30:00+00:00",
               "exit_timestamp":None,"status":"open","direction":"short","account_id":"a1",
               "lot_size":"0.010000000000000001","entry_price":"1.10000","exit_price":None,
               "stop_loss":"1.10500","risk_amount":"5.00","realized_pnl":"2.50",
               "commission":"0.10","swap":"0.05","planned_rr":"2.25","external_deal_id":"deal-1"}
        result = migrate.make_trade(row, "journal-account", {}, "legacy-user")
        self.assertEqual(result["source_system"], "TraderOS")
        self.assertEqual(result["source_trade_id"], "trade-42")
        self.assertEqual(result["quantity"], "0.010000000000000001")
        self.assertEqual(result["gross_pnl"], "2.35")
        self.assertEqual(result["net_pnl"], "2.50")
        self.assertEqual(result["source_payload"]["external_deal_id"], "deal-1")

    def test_trade_rejects_missing_direction(self):
        row = {"id":"t","trade_timestamp":"2026-10-01T09:30:00+00:00",
               "status":"open","lot_size":"0.1","entry_price":"1.2"}
        with self.assertRaises(ValueError):
            migrate.make_trade(row, "a", {}, "u")

    def test_dry_run_does_not_write_state_or_call_api(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            prefix = root / "snapshot.json"
            migrate.dump_json(prefix, {"format":migrate.SOURCE_FORMAT,"source_user_id":"u","tables":{}})
            for table, rows in {
                "accounts":[{"id":"a","user_id":"u","account_name":"Test","currency":"USD","starting_balance":"1000"}],
                "setups":[{"id":"s","user_id":"u","name":"Setup"}],
                "trades":[{"id":"t","user_id":"u","account_id":"a","setup_id":"s",
                           "trade_timestamp":"2026-10-01T09:30:00+00:00","status":"open",
                           "direction":"long","lot_size":"0.1","entry_price":"1.2"}],
                "psychology":[], "trade_screenshots":[], "account_risk_profiles":[],
                "checklist_items":[], "checklist_templates":[], "trade_checklist_responses":[]
            }.items():
                migrate.dump_json(root / f"snapshot_{table}.json", rows)
            state_file = root / "state.json"
            original = {"account_map":{},"setup_map":{}}
            migrate.dump_json(state_file, original)
            config = {"edgenest_api_url":"https://example.invalid","state_file":str(state_file),
                      "owner_map":{"u":{"jwt":"fake-test-token"}}}
            with patch.object(migrate, "api_request", side_effect=AssertionError("API must not be called")):
                report = migrate.migrate(prefix, config, True)
            self.assertTrue(report["complete"])
            self.assertEqual(migrate.load_json(state_file), original)

    def test_snapshot_rejects_trade_account_orphan_and_flags_missing_source_mapping(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            prefix = root / "snapshot.json"
            migrate.dump_json(prefix, {"format":migrate.SOURCE_FORMAT,"source_user_id":"u","tables":{}})
            migrate.dump_json(root / "snapshot_accounts.json", [{"id":"a"}])
            migrate.dump_json(root / "snapshot_setups.json", [])
            migrate.dump_json(root / "snapshot_trades.json", [{
                "id":"t","account_id":"missing","trade_timestamp":"2026-10-01T09:30:00+00:00",
                "status":"open","direction":"long","lot_size":"0.1","entry_price":"1.2"}])
            for table in ("psychology","trade_screenshots","trade_checklist_responses"):
                migrate.dump_json(root / f"snapshot_{table}.json", [])
            plan=migrate.validate_snapshot(prefix,"u",{}, {})
            self.assertFalse(plan["valid"])
            self.assertTrue(any("missing source account" in e for e in plan["errors"]))

    def test_screenshot_checksum_is_validated(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            data=b"image-bytes"
            shot=root/"shot.bin"; shot.write_bytes(data)
            prefix=root/"snapshot.json"
            migrate.dump_json(prefix, {"format":migrate.SOURCE_FORMAT,"source_user_id":"u","tables":{}})
            migrate.dump_json(root/"snapshot_accounts.json",[{"id":"a"}])
            migrate.dump_json(root/"snapshot_setups.json",[])
            migrate.dump_json(root/"snapshot_trades.json",[{"id":"t","account_id":"a","trade_timestamp":"2026-10-01T09:30:00+00:00","status":"open","direction":"long","lot_size":"0.1","entry_price":"1.2"}])
            migrate.dump_json(root/"snapshot_psychology.json",[])
            migrate.dump_json(root/"snapshot_trade_screenshots.json",[{"id":"s","trade_id":"t","file_path":str(shot),"file_sha256":"wrong"}])
            migrate.dump_json(root/"snapshot_trade_checklist_responses.json",[])
            plan=migrate.validate_snapshot(prefix,"u",{"a":"target"}, {})
            self.assertFalse(plan["valid"])
            self.assertTrue(any("checksum mismatch" in e for e in plan["errors"]))

    def test_validate_snapshot_allows_dry_run_placeholder_mappings(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            prefix = root / "snapshot.json"
            migrate.dump_json(prefix, {"format":migrate.SOURCE_FORMAT,"source_user_id":"u","tables":{}})
            migrate.dump_json(root / "snapshot_accounts.json", [{"id":"a"}])
            migrate.dump_json(root / "snapshot_setups.json", [{"id":"s"}])
            migrate.dump_json(root / "snapshot_trades.json", [{
                "id":"t","account_id":"a","setup_id":"s",
                "trade_timestamp":"2026-10-01T09:30:00+00:00","status":"open",
                "direction":"long","lot_size":"0.1","entry_price":"1.2"}])
            for table in ("psychology","trade_screenshots","trade_checklist_responses"):
                migrate.dump_json(root / f"snapshot_{table}.json", [])
            plan = migrate.validate_snapshot(prefix, "u", {"a":"DRYRUN:a"}, {"s":"DRYRUN:s"}, allow_placeholders=True)
            self.assertTrue(plan["valid"])


if __name__ == "__main__":
    unittest.main()
