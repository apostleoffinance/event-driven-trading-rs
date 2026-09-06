# Venue Connectors

`crates/venue-connectors` defines [`VenueAdapter`] and concrete venues.

## Owns

- `VenueAdapter` trait
- `OrderRequest` / `OrderAck` / `CancelRequest`
- `SimulatedVenue` (paper fills + positions)
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
SimulatedVenue   (now)    PropVenue (Phase 11)
```

## SimulatedVenue

Port of the legacy paper fill simulator:

- Splits quantity into up to two partial fills
- Applies fees
- Maintains venue-local positions and account fee impact
- Idempotent submit by `client_order_id`

## Bridge

`submit_approved_order(oms, venue, NewOrderRequest)`:

```text
create_order (risk-gated, idempotent)
  → mark_submitted
  → venue.submit_order
  → mark_accepted
  → apply_fill* (OMS audit + state)
```
