use std::{
    fmt::Display,
    path::PathBuf,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, bail};
use pump_agent_core::{
    AnyStrategy, BacktestReport, BacktestRunResult, BreakoutStrategy, BreakoutStrategyConfig,
    BrokerConfig, EarlyFlowStrategy, EarlyFlowStrategyConfig, Engine, EventEnvelope,
    LiquidityFollowStrategy, LiquidityFollowStrategyConfig, MomentumStrategy,
    MomentumStrategyConfig, NoopStrategy, PaperBroker, PgEventStore, PositionSnapshotInput,
    StrategyKind, StrategyRunPersistOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub strategy: String,
    pub strategy_config: Option<PathBuf>,
    pub starting_sol: f64,
    pub buy_sol: f64,
    pub max_age_secs: i64,
    pub min_buy_count: u64,
    pub min_unique_buyers: usize,
    pub min_net_buy_sol: f64,
    pub take_profit_bps: i64,
    pub stop_loss_bps: i64,
    pub max_hold_secs: i64,
    pub min_total_buy_sol: f64,
    pub max_sell_count: u64,
    pub min_buy_sell_ratio: f64,
    pub max_concurrent_positions: usize,
    pub exit_on_sell_count: u64,
    pub trading_fee_bps: u64,
    pub slippage_bps: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SweepConfig {
    pub buy_sol_values: Option<String>,
    pub max_age_secs_values: Option<String>,
    pub min_buy_count_values: Option<String>,
    pub min_unique_buyers_values: Option<String>,
    pub min_total_buy_sol_values: Option<String>,
    pub max_sell_count_values: Option<String>,
    pub min_buy_sell_ratio_values: Option<String>,
    pub take_profit_bps_values: Option<String>,
    pub stop_loss_bps_values: Option<String>,
    pub max_concurrent_positions_values: Option<String>,
    pub exit_on_sell_count_values: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StrategyExecution {
    pub result: BacktestRunResult,
    pub final_position_snapshot: PositionSnapshotInput,
}

#[derive(Debug, Clone, Serialize)]
pub struct SweepRunSummary {
    pub run_id: i64,
    pub strategy: StrategyConfig,
    pub report: BacktestReport,
}

#[derive(Debug, Deserialize)]
struct StrategyFile {
    strategy: StrategyFileStrategy,
}

#[derive(Debug, Deserialize)]
struct StrategyFileStrategy {
    strategy: Option<String>,
    starting_sol: Option<f64>,
    buy_sol: Option<f64>,
    max_age_secs: Option<i64>,
    min_buy_count: Option<u64>,
    min_unique_buyers: Option<usize>,
    min_net_buy_sol: Option<f64>,
    take_profit_bps: Option<i64>,
    stop_loss_bps: Option<i64>,
    max_hold_secs: Option<i64>,
    min_total_buy_sol: Option<f64>,
    max_sell_count: Option<u64>,
    min_buy_sell_ratio: Option<f64>,
    max_concurrent_positions: Option<usize>,
    exit_on_sell_count: Option<u64>,
    trading_fee_bps: Option<u64>,
    slippage_bps: Option<u64>,
}

pub fn run_strategy(
    events: Vec<EventEnvelope>,
    config: &StrategyConfig,
) -> Result<StrategyExecution> {
    let resolved = resolve_strategy_config(config)?;
    let (strategy, broker) = build_strategy_and_broker(&resolved)?;
    let mut engine = Engine::new(strategy, broker);
    let result = engine.run(events);
    let final_position_snapshot = build_position_snapshot_input(
        "final",
        &result.report,
        &engine.broker_snapshot(),
        engine.market_state(),
        None,
        None,
        None,
    );
    Ok(StrategyExecution {
        result,
        final_position_snapshot,
    })
}

pub fn build_strategy_and_broker(config: &StrategyConfig) -> Result<(AnyStrategy, PaperBroker)> {
    let strategy_kind = StrategyKind::from_str(&config.strategy)
        .map_err(|error| anyhow::anyhow!("invalid --strategy '{}': {}", config.strategy, error))?;
    let broker = PaperBroker::new(BrokerConfig {
        starting_cash_lamports: sol_to_lamports(config.starting_sol),
        trading_fee_bps: config.trading_fee_bps,
        slippage_bps: config.slippage_bps,
    });

    let strategy = match strategy_kind {
        StrategyKind::Momentum => {
            AnyStrategy::Momentum(MomentumStrategy::new(MomentumStrategyConfig {
                max_age_secs: config.max_age_secs,
                min_buy_count: config.min_buy_count,
                min_unique_buyers: config.min_unique_buyers,
                min_net_flow_lamports: sol_to_lamports(config.min_net_buy_sol) as i128,
                buy_lamports: sol_to_lamports(config.buy_sol),
                take_profit_bps: config.take_profit_bps,
                stop_loss_bps: config.stop_loss_bps,
                max_hold_secs: config.max_hold_secs,
            }))
        }
        StrategyKind::EarlyFlow => {
            AnyStrategy::EarlyFlow(EarlyFlowStrategy::new(EarlyFlowStrategyConfig {
                max_age_secs: config.max_age_secs,
                min_buy_count: config.min_buy_count,
                min_unique_buyers: config.min_unique_buyers,
                min_total_buy_lamports: sol_to_lamports(config.min_total_buy_sol) as u128,
                max_sell_count: config.max_sell_count,
                min_buy_sell_ratio: config.min_buy_sell_ratio,
                buy_lamports: sol_to_lamports(config.buy_sol),
                take_profit_bps: config.take_profit_bps,
                stop_loss_bps: config.stop_loss_bps,
                max_hold_secs: config.max_hold_secs,
                max_concurrent_positions: config.max_concurrent_positions,
                exit_on_sell_count: config.exit_on_sell_count,
            }))
        }
        StrategyKind::Breakout => {
            AnyStrategy::Breakout(BreakoutStrategy::new(BreakoutStrategyConfig {
                max_age_secs: config.max_age_secs,
                min_buy_count: config.min_buy_count,
                min_unique_buyers: config.min_unique_buyers,
                min_total_buy_lamports: sol_to_lamports(config.min_total_buy_sol) as u128,
                min_net_flow_lamports: sol_to_lamports(config.min_net_buy_sol) as i128,
                max_sell_count: config.max_sell_count,
                min_buy_sell_ratio: config.min_buy_sell_ratio,
                buy_lamports: sol_to_lamports(config.buy_sol),
                take_profit_bps: config.take_profit_bps,
                stop_loss_bps: config.stop_loss_bps,
                max_hold_secs: config.max_hold_secs,
                max_concurrent_positions: config.max_concurrent_positions,
                exit_on_sell_count: config.exit_on_sell_count,
            }))
        }
        StrategyKind::LiquidityFollow => AnyStrategy::LiquidityFollow(
            LiquidityFollowStrategy::new(LiquidityFollowStrategyConfig {
                max_age_secs: config.max_age_secs,
                min_buy_count: config.min_buy_count,
                min_unique_buyers: config.min_unique_buyers,
                min_total_buy_lamports: sol_to_lamports(config.min_total_buy_sol) as u128,
                min_net_flow_lamports: sol_to_lamports(config.min_net_buy_sol) as i128,
                max_sell_count: config.max_sell_count,
                min_buy_sell_ratio: config.min_buy_sell_ratio,
                buy_lamports: sol_to_lamports(config.buy_sol),
                take_profit_bps: config.take_profit_bps,
                stop_loss_bps: config.stop_loss_bps,
                max_hold_secs: config.max_hold_secs,
                max_concurrent_positions: config.max_concurrent_positions,
                exit_on_sell_count: config.exit_on_sell_count,
            }),
        ),
        StrategyKind::Noop => AnyStrategy::Noop(NoopStrategy::new()),
    };

    Ok((strategy, broker))
}

pub fn resolve_strategy_config(config: &StrategyConfig) -> Result<StrategyConfig> {
    let Some(path) = &config.strategy_config else {
        return Ok(config.clone());
    };

    let content = std::fs::read_to_string(path)?;
    let file_config = toml::from_str::<StrategyFile>(&content)?;
    Ok(merge_strategy_config(config, file_config.strategy))
}

pub fn serialize_strategy_config(config: &StrategyConfig) -> Result<serde_json::Value> {
    let resolved = resolve_strategy_config(config)?;
    Ok(serde_json::to_value(&resolved)?)
}

pub fn deserialize_strategy_config(value: &serde_json::Value) -> Result<StrategyConfig> {
    Ok(serde_json::from_value(value.clone())?)
}

pub async fn persist_run(
    store: &PgEventStore,
    source_type: &str,
    source_ref: &str,
    strategy_config: &StrategyConfig,
    result: &BacktestRunResult,
    options: StrategyRunPersistOptions,
) -> Result<i64> {
    let config = serialize_strategy_config(strategy_config)?;
    store
        .persist_backtest_run(
            source_type,
            source_ref,
            result.report.strategy.name,
            config,
            result,
            options,
        )
        .await
}

pub fn generate_run_group_id(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{}-{}", prefix, millis)
}

pub fn build_sweep_variants(
    base: &StrategyConfig,
    sweep: &SweepConfig,
) -> Result<Vec<StrategyConfig>> {
    let mut variants = vec![StrategyConfig {
        strategy_config: None,
        ..base.clone()
    }];

    let buy_sol_values = parse_csv_values::<f64>(&sweep.buy_sol_values, "buy_sol_values")?
        .unwrap_or_else(|| vec![base.buy_sol]);
    variants = expand_variants(variants, &buy_sol_values, |config, value| {
        config.buy_sol = value
    });

    let max_age_secs_values =
        parse_csv_values::<i64>(&sweep.max_age_secs_values, "max_age_secs_values")?
            .unwrap_or_else(|| vec![base.max_age_secs]);
    variants = expand_variants(variants, &max_age_secs_values, |config, value| {
        config.max_age_secs = value
    });

    let min_buy_count_values =
        parse_csv_values::<u64>(&sweep.min_buy_count_values, "min_buy_count_values")?
            .unwrap_or_else(|| vec![base.min_buy_count]);
    variants = expand_variants(variants, &min_buy_count_values, |config, value| {
        config.min_buy_count = value
    });

    let min_unique_buyers_values =
        parse_csv_values::<usize>(&sweep.min_unique_buyers_values, "min_unique_buyers_values")?
            .unwrap_or_else(|| vec![base.min_unique_buyers]);
    variants = expand_variants(variants, &min_unique_buyers_values, |config, value| {
        config.min_unique_buyers = value
    });

    let min_total_buy_sol_values =
        parse_csv_values::<f64>(&sweep.min_total_buy_sol_values, "min_total_buy_sol_values")?
            .unwrap_or_else(|| vec![base.min_total_buy_sol]);
    variants = expand_variants(variants, &min_total_buy_sol_values, |config, value| {
        config.min_total_buy_sol = value
    });

    let max_sell_count_values =
        parse_csv_values::<u64>(&sweep.max_sell_count_values, "max_sell_count_values")?
            .unwrap_or_else(|| vec![base.max_sell_count]);
    variants = expand_variants(variants, &max_sell_count_values, |config, value| {
        config.max_sell_count = value
    });

    let min_buy_sell_ratio_values = parse_csv_values::<f64>(
        &sweep.min_buy_sell_ratio_values,
        "min_buy_sell_ratio_values",
    )?
    .unwrap_or_else(|| vec![base.min_buy_sell_ratio]);
    variants = expand_variants(variants, &min_buy_sell_ratio_values, |config, value| {
        config.min_buy_sell_ratio = value
    });

    let take_profit_bps_values =
        parse_csv_values::<i64>(&sweep.take_profit_bps_values, "take_profit_bps_values")?
            .unwrap_or_else(|| vec![base.take_profit_bps]);
    variants = expand_variants(variants, &take_profit_bps_values, |config, value| {
        config.take_profit_bps = value
    });

    let stop_loss_bps_values =
        parse_csv_values::<i64>(&sweep.stop_loss_bps_values, "stop_loss_bps_values")?
            .unwrap_or_else(|| vec![base.stop_loss_bps]);
    variants = expand_variants(variants, &stop_loss_bps_values, |config, value| {
        config.stop_loss_bps = value
    });

    let max_concurrent_positions_values = parse_csv_values::<usize>(
        &sweep.max_concurrent_positions_values,
        "max_concurrent_positions_values",
    )?
    .unwrap_or_else(|| vec![base.max_concurrent_positions]);
    variants = expand_variants(
        variants,
        &max_concurrent_positions_values,
        |config, value| config.max_concurrent_positions = value,
    );

    let exit_on_sell_count_values = parse_csv_values::<u64>(
        &sweep.exit_on_sell_count_values,
        "exit_on_sell_count_values",
    )?
    .unwrap_or_else(|| vec![base.exit_on_sell_count]);
    variants = expand_variants(variants, &exit_on_sell_count_values, |config, value| {
        config.exit_on_sell_count = value
    });

    Ok(variants)
}

pub fn build_position_snapshot_input(
    snapshot_kind: &str,
    report: &BacktestReport,
    broker: &pump_agent_core::BrokerSnapshot,
    market_state: &pump_agent_core::MarketState,
    event_seq: Option<u64>,
    event_slot: Option<u64>,
    snapshot_at: Option<i64>,
) -> PositionSnapshotInput {
    let positions = broker
        .positions
        .values()
        .map(|position| {
            let state = market_state.mint(&position.mint);
            let mark_price = state
                .map(|mint| mint.last_price_lamports_per_token)
                .unwrap_or(position.last_mark_price_lamports_per_token);
            let pnl_bps = if position.average_entry_price_lamports_per_token > 0.0 {
                ((mark_price / position.average_entry_price_lamports_per_token) - 1.0) * 10_000.0
            } else {
                0.0
            };

            serde_json::json!({
                "mint": position.mint,
                "symbol": state.and_then(|mint| mint.symbol.clone()),
                "token_amount": position.token_amount,
                "entry_notional_lamports": position.entry_notional_lamports,
                "entry_price_lamports_per_token": position.average_entry_price_lamports_per_token,
                "mark_price_lamports_per_token": mark_price,
                "pnl_bps": pnl_bps,
                "opened_at": position.opened_at,
            })
        })
        .collect::<Vec<_>>();

    PositionSnapshotInput {
        snapshot_kind: snapshot_kind.to_string(),
        event_seq,
        event_slot,
        snapshot_at,
        cash_lamports: report.ending_cash_lamports,
        equity_lamports: report.ending_equity_lamports,
        pending_orders: broker.pending_orders,
        open_positions: broker.positions.len(),
        positions: serde_json::Value::Array(positions),
    }
}

fn merge_strategy_config(cli: &StrategyConfig, file: StrategyFileStrategy) -> StrategyConfig {
    StrategyConfig {
        strategy: if cli.strategy != "momentum" {
            cli.strategy.clone()
        } else {
            file.strategy.unwrap_or_else(|| "momentum".to_string())
        },
        strategy_config: cli.strategy_config.clone(),
        starting_sol: if cli.starting_sol != 10.0 {
            cli.starting_sol
        } else {
            file.starting_sol.unwrap_or(10.0)
        },
        buy_sol: if cli.buy_sol != 0.2 {
            cli.buy_sol
        } else {
            file.buy_sol.unwrap_or(0.2)
        },
        max_age_secs: if cli.max_age_secs != 45 {
            cli.max_age_secs
        } else {
            file.max_age_secs.unwrap_or(45)
        },
        min_buy_count: if cli.min_buy_count != 3 {
            cli.min_buy_count
        } else {
            file.min_buy_count.unwrap_or(3)
        },
        min_unique_buyers: if cli.min_unique_buyers != 3 {
            cli.min_unique_buyers
        } else {
            file.min_unique_buyers.unwrap_or(3)
        },
        min_net_buy_sol: if cli.min_net_buy_sol != 0.3 {
            cli.min_net_buy_sol
        } else {
            file.min_net_buy_sol.unwrap_or(0.3)
        },
        take_profit_bps: if cli.take_profit_bps != 2500 {
            cli.take_profit_bps
        } else {
            file.take_profit_bps.unwrap_or(2500)
        },
        stop_loss_bps: if cli.stop_loss_bps != 1200 {
            cli.stop_loss_bps
        } else {
            file.stop_loss_bps.unwrap_or(1200)
        },
        max_hold_secs: if cli.max_hold_secs != 90 {
            cli.max_hold_secs
        } else {
            file.max_hold_secs.unwrap_or(90)
        },
        min_total_buy_sol: if cli.min_total_buy_sol != 0.8 {
            cli.min_total_buy_sol
        } else {
            file.min_total_buy_sol.unwrap_or(0.8)
        },
        max_sell_count: if cli.max_sell_count != 1 {
            cli.max_sell_count
        } else {
            file.max_sell_count.unwrap_or(1)
        },
        min_buy_sell_ratio: if (cli.min_buy_sell_ratio - 4.0).abs() > f64::EPSILON {
            cli.min_buy_sell_ratio
        } else {
            file.min_buy_sell_ratio.unwrap_or(4.0)
        },
        max_concurrent_positions: if cli.max_concurrent_positions != 3 {
            cli.max_concurrent_positions
        } else {
            file.max_concurrent_positions.unwrap_or(3)
        },
        exit_on_sell_count: if cli.exit_on_sell_count != 3 {
            cli.exit_on_sell_count
        } else {
            file.exit_on_sell_count.unwrap_or(3)
        },
        trading_fee_bps: if cli.trading_fee_bps != 100 {
            cli.trading_fee_bps
        } else {
            file.trading_fee_bps.unwrap_or(100)
        },
        slippage_bps: if cli.slippage_bps != 50 {
            cli.slippage_bps
        } else {
            file.slippage_bps.unwrap_or(50)
        },
    }
}

fn expand_variants<T, F>(
    variants: Vec<StrategyConfig>,
    values: &[T],
    mut apply: F,
) -> Vec<StrategyConfig>
where
    T: Clone,
    F: FnMut(&mut StrategyConfig, T),
{
    let mut expanded = Vec::with_capacity(variants.len() * values.len());
    for variant in variants {
        for value in values {
            let mut next = variant.clone();
            apply(&mut next, value.clone());
            expanded.push(next);
        }
    }
    expanded
}

fn parse_csv_values<T>(raw: &Option<String>, label: &str) -> Result<Option<Vec<T>>>
where
    T: FromStr,
    <T as FromStr>::Err: Display,
{
    let Some(raw) = raw else {
        return Ok(None);
    };

    let values = raw
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .parse::<T>()
                .map_err(|error| anyhow::anyhow!("invalid {} entry '{}': {}", label, value, error))
        })
        .collect::<Result<Vec<_>>>()?;

    if values.is_empty() {
        bail!("{} cannot be empty", label);
    }

    Ok(Some(values))
}

fn sol_to_lamports(sol: f64) -> u64 {
    (sol * 1_000_000_000.0).round() as u64
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{StrategyConfig, SweepConfig, build_sweep_variants, resolve_strategy_config};

    fn sample_strategy_config() -> StrategyConfig {
        StrategyConfig {
            strategy: "momentum".to_string(),
            strategy_config: None,
            starting_sol: 10.0,
            buy_sol: 0.2,
            max_age_secs: 45,
            min_buy_count: 3,
            min_unique_buyers: 3,
            min_net_buy_sol: 0.3,
            take_profit_bps: 2500,
            stop_loss_bps: 1200,
            max_hold_secs: 90,
            min_total_buy_sol: 0.8,
            max_sell_count: 1,
            min_buy_sell_ratio: 4.0,
            max_concurrent_positions: 3,
            exit_on_sell_count: 3,
            trading_fee_bps: 100,
            slippage_bps: 50,
        }
    }

    fn sample_sweep_config() -> SweepConfig {
        SweepConfig {
            buy_sol_values: None,
            max_age_secs_values: None,
            min_buy_count_values: None,
            min_unique_buyers_values: None,
            min_total_buy_sol_values: None,
            max_sell_count_values: None,
            min_buy_sell_ratio_values: None,
            take_profit_bps_values: None,
            stop_loss_bps_values: None,
            max_concurrent_positions_values: None,
            exit_on_sell_count_values: None,
        }
    }

    #[test]
    fn resolve_strategy_config_applies_file_overrides() {
        let mut config = sample_strategy_config();
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        path.push(format!("pump-agent-app-strategy-{unique}.toml"));
        fs::write(
            &path,
            r#"
[strategy]
strategy = "early_flow"
buy_sol = 0.33
"#,
        )
        .expect("config file should be written");
        config.strategy_config = Some(path.clone());

        let resolved = resolve_strategy_config(&config).expect("config should resolve");
        assert_eq!(resolved.strategy, "early_flow");
        assert_eq!(resolved.buy_sol, 0.33);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn build_sweep_variants_expands_cartesian_product() {
        let base = sample_strategy_config();
        let mut sweep = sample_sweep_config();
        sweep.buy_sol_values = Some("0.1,0.2".to_string());
        sweep.max_sell_count_values = Some("0,1".to_string());

        let variants = build_sweep_variants(&base, &sweep).expect("variants should build");
        assert_eq!(variants.len(), 4);
        assert!(
            variants
                .iter()
                .all(|variant| variant.strategy_config.is_none())
        );
        assert!(
            variants
                .iter()
                .any(|variant| variant.buy_sol == 0.1 && variant.max_sell_count == 0)
        );
        assert!(
            variants
                .iter()
                .any(|variant| variant.buy_sol == 0.2 && variant.max_sell_count == 1)
        );
    }

    #[test]
    fn build_sweep_variants_rejects_empty_csv() {
        let base = sample_strategy_config();
        let mut sweep = sample_sweep_config();
        sweep.buy_sol_values = Some(" , ".to_string());

        let error = build_sweep_variants(&base, &sweep).expect_err("empty csv should fail");
        assert!(error.to_string().contains("buy_sol_values cannot be empty"));
    }
}
