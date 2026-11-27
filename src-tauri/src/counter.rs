use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::api::counter_config::{CounterCadence, CounterConfig, CounterMode};
use crate::logging::{mlog, LogCat};
use crate::state::{AutosplitSource, GlobalState, LockGlobalState, SharedState};

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

pub fn resolve_action(cfg: Option<&CounterConfig>) -> CounterAction {
    match cfg {
        None => CounterAction::Buffer(CounterMode::Total),
        Some(c) if !c.enabled => CounterAction::Drop,
        Some(c) if c.cadence == CounterCadence::Instant => CounterAction::Post,
        Some(c) => CounterAction::Buffer(c.mode),
    }
}

#[derive(Debug, Default)]
pub struct InstantSentValues(HashMap<String, i64>);

impl InstantSentValues {
    pub fn claim(&mut self, counter_name: &str, value: i64) -> bool {
        if self.0.get(counter_name) == Some(&value) {
            return false;
        }
        self.0.insert(counter_name.to_string(), value);
        true
    }

    pub fn release(&mut self, counter_name: &str, value: i64) {
        if self.0.get(counter_name) == Some(&value) {
            self.0.remove(counter_name);
        }
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct InstantPost {
    pub lobby_id: String,
    pub counter_name: String,
    pub sample: CounterSample,
}

pub fn admit_sample(g: &mut GlobalState, counter_name: &str, value: i64) -> Option<InstantPost> {
    if g.run_forfeited || g.pending_finish.is_some() {
        return None;
    }
    let lobby_id = g.lobby.as_ref()?.lobby_id.clone();
    let elapsed_ms = g.race_start_at.map(|start| g.server_now_ms() - start);
    let at_ms = match elapsed_ms {
        Some(ms) if ms >= 0 => ms as u64,
        _ => {
            if let Some(ms) = elapsed_ms {
                mlog!(
                    LogCat::Autosplit,
                    "[autosplit] counter sample before the start (raw={ms}ms), held until the start"
                );
            }
            g.counter_prestart.insert(counter_name.to_string(), value);
            return None;
        }
    };
    g.counter_prestart.remove(counter_name);
    if g.split_run
        .as_ref()
        .is_some_and(|run| g.current_split_index >= run.len() as u32)
    {
        return None;
    }
    let sample = CounterSample {
        value,
        split_index: Some(g.current_split_index),
        at_ms,
    };
    let cfg = g
        .counter_config
        .as_ref()
        .and_then(|c| c.iter().find(|x| x.counter_name == counter_name).cloned());
    match resolve_action(cfg.as_ref()) {
        CounterAction::Drop => None,
        CounterAction::Buffer(mode) => {
            g.counter_buffers
                .entry(counter_name.to_string())
                .or_insert_with(|| CounterBuffer::for_mode(mode))
                .record(sample);
            None
        }
        CounterAction::Post => {
            g.counter_instant_sent
                .claim(counter_name, value)
                .then(|| InstantPost {
                    lobby_id,
                    counter_name: counter_name.to_string(),
                    sample,
                })
        }
    }
}

pub fn admit_prestart_samples(g: &mut GlobalState) -> Vec<InstantPost> {
    let started = g
        .race_start_at
        .is_some_and(|start| g.server_now_ms() >= start);
    if g.counter_prestart.is_empty()
        || !started
        || g.autosplit.source != Some(AutosplitSource::Wasm)
    {
        return Vec::new();
    }
    let held = std::mem::take(&mut g.counter_prestart);
    held.into_iter()
        .filter_map(|(name, value)| admit_sample(g, &name, value))
        .collect()
}

pub fn flush_prestart_counters(app: &AppHandle, state: &SharedState) {
    let posts = admit_prestart_samples(&mut state.lock_state());
    for post in posts {
        spawn_instant_post(app, state, post);
    }
}

pub fn spawn_instant_post(app: &AppHandle, state: &SharedState, post: InstantPost) {
    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        let InstantPost {
            lobby_id,
            counter_name,
            sample,
        } = post;
        let value = sample.value;
        if let Err(e) = crate::api::lobby::post_player_counter(
            &app,
            &lobby_id,
            counter_name.clone(),
            vec![sample],
        )
        .await
        {
            mlog!(LogCat::Autosplit, "[autosplit] post_player_counter: {e}");
            state
                .lock_state()
                .counter_instant_sent
                .release(&counter_name, value);
        }
    });
}

pub fn is_valid_counter_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b' ' | b'_' | b'-'))
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
        let mut guard = state.lock_state();
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
            mlog!(LogCat::Counter, "[counter] flush post: {e}");
        }
    }
}
