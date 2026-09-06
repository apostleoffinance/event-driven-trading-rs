# Venue Connectors

`crates/venue-connectors` defines [`VenueAdapter`] and concrete venues.

## Owns

- `VenueAdapter` trait
- `OrderRequest` / `OrderAck` / `CancelRequest`
- `SimulatedVenue` (paper fills + positions)
- `PropVenue` (first prop connector — paper mode + live gate)
- OMS ↔ venue bridge (`submit_approved_order`)

## Must NOT own

- Strategy logic
- Risk evaluation
- Account registry (registers venue-local account state only)
- PostgreSQL

## Architecture

```text
ExecutionEngine (OMS)
       ↓
VenueAdapter          ← execution does not know concrete type
       ↓
SimulatedVenue | PropVenue | (future CEX)
```

## SimulatedVenue

Port of the legacy paper fill simulator:

- Splits quantity into up to two partial fills
- Applies percentage fee rate
- Maintains venue-local positions and account fee impact
- Idempotent submit by `client_order_id`
- Venue id: `simulated`

## PropVenue (Phase 11)

First prop firm connector behind the same adapter:

| Concern | Behavior |
|---------|----------|
| Venue id | `prop` (`VenueType::Prop`) |
| Paper mode | Single full fill, flat commission per unit |
| Live mode | `VenueError::NotImplemented` until a firm REST/WS is wired |
| Firm label | Vendor-agnostic (`generic-prop` by default) |

No proprietary vendor SDK is committed yet — paper mode unblocks OMS / runtime /
reconciliation against a real prop venue id.

## Bridge

`submit_approved_order(oms, venue, NewOrderRequest)`:

```text
create_order (risk-gated, idempotent)
  → mark_submitted
  → venue.submit_order
  → mark_accepted
  → apply_fill* (OMS audit + state)
```
