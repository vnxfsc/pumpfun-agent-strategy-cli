use std::collections::HashMap;

use crate::{
    model::{
        EventEnvelope, ExecutionReport, FillReport, OrderRequest, OrderSide, PendingOrder,
        PumpEvent, RejectedOrder, RejectionReason,
    },
    state::MarketState,
};

const BPS_DENOMINATOR: u64 = 10_000;

#[derive(Debug, Clone, Copy)]
pub struct BrokerConfig {
    pub starting_cash_lamports: u64,
    pub trading_fee_bps: u64,
    pub slippage_bps: u64,
}

impl Default for BrokerConfig {
    fn default() -> Self {
        Self {
            starting_cash_lamports: 10 * 1_000_000_000,
            trading_fee_bps: 100,
            slippage_bps: 50,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Position {
    pub mint: String,
    pub token_amount: u64,
    pub average_entry_price_lamports_per_token: f64,
    pub entry_notional_lamports: u64,
    pub opened_at: Option<i64>,
    pub last_mark_price_lamports_per_token: f64,
    pub realized_pnl_lamports: i128,
}

#[derive(Debug, Clone)]
pub struct BrokerSnapshot {
    pub cash_lamports: u64,
    pub positions: HashMap<String, Position>,
    pub pending_orders: usize,
}

impl BrokerSnapshot {
    pub fn position(&self, mint: &str) -> Option<&Position> {
        self.positions.get(mint)
    }
}

#[derive(Debug)]
pub struct PaperBroker {
    config: BrokerConfig,
    cash_lamports: u64,
    positions: HashMap<String, Position>,
    pending_orders: Vec<PendingOrder>,
    next_order_id: u64,
}

impl PaperBroker {
    pub fn new(config: BrokerConfig) -> Self {
        Self {
            cash_lamports: config.starting_cash_lamports,
            config,
            positions: HashMap::new(),
            pending_orders: Vec::new(),
            next_order_id: 1,
        }
    }

    pub fn snapshot(&self) -> BrokerSnapshot {
        BrokerSnapshot {
            cash_lamports: self.cash_lamports,
            positions: self.positions.clone(),
            pending_orders: self.pending_orders.len(),
        }
    }

    pub fn submit_orders(
        &mut self,
        envelope: &EventEnvelope,
        orders: impl IntoIterator<Item = OrderRequest>,
    ) {
        for request in orders {
            let pending = PendingOrder {
                id: self.next_order_id,
                submitted_at_seq: envelope.seq,
                submitted_at_ts: envelope.timestamp(),
                request,
            };
            self.next_order_id += 1;
            self.pending_orders.push(pending);
        }
    }

    pub fn process_event(&mut self, envelope: &EventEnvelope) -> Vec<ExecutionReport> {
        let PumpEvent::Trade(trade) = &envelope.event else {
            return Vec::new();
        };
        let current_trade_price = trade.price_lamports_per_token();

        let mut reports = Vec::new();
        let mut next_pending = Vec::with_capacity(self.pending_orders.len());

        for pending in self.pending_orders.drain(..) {
            if pending.submitted_at_seq >= envelope.seq || pending.request.mint() != trade.mint {
                next_pending.push(pending);
                continue;
            }

            if current_trade_price <= 0.0 {
                reports.push(ExecutionReport::Rejected(RejectedOrder {
                    order_id: pending.id,
                    mint: trade.mint.clone(),
                    reason: reason_from_request(&pending.request),
                    rejection: RejectionReason::ZeroPrice,
                    timestamp: envelope.timestamp(),
                }));
                continue;
            }

            match pending.request {
                OrderRequest::BuyForLamports {
                    mint,
                    lamports,
                    reason,
                } => {
                    if self.positions.contains_key(&mint) {
                        reports.push(ExecutionReport::Rejected(RejectedOrder {
                            order_id: pending.id,
                            mint,
                            reason,
                            rejection: RejectionReason::DuplicatePosition,
                            timestamp: envelope.timestamp(),
                        }));
                        continue;
                    }

                    let fee_lamports = compute_bps_charge(lamports, self.config.trading_fee_bps);
                    let total_cash_needed = lamports.saturating_add(fee_lamports);

                    if total_cash_needed > self.cash_lamports {
                        reports.push(ExecutionReport::Rejected(RejectedOrder {
                            order_id: pending.id,
                            mint,
                            reason,
                            rejection: RejectionReason::InsufficientCash,
                            timestamp: envelope.timestamp(),
                        }));
                        continue;
                    }

                    let adjusted_price = current_trade_price
                        * (1.0 + self.config.slippage_bps as f64 / BPS_DENOMINATOR as f64);
                    let token_amount = (lamports as f64 / adjusted_price).floor() as u64;
                    if token_amount == 0 {
                        reports.push(ExecutionReport::Rejected(RejectedOrder {
                            order_id: pending.id,
                            mint,
                            reason,
                            rejection: RejectionReason::ZeroPrice,
                            timestamp: envelope.timestamp(),
                        }));
                        continue;
                    }

                    self.cash_lamports -= total_cash_needed;
                    self.positions.insert(
                        mint.clone(),
                        Position {
                            mint: mint.clone(),
                            token_amount,
                            average_entry_price_lamports_per_token: adjusted_price,
                            entry_notional_lamports: lamports,
                            opened_at: envelope.timestamp(),
                            last_mark_price_lamports_per_token: current_trade_price,
                            realized_pnl_lamports: 0,
                        },
                    );
                    reports.push(ExecutionReport::Filled(FillReport {
                        order_id: pending.id,
                        mint,
                        side: OrderSide::Buy,
                        lamports,
                        token_amount,
                        fee_lamports,
                        execution_price_lamports_per_token: adjusted_price,
                        timestamp: envelope.timestamp(),
                        reason,
                    }));
                }
                OrderRequest::SellAll { mint, reason } => {
                    let Some(position) = self.positions.remove(&mint) else {
                        reports.push(ExecutionReport::Rejected(RejectedOrder {
                            order_id: pending.id,
                            mint,
                            reason,
                            rejection: RejectionReason::EmptyPosition,
                            timestamp: envelope.timestamp(),
                        }));
                        continue;
                    };

                    let adjusted_price = current_trade_price
                        * (1.0 - self.config.slippage_bps as f64 / BPS_DENOMINATOR as f64);
                    let gross_lamports =
                        (position.token_amount as f64 * adjusted_price).floor() as u64;
                    let fee_lamports =
                        compute_bps_charge(gross_lamports, self.config.trading_fee_bps);
                    let net_lamports = gross_lamports.saturating_sub(fee_lamports);
                    self.cash_lamports = self.cash_lamports.saturating_add(net_lamports);

                    reports.push(ExecutionReport::Filled(FillReport {
                        order_id: pending.id,
                        mint,
                        side: OrderSide::Sell,
                        lamports: net_lamports,
                        token_amount: position.token_amount,
                        fee_lamports,
                        execution_price_lamports_per_token: adjusted_price,
                        timestamp: envelope.timestamp(),
                        reason,
                    }));
                }
            }
        }

        if let Some(position) = self.positions.get_mut(&trade.mint) {
            position.last_mark_price_lamports_per_token = current_trade_price;
        }

        self.pending_orders = next_pending;
        reports
    }

    pub fn mark_to_market_lamports(&self, market_state: &MarketState) -> u64 {
        let inventory_value = self
            .positions
            .values()
            .map(|position| {
                let price = market_state
                    .mint(&position.mint)
                    .map(|mint| mint.last_price_lamports_per_token)
                    .unwrap_or(position.last_mark_price_lamports_per_token);
                (position.token_amount as f64 * price).floor() as u64
            })
            .sum::<u64>();

        self.cash_lamports.saturating_add(inventory_value)
    }
}

fn compute_bps_charge(amount: u64, bps: u64) -> u64 {
    amount.saturating_mul(bps).div_ceil(BPS_DENOMINATOR)
}

fn reason_from_request(request: &OrderRequest) -> String {
    match request {
        OrderRequest::BuyForLamports { reason, .. } => reason.clone(),
        OrderRequest::SellAll { reason, .. } => reason.clone(),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        BrokerConfig, PaperBroker,
        model::{EventEnvelope, OrderRequest, PumpEvent, TradeEvent},
    };

    #[test]
    fn fills_buy_on_next_trade_and_then_sell() {
        let mint = "mint-1".to_string();
        let mut broker = PaperBroker::new(BrokerConfig {
            starting_cash_lamports: 10_000,
            trading_fee_bps: 0,
            slippage_bps: 0,
        });

        broker.submit_orders(
            &EventEnvelope {
                seq: 1,
                slot: 1,
                block_time: Some(100),
                tx_signature: "sig-1".to_string(),
                tx_index: 0,
                event_index: 0,
                event: PumpEvent::Trade(sample_trade(&mint, true, 100, 10, 100)),
            },
            [OrderRequest::BuyForLamports {
                mint: mint.clone(),
                lamports: 1_000,
                reason: "entry".to_string(),
            }],
        );

        let reports = broker.process_event(&EventEnvelope {
            seq: 2,
            slot: 2,
            block_time: Some(101),
            tx_signature: "sig-2".to_string(),
            tx_index: 0,
            event_index: 0,
            event: PumpEvent::Trade(sample_trade(&mint, true, 200, 20, 101)),
        });
        assert_eq!(reports.len(), 1);
        assert!(broker.snapshot().position(&mint).is_some());

        broker.submit_orders(
            &EventEnvelope {
                seq: 2,
                slot: 2,
                block_time: Some(101),
                tx_signature: "sig-2".to_string(),
                tx_index: 0,
                event_index: 0,
                event: PumpEvent::Trade(sample_trade(&mint, true, 200, 20, 101)),
            },
            [OrderRequest::SellAll {
                mint: mint.clone(),
                reason: "exit".to_string(),
            }],
        );

        let reports = broker.process_event(&EventEnvelope {
            seq: 3,
            slot: 3,
            block_time: Some(102),
            tx_signature: "sig-3".to_string(),
            tx_index: 0,
            event_index: 0,
            event: PumpEvent::Trade(sample_trade(&mint, false, 300, 10, 102)),
        });
        assert_eq!(reports.len(), 1);
        assert!(broker.snapshot().position(&mint).is_none());
    }

    fn sample_trade(
        mint: &str,
        is_buy: bool,
        sol_amount: u64,
        token_amount: u64,
        timestamp: i64,
    ) -> TradeEvent {
        TradeEvent {
            mint: mint.to_string(),
            sol_amount,
            token_amount,
            is_buy,
            user: "user".to_string(),
            timestamp,
            virtual_sol_reserves: 0,
            virtual_token_reserves: 0,
            real_sol_reserves: 0,
            real_token_reserves: 0,
            fee_recipient: "fee".to_string(),
            fee_basis_points: 0,
            fee: 0,
            creator: "creator".to_string(),
            creator_fee_basis_points: 0,
            creator_fee: 0,
            track_volume: true,
            total_unclaimed_tokens: 0,
            total_claimed_tokens: 0,
            current_sol_volume: 0,
            last_update_timestamp: timestamp,
            ix_name: if is_buy { "buy" } else { "sell" }.to_string(),
            mayhem_mode: false,
            cashback_fee_basis_points: 0,
            cashback: 0,
        }
    }
}
