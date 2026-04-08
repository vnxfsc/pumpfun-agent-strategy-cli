use anyhow::Result;
use pump_agent_core::{
    EventStats, PgEventStore, RunInspectReport, StrategyRunRow, SweepBatchInspectReport,
};

use crate::strategy::{StrategyConfig, deserialize_strategy_config};

const SCHEMA_SQL: &str = include_str!("../../../../schema/postgres.sql");

#[derive(Debug, Clone)]
pub struct DatabaseRequest {
    pub database_url: String,
    pub max_db_connections: u32,
    pub apply_schema: bool,
}

#[derive(Debug, Clone)]
pub struct StatsRequest {
    pub database: DatabaseRequest,
}

#[derive(Debug, Clone)]
pub struct RunsRequest {
    pub database: DatabaseRequest,
    pub limit: i64,
}

#[derive(Debug, Clone)]
pub struct RunInspectRequest {
    pub database: DatabaseRequest,
    pub run_id: i64,
    pub fill_limit: i64,
}

#[derive(Debug, Clone)]
pub struct SweepBatchInspectRequest {
    pub database: DatabaseRequest,
    pub batch_id: String,
}

#[derive(Debug, Clone)]
pub struct CompareRunsRequest {
    pub database: DatabaseRequest,
    pub left_run_id: i64,
    pub right_run_id: i64,
    pub fill_limit: i64,
}

#[derive(Debug, Clone)]
pub struct LoadedCountDelta {
    pub left: usize,
    pub right: usize,
}

#[derive(Debug, Clone)]
pub struct CompareRunsDeltas {
    pub events: i64,
    pub fills: i64,
    pub rejections: i64,
    pub cash_sol: f64,
    pub equity_sol: f64,
}

#[derive(Debug, Clone)]
pub struct CompareRunsResult {
    pub left_run: pump_agent_core::StrategyRunDetail,
    pub right_run: pump_agent_core::StrategyRunDetail,
    pub left_strategy: StrategyConfig,
    pub right_strategy: StrategyConfig,
    pub loaded_fills: LoadedCountDelta,
    pub loaded_position_snapshots: LoadedCountDelta,
    pub deltas: CompareRunsDeltas,
}

pub async fn fetch_stats(request: StatsRequest) -> Result<EventStats> {
    let store = connect_store(&request.database).await?;
    store.fetch_event_stats().await
}

pub async fn list_runs(request: RunsRequest) -> Result<Vec<StrategyRunRow>> {
    let store = connect_store(&request.database).await?;
    store.list_strategy_runs(request.limit).await
}

pub async fn inspect_run(request: RunInspectRequest) -> Result<RunInspectReport> {
    let store = connect_store(&request.database).await?;
    store
        .inspect_strategy_run(request.run_id, request.fill_limit)
        .await
}

pub async fn inspect_sweep_batch(
    request: SweepBatchInspectRequest,
) -> Result<SweepBatchInspectReport> {
    let store = connect_store(&request.database).await?;
    store.inspect_sweep_batch(&request.batch_id).await
}

pub async fn compare_runs(request: CompareRunsRequest) -> Result<Option<CompareRunsResult>> {
    let store = connect_store(&request.database).await?;
    let left = store
        .inspect_strategy_run(request.left_run_id, request.fill_limit)
        .await?;
    let right = store
        .inspect_strategy_run(request.right_run_id, request.fill_limit)
        .await?;

    let (Some(left_run), Some(right_run)) = (left.run, right.run) else {
        return Ok(None);
    };

    let left_strategy = deserialize_strategy_config(&left_run.config)?;
    let right_strategy = deserialize_strategy_config(&right_run.config)?;
    let left_equity = lamports_str_to_sol(&left_run.ending_equity_lamports)?;
    let right_equity = lamports_str_to_sol(&right_run.ending_equity_lamports)?;
    let left_cash = lamports_str_to_sol(&left_run.ending_cash_lamports)?;
    let right_cash = lamports_str_to_sol(&right_run.ending_cash_lamports)?;
    let deltas = CompareRunsDeltas {
        events: right_run.processed_events - left_run.processed_events,
        fills: right_run.fills - left_run.fills,
        rejections: right_run.rejections - left_run.rejections,
        cash_sol: right_cash - left_cash,
        equity_sol: right_equity - left_equity,
    };

    Ok(Some(CompareRunsResult {
        left_run,
        right_run,
        left_strategy,
        right_strategy,
        loaded_fills: LoadedCountDelta {
            left: left.fills.len(),
            right: right.fills.len(),
        },
        loaded_position_snapshots: LoadedCountDelta {
            left: left.position_snapshots.len(),
            right: right.position_snapshots.len(),
        },
        deltas,
    }))
}

async fn connect_store(request: &DatabaseRequest) -> Result<PgEventStore> {
    let store = PgEventStore::connect(&request.database_url, request.max_db_connections).await?;
    if request.apply_schema {
        store.apply_schema(SCHEMA_SQL).await?;
    }
    Ok(store)
}

fn lamports_str_to_sol(value: &str) -> Result<f64> {
    Ok(value.parse::<i128>()? as f64 / 1_000_000_000.0)
}
