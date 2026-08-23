use std::time::Instant;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(super) struct ProgressBlock {
    pub(super) frame: Option<u64>,
    pub(super) out_time_us: Option<u64>,
}

#[derive(Default)]
pub(super) struct ProgressParser {
    pending: ProgressBlock,
}

impl ProgressParser {
    pub(super) fn feed(&mut self, line: &str) -> Option<ProgressBlock> {
        let (key, value) = line.split_once('=')?;
        let value = value.trim();
        match key {
            "frame" => self.pending.frame = value.parse().ok(),
            "out_time_us" => self.pending.out_time_us = value.parse().ok(),
            "progress" => return Some(std::mem::take(&mut self.pending)),
            _ => {}
        }
        None
    }
}

#[derive(Default)]
pub(super) struct RealtimeWatchdog {
    last: Option<(Instant, u64)>,
}

impl RealtimeWatchdog {
    pub(super) fn observe(&mut self, now: Instant, out_time_us: u64) -> Option<f64> {
        let prev = self.last.replace((now, out_time_us));
        let (prev_at, prev_out) = prev?;
        if out_time_us < prev_out {
            return None;
        }
        let wall = now.duration_since(prev_at).as_secs_f64();
        if wall <= 0.0 {
            return None;
        }
        Some((out_time_us - prev_out) as f64 / 1_000_000.0 / wall)
    }
}
