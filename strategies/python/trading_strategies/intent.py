"""TradeIntent wire format shared with Rust `intent-bridge`."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from decimal import Decimal
from typing import Any, Optional
import json
import uuid


def _dec_str(value: Optional[Decimal]) -> Optional[str]:
    if value is None:
        return None
    # Normalize without scientific notation.
    return format(value, "f")


@dataclass
class TradeIntent:
    """Strategy output. Must not include account sizing or order submission."""

    id: str
    strategy_id: str
    strategy_version: str
    deployment_id: str
    instrument_id: str
    side: str  # "Buy" | "Sell"
    timestamp: datetime
    target_quantity: Optional[Decimal] = None
    target_notional: Optional[Decimal] = None
    entry_price: Optional[Decimal] = None
    stop_loss: Optional[Decimal] = None
    take_profit: Optional[Decimal] = None
    confidence: Optional[Decimal] = None

    def __post_init__(self) -> None:
        if self.side not in ("Buy", "Sell"):
            raise ValueError(f"side must be Buy or Sell, got {self.side!r}")
        if self.confidence is not None and not (Decimal("0") <= self.confidence <= Decimal("1")):
            raise ValueError("confidence must be in [0, 1]")

    @staticmethod
    def new_id(prefix: str = "ti") -> str:
        return f"{prefix}-{uuid.uuid4().hex[:12]}"

    def to_wire(self) -> dict[str, Any]:
        ts = self.timestamp
        if ts.tzinfo is None:
            ts = ts.replace(tzinfo=timezone.utc)
        payload = {
            "id": self.id,
            "strategy_id": self.strategy_id,
            "strategy_version": self.strategy_version,
            "deployment_id": self.deployment_id,
            "instrument_id": self.instrument_id,
            "side": self.side,
            "timestamp": ts.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "target_quantity": _dec_str(self.target_quantity),
            "target_notional": _dec_str(self.target_notional),
            "entry_price": _dec_str(self.entry_price),
            "stop_loss": _dec_str(self.stop_loss),
            "take_profit": _dec_str(self.take_profit),
            "confidence": _dec_str(self.confidence),
        }
        # Drop null optional fields for a cleaner wire (Rust defaults handle absence).
        return {k: v for k, v in payload.items() if v is not None}

    def to_json(self) -> str:
        return json.dumps(self.to_wire(), separators=(",", ":"))
