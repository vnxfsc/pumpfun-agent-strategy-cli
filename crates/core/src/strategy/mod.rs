mod breakout;
mod early_flow;
mod liquidity_follow;
mod momentum;
mod noop;
// strategy-scaffold: mod

use crate::{
    broker::BrokerSnapshot,
    model::{EventEnvelope, ExecutionReport, OrderRequest},
    state::MarketState,
};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

pub use breakout::{BreakoutStrategy, BreakoutStrategyConfig};
pub use early_flow::{EarlyFlowStrategy, EarlyFlowStrategyConfig};
pub use liquidity_follow::{LiquidityFollowStrategy, LiquidityFollowStrategyConfig};
pub use momentum::{MomentumStrategy, MomentumStrategyConfig};
pub use noop::NoopStrategy;
// strategy-scaffold: pub-use

#[derive(Debug, Clone, Serialize)]
pub struct StrategyMetadata {
    pub name: &'static str,
}

pub trait Strategy {
    fn metadata(&self) -> StrategyMetadata;

    fn on_event(
        &mut self,
        event: &EventEnvelope,
        market_state: &MarketState,
        broker: &BrokerSnapshot,
    ) -> Vec<OrderRequest>;

    fn on_execution_reports(
        &mut self,
        _reports: &[ExecutionReport],
        _market_state: &MarketState,
        _broker: &BrokerSnapshot,
    ) {
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StrategyKind {
    #[default]
    Momentum,
    EarlyFlow,
    Breakout,
    LiquidityFollow,
    Noop,
    // strategy-scaffold: kind-variant
}

impl StrategyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Momentum => "momentum",
            Self::EarlyFlow => "early_flow",
            Self::Breakout => "breakout",
            Self::LiquidityFollow => "liquidity_follow",
            Self::Noop => "noop",
            // strategy-scaffold: kind-as-str
        }
    }
}

impl fmt::Display for StrategyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for StrategyKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "momentum" => Ok(Self::Momentum),
            "early_flow" | "early-flow" | "flow" => Ok(Self::EarlyFlow),
            "breakout" => Ok(Self::Breakout),
            "liquidity_follow" | "liquidity-follow" | "liquidity" => Ok(Self::LiquidityFollow),
            "noop" => Ok(Self::Noop),
            // strategy-scaffold: kind-from-str
            other => Err(format!(
                "unsupported strategy '{}', expected a registered strategy kind",
                other
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub enum AnyStrategy {
    Momentum(MomentumStrategy),
    EarlyFlow(EarlyFlowStrategy),
    Breakout(BreakoutStrategy),
    LiquidityFollow(LiquidityFollowStrategy),
    Noop(NoopStrategy),
    // strategy-scaffold: any-strategy-variant
}

impl Strategy for AnyStrategy {
    fn metadata(&self) -> StrategyMetadata {
        match self {
            Self::Momentum(strategy) => strategy.metadata(),
            Self::EarlyFlow(strategy) => strategy.metadata(),
            Self::Breakout(strategy) => strategy.metadata(),
            Self::LiquidityFollow(strategy) => strategy.metadata(),
            Self::Noop(strategy) => strategy.metadata(),
            // strategy-scaffold: metadata-arm
        }
    }

    fn on_event(
        &mut self,
        event: &EventEnvelope,
        market_state: &MarketState,
        broker: &BrokerSnapshot,
    ) -> Vec<OrderRequest> {
        match self {
            Self::Momentum(strategy) => strategy.on_event(event, market_state, broker),
            Self::EarlyFlow(strategy) => strategy.on_event(event, market_state, broker),
            Self::Breakout(strategy) => strategy.on_event(event, market_state, broker),
            Self::LiquidityFollow(strategy) => strategy.on_event(event, market_state, broker),
            Self::Noop(strategy) => strategy.on_event(event, market_state, broker),
            // strategy-scaffold: on-event-arm
        }
    }

    fn on_execution_reports(
        &mut self,
        reports: &[ExecutionReport],
        market_state: &MarketState,
        broker: &BrokerSnapshot,
    ) {
        match self {
            Self::Momentum(strategy) => {
                strategy.on_execution_reports(reports, market_state, broker)
            }
            Self::EarlyFlow(strategy) => {
                strategy.on_execution_reports(reports, market_state, broker)
            }
            Self::Breakout(strategy) => {
                strategy.on_execution_reports(reports, market_state, broker)
            }
            Self::LiquidityFollow(strategy) => {
                strategy.on_execution_reports(reports, market_state, broker)
            }
            Self::Noop(strategy) => strategy.on_execution_reports(reports, market_state, broker),
            // strategy-scaffold: on-execution-arm
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StrategyKind;
    use std::str::FromStr;

    #[test]
    fn parses_known_strategy_kinds() {
        assert_eq!(
            StrategyKind::from_str("momentum").unwrap(),
            StrategyKind::Momentum
        );
        assert_eq!(
            StrategyKind::from_str("early_flow").unwrap(),
            StrategyKind::EarlyFlow
        );
        assert_eq!(
            StrategyKind::from_str("breakout").unwrap(),
            StrategyKind::Breakout
        );
        assert_eq!(
            StrategyKind::from_str("liquidity_follow").unwrap(),
            StrategyKind::LiquidityFollow
        );
        assert_eq!(StrategyKind::from_str("noop").unwrap(), StrategyKind::Noop);
    }
}
