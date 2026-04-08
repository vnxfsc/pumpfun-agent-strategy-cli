use anyhow::{Result, anyhow};
use pump_agent_core::{BacktestReport, PgEventStore, StrategyRunPersistOptions};
use serde_json::json;

use crate::strategy::{
    StrategyConfig, SweepConfig, SweepRunSummary, generate_run_group_id, persist_run, run_strategy,
};

use super::{
    DatabaseRequest,
    experiments::{ExperimentContext, generate_record_id},
};

const SCHEMA_SQL: &str = include_str!("../../../../schema/postgres.sql");

#[derive(Debug, Clone)]
pub struct ReplayDbRequest {
    pub database: DatabaseRequest,
    pub strategy: StrategyConfig,
    pub save_run: bool,
    pub experiment: Option<ExperimentContext>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReplayDbResult {
    pub report: BacktestReport,
    pub saved_run_id: Option<i64>,
    pub recorded_evaluation_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SweepDbRequest {
    pub database: DatabaseRequest,
    pub strategy: StrategyConfig,
    pub sweep: SweepConfig,
    pub experiment: Option<ExperimentContext>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SweepDbResult {
    pub strategy: String,
    pub combinations: usize,
    pub sweep_batch_id: String,
    pub summaries: Vec<SweepRunSummary>,
    pub recorded_evaluation_ids: Vec<String>,
}

pub async fn replay_db(request: ReplayDbRequest) -> Result<ReplayDbResult> {
    let store = PgEventStore::connect(
        &request.database.database_url,
        request.database.max_db_connections,
    )
    .await?;
    let events = store.load_replay_events().await?;
    let execution = run_strategy(events, &request.strategy)?;
    let report = execution.result.report.clone();
    let mut saved_run_id = None;
    let mut recorded_evaluation_id = None;

    if request.save_run {
        store.apply_schema(SCHEMA_SQL).await?;
        saved_run_id = Some(
            persist_run(
                &store,
                "postgres",
                "pump_event_envelopes",
                &request.strategy,
                &execution.result,
                StrategyRunPersistOptions {
                    run_mode: Some("backtest".to_string()),
                    position_snapshots: vec![execution.final_position_snapshot.clone()],
                    ..Default::default()
                },
            )
            .await?,
        );
    }

    if let Some(context) = request.experiment {
        store.apply_schema(SCHEMA_SQL).await?;
        let experiment = store
            .get_experiment(&context.experiment_id)
            .await?
            .ok_or_else(|| anyhow!("experiment not found: {}", context.experiment_id))?;
        let evaluation_id = generate_record_id("eval");
        store
            .create_evaluation(
                &evaluation_id,
                &context.experiment_id,
                context.hypothesis_id.as_deref(),
                saved_run_id,
                None,
                &experiment.target_wallet,
                Some(&request.strategy.strategy),
                Some(report.strategy.name),
                "replay_db",
                "pump_event_envelopes",
                Some(report.ending_equity_lamports as f64),
                json!({
                    "ending_equity_lamports": report.ending_equity_lamports,
                    "ending_cash_lamports": report.ending_cash_lamports,
                    "fills": report.fills,
                    "rejections": report.rejections,
                    "open_positions": report.open_positions,
                }),
                json!({
                    "report": &report,
                    "strategy": &request.strategy,
                    "saved_run_id": saved_run_id,
                }),
                &context.failure_tags,
                context.artifact_paths,
                context.notes,
                context.conclusion.as_deref(),
            )
            .await?;
        recorded_evaluation_id = Some(evaluation_id);
    }

    Ok(ReplayDbResult {
        report,
        saved_run_id,
        recorded_evaluation_id,
    })
}

pub async fn sweep_db(request: SweepDbRequest) -> Result<SweepDbResult> {
    let store = PgEventStore::connect(
        &request.database.database_url,
        request.database.max_db_connections,
    )
    .await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let events = store.load_replay_events().await?;
    let base = crate::strategy::resolve_strategy_config(&request.strategy)?;
    let variants = crate::strategy::build_sweep_variants(&base, &request.sweep)?;
    let sweep_batch_id = generate_run_group_id("sweep");
    let mut summaries = Vec::with_capacity(variants.len());
    let mut recorded_evaluation_ids = Vec::new();
    let experiment = match &request.experiment {
        Some(context) => Some(
            store
                .get_experiment(&context.experiment_id)
                .await?
                .ok_or_else(|| anyhow!("experiment not found: {}", context.experiment_id))?,
        ),
        None => None,
    };

    for variant in &variants {
        let execution = run_strategy(events.clone(), variant)?;
        let run_id = persist_run(
            &store,
            "postgres_sweep",
            "pump_event_envelopes",
            variant,
            &execution.result,
            StrategyRunPersistOptions {
                run_mode: Some("sweep".to_string()),
                sweep_batch_id: Some(sweep_batch_id.clone()),
                position_snapshots: vec![execution.final_position_snapshot.clone()],
                ..Default::default()
            },
        )
        .await?;

        summaries.push(SweepRunSummary {
            run_id,
            strategy: variant.clone(),
            report: execution.result.report.clone(),
        });

        if let (Some(context), Some(experiment)) = (&request.experiment, &experiment) {
            let evaluation_id = generate_record_id("eval");
            store
                .create_evaluation(
                    &evaluation_id,
                    &context.experiment_id,
                    context.hypothesis_id.as_deref(),
                    Some(run_id),
                    None,
                    &experiment.target_wallet,
                    Some(&variant.strategy),
                    Some(execution.result.report.strategy.name),
                    "sweep_db",
                    &sweep_batch_id,
                    Some(execution.result.report.ending_equity_lamports as f64),
                    json!({
                        "ending_equity_lamports": execution.result.report.ending_equity_lamports,
                        "ending_cash_lamports": execution.result.report.ending_cash_lamports,
                        "fills": execution.result.report.fills,
                        "rejections": execution.result.report.rejections,
                        "open_positions": execution.result.report.open_positions,
                    }),
                    json!({
                        "report": &execution.result.report,
                        "strategy": variant,
                        "run_id": run_id,
                        "sweep_batch_id": &sweep_batch_id,
                    }),
                    &context.failure_tags,
                    context.artifact_paths.clone(),
                    context.notes.clone(),
                    context.conclusion.as_deref(),
                )
                .await?;
            recorded_evaluation_ids.push(evaluation_id);
        }
    }

    summaries.sort_by(|left, right| {
        right
            .report
            .ending_equity_lamports
            .cmp(&left.report.ending_equity_lamports)
            .then_with(|| left.report.rejections.cmp(&right.report.rejections))
            .then_with(|| right.report.fills.cmp(&left.report.fills))
    });

    Ok(SweepDbResult {
        strategy: base.strategy,
        combinations: variants.len(),
        sweep_batch_id,
        summaries,
        recorded_evaluation_ids,
    })
}
