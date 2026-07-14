# SPEC — ffmpeg streaming (local preview → Publish → WHIP + MP4)

Status: **implemented** (2026-07-13) except the Future work section at the end. This is the
single streaming spec — it replaced `README_STREAM_V2.md`, `PLAN_LOCAL_PREVIEW_PUBLISH.md` and
`PLAN_MP4_REPLAY.md`. It records both *what* the system does and *why* each decision was made,
so a change that looks like a simplification can be checked against the constraint it would break.

## What it does

Nothing leaves the machine until the racer presses **Publish**. Inside a lobby:

1. **StreamSetup** auto-starts a **local preview**: a preview-mode ffmpeg captures the selected
   source and streams JPEG frames to the webview. No WHIP, no MP4, no audio. The host sees the
   racer as *not ready* the whole time (see Back contract).
2. Clicking the preview opens the **source picker** (Windows / Fullscreen tabs, live thumbnails).
   Selecting a source restarts the preview.
3. **Publish** runs one Rust transaction: kill preview → spawn the real ffmpeg (WHIP live + MP4
   replay when applicable) → await the first progress block (= live) → `POST stream-ready` →
   `WaitingForStart`. Any failure tears everything down, deletes the stub MP4, restores the
   preview and returns the error — nothing was POSTed, the host never saw ready.
4. From WaitingForStart/Racing the webview shows the **self-WHEP** playback of the published
   stream.

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

## Design decisions and why

Each of these was decided deliberately (several after spikes or user calls). Don't undo one
without re-checking its reason.

- **Publish-gated everything.** Publishing to WHIP as-soon-as-setup meant the racer was visible
  (and recorded) before consenting; the whole point of the rework is that nothing is public or
  on disk until one explicit click.
- **One Publish button = go live + record + ready.** Collapsing the old Start-stream + Ready
  two-step removes the state where a stream is live but the racer isn't ready — a state that had
  no purpose and needed its own UI.
- **The preview is a second ffmpeg, not the live one muted.** The live pipeline can't run
  without publishing (the WHIP output *is* the pipeline), and keeping one process with outputs
  toggled would mean restarting ffmpeg on Publish anyway. A tiny separate preview process keeps
  the live args identical to the proven form.
- **Preview transport: `-f mpjpeg` → stdout, relayed by Rust** (Content-length-framed JPEG
  parts, re-emitted as base64 events). Rejected: ffmpeg's `-listen 1` HTTP server — it serves
  `application/octet-stream` (unreliable for `<img>` MJPEG) and http-on-an-https-origin is
  mixed-content-blocked in the prod custom-protocol origin; it would happen to work in dev and
  break in prod. Rejected: a Tauri `Channel` — the preview is Rust-owned and must survive
  webview component remounts (StrictMode double-mount); events are also the house pattern.
- **Rust owns the preview lifecycle; the webview has zero lifecycle code.**
  `preview::ensure_for_phase` runs at every transition that can land on StreamSetup and
  `start()` is hard-gated on `app_state == StreamSetup`. This is what makes the frontend
  useEffect-free and makes "which paths start/stop the preview" a one-file question.
- **One teardown choke point.** `stream::shutdown` kills preview *and* live session. It already
  had nine call sites (logout, stop, forfeit, finish, WS PlayerResult/LobbyClosed, auth-lost,
  banned, app exit); putting preview teardown inside it means every current *and future* exit
  path handles the preview for free instead of requiring per-site patching.
- **Window selections are session-only; only the monitor choice persists.** An HWND dies with
  its process, so persisting it can only produce a broken restore. Exe/title re-resolution was
  deferred until re-picking proves annoying.
- **MP4 replay = a second output on the *same* ffmpeg process.** One process shares one capture
  and keeps the supervisor, graceful stop and Job Object unchanged. Not the `tee` muxer: tee
  needs identical codecs per branch, but WHIP mandates Opus and MP4 wants AAC — so it's two full
  output blocks via `split`/`asplit` (at ~2× encode cost; the reason NVENC is the natural
  follow-up). Without a replay, the args stay byte-identical to the proven single-output form.
- **Fragmented MP4** (`+frag_keyframe+empty_moov`): a hard-killed ffmpeg still leaves a playable
  file — no moov repair, no next-launch recovery code.
- **The replay spans Publish → stop, not just the race.** Trimming to the race would require
  restarting ffmpeg at the gun, which would drop the live WHIP stream. The racer trims later;
  the run itself is always fully captured. A mid-race reconnect necessarily starts a new file
  (`…_pt{n}.mp4`) — each restart is a new process.
- **Preview is video-only.** Audio capture starts at Publish exactly like before; a pre-publish
  level meter wasn't worth wiring cpal early.

## Requirements

**ffmpeg 8+ with a real DTLS-SRTP backend.** The `-f whip` muxer shipped in ffmpeg 8.0
(Aug 2025). We bundle a **minimal from-source static win64 build** (~10 MB, `--enable-gnutls`,
GPLv3, only the components this pipeline uses) as a Tauri sidecar — run
`src-tauri/scripts/get-ffmpeg.ps1` once after cloning (see
`src-tauri/scripts/README.md`); CI runs it automatically on the Windows leg, and
`bundle.externalBin` lives in `tauri.windows.conf.json` so the macOS/Linux legs build without a
sidecar. The muxer defaults to H.264 + Opus, exactly what MediaMTX ingests.

> **DTLS backend gotcha.** BtbN's default `win64-gpl` build is **SChannel**-only. It compiles
> the whip muxer and lists `dtls` in `-protocols`, but the handshake fails at runtime with
> `SEC_E_ALGORITHM_MISMATCH (0x80090331)` — SChannel's DTLS-SRTP doesn't negotiate with
> MediaMTX. Any replacement binary must show `--enable-gnutls`/`--enable-openssl`/
> `--enable-mbedtls` in `ffmpeg -buildconf` and both `dtls` and `srtp` in `ffmpeg -protocols`.

**Windows-only.** `ddagrab`, WGC, WASAPI loopback, `\\.\pipe\` transports and the Job Object are
Windows primitives; the `stream/` module and its commands are `#[cfg(windows)]` with
non-Windows stubs that return an error.

## Rust module — `src-tauri/src/stream/`

| File | Role |
| --- | --- |
| `types.rs` | **Every data type of the module** (`StreamState`, `CaptureSource`, `StreamSettings`, `LaunchSpec`, sessions, payloads…). Logic files hold no type defs. |
| `mod.rs` | `start`/`publish`/`shutdown`/`shutdown_spawn`, `current_source`, `emit_status`, replay path helpers. |
| `preview.rs` | Local preview: preview-mode ffmpeg → mpjpeg on stdout → base64 `stream:preview` events. `ensure_for_phase` auto-starts it on StreamSetup. |
| `pipeline.rs` | `build_args` / `build_preview_args` → the exact ffmpeg CLI, branched on `CaptureSource`. |
| `ffmpeg.rs` | `resolve_ffmpeg_path`, spawn (tokio::process), Job Object, the **supervisor** task, graceful stop. |
| `wgc.rs` | Window capture: WGC session → fixed-size BGRA letterbox → paced rawvideo named pipe. |
| `monitors.rs` / `window_list.rs` | `list_monitors` (DXGI, same order as ddagrab `output_idx`) / `list_windows` (filtered, non-cloaked). |
| `thumbs.rs` | Picker thumbnails: monitor one-shot ffmpeg (or the preview's last frame), window WGC one-shots behind a `Semaphore(2)`. |
| `audio.rs` | cpal WASAPI loopback on a dedicated thread + a paced named-pipe writer. |

`GlobalState` holds `stream: Option<StreamSession>`, `preview: Option<PreviewSession>`,
`capture_source: Option<CaptureSource>` (session-only; the monitor variant also persists as
`stream_monitor_index`) and `preview_last_jpeg`. The mutex is held only briefly, never across
an `.await`.

## Lifecycle

### Local preview (`preview.rs`)

- `ensure_for_phase` is called at every transition that can land on StreamSetup: WS
  `lobby_setup`, reconnect catch-up, startup restore, `stop_stream`, and the supervisor's
  pre-race death branch.
- Pipeline: `<source input> -vf …scale=640:-2…yuvj420p -q:v 7 -r 15 -f mpjpeg pipe:1` — no
  audio, no WHIP, no MP4 (a confidence preview, not the stream; ~15 KB/frame × 15 fps keeps IPC
  cheap). Rust parses the frames and emits `stream:preview` `{ frame: base64 }`; fatal problems
  emit `{ error }` and clear the session — **no auto-restart loop**, the user re-picks a source.
- **Stop is a plain kill**, awaited. Nothing external to release (the `q` dance only exists for
  the WHIP DELETE), and awaiting the exit guarantees the preview and live ffmpeg never hold the
  same Desktop Duplication concurrently.

### Publish (`stream::publish`)

One awaitable transaction; the frontend button just awaits the command promise with a busy
spinner — no new FSM state, no event orchestration:

1. stop preview (kill + await exit),
2. `stream::start` — the supervisor, handed a `oneshot::Sender<()>` fired on the first
   `-progress` block,
3. await the signal with a **25 s timeout**,
4. live → `post_stream_ready` → `AppState::WaitingForStart`,
5. failure → full `stream::shutdown`, delete the never-went-live MP4 stub, restart the preview,
   `Err(msg)`. No `stream-stopped` POST — ready was never set.

The **recording window is Publish → stop/finish** by construction.

### The supervisor (`ffmpeg::supervise`)

One task owns audio, the WGC thread (window sources) **and** ffmpeg for the whole session,
including mid-race restarts:

1. (window source) start WGC → video named pipe; start audio → build args → spawn ffmpeg
   (Job Object + `kill_on_drop` + `CREATE_NO_WINDOW`).
2. `run_child` reads ffmpeg's `-progress` on stdout: **first progress block ⇒ live** (emits
   `stream:status live` + fires the publish oneshot). stderr is tailed and logged. No progress
   for >10 s after live ⇒ dead. **Never live within 20 s of spawn ⇒ dead** — a hung WHIP
   handshake must not wedge the Publish button (the 25 s command timeout is the belt; this
   pre-live killer also covers mid-race restarts). A `watch` stop signal ⇒ write `q\n`
   (graceful WHIP DELETE) → `timeout(3s)` → else `kill()`.
3. on unexpected death, branch on `app_state` (below).

### Mid-race resilience

- **StreamSetup / WaitingForStart** death: POST `stream-stopped` (resets ready flags), set
  `AppState::StreamSetup`, emit `error` — the preview auto-restarts there.
- **RaceInProgress**: **never POST stream-stopped** — the back forfeits the runner on it. Emit
  `reconnecting`, auto-restart (3 attempts, 5 s apart; each restart writes a new `…_pt{n}.mp4`
  segment); on success emit `live`, on exhaustion emit `error`.
- **Back dies mid-race**: nothing to do — `ServerUnavailable`/WS-drop deliberately does *not*
  touch ffmpeg, so a mid-race server restart never kills the stream.

### Teardown & orphan prevention

`stream::shutdown` (or `shutdown_spawn`) is the single choke point — preview *and* live —
called from: logout, `stop_stream`, forfeit, the local finish, WS `PlayerResult`/`LobbyClosed`,
auth-lost/banned, and app exit. Orphans: a process-lifetime Job Object with
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` covers hard app deaths (both ffmpegs are assigned);
`kill_on_drop` and a best-effort graceful stop on `RunEvent::Exit` (`lib.rs`) cover clean quits.

## ffmpeg pipeline (`pipeline.rs`)

Video input branches on `CaptureSource`:

- **Monitor** → `-f lavfi -i ddagrab=output_idx={n}:framerate={fps}`; the filter is prefixed
  `hwdownload,format=bgra,` because ddagrab emits d3d11 *hardware* frames — feeding them
  straight to a software encoder fails with "Impossible to convert between formats".
- **Window** → `-f rawvideo -pix_fmt bgra -video_size {W}x{H} -framerate {fps}
  -i \\.\pipe\momentum_video_{nonce}` (CPU BGRA; no hwdownload).

Common live tail: `scale=1280:-2:flags=bilinear,format=yuv420p`, x264 veryfast zerolatency
baseline, `-g 2*fps`, Opus 96k, `-f whip`. With a replay, video/audio fan out through
`filter_complex` `split`/`asplit` to the WHIP encoder + an MP4 encoder (`-profile:v high` +
normal GOP — the live profile's `zerolatency`/`baseline` are latency tradeoffs a VOD shouldn't
pay — AAC 160k, `-movflags +frag_keyframe+empty_moov`).

`-ts_buffer_size 4194304` (4 MB UDP send buffer) is **load-bearing**: the DTLS handshake takes
~1.3 s during which capture keeps queuing frames; when the muxer opens they flush at once and
overflow the default ~64 KB socket buffer → `EAGAIN` → "Conversion failed". The 4 MB buffer only
fills during that transient, so it adds no steady-state latency.

### Window capture (`wgc.rs`)

ffmpeg has no WGC input, so Rust runs the capture (via the `windows-capture` crate) and feeds a
rawvideo named pipe. Three constraints shape it:

- `-f rawvideo` demands **one fixed frame size** (a size change alters the per-frame byte count
  and kills ffmpeg). The size is locked to the window rect at start (rounded even); later frames
  are **center-cropped/padded** into that target, so a mid-game resize degrades gracefully. The
  buffer is wiped once per size change to avoid stale borders.
- WGC only delivers frames **on change** — a static menu screen would starve the encoder until
  the stall killer fired. A **paced writer** re-sends the latest frame at constant fps.
- Window closed ⇒ writer stops ⇒ pipe EOF ⇒ ffmpeg exits ⇒ the normal death branch handles it
  (mid-race: reconnect attempts; the restart re-resolves the HWND and fails cleanly if the game
  is gone).

### Audio (`audio.rs`)

cpal WASAPI loopback on a dedicated thread (input stream on the default *output* device),
silent-track fallback, a paced writer that pads zeros during digital silence and drops >200 ms
of backlog, riding `\\.\pipe\momentum_audio_{nonce}`. stdin stays reserved for the `q` quit.

## IPC contract

**Events** (`events.rs` ↔ `lib/events.ts`):

- `stream:status` — `{ state: "connecting"|"live"|"reconnecting"|"error"|"stopped", message? }`
  → `StreamStatusChanged` in the reducer.
- `stream:preview` — `{ frame: base64 } | { error }` → consumed **imperatively** by
  `PreviewCanvas` (callback-ref subscription writing to the `<img>` node; 15 fps must never
  touch React state).

**Commands** (registered in `lib.rs`, wrapped in `lib/commands.ts`):

- `publish_stream(lobbyId)` — the whole go-live transaction (replaced the old `start_stream` +
  `send_stream_ready` pair).
- `stop_stream(lobbyId)` — graceful stop → best-effort `post_stream_stopped` →
  `AppState::StreamSetup` (the preview auto-restarts there).
- `restart_preview()` — used after a source change; start/stop are otherwise Rust-internal.
- `get_capture_source()` / `set_capture_source(source)` — `CaptureSource` is a tagged enum
  (`{kind:"monitor",index}` / `{kind:"window",hwnd,title}`, const-object mirror in `types/`);
  setting a monitor also persists it.
- `get_stream_settings()` / `set_stream_settings(...)` — bitrate/framerate/replay knobs only
  (`tauri_plugin_store`; the source is *not* part of this DTO, so settings edits never churn
  the preview).
- `list_monitors()` / `list_windows()` — picker data.
- `capture_monitor_thumb(index)` / `capture_window_thumb(hwnd)` — base64 JPEG thumbnails.

The frontend FSM carries `streamStatus`; `StreamReady` fires only after `publish_stream`
resolves (Rust confirmed live, so the reducer does not re-guard on the possibly-lagging local
`streamStatus`).

## Back contract (verified in momentum-back)

- `stream-ready` is a **pure boolean flip** (`services/lobby/lobby_service/stream.rs`) — no
  MediaMTX API, no publisher check; the back never knows whether anyone is publishing.
- Race start requires `all(stream_ready && web_ready)`, and `web_ready` **requires
  `stream_ready` first** — so the racer's web "ready" button stays blocked until they hit
  Publish in the app, and the host sees them unready during the whole preview phase.
  **Intended, not a bug**: nothing is public until Publish.
- `stream-stopped` mid-race forfeits the player — hence the two rules above: the
  publish-failure path POSTs nothing, and the mid-race death branch never POSTs it.

## Source picker (`SourcePicker.tsx`)

Clicking the preview opens a full-screen picker with **Windows / Fullscreen** tabs (Windows
first and default — it's the common case) and a thumbnail grid; selecting calls
`set_capture_source` and restarts the preview.

- Monitor thumbnails: a one-shot ffmpeg ddagrab — except the monitor the preview is currently
  duplicating, which reuses the preview's **last JPEG** (`preview_last_jpeg`). Two Desktop
  Duplication sessions on the same output can hit `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE`
  (undocumented session cap); reusing the frame sidesteps the race entirely.
- Window thumbnails load **lazily** (spinner per card): short-lived WGC one-shots, capped at 2
  concurrent by a semaphore. Dead windows show a dash. A titled list without thumbnails is the
  documented fallback if one-shots ever prove too slow.
- `list_windows` filters to visible, non-tool, non-child, non-DWM-cloaked, game-sized windows —
  the filter is what keeps per-window thumbnails affordable.

## WHEP self-preview (post-publish)

After publish the webview plays the racer's own stream via WHEP (`src/stream/whep.ts`,
`WhepPreview.tsx`), retrying the offer on 404 until MediaMTX has the publisher. `whep_url` comes
from the back (LobbySetup / lobby-current), with a `whip→whep` string fallback for old payloads.

## Ranked replay (MP4 VOD)

Ranked races always record; casual races record behind the `stream_replay_casual` opt-in
(default off — recording is driven by the lobby's `race_type`, which the back sends in
LobbySetup; an old back without it defaults to casual, failing safe). `resolve_replay_base`
decides at publish time; files land in `stream_replay_dir` (default `Videos\Momentum`) as
`momentum_{game}_{stamp}.mp4`, auto-deleted after `REPLAY_RETENTION_DAYS` (7) by a best-effort
startup sweep when `stream_replay_autodelete` is on. The `Finished` screen shows "replay saved /
show in folder" whenever a replay was actually recorded. A publish that never went live deletes
its stub file.

## Verification drills

The compile gates prove none of this; these are the runtime drills a streaming change should
re-run:

- StreamSetup: local preview visible; MediaMTX shows **no publisher**; no MP4 written; the web
  lobby shows the racer as not-ready until Publish.
- Publish → spinner → live + ready in one step → WaitingForStart; MP4 begins at Publish.
- Publish failure (unreachable WHIP URL / kill ffmpeg mid-handshake) → error shown, preview
  returns, button not wedged, no MP4 corpse, host never saw ready, nothing POSTed.
- Stop from WaitingForStart → StreamSetup → preview auto-restarts.
- Window source: preview shows only that window; Publish → web viewer confirms; resize /
  minimize / close mid-stream (close mid-race takes the no-forfeit reconnect branch); relaunch →
  window choice gone, monitor fallback works.
- Ranked run → one playable 720p MP4; casual (opt-out) → no file; hard-killed ffmpeg → the
  fragmented MP4 still plays; mid-race reconnect → `…_pt2.mp4`, both playable; disk-full/bad
  dir → WHIP survives, replay error logged.
- Stop/forfeit/finish/lobby-close/logout/app-quit → no orphaned ffmpeg (preview or live) or WGC
  session.

## Future work (aspirational — NOT built)

- **Hardware encoding (NVENC/AMF).** Probe `ffmpeg -encoders` and prefer
  `h264_nvenc`/`h264_amf` over software x264 — the replay's second encode doubles
  CPU, which is exactly what a hardware encoder absorbs. Both encoders are already
  compiled into the bundled sidecar, so this is purely an app-side arg change.
