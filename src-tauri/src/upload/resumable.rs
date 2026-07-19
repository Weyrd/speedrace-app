use super::{emit, UploadPhase};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

// 256 KiB multiple per the resumable protocol
const CHUNK_SIZE: u64 = 8 * 1024 * 1024;
const MAX_RETRIES: u32 = 5;

enum ProbeState {
    Incomplete(u64),
    Complete(String),
}

// where did Google get to? (used to resume after an error)
async fn probe(client: &reqwest::Client, url: &str, total: u64) -> Result<ProbeState, String> {
    let resp = client
        .put(url)
        .header("Content-Length", 0)
        .header("Content-Range", format!("bytes */{total}"))
        .send()
        .await
        .map_err(|e| format!("resume probe failed: {e}"))?;
    let status = resp.status().as_u16();
    if status == 308 {
        let next = resp
            .headers()
            .get("range")
            .and_then(|v| v.to_str().ok())
            .and_then(|r| r.rsplit('-').next()?.parse::<u64>().ok())
            .map_or(0, |last| last + 1);
        return Ok(ProbeState::Incomplete(next));
    }
    if resp.status().is_success() {
        return Ok(ProbeState::Complete(parse_video_id(resp).await?));
    }
    Err(format!("resume probe returned {status}"))
}

async fn parse_video_id(resp: reqwest::Response) -> Result<String, String> {
    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("bad final response: {e}"))?;
    v.get("id")
        .and_then(|x| x.as_str())
        .map(String::from)
        .ok_or_else(|| "no video id in final response".into())
}

pub(super) async fn put_chunks(
    app: &AppHandle,
    file: &Path,
    total: u64,
    url: &str,
    cancel: &Arc<AtomicBool>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let mut f = tokio::fs::File::open(file)
        .await
        .map_err(|e| format!("replay open failed: {e}"))?;
    let mut offset: u64 = 0;
    let mut retries: u32 = 0;

    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("abandoned".into());
        }

        f.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(|e| format!("seek failed: {e}"))?;
        let len = CHUNK_SIZE.min(total - offset);
        let mut buf = vec![0u8; len as usize];
        f.read_exact(&mut buf)
            .await
            .map_err(|e| format!("read failed: {e}"))?;
        let end = offset + len - 1;

        let resp = client
            .put(url)
            .header("Content-Length", len)
            .header("Content-Range", format!("bytes {offset}-{end}/{total}"))
            .body(buf)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().as_u16() == 308 => {
                offset = r
                    .headers()
                    .get("range")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.rsplit('-').next()?.parse::<u64>().ok())
                    .map_or(end + 1, |last| last + 1);
                retries = 0;
                emit(app, UploadPhase::Uploading, offset, total, None);
            }
            Ok(r) if r.status().is_success() => {
                emit(app, UploadPhase::Uploading, total, total, None);
                return parse_video_id(r).await;
            }
            // Session gone (expired/cancelled): retrying chunks is pointless
            Ok(r) if matches!(r.status().as_u16(), 404 | 410) => {
                return Err(format!("upload session expired ({})", r.status()));
            }
            Ok(r) => {
                retries += 1;
                if retries > MAX_RETRIES {
                    return Err(format!("upload failed ({})", r.status()));
                }
                if let Some(id) =
                    backoff_and_resync(&client, url, total, retries, &mut offset).await
                {
                    return Ok(id);
                }
            }
            Err(e) => {
                retries += 1;
                if retries > MAX_RETRIES {
                    return Err(format!("network error: {e}"));
                }
                if let Some(id) =
                    backoff_and_resync(&client, url, total, retries, &mut offset).await
                {
                    return Ok(id);
                }
            }
        }
    }
}

async fn backoff_and_resync(
    client: &reqwest::Client,
    url: &str,
    total: u64,
    attempt: u32,
    offset: &mut u64,
) -> Option<String> {
    tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt.min(5)))).await;
    match probe(client, url, total).await {
        Ok(ProbeState::Incomplete(next)) => {
            *offset = next;
            None
        }
        Ok(ProbeState::Complete(id)) => Some(id),
        Err(_) => None,
    }
}
