use axum::{
    extract::{Path, Query, State},
    response::Html,
};

use super::{
    render::{
        render_dashboard_batch_detail, render_dashboard_error, render_dashboard_home,
        render_dashboard_run_detail,
    },
    state::DashboardState,
};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DashboardListQuery {
    pub limit: Option<i64>,
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
