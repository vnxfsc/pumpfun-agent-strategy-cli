use pump_agent_app::{
    api::compare_runs_output,
    usecases::{
        CompareRunsRequest, DatabaseRequest, RunInspectRequest, RunsRequest,
        SweepBatchInspectRequest, compare_runs as compare_runs_usecase, inspect_run,
        inspect_sweep_batch, list_runs,
    },
};
use serde::Serialize;
use serde_json::json;
use std::time::Instant;

use crate::{
    args::{CompareRunsArgs, OutputFormat, RunInspectArgs, RunsArgs, SweepBatchInspectArgs},
    config::{blank_to_na, lamports_str_to_sol, required_config},
    output::{CommandError, CommandResult, emit_json_success},
};

use crate::commands::helpers::json_num_string;

pub async fn runs(args: RunsArgs, format: OutputFormat) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let runs = list_runs(RunsRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        limit: args.limit,
    })
    .await?;

    if format.is_json() {
        return emit_json_success("runs", &RunsOutput { runs }, started);
    }

    if runs.is_empty() {
        println!("no strategy runs found");
        return Ok(());
    }

    for run in runs {
        println!(
            "id={} mode={} batch={} live={} strategy={} source={} fills={} rejections={} events={} equity={:.6} SOL started_at={}",
            run.id,
            run.run_mode,
            run.sweep_batch_id.as_deref().unwrap_or("-"),
            run.live_run_id.as_deref().unwrap_or("-"),
            run.strategy_name,
            run.source_type,
            run.fills,
            run.rejections,
            run.processed_events,
            lamports_str_to_sol(&run.ending_equity_lamports)?,
            run.started_at
        );
    }

    Ok(())
}

pub async fn run_inspect(args: RunInspectArgs, format: OutputFormat) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let report = inspect_run(RunInspectRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        run_id: args.id,
        fill_limit: args.fill_limit,
    })
    .await?;

    let Some(run) = report.run else {
        return Err(
            CommandError::not_found(format!("strategy run not found: {}", args.id)).with_details(
                json!({
                    "resource": "strategy_run",
                    "id": args.id,
                }),
            ),
        );
    };

    if format.is_json() {
        let output = RunInspectOutput {
            run,
            fills: report.fills,
            position_snapshots: report.position_snapshots,
        };
        return emit_json_success("run_inspect", &output, started);
    }

    println!("id              : {}", run.id);
    println!("strategy        : {}", run.strategy_name);
    println!("run mode        : {}", run.run_mode);
    println!(
        "sweep batch id  : {}",
        run.sweep_batch_id.as_deref().unwrap_or("n/a")
    );
    println!(
        "live run id     : {}",
        run.live_run_id.as_deref().unwrap_or("n/a")
    );
    println!("source type     : {}", run.source_type);
    println!("source ref      : {}", run.source_ref);
    println!("started at      : {}", run.started_at);
    println!(
        "finished at     : {}",
        run.finished_at.as_deref().map(blank_to_na).unwrap_or("n/a")
    );
    println!("events          : {}", run.processed_events);
    println!("fills           : {}", run.fills);
    println!("rejections      : {}", run.rejections);
    println!(
        "ending cash     : {:.6} SOL",
        lamports_str_to_sol(&run.ending_cash_lamports)?
    );
    println!(
        "ending equity   : {:.6} SOL",
        lamports_str_to_sol(&run.ending_equity_lamports)?
    );
    println!(
        "config          : {}",
        serde_json::to_string_pretty(&run.config).map_err(|error| {
            CommandError::internal(format!("failed to render run config as JSON: {error}"))
        })?
    );

    println!();
    println!("fills:");
    for fill in report.fills {
        println!(
            "order_id={} side={} mint={} lamports={} token_amount={} fee={} price={} executed_at={} reason={}",
            fill.order_id,
            fill.side,
            fill.mint,
            fill.lamports,
            fill.token_amount,
            fill.fee_lamports,
            fill.execution_price_lamports_per_token,
            fill.executed_at
                .as_deref()
                .map(blank_to_na)
                .unwrap_or("n/a"),
            fill.reason
        );
    }

    println!();
    println!("position snapshots:");
    for snapshot in report.position_snapshots {
        println!(
            "kind={} event_seq={} event_slot={} cash={:.6} SOL equity={:.6} SOL open_positions={} pending_orders={} snapshot_at={}",
            snapshot.snapshot_kind,
            snapshot
                .event_seq
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            snapshot
                .event_slot
                .map(|value| value.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            lamports_str_to_sol(&snapshot.cash_lamports)?,
            lamports_str_to_sol(&snapshot.equity_lamports)?,
            snapshot.open_positions,
            snapshot.pending_orders,
            snapshot
                .snapshot_at
                .as_deref()
                .map(blank_to_na)
                .unwrap_or("n/a")
        );
    }

    Ok(())
}

pub async fn compare_runs(args: CompareRunsArgs, format: OutputFormat) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let database = DatabaseRequest {
        database_url,
        max_db_connections: args.max_db_connections,
        apply_schema: true,
    };
    let result = compare_runs_usecase(CompareRunsRequest {
        database,
        left_run_id: args.left_id,
        right_run_id: args.right_id,
        fill_limit: args.fill_limit,
    })
    .await?;
    let Some(result) = result else {
        return Err(
            CommandError::not_found("one or both strategy runs were not found").with_details(
                json!({
                    "resource": "strategy_run",
                    "left_id": args.left_id,
                    "right_id": args.right_id,
                }),
            ),
        );
    };
    let output = compare_runs_output(result);
    let left_equity = lamports_str_to_sol(&output.left_run.ending_equity_lamports)?;
    let right_equity = lamports_str_to_sol(&output.right_run.ending_equity_lamports)?;
    let left_cash = lamports_str_to_sol(&output.left_run.ending_cash_lamports)?;
    let right_cash = lamports_str_to_sol(&output.right_run.ending_cash_lamports)?;

    if format.is_json() {
        return emit_json_success("compare_runs", &output, started);
    }

    println!(
        "compare runs left={} right={}",
        output.left_run.id, output.right_run.id
    );
    println!(
        "strategy       : {} vs {}",
        output.left_run.strategy_name, output.right_run.strategy_name
    );
    println!(
        "mode           : {} vs {}",
        output.left_run.run_mode, output.right_run.run_mode
    );
    println!(
        "source         : {} vs {}",
        output.left_run.source_type, output.right_run.source_type
    );
    println!(
        "events         : {} vs {} (delta {:+})",
        output.left_run.processed_events, output.right_run.processed_events, output.deltas.events
    );
    println!(
        "fills          : {} vs {} (delta {:+})",
        output.left_run.fills, output.right_run.fills, output.deltas.fills
    );
    println!(
        "rejections     : {} vs {} (delta {:+})",
        output.left_run.rejections, output.right_run.rejections, output.deltas.rejections
    );
    println!(
        "cash           : {:.6} vs {:.6} SOL (delta {:+.6})",
        left_cash, right_cash, output.deltas.cash_sol
    );
    println!(
        "equity         : {:.6} vs {:.6} SOL (delta {:+.6})",
        left_equity, right_equity, output.deltas.equity_sol
    );
    println!(
        "snapshots      : {} vs {}",
        output.loaded_position_snapshots.left, output.loaded_position_snapshots.right
    );
    println!(
        "fills loaded   : {} vs {}",
        output.loaded_fills.left, output.loaded_fills.right
    );

    println!();
    println!(
        "strategy diff  : family_changed={} changed_fields={}",
        output.strategy_diff.family_changed, output.strategy_diff.changed_field_count
    );
    for field in &output.strategy_diff.changed_fields {
        let delta = field
            .numeric_delta
            .map(|value| format!(" delta {:+.6}", value))
            .unwrap_or_default();
        println!(
            "  - {}: {} -> {}{}",
            field.field, field.left, field.right, delta
        );
    }

    Ok(())
}

pub async fn sweep_batch_inspect(
    args: SweepBatchInspectArgs,
    format: OutputFormat,
) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let report = inspect_sweep_batch(SweepBatchInspectRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        batch_id: args.batch_id.clone(),
    })
    .await?;

    if report.runs.is_empty() {
        return Err(
            CommandError::not_found(format!("sweep batch not found: {}", args.batch_id))
                .with_details(json!({
                    "resource": "sweep_batch",
                    "batch_id": args.batch_id,
                })),
        );
    }

    if format.is_json() {
        return emit_json_success(
            "sweep_batch_inspect",
            &SweepBatchInspectOutput { report },
            started,
        );
    }

    println!("sweep batch id : {}", report.sweep_batch_id);
    println!("runs           : {}", report.runs.len());
    println!();

    for (index, run) in report.runs.iter().enumerate() {
        let strategy = run
            .config
            .get("strategy")
            .and_then(|value| value.as_str())
            .unwrap_or("n/a");
        let buy_sol = json_num_string(&run.config, "buy_sol");
        let min_total_buy_sol = json_num_string(&run.config, "min_total_buy_sol");
        let max_sell_count = json_num_string(&run.config, "max_sell_count");
        let min_buy_sell_ratio = json_num_string(&run.config, "min_buy_sell_ratio");
        let max_concurrent_positions = json_num_string(&run.config, "max_concurrent_positions");
        let exit_on_sell_count = json_num_string(&run.config, "exit_on_sell_count");

        println!(
            "#{rank} id={id} strategy={strategy_name} mode={mode} equity={equity:.6} SOL cash={cash:.6} SOL fills={fills} rejections={rejections} events={events} cfg.strategy={strategy} buy_sol={buy_sol} min_total_buy_sol={min_total_buy_sol} max_sell_count={max_sell_count} ratio={ratio} concurrent={concurrent} exit_on_sell_count={exit_on_sell_count}",
            rank = index + 1,
            id = run.id,
            strategy_name = run.strategy_name,
            mode = run.run_mode,
            equity = lamports_str_to_sol(&run.ending_equity_lamports)?,
            cash = lamports_str_to_sol(&run.ending_cash_lamports)?,
            fills = run.fills,
            rejections = run.rejections,
            events = run.processed_events,
            strategy = strategy,
            buy_sol = buy_sol,
            min_total_buy_sol = min_total_buy_sol,
            max_sell_count = max_sell_count,
            ratio = min_buy_sell_ratio,
            concurrent = max_concurrent_positions,
            exit_on_sell_count = exit_on_sell_count,
        );
    }

    Ok(())
}

#[derive(Debug, Serialize)]
struct RunsOutput {
    runs: Vec<pump_agent_core::StrategyRunRow>,
}

#[derive(Debug, Serialize)]
struct RunInspectOutput {
    run: pump_agent_core::StrategyRunDetail,
    fills: Vec<pump_agent_core::RunFillRow>,
    position_snapshots: Vec<pump_agent_core::PositionSnapshotRow>,
}

#[derive(Debug, Serialize)]
struct SweepBatchInspectOutput {
    report: pump_agent_core::SweepBatchInspectReport,
}
