import importlib.util
import json
import tempfile
import unittest
from datetime import datetime, timezone
from uuid import UUID
from decimal import Decimal
from pathlib import Path
from unittest.mock import patch

MODULE_PATH = Path(__file__).with_name("migrate.py")
spec = importlib.util.spec_from_file_location("traderos_migrate", MODULE_PATH)
migrate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(migrate)


class MigrationTransformTests(unittest.TestCase):
    def test_api_request_refreshes_token_on_401_and_retries_once(self):
        import sys
        import types
        calls = []
        class Response:
            def __init__(self, status_code, payload=None):
                self.status_code = status_code
                self.content = b"{}" if payload is not None else b""
                self._payload = payload
            def json(self):
                return self._payload
        def request(method, url, headers=None, json=None, timeout=None):
            calls.append(headers.get("Authorization"))
            if len(calls) == 1:
                return Response(401, {"error": "unauthorized"})
            return Response(200, {"ok": True})
        fake_requests = types.SimpleNamespace(request=request)
        manager = migrate.TokenManager(
            access_token="old.jwt.token", refresh_token="refresh-token",
            identity_api_url="https://identity.example", config={},
            config_path=None, source_user_id="legacy-user",
        )
        def refresh():
            manager.token = "new.jwt.token"
            manager.refresh_token = "rotated-refresh-token"
            return manager.token
        with patch.dict(sys.modules, {"requests": fake_requests}), patch.object(manager, "refresh", side_effect=refresh):
            result = migrate.api_request("https://api.example", manager, "GET", "/test")
        self.assertEqual(result, {"ok": True})
        self.assertEqual(calls, ["Bearer old.jwt.token", "Bearer new.jwt.token"])


    def test_uuid_is_serialized_as_string(self):
        value = UUID("2af423a1-feab-4b9e-82bf-cfaf839c242e")
        self.assertEqual(migrate.json_value({"id": value}), {"id": str(value)})

    def test_auth_check_rejects_non_jwt_before_network(self):
        with patch.object(migrate, "api_request", side_effect=AssertionError("must not call API")):
            with self.assertRaisesRegex(RuntimeError, "not a JWT"):
                migrate.check_destination("https://journal.example", "placeholder-token")

    def test_api_request_401_message_is_safe_and_actionable(self):
        class Response:
            status_code = 401
            content = b"secret response body"
        import sys
        import types
        fake_requests = types.SimpleNamespace(request=lambda *args, **kwargs: Response())
        with patch.dict(sys.modules, {"requests": fake_requests}):
            with self.assertRaisesRegex(RuntimeError, "JWT is invalid, expired"):
                migrate.api_request("https://journal.example", "secret-token", "GET", "/api")
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

    def test_invalid_trade_chronology_is_quarantinable_without_rewriting_source(self):
        row = {
            "id": "legacy-bad-date", "status": "closed", "result": "loss",
            "trade_timestamp": "2026-10-17T21:11:00+00:00",
            "entry_timestamp": None, "exit_timestamp": "2026-10-14T20:18:00+00:00",
            "created_at": "2026-10-17T21:14:50+00:00",
        }
        self.assertTrue(migrate.has_invalid_trade_chronology(row))
        quarantine = migrate.quarantine_trade_record(row)
        self.assertEqual(quarantine["source_trade_id"], "legacy-bad-date")
        self.assertEqual(quarantine["opened_at_source"], row["trade_timestamp"])
        self.assertEqual(quarantine["exit_timestamp_source"], row["exit_timestamp"])
        with self.assertRaises(migrate.InvalidTradeChronology):
            migrate.make_trade(
                {**row, "direction": "long", "lot_size": "0.01", "entry_price": "1.2",
                 "exit_price": "1.1"},
                "account", {}, "user"
            )

    def test_trade_source_payload_retains_original_timestamps(self):
        row = {"id":"legacy-1","trade_timestamp":"2026-10-01T09:30:00+00:00",
               "entry_timestamp":"2026-10-01T09:30:00+00:00",
               "exit_timestamp":"2026-10-01T10:00:00+00:00","status":"closed",
               "direction":"long","lot_size":"0.1","entry_price":"1.2","exit_price":"1.3"}
        result = migrate.make_trade(row, "account", {}, "user")
        self.assertEqual(result["source_payload"]["source_trade_timestamp"], row["trade_timestamp"])
        self.assertEqual(result["source_payload"]["source_entry_timestamp"], row["entry_timestamp"])
        self.assertEqual(result["source_payload"]["source_exit_timestamp"], row["exit_timestamp"])

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

    def test_plan_preflight_accepts_unmapped_destination_accounts_and_setups(self):
        import sys
        from contextlib import redirect_stdout
        from io import StringIO
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            prefix = root / "snapshot.json"
            migrate.dump_json(prefix, {"format": migrate.SOURCE_FORMAT, "source_user_id": "u", "tables": {}})
            tables = {
                "accounts": [{"id": "a", "user_id": "u", "account_name": "Test",
                              "currency": "USD", "starting_balance": "1000"}],
                "setups": [{"id": "s", "user_id": "u", "name": "Setup"}],
                "trades": [{"id": "t", "user_id": "u", "account_id": "a", "setup_id": "s",
                            "trade_timestamp": "2026-10-01T09:30:00+00:00", "status": "open",
                            "direction": "long", "lot_size": "0.1", "entry_price": "1.2"}],
                "psychology": [], "trade_screenshots": [], "account_risk_profiles": [],
                "checklist_items": [], "checklist_templates": [], "trade_checklist_responses": [],
            }
            for table, rows in tables.items():
                migrate.dump_json(root / f"snapshot_{table}.json", rows)
            config_path = root / "config.json"
            migrate.dump_json(config_path, {
                "state_file": str(root / "migration-state.json"),
                "owner_map": {"u": {"jwt": "plan-only-placeholder"}},
            })
            output = StringIO()
            with patch.object(sys, "argv", ["migrate.py", "plan", "--snapshot", str(prefix),
                                           "--config", str(config_path)]), redirect_stdout(output):
                exit_code = migrate.main()
            result = json.loads(output.getvalue())
            self.assertEqual(exit_code, 0)
            self.assertTrue(result["valid"])
            self.assertTrue(result["owner_mapping_present"])
            self.assertEqual(result["destination_mappings_pending"], {"accounts": 1, "setups": 1})
            self.assertTrue(any("apply will create" in warning for warning in result["warnings"]))

    def test_export_allowlist_contains_account_owned_history_without_auth_table(self):
        self.assertIn("risk_events", migrate.SOURCE_TABLES)
        self.assertIn("ai_analyses", migrate.SOURCE_TABLES)
        self.assertIn("ai_memories", migrate.SOURCE_TABLES)
        self.assertIn("market_analyses", migrate.SOURCE_TABLES)
        self.assertIn("chart_annotations", migrate.SOURCE_TABLES)
        self.assertIn("mt5_processed_deals", migrate.SOURCE_TABLES)
        self.assertIn("mt5_sync_snapshots", migrate.SOURCE_TABLES)
        self.assertIn("mt5_source_deals", migrate.SOURCE_TABLES)
        self.assertIn("broker_candles", migrate.SOURCE_TABLES)
        self.assertIn("mt5_connections", migrate.SOURCE_TABLES)
        self.assertNotIn("users", migrate.SOURCE_TABLES)
        self.assertNotIn("market_candles", migrate.SOURCE_TABLES)

    def test_mt5_connection_export_query_omits_secret_columns(self):
        import inspect
        source = inspect.getsource(migrate.source_snapshot)
        self.assertIn("SELECT id, user_id, account_id, status, mt5_login, mt5_server", source)
        self.assertNotIn("SELECT * FROM mt5_connections", source)
        self.assertIn("token_hash", source)  # documented exclusion in the source comment/manifest
        self.assertIn("token_prefix", source)

    def test_archive_is_content_addressed_and_checksum_verified(self):
        import zipfile
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            prefix = root / "snapshot.json"
            migrate.dump_json(prefix, {"format":migrate.SOURCE_FORMAT,"source_user_id":"u","tables":{"accounts":1}})
            migrate.dump_json(root / "snapshot_accounts.json", [{"id":"a"}])
            storage = {"bucket":"bucket","access_key_id":"key","secret_access_key":"secret","provider":"r2"}
            class FakeS3:
                def __init__(self): self.objects = {}
                def put_object(self, Bucket, Key, Body, ContentType, Metadata):
                    self.objects[Key] = {"Body":Body.read(),"Metadata":Metadata}
                def head_object(self, Bucket, Key):
                    obj=self.objects[Key]
                    return {"Metadata":obj["Metadata"],"ContentLength":len(obj["Body"])}
            fake=FakeS3()
            import sys, types
            fake_module = types.SimpleNamespace(client=lambda *args, **kwargs: fake)
            with patch.dict(sys.modules, {"boto3": fake_module}):
                info=migrate.upload_snapshot_archive(prefix,"u",storage)
            self.assertTrue(info["storage_key"].endswith(info["sha256"]+".zip"))
            self.assertEqual(info["byte_size"],len(fake.objects[info["storage_key"]]["Body"]))
            self.assertEqual(fake.objects[info["storage_key"]]["Metadata"]["sha256"],info["sha256"])

    def test_snapshot_export_lists_full_account_owned_dataset(self):
        expected={"risk_events","ai_analyses","ai_memories","market_analyses","chart_annotations",
                  "mt5_connections","mt5_processed_deals","mt5_sync_snapshots","mt5_source_deals","broker_candles"}
        self.assertTrue(expected.issubset(set(migrate.SOURCE_TABLES)))
        self.assertNotIn("users",migrate.SOURCE_TABLES)
        self.assertNotIn("market_candles",migrate.SOURCE_TABLES)

    def test_token_manager_refresh_rotates_and_persists_credentials_securely(self):
        import os
        import sys
        import types
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            config_path = root / "config.private.json"
            config = {
                "edgenest_identity_api_url": "https://identity.example",
                "owner_map": {"legacy-user": {
                    "jwt": "expired.access.jwt", "refresh_token": "old-refresh-token"
                }},
            }
            config_path.write_text(json.dumps(config), encoding="utf-8")
            manager = migrate.make_token_manager(
                config, "legacy-user", config_path
            )
            self.assertIsNotNone(manager)

            class Response:
                status_code = 200
                content = b'{"access_token":"new.access.jwt","refresh_token":"rotated-refresh-token"}'
                def json(self):
                    return {"access_token": "new.access.jwt", "refresh_token": "rotated-refresh-token"}

            fake_requests = types.SimpleNamespace(
                post=lambda *args, **kwargs: Response(),
                RequestException=Exception,
            )
            with patch.dict(sys.modules, {"requests": fake_requests}):
                manager.refresh()

            persisted = json.loads(config_path.read_text(encoding="utf-8"))
            self.assertEqual(manager.token, "new.access.jwt")
            self.assertEqual(manager.refresh_token, "rotated-refresh-token")
            self.assertEqual(persisted["owner_map"]["legacy-user"]["jwt"], "new.access.jwt")
            self.assertEqual(persisted["owner_map"]["legacy-user"]["refresh_token"], "rotated-refresh-token")
            self.assertEqual(os.stat(config_path).st_mode & 0o777, 0o600)

    def test_api_request_refreshes_after_401_and_retries_once(self):
        import sys
        import types
        config = {"owner_map": {"legacy-user": {
            "jwt": "expired.access.jwt", "refresh_token": "old-refresh-token"
        }}}
        manager = migrate.make_token_manager(config, "legacy-user")
        calls = []

        class Response:
            def __init__(self, status_code, content=b'{"ok":true}'):
                self.status_code = status_code
                self.content = content
            def json(self):
                return {"ok": True}

        def request(method, url, headers, json, timeout):
            calls.append(headers["Authorization"])
            return Response(401, b'') if len(calls) == 1 else Response(200)

        def refresh():
            manager.token = "fresh.access.jwt"
            manager.refresh_token = "rotated-refresh-token"
            return manager.token

        fake_requests = types.SimpleNamespace(request=request, RequestException=Exception)
        with patch.dict(sys.modules, {"requests": fake_requests}), patch.object(manager, "refresh", side_effect=refresh) as refresh_mock:
            result = migrate.api_request("https://journal.example", manager, "GET", "/accounts")

        self.assertEqual(result, {"ok": True})
        self.assertEqual(calls, ["Bearer expired.access.jwt", "Bearer fresh.access.jwt"])
        refresh_mock.assert_called_once()

    def test_identity_login_saves_initial_token_pair_without_echoing_secrets(self):
        import builtins
        import getpass
        import sys
        import types
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "config.private.json"
            config = {
                "edgenest_identity_api_url": "https://identity.example",
                "owner_map": {"legacy-user": {}},
            }
            seen = {}

            class Response:
                status_code = 200
                content = b'{"access_token":"access-secret","refresh_token":"refresh-secret"}'
                def json(self):
                    return {
                        "access_token": "access-secret",
                        "refresh_token": "refresh-secret",
                        "expires_in": 900,
                        "user": {"id": "identity-user"},
                    }

            def post(url, json, headers, timeout):
                seen.update(url=url, email=json["email"], password=json["password"])
                return Response()

            fake_requests = types.SimpleNamespace(post=post, RequestException=Exception)
            with patch.dict(sys.modules, {"requests": fake_requests}), \
                 patch.object(builtins, "input", return_value="user@example.com"), \
                 patch.object(getpass, "getpass", return_value="not-logged"):
                result = migrate.identity_login(config, config_path, "legacy-user")

            saved = json.loads(config_path.read_text(encoding="utf-8"))
            self.assertEqual(seen["url"], "https://identity.example/api/v1/auth/login")
            self.assertEqual(seen["email"], "user@example.com")
            self.assertEqual(seen["password"], "not-logged")
            self.assertEqual(saved["owner_map"]["legacy-user"]["jwt"], "access-secret")
            self.assertEqual(saved["owner_map"]["legacy-user"]["refresh_token"], "refresh-secret")
            self.assertEqual(result["identity_user_id"], "identity-user")
            self.assertEqual(result["expires_in"], 900)

    def test_token_manager_requires_refresh_token_on_expired_access(self):
        config = {"owner_map": {"legacy-user": {"jwt": "expired.access.jwt"}}}
        manager = migrate.make_token_manager(config, "legacy-user")
        with self.assertRaisesRegex(RuntimeError, "no refresh_token is configured"):
            manager.refresh()

if __name__ == "__main__":
    unittest.main()
