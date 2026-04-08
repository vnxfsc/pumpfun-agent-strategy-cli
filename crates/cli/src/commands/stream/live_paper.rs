use std::{
    collections::VecDeque,
    fs::{File, OpenOptions, create_dir_all},
    io::{BufWriter, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use futures::{SinkExt, StreamExt};
use pump_agent_core::{
    Engine, ExecutionReport, PgEventStore, StrategyRunPersistOptions, assign_sequence_numbers,
    connect_pump_subscription, decode_transaction_update, pump_ping_request,
};
use serde::Serialize;
use tokio::time::{Duration, MissedTickBehavior, sleep};

use crate::{
    args::LivePaperArgs,
    runtime::{
        build_position_snapshot_input, generate_run_group_id, persist_run, push_position_snapshot,
        resolve_strategy_args,
    },
};

use super::config::resolve_live_paper_runtime_config;
use crate::commands::helpers::{
    SCHEMA_SQL, format_execution_report, print_report, push_recent_activity, render_live_dashboard,
};

pub async fn live_paper(args: LivePaperArgs) -> Result<()> {
    let runtime = resolve_live_paper_runtime_config(&args)?;

    let store = match runtime.database_url.as_ref() {
        Some(url) if args.persist_events || args.save_run => {
            let store = PgEventStore::connect(url, runtime.max_db_connections).await?;
            if runtime.apply_schema {
                store.apply_schema(SCHEMA_SQL).await?;
            }
            Some(store)
        }
        _ => None,
    };

    let resolved_strategy = resolve_strategy_args(&args.strategy)?;
    let (strategy, broker) = crate::runtime::build_strategy_and_broker(&resolved_strategy)?;
    let mut engine = Engine::new(strategy, broker);
    let session_id = if args.save_run || args.execution_jsonl.is_some() {
        Some(generate_run_group_id("live"))
    } else {
        None
    };
    let mut execution_jsonl = ExecutionJsonlWriter::open(
        args.execution_jsonl.as_deref(),
        &resolved_strategy.strategy,
        session_id.clone(),
    )?;
    let mut persisted_snapshots = Vec::new();

    let mut resume_from_db = args.resume_from_db;
    let mut last_seen_slot = match store.as_ref() {
        Some(store) if args.resume_from_db => store.latest_slot().await?,
        _ => None,
    };
    let mut next_seq = match store.as_ref() {
        Some(store) if args.persist_events => store.next_sequence().await?,
        _ => 1,
    };
    let mut raw_tx_count = 0_u64;
    let mut event_count = 0_u64;
    let mut ping_id = 1_i32;
    let mut recent_activity = VecDeque::with_capacity(args.dashboard_activity_limit.max(1));

    loop {
        let resume_from_slot = if resume_from_db {
            last_seen_slot.map(|slot| slot.saturating_sub(1))
        } else {
            None
        };
        println!(
            "starting live-paper strategy={} endpoint={} from_slot={:?}",
            resolved_strategy.strategy, runtime.yellowstone.endpoint, resume_from_slot
        );
        if let Some(path) = args.execution_jsonl.as_ref() {
            println!("execution jsonl={}", path.display());
        }

        let (mut sink, mut stream) =
            match connect_pump_subscription(runtime.yellowstone.clone(), resume_from_slot).await {
                Ok(subscription) => subscription,
                Err(error) => {
                    eprintln!("connect failed: {error:#}");
                    sleep(Duration::from_secs(runtime.reconnect_delay_secs)).await;
                    continue;
                }
            };

        let mut heartbeat = tokio::time::interval(Duration::from_secs(runtime.heartbeat_secs));
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                maybe_update = stream.next() => {
                    match maybe_update {
                        Some(Ok(update)) => {
                            if let Some(mut decoded) = decode_transaction_update(&update)? {
                                assign_sequence_numbers(&mut decoded.events, &mut next_seq);
                                raw_tx_count += 1;
                                event_count += decoded.events.len() as u64;
                                last_seen_slot = Some(last_seen_slot.map_or(decoded.raw.slot, |slot| slot.max(decoded.raw.slot)));

                                if args.persist_events {
                                    let store = store
                                        .as_ref()
                                        .expect("store must exist when persist_events is enabled");
                                    store.append_decoded_transaction(&decoded).await?;
                                }

                                for event in decoded.events {
                                    let event_seq = event.seq;
                                    let event_slot = event.slot;
                                    let event_ts = event.timestamp();
                                    let execution_reports = engine.step(event);
                                    if !execution_reports.is_empty() {
                                        for report in &execution_reports {
                                            push_recent_activity(
                                                &mut recent_activity,
                                                format_execution_report(report),
                                                args.dashboard_activity_limit,
                                            );
                                            execution_jsonl.append(
                                                report,
                                                event_seq,
                                                event_slot,
                                                event_ts,
                                            )?;
                                        }
                                        render_live_dashboard(
                                            &engine.report_snapshot(),
                                            &engine.broker_snapshot(),
                                            engine.market_state(),
                                            &recent_activity,
                                            args.dashboard_position_limit,
                                        );
                                        if args.save_run {
                                            push_position_snapshot(
                                                &mut persisted_snapshots,
                                                build_position_snapshot_input(
                                                    "execution",
                                                    &engine.report_snapshot(),
                                                    &engine.broker_snapshot(),
                                                    engine.market_state(),
                                                    Some(event_seq),
                                                    Some(event_slot),
                                                    event_ts,
                                                ),
                                            );
                                        }
                                    } else if args.summary_every_events > 0
                                        && event_count.is_multiple_of(args.summary_every_events)
                                    {
                                        render_live_dashboard(
                                            &engine.report_snapshot(),
                                            &engine.broker_snapshot(),
                                            engine.market_state(),
                                            &recent_activity,
                                            args.dashboard_position_limit,
                                        );
                                        if args.save_run {
                                            push_position_snapshot(
                                                &mut persisted_snapshots,
                                                build_position_snapshot_input(
                                                    "heartbeat",
                                                    &engine.report_snapshot(),
                                                    &engine.broker_snapshot(),
                                                    engine.market_state(),
                                                    Some(event_seq),
                                                    Some(event_slot),
                                                    event_ts,
                                                ),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Some(Err(error)) => {
                            if should_disable_resume(error.message()) {
                                eprintln!(
                                    "provider rejected replay position, disabling from_slot resume"
                                );
                                resume_from_db = false;
                            }
                            eprintln!("stream error: {error}");
                            break;
                        }
                        None => {
                            eprintln!("stream closed by provider");
                            break;
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    if let Err(error) = sink.send(pump_ping_request(ping_id)).await {
                        eprintln!("failed to send heartbeat ping: {error}");
                        break;
                    }
                    ping_id = ping_id.wrapping_add(1);
                }
                _ = tokio::signal::ctrl_c() => {
                    let result = engine.finish();
                    println!("received ctrl-c, stopping live-paper");
                    println!("raw tx seen     : {raw_tx_count}");
                    println!("events seen     : {event_count}");
                    print_report(result.report.clone());

                    if args.save_run {
                        let Some(store) = store.as_ref() else {
                            bail!("--save-run requires DATABASE_URL or --database-url in live-paper mode");
                        };
                        store.apply_schema(SCHEMA_SQL).await?;
                        push_position_snapshot(
                            &mut persisted_snapshots,
                            build_position_snapshot_input(
                                "final",
                                &result.report,
                                &engine.broker_snapshot(),
                                engine.market_state(),
                                None,
                                None,
                                None,
                            ),
                        );

                        let run_id = persist_run(
                            store,
                            "live_paper",
                            "yellowstone",
                            &resolved_strategy,
                            &result,
                            StrategyRunPersistOptions {
                                run_mode: Some("live_paper".to_string()),
                                live_run_id: session_id.clone(),
                                position_snapshots: persisted_snapshots.clone(),
                                ..Default::default()
                            },
                        )
                        .await?;
                        println!("saved strategy run id: {}", run_id);
                    }

                    return Ok(());
                }
            }
        }

        println!(
            "reconnecting live-paper after {}s, last_seen_slot={:?}",
            runtime.reconnect_delay_secs, last_seen_slot
        );
        sleep(Duration::from_secs(runtime.reconnect_delay_secs)).await;
    }
}

struct ExecutionJsonlWriter {
    writer: Option<BufWriter<File>>,
    strategy_name: String,
    session_id: Option<String>,
}

impl ExecutionJsonlWriter {
    fn open(path: Option<&Path>, strategy_name: &str, session_id: Option<String>) -> Result<Self> {
        let writer = match path {
            Some(path) => {
                if let Some(parent) = path.parent()
                    && !parent.as_os_str().is_empty()
                {
                    create_dir_all(parent).with_context(|| {
                        format!(
                            "failed to create execution jsonl directory {}",
                            parent.display()
                        )
                    })?;
                }

                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .with_context(|| {
                        format!("failed to open execution jsonl file {}", path.display())
                    })?;
                Some(BufWriter::new(file))
            }
            None => None,
        };

        Ok(Self {
            writer,
            strategy_name: strategy_name.to_string(),
            session_id,
        })
    }

    fn append(
        &mut self,
        report: &ExecutionReport,
        event_seq: u64,
        event_slot: u64,
        event_timestamp: Option<i64>,
    ) -> Result<()> {
        let Some(writer) = self.writer.as_mut() else {
            return Ok(());
        };

        let record = ExecutionJsonlRecord {
            recorded_at_unix_ms: unix_timestamp_ms(),
            session_id: self.session_id.as_deref(),
            strategy: &self.strategy_name,
            event_seq,
            event_slot,
            event_timestamp,
            report: ExecutionJsonlReport::from(report),
        };

        serde_json::to_writer(&mut *writer, &record).context("failed to serialize execution")?;
        writer
            .write_all(b"\n")
            .context("failed to append execution jsonl newline")?;
        writer.flush().context("failed to flush execution jsonl")?;
        Ok(())
    }
}

#[derive(Serialize)]
struct ExecutionJsonlRecord<'a> {
    recorded_at_unix_ms: u64,
    session_id: Option<&'a str>,
    strategy: &'a str,
    event_seq: u64,
    event_slot: u64,
    event_timestamp: Option<i64>,
    #[serde(flatten)]
    report: ExecutionJsonlReport<'a>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ExecutionJsonlReport<'a> {
    Filled {
        #[serde(flatten)]
        fill: &'a pump_agent_core::model::FillReport,
    },
    Rejected {
        #[serde(flatten)]
        rejected: &'a pump_agent_core::model::RejectedOrder,
    },
}

impl<'a> From<&'a ExecutionReport> for ExecutionJsonlReport<'a> {
    fn from(value: &'a ExecutionReport) -> Self {
        match value {
            ExecutionReport::Filled(fill) => Self::Filled { fill },
            ExecutionReport::Rejected(rejected) => Self::Rejected { rejected },
        }
    }
}

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn should_disable_resume(message: &str) -> bool {
    message.contains("from_slot is not supported")
        || message.contains("failed to get replay position")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use pump_agent_core::{
        ExecutionReport, OrderSide, RejectionReason, model::FillReport, model::RejectedOrder,
    };

    use super::ExecutionJsonlWriter;

    fn temp_jsonl_path(label: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        path.push(format!("pump-agent-{label}-{unique}.jsonl"));
        path
    }

    #[test]
    fn appends_execution_records_as_jsonl() {
        let path = temp_jsonl_path("execution");
        let mut writer =
            ExecutionJsonlWriter::open(Some(&path), "early_flow", Some("live-123".to_string()))
                .expect("writer should open");

        writer
            .append(
                &ExecutionReport::Filled(FillReport {
                    order_id: 7,
                    mint: "Mint111".to_string(),
                    side: OrderSide::Buy,
                    lamports: 10,
                    token_amount: 20,
                    fee_lamports: 1,
                    execution_price_lamports_per_token: 0.5,
                    timestamp: Some(123),
                    reason: "entry".to_string(),
                }),
                11,
                22,
                Some(123),
            )
            .expect("fill should append");
        writer
            .append(
                &ExecutionReport::Rejected(RejectedOrder {
                    order_id: 8,
                    mint: "Mint222".to_string(),
                    reason: "duplicate".to_string(),
                    rejection: RejectionReason::DuplicatePosition,
                    timestamp: Some(124),
                }),
                12,
                23,
                Some(124),
            )
            .expect("rejection should append");

        let contents = fs::read_to_string(&path).expect("jsonl should exist");
        let lines = contents.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"strategy\":\"early_flow\""));
        assert!(lines[0].contains("\"session_id\":\"live-123\""));
        assert!(lines[0].contains("\"kind\":\"filled\""));
        assert!(lines[1].contains("\"kind\":\"rejected\""));

        let _ = fs::remove_file(path);
    }
}
