import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

from mt5_adapter.worker import AdapterError, Config, SequenceStore, build_heartbeat, build_market_data_snapshot, build_snapshot, decimal_text, send_frame


class FakeTerminal:
    def __init__(self):
        self.is_connected = True

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
        self.assertEqual(payload["account_currency"], "USD")
        self.assertEqual(payload["instruments"][0]["symbol"], "EURUSD.a")
        self.assertEqual(payload["instruments"][0]["tick_size"], "0.00001")
        self.assertEqual(payload["instruments"][0]["volume_step"], "0.01")
        self.assertEqual(payload["quotes"][0]["bid"], "1.0845")
        self.assertEqual(payload["quotes"][0]["ask"], "1.08462")
        self.assertTrue(payload["quotes"][0]["tick_time"].endswith("Z"))

    def test_market_data_snapshot_fails_closed_when_symbol_or_quote_unavailable(self):
        terminal = FakeTerminal()
        terminal.select_symbol = lambda symbol: False
        with self.assertRaises(AdapterError):
            build_market_data_snapshot(config("unused"), terminal, 13)
        terminal = FakeTerminal()
        terminal.symbol_info_tick = lambda symbol: None
        with self.assertRaises(AdapterError):
            build_market_data_snapshot(config("unused"), terminal, 13)

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


if __name__ == "__main__":
    unittest.main()
