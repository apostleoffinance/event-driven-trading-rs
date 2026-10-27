//! Client order ID allocation derived from trade intents.

use domain::{ClientOrderId, TradeIntentId};

use crate::error::{RuntimeError, RuntimeResult};

/// Stable client order id for a trade intent (retry-safe / idempotent).
pub fn client_order_id_for_intent(intent_id: &TradeIntentId) -> RuntimeResult<ClientOrderId> {
    ClientOrderId::new(format!("clid-{}", intent_id.as_str())).map_err(RuntimeError::from)
}


/// Stable client order ID scoped to both account and trade intent.
///
/// A single strategy intent fanned out to multiple accounts must not reuse a
/// venue-level idempotency key across those accounts. The input order is fixed
/// and deterministic, so retries for the same account/intent reproduce the ID.
pub fn client_order_id_for_account_intent(
    account_id: &domain::AccountId,
    intent_id: &TradeIntentId,
) -> RuntimeResult<ClientOrderId> {
    ClientOrderId::new(format!(
        "clid-{}-{}-{}-{}",
        account_id.as_str().len(),
        account_id.as_str(),
        intent_id.as_str().len(),
        intent_id.as_str()
    ))
    .map_err(RuntimeError::from)
}

#[cfg(test)]
mod account_scoped_tests {
    use super::*;
    use domain::AccountId;

    #[test]
    fn account_scoped_ids_are_stable_and_distinct_across_accounts() {
        let intent = TradeIntentId::new("intent-42").unwrap();
        let first = AccountId::new("account-a").unwrap();
        let second = AccountId::new("account-b").unwrap();
        assert_eq!(
            client_order_id_for_account_intent(&first, &intent).unwrap(),
            client_order_id_for_account_intent(&first, &intent).unwrap()
        );
        assert_ne!(
            client_order_id_for_account_intent(&first, &intent).unwrap(),
            client_order_id_for_account_intent(&second, &intent).unwrap()
        );
    }
}
