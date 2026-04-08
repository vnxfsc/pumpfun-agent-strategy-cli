use std::collections::{HashMap, HashSet};

use anyhow::Result;
use pump_agent_core::{AddressInspectReport, PgEventStore, PumpEvent};
use serde::Serialize;

use crate::clone::extract_wallet_behavior;

use super::inspect::DatabaseRequest;

const SCHEMA_SQL: &str = include_str!("../../../../schema/postgres.sql");

#[derive(Debug, Clone)]
pub struct AddressInspectRequest {
    pub database: DatabaseRequest,
    pub address: String,
    pub top_mints_limit: i64,
    pub roundtrip_limit: i64,
}

#[derive(Debug, Clone)]
pub struct MintShardSummaryRequest {
    pub database: DatabaseRequest,
    pub address: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MintShardSummaryResult {
    pub address: String,
    pub mint_count: i64,
    pub wallet_trade_count: i64,
    pub total_event_count: usize,
    pub shards: Vec<MintShardRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MintShardRow {
    pub mint: String,
    pub symbol: Option<String>,
    pub creator: Option<String>,
    pub event_count: usize,
    pub trade_count: usize,
    pub buy_count: usize,
    pub sell_count: usize,
    pub unique_trader_count: usize,
    pub wallet_trade_count: usize,
    pub wallet_buy_count: usize,
    pub wallet_sell_count: usize,
    pub wallet_entry_count: usize,
    pub wallet_roundtrip_count: usize,
    pub gross_buy_sol: f64,
    pub gross_sell_sol: f64,
    pub net_flow_sol: f64,
    pub wallet_gross_buy_sol: f64,
    pub wallet_gross_sell_sol: f64,
    pub wallet_net_flow_sol: f64,
    pub first_seen_ts: Option<i64>,
    pub last_seen_ts: Option<i64>,
    pub has_create: bool,
    pub is_complete: bool,
}

pub async fn address_inspect(request: AddressInspectRequest) -> Result<AddressInspectReport> {
    let store = PgEventStore::connect(
        &request.database.database_url,
        request.database.max_db_connections,
    )
    .await?;
    if request.database.apply_schema {
        store.apply_schema(SCHEMA_SQL).await?;
    }
    store
        .inspect_address(
            &request.address,
            request.top_mints_limit,
            request.roundtrip_limit,
        )
        .await
}

pub async fn summarize_mint_shards(
    request: MintShardSummaryRequest,
) -> Result<MintShardSummaryResult> {
    let store = PgEventStore::connect(
        &request.database.database_url,
        request.database.max_db_connections,
    )
    .await?;
    if request.database.apply_schema {
        store.apply_schema(SCHEMA_SQL).await?;
    }

    let export = store
        .load_events_for_address_mints(&request.address)
        .await?;
    let wallet = extract_wallet_behavior(&request.address, &export.events);
    let mut by_mint = HashMap::<String, MintShardAccumulator>::new();

    for entry in &wallet.entries {
        by_mint
            .entry(entry.mint.clone())
            .or_default()
            .wallet_entry_count += 1;
    }
    for roundtrip in &wallet.roundtrips {
        by_mint
            .entry(roundtrip.mint.clone())
            .or_default()
            .wallet_roundtrip_count += 1;
    }

    for event in &export.events {
        let Some(mint) = event.mint() else {
            continue;
        };
        let shard = by_mint.entry(mint.to_string()).or_default();
        shard.event_count += 1;
        shard.first_seen_ts = min_option_ts(shard.first_seen_ts, event.timestamp());
        shard.last_seen_ts = max_option_ts(shard.last_seen_ts, event.timestamp());

        match &event.event {
            PumpEvent::MintCreated(created) => {
                shard.has_create = true;
                shard.symbol = Some(created.symbol.clone());
                shard.creator = Some(created.creator.clone());
            }
            PumpEvent::CurveCompleted(_) => {
                shard.is_complete = true;
            }
            PumpEvent::Trade(trade) => {
                shard.trade_count += 1;
                if trade.is_buy {
                    shard.buy_count += 1;
                    shard.gross_buy_lamports += trade.sol_amount as i128;
                } else {
                    shard.sell_count += 1;
                    shard.gross_sell_lamports += trade.sol_amount as i128;
                }
                shard.unique_traders.insert(trade.user.clone());
                if trade.user == request.address {
                    shard.wallet_trade_count += 1;
                    if trade.is_buy {
                        shard.wallet_buy_count += 1;
                        shard.wallet_gross_buy_lamports += trade.sol_amount as i128;
                    } else {
                        shard.wallet_sell_count += 1;
                        shard.wallet_gross_sell_lamports += trade.sol_amount as i128;
                    }
                }
            }
        }
    }

    let mut shards = by_mint
        .into_iter()
        .map(|(mint, shard)| MintShardRow {
            mint,
            symbol: shard.symbol,
            creator: shard.creator,
            event_count: shard.event_count,
            trade_count: shard.trade_count,
            buy_count: shard.buy_count,
            sell_count: shard.sell_count,
            unique_trader_count: shard.unique_traders.len(),
            wallet_trade_count: shard.wallet_trade_count,
            wallet_buy_count: shard.wallet_buy_count,
            wallet_sell_count: shard.wallet_sell_count,
            wallet_entry_count: shard.wallet_entry_count,
            wallet_roundtrip_count: shard.wallet_roundtrip_count,
            gross_buy_sol: lamports_to_sol(shard.gross_buy_lamports),
            gross_sell_sol: lamports_to_sol(shard.gross_sell_lamports),
            net_flow_sol: lamports_to_sol(shard.gross_buy_lamports - shard.gross_sell_lamports),
            wallet_gross_buy_sol: lamports_to_sol(shard.wallet_gross_buy_lamports),
            wallet_gross_sell_sol: lamports_to_sol(shard.wallet_gross_sell_lamports),
            wallet_net_flow_sol: lamports_to_sol(
                shard.wallet_gross_buy_lamports - shard.wallet_gross_sell_lamports,
            ),
            first_seen_ts: shard.first_seen_ts,
            last_seen_ts: shard.last_seen_ts,
            has_create: shard.has_create,
            is_complete: shard.is_complete,
        })
        .collect::<Vec<_>>();

    shards.sort_by(|left, right| {
        right
            .wallet_trade_count
            .cmp(&left.wallet_trade_count)
            .then_with(|| right.wallet_entry_count.cmp(&left.wallet_entry_count))
            .then_with(|| right.event_count.cmp(&left.event_count))
            .then_with(|| left.mint.cmp(&right.mint))
    });
    shards.truncate(request.limit);

    Ok(MintShardSummaryResult {
        address: export.address,
        mint_count: export.mint_count,
        wallet_trade_count: export.wallet_trade_count,
        total_event_count: export.event_count,
        shards,
    })
}

#[derive(Debug, Clone, Default)]
struct MintShardAccumulator {
    symbol: Option<String>,
    creator: Option<String>,
    event_count: usize,
    trade_count: usize,
    buy_count: usize,
    sell_count: usize,
    unique_traders: HashSet<String>,
    wallet_trade_count: usize,
    wallet_buy_count: usize,
    wallet_sell_count: usize,
    wallet_entry_count: usize,
    wallet_roundtrip_count: usize,
    gross_buy_lamports: i128,
    gross_sell_lamports: i128,
    wallet_gross_buy_lamports: i128,
    wallet_gross_sell_lamports: i128,
    first_seen_ts: Option<i64>,
    last_seen_ts: Option<i64>,
    has_create: bool,
    is_complete: bool,
}

fn lamports_to_sol(lamports: i128) -> f64 {
    lamports as f64 / 1_000_000_000.0
}

fn min_option_ts(current: Option<i64>, candidate: Option<i64>) -> Option<i64> {
    match (current, candidate) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn max_option_ts(current: Option<i64>, candidate: Option<i64>) -> Option<i64> {
    match (current, candidate) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}
