"""Mean-reversion strategy — Python side of the trading stack.

Emits TradeIntent only. Risk sizing and execution live in Rust.
"""

from __future__ import annotations

import argparse
import sys
from datetime import datetime, timezone
from decimal import Decimal
from typing import Iterable, Optional

from trading_strategies.intent import TradeIntent


class MeanReversionStrategy:
    def __init__(
        self,
        *,
        strategy_id: str,
        strategy_version: str,
        deployment_id: str,
        instrument_id: str,
        threshold: Decimal,
        window_size: int,
        stop_distance_pct: Optional[Decimal] = None,
    ) -> None:
        if threshold <= 0 or threshold >= 1:
            raise ValueError("threshold must be in (0, 1)")
        if window_size < 1:
            raise ValueError("window_size must be >= 1")
        if stop_distance_pct is not None and (
            stop_distance_pct <= 0 or stop_distance_pct >= 1
        ):
            raise ValueError("stop_distance_pct must be in (0, 1) when set")

        self.strategy_id = strategy_id
        self.strategy_version = strategy_version
        self.deployment_id = deployment_id
        self.instrument_id = instrument_id
        self.threshold = threshold
        self.window_size = window_size
        self.stop_distance_pct = stop_distance_pct
        self._prices: list[Decimal] = []

    @property
    def warmed_up(self) -> bool:
        return len(self._prices) >= self.window_size

    def _mean(self) -> Decimal:
        return sum(self._prices, Decimal("0")) / Decimal(len(self._prices))

    def _push(self, price: Decimal) -> None:
        self._prices.append(price)
        if len(self._prices) > self.window_size:
            self._prices.pop(0)

    def _stop(self, entry: Decimal, side: str) -> Optional[Decimal]:
        if self.stop_distance_pct is None:
            return None
        distance = entry * self.stop_distance_pct
        stop = entry - distance if side == "Buy" else entry + distance
        if stop <= 0:
            raise ValueError("computed stop_loss must be positive")
        return stop

    def _confidence(self, deviation: Decimal) -> Decimal:
        excess = max(deviation - self.threshold, Decimal("0"))
        scaled = min(excess / self.threshold, Decimal("1"))
        return scaled.quantize(Decimal("0.0001"))

    def on_price(self, instrument_id: str, price: Decimal) -> list[TradeIntent]:
        if instrument_id != self.instrument_id:
            return []

        if not self.warmed_up:
            self._push(price)
            return []

        mean = self._mean()
        deviation = abs(price - mean) / mean
        intents: list[TradeIntent] = []

        if deviation > self.threshold:
            if price < mean:
                side = "Buy"
            elif price > mean:
                side = "Sell"
            else:
                self._push(price)
                return []

            confidence = self._confidence(deviation)
            intent = TradeIntent(
                id=TradeIntent.new_id(),
                strategy_id=self.strategy_id,
                strategy_version=self.strategy_version,
                deployment_id=self.deployment_id,
                instrument_id=instrument_id,
                side=side,
                timestamp=datetime.now(timezone.utc),
                entry_price=price,
                stop_loss=self._stop(price, side),
                confidence=confidence if confidence > 0 else None,
            )
            intents.append(intent)

        self._push(price)
        return intents


def run_prices(prices: Iterable[Decimal], strategy: MeanReversionStrategy) -> list[TradeIntent]:
    out: list[TradeIntent] = []
    for price in prices:
        out.extend(strategy.on_price(strategy.instrument_id, price))
    return out


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description="Emit TradeIntent NDJSON for Rust intent-bridge"
    )
    parser.add_argument(
        "--prices",
        required=True,
        help="Comma-separated prices, e.g. 100,100,100,90",
    )
    parser.add_argument("--instrument", default="BTCUSDT")
    parser.add_argument("--strategy-id", default="btc-mean-reversion")
    parser.add_argument("--strategy-version", default="v1")
    parser.add_argument("--deployment-id", default="deployment-001")
    parser.add_argument("--threshold", default="0.05")
    parser.add_argument("--window", type=int, default=3)
    parser.add_argument("--stop-pct", default="0.02")
    args = parser.parse_args(argv)

    strategy = MeanReversionStrategy(
        strategy_id=args.strategy_id,
        strategy_version=args.strategy_version,
        deployment_id=args.deployment_id,
        instrument_id=args.instrument,
        threshold=Decimal(args.threshold),
        window_size=args.window,
        stop_distance_pct=Decimal(args.stop_pct) if args.stop_pct else None,
    )

    prices = [Decimal(p.strip()) for p in args.prices.split(",") if p.strip()]
    for intent in run_prices(prices, strategy):
        sys.stdout.write(intent.to_json() + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
