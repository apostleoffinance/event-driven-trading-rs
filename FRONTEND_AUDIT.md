# Quant OS Frontend Baseline Audit

Date: 2026-10-27
Task: QOS-003
Source: `main` at merge commit `26d4a425816e358ca36ec70529e63a3f686f69d8` (PR #3)
Scope: Inspect scripts, dependencies, configuration, theme tokens, data-source selection, and operational-status presentation before UI changes.

## Summary

The frontend is a Next.js 15 / React 19 / TypeScript mock-first control-plane prototype. CI passed lint, typecheck, and production build on PR #3's latest head. That establishes build health, not data authenticity, runtime health, or safe live-mode semantics.

## Verified findings

### Scripts and dependencies
- `frontend/package.json`: Next.js `^15.5.7`, React/React DOM `^19.0.0`, TypeScript `^5`, ESLint `^9`, `eslint-config-next` `15.1.0`, TanStack Query/Table, ECharts, Lucide, and Zod.
- Scripts: `dev`, `build`, `start`, `lint: next lint`, and `typecheck: tsc --noEmit`.
- The declared Next version range and pinned older `eslint-config-next` are version-skew risk. The existing CI lint passed; do not upgrade dependencies incidentally. Resolve compatibility in a dedicated dependency task with lockfile and CI evidence.

### Data-source and environment semantics
- `src/lib/repositories/index.ts` always returns mock repositories. When `NEXT_PUBLIC_DATA_MODE=api` and an API URL are set, it warns that API mode is not wired and still returns mocks.
- `src/config/frontend.ts` independently accepts `NEXT_PUBLIC_ENVIRONMENT=live`; this label is not derived from a backend handshake or connected venue.
- `src/features/simulation/SimulationProvider.tsx` can generate synthetic intent, risk-approved, order-submitted, fill, and position events in the browser.
- `src/mocks/accounts.ts`, `portfolio.ts`, and `system.ts` contain illustrative balances, P&L, healthy statuses, metrics, and timestamps. System mock timestamps are dated 2026-09-06, so they must never be presented as current telemetry.
- The top bar in `components/layout/Topbar.tsx` renders `SYSTEM HEALTHY` unconditionally. It displays an event-stream indicator but labels the category rather than explicitly rendering connected/disconnected/unknown text. This can overstate operational state.
- The README correctly says mock mode does not execute trades or evaluate risk. Preserve that distinction in all UI copy and actions.

### Theme and design tokens
- Tokens live in `src/styles/globals.css`; theme supports dark and light and `ThemeProvider` persists the choice in localStorage.
- Current root default is dark. Accent and chart tokens use blue (dark `#6ba3f7`, light `#1f6feb`).
- This differs from the authoritative design direction in `RULES.md`: white predominant, navy `#101A8A`, orange `#F58220`, restrained teal, and no purple gradients.
- Layout and core status colors use CSS variables, which gives a reasonable migration point. Existing tokens include backgrounds, text, borders, accent, positive/negative/warning, paper label, chart and drawer tokens.

### Accessibility and presentation
- Global `:focus-visible` styling exists; navigation uses `aria-current`, and several icon buttons have accessible labels.
- Current status text/visuals need stronger source-of-truth semantics before a design refresh: distinguish mock vs connected API, paper vs shadow vs live, and healthy vs degraded/disconnected/unknown/stale. Do not infer system health from a static mock or environment label.
- Dashboard money values are generally strings in domain models, but some display-only ratios are converted to JS numbers. Keep authoritative financial calculations server-side and do not use browser numbers as financial truth.

## QOS-003 conclusion

Acceptance met: scripts, dependencies, config, theme/tokens, data-source selection, and key operational-status presentation were inspected and documented. No UI, runtime, dependency, or trading behavior was changed in this audit.

## Recommended implementation order

1. Before UI redesign, fix truthful state semantics: never show LIVE as proof of connectivity; never show SYSTEM HEALTHY unless backed by current health data; show MOCK DATA explicitly; expose stale/disconnected/unknown states.
2. Apply the white-dominant navy/orange palette in a separate, visual-only task after the state semantics are defined.
3. Keep API integration separate until a real, authenticated, versioned backend contract exists. Do not silently fall back to mocks when API mode is requested; make the mode/fallback explicit.
4. Address Next/ESLint version alignment as a dedicated dependency task, with lockfile review and CI verification.

## Evidence and limitations

- Source files inspected: `frontend/package.json`, `README.md`, `.env.example`, `eslint.config.mjs`, `next.config.ts`, `tsconfig.json`, `src/styles/globals.css`, `src/config/frontend.ts`, `src/components/layout/{AppShell,Topbar,Sidebar}.tsx`, `src/features/theme/ThemeProvider.tsx`, `src/features/events/EventStreamProvider.tsx`, `src/features/simulation/SimulationProvider.tsx`, `src/lib/repositories/{index,mock}.ts`, `src/mocks/{accounts,portfolio,system}.ts`, and `src/app/page.tsx`.
- CI evidence: run 40425954540 completed successfully on PR #3's latest head before it was merged, including frontend lint, typecheck, and production build, as well as Rust format/check/test/Clippy.
- This is a source-backed baseline review, not a browser usability test, security assessment, live API integration test, or production readiness certification.
