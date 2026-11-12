# Analytics Lab — metric contract and methodology

This document defines the trade-level contract used by the Analytics Lab execution and risk workspaces.

## Population and joins

- Analytics are read-only and scoped by the authenticated owner.
- Only closed trades with a close timestamp and a usable realized P&L are included in the summary population.
- Setup metadata is joined by both owner and setup ID.
- Psychology is joined by both owner and trade ID. A missing psychology row remains missing; it is never treated as a zero rating or a negative emotion.
- Currency validation runs before monetary aggregation. Mixed P&L currencies are rejected rather than summed.
- Trade facts retain their own IDs and timestamps so charts and group selections can resolve back to concrete trade records.

## Metric definitions

| Metric | Definition | Missing-data rule |
|---|---|---|
| Realized R | Stored R multiple; otherwise net P&L divided by positive recorded risk amount | Null if neither valid source exists |
| Holding duration | Closed timestamp minus opened timestamp, in seconds | Not available if timestamps are absent; negative durations are rejected by journal constraints |
| MFE capture | Per-trade realized R divided by positive MFE R, multiplied by 100 | Only computed when both values exist and MFE R is positive |
| Profit surrendered | MFE R minus realized R | Only reported when both exist and MFE exceeds realized R |
| Risk coverage | Closed trades with positive recorded risk amount divided by included closed trades | Missing risk is excluded from the numerator, never imputed |
| Average/maximum risk | Arithmetic mean and maximum of positive recorded risk amounts | Only over risk-known trades |
| Win/loss streak | Maximum consecutive positive/negative realized P&L observations in close-time order | Zero if no such streak exists |
| Drawdown | Peak-to-trough decline in cumulative realized P&L, in close-time order | Does not include open/floating P&L |

## Precision and interpretation

- MFE/MAE currently inherit the precision and coverage of the stored excursion source. Historical candle-derived excursions are not tick-level.
- MFE capture can be negative when a trade closed at a loss despite having positive MFE. This is intentional and must not be clipped to zero.
- A stored stop-loss or take-profit is only evidence that the field was recorded. It does not prove correct placement, broker acceptance, or fill quality.
- Risk amounts are monetary values in the Journal account's P&L currency. Do not compare or aggregate across currencies.
- The analytics drawdown is realized Journal drawdown, not broker equity drawdown, open exposure, or a live risk-engine limit.
- Slippage, spread, partial-fill quality, latency, and price improvement require execution/fill source records and are not inferred from entry/exit prices alone.

## Quality gates

1. Validate summary values against deterministic trade fixtures before changing formulas.
2. Test owner isolation and currency rejection in the API integration suite.
3. Keep nullable metrics nullable through API and UI.
4. Show coverage counts beside ratios and averages.
5. Verify the empty state, partial-data state, single-currency state, and responsive layouts.
6. Do not describe candle-derived excursion values as tick-level execution analytics.
