use serde::Serialize;

use crate::{
    clone::{
        CloneScoreBreakdown, StrategyCloneCandidate, WalletBehaviorReport, WalletBehaviorSummary,
        WalletEntryFeature, WalletRoundtrip,
    },
    strategy::{StrategyConfig, SweepConfig, SweepRunSummary},
    usecases::{
        CloneEvalResult, CloneRankResult, CompareRunsResult, EvaluationSummary,
        ExperimentDetailResult, FitParamsResult, InferStrategyResult, MintShardSummaryResult,
        SweepDbResult,
    },
};
use pump_agent_core::{
    AddressInspectReport, AddressMintSummary, AddressOverview, AddressRoundtrip, ExperimentRow,
    HypothesisRow, TaskRunRow,
};

#[derive(Debug, Clone, Serialize)]
pub struct CloneReportOutput {
    pub address: String,
    pub recommended_base_family: String,
    pub recommended_next_strategy_name: String,
    pub base_fit: FitSummary,
    pub runner_up: FitSummary,
    pub confirmed_rules: Vec<String>,
    pub tentative_rules: Vec<String>,
    pub anti_patterns: Vec<String>,
    pub recommended_params_seed: ParamsSeed,
    pub export: Option<CloneReportExportSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExplainWhyOutput {
    pub address: String,
    pub recommended_family: String,
    pub runner_up_family: String,
    pub confidence: String,
    pub decision_summary: String,
    pub family_gap: f64,
    pub wallet_summary: WalletBehaviorSummary,
    pub base_clone_score: f64,
    pub runner_up_clone_score: f64,
    pub base_breakdown: CloneScoreBreakdownOutput,
    pub runner_up_breakdown: CloneScoreBreakdownOutput,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub warnings: Vec<String>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestNextExperimentOutput {
    pub address: String,
    pub experiment_id: Option<String>,
    pub recommended_family: String,
    pub confidence: String,
    pub history_summary: Option<String>,
    pub proposals: Vec<ExperimentProposalOutput>,
    pub skipped_families: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalletDossierOutput {
    pub address: String,
    pub experiment_id: Option<String>,
    pub overview: AddressOverview,
    pub top_mints: Vec<AddressMintSummary>,
    pub recent_roundtrips: Vec<AddressRoundtrip>,
    pub wallet_summary: WalletBehaviorSummary,
    pub sample_entries: Vec<WalletEntryFeature>,
    pub sample_roundtrips: Vec<WalletRoundtrip>,
    pub clone_report: CloneReportOutput,
    pub explain_why: ExplainWhyOutput,
    pub suggest_next_experiment: SuggestNextExperimentOutput,
}

#[derive(Debug, Clone, Serialize)]
pub struct MintShardSummaryOutput {
    pub address: String,
    pub mint_count: i64,
    pub wallet_trade_count: i64,
    pub total_event_count: usize,
    pub shards: Vec<MintShardOutput>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MintShardOutput {
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

#[derive(Debug, Clone, Serialize)]
pub struct CompareRunsOutput {
    pub left_run: pump_agent_core::StrategyRunDetail,
    pub right_run: pump_agent_core::StrategyRunDetail,
    pub left_strategy: StrategyConfig,
    pub right_strategy: StrategyConfig,
    pub loaded_fills: LoadedCountOutput,
    pub loaded_position_snapshots: LoadedCountOutput,
    pub deltas: CompareRunsDeltasOutput,
    pub strategy_diff: StrategyDiffOutput,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoadedCountOutput {
    pub left: usize,
    pub right: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompareRunsDeltasOutput {
    pub events: i64,
    pub fills: i64,
    pub rejections: i64,
    pub cash_sol: f64,
    pub equity_sol: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StrategyDiffOutput {
    pub family_changed: bool,
    pub changed_field_count: usize,
    pub changed_fields: Vec<StrategyFieldDiffOutput>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StrategyFieldDiffOutput {
    pub field: &'static str,
    pub left: serde_json::Value,
    pub right: serde_json::Value,
    pub numeric_delta: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExperimentProposalOutput {
    pub priority: String,
    pub title: String,
    pub family: String,
    pub objective: String,
    pub rationale: String,
    pub expected_learning: String,
    pub strategy: StrategyConfig,
    pub sweep: SweepConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloneReportExportSummary {
    pub output: String,
    pub address_dir: Option<String>,
    pub index_path: Option<String>,
    pub mint_count: i64,
    pub wallet_trade_count: i64,
    pub event_count: usize,
    pub shard_count: usize,
    pub sharded: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloneScoreBreakdownOutput {
    pub entry_timing_similarity: f64,
    pub hold_time_similarity: f64,
    pub size_profile_similarity: f64,
    pub token_selection_similarity: f64,
    pub exit_behavior_similarity: f64,
    pub count_alignment: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FitSummary {
    pub family: String,
    pub clone_score: f64,
    pub f1: f64,
    pub precision: f64,
    pub recall: f64,
    pub breakdown: CloneScoreBreakdownOutput,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParamsSeed {
    pub buy_sol: f64,
    pub max_age_secs: i64,
    pub min_buy_count: u64,
    pub min_unique_buyers: usize,
    pub min_net_buy_sol: f64,
    pub min_total_buy_sol: f64,
    pub max_sell_count: u64,
    pub min_buy_sell_ratio: f64,
    pub max_hold_secs: i64,
    pub max_concurrent_positions: usize,
    pub exit_on_sell_count: u64,
    pub take_profit_bps: i64,
    pub stop_loss_bps: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloneEvalOutput {
    pub address: String,
    pub wallet_entries: usize,
    pub wallet_roundtrips: usize,
    pub wallet_closed_roundtrips: usize,
    pub strategy: String,
    pub strategy_name: String,
    pub eval_source: String,
    pub clone_score: f64,
    pub f1: f64,
    pub precision: f64,
    pub recall: f64,
    pub matched_entries: usize,
    pub strategy_entries: usize,
    pub entry_delay_secs: Option<f64>,
    pub hold_error_secs: Option<f64>,
    pub size_error_ratio: Option<f64>,
    pub count_alignment: f64,
    pub breakdown: CloneScoreBreakdownOutput,
    pub fills: u64,
    pub rejections: u64,
    pub ending_equity_lamports: u64,
    pub ending_cash_lamports: u64,
    pub resolved_strategy: StrategyConfig,
    pub recorded_evaluation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InferStrategyOutput {
    pub address: String,
    pub wallet_summary: WalletBehaviorSummary,
    pub candidates: Vec<InferStrategyCandidate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InferStrategyCandidate {
    pub family: String,
    pub strategy_name: String,
    pub clone_score: f64,
    pub f1: f64,
    pub precision: f64,
    pub recall: f64,
    pub matched_entries: usize,
    pub wallet_entries: usize,
    pub strategy_entries: usize,
    pub entry_delay_secs: Option<f64>,
    pub hold_error_secs: Option<f64>,
    pub size_error_ratio: Option<f64>,
    pub count_alignment: f64,
    pub breakdown: CloneScoreBreakdownOutput,
    pub fills: u64,
    pub ending_equity_lamports: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FitParamsOutput {
    pub address: String,
    pub family: String,
    pub wallet_summary: WalletBehaviorSummary,
    pub candidate_count: usize,
    pub top_candidates: Vec<FitParamsCandidate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FitParamsCandidate {
    pub args: StrategyConfig,
    pub strategy_name: String,
    pub clone_score: f64,
    pub f1: f64,
    pub precision: f64,
    pub recall: f64,
    pub matched_entries: usize,
    pub strategy_entries: usize,
    pub entry_delay_secs: Option<f64>,
    pub hold_error_secs: Option<f64>,
    pub size_error_ratio: Option<f64>,
    pub count_alignment: f64,
    pub breakdown: CloneScoreBreakdownOutput,
    pub fills: u64,
    pub rejections: u64,
    pub ending_equity_lamports: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloneRankOutput {
    pub address: String,
    pub wallet_entries: usize,
    pub wallet_roundtrips: usize,
    pub ranked: Vec<CloneRankRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloneRankRow {
    pub run_id: i64,
    pub strategy: String,
    pub strategy_name: String,
    pub run_mode: String,
    pub source_type: String,
    pub source_ref: String,
    pub started_at: String,
    pub stored_equity_lamports: String,
    pub clone_score: f64,
    pub f1: f64,
    pub precision: f64,
    pub recall: f64,
    pub matched_entries: usize,
    pub strategy_entries: usize,
    pub entry_delay_secs: Option<f64>,
    pub hold_error_secs: Option<f64>,
    pub size_error_ratio: Option<f64>,
    pub count_alignment: f64,
    pub breakdown: CloneScoreBreakdownOutput,
}

#[derive(Debug, Clone, Serialize)]
pub struct SweepDbOutput {
    pub strategy: String,
    pub combinations: usize,
    pub sweep_batch_id: String,
    pub recorded_evaluation_ids: Vec<String>,
    pub top_results: Vec<SweepResultRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SweepResultRow {
    pub run_id: i64,
    pub strategy_name: &'static str,
    pub ending_equity_sol: f64,
    pub ending_cash_sol: f64,
    pub fills: u64,
    pub rejections: u64,
    pub open_positions: usize,
    pub buy_sol: f64,
    pub max_age_secs: i64,
    pub min_total_buy_sol: f64,
    pub max_sell_count: u64,
    pub min_buy_sell_ratio: f64,
    pub max_concurrent_positions: usize,
    pub exit_on_sell_count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskRunOutput {
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

#[derive(Debug, Clone, Serialize)]
pub struct ExperimentOutput {
    pub experiment_id: String,
    pub title: String,
    pub target_wallet: String,
    pub status: String,
    pub thesis: Option<String>,
    pub notes: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HypothesisOutput {
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

#[derive(Debug, Clone, Serialize)]
pub struct EvaluationOutput {
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

#[derive(Debug, Clone, Serialize)]
pub struct ExperimentDetailOutput {
    pub experiment: ExperimentOutput,
    pub hypotheses: Vec<HypothesisOutput>,
    pub evaluations: Vec<EvaluationOutput>,
}

pub fn build_clone_report(
    wallet: &WalletBehaviorReport,
    best_family: &StrategyCloneCandidate,
    runner_up: &StrategyCloneCandidate,
    export: Option<CloneReportExportSummary>,
) -> CloneReportOutput {
    let avg_buy_sol = wallet.summary.avg_entry_buy_sol.unwrap_or(0.2);
    let avg_age_secs = wallet.summary.avg_entry_age_secs.unwrap_or(20.0);
    let avg_buy_count = wallet.summary.avg_entry_buy_count_before.unwrap_or(4.0);
    let avg_unique = wallet.summary.avg_entry_unique_buyers_before.unwrap_or(4.0);
    let avg_total_buy_sol = wallet.summary.avg_entry_total_buy_sol_before.unwrap_or(1.0);
    let avg_ratio = wallet
        .summary
        .avg_entry_buy_sell_ratio_before
        .unwrap_or(4.0);
    let avg_hold_secs = wallet.summary.avg_hold_secs_closed.unwrap_or(45.0);
    let avg_sell_before = wallet.summary.avg_entry_sell_count_before.unwrap_or(0.0);

    let recommended_next_strategy_name = if best_family.args.strategy == "early_flow" {
        if avg_age_secs > 12.0 {
            "confirmed_flow".to_string()
        } else {
            "early_flow_plus".to_string()
        }
    } else if avg_hold_secs < 20.0 {
        "micro_momentum".to_string()
    } else {
        "momentum_plus".to_string()
    };

    let confirmed_rules = vec![
        format!(
            "uses roughly fixed ticket size around {:.3} SOL",
            avg_buy_sol
        ),
        format!(
            "typically enters after confirmation, not at t=0; average entry age {:.1}s",
            avg_age_secs
        ),
        format!(
            "requires meaningful pre-entry activity: buys_before≈{:.1}, unique_buyers≈{:.1}",
            avg_buy_count, avg_unique
        ),
        format!(
            "prefers already-hot mints: total_buy_before≈{:.2} SOL, net_flow_before≈{:.2} SOL",
            avg_total_buy_sol,
            wallet
                .summary
                .avg_entry_net_flow_sol_before
                .unwrap_or_default()
        ),
        format!(
            "exits quickly; average closed hold is {:.1}s",
            avg_hold_secs
        ),
    ];

    let tentative_rules = vec![
        format!(
            "may tolerate some early sell pressure before entry; sells_before≈{:.1}",
            avg_sell_before
        ),
        format!(
            "buy/sell imbalance matters, but not in sniper mode; ratio_before≈{:.2}",
            avg_ratio
        ),
        format!(
            "existing family fit is only moderate (clone_score {:.4}), so custom variant is likely needed",
            best_family.score.overall
        ),
    ];

    let mut anti_patterns = Vec::new();
    if avg_age_secs > 10.0 {
        anti_patterns.push("do not model this as first-seconds sniper entry".to_string());
    }
    if avg_sell_before > 2.0 {
        anti_patterns.push("do not reject every mint with any early sells".to_string());
    }
    if avg_buy_sol < 0.35 {
        anti_patterns.push("do not model this as size-scaling conviction trader".to_string());
    }
    if wallet.summary.orphan_sell_count == 0 {
        anti_patterns.push(
            "do not overfit around missing historical positions; sample window is clean"
                .to_string(),
        );
    }

    CloneReportOutput {
        address: wallet.address.clone(),
        recommended_base_family: best_family.args.strategy.clone(),
        recommended_next_strategy_name,
        base_fit: FitSummary {
            family: best_family.args.strategy.clone(),
            clone_score: best_family.score.overall,
            f1: best_family.score.f1,
            precision: best_family.score.precision,
            recall: best_family.score.recall,
            breakdown: clone_score_breakdown_output(&best_family.score.breakdown),
        },
        runner_up: FitSummary {
            family: runner_up.args.strategy.clone(),
            clone_score: runner_up.score.overall,
            f1: runner_up.score.f1,
            precision: runner_up.score.precision,
            recall: runner_up.score.recall,
            breakdown: clone_score_breakdown_output(&runner_up.score.breakdown),
        },
        confirmed_rules,
        tentative_rules,
        anti_patterns,
        recommended_params_seed: ParamsSeed {
            buy_sol: round_to(avg_buy_sol, 3),
            max_age_secs: avg_age_secs.round().clamp(5.0, 120.0) as i64,
            min_buy_count: avg_buy_count.floor().clamp(2.0, 100.0) as u64,
            min_unique_buyers: avg_unique.floor().clamp(2.0, 100.0) as usize,
            min_net_buy_sol: round_to(
                wallet
                    .summary
                    .avg_entry_net_flow_sol_before
                    .unwrap_or((avg_total_buy_sol * 0.6).max(0.3))
                    .max(0.1),
                3,
            ),
            min_total_buy_sol: round_to(avg_total_buy_sol.max(avg_buy_sol * 5.0), 3),
            max_sell_count: avg_sell_before.ceil().clamp(0.0, 20.0) as u64,
            min_buy_sell_ratio: round_to(avg_ratio.max(1.0), 2),
            max_hold_secs: avg_hold_secs.round().clamp(5.0, 300.0) as i64,
            max_concurrent_positions: 3,
            exit_on_sell_count: avg_sell_before.ceil().clamp(1.0, 6.0) as u64,
            take_profit_bps: 1800,
            stop_loss_bps: 900,
        },
        export,
    }
}

pub fn clone_explain_why_output(
    wallet: &WalletBehaviorReport,
    best_family: &StrategyCloneCandidate,
    runner_up: &StrategyCloneCandidate,
) -> ExplainWhyOutput {
    let confidence = explanation_confidence(best_family.score.overall, runner_up.score.overall);
    let family_gap = round_to(best_family.score.overall - runner_up.score.overall, 4);
    let strongest = strongest_dimensions(best_family, runner_up);
    let weakest = weakest_dimensions(best_family);
    let strengths = strongest
        .iter()
        .map(|(label, best, delta)| {
            format!(
                "{} is a relative strength ({:.3} vs runner-up {:.3}, delta {:+.3})",
                label,
                best,
                best - delta,
                delta
            )
        })
        .collect::<Vec<_>>();
    let weaknesses = if weakest.is_empty() {
        vec![format!(
            "{} is the best fit, but the remaining gaps are distributed rather than concentrated in one dimension",
            best_family.args.strategy
        )]
    } else {
        weakest
            .iter()
            .map(|(label, score)| format!("{} is still weak ({:.3})", label, score))
            .collect()
    };

    let lead_dimension = strongest
        .first()
        .map(|(label, _, _)| *label)
        .unwrap_or("overall alignment");
    let drag_dimension = weakest
        .first()
        .map(|(label, _)| *label)
        .unwrap_or("fine-grained parameter tuning");
    let decision_summary = format!(
        "{} is ahead of {} mainly because {} fits better, but {} still needs work",
        best_family.args.strategy, runner_up.args.strategy, lead_dimension, drag_dimension
    );

    ExplainWhyOutput {
        address: wallet.address.clone(),
        recommended_family: best_family.args.strategy.clone(),
        runner_up_family: runner_up.args.strategy.clone(),
        confidence: confidence.to_string(),
        decision_summary,
        family_gap,
        wallet_summary: wallet.summary.clone(),
        base_clone_score: best_family.score.overall,
        runner_up_clone_score: runner_up.score.overall,
        base_breakdown: clone_score_breakdown_output(&best_family.score.breakdown),
        runner_up_breakdown: clone_score_breakdown_output(&runner_up.score.breakdown),
        strengths,
        weaknesses,
        warnings: explanation_warnings(wallet, best_family, runner_up, family_gap),
        next_actions: explanation_next_actions(wallet, best_family, runner_up, &weakest),
    }
}

pub fn suggest_next_experiment_output(
    wallet: &WalletBehaviorReport,
    best_family: &StrategyCloneCandidate,
    runner_up: &StrategyCloneCandidate,
    experiment: Option<&ExperimentDetailResult>,
) -> SuggestNextExperimentOutput {
    let report = build_clone_report(wallet, best_family, runner_up, None);
    let explain = clone_explain_why_output(wallet, best_family, runner_up);
    let tested_families = experiment
        .map(experiment_tested_families)
        .unwrap_or_default();
    let weak_dimensions = weakest_dimensions(best_family);
    let has_best_history = tested_families.contains(&best_family.args.strategy);
    let has_runner_up_history = tested_families.contains(&runner_up.args.strategy);

    let mut proposals = Vec::new();
    proposals.push(ExperimentProposalOutput {
        priority: "p0".to_string(),
        title: format!("fit {}", best_family.args.strategy),
        family: best_family.args.strategy.clone(),
        objective: format!(
            "turn {} from best family guess into a tuned wallet-specific candidate",
            best_family.args.strategy
        ),
        rationale: explain.decision_summary.clone(),
        expected_learning: format!(
            "confirm whether {} can clear the weakest dimension: {}",
            best_family.args.strategy,
            weak_dimensions
                .first()
                .map(|(label, _)| *label)
                .unwrap_or("parameter tuning")
        ),
        strategy: strategy_from_seed(&best_family.args.strategy, &report.recommended_params_seed),
        sweep: sweep_from_seed(&report.recommended_params_seed, &weak_dimensions),
    });

    if !has_runner_up_history || best_family.score.overall - runner_up.score.overall < 0.05 {
        proposals.push(ExperimentProposalOutput {
            priority: "p1".to_string(),
            title: format!("branch {}", runner_up.args.strategy),
            family: runner_up.args.strategy.clone(),
            objective: format!(
                "test whether {} is a better inductive bias than {}",
                runner_up.args.strategy, best_family.args.strategy
            ),
            rationale: format!(
                "{} remains the closest alternative with clone_score {:.4}",
                runner_up.args.strategy, runner_up.score.overall
            ),
            expected_learning: format!(
                "separate family choice from parameter choice by replaying a tuned {} branch",
                runner_up.args.strategy
            ),
            strategy: strategy_from_seed(&runner_up.args.strategy, &report.recommended_params_seed),
            sweep: sweep_from_seed(&report.recommended_params_seed, &weak_dimensions),
        });
    }

    let focus_family = if has_best_history && !has_runner_up_history {
        &runner_up.args.strategy
    } else {
        &best_family.args.strategy
    };
    proposals.push(ExperimentProposalOutput {
        priority: "p2".to_string(),
        title: format!("stress {}", focus_family),
        family: focus_family.clone(),
        objective: "stress the current leading idea with a tighter targeted sweep".to_string(),
        rationale: targeted_rationale(&weak_dimensions),
        expected_learning: "identify whether the miss is mostly entry timing, exit policy, sizing, or mint selection".to_string(),
        strategy: strategy_from_seed(focus_family, &report.recommended_params_seed),
        sweep: targeted_sweep_from_seed(&report.recommended_params_seed, &weak_dimensions),
    });

    let skipped_families = tested_families
        .iter()
        .filter(|family| !proposals.iter().any(|proposal| proposal.family == **family))
        .cloned()
        .collect::<Vec<_>>();

    SuggestNextExperimentOutput {
        address: wallet.address.clone(),
        experiment_id: experiment.map(|detail| detail.experiment.experiment_id.clone()),
        recommended_family: best_family.args.strategy.clone(),
        confidence: explain.confidence,
        history_summary: experiment.map(|detail| {
            format!(
                "experiment has {} hypotheses, {} evaluations, tested families: {}",
                detail.hypotheses.len(),
                detail.evaluations.len(),
                tested_families.join(", ")
            )
        }),
        proposals,
        skipped_families,
    }
}

pub fn wallet_dossier_output(
    address_inspect: AddressInspectReport,
    wallet: &WalletBehaviorReport,
    best_family: &StrategyCloneCandidate,
    runner_up: &StrategyCloneCandidate,
    experiment: Option<&ExperimentDetailResult>,
    sample_limit: usize,
) -> WalletDossierOutput {
    let clone_report = build_clone_report(wallet, best_family, runner_up, None);
    let explain_why = clone_explain_why_output(wallet, best_family, runner_up);
    let suggest_next_experiment =
        suggest_next_experiment_output(wallet, best_family, runner_up, experiment);

    WalletDossierOutput {
        address: wallet.address.clone(),
        experiment_id: experiment.map(|detail| detail.experiment.experiment_id.clone()),
        overview: address_inspect.overview,
        top_mints: address_inspect.top_mints,
        recent_roundtrips: address_inspect.recent_roundtrips,
        wallet_summary: wallet.summary.clone(),
        sample_entries: wallet.entries.iter().take(sample_limit).cloned().collect(),
        sample_roundtrips: wallet
            .roundtrips
            .iter()
            .take(sample_limit)
            .cloned()
            .collect(),
        clone_report,
        explain_why,
        suggest_next_experiment,
    }
}

pub fn mint_shard_summary_output(result: MintShardSummaryResult) -> MintShardSummaryOutput {
    MintShardSummaryOutput {
        address: result.address,
        mint_count: result.mint_count,
        wallet_trade_count: result.wallet_trade_count,
        total_event_count: result.total_event_count,
        shards: result
            .shards
            .into_iter()
            .map(MintShardOutput::from)
            .collect(),
    }
}

pub fn compare_runs_output(result: CompareRunsResult) -> CompareRunsOutput {
    let strategy_diff = strategy_diff_output(&result.left_strategy, &result.right_strategy);
    CompareRunsOutput {
        left_run: result.left_run,
        right_run: result.right_run,
        left_strategy: result.left_strategy,
        right_strategy: result.right_strategy,
        loaded_fills: LoadedCountOutput {
            left: result.loaded_fills.left,
            right: result.loaded_fills.right,
        },
        loaded_position_snapshots: LoadedCountOutput {
            left: result.loaded_position_snapshots.left,
            right: result.loaded_position_snapshots.right,
        },
        deltas: CompareRunsDeltasOutput {
            events: result.deltas.events,
            fills: result.deltas.fills,
            rejections: result.deltas.rejections,
            cash_sol: result.deltas.cash_sol,
            equity_sol: result.deltas.equity_sol,
        },
        strategy_diff,
    }
}

pub fn clone_eval_output(result: CloneEvalResult) -> CloneEvalOutput {
    CloneEvalOutput {
        address: result.wallet.address.clone(),
        wallet_entries: result.wallet.summary.entry_count,
        wallet_roundtrips: result.wallet.summary.roundtrip_count,
        wallet_closed_roundtrips: result.wallet.summary.closed_roundtrip_count,
        strategy: result.resolved_strategy.strategy.clone(),
        strategy_name: result.candidate.report.strategy.name.to_string(),
        eval_source: result.eval_source,
        clone_score: result.candidate.score.overall,
        f1: result.candidate.score.f1,
        precision: result.candidate.score.precision,
        recall: result.candidate.score.recall,
        matched_entries: result.candidate.score.matched_entries,
        strategy_entries: result.candidate.score.strategy_entries,
        entry_delay_secs: result.candidate.score.avg_entry_delay_secs,
        hold_error_secs: result.candidate.score.avg_hold_error_secs,
        size_error_ratio: result.candidate.score.avg_size_error_ratio,
        count_alignment: result.candidate.score.count_alignment,
        breakdown: clone_score_breakdown_output(&result.candidate.score.breakdown),
        fills: result.candidate.report.fills,
        rejections: result.candidate.report.rejections,
        ending_equity_lamports: result.candidate.report.ending_equity_lamports,
        ending_cash_lamports: result.candidate.report.ending_cash_lamports,
        resolved_strategy: result.resolved_strategy,
        recorded_evaluation_id: result.recorded_evaluation_id,
    }
}

pub fn infer_strategy_output(result: InferStrategyResult) -> InferStrategyOutput {
    InferStrategyOutput {
        address: result.wallet.address,
        wallet_summary: result.wallet.summary,
        candidates: result
            .candidates
            .into_iter()
            .map(InferStrategyCandidate::from)
            .collect(),
    }
}

pub fn fit_params_output(result: FitParamsResult, top: usize) -> FitParamsOutput {
    FitParamsOutput {
        address: result.wallet.address,
        family: result.family,
        wallet_summary: result.wallet.summary,
        candidate_count: result.fit.candidates.len(),
        top_candidates: result
            .fit
            .candidates
            .into_iter()
            .take(top)
            .map(FitParamsCandidate::from)
            .collect(),
    }
}

pub fn clone_rank_output(result: CloneRankResult, top: usize) -> CloneRankOutput {
    CloneRankOutput {
        address: result.wallet.address,
        wallet_entries: result.wallet.summary.entry_count,
        wallet_roundtrips: result.wallet.summary.roundtrip_count,
        ranked: result
            .ranked
            .into_iter()
            .take(top)
            .map(CloneRankRow::from)
            .collect(),
    }
}

pub fn sweep_db_output(result: SweepDbResult, top: usize) -> SweepDbOutput {
    SweepDbOutput {
        strategy: result.strategy,
        combinations: result.combinations,
        sweep_batch_id: result.sweep_batch_id,
        recorded_evaluation_ids: result.recorded_evaluation_ids,
        top_results: result
            .summaries
            .into_iter()
            .take(top)
            .map(SweepResultRow::from)
            .collect(),
    }
}

pub fn task_run_output(row: TaskRunRow) -> TaskRunOutput {
    TaskRunOutput {
        task_id: row.task_id,
        task_kind: row.task_kind,
        status: row.status,
        idempotency_key: row.idempotency_key,
        cancellation_requested: row.cancellation_requested,
        request_payload: row.request_payload,
        result_payload: row.result_payload,
        error_payload: row.error_payload,
        submitted_at: row.submitted_at,
        started_at: row.started_at,
        finished_at: row.finished_at,
    }
}

pub fn experiment_output(row: ExperimentRow) -> ExperimentOutput {
    ExperimentOutput {
        experiment_id: row.experiment_id,
        title: row.title,
        target_wallet: row.target_wallet,
        status: row.status,
        thesis: row.thesis,
        notes: row.notes,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub fn hypothesis_output(row: HypothesisRow) -> HypothesisOutput {
    HypothesisOutput {
        hypothesis_id: row.hypothesis_id,
        experiment_id: row.experiment_id,
        family: row.family,
        description: row.description,
        status: row.status,
        strategy_config: row.strategy_config,
        sample_window: row.sample_window,
        notes: row.notes,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

pub fn evaluation_output(row: EvaluationSummary) -> EvaluationOutput {
    EvaluationOutput {
        evaluation_id: row.evaluation_id,
        experiment_id: row.experiment_id,
        hypothesis_id: row.hypothesis_id,
        strategy_run_id: row.strategy_run_id,
        task_id: row.task_id,
        target_wallet: row.target_wallet,
        family: row.family,
        strategy_name: row.strategy_name,
        source_type: row.source_type,
        source_ref: row.source_ref,
        score_overall: row.score_overall,
        score_breakdown: row.score_breakdown,
        metrics: row.metrics,
        failure_tags: row.failure_tags,
        artifact_paths: row.artifact_paths,
        notes: row.notes,
        conclusion: row.conclusion,
        created_at: row.created_at,
    }
}

pub fn experiment_detail_output(result: ExperimentDetailResult) -> ExperimentDetailOutput {
    ExperimentDetailOutput {
        experiment: experiment_output(result.experiment),
        hypotheses: result
            .hypotheses
            .into_iter()
            .map(hypothesis_output)
            .collect(),
        evaluations: result
            .evaluations
            .into_iter()
            .map(evaluation_output)
            .collect(),
    }
}

impl From<StrategyCloneCandidate> for InferStrategyCandidate {
    fn from(candidate: StrategyCloneCandidate) -> Self {
        Self {
            family: candidate.args.strategy,
            strategy_name: candidate.report.strategy.name.to_string(),
            clone_score: candidate.score.overall,
            f1: candidate.score.f1,
            precision: candidate.score.precision,
            recall: candidate.score.recall,
            matched_entries: candidate.score.matched_entries,
            wallet_entries: candidate.score.wallet_entries,
            strategy_entries: candidate.score.strategy_entries,
            entry_delay_secs: candidate.score.avg_entry_delay_secs,
            hold_error_secs: candidate.score.avg_hold_error_secs,
            size_error_ratio: candidate.score.avg_size_error_ratio,
            count_alignment: candidate.score.count_alignment,
            breakdown: clone_score_breakdown_output(&candidate.score.breakdown),
            fills: candidate.report.fills,
            ending_equity_lamports: candidate.report.ending_equity_lamports,
        }
    }
}

impl From<StrategyCloneCandidate> for FitParamsCandidate {
    fn from(candidate: StrategyCloneCandidate) -> Self {
        Self {
            args: candidate.args,
            strategy_name: candidate.report.strategy.name.to_string(),
            clone_score: candidate.score.overall,
            f1: candidate.score.f1,
            precision: candidate.score.precision,
            recall: candidate.score.recall,
            matched_entries: candidate.score.matched_entries,
            strategy_entries: candidate.score.strategy_entries,
            entry_delay_secs: candidate.score.avg_entry_delay_secs,
            hold_error_secs: candidate.score.avg_hold_error_secs,
            size_error_ratio: candidate.score.avg_size_error_ratio,
            count_alignment: candidate.score.count_alignment,
            breakdown: clone_score_breakdown_output(&candidate.score.breakdown),
            fills: candidate.report.fills,
            rejections: candidate.report.rejections,
            ending_equity_lamports: candidate.report.ending_equity_lamports,
        }
    }
}

impl From<crate::usecases::CloneRankedRun> for CloneRankRow {
    fn from(value: crate::usecases::CloneRankedRun) -> Self {
        Self {
            run_id: value.run_id,
            strategy: value.strategy.strategy,
            strategy_name: value.strategy_name,
            run_mode: value.run_mode,
            source_type: value.source_type,
            source_ref: value.source_ref,
            started_at: value.started_at,
            stored_equity_lamports: value.stored_equity_lamports,
            clone_score: value.candidate.score.overall,
            f1: value.candidate.score.f1,
            precision: value.candidate.score.precision,
            recall: value.candidate.score.recall,
            matched_entries: value.candidate.score.matched_entries,
            strategy_entries: value.candidate.score.strategy_entries,
            entry_delay_secs: value.candidate.score.avg_entry_delay_secs,
            hold_error_secs: value.candidate.score.avg_hold_error_secs,
            size_error_ratio: value.candidate.score.avg_size_error_ratio,
            count_alignment: value.candidate.score.count_alignment,
            breakdown: clone_score_breakdown_output(&value.candidate.score.breakdown),
        }
    }
}

pub fn clone_score_breakdown_output(breakdown: &CloneScoreBreakdown) -> CloneScoreBreakdownOutput {
    CloneScoreBreakdownOutput {
        entry_timing_similarity: breakdown.entry_timing_similarity,
        hold_time_similarity: breakdown.hold_time_similarity,
        size_profile_similarity: breakdown.size_profile_similarity,
        token_selection_similarity: breakdown.token_selection_similarity,
        exit_behavior_similarity: breakdown.exit_behavior_similarity,
        count_alignment: breakdown.count_alignment,
    }
}

impl From<SweepRunSummary> for SweepResultRow {
    fn from(summary: SweepRunSummary) -> Self {
        Self {
            run_id: summary.run_id,
            strategy_name: summary.report.strategy.name,
            ending_equity_sol: lamports_to_sol(summary.report.ending_equity_lamports),
            ending_cash_sol: lamports_to_sol(summary.report.ending_cash_lamports),
            fills: summary.report.fills,
            rejections: summary.report.rejections,
            open_positions: summary.report.open_positions,
            buy_sol: summary.strategy.buy_sol,
            max_age_secs: summary.strategy.max_age_secs,
            min_total_buy_sol: summary.strategy.min_total_buy_sol,
            max_sell_count: summary.strategy.max_sell_count,
            min_buy_sell_ratio: summary.strategy.min_buy_sell_ratio,
            max_concurrent_positions: summary.strategy.max_concurrent_positions,
            exit_on_sell_count: summary.strategy.exit_on_sell_count,
        }
    }
}

fn round_to(value: f64, decimals: u32) -> f64 {
    let factor = 10_f64.powi(decimals as i32);
    (value * factor).round() / factor
}

fn lamports_to_sol(lamports: u64) -> f64 {
    lamports as f64 / 1_000_000_000.0
}

fn explanation_confidence(best: f64, runner_up: f64) -> &'static str {
    let gap = best - runner_up;
    if best >= 0.65 && gap >= 0.08 {
        "strong"
    } else if best >= 0.5 && gap >= 0.03 {
        "moderate"
    } else {
        "weak"
    }
}

fn strongest_dimensions<'a>(
    best_family: &'a StrategyCloneCandidate,
    runner_up: &'a StrategyCloneCandidate,
) -> Vec<(&'static str, f64, f64)> {
    let mut deltas = breakdown_dimensions(&best_family.score.breakdown)
        .into_iter()
        .zip(breakdown_dimensions(&runner_up.score.breakdown))
        .map(|((label, best), (_, runner))| (label, best, best - runner))
        .collect::<Vec<_>>();
    deltas.sort_by(|left, right| right.2.total_cmp(&left.2));
    deltas
        .into_iter()
        .filter(|(_, _, delta)| *delta > 0.02)
        .take(3)
        .collect()
}

fn weakest_dimensions(candidate: &StrategyCloneCandidate) -> Vec<(&'static str, f64)> {
    let mut values = breakdown_dimensions(&candidate.score.breakdown);
    values.sort_by(|left, right| left.1.total_cmp(&right.1));
    values
        .into_iter()
        .filter(|(_, score)| *score < 0.65)
        .take(3)
        .collect()
}

fn explanation_warnings(
    wallet: &WalletBehaviorReport,
    best_family: &StrategyCloneCandidate,
    runner_up: &StrategyCloneCandidate,
    family_gap: f64,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if family_gap < 0.03 {
        warnings.push(format!(
            "family choice is close: {} and {} are separated by only {:.4} clone_score",
            best_family.args.strategy, runner_up.args.strategy, family_gap
        ));
    }
    if best_family.score.matched_entries < 3 {
        warnings.push(
            "matched entry count is small, so the explanation is directional rather than robust"
                .to_string(),
        );
    }
    if best_family.score.breakdown.token_selection_similarity < 0.35 {
        warnings.push(
            "token selection overlap is weak; the current family may capture timing but miss which mints this wallet prefers"
                .to_string(),
        );
    }
    if best_family.score.f1 < 0.4 {
        warnings.push(
            "overall fit is still partial; treat this as a base family guess, not a finished strategy"
                .to_string(),
        );
    }
    if wallet.summary.closed_roundtrip_count < 5 {
        warnings.push(
            "closed roundtrip sample is small, so hold-time and exit conclusions may be noisy"
                .to_string(),
        );
    }
    warnings
}

fn explanation_next_actions(
    wallet: &WalletBehaviorReport,
    best_family: &StrategyCloneCandidate,
    runner_up: &StrategyCloneCandidate,
    weakest: &[(&'static str, f64)],
) -> Vec<String> {
    let mut actions = Vec::new();
    for (label, _) in weakest {
        match *label {
            "entry timing" => actions.push(format!(
                "tighten the entry gate for {}: retune max_age_secs, min_buy_count, min_unique_buyers, min_total_buy_sol, and min_net_buy_sol",
                best_family.args.strategy
            )),
            "hold time" => actions.push(format!(
                "retune exit cadence for {}: max_hold_secs, take_profit_bps, and stop_loss_bps are the next levers",
                best_family.args.strategy
            )),
            "size profile" => actions.push(format!(
                "re-center buy_sol around the wallet's average ticket size ({:.3} SOL)",
                wallet.summary.avg_entry_buy_sol.unwrap_or(best_family.args.buy_sol)
            )),
            "token selection" => actions.push(format!(
                "token overlap is the main miss; compare {} against {} and consider adding a mint-selection filter",
                best_family.args.strategy, runner_up.args.strategy
            )),
            "exit behavior" => actions.push(format!(
                "sell-pressure exits need work; tune exit_on_sell_count and max_sell_count for {}",
                best_family.args.strategy
            )),
            "count alignment" => actions.push(format!(
                "entry frequency is off; adjust selectivity and max_concurrent_positions for {}",
                best_family.args.strategy
            )),
            _ => {}
        }
    }

    if actions.is_empty() {
        actions.push(format!(
            "run fit-params on {} to convert this family-level explanation into concrete thresholds",
            best_family.args.strategy
        ));
    }

    if best_family.score.overall < 0.55 {
        actions.push(format!(
            "keep {} as the base family, but test {} as a fallback branch because the current fit is still moderate",
            best_family.args.strategy, runner_up.args.strategy
        ));
    }

    actions
}

fn breakdown_dimensions(breakdown: &CloneScoreBreakdown) -> Vec<(&'static str, f64)> {
    vec![
        ("entry timing", breakdown.entry_timing_similarity),
        ("hold time", breakdown.hold_time_similarity),
        ("size profile", breakdown.size_profile_similarity),
        ("token selection", breakdown.token_selection_similarity),
        ("exit behavior", breakdown.exit_behavior_similarity),
        ("count alignment", breakdown.count_alignment),
    ]
}

fn strategy_from_seed(family: &str, seed: &ParamsSeed) -> StrategyConfig {
    StrategyConfig {
        strategy: family.replace('-', "_"),
        strategy_config: None,
        starting_sol: 10.0,
        buy_sol: seed.buy_sol,
        max_age_secs: seed.max_age_secs,
        min_buy_count: seed.min_buy_count,
        min_unique_buyers: seed.min_unique_buyers,
        min_net_buy_sol: seed.min_net_buy_sol,
        take_profit_bps: seed.take_profit_bps,
        stop_loss_bps: seed.stop_loss_bps,
        max_hold_secs: seed.max_hold_secs,
        min_total_buy_sol: seed.min_total_buy_sol,
        max_sell_count: seed.max_sell_count,
        min_buy_sell_ratio: seed.min_buy_sell_ratio,
        max_concurrent_positions: seed.max_concurrent_positions,
        exit_on_sell_count: seed.exit_on_sell_count,
        trading_fee_bps: 100,
        slippage_bps: 50,
    }
}

fn sweep_from_seed(seed: &ParamsSeed, weakest: &[(&'static str, f64)]) -> SweepConfig {
    let mut sweep = SweepConfig {
        buy_sol_values: Some(format!(
            "{:.3},{:.3},{:.3}",
            (seed.buy_sol * 0.8).max(0.01),
            seed.buy_sol,
            seed.buy_sol * 1.2
        )),
        max_age_secs_values: Some(format!(
            "{},{},{}",
            (seed.max_age_secs - 8).max(3),
            seed.max_age_secs,
            seed.max_age_secs + 8
        )),
        min_buy_count_values: Some(format!(
            "{},{},{}",
            seed.min_buy_count.saturating_sub(1).max(1),
            seed.min_buy_count,
            seed.min_buy_count + 1
        )),
        min_unique_buyers_values: Some(format!(
            "{},{},{}",
            seed.min_unique_buyers.saturating_sub(1).max(1),
            seed.min_unique_buyers,
            seed.min_unique_buyers + 1
        )),
        min_total_buy_sol_values: Some(format!(
            "{:.3},{:.3},{:.3}",
            (seed.min_total_buy_sol * 0.8).max(0.1),
            seed.min_total_buy_sol,
            seed.min_total_buy_sol * 1.2
        )),
        max_sell_count_values: Some(format!(
            "{},{},{}",
            seed.max_sell_count.saturating_sub(1),
            seed.max_sell_count,
            seed.max_sell_count + 1
        )),
        min_buy_sell_ratio_values: Some(format!(
            "{:.2},{:.2},{:.2}",
            (seed.min_buy_sell_ratio - 0.75).max(1.0),
            seed.min_buy_sell_ratio,
            seed.min_buy_sell_ratio + 0.75
        )),
        take_profit_bps_values: Some(format!(
            "{},{},{}",
            (seed.take_profit_bps - 300).max(300),
            seed.take_profit_bps,
            seed.take_profit_bps + 300
        )),
        stop_loss_bps_values: Some(format!(
            "{},{},{}",
            (seed.stop_loss_bps - 200).max(100),
            seed.stop_loss_bps,
            seed.stop_loss_bps + 200
        )),
        max_concurrent_positions_values: Some(format!(
            "{},{},{}",
            seed.max_concurrent_positions.saturating_sub(1).max(1),
            seed.max_concurrent_positions,
            seed.max_concurrent_positions + 1
        )),
        exit_on_sell_count_values: Some(format!(
            "{},{},{}",
            seed.exit_on_sell_count.saturating_sub(1).max(1),
            seed.exit_on_sell_count,
            seed.exit_on_sell_count + 1
        )),
    };

    if weakest.iter().any(|(label, _)| *label == "entry timing") {
        sweep.max_age_secs_values = Some(format!(
            "{},{},{}",
            (seed.max_age_secs - 12).max(3),
            seed.max_age_secs,
            seed.max_age_secs + 12
        ));
        sweep.min_buy_count_values = Some(format!(
            "{},{},{}",
            seed.min_buy_count.saturating_sub(2).max(1),
            seed.min_buy_count,
            seed.min_buy_count + 2
        ));
    }
    if weakest
        .iter()
        .any(|(label, _)| *label == "hold time" || *label == "exit behavior")
    {
        sweep.take_profit_bps_values = Some(format!(
            "{},{},{}",
            (seed.take_profit_bps - 500).max(300),
            seed.take_profit_bps,
            seed.take_profit_bps + 500
        ));
        sweep.exit_on_sell_count_values = Some(format!(
            "{},{},{}",
            seed.exit_on_sell_count.saturating_sub(2).max(1),
            seed.exit_on_sell_count,
            seed.exit_on_sell_count + 2
        ));
    }
    sweep
}

fn targeted_sweep_from_seed(seed: &ParamsSeed, weakest: &[(&'static str, f64)]) -> SweepConfig {
    let mut sweep = SweepConfig::default();
    for (label, _) in weakest.iter().take(2) {
        match *label {
            "entry timing" => {
                sweep.max_age_secs_values = Some(format!(
                    "{},{},{}",
                    (seed.max_age_secs - 10).max(3),
                    seed.max_age_secs,
                    seed.max_age_secs + 10
                ));
                sweep.min_buy_count_values = Some(format!(
                    "{},{},{}",
                    seed.min_buy_count.saturating_sub(1).max(1),
                    seed.min_buy_count,
                    seed.min_buy_count + 1
                ));
                sweep.min_unique_buyers_values = Some(format!(
                    "{},{},{}",
                    seed.min_unique_buyers.saturating_sub(1).max(1),
                    seed.min_unique_buyers,
                    seed.min_unique_buyers + 1
                ));
            }
            "hold time" | "exit behavior" => {
                sweep.take_profit_bps_values = Some(format!(
                    "{},{},{}",
                    (seed.take_profit_bps - 400).max(300),
                    seed.take_profit_bps,
                    seed.take_profit_bps + 400
                ));
                sweep.stop_loss_bps_values = Some(format!(
                    "{},{},{}",
                    (seed.stop_loss_bps - 200).max(100),
                    seed.stop_loss_bps,
                    seed.stop_loss_bps + 200
                ));
                sweep.exit_on_sell_count_values = Some(format!(
                    "{},{},{}",
                    seed.exit_on_sell_count.saturating_sub(1).max(1),
                    seed.exit_on_sell_count,
                    seed.exit_on_sell_count + 1
                ));
            }
            "size profile" => {
                sweep.buy_sol_values = Some(format!(
                    "{:.3},{:.3},{:.3}",
                    (seed.buy_sol * 0.85).max(0.01),
                    seed.buy_sol,
                    seed.buy_sol * 1.15
                ));
            }
            "count alignment" => {
                sweep.max_concurrent_positions_values = Some(format!(
                    "{},{},{}",
                    seed.max_concurrent_positions.saturating_sub(1).max(1),
                    seed.max_concurrent_positions,
                    seed.max_concurrent_positions + 1
                ));
                sweep.min_buy_count_values = Some(format!(
                    "{},{},{}",
                    seed.min_buy_count.saturating_sub(1).max(1),
                    seed.min_buy_count,
                    seed.min_buy_count + 1
                ));
            }
            "token selection" => {
                sweep.min_total_buy_sol_values = Some(format!(
                    "{:.3},{:.3},{:.3}",
                    (seed.min_total_buy_sol * 0.8).max(0.1),
                    seed.min_total_buy_sol,
                    seed.min_total_buy_sol * 1.2
                ));
                sweep.min_buy_sell_ratio_values = Some(format!(
                    "{:.2},{:.2},{:.2}",
                    (seed.min_buy_sell_ratio - 0.5).max(1.0),
                    seed.min_buy_sell_ratio,
                    seed.min_buy_sell_ratio + 0.5
                ));
            }
            _ => {}
        }
    }

    if sweep.buy_sol_values.is_none() {
        sweep.buy_sol_values = Some(format!(
            "{:.3},{:.3},{:.3}",
            (seed.buy_sol * 0.9).max(0.01),
            seed.buy_sol,
            seed.buy_sol * 1.1
        ));
    }
    sweep
}

fn targeted_rationale(weakest: &[(&'static str, f64)]) -> String {
    if weakest.is_empty() {
        return "the current family fit is coherent, so the next step is a narrow confidence-building sweep".to_string();
    }
    format!(
        "the next experiment should focus on {}",
        weakest
            .iter()
            .take(2)
            .map(|(label, _)| *label)
            .collect::<Vec<_>>()
            .join(" and ")
    )
}

fn experiment_tested_families(detail: &ExperimentDetailResult) -> Vec<String> {
    let mut families = detail
        .hypotheses
        .iter()
        .map(|hypothesis| hypothesis.family.clone())
        .chain(
            detail
                .evaluations
                .iter()
                .filter_map(|evaluation| evaluation.family.clone()),
        )
        .collect::<Vec<_>>();
    families.sort();
    families.dedup();
    families
}

impl From<crate::usecases::MintShardRow> for MintShardOutput {
    fn from(value: crate::usecases::MintShardRow) -> Self {
        Self {
            mint: value.mint,
            symbol: value.symbol,
            creator: value.creator,
            event_count: value.event_count,
            trade_count: value.trade_count,
            buy_count: value.buy_count,
            sell_count: value.sell_count,
            unique_trader_count: value.unique_trader_count,
            wallet_trade_count: value.wallet_trade_count,
            wallet_buy_count: value.wallet_buy_count,
            wallet_sell_count: value.wallet_sell_count,
            wallet_entry_count: value.wallet_entry_count,
            wallet_roundtrip_count: value.wallet_roundtrip_count,
            gross_buy_sol: value.gross_buy_sol,
            gross_sell_sol: value.gross_sell_sol,
            net_flow_sol: value.net_flow_sol,
            wallet_gross_buy_sol: value.wallet_gross_buy_sol,
            wallet_gross_sell_sol: value.wallet_gross_sell_sol,
            wallet_net_flow_sol: value.wallet_net_flow_sol,
            first_seen_ts: value.first_seen_ts,
            last_seen_ts: value.last_seen_ts,
            has_create: value.has_create,
            is_complete: value.is_complete,
        }
    }
}

pub fn strategy_diff_output(left: &StrategyConfig, right: &StrategyConfig) -> StrategyDiffOutput {
    let mut changed_fields = Vec::new();
    push_diff_str(
        &mut changed_fields,
        "strategy",
        &left.strategy,
        &right.strategy,
    );
    push_diff_f64(
        &mut changed_fields,
        "starting_sol",
        left.starting_sol,
        right.starting_sol,
    );
    push_diff_f64(&mut changed_fields, "buy_sol", left.buy_sol, right.buy_sol);
    push_diff_i64(
        &mut changed_fields,
        "max_age_secs",
        left.max_age_secs,
        right.max_age_secs,
    );
    push_diff_u64(
        &mut changed_fields,
        "min_buy_count",
        left.min_buy_count,
        right.min_buy_count,
    );
    push_diff_usize(
        &mut changed_fields,
        "min_unique_buyers",
        left.min_unique_buyers,
        right.min_unique_buyers,
    );
    push_diff_f64(
        &mut changed_fields,
        "min_net_buy_sol",
        left.min_net_buy_sol,
        right.min_net_buy_sol,
    );
    push_diff_i64(
        &mut changed_fields,
        "take_profit_bps",
        left.take_profit_bps,
        right.take_profit_bps,
    );
    push_diff_i64(
        &mut changed_fields,
        "stop_loss_bps",
        left.stop_loss_bps,
        right.stop_loss_bps,
    );
    push_diff_i64(
        &mut changed_fields,
        "max_hold_secs",
        left.max_hold_secs,
        right.max_hold_secs,
    );
    push_diff_f64(
        &mut changed_fields,
        "min_total_buy_sol",
        left.min_total_buy_sol,
        right.min_total_buy_sol,
    );
    push_diff_u64(
        &mut changed_fields,
        "max_sell_count",
        left.max_sell_count,
        right.max_sell_count,
    );
    push_diff_f64(
        &mut changed_fields,
        "min_buy_sell_ratio",
        left.min_buy_sell_ratio,
        right.min_buy_sell_ratio,
    );
    push_diff_usize(
        &mut changed_fields,
        "max_concurrent_positions",
        left.max_concurrent_positions,
        right.max_concurrent_positions,
    );
    push_diff_u64(
        &mut changed_fields,
        "exit_on_sell_count",
        left.exit_on_sell_count,
        right.exit_on_sell_count,
    );
    push_diff_u64(
        &mut changed_fields,
        "trading_fee_bps",
        left.trading_fee_bps,
        right.trading_fee_bps,
    );
    push_diff_u64(
        &mut changed_fields,
        "slippage_bps",
        left.slippage_bps,
        right.slippage_bps,
    );

    StrategyDiffOutput {
        family_changed: left.strategy != right.strategy,
        changed_field_count: changed_fields.len(),
        changed_fields,
    }
}

fn push_diff_str(
    output: &mut Vec<StrategyFieldDiffOutput>,
    field: &'static str,
    left: &str,
    right: &str,
) {
    if left != right {
        output.push(StrategyFieldDiffOutput {
            field,
            left: serde_json::Value::String(left.to_string()),
            right: serde_json::Value::String(right.to_string()),
            numeric_delta: None,
        });
    }
}

fn push_diff_f64(
    output: &mut Vec<StrategyFieldDiffOutput>,
    field: &'static str,
    left: f64,
    right: f64,
) {
    if (left - right).abs() > f64::EPSILON {
        output.push(StrategyFieldDiffOutput {
            field,
            left: serde_json::json!(left),
            right: serde_json::json!(right),
            numeric_delta: Some(round_to(right - left, 6)),
        });
    }
}

fn push_diff_i64(
    output: &mut Vec<StrategyFieldDiffOutput>,
    field: &'static str,
    left: i64,
    right: i64,
) {
    if left != right {
        output.push(StrategyFieldDiffOutput {
            field,
            left: serde_json::json!(left),
            right: serde_json::json!(right),
            numeric_delta: Some((right - left) as f64),
        });
    }
}

fn push_diff_u64(
    output: &mut Vec<StrategyFieldDiffOutput>,
    field: &'static str,
    left: u64,
    right: u64,
) {
    if left != right {
        output.push(StrategyFieldDiffOutput {
            field,
            left: serde_json::json!(left),
            right: serde_json::json!(right),
            numeric_delta: Some(right as f64 - left as f64),
        });
    }
}

fn push_diff_usize(
    output: &mut Vec<StrategyFieldDiffOutput>,
    field: &'static str,
    left: usize,
    right: usize,
) {
    if left != right {
        output.push(StrategyFieldDiffOutput {
            field,
            left: serde_json::json!(left),
            right: serde_json::json!(right),
            numeric_delta: Some(right as f64 - left as f64),
        });
    }
}
