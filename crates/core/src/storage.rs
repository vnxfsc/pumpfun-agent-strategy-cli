use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::model::EventEnvelope;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPumpTransaction {
    pub slot: u64,
    pub signature: String,
    pub tx_index: u32,
    pub program_id: String,
    pub logs: Vec<String>,
    pub raw_base64: String,
    pub block_time: Option<i64>,
}

pub trait EventStore {
    fn append_raw_transaction(&mut self, tx: RawPumpTransaction) -> Result<()>;
    fn append_events(&mut self, events: &[EventEnvelope]) -> Result<()>;
    fn replay_events(&self) -> Result<Vec<EventEnvelope>>;
}
