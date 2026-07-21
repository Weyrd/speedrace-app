---
name: speedrace-stream
description: How the speedrace-app racer client captures and publishes its live video — Rust captures the picked window/monitor via WGC (`windows-capture`), or injects the OBS graphics hook into exclusive-fullscreen games (`gamecapture/`), feeds one BGRA rawvideo pipe + system audio (cpal WASAPI loopback) to an ffmpeg sidecar that muxes to MediaMTX via `-f whip` and forks a segmented MP4 VOD, plus the webview's self-WHEP preview. Read this BEFORE touching anything stream-related in speedrace-app — `src-tauri/src/stream/` (mod/monitors/audio/pipeline/ffmpeg/wgc/gamecapture/replay/preview), the `stream:status` event, `start_stream`/`stop_stream`/`list_monitors`/`get`+`set_stream_settings`/`set_capture_source` commands, `StreamSession` in `SharedState`, the `streamStatus` FSM field, `src/stream/whep.ts`, `WhepPreview`, `StreamSetup.tsx`, or `scripts/get-ffmpeg.ps1`/`get-game-capture.ps1` — because the invariants are easy to break and costly: mid-race NEVER POST stream-stopped (the back forfeits the runner); a window pick must NEVER fall back to screen capture (privacy); and it needs an ffmpeg 8.x sidecar with a real DTLS backend + the OBS win-capture binaries, both fetched once after clone or nothing streams. The storage/contract half lives in speedrace-back (`services/lobby/.../stream.rs`), the viewer half in speedrace-web (WHEP playback).
---

# Speedrace Stream (ffmpeg WHIP + self-WHEP preview)

**Rust owns the stream.** Rust captures the picked source and feeds a single fixed-size **BGRA
rawvideo named pipe** to the ffmpeg sidecar, which muxes video + system audio to MediaMTX via
`-f whip` and forks a segmented MP4 VOD. Two capture backends fill that pipe (both behind a
`CaptureHandle` with `pipe_name`/`width`/`height`/`shutdown`, so ffmpeg is agnostic):

- **WGC** (`wgc.rs`, `windows-capture`) — windowed apps, borderless games, and explicit Monitor
  picks. Fixed-size letterbox, self-healing session supervisor.
- **Game-capture injection** (`gamecapture/`) — exclusive-fullscreen games (Celeste etc.) that
  WGC captures as black. Injects OBS's graphics hook, reads the game's own D3D11 backbuffer.

The webview captures **nothing** — it only plays the racer's own stream back via WHEP.

```
cpal WASAPI loopback ─▶ paced writer ─▶ \\.\pipe\speedrace_audio_<nonce> ┐
(system audio, f32le)   (silence-pads)                                  ▼
WGC (window/monitor)   ─┐                                                │
 OR OBS-hook injection ─┴▶ latest BGRA ─▶ paced writer ─▶ \\.\pipe\momentum_video_<nonce>
 (exclusive fullscreen)                            ──▶ ffmpeg sidecar ─▶ WHIP ─▶ MediaMTX
                                                   (-f whip, h264+opus) ├▶ MP4 VOD (split=2)
GlobalState.stream = Some(StreamSession{ stop_tx, join })              │ WHEP
                                    webview ◀── WhepClient (recvonly) ──┘
```

**Capture routing (`wgc.rs::start_capture_for`, async):** windowed window → WGC window capture;
**fullscreen window → try WGC ~1.5 s, and if it stays black (exclusive fullscreen) escalate to
`gamecapture::start` injection**; Monitor pick (explicit screen-share consent) → WGC display, or
`None` → in-ffmpeg `ddagrab` fallback. `ddagrab` is a *monitor-only* net (it dies with
`DXGI_ERROR_ACCESS_LOST` on a fullscreen mode flip — never the primary path). **A window pick
must NEVER read the screen** — if injection can't deliver, publish is blocked with an error. (The
old window→monitor privacy-gate is gone — injection replaced it.)

Design doc with the full rationale: `docs/SPEC_FFMPEG.md`. This skill is the fast path;
the doc is the deep dive. Confirm both against the code.

## Invariants you must not break

1. **Mid-race, NEVER POST `stream-stopped`.** The back treats a stream-stopped during
   `RaceInProgress` as a forfeit (speedrace-back `services/lobby/lobby_service/stream.rs`). When
   ffmpeg dies mid-race the supervisor emits `reconnecting` and auto-restarts — it does **not**
   tell the back. Only pre-race (StreamSetup/WaitingForStart) is it safe to POST stream-stopped.
2. **A window pick is never the screen.** The racer picked a *window*, not their desktop. WGC
   window capture and game-capture injection are both window-only; the `ddagrab` fallback is
   reachable **only** from an explicit Monitor pick. A fullscreen game that can't be injected
   (anti-cheat) blocks publish — it never falls back to screen capture.
3. **The ffmpeg sidecar must exist and have a *working* DTLS backend.** `-f whip` shipped in
   ffmpeg 8.0, but "has the whip muxer" is not enough — it needs GnuTLS, OpenSSL, or mbedTLS for
   the DTLS-SRTP handshake. We ship our own **minimal from-source build** (`--enable-gnutls`,
   pinned in `get-ffmpeg.ps1`; Gyan's GPL `full_build` is the fallback). **Do not swap in a
   Windows SChannel build** (BtbN's default `win64-gpl`): it compiles the whip muxer and lists
   `dtls` in `-protocols`, but the handshake dies at runtime with `SEC_E_ALGORITHM_MISMATCH
   (0x80090331)` — SChannel's DTLS-SRTP doesn't negotiate with MediaMTX. Verify any build with
   `ffmpeg -buildconf` (expect `--enable-gnutls`/`--enable-openssl`/`--enable-mbedtls`) and
   `ffmpeg -protocols` (expect both `dtls` and `srtp`). Sidecar + OBS binaries are gitignored —
   **run `get-ffmpeg.ps1` AND `get-game-capture.ps1` once after clone** (see "Dev prep").

## Rust module map (`src-tauri/src/stream/`)

| File | Role |
| --- | --- |
| `mod.rs` | `StreamState` (event enum), `StreamSettings`, `StreamSession`, `start`/`shutdown`/`shutdown_spawn`, `emit_status`. |
| `monitors.rs` | `list_monitors` command — DXGI `EnumAdapters1`→`EnumOutputs` on adapter 0, the **same order ddagrab's `output_idx` counts**. Returns structured `MonitorInfo` (no baked English; the frontend composes the label via i18n). |
| `audio.rs` | cpal WASAPI loopback on a dedicated `!Send` thread + a paced named-pipe writer. Falls back to a silent track if loopback can't init. |
| `encoder.rs` | Hardware-encoder probe (`warm`/`select`/`poison`/`detected`). A **real** 2-leg trial encode — `ffmpeg -encoders` lists compiled, not usable. **The pinned sidecar's nvenc needs driver 610+ (nv-codec-headers 13.1), verified working on 610.74; below that the probe falls back to x264. AMF untested (no AMD box).** See `docs/SPEC_FFMPEG.md`. |
| `capture.rs` | **Capture router + window/monitor geometry** (the orchestration layer; the two backends are peers). `start_capture_for(source, fps)` (async) → `CaptureHandle`: windowed→WGC window capture; **fullscreen window→ try WGC ~1.5s, and if it stays black (exclusive fullscreen) escalate to `gamecapture::start` injection** — never the screen; Monitor{index}→ WGC display / `None`=ddagrab. Owns `covers_monitor`/`target_size_even`/`monitor_size_even`/`hmonitor_for_index` and computes the dims each backend needs. |
| `capture_pipe.rs` | Shared plumbing both backends call: `new_video_pipe()` (the `\\.\pipe\momentum_video_*` server) + `spawn_paced_writer()` (drains a fixed-size BGRA `latest` to the pipe at fps, black until the first frame). One implementation — do **not** re-duplicate it per backend. |
| `wgc.rs` | **WGC backend only** (no routing). `start_capture(target, w, h, fps)` locks a fixed BGRA size, letterboxes, and a **session supervisor** recreates the WGC session on close/stall while the shared writer keeps pumping the last frame. |
| `gamecapture/` | OBS-hook injection so exclusive-fullscreen games (Celeste, emulators) are captured **window-only**. `protocol.rs` (OBS `hook_info` ABI, `const`-guarded `sizeof==648`), `inject.rs` (direct `LoadLibraryW` remote thread same-bitness / `inject-helper` exe cross-bitness — a 64-bit host captures 32-bit games), `offsets.rs` (runs `get-graphics-offsets`, caches **only on success**), `session.rs` (`inject_and_arm` + `ArmedSession::try_texture`), `frame.rs` (`SharedTextureReader` — one D3D11 device, re-openable shared texture, **normalizes any source format → BGRA**: BGRA passthrough / RGBA swizzle / 10-bit unpack), `capture.rs` (`GameCaptureHandle`: dedicated thread reads the shared texture → `latest` → shared paced pipe). Binaries vendored by `scripts/get-game-capture.ps1` (OBS GPLv2). See the game-capture section below. |
| `pipeline.rs` | `build_args(settings, whip_url, audio)` → the exact ffmpeg CLI. Pipe-first video input (rawvideo BGRA from either backend), in-ffmpeg ddagrab only when no pipe. `AudioSource::Pipe \| Silent`. Carries `-ts_buffer_size 4194304`: the ~1.3s DTLS handshake backlogs frames that flush at once when the muxer opens, overflowing the default ~64 KB UDP send buffer → `EAGAIN` (`ret=-11`, "UDP send blocked") → the muxer dies. A busy monitor bursts more, which looks like "stream works on one screen, black on another." Don't drop this arg. |
| `ffmpeg.rs` | `resolve_ffmpeg_path`, `spawn_ffmpeg`, the **supervisor** task, Job Object, graceful stop. Also writes the per-run `encoder.txt` / `mixed_encoders.flag` markers the VOD assembly reads. |
| `preview.rs` | Local mpjpeg preview (own ffmpeg → base64 `stream:preview` frames). `ensure_for_phase` auto-starts it in StreamSetup. Startup **reserves** `GlobalState.preview_starting` before the slow (injecting) capture start so a second concurrent start can't double-inject, and checks `preview_gen` (bumped by `stop`) so a preview mid-startup when publish stops it cancels itself instead of running alongside the live stream. |
| `replay.rs` | Prunes and anchors the segmented MP4 branch. Reads the live `-segment_list` CSV, keeps only the newest ~2 closed segments **while the phase is StreamSetup/WaitingForStart**, and freezes a `trimplan.json` once `countdown_start_at` lands inside a closed segment. **Prune is gated on phase, not on the countdown timestamp** — an old back sends no `countdown_start_at`, and a timestamp gate would prune straight through the race. On exit it writes `r{run}.anchor.json` so the uploader can size the black filler over a reconnect gap. |

`GlobalState.stream: Option<StreamSession>` where `StreamSession { stop_tx: watch::Sender<bool>,
join }`. The mutex is held only briefly, **never across an `.await`** — the supervisor task owns
the child process; the session just holds the stop signal + join handle.

### The supervisor (`ffmpeg::supervise`) — one task owns the whole session

`start()` spawns a single supervisor that loops (for restarts):

1. `audio::start_audio()` → `pipeline::build_args` → `spawn_ffmpeg` (Job Object + `kill_on_drop` +
   `CREATE_NO_WINDOW`).
2. `run_child` reads ffmpeg's `-progress` on stdout: the **first `progress=` block ⇒ WHIP
   handshake done ⇒ emit `live`** (proxy). stderr is tailed + logged via `mlog!(LogCat::Stream, …)`.
   No progress >10 s ⇒ dead. A `watch` stop signal ⇒ write `q\n` (graceful WHIP DELETE) →
   `timeout(3s)` → else `kill()`.
3. On unexpected death, branch on `app_state` (see "Mid-race resilience").

### Audio (`audio.rs`) — the fiddly part

cpal loopback = building an **input** stream on the **default output device**. Only f32 mix
formats are handled; anything else (or any failure) falls back to **silent** so the mux still
works. The detected `(rate, channels)` are reported back so the pipeline tells ffmpeg the real
input format. A paced writer drains the ring every 20 ms targeting `bytes = elapsed × rate×ch×4`
and **pads zeros on underrun** — WASAPI loopback delivers no callbacks during digital silence, so
without padding ffmpeg stalls. Audio rides a **named pipe, not stdin**: stdin is reserved for the
`q` quit that triggers the WHIP DELETE (without it MediaMTX holds the session until ICE timeout).

### Game-capture injection (`gamecapture/`) — the exclusive-fullscreen path

Exclusive-fullscreen games bypass the DWM compositor, so WGC window capture returns black and
`ddagrab` dies on the mode flip. The only window-only capture is **graphics-hook injection** (the
OBS Game Capture model). We embed OBS's `win-capture` binaries and drive them from Rust over the
documented shared-memory/event protocol (`protocol.rs` mirrors `graphics-hook-info.h`). Ground
truth: obsproject/obs-studio `shared/obs-hook-config/` + `plugins/win-capture/game-capture.c`.

Host flow (`session.rs`): create the `CaptureHook_KeepAlive<pid>` mutex → inject
`graphics-hook{32,64}.dll` (direct `LoadLibraryW` for a same-bitness target, else `inject-helper`
exe) → open the hook-created `CaptureHook_HookInfo<pid>` mapping → write graphics offsets +
`allow_srgb_alias` + `frame_interval` → `SetEvent(init)`+`restart`. Then, per frame, resolve the
shared texture `CaptureHook_Texture_<hwnd>_<map_id>` → `shtex_data.tex_handle` → D3D11
`OpenSharedResource` (legacy `MISC_SHARED`, no keyed mutex — just `CopyResource` to a staging
texture) → **normalize `desc.Format` → BGRA** into `latest`.

Five subtleties that took a live-debug to find — **don't regress them**:

1. **Arm, don't wait for a frame.** `inject_and_arm` returns as soon as injection succeeds; it
   does **not** wait for `HookReady`. At publish time the app has focus, so an exclusive-fullscreen
   game is minimized and presents nothing — the game only renders once the racer alt-tabs in
   (after publish). So the pipe is sized to the game's **monitor** up front and carries black
   until frames arrive. Injection *failing* (anti-cheat) blocks publish; "not rendering yet" does
   not. Matches OBS/Discord (black/last-frame until the game presents).
2. **Re-resolve the texture handle.** An exclusive-fullscreen game **recreates its swapchain when
   the racer alt-tabs in** → a new shared texture with a new handle; the first one goes stale and
   the stream freezes. `capture.rs` re-checks `try_texture` every 250 ms and re-opens when the
   handle changes (`SharedTextureReader` keeps one device, just re-opens the texture — no churn).
3. **Teardown = drop keepalive only, never `EVENT_CAPTURE_STOP`.** That event is *global* — it
   would stop capture for any other live session (preview + stream run concurrently). The keepalive
   named mutex ref-counts across sessions; the hook self-stops when the last handle closes.
   `release_session` closes only the keepalive. **Never kill the game process** (the Job Object
   must not cover it — only our own ffmpeg/threads).
4. **Normalize the texture format — don't assume BGRA.** The hook shares the game's backbuffer in
   whatever format it presents: BGRA (most DXGI games), **RGBA** (Dolphin, many D3D/GL games), or
   10-bit. `frame.rs::read_into` branches on `desc.Format` (passthrough / R↔B swizzle / 10-bit
   unpack) so the pipe stays fixed BGRA and every speedrun target works. `ffmpeg`'s `-pix_fmt` is
   locked at launch (before the game renders), which is *why* the conversion must happen here, not
   in ffmpeg. An unsupported (e.g. 64-bit float) format is rejected and streams black rather than
   garbage.
5. **Injection cannot be unit-tested here** (needs a real game + GPU); validate live against a
   fullscreen game. Anti-cheat games block injection by design.

### Orphan prevention

`tokio::process` (not tauri-plugin-shell — Rust needs direct stdio + the raw HANDLE). A
process-lifetime **Job Object** (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) + `kill_on_drop` +
best-effort graceful stop on `RunEvent::Exit`/`ExitRequested` in `lib.rs`. Verify after any
teardown change: **no orphaned `ffmpeg.exe` in Task Manager** after finish/forfeit/logout/quit.

## IPC contract

- **Event** `stream:status` (constant in both `src-tauri/src/events.rs` and `src/lib/events.ts`),
  payload `{ state: "connecting"|"live"|"reconnecting"|"error"|"stopped", message? }` →
  `onStreamStatus` (`lib/listeners.ts`) → `StreamStatusChanged` (`appReducer.ts`).
- **Event** `stream:source` (both-sides constant), payload `CaptureSource` → `onStreamSource` →
  `AppEventBridge` `qc.setQueryData(captureSourceKey, …)`. Emitted by
  `stream::auto_select_game_window` when the autosplitter attaches pre-publish (see the
  `speedrace-autosplitter` skill) so the source picker/label follow the backend's auto-selection.
- **Commands** (registered in `lib.rs`, wrapped in `lib/commands.ts`):
  `start_stream()` (no args — reads whip_url + settings from state; the webview never handles the
  URL), `stop_stream(lobbyId)`, `list_monitors()`, `get_stream_settings()`,
  `set_stream_settings(bitrateKbps, framerate, resolution, replayDir, replayAutodelete,
  replayCasual, replayDeleteUploaded)` (positional; monitor index is owned by
  `set_capture_source` and preserved on save), `send_stream_ready(lobbyId)`.
- **FSM**: `StreamSetup`/`WaitingForStart`/`RaceInProgress` carry `streamStatus: StreamStatus`
  (not a `MediaStream`). Types in `src/types/index.ts`: `StreamStatus` (const-object enum) and
  `StreamEventState` (the junction that also includes `Stopped`; the event payload uses it).
  `StreamReady` (confirm-ready) only fires when `streamStatus === Live`. Compare against enum
  members (`StreamEventState.Stopped`), never bare strings.

## Teardown ownership (Rust, six sites)

`stream::shutdown` (async) or `shutdown_spawn` (from sync contexts) is called from: `logout`
(auth command), `stop_stream`, `send_player_forfeited`, the local finish (`finalize_finish`),
the WS `PlayerResult` and `LobbyClosed` arms, and the auth-lost/banned paths (`ws/client.rs`
`logout` + both `Banned` arms, `auth/refresh.rs::logout_and_notify`). **`ServerUnavailable` / WS
drop is deliberately untouched** — ffmpeg survives a back outage by construction. If you add a
race-ending path, add a shutdown call; if you add a "back is down" path, do NOT.

## Mid-race resilience (`ffmpeg::supervise` death branch)

- **StreamSetup / WaitingForStart**: safe — POST `stream-stopped` (resets ready flags), set
  `AppState::StreamSetup`, emit `error`. The reducer bounces WaitingForStart→StreamSetup.
- **RaceInProgress**: emit `reconnecting`, auto-restart (3 attempts, 5 s apart), on success emit
  `live`, on exhaustion emit `error`. **Never POST stream-stopped.** Racer keeps racing.
- **Back dies mid-race**: nothing stops ffmpeg, so nothing to do.

## `whep_url` plumbing (cross-repo)

The preview needs the WHEP URL. It's minted in speedrace-back (`stream_helpers::build_whep_url`)
and shipped to the app via **`AppEvent::LobbySetup`** and **`LobbyCurrentDto`**, mirrored in the
app's `models/lobby.rs` `LobbySetup`, `api/lobby.rs` `LobbyCurrentResponse`, and `src/types.ts`.
If a build predates the field, the frontend derives it: `whip_url.replace(/\/whip$/, "/whep")`.

## Settings (tauri_plugin_store + TanStack Query)

Keys in `settings.rs`: `stream_monitor_index`, `stream_bitrate_kbps` (=2000),
`stream_framerate` (=60), `stream_resolution` (=720), same pattern as `finish_hotkey` —
**no settings module, no Zustand**. `stream_resolution` is the output height the user picks
(720/1080); `pipeline::scale_tail` turns it into a **width-locked** `scale={h*16/9}:-2` so
ultrawide sources keep their aspect. It applies to the live WHIP leg *and* the MP4 replay,
because the `split=2` happens after the scale filter (see `build_args`). The panel couples it
to bitrate (720p → 1500/2000/2500, 1080p → 3000/4500/6000) and shows a GB/hour estimate —
only bitrate moves file size, resolution/framerate only affect sharpness.
The frontend reads/writes via query hooks in `src/hooks/useStreamSettings.ts` (`useMonitors`,
`useStreamSettings`, `useSetStreamSettings`) — server state goes through TanStack Query, not a
mount `useEffect`. `StreamSetup` and `SettingsPanel` persist on change via the mutation.

## Frontend

- `src/stream/whep.ts` `WhepClient` — recvonly WHEP (mirror of the deleted `whip.ts`): POST offer
  to `whep_url` retrying on 404 (MediaMTX 404s until the publisher connects), DELETE `Location`
  on stop. `components/ui/WhepPreview.tsx` owns a client per mount, connecting **only while
  `live`** and re-acquiring when `live` returns after a reconnect (its `useEffect` is legit —
  WebRTC is an external system). Shows a green "active" or amber "reconnecting/lost" badge.
- `StreamSetup.tsx` flow: monitor dropdown (persist on change) → **Start stream** → `connecting`
  spinner → on `live` mount `WhepPreview` → **Confirm ready** (`streamReady`) → WaitingForStart.
  Cancel → `stop_stream`. `WaitingForStart`/`Racing` render `WhepPreview` from `state.streamStatus`.

## Adding / changing things — checklists

- **A new `stream:status` value**: add to `StreamState` (Rust) + `StreamStatus`/`StreamEventState`
  (TS) + the reducer's `StreamStatusChanged` + wherever the supervisor emits it.
- **A new stream setting**: key + default in `settings.rs`, add to `StreamSettings` (Rust) +
  `StreamSettings` (TS) + `get`/`set_stream_settings` + `useStreamSettings` hook + the UI select,
  and thread it into `pipeline::build_args`.
- **Change the ffmpeg args**: `pipeline.rs::build_args` only. Test the new command by hand first
  (see Dev prep) before wiring it.
- **A new command / event**: follow the speedrace-app skill's IPC checklists (both-sides constants).

## Dev prep (do before `pnpm tauri dev`)

1. `src-tauri/scripts/get-ffmpeg.ps1` once (installs the gitignored sidecar — pinned minimal
   from-source build, GnuTLS). `-Force` re-downloads. The SHA check fails loud if the pin drifts.
   Bumping the pin ⇒ re-verify the DTLS backend (`-buildconf` + `-protocols`, see invariant 3).
2. `src-tauri/scripts/get-game-capture.ps1` once (installs the six OBS `win-capture` binaries into
   `binaries/gamecapture/`, SHA-verified; bundled via `tauri.conf.json` → `bundle.resources`).
   Needed for fullscreen-game capture; see `scripts/README.md` for the OBS pin + GPLv2 note. When
   you bump the OBS pin, re-verify `gamecapture/protocol.rs` against that tag's `hook_info` ABI.
3. Both `binaries/` sets are gitignored and resolved next to the exe (with a
   `#[cfg(debug_assertions)]` `CARGO_MANIFEST_DIR/binaries/…` fallback so dev works either way).
4. Gate Rust with `CARGO_TARGET_DIR=target-gate rtk proxy cargo check` when a dev server is running
   (it locks the normal `target/` exe; `rtk proxy` because `rtk cargo` can hide build errors).

### Manual ffmpeg smoke test (validate before trusting the pipeline)

```
ffmpeg -f lavfi -i ddagrab=output_idx=0:framerate=60 -f lavfi -i anullsrc=r=48000:cl=stereo ^
  -map 0:v -map 1:a -vf hwdownload,format=bgra,scale=1280:-2:flags=bilinear,format=yuv420p ^
  -c:v libx264 -preset veryfast -tune zerolatency -profile:v baseline -bf 0 -g 120 -r 60 ^
  -b:v 2000k -maxrate 2500k -bufsize 4000k -c:a libopus -b:a 96k -ar 48000 -ac 2 ^
  -f whip "<whip_url from a real dev lobby>"
```

Verify on the web viewer (WHEP), then press `q` and confirm MediaMTX logs an immediate DELETE +
that an instant re-publish to the same path works (this underpins the mid-race auto-restart).

## Failure drills (after any teardown/lifecycle change)

- Kill `ffmpeg.exe` mid-race → expect `reconnecting`→`live`, **no forfeit**.
- Stop the back mid-race → stream stays live on MediaMTX; app WS retries.
- Forfeit / finish / lobby-close / logout / app quit → **no orphaned `ffmpeg.exe`**, and the
  **captured game process is untouched** (inject-helper is short-lived; we never kill the game).
- Stop stream from WaitingForStart → host sees ready reset; app returns to StreamSetup (both sides).
- Publish a fullscreen game → `injecting game-capture` → `[gc] armed … awaiting game frames`
  (black); alt-tab into the game → `[gc] texture … (handle …)` and the picture tracks live, the
  handle re-resolving on subsequent alt-tabs. One `[gc] texture` per handle, not two.

## Anti-patterns (never)

- POST `stream-stopped` while `RaceInProgress` (forfeits the runner).
- Hold the `SharedState` mutex across an `.await` or `app.emit` (incl. the `preview_starting`/
  `preview_gen` guards — scope the guard, then await).
- Letting a **window** pick reach `ddagrab` / any screen capture (privacy — invariant 2).
- Signalling `EVENT_CAPTURE_STOP` on per-session game-capture teardown (it's global — drop the
  keepalive only), or killing / Job-Object-covering the captured game process.
- Blocking on `HookReady` at capture start (an exclusive-fullscreen game isn't rendering yet — arm
  and stream black until frames arrive), or latching one shared-texture handle forever (re-resolve).
- Audio over stdin (steals the `q` quit → dangling MediaMTX session).
- tauri-plugin-shell for ffmpeg (need raw HANDLE for the Job Object + direct stdio).
- Tearing the stream down on `ServerUnavailable` / WS drop (it must survive a back outage).
- Bare status strings in the reducer — use `StreamStatus.*` / `StreamEventState.*`.

## Maintaining this skill

**Update this file in the same change** when you touch the stream code. Keep the module map,
command list, invariants, and the capture-routing rules accurate, and keep `docs/SPEC_FFMPEG.md`
in sync with the pipe/output topology. Core capture (WGC window/monitor + game-capture injection +
segmented MP4) is implemented and validated live; note any new backend or lifecycle path here.
