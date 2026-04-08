use crate::{
    broker::BrokerSnapshot,
    model::{EventEnvelope, OrderRequest},
    state::MarketState,
};

use super::{Strategy, StrategyMetadata};

#[derive(Debug, Clone, Default)]
pub struct NoopStrategy;

impl NoopStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Strategy for NoopStrategy {
    fn metadata(&self) -> StrategyMetadata {
        StrategyMetadata {
            name: "noop_strategy",
        }
    }

    fn on_event(
        &mut self,
        _event: &EventEnvelope,
        _market_state: &MarketState,
        _broker: &BrokerSnapshot,
    ) -> Vec<OrderRequest> {
        Vec::new()
    }
}
