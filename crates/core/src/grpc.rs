use std::{collections::HashMap, time::Duration};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures::{Sink, channel::mpsc};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::{
    prelude::{
        CommitmentLevel, SubscribeRequest, SubscribeRequestFilterTransactions,
        SubscribeRequestPing, SubscribeUpdate, SubscribeUpdateTransaction,
        SubscribeUpdateTransactionInfo, subscribe_update,
    },
    prost::Message,
};

use crate::{
    decoder::decode_anchor_events_from_logs, model::EventEnvelope, storage::RawPumpTransaction,
};

pub const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";

#[derive(Debug, Clone)]
pub struct YellowstoneConfig {
    pub endpoint: String,
    pub x_token: Option<String>,
    pub commitment: CommitmentLevel,
    pub x_request_snapshot: bool,
    pub max_decoding_message_size: usize,
    pub connect_timeout_secs: u64,
    pub request_timeout_secs: u64,
    pub http2_keep_alive_interval_secs: u64,
    pub keep_alive_timeout_secs: u64,
    pub tcp_keepalive_secs: u64,
}

impl Default for YellowstoneConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            x_token: None,
            commitment: CommitmentLevel::Processed,
            x_request_snapshot: false,
            max_decoding_message_size: 64 * 1024 * 1024,
            connect_timeout_secs: 10,
            request_timeout_secs: 30,
            http2_keep_alive_interval_secs: 15,
            keep_alive_timeout_secs: 10,
            tcp_keepalive_secs: 30,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodedPumpTransaction {
    pub raw: RawPumpTransaction,
    pub events: Vec<EventEnvelope>,
}

pub async fn connect_pump_subscription(
    config: YellowstoneConfig,
    from_slot: Option<u64>,
) -> Result<(
    impl Sink<SubscribeRequest, Error = mpsc::SendError>,
    impl futures::Stream<Item = Result<SubscribeUpdate, tonic::Status>>,
)> {
    let mut builder = GeyserGrpcClient::build_from_shared(config.endpoint.clone())
        .context("failed to create Yellowstone gRPC builder")?
        .x_token(config.x_token.clone())
        .context("failed to configure Yellowstone x-token")?
        .connect_timeout(Duration::from_secs(config.connect_timeout_secs))
        .timeout(Duration::from_secs(config.request_timeout_secs))
        .http2_keep_alive_interval(Duration::from_secs(config.http2_keep_alive_interval_secs))
        .keep_alive_timeout(Duration::from_secs(config.keep_alive_timeout_secs))
        .keep_alive_while_idle(true)
        .tcp_keepalive(Some(Duration::from_secs(config.tcp_keepalive_secs)))
        .max_decoding_message_size(config.max_decoding_message_size)
        .set_x_request_snapshot(config.x_request_snapshot);

    if config.endpoint.starts_with("https://") {
        builder = builder
            .tls_config(ClientTlsConfig::new().with_native_roots())
            .context("failed to configure TLS for Yellowstone endpoint")?;
    }

    let mut client = builder
        .connect()
        .await
        .context("failed to connect to Yellowstone gRPC endpoint")?;

    client
        .subscribe_with_request(Some(pump_subscribe_request(config.commitment, from_slot)))
        .await
        .context("failed to subscribe to Pump transactions")
}

pub async fn subscribe_pump_transactions(
    config: YellowstoneConfig,
) -> Result<impl futures::Stream<Item = Result<SubscribeUpdate, tonic::Status>>> {
    let (_, stream) = connect_pump_subscription(config, None).await?;
    Ok(stream)
}

pub fn pump_subscribe_request(
    commitment: CommitmentLevel,
    from_slot: Option<u64>,
) -> SubscribeRequest {
    let mut transactions = HashMap::new();
    transactions.insert(
        "pump".to_string(),
        SubscribeRequestFilterTransactions {
            vote: Some(false),
            failed: Some(false),
            signature: None,
            account_include: Vec::new(),
            account_exclude: Vec::new(),
            account_required: vec![PUMP_PROGRAM_ID.to_string()],
        },
    );

    SubscribeRequest {
        accounts: HashMap::new(),
        slots: HashMap::new(),
        transactions,
        transactions_status: HashMap::new(),
        blocks: HashMap::new(),
        blocks_meta: HashMap::new(),
        entry: HashMap::new(),
        commitment: Some(commitment as i32),
        accounts_data_slice: Vec::new(),
        ping: None,
        from_slot,
    }
}

pub fn pump_ping_request(id: i32) -> SubscribeRequest {
    SubscribeRequest {
        accounts: HashMap::new(),
        slots: HashMap::new(),
        transactions: HashMap::new(),
        transactions_status: HashMap::new(),
        blocks: HashMap::new(),
        blocks_meta: HashMap::new(),
        entry: HashMap::new(),
        commitment: None,
        accounts_data_slice: Vec::new(),
        ping: Some(SubscribeRequestPing { id }),
        from_slot: None,
    }
}

pub fn decode_transaction_update(
    update: &SubscribeUpdate,
) -> Result<Option<DecodedPumpTransaction>> {
    let created_at = update.created_at.as_ref().map(|ts| ts.seconds);
    let Some(update_oneof) = update.update_oneof.as_ref() else {
        return Ok(None);
    };

    let subscribe_update::UpdateOneof::Transaction(transaction) = update_oneof else {
        return Ok(None);
    };

    decode_subscribe_transaction(transaction, created_at)
}

pub fn decode_subscribe_transaction(
    transaction: &SubscribeUpdateTransaction,
    created_at: Option<i64>,
) -> Result<Option<DecodedPumpTransaction>> {
    let Some(info) = transaction.transaction.as_ref() else {
        return Ok(None);
    };

    decode_transaction_info(info, transaction.slot, created_at)
}

pub fn assign_sequence_numbers(events: &mut [EventEnvelope], next_seq: &mut u64) {
    for event in events {
        event.seq = *next_seq;
        *next_seq += 1;
    }
}

fn decode_transaction_info(
    info: &SubscribeUpdateTransactionInfo,
    slot: u64,
    created_at: Option<i64>,
) -> Result<Option<DecodedPumpTransaction>> {
    let Some(meta) = info.meta.as_ref() else {
        return Ok(None);
    };

    let events = decode_anchor_events_from_logs(&meta.log_messages)?;
    if events.is_empty() {
        return Ok(None);
    }

    let signature = bs58::encode(&info.signature).into_string();
    let raw_base64 = STANDARD.encode(info.encode_to_vec());

    let events = events
        .into_iter()
        .enumerate()
        .map(|(event_index, event)| EventEnvelope {
            seq: 0,
            slot,
            block_time: created_at,
            tx_signature: signature.clone(),
            tx_index: info.index as u32,
            event_index: event_index as u32,
            event,
        })
        .collect();

    Ok(Some(DecodedPumpTransaction {
        raw: RawPumpTransaction {
            slot,
            signature,
            tx_index: info.index as u32,
            program_id: PUMP_PROGRAM_ID.to_string(),
            logs: meta.log_messages.clone(),
            raw_base64,
            block_time: created_at,
        },
        events,
    }))
}
