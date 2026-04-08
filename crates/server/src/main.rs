mod http;

use anyhow::Result;
use clap::Parser;
use dotenvy::dotenv;

#[derive(Debug, Parser)]
#[command(name = "pump-agent-server")]
#[command(about = "HTTP API for Pump agent research workflows")]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 3001)]
    port: u16,

    #[arg(long)]
    database_url: Option<String>,

    #[arg(long, default_value_t = 5)]
    max_db_connections: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenv();
    let args = Args::parse();
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let state = http::ApiState::new(database_url, args.max_db_connections);
    let app = http::router(state);
    let listener = tokio::net::TcpListener::bind((args.host.as_str(), args.port)).await?;
    println!("api listening on http://{}:{}", args.host, args.port);
    axum::serve(listener, app).await?;
    Ok(())
}

fn required_config(cli_value: Option<String>, env_key: &str) -> Result<String> {
    cli_value
        .or_else(|| std::env::var(env_key).ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing configuration: --database-url or {}", env_key))
}
