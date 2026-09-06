# Python strategies

Strategies run in **Python** and emit `TradeIntent` NDJSON.  
Risk, OMS, venues, persistence, and reconciliation run in **Rust**.

```text
Python strategy                     Rust intent-bridge
───────────────                     ──────────────────
Market data / signals
        ↓
TradeIntent JSON (NDJSON stdout)
        ↓  pipe / file
                              → parse → risk → OMS → venue → fills
```

## Install

```bash
cd strategies/python
python3 -m venv .venv
source .venv/bin/activate
pip install -e .
python -m unittest discover -s tests -v
```

## Emit intents

```bash
python -m trading_strategies.mean_reversion \
  --prices 100,100,100,90 \
  --instrument BTCUSDT
```

## Pipe into Rust

```bash
python -m trading_strategies.mean_reversion --prices 100,100,100,90 \
  | cargo run -p intent-bridge -- --venue prop
```

## Contract

See `docs/architecture/STRATEGY_BRIDGE.md` and `docs/contracts/trade_intent.schema.json`.
