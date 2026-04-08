use std::collections::{HashMap, HashSet};

use crate::{
    broker::{BrokerSnapshot, Position},
    model::{EventEnvelope, ExecutionReport, OrderRequest, OrderSide, PumpEvent},
    state::{MarketState, MintState},
};

use super::{Strategy, StrategyMetadata};

#[derive(Debug, Clone)]
pub struct BreakoutStrategyConfig {
    pub max_age_secs: i64,
    pub min_buy_count: u64,
    pub min_unique_buyers: usize,
    pub min_total_buy_lamports: u128,
    pub min_net_flow_lamports: i128,
    pub max_sell_count: u64,
    pub min_buy_sell_ratio: f64,
    pub buy_lamports: u64,
    pub take_profit_bps: i64,
    pub stop_loss_bps: i64,
    pub max_hold_secs: i64,
    pub max_concurrent_positions: usize,
    pub exit_on_sell_count: u64,
}

impl Default for BreakoutStrategyConfig {
    fn default() -> Self {
        Self {
            max_age_secs: 35,
            min_buy_count: 5,
            min_unique_buyers: 5,
            min_total_buy_lamports: 1_200_000_000,
            min_net_flow_lamports: 700_000_000,
            max_sell_count: 2,
            min_buy_sell_ratio: 3.5,
            buy_lamports: 180_000_000,
            take_profit_bps: 2_200,
            stop_loss_bps: 900,
            max_hold_secs: 75,
            max_concurrent_positions: 3,
            exit_on_sell_count: 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BreakoutStrategy {
    config: BreakoutStrategyConfig,
    pending_entries: HashSet<String>,
    pending_exits: HashSet<String>,
    entry_sell_counts: HashMap<String, u64>,
}

impl BreakoutStrategy {
    pub fn new(config: BreakoutStrategyConfig) -> Self {
        Self {
            config,
            pending_entries: HashSet::new(),
            pending_exits: HashSet::new(),
            entry_sell_counts: HashMap::new(),
        }
    }

    fn maybe_entry(
        &mut self,
        mint_state: &MintState,
        event: &EventEnvelope,
        broker: &BrokerSnapshot,
    ) -> Option<OrderRequest> {
        let PumpEvent::Trade(trade) = &event.event else {
            return None;
        };

        if !trade.is_buy {
            return None;
        }

        if broker.position(&mint_state.mint).is_some()
            || self.pending_entries.contains(&mint_state.mint)
        {
            return None;
        }

        let exposure_count = broker.positions.len() + self.pending_entries.len();
        if exposure_count >= self.config.max_concurrent_positions {
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

        if mint_state.buy_volume_lamports < self.config.min_total_buy_lamports {
            return None;
        }

        if mint_state.net_flow_lamports < self.config.min_net_flow_lamports {
            return None;
        }

        if mint_state.sell_count > self.config.max_sell_count {
            return None;
        }

        let buy_sell_ratio = mint_state.buy_count as f64 / mint_state.sell_count.max(1) as f64;
        if buy_sell_ratio < self.config.min_buy_sell_ratio {
            return None;
        }

        self.pending_entries.insert(mint_state.mint.clone());
        Some(OrderRequest::BuyForLamports {
            mint: mint_state.mint.clone(),
            lamports: self.config.buy_lamports,
            reason: format!(
                "breakout age={}s buys={} unique_buyers={} total_buy={} net_flow={} sells={} ratio={:.2}",
                age_secs,
                mint_state.buy_count,
                mint_state.unique_buyer_count(),
                mint_state.buy_volume_lamports,
                mint_state.net_flow_lamports,
                mint_state.sell_count,
                buy_sell_ratio,
            ),
        })
    }

    fn maybe_exit(
        &mut self,
        mint_state: &MintState,
        event: &EventEnvelope,
        position: &Position,
    ) -> Option<OrderRequest> {
        if self.pending_exits.contains(&mint_state.mint) {
            return None;
        }

        if position.token_amount == 0 || position.average_entry_price_lamports_per_token <= 0.0 {
            return None;
        }

        let current_price = mint_state.last_price_lamports_per_token;
        if current_price <= 0.0 {
            return None;
        }

        let pnl_bps =
            ((current_price / position.average_entry_price_lamports_per_token) - 1.0) * 10_000.0;
        let mut reason = None;

        if pnl_bps >= self.config.take_profit_bps as f64 {
            reason = Some(format!("take_profit {:.0}bps", pnl_bps));
        } else if pnl_bps <= -(self.config.stop_loss_bps as f64) {
            reason = Some(format!("stop_loss {:.0}bps", pnl_bps));
        } else if let Some(opened_at) = position.opened_at
            && let Some(now_ts) = event.timestamp()
        {
            let held_for = now_ts - opened_at;
            if held_for >= self.config.max_hold_secs {
                reason = Some(format!("time_exit held={}s", held_for));
            }
        }

        if reason.is_none() && mint_state.is_complete {
            reason = Some("curve_complete".to_string());
        }

        let entry_sell_count = self
            .entry_sell_counts
            .get(&mint_state.mint)
            .copied()
            .unwrap_or_default();
        let post_entry_sells = mint_state.sell_count.saturating_sub(entry_sell_count);
        if reason.is_none() && post_entry_sells >= self.config.exit_on_sell_count {
            reason = Some(format!(
                "breakout_faded sells_since_entry={}",
                post_entry_sells
            ));
        }

        let reason = reason?;
        self.pending_exits.insert(mint_state.mint.clone());
        Some(OrderRequest::SellAll {
            mint: mint_state.mint.clone(),
            reason,
        })
    }
}

impl Strategy for BreakoutStrategy {
    fn metadata(&self) -> StrategyMetadata {
        StrategyMetadata {
            name: "breakout_strategy",
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

        if let Some(position) = broker.position(mint) {
            if let Some(exit) = self.maybe_exit(mint_state, event, position) {
                return vec![exit];
            }
            return Vec::new();
        }

        self.maybe_entry(mint_state, event, broker)
            .into_iter()
            .collect()
    }

    fn on_execution_reports(
        &mut self,
        reports: &[ExecutionReport],
        market_state: &MarketState,
        _broker: &BrokerSnapshot,
    ) {
        for report in reports {
            match report {
                ExecutionReport::Filled(fill) => match fill.side {
                    OrderSide::Buy => {
                        self.pending_entries.remove(&fill.mint);
                        let sell_count = market_state
                            .mint(&fill.mint)
                            .map(|state| state.sell_count)
                            .unwrap_or_default();
                        self.entry_sell_counts.insert(fill.mint.clone(), sell_count);
                    }
                    OrderSide::Sell => {
                        self.pending_exits.remove(&fill.mint);
                        self.entry_sell_counts.remove(&fill.mint);
                    }
                },
                ExecutionReport::Rejected(rejected) => {
                    self.pending_entries.remove(&rejected.mint);
                    self.pending_exits.remove(&rejected.mint);
                }
            }
        }
    }
}
