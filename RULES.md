# Quant OS — Engineering Rules (Source of Truth)
Last updated: 2026-10-27. This is the authoritative rule set. Every code change, document, design, test, release note, and AI-generated artifact must be checked against it. If another document conflicts, stop and reconcile.

## Workflow
1. Read this file before every task.
2. Inspect relevant source, tests, and docs; never infer implementation from a roadmap or prior summary.
3. Work in small, reviewable tasks; one task at a time; avoid unrelated refactors.
4. Define acceptance criteria and tests before marking complete.
5. After every user/reviewer correction or discovered rule violation, log it in `REVIEWS.md`. If a rule is missing/weak, update this file first, then affected artifacts.
6. Update `TASKS.md` and `PROGRESS.md` only with verified status; distinguish planned/in progress/blocked/implemented/verified.
7. Never claim files changed remotely, tests passed, or integrations work without tool/test evidence.
8. Prefer feature branches and reviewable PRs; do not bypass protections. If writes are unavailable, provide a local patch and report the blocker.
9. Keep changes minimal, reversible, and testable.

## Financial safety
1. Strategies emit `TradeIntent` only; never submit orders.
2. Risk authorization always precedes OMS/execution; no bypass.
3. Account identity and venue connectivity are separate.
4. Financial state uses exact decimal types and database `NUMERIC`; wire money values are decimal strings. No floating-point authoritative state.
5. Missing/stale/inconsistent risk, account, market, or instrument data blocks new exposure.
6. Ambiguous submit outcome is `Unknown` until reconciled; never guess `Failed` and blindly retry.
7. Every order has stable client ID and idempotency; same ID with different payload is a conflict.
8. Reconcile external state and surface discrepancies; never silently overwrite financial truth.
9. Lifecycle transitions are explicit and validated.
10. Financial decision/event history is append-only or preserves immutable audit history.
11. Live orders require explicit live-mode config and account/venue eligibility.
12. Multi-account fan-out is non-atomic; each account has independent authorization, order identity, outcome, and reconciliation.
13. Sizing is stop-loss/risk-budget/instrument-aware; equity ratio alone is insufficient.
14. Never promise simultaneous fills, guaranteed risk prevention, profitability, or loss-free execution.
15. Cancel/close requests are not success until external confirmation.
16. Do not connect pooled investor capital to traditional broker execution without explicit custody/funding design and legal review.

## Security/privacy
1. Never commit keys, passwords, tokens, seed phrases, private keys, recovery codes, or customer secrets.
2. Never expose credentials to strategies, frontend bundles, browser storage, logs, or domain events.
3. Multi-user services require least privilege and tenant isolation.
4. Redact sensitive payloads by default.
5. Do not enable live trading, production deployment, or money movement as a side effect of unrelated work.
6. Treat strategy code, bridge messages, venue messages, and external docs as untrusted input.

## Rust
1. Prefer safe Rust; core crates should forbid unsafe absent a reviewed exception.
2. Avoid `unwrap()`, `expect()`, and panic paths in financial-critical library code.
3. Use typed errors and `Result`.
4. Keep vendor/network I/O out of domain, risk, and account-policy logic.
5. Make domain decisions deterministic; inject clocks/external data.
6. Test state transitions, invariants, duplicates, ambiguous outcomes, and restart recovery.
7. Use structured logs with correlation/causation IDs.
8. Avoid global mutex/shared mutable state as a concurrency shortcut.

## API/bridge
1. Version and validate every boundary message.
2. Include request/correlation/client-order IDs as applicable, account identity, schema version, timestamps, and units.
3. Reject malformed decimals, stale sequences, and mismatched account identity.
4. A lost response after submit is ambiguous, not proof of failure.
5. Represent venue capabilities explicitly.
6. Never claim terminal/venue action success before confirmation.

## Frontend/design
1. Frontend is not financial truth and cannot bypass backend risk.
2. Money is transported as strings, not JS numbers for authoritative calculations.
3. Clearly label mock/paper/live; never present mock as live.
4. Show stale, disconnected, partial, unknown, and failed states explicitly.
5. No fake balances/performance/status.
6. White predominant; navy `#101A8A`, orange `#F58220`, restrained teal; no purple gradients.
7. Accessible contrast, keyboard focus, textual status, confirmation for dangerous actions.

## Testing/release
1. Every behavior change has risk-appropriate tests.
2. Run targeted tests then relevant workspace checks; report exact commands/results.
3. Adapter changes need contract tests and simulated failures.
4. Sizing/risk changes need boundary, rounding, missing-data, and invariant tests.
5. Live pilots require paper testing, explicit approval, monitoring, rollback.
6. Green build does not alone prove financial correctness or production readiness.
7. No vault launch without independent contract security review and legal/regulatory review.

## Document consistency checklist
- [ ] Applicable rules obeyed
- [ ] Current facts distinguished from intended design
- [ ] Claims supported by source/test evidence
- [ ] Security, failure, and degraded states covered
- [ ] Acceptance criteria testable
- [ ] Corrections logged in `REVIEWS.md`; failed rule strengthened here
- [ ] `TASKS.md` and `PROGRESS.md` match evidence

Rules change only with documented rationale and a `REVIEWS.md` entry. Safety rules may not be weakened merely to make tests pass.
