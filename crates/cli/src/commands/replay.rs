use pump_agent_app::{
    strategy::{StrategyConfig, SweepConfig, run_strategy},
    usecases::{
        DatabaseRequest, ReplayDbRequest, SweepDbRequest, replay_db as replay_db_usecase,
        sweep_db as sweep_db_usecase,
    },
};
use pump_agent_core::{PgEventStore, StrategyRunPersistOptions, load_jsonl_events};
use serde::Serialize;
use std::time::Instant;

use crate::{
    args::{OutputFormat, ReplayArgs, ReplayDbArgs, SweepDbArgs},
    config::{lamports_to_sol, required_config},
    output::{CommandResult, emit_json_success},
    runtime::persist_run,
};

use super::helpers::{SCHEMA_SQL, print_report};

pub async fn replay(args: ReplayArgs) -> anyhow::Result<()> {
    let events = load_jsonl_events(&args.input)?;
    let strategy = strategy_config_from_args(&args.strategy);
    let execution = run_strategy(events, &strategy)?;
    print_report(execution.result.report.clone());

    if args.save_run {
        let database_url = required_config(args.database_url, "DATABASE_URL")?;
        let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
        store.apply_schema(SCHEMA_SQL).await?;
        let run_id = persist_run(
            &store,
            "jsonl",
            &args.input.display().to_string(),
            &args.strategy,
            &execution.result,
            StrategyRunPersistOptions {
                run_mode: Some("backtest".to_string()),
                position_snapshots: vec![execution.final_position_snapshot.clone()],
                ..Default::default()
            },
        )
        .await?;
        println!("saved strategy run id: {}", run_id);
    }

    Ok(())
}

pub async fn replay_db(args: ReplayDbArgs, format: OutputFormat) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let result = replay_db_usecase(ReplayDbRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: false,
        },
        strategy: strategy_config_from_args(&args.strategy),
        save_run: args.save_run,
        experiment: None,
    })
    .await?;

    if format.is_json() {
        let output = ReplayDbOutput {
            report: result.report,
            saved_run_id: result.saved_run_id,
            recorded_evaluation_id: result.recorded_evaluation_id,
        };
        return emit_json_success("replay_db", &output, started);
    }

    print_report(result.report);
    if let Some(run_id) = result.saved_run_id {
        println!("saved strategy run id: {}", run_id);
    }
    if let Some(evaluation_id) = result.recorded_evaluation_id {
        println!("recorded evaluation id: {}", evaluation_id);
    }

    Ok(())
}

pub async fn sweep_db(args: SweepDbArgs, format: OutputFormat) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let base = strategy_config_from_args(&args.strategy);
    let result = sweep_db_usecase(SweepDbRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        strategy: base.clone(),
        sweep: sweep_config_from_args(&args),
        experiment: None,
    })
    .await?;

    if !format.is_json() {
        println!(
            "running sweep strategy={} combinations={} source=postgres sweep_batch_id={}",
            result.strategy, result.combinations, result.sweep_batch_id
        );
    }

    for (index, summary) in result.summaries.iter().enumerate() {
        if !format.is_json() {
            println!(
                "combo {}/{} strategy={} buy_sol={} max_age_secs={} min_total_buy_sol={} max_sell_count={} ratio={} concurrent={}",
                index + 1,
                result.combinations,
                summary.strategy.strategy,
                summary.strategy.buy_sol,
                summary.strategy.max_age_secs,
                summary.strategy.min_total_buy_sol,
                summary.strategy.max_sell_count,
                summary.strategy.min_buy_sell_ratio,
                summary.strategy.max_concurrent_positions
            );
        }
    }

    let top_results = result
        .summaries
        .iter()
        .take(args.top)
        .map(SweepResultRow::from_summary)
        .collect::<Vec<_>>();

    if format.is_json() {
        let output = SweepDbOutput {
            strategy: result.strategy,
            combinations: result.combinations,
            sweep_batch_id: result.sweep_batch_id,
            recorded_evaluation_ids: result.recorded_evaluation_ids,
            top_results,
        };
        return emit_json_success("sweep_db", &output, started);
    }

    println!();
    println!("top {} results:", args.top.min(result.summaries.len()));
    for summary in &top_results {
        println!(
            "run_id={} strategy={} equity={:.6} SOL cash={:.6} SOL fills={} rejections={} open_positions={} buy_sol={} max_age_secs={} min_total_buy_sol={} max_sell_count={} ratio={} concurrent={} exit_on_sell_count={}",
            summary.run_id,
            summary.strategy_name,
            summary.ending_equity_sol,
            summary.ending_cash_sol,
            summary.fills,
            summary.rejections,
            summary.open_positions,
            summary.buy_sol,
            summary.max_age_secs,
            summary.min_total_buy_sol,
            summary.max_sell_count,
            summary.min_buy_sell_ratio,
            summary.max_concurrent_positions,
            summary.exit_on_sell_count
        );
    }

    Ok(())
}

#[derive(Debug, Serialize)]
struct ReplayDbOutput {
    report: pump_agent_core::BacktestReport,
    saved_run_id: Option<i64>,
    recorded_evaluation_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct SweepDbOutput {
    strategy: String,
    combinations: usize,
    sweep_batch_id: String,
    recorded_evaluation_ids: Vec<String>,
    top_results: Vec<SweepResultRow>,
}

#[derive(Debug, Serialize)]
struct SweepResultRow {
    run_id: i64,
    strategy_name: &'static str,
    ending_equity_sol: f64,
    ending_cash_sol: f64,
    fills: u64,
    rejections: u64,
    open_positions: usize,
    buy_sol: f64,
    max_age_secs: i64,
    min_total_buy_sol: f64,
    max_sell_count: u64,
    min_buy_sell_ratio: f64,
    max_concurrent_positions: usize,
    exit_on_sell_count: u64,
}

impl SweepResultRow {
    fn from_summary(summary: &pump_agent_app::strategy::SweepRunSummary) -> Self {
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

fn sweep_config_from_args(args: &SweepDbArgs) -> SweepConfig {
    SweepConfig {
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
