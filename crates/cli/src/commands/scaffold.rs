use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{Result, bail};
use pump_agent_app::api::{
    CloneReportExportSummary, CloneReportOutput, ParamsSeed, build_clone_report,
};
use pump_agent_core::StrategyKind;

use crate::{
    args::{CloneScaffoldArgs, StrategyScaffoldArgs},
    commands::{
        helpers::SCHEMA_SQL,
        inspect::{
            export::{export_address_events, print_export_summary},
            report::analyze_clone_candidates,
        },
    },
    config::required_config,
};

const MOD_MARKER: &str = "// strategy-scaffold: mod";
const PUB_USE_MARKER: &str = "// strategy-scaffold: pub-use";
const KIND_VARIANT_MARKER: &str = "// strategy-scaffold: kind-variant";
const KIND_AS_STR_MARKER: &str = "// strategy-scaffold: kind-as-str";
const KIND_FROM_STR_MARKER: &str = "// strategy-scaffold: kind-from-str";
const ANY_STRATEGY_VARIANT_MARKER: &str = "// strategy-scaffold: any-strategy-variant";
const METADATA_ARM_MARKER: &str = "// strategy-scaffold: metadata-arm";
const ON_EVENT_ARM_MARKER: &str = "// strategy-scaffold: on-event-arm";
const ON_EXECUTION_ARM_MARKER: &str = "// strategy-scaffold: on-execution-arm";
const RUNTIME_MATCH_MARKER: &str = "// strategy-scaffold: runtime-match";
const LIB_PUB_USE_MARKER: &str = "// strategy-scaffold: lib-pub-use";

pub async fn strategy_scaffold(args: StrategyScaffoldArgs) -> Result<()> {
    if args.output.exists() && !args.force {
        bail!(
            "output already exists: {} (use --force to overwrite)",
            args.output.display()
        );
    }

    let template_kind = StrategyKind::from_str(&args.strategy)
        .map_err(|error| anyhow::anyhow!("invalid --strategy '{}': {}", args.strategy, error))?;

    if let Some(name) = &args.name {
        let generated = GeneratedStrategy::new(name)?;
        scaffold_strategy_code(&generated, template_kind, args.force)?;
        let content = render_strategy_config_template(template_kind, &generated.module_name);
        ensure_parent_dir(&args.output)?;
        fs::write(&args.output, content)?;
        println!(
            "wrote strategy code scaffold template={} name={} module={} config={}",
            template_kind,
            generated.strategy_type,
            generated.module_path().display(),
            args.output.display()
        );
        return Ok(());
    }

    let content = render_strategy_config_template(template_kind, template_kind.as_str());
    ensure_parent_dir(&args.output)?;
    fs::write(&args.output, content)?;

    println!(
        "wrote strategy scaffold strategy={} path={}",
        template_kind,
        args.output.display()
    );
    Ok(())
}

pub async fn clone_scaffold(args: CloneScaffoldArgs) -> Result<()> {
    let database_url = required_config(args.database_url.clone(), "DATABASE_URL")?;
    let store =
        pump_agent_core::PgEventStore::connect(&database_url, args.max_db_connections).await?;
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

    let strategy_name = args
        .name
        .clone()
        .unwrap_or_else(|| report.recommended_next_strategy_name.clone());
    let generated = GeneratedStrategy::new(&strategy_name)?;
    let template_kind = StrategyKind::from_str(&report.recommended_base_family)
        .map_err(|error| anyhow::anyhow!("invalid recommended base family: {}", error))?;
    let output = args.output.unwrap_or_else(|| {
        repo_root()
            .join("strategies")
            .join(format!("{}.toml", strategy_name.replace('_', "-")))
    });

    if output.exists() && !args.force {
        bail!(
            "output already exists: {} (use --force to overwrite)",
            output.display()
        );
    }

    scaffold_strategy_code(&generated, template_kind, args.force)?;
    let content = render_seeded_strategy_config(template_kind, &generated.module_name, &report);
    ensure_parent_dir(&output)?;
    fs::write(&output, content)?;

    println!(
        "wrote clone scaffold address={} base_family={} recommended={} module={} config={}",
        args.address,
        report.recommended_base_family,
        generated.strategy_type,
        generated.module_path().display(),
        output.display()
    );
    println!(
        "params seed       : buy_sol={:.3} max_age_secs={} min_buy_count={} min_unique_buyers={} min_total_buy_sol={:.3}",
        report.recommended_params_seed.buy_sol,
        report.recommended_params_seed.max_age_secs,
        report.recommended_params_seed.min_buy_count,
        report.recommended_params_seed.min_unique_buyers,
        report.recommended_params_seed.min_total_buy_sol,
    );
    if let Some(summary) = export_summary {
        println!();
        print_export_summary(&summary);
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct GeneratedStrategy {
    module_name: String,
    variant_name: String,
    strategy_type: String,
    config_type: String,
}

impl GeneratedStrategy {
    fn new(raw_name: &str) -> Result<Self> {
        validate_module_name(raw_name)?;
        let type_prefix = to_upper_camel_case(raw_name);
        Ok(Self {
            module_name: raw_name.to_string(),
            variant_name: type_prefix.clone(),
            strategy_type: format!("{type_prefix}Strategy"),
            config_type: format!("{type_prefix}StrategyConfig"),
        })
    }

    fn module_path(&self) -> PathBuf {
        repo_root()
            .join("crates/core/src/strategy")
            .join(format!("{}.rs", self.module_name))
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should exist")
        .to_path_buf()
}

fn validate_module_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        bail!("strategy name cannot be empty");
    };
    if !(first.is_ascii_lowercase() || first == '_') {
        bail!("strategy name must start with a lowercase letter or underscore");
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        bail!("strategy name must use snake_case ascii characters");
    }
    Ok(())
}

fn to_upper_camel_case(name: &str) -> String {
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let first = chars
                .next()
                .map(|ch| ch.to_ascii_uppercase())
                .unwrap_or_default();
            let rest = chars.collect::<String>();
            format!("{first}{rest}")
        })
        .collect::<String>()
}

fn scaffold_strategy_code(
    generated: &GeneratedStrategy,
    template_kind: StrategyKind,
    force: bool,
) -> Result<()> {
    let module_path = generated.module_path();
    if module_path.exists() && !force {
        bail!(
            "strategy module already exists: {} (use --force to overwrite)",
            module_path.display()
        );
    }

    let strategy_mod_path = repo_root().join("crates/core/src/strategy/mod.rs");
    let runtime_strategy_path = repo_root().join("crates/cli/src/runtime/strategy.rs");
    let core_lib_path = repo_root().join("crates/core/src/lib.rs");

    let strategy_mod = fs::read_to_string(&strategy_mod_path)?;
    let runtime_strategy = fs::read_to_string(&runtime_strategy_path)?;
    let core_lib = fs::read_to_string(&core_lib_path)?;

    let updated_strategy_mod = register_core_strategy_module(&strategy_mod, generated)?;
    let updated_runtime_strategy =
        register_runtime_strategy_builder(&runtime_strategy, generated, template_kind)?;
    let updated_core_lib = register_core_lib_exports(&core_lib, generated)?;

    ensure_parent_dir(&module_path)?;
    fs::write(
        &module_path,
        render_strategy_code_template(template_kind, generated),
    )?;
    fs::write(&strategy_mod_path, updated_strategy_mod)?;
    fs::write(&runtime_strategy_path, updated_runtime_strategy)?;
    fs::write(&core_lib_path, updated_core_lib)?;
    Ok(())
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn render_strategy_config_template(kind: StrategyKind, strategy_name: &str) -> String {
    match kind {
        StrategyKind::Momentum => format!(
            r#"# Generated by `pump-agent-cli strategy-scaffold`
# Keep one file per strategy config under `strategies/`.

[strategy]
strategy = "{strategy_name}"
starting_sol = 10.0
buy_sol = 0.2
max_age_secs = 45
min_buy_count = 3
min_unique_buyers = 3
min_net_buy_sol = 0.3
take_profit_bps = 2500
stop_loss_bps = 1200
max_hold_secs = 90
trading_fee_bps = 100
slippage_bps = 50
"#
        ),
        StrategyKind::EarlyFlow => format!(
            r#"# Generated by `pump-agent-cli strategy-scaffold`
# Keep one file per strategy config under `strategies/`.

[strategy]
strategy = "{strategy_name}"
starting_sol = 10.0
buy_sol = 0.15
max_age_secs = 20
min_buy_count = 4
min_unique_buyers = 4
min_total_buy_sol = 0.8
max_sell_count = 1
min_buy_sell_ratio = 4.0
take_profit_bps = 1800
stop_loss_bps = 900
max_hold_secs = 45
max_concurrent_positions = 3
exit_on_sell_count = 3
trading_fee_bps = 100
slippage_bps = 50
"#
        ),
        StrategyKind::Breakout => format!(
            r#"# Generated by `pump-agent-cli strategy-scaffold`
# Keep one file per strategy config under `strategies/`.

[strategy]
strategy = "{strategy_name}"
starting_sol = 10.0
buy_sol = 0.18
max_age_secs = 35
min_buy_count = 5
min_unique_buyers = 5
min_total_buy_sol = 1.2
min_net_buy_sol = 0.7
max_sell_count = 2
min_buy_sell_ratio = 3.5
take_profit_bps = 2200
stop_loss_bps = 900
max_hold_secs = 75
max_concurrent_positions = 3
exit_on_sell_count = 4
trading_fee_bps = 100
slippage_bps = 50
"#
        ),
        StrategyKind::LiquidityFollow => format!(
            r#"# Generated by `pump-agent-cli strategy-scaffold`
# Keep one file per strategy config under `strategies/`.

[strategy]
strategy = "{strategy_name}"
starting_sol = 10.0
buy_sol = 0.18
max_age_secs = 55
min_buy_count = 4
min_unique_buyers = 4
min_total_buy_sol = 1.5
min_net_buy_sol = 0.5
max_sell_count = 3
min_buy_sell_ratio = 2.5
take_profit_bps = 2000
stop_loss_bps = 1000
max_hold_secs = 120
max_concurrent_positions = 4
exit_on_sell_count = 4
trading_fee_bps = 100
slippage_bps = 50
"#
        ),
        StrategyKind::Noop => format!(
            r#"# Generated by `pump-agent-cli strategy-scaffold`
# Useful as a baseline or plumbing test.

[strategy]
strategy = "{strategy_name}"
starting_sol = 10.0
trading_fee_bps = 100
slippage_bps = 50
"#
        ),
    }
}

fn render_seeded_strategy_config(
    kind: StrategyKind,
    strategy_name: &str,
    report: &CloneReportOutput,
) -> String {
    match kind {
        StrategyKind::Momentum => render_momentum_seeded_config(
            strategy_name,
            &report.address,
            &report.recommended_base_family,
            &report.recommended_params_seed,
            &report.confirmed_rules,
        ),
        StrategyKind::EarlyFlow => render_early_flow_seeded_config(
            strategy_name,
            &report.address,
            &report.recommended_base_family,
            &report.recommended_params_seed,
            &report.confirmed_rules,
        ),
        StrategyKind::Breakout => render_breakout_seeded_config(
            strategy_name,
            &report.address,
            &report.recommended_base_family,
            &report.recommended_params_seed,
            &report.confirmed_rules,
        ),
        StrategyKind::LiquidityFollow => render_liquidity_follow_seeded_config(
            strategy_name,
            &report.address,
            &report.recommended_base_family,
            &report.recommended_params_seed,
            &report.confirmed_rules,
        ),
        StrategyKind::Noop => render_strategy_config_template(kind, strategy_name),
    }
}

fn render_momentum_seeded_config(
    strategy_name: &str,
    address: &str,
    base_family: &str,
    params: &ParamsSeed,
    confirmed_rules: &[String],
) -> String {
    let mut output = render_clone_header(address, base_family, confirmed_rules);
    output.push_str(&format!(
        r#"[strategy]
strategy = "{strategy_name}"
starting_sol = 10.0
buy_sol = {buy_sol:.3}
max_age_secs = {max_age_secs}
min_buy_count = {min_buy_count}
min_unique_buyers = {min_unique_buyers}
min_net_buy_sol = {min_net_buy_sol:.3}
take_profit_bps = {take_profit_bps}
stop_loss_bps = {stop_loss_bps}
max_hold_secs = {max_hold_secs}
trading_fee_bps = 100
slippage_bps = 50
"#,
        buy_sol = params.buy_sol,
        max_age_secs = params.max_age_secs,
        min_buy_count = params.min_buy_count,
        min_unique_buyers = params.min_unique_buyers,
        min_net_buy_sol = params.min_net_buy_sol,
        take_profit_bps = params.take_profit_bps,
        stop_loss_bps = params.stop_loss_bps,
        max_hold_secs = params.max_hold_secs,
    ));
    output
}

fn render_early_flow_seeded_config(
    strategy_name: &str,
    address: &str,
    base_family: &str,
    params: &ParamsSeed,
    confirmed_rules: &[String],
) -> String {
    let mut output = render_clone_header(address, base_family, confirmed_rules);
    output.push_str(&format!(
        r#"[strategy]
strategy = "{strategy_name}"
starting_sol = 10.0
buy_sol = {buy_sol:.3}
max_age_secs = {max_age_secs}
min_buy_count = {min_buy_count}
min_unique_buyers = {min_unique_buyers}
min_total_buy_sol = {min_total_buy_sol:.3}
max_sell_count = {max_sell_count}
min_buy_sell_ratio = {min_buy_sell_ratio:.2}
take_profit_bps = {take_profit_bps}
stop_loss_bps = {stop_loss_bps}
max_hold_secs = {max_hold_secs}
max_concurrent_positions = {max_concurrent_positions}
exit_on_sell_count = {exit_on_sell_count}
trading_fee_bps = 100
slippage_bps = 50
"#,
        buy_sol = params.buy_sol,
        max_age_secs = params.max_age_secs,
        min_buy_count = params.min_buy_count,
        min_unique_buyers = params.min_unique_buyers,
        min_total_buy_sol = params.min_total_buy_sol,
        max_sell_count = params.max_sell_count,
        min_buy_sell_ratio = params.min_buy_sell_ratio,
        take_profit_bps = params.take_profit_bps,
        stop_loss_bps = params.stop_loss_bps,
        max_hold_secs = params.max_hold_secs,
        max_concurrent_positions = params.max_concurrent_positions,
        exit_on_sell_count = params.exit_on_sell_count,
    ));
    output
}

fn render_breakout_seeded_config(
    strategy_name: &str,
    address: &str,
    base_family: &str,
    params: &ParamsSeed,
    confirmed_rules: &[String],
) -> String {
    let mut output = render_clone_header(address, base_family, confirmed_rules);
    output.push_str(&format!(
        r#"[strategy]
strategy = "{strategy_name}"
starting_sol = 10.0
buy_sol = {buy_sol:.3}
max_age_secs = {max_age_secs}
min_buy_count = {min_buy_count}
min_unique_buyers = {min_unique_buyers}
min_total_buy_sol = {min_total_buy_sol:.3}
min_net_buy_sol = {min_net_buy_sol:.3}
max_sell_count = {max_sell_count}
min_buy_sell_ratio = {min_buy_sell_ratio:.2}
take_profit_bps = {take_profit_bps}
stop_loss_bps = {stop_loss_bps}
max_hold_secs = {max_hold_secs}
max_concurrent_positions = {max_concurrent_positions}
exit_on_sell_count = {exit_on_sell_count}
trading_fee_bps = 100
slippage_bps = 50
"#,
        buy_sol = params.buy_sol,
        max_age_secs = params.max_age_secs,
        min_buy_count = params.min_buy_count,
        min_unique_buyers = params.min_unique_buyers,
        min_total_buy_sol = params.min_total_buy_sol,
        min_net_buy_sol = params.min_net_buy_sol,
        max_sell_count = params.max_sell_count,
        min_buy_sell_ratio = params.min_buy_sell_ratio,
        take_profit_bps = params.take_profit_bps,
        stop_loss_bps = params.stop_loss_bps,
        max_hold_secs = params.max_hold_secs,
        max_concurrent_positions = params.max_concurrent_positions,
        exit_on_sell_count = params.exit_on_sell_count,
    ));
    output
}

fn render_liquidity_follow_seeded_config(
    strategy_name: &str,
    address: &str,
    base_family: &str,
    params: &ParamsSeed,
    confirmed_rules: &[String],
) -> String {
    let mut output = render_clone_header(address, base_family, confirmed_rules);
    output.push_str(&format!(
        r#"[strategy]
strategy = "{strategy_name}"
starting_sol = 10.0
buy_sol = {buy_sol:.3}
max_age_secs = {max_age_secs}
min_buy_count = {min_buy_count}
min_unique_buyers = {min_unique_buyers}
min_total_buy_sol = {min_total_buy_sol:.3}
min_net_buy_sol = {min_net_buy_sol:.3}
max_sell_count = {max_sell_count}
min_buy_sell_ratio = {min_buy_sell_ratio:.2}
take_profit_bps = {take_profit_bps}
stop_loss_bps = {stop_loss_bps}
max_hold_secs = {max_hold_secs}
max_concurrent_positions = {max_concurrent_positions}
exit_on_sell_count = {exit_on_sell_count}
trading_fee_bps = 100
slippage_bps = 50
"#,
        buy_sol = params.buy_sol,
        max_age_secs = params.max_age_secs,
        min_buy_count = params.min_buy_count,
        min_unique_buyers = params.min_unique_buyers,
        min_total_buy_sol = params.min_total_buy_sol,
        min_net_buy_sol = params.min_net_buy_sol,
        max_sell_count = params.max_sell_count,
        min_buy_sell_ratio = params.min_buy_sell_ratio,
        take_profit_bps = params.take_profit_bps,
        stop_loss_bps = params.stop_loss_bps,
        max_hold_secs = params.max_hold_secs,
        max_concurrent_positions = params.max_concurrent_positions,
        exit_on_sell_count = params.exit_on_sell_count,
    ));
    output
}

fn render_clone_header(address: &str, base_family: &str, confirmed_rules: &[String]) -> String {
    let mut output = String::from("# Generated by `pump-agent-cli clone-scaffold`\n");
    output.push_str(&format!("# Source address: {address}\n"));
    output.push_str(&format!("# Recommended base family: {base_family}\n"));
    for rule in confirmed_rules.iter().take(3) {
        output.push_str(&format!("# Signal: {rule}\n"));
    }
    output.push('\n');
    output
}

fn render_strategy_code_template(kind: StrategyKind, generated: &GeneratedStrategy) -> String {
    let metadata_name = format!("{}_strategy", generated.module_name);
    match kind {
        StrategyKind::Momentum => format!(
            r#"use crate::{{
    broker::BrokerSnapshot,
    model::{{EventEnvelope, ExecutionReport, OrderRequest}},
    state::MarketState,
}};

use super::{{Strategy, StrategyMetadata}};

#[derive(Debug, Clone)]
pub struct {config_type} {{
    pub max_age_secs: i64,
    pub min_buy_count: u64,
    pub min_unique_buyers: usize,
    pub min_net_flow_lamports: i128,
    pub buy_lamports: u64,
    pub take_profit_bps: i64,
    pub stop_loss_bps: i64,
    pub max_hold_secs: i64,
}}

impl Default for {config_type} {{
    fn default() -> Self {{
        Self {{
            max_age_secs: 45,
            min_buy_count: 3,
            min_unique_buyers: 3,
            min_net_flow_lamports: 300_000_000,
            buy_lamports: 200_000_000,
            take_profit_bps: 2500,
            stop_loss_bps: 1200,
            max_hold_secs: 90,
        }}
    }}
}}

#[derive(Debug, Clone)]
pub struct {strategy_type} {{
    config: {config_type},
}}

impl {strategy_type} {{
    pub fn new(config: {config_type}) -> Self {{
        Self {{ config }}
    }}
}}

impl Strategy for {strategy_type} {{
    fn metadata(&self) -> StrategyMetadata {{
        StrategyMetadata {{
            name: "{metadata_name}",
        }}
    }}

    fn on_event(
        &mut self,
        event: &EventEnvelope,
        market_state: &MarketState,
        broker: &BrokerSnapshot,
    ) -> Vec<OrderRequest> {{
        let _ = (&self.config, event, market_state, broker);
        Vec::new()
    }}

    fn on_execution_reports(
        &mut self,
        reports: &[ExecutionReport],
        market_state: &MarketState,
        broker: &BrokerSnapshot,
    ) {{
        let _ = (&self.config, reports, market_state, broker);
    }}
}}
"#,
            config_type = generated.config_type,
            strategy_type = generated.strategy_type,
            metadata_name = metadata_name,
        ),
        StrategyKind::EarlyFlow => format!(
            r#"use crate::{{
    broker::BrokerSnapshot,
    model::{{EventEnvelope, ExecutionReport, OrderRequest}},
    state::MarketState,
}};

use super::{{Strategy, StrategyMetadata}};

#[derive(Debug, Clone)]
pub struct {config_type} {{
    pub max_age_secs: i64,
    pub min_buy_count: u64,
    pub min_unique_buyers: usize,
    pub min_total_buy_lamports: u128,
    pub max_sell_count: u64,
    pub min_buy_sell_ratio: f64,
    pub buy_lamports: u64,
    pub take_profit_bps: i64,
    pub stop_loss_bps: i64,
    pub max_hold_secs: i64,
    pub max_concurrent_positions: usize,
    pub exit_on_sell_count: u64,
}}

impl Default for {config_type} {{
    fn default() -> Self {{
        Self {{
            max_age_secs: 20,
            min_buy_count: 4,
            min_unique_buyers: 4,
            min_total_buy_lamports: 800_000_000,
            max_sell_count: 1,
            min_buy_sell_ratio: 4.0,
            buy_lamports: 150_000_000,
            take_profit_bps: 1800,
            stop_loss_bps: 900,
            max_hold_secs: 45,
            max_concurrent_positions: 3,
            exit_on_sell_count: 3,
        }}
    }}
}}

#[derive(Debug, Clone)]
pub struct {strategy_type} {{
    config: {config_type},
}}

impl {strategy_type} {{
    pub fn new(config: {config_type}) -> Self {{
        Self {{ config }}
    }}
}}

impl Strategy for {strategy_type} {{
    fn metadata(&self) -> StrategyMetadata {{
        StrategyMetadata {{
            name: "{metadata_name}",
        }}
    }}

    fn on_event(
        &mut self,
        event: &EventEnvelope,
        market_state: &MarketState,
        broker: &BrokerSnapshot,
    ) -> Vec<OrderRequest> {{
        let _ = (&self.config, event, market_state, broker);
        Vec::new()
    }}

    fn on_execution_reports(
        &mut self,
        reports: &[ExecutionReport],
        market_state: &MarketState,
        broker: &BrokerSnapshot,
    ) {{
        let _ = (&self.config, reports, market_state, broker);
    }}
}}
"#,
            config_type = generated.config_type,
            strategy_type = generated.strategy_type,
            metadata_name = metadata_name,
        ),
        StrategyKind::Breakout | StrategyKind::LiquidityFollow => format!(
            r#"use crate::{{
    broker::BrokerSnapshot,
    model::{{EventEnvelope, ExecutionReport, OrderRequest}},
    state::MarketState,
}};

use super::{{Strategy, StrategyMetadata}};

#[derive(Debug, Clone)]
pub struct {config_type} {{
    pub max_age_secs: i64,
    pub min_buy_count: u64,
    pub min_unique_buyers: usize,
    pub min_total_buy_lamports: u128,
    pub min_net_flow_lamports: i128,
    pub max_sell_count: u64,
    pub min_buy_sell_ratio: f64,
    pub buy_lamports: u64,
    pub take_profit_bps: i64,
    pub stop_loss_bps: i64,
    pub max_hold_secs: i64,
    pub max_concurrent_positions: usize,
    pub exit_on_sell_count: u64,
}}

impl Default for {config_type} {{
    fn default() -> Self {{
        Self {{
            max_age_secs: 30,
            min_buy_count: 4,
            min_unique_buyers: 4,
            min_total_buy_lamports: 1_000_000_000,
            min_net_flow_lamports: 500_000_000,
            max_sell_count: 2,
            min_buy_sell_ratio: 3.0,
            buy_lamports: 180_000_000,
            take_profit_bps: 2_000,
            stop_loss_bps: 1_000,
            max_hold_secs: 90,
            max_concurrent_positions: 3,
            exit_on_sell_count: 4,
        }}
    }}
}}

#[derive(Debug, Clone)]
pub struct {strategy_type} {{
    config: {config_type},
}}

impl {strategy_type} {{
    pub fn new(config: {config_type}) -> Self {{
        Self {{ config }}
    }}
}}

impl Strategy for {strategy_type} {{
    fn metadata(&self) -> StrategyMetadata {{
        StrategyMetadata {{
            name: "{metadata_name}",
        }}
    }}

    fn on_event(
        &mut self,
        event: &EventEnvelope,
        market_state: &MarketState,
        broker: &BrokerSnapshot,
    ) -> Vec<OrderRequest> {{
        let _ = (&self.config, event, market_state, broker);
        Vec::new()
    }}

    fn on_execution_reports(
        &mut self,
        reports: &[ExecutionReport],
        market_state: &MarketState,
        broker: &BrokerSnapshot,
    ) {{
        let _ = (&self.config, reports, market_state, broker);
    }}
}}
"#,
            config_type = generated.config_type,
            strategy_type = generated.strategy_type,
            metadata_name = metadata_name,
        ),
        StrategyKind::Noop => format!(
            r#"use crate::{{
    broker::BrokerSnapshot,
    model::{{EventEnvelope, ExecutionReport, OrderRequest}},
    state::MarketState,
}};

use super::{{Strategy, StrategyMetadata}};

#[derive(Debug, Clone, Default)]
pub struct {config_type};

#[derive(Debug, Clone)]
pub struct {strategy_type} {{
    config: {config_type},
}}

impl {strategy_type} {{
    pub fn new(config: {config_type}) -> Self {{
        Self {{ config }}
    }}
}}

impl Strategy for {strategy_type} {{
    fn metadata(&self) -> StrategyMetadata {{
        StrategyMetadata {{
            name: "{metadata_name}",
        }}
    }}

    fn on_event(
        &mut self,
        event: &EventEnvelope,
        market_state: &MarketState,
        broker: &BrokerSnapshot,
    ) -> Vec<OrderRequest> {{
        let _ = (&self.config, event, market_state, broker);
        Vec::new()
    }}

    fn on_execution_reports(
        &mut self,
        reports: &[ExecutionReport],
        market_state: &MarketState,
        broker: &BrokerSnapshot,
    ) {{
        let _ = (&self.config, reports, market_state, broker);
    }}
}}
"#,
            config_type = generated.config_type,
            strategy_type = generated.strategy_type,
            metadata_name = metadata_name,
        ),
    }
}

fn register_core_strategy_module(contents: &str, generated: &GeneratedStrategy) -> Result<String> {
    let mut updated = contents.to_string();
    updated = insert_before_marker(
        &updated,
        MOD_MARKER,
        &format!("mod {};\n", generated.module_name),
    )?;
    updated = insert_before_marker(
        &updated,
        PUB_USE_MARKER,
        &format!(
            "pub use {}::{{{}, {}}};\n",
            generated.module_name, generated.strategy_type, generated.config_type
        ),
    )?;
    updated = insert_before_marker(
        &updated,
        KIND_VARIANT_MARKER,
        &format!("    {},\n", generated.variant_name),
    )?;
    updated = insert_before_marker(
        &updated,
        KIND_AS_STR_MARKER,
        &format!(
            "            Self::{} => \"{}\",\n",
            generated.variant_name, generated.module_name
        ),
    )?;
    updated = insert_before_marker(
        &updated,
        KIND_FROM_STR_MARKER,
        &format!(
            "            \"{name}\" => Ok(Self::{variant}),\n            \"{name_hyphen}\" => Ok(Self::{variant}),\n",
            name = generated.module_name,
            name_hyphen = generated.module_name.replace('_', "-"),
            variant = generated.variant_name
        ),
    )?;
    updated = insert_before_marker(
        &updated,
        ANY_STRATEGY_VARIANT_MARKER,
        &format!(
            "    {}({}),\n",
            generated.variant_name, generated.strategy_type
        ),
    )?;
    updated = insert_before_marker(
        &updated,
        METADATA_ARM_MARKER,
        &format!(
            "            Self::{}(strategy) => strategy.metadata(),\n",
            generated.variant_name
        ),
    )?;
    updated = insert_before_marker(
        &updated,
        ON_EVENT_ARM_MARKER,
        &format!(
            "            Self::{}(strategy) => strategy.on_event(event, market_state, broker),\n",
            generated.variant_name
        ),
    )?;
    updated = insert_before_marker(
        &updated,
        ON_EXECUTION_ARM_MARKER,
        &format!(
            "            Self::{}(strategy) => {{\n                strategy.on_execution_reports(reports, market_state, broker)\n            }}\n",
            generated.variant_name
        ),
    )?;
    updated = updated.replace(
        "unsupported strategy '{}', expected one of: momentum, early_flow, noop",
        "unsupported strategy '{}', expected a registered strategy kind",
    );
    Ok(updated)
}

fn register_runtime_strategy_builder(
    contents: &str,
    generated: &GeneratedStrategy,
    template_kind: StrategyKind,
) -> Result<String> {
    let arm = match template_kind {
        StrategyKind::Momentum => format!(
            "        StrategyKind::{variant} => AnyStrategy::{variant}(\n            pump_agent_core::{strategy_type}::new(pump_agent_core::{config_type} {{\n                max_age_secs: args.max_age_secs,\n                min_buy_count: args.min_buy_count,\n                min_unique_buyers: args.min_unique_buyers,\n                min_net_flow_lamports: sol_to_lamports(args.min_net_buy_sol) as i128,\n                buy_lamports: sol_to_lamports(args.buy_sol),\n                take_profit_bps: args.take_profit_bps,\n                stop_loss_bps: args.stop_loss_bps,\n                max_hold_secs: args.max_hold_secs,\n            }}),\n        ),\n",
            variant = generated.variant_name,
            strategy_type = generated.strategy_type,
            config_type = generated.config_type
        ),
        StrategyKind::EarlyFlow => format!(
            "        StrategyKind::{variant} => AnyStrategy::{variant}(\n            pump_agent_core::{strategy_type}::new(pump_agent_core::{config_type} {{\n                max_age_secs: args.max_age_secs,\n                min_buy_count: args.min_buy_count,\n                min_unique_buyers: args.min_unique_buyers,\n                min_total_buy_lamports: sol_to_lamports(args.min_total_buy_sol) as u128,\n                max_sell_count: args.max_sell_count,\n                min_buy_sell_ratio: args.min_buy_sell_ratio,\n                buy_lamports: sol_to_lamports(args.buy_sol),\n                take_profit_bps: args.take_profit_bps,\n                stop_loss_bps: args.stop_loss_bps,\n                max_hold_secs: args.max_hold_secs,\n                max_concurrent_positions: args.max_concurrent_positions,\n                exit_on_sell_count: args.exit_on_sell_count,\n            }}),\n        ),\n",
            variant = generated.variant_name,
            strategy_type = generated.strategy_type,
            config_type = generated.config_type
        ),
        StrategyKind::Breakout | StrategyKind::LiquidityFollow => format!(
            "        StrategyKind::{variant} => AnyStrategy::{variant}(\n            pump_agent_core::{strategy_type}::new(pump_agent_core::{config_type} {{\n                max_age_secs: args.max_age_secs,\n                min_buy_count: args.min_buy_count,\n                min_unique_buyers: args.min_unique_buyers,\n                min_total_buy_lamports: sol_to_lamports(args.min_total_buy_sol) as u128,\n                min_net_flow_lamports: sol_to_lamports(args.min_net_buy_sol) as i128,\n                max_sell_count: args.max_sell_count,\n                min_buy_sell_ratio: args.min_buy_sell_ratio,\n                buy_lamports: sol_to_lamports(args.buy_sol),\n                take_profit_bps: args.take_profit_bps,\n                stop_loss_bps: args.stop_loss_bps,\n                max_hold_secs: args.max_hold_secs,\n                max_concurrent_positions: args.max_concurrent_positions,\n                exit_on_sell_count: args.exit_on_sell_count,\n            }}),\n        ),\n",
            variant = generated.variant_name,
            strategy_type = generated.strategy_type,
            config_type = generated.config_type
        ),
        StrategyKind::Noop => format!(
            "        StrategyKind::{variant} => AnyStrategy::{variant}(\n            pump_agent_core::{strategy_type}::new(pump_agent_core::{config_type}),\n        ),\n",
            variant = generated.variant_name,
            strategy_type = generated.strategy_type,
            config_type = generated.config_type
        ),
    };
    insert_before_marker(contents, RUNTIME_MATCH_MARKER, &arm)
}

fn register_core_lib_exports(contents: &str, generated: &GeneratedStrategy) -> Result<String> {
    insert_before_marker(
        contents,
        LIB_PUB_USE_MARKER,
        &format!(
            "pub use strategy::{{{}, {}}};\n",
            generated.strategy_type, generated.config_type
        ),
    )
}

fn insert_before_marker(contents: &str, marker: &str, snippet: &str) -> Result<String> {
    if contents.contains(snippet.trim_end()) {
        return Ok(contents.to_string());
    }

    let Some(index) = contents.find(marker) else {
        bail!("registration marker not found: {marker}");
    };

    let mut updated = String::with_capacity(contents.len() + snippet.len());
    updated.push_str(&contents[..index]);
    updated.push_str(snippet);
    updated.push_str(&contents[index..]);
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use pump_agent_app::api::{CloneScoreBreakdownOutput, FitSummary};

    use super::{
        ANY_STRATEGY_VARIANT_MARKER, CloneReportOutput, GeneratedStrategy, KIND_AS_STR_MARKER,
        KIND_FROM_STR_MARKER, KIND_VARIANT_MARKER, METADATA_ARM_MARKER, MOD_MARKER,
        ON_EVENT_ARM_MARKER, ON_EXECUTION_ARM_MARKER, PUB_USE_MARKER, ParamsSeed,
        RUNTIME_MATCH_MARKER, StrategyKind, insert_before_marker, register_core_strategy_module,
        register_runtime_strategy_builder, render_seeded_strategy_config,
        render_strategy_code_template, render_strategy_config_template, to_upper_camel_case,
        validate_module_name,
    };

    fn sample_clone_report() -> CloneReportOutput {
        CloneReportOutput {
            address: "wallet111".to_string(),
            recommended_base_family: "early_flow".to_string(),
            recommended_next_strategy_name: "confirmed_flow".to_string(),
            base_fit: FitSummary {
                family: "early_flow".to_string(),
                clone_score: 0.4,
                f1: 0.3,
                precision: 0.4,
                recall: 0.25,
                breakdown: CloneScoreBreakdownOutput {
                    entry_timing_similarity: 0.6,
                    hold_time_similarity: 0.5,
                    size_profile_similarity: 0.7,
                    token_selection_similarity: 0.4,
                    exit_behavior_similarity: 0.5,
                    count_alignment: 0.8,
                },
            },
            runner_up: FitSummary {
                family: "momentum".to_string(),
                clone_score: 0.2,
                f1: 0.2,
                precision: 0.2,
                recall: 0.2,
                breakdown: CloneScoreBreakdownOutput {
                    entry_timing_similarity: 0.3,
                    hold_time_similarity: 0.2,
                    size_profile_similarity: 0.4,
                    token_selection_similarity: 0.2,
                    exit_behavior_similarity: 0.3,
                    count_alignment: 0.5,
                },
            },
            confirmed_rules: vec![
                "uses roughly fixed ticket size around 0.208 SOL".to_string(),
                "typically enters after confirmation".to_string(),
            ],
            tentative_rules: Vec::new(),
            anti_patterns: Vec::new(),
            recommended_params_seed: ParamsSeed {
                buy_sol: 0.208,
                max_age_secs: 21,
                min_buy_count: 24,
                min_unique_buyers: 20,
                min_net_buy_sol: 12.0,
                min_total_buy_sol: 20.4,
                max_sell_count: 8,
                min_buy_sell_ratio: 5.2,
                max_hold_secs: 41,
                max_concurrent_positions: 3,
                exit_on_sell_count: 6,
                take_profit_bps: 1800,
                stop_loss_bps: 900,
            },
            export: None,
        }
    }

    #[test]
    fn renders_early_flow_config_template() {
        let template = render_strategy_config_template(StrategyKind::EarlyFlow, "alpha_flow");
        assert!(template.contains("strategy = \"alpha_flow\""));
        assert!(template.contains("min_total_buy_sol = 0.8"));
    }

    #[test]
    fn renders_noop_template() {
        let template = render_strategy_config_template(StrategyKind::Noop, "alpha_noop");
        assert!(template.contains("strategy = \"alpha_noop\""));
        assert!(!template.contains("min_buy_count"));
    }

    #[test]
    fn renders_seeded_clone_config_template() {
        let template = render_seeded_strategy_config(
            StrategyKind::EarlyFlow,
            "confirmed_flow",
            &sample_clone_report(),
        );
        assert!(template.contains("Source address: wallet111"));
        assert!(template.contains("strategy = \"confirmed_flow\""));
        assert!(template.contains("buy_sol = 0.208"));
        assert!(template.contains("min_total_buy_sol = 20.400"));
        assert!(template.contains("exit_on_sell_count = 6"));
    }

    #[test]
    fn validates_and_converts_module_names() {
        validate_module_name("alpha_flow").expect("valid snake_case should pass");
        assert_eq!(to_upper_camel_case("alpha_flow"), "AlphaFlow");
        assert!(validate_module_name("AlphaFlow").is_err());
        assert!(validate_module_name("alpha-flow").is_err());
    }

    #[test]
    fn renders_strategy_code_template() {
        let generated = GeneratedStrategy::new("alpha_flow").expect("generated names should work");
        let code = render_strategy_code_template(StrategyKind::EarlyFlow, &generated);
        assert!(code.contains("pub struct AlphaFlowStrategy"));
        assert!(code.contains("pub struct AlphaFlowStrategyConfig"));
        assert!(code.contains("name: \"alpha_flow_strategy\""));
    }

    #[test]
    fn inserts_before_marker_idempotently() {
        let source = format!("before\n{}\nafter\n", MOD_MARKER);
        let once = insert_before_marker(&source, MOD_MARKER, "line\n").expect("insert should work");
        let twice =
            insert_before_marker(&once, MOD_MARKER, "line\n").expect("insert should be idempotent");
        assert_eq!(once, twice);
    }

    #[test]
    fn registers_core_and_runtime_snippets() {
        let generated = GeneratedStrategy::new("alpha_flow").expect("generated names should work");
        let core_stub = format!(
            "mod noop;\n{mod_marker}\npub use noop::NoopStrategy;\n{pub_use_marker}\n\
             pub enum StrategyKind {{\n    Noop,\n    {kind_marker}\n}}\n\
             impl StrategyKind {{ pub fn as_str(self) -> &'static str {{ match self {{\n            Self::Noop => \"noop\",\n            {kind_as_str}\n        }} }} }}\n\
             impl std::str::FromStr for StrategyKind {{ type Err = String; fn from_str(value: &str) -> Result<Self, Self::Err> {{ match value {{\n            \"noop\" => Ok(Self::Noop),\n            {kind_from_str}\n            other => Err(other.to_string()),\n        }} }} }}\n\
             pub enum AnyStrategy {{\n    Noop(NoopStrategy),\n    {any_marker}\n}}\n\
             impl Strategy for AnyStrategy {{ fn metadata(&self) -> StrategyMetadata {{ match self {{\n            Self::Noop(strategy) => strategy.metadata(),\n            {metadata_marker}\n        }} }} fn on_event(&mut self, event: &EventEnvelope, market_state: &MarketState, broker: &BrokerSnapshot) -> Vec<OrderRequest> {{ match self {{\n            Self::Noop(strategy) => strategy.on_event(event, market_state, broker),\n            {on_event_marker}\n        }} }} fn on_execution_reports(&mut self, reports: &[ExecutionReport], market_state: &MarketState, broker: &BrokerSnapshot) {{ match self {{\n            Self::Noop(strategy) => strategy.on_execution_reports(reports, market_state, broker),\n            {on_exec_marker}\n        }} }} }}\n",
            mod_marker = MOD_MARKER,
            pub_use_marker = PUB_USE_MARKER,
            kind_marker = KIND_VARIANT_MARKER,
            kind_as_str = KIND_AS_STR_MARKER,
            kind_from_str = KIND_FROM_STR_MARKER,
            any_marker = ANY_STRATEGY_VARIANT_MARKER,
            metadata_marker = METADATA_ARM_MARKER,
            on_event_marker = ON_EVENT_ARM_MARKER,
            on_exec_marker = ON_EXECUTION_ARM_MARKER,
        );
        let runtime_stub = format!(
            "match strategy_kind {{\n        StrategyKind::Noop => AnyStrategy::Noop(NoopStrategy::new()),\n        {marker}\n    }}",
            marker = RUNTIME_MATCH_MARKER
        );

        let updated_core = register_core_strategy_module(&core_stub, &generated)
            .expect("core registration should work");
        let updated_runtime =
            register_runtime_strategy_builder(&runtime_stub, &generated, StrategyKind::EarlyFlow)
                .expect("runtime registration should work");

        assert!(updated_core.contains("mod alpha_flow;"));
        assert!(
            updated_core
                .contains("pub use alpha_flow::{AlphaFlowStrategy, AlphaFlowStrategyConfig};")
        );
        assert!(updated_core.contains("AlphaFlow,"));
        assert!(updated_core.contains("\"alpha_flow\" => Ok(Self::AlphaFlow)"));
        assert!(updated_runtime.contains("StrategyKind::AlphaFlow => AnyStrategy::AlphaFlow("));
        assert!(updated_runtime.contains("pump_agent_core::AlphaFlowStrategy::new"));
    }
}
