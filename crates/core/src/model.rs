use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub seq: u64,
    pub slot: u64,
    pub block_time: Option<i64>,
    pub tx_signature: String,
    pub tx_index: u32,
    pub event_index: u32,
    pub event: PumpEvent,
}

impl EventEnvelope {
    pub fn timestamp(&self) -> Option<i64> {
        self.event.timestamp().or(self.block_time)
    }

    pub fn mint(&self) -> Option<&str> {
        self.event.mint()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PumpEvent {
    MintCreated(MintCreatedEvent),
    Trade(TradeEvent),
    CurveCompleted(CurveCompletedEvent),
}

impl PumpEvent {
    pub fn timestamp(&self) -> Option<i64> {
        match self {
            Self::MintCreated(event) => Some(event.timestamp),
            Self::Trade(event) => Some(event.timestamp),
            Self::CurveCompleted(event) => Some(event.timestamp),
        }
    }

    pub fn mint(&self) -> Option<&str> {
        match self {
            Self::MintCreated(event) => Some(&event.mint),
            Self::Trade(event) => Some(&event.mint),
            Self::CurveCompleted(event) => Some(&event.mint),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintCreatedEvent {
    pub mint: String,
    pub bonding_curve: String,
    pub user: String,
    pub creator: String,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub timestamp: i64,
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub token_total_supply: u64,
    pub token_program: String,
    pub is_mayhem_mode: bool,
    pub is_cashback_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeEvent {
    pub mint: String,
    pub sol_amount: u64,
    pub token_amount: u64,
    pub is_buy: bool,
    pub user: String,
    pub timestamp: i64,
    pub virtual_sol_reserves: u64,
    pub virtual_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub fee_recipient: String,
    pub fee_basis_points: u64,
    pub fee: u64,
    pub creator: String,
    pub creator_fee_basis_points: u64,
    pub creator_fee: u64,
    pub track_volume: bool,
    pub total_unclaimed_tokens: u64,
    pub total_claimed_tokens: u64,
    pub current_sol_volume: u64,
    pub last_update_timestamp: i64,
    pub ix_name: String,
    pub mayhem_mode: bool,
    pub cashback_fee_basis_points: u64,
    pub cashback: u64,
}

impl TradeEvent {
    pub fn side(&self) -> OrderSide {
        if self.is_buy {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        }
    }

    pub fn price_lamports_per_token(&self) -> f64 {
        if self.token_amount == 0 {
            return 0.0;
        }

        self.sol_amount as f64 / self.token_amount as f64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurveCompletedEvent {
    pub mint: String,
    pub bonding_curve: String,
    pub user: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub enum OrderRequest {
    BuyForLamports {
        mint: String,
        lamports: u64,
        reason: String,
    },
    SellAll {
        mint: String,
        reason: String,
    },
}

impl OrderRequest {
    pub fn mint(&self) -> &str {
        match self {
            Self::BuyForLamports { mint, .. } => mint,
            Self::SellAll { mint, .. } => mint,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendingOrder {
    pub id: u64,
    pub submitted_at_seq: u64,
    pub submitted_at_ts: Option<i64>,
    pub request: OrderRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionReport {
    Filled(FillReport),
    Rejected(RejectedOrder),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillReport {
    pub order_id: u64,
    pub mint: String,
    pub side: OrderSide,
    pub lamports: u64,
    pub token_amount: u64,
    pub fee_lamports: u64,
    pub execution_price_lamports_per_token: f64,
    pub timestamp: Option<i64>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectedOrder {
    pub order_id: u64,
    pub mint: String,
    pub reason: String,
    pub rejection: RejectionReason,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectionReason {
    UnknownMint,
    DuplicatePosition,
    InsufficientCash,
    EmptyPosition,
    ZeroPrice,
}
