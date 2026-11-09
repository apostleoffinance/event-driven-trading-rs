# Account Risk Policies and FX Conversion

## Purpose

Account risk policies are per Journal account. They support personal risk plans and prop-firm/challenge/funded-account rules. They are used by the calculator and Journal risk assessment only. They do **not** submit orders or block MT5 orders; order execution remains disabled in Quant OS.

Firm rules change and differ by program. Users should enter and verify the current official rules for their own firm/account. EdgeNest does not hardcode or claim official prop-firm presets.

## Policy configuration

The `journal_account_risk_policies` table stores a versioned JSON policy keyed by `(owner_id, account_id)`, with a composite foreign key to the owned Journal account.

Policy fields include:

- Risk appetite: conservative (0.25%), balanced (0.5%), aggressive (1%), or custom.
- Risk per trade (%) and an optional absolute cap in account currency.
- Daily loss limit (%) and basis: initial balance, start-of-day balance, or start-of-day equity.
- Daily risk budget (%) of equity.
- Total drawdown (%) and basis: static initial balance or trailing high-water mark.
- Daily trade count, simultaneous position count, total open risk, lot size, leverage and minimum R:R limits.
- IANA daily reset timezone.
- Separate firm-rule and personal-limit enforcement: block assessment or warn.
- Firm/program labels and notes.

## API

All account policy endpoints require the authenticated Journal account owner.

- `GET /api/v1/journal/accounts/{id}/risk-policy`
- `PUT /api/v1/journal/accounts/{id}/risk-policy`
- `POST /api/v1/journal/accounts/{id}/risk-policy/evaluate`

The evaluation endpoint derives daily closed P&L, daily trade counts, open positions and recorded open risk from owner-scoped Journal history. The account's initial balance is read from the Journal account record. Current equity, start-of-day balance/equity, peak equity and current open notional are calculator inputs because Journal does not currently provide authoritative broker equity/high-water-mark snapshots.

Missing references for configured firm limits are not silently treated as zero. The response reports blocked/warning/within-policy plus violations. Currency-mismatched Journal P&L/risk entries are disclosed and excluded rather than falsely converted. Open floating P&L is not available from Journal; the assessment reports this when relevant. A conservative stop-risk projection is used for daily loss checks.

## FX conversion

`GET /api/v1/market-data/fx/convert?from=JPY&to=USD&amount=1`

- Uses the live BiQuote M1 candle close as the conversion reference.
- Supports direct, inverse and USD-triangulated pair lookup through the market-data instrument registry.
- Returns rate, conversion method, provider, timestamp, quote age and freshness.
- Same-currency conversion returns an identity rate.
- Demo, historical and unknown data are rejected.
- Stale quotes are rejected by default. `allow_stale=true` explicitly permits stale-rate estimates only; it does not permit demo or historical data.
- Unsupported pairs fail closed. The engine does not assume USDT is pegged to USD.

The calculator refreshes FX rates while active and applies quote-currency-to-account-currency conversion to position sizing, risk-to-levels, target-price and trade-analysis calculations. Risk calculations are estimates: broker contract size, tick value, spread, commission, swap, slippage and exact broker conversion conventions must still be verified.

## UI

- Policy editor: `/risk/policies`
- Trade Calculator: account policy selection and explicit policy assessment
- Risk navigation: Account Policies link

The FX provider and current account-risk state are visible in the calculator. Changing calculator inputs or FX freshness invalidates the displayed assessment; users must re-run it.
