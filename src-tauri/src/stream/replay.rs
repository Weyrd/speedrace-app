use super::ReplayRun;
use crate::models::app_state::AppState;
use crate::state::SharedState;
use crate::{mlog, LogCat};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const POLL_MS: u64 = 200;
const KEEP_WHILE_WAITING: usize = 2;

#[derive(Debug, Clone)]
pub(crate) struct SegmentLine {
    pub index: u32,
    pub start: f64,
    pub end: f64,
}

const PARTS_SUFFIX: &str = ".parts";
const MIXED_ENCODERS: &str = "mixed_encoders.flag";

pub(super) fn parts_dir(base: &Path) -> Option<PathBuf> {
    let stem = base.file_stem()?.to_str()?;
    Some(base.parent()?.join(format!("{stem}{PARTS_SUFFIX}")))
}

pub(super) fn is_parts_dir(p: &Path) -> bool {
    p.is_dir()
        && p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(PARTS_SUFFIX))
}

fn segment_name(run: u32, index: u32) -> String {
    format!("r{run}_s{index:05}.mp4")
}

pub(super) fn segment_pattern(run: u32) -> String {
    format!("r{run}_s%05d.mp4")
}

pub(super) fn list_name(run: u32) -> String {
    format!("r{run}.csv")
}

fn encoder_name(run: u32) -> String {
    format!("r{run}.encoder.txt")
}

pub(super) fn mixed_encoders_path(dir: &Path) -> PathBuf {
    dir.join(MIXED_ENCODERS)
}

pub(super) fn encoder_path(dir: &Path, run: u32) -> PathBuf {
    dir.join(encoder_name(run))
}

fn parse_segment_name(name: &str) -> Option<(u32, u32)> {
    let rest = name.strip_prefix('r')?.strip_suffix(".mp4")?;
    let (run, index) = rest.split_once("_s")?;
    Some((run.parse().ok()?, index.parse().ok()?))
}

pub(crate) struct ReplayArtifacts {
    dir: PathBuf,
}

impl ReplayArtifacts {
    pub(crate) fn open(base: &Path) -> Option<Self> {
        Some(Self {
            dir: parts_dir(base)?,
        })
    }

    pub(crate) fn segments(&self) -> Vec<((u32, u32), PathBuf)> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut v: Vec<((u32, u32), PathBuf)> = entries
            .flatten()
            .filter_map(|e| {
                let k = parse_segment_name(&e.file_name().to_string_lossy())?;
                Some((k, e.path()))
            })
            .collect();
        v.sort_by_key(|(k, _)| *k);
        v
    }

    pub(crate) fn trim_plan(&self) -> Option<TrimPlan> {
        read_trim_plan(&self.dir)
    }

    pub(crate) fn anchor(&self, run: u32) -> Option<RunAnchor> {
        read_anchor(&self.dir, run)
    }

    pub(crate) fn encoder(&self, run: u32) -> super::Encoder {
        std::fs::read_to_string(encoder_path(&self.dir, run))
            .ok()
            .and_then(|s| super::Encoder::parse(&s))
            .unwrap_or(super::Encoder::X264)
    }

    pub(crate) fn mixed_encoders(&self) -> bool {
        mixed_encoders_path(&self.dir).exists()
    }

    pub(crate) fn segment_durations(&self, keys: &[(u32, u32)]) -> HashMap<(u32, u32), f64> {
        let mut runs: Vec<u32> = keys.iter().map(|(r, _)| *r).collect();
        runs.sort_unstable();
        runs.dedup();
        let mut m = HashMap::new();
        for run in runs {
            for l in read_list(&self.dir.join(list_name(run))) {
                m.insert((run, l.index), (l.end - l.start).max(0.0));
            }
        }
        m
    }

    pub(crate) fn head_trim_path(&self) -> PathBuf {
        self.dir.join("head_trimmed.mp4")
    }

    pub(crate) fn filler_path(&self, after_run: u32) -> PathBuf {
        self.dir.join(format!("filler_r{after_run}.mp4"))
    }

    pub(crate) fn concat_list_path(&self) -> PathBuf {
        self.dir.join("concat.txt")
    }

    pub(crate) fn discard(&self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn parse_line(l: &str) -> Option<SegmentLine> {
    let mut f = l.split(',');
    let name = f.next()?;
    let start: f64 = f.next()?.trim().parse().ok()?;
    let end: f64 = f.next()?.trim().parse().ok()?;
    let (_, index) = parse_segment_name(name)?;
    Some(SegmentLine { index, start, end })
}

fn read_list(path: &Path) -> Vec<SegmentLine> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines().filter_map(parse_line).collect()
}

fn read_appended(path: &Path, from: u64) -> (Vec<SegmentLine>, u64) {
    let Ok(mut f) = std::fs::File::open(path) else {
        return (Vec::new(), from);
    };
    if f.seek(SeekFrom::Start(from)).is_err() {
        return (Vec::new(), from);
    }
    let mut text = String::new();
    if f.read_to_string(&mut text).is_err() {
        return (Vec::new(), from);
    }
    // A trailing partial line is left for the next poll.
    let complete = text.rfind('\n').map_or(0, |i| i + 1);
    let lines = text[..complete].lines().filter_map(parse_line).collect();
    (lines, from + complete as u64)
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub(crate) struct TrimPlan {
    pub run: u32,
    pub first_index: u32,
    pub head_trim_ms: i64,
}

fn trim_plan_path(dir: &Path) -> PathBuf {
    dir.join("trimplan.json")
}

fn read_trim_plan(dir: &Path) -> Option<TrimPlan> {
    let text = std::fs::read_to_string(trim_plan_path(dir)).ok()?;
    serde_json::from_str(&text).ok()
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub(crate) struct RunAnchor {
    pub anchor_ms: i64,
    pub last_end_ms: i64,
}

fn anchor_path(dir: &Path, run: u32) -> PathBuf {
    dir.join(format!("r{run}.anchor.json"))
}

fn read_anchor(dir: &Path, run: u32) -> Option<RunAnchor> {
    let text = std::fs::read_to_string(anchor_path(dir, run)).ok()?;
    serde_json::from_str(&text).ok()
}

pub(crate) async fn supervise_run(
    state: SharedState,
    run: ReplayRun,
    run_idx: u32,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    let mut anchor_ms: Option<i64> = None;
    let mut last_end_ms: i64 = 0;
    let mut lines: Vec<SegmentLine> = Vec::new();
    let mut read_pos: u64 = 0;
    let mut pruned: usize = 0;
    let mut planned = false;

    loop {
        tokio::select! {
            _ = stop_rx.changed() => break,
            _ = tokio::time::sleep(std::time::Duration::from_millis(POLL_MS)) => {}
        }

        let (fresh, next_pos) = read_appended(&run.list, read_pos);
        if !fresh.is_empty() {
            let (now_server, _) = match state.lock() {
                Ok(g) => (crate::autosplit::now_epoch_ms() + g.clock_offset_ms, ()),
                // Leave read_pos where it is so these lines are re-read next tick.
                Err(_) => continue,
            };
            for l in &fresh {
                let sample = now_server - (l.end * 1000.0) as i64;
                anchor_ms = Some(anchor_ms.map_or(sample, |a: i64| a.min(sample)));
            }
            last_end_ms = fresh
                .last()
                .map_or(last_end_ms, |l| (l.end * 1000.0) as i64);
            lines.extend(fresh);
            read_pos = next_pos;
        }

        let (phase, countdown) = match state.lock() {
            Ok(g) => (g.app_state.clone(), g.countdown_start_at_ms),
            Err(_) => continue,
        };

        let waiting = matches!(phase, AppState::StreamSetup | AppState::WaitingForStart);
        if waiting && !planned && lines.len() > KEEP_WHILE_WAITING {
            let upto = lines.len() - KEEP_WHILE_WAITING;
            for l in &lines[pruned..upto] {
                let p = run.dir.join(segment_name(run_idx, l.index));
                let _ = std::fs::remove_file(&p);
            }
            pruned = pruned.max(upto);
        }

        if planned {
            continue;
        }
        let (Some(anchor), Some(countdown)) = (anchor_ms, countdown) else {
            continue;
        };
        if let Some(head) = lines
            .iter()
            .find(|l| anchor + (l.end * 1000.0) as i64 > countdown)
        {
            let head_start_wall = anchor + (head.start * 1000.0) as i64;
            let plan = TrimPlan {
                run: run_idx,
                first_index: head.index,
                head_trim_ms: (countdown - head_start_wall).max(0),
            };
            match serde_json::to_string(&plan)
                .map_err(|e| e.to_string())
                .and_then(|s| {
                    std::fs::write(trim_plan_path(&run.dir), s).map_err(|e| e.to_string())
                }) {
                Ok(()) => mlog!(LogCat::Stream, "[replay] trim plan: {plan:?}"),
                Err(e) => mlog!(LogCat::Stream, "[replay] trim plan write failed: {e}"),
            }
            planned = true;
        }
    }

    let Some(anchor_ms) = anchor_ms else { return };
    let a = RunAnchor {
        anchor_ms,
        last_end_ms,
    };
    if let Err(e) = serde_json::to_string(&a)
        .map_err(|e| e.to_string())
        .and_then(|s| std::fs::write(anchor_path(&run.dir, run_idx), s).map_err(|e| e.to_string()))
    {
        mlog!(LogCat::Stream, "[replay] anchor write failed: {e}");
    }
}
