use std::path::PathBuf;

use tauri::{AppHandle, Manager};

#[derive(Clone, Copy)]
pub enum CacheKind {
    Split,
    Autosplitter,
}

impl CacheKind {
    const fn subdir(self) -> &'static str {
        match self {
            CacheKind::Split => "splits",
            CacheKind::Autosplitter => "autosplitters",
        }
    }

    const fn extension(self) -> &'static str {
        match self {
            CacheKind::Split => "lss",
            CacheKind::Autosplitter => "wasm",
        }
    }
}

pub struct CachePaths {
    pub content: PathBuf,
    pub stamp: PathBuf,
}

// Resolve `<app_local_data>/<subdir>/<id>.<ext>` (content) + `<id>.stamp`. None if no data dir.
#[allow(dead_code)]
pub fn cache_paths(app: &AppHandle, kind: CacheKind, id: &str) -> Option<CachePaths> {
    let dir = app.path().app_local_data_dir().ok()?.join(kind.subdir());
    Some(CachePaths {
        content: dir.join(format!("{id}.{}", kind.extension())),
        stamp: dir.join(format!("{id}.stamp")),
    })
}
