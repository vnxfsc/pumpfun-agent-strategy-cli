mod args;
mod commands;
mod config;
mod dashboard;
mod output;
mod runtime;

use args::{Cli, Command, OutputFormat};
use clap::Parser;
use dotenvy::dotenv;
use output::{CommandError, require_json_support};
use std::{process::ExitCode, time::Instant};

#[tokio::main]
async fn main() -> ExitCode {
    let _ = dotenv();
    let cli = Cli::parse();
    let started = Instant::now();
    let command_name = cli.command.name();
    let global_format = cli.format;

    let (result, effective_format) = match cli.command {
        Command::Replay(args) => (
            match require_json_support(command_name, global_format) {
                Ok(()) => commands::replay(args).await.map_err(CommandError::from),
                Err(error) => Err(error),
            },
            global_format,
        ),
        Command::ReplayDb(args) => (
            commands::replay_db(args, global_format).await,
            global_format,
        ),
        Command::SweepDb(args) => (commands::sweep_db(args, global_format).await, global_format),
        Command::LivePaper(args) => (
            match require_json_support(command_name, global_format) {
                Ok(()) => commands::live_paper(args).await.map_err(CommandError::from),
                Err(error) => Err(error),
            },
            global_format,
        ),
        Command::ServeDashboard(args) => (
            match require_json_support(command_name, global_format) {
                Ok(()) => commands::serve_dashboard(args)
                    .await
                    .map_err(CommandError::from),
                Err(error) => Err(error),
            },
            global_format,
        ),
        Command::StrategyScaffold(args) => (
            match require_json_support(command_name, global_format) {
                Ok(()) => commands::strategy_scaffold(args)
                    .await
                    .map_err(CommandError::from),
                Err(error) => Err(error),
            },
            global_format,
        ),
        Command::CloneScaffold(args) => (
            match require_json_support(command_name, global_format) {
                Ok(()) => commands::clone_scaffold(args)
                    .await
                    .map_err(CommandError::from),
                Err(error) => Err(error),
            },
            global_format,
        ),
        Command::Stats(args) => (commands::stats(args, global_format).await, global_format),
        Command::Runs(args) => (commands::runs(args, global_format).await, global_format),
        Command::RunInspect(args) => (
            commands::run_inspect(args, global_format).await,
            global_format,
        ),
        Command::CompareRuns(args) => (
            commands::compare_runs(args, global_format).await,
            global_format,
        ),
        Command::SweepBatchInspect(args) => (
            commands::sweep_batch_inspect(args, global_format).await,
            global_format,
        ),
        Command::Tail(args) => (
            match require_json_support(command_name, global_format) {
                Ok(()) => commands::tail(args).await.map_err(CommandError::from),
                Err(error) => Err(error),
            },
            global_format,
        ),
        Command::MintInspect(args) => (
            match require_json_support(command_name, global_format) {
                Ok(()) => commands::mint_inspect(args)
                    .await
                    .map_err(CommandError::from),
                Err(error) => Err(error),
            },
            global_format,
        ),
        Command::AddressInspect(args) => (
            commands::address_inspect(args, global_format).await,
            global_format,
        ),
        Command::AddressTimeline(args) => (
            match require_json_support(command_name, global_format) {
                Ok(()) => commands::address_timeline(args)
                    .await
                    .map_err(CommandError::from),
                Err(error) => Err(error),
            },
            global_format,
        ),
        Command::AddressRoundtrips(args) => (
            match require_json_support(command_name, global_format) {
                Ok(()) => commands::address_roundtrips(args)
                    .await
                    .map_err(CommandError::from),
                Err(error) => Err(error),
            },
            global_format,
        ),
        Command::AddressFeatures(args) => (
            commands::address_features_json(args, global_format).await,
            global_format,
        ),
        Command::WalletDossier(args) => (
            commands::wallet_dossier(args, global_format).await,
            global_format,
        ),
        Command::MintShardSummary(args) => (
            commands::mint_shard_summary(args, global_format).await,
            global_format,
        ),
        Command::AddressBrief(args) => {
            let effective_format = if args.json {
                OutputFormat::Json
            } else {
                global_format
            };
            (
                commands::address_brief(args, effective_format).await,
                effective_format,
            )
        }
        Command::CloneReport(args) => {
            let effective_format = if args.json {
                OutputFormat::Json
            } else {
                global_format
            };
            (
                commands::clone_report(args, effective_format).await,
                effective_format,
            )
        }
        Command::ExplainWhy(args) => (
            commands::explain_why(args, global_format).await,
            global_format,
        ),
        Command::SuggestNextExperiment(args) => (
            commands::suggest_next_experiment(args, global_format).await,
            global_format,
        ),
        Command::CloneEval(args) => {
            let effective_format = if args.json {
                OutputFormat::Json
            } else {
                global_format
            };
            (
                commands::clone_eval(args, effective_format).await,
                effective_format,
            )
        }
        Command::CloneRank(args) => {
            let effective_format = if args.json {
                OutputFormat::Json
            } else {
                global_format
            };
            (
                commands::clone_rank(args, effective_format).await,
                effective_format,
            )
        }
        Command::InferStrategy(args) => (
            commands::infer_strategy_json(args, global_format).await,
            global_format,
        ),
        Command::FitParams(args) => (
            commands::fit_params_json(args, global_format).await,
            global_format,
        ),
        Command::AddressExport(args) => (
            match require_json_support(command_name, global_format) {
                Ok(()) => commands::address_export(args)
                    .await
                    .map_err(CommandError::from),
                Err(error) => Err(error),
            },
            global_format,
        ),
        Command::Ingest(args) => (
            match require_json_support(command_name, global_format) {
                Ok(()) => commands::ingest(args).await.map_err(CommandError::from),
                Err(error) => Err(error),
            },
            global_format,
        ),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => output::render_error(command_name, effective_format, error, started),
    }
}
