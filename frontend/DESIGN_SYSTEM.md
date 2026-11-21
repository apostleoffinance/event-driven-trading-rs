# Quant OS — Premium Dark Design System

**Status:** Approved visual direction; refinement in progress  
**Approved:** 2026-11-21  
**Reference:** User-approved Quant OS redesign mockup inspired by the supplied Backed frontend analysis.

## Design intent
A premium quantitative trading operations terminal: restrained, information-dense, calm, technical, and legible. Use dark layered surfaces, subtle cool-blue light, thin borders, compact labels, clear hierarchy, and high-signal financial data.

## Tokens
- Page background: `#090A0D`
- Primary surface: `#121418`
- Secondary surface: `#191C22`
- Elevated surface: `#222731`
- Primary text: `#F4F6F8`
- Secondary text: `#B7B1BC`
- Muted text: `#858995`
- Cool blue accent: `#78A8CB`
- Positive / healthy: `#18A66A`
- Negative / loss / error / rejected: `#E16E75`
- Warning / degraded / pending: `#D5A84F`
- Subtle borders: `rgba(255,255,255,.085)`
- Strong borders: `rgba(255,255,255,.14)`

## Typography
- Interface, navigation, headings, body copy, and primary metric values: Inter, predominantly 400–500; reserve 600–700 for rare emphasis.
- Technical labels, identifiers, timestamps, and dense tabular figures: DM Mono, 400–500.
- Headlines: lightweight (400), tight tracking, responsive 24–30px for page titles.
- Metric values: Inter 400–500 with tabular numerals; avoid using monospaced display typography for every KPI.
- Metadata: 9–10px DM Mono labels, used sparingly.

## Surfaces and layout
- Shared application shell with a 54px topbar and 200px expanded sidebar on desktop; collapse the sidebar to 68px when requested.
- Main content max-width is governed by each existing page layout; use 28–32px desktop padding and compact mobile gutters.
- Cards use 10px radius, low-contrast border, subtle dark gradient, and restrained shadow.
- Keep dense operational tables horizontally scrollable on narrow screens.
- Maintain a fixed, readable hierarchy between page heading, explanatory copy, controls, and data surfaces.

## State colour semantics
- Green is reserved for confirmed positive P&L, healthy connectivity, approved risk decisions, and successful/confirmed states.
- Red is reserved for negative P&L, drawdown/loss, rejected/failed/error states, and explicit danger.
- Amber is used for warnings, pending, stale, unknown, and degraded states when domain semantics classify them that way.
- Blue is for neutral information, active selection, chart series, and navigational emphasis.
- Never infer or fabricate status to obtain a desired colour. Unknown remains unknown.

## Motion
- Standard hover transitions: 180–220ms ease.
- Page/section reveals: subtle 450–650ms fade and 8–16px vertical shift only where it improves orientation.
- Avoid continuous decorative animation in dense trading workspaces.
- Respect `prefers-reduced-motion`.

## Responsive rules
- Desktop: persistent sidebar and topbar.
- Tablet: narrower content gutters and adaptive two-column layouts.
- Mobile: menu-driven sidebar, single-column page content, horizontally scrollable data tables, compact KPI grids, hidden global search where space is constrained.
- Preserve keyboard focus visibility, textual status labels, and accessible contrast.

## Product and safety invariants
- Frontend is not financial truth and cannot bypass backend risk authorization.
- Money remains string-based at API/domain boundaries; do not introduce JS floating-point authoritative balances/P&L.
- Never replace API errors or empty states with fabricated records.
- Clearly distinguish PAPER, SHADOW, and LIVE.
- Unknown, stale, disconnected, partial, and failed states remain explicit.
- Visual redesign must not enable live trading, submit orders, cancel orders, or change risk decisions.
