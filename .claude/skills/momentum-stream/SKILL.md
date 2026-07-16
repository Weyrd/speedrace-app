---
name: speedrace-stream
description: How the speedrace-app racer client captures and publishes its live video — the Rust-owned ffmpeg sidecar that grabs the monitor (ddagrab) + system audio (cpal WASAPI loopback) and publishes to MediaMTX via ffmpeg's native `-f whip` muxer, plus the webview's self-WHEP preview. Read this BEFORE touching anything stream-related in speedrace-app — `src-tauri/src/stream/` (mod/monitors/audio/pipeline/ffmpeg), the `stream:status` event, `start_stream`/`stop_stream`/`list_monitors`/`get`+`set_stream_settings` commands, `StreamSession` in `SharedState`, the `streamStatus` FSM field, `src/stream/whep.ts`, `WhepPreview`, `StreamSetup.tsx`, or `scripts/get-ffmpeg.ps1` — because two invariants are easy to break and both are costly: mid-race you must NEVER POST stream-stopped (the back forfeits the runner), and the whole thing needs an ffmpeg 8.x sidecar with DTLS that must be fetched once after clone or nothing streams. Phase 1 = WHIP live only; MP4 replay and window capture are future phases. The storage/contract half lives in speedrace-back (`services/lobby/.../stream.rs`), the viewer half in speedrace-web (WHEP playback).
---

# Speedrace Stream (ffmpeg WHIP + self-WHEP preview)

**Rust owns the stream.** An ffmpeg sidecar spawned by `src-tauri/src/stream/` captures the
monitor (`ddagrab`) + system audio (cpal WASAPI loopback) and publishes to MediaMTX using
ffmpeg's native `-f whip` muxer. The webview captures **nothing** anymore — it only plays the
racer's own stream back via WHEP for the local preview.

```
cpal WASAPI loopback ─▶ paced writer ─▶ \\.\pipe\speedrace_audio_<nonce> ┐
(system audio, f32le)   (silence-pads)                                  ▼
ddagrab (monitor, in-ffmpeg) ─────────────────────▶ ffmpeg sidecar ─▶ WHIP ─▶ MediaMTX
                                                    (-f whip, h264+opus)      │ WHEP
GlobalState.stream = Some(StreamSession{ stop_tx, join })                     ▼
                                    webview ◀── WhepClient (recvonly preview) ┘
```

Design doc with the full rationale: `docs/SPEC_FFMPEG.md`. This skill is the fast path;
the doc is the deep dive. Confirm both against the code — this is an actively evolving feature.

## Two invariants you must not break

1. **Mid-race, NEVER POST `stream-stopped`.** The back treats a stream-stopped during
   `RaceInProgress` as a forfeit (speedrace-back `services/lobby/lobby_service/stream.rs`). When
   ffmpeg dies mid-race the supervisor emits `reconnecting` and auto-restarts — it does **not**
   tell the back. Only pre-race (StreamSetup/WaitingForStart) is it safe to POST stream-stopped.
2. **The ffmpeg sidecar must exist and have a *working* DTLS backend.** `-f whip` shipped in
   ffmpeg 8.0, but "has the whip muxer" is not enough — it needs GnuTLS, OpenSSL, or mbedTLS for
   the DTLS-SRTP handshake. We bundle **Gyan's GPL `full_build` win64** (8.1.x, `--enable-gnutls`).
   **Do not swap in a Windows SChannel build** (BtbN's default `win64-gpl` is SChannel): it
   compiles the whip muxer and even lists `dtls` in `-protocols`, but the handshake dies at
   runtime with `SEC_E_ALGORITHM_MISMATCH (0x80090331)` / "DTLS session failed" — SChannel's
   DTLS-SRTP doesn't negotiate with MediaMTX. Verify any build with `ffmpeg -buildconf` (expect
   `--enable-gnutls`/`--enable-openssl`/`--enable-mbedtls`) and `ffmpeg -protocols` (expect both
   `dtls` and `srtp`). The sidecar is gitignored — **run `src-tauri/scripts/get-ffmpeg.ps1` once
   after clone** or `start_stream` fails with "ffmpeg sidecar not found". See "Dev prep" below.

## Rust module map (`src-tauri/src/stream/`)

| File | Role |
| --- | --- |
| `mod.rs` | `StreamState` (event enum), `StreamSettings`, `StreamSession`, `start`/`shutdown`/`shutdown_spawn`, `emit_status`. |
| `monitors.rs` | `list_monitors` command — DXGI `EnumAdapters1`→`EnumOutputs` on adapter 0, the **same order ddagrab's `output_idx` counts**. Returns structured `MonitorInfo` (no baked English; the frontend composes the label via i18n). |
| `audio.rs` | cpal WASAPI loopback on a dedicated `!Send` thread + a paced named-pipe writer. Falls back to a silent track if loopback can't init. |
| `pipeline.rs` | `build_args(settings, whip_url, audio)` → the exact ffmpeg CLI. `AudioSource::Pipe \| Silent`. Carries `-ts_buffer_size 4194304`: the ~1.3s DTLS handshake backlogs frames that flush at once when the muxer opens, overflowing the default ~64 KB UDP send buffer → `EAGAIN` (`ret=-11`, "UDP send blocked") → the muxer dies. A busy monitor bursts more, which looks like "stream works on one screen, black on another." Don't drop this arg. |
| `ffmpeg.rs` | `resolve_ffmpeg_path`, `spawn_ffmpeg`, the **supervisor** task, Job Object, graceful stop. |

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

### Orphan prevention

`tokio::process` (not tauri-plugin-shell — Rust needs direct stdio + the raw HANDLE). A
process-lifetime **Job Object** (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) + `kill_on_drop` +
best-effort graceful stop on `RunEvent::Exit`/`ExitRequested` in `lib.rs`. Verify after any
teardown change: **no orphaned `ffmpeg.exe` in Task Manager** after finish/forfeit/logout/quit.

## IPC contract

- **Event** `stream:status` (constant in both `src-tauri/src/events.rs` and `src/lib/events.ts`),
  payload `{ state: "connecting"|"live"|"reconnecting"|"error"|"stopped", message? }` →
  `onStreamStatus` (`lib/listeners.ts`) → `StreamStatusChanged` (`appReducer.ts`).
- **Commands** (registered in `lib.rs`, wrapped in `lib/commands.ts`):
  `start_stream()` (no args — reads whip_url + settings from state; the webview never handles the
  URL), `stop_stream(lobbyId)`, `list_monitors()`, `get_stream_settings()`,
  `set_stream_settings(monitorIndex, bitrateKbps, framerate)`, `send_stream_ready(lobbyId)`.
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
`stream_framerate` (=60), same pattern as `finish_hotkey` — **no settings module, no Zustand**.
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

1. `src-tauri/scripts/get-ffmpeg.ps1` once (installs the gitignored sidecar — pinned Gyan
   `full_build`, GnuTLS). `-Force` re-downloads. The SHA check fails loud if the pin ever drifts.
   If you bump the pin, re-verify the DTLS backend (`-buildconf` + `-protocols`, see invariant 2).
2. `externalBin: ["binaries/ffmpeg"]` in `tauri.conf.json` bundles it next to the exe. In dev it
   may not be copied next to the debug binary — `resolve_ffmpeg_path` has a `#[cfg(debug_assertions)]`
   fallback to `CARGO_MANIFEST_DIR/binaries/ffmpeg-*.exe`, so dev works either way.
3. Gate Rust with `CARGO_TARGET_DIR=target-gate cargo check` when a dev server is running (it locks
   the normal `target/` exe).

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
- Forfeit / finish / lobby-close / logout / app quit → **no orphaned `ffmpeg.exe`**.
- Stop stream from WaitingForStart → host sees ready reset; app returns to StreamSetup (both sides).

## Anti-patterns (never)

- POST `stream-stopped` while `RaceInProgress` (forfeits the runner).
- Hold the `SharedState` mutex across an `.await` or `app.emit`.
- Audio over stdin (steals the `q` quit → dangling MediaMTX session).
- tauri-plugin-shell for ffmpeg (need raw HANDLE for the Job Object + direct stdio).
- Tearing the stream down on `ServerUnavailable` / WS drop (it must survive a back outage).
- Bare status strings in the reducer — use `StreamStatus.*` / `StreamEventState.*`.

## Maintaining this skill

This feature is being built out — **update this file in the same change** when you touch the
stream code. Keep the module map, command list, the two invariants, and the implemented vs
future split accurate, and keep `docs/SPEC_FFMPEG.md` in sync with the pipe/output topology.
