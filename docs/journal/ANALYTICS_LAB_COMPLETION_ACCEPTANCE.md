# Analytics Lab completion acceptance matrix

This file is the release gate for the Analytics Lab Completion milestone. A green build is necessary but is not a substitute for the data, interaction, and visual checks below.

## 1. Deterministic metric contract

| Area | Acceptance assertion | Evidence |
|---|---|---|
| Population | Only closed trades with close timestamp and realized P&L enter the result | API integration fixture |
| Ownership | Every trade, setup and psychology join remains scoped to the authenticated owner | Two-owner PostgreSQL fixture |
| Currency | A mixed-currency population is rejected before monetary aggregation | API integration fixture |
| Realized R | Stored R wins over net-P&L / positive-risk fallback; absent inputs remain null | Domain regression tests |
| Holding time | UTC close minus open in seconds; zero is valid; negative duration is rejected | Domain regression tests |
| MFE capture | Realized R / positive MFE R × 100; negative capture is retained | Domain regression tests |
| Profit surrendered | Positive MFE less realized R, only when MFE exceeds realized R | Domain regression tests |
| Risk coverage | Positive recorded risk only; no imputation | API fixture with known and missing risk |
| Risk distribution | Quantiles are computed from positive risk-known records only | API fixture with odd/even sample counts |
| Drawdown | Peak-to-trough cumulative realized P&L in deterministic close-time order | Domain/API fixture |
| Streaks | Consecutive positive/negative realized outcomes; zero resets either streak | API fixture with win/loss/breakeven sequence |
| Behaviour | Each ratio uses its own known-value denominator; unknown is not false | API fixture with null annotations |
| Psychology | Missing records/ratings stay null and are excluded from means | API fixture with partial psychology |
| Calendar | UTC day/week/month buckets agree with the same filtered trade population | Calendar fixture crossing month/year boundaries |

## 2. Interactive investigation

- [x] Calendar date selection applies the exact UTC date filter.
- [x] Calendar date selection exposes the trade rows for that date.
- [x] A calendar trade row opens the shared trade-detail investigation panel.
- [x] Behaviour is segmented by session, setup and weekday with known-value denominators.
- [x] Every chart, heatmap, table and calendar summary in the current Phase 3/4 scope opens a matching investigation; visual/data-set equivalence remains subject to the exact-population browser verification below.
- [ ] Verify API-backed saved views after reload, sign-out/sign-in, and account switching without leaking filters across accounts.
- [ ] Verify API-backed saved views on a second device/browser after the Journal API migration is deployed.

## 3. Data and API acceptance

Required database-backed integration fixtures:
1. Owner A and Owner B with same-looking setup names and trade IDs where schema permits; verify no cross-owner joins.
2. Same owner with two P&L currencies; verify HTTP 422 and no totals.
3. No closed trades, one closed trade, and mixed complete/partial trade records.
4. Trade with stored R and risk-derived R disagreement; stored R must win.
5. Missing risk, missing MFE, missing MAE, missing psychology row, and partially populated psychology row.
6. Multiple trades with identical close timestamps; tie-break by stable trade ID.
7. Win/loss/breakeven sequences that prove streak reset behaviour.
8. Calendar samples across Sunday/Monday, month-end, year-end, leap day, and UTC midnight.
9. Setup/session/weekday behaviour segments with null values and unequal known-value denominators.
10. Saved-view API: owner isolation, account-scope isolation, duplicate-name upsert, malformed filter rejection, and retrieval after a new authenticated session.

## 4. Saved-view persistence implementation

- Persistent saved views are stored in `journal_analytics_saved_views` and scoped by verified JWT owner plus Journal account (or the explicit all-accounts scope).
- The API validates account ownership for list and write operations; owner IDs are never accepted from the client.
- The frontend uses the Journal API rather than browser-local storage for saved views. Filter convenience state remains user-scoped browser-local state.
- Database migration and persistence scope constraints are covered by the Journal schema integration test. Authenticated HTTP and second-device acceptance remains a deployed-environment gate.

## 5. Risk and execution scope boundaries

The following must not be fabricated from Journal fields:
- Account-equity-normalized risk without a reliable balance/equity snapshot at trade open.
- Risk-policy violations without the historical risk-policy decision/event record.
- Slippage, spread, latency, price improvement or partial-fill efficiency without authoritative broker fill data.
- Floating/broker drawdown from realized Journal P&L.
- Stop/target placement correctness from presence flags alone.
- Tick-level excursion precision from candle-derived MFE/MAE.

When the source system does not provide these facts, the UI must label the metric unavailable and explain the missing source.

## 6. Responsive and authenticated acceptance

Run the following against a deployed test environment with seeded accounts:
- Desktop 1440×900, laptop 1280×800, tablet 768×1024, mobile 390×844 and 360×800.
- Verify no horizontal page overflow; tables may scroll inside their own wrappers.
- Verify filters, tabs, dialogs, charts and calendar controls are keyboard reachable and have accessible names.
- Verify empty, loading, error, partial-data, mixed-currency and populated states.
- Verify trade-detail dialogs open the selected trade, close by button and backdrop, and do not select a different row.
- Verify account switching and sign-out clear or partition saved views and filters.
- Verify date boundaries and all grouped calendar labels are explicitly UTC until timezone-selectable aggregation is implemented.

## 7. Release decision

Do not call the milestone complete until all five phases have evidence:
1. Shared fact and joins validated.
2. Execution and risk metrics validated against known records.
3. Behaviour segmentation and calendar aggregation validated.
4. All investigation paths resolve to exact trade records; saved-view scope is verified.
5. CI, API/database integration, authenticated acceptance and responsive visual QA pass.

A CI-only pass is a code-quality signal, not production acceptance. Live checks requiring database credentials, a deployed authenticated account, broker fills, object storage or real devices must be run in that environment and attached to the release evidence.


## 8. Repeatable browser acceptance evidence

Run the Analytics Lab acceptance suite against the same preview commit and seeded test dataset. Do not substitute a local unauthenticated shell or a production account for this run.

### Viewport matrix

| Viewport | CSS viewport | Minimum assertions |
|---|---:|---|
| Desktop | 1440 × 900 | Seven-workspace navigation, KPI alignment, chart/table density, no clipped dialogs |
| Laptop | 1280 × 800 | Main content width, filter layout, chart legends, sticky/scrolling table headers |
| Compact desktop | 1024 × 768 | Navigation collapse, filter wrapping, no overlapping cards |
| Tablet | 768 × 1024 | Single/two-column transitions, chart readability, usable calendar controls |
| Mobile | 390 × 844 | No page-level horizontal overflow, controls remain tappable, trade rows inspectable |
| Narrow mobile | 360 × 800 | No clipped filter actions, modal content fits/scrolls, table scroll remains local |

### Required interaction paths

For every run, record the preview URL, git SHA, browser/version, viewport, account fixture identifier (never credentials), and pass/fail result.

1. Open all seven Analytics workspaces and capture the initial viewport plus the scrolled content.
2. Apply account, date, instrument and setup filters; confirm the trade count and all workspace populations update consistently.
3. Select one sample from each chart/heatmap/table/calendar interaction and compare the investigation rows to the exact source population.
4. Open a trade from each investigation path; verify the selected trade ID, detail fields and close behaviour.
5. Save a named view; reload; sign out/in; switch between two accounts; verify only the expected owner/account scope is returned.
6. Open the same account in a separate browser profile/device and verify the saved view is available from the server API.
7. Exercise empty, one-trade, partial annotation, missing risk, no psychology, API error and slow-loading states.
8. Check keyboard-only operation, visible focus, accessible names, dialog focus handling and reduced-motion behaviour.

### Evidence location and release gate

Store screenshots and a machine-readable result summary under the release artifact for this PR. A viewport passes only when its screenshot and interaction assertions are attached. A check must remain **not run** when no authenticated preview, test accounts, or second device are available. Never translate an unrun manual check into a pass based on CI.
