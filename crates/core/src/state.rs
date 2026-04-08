use std::collections::{HashMap, HashSet};

use crate::model::{
    CurveCompletedEvent, EventEnvelope, MintCreatedEvent, OrderSide, PumpEvent, TradeEvent,
};

#[derive(Debug, Default)]
pub struct MarketState {
    mints: HashMap<String, MintState>,
}

impl MarketState {
    pub fn apply(&mut self, envelope: &EventEnvelope) {
        match &envelope.event {
            PumpEvent::MintCreated(event) => self.apply_mint_created(envelope, event),
            PumpEvent::Trade(event) => self.apply_trade(envelope, event),
            PumpEvent::CurveCompleted(event) => self.apply_curve_completed(envelope, event),
        }
    }

    pub fn mint(&self, mint: &str) -> Option<&MintState> {
        self.mints.get(mint)
    }

    pub fn mints(&self) -> &HashMap<String, MintState> {
        &self.mints
    }

    fn apply_mint_created(&mut self, envelope: &EventEnvelope, event: &MintCreatedEvent) {
        let state = self
            .mints
            .entry(event.mint.clone())
            .or_insert_with(|| MintState::new(&event.mint));

        state.bonding_curve = Some(event.bonding_curve.clone());
        state.creator = Some(event.creator.clone());
        state.name = Some(event.name.clone());
        state.symbol = Some(event.symbol.clone());
        state.uri = Some(event.uri.clone());
        state.created_at = Some(event.timestamp);
        state.created_slot = Some(envelope.slot);
        state.is_mayhem_mode = event.is_mayhem_mode;
        state.is_cashback_enabled = event.is_cashback_enabled;
        state.virtual_sol_reserves = event.virtual_sol_reserves;
        state.virtual_token_reserves = event.virtual_token_reserves;
        state.real_sol_reserves = 0;
        state.real_token_reserves = event.real_token_reserves;
        state.token_total_supply = event.token_total_supply;
    }

    fn apply_trade(&mut self, envelope: &EventEnvelope, event: &TradeEvent) {
        let state = self
            .mints
            .entry(event.mint.clone())
            .or_insert_with(|| MintState::new(&event.mint));

        state.last_trade_at = Some(event.timestamp);
        state.last_trade_slot = Some(envelope.slot);
        state.trade_count += 1;
        state.last_price_lamports_per_token = event.price_lamports_per_token();
        state.virtual_sol_reserves = event.virtual_sol_reserves;
        state.virtual_token_reserves = event.virtual_token_reserves;
        state.real_sol_reserves = event.real_sol_reserves;
        state.real_token_reserves = event.real_token_reserves;
        state.is_mayhem_mode = event.mayhem_mode;
        state.creator = Some(event.creator.clone());
        state.protocol_fee_lamports += event.fee as u128;
        state.creator_fee_lamports += event.creator_fee as u128;
        state.cashback_lamports += event.cashback as u128;

        match event.side() {
            OrderSide::Buy => {
                state.buy_count += 1;
                state.buy_volume_lamports += event.sol_amount as u128;
                state.net_flow_lamports += event.sol_amount as i128;
                state.unique_buyers.insert(event.user.clone());
            }
            OrderSide::Sell => {
                state.sell_count += 1;
                state.sell_volume_lamports += event.sol_amount as u128;
                state.net_flow_lamports -= event.sol_amount as i128;
                state.unique_sellers.insert(event.user.clone());
            }
        }
    }

    fn apply_curve_completed(&mut self, _: &EventEnvelope, event: &CurveCompletedEvent) {
        let state = self
            .mints
            .entry(event.mint.clone())
            .or_insert_with(|| MintState::new(&event.mint));

        state.is_complete = true;
        state.completed_at = Some(event.timestamp);
        state.bonding_curve = Some(event.bonding_curve.clone());
    }
}

#[derive(Debug, Clone)]
pub struct MintState {
    pub mint: String,
    pub bonding_curve: Option<String>,
    pub creator: Option<String>,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub uri: Option<String>,
    pub created_at: Option<i64>,
    pub created_slot: Option<u64>,
    pub completed_at: Option<i64>,
    pub is_complete: bool,
    pub is_mayhem_mode: bool,
    pub is_cashback_enabled: bool,
    pub trade_count: u64,
    pub buy_count: u64,
    pub sell_count: u64,
    pub buy_volume_lamports: u128,
    pub sell_volume_lamports: u128,
    pub net_flow_lamports: i128,
    pub protocol_fee_lamports: u128,
    pub creator_fee_lamports: u128,
    pub cashback_lamports: u128,
    pub virtual_sol_reserves: u64,
    pub virtual_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub token_total_supply: u64,
    pub last_trade_at: Option<i64>,
    pub last_trade_slot: Option<u64>,
    pub last_price_lamports_per_token: f64,
    unique_buyers: HashSet<String>,
    unique_sellers: HashSet<String>,
}

impl MintState {
    pub fn new(mint: &str) -> Self {
        Self {
            mint: mint.to_owned(),
            bonding_curve: None,
            creator: None,
            name: None,
            symbol: None,
            uri: None,
            created_at: None,
            created_slot: None,
            completed_at: None,
            is_complete: false,
            is_mayhem_mode: false,
            is_cashback_enabled: false,
            trade_count: 0,
            buy_count: 0,
            sell_count: 0,
            buy_volume_lamports: 0,
            sell_volume_lamports: 0,
            net_flow_lamports: 0,
            protocol_fee_lamports: 0,
            creator_fee_lamports: 0,
            cashback_lamports: 0,
            virtual_sol_reserves: 0,
            virtual_token_reserves: 0,
            real_sol_reserves: 0,
            real_token_reserves: 0,
            token_total_supply: 0,
            last_trade_at: None,
            last_trade_slot: None,
            last_price_lamports_per_token: 0.0,
            unique_buyers: HashSet::new(),
            unique_sellers: HashSet::new(),
        }
    }

    pub fn age_secs_at(&self, now_ts: i64) -> Option<i64> {
        self.created_at.map(|created_at| now_ts - created_at)
    }

    pub fn unique_buyer_count(&self) -> usize {
        self.unique_buyers.len()
    }

    pub fn unique_seller_count(&self) -> usize {
        self.unique_sellers.len()
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{EventEnvelope, MintCreatedEvent, PumpEvent, TradeEvent};

    use super::MarketState;

    #[test]
    fn aggregates_buy_and_sell_flow() {
        let mint = "mint-1".to_string();
        let mut state = MarketState::default();

        state.apply(&EventEnvelope {
            seq: 1,
            slot: 1,
            block_time: Some(100),
            tx_signature: "sig-1".to_string(),
            tx_index: 0,
            event_index: 0,
            event: PumpEvent::MintCreated(MintCreatedEvent {
                mint: mint.clone(),
                bonding_curve: "curve".to_string(),
                user: "user-a".to_string(),
                creator: "user-a".to_string(),
                name: "A".to_string(),
                symbol: "A".to_string(),
                uri: "uri".to_string(),
                timestamp: 100,
                virtual_token_reserves: 1000,
                virtual_sol_reserves: 2000,
                real_token_reserves: 800,
                token_total_supply: 1000,
                token_program: "Tokenkeg".to_string(),
                is_mayhem_mode: false,
                is_cashback_enabled: false,
            }),
        });

        state.apply(&EventEnvelope {
            seq: 2,
            slot: 2,
            block_time: Some(101),
            tx_signature: "sig-2".to_string(),
            tx_index: 0,
            event_index: 0,
            event: PumpEvent::Trade(TradeEvent {
                mint: mint.clone(),
                sol_amount: 100,
                token_amount: 20,
                is_buy: true,
                user: "user-b".to_string(),
                timestamp: 101,
                virtual_sol_reserves: 2100,
                virtual_token_reserves: 980,
                real_sol_reserves: 100,
                real_token_reserves: 780,
                fee_recipient: "fee".to_string(),
                fee_basis_points: 100,
                fee: 1,
                creator: "user-a".to_string(),
                creator_fee_basis_points: 0,
                creator_fee: 0,
                track_volume: true,
                total_unclaimed_tokens: 0,
                total_claimed_tokens: 0,
                current_sol_volume: 100,
                last_update_timestamp: 101,
                ix_name: "buy".to_string(),
                mayhem_mode: false,
                cashback_fee_basis_points: 0,
                cashback: 0,
            }),
        });

        state.apply(&EventEnvelope {
            seq: 3,
            slot: 3,
            block_time: Some(102),
            tx_signature: "sig-3".to_string(),
            tx_index: 0,
            event_index: 0,
            event: PumpEvent::Trade(TradeEvent {
                mint: mint.clone(),
                sol_amount: 40,
                token_amount: 5,
                is_buy: false,
                user: "user-b".to_string(),
                timestamp: 102,
                virtual_sol_reserves: 2060,
                virtual_token_reserves: 985,
                real_sol_reserves: 60,
                real_token_reserves: 785,
                fee_recipient: "fee".to_string(),
                fee_basis_points: 100,
                fee: 1,
                creator: "user-a".to_string(),
                creator_fee_basis_points: 0,
                creator_fee: 0,
                track_volume: true,
                total_unclaimed_tokens: 0,
                total_claimed_tokens: 0,
                current_sol_volume: 140,
                last_update_timestamp: 102,
                ix_name: "sell".to_string(),
                mayhem_mode: false,
                cashback_fee_basis_points: 0,
                cashback: 0,
            }),
        });

        let mint_state = state.mint(&mint).expect("missing mint state");
        assert_eq!(mint_state.buy_count, 1);
        assert_eq!(mint_state.sell_count, 1);
        assert_eq!(mint_state.trade_count, 2);
        assert_eq!(mint_state.net_flow_lamports, 60);
        assert_eq!(mint_state.unique_buyer_count(), 1);
        assert_eq!(mint_state.unique_seller_count(), 1);
    }
}
