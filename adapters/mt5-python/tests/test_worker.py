import json
import tempfile
import unittest
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from mt5_adapter.worker import (
    AdapterError,
    Config,
    SequenceStore,
    build_heartbeat,
    build_market_data_snapshot,
    build_snapshot,
    decimal_text,
    load_terminal_runtime,
    run_worker,
    send_frame,
)


class FakeTerminal:
    def __init__(self):
        self.is_connected = True

    def connect(self, config):
        self.is_connected = True

    def shutdown(self):
        self.is_connected = False

    def connected(self):
        return self.is_connected

    def account_info(self):
        return SimpleNamespace(login=123456, server="Broker-Demo", currency="USD",
                               balance=10000.0, equity=9990.5, margin=100.0, margin_free=9890.5)

    def orders(self):
        return [SimpleNamespace(ticket=77, type=2, symbol="EURUSD.a", volume_current=0.1, state=1)]

    def positions(self):
        return [SimpleNamespace(ticket=88, type=0, symbol="EURUSD.a", volume=0.1,
                                price_open=1.08, profit=-9.5)]

    def select_symbol(self, symbol):
        return symbol == "EURUSD.a"

    def symbol_info(self, symbol):
        return SimpleNamespace(name=symbol, currency_base="EUR", currency_profit="USD",
                               digits=5, point=0.00001, trade_tick_size=0.00001,
                               trade_tick_value=1.0, trade_contract_size=100000.0,
                               volume_min=0.01, volume_max=100.0, volume_step=0.01,
                               trade_mode=4)

    def symbol_info_tick(self, symbol):
        import time
        return SimpleNamespace(symbol=symbol, bid=1.08450, ask=1.08462, time=int(time.time()))


class FakeConstants:
    ORDER_TYPE_BUY = 0
    ORDER_TYPE_SELL = 1
    ORDER_TYPE_BUY_LIMIT = 2
    ORDER_TYPE_SELL_LIMIT = 3
    ORDER_TYPE_BUY_STOP = 4
    ORDER_TYPE_SELL_STOP = 5
    ORDER_TYPE_BUY_STOP_LIMIT = 6
    ORDER_TYPE_SELL_STOP_LIMIT = 7
    POSITION_TYPE_BUY = 0


def config(path):
    return Config("C:/terminal64.exe", 123456, "not-logged", "Broker-Demo", "123456",
                  "terminal-demo-1", "127.0.0.1", 9443, "bridge.example.test",
                  "ca.pem", "client.pem", "client.key", str(path), ("EURUSD.a",))


class AdapterContractTests(unittest.TestCase):
    def test_explicit_terminal_provider_returns_read_only_protocol(self):
        import types

        provider = types.ModuleType("quant_os_test_provider")
        provider.create = lambda cfg: (FakeTerminal(), FakeConstants)
        with patch.dict(sys.modules, {"quant_os_test_provider": provider}):
            with patch.dict(os.environ, {"MT5_TERMINAL_PROVIDER": "quant_os_test_provider:create"}):
                terminal, constants = load_terminal_runtime(config("unused"))
        self.assertIsInstance(terminal, FakeTerminal)
        self.assertIs(constants, FakeConstants)

    def test_terminal_provider_rejects_invalid_specification(self):
        with patch.dict(os.environ, {"MT5_TERMINAL_PROVIDER": "missing-separator"}):
            with self.assertRaises(AdapterError):
                load_terminal_runtime(config("unused"))

    def test_terminal_provider_rejects_incomplete_terminal_protocol(self):
        import types

        provider = types.ModuleType("quant_os_bad_provider")
        provider.create = lambda cfg: (object(), FakeConstants)
        with patch.dict(sys.modules, {"quant_os_bad_provider": provider}):
            with patch.dict(os.environ, {"MT5_TERMINAL_PROVIDER": "quant_os_bad_provider:create"}):
                with self.assertRaises(AdapterError):
                    load_terminal_runtime(config("unused"))

    def test_decimal_text_never_uses_exponent_notation(self):
        self.assertEqual(decimal_text(1e-7, "price"), "0.0000001")
        self.assertEqual(decimal_text(1000.0, "balance"), "1000.0")
        with self.assertRaises(AdapterError):
            decimal_text(float("nan"), "balance")

    def test_sequence_persists_across_store_instances(self):
        with tempfile.TemporaryDirectory() as directory:
            path = str(Path(directory) / "sequence.json")
            self.assertEqual(SequenceStore(path).next(), 0)
            self.assertEqual(SequenceStore(path).next(), 1)

    def test_heartbeat_advertises_no_order_execution_capabilities(self):
        caps = build_heartbeat(config("unused"), 12)["payload"]["capabilities"]
        self.assertFalse(caps["market_orders"])
        self.assertFalse(caps["cancel_requests"])
        self.assertFalse(caps["idempotent_client_order_id"])
        self.assertTrue(caps["order_lookup"])
        self.assertTrue(caps["sequence_numbers"])

    def test_snapshot_is_complete_identity_bound_and_decimal_string_only(self):
        payload = build_snapshot(config("unused"), FakeTerminal(), FakeConstants, 12)["payload"]
        self.assertEqual(payload["terminal_id"], "terminal-demo-1")
        self.assertEqual(payload["server"], "Broker-Demo")
        self.assertEqual(payload["account_state"]["balance"], "10000.0")
        self.assertEqual(payload["open_orders"][0]["instrument_id"], "EURUSD.a")
        self.assertEqual(payload["positions"][0]["unrealized_pnl"], "-9.5")
        self.assertEqual(payload["sequence"], 12)

    def test_market_data_snapshot_contains_exact_specs_and_fresh_bid_ask(self):
        payload = build_market_data_snapshot(config("unused"), FakeTerminal(), 13)["payload"]
        self.assertEqual(payload["type"], "market_data_snapshot")
        self.assertEqual(payload["status"], "fresh")
        self.assertEqual(payload["account_currency"], "USD")
        self.assertEqual(payload["instruments"][0]["symbol"], "EURUSD.a")
        self.assertEqual(payload["instruments"][0]["tick_size"], "0.00001")
        self.assertEqual(payload["instruments"][0]["volume_step"], "0.01")
        self.assertEqual(payload["quotes"][0]["bid"], "1.0845")
        self.assertEqual(payload["quotes"][0]["ask"], "1.08462")
        self.assertEqual(payload["quotes"][0]["status"], "fresh")
        self.assertTrue(payload["quotes"][0]["tick_time"].endswith("Z"))

    def test_market_data_snapshot_fails_closed_when_symbol_or_quote_unavailable(self):
        terminal = FakeTerminal()
        terminal.select_symbol = lambda symbol: False
        with self.assertRaises(AdapterError):
            build_market_data_snapshot(config("unused"), terminal, 13)
        terminal = FakeTerminal()
        terminal.symbol_info_tick = lambda symbol: None
        payload = build_market_data_snapshot(config("unused"), terminal, 13)["payload"]
        self.assertEqual(payload["status"], "unavailable")
        self.assertEqual(payload["quotes"][0]["status"], "unavailable")
        self.assertIsNone(payload["quotes"][0]["bid"])
        self.assertIsNone(payload["quotes"][0]["ask"])
        self.assertIsNone(payload["quotes"][0]["tick_time"])

    def test_native_provider_normalizes_server_timezone_offset_against_host_utc(self):
        terminal = FakeTerminal()
        terminal.source = "mt5_macos_mql5_socket_v1"
        now = int(datetime.now(timezone.utc).timestamp())
        terminal.symbol_info_tick = lambda symbol: SimpleNamespace(
            symbol=symbol, bid=1.08450, ask=1.08462,
            time=now + 7200, server_time=now + 7201
        )
        payload = build_market_data_snapshot(config("unused"), terminal, 13)["payload"]
        quote_time = datetime.fromisoformat(payload["quotes"][0]["tick_time"].replace("Z", "+00:00"))
        self.assertLessEqual(abs(quote_time.timestamp() - now), 2)

    def test_native_provider_marks_non_timezone_server_offset_invalid(self):
        terminal = FakeTerminal()
        terminal.source = "mt5_macos_mql5_socket_v1"
        now = int(datetime.now(timezone.utc).timestamp())
        terminal.symbol_info_tick = lambda symbol: SimpleNamespace(
            symbol=symbol, bid=1.08450, ask=1.08462,
            time=now + 7200, server_time=now + 7650
        )
        payload = build_market_data_snapshot(config("unused"), terminal, 13)["payload"]
        self.assertEqual(payload["status"], "invalid")
        self.assertEqual(
            payload["quotes"][0]["reason_code"], "invalid_server_time_offset"
        )

    def test_native_provider_marks_frozen_quote_stale_against_advancing_server_clock(self):
        terminal = FakeTerminal()
        terminal.source = "mt5_macos_mql5_socket_v1"
        now = int(datetime.now(timezone.utc).timestamp())
        terminal.symbol_info_tick = lambda symbol: SimpleNamespace(
            symbol=symbol,
            bid=1.08450,
            ask=1.08462,
            time=now - 10_000,
            server_time=now + 7200,
        )
        payload = build_market_data_snapshot(config("unused"), terminal, 13)["payload"]
        quote = payload["quotes"][0]
        self.assertEqual(payload["status"], "stale")
        self.assertEqual(quote["status"], "stale")
        self.assertEqual(quote["bid"], "1.0845")
        self.assertEqual(quote["ask"], "1.08462")
        self.assertLess(datetime.fromisoformat(quote["tick_time"].replace("Z", "+00:00")).timestamp(), now)

    def test_market_data_snapshot_marks_stale_quote_without_rejecting_it(self):
        import time

        terminal = FakeTerminal()
        terminal.symbol_info_tick = lambda symbol: SimpleNamespace(
            symbol=symbol, bid=1.08450, ask=1.08462, time=int(time.time()) - 120
        )
        payload = build_market_data_snapshot(config("unused"), terminal, 13)["payload"]
        self.assertEqual(payload["status"], "stale")
        self.assertEqual(payload["quotes"][0]["status"], "stale")
        self.assertEqual(payload["quotes"][0]["bid"], "1.0845")
        self.assertEqual(payload["quotes"][0]["ask"], "1.08462")

    def test_subsequent_fresh_quote_recovers_market_data_status(self):
        import time

        terminal = FakeTerminal()
        terminal.symbol_info_tick = lambda symbol: SimpleNamespace(
            symbol=symbol, bid=1.08450, ask=1.08462, time=int(time.time()) - 120
        )
        stale = build_market_data_snapshot(config("unused"), terminal, 13)["payload"]
        terminal.symbol_info_tick = lambda symbol: SimpleNamespace(
            symbol=symbol, bid=1.08455, ask=1.08467, time=int(time.time())
        )
        fresh = build_market_data_snapshot(config("unused"), terminal, 14)["payload"]
        self.assertEqual(stale["status"], "stale")
        self.assertEqual(fresh["status"], "fresh")
        self.assertEqual(fresh["quotes"][0]["status"], "fresh")
        self.assertEqual(fresh["quotes"][0]["bid"], "1.08455")

    def test_market_data_snapshot_reports_future_quote_timestamp(self):
        import time

        terminal = FakeTerminal()
        terminal.symbol_info_tick = lambda symbol: SimpleNamespace(
            symbol=symbol, bid=1.08450, ask=1.08462, time=int(time.time()) + 30
        )
        payload = build_market_data_snapshot(config("unused"), terminal, 13)["payload"]
        self.assertEqual(payload["status"], "invalid")
        self.assertEqual(payload["quotes"][0]["reason_code"], "future_timestamp")
        self.assertIsNone(payload["quotes"][0]["bid"])

    def test_stale_quote_keeps_worker_connection_and_reconciliation_running(self):
        class StopAfterOnePoll:
            def is_set(self):
                return False

            def wait(self, seconds):
                return True

        class FakeConnection:
            def __init__(self):
                self.frames = []

            def __enter__(self):
                return self

            def __exit__(self, *args):
                return False

            def sendall(self, data):
                self.frames.append(json.loads(data))

        class FakeTlsContext:
            def __init__(self, connection):
                self.connection = connection

            def wrap_socket(self, raw, server_hostname):
                return self.connection

        class FakeRaw:
            def __enter__(self):
                return self

            def __exit__(self, *args):
                return False

        import time

        terminal = FakeTerminal()
        terminal.connect_count = 0
        original_connect = terminal.connect

        def connect(cfg):
            terminal.connect_count += 1
            original_connect(cfg)

        terminal.connect = connect
        terminal.symbol_info_tick = lambda symbol: SimpleNamespace(
            symbol=symbol, bid=1.08450, ask=1.08462, time=int(time.time()) - 120
        )
        connection = FakeConnection()
        with tempfile.TemporaryDirectory() as directory:
            with patch("mt5_adapter.worker.tls_context", return_value=FakeTlsContext(connection)):
                with patch("mt5_adapter.worker.socket.create_connection", return_value=FakeRaw()):
                    run_worker(
                        config(Path(directory) / "sequence.json"),
                        terminal,
                        FakeConstants,
                        stop=StopAfterOnePoll(),
                    )

        self.assertEqual(terminal.connect_count, 1)
        self.assertFalse(terminal.is_connected)
        self.assertEqual([frame["payload"]["type"] for frame in connection.frames], [
            "heartbeat", "market_data_snapshot", "reconciliation_snapshot"
        ])
        self.assertEqual(connection.frames[1]["payload"]["status"], "stale")
        self.assertEqual(connection.frames[2]["payload"]["account_state"]["currency"], "USD")
        self.assertEqual(len(connection.frames[2]["payload"]["open_orders"]), 1)
        self.assertEqual(len(connection.frames[2]["payload"]["positions"]), 1)

    def test_snapshot_rejects_disconnect_or_account_switch(self):
        terminal = FakeTerminal()
        terminal.is_connected = False
        with self.assertRaises(AdapterError):
            build_snapshot(config("unused"), terminal, FakeConstants, 1)
        terminal.is_connected = True
        terminal.account_info = lambda: SimpleNamespace(login=999, server="Broker-Demo")
        with self.assertRaises(AdapterError):
            build_snapshot(config("unused"), terminal, FakeConstants, 1)

    def test_frame_encoder_enforces_size_and_newline(self):
        class Sink:
            def __init__(self):
                self.data = b""
            def sendall(self, data):
                self.data = data
        sink = Sink()
        send_frame(sink, build_heartbeat(config("unused"), 1))
        self.assertTrue(sink.data.endswith(b"\n"))
        self.assertLessEqual(len(sink.data), 64 * 1024)

    def test_worker_retry_log_has_utc_stage_and_safe_os_error(self):
        class FailingTerminal(FakeTerminal):
            def connect(self, cfg):
                raise OSError(111, "sensitive-host-path")

        class StopAfterRetry:
            def is_set(self):
                return False

            def wait(self, seconds):
                return True

        with tempfile.TemporaryDirectory() as directory:
            with tempfile.TemporaryFile(mode="w+t") as output:
                with patch("sys.stderr", output):
                    run_worker(
                        config(Path(directory) / "sequence.json"),
                        FailingTerminal(),
                        FakeConstants,
                        stop=StopAfterRetry(),
                    )
                output.seek(0)
                log = output.read()

        timestamp = log.split(" ", 1)[0]
        self.assertEqual(datetime.fromisoformat(timestamp.replace("Z", "+00:00")).utcoffset(), timezone.utc.utcoffset(None))
        self.assertIn("stage=provider_connect", log)
        self.assertRegex(log, r"\([A-Za-z]+Error errno=111\)")
        self.assertNotIn("sensitive-host-path", log)


if __name__ == "__main__":
    unittest.main()
