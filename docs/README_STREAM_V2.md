# Streaming — ffmpeg sidecar (local preview → Publish → WHIP + MP4)

Status: **Phases 1–3 implemented** — monitor *and* window capture, a local unpublished
preview, an explicit Publish step (go live + record + ready in one action), and a local MP4
VOD. Hardware encoding and a minimal from-source ffmpeg remain **future work** (see the end).

## Summary

Nothing leaves the machine until the racer presses **Publish**. The flow inside a lobby:

1. **StreamSetup** auto-starts a **local preview**: a preview-mode ffmpeg captures the selected
   source and streams JPEG frames to the webview. No WHIP, no MP4, no audio. The host sees the
   racer as *not ready*; the web "ready" button stays blocked (see Back contract).
2. Clicking the preview opens the **source picker** (Fullscreen monitors / Windows tabs, live
   thumbnails). Selecting a source restarts the preview.
3. **Publish** runs one Rust transaction: kill preview → spawn the real ffmpeg (WHIP live +
   MP4 replay when applicable) → await the first progress block (= live) → `POST stream-ready`
   → `WaitingForStart`. Any failure tears everything down, deletes the stub MP4, restores the
   preview and returns the error — nothing was POSTed, the host never saw ready.
4. From WaitingForStart/Racing the webview shows the **self-WHEP** playback of the published
   stream, exactly as before.

```
┌ Tauri app (Rust) ──────────────────────────────────────────────────────────┐
│ preview:  ffmpeg (ddagrab | WGC pipe) ─ mpjpeg → stdout → base64 frames ───┼─▶ webview <img>
│                                                                             │
│ live:     cpal WASAPI loopback ──▶ paced writer ──▶ \\.\pipe\momentum_audio │
│           WGC thread (window src) ─▶ letterbox ──▶ \\.\pipe\momentum_video  │
│           ddagrab (monitor src, in-ffmpeg)          │                       │
│                                                     ▼                       │
│                                    ffmpeg sidecar ──▶ WHIP ─▶ MediaMTX ─▶ WHEP preview
│                                          └──▶ MP4 VOD (ranked / casual opt-in)
└─────────────────────────────────────────────────────────────────────────────┘
```

## Requirements (why ffmpeg 8)

ffmpeg's `-f whip` muxer shipped in **ffmpeg 8.0 (Aug 2025)** and needs a build with a real
**DTLS-SRTP backend** — GnuTLS, OpenSSL, or mbedTLS. We bundle **Gyan's GPL `full_build` static
win64** (currently 8.1.x, `--enable-gnutls`) as a Tauri sidecar — see `src-tauri/scripts/README.md`
and run `scripts/get-ffmpeg.ps1` once after cloning. The whip muxer defaults to **H.264 video +
Opus audio**, exactly what MediaMTX ingests.

> **DTLS backend gotcha.** BtbN's default `win64-gpl` build is **SChannel**-only. It compiles the
> whip muxer and lists `dtls` in `-protocols`, but the handshake fails at runtime with
> `SEC_E_ALGORITHM_MISMATCH (0x80090331)` / "DTLS session failed" — SChannel's DTLS-SRTP doesn't
> negotiate with MediaMTX. Any replacement binary must show `--enable-gnutls`/`--enable-openssl`/
> `--enable-mbedtls` in `ffmpeg -buildconf` and both `dtls` and `srtp` in `ffmpeg -protocols`.

> Windows-only. `ddagrab`, WGC, WASAPI loopback, `\\.\pipe\` transports, and the orphan-killing
> Job Object are all Windows primitives; the `stream/` module and its commands are `#[cfg(windows)]`
> with non-Windows stubs that return an error.

## Rust module — `src-tauri/src/stream/`

| File | Role |
| --- | --- |
| `types.rs` | **Every data type of the module** (`StreamState`, `CaptureSource`, `StreamSettings`, `LaunchSpec`, sessions, payloads…). Logic files hold no type defs. |
| `mod.rs` | `start`/`publish`/`shutdown`/`shutdown_spawn`, `current_source`, `emit_status`, replay path helpers. |
| `preview.rs` | The local preview: preview-mode ffmpeg → mpjpeg on stdout → base64 `stream:preview` events. `ensure_for_phase` auto-starts it on StreamSetup. |
| `pipeline.rs` | `build_args` / `build_preview_args` → the exact ffmpeg CLI, branched on `CaptureSource`. |
| `ffmpeg.rs` | `resolve_ffmpeg_path`, spawn (tokio::process), Job Object, the **supervisor** task, graceful stop. |
| `wgc.rs` | Window capture: WGC session → fixed-size BGRA letterbox → paced rawvideo named pipe. |
| `monitors.rs` / `window_list.rs` | `list_monitors` (DXGI, same order as ddagrab `output_idx`) / `list_windows` (filtered, non-cloaked). |
| `thumbs.rs` | Picker thumbnails: monitor one-shot ffmpeg (or the preview's last frame), window WGC one-shots behind a `Semaphore(2)`. |
| `audio.rs` | cpal WASAPI loopback on a dedicated thread + a paced named-pipe writer. |

`GlobalState` holds `stream: Option<StreamSession>`, `preview: Option<PreviewSession>`,
`capture_source: Option<CaptureSource>` (session-only; the monitor variant also persists) and
`preview_last_jpeg`. The mutex is held only briefly, never across an `.await`.

### Local preview (`preview.rs`)

- **Rust owns the whole lifecycle.** `preview::ensure_for_phase` is called at every transition
  that can land on StreamSetup (WS lobby_setup, reconnect catch-up, startup restore, stop_stream,
  the supervisor's pre-race death) and `start()` is hard-gated on `app_state == StreamSetup`.
  The webview has **zero** preview lifecycle code.
- Pipeline: `<source input> -vf …scale=640:-2…yuvj420p -q:v 7 -r 15 -f mpjpeg pipe:1` — no audio,
  no WHIP, no MP4. Rust parses the Content-length-framed JPEG parts and emits each as
  `stream:preview` `{ frame: base64 }`; fatal problems emit `{ error }` (no auto-restart — the
  user re-picks a source).
- **Stop is a plain kill** (nothing external to release; the `q` dance is only needed for the
  WHIP DELETE) and is awaited, so a following live spawn never overlaps the same Desktop
  Duplication. Teardown also rides `stream::shutdown` — the one choke point every exit path uses.
- Transport rationale: ffmpeg's `-listen 1` HTTP server serves `application/octet-stream`
  (unreliable for `<img>`) and would be mixed-content-blocked in prod; stdout relay is origin-proof.

### Publish (`stream::publish`)

One awaitable transaction; the frontend button just awaits the command promise:

1. stop preview (kill + await exit),
2. `stream::start` — the supervisor below, handed a `oneshot::Sender<()>` fired on the first
   `-progress` block,
3. await the signal with a **25 s timeout**,
4. live → `post_stream_ready` → `AppState::WaitingForStart`,
5. failure → full `stream::shutdown`, delete the never-went-live MP4 stub, restart the preview,
   `Err(msg)`. No `stream-stopped` POST — ready was never set.

The **recording window is Publish → stop/finish** by construction.

### The supervisor (`ffmpeg::supervise`)

One task owns audio, the WGC thread (window sources) **and** ffmpeg for the whole session,
including mid-race restarts:

1. (window source) start the WGC capture → video named pipe; start audio → build args → spawn
   ffmpeg (job object + `kill_on_drop` + `CREATE_NO_WINDOW`).
2. `run_child` reads ffmpeg's `-progress` on stdout: the **first progress block ⇒ live** (emits
   `stream:status live` + fires the publish oneshot). stderr is tailed and logged. No progress for
   >10 s after live ⇒ dead. **Never live within 20 s of spawn ⇒ dead** (a hung WHIP handshake
   must not wedge Publish). A `watch` stop signal ⇒ write `q\n` (graceful WHIP DELETE) →
   `timeout(3s)` → else `kill()`.
3. on unexpected death, branch on `app_state` (see mid-race resilience).

### ffmpeg pipeline (`pipeline.rs`)

Video input branches on `CaptureSource`:

- **Monitor** → `-f lavfi -i ddagrab=output_idx={n}:framerate={fps}` and the filter is prefixed
  `hwdownload,format=bgra,` (ddagrab emits d3d11 hardware frames).
- **Window** → `-f rawvideo -pix_fmt bgra -video_size {W}x{H} -framerate {fps} -i \\.\pipe\momentum_video_{nonce}`
  (frames arrive as CPU BGRA; no hwdownload).

Common tail (live): `scale=1280:-2:flags=bilinear,format=yuv420p`, x264 veryfast zerolatency
baseline, `-g 2*fps`, Opus 96k, `-ts_buffer_size 4194304`, `-f whip`. With a replay the video/audio
go through a `filter_complex` `split`/`asplit` fan-out to the WHIP encoder + an MP4 encoder
(`-profile:v high`, AAC, `-movflags +frag_keyframe+empty_moov`); without, the args stay identical
to the proven single-output form.

`-ts_buffer_size 4194304` (4 MB UDP send buffer) is load-bearing. The DTLS handshake takes ~1.3s,
during which capture keeps queuing frames; when the muxer opens they flush at once and overflow
the default ~64 KB socket buffer → `EAGAIN` → "Conversion failed". The 4 MB buffer only fills
during that transient flush, so it adds no steady-state latency.

### Window capture (`wgc.rs`)

ffmpeg has no WGC input, so Rust runs the capture (via the `windows-capture` crate):

- `-f rawvideo` demands **one fixed frame size**: it is locked to the window rect at start
  (rounded even). Later frames are **center-cropped/padded** into that target — a mid-game resize
  degrades gracefully instead of killing ffmpeg. The buffer is wiped once per size change.
- A **paced writer** re-sends the latest frame at constant fps: WGC only delivers frames on
  change, and a static menu screen would otherwise starve the encoder until the >10 s stall
  killer fired.
- Window closed ⇒ writer stops ⇒ pipe EOF ⇒ ffmpeg exits ⇒ the normal death branch handles it
  (mid-race: reconnect attempts; the restart re-resolves the HWND and fails cleanly if the game
  is gone).
- **HWND selections are session-only** (a handle dies with the process); only the monitor choice
  persists (`stream_monitor_index`). On restart the source falls back to the persisted monitor.

### Audio (`audio.rs`)

Unchanged from Phase 1: cpal WASAPI loopback on a dedicated thread (input stream on the default
*output* device), silent-track fallback, a paced writer that pads zeros during digital silence and
drops >200 ms of backlog, riding `\\.\pipe\momentum_audio_{nonce}`. stdin stays reserved for the
`q` graceful quit.

### Orphan prevention

`tokio::process` (not tauri-plugin-shell). A process-lifetime **Job Object** with
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` plus `AssignProcessToJobObject` covers hard deaths (both the
live and preview ffmpeg are assigned); `kill_on_drop` and a best-effort graceful stop on
`RunEvent::Exit`/`ExitRequested` (`lib.rs`) cover clean quits.

## IPC contract

**Events** (`events.rs` ↔ `lib/events.ts`):

- `stream:status` — `{ state: "connecting"|"live"|"reconnecting"|"error"|"stopped", message? }`
  → `StreamStatusChanged` in the reducer.
- `stream:preview` — `{ frame: base64 } | { error }` → consumed **imperatively** by
  `PreviewCanvas` (callback-ref subscription writing to the `<img>` node; 15 fps must never touch
  React state).

**Commands** (registered in `lib.rs`, wrapped in `lib/commands.ts`):

- `publish_stream(lobbyId)` — the whole go-live transaction (replaces the old `start_stream` +
  `send_stream_ready` pair; those commands are gone).
- `stop_stream(lobbyId)` — graceful stop → best-effort `post_stream_stopped` →
  `AppState::StreamSetup` (the preview auto-restarts there).
- `restart_preview()` — used after a source change; start/stop are otherwise Rust-internal.
- `get_capture_source()` / `set_capture_source(source)` — `CaptureSource` is a tagged enum
  (`{kind:"monitor",index}` / `{kind:"window",hwnd,title}`, const-object mirror in `types/`);
  setting a monitor also persists it.
- `get_stream_settings()` / `set_stream_settings(...)` — bitrate/framerate/replay knobs only
  (`tauri_plugin_store`; the source is *not* part of this DTO).
- `list_monitors()` / `list_windows()` — picker data.
- `capture_monitor_thumb(index)` / `capture_window_thumb(hwnd)` — base64 JPEG thumbnails
  (see picker).

The frontend FSM carries `streamStatus`; `StreamReady` fires only after `publish_stream`
resolves (Rust confirmed live, so the reducer does not re-guard on the possibly-lagging local
`streamStatus`).

## Back contract (verified in momentum-back)

- `stream-ready` is a pure boolean flip (`services/lobby/lobby_service/stream.rs`) — no MediaMTX
  API, no publisher check; the back never knows whether anyone is publishing.
- Race start requires `all(stream_ready && web_ready)`, and `web_ready` **requires `stream_ready`
  first** — so the racer's web "ready" button stays blocked until they hit Publish in the app,
  and the host sees them unready during the whole preview phase. **Intended, not a bug**: nothing
  is public until Publish.
- `stream-stopped` mid-race forfeits the player — the publish-failure path deliberately POSTs
  nothing (ready was never set).

## Teardown ownership (Rust)

`stream::shutdown` (or `shutdown_spawn`) is the single choke point — it kills the preview *and*
the live session — called from: `logout`, `stop_stream`, `send_player_forfeited`, the local
finish, the WS `PlayerResult`/`LobbyClosed` arms, the auth-lost/banned paths, and app exit.

**`ServerUnavailable` / WS drop is deliberately untouched** — ffmpeg survives a back outage by
construction, so a mid-race server restart doesn't kill the stream.

## Mid-race resilience

When the live ffmpeg dies unexpectedly, the supervisor branches on `app_state`:

- **StreamSetup / WaitingForStart**: POST `stream-stopped` (resets ready flags), set
  `AppState::StreamSetup`, emit `error` — and the local preview auto-restarts.
- **RaceInProgress**: **never POST stream-stopped** (the back would forfeit the runner). Emit
  `reconnecting` and auto-restart (3 attempts, 5 s apart; each restart writes a new `…_pt{n}.mp4`
  segment); on success emit `live`, on exhaustion emit `error`.
- **Back dies mid-race**: nothing to do — no code path stops ffmpeg.

## Source picker (`SourcePicker.tsx`)

Clicking the preview box opens a full-screen picker with **Fullscreen / Windows** tabs and a
thumbnail grid; selecting calls `set_capture_source` and restarts the preview.

- Monitor thumbnails: a one-shot ffmpeg ddagrab — except for the monitor the preview is currently
  duplicating, which reuses the preview's **last JPEG** (`preview_last_jpeg`) to avoid racing a
  second Desktop Duplication on the same output.
- Window thumbnails load **lazily** (spinner per card): each is a short-lived WGC one-shot,
  capped at 2 concurrent by a Rust semaphore. Dead windows show a dash.
- `list_windows` filters to visible, non-tool, non-child, non-DWM-cloaked, game-sized windows.

## WHEP preview (post-publish)

Unchanged: after publish the webview plays the racer's own stream via WHEP (`src/stream/whep.ts`,
`WhepPreview.tsx`), retrying the offer on 404 until MediaMTX has the publisher. `whep_url` comes
from the back (LobbySetup / lobby-current), with a `whip→whep` string fallback for old payloads.

## Ranked replay (MP4 VOD)

Ranked races always record; casual races record behind the `stream_replay_casual` opt-in
(default off). `resolve_replay_base` decides at publish time; files land in `stream_replay_dir`
(default `Videos\Momentum`) as `momentum_{game}_{stamp}.mp4`, auto-deleted after
`REPLAY_RETENTION_DAYS` (7) when `stream_replay_autodelete` is on. Fragmented MP4 keeps a
hard-killed file playable. The `Finished` screen shows "replay saved / show in folder" whenever a
replay was actually recorded. A publish that never went live deletes its stub file.

## Future work (aspirational — NOT built)

### Hardware encoding (NVENC/QSV/AMF)

Probe `ffmpeg -encoders` and prefer `h264_nvenc`/`h264_qsv`/`h264_amf` over software x264 to cut
CPU on 720p60. Software `libx264 -preset veryfast` is the current encoder.

### Minimal from-source ffmpeg

`src-tauri/scripts/build-ffmpeg.ps1` could produce a ~15–20 MB build vs the ~240 MB Gyan
`full_build` prebuilt. It is **not used** and has known bugs (see the banner in that file and
`README_FFMPEG_MINIMAL_BUILD.md`).
