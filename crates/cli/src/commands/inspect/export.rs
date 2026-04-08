use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use pump_agent_core::{EventEnvelope, PgEventStore};
use serde::Serialize;

use crate::{args::AddressExportArgs, config::required_config};

use crate::commands::helpers::SCHEMA_SQL;

pub async fn address_export(args: AddressExportArgs) -> Result<()> {
    let database_url = required_config(args.database_url, "DATABASE_URL")?;
    let store = PgEventStore::connect(&database_url, args.max_db_connections).await?;
    store.apply_schema(SCHEMA_SQL).await?;
    let summary = export_address_events(&store, &args.address, &args.output).await?;

    print_export_summary(&summary);

    Ok(())
}

#[derive(Debug, Serialize)]
struct AddressExportIndex {
    address: String,
    mint_count: i64,
    wallet_trade_count: i64,
    event_count: usize,
    shard_count: usize,
    mints: Vec<AddressExportMintFile>,
}

#[derive(Debug, Serialize)]
struct AddressExportMintFile {
    mint: String,
    event_count: usize,
    file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddressExportSummary {
    pub address: String,
    pub output: PathBuf,
    pub address_dir: Option<PathBuf>,
    pub index_path: Option<PathBuf>,
    pub mint_count: i64,
    pub wallet_trade_count: i64,
    pub event_count: usize,
    pub shard_count: usize,
    pub sharded: bool,
}

pub async fn export_address_events(
    store: &PgEventStore,
    address: &str,
    output: &Path,
) -> Result<AddressExportSummary> {
    let export = store.load_events_for_address_mints(address).await?;

    if is_jsonl_path(output) {
        write_jsonl_file(output, &export.events)?;
        return Ok(AddressExportSummary {
            address: export.address,
            output: output.to_path_buf(),
            address_dir: None,
            index_path: None,
            mint_count: export.mint_count,
            wallet_trade_count: export.wallet_trade_count,
            event_count: export.event_count,
            shard_count: 1,
            sharded: false,
        });
    }

    let address_dir = output.join(&export.address);
    fs::create_dir_all(&address_dir)
        .with_context(|| format!("failed to create {}", address_dir.display()))?;

    let mut events_by_mint = BTreeMap::<String, Vec<&EventEnvelope>>::new();
    for event in &export.events {
        if let Some(mint) = event.mint() {
            events_by_mint
                .entry(mint.to_string())
                .or_default()
                .push(event);
        }
    }

    let mut mint_files = Vec::with_capacity(events_by_mint.len());
    for (mint, events) in events_by_mint {
        let file_path = address_dir.join(format!("{mint}.jsonl"));
        write_jsonl_file(&file_path, &events)?;
        mint_files.push(AddressExportMintFile {
            mint,
            event_count: events.len(),
            file: file_path
                .file_name()
                .expect("mint shard file name should exist")
                .to_string_lossy()
                .into_owned(),
        });
    }

    let index = AddressExportIndex {
        address: export.address.clone(),
        mint_count: export.mint_count,
        wallet_trade_count: export.wallet_trade_count,
        event_count: export.event_count,
        shard_count: mint_files.len(),
        mints: mint_files,
    };
    let index_path = address_dir.join("index.json");
    let index_file = File::create(&index_path)
        .with_context(|| format!("failed to create {}", index_path.display()))?;
    serde_json::to_writer_pretty(BufWriter::new(index_file), &index)
        .context("failed to write address export index")?;

    Ok(AddressExportSummary {
        address: export.address,
        output: output.to_path_buf(),
        address_dir: Some(address_dir),
        index_path: Some(index_path),
        mint_count: export.mint_count,
        wallet_trade_count: export.wallet_trade_count,
        event_count: export.event_count,
        shard_count: index.shard_count,
        sharded: true,
    })
}

pub fn print_export_summary(summary: &AddressExportSummary) {
    println!("address          : {}", summary.address);
    if summary.sharded {
        println!(
            "output dir       : {}",
            summary
                .address_dir
                .as_deref()
                .expect("sharded export should include address dir")
                .display()
        );
    } else {
        println!("output           : {}", summary.output.display());
    }
    println!("mint count       : {}", summary.mint_count);
    println!("wallet trades    : {}", summary.wallet_trade_count);
    println!("exported events  : {}", summary.event_count);
    if summary.sharded {
        println!("shards           : {}", summary.shard_count);
        println!(
            "index            : {}",
            summary
                .index_path
                .as_deref()
                .expect("sharded export should include index path")
                .display()
        );
        println!("format           : <output>/<address>/<mint>.jsonl + index.json");
    } else {
        println!("format           : jsonl EventEnvelope");
    }
}

fn write_jsonl_file(path: impl AsRef<Path>, events: &[impl Serialize]) -> Result<()> {
    let path = path.as_ref();
    let file =
        File::create(path).with_context(|| format!("failed to create {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for event in events {
        serde_json::to_writer(&mut writer, event).context("failed to serialize event to json")?;
        writer.write_all(b"\n").context("failed to write newline")?;
    }
    writer.flush().context("failed to flush export file")?;
    Ok(())
}

fn is_jsonl_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("jsonl"))
        .unwrap_or(false)
}
