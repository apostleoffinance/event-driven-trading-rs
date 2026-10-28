"""Read-only provider for the MetaTrader 5 macOS terminal via an MQL5 EA.

The terminal-side Expert Advisor initiates a loopback TCP connection and serves
strict request/response commands. No order placement, modification, or cancel
command exists in this protocol. This provider is not considered release
conformance until it is exercised against a real demo terminal.
"""
from __future__ import annotations

import hmac
import json
import os
import socket
import threading
from types import SimpleNamespace
from typing import Any

from .worker import AdapterError, Config


class Mql5Constants:
    """MQL5 order/position enum values consumed by the existing read-only mapper."""

    ORDER_TYPE_BUY = 0
    ORDER_TYPE_SELL = 1
    ORDER_TYPE_BUY_LIMIT = 2
    ORDER_TYPE_SELL_LIMIT = 3
    ORDER_TYPE_BUY_STOP = 4
    ORDER_TYPE_SELL_STOP = 5
    ORDER_TYPE_BUY_STOP_LIMIT = 6
    ORDER_TYPE_SELL_STOP_LIMIT = 7
    POSITION_TYPE_BUY = 0


_MAX_LINE_BYTES = 64 * 1024
_DEFAULT_PORT = 8765


def _positive_int_env(name: str, default: int, minimum: int, maximum: int) -> int:
    raw = os.environ.get(name, str(default))
    try:
        value = int(raw)
    except ValueError as exc:
        raise AdapterError(f"{name} must be an integer") from exc
    if not minimum <= value <= maximum:
        raise AdapterError(f"{name} is outside its allowed range")
    return value


def _required_token() -> str:
    token = os.environ.get("MT5_MACOS_PROVIDER_TOKEN", "")
    if len(token) < 32 or token.strip() != token or any(ch.isspace() for ch in token):
        raise AdapterError("MT5_MACOS_PROVIDER_TOKEN must be a non-whitespace secret of at least 32 characters")
    return token


def create_terminal_provider(config: Config) -> tuple["MacOsMql5Terminal", type[Mql5Constants]]:
    """Factory loaded by MT5_TERMINAL_PROVIDER=module:function."""
    host = os.environ.get("MT5_MACOS_PROVIDER_HOST", "127.0.0.1")
    if host != "127.0.0.1":
        raise AdapterError("macOS MQL5 provider must bind to loopback only")
    port = _positive_int_env("MT5_MACOS_PROVIDER_PORT", _DEFAULT_PORT, 1024, 65535)
    token = _required_token()
    timeout = _positive_int_env("MT5_MACOS_PROVIDER_CONNECT_TIMEOUT_SECONDS", 60, 5, 300)
    return MacOsMql5Terminal(config, host, port, token, timeout), Mql5Constants


class MacOsMql5Terminal:
    """Synchronous read-only client for the terminal-attached MQL5 EA."""

    source = "mt5_macos_mql5_socket_v1"

    def __init__(self, config: Config, host: str, port: int, token: str, connect_timeout: int) -> None:
        self.config = config
        self.host = host
        self.port = port
        self._token = token
        self._connect_timeout = connect_timeout
        self._listener: socket.socket | None = None
        self._connection: socket.socket | None = None
        self._reader: Any = None
        self._lock = threading.RLock()
        self._closed = False
        self._allowed_symbols = frozenset(config.symbols)

    def connect(self, config: Config) -> None:
        if config.login <= 0 or not config.server:
            raise AdapterError("configured MT5 account identity is invalid")
        self.shutdown()
        self._closed = False
        listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            listener.bind(("127.0.0.1", self.port))
            listener.listen(1)
            listener.settimeout(self._connect_timeout)
            self._listener = listener
            try:
                connection, peer = listener.accept()
            except (OSError, TimeoutError) as exc:
                raise AdapterError("timed out waiting for the MT5 MQL5 provider EA on loopback") from exc
            if not peer or peer[0] != "127.0.0.1":
                connection.close()
                raise AdapterError("MQL5 provider peer is not loopback")
            connection.settimeout(5.0)
            reader = connection.makefile("rb")
            hello = self._read_line(reader)
            expected = f"HELLO\t{self._token}".encode("utf-8")
            if not hmac.compare_digest(hello, expected):
                connection.close()
                reader.close()
                raise AdapterError("MQL5 provider handshake was rejected")
            self._connection = connection
            self._reader = reader
            state = self._request("PING")
            if not isinstance(state, dict) or state.get("connected") is not True:
                raise AdapterError("MQL5 provider reports the terminal disconnected")
            if int(state.get("login", -1)) != config.login or str(state.get("server", "")) != config.server:
                raise AdapterError("connected MT5 account/server does not match configured identity")
        except Exception:
            self.shutdown()
            raise
        finally:
            if self._listener is not None:
                try:
                    self._listener.settimeout(None)
                except (OSError, ValueError):
                    pass

    @staticmethod
    def _read_line(reader: Any) -> bytes:
        line = reader.readline(_MAX_LINE_BYTES + 1)
        if not line or len(line) > _MAX_LINE_BYTES or not line.endswith(b"\n"):
            raise AdapterError("MQL5 provider sent an empty, oversized, or unterminated frame")
        return line[:-1].removesuffix(b"\r")

    def _request(self, command: str, argument: str | None = None) -> Any:
        if command not in {"PING", "ACCOUNT", "ORDERS", "POSITIONS", "SYMBOL_INFO", "TICK", "SELECT_SYMBOL"}:
            raise AdapterError("unsupported MQL5 provider command")
        if argument is not None:
            if command not in {"SYMBOL_INFO", "TICK", "SELECT_SYMBOL"}:
                raise AdapterError("MQL5 provider command does not accept an argument")
            if argument not in self._allowed_symbols or any(ch in argument for ch in "\r\n\t"):
                raise AdapterError("requested symbol is not in the configured allowlist")
        connection, reader = self._connection, self._reader
        if connection is None or reader is None or self._closed:
            raise AdapterError("MQL5 provider is not connected")
        request = command if argument is None else f"{command}\t{argument}"
        encoded = request.encode("utf-8") + b"\n"
        if len(encoded) > 1024:
            raise AdapterError("MQL5 provider request is oversized")
        with self._lock:
            try:
                connection.sendall(encoded)
                raw = self._read_line(reader)
            except (OSError, TimeoutError, ValueError) as exc:
                raise AdapterError("MQL5 provider connection failed during a read-only request") from exc
        try:
            response = json.loads(raw.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as exc:
            raise AdapterError("MQL5 provider returned invalid JSON") from exc
        if not isinstance(response, dict) or response.get("ok") is not True or "data" not in response:
            # Do not include provider-supplied error text in logs/exceptions.
            raise AdapterError("MQL5 provider rejected or failed a read-only request")
        return response["data"]

    def connected(self) -> bool:
        try:
            state = self._request("PING")
            return (
                isinstance(state, dict)
                and state.get("connected") is True
                and int(state.get("login", -1)) == self.config.login
                and str(state.get("server", "")) == self.config.server
            )
        except (AdapterError, TypeError, ValueError):
            return False

    def account_info(self) -> Any:
        data = self._request("ACCOUNT")
        required = ("login", "server", "currency", "balance", "equity", "margin", "margin_free")
        if not isinstance(data, dict) or any(key not in data for key in required):
            raise AdapterError("MQL5 provider account response is incomplete")
        return SimpleNamespace(**data)

    def orders(self) -> list[Any]:
        data = self._request("ORDERS")
        if not isinstance(data, list) or any(not isinstance(item, dict) for item in data):
            raise AdapterError("MQL5 provider order response is invalid")
        required = ("ticket", "type", "symbol", "volume_current", "state")
        if any(any(key not in item for key in required) for item in data):
            raise AdapterError("MQL5 provider order response is incomplete")
        return [SimpleNamespace(**item) for item in data]

    def positions(self) -> list[Any]:
        data = self._request("POSITIONS")
        if not isinstance(data, list) or any(not isinstance(item, dict) for item in data):
            raise AdapterError("MQL5 provider position response is invalid")
        required = ("ticket", "type", "symbol", "volume", "price_open", "profit")
        if any(any(key not in item for key in required) for item in data):
            raise AdapterError("MQL5 provider position response is incomplete")
        return [SimpleNamespace(**item) for item in data]

    def select_symbol(self, symbol: str) -> bool:
        if symbol not in self._allowed_symbols:
            return False
        result = self._request("SELECT_SYMBOL", symbol)
        return result is True

    def symbol_info(self, symbol: str) -> Any:
        data = self._request("SYMBOL_INFO", symbol)
        required = (
            "name", "currency_base", "currency_profit", "digits", "point",
            "trade_tick_size", "trade_tick_value", "trade_contract_size",
            "volume_min", "volume_max", "volume_step", "trade_mode",
        )
        if not isinstance(data, dict) or any(key not in data for key in required):
            raise AdapterError("MQL5 provider symbol metadata is incomplete")
        return SimpleNamespace(**data)

    def symbol_info_tick(self, symbol: str) -> Any:
        data = self._request("TICK", symbol)
        required = ("symbol", "bid", "ask", "time", "server_time")
        if not isinstance(data, dict) or any(key not in data for key in required):
            raise AdapterError("MQL5 provider tick response is incomplete")
        return SimpleNamespace(**data)

    def shutdown(self) -> None:
        self._closed = True
        reader, connection, listener = self._reader, self._connection, self._listener
        self._reader = self._connection = self._listener = None
        for item in (reader, connection, listener):
            if item is not None:
                try:
                    item.close()
                except (OSError, ValueError):
                    pass
