use anyhow::Result;
use pump_agent_core::PgEventStore;

use crate::{
    args::{DbArgs, TailArgs},
    config::required_config,
};

use crate::commands::helpers::print_event;

pub async fn stats(args: DbArgs) -> Result<()> {
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    let stats = store.fetch_event_stats().await?;

    println!("total events      : {}", stats.total_events);
    println!("total trades      : {}", stats.total_trades);
    println!("mint events       : {}", stats.total_mint_events);
    println!("total completions : {}", stats.total_completions);
    println!("distinct mints    : {}", stats.distinct_mints_seen);
    println!("stored mints      : {}", stats.stored_mints);
    println!("real created      : {}", stats.real_created_mints);
    println!("trade-only mints  : {}", stats.inferred_trade_only_mints);
    println!(
        "latest slot       : {}",
        stats
            .latest_slot
            .map(|slot| slot.to_string())
            .unwrap_or_else(|| "n/a".to_string())
    );
    Ok(())
}

pub async fn tail(args: TailArgs) -> Result<()> {
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    let events = store.tail_events(args.limit).await?;

    for event in events {
        print_event(&event);
    }
    Ok(())
}
