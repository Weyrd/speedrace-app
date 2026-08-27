use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

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
    match std::env::var("SPEEDRACE_LOG") {
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

const BUFFER_CAP: usize = 1500;

fn buffer() -> &'static Mutex<VecDeque<String>> {
    static BUFFER: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
    BUFFER.get_or_init(|| Mutex::new(VecDeque::with_capacity(BUFFER_CAP)))
}

pub fn redact(line: &str) -> String {
    const URL_END: [char; 6] = [' ', '\t', '\n', '\'', '"', ')'];

    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(pos) = rest.find("http://").or_else(|| rest.find("https://")) {
        out.push_str(&rest[..pos]);
        let scheme_len = if rest[pos..].starts_with("https://") {
            8
        } else {
            7
        };
        let after_scheme = &rest[pos + scheme_len..];
        let url_len = after_scheme.find(URL_END).unwrap_or(after_scheme.len());
        let url_rest = &after_scheme[..url_len];
        let host_len = url_rest.find('/').unwrap_or(url_rest.len());

        out.push_str(&rest[pos..pos + scheme_len]);
        out.push_str(&url_rest[..host_len]);
        if host_len < url_rest.len() {
            out.push_str("/...");
        }
        rest = &after_scheme[url_len..];
    }
    out.push_str(rest);
    out
}

pub fn record(cat: LogCat, args: std::fmt::Arguments) {
    let now = chrono::Local::now().format("%H:%M:%S%.3f");
    let line = redact(&format!("{now} [{}] {args}", cat.key()));
    if enabled(cat) {
        eprintln!("{line}");
    }
    if let Ok(mut b) = buffer().lock() {
        b.push_back(line);
        while b.len() > BUFFER_CAP {
            b.pop_front();
        }
    }
}

pub fn snapshot() -> Vec<String> {
    buffer()
        .lock()
        .map(|b| b.iter().cloned().collect())
        .unwrap_or_default()
}

macro_rules! mlog {
    ($cat:expr, $($arg:tt)*) => {
        $crate::logging::record($cat, format_args!($($arg)*))
    };
}
pub(crate) use mlog;
