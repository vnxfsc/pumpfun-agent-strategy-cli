use anyhow::Result;
use pump_agent_core::{
    BacktestReport, BacktestRunResult, PgEventStore, PositionSnapshotInput,
    StrategyRunPersistOptions,
};

use crate::args::StrategyArgs;

use super::strategy::resolve_strategy_args;

pub fn serialize_strategy_config(strategy_args: &StrategyArgs) -> Result<serde_json::Value> {
    let resolved = resolve_strategy_args(strategy_args)?;
    Ok(serde_json::to_value(&resolved)?)
}

pub fn deserialize_strategy_config(value: &serde_json::Value) -> Result<StrategyArgs> {
    Ok(serde_json::from_value(value.clone())?)
}

pub async fn persist_run(
    store: &PgEventStore,
    source_type: &str,
    source_ref: &str,
    strategy_args: &StrategyArgs,
    result: &BacktestRunResult,
    options: StrategyRunPersistOptions,
) -> Result<i64> {
    let config = serialize_strategy_config(strategy_args)?;
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
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    format!("{}-{}", prefix, millis)
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

pub fn push_position_snapshot(
    snapshots: &mut Vec<PositionSnapshotInput>,
    snapshot: PositionSnapshotInput,
) {
    const MAX_POSITION_SNAPSHOTS: usize = 256;
    if snapshots.len() >= MAX_POSITION_SNAPSHOTS {
        snapshots.remove(0);
    }
    snapshots.push(snapshot);
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use pump_agent_core::{
        BacktestReport, BrokerSnapshot, EventEnvelope, MarketState, MintCreatedEvent, PumpEvent,
        TradeEvent, broker::Position,
    };

    use crate::args::StrategyArgs;

    use super::{
        build_position_snapshot_input, generate_run_group_id, push_position_snapshot,
        serialize_strategy_config,
    };

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

    fn sample_report() -> BacktestReport {
        BacktestReport {
            strategy: pump_agent_core::StrategyMetadata {
                name: "test_strategy",
            },
            processed_events: 5,
            fills: 2,
            rejections: 0,
            ending_cash_lamports: 900_000_000,
            ending_equity_lamports: 1_100_000_000,
            open_positions: 1,
        }
    }

    fn apply_market_state(mint: &str) -> MarketState {
        let mut state = MarketState::default();
        state.apply(&EventEnvelope {
            seq: 1,
            slot: 1,
            block_time: Some(100),
            tx_signature: "sig-1".to_string(),
            tx_index: 0,
            event_index: 0,
            event: PumpEvent::MintCreated(MintCreatedEvent {
                mint: mint.to_string(),
                bonding_curve: "curve".to_string(),
                user: "creator".to_string(),
                creator: "creator".to_string(),
                name: "Token".to_string(),
                symbol: "TOK".to_string(),
                uri: "uri".to_string(),
                timestamp: 100,
                virtual_token_reserves: 1_000_000,
                virtual_sol_reserves: 2_000_000,
                real_token_reserves: 900_000,
                token_total_supply: 1_000_000,
                token_program: "Tokenkeg".to_string(),
                is_mayhem_mode: false,
                is_cashback_enabled: false,
            }),
        });
        state.apply(&EventEnvelope {
            seq: 2,
            slot: 2,
            block_time: Some(101),
            tx_signature: "sig-2".to_string(),
            tx_index: 0,
            event_index: 0,
            event: PumpEvent::Trade(TradeEvent {
                mint: mint.to_string(),
                sol_amount: 200,
                token_amount: 100,
                is_buy: true,
                user: "buyer".to_string(),
                timestamp: 101,
                virtual_sol_reserves: 2_100_000,
                virtual_token_reserves: 999_900,
                real_sol_reserves: 100,
                real_token_reserves: 899_900,
                fee_recipient: "fee".to_string(),
                fee_basis_points: 0,
                fee: 0,
                creator: "creator".to_string(),
                creator_fee_basis_points: 0,
                creator_fee: 0,
                track_volume: true,
                total_unclaimed_tokens: 0,
                total_claimed_tokens: 0,
                current_sol_volume: 200,
                last_update_timestamp: 101,
                ix_name: "buy".to_string(),
                mayhem_mode: false,
                cashback_fee_basis_points: 0,
                cashback: 0,
            }),
        });
        state
    }

    #[test]
    fn serialize_strategy_config_resolves_file_overrides() {
        let mut args = sample_strategy_args();
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        path.push(format!("pump-agent-persist-{unique}.toml"));
        fs::write(
            &path,
            r#"
[strategy]
strategy = "early_flow"
buy_sol = 0.33
"#,
        )
        .expect("config file should be written");
        args.strategy_config = Some(path.clone());

        let config = serialize_strategy_config(&args).expect("config should serialize");
        assert_eq!(
            config.get("strategy").and_then(|v| v.as_str()),
            Some("early_flow")
        );
        assert_eq!(config.get("buy_sol").and_then(|v| v.as_f64()), Some(0.33));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn generate_run_group_id_keeps_prefix_and_numeric_suffix() {
        let id = generate_run_group_id("sweep");
        let (prefix, suffix) = id.split_once('-').expect("id should contain delimiter");
        assert_eq!(prefix, "sweep");
        assert!(suffix.parse::<u128>().is_ok(), "suffix should be numeric");
    }

    #[test]
    fn build_position_snapshot_input_includes_position_payload() {
        let mint = "mint-1";
        let market_state = apply_market_state(mint);
        let mut positions = HashMap::new();
        positions.insert(
            mint.to_string(),
            Position {
                mint: mint.to_string(),
                token_amount: 50,
                average_entry_price_lamports_per_token: 1.5,
                entry_notional_lamports: 75,
                opened_at: Some(100),
                last_mark_price_lamports_per_token: 1.5,
                realized_pnl_lamports: 0,
            },
        );
        let broker = BrokerSnapshot {
            cash_lamports: 900_000_000,
            positions,
            pending_orders: 2,
        };

        let snapshot = build_position_snapshot_input(
            "heartbeat",
            &sample_report(),
            &broker,
            &market_state,
            Some(10),
            Some(20),
            Some(123),
        );

        assert_eq!(snapshot.snapshot_kind, "heartbeat");
        assert_eq!(snapshot.event_seq, Some(10));
        assert_eq!(snapshot.event_slot, Some(20));
        assert_eq!(snapshot.snapshot_at, Some(123));
        assert_eq!(snapshot.cash_lamports, 900_000_000);
        assert_eq!(snapshot.equity_lamports, 1_100_000_000);
        assert_eq!(snapshot.pending_orders, 2);
        assert_eq!(snapshot.open_positions, 1);

        let positions = snapshot
            .positions
            .as_array()
            .expect("positions should be array");
        assert_eq!(positions.len(), 1);
        let position = &positions[0];
        assert_eq!(position.get("mint").and_then(|v| v.as_str()), Some(mint));
        assert_eq!(position.get("symbol").and_then(|v| v.as_str()), Some("TOK"));
        assert_eq!(
            position.get("token_amount").and_then(|v| v.as_u64()),
            Some(50)
        );
        assert_eq!(
            position
                .get("entry_notional_lamports")
                .and_then(|v| v.as_u64()),
            Some(75)
        );
        assert_eq!(
            position.get("opened_at").and_then(|v| v.as_i64()),
            Some(100)
        );
        assert_eq!(
            position
                .get("mark_price_lamports_per_token")
                .and_then(|v| v.as_f64()),
            Some(2.0)
        );
        let pnl = position
            .get("pnl_bps")
            .and_then(|v| v.as_f64())
            .expect("pnl should exist");
        assert!((pnl - 3333.333333333333).abs() < 1e-6);
    }

    #[test]
    fn push_position_snapshot_caps_history_at_256() {
        let mut snapshots = Vec::new();
        for index in 0..300_u64 {
            push_position_snapshot(
                &mut snapshots,
                pump_agent_core::PositionSnapshotInput {
                    snapshot_kind: format!("kind-{index}"),
                    event_seq: Some(index),
                    event_slot: Some(index),
                    snapshot_at: Some(index as i64),
                    cash_lamports: index,
                    equity_lamports: index,
                    pending_orders: 0,
                    open_positions: 0,
                    positions: serde_json::json!([]),
                },
            );
        }

        assert_eq!(snapshots.len(), 256);
        assert_eq!(snapshots.first().and_then(|s| s.event_seq), Some(44));
        assert_eq!(snapshots.last().and_then(|s| s.event_seq), Some(299));
    }
}
