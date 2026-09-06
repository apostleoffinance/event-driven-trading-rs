"""Unit tests for Python mean-reversion strategy."""

from decimal import Decimal
import unittest

from trading_strategies.mean_reversion import MeanReversionStrategy, run_prices


class MeanReversionTests(unittest.TestCase):
    def test_warmup_then_buy_below_mean(self) -> None:
        strategy = MeanReversionStrategy(
            strategy_id="btc-mean-reversion",
            strategy_version="v1",
            deployment_id="deployment-001",
            instrument_id="BTCUSDT",
            threshold=Decimal("0.05"),
            window_size=3,
            stop_distance_pct=Decimal("0.02"),
        )
        intents = run_prices(
            [Decimal("100"), Decimal("100"), Decimal("100"), Decimal("90")],
            strategy,
        )
        self.assertEqual(len(intents), 1)
        self.assertEqual(intents[0].side, "Buy")
        self.assertEqual(intents[0].entry_price, Decimal("90"))
        self.assertIsNotNone(intents[0].stop_loss)
        # No account sizing in strategy output.
        self.assertIsNone(intents[0].target_quantity)

    def test_sell_above_mean(self) -> None:
        strategy = MeanReversionStrategy(
            strategy_id="btc-mean-reversion",
            strategy_version="v1",
            deployment_id="deployment-001",
            instrument_id="BTCUSDT",
            threshold=Decimal("0.05"),
            window_size=3,
            stop_distance_pct=None,
        )
        intents = run_prices(
            [Decimal("100"), Decimal("100"), Decimal("100"), Decimal("110")],
            strategy,
        )
        self.assertEqual(len(intents), 1)
        self.assertEqual(intents[0].side, "Sell")


if __name__ == "__main__":
    unittest.main()
