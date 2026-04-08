use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, postgres::PgPoolOptions, types::Json};
use std::collections::HashMap;

use crate::{
    engine::BacktestRunResult,
    grpc::DecodedPumpTransaction,
    model::{EventEnvelope, FillReport, PumpEvent},
};

#[derive(Debug, Clone)]
pub struct PgEventStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStats {
    pub total_events: i64,
    pub total_trades: i64,
    pub total_mint_events: i64,
    pub total_completions: i64,
    pub distinct_mints_seen: i64,
    pub stored_mints: i64,
    pub real_created_mints: i64,
    pub inferred_trade_only_mints: i64,
    pub latest_slot: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintInspectReport {
    pub overview: Option<MintOverview>,
    pub recent_trades: Vec<MintTradeRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintOverview {
    pub mint: String,
    pub creator: String,
    pub bonding_curve: String,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub token_program: String,
    pub is_inferred: bool,
    pub created_slot: i64,
    pub created_at: Option<String>,
    pub trade_count: i64,
    pub buy_count: i64,
    pub sell_count: i64,
    pub gross_buy_lamports: String,
    pub gross_sell_lamports: String,
    pub net_flow_lamports: String,
    pub last_trade_slot: Option<i64>,
    pub last_trade_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintTradeRow {
    pub seq: i64,
    pub slot: i64,
    pub side: String,
    pub sol_amount: String,
    pub token_amount: String,
    pub user_address: String,
    pub tx_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRunRow {
    pub id: i64,
    pub strategy_name: String,
    pub run_mode: String,
    pub sweep_batch_id: Option<String>,
    pub live_run_id: Option<String>,
    pub source_type: String,
    pub source_ref: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub processed_events: i64,
    pub fills: i64,
    pub rejections: i64,
    pub ending_cash_lamports: String,
    pub ending_equity_lamports: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunRow {
    pub task_id: String,
    pub task_kind: String,
    pub status: String,
    pub idempotency_key: Option<String>,
    pub cancellation_requested: bool,
    pub request_payload: serde_json::Value,
    pub result_payload: Option<serde_json::Value>,
    pub error_payload: Option<serde_json::Value>,
    pub submitted_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRow {
    pub experiment_id: String,
    pub title: String,
    pub target_wallet: String,
    pub status: String,
    pub thesis: Option<String>,
    pub notes: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypothesisRow {
    pub hypothesis_id: String,
    pub experiment_id: String,
    pub family: String,
    pub description: String,
    pub status: String,
    pub strategy_config: serde_json::Value,
    pub sample_window: serde_json::Value,
    pub notes: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationRow {
    pub evaluation_id: String,
    pub experiment_id: String,
    pub hypothesis_id: Option<String>,
    pub strategy_run_id: Option<i64>,
    pub task_id: Option<String>,
    pub target_wallet: String,
    pub family: Option<String>,
    pub strategy_name: Option<String>,
    pub source_type: String,
    pub source_ref: String,
    pub score_overall: Option<f64>,
    pub score_breakdown: serde_json::Value,
    pub metrics: serde_json::Value,
    pub failure_tags: Vec<String>,
    pub artifact_paths: serde_json::Value,
    pub notes: serde_json::Value,
    pub conclusion: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentDetail {
    pub experiment: ExperimentRow,
    pub hypotheses: Vec<HypothesisRow>,
    pub evaluations: Vec<EvaluationRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInspectReport {
    pub run: Option<StrategyRunDetail>,
    pub fills: Vec<RunFillRow>,
    pub position_snapshots: Vec<PositionSnapshotRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRunDetail {
    pub id: i64,
    pub strategy_name: String,
    pub run_mode: String,
    pub sweep_batch_id: Option<String>,
    pub live_run_id: Option<String>,
    pub config: serde_json::Value,
    pub source_type: String,
    pub source_ref: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub processed_events: i64,
    pub fills: i64,
    pub rejections: i64,
    pub ending_cash_lamports: String,
    pub ending_equity_lamports: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFillRow {
    pub order_id: i64,
    pub mint: String,
    pub side: String,
    pub lamports: String,
    pub token_amount: String,
    pub fee_lamports: String,
    pub execution_price_lamports_per_token: f64,
    pub reason: String,
    pub executed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSnapshotInput {
    pub snapshot_kind: String,
    pub event_seq: Option<u64>,
    pub event_slot: Option<u64>,
    pub snapshot_at: Option<i64>,
    pub cash_lamports: u64,
    pub equity_lamports: u64,
    pub pending_orders: usize,
    pub open_positions: usize,
    pub positions: serde_json::Value,
}

#[derive(Debug, Clone, Default)]
pub struct StrategyRunPersistOptions {
    pub run_mode: Option<String>,
    pub sweep_batch_id: Option<String>,
    pub live_run_id: Option<String>,
    pub position_snapshots: Vec<PositionSnapshotInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSnapshotRow {
    pub snapshot_kind: String,
    pub event_seq: Option<i64>,
    pub event_slot: Option<i64>,
    pub snapshot_at: Option<String>,
    pub cash_lamports: String,
    pub equity_lamports: String,
    pub pending_orders: i32,
    pub open_positions: i32,
    pub positions: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepBatchRunRow {
    pub id: i64,
    pub strategy_name: String,
    pub run_mode: String,
    pub sweep_batch_id: String,
    pub started_at: String,
    pub processed_events: i64,
    pub fills: i64,
    pub rejections: i64,
    pub ending_cash_lamports: String,
    pub ending_equity_lamports: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepBatchInspectReport {
    pub sweep_batch_id: String,
    pub runs: Vec<SweepBatchRunRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressExportData {
    pub address: String,
    pub mint_count: i64,
    pub wallet_trade_count: i64,
    pub event_count: usize,
    pub events: Vec<EventEnvelope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressInspectReport {
    pub overview: AddressOverview,
    pub top_mints: Vec<AddressMintSummary>,
    pub recent_roundtrips: Vec<AddressRoundtrip>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressOverview {
    pub address: String,
    pub total_trades: i64,
    pub buy_count: i64,
    pub sell_count: i64,
    pub distinct_mints: i64,
    pub first_trade_seq: Option<i64>,
    pub first_trade_at: Option<String>,
    pub last_trade_seq: Option<i64>,
    pub last_trade_at: Option<String>,
    pub gross_buy_lamports: String,
    pub gross_sell_lamports: String,
    pub net_cash_flow_lamports: String,
    pub roundtrip_count: i64,
    pub closed_roundtrip_count: i64,
    pub open_roundtrip_count: i64,
    pub orphan_sell_count: i64,
    pub realized_pnl_lamports: String,
    pub win_rate_closed: Option<f64>,
    pub avg_hold_secs_closed: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressMintSummary {
    pub mint: String,
    pub trade_count: i64,
    pub buy_count: i64,
    pub sell_count: i64,
    pub gross_buy_lamports: String,
    pub gross_sell_lamports: String,
    pub net_cash_flow_lamports: String,
    pub first_seq: i64,
    pub last_seq: i64,
    pub last_trade_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressTimelineRow {
    pub seq: i64,
    pub slot: i64,
    pub timestamp: Option<String>,
    pub mint: String,
    pub side: String,
    pub sol_amount: String,
    pub token_amount: String,
    pub fee_lamports: String,
    pub creator_fee_lamports: String,
    pub cashback_lamports: String,
    pub ix_name: String,
    pub tx_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressRoundtripReport {
    pub address: String,
    pub total_roundtrips: i64,
    pub closed_roundtrips: i64,
    pub open_roundtrips: i64,
    pub orphan_sell_count: i64,
    pub roundtrips: Vec<AddressRoundtrip>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressRoundtrip {
    pub mint: String,
    pub status: String,
    pub opened_seq: i64,
    pub opened_slot: i64,
    pub opened_at: Option<String>,
    pub closed_seq: Option<i64>,
    pub closed_slot: Option<i64>,
    pub closed_at: Option<String>,
    pub hold_secs: Option<i64>,
    pub entry_count: i64,
    pub exit_count: i64,
    pub bought_token_amount: String,
    pub sold_token_amount: String,
    pub gross_buy_lamports: String,
    pub gross_sell_lamports: String,
    pub total_fees_lamports: String,
    pub total_cashback_lamports: String,
    pub net_entry_lamports: String,
    pub net_exit_lamports: String,
    pub realized_pnl_lamports: Option<String>,
    pub roi_bps: Option<i64>,
}

#[derive(Debug, Clone)]
struct AddressTradeFact {
    seq: i64,
    slot: i64,
    timestamp_text: Option<String>,
    timestamp_epoch: Option<i64>,
    mint: String,
    is_buy: bool,
    sol_amount: i128,
    token_amount: i128,
    fee_lamports: i128,
    creator_fee_lamports: i128,
    cashback_lamports: i128,
}

#[derive(Debug, Clone)]
struct AddressRoundtripBuild {
    roundtrips: Vec<AddressRoundtrip>,
    orphan_sell_count: i64,
}

#[derive(Debug, Clone)]
struct ActiveAddressRoundtrip {
    mint: String,
    opened_seq: i64,
    opened_slot: i64,
    opened_at: Option<String>,
    opened_at_epoch: Option<i64>,
    closed_seq: Option<i64>,
    closed_slot: Option<i64>,
    closed_at: Option<String>,
    closed_at_epoch: Option<i64>,
    entry_count: i64,
    exit_count: i64,
    bought_token_amount: i128,
    sold_token_amount: i128,
    gross_buy_lamports: i128,
    gross_sell_lamports: i128,
    total_fees_lamports: i128,
    total_cashback_lamports: i128,
    net_entry_lamports: i128,
    net_exit_lamports: i128,
    remaining_cost_basis_lamports: i128,
    token_balance: i128,
    realized_pnl_lamports: i128,
}

impl PgEventStore {
    pub async fn connect(database_url: &str, max_connections: u32) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await
            .with_context(|| format!("failed to connect to postgres at {database_url}"))?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn apply_schema(&self, schema_sql: &str) -> Result<()> {
        sqlx::raw_sql(schema_sql)
            .execute(&self.pool)
            .await
            .context("failed to apply postgres schema")?;
        Ok(())
    }

    pub async fn next_sequence(&self) -> Result<u64> {
        let seq: i64 =
            sqlx::query_scalar("select coalesce(max(seq), 0) + 1 from pump_event_envelopes")
                .fetch_one(&self.pool)
                .await
                .context("failed to query next event sequence")?;

        u64::try_from(seq).context("next sequence should not be negative")
    }

    pub async fn latest_slot(&self) -> Result<Option<u64>> {
        let slot: Option<i64> = sqlx::query_scalar("select max(slot) from raw_transactions")
            .fetch_one(&self.pool)
            .await
            .context("failed to query latest ingested slot")?;

        slot.map(|value| u64::try_from(value).context("latest slot should not be negative"))
            .transpose()
    }

    pub async fn append_decoded_transaction(&self, decoded: &DecodedPumpTransaction) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin transaction")?;

        let raw_block_time = decoded.raw.block_time.map(|value| value as f64);
        sqlx::query(
            r#"
            insert into raw_transactions (
                slot, signature, tx_index, program_id, block_time, logs, raw_base64
            )
            values ($1, $2, $3, $4, to_timestamp($5), $6, $7)
            on conflict (slot, signature, tx_index) do update set
                logs = excluded.logs,
                raw_base64 = excluded.raw_base64,
                block_time = excluded.block_time
            "#,
        )
        .bind(i64_from_u64(decoded.raw.slot)?)
        .bind(&decoded.raw.signature)
        .bind(i32_from_u32(decoded.raw.tx_index))
        .bind(&decoded.raw.program_id)
        .bind(raw_block_time)
        .bind(Json(decoded.raw.logs.clone()))
        .bind(&decoded.raw.raw_base64)
        .execute(&mut *tx)
        .await
        .context("failed to insert raw transaction")?;

        for event in &decoded.events {
            self.insert_event_envelope(&mut tx, event).await?;

            match &event.event {
                PumpEvent::MintCreated(mint) => {
                    sqlx::query(
                        r#"
                        insert into pump_mints (
                            mint, seq, slot, tx_signature, tx_index, event_index,
                            bonding_curve, creator, name, symbol, uri, created_slot, created_at,
                            is_mayhem_mode, is_cashback_enabled, virtual_token_reserves,
                            virtual_sol_reserves, real_token_reserves, token_total_supply,
                            token_program, raw_event
                        )
                        values (
                            $1, $2, $3, $4, $5, $6,
                            $7, $8, $9, $10, $11, $12, to_timestamp($13),
                            $14, $15, $16, $17, $18, $19, $20, $21
                        )
                        on conflict (mint) do update set
                            seq = excluded.seq,
                            slot = excluded.slot,
                            tx_signature = excluded.tx_signature,
                            tx_index = excluded.tx_index,
                            event_index = excluded.event_index,
                            bonding_curve = excluded.bonding_curve,
                            creator = excluded.creator,
                            name = excluded.name,
                            symbol = excluded.symbol,
                            uri = excluded.uri,
                            created_slot = excluded.created_slot,
                            created_at = excluded.created_at,
                            is_mayhem_mode = excluded.is_mayhem_mode,
                            is_cashback_enabled = excluded.is_cashback_enabled,
                            virtual_token_reserves = excluded.virtual_token_reserves,
                            virtual_sol_reserves = excluded.virtual_sol_reserves,
                            real_token_reserves = excluded.real_token_reserves,
                            token_total_supply = excluded.token_total_supply,
                            token_program = excluded.token_program,
                            raw_event = excluded.raw_event
                        "#,
                    )
                    .bind(&mint.mint)
                    .bind(i64_from_u64(event.seq)?)
                    .bind(i64_from_u64(event.slot)?)
                    .bind(&event.tx_signature)
                    .bind(i32_from_u32(event.tx_index))
                    .bind(i32_from_u32(event.event_index))
                    .bind(&mint.bonding_curve)
                    .bind(&mint.creator)
                    .bind(&mint.name)
                    .bind(&mint.symbol)
                    .bind(&mint.uri)
                    .bind(i64_from_u64(event.slot)?)
                    .bind(mint.timestamp as f64)
                    .bind(mint.is_mayhem_mode)
                    .bind(mint.is_cashback_enabled)
                    .bind(i64_from_u64(mint.virtual_token_reserves)?)
                    .bind(i64_from_u64(mint.virtual_sol_reserves)?)
                    .bind(i64_from_u64(mint.real_token_reserves)?)
                    .bind(i64_from_u64(mint.token_total_supply)?)
                    .bind(&mint.token_program)
                    .bind(Json(event.clone()))
                    .execute(&mut *tx)
                    .await
                    .context("failed to insert pump mint")?;
                }
                PumpEvent::Trade(trade) => {
                    self.ensure_mint_row_for_trade(&mut tx, event, trade)
                        .await?;
                    sqlx::query(
                        r#"
                        insert into pump_trades (
                            slot, tx_signature, tx_index, event_index, seq, mint, user_address,
                            side, ix_name, timestamp, sol_amount, token_amount, fee,
                            fee_basis_points, creator, creator_fee, creator_fee_basis_points,
                            cashback, cashback_fee_basis_points, virtual_sol_reserves,
                            virtual_token_reserves, real_sol_reserves, real_token_reserves,
                            track_volume, raw_event
                        )
                        values (
                            $1, $2, $3, $4, $5, $6, $7,
                            $8, $9, to_timestamp($10), $11, $12, $13, $14, $15, $16, $17,
                            $18, $19, $20, $21, $22, $23, $24, $25
                        )
                        on conflict (slot, tx_signature, tx_index, event_index) do update set
                            seq = excluded.seq,
                            raw_event = excluded.raw_event
                        "#,
                    )
                    .bind(i64_from_u64(event.slot)?)
                    .bind(&event.tx_signature)
                    .bind(i32_from_u32(event.tx_index))
                    .bind(i32_from_u32(event.event_index))
                    .bind(i64_from_u64(event.seq)?)
                    .bind(&trade.mint)
                    .bind(&trade.user)
                    .bind(if trade.is_buy { "buy" } else { "sell" })
                    .bind(&trade.ix_name)
                    .bind(trade.timestamp as f64)
                    .bind(i64_from_u64(trade.sol_amount)?)
                    .bind(i64_from_u64(trade.token_amount)?)
                    .bind(i64_from_u64(trade.fee)?)
                    .bind(i64_from_u64(trade.fee_basis_points)?)
                    .bind(&trade.creator)
                    .bind(i64_from_u64(trade.creator_fee)?)
                    .bind(i64_from_u64(trade.creator_fee_basis_points)?)
                    .bind(i64_from_u64(trade.cashback)?)
                    .bind(i64_from_u64(trade.cashback_fee_basis_points)?)
                    .bind(i64_from_u64(trade.virtual_sol_reserves)?)
                    .bind(i64_from_u64(trade.virtual_token_reserves)?)
                    .bind(i64_from_u64(trade.real_sol_reserves)?)
                    .bind(i64_from_u64(trade.real_token_reserves)?)
                    .bind(trade.track_volume)
                    .bind(Json(event.clone()))
                    .execute(&mut *tx)
                    .await
                    .context("failed to insert pump trade")?;
                }
                PumpEvent::CurveCompleted(complete) => {
                    sqlx::query(
                        r#"
                        insert into pump_curve_completions (
                            slot, tx_signature, tx_index, event_index, seq, mint, bonding_curve,
                            user_address, timestamp, raw_event
                        )
                        values ($1, $2, $3, $4, $5, $6, $7, $8, to_timestamp($9), $10)
                        on conflict (slot, tx_signature, tx_index, event_index) do update set
                            seq = excluded.seq,
                            raw_event = excluded.raw_event
                        "#,
                    )
                    .bind(i64_from_u64(event.slot)?)
                    .bind(&event.tx_signature)
                    .bind(i32_from_u32(event.tx_index))
                    .bind(i32_from_u32(event.event_index))
                    .bind(i64_from_u64(event.seq)?)
                    .bind(&complete.mint)
                    .bind(&complete.bonding_curve)
                    .bind(&complete.user)
                    .bind(complete.timestamp as f64)
                    .bind(Json(event.clone()))
                    .execute(&mut *tx)
                    .await
                    .context("failed to insert curve completion event")?;
                }
            }
        }

        tx.commit()
            .await
            .context("failed to commit postgres transaction")?;
        Ok(())
    }

    pub async fn load_replay_events(&self) -> Result<Vec<EventEnvelope>> {
        let rows = sqlx::query(
            r#"
            select envelope
            from pump_event_envelopes
            order by seq asc
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to load replay events from postgres")?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let value: serde_json::Value = row
                .try_get("envelope")
                .context("missing envelope json column")?;
            let event = serde_json::from_value::<EventEnvelope>(value)
                .context("failed to deserialize EventEnvelope from postgres")?;
            events.push(event);
        }

        Ok(events)
    }

    pub async fn load_events_for_address_mints(&self, address: &str) -> Result<AddressExportData> {
        let summary = sqlx::query(
            r#"
            select
                count(distinct mint) as mint_count,
                count(*) as wallet_trade_count
            from pump_trades
            where user_address = $1
            "#,
        )
        .bind(address)
        .fetch_one(&self.pool)
        .await
        .context("failed to fetch address export summary")?;

        let rows = sqlx::query(
            r#"
            with wallet_mints as (
                select distinct mint
                from pump_trades
                where user_address = $1
            )
            select envelope
            from pump_event_envelopes
            where envelope #>> '{event,mint}' in (select mint from wallet_mints)
            order by seq asc
            "#,
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await
        .context("failed to load events for address mints")?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let value: serde_json::Value = row
                .try_get("envelope")
                .context("missing envelope json column")?;
            let event = serde_json::from_value::<EventEnvelope>(value)
                .context("failed to deserialize EventEnvelope from postgres")?;
            events.push(event);
        }

        Ok(AddressExportData {
            address: address.to_string(),
            mint_count: summary.try_get("mint_count")?,
            wallet_trade_count: summary.try_get("wallet_trade_count")?,
            event_count: events.len(),
            events,
        })
    }

    pub async fn fetch_event_stats(&self) -> Result<EventStats> {
        let row = sqlx::query(
            r#"
            select
                count(*) as total_events,
                count(*) filter (where event_kind = 'trade') as total_trades,
                count(*) filter (where event_kind = 'mint_created') as total_mint_events,
                count(*) filter (where event_kind = 'curve_completed') as total_completions,
                count(distinct envelope #>> '{event,mint}') as distinct_mints_seen,
                (select count(*) from pump_mints) as stored_mints,
                (
                    select count(*)
                    from pump_mints
                    where not (
                        coalesce(name, '') = ''
                        and coalesce(symbol, '') = ''
                        and coalesce(uri, '') = ''
                        and coalesce(token_program, '') = ''
                    )
                ) as real_created_mints,
                (
                    select count(*)
                    from pump_mints
                    where (
                        coalesce(name, '') = ''
                        and coalesce(symbol, '') = ''
                        and coalesce(uri, '') = ''
                        and coalesce(token_program, '') = ''
                    )
                ) as inferred_trade_only_mints,
                max(slot) as latest_slot
            from pump_event_envelopes
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to fetch event stats")?;

        Ok(EventStats {
            total_events: row.try_get("total_events")?,
            total_trades: row.try_get("total_trades")?,
            total_mint_events: row.try_get("total_mint_events")?,
            total_completions: row.try_get("total_completions")?,
            distinct_mints_seen: row.try_get("distinct_mints_seen")?,
            stored_mints: row.try_get("stored_mints")?,
            real_created_mints: row.try_get("real_created_mints")?,
            inferred_trade_only_mints: row.try_get("inferred_trade_only_mints")?,
            latest_slot: row.try_get("latest_slot")?,
        })
    }

    pub async fn tail_events(&self, limit: i64) -> Result<Vec<EventEnvelope>> {
        let rows = sqlx::query(
            r#"
            select envelope
            from pump_event_envelopes
            order by seq desc
            limit $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to tail events")?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let value: serde_json::Value = row
                .try_get("envelope")
                .context("missing envelope json column")?;
            let event = serde_json::from_value::<EventEnvelope>(value)
                .context("failed to deserialize EventEnvelope from postgres")?;
            events.push(event);
        }
        events.sort_by_key(|event| event.seq);
        Ok(events)
    }

    pub async fn list_strategy_runs(&self, limit: i64) -> Result<Vec<StrategyRunRow>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                strategy_name,
                run_mode,
                sweep_batch_id,
                live_run_id,
                source_type,
                source_ref,
                started_at::text as started_at,
                finished_at::text as finished_at,
                processed_events,
                fills,
                rejections,
                ending_cash_lamports::text as ending_cash_lamports,
                ending_equity_lamports::text as ending_equity_lamports
            from strategy_runs
            order by id desc
            limit $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to list strategy runs")?;

        let mut runs = Vec::with_capacity(rows.len());
        for row in rows {
            runs.push(StrategyRunRow {
                id: row.try_get("id")?,
                strategy_name: row.try_get("strategy_name")?,
                run_mode: row.try_get("run_mode")?,
                sweep_batch_id: row.try_get("sweep_batch_id")?,
                live_run_id: row.try_get("live_run_id")?,
                source_type: row.try_get("source_type")?,
                source_ref: row.try_get("source_ref")?,
                started_at: row.try_get("started_at")?,
                finished_at: row.try_get("finished_at")?,
                processed_events: row.try_get("processed_events")?,
                fills: row.try_get("fills")?,
                rejections: row.try_get("rejections")?,
                ending_cash_lamports: row.try_get("ending_cash_lamports")?,
                ending_equity_lamports: row.try_get("ending_equity_lamports")?,
            });
        }

        Ok(runs)
    }

    pub async fn insert_task_run(
        &self,
        task_id: &str,
        task_kind: &str,
        idempotency_key: Option<&str>,
        request_payload: serde_json::Value,
    ) -> Result<TaskRunRow> {
        let row = sqlx::query(
            r#"
            insert into task_runs (
                task_id,
                task_kind,
                status,
                idempotency_key,
                request_payload
            )
            values ($1, $2, 'queued', $3, $4)
            returning
                task_id,
                task_kind,
                status,
                idempotency_key,
                cancellation_requested,
                request_payload,
                result_payload,
                error_payload,
                submitted_at::text as submitted_at,
                started_at::text as started_at,
                finished_at::text as finished_at
            "#,
        )
        .bind(task_id)
        .bind(task_kind)
        .bind(idempotency_key)
        .bind(Json(request_payload))
        .fetch_one(&self.pool)
        .await
        .context("failed to insert task run")?;

        task_run_from_row(&row)
    }

    pub async fn find_task_run_by_idempotency_key(
        &self,
        task_kind: &str,
        idempotency_key: &str,
    ) -> Result<Option<TaskRunRow>> {
        let row = sqlx::query(
            r#"
            select
                task_id,
                task_kind,
                status,
                idempotency_key,
                cancellation_requested,
                request_payload,
                result_payload,
                error_payload,
                submitted_at::text as submitted_at,
                started_at::text as started_at,
                finished_at::text as finished_at
            from task_runs
            where task_kind = $1 and idempotency_key = $2
            "#,
        )
        .bind(task_kind)
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch task run by idempotency key")?;

        row.as_ref().map(task_run_from_row).transpose()
    }

    pub async fn get_task_run(&self, task_id: &str) -> Result<Option<TaskRunRow>> {
        let row = sqlx::query(
            r#"
            select
                task_id,
                task_kind,
                status,
                idempotency_key,
                cancellation_requested,
                request_payload,
                result_payload,
                error_payload,
                submitted_at::text as submitted_at,
                started_at::text as started_at,
                finished_at::text as finished_at
            from task_runs
            where task_id = $1
            "#,
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch task run")?;

        row.as_ref().map(task_run_from_row).transpose()
    }

    pub async fn mark_task_running(&self, task_id: &str) -> Result<Option<TaskRunRow>> {
        let row = sqlx::query(
            r#"
            update task_runs
            set
                status = case
                    when cancellation_requested then 'cancelling'
                    else 'running'
                end,
                started_at = coalesce(started_at, now())
            where task_id = $1
            returning
                task_id,
                task_kind,
                status,
                idempotency_key,
                cancellation_requested,
                request_payload,
                result_payload,
                error_payload,
                submitted_at::text as submitted_at,
                started_at::text as started_at,
                finished_at::text as finished_at
            "#,
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to mark task running")?;

        row.as_ref().map(task_run_from_row).transpose()
    }

    pub async fn request_task_cancel(&self, task_id: &str) -> Result<Option<TaskRunRow>> {
        let row = sqlx::query(
            r#"
            update task_runs
            set
                cancellation_requested = true,
                status = case
                    when status in ('queued', 'running', 'cancelling') then 'cancelling'
                    else status
                end
            where task_id = $1
            returning
                task_id,
                task_kind,
                status,
                idempotency_key,
                cancellation_requested,
                request_payload,
                result_payload,
                error_payload,
                submitted_at::text as submitted_at,
                started_at::text as started_at,
                finished_at::text as finished_at
            "#,
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to request task cancellation")?;

        row.as_ref().map(task_run_from_row).transpose()
    }

    pub async fn complete_task_run(
        &self,
        task_id: &str,
        status: &str,
        result_payload: serde_json::Value,
    ) -> Result<Option<TaskRunRow>> {
        let row = sqlx::query(
            r#"
            update task_runs
            set
                status = $2,
                result_payload = $3,
                error_payload = null,
                finished_at = now()
            where task_id = $1
            returning
                task_id,
                task_kind,
                status,
                idempotency_key,
                cancellation_requested,
                request_payload,
                result_payload,
                error_payload,
                submitted_at::text as submitted_at,
                started_at::text as started_at,
                finished_at::text as finished_at
            "#,
        )
        .bind(task_id)
        .bind(status)
        .bind(Json(result_payload))
        .fetch_optional(&self.pool)
        .await
        .context("failed to complete task run")?;

        row.as_ref().map(task_run_from_row).transpose()
    }

    pub async fn fail_task_run(
        &self,
        task_id: &str,
        error_payload: serde_json::Value,
    ) -> Result<Option<TaskRunRow>> {
        let row = sqlx::query(
            r#"
            update task_runs
            set
                status = 'failed',
                error_payload = $2,
                finished_at = now()
            where task_id = $1
            returning
                task_id,
                task_kind,
                status,
                idempotency_key,
                cancellation_requested,
                request_payload,
                result_payload,
                error_payload,
                submitted_at::text as submitted_at,
                started_at::text as started_at,
                finished_at::text as finished_at
            "#,
        )
        .bind(task_id)
        .bind(Json(error_payload))
        .fetch_optional(&self.pool)
        .await
        .context("failed to mark task run failed")?;

        row.as_ref().map(task_run_from_row).transpose()
    }

    pub async fn create_experiment(
        &self,
        experiment_id: &str,
        title: &str,
        target_wallet: &str,
        thesis: Option<&str>,
        notes: serde_json::Value,
    ) -> Result<ExperimentRow> {
        let row = sqlx::query(
            r#"
            insert into experiments (
                experiment_id,
                title,
                target_wallet,
                thesis,
                notes
            )
            values ($1, $2, $3, $4, $5)
            returning
                experiment_id,
                title,
                target_wallet,
                status,
                thesis,
                notes,
                created_at::text as created_at,
                updated_at::text as updated_at
            "#,
        )
        .bind(experiment_id)
        .bind(title)
        .bind(target_wallet)
        .bind(thesis)
        .bind(Json(notes))
        .fetch_one(&self.pool)
        .await
        .context("failed to create experiment")?;

        experiment_from_row(&row)
    }

    pub async fn get_experiment(&self, experiment_id: &str) -> Result<Option<ExperimentRow>> {
        let row = sqlx::query(
            r#"
            select
                experiment_id,
                title,
                target_wallet,
                status,
                thesis,
                notes,
                created_at::text as created_at,
                updated_at::text as updated_at
            from experiments
            where experiment_id = $1
            "#,
        )
        .bind(experiment_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch experiment row")?;

        row.as_ref().map(experiment_from_row).transpose()
    }

    pub async fn list_experiments(&self, limit: i64) -> Result<Vec<ExperimentRow>> {
        let rows = sqlx::query(
            r#"
            select
                experiment_id,
                title,
                target_wallet,
                status,
                thesis,
                notes,
                created_at::text as created_at,
                updated_at::text as updated_at
            from experiments
            order by created_at desc
            limit $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to list experiments")?;

        rows.iter().map(experiment_from_row).collect()
    }

    pub async fn create_hypothesis(
        &self,
        hypothesis_id: &str,
        experiment_id: &str,
        family: &str,
        description: &str,
        strategy_config: serde_json::Value,
        sample_window: serde_json::Value,
        notes: serde_json::Value,
    ) -> Result<HypothesisRow> {
        let row = sqlx::query(
            r#"
            insert into hypotheses (
                hypothesis_id,
                experiment_id,
                family,
                description,
                strategy_config,
                sample_window,
                notes
            )
            values ($1, $2, $3, $4, $5, $6, $7)
            returning
                hypothesis_id,
                experiment_id,
                family,
                description,
                status,
                strategy_config,
                sample_window,
                notes,
                created_at::text as created_at,
                updated_at::text as updated_at
            "#,
        )
        .bind(hypothesis_id)
        .bind(experiment_id)
        .bind(family)
        .bind(description)
        .bind(Json(strategy_config))
        .bind(Json(sample_window))
        .bind(Json(notes))
        .fetch_one(&self.pool)
        .await
        .context("failed to create hypothesis")?;

        hypothesis_from_row(&row)
    }

    pub async fn create_evaluation(
        &self,
        evaluation_id: &str,
        experiment_id: &str,
        hypothesis_id: Option<&str>,
        strategy_run_id: Option<i64>,
        task_id: Option<&str>,
        target_wallet: &str,
        family: Option<&str>,
        strategy_name: Option<&str>,
        source_type: &str,
        source_ref: &str,
        score_overall: Option<f64>,
        score_breakdown: serde_json::Value,
        metrics: serde_json::Value,
        failure_tags: &[String],
        artifact_paths: serde_json::Value,
        notes: serde_json::Value,
        conclusion: Option<&str>,
    ) -> Result<EvaluationRow> {
        let row = sqlx::query(
            r#"
            insert into evaluations (
                evaluation_id,
                experiment_id,
                hypothesis_id,
                strategy_run_id,
                task_id,
                target_wallet,
                family,
                strategy_name,
                source_type,
                source_ref,
                score_overall,
                score_breakdown,
                metrics,
                failure_tags,
                artifact_paths,
                notes,
                conclusion
            )
            values (
                $1, $2, $3, $4, $5, $6, $7, $8,
                $9, $10, $11, $12, $13, $14, $15, $16, $17
            )
            returning
                evaluation_id,
                experiment_id,
                hypothesis_id,
                strategy_run_id,
                task_id,
                target_wallet,
                family,
                strategy_name,
                source_type,
                source_ref,
                score_overall,
                score_breakdown,
                metrics,
                failure_tags,
                artifact_paths,
                notes,
                conclusion,
                created_at::text as created_at
            "#,
        )
        .bind(evaluation_id)
        .bind(experiment_id)
        .bind(hypothesis_id)
        .bind(strategy_run_id)
        .bind(task_id)
        .bind(target_wallet)
        .bind(family)
        .bind(strategy_name)
        .bind(source_type)
        .bind(source_ref)
        .bind(score_overall)
        .bind(Json(score_breakdown))
        .bind(Json(metrics))
        .bind(failure_tags)
        .bind(Json(artifact_paths))
        .bind(Json(notes))
        .bind(conclusion)
        .fetch_one(&self.pool)
        .await
        .context("failed to create evaluation")?;

        evaluation_from_row(&row)
    }

    pub async fn inspect_experiment(
        &self,
        experiment_id: &str,
    ) -> Result<Option<ExperimentDetail>> {
        let experiment = sqlx::query(
            r#"
            select
                experiment_id,
                title,
                target_wallet,
                status,
                thesis,
                notes,
                created_at::text as created_at,
                updated_at::text as updated_at
            from experiments
            where experiment_id = $1
            "#,
        )
        .bind(experiment_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch experiment")?;

        let Some(experiment) = experiment.as_ref().map(experiment_from_row).transpose()? else {
            return Ok(None);
        };

        let hypothesis_rows = sqlx::query(
            r#"
            select
                hypothesis_id,
                experiment_id,
                family,
                description,
                status,
                strategy_config,
                sample_window,
                notes,
                created_at::text as created_at,
                updated_at::text as updated_at
            from hypotheses
            where experiment_id = $1
            order by created_at asc
            "#,
        )
        .bind(experiment_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to fetch hypotheses")?;

        let evaluation_rows = sqlx::query(
            r#"
            select
                evaluation_id,
                experiment_id,
                hypothesis_id,
                strategy_run_id,
                task_id,
                target_wallet,
                family,
                strategy_name,
                source_type,
                source_ref,
                score_overall,
                score_breakdown,
                metrics,
                failure_tags,
                artifact_paths,
                notes,
                conclusion,
                created_at::text as created_at
            from evaluations
            where experiment_id = $1
            order by created_at asc
            "#,
        )
        .bind(experiment_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to fetch evaluations")?;

        Ok(Some(ExperimentDetail {
            experiment,
            hypotheses: hypothesis_rows
                .iter()
                .map(hypothesis_from_row)
                .collect::<Result<Vec<_>>>()?,
            evaluations: evaluation_rows
                .iter()
                .map(evaluation_from_row)
                .collect::<Result<Vec<_>>>()?,
        }))
    }

    pub async fn inspect_strategy_run(
        &self,
        run_id: i64,
        fill_limit: i64,
    ) -> Result<RunInspectReport> {
        let run = sqlx::query(
            r#"
            select
                id,
                strategy_name,
                run_mode,
                sweep_batch_id,
                live_run_id,
                config,
                source_type,
                source_ref,
                started_at::text as started_at,
                finished_at::text as finished_at,
                processed_events,
                fills,
                rejections,
                ending_cash_lamports::text as ending_cash_lamports,
                ending_equity_lamports::text as ending_equity_lamports
            from strategy_runs
            where id = $1
            "#,
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch strategy run")?
        .map(|row| StrategyRunDetail {
            id: row.try_get("id").expect("id"),
            strategy_name: row.try_get("strategy_name").expect("strategy_name"),
            run_mode: row.try_get("run_mode").expect("run_mode"),
            sweep_batch_id: row.try_get("sweep_batch_id").expect("sweep_batch_id"),
            live_run_id: row.try_get("live_run_id").expect("live_run_id"),
            config: row.try_get("config").expect("config"),
            source_type: row.try_get("source_type").expect("source_type"),
            source_ref: row.try_get("source_ref").expect("source_ref"),
            started_at: row.try_get("started_at").expect("started_at"),
            finished_at: row.try_get("finished_at").expect("finished_at"),
            processed_events: row.try_get("processed_events").expect("processed_events"),
            fills: row.try_get("fills").expect("fills"),
            rejections: row.try_get("rejections").expect("rejections"),
            ending_cash_lamports: row
                .try_get("ending_cash_lamports")
                .expect("ending_cash_lamports"),
            ending_equity_lamports: row
                .try_get("ending_equity_lamports")
                .expect("ending_equity_lamports"),
        });

        let rows = sqlx::query(
            r#"
            select
                order_id,
                mint,
                side,
                lamports::text as lamports,
                token_amount::text as token_amount,
                fee_lamports::text as fee_lamports,
                execution_price_lamports_per_token,
                reason,
                executed_at::text as executed_at
            from paper_fills
            where strategy_run_id = $1
            order by id asc
            limit $2
            "#,
        )
        .bind(run_id)
        .bind(fill_limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to fetch strategy run fills")?;

        let mut fills = Vec::with_capacity(rows.len());
        for row in rows {
            fills.push(RunFillRow {
                order_id: row.try_get("order_id")?,
                mint: row.try_get("mint")?,
                side: row.try_get("side")?,
                lamports: row.try_get("lamports")?,
                token_amount: row.try_get("token_amount")?,
                fee_lamports: row.try_get("fee_lamports")?,
                execution_price_lamports_per_token: row
                    .try_get("execution_price_lamports_per_token")?,
                reason: row.try_get("reason")?,
                executed_at: row.try_get("executed_at")?,
            });
        }

        let rows = sqlx::query(
            r#"
            select
                snapshot_kind,
                event_seq,
                event_slot,
                snapshot_at::text as snapshot_at,
                cash_lamports::text as cash_lamports,
                equity_lamports::text as equity_lamports,
                pending_orders,
                open_positions,
                positions
            from paper_position_snapshots
            where strategy_run_id = $1
            order by id desc
            limit 10
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to fetch strategy run position snapshots")?;

        let mut position_snapshots = Vec::with_capacity(rows.len());
        for row in rows {
            position_snapshots.push(PositionSnapshotRow {
                snapshot_kind: row.try_get("snapshot_kind")?,
                event_seq: row.try_get("event_seq")?,
                event_slot: row.try_get("event_slot")?,
                snapshot_at: row.try_get("snapshot_at")?,
                cash_lamports: row.try_get("cash_lamports")?,
                equity_lamports: row.try_get("equity_lamports")?,
                pending_orders: row.try_get("pending_orders")?,
                open_positions: row.try_get("open_positions")?,
                positions: row.try_get("positions")?,
            });
        }
        position_snapshots.reverse();

        Ok(RunInspectReport {
            run,
            fills,
            position_snapshots,
        })
    }

    pub async fn inspect_sweep_batch(
        &self,
        sweep_batch_id: &str,
    ) -> Result<SweepBatchInspectReport> {
        let rows = sqlx::query(
            r#"
            select
                id,
                strategy_name,
                run_mode,
                sweep_batch_id,
                started_at::text as started_at,
                processed_events,
                fills,
                rejections,
                ending_cash_lamports::text as ending_cash_lamports,
                ending_equity_lamports::text as ending_equity_lamports,
                config
            from strategy_runs
            where sweep_batch_id = $1
            order by ending_equity_lamports desc, rejections asc, fills desc, id asc
            "#,
        )
        .bind(sweep_batch_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to fetch sweep batch runs")?;

        let mut runs = Vec::with_capacity(rows.len());
        for row in rows {
            runs.push(SweepBatchRunRow {
                id: row.try_get("id")?,
                strategy_name: row.try_get("strategy_name")?,
                run_mode: row.try_get("run_mode")?,
                sweep_batch_id: row.try_get("sweep_batch_id")?,
                started_at: row.try_get("started_at")?,
                processed_events: row.try_get("processed_events")?,
                fills: row.try_get("fills")?,
                rejections: row.try_get("rejections")?,
                ending_cash_lamports: row.try_get("ending_cash_lamports")?,
                ending_equity_lamports: row.try_get("ending_equity_lamports")?,
                config: row.try_get("config")?,
            });
        }

        Ok(SweepBatchInspectReport {
            sweep_batch_id: sweep_batch_id.to_string(),
            runs,
        })
    }

    pub async fn persist_backtest_run(
        &self,
        source_type: &str,
        source_ref: &str,
        strategy_name: &str,
        strategy_config: serde_json::Value,
        result: &BacktestRunResult,
        options: StrategyRunPersistOptions,
    ) -> Result<i64> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("failed to begin strategy run transaction")?;

        let run_id: i64 = sqlx::query_scalar(
            r#"
            insert into strategy_runs (
                strategy_name, run_mode, sweep_batch_id, live_run_id,
                config, source_type, source_ref, started_at, finished_at,
                processed_events, fills, rejections, ending_cash_lamports, ending_equity_lamports
            )
            values (
                $1, $2, $3, $4,
                $5, $6, $7, now(), now(),
                $8, $9, $10, $11, $12
            )
            returning id
            "#,
        )
        .bind(strategy_name)
        .bind(
            options
                .run_mode
                .clone()
                .unwrap_or_else(|| "backtest".to_string()),
        )
        .bind(options.sweep_batch_id.clone())
        .bind(options.live_run_id.clone())
        .bind(Json(strategy_config))
        .bind(source_type)
        .bind(source_ref)
        .bind(i64_from_u64(result.report.processed_events)?)
        .bind(i64_from_u64(result.report.fills)?)
        .bind(i64_from_u64(result.report.rejections)?)
        .bind(i64_from_u64(result.report.ending_cash_lamports)?)
        .bind(i64_from_u64(result.report.ending_equity_lamports)?)
        .fetch_one(&mut *tx)
        .await
        .context("failed to insert strategy run")?;

        for fill in &result.fills {
            self.insert_paper_fill(&mut tx, run_id, fill).await?;
        }

        for snapshot in &options.position_snapshots {
            self.insert_position_snapshot(&mut tx, run_id, snapshot)
                .await?;
        }

        tx.commit()
            .await
            .context("failed to commit strategy run transaction")?;

        Ok(run_id)
    }

    pub async fn inspect_mint(&self, mint: &str, trade_limit: i64) -> Result<MintInspectReport> {
        let overview = sqlx::query(
            r#"
            with trade_agg as (
                select
                    mint,
                    count(*) as trade_count,
                    count(*) filter (where side = 'buy') as buy_count,
                    count(*) filter (where side = 'sell') as sell_count,
                    coalesce(sum(case when side = 'buy' then sol_amount else 0 end), 0)::text as gross_buy_lamports,
                    coalesce(sum(case when side = 'sell' then sol_amount else 0 end), 0)::text as gross_sell_lamports,
                    coalesce(sum(case when side = 'buy' then sol_amount else -sol_amount end), 0)::text as net_flow_lamports,
                    max(slot) as last_trade_slot,
                    max(timestamp)::text as last_trade_at
                from pump_trades
                where mint = $1
                group by mint
            )
            select
                m.mint,
                m.creator,
                m.bonding_curve,
                m.name,
                m.symbol,
                m.uri,
                m.token_program,
                (
                    coalesce(m.name, '') = ''
                    and coalesce(m.symbol, '') = ''
                    and coalesce(m.uri, '') = ''
                    and coalesce(m.token_program, '') = ''
                ) as is_inferred,
                m.created_slot,
                m.created_at::text as created_at,
                coalesce(t.trade_count, 0) as trade_count,
                coalesce(t.buy_count, 0) as buy_count,
                coalesce(t.sell_count, 0) as sell_count,
                coalesce(t.gross_buy_lamports, '0') as gross_buy_lamports,
                coalesce(t.gross_sell_lamports, '0') as gross_sell_lamports,
                coalesce(t.net_flow_lamports, '0') as net_flow_lamports,
                t.last_trade_slot,
                t.last_trade_at
            from pump_mints m
            left join trade_agg t on t.mint = m.mint
            where m.mint = $1
            "#,
        )
        .bind(mint)
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch mint overview")?
        .map(|row| MintOverview {
            mint: row.try_get("mint").expect("mint"),
            creator: row.try_get("creator").expect("creator"),
            bonding_curve: row.try_get("bonding_curve").expect("bonding_curve"),
            name: row.try_get("name").expect("name"),
            symbol: row.try_get("symbol").expect("symbol"),
            uri: row.try_get("uri").expect("uri"),
            token_program: row.try_get("token_program").expect("token_program"),
            is_inferred: row.try_get("is_inferred").expect("is_inferred"),
            created_slot: row.try_get("created_slot").expect("created_slot"),
            created_at: row.try_get("created_at").expect("created_at"),
            trade_count: row.try_get("trade_count").expect("trade_count"),
            buy_count: row.try_get("buy_count").expect("buy_count"),
            sell_count: row.try_get("sell_count").expect("sell_count"),
            gross_buy_lamports: row.try_get("gross_buy_lamports").expect("gross_buy_lamports"),
            gross_sell_lamports: row.try_get("gross_sell_lamports").expect("gross_sell_lamports"),
            net_flow_lamports: row.try_get("net_flow_lamports").expect("net_flow_lamports"),
            last_trade_slot: row.try_get("last_trade_slot").expect("last_trade_slot"),
            last_trade_at: row.try_get("last_trade_at").expect("last_trade_at"),
        });

        let rows = sqlx::query(
            r#"
            select
                seq,
                slot,
                side,
                sol_amount::text as sol_amount,
                token_amount::text as token_amount,
                user_address,
                tx_signature
            from pump_trades
            where mint = $1
            order by seq desc
            limit $2
            "#,
        )
        .bind(mint)
        .bind(trade_limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to fetch mint recent trades")?;

        let mut recent_trades = Vec::with_capacity(rows.len());
        for row in rows {
            recent_trades.push(MintTradeRow {
                seq: row.try_get("seq")?,
                slot: row.try_get("slot")?,
                side: row.try_get("side")?,
                sol_amount: row.try_get("sol_amount")?,
                token_amount: row.try_get("token_amount")?,
                user_address: row.try_get("user_address")?,
                tx_signature: row.try_get("tx_signature")?,
            });
        }
        recent_trades.reverse();

        Ok(MintInspectReport {
            overview,
            recent_trades,
        })
    }

    pub async fn inspect_address(
        &self,
        address: &str,
        top_mints_limit: i64,
        roundtrip_limit: i64,
    ) -> Result<AddressInspectReport> {
        let overview_row = sqlx::query(
            r#"
            select
                count(*) as total_trades,
                count(*) filter (where side = 'buy') as buy_count,
                count(*) filter (where side = 'sell') as sell_count,
                count(distinct mint) as distinct_mints,
                min(seq) as first_trade_seq,
                min(timestamp)::text as first_trade_at,
                max(seq) as last_trade_seq,
                max(timestamp)::text as last_trade_at,
                coalesce(sum(case when side = 'buy' then sol_amount else 0 end), 0)::text as gross_buy_lamports,
                coalesce(sum(case when side = 'sell' then sol_amount else 0 end), 0)::text as gross_sell_lamports,
                coalesce(
                    sum(
                        case
                            when side = 'buy' then -(sol_amount + fee + creator_fee - cashback)
                            else (sol_amount - fee - creator_fee + cashback)
                        end
                    ),
                    0
                )::text as net_cash_flow_lamports
            from pump_trades
            where user_address = $1
            "#,
        )
        .bind(address)
        .fetch_one(&self.pool)
        .await
        .context("failed to fetch address overview")?;

        let top_mint_rows = sqlx::query(
            r#"
            select
                mint,
                count(*) as trade_count,
                count(*) filter (where side = 'buy') as buy_count,
                count(*) filter (where side = 'sell') as sell_count,
                coalesce(sum(case when side = 'buy' then sol_amount else 0 end), 0)::text as gross_buy_lamports,
                coalesce(sum(case when side = 'sell' then sol_amount else 0 end), 0)::text as gross_sell_lamports,
                coalesce(
                    sum(
                        case
                            when side = 'buy' then -(sol_amount + fee + creator_fee - cashback)
                            else (sol_amount - fee - creator_fee + cashback)
                        end
                    ),
                    0
                )::text as net_cash_flow_lamports,
                min(seq) as first_seq,
                max(seq) as last_seq,
                max(timestamp)::text as last_trade_at
            from pump_trades
            where user_address = $1
            group by mint
            order by trade_count desc, last_seq desc
            limit $2
            "#,
        )
        .bind(address)
        .bind(top_mints_limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to fetch address top mints")?;

        let top_mints = top_mint_rows
            .into_iter()
            .map(|row| AddressMintSummary {
                mint: row.try_get("mint").expect("mint"),
                trade_count: row.try_get("trade_count").expect("trade_count"),
                buy_count: row.try_get("buy_count").expect("buy_count"),
                sell_count: row.try_get("sell_count").expect("sell_count"),
                gross_buy_lamports: row
                    .try_get("gross_buy_lamports")
                    .expect("gross_buy_lamports"),
                gross_sell_lamports: row
                    .try_get("gross_sell_lamports")
                    .expect("gross_sell_lamports"),
                net_cash_flow_lamports: row
                    .try_get("net_cash_flow_lamports")
                    .expect("net_cash_flow_lamports"),
                first_seq: row.try_get("first_seq").expect("first_seq"),
                last_seq: row.try_get("last_seq").expect("last_seq"),
                last_trade_at: row.try_get("last_trade_at").expect("last_trade_at"),
            })
            .collect::<Vec<_>>();

        let trade_facts = self.load_address_trade_facts(address).await?;
        let roundtrip_build = build_address_roundtrips(&trade_facts);
        let mut roundtrips = roundtrip_build.roundtrips.clone();
        roundtrips.sort_by(|left, right| right.opened_seq.cmp(&left.opened_seq));

        let closed_roundtrips = roundtrip_build
            .roundtrips
            .iter()
            .filter(|roundtrip| roundtrip.status == "closed")
            .collect::<Vec<_>>();
        let open_roundtrip_count =
            (roundtrip_build.roundtrips.len() - closed_roundtrips.len()) as i64;
        let realized_pnl_lamports = closed_roundtrips
            .iter()
            .filter_map(|roundtrip| roundtrip.realized_pnl_lamports.as_deref())
            .map(parse_i128_str)
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .sum::<i128>();
        let winning_roundtrips = closed_roundtrips
            .iter()
            .filter_map(|roundtrip| roundtrip.realized_pnl_lamports.as_deref())
            .map(parse_i128_str)
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|pnl| *pnl > 0)
            .count() as i64;
        let avg_hold_secs_closed = if closed_roundtrips.is_empty() {
            None
        } else {
            Some(
                closed_roundtrips
                    .iter()
                    .filter_map(|roundtrip| roundtrip.hold_secs)
                    .sum::<i64>()
                    / i64::try_from(closed_roundtrips.len()).expect("len fits into i64"),
            )
        };

        let overview = AddressOverview {
            address: address.to_string(),
            total_trades: overview_row.try_get("total_trades")?,
            buy_count: overview_row.try_get("buy_count")?,
            sell_count: overview_row.try_get("sell_count")?,
            distinct_mints: overview_row.try_get("distinct_mints")?,
            first_trade_seq: overview_row.try_get("first_trade_seq")?,
            first_trade_at: overview_row.try_get("first_trade_at")?,
            last_trade_seq: overview_row.try_get("last_trade_seq")?,
            last_trade_at: overview_row.try_get("last_trade_at")?,
            gross_buy_lamports: overview_row.try_get("gross_buy_lamports")?,
            gross_sell_lamports: overview_row.try_get("gross_sell_lamports")?,
            net_cash_flow_lamports: overview_row.try_get("net_cash_flow_lamports")?,
            roundtrip_count: i64::try_from(roundtrip_build.roundtrips.len())
                .expect("roundtrip count fits into i64"),
            closed_roundtrip_count: i64::try_from(closed_roundtrips.len())
                .expect("closed roundtrip count fits into i64"),
            open_roundtrip_count,
            orphan_sell_count: roundtrip_build.orphan_sell_count,
            realized_pnl_lamports: realized_pnl_lamports.to_string(),
            win_rate_closed: if closed_roundtrips.is_empty() {
                None
            } else {
                Some(winning_roundtrips as f64 / closed_roundtrips.len() as f64)
            },
            avg_hold_secs_closed,
        };

        roundtrips.truncate(usize::try_from(roundtrip_limit.max(0)).unwrap_or_default());

        Ok(AddressInspectReport {
            overview,
            top_mints,
            recent_roundtrips: roundtrips,
        })
    }

    pub async fn address_timeline(
        &self,
        address: &str,
        limit: i64,
        ascending: bool,
    ) -> Result<Vec<AddressTimelineRow>> {
        let order = if ascending { "asc" } else { "desc" };
        let query = format!(
            r#"
            select
                seq,
                slot,
                timestamp::text as timestamp,
                mint,
                side,
                sol_amount::text as sol_amount,
                token_amount::text as token_amount,
                fee::text as fee_lamports,
                creator_fee::text as creator_fee_lamports,
                cashback::text as cashback_lamports,
                ix_name,
                tx_signature
            from pump_trades
            where user_address = $1
            order by seq {order}
            limit $2
            "#
        );

        let rows = sqlx::query(&query)
            .bind(address)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .context("failed to fetch address timeline")?;

        rows.into_iter()
            .map(|row| {
                Ok(AddressTimelineRow {
                    seq: row.try_get("seq")?,
                    slot: row.try_get("slot")?,
                    timestamp: row.try_get("timestamp")?,
                    mint: row.try_get("mint")?,
                    side: row.try_get("side")?,
                    sol_amount: row.try_get("sol_amount")?,
                    token_amount: row.try_get("token_amount")?,
                    fee_lamports: row.try_get("fee_lamports")?,
                    creator_fee_lamports: row.try_get("creator_fee_lamports")?,
                    cashback_lamports: row.try_get("cashback_lamports")?,
                    ix_name: row.try_get("ix_name")?,
                    tx_signature: row.try_get("tx_signature")?,
                })
            })
            .collect()
    }

    pub async fn address_roundtrips(
        &self,
        address: &str,
        limit: i64,
    ) -> Result<AddressRoundtripReport> {
        let roundtrip_build =
            build_address_roundtrips(&self.load_address_trade_facts(address).await?);
        let total_roundtrips =
            i64::try_from(roundtrip_build.roundtrips.len()).expect("roundtrip count fits into i64");
        let closed_roundtrips = roundtrip_build
            .roundtrips
            .iter()
            .filter(|roundtrip| roundtrip.status == "closed")
            .count();
        let open_roundtrips = roundtrip_build
            .roundtrips
            .len()
            .saturating_sub(closed_roundtrips);
        let orphan_sell_count = roundtrip_build.orphan_sell_count;
        let mut roundtrips = roundtrip_build.roundtrips;
        roundtrips.sort_by(|left, right| right.opened_seq.cmp(&left.opened_seq));
        roundtrips.truncate(usize::try_from(limit.max(0)).unwrap_or_default());

        Ok(AddressRoundtripReport {
            address: address.to_string(),
            total_roundtrips,
            closed_roundtrips: i64::try_from(closed_roundtrips)
                .expect("closed roundtrip count fits into i64"),
            open_roundtrips: i64::try_from(open_roundtrips)
                .expect("open roundtrip count fits into i64"),
            orphan_sell_count,
            roundtrips,
        })
    }

    async fn load_address_trade_facts(&self, address: &str) -> Result<Vec<AddressTradeFact>> {
        let rows = sqlx::query(
            r#"
            select
                seq,
                slot,
                timestamp::text as timestamp_text,
                extract(epoch from timestamp)::bigint as timestamp_epoch,
                mint,
                side,
                sol_amount::text as sol_amount,
                token_amount::text as token_amount,
                fee::text as fee_lamports,
                creator_fee::text as creator_fee_lamports,
                cashback::text as cashback_lamports
            from pump_trades
            where user_address = $1
            order by seq asc
            "#,
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await
        .context("failed to load address trades")?;

        rows.into_iter()
            .map(|row| {
                Ok(AddressTradeFact {
                    seq: row.try_get("seq")?,
                    slot: row.try_get("slot")?,
                    timestamp_text: row.try_get("timestamp_text")?,
                    timestamp_epoch: row.try_get("timestamp_epoch")?,
                    mint: row.try_get("mint")?,
                    is_buy: row.try_get::<String, _>("side")? == "buy",
                    sol_amount: parse_i128_str(&row.try_get::<String, _>("sol_amount")?)?,
                    token_amount: parse_i128_str(&row.try_get::<String, _>("token_amount")?)?,
                    fee_lamports: parse_i128_str(&row.try_get::<String, _>("fee_lamports")?)?,
                    creator_fee_lamports: parse_i128_str(
                        &row.try_get::<String, _>("creator_fee_lamports")?,
                    )?,
                    cashback_lamports: parse_i128_str(
                        &row.try_get::<String, _>("cashback_lamports")?,
                    )?,
                })
            })
            .collect()
    }

    async fn ensure_mint_row_for_trade(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event: &EventEnvelope,
        trade: &crate::model::TradeEvent,
    ) -> Result<()> {
        sqlx::query(
            r#"
            insert into pump_mints (
                mint, seq, slot, tx_signature, tx_index, event_index,
                bonding_curve, creator, name, symbol, uri, created_slot, created_at,
                is_mayhem_mode, is_cashback_enabled, virtual_token_reserves,
                virtual_sol_reserves, real_token_reserves, token_total_supply,
                token_program, raw_event
            )
            values (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10, $11, $12, to_timestamp($13),
                $14, $15, $16, $17, $18, $19, $20, $21
            )
            on conflict (mint) do nothing
            "#,
        )
        .bind(&trade.mint)
        .bind(i64_from_u64(event.seq)?)
        .bind(i64_from_u64(event.slot)?)
        .bind(&event.tx_signature)
        .bind(i32_from_u32(event.tx_index))
        .bind(i32_from_u32(event.event_index))
        .bind("")
        .bind(&trade.creator)
        .bind("")
        .bind("")
        .bind("")
        .bind(i64_from_u64(event.slot)?)
        .bind(trade.timestamp as f64)
        .bind(trade.mayhem_mode)
        .bind(trade.cashback_fee_basis_points > 0 || trade.cashback > 0)
        .bind(i64_from_u64(trade.virtual_token_reserves)?)
        .bind(i64_from_u64(trade.virtual_sol_reserves)?)
        .bind(i64_from_u64(trade.real_token_reserves)?)
        .bind(0_i64)
        .bind("")
        .bind(Json(event.clone()))
        .execute(&mut **tx)
        .await
        .context("failed to insert placeholder pump mint for trade")?;

        Ok(())
    }

    async fn insert_paper_fill(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        strategy_run_id: i64,
        fill: &FillReport,
    ) -> Result<()> {
        sqlx::query(
            r#"
            insert into paper_fills (
                strategy_run_id, order_id, mint, side, lamports, token_amount,
                fee_lamports, execution_price_lamports_per_token, reason, executed_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, to_timestamp($10))
            "#,
        )
        .bind(strategy_run_id)
        .bind(i64_from_u64(fill.order_id)?)
        .bind(&fill.mint)
        .bind(match fill.side {
            crate::model::OrderSide::Buy => "buy",
            crate::model::OrderSide::Sell => "sell",
        })
        .bind(i64_from_u64(fill.lamports)?)
        .bind(i64_from_u64(fill.token_amount)?)
        .bind(i64_from_u64(fill.fee_lamports)?)
        .bind(fill.execution_price_lamports_per_token)
        .bind(&fill.reason)
        .bind(fill.timestamp.map(|ts| ts as f64))
        .execute(&mut **tx)
        .await
        .context("failed to insert paper fill")?;

        Ok(())
    }

    async fn insert_position_snapshot(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        strategy_run_id: i64,
        snapshot: &PositionSnapshotInput,
    ) -> Result<()> {
        sqlx::query(
            r#"
            insert into paper_position_snapshots (
                strategy_run_id, snapshot_kind, event_seq, event_slot, snapshot_at,
                cash_lamports, equity_lamports, pending_orders, open_positions, positions
            )
            values ($1, $2, $3, $4, to_timestamp($5), $6, $7, $8, $9, $10)
            "#,
        )
        .bind(strategy_run_id)
        .bind(&snapshot.snapshot_kind)
        .bind(snapshot.event_seq.map(i64_from_u64).transpose()?)
        .bind(snapshot.event_slot.map(i64_from_u64).transpose()?)
        .bind(snapshot.snapshot_at.map(|ts| ts as f64))
        .bind(i64_from_u64(snapshot.cash_lamports)?)
        .bind(i64_from_u64(snapshot.equity_lamports)?)
        .bind(
            i32::try_from(snapshot.pending_orders)
                .map_err(|_| anyhow!("pending_orders does not fit into i32"))?,
        )
        .bind(
            i32::try_from(snapshot.open_positions)
                .map_err(|_| anyhow!("open_positions does not fit into i32"))?,
        )
        .bind(Json(snapshot.positions.clone()))
        .execute(&mut **tx)
        .await
        .context("failed to insert paper position snapshot")?;

        Ok(())
    }

    async fn insert_event_envelope(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        event: &EventEnvelope,
    ) -> Result<()> {
        sqlx::query(
            r#"
            insert into pump_event_envelopes (
                seq, slot, tx_signature, tx_index, event_index, event_kind, envelope
            )
            values ($1, $2, $3, $4, $5, $6, $7)
            on conflict (slot, tx_signature, tx_index, event_index) do update set
                seq = excluded.seq,
                event_kind = excluded.event_kind,
                envelope = excluded.envelope
            "#,
        )
        .bind(i64_from_u64(event.seq)?)
        .bind(i64_from_u64(event.slot)?)
        .bind(&event.tx_signature)
        .bind(i32_from_u32(event.tx_index))
        .bind(i32_from_u32(event.event_index))
        .bind(event_kind(event))
        .bind(Json(event.clone()))
        .execute(&mut **tx)
        .await
        .context("failed to insert event envelope")?;

        Ok(())
    }
}

fn event_kind(event: &EventEnvelope) -> &'static str {
    match event.event {
        PumpEvent::MintCreated(_) => "mint_created",
        PumpEvent::Trade(_) => "trade",
        PumpEvent::CurveCompleted(_) => "curve_completed",
    }
}

fn build_address_roundtrips(trades: &[AddressTradeFact]) -> AddressRoundtripBuild {
    let mut active = HashMap::<String, ActiveAddressRoundtrip>::new();
    let mut roundtrips = Vec::new();
    let mut orphan_sell_count = 0_i64;

    for trade in trades {
        let total_fee = trade.fee_lamports + trade.creator_fee_lamports;

        if trade.is_buy {
            let state =
                active
                    .entry(trade.mint.clone())
                    .or_insert_with(|| ActiveAddressRoundtrip {
                        mint: trade.mint.clone(),
                        opened_seq: trade.seq,
                        opened_slot: trade.slot,
                        opened_at: trade.timestamp_text.clone(),
                        opened_at_epoch: trade.timestamp_epoch,
                        closed_seq: None,
                        closed_slot: None,
                        closed_at: None,
                        closed_at_epoch: None,
                        entry_count: 0,
                        exit_count: 0,
                        bought_token_amount: 0,
                        sold_token_amount: 0,
                        gross_buy_lamports: 0,
                        gross_sell_lamports: 0,
                        total_fees_lamports: 0,
                        total_cashback_lamports: 0,
                        net_entry_lamports: 0,
                        net_exit_lamports: 0,
                        remaining_cost_basis_lamports: 0,
                        token_balance: 0,
                        realized_pnl_lamports: 0,
                    });

            let buy_net = trade.sol_amount + total_fee - trade.cashback_lamports;
            state.entry_count += 1;
            state.bought_token_amount += trade.token_amount;
            state.gross_buy_lamports += trade.sol_amount;
            state.total_fees_lamports += total_fee;
            state.total_cashback_lamports += trade.cashback_lamports;
            state.net_entry_lamports += buy_net;
            state.remaining_cost_basis_lamports += buy_net;
            state.token_balance += trade.token_amount;
            continue;
        }

        let Some(state) = active.get_mut(&trade.mint) else {
            orphan_sell_count += 1;
            continue;
        };

        if state.token_balance <= 0 {
            orphan_sell_count += 1;
            continue;
        }

        let balance_before = state.token_balance;
        let matched_token_amount = trade.token_amount.min(balance_before);
        let sell_net = trade.sol_amount - total_fee + trade.cashback_lamports;
        let allocated_cost = if balance_before > 0 {
            state.remaining_cost_basis_lamports * matched_token_amount / balance_before
        } else {
            0
        };

        state.exit_count += 1;
        state.sold_token_amount += matched_token_amount;
        state.gross_sell_lamports += trade.sol_amount;
        state.total_fees_lamports += total_fee;
        state.total_cashback_lamports += trade.cashback_lamports;
        state.net_exit_lamports += sell_net;
        state.realized_pnl_lamports += sell_net - allocated_cost;
        state.remaining_cost_basis_lamports -= allocated_cost;
        state.token_balance -= matched_token_amount;
        state.closed_seq = Some(trade.seq);
        state.closed_slot = Some(trade.slot);
        state.closed_at = trade.timestamp_text.clone();
        state.closed_at_epoch = trade.timestamp_epoch;

        if trade.token_amount > matched_token_amount {
            orphan_sell_count += 1;
        }

        if state.token_balance == 0 {
            let completed = active
                .remove(&trade.mint)
                .expect("active roundtrip should still exist");
            roundtrips.push(finalize_address_roundtrip(completed));
        }
    }

    let mut open_roundtrips = active
        .into_values()
        .map(finalize_address_roundtrip)
        .collect::<Vec<_>>();
    roundtrips.append(&mut open_roundtrips);

    AddressRoundtripBuild {
        roundtrips,
        orphan_sell_count,
    }
}

fn finalize_address_roundtrip(state: ActiveAddressRoundtrip) -> AddressRoundtrip {
    let is_closed = state.token_balance == 0 && state.exit_count > 0;
    let realized_pnl_lamports = if is_closed {
        Some(state.realized_pnl_lamports.to_string())
    } else {
        None
    };
    let roi_bps = if is_closed && state.net_entry_lamports > 0 {
        Some((state.realized_pnl_lamports * 10_000 / state.net_entry_lamports) as i64)
    } else {
        None
    };
    let hold_secs = match (state.opened_at_epoch, state.closed_at_epoch) {
        (Some(opened), Some(closed)) => Some(closed - opened),
        _ => None,
    };

    AddressRoundtrip {
        mint: state.mint,
        status: if is_closed {
            "closed".to_string()
        } else {
            "open".to_string()
        },
        opened_seq: state.opened_seq,
        opened_slot: state.opened_slot,
        opened_at: state.opened_at,
        closed_seq: state.closed_seq,
        closed_slot: state.closed_slot,
        closed_at: state.closed_at,
        hold_secs,
        entry_count: state.entry_count,
        exit_count: state.exit_count,
        bought_token_amount: state.bought_token_amount.to_string(),
        sold_token_amount: state.sold_token_amount.to_string(),
        gross_buy_lamports: state.gross_buy_lamports.to_string(),
        gross_sell_lamports: state.gross_sell_lamports.to_string(),
        total_fees_lamports: state.total_fees_lamports.to_string(),
        total_cashback_lamports: state.total_cashback_lamports.to_string(),
        net_entry_lamports: state.net_entry_lamports.to_string(),
        net_exit_lamports: state.net_exit_lamports.to_string(),
        realized_pnl_lamports,
        roi_bps,
    }
}

fn parse_i128_str(value: &str) -> Result<i128> {
    value
        .parse::<i128>()
        .map_err(|error| anyhow!("invalid numeric value '{}': {}", value, error))
}

fn i64_from_u64(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| anyhow!("value {value} does not fit into i64"))
}

fn task_run_from_row(row: &sqlx::postgres::PgRow) -> Result<TaskRunRow> {
    Ok(TaskRunRow {
        task_id: row.try_get("task_id")?,
        task_kind: row.try_get("task_kind")?,
        status: row.try_get("status")?,
        idempotency_key: row.try_get("idempotency_key")?,
        cancellation_requested: row.try_get("cancellation_requested")?,
        request_payload: row.try_get("request_payload")?,
        result_payload: row.try_get("result_payload")?,
        error_payload: row.try_get("error_payload")?,
        submitted_at: row.try_get("submitted_at")?,
        started_at: row.try_get("started_at")?,
        finished_at: row.try_get("finished_at")?,
    })
}

fn experiment_from_row(row: &sqlx::postgres::PgRow) -> Result<ExperimentRow> {
    Ok(ExperimentRow {
        experiment_id: row.try_get("experiment_id")?,
        title: row.try_get("title")?,
        target_wallet: row.try_get("target_wallet")?,
        status: row.try_get("status")?,
        thesis: row.try_get("thesis")?,
        notes: row.try_get("notes")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn hypothesis_from_row(row: &sqlx::postgres::PgRow) -> Result<HypothesisRow> {
    Ok(HypothesisRow {
        hypothesis_id: row.try_get("hypothesis_id")?,
        experiment_id: row.try_get("experiment_id")?,
        family: row.try_get("family")?,
        description: row.try_get("description")?,
        status: row.try_get("status")?,
        strategy_config: row.try_get("strategy_config")?,
        sample_window: row.try_get("sample_window")?,
        notes: row.try_get("notes")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn evaluation_from_row(row: &sqlx::postgres::PgRow) -> Result<EvaluationRow> {
    Ok(EvaluationRow {
        evaluation_id: row.try_get("evaluation_id")?,
        experiment_id: row.try_get("experiment_id")?,
        hypothesis_id: row.try_get("hypothesis_id")?,
        strategy_run_id: row.try_get("strategy_run_id")?,
        task_id: row.try_get("task_id")?,
        target_wallet: row.try_get("target_wallet")?,
        family: row.try_get("family")?,
        strategy_name: row.try_get("strategy_name")?,
        source_type: row.try_get("source_type")?,
        source_ref: row.try_get("source_ref")?,
        score_overall: row.try_get("score_overall")?,
        score_breakdown: row.try_get("score_breakdown")?,
        metrics: row.try_get("metrics")?,
        failure_tags: row.try_get("failure_tags")?,
        artifact_paths: row.try_get("artifact_paths")?,
        notes: row.try_get("notes")?,
        conclusion: row.try_get("conclusion")?,
        created_at: row.try_get("created_at")?,
    })
}

fn i32_from_u32(value: u32) -> i32 {
    value as i32
}

#[cfg(test)]
mod tests {
    use super::{AddressTradeFact, build_address_roundtrips};

    fn trade(
        seq: i64,
        mint: &str,
        is_buy: bool,
        sol_amount: i128,
        token_amount: i128,
        timestamp_epoch: i64,
    ) -> AddressTradeFact {
        AddressTradeFact {
            seq,
            slot: 1_000 + seq,
            timestamp_text: Some(format!("2026-01-01 00:00:{:02}+00", seq)),
            timestamp_epoch: Some(timestamp_epoch),
            mint: mint.to_string(),
            is_buy,
            sol_amount,
            token_amount,
            fee_lamports: 0,
            creator_fee_lamports: 0,
            cashback_lamports: 0,
        }
    }

    #[test]
    fn builds_closed_roundtrip_with_positive_pnl() {
        let trades = vec![
            trade(1, "mint-a", true, 100, 1_000, 10),
            trade(2, "mint-a", true, 50, 500, 20),
            trade(3, "mint-a", false, 180, 1_500, 70),
        ];

        let report = build_address_roundtrips(&trades);
        assert_eq!(report.orphan_sell_count, 0);
        assert_eq!(report.roundtrips.len(), 1);

        let roundtrip = &report.roundtrips[0];
        assert_eq!(roundtrip.status, "closed");
        assert_eq!(roundtrip.entry_count, 2);
        assert_eq!(roundtrip.exit_count, 1);
        assert_eq!(roundtrip.realized_pnl_lamports.as_deref(), Some("30"));
        assert_eq!(roundtrip.roi_bps, Some(2000));
        assert_eq!(roundtrip.hold_secs, Some(60));
    }

    #[test]
    fn tracks_orphan_sells_and_open_positions() {
        let trades = vec![
            trade(1, "mint-a", false, 90, 1_000, 10),
            trade(2, "mint-b", true, 100, 1_000, 20),
            trade(3, "mint-b", false, 40, 400, 30),
        ];

        let report = build_address_roundtrips(&trades);
        assert_eq!(report.orphan_sell_count, 1);
        assert_eq!(report.roundtrips.len(), 1);

        let roundtrip = &report.roundtrips[0];
        assert_eq!(roundtrip.status, "open");
        assert_eq!(roundtrip.entry_count, 1);
        assert_eq!(roundtrip.exit_count, 1);
        assert_eq!(roundtrip.realized_pnl_lamports, None);
        assert_eq!(roundtrip.sold_token_amount, "400");
    }
}
