pub mod early_start;
pub mod run_started;
pub mod split;
pub mod tcp;
pub mod timer;
pub mod wasm;

pub(crate) fn now_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
