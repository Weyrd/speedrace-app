---
name: speedrace-app
description: Conventions and code patterns for the Speedrace desktop app — a Tauri 2 app with a React 18 frontend (useReducer state machine) and a Rust src-tauri backend. Covers the IPC contract (invoke commands ↔ emitted events), the Phase state machine mirrored on both sides, the AppEventBridge, the SharedState global, the WebSocket/API/OAuth Rust layers, and WHIP streaming. Use when adding or editing a Tauri command, an event, a reducer phase/action, a React screen, or anything in src/ or src-tauri/.
---

# Speedrace App

**Tauri 2** desktop app — the racer-side client of the Speedrace speedrun platform. Frontend: **React 18 + Vite + Tailwind v4**, state via a `useReducer` finite-state machine (no TanStack Router; it's a single-window phase switcher). Backend: **Rust** (`src-tauri/`) owning auth (OAuth deep-link), the persistent WebSocket to speedrace-back, HTTP calls, and a `SharedState` global. The two halves talk over Tauri IPC.

> There is no copilot-instructions.md here — this skill is derived directly from the code. The one design doc is `docs/SPEC_FFMPEG.md` (streaming). Verify paths against `src/` and `src-tauri/src/`.

## Workflow

```bash
pnpm dev            # Vite frontend only (port 1420)
pnpm tauri dev      # full app (Tauri spawns `pnpm dev` via beforeDevCommand)
pnpm tauri build    # production bundle (runs `pnpm build` first)
pnpm build          # tsc + vite build (frontend type-check + bundle)
```

Rust side: `cargo build` / `cargo clippy` / `cargo check` run from `src-tauri/`. Prefix shell commands with `rtk` per the user's global convention (e.g. `rtk pnpm tauri dev`, `rtk cargo clippy`).

## The core idea: a Phase state machine mirrored across the IPC boundary

The app is a finite-state machine. The same phases exist on **both** sides and must stay in sync:

- TS: `Phase` + `AppState` union in `src/store/types.ts`
- Rust: `AppState` enum in `src-tauri/src/models/app_state.rs`

Phases: `Unauthenticated → Connecting → Idle → StreamSetup → WaitingForStart → RaceInProgress → Finished`. `App.tsx` is a pure `switch (state.phase)` that renders one screen per phase (`Login/Idle/StreamSetup/WaitingForStart/Racing/Finished`).

```
Rust backend emits a Tauri event
  → AppEventBridge (src/store/AppEventBridge.tsx) catches it
    → dispatch(action)
      → appReducer (src/store/appReducer.ts) → new AppState
        → React re-renders the matching screen
```

User-initiated actions go the other way: a screen calls a hook from `useActions` → which calls a typed wrapper in `lib/commands.ts` → `invoke("command_name")` → a `#[tauri::command]` in Rust.

## Frontend state (`src/store/`)

- **`types.ts`** — `Phase`, the `AppState` discriminated union (each phase carries exactly the data it needs, e.g. `RaceInProgress` has `lobby`, `raceStartAt`, `streamStatus`), `ActionType`, and the `AppAction` union. Add new state by extending these three.
- **`appReducer.ts`** — pure `(state, action) => AppState`. **Guard every transition**: each `case` checks the current phase and returns `state` unchanged if the transition is invalid (e.g. `StreamReady` only applies in `StreamSetup` and only when `streamStatus === live`). This is the single source of transition rules.
- **`AppContext.tsx`** — `AppProvider` holds the reducer. Exposes `useAppState()`, `useAppDispatch()`. On mount it **hydrates** from Rust via `getCurrentUser()` + `getLobbyState()`. (No `whipRef` — Rust owns the ffmpeg stream now.)
- **`AppEventBridge.tsx`** — the only place that subscribes to Rust events; translates each into a `dispatch(...)` (including `onStreamStatus` → `StreamStatusChanged`). Renders `null`. Stream teardown lives in Rust, not here.
- **`useActions.ts`** — the imperative API screens use (`login`, `logout`, `streamReady`, `stopStream`, `finish`, `forfeit`, `newRace`). Each calls a command wrapper and dispatches the local action. Wrapped in `useMemo`.

Components read `useAppState()` for data and call `useActions()` for behavior — they don't `invoke` or `listen` directly.

## The IPC contract (keep both sides in lockstep)

**Event names are duplicated string constants** that MUST match:

- Rust: `src-tauri/src/events.rs` (`pub const WS_LOBBY_SETUP: &str = "ws:lobby_setup";` …)
- TS: `src/lib/events.ts` (same strings)

Adding/changing an event means editing **both** files plus `lib/listeners.ts` (typed `onX` subscriber) and `AppEventBridge.tsx` (dispatch). Listeners use `safeListen` (a wrapper that handles the async `listen()` returning an unlisten fn, guarding against unmount races) — reuse it, don't call `listen` raw.

**Commands** are the inverse: a `#[tauri::command] pub async fn` registered in `src-tauri/src/lib.rs`'s `invoke_handler![…]`, with a typed wrapper in `src/lib/commands.ts` calling `invoke("snake_case_name", { camelCaseArgs })`. Note: command **args are camelCase** on the JS side (`{ lobbyId }`) and snake_case in Rust (`lobby_id: String`) — Tauri converts automatically.

To add a command: write the Rust fn → register it in `lib.rs` invoke_handler → add the wrapper in `commands.ts` → call it from `useActions`.

## Rust backend (`src-tauri/src/`)

```
lib.rs              run(): builds tauri::Builder, registers plugins, .manage(SharedState),
                    invoke_handler![…], setup() (deep-link handler + restore_session)
state.rs            GlobalState { app_state, user, ws_status, lobby, race_start_at, …loop flags }
                    SharedState = Arc<Mutex<GlobalState>>  — injected via State<'_, SharedState>
events.rs           event-name constants (mirror of lib/events.ts)
commands/           auth_commands, lobby_commands, stream_commands (re-exported via mod.rs glob)
api/                client.rs (ApiClient → AuthenticatedClient, Bearer token), lobby.rs (calls)
auth/               oauth.rs (deep-link callback), refresh.rs (token refresh loop), token_store.rs
ws/                 client.rs (persistent WS), handler.rs (dispatch inbound), messages.rs (ServerMessage)
models/             app_state.rs, auth.rs, lobby.rs — serde types shared with the frontend
lifecycle.rs        restore_session() on startup
config.rs           api_url(), AUTH_CALLBACK_PREFIX, endpoints
```

### Command pattern

```rust
#[tauri::command]
pub async fn send_stream_ready(
    lobby_id: String,
    state: State<'_, SharedState>,
    app: AppHandle,
) -> Result<(), String> {
    api::lobby::post_stream_ready(&app, &lobby_id).await?;   // HTTP to speedrace-back
    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;  // lock briefly
        guard.app_state = AppState::WaitingForStart;               // mutate global state
    }                                                              // drop guard before await/emit
    Ok(())
}
```

Rules followed throughout:

- Commands return `Result<T, String>` (errors stringified for JS).
- **Hold the `Mutex` guard for the shortest possible scope** — copy what you need out, then drop the lock before any `.await` or `app.emit`. Never hold it across an await.
- After changing `GlobalState`, emit the corresponding event so the frontend's reducer updates: `app.emit(APP_STATE, AppState::Idle)`. Use the constant from `events.rs`, never a literal.
- HTTP goes through `ApiClient::new(&app).authenticated()` (attaches the stored Bearer token); base URLs via `config::api_url(path)`.

### WebSocket inbound

`ws/messages.rs` defines `ServerMessage` as `#[serde(tag = "type", rename_all = "snake_case")]` — inbound frames from speedrace-back are matched by their `type` field (`lobby_setup`, `lobby_start`, `lobby_closed`, `player_result`, `ping`). Handling lives in `ws/handler.rs`, which mutates `SharedState` and emits the matching `ws:*` event to the frontend. A new server message = a new `ServerMessage` variant + handler arm + (usually) a new emitted event mirrored on both sides.

## Plugins & platform features (registered in `lib.rs`)

`single_instance` (focus existing window), `opener`, `deep_link` (OAuth callback via `speedrace://` scheme — registered in DEV in `setup()`), `store` (persisted token storage), `updater` (+ `UpdateChecker`/`UpdateModal` on the frontend, see `lib/updater.ts`), `process`. OAuth: `open_login` opens the browser; the deep-link callback is handled by `auth::oauth::handle_callback`. Session is restored on startup via `lifecycle::restore_session`.

## Streaming (local preview → Publish → WHIP + MP4, self-WHEP after)

**Rust owns the stream — and the preview.** Nothing is published or recorded until the racer clicks **Publish**. On StreamSetup, Rust auto-starts a **local preview** (`stream/preview.rs`: a preview-mode ffmpeg → mpjpeg on stdout → base64 `stream:preview` events, rendered imperatively by `components/ui/PreviewCanvas.tsx` — never through React state). Clicking the preview opens `SourcePicker.tsx` (monitors + windows, lazy thumbnails); the source is a tagged `CaptureSource` (`monitor`/`window`, const-object mirror in `src/types/`). Capture feeds **one BGRA rawvideo pipe** (`wgc.rs::start_capture_for` → `CaptureHandle`): a windowed pick uses WGC window capture; a **fullscreen game** that WGC captures as black escalates to **OBS-hook injection** (`gamecapture/`, window-only — a window pick never reads the screen); a Monitor pick uses WGC display or in-ffmpeg `ddagrab`. `publish_stream(lobbyId)` is **one Rust transaction**: kill preview → capture + cpal WASAPI audio pipe → ffmpeg `-f whip` to MediaMTX (+ segmented MP4 replay when applicable) → await live (25 s timeout) → POST stream-ready → `WaitingForStart`; failure restores the preview and POSTs nothing. Requires the bundled **ffmpeg 8.x** sidecar with a real DTLS backend (GnuTLS/OpenSSL/mbedTLS — **not** SChannel) **and** the OBS `win-capture` binaries — run `scripts/get-ffmpeg.ps1` + `scripts/get-game-capture.ps1`. Deep dive: the **momentum-stream** skill + `docs/SPEC_FFMPEG.md`. The supervisor emits `stream:status`; the FSM tracks it as `streamStatus`. Commands: `publish_stream`, `stop_stream`, `restart_preview`, `get`/`set_capture_source`, `list_monitors`, `list_windows`, `capture_monitor_thumb`, `capture_window_thumb`, `get`/`set_stream_settings`.

After publish the webview **plays the racer's own stream back via WHEP** (`src/stream/whep.ts`, `components/ui/WhepPreview.tsx`). Teardown is Rust-owned via the single `stream::shutdown` choke point (it kills preview + live) called from logout, `stop_stream`, forfeit, finish, WS `PlayerResult`/`LobbyClosed`, auth-lost/banned and app exit; `ServerUnavailable` keeps the stream alive, and mid-race ffmpeg death never POSTs stream-stopped (the back would forfeit). All stream data types live in `stream/types.rs`. Background/design: `docs/SPEC_FFMPEG.md`.

## i18n

`react-i18next`; namespaces under `src/locales/<lang>/<ns>.json` (`app`, `common`, `settings`), wired in `src/i18n/`. Use `useTranslation('<ns>')` in components.

## Adding things — quick checklists

**New phase/transition:** extend `Phase` + `AppState` + `AppAction` in `types.ts` → add a guarded `case` in `appReducer.ts` → add a screen + `App.tsx` switch arm → mirror the phase in Rust `models/app_state.rs` if the backend tracks it.

**New command (JS → Rust):** `#[tauri::command]` fn → register in `lib.rs` invoke_handler → wrapper in `lib/commands.ts` → expose via `useActions`.

**New event (Rust → JS):** add the const in `events.rs` **and** `lib/events.ts` → emit it from Rust after a state change → add `onX` in `lib/listeners.ts` → dispatch it in `AppEventBridge.tsx` → handle the action in `appReducer.ts`.

## Anti-patterns (never)

- `listen`/`invoke` directly inside a screen component → go through `listeners.ts`/`commands.ts` + the store hooks.
- Holding the `SharedState` mutex across an `.await` or `emit`.
- Hardcoding an event-name string → use the `events.rs` / `lib/events.ts` constant (and keep both in sync).
- Unguarded reducer transitions → every `case` must validate the current phase.
- Letting TS `Phase`/`AppState` and Rust `AppState` drift out of sync.

- Avoid useEffect for state: Use TanStack Query for server state, React state/hooks for local state.
- Avoid useEffect as much as possible: Only useEffect when synchronizing with an external system. Use patterns like callback refs, component keys, and other techniques to avoid unnecessary re-renders.
