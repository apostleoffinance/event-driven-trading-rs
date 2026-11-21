# EdgeNest — Frontend Design System
Last updated: 2026-11-21. This document is the implementation guide for the approved Quant OS premium dark visual system and its EdgeNest blue identity.

## Product feel
A precise, calm, premium quantitative trading terminal. Keep high information density, clear hierarchy, light typographic weights, restrained borders, and minimal decoration. The blue gradient is a selective signature treatment for account and portfolio highlights—not a global background or a default card treatment.

## Foundations
- Default theme: near-black dark trading-terminal surfaces.
- Optional light theme: retained as a user preference.
- Interface type: Inter, generally weights 400–500.
- Technical and financial data: DM Mono with tabular numerals, generally weights 400–500.
- Use green and red only for confirmed financial/operational semantics.
- Use amber for PAPER/DEMO context and warnings; preserve explicit text labels.
- Keep focus rings, keyboard navigation, textual statuses and responsive behavior.

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

Do not use for:
- Page or app backgrounds.
- Every KPI card.
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

## Reusable primitives
- `SurfaceCard`: neutral surface container for content grouping.
- `MetricCard`: lightweight metric label/value/subtext hierarchy; accepts already formatted display strings and does not calculate financial values.
- `GradientAccountCard`: opt-in gradient surface for account/portfolio highlights only.
- Existing `StatusBadge`, `DataTable` and `PageHeader` remain the basis for their established responsibilities.

## Interaction and financial safety
- Frontend remains presentation-only and is not financial truth.
- Preserve explicit PAPER/DEMO/LIVE labels and actual connection state.
- Never infer health, connectivity, fills, balances or P&L from decoration.
- Money remains sourced from existing string-valued domain/API data; display components do not perform authoritative calculations.
- Do not change order, risk, sizing, authorization, reconciliation, or execution behavior as part of visual work.
- Keep empty, loading, stale, disconnected, partial, unknown and failed states explicit.
- Dangerous actions retain their existing confirmation and disabled/loading/error states.

## Phase rollout
1. Phase 0: source audit, visual specification and token mapping.
2. Phase 1: reusable visual tokens and foundational components.
3. Phase 2: account and portfolio experience.
4. Phase 3: strategies, positions, orders, risk and market-data workspaces.
5. Phase 4: Analytics Lab comparison and visualization refinement.
6. Phase 5: remaining page consistency and responsive refinement.
7. Phase 6: six-viewport visual QA and production release.

Each phase is delivered through a reviewable PR and requires screenshot/behavior review before the next phase begins. Phase 2 onward is not authorized until the user approves Phase 0/1.

## Acceptance checklist
- [ ] RULES.md and financial-safety constraints checked.
- [ ] Dark palette matches the approved near-black / steel-blue direction.
- [ ] Blue gradient is selective and never substitutes for status semantics.
- [ ] Inter and DM Mono use restrained weights and tabular financial numerals.
- [ ] Light theme remains supported.
- [ ] Accessible contrast, keyboard focus and responsive layout checked.
- [ ] No fake balances, fabricated health or unverified live claims.
- [ ] No backend/API, financial calculation, risk, account, or execution behavior changed.
