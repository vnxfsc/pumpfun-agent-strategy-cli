use anyhow::Result;
use futures::{SinkExt, StreamExt};
use pump_agent_core::{
    PgEventStore, assign_sequence_numbers, connect_pump_subscription, decode_transaction_update,
    pump_ping_request,
};
use tokio::time::{Duration, MissedTickBehavior, sleep};

use crate::args::IngestArgs;

use super::config::resolve_ingest_runtime_config;
use crate::commands::helpers::{SCHEMA_SQL, print_event};

pub async fn ingest(args: IngestArgs) -> Result<()> {
    let runtime = resolve_ingest_runtime_config(&args)?;
    let database_url = runtime
        .database_url
        .as_ref()
        .expect("ingest requires database_url");

    let store = PgEventStore::connect(database_url, runtime.max_db_connections).await?;
    if runtime.apply_schema {
        store.apply_schema(SCHEMA_SQL).await?;
    }

    let mut last_seen_slot = store.latest_slot().await?;
    let mut next_seq = store.next_sequence().await?;
    let mut raw_tx_count = 0_u64;
    let mut event_count = 0_u64;
    let mut ping_id = 1_i32;
    let mut resume_from_db = args.resume_from_db;

    loop {
        let resume_from_slot = if resume_from_db {
            last_seen_slot.map(|slot| slot.saturating_sub(1))
        } else {
            None
        };
        println!(
            "connecting yellowstone endpoint={} from_slot={:?}",
            runtime.yellowstone.endpoint, resume_from_slot
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
                                event_count += decoded.events.len() as u64;
                                raw_tx_count += 1;
                                last_seen_slot = Some(last_seen_slot.map_or(decoded.raw.slot, |slot| slot.max(decoded.raw.slot)));
                                store.append_decoded_transaction(&decoded).await?;

                                for event in &decoded.events {
                                    print_event(event);
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
                    println!("received ctrl-c, stopping ingest");
                    println!("raw tx stored : {raw_tx_count}");
                    println!("events stored : {event_count}");
                    return Ok(());
                }
            }
        }

        println!(
            "reconnecting after {}s, last_seen_slot={:?}",
            runtime.reconnect_delay_secs, last_seen_slot
        );
        sleep(Duration::from_secs(runtime.reconnect_delay_secs)).await;
    }
}

fn should_disable_resume(message: &str) -> bool {
    message.contains("from_slot is not supported")
        || message.contains("failed to get replay position")
}
