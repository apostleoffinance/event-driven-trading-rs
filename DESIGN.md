# EdgeNest — Frontend Design System
Last updated: 2026-11-21. This document is the implementation guide for the approved Quant OS premium dark visual system, EdgeNest blue identity, and concise interface language.

## Product feel
A precise, calm, premium quantitative trading terminal. Keep high information density, clear hierarchy, light typographic weights, restrained borders, and minimal decoration. The interface should feel immediately understandable: short labels, clear headings, contextual help only when needed, and detailed explanations available on demand. The blue gradient is a selective signature treatment—not a global background or a default card treatment.

## Foundations
- Default theme: near-black dark trading-terminal surfaces.
- Optional light theme: retained as a user preference.
- Interface type: Inter, generally weights 400–500.
- Technical and financial data: DM Mono with tabular numerals, generally weights 400–500.
- Use green and red only for confirmed financial/operational semantics.
- Use amber for PAPER/DEMO context and warnings; preserve explicit text labels.
- Keep focus rings, keyboard navigation, textual statuses and responsive behavior.
- Default visible copy should be concise and scannable. Put methodology, definitions, caveats, and long explanations behind contextual help, expandable details, or dedicated documentation.
- Never remove or hide information required to understand a material risk, financial limitation, data-quality issue, or user decision.

## Core palette

| Token | Value | Use |
|---|---|---|
| Base background | `#090A0D` | App canvas |
| Panel | `#121418` | Main panels |
| Elevated surface | `#181C22` | Nested controls and subtle elevation |
| Deep navy | `#06111C` | Gradient endpoint |
| Midnight blue | `#081F32` | Gradient transition |
| Ocean blue | `#084360` | Signature gradient body |
| Steel blue | `#315570` | Gradient and selected-state structure |
| Muted blue | `#6F869E` | Accent, chart line, secondary emphasis |
| Amber | `#98590C` | Restrained warm accent |
| Primary text | `#F2F4F7` | Main content |
| Secondary text | `#A1A8B3` | Supporting content |
| Positive | `#18A66A` | Confirmed positive/healthy state |
| Negative | `#E16E75` | Loss/error/rejection state |
| Warning | `#D6A24B` | Warning and PAPER/DEMO indicators |
| Border | `#252B34` | Dividers and card boundaries |

## Gradient usage
Signature account gradient:
`linear-gradient(115deg, #6F869E 0%, #315570 30%, #084360 58%, #06111C 100%)`

Use selectively for:
- Account identity and funded-capital/equity summary cards.
- Portfolio-level headline balance or equity summary where it improves hierarchy.
- A small number of intentionally featured account/portfolio surfaces.
- The public landing page's primary CTA and restrained ambient visual accents.

Do not use for:
- App backgrounds or every KPI card.
- Tables, forms, risk alerts or dense data surfaces.
- Positive/negative P&L semantics.
- Live/connected status indicators.

Amber should remain a small highlight, never a competing primary accent. The optional light theme must remain legible and usable; gradient text contrast stays light.

## Typography hierarchy
- Page headings: Inter 400–500.
- Section/card headings: Inter 500.
- Body and descriptions: Inter 400.
- Labels and metadata: Inter 400, with restrained tracking.
- Balances, P&L, prices, quantities and timestamps: DM Mono 400–500 with tabular numerals.
- Avoid defaulting all headings, metrics and navigation to bold weights.
- Never shrink risk-critical information to fit a card.

## Interface language standard

### Core rule
**Short labels. Clear headings. Contextual help only when needed. Detailed explanations on demand.**

### Copy hierarchy
1. **Primary:** concise page title or metric label that identifies the content.
2. **Secondary:** one short sentence only when it helps users understand the purpose or next step.
3. **Contextual:** definitions, methodology, calculation assumptions, caveats, and UTC explanations in an info tooltip, popover, expandable details, or a nearby help link.
4. **Reference:** full technical/statistical explanations in dedicated documentation or a methodology view—not repeated on every page.

### Writing rules
- Prefer familiar words over internal implementation terminology.
- Use sentence case for headings and labels; reserve uppercase for compact technical metadata where it is already an established visual convention.
- Use concise, action-oriented button labels: “Retry”, “Connect account”, “View details”, “Create journal”.
- Avoid repeating the same explanation in a page subtitle, card description, alert, and tooltip.
- Do not expose internal enum values, snake_case identifiers, raw API names, or machine-oriented group keys as user-facing labels. Map display labels only; preserve canonical stored values and grouping behavior.
- Format labels consistently: e.g. `london` → “London”, `london_ny_overlap` → “London–New York overlap”, `new_york` → “New York”.
- Show timezone context once at the relevant section/filter level (e.g. “Times shown in UTC”), not repeated in every row or chart.
- Keep timestamps compact in the interface; provide exact generation time in metadata or details where auditability requires it.
- Keep privacy-sensitive identity information out of public pages and avoid repeating email addresses unless the user needs them for account management.
- Do not simplify away important distinctions between Journal and broker execution data, currencies, stale data, sample sizes, paper/live modes, or risk-policy outcomes.

### Contextual help placement
- Use an info icon with an accessible label for definitions and methodology.
- Use a tooltip/popover for short definitions; use an expandable “How this is calculated” or “Methodology” section for longer explanations.
- Keep critical warnings, stale/disconnected/unknown states, required acknowledgements, and material financial limitations visible. They must not be hidden in a tooltip.
- Tooltips must be keyboard-accessible and not hover-only.
- Avoid help icons on obvious labels; help should answer a real likely question.

### Before / after examples
| Avoid | Prefer |
|---|---|
| TraderOS-style position sizing with backend FX conversion and account risk policies | Position sizing & risk planning |
| Understand your trading edge across profitability, execution, behaviour and risk. | Understand your trading performance. |
| Signed in as user@example.com. | Keep identity in the account menu. |
| Analytics snapshot generated [timestamp] · All grouped session, weekday and hour metrics use UTC. | Updated [time] UTC · Times grouped by UTC |
| Statistical context, not a prediction. The interval communicates uncertainty in the observed win rate; it is not a probability that the true rate lies inside these bounds. | Win-rate confidence interval · Not a prediction. Put the full statistical interpretation in methodology help. |

These are copy/display examples only. They do not authorize changes to formulas, stored values, timestamps, grouping keys, or risk semantics.

## Reusable primitives
- `SurfaceCard`: neutral surface container for content grouping.
- `MetricCard`: lightweight metric label/value/subtext hierarchy; accepts already formatted display strings and does not calculate financial values.
- `GradientAccountCard`: opt-in gradient surface for account/portfolio highlights only.
- Existing `StatusBadge`, `DataTable` and `PageHeader` remain the basis for their established responsibilities.
- Reuse a consistent contextual-help pattern rather than adding bespoke long explanatory text to every screen.

## Interaction and financial safety
- Frontend remains presentation-only and is not financial truth.
- Preserve explicit PAPER/DEMO/LIVE labels and actual connection state.
- Never infer health, connectivity, fills, balances or P&L from decoration.
- Money remains sourced from existing string-valued domain/API data; display components do not perform authoritative calculations.
- Do not change order, risk, sizing, authorization, reconciliation, or execution behavior as part of visual or copy work.
- Keep empty, loading, stale, disconnected, partial, unknown and failed states explicit.
- Dangerous actions retain their existing confirmation and disabled/loading/error states.
- A concise UI must not conceal material risk, uncertainty, data limitations, or action consequences.

## Phase rollout
1. Phase 0: source audit, visual specification and token mapping.
2. Phase 1: reusable visual tokens and foundational components.
3. Phase 2: account and portfolio experience.
4. Phase 3: strategies, positions, orders, risk and market-data workspaces.
5. Phase 4: Analytics Lab comparison and visualization refinement.
6. Phase 5: remaining page consistency and responsive refinement.
7. Phase 6: six-viewport visual QA and production release.
8. **Copy consistency pass:** audit every route and shared component against the concise interface language standard; prioritize repeated descriptions, raw enum labels, long methodology copy, redundant timestamps, and public/private copy separation.

Each phase is delivered through a reviewable PR and requires screenshot/behavior review before the next phase begins. Visual/copy changes must preserve existing product behavior and financial semantics.

## Acceptance checklist
- [ ] RULES.md and financial-safety constraints checked.
- [ ] Dark palette matches the approved near-black / steel-blue direction.
- [ ] Blue gradient is selective and never substitutes for status semantics.
- [ ] Inter and DM Mono use restrained weights and tabular financial numerals.
- [ ] Light theme remains supported.
- [ ] Page titles and visible labels are concise and consistent.
- [ ] Long explanations are contextual, discoverable, and accessible.
- [ ] Critical risk, stale, disconnected, unknown, and failure information remains visible.
- [ ] Raw internal identifiers are not shown as user-facing labels.
- [ ] Accessible contrast, keyboard focus and responsive layout checked.
- [ ] No fake balances, fabricated health or unverified live claims.
- [ ] No backend/API, financial calculation, risk, account, or execution behavior changed.
