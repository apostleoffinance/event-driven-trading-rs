from __future__ import annotations

import pathlib
import unittest
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]
MIGRATION = ROOT / "migrations" / "20261120000001_mt5_gateway_terminal_registry.sql"
CONFIG = ROOT / "config" / "mt5" / "tentrade.demo.example.toml"


class Mt5RegistryContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.sql = MIGRATION.read_text(encoding="utf-8")
        cls.config = tomllib.loads(CONFIG.read_text(encoding="utf-8"))

    def test_registry_has_separate_gateway_and_terminal_slot_entities(self):
        self.assertIn("CREATE TABLE mt5_gateway_hosts", self.sql)
        self.assertIn("CREATE TABLE mt5_terminal_slots", self.sql)
        self.assertIn("UNIQUE (gateway_host_id,terminal_id)", self.sql)

    def test_terminal_slot_has_distinct_non_shared_workload_identities(self):
        for column in ("terminal_profile_ref", "service_identity_ref", "worker_ref"):
            self.assertIn(f"{column} TEXT NOT NULL UNIQUE", self.sql)

    def test_capacity_is_bounded_and_enforced(self):
        self.assertIn("terminal_capacity INTEGER NOT NULL DEFAULT 1 CHECK (terminal_capacity BETWEEN 1 AND 128)", self.sql)
        self.assertIn("gateway terminal capacity has been reached", self.sql)

    def test_assignment_must_match_ready_slot(self):
        self.assertIn("BEFORE INSERT ON broker_worker_assignments", self.sql)
        self.assertIn("slot.status <> 'READY'", self.sql)
        self.assertIn("slot.worker_ref <> NEW.worker_ref", self.sql)
        self.assertIn("slot.broker_server_id <> NEW.broker_server_id", self.sql)

    def test_existing_assignments_are_not_silently_backfilled(self):
        self.assertIn("Existing single-slot assignments remain valid", self.sql)
        self.assertIn("ADD COLUMN terminal_slot_id TEXT", self.sql)

    def test_tentrade_template_is_disabled_and_has_no_guessed_server(self):
        self.assertEqual(self.config["broker"]["id"], "tentrade")
        self.assertFalse(self.config["broker"]["enabled"])
        self.assertEqual(self.config["server"]["canonical_identifier"], "")
        self.assertEqual(self.config["server"]["environment"], "DEMO")

    def test_customer_credentials_and_execution_remain_disabled(self):
        enrollment = self.config["enrollment"]
        self.assertFalse(enrollment["credential_intake_enabled"])
        self.assertFalse(enrollment["customer_self_service_enabled"])
        self.assertTrue(enrollment["read_only"])
        self.assertFalse(enrollment["order_submission_enabled"])
        self.assertFalse(enrollment["order_modification_enabled"])
        self.assertFalse(enrollment["order_cancellation_enabled"])
        self.assertFalse(enrollment["position_close_enabled"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
