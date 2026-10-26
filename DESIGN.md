# Quant OS — Design System
Last updated: 2026-10-27. Check `RULES.md` and existing frontend tokens before implementation.

## Feel
Precise, calm, readable trading operations console. High information density without visual noise. Avoid hype, casino aesthetics, decorative gradients, and fake live data.

## Tokens
| Token | Value | Use |
|---|---|---|
| Canvas/surface | `#FFFFFF` | Predominant background |
| Ink | `#172033` | Main text |
| Navy | `#101A8A` | Structure/navigation/headings |
| Orange | `#F58220` | Important actions/emphasis |
| Teal | `#126B5B` | Restrained healthy status |
| Danger | `#B42318` | Critical risk/failure |
| Warning | `#B54708` | Warning/degraded |
| Border | `#E5E7EB` | Dividers/grids |
| Muted | `#667085` | Secondary labels |

Color is never the sole status indicator. Check contrast in actual components.

## Typography
Manrope preferred for headings/UI if already available; system sans-serif fallback. Use tabular numerals for balances, P&L, prices, quantities, and timestamps. Avoid tiny text for risk-critical information. Do not add font dependencies without checking current frontend.

## Components
- Account status: identity/type, venue, mock/paper/live mode, equity, drawdown, connection, last update.
- Risk decision: Approved/Resized/Rejected/Halted, reason, policy version.
- Order lifecycle: show `Unknown` distinctly from `Failed`.
- Multi-account results: independent outcomes and explicit partial success.
- Risk alert: severity, account, time, observed value, limit, requested action, confirmed result.
- Tables: useful sorting/filtering and clear empty/loading/error states.
- Dangerous actions: scope and explicit confirmation.
- Live mode: distinct visual state and clear user intent.

## Interaction
Async controls need loading, success, failure, timeout/unknown, and disabled states. Never translate timeout into success. Cancellation/closure remains requested until confirmed.

## Acceptance checklist
- [ ] RULES.md checked
- [ ] Contrast, keyboard focus, responsive layout checked
- [ ] Mock/paper/live mode visible
- [ ] Status includes text/icon, not color only
- [ ] Empty/loading/stale/disconnected/error/partial states handled
- [ ] Money uses string wire values and tabular presentation
- [ ] No fake balances or unverified live claims
