use anyhow::Result;
use pump_agent_core::PgEventStore;
use serde::Serialize;

use crate::{
    args::CloneReportArgs,
    config::required_config,
    runtime::{
        default_strategy_args_for_family, extract_wallet_behavior, score_strategy_execution,
    },
};

use super::export::{AddressExportSummary, export_address_events};
use crate::commands::helpers::SCHEMA_SQL;

pub async fn clone_report(args: CloneReportArgs) -> Result<()> {
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let analysis = analyze_clone_candidates(&store, &args.address).await?;

    let export_summary = if args.export {
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

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
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

#[derive(Debug, Clone)]
pub(crate) struct CloneAnalysis {
    pub(crate) wallet: crate::runtime::WalletBehaviorReport,
    pub(crate) best_family: crate::runtime::StrategyCloneCandidate,
    pub(crate) runner_up: crate::runtime::StrategyCloneCandidate,
}

#[derive(Debug, Serialize)]
pub(crate) struct CloneReportOutput {
    pub(crate) address: String,
    pub(crate) recommended_base_family: String,
    pub(crate) recommended_next_strategy_name: String,
    pub(crate) base_fit: FitSummary,
    pub(crate) runner_up: FitSummary,
    pub(crate) confirmed_rules: Vec<String>,
    pub(crate) tentative_rules: Vec<String>,
    pub(crate) anti_patterns: Vec<String>,
    pub(crate) recommended_params_seed: ParamsSeed,
    pub(crate) export: Option<CloneReportExportSummary>,
}

#[derive(Debug, Serialize)]
pub(crate) struct FitSummary {
    pub(crate) family: String,
    pub(crate) clone_score: f64,
    pub(crate) f1: f64,
    pub(crate) precision: f64,
    pub(crate) recall: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct ParamsSeed {
    pub(crate) buy_sol: f64,
    pub(crate) max_age_secs: i64,
    pub(crate) min_buy_count: u64,
    pub(crate) min_unique_buyers: usize,
    pub(crate) min_net_buy_sol: f64,
    pub(crate) min_total_buy_sol: f64,
    pub(crate) max_sell_count: u64,
    pub(crate) min_buy_sell_ratio: f64,
    pub(crate) max_hold_secs: i64,
    pub(crate) max_concurrent_positions: usize,
    pub(crate) exit_on_sell_count: u64,
    pub(crate) take_profit_bps: i64,
    pub(crate) stop_loss_bps: i64,
}

pub(crate) async fn analyze_clone_candidates(
    store: &PgEventStore,
    address: &str,
) -> Result<CloneAnalysis> {
    let events = store.load_replay_events().await?;
    let wallet = extract_wallet_behavior(address, &events);

    let early_flow_args = default_strategy_args_for_family("early_flow")?;
    let momentum_args = default_strategy_args_for_family("momentum")?;
    let early_flow_execution = crate::runtime::run_strategy(events.clone(), &early_flow_args)?;
    let momentum_execution = crate::runtime::run_strategy(events, &momentum_args)?;
    let early_flow_fit = score_strategy_execution(&wallet, &early_flow_args, &early_flow_execution);
    let momentum_fit = score_strategy_execution(&wallet, &momentum_args, &momentum_execution);

    let (best_family, runner_up) = if early_flow_fit.score.overall >= momentum_fit.score.overall {
        (early_flow_fit, momentum_fit)
    } else {
        (momentum_fit, early_flow_fit)
    };

    Ok(CloneAnalysis {
        wallet,
        best_family,
        runner_up,
    })
}

pub(crate) fn build_clone_report(
    wallet: &crate::runtime::WalletBehaviorReport,
    best_family: &crate::runtime::StrategyCloneCandidate,
    runner_up: &crate::runtime::StrategyCloneCandidate,
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

    let mut confirmed_rules = Vec::new();
    confirmed_rules.push(format!(
        "uses roughly fixed ticket size around {:.3} SOL",
        avg_buy_sol
    ));
    confirmed_rules.push(format!(
        "typically enters after confirmation, not at t=0; average entry age {:.1}s",
        avg_age_secs
    ));
    confirmed_rules.push(format!(
        "requires meaningful pre-entry activity: buys_before≈{:.1}, unique_buyers≈{:.1}",
        avg_buy_count, avg_unique
    ));
    confirmed_rules.push(format!(
        "prefers already-hot mints: total_buy_before≈{:.2} SOL, net_flow_before≈{:.2} SOL",
        avg_total_buy_sol,
        wallet
            .summary
            .avg_entry_net_flow_sol_before
            .unwrap_or_default()
    ));
    confirmed_rules.push(format!(
        "exits quickly; average closed hold is {:.1}s",
        avg_hold_secs
    ));

    let mut tentative_rules = Vec::new();
    tentative_rules.push(format!(
        "may tolerate some early sell pressure before entry; sells_before≈{:.1}",
        avg_sell_before
    ));
    tentative_rules.push(format!(
        "buy/sell imbalance matters, but not in sniper mode; ratio_before≈{:.2}",
        avg_ratio
    ));
    tentative_rules.push(format!(
        "existing family fit is only moderate (clone_score {:.4}), so custom variant is likely needed",
        best_family.score.overall
    ));

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
        },
        runner_up: FitSummary {
            family: runner_up.args.strategy.clone(),
            clone_score: runner_up.score.overall,
            f1: runner_up.score.f1,
            precision: runner_up.score.precision,
            recall: runner_up.score.recall,
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

fn round_to(value: f64, decimals: u32) -> f64 {
    let factor = 10_f64.powi(decimals as i32);
    (value * factor).round() / factor
}

#[derive(Debug, Serialize)]
pub(crate) struct CloneReportExportSummary {
    pub(crate) output: String,
    pub(crate) address_dir: Option<String>,
    pub(crate) index_path: Option<String>,
    pub(crate) mint_count: i64,
    pub(crate) wallet_trade_count: i64,
    pub(crate) event_count: usize,
    pub(crate) shard_count: usize,
    pub(crate) sharded: bool,
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
