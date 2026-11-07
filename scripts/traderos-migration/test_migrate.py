import importlib.util
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("migrate.py")
spec = importlib.util.spec_from_file_location("traderos_migrate", MODULE_PATH)
migrate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(migrate)


class MigrationTransformTests(unittest.TestCase):
    def test_decimal_is_exact_string(self):
        self.assertEqual(migrate.decimal_str("0.010000000000000001"), "0.010000000000000001")

    def test_naive_timestamp_fails_closed(self):
        with self.assertRaises(ValueError):
            migrate.parse_time("2026-10-01T09:30:00")

    def test_timezone_timestamp_preserves_instant(self):
        self.assertTrue(migrate.parse_time("2026-10-01T09:30:00+01:00").endswith("+01:00"))

    def test_closed_trade_requires_close_timestamp_and_exit_price(self):
        row = {"id":"trade-1","trade_timestamp":"2026-10-01T09:30:00+00:00",
               "exit_timestamp":None,"status":"closed","direction":"long","account_id":"a1",
               "lot_size":"0.01","entry_price":"1.10000","exit_price":None,
               "stop_loss":"1.09500","risk_amount":"5.00"}
        with self.assertRaises(ValueError):
            migrate.make_trade(row, "journal-account", {}, "legacy-user")

    def test_source_identity_is_stable(self):
        row = {"id":"trade-42","trade_timestamp":"2026-10-01T09:30:00+00:00",
               "exit_timestamp":None,"status":"open","direction":"short","account_id":"a1",
               "lot_size":"0.01","entry_price":"1.10000","exit_price":None,
               "stop_loss":"1.10500","risk_amount":"5.00"}
        result = migrate.make_trade(row, "journal-account", {}, "legacy-user")
        self.assertEqual(result["source_system"], "TraderOS")
        self.assertEqual(result["source_trade_id"], "trade-42")
        self.assertEqual(result["status"], "open")
        self.assertEqual(result["direction"], "short")

    def test_secret_fields_are_removed_from_json_snapshot(self):
        self.assertEqual(migrate.json_value({"api_token":"nope","ok":1}), {"ok":1})


if __name__ == "__main__":
    unittest.main()
