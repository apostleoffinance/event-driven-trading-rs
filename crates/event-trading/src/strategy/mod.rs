pub mod mean_reversion;
#[allow(clippy::module_inception)]
pub mod strategy;
pub mod strategy_factory;

pub use mean_reversion::MeanReversionStrategy;
pub use strategy::{Signal, Strategy};
pub use strategy_factory::StrategyFactory;
