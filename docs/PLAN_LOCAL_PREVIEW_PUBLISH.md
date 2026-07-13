# Plan — local preview + explicit Publish (go-live & record on click) + source picker

> **Status: IMPLEMENTED** (2026-07-13). The live documentation is `README_STREAM_V2.md`;
> this file is kept as the design record. `PLAN_WINDOW_PICKER.md` was absorbed here and deleted.

**Goal (user's words):** we currently publish to WHIP/WHEP *as soon as the stream starts*, and
record the MP4 from that moment too. We shouldn't. The racer should get a **local preview of
ffmpeg's capture with nothing published and nothing recorded**, pick their source (monitor **or**
game window), and only when they press **Publish** do we (1) go live on WHIP, (2) start the MP4,
and (3) signal ready to the host — all in one action.

This absorbs and supersedes `PLAN_WINDOW_PICKER.md` (Phase 3): the source picker is built here
because the preview must already handle both monitor and window sources.

## Decisions locked (from the user)

| Question | Decision |
| --- | --- |
| Scope | **All together**: local-preview rework **and** the full window picker in one feature. |
| What "Publish" does | **One button** = go live (WHIP) + start MP4 + signal stream-ready → `WaitingForStart`. Collapses today's Start-stream + Ready two-step. |
| Preview mechanism | **ffmpeg local preview**: a preview-mode ffmpeg captures and emits frames the app displays; on Publish it's torn down and the real WHIP+MP4 ffmpeg starts. |
| HWND persistence | **Re-pick each session** — window choice is not persisted (monitor choice still is). Exe/title identity re-resolution deferred until it proves annoying. |
| Window thumbnails | **Lazy WGC thumbnails** — the grid renders immediately with spinners; real screenshots fade in per window. A titled list (no thumbnails) is the documented fallback if WGC one-shots prove too slow. |
| Preview audio | **Video only** — audio capture starts at Publish exactly as today; no pre-publish cpal wiring, no level meter. |

## Spike results (verified against the bundled `binaries/ffmpeg-x86_64-pc-windows-msvc.exe`)

The preview approach is **feasible** — confirmed, not assumed:

- `ddagrab → hwdownload,format=bgra,scale=…,format=yuvj420p → JPEG` works in this environment
  (25 KB for a 640-wide frame). ddagrab emits `d3d11` hardware frames; the `hwdownload` step is
  mandatory before the JPEG encoder (same filter tail the live pipeline already uses).
- The binary has the `mpjpeg` / `mjpeg` / `image2pipe` muxers and the `http` protocol.
- **Transport chosen: mpjpeg → stdout, relayed by Rust.** `-f mpjpeg` to a pipe produces clean
  multipart frames: `--ffmpeg` boundary, `Content-type: image/jpeg`, `Content-length: N`, then
  the JPEG bytes. Rust reads stdout, splits on `Content-length`, forwards each JPEG to the webview.
- **Rejected: ffmpeg `-f mpjpeg -listen 1 http://…`.** It streams, but ffmpeg serves it as
  `application/octet-stream` (not `multipart/x-mixed-replace`), so a direct `<img>` MJPEG render
  is unreliable; and http-on-an-https-origin is blocked as mixed content in the production
  custom-protocol origin. (Dev CSP is `null` and dev origin is `http://localhost:1420`, so it
  would *happen* to work in dev and break in prod — avoid.)

## Back contract (verified in momentum-back, file:line cited)

Checked before building — the back has **zero dependency on WHIP publishing happening early or
existing at all**:

- `stream_ready` is a **pure boolean flip** (`services/lobby/lobby_service/stream.rs:364-396`):
  no MediaMTX API call, no webhook, no publisher-exists check. The back has no HTTP client at
  all; its only MediaMTX touchpoint is URL string-minting (`stream_helpers.rs:11-18`).
- Race start is gated on `all(p.stream_ready && p.web_ready)` (`membership.rs:52`) — semantics
  unchanged as long as Publish POSTs stream-ready.
- **Intended UX consequence:** `web_ready` *requires* `stream_ready` first (`stream.rs:31-39`;
  referee force-ready too, `player_mgmt.rs:44`). With the new flow the racer's web "ready" button
  stays blocked until they click Publish in the app, and the host sees them unready during the
  whole preview phase. That is correct — nothing is visible until Publish — not a bug.
- `stream_stopped` mid-race still forfeits (`stream.rs:436`). The new flow must not emit spurious
  stream-stopped: a **failed publish POSTs nothing** (ready was never set; pre-race
  stream-stopped is only a flag reset anyway).

---

## Target flow & state machine

Phases are unchanged (`… → StreamSetup → WaitingForStart → RaceInProgress → Finished`). What
changes is **behavior inside `StreamSetup`** and the button semantics.

**StreamSetup (new):**
1. On entry (lobby joined) the app **auto-starts the local preview** for the currently selected
   source. Nothing is published; nothing is recorded; the host does not see the racer as ready.
2. The preview box renders the live local frames. Clicking it opens the **source picker** modal
   (Fullscreen / Windows). Selecting a source **restarts the preview** with the new source.
3. A single **Publish** button → the awaitable `publish_stream` command (below). The button shows
   a local "Publishing…" spinner while the promise is pending — same pattern as the StopModal
   spinner. On success Rust has already moved the app to `WaitingForStart`; on error the message
   is shown and the preview is already back.

**WaitingForStart / Racing:** unchanged — we're live, so the existing **self-WHEP** preview of
the published stream is shown. The MP4 is recording (started at Publish). The existing
`StopModal` → `stop_stream` teardown applies; stopping bounces to StreamSetup, where the preview
auto-restarts.

**Recording window:** the MP4 spans **Publish → Stop/finish**, not Start-of-setup → Stop. That is
the whole point — `resolve_replay_base` and the replay half of `pipeline.rs` are unchanged
(ranked always, casual behind the opt-in); recording simply begins with the real ffmpeg, which now
starts at Publish.

---

## Part A — local preview subsystem (Rust)

New `src-tauri/src/stream/preview.rs` (`#[cfg(windows)]` with non-Windows stubs, like the rest of
`stream/`):

- `PreviewSession { stop_tx, join }`, stored in `GlobalState.preview: Option<PreviewSession>`
  (sibling to `stream`, `state.rs`).
- `start_preview(app, state)`: **idempotent — no-ops if a preview is already running**
  (StrictMode double-effect safe). Reads the selected `CaptureSource` + settings; spawns a
  **preview-mode ffmpeg** (`pipeline::build_preview_args`); Rust reads its **stdout**, parses
  mpjpeg frames (Content-length framed), and emits each via the `stream:preview` event. Same Job
  Object + `kill_on_drop` + `CREATE_NO_WINDOW` as the live ffmpeg. Rust also keeps the **latest
  JPEG** around (thumbnail fallback, see risks).
- `stop_preview(...)`: **plain kill, no graceful dance** — the preview publishes nothing, so
  there is no WHIP DELETE to trigger; `q\n` stdin is pointless. Kill + **await the child's exit**
  so a following live spawn never overlaps the same Desktop Duplication.
- `restart_preview(...)`: stop + start; called by the SourcePicker after `set_stream_settings`.
  `set_stream_settings` itself stays a dumb save — bitrate/framerate edits in SettingsPanel must
  not churn the preview; only the picker changes the source.
- On unexpected preview death (bad monitor index, ffmpeg error): emit the error variant of
  `stream:preview` and clear the session. **No auto-restart loop** — the UI shows a placeholder +
  message and the user re-picks a source.

`pipeline::build_preview_args(source, …)` — **no audio, no WHIP, no MP4**:
```
-hide_banner -loglevel error -nostats
<source input: ddagrab=output_idx=N:framerate=15   |   rawvideo pipe for a window>
-vf hwdownload,format=bgra,scale=<w>:-2,format=yuvj420p    (drop hwdownload for the window/rawvideo path)
-q:v 7 -r 15 -f mpjpeg pipe:1
```
Preview target ~**480–640 px wide @ ~15 fps** (a confidence preview, not the stream).

**Event contract:** one event `stream:preview` (constants in `events.rs` + `lib/events.ts`) with a
tagged payload `{ frame: base64 } | { error: message }`. ~15 KB/frame × 15 fps ≈ 300–500 KB/s over
IPC after base64 — acceptable; tune size/fps/quality down if needed. Chosen over a Tauri `Channel`
deliberately: the preview is Rust-owned and survives component remounts (StrictMode double-mount),
and events are the house pattern. The webview must **not** put frames in React state — a dedicated
listener sets an `<img>` ref's `src` imperatively.

**Teardown ownership — one choke point.** `stream::shutdown` itself also kills any
`PreviewSession`. It is already called from **nine** sites (`auth_commands.rs` logout,
`stream_commands.rs` stop, `lobby_commands.rs` forfeit/finish, `ws/handler.rs`
PlayerResult/LobbyClosed, `ws/client.rs` + `auth/refresh.rs` banned/auth-lost, `lib.rs` app exit) —
every current and future teardown path handles the preview for free, no per-site patching.
(The preview never needs a `post_stream_stopped` — the server was never told anything.)

---

## Part B — the Publish path (Rust + commands)

**`publish_stream(lobby_id)` is ONE awaitable Rust command** — Rust owns the whole transaction,
the frontend just awaits the promise with a busy flag. No new FSM state, no event orchestration:

1. stop the preview (kill + await exit),
2. `stream::start` — the existing supervisor (WHIP + MP4), now handed a `oneshot::Sender<()>`
   that `run_child` fires on the first `-progress` block (= went live),
3. await that signal with a **timeout (~20 s)**,
4. on live → `post_stream_ready` (existing) → `AppState::WaitingForStart` → emit `APP_STATE`,
5. on timeout / spawn error / early death → full `stream::shutdown`, **delete the never-went-live
   MP4 fragment** (no 200 KB corpses in `Videos\Momentum`), **restart the preview**, return
   `Err(msg)`. Nothing is POSTed to the back — ready was never set.

**Supervisor fix (pre-existing gap, now load-bearing):** in `ffmpeg.rs` the stall-kill requires
`went_live`, so a WHIP handshake that hangs (no progress block ever) is never killed — today a
cosmetic "Connecting…" hang, but it would wedge the Publish button. Add:
`!went_live && spawn_elapsed > PRELIVE_TIMEOUT (20 s)` → kill, treat as death. (The command-level
timeout is the belt; this covers mid-race restarts too.)

**Command surface changes:** `start_stream` (auto-publish-on-setup) is removed entirely;
`send_stream_ready` remains a Rust-internal fn used by `publish_stream` (the frontend `streamReady`
action is removed); `stop_stream` remains only for WaitingForStart's StopModal.

---

## Part C — source picker + window capture (folds in `PLAN_WINDOW_PICKER.md`)

The preview must render **both** monitor and window sources, so window capture is built here.

- **`CaptureSource` model + settings** (Step 1 of the picker plan): widen the bare
  `stream_monitor_index` into `CaptureSource = Monitor { index } | Window { hwnd, title }` across
  `settings.rs`, `stream/mod.rs` `StreamSettings`, `stream_commands.rs` DTO + get/set, and
  `src/types/index.ts` + `useStreamSettings.ts`. Only the monitor variant is **persisted**
  (re-pick-each-session decision): a window selection lives in `GlobalState`/settings-in-memory
  for the session and falls back to the persisted monitor on app restart. TS side uses the
  **const-object enum** for the kind (repo convention), not a bare union.
- **`list_windows`** command (`stream/windows.rs`): `EnumWindows` filtered to real, visible,
  non-cloaked, non-tool top-level windows with a title and reasonable size →
  `WindowInfo { hwnd, title, process_name }`. The filter is what keeps per-window thumbnails
  affordable.
- **Thumbnails** (`capture_thumbnails`): monitors via a one-shot ddagrab frame → JPEG → base64
  (cheap). Windows via a short-lived **WGC one-shot** per HWND → base64, delivered
  **lazily/async** — the grid renders first with spinners, thumbnails fade in as each capture
  completes (a Tauri event per thumbnail; cap concurrency). New dep recommendation: the
  `windows-capture` crate over hand-rolled WGC on the `windows` crate.
- **Live window capture** (`stream/wgc.rs`): a dedicated capture thread (mirrors `audio.rs`) —
  WGC session for the HWND → each BGRA frame **letterboxed to a fixed target size** → written to
  `\\.\pipe\momentum_video_<nonce>`. Fixed size is mandatory: `-f rawvideo` has one declared
  `-video_size`; a resize would change the per-frame byte count and kill ffmpeg. Handle minimize
  (hold/pad last frame), close (ends capture → reuse the existing mid-race death branch), DPI
  change (re-normalize).
- **`pipeline.rs`** branches on `settings.source` for **both** the live args and the preview
  args: `Monitor → ddagrab`; `Window → -f rawvideo -pix_fmt bgra -video_size WxH -framerate F -i
  {video_pipe}` (and drop the leading `hwdownload,format=bgra` since frames are already CPU BGRA).
- **`SourcePicker.tsx`** (custom shared component → `components/` root, shadcn `Dialog` shell):
  header **Fullscreen / Windows** toggle; responsive grid of `Button`-card thumbnails
  (`rounded-sm`). Selecting → `set_stream_settings(source)` → `restart_preview` → modal closes.

---

## Part D — frontend wiring

- **New event** `stream:preview` in `events.rs` **and** `lib/events.ts`; `onPreviewFrame` in
  `lib/listeners.ts` (via `safeListen`). A `PreviewCanvas` component owns the `<img>` ref and the
  subscription; frames never touch the reducer. The error variant flips it to a placeholder +
  message.
- **`StreamSetup.tsx`**: starts the preview on mount, stops it on unmount via `useEffect` —
  sanctioned external-system sync; combined with idempotent `start_preview` this uniformly covers
  every way into StreamSetup (initial entry, stop-stream bounce, supervisor pre-race-death
  bounce). Renders `PreviewCanvas` (clickable → `SourcePicker`). The live-state Cancel/Ready
  buttons and the inline monitor `<select>` are removed (StreamSetup is never live anymore);
  replaced by the **Publish** button with its local busy spinner.
- **New commands** wired in `lib/commands.ts` + `useActions`: `startPreview`, `stopPreview`,
  `restartPreview`, `publishStream`, `listWindows`, `captureThumbnails`. Register the Rust side
  in `lib.rs`. Removed from the frontend: `startStream`, `streamReady`.
- **i18n** (`src/locales/{en,fr}/app.json`, `stream` block): `publish`, `publishing`,
  `preview_starting`, `preview_error`, plus the picker keys (`source_picker_title`,
  `tab_fullscreen`, `tab_windows`, `no_windows`, `thumbnail_loading`, `select_source`). FR:
  "écran"/"fenêtre", infinitive for buttons, no em-dashes, "race" not "course".

---

## Execution order & gates

Build bottom-up; the running dev server locks the default target dir, so gate the Rust with
`CARGO_TARGET_DIR=target-gate cargo check` / `cargo clippy`.

1. **`CaptureSource` model + settings + DTO widening** (Part C step 1). Gate: `cargo check` + `pnpm build`.
2. **Preview subsystem** — `preview.rs` + `build_preview_args` + the `stream:preview` event +
   idempotent `start_preview` / kill-only `stop_preview` / `restart_preview` commands + the
   `stream::shutdown` choke-point integration, **monitor source only** first. Gate: `cargo check`.
3. **Frontend preview** — `PreviewCanvas`, StreamSetup auto-preview, still with the old monitor
   `<select>`. Manually verify: local preview renders, MediaMTX shows **no publisher**, nothing
   recorded. Gate: `pnpm build`.
4. **Publish path** — the awaitable `publish_stream`, the pre-live supervisor timeout fix, MP4
   cleanup on failed publish, Publish button, remove `start_stream`/`streamReady`. Manually
   verify: preview → Publish → live+ready in one step → WaitingForStart; MP4 starts at Publish;
   a forced failure returns to preview without wedging. Gate: `cargo check` + `pnpm build`.
5. **Window capture** — `list_windows`, `wgc.rs`, pipeline `Window` branch for **both** preview
   and live. **Pause here to confirm window→preview and window→WHIP interop** before modal polish.
   Gate: `cargo check`.
6. **Thumbnails + `SourcePicker` modal**, remove the inline `<select>`. Gate: `pnpm build`.
7. **Docs** — promote Phase 3 in `README_STREAM_V2.md` to implemented; document the preview
   subsystem (preview-mode ffmpeg, mpjpeg-stdout relay, `stream:preview`), the one-command
   Publish flow, the pre-live timeout, the WGC capture thread + fixed-size normalization, the
   picker, and the web-ready-blocked-until-Publish UX note. Update the `momentum-app` skill.
   Delete/retire `PLAN_WINDOW_PICKER.md` (absorbed here).

## Verification (end-to-end)

- StreamSetup shows a live local preview; MediaMTX shows **no** publisher and **no** MP4 is
  written until Publish; the web lobby shows the racer as not-ready (web ready button blocked)
  during the whole preview phase.
- Publish → spinner → live + ready in one step → WaitingForStart; MP4 begins at Publish.
- Publish failure (kill ffmpeg during the handshake / unreachable WHIP URL) → error shown,
  preview returns, button not wedged, no MP4 corpse, host never saw ready, nothing POSTed.
- Stop from WaitingForStart → StreamSetup → preview auto-restarts.
- Pick a windowed game in the picker → preview shows only that window → Publish → web viewer
  confirms the same.
- Resize/maximize/DPI-move the game mid-stream → stays alive at the fixed target size.
- Minimize/close the game mid-race → no crash; close reuses the no-forfeit reconnect branch.
- Relaunch the game → window choice is gone (re-pick), monitor fallback works.
- Stop/forfeit/finish/lobby-close/logout/app-quit → **no orphaned ffmpeg (preview or live) or WGC
  session** — the single `stream::shutdown` choke point covers all nine teardown sites.
- Monitor path unchanged (regression).

## Flagged risks

- **Preview→publish handoff** briefly freezes the local preview (~1 s while ffmpeg swaps). No
  viewer impact — nothing is published during preview. Acceptable by the chosen design.
- **IPC frame throughput** — keep preview small/low-fps; imperative `<img>` update, never React
  state per frame.
- **Thumbnail vs preview DDA contention.** The picker opens while the preview ffmpeg holds a
  Desktop Duplication session; a one-shot ddagrab of the same output usually coexists on Win10+
  but can hit `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE` (undocumented session cap). Ladder:
  (a) concurrent one-shots — try first; (b) reuse the preview's **latest JPEG** as the current
  monitor's thumbnail, one-shot only the others; (c) pause the preview while the picker is open.
  Decide at step 6.
- **WGC window capture** is the real Phase-3 cost/risk: per-window thumbnail latency, fixed-size
  normalization, minimize/close/DPI edge cases. Pause after step 5 to de-risk; a titled list
  without thumbnails is the fallback.
- **`ddagrab` single-duplication** — the preview and live ffmpeg must never duplicate the same
  monitor simultaneously; `publish_stream` awaits the preview child's exit before spawning live.
