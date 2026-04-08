use pump_agent_app::usecases::{
    CloneAnalysisRequest, DatabaseRequest,
    analyze_clone_candidates as analyze_clone_candidates_usecase,
};
use pump_agent_core::PgEventStore;
use serde::Serialize;
use std::time::Instant;

use crate::{
    args::{AddressBriefArgs, OutputFormat},
    config::required_config,
    output::{CommandResult, emit_json_success, wants_json},
};

use super::export::{export_address_events, print_export_summary};
use crate::commands::helpers::SCHEMA_SQL;

pub async fn address_brief(args: AddressBriefArgs, format: OutputFormat) -> CommandResult<()> {
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
    let wallet = analysis.wallet;
    let best_family = analysis.best_family;
    let runner_up = analysis.runner_up;

    let export_summary = if args.export {
        let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
        store.apply_schema(SCHEMA_SQL).await?;
        Some(export_address_events(&store, &args.address, &args.export_root).await?)
    } else {
        None
    };

    if wants_json(format, args.json) {
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
        emit_json_success("address_brief", &output, started)?;
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
            "best breakdown     : entry={:.3} hold={:.3} size={:.3} token={:.3} exit={:.3} count={:.3}",
            best_family.score.breakdown.entry_timing_similarity,
            best_family.score.breakdown.hold_time_similarity,
            best_family.score.breakdown.size_profile_similarity,
            best_family.score.breakdown.token_selection_similarity,
            best_family.score.breakdown.exit_behavior_similarity,
            best_family.score.breakdown.count_alignment,
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
    breakdown: pump_agent_app::api::CloneScoreBreakdownOutput,
}

impl FamilySummary {
    fn from_candidate(candidate: &pump_agent_app::clone::StrategyCloneCandidate) -> Self {
        Self {
            family: candidate.args.strategy.clone(),
            clone_score: candidate.score.overall,
            f1: candidate.score.f1,
            precision: candidate.score.precision,
            recall: candidate.score.recall,
            breakdown: pump_agent_app::api::clone_score_breakdown_output(
                &candidate.score.breakdown,
            ),
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
