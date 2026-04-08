use crate::{
    broker::{BrokerSnapshot, Position},
    model::{EventEnvelope, OrderRequest, PumpEvent},
    state::{MarketState, MintState},
};

use super::{Strategy, StrategyMetadata};

#[derive(Debug, Clone)]
pub struct MomentumStrategyConfig {
    pub max_age_secs: i64,
    pub min_buy_count: u64,
    pub min_unique_buyers: usize,
    pub min_net_flow_lamports: i128,
    pub buy_lamports: u64,
    pub take_profit_bps: i64,
    pub stop_loss_bps: i64,
    pub max_hold_secs: i64,
}

impl Default for MomentumStrategyConfig {
    fn default() -> Self {
        Self {
            max_age_secs: 45,
            min_buy_count: 3,
            min_unique_buyers: 3,
            min_net_flow_lamports: 3 * 100_000_000,
            buy_lamports: 200_000_000,
            take_profit_bps: 2_500,
            stop_loss_bps: 1_200,
            max_hold_secs: 90,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MomentumStrategy {
    config: MomentumStrategyConfig,
}

impl MomentumStrategy {
    pub fn new(config: MomentumStrategyConfig) -> Self {
        Self { config }
    }

    fn maybe_entry(
        &self,
        mint_state: &MintState,
        event: &EventEnvelope,
        broker: &BrokerSnapshot,
    ) -> Option<OrderRequest> {
        let PumpEvent::Trade(trade) = &event.event else {
            return None;
        };

        if !trade.is_buy || broker.position(&mint_state.mint).is_some() {
            return None;
        }

        let age_secs = mint_state.age_secs_at(trade.timestamp)?;
        if age_secs < 0 || age_secs > self.config.max_age_secs {
            return None;
        }

        if mint_state.buy_count < self.config.min_buy_count {
            return None;
        }

        if mint_state.unique_buyer_count() < self.config.min_unique_buyers {
            return None;
        }

        if mint_state.net_flow_lamports < self.config.min_net_flow_lamports {
            return None;
        }

        Some(OrderRequest::BuyForLamports {
            mint: mint_state.mint.clone(),
            lamports: self.config.buy_lamports,
            reason: format!(
                "entry age={}s buys={} unique_buyers={} net_flow={}",
                age_secs,
                mint_state.buy_count,
                mint_state.unique_buyer_count(),
                mint_state.net_flow_lamports
            ),
        })
    }

    fn maybe_exit(
        &self,
        mint_state: &MintState,
        now_ts: i64,
        position: &Position,
    ) -> Option<OrderRequest> {
        if position.token_amount == 0 || position.average_entry_price_lamports_per_token <= 0.0 {
            return None;
        }

        let current_price = mint_state.last_price_lamports_per_token;
        if current_price <= 0.0 {
            return None;
        }

        let pnl_bps =
            ((current_price / position.average_entry_price_lamports_per_token) - 1.0) * 10_000.0;
        if pnl_bps >= self.config.take_profit_bps as f64 {
            return Some(OrderRequest::SellAll {
                mint: mint_state.mint.clone(),
                reason: format!("take_profit {:.0}bps", pnl_bps),
            });
        }

        if pnl_bps <= -(self.config.stop_loss_bps as f64) {
            return Some(OrderRequest::SellAll {
                mint: mint_state.mint.clone(),
                reason: format!("stop_loss {:.0}bps", pnl_bps),
            });
        }

        if let Some(opened_at) = position.opened_at {
            let held_for = now_ts - opened_at;
            if held_for >= self.config.max_hold_secs {
                return Some(OrderRequest::SellAll {
                    mint: mint_state.mint.clone(),
                    reason: format!("time_exit held={}s", held_for),
                });
            }
        }

        if mint_state.is_complete {
            return Some(OrderRequest::SellAll {
                mint: mint_state.mint.clone(),
                reason: "curve_complete".to_string(),
            });
        }

        None
    }
}

impl Strategy for MomentumStrategy {
    fn metadata(&self) -> StrategyMetadata {
        StrategyMetadata {
            name: "momentum_strategy",
        }
    }

    fn on_event(
        &mut self,
        event: &EventEnvelope,
        market_state: &MarketState,
        broker: &BrokerSnapshot,
    ) -> Vec<OrderRequest> {
        let Some(mint) = event.mint() else {
            return Vec::new();
        };
        let Some(mint_state) = market_state.mint(mint) else {
            return Vec::new();
        };

        let mut requests = Vec::new();
        if let Some(position) = broker.position(mint) {
            if let Some(now_ts) = event.timestamp()
                && let Some(exit) = self.maybe_exit(mint_state, now_ts, position)
            {
                requests.push(exit);
            }
            return requests;
        }

        if let Some(entry) = self.maybe_entry(mint_state, event, broker) {
            requests.push(entry);
        }

        requests
    }
}
