use anyhow::Result;
use pump_agent_core::{PgEventStore, StrategyRunPersistOptions, load_jsonl_events};

use crate::{
    args::{ReplayArgs, ReplayDbArgs, SweepDbArgs},
    config::{lamports_to_sol, required_config},
    runtime::{
        SweepRunSummary, build_sweep_variants, generate_run_group_id, persist_run,
        resolve_strategy_args, run_strategy,
    },
};

use super::helpers::{SCHEMA_SQL, print_report};

pub async fn replay(args: ReplayArgs) -> Result<()> {
    let events = load_jsonl_events(&args.input)?;
    let execution = run_strategy(events, &args.strategy)?;
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

pub async fn replay_db(args: ReplayDbArgs) -> Result<()> {
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    let events = store.load_replay_events().await?;
    let execution = run_strategy(events, &args.strategy)?;
    print_report(execution.result.report.clone());

    if args.save_run {
        store.apply_schema(SCHEMA_SQL).await?;
        let run_id = persist_run(
            &store,
            "postgres",
            "pump_event_envelopes",
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

pub async fn sweep_db(args: SweepDbArgs) -> Result<()> {
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let events = store.load_replay_events().await?;
    let base = resolve_strategy_args(&args.strategy)?;
    let variants = build_sweep_variants(&base, &args)?;
    let sweep_batch_id = generate_run_group_id("sweep");

    println!(
        "running sweep strategy={} combinations={} source=postgres sweep_batch_id={}",
        base.strategy,
        variants.len(),
        sweep_batch_id
    );

    let mut summaries = Vec::with_capacity(variants.len());
    for (index, variant) in variants.iter().enumerate() {
        println!(
            "combo {}/{} strategy={} buy_sol={} max_age_secs={} min_total_buy_sol={} max_sell_count={} ratio={} concurrent={}",
            index + 1,
            variants.len(),
            variant.strategy,
            variant.buy_sol,
            variant.max_age_secs,
            variant.min_total_buy_sol,
            variant.max_sell_count,
            variant.min_buy_sell_ratio,
            variant.max_concurrent_positions
        );

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
            args: variant.clone(),
            report: execution.result.report,
        });
    }

    summaries.sort_by(|left, right| {
        right
            .report
            .ending_equity_lamports
            .cmp(&left.report.ending_equity_lamports)
            .then_with(|| left.report.rejections.cmp(&right.report.rejections))
            .then_with(|| right.report.fills.cmp(&left.report.fills))
    });

    println!();
    println!("top {} results:", args.top.min(summaries.len()));
    for summary in summaries.iter().take(args.top) {
        println!(
            "run_id={} strategy={} equity={:.6} SOL cash={:.6} SOL fills={} rejections={} open_positions={} buy_sol={} max_age_secs={} min_total_buy_sol={} max_sell_count={} ratio={} concurrent={} exit_on_sell_count={}",
            summary.run_id,
            summary.report.strategy.name,
            lamports_to_sol(summary.report.ending_equity_lamports),
            lamports_to_sol(summary.report.ending_cash_lamports),
            summary.report.fills,
            summary.report.rejections,
            summary.report.open_positions,
            summary.args.buy_sol,
            summary.args.max_age_secs,
            summary.args.min_total_buy_sol,
            summary.args.max_sell_count,
            summary.args.min_buy_sell_ratio,
            summary.args.max_concurrent_positions,
            summary.args.exit_on_sell_count
        );
    }

    Ok(())
}
