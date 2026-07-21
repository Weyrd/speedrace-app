---
name: speedrace-autosplitter
description: How the speedrace-app racer client drives splits/finish from a runner's timer — the WASM + LiveSplit dual-source autosplitter, the sticky source arbiter, the connection/forfeit contract with the back, and the UI badges. Use this whenever you touch anything autosplit-related: LiveSplit TCP, the WASM auto-splitter (.wasm), split/finish firing, the `autosplit:probe` event or connection badges, the ranked auto-forfeit-on-disconnect, `src-tauri/src/autosplit/`, `SpeedraceTimer`, or `maybe_commit_source` — and especially before adding a new timer source or changing when/how splits fire, because the "one source per race, never swapped" invariant is easy to break and corrupts ranked results if you do.
---

# Speedrace Autosplitter

The racer client turns a runner's timer into split/finish events for the race. Two sources exist
and **run in parallel**; one is chosen per race. Getting the arbitration wrong double-fires splits
or desyncs the index, which corrupts results — so read this before changing split-firing logic.

**In-tree spec (source of truth, keep it in sync):** `src-tauri/src/autosplit/README.md`.

## The two sources

| Source | File | "Connected" means | Fires splits via |
| --- | --- | --- | --- |
| **WASM** | `wasm.rs` | the per-game `.wasm` auto-splitter is **attached to the game process** (`attached_processes()`) | `timer.rs` `SpeedraceTimer::split()` |
| **LiveSplit TCP** | `tcp.rs` | a socket to LiveSplit Server (`127.0.0.1:16834`) is open | `poll_loop` diffing `getsplitindex` |

WASM is preferred; LiveSplit is the fallback (no `.wasm`, won't compile, or can't attach).

## The invariant (do not break)

**Exactly one source fires per race, committed once at race start, never swapped mid-race.**

The two sources track position with incompatible models — WASM increments `current_split_index`
as the game advances; LiveSplit diffs an absolute index from the server. If both fire, or you hand
off mid-race, you double-fire or desync. So firing is gated on the committed source:

- `SpeedraceTimer::split()` fires only if `autosplit_source == Wasm`.
- `tcp.rs poll_loop` fires only if `autosplit_source == LiveSplit`.

`autosplit_source: Option<AutosplitSource>` lives in `state.rs` and is **sticky** for the race.

## Lifecycle (ws/handler.rs)

1. **LobbySetup** → `init_lobby_resources`: load the `.lss` split file, then start **both**
   supervisors (WASM only if a `.wasm` was fetched). Resets `autosplit_source`, `wasm_attached`,
   `livesplit_connected`.
2. Both run through setup/waiting. Each maintains its own flag (`wasm_attached`,
   `livesplit_connected`) and calls `report_autosplit_state`.
3. **Race start** (`race_start_at` reached): `maybe_commit_source` locks the source — `Wasm` if
   attached, else `LiveSplit` if connected. The loser stops (`wasm_won` / source check).
4. `start_autosplitter` (on LobbyStart) is an idempotent safety net for lobbies with no split file.

Both supervisors loop while `in_lobby(&state)` and `!autosplitter_cancel`.

## Auto-select the game window on attach

On the WASM **false→true attach transition** (`wasm.rs::maybe_switch_source`), the supervisor
auto-selects the game's window as the **capture source** — the attach gives us the game's exact
**PID** (`attached_pid` from `Process::pid()`), so the match is precise. It spawns
`stream::auto_select_game_window(app, state, pid)` (off the tick loop, so a preview restart doesn't
stall autosplitting), which: finds the game window via `stream::window_list::game_window_for_pid`
(largest visible top-level of that PID + its title), sets `capture_source = Window { hwnd, title }`,
restarts the preview, and emits **`stream:source`** so the frontend updates the `["captureSource"]`
query cache (`AppEventBridge` → `qc.setQueryData`) — keeping the source label/picker in sync.
**Gated on `state.stream.is_none()`** — fires only *before* the racer has published; never touches
a live stream and never triggers a publish. No-ops if that window is already the source (absorbs the
re-attach double-fire). Non-windows is a no-op. `stream:source` is a both-sides constant
(`events.rs` + `lib/events.ts`).

## Recovery (why the supervisors look the way they do)

- **WASM**: compiled **once**; `supervise` re-instantiates cheaply on an `update()` **trap**. A
  trap is permanent for an instance and usually means the game isn't running yet — re-instantiating
  lets a late game launch attach. Don't "fix" this by recompiling per tick.
- **LiveSplit**: reconnects every `RECONNECT_DELAY_MS` after a drop, but gives up if it *never*
  connected by race start (a timer appearing mid-race is useless and would desync).

## Contract with the back (the back is source-agnostic)

`report_autosplit_state` does two things:

- Emits the `autosplit:probe` Tauri event `{ wasm: bool, livesplit: bool }`.
- POSTs **one** `connected` bool to the back (deduped via `last_autosplit_reported`).
  - **Pre-commit**: `connected = wasm_attached || livesplit_connected` — so a ranked player can
    ready up if *either* source works.
  - **Post-commit**: `connected` tracks **only the committed source's health** — so losing it
    triggers the forfeit.

Back side (`speedrace-back`): `lobby_service/stream.rs::set_autosplit_connected` records the bool and,
in a ranked in-progress race, schedules `spawn_autosplit_forfeit_guard`, which forfeits the player
if still disconnected after `AUTOSPLIT_FORFEIT_GRACE_SECS` (in `constants.rs`). A server-initiated
forfeit must also `send_to_app(AppEvent::PlayerResult)` or the still-connected app stays on Racing.

## Frontend (badges)

- `lib/events.ts` `AUTOSPLIT_PROBE` ↔ `lib/listeners.ts` `onAutosplitProbe` (payload `AutosplitState`
  `{ wasm, livesplit }` in `types/index.ts`).
- `AppEventBridge` dispatches `ActionType.AutosplitStatus`; the reducer stores `state.autosplit` in
  StreamSetup / WaitingForStart / RaceInProgress (carried across `LobbyStart`).
- `components/ui/BadgeHelper.tsx` renders one badge per connected source: lucide `MonitorCheck`
  (WASM) and `assets/livesplit.svg` (LiveSplit). Both show if both connected; each hides when its
  source drops.

## Extending safely — checklists

**Add a third timer source:** add an `AutosplitSource` variant; start its supervisor in parallel in
`init_lobby_resources`; have it set a `*_connected` flag + call `report_autosplit_state`; teach
`maybe_commit_source` the priority order; gate its firing on `autosplit_source == <yours>`; make the
losers stop when another source is committed. Never let two fire at once.

**Change the connection signal:** keep "connected" meaning *usable right now* (WASM=attached,
LiveSplit=socket open). The ranked gate and forfeit both ride on this single bool — widening it
(e.g. "module loaded" for WASM) weakens the ranked guarantee.

**Add a new probe/event field:** event names are duplicated constants — edit `events.rs` **and**
`lib/events.ts`, then `listeners.ts` + `AppEventBridge` + the reducer (see the `speedrace-app` skill's
IPC checklist).

## Anti-patterns (never)

- Letting WASM and LiveSplit both fire, or swapping source mid-race.
- Holding the `SharedState` mutex or the WASM `ExecutionGuard` (which is `!Send`) across an `.await`.
- Reporting WASM "connected" on compile success instead of game attachment.
- Recompiling the `.wasm` on every retry instead of re-instantiating the cached module.
