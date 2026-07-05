// Dev-gated, category-filtered logging. Release is silent unless MOMENTUM_LOG is set.
// Categories are *types* of log (ws, livesplit, wasm, ...), not severity levels.
// Filter (evaluated once): MOMENTUM_LOG="all"|"none"|"ws,livesplit,..." selects categories.
// Legacy WS_DEBUG=true still forces the Ws category. With nothing set: all on in dev, off in release.

use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LogCat {
    Ws,
    LiveSplit,
    Wasm,
    Autosplit,
    Api,
    Auth,
    Stream,
    Counter,
    Lifecycle,
}

const ALL: [LogCat; 9] = [
    LogCat::Ws,
    LogCat::LiveSplit,
    LogCat::Wasm,
    LogCat::Autosplit,
    LogCat::Api,
    LogCat::Auth,
    LogCat::Stream,
    LogCat::Counter,
    LogCat::Lifecycle,
];

impl LogCat {
    fn key(self) -> &'static str {
        match self {
            LogCat::Ws => "ws",
            LogCat::LiveSplit => "livesplit",
            LogCat::Wasm => "wasm",
            LogCat::Autosplit => "autosplit",
            LogCat::Api => "api",
            LogCat::Auth => "auth",
            LogCat::Stream => "stream",
            LogCat::Counter => "counter",
            LogCat::Lifecycle => "lifecycle",
        }
    }
}

fn compute() -> [bool; ALL.len()] {
    let mut on = [false; ALL.len()];
    match std::env::var("MOMENTUM_LOG") {
        Ok(spec) => {
            for token in spec.split(',') {
                match token.trim().to_ascii_lowercase().as_str() {
                    "" => {}
                    "all" | "*" => on = [true; ALL.len()],
                    "none" | "off" => on = [false; ALL.len()],
                    other => {
                        if let Some(c) = ALL.iter().find(|c| c.key() == other) {
                            on[*c as usize] = true;
                        }
                    }
                }
            }
        }
        // No explicit filter: everything in dev, nothing in release.
        Err(_) => {
            if cfg!(debug_assertions) {
                on = [true; ALL.len()];
            }
        }
    }
    if std::env::var("WS_DEBUG").unwrap_or_default() == "true" {
        on[LogCat::Ws as usize] = true;
    }
    on
}

pub fn enabled(cat: LogCat) -> bool {
    static FILTER: OnceLock<[bool; ALL.len()]> = OnceLock::new();
    FILTER.get_or_init(compute)[cat as usize]
}

// Gate a log line on its category. Keeps each call site's own `[tag]` prefix; the
// category only decides whether the line prints. Zero output in release by default.
macro_rules! mlog {
    ($cat:expr, $($arg:tt)*) => {
        if $crate::logging::enabled($cat) {
            eprintln!($($arg)*);
        }
    };
}
pub(crate) use mlog;
