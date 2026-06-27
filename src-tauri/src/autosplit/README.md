# Autosplitter

Drives split/finish events for a race from the runner's timer. Two independent sources run **in
parallel** during a lobby; exactly one is locked in at race start and used for the whole race.

## Sources

| Source | "Connected" means | Fires splits via |
| --- | --- | --- |
| **WASM** (`wasm.rs`) | the LiveSplit auto-splitter `.wasm` (per game) is **attached to the game process** | `timer.rs` `MomentumTimer::split()` |
| **LiveSplit TCP** (`tcp.rs`) | a TCP connection to LiveSplit Server (`127.0.0.1:16834`) is open | `poll_loop` diffing `getsplitindex` |

WASM is preferred. LiveSplit is the fallback when there is no `.wasm`, it won't compile, or it
can't attach to the game.

## Lifecycle

1. **LobbySetup** -> `handler.rs::init_lobby_resources`: load the `.lss` split file, then start
   **both** supervisors (WASM only if a `.wasm` was fetched).
2. Both run through setup/waiting. Each maintains its own connection flag in `GlobalState`
   (`wasm_attached`, `livesplit_connected`) and reports via `report_autosplit_state`.
3. **Race start** (`race_start_at` reached): `maybe_commit_source` locks `autosplit_source` to
   `Wasm` if attached, else `LiveSplit` if connected. **Sticky** for the race.
4. The loser stops; only the committed source fires splits.

## The one invariant

**Exactly one source fires per race, chosen once, never swapped mid-race.** WASM tracks position
via `current_split_index`/`split()`; LiveSplit via `getsplitindex`/`last_index`. Mixing them
double-fires or desyncs the index. Firing is gated:

- `timer.rs split()` fires only if `autosplit_source == Wasm`.
- `tcp.rs poll_loop` fires only if `autosplit_source == LiveSplit`.

## Recovery

- **WASM**: compiled once; on a `update()` trap (permanent for an instance, usually because the
  game isn't running yet) `supervise` re-instantiates cheaply so a late game launch attaches.
- **LiveSplit**: reconnects every `RECONNECT_DELAY_MS` after a drop; gives up only if it *never*
  connected by race start.

## Reporting & the back

`report_autosplit_state` does two things:

- Emits the `autosplit:probe` Tauri event `{ wasm: bool, livesplit: bool, splits_match: bool? }` ->
  UI badges (`BadgeHelper.tsx`: lucide `MonitorCheck` for WASM, `assets/livesplit.svg` for LiveSplit,
  red `TriangleAlert` when `splits_match == false`).
- POSTs `{ connected, splits_valid }` to the back (deduped on the pair). Pre-commit `connected` =
  either source ready (so a ranked player can ready up); post-commit `connected` = the committed
  source's health. `splits_valid` is false only on a confirmed LiveSplit split-set mismatch.

The back is source-agnostic. It uses `connected` for the ranked-ready gate and, in a ranked race,
auto-forfeits the player if `connected` stays false past a grace window (`AUTOSPLIT_FORFEIT_GRACE_SECS`).
A `splits_valid == false` (LiveSplit source loaded the wrong split set) forfeits a ranked in-progress
player **immediately**. See **`LIVESPLIT_SPLIT_VERIFICATION.md`** for the split-check, the `starttimer`
workaround, and how to migrate to pre-start verification once a LiveSplit release adds the commands.

`report_autosplit_state` fires only on a connection *change*, so on **startup restore** (app launches
already in a lobby) the probe event can fire before the UI subscribes. To cover that, `get_lobby_state`
(the hydration command) returns the current `{ wasm, livesplit }` in `ClientState`, and the frontend
seeds the badge from it; the reducer then carries `autosplit` across in-lobby phase transitions.

## Files

- `mod.rs` - module exports
- `wasm.rs` - WASM compile + supervise (re-instantiate on trap), attach detection
- `tcp.rs` - LiveSplit TCP connect + poll loop, split diffing
- `timer.rs` - `MomentumTimer` bridging the WASM runtime to our race clock/state
- `split.rs` - `fire_split`: records a split, posts to the back, handles the final split/finish
