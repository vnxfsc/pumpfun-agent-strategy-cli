use std::{
    fs::{File, OpenOptions, create_dir_all},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use futures::{SinkExt, StreamExt};
use pump_agent_core::{
    BacktestReport, CommitmentLevel, Engine, ExecutionReport, PgEventStore,
    StrategyRunPersistOptions, YellowstoneConfig, assign_sequence_numbers,
    connect_pump_subscription, decode_transaction_update, pump_ping_request,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    sync::watch,
    time::{Duration, MissedTickBehavior, sleep},
};

use crate::strategy::{
    StrategyConfig, build_position_snapshot_input, build_strategy_and_broker,
    generate_run_group_id, persist_run, resolve_strategy_config,
};
use crate::usecases::{ExperimentContext, generate_record_id};

const SCHEMA_SQL: &str = include_str!("../../../../schema/postgres.sql");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamTaskConfig {
    pub endpoint: String,
    pub database_url: Option<String>,
    pub x_token: Option<String>,
    pub commitment: Option<String>,
    pub apply_schema: bool,
    pub x_request_snapshot: bool,
    pub max_decoding_message_size: Option<usize>,
    pub connect_timeout_secs: Option<u64>,
    pub request_timeout_secs: Option<u64>,
    pub max_db_connections: u32,
    pub heartbeat_secs: Option<u64>,
    pub reconnect_delay_secs: Option<u64>,
    pub http2_keep_alive_interval_secs: Option<u64>,
    pub keep_alive_timeout_secs: Option<u64>,
    pub tcp_keepalive_secs: Option<u64>,
    pub resume_from_db: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestTaskRequest {
    pub stream: StreamTaskConfig,
    pub experiment: Option<ExperimentContext>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IngestTaskResult {
    pub raw_tx_count: u64,
    pub event_count: u64,
    pub last_seen_slot: Option<u64>,
    pub recorded_evaluation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivePaperTaskRequest {
    pub stream: StreamTaskConfig,
    pub strategy: StrategyConfig,
    pub save_run: bool,
    pub execution_jsonl: Option<PathBuf>,
    pub persist_events: bool,
    pub summary_every_events: u64,
    pub experiment: Option<ExperimentContext>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LivePaperTaskResult {
    pub strategy: String,
    pub raw_tx_count: u64,
    pub event_count: u64,
    pub last_seen_slot: Option<u64>,
    pub session_id: Option<String>,
    pub saved_run_id: Option<i64>,
    pub recorded_evaluation_id: Option<String>,
    pub report: BacktestReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskExecution<T> {
    pub status: String,
    pub result: T,
}

pub async fn run_ingest_task(
    task_id: &str,
    request: IngestTaskRequest,
    mut cancel_rx: watch::Receiver<bool>,
) -> Result<TaskExecution<IngestTaskResult>> {
    let runtime = resolve_stream_runtime(&request.stream, true)?;
    let database_url = runtime
        .database_url
        .as_ref()
        .expect("ingest requires database_url");

    let store = PgEventStore::connect(database_url, runtime.max_db_connections).await?;
    if runtime.apply_schema || request.experiment.is_some() {
        store.apply_schema(SCHEMA_SQL).await?;
    }

    let mut last_seen_slot = store.latest_slot().await?;
    let mut next_seq = store.next_sequence().await?;
    let mut raw_tx_count = 0_u64;
    let mut event_count = 0_u64;
    let mut ping_id = 1_i32;
    let mut resume_from_db = request.stream.resume_from_db;

    loop {
        if *cancel_rx.borrow() {
            return Ok(cancelled(TaskExecution {
                status: "cancelled".to_string(),
                result: IngestTaskResult {
                    raw_tx_count,
                    event_count,
                    last_seen_slot,
                    recorded_evaluation_id: None,
                },
            }));
        }

        let resume_from_slot = if resume_from_db {
            last_seen_slot.map(|slot| slot.saturating_sub(1))
        } else {
            None
        };

        let (mut sink, mut stream) =
            match connect_pump_subscription(runtime.yellowstone.clone(), resume_from_slot).await {
                Ok(subscription) => subscription,
                Err(error) => {
                    wait_or_cancel(
                        Duration::from_secs(runtime.reconnect_delay_secs),
                        &mut cancel_rx,
                    )
                    .await?;
                    if *cancel_rx.borrow() {
                        return Ok(cancelled(TaskExecution {
                            status: "cancelled".to_string(),
                            result: IngestTaskResult {
                                raw_tx_count,
                                event_count,
                                last_seen_slot,
                                recorded_evaluation_id: None,
                            },
                        }));
                    }
                    if should_disable_resume(&error.to_string()) {
                        resume_from_db = false;
                    }
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
                                event_count += decoded.events.len() as u64;
                                raw_tx_count += 1;
                                last_seen_slot = Some(last_seen_slot.map_or(decoded.raw.slot, |slot| slot.max(decoded.raw.slot)));
                                store.append_decoded_transaction(&decoded).await?;
                            }
                        }
                        Some(Err(error)) => {
                            if should_disable_resume(error.message()) {
                                resume_from_db = false;
                            }
                            break;
                        }
                        None => break,
                    }
                }
                _ = heartbeat.tick() => {
                    if sink.send(pump_ping_request(ping_id)).await.is_err() {
                        break;
                    }
                    ping_id = ping_id.wrapping_add(1);
                }
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() {
                        return Ok(cancelled(TaskExecution {
                            status: "cancelled".to_string(),
                            result: IngestTaskResult {
                                raw_tx_count,
                                event_count,
                                last_seen_slot,
                                recorded_evaluation_id: None,
                            },
                        }));
                    }
                }
            }
        }

        if *cancel_rx.borrow() {
            break;
        }

        wait_or_cancel(
            Duration::from_secs(runtime.reconnect_delay_secs),
            &mut cancel_rx,
        )
        .await?;
    }

    let recorded_evaluation_id = if let Some(context) = request.experiment {
        let experiment = store
            .get_experiment(&context.experiment_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("experiment not found: {}", context.experiment_id))?;
        let evaluation_id = generate_record_id("eval");
        store
            .create_evaluation(
                &evaluation_id,
                &context.experiment_id,
                context.hypothesis_id.as_deref(),
                None,
                Some(task_id),
                &experiment.target_wallet,
                None,
                None,
                "ingest_task",
                &request.stream.endpoint,
                None,
                json!({
                    "raw_tx_count": raw_tx_count,
                    "event_count": event_count,
                    "last_seen_slot": last_seen_slot,
                }),
                json!({
                    "task_id": task_id,
                    "task_kind": "ingest",
                    "endpoint": request.stream.endpoint,
                }),
                &context.failure_tags,
                context.artifact_paths,
                context.notes,
                context.conclusion.as_deref(),
            )
            .await?;
        Some(evaluation_id)
    } else {
        None
    };

    Ok(cancelled(TaskExecution {
        status: "cancelled".to_string(),
        result: IngestTaskResult {
            raw_tx_count,
            event_count,
            last_seen_slot,
            recorded_evaluation_id,
        },
    }))
}

pub async fn run_live_paper_task(
    task_id: &str,
    request: LivePaperTaskRequest,
    mut cancel_rx: watch::Receiver<bool>,
) -> Result<TaskExecution<LivePaperTaskResult>> {
    let runtime = resolve_stream_runtime(
        &request.stream,
        request.persist_events
            || request.save_run
            || request.stream.resume_from_db
            || request.experiment.is_some(),
    )?;

    let store = match runtime.database_url.as_ref() {
        Some(url) if request.persist_events || request.save_run || request.experiment.is_some() => {
            let store = PgEventStore::connect(url, runtime.max_db_connections).await?;
            if runtime.apply_schema || request.experiment.is_some() {
                store.apply_schema(SCHEMA_SQL).await?;
            }
            Some(store)
        }
        Some(url) if request.stream.resume_from_db => {
            Some(PgEventStore::connect(url, runtime.max_db_connections).await?)
        }
        _ => None,
    };

    let resolved_strategy = resolve_strategy_config(&request.strategy)?;
    let (strategy, broker) = build_strategy_and_broker(&resolved_strategy)?;
    let mut engine = Engine::new(strategy, broker);
    let session_id = if request.save_run || request.execution_jsonl.is_some() {
        Some(generate_run_group_id("live"))
    } else {
        None
    };
    let mut execution_jsonl = ExecutionJsonlWriter::open(
        request.execution_jsonl.as_deref(),
        &resolved_strategy.strategy,
        session_id.clone(),
    )?;
    let mut persisted_snapshots = Vec::new();

    let mut resume_from_db = request.stream.resume_from_db;
    let mut last_seen_slot = match store.as_ref() {
        Some(store) if request.stream.resume_from_db => store.latest_slot().await?,
        _ => None,
    };
    let mut next_seq = match store.as_ref() {
        Some(store) if request.persist_events => store.next_sequence().await?,
        _ => 1,
    };
    let mut raw_tx_count = 0_u64;
    let mut event_count = 0_u64;
    let mut ping_id = 1_i32;

    loop {
        if *cancel_rx.borrow() {
            return finish_live_paper(
                &request,
                &resolved_strategy,
                &store,
                &mut engine,
                &mut persisted_snapshots,
                session_id.clone(),
                raw_tx_count,
                event_count,
                last_seen_slot,
                task_id,
            )
            .await;
        }

        let resume_from_slot = if resume_from_db {
            last_seen_slot.map(|slot| slot.saturating_sub(1))
        } else {
            None
        };

        let (mut sink, mut stream) =
            match connect_pump_subscription(runtime.yellowstone.clone(), resume_from_slot).await {
                Ok(subscription) => subscription,
                Err(error) => {
                    wait_or_cancel(
                        Duration::from_secs(runtime.reconnect_delay_secs),
                        &mut cancel_rx,
                    )
                    .await?;
                    if *cancel_rx.borrow() {
                        return finish_live_paper(
                            &request,
                            &resolved_strategy,
                            &store,
                            &mut engine,
                            &mut persisted_snapshots,
                            session_id.clone(),
                            raw_tx_count,
                            event_count,
                            last_seen_slot,
                            task_id,
                        )
                        .await;
                    }
                    if should_disable_resume(&error.to_string()) {
                        resume_from_db = false;
                    }
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

                                if request.persist_events {
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
                                            execution_jsonl.append(report, event_seq, event_slot, event_ts)?;
                                        }
                                        if request.save_run {
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
                                    } else if request.summary_every_events > 0
                                        && event_count.is_multiple_of(request.summary_every_events)
                                        && request.save_run
                                    {
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
                        Some(Err(error)) => {
                            if should_disable_resume(error.message()) {
                                resume_from_db = false;
                            }
                            break;
                        }
                        None => break,
                    }
                }
                _ = heartbeat.tick() => {
                    if sink.send(pump_ping_request(ping_id)).await.is_err() {
                        break;
                    }
                    ping_id = ping_id.wrapping_add(1);
                }
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() {
                        return finish_live_paper(
                            &request,
                            &resolved_strategy,
                            &store,
                            &mut engine,
                            &mut persisted_snapshots,
                            session_id.clone(),
                            raw_tx_count,
                            event_count,
                            last_seen_slot,
                            task_id,
                        )
                        .await;
                    }
                }
            }
        }

        wait_or_cancel(
            Duration::from_secs(runtime.reconnect_delay_secs),
            &mut cancel_rx,
        )
        .await?;
    }
}

#[derive(Debug, Clone)]
struct StreamRuntimeResolved {
    yellowstone: YellowstoneConfig,
    database_url: Option<String>,
    heartbeat_secs: u64,
    reconnect_delay_secs: u64,
    max_db_connections: u32,
    apply_schema: bool,
}

fn resolve_stream_runtime(
    request: &StreamTaskConfig,
    require_database: bool,
) -> Result<StreamRuntimeResolved> {
    if require_database && request.database_url.is_none() {
        bail!("database_url is required for this task");
    }

    Ok(StreamRuntimeResolved {
        yellowstone: YellowstoneConfig {
            endpoint: request.endpoint.clone(),
            x_token: request.x_token.clone(),
            commitment: parse_commitment(request.commitment.as_deref())?,
            x_request_snapshot: request.x_request_snapshot,
            max_decoding_message_size: request
                .max_decoding_message_size
                .unwrap_or(64 * 1024 * 1024),
            connect_timeout_secs: request.connect_timeout_secs.unwrap_or(10),
            request_timeout_secs: request.request_timeout_secs.unwrap_or(30),
            http2_keep_alive_interval_secs: request.http2_keep_alive_interval_secs.unwrap_or(15),
            keep_alive_timeout_secs: request.keep_alive_timeout_secs.unwrap_or(10),
            tcp_keepalive_secs: request.tcp_keepalive_secs.unwrap_or(30),
        },
        database_url: request.database_url.clone(),
        heartbeat_secs: request.heartbeat_secs.unwrap_or(15),
        reconnect_delay_secs: request.reconnect_delay_secs.unwrap_or(3),
        max_db_connections: request.max_db_connections,
        apply_schema: request.apply_schema,
    })
}

async fn finish_live_paper(
    request: &LivePaperTaskRequest,
    resolved_strategy: &StrategyConfig,
    store: &Option<PgEventStore>,
    engine: &mut Engine<pump_agent_core::AnyStrategy>,
    persisted_snapshots: &mut Vec<pump_agent_core::PositionSnapshotInput>,
    session_id: Option<String>,
    raw_tx_count: u64,
    event_count: u64,
    last_seen_slot: Option<u64>,
    task_id: &str,
) -> Result<TaskExecution<LivePaperTaskResult>> {
    let result = engine.finish();
    let mut saved_run_id = None;

    if request.save_run {
        let Some(store) = store.as_ref() else {
            bail!("save_run requires database_url");
        };
        store.apply_schema(SCHEMA_SQL).await?;
        push_position_snapshot(
            persisted_snapshots,
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

        saved_run_id = Some(
            persist_run(
                store,
                "live_paper",
                "yellowstone",
                resolved_strategy,
                &result,
                StrategyRunPersistOptions {
                    run_mode: Some("live_paper".to_string()),
                    live_run_id: session_id.clone(),
                    position_snapshots: persisted_snapshots.clone(),
                    ..Default::default()
                },
            )
            .await?,
        );
    }

    let recorded_evaluation_id = if let Some(context) = &request.experiment {
        let Some(store) = store.as_ref() else {
            bail!("experiment context requires database_url");
        };
        let experiment = store
            .get_experiment(&context.experiment_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("experiment not found: {}", context.experiment_id))?;
        let evaluation_id = generate_record_id("eval");
        store
            .create_evaluation(
                &evaluation_id,
                &context.experiment_id,
                context.hypothesis_id.as_deref(),
                saved_run_id,
                Some(task_id),
                &experiment.target_wallet,
                Some(&resolved_strategy.strategy),
                Some(result.report.strategy.name),
                "live_paper_task",
                &request.stream.endpoint,
                Some(result.report.ending_equity_lamports as f64),
                json!({
                    "ending_equity_lamports": result.report.ending_equity_lamports,
                    "ending_cash_lamports": result.report.ending_cash_lamports,
                    "fills": result.report.fills,
                    "rejections": result.report.rejections,
                    "open_positions": result.report.open_positions,
                }),
                json!({
                    "task_id": task_id,
                    "task_kind": "live_paper",
                    "session_id": &session_id,
                    "saved_run_id": saved_run_id,
                    "strategy": resolved_strategy,
                    "report": &result.report,
                }),
                &context.failure_tags,
                context.artifact_paths.clone(),
                context.notes.clone(),
                context.conclusion.as_deref(),
            )
            .await?;
        Some(evaluation_id)
    } else {
        None
    };

    Ok(cancelled(TaskExecution {
        status: "cancelled".to_string(),
        result: LivePaperTaskResult {
            strategy: resolved_strategy.strategy.clone(),
            raw_tx_count,
            event_count,
            last_seen_slot,
            session_id,
            saved_run_id,
            recorded_evaluation_id,
            report: result.report,
        },
    }))
}

fn cancelled<T>(execution: TaskExecution<T>) -> TaskExecution<T> {
    execution
}

async fn wait_or_cancel(duration: Duration, cancel_rx: &mut watch::Receiver<bool>) -> Result<()> {
    tokio::select! {
        _ = sleep(duration) => Ok(()),
        _ = cancel_rx.changed() => Ok(()),
    }
}

fn parse_commitment(value: Option<&str>) -> Result<CommitmentLevel> {
    match value.unwrap_or("processed").to_ascii_lowercase().as_str() {
        "processed" => Ok(CommitmentLevel::Processed),
        "confirmed" => Ok(CommitmentLevel::Confirmed),
        "finalized" => Ok(CommitmentLevel::Finalized),
        other => bail!("invalid commitment level: {other}"),
    }
}

fn push_position_snapshot(
    snapshots: &mut Vec<pump_agent_core::PositionSnapshotInput>,
    snapshot: pump_agent_core::PositionSnapshotInput,
) {
    const MAX_POSITION_SNAPSHOTS: usize = 256;
    if snapshots.len() >= MAX_POSITION_SNAPSHOTS {
        snapshots.remove(0);
    }
    snapshots.push(snapshot);
}

fn should_disable_resume(message: &str) -> bool {
    message.contains("from_slot is not supported")
        || message.contains("failed to get replay position")
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
