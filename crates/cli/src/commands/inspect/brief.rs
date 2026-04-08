use anyhow::Result;
use pump_agent_core::PgEventStore;
use serde::Serialize;

use crate::{
    args::AddressBriefArgs,
    config::required_config,
    runtime::{
        default_strategy_args_for_family, extract_wallet_behavior, score_strategy_execution,
    },
};

use super::export::{export_address_events, print_export_summary};
use crate::commands::helpers::SCHEMA_SQL;

pub async fn address_brief(args: AddressBriefArgs) -> Result<()> {
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let events = store.load_replay_events().await?;
    let wallet = extract_wallet_behavior(&args.address, &events);

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

    let export_summary = if args.export {
        Some(export_address_events(&store, &args.address, &args.export_root).await?)
    } else {
        None
    };

    if args.json {
        let output = AddressBriefJsonOutput {
            address: wallet.address.clone(),
            shape: ShapeSummary {
                entries: wallet.summary.entry_count,
                closed: wallet.summary.closed_roundtrip_count,
                open: wallet.summary.open_roundtrip_count,
                orphan_sells: wallet.summary.orphan_sell_count,
            },
            entry_profile: EntryProfile {
                avg_buy_sol: wallet.summary.avg_entry_buy_sol.unwrap_or_default(),
                avg_age_secs: wallet.summary.avg_entry_age_secs.unwrap_or_default(),
                buys_before: wallet
                    .summary
                    .avg_entry_buy_count_before
                    .unwrap_or_default(),
                sells_before: wallet
                    .summary
                    .avg_entry_sell_count_before
                    .unwrap_or_default(),
                unique_before: wallet
                    .summary
                    .avg_entry_unique_buyers_before
                    .unwrap_or_default(),
            },
            flow_profile: FlowProfile {
                total_buy_before_sol: wallet
                    .summary
                    .avg_entry_total_buy_sol_before
                    .unwrap_or_default(),
                net_flow_before_sol: wallet
                    .summary
                    .avg_entry_net_flow_sol_before
                    .unwrap_or_default(),
                ratio_before: wallet
                    .summary
                    .avg_entry_buy_sell_ratio_before
                    .unwrap_or_default(),
            },
            exit_profile: ExitProfile {
                avg_hold_closed_secs: wallet.summary.avg_hold_secs_closed.unwrap_or_default(),
            },
            best_family: FamilySummary::from_candidate(&best_family),
            runner_up: FamilySummary::from_candidate(&runner_up),
            export: export_summary.as_ref().map(ExportSummary::from),
            export_hint: if export_summary.is_none() {
                Some(ExportHint {
                    export_dir: args.export_root.join(&args.address).display().to_string(),
                    index_path: args
                        .export_root
                        .join(&args.address)
                        .join("index.json")
                        .display()
                        .to_string(),
                })
            } else {
                None
            },
        };
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("address            : {}", wallet.address);
        println!(
            "shape              : entries={} closed={} open={} orphan_sells={}",
            wallet.summary.entry_count,
            wallet.summary.closed_roundtrip_count,
            wallet.summary.open_roundtrip_count,
            wallet.summary.orphan_sell_count
        );
        println!(
            "entry profile      : avg_buy={:.6} SOL avg_age={:.2}s buys_before={:.2} sells_before={:.2} unique_before={:.2}",
            wallet.summary.avg_entry_buy_sol.unwrap_or_default(),
            wallet.summary.avg_entry_age_secs.unwrap_or_default(),
            wallet
                .summary
                .avg_entry_buy_count_before
                .unwrap_or_default(),
            wallet
                .summary
                .avg_entry_sell_count_before
                .unwrap_or_default(),
            wallet
                .summary
                .avg_entry_unique_buyers_before
                .unwrap_or_default()
        );
        println!(
            "flow profile       : total_buy_before={:.6} SOL net_flow_before={:.6} SOL ratio_before={:.2}",
            wallet
                .summary
                .avg_entry_total_buy_sol_before
                .unwrap_or_default(),
            wallet
                .summary
                .avg_entry_net_flow_sol_before
                .unwrap_or_default(),
            wallet
                .summary
                .avg_entry_buy_sell_ratio_before
                .unwrap_or_default()
        );
        println!(
            "exit profile       : avg_hold_closed={:.2}s",
            wallet.summary.avg_hold_secs_closed.unwrap_or_default()
        );
        println!(
            "best family        : {} clone_score={:.4} f1={:.4} precision={:.4} recall={:.4}",
            best_family.args.strategy,
            best_family.score.overall,
            best_family.score.f1,
            best_family.score.precision,
            best_family.score.recall
        );
        println!(
            "runner up          : {} clone_score={:.4}",
            runner_up.args.strategy, runner_up.score.overall
        );

        if let Some(summary) = export_summary {
            println!();
            print_export_summary(&summary);
        } else {
            let export_dir = args.export_root.join(&args.address);
            let index_path = export_dir.join("index.json");
            println!("export hint        : rerun with --export");
            println!("export dir         : {}", export_dir.display());
            println!("index path         : {}", index_path.display());
        }
    }

    Ok(())
}

#[derive(Debug, Serialize)]
struct AddressBriefJsonOutput {
    address: String,
    shape: ShapeSummary,
    entry_profile: EntryProfile,
    flow_profile: FlowProfile,
    exit_profile: ExitProfile,
    best_family: FamilySummary,
    runner_up: FamilySummary,
    export: Option<ExportSummary>,
    export_hint: Option<ExportHint>,
}

#[derive(Debug, Serialize)]
struct ShapeSummary {
    entries: usize,
    closed: usize,
    open: usize,
    orphan_sells: usize,
}

#[derive(Debug, Serialize)]
struct EntryProfile {
    avg_buy_sol: f64,
    avg_age_secs: f64,
    buys_before: f64,
    sells_before: f64,
    unique_before: f64,
}

#[derive(Debug, Serialize)]
struct FlowProfile {
    total_buy_before_sol: f64,
    net_flow_before_sol: f64,
    ratio_before: f64,
}

#[derive(Debug, Serialize)]
struct ExitProfile {
    avg_hold_closed_secs: f64,
}

#[derive(Debug, Serialize)]
struct FamilySummary {
    family: String,
    clone_score: f64,
    f1: f64,
    precision: f64,
    recall: f64,
}

impl FamilySummary {
    fn from_candidate(candidate: &crate::runtime::StrategyCloneCandidate) -> Self {
        Self {
            family: candidate.args.strategy.clone(),
            clone_score: candidate.score.overall,
            f1: candidate.score.f1,
            precision: candidate.score.precision,
            recall: candidate.score.recall,
        }
    }
}

#[derive(Debug, Serialize)]
struct ExportSummary {
    output: String,
    address_dir: Option<String>,
    index_path: Option<String>,
    mint_count: i64,
    wallet_trade_count: i64,
    event_count: usize,
    shard_count: usize,
    sharded: bool,
}

impl From<&super::export::AddressExportSummary> for ExportSummary {
    fn from(value: &super::export::AddressExportSummary) -> Self {
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

#[derive(Debug, Serialize)]
struct ExportHint {
    export_dir: String,
    index_path: String,
}
