use std::collections::VecDeque;

use anyhow::{Result, bail};
use futures::{SinkExt, StreamExt};
use pump_agent_core::{
    Engine, PgEventStore, StrategyRunPersistOptions, assign_sequence_numbers,
    connect_pump_subscription, decode_transaction_update, pump_ping_request,
};
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
    let live_run_id = if args.save_run {
        Some(generate_run_group_id("live"))
    } else {
        None
    };
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
                                        }
                                        render_live_dashboard(
                                            &engine.report_snapshot(),
                                            &engine.broker_snapshot(),
                                            engine.market_state(),
                                            &recent_activity,
                                            args.dashboard_top_mints,
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
                                            args.dashboard_top_mints,
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
                                live_run_id: live_run_id.clone(),
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

fn should_disable_resume(message: &str) -> bool {
    message.contains("from_slot is not supported")
        || message.contains("failed to get replay position")
}
