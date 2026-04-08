use axum::{Router, routing::get};
use pump_agent_core::PgEventStore;

use super::handlers::{
    dashboard_batch_detail, dashboard_healthz, dashboard_home, dashboard_run_detail,
};

#[derive(Clone)]
pub struct DashboardState {
    pub store: PgEventStore,
}

pub fn router(store: PgEventStore) -> Router {
    Router::new()
        .route("/", get(dashboard_home))
        .route("/runs/{id}", get(dashboard_run_detail))
        .route("/batches/{batch_id}", get(dashboard_batch_detail))
        .route("/healthz", get(dashboard_healthz))
        .with_state(DashboardState { store })
}
