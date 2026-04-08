use anyhow::{Result, anyhow};
use pump_agent_core::PgEventStore;
use serde_json::json;

use crate::{
    clone::{
        CloneFitSummary, StrategyCloneCandidate, WalletBehaviorReport, build_fit_variants,
        default_strategy_config_for_family, extract_wallet_behavior, run_clone_fit,
        score_strategy_execution,
    },
    strategy::{
        StrategyConfig, SweepConfig, deserialize_strategy_config, resolve_strategy_config,
        run_strategy,
    },
};

use super::{
    DatabaseRequest,
    experiments::{ExperimentContext, generate_record_id},
};

const SCHEMA_SQL: &str = include_str!("../../../../schema/postgres.sql");

#[derive(Debug, Clone)]
pub struct CloneAnalysisRequest {
    pub database: DatabaseRequest,
    pub address: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CloneAnalysis {
    pub wallet: WalletBehaviorReport,
    pub best_family: StrategyCloneCandidate,
    pub runner_up: StrategyCloneCandidate,
}

#[derive(Debug, Clone)]
pub struct CloneEvalRequest {
    pub database: DatabaseRequest,
    pub address: String,
    pub strategy: Option<StrategyConfig>,
    pub run_id: Option<i64>,
    pub experiment: Option<ExperimentContext>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CloneEvalResult {
    pub wallet: WalletBehaviorReport,
    pub resolved_strategy: StrategyConfig,
    pub eval_source: String,
    pub candidate: StrategyCloneCandidate,
    pub recorded_evaluation_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InferStrategyRequest {
    pub database: DatabaseRequest,
    pub address: String,
    pub family: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InferStrategyResult {
    pub wallet: WalletBehaviorReport,
    pub candidates: Vec<StrategyCloneCandidate>,
}

#[derive(Debug, Clone)]
pub struct FitParamsRequest {
    pub database: DatabaseRequest,
    pub address: String,
    pub family: String,
    pub base_overrides: StrategyConfig,
    pub sweep: SweepConfig,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FitParamsResult {
    pub wallet: WalletBehaviorReport,
    pub family: String,
    pub fit: CloneFitSummary,
}

#[derive(Debug, Clone)]
pub struct CloneRankRequest {
    pub database: DatabaseRequest,
    pub address: String,
    pub scan_limit: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CloneRankedRun {
    pub run_id: i64,
    pub strategy: StrategyConfig,
    pub strategy_name: String,
    pub run_mode: String,
    pub source_type: String,
    pub source_ref: String,
    pub started_at: String,
    pub stored_equity_lamports: String,
    pub candidate: StrategyCloneCandidate,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CloneRankResult {
    pub wallet: WalletBehaviorReport,
    pub ranked: Vec<CloneRankedRun>,
}

pub async fn analyze_clone_candidates(request: CloneAnalysisRequest) -> Result<CloneAnalysis> {
    let store = connect_store(&request.database).await?;
    let events = store.load_replay_events().await?;
    let wallet = extract_wallet_behavior(&request.address, &events);
    let mut candidates = evaluate_family_candidates(
        &events,
        &wallet,
        &["early_flow", "momentum", "breakout", "liquidity_follow"],
    )?;
    let best_family = candidates.remove(0);
    let runner_up = candidates.remove(0);

    Ok(CloneAnalysis {
        wallet,
        best_family,
        runner_up,
    })
}

pub async fn clone_eval(request: CloneEvalRequest) -> Result<Option<CloneEvalResult>> {
    let store = connect_store(&request.database).await?;
    let events = store.load_replay_events().await?;
    let wallet = extract_wallet_behavior(&request.address, &events);
    let (resolved_strategy, eval_source) = if let Some(run_id) = request.run_id {
        let inspect = store.inspect_strategy_run(run_id, 0).await?;
        let Some(run) = inspect.run else {
            return Ok(None);
        };
        (
            deserialize_strategy_config(&run.config)?,
            format!("run_id={}", run_id),
        )
    } else {
        let strategy = request
            .strategy
            .ok_or_else(|| anyhow!("missing strategy configuration for clone eval"))?;
        (
            resolve_strategy_config(&strategy)?,
            "strategy_args".to_string(),
        )
    };

    let execution = run_strategy(events, &resolved_strategy)?;
    let candidate = score_strategy_execution(&wallet, &resolved_strategy, &execution);
    let mut recorded_evaluation_id = None;

    if let Some(context) = request.experiment {
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
                request.run_id,
                None,
                &experiment.target_wallet,
                Some(&resolved_strategy.strategy),
                Some(candidate.report.strategy.name),
                "clone_eval",
                &eval_source,
            Some(candidate.score.overall),
            json!({
                "entry_timing_similarity": candidate.score.breakdown.entry_timing_similarity,
                "hold_time_similarity": candidate.score.breakdown.hold_time_similarity,
                "size_profile_similarity": candidate.score.breakdown.size_profile_similarity,
                "token_selection_similarity": candidate.score.breakdown.token_selection_similarity,
                "exit_behavior_similarity": candidate.score.breakdown.exit_behavior_similarity,
                "count_alignment": candidate.score.breakdown.count_alignment,
            }),
            json!({
                "wallet_summary": &wallet.summary,
                "resolved_strategy": &resolved_strategy,
                "eval_source": &eval_source,
                "precision": candidate.score.precision,
                "recall": candidate.score.recall,
                "f1": candidate.score.f1,
                "matched_entries": candidate.score.matched_entries,
                "wallet_entries": candidate.score.wallet_entries,
                "strategy_entries": candidate.score.strategy_entries,
                "entry_delay_secs": candidate.score.avg_entry_delay_secs,
                "hold_error_secs": candidate.score.avg_hold_error_secs,
                "size_error_ratio": candidate.score.avg_size_error_ratio,
                "fills": candidate.report.fills,
                "rejections": candidate.report.rejections,
                "ending_cash_lamports": candidate.report.ending_cash_lamports,
                    "ending_equity_lamports": candidate.report.ending_equity_lamports,
                }),
                &context.failure_tags,
                context.artifact_paths,
                context.notes,
                context.conclusion.as_deref(),
            )
            .await?;
        recorded_evaluation_id = Some(evaluation_id);
    }

    Ok(Some(CloneEvalResult {
        wallet,
        resolved_strategy,
        eval_source,
        candidate,
        recorded_evaluation_id,
    }))
}

pub async fn infer_strategy(request: InferStrategyRequest) -> Result<InferStrategyResult> {
    let store = connect_store(&request.database).await?;
    let events = store.load_replay_events().await?;
    let wallet = extract_wallet_behavior(&request.address, &events);
    let families = request
        .family
        .as_deref()
        .map(|family| vec![family.to_string()])
        .unwrap_or_else(|| {
            vec![
                "early_flow".to_string(),
                "momentum".to_string(),
                "breakout".to_string(),
                "liquidity_follow".to_string(),
            ]
        });

    let family_refs = families.iter().map(String::as_str).collect::<Vec<_>>();
    let candidates = evaluate_family_candidates(&events, &wallet, &family_refs)?;

    Ok(InferStrategyResult { wallet, candidates })
}

fn evaluate_family_candidates(
    events: &[pump_agent_core::EventEnvelope],
    wallet: &WalletBehaviorReport,
    families: &[&str],
) -> Result<Vec<StrategyCloneCandidate>> {
    let mut candidates = Vec::with_capacity(families.len());
    for family in families {
        let strategy = default_strategy_config_for_family(family)?;
        let execution = run_strategy(events.to_vec(), &strategy)?;
        candidates.push(score_strategy_execution(wallet, &strategy, &execution));
    }

    candidates.sort_by(|left, right| right.score.overall.total_cmp(&left.score.overall));
    Ok(candidates)
}

pub async fn fit_params(request: FitParamsRequest) -> Result<FitParamsResult> {
    let store = connect_store(&request.database).await?;
    let events = store.load_replay_events().await?;
    let wallet = extract_wallet_behavior(&request.address, &events);
    let mut base = default_strategy_config_for_family(&request.family)?;
    base.strategy = request.family.replace('-', "_");
    base.starting_sol = request.base_overrides.starting_sol;
    base.trading_fee_bps = request.base_overrides.trading_fee_bps;
    base.slippage_bps = request.base_overrides.slippage_bps;
    let variants = build_fit_variants(&base, &request.sweep)?;
    let fit = run_clone_fit(&events, &wallet, variants)?;

    Ok(FitParamsResult {
        wallet,
        family: request.family,
        fit,
    })
}

pub async fn clone_rank(request: CloneRankRequest) -> Result<CloneRankResult> {
    let store = connect_store(&request.database).await?;
    let events = store.load_replay_events().await?;
    let wallet = extract_wallet_behavior(&request.address, &events);
    let runs = store.list_strategy_runs(request.scan_limit).await?;

    let mut ranked = Vec::new();
    for run in runs {
        let inspect = store.inspect_strategy_run(run.id, 0).await?;
        let Some(detail) = inspect.run else {
            continue;
        };
        let Ok(strategy) = deserialize_strategy_config(&detail.config) else {
            continue;
        };
        let execution = run_strategy(events.clone(), &strategy)?;
        let candidate = score_strategy_execution(&wallet, &strategy, &execution);
        ranked.push(CloneRankedRun {
            run_id: detail.id,
            strategy,
            strategy_name: candidate.report.strategy.name.to_string(),
            run_mode: detail.run_mode,
            source_type: detail.source_type,
            source_ref: detail.source_ref,
            started_at: detail.started_at,
            stored_equity_lamports: detail.ending_equity_lamports,
            candidate,
        });
    }

    ranked.sort_by(|left, right| {
        right
            .candidate
            .score
            .overall
            .total_cmp(&left.candidate.score.overall)
            .then_with(|| right.candidate.score.f1.total_cmp(&left.candidate.score.f1))
            .then_with(|| {
                right
                    .candidate
                    .score
                    .count_alignment
                    .total_cmp(&left.candidate.score.count_alignment)
            })
            .then_with(|| right.run_id.cmp(&left.run_id))
    });

    Ok(CloneRankResult { wallet, ranked })
}

async fn connect_store(request: &DatabaseRequest) -> Result<PgEventStore> {
    let store = PgEventStore::connect(&request.database_url, request.max_db_connections).await?;
    if request.apply_schema {
        store.apply_schema(SCHEMA_SQL).await?;
    }
    Ok(store)
}
