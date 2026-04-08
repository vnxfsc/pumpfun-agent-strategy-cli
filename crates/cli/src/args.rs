use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "pump-agent")]
#[command(
    about = "Replay Pump mint/trade events, ingest Yellowstone streams, and run local paper strategies"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Replay(ReplayArgs),
    ReplayDb(ReplayDbArgs),
    SweepDb(SweepDbArgs),
    LivePaper(LivePaperArgs),
    ServeDashboard(ServeDashboardArgs),
    StrategyScaffold(StrategyScaffoldArgs),
    CloneScaffold(CloneScaffoldArgs),
    Stats(DbArgs),
    Runs(RunsArgs),
    RunInspect(RunInspectArgs),
    CompareRuns(CompareRunsArgs),
    SweepBatchInspect(SweepBatchInspectArgs),
    Tail(TailArgs),
    MintInspect(MintInspectArgs),
    AddressInspect(AddressInspectArgs),
    AddressTimeline(AddressTimelineArgs),
    AddressRoundtrips(AddressRoundtripsArgs),
    AddressFeatures(AddressFeaturesArgs),
    AddressBrief(AddressBriefArgs),
    CloneReport(CloneReportArgs),
    CloneEval(CloneEvalArgs),
    CloneRank(CloneRankArgs),
    InferStrategy(InferStrategyArgs),
    FitParams(FitParamsArgs),
    AddressExport(AddressExportArgs),
    Ingest(IngestArgs),
}

#[derive(Debug, Parser, Clone, serde::Serialize, serde::Deserialize)]
pub struct StrategyArgs {
    #[arg(long, default_value = "momentum")]
    pub strategy: String,

    #[arg(long)]
    pub strategy_config: Option<PathBuf>,

    #[arg(long, default_value_t = 10.0)]
    pub starting_sol: f64,

    #[arg(long, default_value_t = 0.2)]
    pub buy_sol: f64,

    #[arg(long, default_value_t = 45)]
    pub max_age_secs: i64,

    #[arg(long, default_value_t = 3)]
    pub min_buy_count: u64,

    #[arg(long, default_value_t = 3)]
    pub min_unique_buyers: usize,

    #[arg(long, default_value_t = 0.3)]
    pub min_net_buy_sol: f64,

    #[arg(long, default_value_t = 2500)]
    pub take_profit_bps: i64,

    #[arg(long, default_value_t = 1200)]
    pub stop_loss_bps: i64,

    #[arg(long, default_value_t = 90)]
    pub max_hold_secs: i64,

    #[arg(long, default_value_t = 0.8)]
    pub min_total_buy_sol: f64,

    #[arg(long, default_value_t = 1)]
    pub max_sell_count: u64,

    #[arg(long, default_value_t = 4.0)]
    pub min_buy_sell_ratio: f64,

    #[arg(long, default_value_t = 3)]
    pub max_concurrent_positions: usize,

    #[arg(long, default_value_t = 3)]
    pub exit_on_sell_count: u64,

    #[arg(long, default_value_t = 100)]
    pub trading_fee_bps: u64,

    #[arg(long, default_value_t = 50)]
    pub slippage_bps: u64,
}

#[derive(Debug, Parser)]
pub struct ReplayArgs {
    #[arg(long)]
    pub input: PathBuf,

    #[arg(long)]
    pub save_run: bool,

    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[command(flatten)]
    pub strategy: StrategyArgs,
}

#[derive(Debug, Parser)]
pub struct ReplayDbArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub save_run: bool,

    #[command(flatten)]
    pub strategy: StrategyArgs,
}

#[derive(Debug, Parser)]
pub struct SweepDbArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long, default_value_t = 10)]
    pub top: usize,

    #[arg(long)]
    pub buy_sol_values: Option<String>,

    #[arg(long)]
    pub max_age_secs_values: Option<String>,

    #[arg(long)]
    pub min_buy_count_values: Option<String>,

    #[arg(long)]
    pub min_unique_buyers_values: Option<String>,

    #[arg(long)]
    pub min_total_buy_sol_values: Option<String>,

    #[arg(long)]
    pub max_sell_count_values: Option<String>,

    #[arg(long)]
    pub min_buy_sell_ratio_values: Option<String>,

    #[arg(long)]
    pub take_profit_bps_values: Option<String>,

    #[arg(long)]
    pub stop_loss_bps_values: Option<String>,

    #[arg(long)]
    pub max_concurrent_positions_values: Option<String>,

    #[arg(long)]
    pub exit_on_sell_count_values: Option<String>,

    #[command(flatten)]
    pub strategy: StrategyArgs,
}

#[derive(Debug, Parser)]
pub struct DbArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,
}

#[derive(Debug, Parser)]
pub struct TailArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long, default_value_t = 20)]
    pub limit: i64,
}

#[derive(Debug, Parser)]
pub struct RunsArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long, default_value_t = 20)]
    pub limit: i64,
}

#[derive(Debug, Parser)]
pub struct RunInspectArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub id: i64,

    #[arg(long, default_value_t = 50)]
    pub fill_limit: i64,
}

#[derive(Debug, Parser)]
pub struct CompareRunsArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub left_id: i64,

    #[arg(long)]
    pub right_id: i64,

    #[arg(long, default_value_t = 20)]
    pub fill_limit: i64,
}

#[derive(Debug, Parser)]
pub struct SweepBatchInspectArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub batch_id: String,
}

#[derive(Debug, Parser)]
pub struct MintInspectArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub mint: String,

    #[arg(long, default_value_t = 20)]
    pub limit: i64,
}

#[derive(Debug, Parser)]
pub struct AddressInspectArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub address: String,

    #[arg(long, default_value_t = 10)]
    pub top_mints_limit: i64,

    #[arg(long, default_value_t = 10)]
    pub roundtrip_limit: i64,
}

#[derive(Debug, Parser)]
pub struct AddressTimelineArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub address: String,

    #[arg(long, default_value_t = 100)]
    pub limit: i64,

    #[arg(long, default_value_t = false)]
    pub ascending: bool,
}

#[derive(Debug, Parser)]
pub struct AddressRoundtripsArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub address: String,

    #[arg(long, default_value_t = 50)]
    pub limit: i64,
}

#[derive(Debug, Parser)]
pub struct AddressFeaturesArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub address: String,

    #[arg(long, default_value_t = 10)]
    pub sample_limit: usize,
}

#[derive(Debug, Parser)]
pub struct AddressBriefArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub address: String,

    #[arg(long, default_value_t = false)]
    pub export: bool,

    #[arg(long, default_value = "./exports")]
    pub export_root: PathBuf,

    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Debug, Parser)]
pub struct CloneReportArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub address: String,

    #[arg(long, default_value_t = false)]
    pub export: bool,

    #[arg(long, default_value = "./exports")]
    pub export_root: PathBuf,

    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Debug, Parser)]
pub struct CloneEvalArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub address: String,

    #[arg(long)]
    pub run_id: Option<i64>,

    #[arg(long, default_value_t = false)]
    pub json: bool,

    #[command(flatten)]
    pub strategy: StrategyArgs,
}

#[derive(Debug, Parser)]
pub struct CloneRankArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub address: String,

    #[arg(long, default_value_t = 50)]
    pub scan_limit: i64,

    #[arg(long, default_value_t = 10)]
    pub top: usize,

    #[arg(long, default_value_t = false)]
    pub json: bool,
}

#[derive(Debug, Parser)]
pub struct InferStrategyArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub address: String,

    #[arg(long)]
    pub family: Option<String>,
}

#[derive(Debug, Parser)]
pub struct FitParamsArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub address: String,

    #[arg(long, default_value = "early_flow")]
    pub family: String,

    #[arg(long, default_value_t = 10)]
    pub top: usize,

    #[arg(long)]
    pub buy_sol_values: Option<String>,

    #[arg(long)]
    pub max_age_secs_values: Option<String>,

    #[arg(long)]
    pub min_buy_count_values: Option<String>,

    #[arg(long)]
    pub min_unique_buyers_values: Option<String>,

    #[arg(long)]
    pub min_total_buy_sol_values: Option<String>,

    #[arg(long)]
    pub max_sell_count_values: Option<String>,

    #[arg(long)]
    pub min_buy_sell_ratio_values: Option<String>,

    #[arg(long)]
    pub take_profit_bps_values: Option<String>,

    #[arg(long)]
    pub stop_loss_bps_values: Option<String>,

    #[arg(long)]
    pub max_concurrent_positions_values: Option<String>,

    #[arg(long)]
    pub exit_on_sell_count_values: Option<String>,

    #[command(flatten)]
    pub strategy: StrategyArgs,
}

#[derive(Debug, Parser)]
pub struct AddressExportArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub address: String,

    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Debug, Parser)]
pub struct IngestArgs {
    #[arg(long)]
    pub endpoint: Option<String>,

    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long)]
    pub x_token: Option<String>,

    #[arg(long)]
    pub commitment: Option<String>,

    #[arg(long, default_value_t = false)]
    pub apply_schema: bool,

    #[arg(long, default_value_t = false)]
    pub x_request_snapshot: bool,

    #[arg(long)]
    pub max_decoding_message_size: Option<usize>,

    #[arg(long)]
    pub connect_timeout_secs: Option<u64>,

    #[arg(long)]
    pub request_timeout_secs: Option<u64>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub heartbeat_secs: Option<u64>,

    #[arg(long)]
    pub reconnect_delay_secs: Option<u64>,

    #[arg(long)]
    pub http2_keep_alive_interval_secs: Option<u64>,

    #[arg(long)]
    pub keep_alive_timeout_secs: Option<u64>,

    #[arg(long)]
    pub tcp_keepalive_secs: Option<u64>,

    #[arg(long, default_value_t = false)]
    pub resume_from_db: bool,
}

#[derive(Debug, Parser)]
pub struct LivePaperArgs {
    #[arg(long)]
    pub endpoint: Option<String>,

    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long)]
    pub x_token: Option<String>,

    #[arg(long)]
    pub commitment: Option<String>,

    #[arg(long, default_value_t = false)]
    pub apply_schema: bool,

    #[arg(long, default_value_t = false)]
    pub x_request_snapshot: bool,

    #[arg(long)]
    pub max_decoding_message_size: Option<usize>,

    #[arg(long)]
    pub connect_timeout_secs: Option<u64>,

    #[arg(long)]
    pub request_timeout_secs: Option<u64>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub heartbeat_secs: Option<u64>,

    #[arg(long)]
    pub reconnect_delay_secs: Option<u64>,

    #[arg(long)]
    pub http2_keep_alive_interval_secs: Option<u64>,

    #[arg(long)]
    pub keep_alive_timeout_secs: Option<u64>,

    #[arg(long)]
    pub tcp_keepalive_secs: Option<u64>,

    #[arg(long, default_value_t = false)]
    pub save_run: bool,

    #[arg(long)]
    pub execution_jsonl: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub persist_events: bool,

    #[arg(long, default_value_t = false)]
    pub resume_from_db: bool,

    #[arg(long, default_value_t = 100)]
    pub summary_every_events: u64,

    #[arg(long, default_value_t = 5)]
    pub dashboard_position_limit: usize,

    #[arg(long, default_value_t = 8)]
    pub dashboard_activity_limit: usize,

    #[command(flatten)]
    pub strategy: StrategyArgs,
}

#[derive(Debug, Parser)]
pub struct ServeDashboardArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    #[arg(long, default_value_t = 3000)]
    pub port: u16,
}

#[derive(Debug, Parser)]
pub struct StrategyScaffoldArgs {
    #[arg(long)]
    pub name: Option<String>,

    #[arg(long, default_value = "early_flow")]
    pub strategy: String,

    #[arg(long)]
    pub output: PathBuf,

    #[arg(long, default_value_t = false)]
    pub force: bool,
}

#[derive(Debug, Parser)]
pub struct CloneScaffoldArgs {
    #[arg(long)]
    pub database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    pub max_db_connections: u32,

    #[arg(long)]
    pub address: String,

    #[arg(long)]
    pub name: Option<String>,

    #[arg(long)]
    pub output: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub export: bool,

    #[arg(long, default_value = "./exports")]
    pub export_root: PathBuf,

    #[arg(long, default_value_t = false)]
    pub force: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct StrategyFile {
    pub strategy: StrategyFileStrategy,
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct StrategyFileStrategy {
    pub strategy: Option<String>,
    pub starting_sol: Option<f64>,
    pub buy_sol: Option<f64>,
    pub max_age_secs: Option<i64>,
    pub min_buy_count: Option<u64>,
    pub min_unique_buyers: Option<usize>,
    pub min_net_buy_sol: Option<f64>,
    pub take_profit_bps: Option<i64>,
    pub stop_loss_bps: Option<i64>,
    pub max_hold_secs: Option<i64>,
    pub min_total_buy_sol: Option<f64>,
    pub max_sell_count: Option<u64>,
    pub min_buy_sell_ratio: Option<f64>,
    pub max_concurrent_positions: Option<usize>,
    pub exit_on_sell_count: Option<u64>,
    pub trading_fee_bps: Option<u64>,
    pub slippage_bps: Option<u64>,
}
