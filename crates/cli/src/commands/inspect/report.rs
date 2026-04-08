use anyhow::Result;
use pump_agent_app::{
    api::{
        CloneReportExportSummary, build_clone_report, clone_explain_why_output,
        mint_shard_summary_output, suggest_next_experiment_output, wallet_dossier_output,
    },
    clone::{
        default_strategy_config_for_family, extract_wallet_behavior, score_strategy_execution,
    },
    usecases::{
        AddressInspectRequest, CloneAnalysisRequest, DatabaseRequest, InspectExperimentRequest,
        address_inspect, analyze_clone_candidates as analyze_clone_candidates_usecase,
        inspect_experiment, summarize_mint_shards,
    },
};
use pump_agent_core::PgEventStore;
use serde_json::json;
use std::time::Instant;

use crate::{
    args::{
        CloneReportArgs, ExplainWhyArgs, MintShardSummaryArgs, OutputFormat,
        SuggestNextExperimentArgs, WalletDossierArgs,
    },
    config::required_config,
    output::{CommandResult, emit_json_success, wants_json},
};

use super::export::{AddressExportSummary, export_address_events};
use crate::commands::helpers::SCHEMA_SQL;

pub async fn clone_report(args: CloneReportArgs, format: OutputFormat) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let analysis = analyze_clone_candidates_usecase(CloneAnalysisRequest {
        database: DatabaseRequest {
            database_url: database_url.clone(),
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        address: args.address.clone(),
    })
    .await?;

    let export_summary = if args.export {
        let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
        store.apply_schema(SCHEMA_SQL).await?;
        Some(export_address_events(&store, &args.address, &args.export_root).await?)
    } else {
        None
    };

    let report = build_clone_report(
        &analysis.wallet,
        &analysis.best_family,
        &analysis.runner_up,
        export_summary.as_ref().map(CloneReportExportSummary::from),
    );

    if wants_json(format, args.json) {
        emit_json_success("clone_report", &report, started)?;
    } else {
        println!("address                  : {}", report.address);
        println!(
            "recommended base family  : {}",
            report.recommended_base_family
        );
        println!(
            "recommended strategy     : {}",
            report.recommended_next_strategy_name
        );
        println!(
            "base fit                 : clone_score={:.4} f1={:.4} runner_up={}({:.4})",
            report.base_fit.clone_score,
            report.base_fit.f1,
            report.runner_up.family,
            report.runner_up.clone_score
        );
        println!(
            "base breakdown           : entry={:.3} hold={:.3} size={:.3} token={:.3} exit={:.3} count={:.3}",
            report.base_fit.breakdown.entry_timing_similarity,
            report.base_fit.breakdown.hold_time_similarity,
            report.base_fit.breakdown.size_profile_similarity,
            report.base_fit.breakdown.token_selection_similarity,
            report.base_fit.breakdown.exit_behavior_similarity,
            report.base_fit.breakdown.count_alignment,
        );
        println!(
            "runner breakdown         : entry={:.3} hold={:.3} size={:.3} token={:.3} exit={:.3} count={:.3}",
            report.runner_up.breakdown.entry_timing_similarity,
            report.runner_up.breakdown.hold_time_similarity,
            report.runner_up.breakdown.size_profile_similarity,
            report.runner_up.breakdown.token_selection_similarity,
            report.runner_up.breakdown.exit_behavior_similarity,
            report.runner_up.breakdown.count_alignment,
        );
        println!("confirmed rules:");
        for line in &report.confirmed_rules {
            println!("  - {}", line);
        }
        println!("tentative rules:");
        for line in &report.tentative_rules {
            println!("  - {}", line);
        }
        println!("anti patterns:");
        for line in &report.anti_patterns {
            println!("  - {}", line);
        }
        println!("params seed:");
        println!(
            "  buy_sol={:.6} max_age_secs={} min_buy_count={} min_unique_buyers={} min_net_buy_sol={:.6} min_total_buy_sol={:.6} max_sell_count={} min_buy_sell_ratio={:.2} max_hold_secs={} max_concurrent_positions={} exit_on_sell_count={} take_profit_bps={} stop_loss_bps={}",
            report.recommended_params_seed.buy_sol,
            report.recommended_params_seed.max_age_secs,
            report.recommended_params_seed.min_buy_count,
            report.recommended_params_seed.min_unique_buyers,
            report.recommended_params_seed.min_net_buy_sol,
            report.recommended_params_seed.min_total_buy_sol,
            report.recommended_params_seed.max_sell_count,
            report.recommended_params_seed.min_buy_sell_ratio,
            report.recommended_params_seed.max_hold_secs,
            report.recommended_params_seed.max_concurrent_positions,
            report.recommended_params_seed.exit_on_sell_count,
            report.recommended_params_seed.take_profit_bps,
            report.recommended_params_seed.stop_loss_bps
        );

        if let Some(export) = &report.export {
            println!(
                "export dir               : {}",
                export.address_dir.as_deref().unwrap_or("-")
            );
            println!(
                "export index             : {}",
                export.index_path.as_deref().unwrap_or("-")
            );
        }
    }

    Ok(())
}

pub async fn explain_why(args: ExplainWhyArgs, format: OutputFormat) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let analysis = analyze_clone_candidates_usecase(CloneAnalysisRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        address: args.address.clone(),
    })
    .await?;

    let output =
        clone_explain_why_output(&analysis.wallet, &analysis.best_family, &analysis.runner_up);
    if format.is_json() {
        emit_json_success("explain_why", &output, started)?;
    } else {
        println!("address            : {}", output.address);
        println!("recommended family : {}", output.recommended_family);
        println!("runner-up family   : {}", output.runner_up_family);
        println!("confidence         : {}", output.confidence);
        println!("family gap         : {:.4}", output.family_gap);
        println!("decision           : {}", output.decision_summary);
        println!(
            "wallet shape       : entries={} roundtrips={} closed={} orphan_sells={}",
            output.wallet_summary.entry_count,
            output.wallet_summary.roundtrip_count,
            output.wallet_summary.closed_roundtrip_count,
            output.wallet_summary.orphan_sell_count
        );
        println!(
            "base breakdown     : entry={:.3} hold={:.3} size={:.3} token={:.3} exit={:.3} count={:.3}",
            output.base_breakdown.entry_timing_similarity,
            output.base_breakdown.hold_time_similarity,
            output.base_breakdown.size_profile_similarity,
            output.base_breakdown.token_selection_similarity,
            output.base_breakdown.exit_behavior_similarity,
            output.base_breakdown.count_alignment,
        );
        println!(
            "runner breakdown   : entry={:.3} hold={:.3} size={:.3} token={:.3} exit={:.3} count={:.3}",
            output.runner_up_breakdown.entry_timing_similarity,
            output.runner_up_breakdown.hold_time_similarity,
            output.runner_up_breakdown.size_profile_similarity,
            output.runner_up_breakdown.token_selection_similarity,
            output.runner_up_breakdown.exit_behavior_similarity,
            output.runner_up_breakdown.count_alignment,
        );
        println!("strengths:");
        for line in &output.strengths {
            println!("  - {}", line);
        }
        println!("weaknesses:");
        for line in &output.weaknesses {
            println!("  - {}", line);
        }
        println!("warnings:");
        for line in &output.warnings {
            println!("  - {}", line);
        }
        println!("next actions:");
        for line in &output.next_actions {
            println!("  - {}", line);
        }
    }

    Ok(())
}

pub async fn suggest_next_experiment(
    args: SuggestNextExperimentArgs,
    format: OutputFormat,
) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let analysis = analyze_clone_candidates_usecase(CloneAnalysisRequest {
        database: DatabaseRequest {
            database_url: database_url.clone(),
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        address: args.address.clone(),
    })
    .await?;

    let experiment = if let Some(experiment_id) = args.experiment_id.as_deref() {
        let detail = inspect_experiment(InspectExperimentRequest {
            database: DatabaseRequest {
                database_url,
                max_db_connections: args.max_db_connections,
                apply_schema: true,
            },
            experiment_id: experiment_id.to_string(),
        })
        .await?;
        let Some(detail) = detail else {
            return Err(crate::output::CommandError::not_found(format!(
                "experiment not found: {}",
                experiment_id
            ))
            .with_details(json!({
                "resource": "experiment",
                "id": experiment_id,
            })));
        };
        Some(detail)
    } else {
        None
    };

    let output = suggest_next_experiment_output(
        &analysis.wallet,
        &analysis.best_family,
        &analysis.runner_up,
        experiment.as_ref(),
    );
    if format.is_json() {
        emit_json_success("suggest_next_experiment", &output, started)?;
    } else {
        println!("address            : {}", output.address);
        println!("recommended family : {}", output.recommended_family);
        println!("confidence         : {}", output.confidence);
        if let Some(experiment_id) = &output.experiment_id {
            println!("experiment         : {}", experiment_id);
        }
        if let Some(history) = &output.history_summary {
            println!("history            : {}", history);
        }
        if !output.skipped_families.is_empty() {
            println!(
                "skipped families   : {}",
                output.skipped_families.join(", ")
            );
        }
        println!("proposals:");
        for proposal in &output.proposals {
            println!(
                "  - [{}] {} | family={} | objective={}",
                proposal.priority, proposal.title, proposal.family, proposal.objective
            );
            println!("    rationale: {}", proposal.rationale);
            println!("    learning : {}", proposal.expected_learning);
            println!(
                "    strategy : buy_sol={} max_age_secs={} min_buy_count={} min_unique_buyers={} min_net_buy_sol={} min_total_buy_sol={} max_sell_count={} ratio={} tp={} sl={} concurrent={} exit_on_sell_count={}",
                proposal.strategy.buy_sol,
                proposal.strategy.max_age_secs,
                proposal.strategy.min_buy_count,
                proposal.strategy.min_unique_buyers,
                proposal.strategy.min_net_buy_sol,
                proposal.strategy.min_total_buy_sol,
                proposal.strategy.max_sell_count,
                proposal.strategy.min_buy_sell_ratio,
                proposal.strategy.take_profit_bps,
                proposal.strategy.stop_loss_bps,
                proposal.strategy.max_concurrent_positions,
                proposal.strategy.exit_on_sell_count,
            );
            println!(
                "    sweep    : buy_sol={:?} max_age={:?} min_buy_count={:?} min_unique_buyers={:?} min_total_buy_sol={:?} max_sell_count={:?} ratio={:?} tp={:?} sl={:?} concurrent={:?} exit_on_sell_count={:?}",
                proposal.sweep.buy_sol_values,
                proposal.sweep.max_age_secs_values,
                proposal.sweep.min_buy_count_values,
                proposal.sweep.min_unique_buyers_values,
                proposal.sweep.min_total_buy_sol_values,
                proposal.sweep.max_sell_count_values,
                proposal.sweep.min_buy_sell_ratio_values,
                proposal.sweep.take_profit_bps_values,
                proposal.sweep.stop_loss_bps_values,
                proposal.sweep.max_concurrent_positions_values,
                proposal.sweep.exit_on_sell_count_values,
            );
        }
    }

    Ok(())
}

pub async fn wallet_dossier(args: WalletDossierArgs, format: OutputFormat) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let database = DatabaseRequest {
        database_url: database_url.clone(),
        max_db_connections: args.max_db_connections,
        apply_schema: true,
    };
    let analysis = analyze_clone_candidates_usecase(CloneAnalysisRequest {
        database: database.clone(),
        address: args.address.clone(),
    })
    .await?;
    let inspect = address_inspect(AddressInspectRequest {
        database: database.clone(),
        address: args.address.clone(),
        top_mints_limit: args.top_mints_limit,
        roundtrip_limit: args.roundtrip_limit,
    })
    .await?;
    let experiment = if let Some(experiment_id) = args.experiment_id.as_deref() {
        let detail = inspect_experiment(InspectExperimentRequest {
            database,
            experiment_id: experiment_id.to_string(),
        })
        .await?;
        let Some(detail) = detail else {
            return Err(crate::output::CommandError::not_found(format!(
                "experiment not found: {}",
                experiment_id
            ))
            .with_details(json!({
                "resource": "experiment",
                "id": experiment_id,
            })));
        };
        Some(detail)
    } else {
        None
    };
    let output = wallet_dossier_output(
        inspect,
        &analysis.wallet,
        &analysis.best_family,
        &analysis.runner_up,
        experiment.as_ref(),
        args.sample_limit,
    );

    if format.is_json() {
        emit_json_success("wallet_dossier", &output, started)?;
    } else {
        println!("address            : {}", output.address);
        if let Some(experiment_id) = &output.experiment_id {
            println!("experiment         : {}", experiment_id);
        }
        println!(
            "overview           : trades={} buys={} sells={} mints={} roundtrips={} win_rate={}",
            output.overview.total_trades,
            output.overview.buy_count,
            output.overview.sell_count,
            output.overview.distinct_mints,
            output.overview.roundtrip_count,
            output
                .overview
                .win_rate_closed
                .map(|value| format!("{:.2}%", value * 100.0))
                .unwrap_or_else(|| "n/a".to_string()),
        );
        println!(
            "wallet shape       : entries={} roundtrips={} closed={} open={} orphan_sells={}",
            output.wallet_summary.entry_count,
            output.wallet_summary.roundtrip_count,
            output.wallet_summary.closed_roundtrip_count,
            output.wallet_summary.open_roundtrip_count,
            output.wallet_summary.orphan_sell_count,
        );
        println!(
            "recommended family : {} ({})",
            output.clone_report.recommended_base_family, output.explain_why.confidence
        );
        println!(
            "decision           : {}",
            output.explain_why.decision_summary
        );
        println!("top strengths:");
        for line in output.explain_why.strengths.iter().take(3) {
            println!("  - {}", line);
        }
        println!("main warnings:");
        for line in output.explain_why.warnings.iter().take(3) {
            println!("  - {}", line);
        }
        println!("top mints:");
        for mint in output.top_mints.iter().take(5) {
            println!(
                "  - mint={} trades={} buys={} sells={} last_trade_at={}",
                mint.mint,
                mint.trade_count,
                mint.buy_count,
                mint.sell_count,
                mint.last_trade_at.as_deref().unwrap_or("n/a"),
            );
        }
        println!("sample entries:");
        for entry in output.sample_entries.iter().take(args.sample_limit) {
            println!(
                "  - mint={} age_before={} buys_before={} unique_before={} total_buy_before={} net_flow_before={}",
                entry.mint,
                entry
                    .age_secs_before
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "n/a".to_string()),
                entry.buy_count_before,
                entry.unique_buyers_before,
                entry.total_buy_lamports_before,
                entry.net_flow_lamports_before,
            );
        }
        println!("next experiments:");
        for proposal in output.suggest_next_experiment.proposals.iter().take(3) {
            println!(
                "  - [{}] {} | family={} | objective={}",
                proposal.priority, proposal.title, proposal.family, proposal.objective
            );
        }
    }

    Ok(())
}

pub async fn mint_shard_summary(
    args: MintShardSummaryArgs,
    format: OutputFormat,
) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let result = summarize_mint_shards(pump_agent_app::usecases::MintShardSummaryRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        address: args.address,
        limit: args.limit,
    })
    .await?;
    let output = mint_shard_summary_output(result);

    if format.is_json() {
        emit_json_success("mint_shard_summary", &output, started)?;
    } else {
        println!("address            : {}", output.address);
        println!("mint count         : {}", output.mint_count);
        println!("wallet trades      : {}", output.wallet_trade_count);
        println!("total event count  : {}", output.total_event_count);
        println!("shards:");
        for shard in &output.shards {
            println!(
                "  - mint={} symbol={} wallet_trades={} wallet_entries={} roundtrips={} events={} trades={} unique_traders={} net_flow={:.3} wallet_net_flow={:.3} complete={}",
                shard.mint,
                shard.symbol.as_deref().unwrap_or("-"),
                shard.wallet_trade_count,
                shard.wallet_entry_count,
                shard.wallet_roundtrip_count,
                shard.event_count,
                shard.trade_count,
                shard.unique_trader_count,
                shard.net_flow_sol,
                shard.wallet_net_flow_sol,
                shard.is_complete,
            );
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct CloneAnalysis {
    pub(crate) wallet: pump_agent_app::clone::WalletBehaviorReport,
    pub(crate) best_family: pump_agent_app::clone::StrategyCloneCandidate,
    pub(crate) runner_up: pump_agent_app::clone::StrategyCloneCandidate,
}

pub(crate) async fn analyze_clone_candidates(
    store: &PgEventStore,
    address: &str,
) -> Result<CloneAnalysis> {
    let events = store.load_replay_events().await?;
    let wallet = extract_wallet_behavior(address, &events);
    let mut candidates = Vec::new();
    for family in ["early_flow", "momentum", "breakout", "liquidity_follow"] {
        let args = default_strategy_config_for_family(family)?;
        let execution = pump_agent_app::strategy::run_strategy(events.clone(), &args)?;
        candidates.push(score_strategy_execution(&wallet, &args, &execution));
    }
    candidates.sort_by(|left, right| right.score.overall.total_cmp(&left.score.overall));
    let best_family = candidates.remove(0);
    let runner_up = candidates.remove(0);

    Ok(CloneAnalysis {
        wallet,
        best_family,
        runner_up,
    })
}

impl From<&AddressExportSummary> for CloneReportExportSummary {
    fn from(value: &AddressExportSummary) -> Self {
        Self {
            output: value.output.display().to_string(),
            address_dir: value
                .address_dir
                .as_ref()
                .map(|path| path.display().to_string()),
            index_path: value
                .index_path
                .as_ref()
                .map(|path| path.display().to_string()),
            mint_count: value.mint_count,
            wallet_trade_count: value.wallet_trade_count,
            event_count: value.event_count,
            shard_count: value.shard_count,
            sharded: value.sharded,
        }
    }
}
