use anyhow::Result;
use pump_agent_core::PgEventStore;
use serde::Serialize;

use crate::{
    args::{AddressFeaturesArgs, CloneEvalArgs, CloneRankArgs, FitParamsArgs, InferStrategyArgs},
    config::required_config,
    runtime::{
        build_fit_variants, default_strategy_args_for_family, deserialize_strategy_config,
        extract_wallet_behavior, resolve_strategy_args, run_clone_fit, score_strategy_execution,
    },
};

use crate::commands::helpers::SCHEMA_SQL;

pub async fn address_features(args: AddressFeaturesArgs) -> Result<()> {
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

pub async fn infer_strategy(args: InferStrategyArgs) -> Result<()> {
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let events = store.load_replay_events().await?;
    let wallet = extract_wallet_behavior(&args.address, &events);
    let families = args
        .family
        .as_deref()
        .map(|family| vec![family.to_string()])
        .unwrap_or_else(|| vec!["early_flow".to_string(), "momentum".to_string()]);

    let mut candidates = Vec::new();
    for family in families {
        let strategy_args = default_strategy_args_for_family(&family)?;
        let execution = crate::runtime::run_strategy(events.clone(), &strategy_args)?;
        candidates.push(score_strategy_execution(
            &wallet,
            &strategy_args,
            &execution,
        ));
    }

    candidates.sort_by(|left, right| right.score.overall.total_cmp(&left.score.overall));

    println!("address: {}", wallet.address);
    println!(
        "wallet entries={} roundtrips={} closed={} open={} orphan_sells={}",
        wallet.summary.entry_count,
        wallet.summary.roundtrip_count,
        wallet.summary.closed_roundtrip_count,
        wallet.summary.open_roundtrip_count,
        wallet.summary.orphan_sell_count
    );
    println!();

    for candidate in candidates {
        print_candidate(&candidate);
        println!();
    }

    Ok(())
}

pub async fn clone_eval(args: CloneEvalArgs) -> Result<()> {
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let events = store.load_replay_events().await?;
    let wallet = extract_wallet_behavior(&args.address, &events);
    let (resolved, eval_source) = if let Some(run_id) = args.run_id {
        let inspect = store.inspect_strategy_run(run_id, 0).await?;
        let Some(run) = inspect.run else {
            println!("strategy run not found: {}", run_id);
            return Ok(());
        };
        (
            deserialize_strategy_config(&run.config)?,
            format!("run_id={}", run_id),
        )
    } else {
        (
            resolve_strategy_args(&args.strategy)?,
            "strategy_args".to_string(),
        )
    };
    let execution = crate::runtime::run_strategy(events, &resolved)?;
    let candidate = score_strategy_execution(&wallet, &resolved, &execution);

    let output = CloneEvalOutput {
        address: wallet.address.clone(),
        wallet_entries: wallet.summary.entry_count,
        wallet_roundtrips: wallet.summary.roundtrip_count,
        wallet_closed_roundtrips: wallet.summary.closed_roundtrip_count,
        strategy: resolved.strategy.clone(),
        strategy_name: candidate.report.strategy.name.to_string(),
        eval_source,
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
        fills: candidate.report.fills,
        rejections: candidate.report.rejections,
        ending_equity_lamports: candidate.report.ending_equity_lamports,
        ending_cash_lamports: candidate.report.ending_cash_lamports,
        resolved_strategy: resolved,
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
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
    }

    Ok(())
}

pub async fn fit_params(args: FitParamsArgs) -> Result<()> {
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let events = store.load_replay_events().await?;
    let wallet = extract_wallet_behavior(&args.address, &events);
    let mut base = default_strategy_args_for_family(&args.family)?;
    base.strategy = args.family.replace('-', "_");
    base.starting_sol = args.strategy.starting_sol;
    base.trading_fee_bps = args.strategy.trading_fee_bps;
    base.slippage_bps = args.strategy.slippage_bps;
    let variants = build_fit_variants(&base, &args)?;
    let fit = run_clone_fit(&events, &wallet, variants)?;

    println!(
        "address={} family={} candidates={} wallet_entries={} wallet_roundtrips={}",
        wallet.address,
        args.family,
        fit.candidates.len(),
        wallet.summary.entry_count,
        wallet.summary.roundtrip_count
    );
    println!();

    for candidate in fit.candidates.iter().take(args.top) {
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

pub async fn clone_rank(args: CloneRankArgs) -> Result<()> {
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let events = store.load_replay_events().await?;
    let wallet = extract_wallet_behavior(&args.address, &events);
    let runs = store.list_strategy_runs(args.scan_limit).await?;

    let mut ranked = Vec::new();
    for run in runs {
        let inspect = store.inspect_strategy_run(run.id, 0).await?;
        let Some(detail) = inspect.run else {
            continue;
        };
        let Ok(resolved) = deserialize_strategy_config(&detail.config) else {
            continue;
        };
        let execution = crate::runtime::run_strategy(events.clone(), &resolved)?;
        let candidate = score_strategy_execution(&wallet, &resolved, &execution);
        ranked.push(CloneRankRow {
            run_id: detail.id,
            strategy: resolved.strategy.clone(),
            strategy_name: candidate.report.strategy.name.to_string(),
            run_mode: detail.run_mode,
            source_type: detail.source_type,
            source_ref: detail.source_ref,
            started_at: detail.started_at,
            stored_equity_lamports: detail.ending_equity_lamports,
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
        });
    }

    ranked.sort_by(|left, right| {
        right
            .clone_score
            .total_cmp(&left.clone_score)
            .then_with(|| right.f1.total_cmp(&left.f1))
            .then_with(|| right.count_alignment.total_cmp(&left.count_alignment))
            .then_with(|| right.run_id.cmp(&left.run_id))
    });

    let top = ranked.into_iter().take(args.top).collect::<Vec<_>>();
    if args.json {
        let output = CloneRankOutput {
            address: wallet.address,
            wallet_entries: wallet.summary.entry_count,
            wallet_roundtrips: wallet.summary.roundtrip_count,
            scanned_runs: args.scan_limit,
            ranked: top,
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "address={} wallet_entries={} wallet_roundtrips={} scanned_runs={}",
            wallet.address,
            wallet.summary.entry_count,
            wallet.summary.roundtrip_count,
            args.scan_limit
        );
        println!();
        for row in top {
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

#[derive(Debug, Serialize)]
struct CloneEvalOutput {
    address: String,
    wallet_entries: usize,
    wallet_roundtrips: usize,
    wallet_closed_roundtrips: usize,
    strategy: String,
    strategy_name: String,
    eval_source: String,
    clone_score: f64,
    f1: f64,
    precision: f64,
    recall: f64,
    matched_entries: usize,
    strategy_entries: usize,
    entry_delay_secs: Option<f64>,
    hold_error_secs: Option<f64>,
    size_error_ratio: Option<f64>,
    count_alignment: f64,
    fills: u64,
    rejections: u64,
    ending_equity_lamports: u64,
    ending_cash_lamports: u64,
    resolved_strategy: crate::args::StrategyArgs,
}

#[derive(Debug, Serialize)]
struct CloneRankOutput {
    address: String,
    wallet_entries: usize,
    wallet_roundtrips: usize,
    scanned_runs: i64,
    ranked: Vec<CloneRankRow>,
}

#[derive(Debug, Serialize)]
struct CloneRankRow {
    run_id: i64,
    strategy: String,
    strategy_name: String,
    run_mode: String,
    source_type: String,
    source_ref: String,
    started_at: String,
    stored_equity_lamports: String,
    clone_score: f64,
    f1: f64,
    precision: f64,
    recall: f64,
    matched_entries: usize,
    strategy_entries: usize,
    entry_delay_secs: Option<f64>,
    hold_error_secs: Option<f64>,
    size_error_ratio: Option<f64>,
    count_alignment: f64,
}

fn print_candidate(candidate: &crate::runtime::StrategyCloneCandidate) {
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
}
