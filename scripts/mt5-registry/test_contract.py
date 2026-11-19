from __future__ import annotations

import pathlib
import unittest
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]
MIGRATION = ROOT / "migrations" / "20261119000002_mt5_gateway_terminal_registry.sql"
CONFIG = ROOT / "config" / "mt5" / "tentrade.demo.example.toml"
SLOT_ADMISSION = ROOT / "crates" / "persistence" / "src" / "mt5_terminal_slot_admission.rs"
RUNTIME = ROOT / "crates" / "intent-bridge" / "src" / "mt5_runtime.rs"
DAEMON = ROOT / "crates" / "intent-bridge" / "src" / "bin" / "mt5-bridge-daemon.rs"


class Mt5RegistryContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.sql = MIGRATION.read_text(encoding="utf-8")
        cls.config = tomllib.loads(CONFIG.read_text(encoding="utf-8"))
        cls.slot_admission = SLOT_ADMISSION.read_text(encoding="utf-8")
        cls.runtime = RUNTIME.read_text(encoding="utf-8")
        cls.daemon = DAEMON.read_text(encoding="utf-8")

    def test_registry_has_separate_gateway_and_terminal_slot_entities(self):
        self.assertIn("CREATE TABLE mt5_gateway_hosts", self.sql)
        self.assertIn("CREATE TABLE mt5_terminal_slots", self.sql)
        self.assertIn("UNIQUE (gateway_host_id,terminal_id)", self.sql)

    def test_terminal_slot_has_distinct_non_shared_workload_identities(self):
        for column in ("terminal_profile_ref", "service_identity_ref", "worker_ref"):
            self.assertIn(f"{column} TEXT NOT NULL UNIQUE", self.sql)

    def test_capacity_is_bounded_and_enforced(self):
        self.assertIn("terminal_capacity INTEGER NOT NULL DEFAULT 1 CHECK (terminal_capacity BETWEEN 1 AND 128)", self.sql)
        self.assertIn("registered_terminal_slots=registered_terminal_slots+1", self.sql)
        self.assertIn("registered_terminal_slots < terminal_capacity", self.sql)

    def test_assignment_must_match_ready_slot(self):
        self.assertIn("BEFORE INSERT ON broker_worker_assignments", self.sql)
        self.assertIn("slot.status <> 'READY'", self.sql)
        self.assertIn("slot.worker_ref <> NEW.worker_ref", self.sql)
        self.assertIn("slot.organization_id <> NEW.organization_id", self.sql)
        self.assertIn("slot.external_broker_account_id <> NEW.external_broker_account_id", self.sql)
        self.assertIn("binding.status='VERIFIED'", self.sql)
        self.assertIn("slot.broker_server_id <> NEW.broker_server_id", self.sql)

    def test_existing_assignments_are_not_silently_backfilled(self):
        self.assertIn("Existing single-slot assignments remain valid", self.sql)
        self.assertIn("ADD COLUMN terminal_slot_id TEXT", self.sql)

    def test_runtime_requires_ready_managed_gateway_slot_and_worker_session(self):
        for required in (
            "slot.id = $1",
            "slot.status = 'READY'",
            "host.status = 'READY'",
            "assignment.terminal_slot_id = slot.id",
            "assignment.status = 'ACTIVE'",
            "assignment.valid_until > NOW()",
            "session.status = 'READY'",
            "session.ready_until > NOW()",
        ):
            self.assertIn(required, self.slot_admission)

    def test_every_frame_revalidates_the_exact_terminal_slot(self):
        self.assertIn("validate_ready_mt5_terminal_slot(", self.runtime)
        self.assertIn("guard.terminal_slot_ref", self.runtime)
        self.assertIn("MT5_TERMINAL_SLOT_ID", self.daemon)
        self.assertIn("validate_persisted_admission_or_degrade", self.runtime)

    def test_terminal_slot_registry_outage_fails_closed(self):
        self.assertIn("terminal-slot registry is unavailable; refusing frame ingestion", self.runtime)
        self.assertIn("if !slot_ready", self.runtime)

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
