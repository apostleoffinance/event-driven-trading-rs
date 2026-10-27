"""Read-only Windows MT5 worker speaking the authenticated Rust bridge protocol.

This module intentionally has no order submission, order modification, or cancellation
code paths. Financial values on the wire are decimal strings.
"""
from __future__ import annotations

import json
import os
import socket
import ssl
import sys
import time
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any, Protocol


class AdapterError(RuntimeError):
    """Terminal or bridge state is unavailable or inconsistent."""


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def decimal_text(value: Any, field: str) -> str:
    """Convert an MT5 numeric value to a plain decimal lexeme without exponent notation."""
    try:
        number = Decimal(str(value))
    except (InvalidOperation, ValueError, TypeError) as exc:
        raise AdapterError(f"MT5 returned invalid numeric field: {field}") from exc
    if not number.is_finite():
        raise AdapterError(f"MT5 returned non-finite numeric field: {field}")
    return format(number, "f")


def required_text(value: Any, field: str) -> str:
    if value is None:
        raise AdapterError(f"MT5 omitted required field: {field}")
    result = str(value)
    if not result.strip() or result.strip() != result:
        raise AdapterError(f"MT5 returned invalid required field: {field}")
    return result


@dataclass(frozen=True)
class Config:
    terminal_path: str
    login: int
    password: str
    server: str
    account_id: str
    terminal_id: str
    bridge_host: str
    bridge_port: int
    bridge_server_name: str
    ca_file: str
    client_cert: str
    client_key: str
    sequence_file: str
    poll_seconds: float = 10.0

    @classmethod
    def from_env(cls) -> "Config":
        def need(name: str) -> str:
            value = os.environ.get(name, "")
            if not value or value.strip() != value:
                raise AdapterError(f"required environment variable is missing or invalid: {name}")
            return value

        try:
            login = int(need("MT5_LOGIN"))
            port = int(os.environ.get("MT5_BRIDGE_PORT", "9443"))
            poll = float(os.environ.get("MT5_POLL_INTERVAL_SECONDS", "10"))
        except ValueError as exc:
            raise AdapterError("MT5 login, bridge port, or poll interval has invalid format") from exc
        if login <= 0 or not 1 <= port <= 65535 or not 1 <= poll <= 300:
            raise AdapterError("login, bridge port, or poll interval is outside its allowed range")
        account_id = os.environ.get("MT5_ACCOUNT_ID", str(login))
        if not account_id or account_id.strip() != account_id:
            raise AdapterError("MT5_ACCOUNT_ID is invalid")
        bridge_host = need("MT5_BRIDGE_HOST")
        return cls(
            terminal_path=need("MT5_TERMINAL_PATH"),
            login=login,
            password=need("MT5_PASSWORD"),
            server=need("MT5_SERVER"),
            account_id=account_id,
            terminal_id=need("MT5_TERMINAL_ID"),
            bridge_host=bridge_host,
            bridge_port=port,
            bridge_server_name=os.environ.get("MT5_BRIDGE_SERVER_NAME", bridge_host),
            ca_file=need("MT5_BRIDGE_CA_FILE"),
            client_cert=need("MT5_BRIDGE_CLIENT_CERT"),
            client_key=need("MT5_BRIDGE_CLIENT_KEY"),
            sequence_file=os.environ.get(
                "MT5_SEQUENCE_FILE",
                str(Path(os.environ.get("PROGRAMDATA", ".")) / "QuantOS" / "mt5-sequence.json"),
            ),
            poll_seconds=poll,
        )


class Terminal(Protocol):
    def connect(self, config: Config) -> None: ...
    def connected(self) -> bool: ...
    def account_info(self) -> Any: ...
    def orders(self) -> list[Any]: ...
    def positions(self) -> list[Any]: ...
    def shutdown(self) -> None: ...


class MetaTraderTerminal:
    """Thin wrapper over the official MetaTrader5 Python package."""

    def __init__(self) -> None:
        if sys.platform != "win32":
            raise AdapterError("MetaTrader5 Python adapter must run on Windows beside terminal64.exe")
        try:
            import MetaTrader5 as mt5
        except ImportError as exc:
            raise AdapterError("install the Windows-only terminal extra to use MetaTrader5") from exc
        self.mt5 = mt5

    def connect(self, config: Config) -> None:
        ok = self.mt5.initialize(
            path=config.terminal_path,
            login=config.login,
            password=config.password,
            server=config.server,
        )
        if not ok:
            raise AdapterError(f"MetaTrader5 initialize failed (code={self.mt5.last_error()[0]})")
        if not self.connected():
            raise AdapterError("MT5 terminal is not connected to its trade server")
        account = self.account_info()
        if account is None or int(account.login) != config.login or str(account.server) != config.server:
            raise AdapterError("connected MT5 account/server does not match configured identity")

    def connected(self) -> bool:
        info = self.mt5.terminal_info()
        return bool(info is not None and info.connected)

    def account_info(self) -> Any:
        result = self.mt5.account_info()
        if result is None:
            raise AdapterError(f"MT5 account_info failed (code={self.mt5.last_error()[0]})")
        return result

    def orders(self) -> list[Any]:
        result = self.mt5.orders_get()
        if result is None:
            raise AdapterError(f"MT5 orders_get failed (code={self.mt5.last_error()[0]})")
        return list(result)

    def positions(self) -> list[Any]:
        result = self.mt5.positions_get()
        if result is None:
            raise AdapterError(f"MT5 positions_get failed (code={self.mt5.last_error()[0]})")
        return list(result)

    def shutdown(self) -> None:
        self.mt5.shutdown()


class SequenceStore:
    """Persist a monotonic stream sequence across worker restarts."""

    def __init__(self, path: str) -> None:
        self.path = Path(path)

    def next(self) -> int:
        try:
            current = json.loads(self.path.read_text(encoding="utf-8")).get("sequence", -1)
        except FileNotFoundError:
            current = -1
        except (OSError, ValueError, AttributeError, TypeError) as exc:
            raise AdapterError("sequence checkpoint is unreadable; refusing to start") from exc
        if not isinstance(current, int) or current < -1 or current >= 2**63 - 1:
            raise AdapterError("sequence checkpoint is invalid or exhausted")
        sequence = current + 1
        self.path.parent.mkdir(parents=True, exist_ok=True)
        temporary = self.path.with_suffix(self.path.suffix + ".tmp")
        temporary.write_text(json.dumps({"sequence": sequence}), encoding="utf-8")
        os.replace(temporary, self.path)
        return sequence


def _order_type(mt5: Any, value: Any) -> str:
    mapping = {
        mt5.ORDER_TYPE_BUY: "market_buy",
        mt5.ORDER_TYPE_SELL: "market_sell",
        mt5.ORDER_TYPE_BUY_LIMIT: "buy_limit",
        mt5.ORDER_TYPE_SELL_LIMIT: "sell_limit",
        mt5.ORDER_TYPE_BUY_STOP: "buy_stop",
        mt5.ORDER_TYPE_SELL_STOP: "sell_stop",
        mt5.ORDER_TYPE_BUY_STOP_LIMIT: "buy_stop_limit",
        mt5.ORDER_TYPE_SELL_STOP_LIMIT: "sell_stop_limit",
    }
    return mapping.get(value, f"unknown_{value}")


def build_snapshot(config: Config, terminal: Terminal, mt5_constants: Any, sequence: int) -> dict[str, Any]:
    if not terminal.connected():
        raise AdapterError("MT5 terminal disconnected; refusing to emit a reconciliation snapshot")
    account = terminal.account_info()
    if account is None or int(account.login) != config.login or str(account.server) != config.server:
        raise AdapterError("MT5 account identity changed; refusing to emit a reconciliation snapshot")

    orders = []
    for order in terminal.orders():
        order_type = _order_type(mt5_constants, order.type)
        side = "buy" if "buy" in order_type else "sell" if "sell" in order_type else "unknown"
        orders.append({
            "venue_order_id": required_text(order.ticket, "order ticket"),
            "client_order_id": None,
            "instrument_id": required_text(order.symbol, "order symbol"),
            "side": side,
            "order_type": order_type,
            "quantity": decimal_text(order.volume_current, "order volume"),
            "status": str(getattr(order, "state", "unknown")),
        })

    positions = []
    for position in terminal.positions():
        positions.append({
            "venue_position_id": required_text(position.ticket, "position ticket"),
            "instrument_id": required_text(position.symbol, "position symbol"),
            "side": "buy" if position.type == mt5_constants.POSITION_TYPE_BUY else "sell",
            "quantity": decimal_text(position.volume, "position volume"),
            "average_price": decimal_text(position.price_open, "position open price"),
            "unrealized_pnl": decimal_text(position.profit, "position profit"),
        })

    captured_at = utc_now()
    return {
        "schema_version": 1,
        "message_id": str(uuid.uuid4()),
        "correlation_id": str(uuid.uuid4()),
        "account_id": config.account_id,
        "sent_at": captured_at,
        "payload": {
            "type": "reconciliation_snapshot",
            "terminal_id": config.terminal_id,
            "server": config.server,
            "source": "mt5_python_readonly_v1",
            "sequence": sequence,
            "captured_at": captured_at,
            "account_state": {
                "currency": required_text(account.currency, "account currency"),
                "balance": decimal_text(account.balance, "account balance"),
                "equity": decimal_text(account.equity, "account equity"),
                "margin_used": decimal_text(account.margin, "account margin"),
                "free_margin": decimal_text(account.margin_free, "account free margin"),
            },
            "open_orders": orders,
            "positions": positions,
        },
    }


def build_heartbeat(config: Config, sequence: int) -> dict[str, Any]:
    now = utc_now()
    return {
        "schema_version": 1,
        "message_id": str(uuid.uuid4()),
        "correlation_id": str(uuid.uuid4()),
        "account_id": config.account_id,
        "sent_at": now,
        "payload": {
            "type": "heartbeat",
            "sequence": sequence,
            "terminal_id": config.terminal_id,
            "server": config.server,
            "terminal_time": now,
            "connection": "connected",
            "capabilities": {
                "market_orders": False,
                "limit_orders": False,
                "stop_orders": False,
                "cancel_requests": False,
                "order_lookup": True,
                "idempotent_client_order_id": False,
                "sequence_numbers": True,
            },
        },
    }


def tls_context(config: Config) -> ssl.SSLContext:
    context = ssl.create_default_context(ssl.Purpose.SERVER_AUTH, cafile=config.ca_file)
    context.minimum_version = ssl.TLSVersion.TLSv1_3
    context.maximum_version = ssl.TLSVersion.TLSv1_3
    context.check_hostname = True
    context.verify_mode = ssl.CERT_REQUIRED
    context.load_cert_chain(certfile=config.client_cert, keyfile=config.client_key)
    return context


def send_frame(connection: Any, frame: dict[str, Any]) -> None:
    encoded = json.dumps(frame, separators=(",", ":"), ensure_ascii=False, allow_nan=False).encode("utf-8") + b"\n"
    if len(encoded) > 64 * 1024:
        raise AdapterError("outbound MT5 bridge frame exceeds the 64 KiB contract limit")
    connection.sendall(encoded)


def run_worker(config: Config, terminal: Terminal, mt5_constants: Any, stop: Any = None) -> None:
    """Connect and stream heartbeat/snapshot pairs; never submit or cancel orders."""
    terminal.connect(config)
    sequence_store = SequenceStore(config.sequence_file)
    backoff = 1.0
    try:
        while stop is None or not stop.is_set():
            try:
                if not terminal.connected():
                    terminal.shutdown()
                    terminal.connect(config)
                context = tls_context(config)
                with socket.create_connection((config.bridge_host, config.bridge_port), timeout=10) as raw:
                    with context.wrap_socket(raw, server_hostname=config.bridge_server_name) as connection:
                        while stop is None or not stop.is_set():
                            sequence = sequence_store.next()
                            send_frame(connection, build_heartbeat(config, sequence))
                            send_frame(connection, build_snapshot(config, terminal, mt5_constants, sequence))
                            backoff = 1.0
                            if stop is not None and stop.wait(config.poll_seconds):
                                return
                            if stop is None:
                                time.sleep(config.poll_seconds)
            except (OSError, ssl.SSLError, AdapterError):
                # Never log credentials, terminal responses, or full frame contents.
                if stop is not None and stop.wait(backoff):
                    return
                if stop is None:
                    time.sleep(backoff)
                backoff = min(backoff * 2, 30.0)
    finally:
        terminal.shutdown()


def main() -> None:
    config = Config.from_env()
    terminal = MetaTraderTerminal()
    try:
        import MetaTrader5 as mt5
    except ImportError as exc:
        raise SystemExit("Install with: pip install -e '.[terminal]' on Windows") from exc
    run_worker(config, terminal, mt5)


if __name__ == "__main__":
    main()
