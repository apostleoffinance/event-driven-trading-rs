//! Order management system with idempotent create and full lifecycle.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use domain::{ClientOrderId, Fill, Order, OrderId, OrderStatus, RiskDecision};
use rust_decimal::Decimal;

use crate::audit::{AuditEvent, AuditKind};
use crate::error::{ExecutionError, ExecutionResult};
use crate::ids::{next_fill_id, next_order_id};
use crate::request::NewOrderRequest;

/// Result of create (new or idempotent replay).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateOrderOutcome {
    pub order: Order,
    /// `false` when the same `client_order_id` was replayed.
    pub created: bool,
}

/// In-memory OMS / execution coordinator.
///
/// Venue I/O goes through `venue_connectors::VenueAdapter` via
/// `submit_approved_order` — this type stays venue-agnostic.
#[derive(Debug, Default)]
pub struct ExecutionEngine {
    orders: HashMap<OrderId, Order>,
    by_client_id: HashMap<ClientOrderId, OrderId>,
    fills: Vec<Fill>,
    /// Risk decision recorded per order for auditability.
    risk_by_order: HashMap<OrderId, RiskDecision>,
    audit: Vec<AuditEvent>,
}

impl ExecutionEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, order_id: &OrderId) -> ExecutionResult<&Order> {
        self.orders
            .get(order_id)
            .ok_or_else(|| ExecutionError::OrderNotFound(order_id.as_str().to_string()))
    }

    pub fn get_by_client_id(&self, client_order_id: &ClientOrderId) -> ExecutionResult<&Order> {
        let order_id = self
            .by_client_id
            .get(client_order_id)
            .ok_or_else(|| ExecutionError::OrderNotFound(client_order_id.as_str().to_string()))?;
        self.get(order_id)
    }

    pub fn fills_for(&self, order_id: &OrderId) -> Vec<&Fill> {
        self.fills
            .iter()
            .filter(|f| &f.order_id == order_id)
            .collect()
    }

    pub fn audit_trail(&self) -> &[AuditEvent] {
        &self.audit
    }

    pub fn audit_for_order(&self, order_id: &OrderId) -> Vec<&AuditEvent> {
        self.audit
            .iter()
            .filter(|e| &e.order_id == order_id)
            .collect()
    }

    pub fn risk_decision_for(&self, order_id: &OrderId) -> Option<&RiskDecision> {
        self.risk_by_order.get(order_id)
    }

    /// All orders currently tracked by the OMS.
    pub fn orders(&self) -> Vec<&Order> {
        self.orders.values().collect()
    }

    /// Non-terminal orders for an account (open / working).
    pub fn open_orders_for(&self, account_id: &domain::AccountId) -> Vec<&Order> {
        self.orders
            .values()
            .filter(|o| &o.account_id == account_id && !o.status.is_terminal())
            .collect()
    }

    /// Create an order from a risk-approved request.
    ///
    /// Retries with the same `client_order_id` return the existing order without
    /// creating a duplicate (idempotency).
    pub fn create_order(
        &mut self,
        request: NewOrderRequest,
    ) -> ExecutionResult<CreateOrderOutcome> {
        self.validate_request_ids(&request)?;
        let quantity = request.validated_quantity()?;

        if let Some(existing_id) = self.by_client_id.get(&request.client_order_id).cloned() {
            let existing = self.get(&existing_id)?.clone();
            self.assert_idempotent_match(&existing, &request, quantity)?;
            self.push_audit(
                existing.updated_at,
                &existing,
                AuditKind::OrderIdempotentReplay,
                "retry returned existing order",
            );
            return Ok(CreateOrderOutcome {
                order: existing,
                created: false,
            });
        }

        let order_id = next_order_id()?;
        let at = request.created_at;
        let mut order = Order::create(
            order_id.clone(),
            request.client_order_id.clone(),
            request.account_id.clone(),
            request.venue_id.clone(),
            request.deployment_id.clone(),
            request.strategy_id.clone(),
            Some(request.trade_intent_id.clone()),
            request.instrument_id.clone(),
            request.side,
            request.order_type,
            request.time_in_force,
            quantity,
            request.price,
            at,
        )?;

        self.push_audit(
            at,
            &order,
            AuditKind::OrderCreated,
            format!("qty={quantity}"),
        );

        order.transition_to(OrderStatus::PendingRisk, at)?;
        self.push_audit(
            at,
            &order,
            AuditKind::PendingRisk,
            "awaiting recorded decision",
        );

        order.transition_to(OrderStatus::Approved, at)?;
        self.push_audit(
            at,
            &order,
            AuditKind::RiskApproved,
            format!("{:?}", request.risk_decision),
        );

        self.risk_by_order
            .insert(order_id.clone(), request.risk_decision.clone());
        self.by_client_id
            .insert(request.client_order_id.clone(), order_id.clone());
        self.orders.insert(order_id, order.clone());

        tracing::info!(
            order_id = %order.id,
            client_order_id = %order.client_order_id,
            account_id = %order.account_id,
            venue_id = %order.venue_id,
            "order created and risk-approved"
        );

        Ok(CreateOrderOutcome {
            order,
            created: true,
        })
    }

    pub fn mark_submitted(
        &mut self,
        order_id: &OrderId,
        at: DateTime<Utc>,
    ) -> ExecutionResult<Order> {
        let order = self.order_mut(order_id)?;
        order.transition_to(OrderStatus::Submitted, at)?;
        let order = order.clone();
        self.push_audit(at, &order, AuditKind::Submitted, "submitted to venue");
        Ok(order)
    }

    pub fn mark_accepted(
        &mut self,
        order_id: &OrderId,
        at: DateTime<Utc>,
    ) -> ExecutionResult<Order> {
        let order = self.order_mut(order_id)?;
        order.transition_to(OrderStatus::Accepted, at)?;
        let order = order.clone();
        self.push_audit(at, &order, AuditKind::Accepted, "accepted by venue");
        Ok(order)
    }

    pub fn mark_rejected(
        &mut self,
        order_id: &OrderId,
        reason: impl Into<String>,
        at: DateTime<Utc>,
    ) -> ExecutionResult<Order> {
        let reason = reason.into();
        let order = self.order_mut(order_id)?;
        order.transition_to(OrderStatus::Rejected, at)?;
        let order = order.clone();
        self.push_audit(at, &order, AuditKind::Rejected, reason);
        Ok(order)
    }

    pub fn mark_failed(
        &mut self,
        order_id: &OrderId,
        reason: impl Into<String>,
        at: DateTime<Utc>,
    ) -> ExecutionResult<Order> {
        let reason = reason.into();
        let order = self.order_mut(order_id)?;
        order.transition_to(OrderStatus::Failed, at)?;
        let order = order.clone();
        self.push_audit(at, &order, AuditKind::Failed, reason);
        Ok(order)
    }

    pub fn request_cancel(
        &mut self,
        order_id: &OrderId,
        at: DateTime<Utc>,
    ) -> ExecutionResult<Order> {
        let order = self.order_mut(order_id)?;
        order.transition_to(OrderStatus::CancelPending, at)?;
        let order = order.clone();
        self.push_audit(at, &order, AuditKind::CancelPending, "cancel requested");
        Ok(order)
    }

    pub fn mark_cancelled(
        &mut self,
        order_id: &OrderId,
        at: DateTime<Utc>,
    ) -> ExecutionResult<Order> {
        let order = self.order_mut(order_id)?;
        order.transition_to(OrderStatus::Cancelled, at)?;
        let order = order.clone();
        self.push_audit(at, &order, AuditKind::Cancelled, "cancelled");
        Ok(order)
    }

    /// Apply a fill; supports partial then full fill.
    pub fn apply_fill(
        &mut self,
        order_id: &OrderId,
        price: Decimal,
        quantity: Decimal,
        fee: Decimal,
        at: DateTime<Utc>,
    ) -> ExecutionResult<(Order, Fill)> {
        let fill_id = next_fill_id()?;
        let instrument_id = self.get(order_id)?.instrument_id.clone();
        let fill = Fill::new(
            fill_id,
            order_id.clone(),
            instrument_id,
            price,
            quantity,
            fee,
            at,
        )?;

        let order = self.order_mut(order_id)?;
        order.apply_fill(quantity, at)?;
        let order = order.clone();

        let kind = if order.status == OrderStatus::Filled {
            AuditKind::Filled
        } else {
            AuditKind::PartiallyFilled
        };
        self.push_audit(
            at,
            &order,
            kind,
            format!("fill_qty={quantity} price={price}"),
        );
        self.fills.push(fill.clone());
        Ok((order, fill))
    }

    /// Convenience: create → submit → accept (still no venue I/O).
    pub fn create_submit_accept(
        &mut self,
        request: NewOrderRequest,
    ) -> ExecutionResult<CreateOrderOutcome> {
        let at = request.created_at;
        let outcome = self.create_order(request)?;
        if !outcome.created {
            return Ok(outcome);
        }
        let id = outcome.order.id.clone();
        self.mark_submitted(&id, at)?;
        let order = self.mark_accepted(&id, at)?;
        Ok(CreateOrderOutcome {
            order,
            created: true,
        })
    }

    fn validate_request_ids(&self, request: &NewOrderRequest) -> ExecutionResult<()> {
        // Strong IDs are already non-empty from domain constructors; keep explicit
        // invariant checks for OMS auditability.
        if request.account_id.as_str().is_empty() {
            return Err(ExecutionError::MissingAccount);
        }
        if request.venue_id.as_str().is_empty() {
            return Err(ExecutionError::MissingVenue);
        }
        if request.client_order_id.as_str().is_empty() {
            return Err(ExecutionError::MissingClientOrderId);
        }
        if !request.risk_decision.is_executable() {
            return Err(ExecutionError::MissingRiskDecision);
        }
        Ok(())
    }

    fn assert_idempotent_match(
        &self,
        existing: &Order,
        request: &NewOrderRequest,
        quantity: Decimal,
    ) -> ExecutionResult<()> {
        if existing.account_id != request.account_id
            || existing.venue_id != request.venue_id
            || existing.instrument_id != request.instrument_id
            || existing.side != request.side
            || existing.quantity != quantity
        {
            return Err(ExecutionError::IdempotencyConflict {
                client_order_id: request.client_order_id.as_str().to_string(),
                reason: "request payload does not match existing order".to_string(),
            });
        }
        Ok(())
    }

    fn order_mut(&mut self, order_id: &OrderId) -> ExecutionResult<&mut Order> {
        self.orders
            .get_mut(order_id)
            .ok_or_else(|| ExecutionError::OrderNotFound(order_id.as_str().to_string()))
    }

    fn push_audit(
        &mut self,
        at: DateTime<Utc>,
        order: &Order,
        kind: AuditKind,
        detail: impl Into<String>,
    ) {
        self.audit.push(AuditEvent::new(
            at,
            order.id.clone(),
            order.client_order_id.clone(),
            kind,
            detail,
        ));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use chrono::Utc;
    use domain::{
        AccountId, ClientOrderId, DeploymentId, InstrumentId, OrderSide, OrderType, RiskDecision,
        StrategyId, TimeInForce, TradeIntentId, VenueId,
    };
    use rust_decimal::Decimal;

    fn sample_request(client: &str, qty: Decimal) -> NewOrderRequest {
        NewOrderRequest {
            client_order_id: ClientOrderId::new(client).unwrap(),
            account_id: AccountId::new("prop-account-001").unwrap(),
            venue_id: VenueId::new("simulated").unwrap(),
            deployment_id: DeploymentId::new("deployment-001").unwrap(),
            strategy_id: StrategyId::new("btc-mean-reversion").unwrap(),
            trade_intent_id: TradeIntentId::new("ti-1").unwrap(),
            instrument_id: InstrumentId::new("BTCUSDT").unwrap(),
            side: OrderSide::Buy,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Ioc,
            price: Some(Decimal::from(100)),
            risk_decision: RiskDecision::Approved { quantity: qty },
            created_at: Utc::now(),
        }
    }

    #[test]
    fn rejects_without_executable_risk() {
        let mut oms = ExecutionEngine::new();
        let mut req = sample_request("clid-1", Decimal::ONE);
        req.risk_decision = RiskDecision::Halted {
            reason: "kill".into(),
        };
        let err = oms.create_order(req).unwrap_err();
        assert!(matches!(
            err,
            ExecutionError::MissingRiskDecision | ExecutionError::RiskNotExecutable(_)
        ));
    }

    #[test]
    fn idempotent_retry_does_not_duplicate() {
        let mut oms = ExecutionEngine::new();
        let req = sample_request("clid-dup", Decimal::from(2));
        let first = oms.create_order(req.clone()).unwrap();
        assert!(first.created);
        let second = oms.create_order(req).unwrap();
        assert!(!second.created);
        assert_eq!(first.order.id, second.order.id);
        assert_eq!(oms.orders.len(), 1);
    }

    #[test]
    fn partial_then_full_fill() {
        let mut oms = ExecutionEngine::new();
        let outcome = oms
            .create_submit_accept(sample_request("clid-fill", Decimal::from(2)))
            .unwrap();
        let id = outcome.order.id;
        let now = Utc::now();
        let (o1, _) = oms
            .apply_fill(&id, Decimal::from(100), Decimal::ONE, Decimal::ZERO, now)
            .unwrap();
        assert_eq!(o1.status, OrderStatus::PartiallyFilled);
        let (o2, _) = oms
            .apply_fill(&id, Decimal::from(100), Decimal::ONE, Decimal::ZERO, now)
            .unwrap();
        assert_eq!(o2.status, OrderStatus::Filled);
        assert!(!oms.audit_for_order(&id).is_empty());
        assert!(oms.risk_decision_for(&id).is_some());
    }

    #[test]
    fn cancel_lifecycle() {
        let mut oms = ExecutionEngine::new();
        let outcome = oms
            .create_submit_accept(sample_request("clid-cx", Decimal::ONE))
            .unwrap();
        let id = outcome.order.id;
        let now = Utc::now();
        oms.request_cancel(&id, now).unwrap();
        let order = oms.mark_cancelled(&id, now).unwrap();
        assert_eq!(order.status, OrderStatus::Cancelled);
    }

    #[test]
    fn every_order_has_client_account_venue() {
        let mut oms = ExecutionEngine::new();
        let order = oms
            .create_order(sample_request("clid-ids", Decimal::ONE))
            .unwrap()
            .order;
        assert!(!order.client_order_id.as_str().is_empty());
        assert!(!order.account_id.as_str().is_empty());
        assert!(!order.venue_id.as_str().is_empty());
    }
}
