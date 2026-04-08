use std::str::FromStr;

use anyhow::Result;
use pump_agent_core::{
    AnyStrategy, BreakoutStrategy, BreakoutStrategyConfig, BrokerConfig, EarlyFlowStrategy,
    EarlyFlowStrategyConfig, LiquidityFollowStrategy, LiquidityFollowStrategyConfig,
    MomentumStrategy, MomentumStrategyConfig, NoopStrategy, PaperBroker, StrategyKind,
};

use crate::{
    args::{StrategyArgs, StrategyFile, StrategyFileStrategy},
    config::sol_to_lamports,
};

#[cfg(test)]
use pump_agent_core::{Engine, EventEnvelope};
#[cfg(test)]
use super::persist::build_position_snapshot_input;

#[cfg(test)]
#[derive(Debug, Clone)]
pub struct StrategyExecution {
    pub result: pump_agent_core::BacktestRunResult,
    pub final_position_snapshot: pump_agent_core::PositionSnapshotInput,
}

#[cfg(test)]
pub fn run_strategy(events: Vec<EventEnvelope>, args: &StrategyArgs) -> Result<StrategyExecution> {
    let resolved = resolve_strategy_args(args)?;
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

pub fn build_strategy_and_broker(args: &StrategyArgs) -> Result<(AnyStrategy, PaperBroker)> {
    let strategy_kind = StrategyKind::from_str(&args.strategy)
        .map_err(|error| anyhow::anyhow!("invalid --strategy '{}': {}", args.strategy, error))?;
    let broker = PaperBroker::new(BrokerConfig {
        starting_cash_lamports: sol_to_lamports(args.starting_sol),
        trading_fee_bps: args.trading_fee_bps,
        slippage_bps: args.slippage_bps,
    });

    let strategy = match strategy_kind {
        StrategyKind::Momentum => {
            AnyStrategy::Momentum(MomentumStrategy::new(MomentumStrategyConfig {
                max_age_secs: args.max_age_secs,
                min_buy_count: args.min_buy_count,
                min_unique_buyers: args.min_unique_buyers,
                min_net_flow_lamports: sol_to_lamports(args.min_net_buy_sol) as i128,
                buy_lamports: sol_to_lamports(args.buy_sol),
                take_profit_bps: args.take_profit_bps,
                stop_loss_bps: args.stop_loss_bps,
                max_hold_secs: args.max_hold_secs,
            }))
        }
        StrategyKind::EarlyFlow => {
            AnyStrategy::EarlyFlow(EarlyFlowStrategy::new(EarlyFlowStrategyConfig {
                max_age_secs: args.max_age_secs,
                min_buy_count: args.min_buy_count,
                min_unique_buyers: args.min_unique_buyers,
                min_total_buy_lamports: sol_to_lamports(args.min_total_buy_sol) as u128,
                max_sell_count: args.max_sell_count,
                min_buy_sell_ratio: args.min_buy_sell_ratio,
                buy_lamports: sol_to_lamports(args.buy_sol),
                take_profit_bps: args.take_profit_bps,
                stop_loss_bps: args.stop_loss_bps,
                max_hold_secs: args.max_hold_secs,
                max_concurrent_positions: args.max_concurrent_positions,
                exit_on_sell_count: args.exit_on_sell_count,
            }))
        }
        StrategyKind::Breakout => {
            AnyStrategy::Breakout(BreakoutStrategy::new(BreakoutStrategyConfig {
                max_age_secs: args.max_age_secs,
                min_buy_count: args.min_buy_count,
                min_unique_buyers: args.min_unique_buyers,
                min_total_buy_lamports: sol_to_lamports(args.min_total_buy_sol) as u128,
                min_net_flow_lamports: sol_to_lamports(args.min_net_buy_sol) as i128,
                max_sell_count: args.max_sell_count,
                min_buy_sell_ratio: args.min_buy_sell_ratio,
                buy_lamports: sol_to_lamports(args.buy_sol),
                take_profit_bps: args.take_profit_bps,
                stop_loss_bps: args.stop_loss_bps,
                max_hold_secs: args.max_hold_secs,
                max_concurrent_positions: args.max_concurrent_positions,
                exit_on_sell_count: args.exit_on_sell_count,
            }))
        }
        StrategyKind::LiquidityFollow => AnyStrategy::LiquidityFollow(
            LiquidityFollowStrategy::new(LiquidityFollowStrategyConfig {
                max_age_secs: args.max_age_secs,
                min_buy_count: args.min_buy_count,
                min_unique_buyers: args.min_unique_buyers,
                min_total_buy_lamports: sol_to_lamports(args.min_total_buy_sol) as u128,
                min_net_flow_lamports: sol_to_lamports(args.min_net_buy_sol) as i128,
                max_sell_count: args.max_sell_count,
                min_buy_sell_ratio: args.min_buy_sell_ratio,
                buy_lamports: sol_to_lamports(args.buy_sol),
                take_profit_bps: args.take_profit_bps,
                stop_loss_bps: args.stop_loss_bps,
                max_hold_secs: args.max_hold_secs,
                max_concurrent_positions: args.max_concurrent_positions,
                exit_on_sell_count: args.exit_on_sell_count,
            }),
        ),
        StrategyKind::Noop => AnyStrategy::Noop(NoopStrategy::new()),
        // strategy-scaffold: runtime-match
    };

    Ok((strategy, broker))
}

pub fn resolve_strategy_args(args: &StrategyArgs) -> Result<StrategyArgs> {
    let Some(path) = &args.strategy_config else {
        return Ok(args.clone());
    };

    let content = std::fs::read_to_string(path)?;
    let file_args = toml::from_str::<StrategyFile>(&content)?;
    Ok(merge_strategy_args(args, file_args.strategy))
}

fn merge_strategy_args(cli: &StrategyArgs, file: StrategyFileStrategy) -> StrategyArgs {
    StrategyArgs {
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use pump_agent_core::{EventEnvelope, MintCreatedEvent, OrderSide, PumpEvent, TradeEvent};

    use crate::args::StrategyArgs;

    use super::{build_strategy_and_broker, resolve_strategy_args, run_strategy};

    fn sample_strategy_args() -> StrategyArgs {
        StrategyArgs {
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

    fn write_temp_strategy_file(contents: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        path.push(format!("pump-agent-strategy-{unique}.toml"));
        fs::write(&path, contents).expect("temp strategy file should be written");
        path
    }

    fn mint_created_event(seq: u64, slot: u64, mint: &str, ts: i64) -> EventEnvelope {
        EventEnvelope {
            seq,
            slot,
            block_time: Some(ts),
            tx_signature: format!("sig-{seq}"),
            tx_index: 0,
            event_index: 0,
            event: PumpEvent::MintCreated(MintCreatedEvent {
                mint: mint.to_string(),
                bonding_curve: "curve-1".to_string(),
                user: "creator".to_string(),
                creator: "creator".to_string(),
                name: "Token".to_string(),
                symbol: "TOK".to_string(),
                uri: "https://example.com".to_string(),
                timestamp: ts,
                virtual_token_reserves: 1_000_000_000,
                virtual_sol_reserves: 1_000_000_000,
                real_token_reserves: 800_000_000,
                token_total_supply: 1_000_000_000,
                token_program: "Tokenkeg".to_string(),
                is_mayhem_mode: false,
                is_cashback_enabled: false,
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn trade_event(
        seq: u64,
        slot: u64,
        mint: &str,
        user: &str,
        ts: i64,
        sol_amount: u64,
        token_amount: u64,
        is_buy: bool,
    ) -> EventEnvelope {
        EventEnvelope {
            seq,
            slot,
            block_time: Some(ts),
            tx_signature: format!("sig-{seq}"),
            tx_index: 0,
            event_index: 0,
            event: PumpEvent::Trade(TradeEvent {
                mint: mint.to_string(),
                sol_amount,
                token_amount,
                is_buy,
                user: user.to_string(),
                timestamp: ts,
                virtual_sol_reserves: 1_000_000_000 + seq,
                virtual_token_reserves: 1_000_000_000 - seq,
                real_sol_reserves: 100_000_000 + seq,
                real_token_reserves: 800_000_000 - seq,
                fee_recipient: "fee".to_string(),
                fee_basis_points: 0,
                fee: 0,
                creator: "creator".to_string(),
                creator_fee_basis_points: 0,
                creator_fee: 0,
                track_volume: true,
                total_unclaimed_tokens: 0,
                total_claimed_tokens: 0,
                current_sol_volume: sol_amount,
                last_update_timestamp: ts,
                ix_name: if is_buy {
                    "buy".to_string()
                } else {
                    "sell".to_string()
                },
                mayhem_mode: false,
                cashback_fee_basis_points: 0,
                cashback: 0,
            }),
        }
    }

    #[test]
    fn resolve_strategy_args_uses_file_values_when_cli_is_default() {
        let path = write_temp_strategy_file(
            r#"
[strategy]
strategy = "early_flow"
buy_sol = 0.35
max_age_secs = 12
max_sell_count = 0
"#,
        );

        let mut args = sample_strategy_args();
        args.strategy_config = Some(path.clone());

        let resolved = resolve_strategy_args(&args).expect("strategy args should resolve");
        assert_eq!(resolved.strategy, "early_flow");
        assert_eq!(resolved.buy_sol, 0.35);
        assert_eq!(resolved.max_age_secs, 12);
        assert_eq!(resolved.max_sell_count, 0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn resolve_strategy_args_prefers_cli_over_file() {
        let path = write_temp_strategy_file(
            r#"
[strategy]
strategy = "early_flow"
buy_sol = 0.35
max_age_secs = 12
take_profit_bps = 999
"#,
        );

        let mut args = sample_strategy_args();
        args.strategy_config = Some(path.clone());
        args.strategy = "noop".to_string();
        args.buy_sol = 0.5;
        args.take_profit_bps = 3000;

        let resolved = resolve_strategy_args(&args).expect("strategy args should resolve");
        assert_eq!(resolved.strategy, "noop");
        assert_eq!(resolved.buy_sol, 0.5);
        assert_eq!(resolved.take_profit_bps, 3000);
        assert_eq!(resolved.max_age_secs, 12);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn build_strategy_and_broker_rejects_unknown_strategy() {
        let mut args = sample_strategy_args();
        args.strategy = "not_real".to_string();

        let error = build_strategy_and_broker(&args).expect_err("invalid strategy should fail");
        assert!(error.to_string().contains("invalid --strategy"));
    }

    #[test]
    fn run_strategy_with_noop_and_empty_events_keeps_starting_balance() {
        let mut args = sample_strategy_args();
        args.strategy = "noop".to_string();
        args.starting_sol = 12.5;

        let execution = run_strategy(Vec::new(), &args).expect("empty replay should run");
        assert_eq!(execution.result.report.strategy.name, "noop_strategy");
        assert_eq!(execution.result.report.processed_events, 0);
        assert_eq!(execution.result.report.fills, 0);
        assert_eq!(execution.result.report.rejections, 0);
        assert_eq!(execution.result.report.open_positions, 0);
        assert_eq!(execution.result.report.ending_cash_lamports, 12_500_000_000);
        assert_eq!(
            execution.result.report.ending_equity_lamports,
            12_500_000_000
        );
        assert_eq!(execution.final_position_snapshot.open_positions, 0);
        assert_eq!(execution.final_position_snapshot.pending_orders, 0);
    }

    #[test]
    fn run_strategy_executes_momentum_round_trip_on_sample_events() {
        let mint = "mint-1";
        let events = vec![
            mint_created_event(1, 1, mint, 100),
            trade_event(2, 2, mint, "buyer-1", 101, 500_000_000, 500_000_000, true),
            trade_event(3, 3, mint, "buyer-2", 102, 500_000_000, 500_000_000, true),
            trade_event(4, 4, mint, "buyer-3", 103, 600_000_000, 300_000_000, true),
            trade_event(5, 5, mint, "buyer-4", 104, 1_200_000_000, 300_000_000, true),
            trade_event(
                6,
                6,
                mint,
                "seller-1",
                105,
                1_200_000_000,
                300_000_000,
                false,
            ),
        ];

        let mut args = sample_strategy_args();
        args.strategy = "momentum".to_string();
        args.starting_sol = 1.0;
        args.buy_sol = 0.1;
        args.min_buy_count = 2;
        args.min_unique_buyers = 2;
        args.min_net_buy_sol = 0.9;
        args.max_age_secs = 60;
        args.take_profit_bps = 1_000;
        args.stop_loss_bps = 5_000;
        args.max_hold_secs = 300;
        args.trading_fee_bps = 0;
        args.slippage_bps = 0;

        let execution = run_strategy(events, &args).expect("sample replay should run");
        assert_eq!(execution.result.report.strategy.name, "momentum_strategy");
        assert_eq!(execution.result.report.processed_events, 6);
        assert_eq!(execution.result.report.fills, 2);
        assert_eq!(execution.result.report.rejections, 0);
        assert_eq!(execution.result.report.open_positions, 0);
        assert_eq!(execution.result.fills.len(), 2);
        assert_eq!(execution.result.fills[0].side, OrderSide::Buy);
        assert_eq!(execution.result.fills[1].side, OrderSide::Sell);
        assert_eq!(execution.result.report.ending_cash_lamports, 1_100_000_000);
        assert_eq!(
            execution.result.report.ending_equity_lamports,
            1_100_000_000
        );
        assert_eq!(execution.final_position_snapshot.open_positions, 0);
        assert_eq!(
            execution.final_position_snapshot.cash_lamports,
            1_100_000_000
        );
        assert_eq!(
            execution.final_position_snapshot.equity_lamports,
            1_100_000_000
        );
    }
}
