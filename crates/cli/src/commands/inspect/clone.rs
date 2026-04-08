use pump_agent_app::{
    api::{clone_eval_output, clone_rank_output, fit_params_output, infer_strategy_output},
    clone::{
        StrategyCloneCandidate, WalletBehaviorSummary, WalletEntryFeature, WalletRoundtrip,
        extract_wallet_behavior,
    },
    strategy::StrategyConfig,
    usecases::{
        CloneEvalRequest, CloneRankRequest, DatabaseRequest, FitParamsRequest,
        InferStrategyRequest, clone_eval as clone_eval_usecase, clone_rank as clone_rank_usecase,
        fit_params as fit_params_usecase, infer_strategy as infer_strategy_usecase,
    },
};
use pump_agent_core::PgEventStore;
use serde_json::json;
use std::time::Instant;

use crate::{
    args::{
        AddressFeaturesArgs, CloneEvalArgs, CloneRankArgs, FitParamsArgs, InferStrategyArgs,
        OutputFormat,
    },
    config::required_config,
    output::{CommandError, CommandResult, emit_json_success, wants_json},
};

use crate::commands::helpers::SCHEMA_SQL;

pub async fn address_features(args: AddressFeaturesArgs) -> anyhow::Result<()> {
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let events = store.load_replay_events().await?;
    let report = extract_wallet_behavior(&args.address, &events);

    println!("address                     : {}", report.address);
    println!(
        "entries                     : {}",
        report.summary.entry_count
    );
    println!(
        "roundtrips                  : {}",
        report.summary.roundtrip_count
    );
    println!(
        "closed roundtrips           : {}",
        report.summary.closed_roundtrip_count
    );
    println!(
        "open roundtrips             : {}",
        report.summary.open_roundtrip_count
    );
    println!(
        "orphan sells                : {}",
        report.summary.orphan_sell_count
    );
    println!(
        "avg entry age               : {}",
        report
            .summary
            .avg_entry_age_secs
            .map(|value| format!("{value:.2}s"))
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!(
        "avg entry buy count before  : {}",
        report
            .summary
            .avg_entry_buy_count_before
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!(
        "avg entry sell count before : {}",
        report
            .summary
            .avg_entry_sell_count_before
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!(
        "avg unique buyers before    : {}",
        report
            .summary
            .avg_entry_unique_buyers_before
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!(
        "avg total buy before        : {}",
        report
            .summary
            .avg_entry_total_buy_sol_before
            .map(|value| format!("{value:.6} SOL"))
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!(
        "avg net flow before         : {}",
        report
            .summary
            .avg_entry_net_flow_sol_before
            .map(|value| format!("{value:.6} SOL"))
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!(
        "avg buy/sell ratio before   : {}",
        report
            .summary
            .avg_entry_buy_sell_ratio_before
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!(
        "avg entry buy size          : {}",
        report
            .summary
            .avg_entry_buy_sol
            .map(|value| format!("{value:.6} SOL"))
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!(
        "avg hold closed             : {}",
        report
            .summary
            .avg_hold_secs_closed
            .map(|value| format!("{value:.2}s"))
            .unwrap_or_else(|| "n/a".to_string())
    );

    println!();
    println!("sample entries:");
    for entry in report.entries.iter().take(args.sample_limit) {
        println!(
            "mint={} seq={} slot={} ts={} age_before={} buy_count_before={} sell_count_before={} unique_buyers_before={} total_buy_before={} lamports net_flow_before={} lamports ratio_before={:.2} entry_buy={} lamports",
            entry.mint,
            entry.entry_seq,
            entry.entry_slot,
            entry
                .entry_ts
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            entry
                .age_secs_before
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            entry.buy_count_before,
            entry.sell_count_before,
            entry.unique_buyers_before,
            entry.total_buy_lamports_before,
            entry.net_flow_lamports_before,
            entry.buy_sell_ratio_before,
            entry.entry_buy_lamports
        );
    }

    println!();
    println!("sample roundtrips:");
    for roundtrip in report.roundtrips.iter().take(args.sample_limit) {
        println!(
            "mint={} status={} entry_ts={} exit_ts={} hold={} gross_buy={} lamports gross_sell={} lamports",
            roundtrip.mint,
            roundtrip.status,
            roundtrip
                .entry_ts
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            roundtrip
                .exit_ts
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            roundtrip
                .hold_secs
                .map(|value| format!("{value}s"))
                .unwrap_or_else(|| "n/a".to_string()),
            roundtrip.gross_buy_lamports,
            roundtrip.gross_sell_lamports
        );
    }

    Ok(())
}

pub async fn address_features_json(
    args: AddressFeaturesArgs,
    format: OutputFormat,
) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let events = store.load_replay_events().await?;
    let report = extract_wallet_behavior(&args.address, &events);

    if format.is_json() {
        let output = AddressFeaturesOutput {
            address: report.address,
            summary: report.summary,
            entries: report.entries.into_iter().take(args.sample_limit).collect(),
            roundtrips: report
                .roundtrips
                .into_iter()
                .take(args.sample_limit)
                .collect(),
        };
        return emit_json_success("address_features", &output, started);
    }

    address_features(args).await.map_err(CommandError::from)
}

pub async fn infer_strategy(args: InferStrategyArgs) -> anyhow::Result<()> {
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let result = infer_strategy_usecase(InferStrategyRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        address: args.address,
        family: args.family,
    })
    .await?;

    println!("address: {}", result.wallet.address);
    println!(
        "wallet entries={} roundtrips={} closed={} open={} orphan_sells={}",
        result.wallet.summary.entry_count,
        result.wallet.summary.roundtrip_count,
        result.wallet.summary.closed_roundtrip_count,
        result.wallet.summary.open_roundtrip_count,
        result.wallet.summary.orphan_sell_count
    );
    println!();

    for candidate in result.candidates {
        print_candidate(&candidate);
        println!();
    }

    Ok(())
}

pub async fn infer_strategy_json(
    args: InferStrategyArgs,
    format: OutputFormat,
) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let result = infer_strategy_usecase(InferStrategyRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        address: args.address.clone(),
        family: args.family.clone(),
    })
    .await?;

    if format.is_json() {
        let output = infer_strategy_output(result);
        return emit_json_success("infer_strategy", &output, started);
    }

    infer_strategy(args).await.map_err(CommandError::from)
}

pub async fn clone_eval(args: CloneEvalArgs, format: OutputFormat) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let eval = clone_eval_usecase(CloneEvalRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        address: args.address.clone(),
        strategy: args
            .run_id
            .is_none()
            .then(|| strategy_config_from_args(&args.strategy)),
        run_id: args.run_id,
        experiment: None,
    })
    .await?;
    let Some(eval) = eval else {
        let run_id = args
            .run_id
            .expect("run_id should exist when eval result is missing");
        return Err(
            CommandError::not_found(format!("strategy run not found: {}", run_id)).with_details(
                json!({
                    "resource": "strategy_run",
                    "id": run_id,
                }),
            ),
        );
    };
    let output = clone_eval_output(eval);

    if wants_json(format, args.json) {
        emit_json_success("clone_eval", &output, started)?;
    } else {
        println!("address            : {}", output.address);
        println!("strategy           : {}", output.strategy);
        println!("strategy name      : {}", output.strategy_name);
        println!("eval source        : {}", output.eval_source);
        println!(
            "wallet shape       : entries={} roundtrips={} closed={}",
            output.wallet_entries, output.wallet_roundtrips, output.wallet_closed_roundtrips
        );
        println!(
            "clone metrics      : score={:.4} f1={:.4} precision={:.4} recall={:.4}",
            output.clone_score, output.f1, output.precision, output.recall
        );
        println!(
            "match detail       : matched_entries={} strategy_entries={} count_alignment={:.4}",
            output.matched_entries, output.strategy_entries, output.count_alignment
        );
        println!(
            "timing             : entry_delay={} hold_error={}",
            output
                .entry_delay_secs
                .map(|value| format!("{value:.2}s"))
                .unwrap_or_else(|| "n/a".to_string()),
            output
                .hold_error_secs
                .map(|value| format!("{value:.2}s"))
                .unwrap_or_else(|| "n/a".to_string())
        );
        println!(
            "size error         : {}",
            output
                .size_error_ratio
                .map(|value| format!("{:.2}%", value * 100.0))
                .unwrap_or_else(|| "n/a".to_string())
        );
        println!(
            "breakdown          : entry={:.3} hold={:.3} size={:.3} token={:.3} exit={:.3} count={:.3}",
            output.breakdown.entry_timing_similarity,
            output.breakdown.hold_time_similarity,
            output.breakdown.size_profile_similarity,
            output.breakdown.token_selection_similarity,
            output.breakdown.exit_behavior_similarity,
            output.breakdown.count_alignment,
        );
        println!(
            "execution          : fills={} rejections={} ending_cash={} ending_equity={}",
            output.fills,
            output.rejections,
            output.ending_cash_lamports,
            output.ending_equity_lamports
        );
        println!(
            "resolved params    : buy_sol={} max_age_secs={} min_buy_count={} min_unique_buyers={} min_net_buy_sol={} min_total_buy_sol={} max_sell_count={} ratio={} max_hold_secs={} concurrent={} exit_on_sell_count={}",
            output.resolved_strategy.buy_sol,
            output.resolved_strategy.max_age_secs,
            output.resolved_strategy.min_buy_count,
            output.resolved_strategy.min_unique_buyers,
            output.resolved_strategy.min_net_buy_sol,
            output.resolved_strategy.min_total_buy_sol,
            output.resolved_strategy.max_sell_count,
            output.resolved_strategy.min_buy_sell_ratio,
            output.resolved_strategy.max_hold_secs,
            output.resolved_strategy.max_concurrent_positions,
            output.resolved_strategy.exit_on_sell_count,
        );
        if let Some(evaluation_id) = &output.recorded_evaluation_id {
            println!("recorded eval      : {}", evaluation_id);
        }
    }

    Ok(())
}

pub async fn fit_params(args: FitParamsArgs) -> anyhow::Result<()> {
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let sweep = sweep_config_from_fit_args(&args);
    let result = fit_params_usecase(FitParamsRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        address: args.address,
        family: args.family,
        base_overrides: strategy_config_from_args(&args.strategy),
        sweep,
    })
    .await?;

    println!(
        "address={} family={} candidates={} wallet_entries={} wallet_roundtrips={}",
        result.wallet.address,
        result.family,
        result.fit.candidates.len(),
        result.wallet.summary.entry_count,
        result.wallet.summary.roundtrip_count
    );
    println!();

    for candidate in result.fit.candidates.iter().take(args.top) {
        print_candidate(candidate);
        println!(
            "  params: buy_sol={} max_age_secs={} min_buy_count={} min_unique_buyers={} min_total_buy_sol={} max_sell_count={} ratio={} take_profit_bps={} stop_loss_bps={} concurrent={} exit_on_sell_count={}",
            candidate.args.buy_sol,
            candidate.args.max_age_secs,
            candidate.args.min_buy_count,
            candidate.args.min_unique_buyers,
            candidate.args.min_total_buy_sol,
            candidate.args.max_sell_count,
            candidate.args.min_buy_sell_ratio,
            candidate.args.take_profit_bps,
            candidate.args.stop_loss_bps,
            candidate.args.max_concurrent_positions,
            candidate.args.exit_on_sell_count
        );
        println!();
    }

    Ok(())
}

pub async fn fit_params_json(args: FitParamsArgs, format: OutputFormat) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let result = fit_params_usecase(FitParamsRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        address: args.address.clone(),
        family: args.family.clone(),
        base_overrides: strategy_config_from_args(&args.strategy),
        sweep: sweep_config_from_fit_args(&args),
    })
    .await?;

    if format.is_json() {
        let output = fit_params_output(result, args.top);
        return emit_json_success("fit_params", &output, started);
    }

    fit_params(args).await.map_err(CommandError::from)
}

pub async fn clone_rank(args: CloneRankArgs, format: OutputFormat) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let result = clone_rank_usecase(CloneRankRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        address: args.address.clone(),
        scan_limit: args.scan_limit,
    })
    .await?;
    let output = clone_rank_output(result, args.top);
    if wants_json(format, args.json) {
        emit_json_success("clone_rank", &output, started)?;
    } else {
        println!(
            "address={} wallet_entries={} wallet_roundtrips={} scanned_runs={}",
            output.address, output.wallet_entries, output.wallet_roundtrips, args.scan_limit
        );
        println!();
        for row in output.ranked {
            println!(
                "run_id={} strategy={} strategy_name={} mode={} source={} clone_score={:.4} f1={:.4} precision={:.4} recall={:.4} matched={}/{} entry_delay={} hold_error={} size_error={} count_alignment={:.4} stored_equity={}",
                row.run_id,
                row.strategy,
                row.strategy_name,
                row.run_mode,
                row.source_type,
                row.clone_score,
                row.f1,
                row.precision,
                row.recall,
                row.matched_entries,
                row.strategy_entries,
                row.entry_delay_secs
                    .map(|value| format!("{value:.2}s"))
                    .unwrap_or_else(|| "n/a".to_string()),
                row.hold_error_secs
                    .map(|value| format!("{value:.2}s"))
                    .unwrap_or_else(|| "n/a".to_string()),
                row.size_error_ratio
                    .map(|value| format!("{:.2}%", value * 100.0))
                    .unwrap_or_else(|| "n/a".to_string()),
                row.count_alignment,
                row.stored_equity_lamports,
            );
        }
    }

    Ok(())
}

fn strategy_config_from_args(args: &crate::args::StrategyArgs) -> StrategyConfig {
    StrategyConfig {
        strategy: args.strategy.clone(),
        strategy_config: args.strategy_config.clone(),
        starting_sol: args.starting_sol,
        buy_sol: args.buy_sol,
        max_age_secs: args.max_age_secs,
        min_buy_count: args.min_buy_count,
        min_unique_buyers: args.min_unique_buyers,
        min_net_buy_sol: args.min_net_buy_sol,
        take_profit_bps: args.take_profit_bps,
        stop_loss_bps: args.stop_loss_bps,
        max_hold_secs: args.max_hold_secs,
        min_total_buy_sol: args.min_total_buy_sol,
        max_sell_count: args.max_sell_count,
        min_buy_sell_ratio: args.min_buy_sell_ratio,
        max_concurrent_positions: args.max_concurrent_positions,
        exit_on_sell_count: args.exit_on_sell_count,
        trading_fee_bps: args.trading_fee_bps,
        slippage_bps: args.slippage_bps,
    }
}

#[derive(Debug, serde::Serialize)]
struct AddressFeaturesOutput {
    address: String,
    summary: WalletBehaviorSummary,
    entries: Vec<WalletEntryFeature>,
    roundtrips: Vec<WalletRoundtrip>,
}

fn print_candidate(candidate: &StrategyCloneCandidate) {
    println!(
        "family={} strategy_name={} clone_score={:.4} f1={:.4} precision={:.4} recall={:.4} matches={}/{} strategy_entries={} entry_delay={} hold_error={} size_error={} count_alignment={:.4} fills={} equity={} lamports",
        candidate.args.strategy,
        candidate.report.strategy.name,
        candidate.score.overall,
        candidate.score.f1,
        candidate.score.precision,
        candidate.score.recall,
        candidate.score.matched_entries,
        candidate.score.wallet_entries,
        candidate.score.strategy_entries,
        candidate
            .score
            .avg_entry_delay_secs
            .map(|value| format!("{value:.2}s"))
            .unwrap_or_else(|| "n/a".to_string()),
        candidate
            .score
            .avg_hold_error_secs
            .map(|value| format!("{value:.2}s"))
            .unwrap_or_else(|| "n/a".to_string()),
        candidate
            .score
            .avg_size_error_ratio
            .map(|value| format!("{:.2}%", value * 100.0))
            .unwrap_or_else(|| "n/a".to_string()),
        candidate.score.count_alignment,
        candidate.report.fills,
        candidate.report.ending_equity_lamports
    );
    println!(
        "  breakdown: entry={:.3} hold={:.3} size={:.3} token={:.3} exit={:.3} count={:.3}",
        candidate.score.breakdown.entry_timing_similarity,
        candidate.score.breakdown.hold_time_similarity,
        candidate.score.breakdown.size_profile_similarity,
        candidate.score.breakdown.token_selection_similarity,
        candidate.score.breakdown.exit_behavior_similarity,
        candidate.score.breakdown.count_alignment,
    );
}

fn sweep_config_from_fit_args(args: &FitParamsArgs) -> pump_agent_app::strategy::SweepConfig {
    pump_agent_app::strategy::SweepConfig {
        buy_sol_values: args.buy_sol_values.clone(),
        max_age_secs_values: args.max_age_secs_values.clone(),
        min_buy_count_values: args.min_buy_count_values.clone(),
        min_unique_buyers_values: args.min_unique_buyers_values.clone(),
        min_total_buy_sol_values: args.min_total_buy_sol_values.clone(),
        max_sell_count_values: args.max_sell_count_values.clone(),
        min_buy_sell_ratio_values: args.min_buy_sell_ratio_values.clone(),
        take_profit_bps_values: args.take_profit_bps_values.clone(),
        stop_loss_bps_values: args.stop_loss_bps_values.clone(),
        max_concurrent_positions_values: args.max_concurrent_positions_values.clone(),
        exit_on_sell_count_values: args.exit_on_sell_count_values.clone(),
    }
}
