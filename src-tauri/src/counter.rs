use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::api::counter_config::{CounterCadence, CounterConfig, CounterMode};
use crate::state::SharedState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterSample {
    pub value: i64,
    pub split_index: Option<u32>,
    pub at_ms: u64,
}

#[derive(Debug)]
pub enum CounterBuffer {
    Total(Option<CounterSample>),
    PerSplit {
        per_split: BTreeMap<u32, CounterSample>,
        no_split: Option<CounterSample>,
    },
    Timeline(Vec<CounterSample>),
}

impl CounterBuffer {
    pub fn for_mode(mode: CounterMode) -> Self {
        match mode {
            CounterMode::Total => CounterBuffer::Total(None),
            CounterMode::PerSplit => CounterBuffer::PerSplit {
                per_split: BTreeMap::new(),
                no_split: None,
            },
            CounterMode::Timeline => CounterBuffer::Timeline(Vec::new()),
        }
    }

    pub fn record(&mut self, sample: CounterSample) {
        match self {
            CounterBuffer::Total(slot) => *slot = Some(sample),
            CounterBuffer::PerSplit {
                per_split,
                no_split,
            } => match sample.split_index {
                Some(idx) => {
                    per_split.insert(idx, sample);
                }
                // No split context degrades to latest-wins
                None => *no_split = Some(sample),
            },
            CounterBuffer::Timeline(events) => events.push(sample),
        }
    }

    pub fn drain(&mut self) -> Vec<CounterSample> {
        match self {
            CounterBuffer::Total(slot) => slot.take().into_iter().collect(),
            CounterBuffer::PerSplit {
                per_split,
                no_split,
            } => {
                let mut out: Vec<CounterSample> = std::mem::take(per_split).into_values().collect();
                out.extend(no_split.take());
                out
            }
            CounterBuffer::Timeline(events) => std::mem::take(events),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CounterAction {
    Post,
    Buffer(CounterMode),
    Drop,
}

// Unknown counter = discovered default Total+EndOnly: buffer so it flushes as one POST at finish,
// not a per-event flood. Disabled = drop; Instant = post now; else buffer (mode-shaped).
pub fn resolve_action(cfg: Option<&CounterConfig>) -> CounterAction {
    match cfg {
        None => CounterAction::Buffer(CounterMode::Total),
        Some(c) if !c.enabled => CounterAction::Drop,
        Some(c) if c.cadence == CounterCadence::Instant => CounterAction::Post,
        Some(c) => CounterAction::Buffer(c.mode),
    }
}

pub async fn flush_all_counter_buffers(app: &AppHandle, state: &SharedState, lobby_id: &str) {
    flush_counter_buffers(app, state, lobby_id, None).await;
}

pub async fn flush_counter_buffers(
    app: &AppHandle,
    state: &SharedState,
    lobby_id: &str,
    only: Option<CounterCadence>,
) {
    let batches: Vec<(String, Vec<CounterSample>)> = {
        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let config = guard.counter_config.clone();
        let names: Vec<String> = guard.counter_buffers.keys().cloned().collect();
        let mut out = Vec::new();
        for name in names {
            if let Some(filter) = only {
                let cadence = config
                    .as_ref()
                    .and_then(|c| c.iter().find(|x| x.counter_name == name))
                    .map(|x| x.cadence);
                if cadence != Some(filter) {
                    continue;
                }
            }
            if let Some(buf) = guard.counter_buffers.get_mut(&name) {
                let samples = buf.drain();
                if !samples.is_empty() {
                    out.push((name, samples));
                }
            }
        }
        out
    };

    for (name, samples) in batches {
        if let Err(e) = crate::api::lobby::post_player_counter(app, lobby_id, name, samples).await {
            eprintln!("[counter] flush post: {e}");
        }
    }
}

#[cfg(test)]
mod tests;
