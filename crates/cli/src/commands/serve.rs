use anyhow::Result;
use pump_agent_core::PgEventStore;

use crate::{args::ServeDashboardArgs, config::required_config, dashboard};

use super::helpers::SCHEMA_SQL;

pub async fn serve_dashboard(args: ServeDashboardArgs) -> Result<()> {
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;

    let app = dashboard::router(store);
    let listener = tokio::net::TcpListener::bind((args.host.as_str(), args.port)).await?;
    println!("dashboard listening on http://{}:{}", args.host, args.port);
    axum::serve(listener, app).await?;
    Ok(())
}
