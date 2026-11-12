//! Shared quantitative trading domain primitives.
//!
//! This crate owns business concepts only. It must not depend on:
//! databases, HTTP clients, exchange SDKs, strategy implementations,
//! or runtime orchestration.
//!
//! Money, prices, quantities, and risk ratios use [`rust_decimal::Decimal`].
//! All fallible construction returns [`DomainError`] via [`DomainResult`].

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod account;
pub mod broker_registry;
pub mod error;
pub mod execution_authorization;
pub mod fill;
pub mod ids;
pub mod instrument;
pub mod journal;
pub mod journal_analytics;
pub mod money;
pub mod order;
pub mod organization;
pub mod position;
pub mod risk;
pub mod strategy;
pub mod trade_intent;
pub mod venue;

pub use account::{Account, AccountSnapshot, AccountState, AccountStatus, AccountType};
pub use broker_registry::{
    normalize_broker_id, normalize_external_account_identifier, normalize_mt5_server_id, Broker,
    BrokerAccountEvidence, BrokerAccountVerification, BrokerAccountVerificationStatus,
    BrokerBindingStatus, BrokerRegistryStatus, BrokerServer, ExternalBrokerAccount,
    NormalizedBrokerIdentity, QuantBrokerAccountBinding, BROKER_ID_NORMALIZATION_VERSION,
    EXTERNAL_ACCOUNT_IDENTIFIER_NORMALIZATION_VERSION, SERVER_ID_NORMALIZATION_VERSION,
};
pub use error::{DomainError, DomainResult};
pub use execution_authorization::{
    require_account_broker_eligibility, require_verified_live_environment, AccountEnvironment,
    BrokerAuthorizationError, EnvironmentVerification,
};
pub use fill::Fill;
pub use ids::*;
pub use instrument::{Instrument, InstrumentSpec, InstrumentType};
pub use journal::{JournalDirection, JournalTrade, JournalTradeSource, JournalTradeStatus};
pub use journal_analytics::{
    calculate_position_size, summarize_net_pnl, AnalyticsSummary, PositionSizingInput,
    PositionSizingResult,
};
pub use order::{Order, OrderSide, OrderStatus, OrderType, TimeInForce};
pub use organization::{
    EntitlementStatus, MembershipStatus, Organization, OrganizationAccountEntitlement,
    OrganizationMembership, OrganizationRole, OrganizationStatus,
};
pub use position::{Position, PositionSide};
pub use risk::{RiskDecision, RiskPolicy, RiskProfile, RiskRejectReason, RiskRequest};
pub use strategy::{Strategy, StrategyDeployment, StrategyMetadata};
pub use trade_intent::TradeIntent;
pub use venue::{Venue, VenueType};
