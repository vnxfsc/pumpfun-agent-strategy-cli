use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use pump_agent_core::model::FillReport;
use pump_agent_core::{BacktestReport, EventEnvelope, MarketState, PumpEvent};

use crate::strategy::{
    StrategyConfig, StrategyExecution, SweepConfig, build_sweep_variants, run_strategy,
};

const MATCH_TOLERANCE_SECS: i64 = 15;

#[derive(Debug, Clone, serde::Serialize)]
pub struct WalletBehaviorReport {
    pub address: String,
    pub summary: WalletBehaviorSummary,
    pub entries: Vec<WalletEntryFeature>,
    pub roundtrips: Vec<WalletRoundtrip>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WalletBehaviorSummary {
    pub entry_count: usize,
    pub roundtrip_count: usize,
    pub closed_roundtrip_count: usize,
    pub open_roundtrip_count: usize,
    pub orphan_sell_count: usize,
    pub avg_entry_age_secs: Option<f64>,
    pub avg_entry_buy_count_before: Option<f64>,
    pub avg_entry_sell_count_before: Option<f64>,
    pub avg_entry_unique_buyers_before: Option<f64>,
    pub avg_entry_total_buy_sol_before: Option<f64>,
    pub avg_entry_net_flow_sol_before: Option<f64>,
    pub avg_entry_buy_sell_ratio_before: Option<f64>,
    pub avg_entry_buy_sol: Option<f64>,
    pub avg_hold_secs_closed: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WalletEntryFeature {
    pub mint: String,
    pub entry_seq: u64,
    pub entry_slot: u64,
    pub entry_ts: Option<i64>,
    pub entry_buy_lamports: u64,
    pub age_secs_before: Option<i64>,
    pub buy_count_before: u64,
    pub sell_count_before: u64,
    pub unique_buyers_before: usize,
    pub total_buy_lamports_before: u128,
    pub net_flow_lamports_before: i128,
    pub buy_sell_ratio_before: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WalletRoundtrip {
    pub mint: String,
    pub status: String,
    pub entry_ts: Option<i64>,
    pub exit_ts: Option<i64>,
    pub hold_secs: Option<i64>,
    pub gross_buy_lamports: u64,
    pub gross_sell_lamports: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CloneScoreBreakdown {
    pub entry_timing_similarity: f64,
    pub hold_time_similarity: f64,
    pub size_profile_similarity: f64,
    pub token_selection_similarity: f64,
    pub exit_behavior_similarity: f64,
    pub count_alignment: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CloneScore {
    pub overall: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub matched_entries: usize,
    pub wallet_entries: usize,
    pub strategy_entries: usize,
    pub avg_entry_delay_secs: Option<f64>,
    pub avg_hold_error_secs: Option<f64>,
    pub avg_size_error_ratio: Option<f64>,
    pub count_alignment: f64,
    pub breakdown: CloneScoreBreakdown,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StrategyCloneCandidate {
    pub args: StrategyConfig,
    pub report: BacktestReport,
    pub score: CloneScore,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CloneFitSummary {
    pub candidates: Vec<StrategyCloneCandidate>,
}

#[derive(Debug, Clone)]
struct ActiveWalletRoundtrip {
    mint: String,
    entry_ts: Option<i64>,
    gross_buy_lamports: u64,
    gross_sell_lamports: u64,
    token_balance: i128,
    last_exit_ts: Option<i64>,
}

#[derive(Debug, Clone)]
struct StrategyRoundtrip {
    mint: String,
    entry_ts: Option<i64>,
    hold_secs: Option<i64>,
    gross_buy_lamports: u64,
}

pub fn extract_wallet_behavior(address: &str, events: &[EventEnvelope]) -> WalletBehaviorReport {
    let mut market_state = MarketState::default();
    let mut active = HashMap::<String, ActiveWalletRoundtrip>::new();
    let mut entries = Vec::new();
    let mut roundtrips = Vec::new();
    let mut orphan_sell_count = 0_usize;

    for event in events {
        if let PumpEvent::Trade(trade) = &event.event
            && trade.user == address
        {
            let pre_state = market_state.mint(&trade.mint);
            let balance_before = active
                .get(&trade.mint)
                .map(|value| value.token_balance)
                .unwrap_or_default();

            if trade.is_buy && balance_before == 0 {
                entries.push(WalletEntryFeature {
                    mint: trade.mint.clone(),
                    entry_seq: event.seq,
                    entry_slot: event.slot,
                    entry_ts: Some(trade.timestamp),
                    entry_buy_lamports: trade.sol_amount,
                    age_secs_before: pre_state.and_then(|state| state.age_secs_at(trade.timestamp)),
                    buy_count_before: pre_state.map(|state| state.buy_count).unwrap_or_default(),
                    sell_count_before: pre_state.map(|state| state.sell_count).unwrap_or_default(),
                    unique_buyers_before: pre_state
                        .map(|state| state.unique_buyer_count())
                        .unwrap_or_default(),
                    total_buy_lamports_before: pre_state
                        .map(|state| state.buy_volume_lamports)
                        .unwrap_or_default(),
                    net_flow_lamports_before: pre_state
                        .map(|state| state.net_flow_lamports)
                        .unwrap_or_default(),
                    buy_sell_ratio_before: pre_state
                        .map(|state| state.buy_count as f64 / state.sell_count.max(1) as f64)
                        .unwrap_or(0.0),
                });
            }

            if trade.is_buy {
                let state =
                    active
                        .entry(trade.mint.clone())
                        .or_insert_with(|| ActiveWalletRoundtrip {
                            mint: trade.mint.clone(),
                            entry_ts: Some(trade.timestamp),
                            gross_buy_lamports: 0,
                            gross_sell_lamports: 0,
                            token_balance: 0,
                            last_exit_ts: None,
                        });
                state.gross_buy_lamports =
                    state.gross_buy_lamports.saturating_add(trade.sol_amount);
                state.token_balance += i128::from(trade.token_amount);
            } else if balance_before <= 0 {
                orphan_sell_count += 1;
            } else {
                let mut should_close = false;
                if let Some(state) = active.get_mut(&trade.mint) {
                    let matched_sell = i128::from(trade.token_amount).min(state.token_balance);
                    state.gross_sell_lamports =
                        state.gross_sell_lamports.saturating_add(trade.sol_amount);
                    state.token_balance -= matched_sell;
                    state.last_exit_ts = Some(trade.timestamp);
                    if state.token_balance == 0 {
                        should_close = true;
                    }
                    if i128::from(trade.token_amount) > matched_sell {
                        orphan_sell_count += 1;
                    }
                }

                if should_close && let Some(state) = active.remove(&trade.mint) {
                    roundtrips.push(WalletRoundtrip {
                        mint: state.mint,
                        status: "closed".to_string(),
                        entry_ts: state.entry_ts,
                        exit_ts: state.last_exit_ts,
                        hold_secs: match (state.entry_ts, state.last_exit_ts) {
                            (Some(entry), Some(exit)) => Some(exit - entry),
                            _ => None,
                        },
                        gross_buy_lamports: state.gross_buy_lamports,
                        gross_sell_lamports: state.gross_sell_lamports,
                    });
                }
            }
        }

        market_state.apply(event);
    }

    for state in active.into_values() {
        roundtrips.push(WalletRoundtrip {
            mint: state.mint,
            status: "open".to_string(),
            entry_ts: state.entry_ts,
            exit_ts: state.last_exit_ts,
            hold_secs: match (state.entry_ts, state.last_exit_ts) {
                (Some(entry), Some(exit)) => Some(exit - entry),
                _ => None,
            },
            gross_buy_lamports: state.gross_buy_lamports,
            gross_sell_lamports: state.gross_sell_lamports,
        });
    }

    roundtrips.sort_by(|left, right| right.entry_ts.cmp(&left.entry_ts));
    let summary = summarize_wallet_behavior(&entries, &roundtrips, orphan_sell_count);

    WalletBehaviorReport {
        address: address.to_string(),
        summary,
        entries,
        roundtrips,
    }
}

pub fn score_strategy_execution(
    wallet: &WalletBehaviorReport,
    args: &StrategyConfig,
    execution: &StrategyExecution,
) -> StrategyCloneCandidate {
    let strategy_roundtrips = derive_strategy_roundtrips(&execution.result.fills);
    let score = score_clone_similarity(wallet, &strategy_roundtrips);

    StrategyCloneCandidate {
        args: args.clone(),
        report: execution.result.report.clone(),
        score,
    }
}

pub fn run_clone_fit(
    events: &[EventEnvelope],
    wallet: &WalletBehaviorReport,
    variants: Vec<StrategyConfig>,
) -> Result<CloneFitSummary> {
    let mut candidates = Vec::with_capacity(variants.len());
    for variant in variants {
        let execution = run_strategy(events.to_vec(), &variant)?;
        candidates.push(score_strategy_execution(wallet, &variant, &execution));
    }

    candidates.sort_by(|left, right| {
        right
            .score
            .overall
            .total_cmp(&left.score.overall)
            .then_with(|| right.score.f1.total_cmp(&left.score.f1))
            .then_with(|| {
                right
                    .report
                    .ending_equity_lamports
                    .cmp(&left.report.ending_equity_lamports)
            })
    });

    Ok(CloneFitSummary { candidates })
}

pub fn default_strategy_config_for_family(family: &str) -> Result<StrategyConfig> {
    match family {
        "momentum" => Ok(StrategyConfig {
            strategy: "momentum".to_string(),
            strategy_config: None,
            starting_sol: 10.0,
            buy_sol: 0.2,
            max_age_secs: 45,
            min_buy_count: 3,
            min_unique_buyers: 3,
            min_net_buy_sol: 0.3,
            take_profit_bps: 2_500,
            stop_loss_bps: 1_200,
            max_hold_secs: 90,
            min_total_buy_sol: 0.8,
            max_sell_count: 1,
            min_buy_sell_ratio: 4.0,
            max_concurrent_positions: 3,
            exit_on_sell_count: 3,
            trading_fee_bps: 100,
            slippage_bps: 50,
        }),
        "early_flow" | "early-flow" => Ok(StrategyConfig {
            strategy: "early_flow".to_string(),
            strategy_config: None,
            starting_sol: 10.0,
            buy_sol: 0.15,
            max_age_secs: 20,
            min_buy_count: 4,
            min_unique_buyers: 4,
            min_net_buy_sol: 0.3,
            take_profit_bps: 1_800,
            stop_loss_bps: 900,
            max_hold_secs: 45,
            min_total_buy_sol: 0.8,
            max_sell_count: 1,
            min_buy_sell_ratio: 4.0,
            max_concurrent_positions: 3,
            exit_on_sell_count: 3,
            trading_fee_bps: 100,
            slippage_bps: 50,
        }),
        "breakout" => Ok(StrategyConfig {
            strategy: "breakout".to_string(),
            strategy_config: None,
            starting_sol: 10.0,
            buy_sol: 0.18,
            max_age_secs: 35,
            min_buy_count: 5,
            min_unique_buyers: 5,
            min_net_buy_sol: 0.7,
            take_profit_bps: 2_200,
            stop_loss_bps: 900,
            max_hold_secs: 75,
            min_total_buy_sol: 1.2,
            max_sell_count: 2,
            min_buy_sell_ratio: 3.5,
            max_concurrent_positions: 3,
            exit_on_sell_count: 4,
            trading_fee_bps: 100,
            slippage_bps: 50,
        }),
        "liquidity_follow" | "liquidity-follow" => Ok(StrategyConfig {
            strategy: "liquidity_follow".to_string(),
            strategy_config: None,
            starting_sol: 10.0,
            buy_sol: 0.18,
            max_age_secs: 55,
            min_buy_count: 4,
            min_unique_buyers: 4,
            min_net_buy_sol: 0.5,
            take_profit_bps: 2_000,
            stop_loss_bps: 1_000,
            max_hold_secs: 120,
            min_total_buy_sol: 1.5,
            max_sell_count: 3,
            min_buy_sell_ratio: 2.5,
            max_concurrent_positions: 4,
            exit_on_sell_count: 4,
            trading_fee_bps: 100,
            slippage_bps: 50,
        }),
        other => bail!("unsupported strategy family '{}'", other),
    }
}

pub fn build_fit_variants(
    base: &StrategyConfig,
    sweep: &SweepConfig,
) -> Result<Vec<StrategyConfig>> {
    build_sweep_variants(base, sweep)
}

fn summarize_wallet_behavior(
    entries: &[WalletEntryFeature],
    roundtrips: &[WalletRoundtrip],
    orphan_sell_count: usize,
) -> WalletBehaviorSummary {
    let closed_roundtrips = roundtrips
        .iter()
        .filter(|roundtrip| roundtrip.status == "closed")
        .collect::<Vec<_>>();

    WalletBehaviorSummary {
        entry_count: entries.len(),
        roundtrip_count: roundtrips.len(),
        closed_roundtrip_count: closed_roundtrips.len(),
        open_roundtrip_count: roundtrips.len().saturating_sub(closed_roundtrips.len()),
        orphan_sell_count,
        avg_entry_age_secs: average_option_i64(
            entries.iter().filter_map(|entry| entry.age_secs_before),
        ),
        avg_entry_buy_count_before: average_u64(entries.iter().map(|entry| entry.buy_count_before)),
        avg_entry_sell_count_before: average_u64(
            entries.iter().map(|entry| entry.sell_count_before),
        ),
        avg_entry_unique_buyers_before: average_usize(
            entries.iter().map(|entry| entry.unique_buyers_before),
        ),
        avg_entry_total_buy_sol_before: average_f64(
            entries
                .iter()
                .map(|entry| lamports_i128_to_sol(entry.total_buy_lamports_before as i128)),
        ),
        avg_entry_net_flow_sol_before: average_f64(
            entries
                .iter()
                .map(|entry| lamports_i128_to_sol(entry.net_flow_lamports_before)),
        ),
        avg_entry_buy_sell_ratio_before: average_f64(
            entries.iter().map(|entry| entry.buy_sell_ratio_before),
        ),
        avg_entry_buy_sol: average_f64(
            entries
                .iter()
                .map(|entry| lamports_to_sol(entry.entry_buy_lamports)),
        ),
        avg_hold_secs_closed: average_option_i64(
            closed_roundtrips
                .iter()
                .filter_map(|roundtrip| roundtrip.hold_secs),
        ),
    }
}

fn derive_strategy_roundtrips(fills: &[FillReport]) -> Vec<StrategyRoundtrip> {
    #[derive(Debug, Clone)]
    struct ActiveStrategyRoundtrip {
        mint: String,
        entry_ts: Option<i64>,
        gross_buy_lamports: u64,
    }

    let mut active = HashMap::<String, ActiveStrategyRoundtrip>::new();
    let mut roundtrips = Vec::new();

    for fill in fills {
        match fill.side {
            pump_agent_core::OrderSide::Buy => {
                active.insert(
                    fill.mint.clone(),
                    ActiveStrategyRoundtrip {
                        mint: fill.mint.clone(),
                        entry_ts: fill.timestamp,
                        gross_buy_lamports: fill.lamports,
                    },
                );
            }
            pump_agent_core::OrderSide::Sell => {
                if let Some(state) = active.remove(&fill.mint) {
                    roundtrips.push(StrategyRoundtrip {
                        mint: state.mint,
                        entry_ts: state.entry_ts,
                        hold_secs: match (state.entry_ts, fill.timestamp) {
                            (Some(entry), Some(exit)) => Some(exit - entry),
                            _ => None,
                        },
                        gross_buy_lamports: state.gross_buy_lamports,
                    });
                }
            }
        }
    }

    roundtrips.extend(active.into_values().map(|state| StrategyRoundtrip {
        mint: state.mint,
        entry_ts: state.entry_ts,
        hold_secs: None,
        gross_buy_lamports: state.gross_buy_lamports,
    }));

    roundtrips.sort_by(|left, right| right.entry_ts.cmp(&left.entry_ts));
    roundtrips
}

fn score_clone_similarity(
    wallet: &WalletBehaviorReport,
    strategy_roundtrips: &[StrategyRoundtrip],
) -> CloneScore {
    let mut matched_strategy_indices = HashSet::<usize>::new();
    let mut matched_entries = 0_usize;
    let mut entry_delay_secs = Vec::new();
    let mut hold_error_secs = Vec::new();
    let mut size_error_ratio = Vec::new();
    let mut exit_alignment = Vec::new();

    let wallet_roundtrips = wallet.roundtrips.iter().collect::<Vec<_>>();
    let wallet_mints = wallet_roundtrips
        .iter()
        .map(|roundtrip| roundtrip.mint.as_str())
        .collect::<HashSet<_>>();
    let strategy_mints = strategy_roundtrips
        .iter()
        .map(|roundtrip| roundtrip.mint.as_str())
        .collect::<HashSet<_>>();
    for wallet_roundtrip in &wallet_roundtrips {
        let Some(wallet_entry_ts) = wallet_roundtrip.entry_ts else {
            continue;
        };

        let mut best_match: Option<(usize, i64)> = None;
        for (index, strategy_roundtrip) in strategy_roundtrips.iter().enumerate() {
            if matched_strategy_indices.contains(&index)
                || strategy_roundtrip.mint != wallet_roundtrip.mint
            {
                continue;
            }
            let Some(strategy_entry_ts) = strategy_roundtrip.entry_ts else {
                continue;
            };
            let delay = (strategy_entry_ts - wallet_entry_ts).abs();
            if delay > MATCH_TOLERANCE_SECS {
                continue;
            }
            match best_match {
                Some((_, best_delay)) if delay >= best_delay => {}
                _ => best_match = Some((index, delay)),
            }
        }

        if let Some((index, delay)) = best_match {
            matched_strategy_indices.insert(index);
            matched_entries += 1;
            entry_delay_secs.push(delay as f64);
            let strategy_roundtrip = &strategy_roundtrips[index];

            if let (Some(wallet_hold), Some(strategy_hold)) =
                (wallet_roundtrip.hold_secs, strategy_roundtrip.hold_secs)
            {
                hold_error_secs.push((wallet_hold - strategy_hold).abs() as f64);
            }
            let wallet_closed = wallet_roundtrip.status == "closed";
            let strategy_closed = strategy_roundtrip.hold_secs.is_some();
            exit_alignment.push(if wallet_closed == strategy_closed {
                1.0
            } else {
                0.0
            });
            if wallet_roundtrip.gross_buy_lamports > 0 {
                let diff = wallet_roundtrip
                    .gross_buy_lamports
                    .abs_diff(strategy_roundtrip.gross_buy_lamports)
                    as f64
                    / wallet_roundtrip.gross_buy_lamports as f64;
                size_error_ratio.push(diff);
            }
        }
    }

    let wallet_entries = wallet.roundtrips.len();
    let strategy_entries = strategy_roundtrips.len();
    let precision = ratio(matched_entries, strategy_entries);
    let recall = ratio(matched_entries, wallet_entries);
    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };
    let avg_entry_delay_secs = average_f64(entry_delay_secs.iter().copied());
    let avg_hold_error_secs = average_f64(hold_error_secs.iter().copied());
    let avg_size_error_ratio = average_f64(size_error_ratio.iter().copied());
    let count_alignment = if wallet_entries == 0 && strategy_entries == 0 {
        1.0
    } else {
        1.0 - (wallet_entries.abs_diff(strategy_entries) as f64
            / wallet_entries.max(strategy_entries).max(1) as f64)
    };
    let delay_score = avg_entry_delay_secs
        .map(|value| 1.0 / (1.0 + value / 10.0))
        .unwrap_or(0.0);
    let hold_score = avg_hold_error_secs
        .map(|value| 1.0 / (1.0 + value / 15.0))
        .unwrap_or(0.0);
    let size_score = avg_size_error_ratio
        .map(|value| (1.0 - value).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let token_selection_similarity = if wallet_mints.is_empty() && strategy_mints.is_empty() {
        1.0
    } else {
        wallet_mints.intersection(&strategy_mints).count() as f64
            / wallet_mints.union(&strategy_mints).count().max(1) as f64
    };
    let exit_behavior_similarity = average_f64(exit_alignment.iter().copied()).unwrap_or(0.0);
    let breakdown = CloneScoreBreakdown {
        entry_timing_similarity: delay_score,
        hold_time_similarity: hold_score,
        size_profile_similarity: size_score,
        token_selection_similarity,
        exit_behavior_similarity,
        count_alignment,
    };

    CloneScore {
        overall: 0.25 * f1
            + 0.15 * breakdown.entry_timing_similarity
            + 0.15 * breakdown.hold_time_similarity
            + 0.1 * breakdown.size_profile_similarity
            + 0.15 * breakdown.token_selection_similarity
            + 0.1 * breakdown.exit_behavior_similarity
            + 0.1 * breakdown.count_alignment,
        precision,
        recall,
        f1,
        matched_entries,
        wallet_entries,
        strategy_entries,
        avg_entry_delay_secs,
        avg_hold_error_secs,
        avg_size_error_ratio,
        count_alignment,
        breakdown,
    }
}

fn average_option_i64(values: impl Iterator<Item = i64>) -> Option<f64> {
    average_f64(values.map(|value| value as f64))
}

fn average_u64(values: impl Iterator<Item = u64>) -> Option<f64> {
    average_f64(values.map(|value| value as f64))
}

fn average_usize(values: impl Iterator<Item = usize>) -> Option<f64> {
    average_f64(values.map(|value| value as f64))
}

fn average_f64(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut count = 0_u64;
    let mut sum = 0.0_f64;
    for value in values {
        sum += value;
        count += 1;
    }
    (count > 0).then_some(sum / count as f64)
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn lamports_to_sol(lamports: u64) -> f64 {
    lamports as f64 / 1_000_000_000.0
}

fn lamports_i128_to_sol(lamports: i128) -> f64 {
    lamports as f64 / 1_000_000_000.0
}

#[cfg(test)]
mod tests {
    use pump_agent_core::{MintCreatedEvent, OrderSide, PumpEvent, StrategyMetadata, TradeEvent};

    use super::*;

    fn mint_created(seq: u64, mint: &str, ts: i64) -> EventEnvelope {
        EventEnvelope {
            seq,
            slot: seq,
            block_time: Some(ts),
            tx_signature: format!("sig-{seq}"),
            tx_index: 0,
            event_index: 0,
            event: PumpEvent::MintCreated(MintCreatedEvent {
                mint: mint.to_string(),
                bonding_curve: "curve".to_string(),
                user: "creator".to_string(),
                creator: "creator".to_string(),
                name: "Token".to_string(),
                symbol: "TOK".to_string(),
                uri: String::new(),
                timestamp: ts,
                virtual_token_reserves: 1_000,
                virtual_sol_reserves: 1_000,
                real_token_reserves: 1_000,
                token_total_supply: 1_000,
                token_program: String::new(),
                is_mayhem_mode: false,
                is_cashback_enabled: false,
            }),
        }
    }

    fn trade(
        seq: u64,
        mint: &str,
        user: &str,
        is_buy: bool,
        sol: u64,
        token: u64,
        ts: i64,
    ) -> EventEnvelope {
        EventEnvelope {
            seq,
            slot: seq,
            block_time: Some(ts),
            tx_signature: format!("sig-{seq}"),
            tx_index: 0,
            event_index: 0,
            event: PumpEvent::Trade(TradeEvent {
                mint: mint.to_string(),
                sol_amount: sol,
                token_amount: token,
                is_buy,
                user: user.to_string(),
                timestamp: ts,
                virtual_sol_reserves: 1_000,
                virtual_token_reserves: 1_000,
                real_sol_reserves: 1_000,
                real_token_reserves: 1_000,
                fee_recipient: String::new(),
                fee_basis_points: 0,
                fee: 0,
                creator: "creator".to_string(),
                creator_fee_basis_points: 0,
                creator_fee: 0,
                track_volume: true,
                total_unclaimed_tokens: 0,
                total_claimed_tokens: 0,
                current_sol_volume: 0,
                last_update_timestamp: ts,
                ix_name: if is_buy { "buy" } else { "sell" }.to_string(),
                mayhem_mode: false,
                cashback_fee_basis_points: 0,
                cashback: 0,
            }),
        }
    }

    #[test]
    fn extracts_wallet_entries_and_roundtrips() {
        let events = vec![
            mint_created(1, "mint-a", 10),
            trade(2, "mint-a", "other", true, 100, 1_000, 11),
            trade(3, "mint-a", "wallet", true, 120, 1_000, 12),
            trade(4, "mint-a", "wallet", false, 150, 1_000, 14),
        ];

        let report = extract_wallet_behavior("wallet", &events);
        assert_eq!(report.summary.entry_count, 1);
        assert_eq!(report.summary.roundtrip_count, 1);
        assert_eq!(report.summary.closed_roundtrip_count, 1);
        assert_eq!(report.entries[0].buy_count_before, 1);
        assert_eq!(report.roundtrips[0].hold_secs, Some(2));
    }

    #[test]
    fn scores_matching_strategy_roundtrips() {
        let wallet = WalletBehaviorReport {
            address: "wallet".to_string(),
            summary: WalletBehaviorSummary {
                entry_count: 1,
                roundtrip_count: 1,
                closed_roundtrip_count: 1,
                open_roundtrip_count: 0,
                orphan_sell_count: 0,
                avg_entry_age_secs: Some(1.0),
                avg_entry_buy_count_before: Some(1.0),
                avg_entry_sell_count_before: Some(0.0),
                avg_entry_unique_buyers_before: Some(1.0),
                avg_entry_total_buy_sol_before: Some(0.1),
                avg_entry_net_flow_sol_before: Some(0.1),
                avg_entry_buy_sell_ratio_before: Some(1.0),
                avg_entry_buy_sol: Some(0.12),
                avg_hold_secs_closed: Some(2.0),
            },
            entries: Vec::new(),
            roundtrips: vec![WalletRoundtrip {
                mint: "mint-a".to_string(),
                status: "closed".to_string(),
                entry_ts: Some(10),
                exit_ts: Some(20),
                hold_secs: Some(10),
                gross_buy_lamports: 100,
                gross_sell_lamports: 120,
            }],
        };
        let execution = StrategyExecution {
            result: pump_agent_core::BacktestRunResult {
                report: BacktestReport {
                    strategy: StrategyMetadata { name: "test" },
                    processed_events: 0,
                    fills: 2,
                    rejections: 0,
                    ending_cash_lamports: 0,
                    ending_equity_lamports: 0,
                    open_positions: 0,
                },
                fills: vec![
                    FillReport {
                        order_id: 1,
                        mint: "mint-a".to_string(),
                        side: OrderSide::Buy,
                        lamports: 110,
                        token_amount: 1_000,
                        fee_lamports: 0,
                        execution_price_lamports_per_token: 0.1,
                        timestamp: Some(12),
                        reason: String::new(),
                    },
                    FillReport {
                        order_id: 2,
                        mint: "mint-a".to_string(),
                        side: OrderSide::Sell,
                        lamports: 130,
                        token_amount: 1_000,
                        fee_lamports: 0,
                        execution_price_lamports_per_token: 0.1,
                        timestamp: Some(21),
                        reason: String::new(),
                    },
                ],
                rejections: Vec::new(),
            },
            final_position_snapshot: pump_agent_core::PositionSnapshotInput {
                snapshot_kind: "final".to_string(),
                event_seq: None,
                event_slot: None,
                snapshot_at: None,
                cash_lamports: 0,
                equity_lamports: 0,
                pending_orders: 0,
                open_positions: 0,
                positions: serde_json::Value::Array(Vec::new()),
            },
        };

        let args = default_strategy_config_for_family("momentum").expect("family should exist");
        let candidate = score_strategy_execution(&wallet, &args, &execution);
        assert_eq!(candidate.score.matched_entries, 1);
        assert!(candidate.score.overall > 0.7);
    }
}
