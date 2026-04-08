use pump_agent_app::usecases::{
    AddressInspectRequest, DatabaseRequest, address_inspect as load_address_inspect,
};
use serde::Serialize;
use std::time::Instant;

use crate::{
    args::{AddressInspectArgs, AddressRoundtripsArgs, AddressTimelineArgs, OutputFormat},
    config::{blank_to_na, lamports_i128_to_sol, lamports_str_to_sol, required_config},
    output::{CommandError, CommandResult, emit_json_success},
};

pub async fn address_inspect(args: AddressInspectArgs, format: OutputFormat) -> CommandResult<()> {
    let started = Instant::now();
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let report = load_address_inspect(AddressInspectRequest {
        database: DatabaseRequest {
            database_url,
            max_db_connections: args.max_db_connections,
            apply_schema: true,
        },
        address: args.address,
        top_mints_limit: args.top_mints_limit,
        roundtrip_limit: args.roundtrip_limit,
    })
    .await?;

    if format.is_json() {
        return emit_json_success("address_inspect", &AddressInspectOutput { report }, started);
    }

    let overview = report.overview;
    println!("address              : {}", overview.address);
    println!("total trades         : {}", overview.total_trades);
    println!("buy count            : {}", overview.buy_count);
    println!("sell count           : {}", overview.sell_count);
    println!("distinct mints       : {}", overview.distinct_mints);
    println!(
        "first trade seq      : {}",
        overview
            .first_trade_seq
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!(
        "first trade at       : {}",
        overview
            .first_trade_at
            .as_deref()
            .map(blank_to_na)
            .unwrap_or("n/a")
    );
    println!(
        "last trade seq       : {}",
        overview
            .last_trade_seq
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!(
        "last trade at        : {}",
        overview
            .last_trade_at
            .as_deref()
            .map(blank_to_na)
            .unwrap_or("n/a")
    );
    println!(
        "gross buy            : {:.6} SOL",
        lamports_str_to_sol(&overview.gross_buy_lamports)?
    );
    println!(
        "gross sell           : {:.6} SOL",
        lamports_str_to_sol(&overview.gross_sell_lamports)?
    );
    println!(
        "net cash flow        : {:.6} SOL",
        lamports_i128_to_sol(
            overview
                .net_cash_flow_lamports
                .parse::<i128>()
                .map_err(|error| {
                    CommandError::internal(format!(
                        "failed to parse net_cash_flow_lamports '{}': {}",
                        overview.net_cash_flow_lamports, error
                    ))
                })?,
        )
    );
    println!("roundtrips           : {}", overview.roundtrip_count);
    println!("closed roundtrips    : {}", overview.closed_roundtrip_count);
    println!("open roundtrips      : {}", overview.open_roundtrip_count);
    println!("orphan sells         : {}", overview.orphan_sell_count);
    println!(
        "realized pnl         : {:.6} SOL",
        lamports_i128_to_sol(
            overview
                .realized_pnl_lamports
                .parse::<i128>()
                .map_err(|error| {
                    CommandError::internal(format!(
                        "failed to parse realized_pnl_lamports '{}': {}",
                        overview.realized_pnl_lamports, error
                    ))
                })?,
        )
    );
    println!(
        "win rate closed      : {}",
        overview
            .win_rate_closed
            .map(|value| format!("{:.2}%", value * 100.0))
            .unwrap_or_else(|| "n/a".to_string())
    );
    println!(
        "avg hold closed      : {}",
        overview
            .avg_hold_secs_closed
            .map(|value| format!("{value}s"))
            .unwrap_or_else(|| "n/a".to_string())
    );

    println!();
    println!("top mints:");
    for mint in report.top_mints {
        println!(
            "mint={} trades={} buys={} sells={} gross_buy={:.6} SOL gross_sell={:.6} SOL net_cash_flow={:.6} SOL first_seq={} last_seq={} last_trade_at={}",
            mint.mint,
            mint.trade_count,
            mint.buy_count,
            mint.sell_count,
            lamports_str_to_sol(&mint.gross_buy_lamports)?,
            lamports_str_to_sol(&mint.gross_sell_lamports)?,
            lamports_i128_to_sol(
                mint.net_cash_flow_lamports
                    .parse::<i128>()
                    .map_err(|error| {
                        CommandError::internal(format!(
                            "failed to parse mint net_cash_flow_lamports '{}': {}",
                            mint.net_cash_flow_lamports, error
                        ))
                    },)?
            ),
            mint.first_seq,
            mint.last_seq,
            mint.last_trade_at
                .as_deref()
                .map(blank_to_na)
                .unwrap_or("n/a")
        );
    }

    println!();
    println!("recent roundtrips:");
    for roundtrip in report.recent_roundtrips {
        print_roundtrip(&roundtrip)?;
    }

    Ok(())
}

pub async fn address_timeline(args: AddressTimelineArgs) -> anyhow::Result<()> {
    use pump_agent_core::PgEventStore;

    use crate::commands::helpers::SCHEMA_SQL;

    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let rows = store
        .address_timeline(&args.address, args.limit, args.ascending)
        .await?;

    if rows.is_empty() {
        println!("no trades found for address: {}", args.address);
        return Ok(());
    }

    for row in rows {
        println!(
            "seq={} slot={} ts={} mint={} side={} sol={:.6} SOL token={} fee={} creator_fee={} cashback={} ix={} sig={}",
            row.seq,
            row.slot,
            row.timestamp.as_deref().map(blank_to_na).unwrap_or("n/a"),
            row.mint,
            row.side,
            lamports_str_to_sol(&row.sol_amount)?,
            row.token_amount,
            row.fee_lamports,
            row.creator_fee_lamports,
            row.cashback_lamports,
            row.ix_name,
            row.tx_signature
        );
    }

    Ok(())
}

pub async fn address_roundtrips(args: AddressRoundtripsArgs) -> anyhow::Result<()> {
    use pump_agent_core::PgEventStore;

    use crate::commands::helpers::SCHEMA_SQL;

    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let report = store.address_roundtrips(&args.address, args.limit).await?;

    println!("address          : {}", report.address);
    println!("roundtrips       : {}", report.total_roundtrips);
    println!("closed           : {}", report.closed_roundtrips);
    println!("open             : {}", report.open_roundtrips);
    println!("orphan sells     : {}", report.orphan_sell_count);
    println!();

    for roundtrip in report.roundtrips {
        print_roundtrip(&roundtrip)?;
    }

    Ok(())
}

fn print_roundtrip(roundtrip: &pump_agent_core::AddressRoundtrip) -> anyhow::Result<()> {
    println!(
        "mint={} status={} opened_seq={} opened_at={} closed_seq={} closed_at={} hold={} entries={} exits={} gross_buy={:.6} SOL gross_sell={:.6} SOL net_entry={:.6} SOL net_exit={:.6} SOL pnl={} roi_bps={} bought_token={} sold_token={}",
        roundtrip.mint,
        roundtrip.status,
        roundtrip.opened_seq,
        roundtrip
            .opened_at
            .as_deref()
            .map(blank_to_na)
            .unwrap_or("n/a"),
        roundtrip
            .closed_seq
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        roundtrip
            .closed_at
            .as_deref()
            .map(blank_to_na)
            .unwrap_or("n/a"),
        roundtrip
            .hold_secs
            .map(|value| format!("{value}s"))
            .unwrap_or_else(|| "n/a".to_string()),
        roundtrip.entry_count,
        roundtrip.exit_count,
        lamports_str_to_sol(&roundtrip.gross_buy_lamports)?,
        lamports_str_to_sol(&roundtrip.gross_sell_lamports)?,
        lamports_i128_to_sol(roundtrip.net_entry_lamports.parse::<i128>()?),
        lamports_i128_to_sol(roundtrip.net_exit_lamports.parse::<i128>()?),
        roundtrip
            .realized_pnl_lamports
            .as_deref()
            .map(|value| {
                format!(
                    "{:.6} SOL",
                    lamports_i128_to_sol(value.parse::<i128>().unwrap_or_default())
                )
            })
            .unwrap_or_else(|| "n/a".to_string()),
        roundtrip
            .roi_bps
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        roundtrip.bought_token_amount,
        roundtrip.sold_token_amount
    );

    Ok(())
}

#[derive(Debug, Serialize)]
struct AddressInspectOutput {
    report: pump_agent_core::AddressInspectReport,
}
