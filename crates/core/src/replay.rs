use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use anyhow::{Context, Result};

use crate::model::EventEnvelope;

pub fn load_jsonl_events(path: impl AsRef<Path>) -> Result<Vec<EventEnvelope>> {
    let path = path.as_ref();
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for (line_number, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("failed to read line {}", line_number + 1))?;
        if line.trim().is_empty() {
            continue;
        }

        let event = serde_json::from_str::<EventEnvelope>(&line).with_context(|| {
            format!(
                "failed to parse EventEnvelope from {} at line {}",
                path.display(),
                line_number + 1
            )
        })?;
        events.push(event);
    }

    events.sort_by_key(|event| (event.slot, event.tx_index, event.event_index, event.seq));
    Ok(events)
}
