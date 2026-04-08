mod args;
mod commands;
mod config;
mod dashboard;
mod runtime;

use anyhow::Result;
use args::{Cli, Command};
use clap::Parser;
use dotenvy::dotenv;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenv();
    let cli = Cli::parse();

    match cli.command {
        Command::Replay(args) => commands::replay(args).await,
        Command::ReplayDb(args) => commands::replay_db(args).await,
        Command::SweepDb(args) => commands::sweep_db(args).await,
        Command::LivePaper(args) => commands::live_paper(args).await,
        Command::ServeDashboard(args) => commands::serve_dashboard(args).await,
        Command::StrategyScaffold(args) => commands::strategy_scaffold(args).await,
        Command::CloneScaffold(args) => commands::clone_scaffold(args).await,
        Command::Stats(args) => commands::stats(args).await,
        Command::Runs(args) => commands::runs(args).await,
        Command::RunInspect(args) => commands::run_inspect(args).await,
        Command::CompareRuns(args) => commands::compare_runs(args).await,
        Command::SweepBatchInspect(args) => commands::sweep_batch_inspect(args).await,
        Command::Tail(args) => commands::tail(args).await,
        Command::MintInspect(args) => commands::mint_inspect(args).await,
        Command::AddressInspect(args) => commands::address_inspect(args).await,
        Command::AddressTimeline(args) => commands::address_timeline(args).await,
        Command::AddressRoundtrips(args) => commands::address_roundtrips(args).await,
        Command::AddressFeatures(args) => commands::address_features(args).await,
        Command::AddressBrief(args) => commands::address_brief(args).await,
        Command::CloneReport(args) => commands::clone_report(args).await,
        Command::CloneEval(args) => commands::clone_eval(args).await,
        Command::CloneRank(args) => commands::clone_rank(args).await,
        Command::InferStrategy(args) => commands::infer_strategy(args).await,
        Command::FitParams(args) => commands::fit_params(args).await,
        Command::AddressExport(args) => commands::address_export(args).await,
        Command::Ingest(args) => commands::ingest(args).await,
    }
}
