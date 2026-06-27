# LiveSplit split verification (+ the `starttimer` workaround)

When the **LiveSplit** source drives a race (no WASM auto-splitter for the game), the runner could
have a **different split file loaded** than the one the race expects (e.g. Level 2's splits while
racing Level 1). LiveSplit's split index would then map onto the wrong segments — wrong segment
names recorded, wrong finish point. This feature **detects that and refuses to record the run**
(and forfeits it in ranked).

It only concerns the **LiveSplit** source. The WASM source fires against our own `split_run`, so its
segment names are ours by construction and can never mismatch.

---

## Why this is hard: the LiveSplit Server protocol

We talk to the LiveSplit Server (TCP line protocol on `127.0.0.1:16834`). Verified against the real
server **and** the source (`LiveSplit/LiveSplit` `src/LiveSplit.Core/Server/CommandServer.cs`):

- There is **no command** in any **released** LiveSplit (≤ **1.8.37**, the latest release) to read the
  loaded split **list**, **count**, **game/category name**, or file name.
- The only commands that reveal split *identity* — `getcurrentsplitname`, `getprevioussplitname`,
  `getsplitindex` — return `-` / `-1` while the timer is **`NotRunning`**. They only become useful
  once the timer is **`Running`**.

So: **we cannot verify the splits before the race starts.** The earliest we can read the runner's
current split name is the moment their timer starts.

### The aggravating case (why `starttimer` exists)

If the runner uses LiveSplit's own auto-start bound to the *wrong* game/level, starting the *right*
level never trips it → the timer stays `NotRunning` forever → `getcurrentsplitname` stays `-` → the
comparison can never run. This is exactly the case we most need to catch.

**Workaround:** when LiveSplit is the committed source and the timer is still `NotRunning` at race
time, the app sends **`starttimer`** once (`tcp.rs` `poll_loop`, guarded by `forced_start`). The
LiveSplit timer goes `Running`, `getsplitindex` → `0`, `getcurrentsplitname` → the first segment of
the *loaded* (wrong) split file → the comparison fires and flags the mismatch.

`starttimer` is a no-op if the timer is already running (LiveSplit's `Model.Start()` guards on
`NotRunning`) and sends no reply, so we don't read one. It does **not** affect the times momentum
records (those are computed from `race_start_at`, not LiveSplit's clock); it only starts the runner's
*displayed* LiveSplit timer at the race go instead of at their own trigger.

---

## What was implemented

### App — Rust (`momentum-app/src-tauri/`)

- **State** (`state.rs`): `livesplit_splits_match: Option<bool>` — `None` until checked, `Some(false)`
  when the running timer's current split name differs from `split_run`. Reset to `None` in the
  `LobbySetup` / `LobbyClosed` handlers (`ws/handler.rs`).
- **Detection** (`autosplit/tcp.rs` `poll_loop`): once per index change (and once the timer is
  running), query `getcurrentsplitname`, normalize (`trim` + `eq_ignore_ascii_case`), compare to
  `split_run.segment(index).name()`, store the verdict, and `report_autosplit_state` on change.
- **`starttimer` force-start** (`autosplit/tcp.rs` `poll_loop`, `forced_start` flag): see above.
- **Don't record wrong splits** (`autosplit/split.rs` `fire_split`): early-return when
  `autosplit_source == LiveSplit && livesplit_splits_match == Some(false)` → no `post_player_split`,
  no `post_player_finished`. Applies in **all** modes (ranked and casual).
- **Reporting** (`ws/handler.rs` `report_autosplit_state` / `report_autosplit`): the
  `autosplit:probe` payload gained `splits_match: Option<bool>`; the back POST (`autosplit-status`)
  gained `splits_valid: bool` (`= false` only on a confirmed LiveSplit mismatch; always `true` for the
  WASM source). Dedup key is now the `(connected, splits_valid)` tuple (`last_autosplit_reported`).
  `api/lobby.rs` `post_autosplit_status` carries `splits_valid`.

### App — frontend (`momentum-app/src/`)

- `types/index.ts` `AutosplitState` gained `splits_match?: boolean | null`.
- `store/appReducer.ts` `AutosplitStatus` stores it (and no-ops when unchanged).
- `components/ui/BadgeHelper.tsx` shows a red `TriangleAlert` badge when `splits_match === false`.

### Back (`momentum-back/`)

- `dto/lobby.rs` `AutosplitStatusDto` gained `splits_valid: Option<bool>` (absent = treated as valid,
  for older app clients).
- `handlers/lobby/racing.rs` passes `dto.splits_valid.unwrap_or(true)` to the service.
- `services/lobby/lobby_service/stream.rs` `set_autosplit_connected(..., splits_valid)`: in a **ranked
  in-progress** race, `splits_valid == false` forfeits the player **immediately** (no grace window —
  unlike a dropped connection, wrong splits never recover), broadcasts `RaceForfeited`, and
  `send_to_app(AppEvent::PlayerResult)` so the still-connected app leaves Racing.

---

## Migrating to the clean pre-start verification (when a release ships the new commands)

`master` of `LiveSplit/LiveSplit` already has commands that read the loaded run **regardless of timer
phase** — they just aren't in any release yet (not in 1.8.37):

- `getsplitcount` → `State.Run.Count`
- `getsplitname <index>` → `State.Run[index].Name` (supports negative index)
- `getgamename`, `getcategoryname`, `getcategoryvariables`

Unknown commands are **silently ignored** by the internal server (no reply → client read timeout), so
capability is detectable at runtime.

When these land in a LiveSplit **release**, prefer pre-start verification and **retire the
`starttimer` workaround**:

1. **Capability probe** at `init_lobby_resources` (or on first connect, timer still stopped): send
   `getsplitcount` with a short read timeout (~500 ms).
   - **Integer reply** → new path. Compare `getsplitcount` to `split_run.len()`, then `getsplitname 0..n`
     to each `split_run.segment(i).name()` (optionally `getgamename`/`getcategoryname` against the
     lobby's game/category). Set `livesplit_splits_match` **before the race even starts**, so the badge
     and the `splits_valid` POST happen pre-start — the back can then block ready-up / forfeit early.
   - **Timeout** → keep the current fallback (running-timer comparison).
2. **Remove the `starttimer` force-start** (`tcp.rs` `poll_loop`: the `forced_start` block and the
   `forced_start` variable). With pre-start verification it is no longer needed, and not touching the
   runner's timer is preferable.
3. Keep the running-timer `getcurrentsplitname` comparison as a secondary check (cheap, catches a run
   that diverges only mid-list), or drop it if the pre-start check is exhaustive.
4. The back contract (`splits_valid`) is unchanged — only **when** the app discovers the mismatch
   moves earlier. If you want a pre-start *ready-up block* (not just an in-progress forfeit), extend
   the back to also gate web-ready on `splits_valid` (mirror the `connected` ranked-ready gate in
   `set_autosplit_connected`).

Until then, the `starttimer` workaround is the only thing that covers the "timer never starts" case on
shipped LiveSplit versions.
