use anyhow::Result;
use pump_agent_core::PgEventStore;

use crate::{
    args::MintInspectArgs,
    config::{blank_to_na, required_config},
};

pub async fn mint_inspect(args: MintInspectArgs) -> Result<()> {
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    let report = store.inspect_mint(&args.mint, args.limit).await?;

    let Some(overview) = report.overview else {
        println!("mint not found: {}", args.mint);
        return Ok(());
    };

    println!("mint            : {}", overview.mint);
    println!("symbol          : {}", blank_to_na(&overview.symbol));
    println!("name            : {}", blank_to_na(&overview.name));
    println!("creator         : {}", blank_to_na(&overview.creator));
    println!("bonding curve   : {}", blank_to_na(&overview.bonding_curve));
    println!("token program   : {}", blank_to_na(&overview.token_program));
    println!("uri             : {}", blank_to_na(&overview.uri));
    println!("is inferred     : {}", overview.is_inferred);
    println!("created slot    : {}", overview.created_slot);
    println!(
        "created at      : {}",
        overview
            .created_at
            .as_deref()
            .map(blank_to_na)
            .unwrap_or("n/a")
    );
    println!("trade count     : {}", overview.trade_count);
    println!("buy count       : {}", overview.buy_count);
    println!("sell count      : {}", overview.sell_count);
    println!("gross buy       : {} lamports", overview.gross_buy_lamports);
    println!(
        "gross sell      : {} lamports",
        overview.gross_sell_lamports
    );
    println!("net flow        : {} lamports", overview.net_flow_lamports);
    println!(
        "last trade slot : {}",
        overview
            .last_trade_slot
            .map(|slot| slot.to_string())
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!(
        "last trade at   : {}",
        overview
            .last_trade_at
            .as_deref()
            .map(blank_to_na)
            .unwrap_or("n/a")
    );

    println!();
    println!("recent trades:");
    for trade in report.recent_trades {
        println!(
            "seq={} slot={} side={} sol={} token={} user={}",
            trade.seq,
            trade.slot,
            trade.side,
            trade.sol_amount,
            trade.token_amount,
            trade.user_address
        );
    }

    Ok(())
}
