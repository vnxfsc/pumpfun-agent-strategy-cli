use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use pump_agent_app::{
    api::{compare_runs_output, wallet_dossier_output},
    clone::{
        default_strategy_config_for_family, extract_wallet_behavior, score_strategy_execution,
    },
    strategy::{deserialize_strategy_config, run_strategy},
    usecases::{CompareRunsDeltas, CompareRunsResult, LoadedCountDelta},
};

use super::{
    render::{
        render_dashboard_batch_detail, render_dashboard_compare, render_dashboard_error,
        render_dashboard_home, render_dashboard_run_detail, render_dashboard_wallet,
    },
    state::DashboardState,
};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DashboardListQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DashboardCompareQuery {
    pub left_id: Option<i64>,
    pub right_id: Option<i64>,
    pub fill_limit: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DashboardWalletQuery {
    pub address: Option<String>,
    pub top_mints_limit: Option<i64>,
    pub roundtrip_limit: Option<i64>,
    pub sample_limit: Option<usize>,
}

pub async fn dashboard_healthz() -> Html<&'static str> {
    Html("ok")
}

pub async fn dashboard_home(
    State(state): State<DashboardState>,
    Query(query): Query<DashboardListQuery>,
) -> Html<String> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    match state.store.list_strategy_runs(limit).await {
        Ok(runs) => Html(render_dashboard_home(&runs, limit)),
        Err(error) => Html(render_dashboard_error(
            "Failed To Load Runs",
            &error.to_string(),
        )),
    }
}

pub async fn dashboard_run_detail(
    State(state): State<DashboardState>,
    Path(id): Path<i64>,
) -> Html<String> {
    match state.store.inspect_strategy_run(id, 200).await {
        Ok(report) => Html(render_dashboard_run_detail(id, report)),
        Err(error) => Html(render_dashboard_error(
            "Failed To Load Run",
            &error.to_string(),
        )),
    }
}

pub async fn dashboard_compare(
    State(state): State<DashboardState>,
    Query(query): Query<DashboardCompareQuery>,
) -> Html<String> {
    let Some(left_id) = query.left_id else {
        return Html(render_dashboard_compare(
            None,
            query.left_id,
            query.right_id,
        ));
    };
    let Some(right_id) = query.right_id else {
        return Html(render_dashboard_compare(
            None,
            query.left_id,
            query.right_id,
        ));
    };

    let fill_limit = query.fill_limit.unwrap_or(20).clamp(0, 500);
    let left = match state.store.inspect_strategy_run(left_id, fill_limit).await {
        Ok(report) => report,
        Err(error) => {
            return Html(render_dashboard_error(
                "Failed To Load Left Run",
                &error.to_string(),
            ));
        }
    };
    let right = match state.store.inspect_strategy_run(right_id, fill_limit).await {
        Ok(report) => report,
        Err(error) => {
            return Html(render_dashboard_error(
                "Failed To Load Right Run",
                &error.to_string(),
            ));
        }
    };
    let (Some(left_run), Some(right_run)) = (left.run, right.run) else {
        return Html(render_dashboard_error(
            "Run Not Found",
            "One or both strategy runs do not exist.",
        ));
    };
    let left_strategy = match deserialize_strategy_config(&left_run.config) {
        Ok(config) => config,
        Err(error) => {
            return Html(render_dashboard_error(
                "Invalid Left Config",
                &error.to_string(),
            ));
        }
    };
    let right_strategy = match deserialize_strategy_config(&right_run.config) {
        Ok(config) => config,
        Err(error) => {
            return Html(render_dashboard_error(
                "Invalid Right Config",
                &error.to_string(),
            ));
        }
    };
    let left_cash = left_run
        .ending_cash_lamports
        .parse::<i128>()
        .unwrap_or_default() as f64
        / 1_000_000_000.0;
    let right_cash = right_run
        .ending_cash_lamports
        .parse::<i128>()
        .unwrap_or_default() as f64
        / 1_000_000_000.0;
    let left_equity = left_run
        .ending_equity_lamports
        .parse::<i128>()
        .unwrap_or_default() as f64
        / 1_000_000_000.0;
    let right_equity = right_run
        .ending_equity_lamports
        .parse::<i128>()
        .unwrap_or_default() as f64
        / 1_000_000_000.0;
    let deltas = CompareRunsDeltas {
        events: right_run.processed_events - left_run.processed_events,
        fills: right_run.fills - left_run.fills,
        rejections: right_run.rejections - left_run.rejections,
        cash_sol: right_cash - left_cash,
        equity_sol: right_equity - left_equity,
    };
    let output = compare_runs_output(CompareRunsResult {
        left_run,
        right_run,
        left_strategy,
        right_strategy,
        loaded_fills: LoadedCountDelta {
            left: left.fills.len(),
            right: right.fills.len(),
        },
        loaded_position_snapshots: LoadedCountDelta {
            left: left.position_snapshots.len(),
            right: right.position_snapshots.len(),
        },
        deltas,
    });
    Html(render_dashboard_compare(
        Some(output),
        Some(left_id),
        Some(right_id),
    ))
}

pub async fn dashboard_wallet(
    State(state): State<DashboardState>,
    Query(query): Query<DashboardWalletQuery>,
) -> Html<String> {
    let Some(address) = query.address.clone() else {
        return Html(render_dashboard_wallet(None, query.address.as_deref()));
    };
    let inspect = match state
        .store
        .inspect_address(
            &address,
            query.top_mints_limit.unwrap_or(10).clamp(1, 100),
            query.roundtrip_limit.unwrap_or(10).clamp(1, 100),
        )
        .await
    {
        Ok(report) => report,
        Err(error) => {
            return Html(render_dashboard_error(
                "Failed To Load Wallet",
                &error.to_string(),
            ));
        }
    };
    let events = match state.store.load_replay_events().await {
        Ok(events) => events,
        Err(error) => {
            return Html(render_dashboard_error(
                "Failed To Load Replay Events",
                &error.to_string(),
            ));
        }
    };
    let wallet = extract_wallet_behavior(&address, &events);
    let mut candidates = Vec::new();
    for family in ["early_flow", "momentum", "breakout", "liquidity_follow"] {
        let strategy = match default_strategy_config_for_family(family) {
            Ok(strategy) => strategy,
            Err(error) => {
                return Html(render_dashboard_error("Invalid Family", &error.to_string()));
            }
        };
        let execution = match run_strategy(events.clone(), &strategy) {
            Ok(execution) => execution,
            Err(error) => {
                return Html(render_dashboard_error(
                    "Failed To Evaluate Strategy",
                    &error.to_string(),
                ));
            }
        };
        candidates.push(score_strategy_execution(&wallet, &strategy, &execution));
    }
    candidates.sort_by(|left, right| right.score.overall.total_cmp(&left.score.overall));
    let mut candidates = candidates;
    let best_family = candidates.remove(0);
    let runner_up = candidates.remove(0);
    let dossier = wallet_dossier_output(
        inspect,
        &wallet,
        &best_family,
        &runner_up,
        None,
        query.sample_limit.unwrap_or(5).clamp(1, 20),
    );
    Html(render_dashboard_wallet(Some(dossier), Some(&address)))
}

pub async fn dashboard_batch_detail(
    State(state): State<DashboardState>,
    Path(batch_id): Path<String>,
) -> Html<String> {
    match state.store.inspect_sweep_batch(&batch_id).await {
        Ok(report) => Html(render_dashboard_batch_detail(report)),
        Err(error) => Html(render_dashboard_error(
            "Failed To Load Sweep Batch",
            &error.to_string(),
        )),
    }
}
