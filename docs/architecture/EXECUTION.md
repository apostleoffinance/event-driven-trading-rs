# Execution / OMS

`crates/execution-engine` owns order lifecycle after risk approval.

## Owns

- Order creation from risk-approved requests
- Client order ID idempotency
- State transitions (submit / accept / fill / cancel / reject / fail)
- Fill application (partial + full)
- Audit trail per order
- Recording the `RiskDecision` that authorized each order

## Must NOT own

- Strategy logic
- Risk rule evaluation (consumes `RiskDecision` only)
- Venue HTTP (Phase 7 `VenueAdapter`)
- Account registry mutations (Phase 5)

## Critical invariants

1. No order without an executable risk decision  
2. Every order has `account_id`, `venue_id`, and unique `client_order_id`  
3. Same `client_order_id` + matching payload → same order (no duplicates on retry)  
4. Mismatched retry payload → `IdempotencyConflict`  
5. Every order has an audit trail  

## Lifecycle

```text
TradeIntent → RiskDecision(Approved|Resized)
      ↓
OMS create_order (Created → PendingRisk → Approved)
      ↓
Submitted → Accepted
      ↓
PartiallyFilled* → Filled
   or CancelPending → Cancelled
   or Rejected / Failed
```

Venue submission is coordinated here; live venue I/O arrives in Phase 7.
