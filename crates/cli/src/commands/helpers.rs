use std::collections::VecDeque;

use pump_agent_core::{BacktestReport, EventEnvelope, ExecutionReport, PumpEvent};

use crate::config::{lamports_i128_to_sol, lamports_to_sol, lamports_u128_to_sol};

pub const SCHEMA_SQL: &str = include_str!("../../../../schema/postgres.sql");

pub fn print_event(event: &EventEnvelope) {
    match &event.event {
        PumpEvent::MintCreated(mint) => {
            println!(
                "mint_created seq={} slot={} mint={} symbol={} creator={}",
                event.seq, event.slot, mint.mint, mint.symbol, mint.creator
            );
        }
        PumpEvent::Trade(trade) => {
            println!(
                "trade seq={} slot={} mint={} side={} sol={} token={}",
                event.seq,
                event.slot,
                trade.mint,
                if trade.is_buy { "buy" } else { "sell" },
                trade.sol_amount,
                trade.token_amount
            );
        }
        PumpEvent::CurveCompleted(complete) => {
            println!(
                "curve_completed seq={} slot={} mint={} user={}",
                event.seq, event.slot, complete.mint, complete.user
            );
        }
    }
}

pub fn print_report(report: BacktestReport) {
    println!("strategy      : {}", report.strategy.name);
    println!("events        : {}", report.processed_events);
    println!("fills         : {}", report.fills);
    println!("rejections    : {}", report.rejections);
    println!(
        "ending cash   : {:.6} SOL",
        lamports_to_sol(report.ending_cash_lamports)
    );
    println!(
        "ending equity : {:.6} SOL",
        lamports_to_sol(report.ending_equity_lamports)
    );
    println!("open positions: {}", report.open_positions);
}

pub fn render_live_dashboard(
    report: &BacktestReport,
    broker: &pump_agent_core::BrokerSnapshot,
    market_state: &pump_agent_core::MarketState,
    recent_activity: &VecDeque<String>,
    top_mints_limit: usize,
    position_limit: usize,
) {
    print!("\x1b[2J\x1b[H");
    println!("Live Paper Dashboard");
    println!(
        "strategy={} events={} fills={} rejections={} cash={:.6} SOL equity={:.6} SOL open_positions={} pending_orders={}",
        report.strategy.name,
        report.processed_events,
        report.fills,
        report.rejections,
        lamports_to_sol(report.ending_cash_lamports),
        lamports_to_sol(report.ending_equity_lamports),
        report.open_positions,
        broker.pending_orders
    );
    println!();

    println!("Positions");
    let mut positions = broker.positions.values().cloned().collect::<Vec<_>>();
    positions.sort_by(|left, right| left.mint.cmp(&right.mint));
    if positions.is_empty() {
        println!("  none");
    } else {
        for position in positions.into_iter().take(position_limit) {
            let mark_price = market_state
                .mint(&position.mint)
                .map(|mint| mint.last_price_lamports_per_token)
                .unwrap_or(position.last_mark_price_lamports_per_token);
            let pnl_bps = if position.average_entry_price_lamports_per_token > 0.0 {
                ((mark_price / position.average_entry_price_lamports_per_token) - 1.0) * 10_000.0
            } else {
                0.0
            };
            println!(
                "  {} qty={} entry_px={:.12} mark_px={:.12} pnl={:.0}bps",
                mint_label(&position.mint, market_state),
                position.token_amount,
                position.average_entry_price_lamports_per_token,
                mark_price,
                pnl_bps
            );
        }
    }
    println!();

    println!("Hot Mints");
    let mut hot_mints = market_state.mints().values().collect::<Vec<_>>();
    hot_mints.sort_by(|left, right| {
        right
            .last_trade_slot
            .cmp(&left.last_trade_slot)
            .then_with(|| right.buy_volume_lamports.cmp(&left.buy_volume_lamports))
    });
    if hot_mints.is_empty() {
        println!("  none");
    } else {
        for mint in hot_mints.into_iter().take(top_mints_limit) {
            println!(
                "  {} buys={} sells={} buy_sol={:.3} net_flow={:.3} last_slot={} complete={}",
                mint_label(&mint.mint, market_state),
                mint.buy_count,
                mint.sell_count,
                lamports_u128_to_sol(mint.buy_volume_lamports),
                lamports_i128_to_sol(mint.net_flow_lamports),
                mint.last_trade_slot.unwrap_or_default(),
                mint.is_complete
            );
        }
    }
    println!();

    println!("Recent Activity");
    if recent_activity.is_empty() {
        println!("  none");
    } else {
        for line in recent_activity {
            println!("  {}", line);
        }
    }
}

pub fn format_execution_report(report: &ExecutionReport) -> String {
    match report {
        ExecutionReport::Filled(fill) => format!(
            "fill order_id={} side={:?} mint={} lamports={} token_amount={} fee={} price={:.12} reason={}",
            fill.order_id,
            fill.side,
            short_mint(&fill.mint),
            fill.lamports,
            fill.token_amount,
            fill.fee_lamports,
            fill.execution_price_lamports_per_token,
            fill.reason
        ),
        ExecutionReport::Rejected(rejection) => format!(
            "reject order_id={} mint={} rejection={:?} reason={}",
            rejection.order_id,
            short_mint(&rejection.mint),
            rejection.rejection,
            rejection.reason
        ),
    }
}

pub fn push_recent_activity(recent_activity: &mut VecDeque<String>, line: String, limit: usize) {
    let limit = limit.max(1);
    if recent_activity.len() >= limit {
        recent_activity.pop_front();
    }
    recent_activity.push_back(line);
}

pub fn mint_label(mint: &str, market_state: &pump_agent_core::MarketState) -> String {
    if let Some(state) = market_state.mint(mint)
        && let Some(symbol) = state.symbol.as_deref()
        && !symbol.trim().is_empty()
    {
        return format!("{} ({})", symbol, short_mint(mint));
    }

    short_mint(mint)
}

pub fn short_mint(mint: &str) -> String {
    if mint.len() <= 12 {
        return mint.to_string();
    }
    format!("{}..{}", &mint[..6], &mint[mint.len() - 4..])
}

pub fn json_num_string(value: &serde_json::Value, key: &str) -> String {
    match value.get(key) {
        Some(serde_json::Value::Number(number)) => number.to_string(),
        Some(serde_json::Value::String(string)) => string.clone(),
        Some(other) => other.to_string(),
        None => "-".to_string(),
    }
}
