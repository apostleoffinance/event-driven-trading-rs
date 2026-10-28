import json
import os
import socket
import threading
import time
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from mt5_adapter.macos_socket_provider import (
    MacOsMql5Terminal,
    Mql5Constants,
    create_terminal_provider,
)
from mt5_adapter.worker import AdapterError, Config


def connect_retry(port):
    deadline = time.monotonic() + 3
    while True:
        try:
            return socket.create_connection(("127.0.0.1", port), timeout=1)
        except OSError:
            if time.monotonic() >= deadline:
                raise
            time.sleep(0.05)


def config(path="unused"):
    return Config(
        terminal_path="/Applications/MetaTrader 5.app",
        login=123456,
        password="",
        server="Broker-Demo",
        account_id="demo-123456",
        terminal_id="mac-demo-01",
        bridge_host="127.0.0.1",
        bridge_port=9443,
        bridge_server_name="bridge.example.test",
        ca_file="ca.pem",
        client_cert="client.pem",
        client_key="client.key",
        sequence_file=str(Path(path) / "sequence.json") if path != "unused" else "unused",
        symbols=("EURUSD.a", "GBPUSD.a"),
    )


class MacOsProviderTests(unittest.TestCase):
    def test_config_does_not_require_broker_password_for_native_mql5_provider(self):
        env = {
            "MT5_TERMINAL_PROVIDER": "mt5_adapter.macos_socket_provider:create_terminal_provider",
            "MT5_TERMINAL_PATH": "/Applications/MetaTrader 5.app",
            "MT5_LOGIN": "123456",
            "MT5_SERVER": "Broker-Demo",
            "MT5_TERMINAL_ID": "mac-demo-01",
            "MT5_BRIDGE_HOST": "127.0.0.1",
            "MT5_SYMBOLS": "EURUSD.a",
            "MT5_BRIDGE_CA_FILE": "ca.pem",
            "MT5_BRIDGE_CLIENT_CERT": "client.pem",
            "MT5_BRIDGE_CLIENT_KEY": "client.key",
        }
        with patch.dict(os.environ, env, clear=True):
            parsed = Config.from_env()
        self.assertEqual(parsed.password, "")
        self.assertEqual(parsed.symbols, ("EURUSD.a",))

    def test_factory_requires_long_token_and_loopback_only(self):
        with patch.dict(os.environ, {
            "MT5_MACOS_PROVIDER_TOKEN": "too-short",
            "MT5_MACOS_PROVIDER_PORT": "18765",
        }, clear=True):
            with self.assertRaises(AdapterError):
                create_terminal_provider(config())

        with patch.dict(os.environ, {
            "MT5_MACOS_PROVIDER_TOKEN": "x" * 48,
            "MT5_MACOS_PROVIDER_HOST": "0.0.0.0",
            "MT5_MACOS_PROVIDER_PORT": "18765",
        }, clear=True):
            with self.assertRaises(AdapterError):
                create_terminal_provider(config())

    def test_factory_returns_terminal_and_mql5_enum_constants(self):
        with patch.dict(os.environ, {
            "MT5_MACOS_PROVIDER_TOKEN": "s" * 48,
            "MT5_MACOS_PROVIDER_HOST": "127.0.0.1",
            "MT5_MACOS_PROVIDER_PORT": "18765",
            "MT5_MACOS_PROVIDER_CONNECT_TIMEOUT_SECONDS": "10",
        }, clear=True):
            terminal, constants = create_terminal_provider(config())
        self.assertIsInstance(terminal, MacOsMql5Terminal)
        self.assertIs(constants, Mql5Constants)
        self.assertEqual(constants.ORDER_TYPE_BUY_STOP_LIMIT, 6)
        self.assertEqual(constants.POSITION_TYPE_BUY, 0)
        terminal.shutdown()

    def test_mql5_ea_has_no_order_execution_api_calls(self):
        ea_path = Path(__file__).resolve().parents[1] / "mql5" / "QuantOSReadOnlyProvider.mq5"
        source = ea_path.read_text(encoding="utf-8")
        for forbidden in ("OrderSend(", "OrderSendAsync(", "OrderDelete(", "OrderModify(", "PositionClose("):
            self.assertNotIn(forbidden, source)
        for command in ('"ACCOUNT"', '"ORDERS"', '"POSITIONS"', '"SYMBOL_INFO"', '"TICK"'):
            self.assertIn(command, source)

    def test_provider_rejects_non_allowlisted_symbol_before_socket_io(self):
        terminal = MacOsMql5Terminal(config(), "127.0.0.1", 18765, "t" * 48, 5)
        with self.assertRaises(AdapterError):
            terminal._request("TICK", "XAUUSD")
        with self.assertRaises(AdapterError):
            terminal._request("EXECUTE_ORDER")
        terminal.shutdown()

    def test_connect_authenticates_loopback_peer_and_checks_account_identity(self):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as reserve:
            reserve.bind(("127.0.0.1", 0))
            port = reserve.getsockname()[1]
        token = "a" * 48
        terminal = MacOsMql5Terminal(config(), "127.0.0.1", port, token, 5)
        failures = []

        def fake_ea():
            try:
                with connect_retry(port) as peer:
                    peer.sendall(f"HELLO\t{token}\n".encode())
                    reader = peer.makefile("rb")
                    while True:
                        request = reader.readline().decode().strip()
                        if not request:
                            break
                        self.assertEqual(request, "PING")
                        peer.sendall((json.dumps({
                            "ok": True,
                            "data": {"connected": True, "login": 123456, "server": "Broker-Demo"},
                        }) + "\n").encode())
            except Exception as exc:  # propagate thread assertion failures
                failures.append(exc)

        thread = threading.Thread(target=fake_ea, daemon=True)
        thread.start()
        terminal.connect(config())
        self.assertTrue(terminal.connected())
        terminal.shutdown()
        thread.join(timeout=2)
        if failures:
            raise failures[0]

    def test_mismatched_login_fails_closed(self):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as reserve:
            reserve.bind(("127.0.0.1", 0))
            port = reserve.getsockname()[1]
        token = "b" * 48
        terminal = MacOsMql5Terminal(config(), "127.0.0.1", port, token, 5)
        failures = []

        def fake_ea():
            try:
                with connect_retry(port) as peer:
                    peer.sendall(f"HELLO\t{token}\n".encode())
                    reader = peer.makefile("rb")
                    reader.readline()
                    peer.sendall((json.dumps({
                        "ok": True,
                        "data": {"connected": True, "login": 999999, "server": "Broker-Demo"},
                    }) + "\n").encode())
            except Exception as exc:
                failures.append(exc)

        thread = threading.Thread(target=fake_ea, daemon=True)
        thread.start()
        with self.assertRaises(AdapterError):
            terminal.connect(config())
        terminal.shutdown()
        thread.join(timeout=2)
        if failures:
            raise failures[0]

    def test_provider_maps_account_orders_positions_symbol_and_tick(self):
        terminal = MacOsMql5Terminal(config(), "127.0.0.1", 18765, "c" * 48, 5)
        left, right = socket.socketpair()
        terminal._connection = left
        terminal._reader = left.makefile("rb")
        terminal._closed = False
        seen = []

        responses = {
            "PING": {"connected": True, "login": 123456, "server": "Broker-Demo"},
            "ACCOUNT": {
                "login": 123456, "server": "Broker-Demo", "currency": "USD",
                "balance": 10000.0, "equity": 9990.0, "margin": 100.0, "margin_free": 9890.0,
            },
            "ORDERS": [{"ticket": 12, "type": 2, "symbol": "EURUSD.a", "volume_current": 0.1, "state": 1}],
            "POSITIONS": [{"ticket": 13, "type": 0, "symbol": "EURUSD.a", "volume": 0.1, "price_open": 1.08, "profit": -2.0}],
            "SELECT_SYMBOL\tEURUSD.a": True,
            "SYMBOL_INFO\tEURUSD.a": {
                "name": "EURUSD.a", "currency_base": "EUR", "currency_profit": "USD",
                "digits": 5, "point": 0.00001, "trade_tick_size": 0.00001,
                "trade_tick_value": 1.0, "trade_contract_size": 100000.0,
                "volume_min": 0.01, "volume_max": 100.0, "volume_step": 0.01, "trade_mode": 4,
            },
            "TICK\tEURUSD.a": {"symbol": "EURUSD.a", "bid": 1.0845, "ask": 1.08462, "time": 1793000000, "server_time": 1793000001},
        }

        def fake_ea():
            reader = right.makefile("rb")
            while True:
                line = reader.readline()
                if not line:
                    break
                command = line.decode().strip()
                seen.append(command)
                data = responses.get(command)
                response = {"ok": command in responses, "data": data}
                right.sendall((json.dumps(response) + "\n").encode())

        thread = threading.Thread(target=fake_ea, daemon=True)
        thread.start()
        try:
            self.assertTrue(terminal.connected())
            account = terminal.account_info()
            self.assertEqual(account.currency, "USD")
            self.assertEqual(terminal.orders()[0].ticket, 12)
            self.assertEqual(terminal.positions()[0].profit, -2.0)
            self.assertTrue(terminal.select_symbol("EURUSD.a"))
            self.assertEqual(terminal.symbol_info("EURUSD.a").volume_step, 0.01)
            self.assertEqual(terminal.symbol_info_tick("EURUSD.a").ask, 1.08462)
            self.assertNotIn("EXECUTE_ORDER", seen)
        finally:
            terminal.shutdown()
            right.close()
            thread.join(timeout=2)


if __name__ == "__main__":
    unittest.main()
