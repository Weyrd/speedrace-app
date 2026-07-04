---
name: player-counters
description: How the momentum-app racer client EMITS per-game "player counters" (deaths / dashes / money the wasm autosplitter exposes) to the back — the config-driven drop/buffer/post decision in `set_variable`, the mode-shaped per-counter buffers in `SharedState`, the cadence-driven flush routine and its finish/forfeit "flush before archive" ordering rule, the `fetch_counter_config` updated-at cache, and the batch `POST /lobby/{id}/counter`. Read this BEFORE touching anything counter-related in momentum-app — `src-tauri/src/counter.rs`, `set_variable` in `autosplit/timer.rs`, the flush call sites, `api/counter_config.rs`, or `post_player_counter` — because dropping/buffering by the wrong cadence, or forgetting to flush before a finish/forfeit POST, silently loses a player's counter data. Counters are Rust-only (no React/IPC). The storage half lives in momentum-back and the display half in momentum-web, each with its own sibling `player-counters` skill.
---

# Player Counters — momentum-app (emit side)

> This is the **app** skill. Counters are purely `src-tauri` Rust — no React, no IPC.
> The back **stores** them (`momentum-back/.claude/skills/player-counters/`); the web
> **displays** them (`momentum-web/.claude/skills/player-counters/`). Shared canonical
> spec: `momentum-back/TEST & DOCS/SPEC_Player_Counters.md`. For the timer/wasm side that
> *produces* the values, see the `momentum-autosplitter` skill.

## The axes the app cares about

A counter is configured per-game on three axes. The app only acts on two of them:

| Axis | Field | App's job |
| --- | --- | --- |
| **mode** | `mode` (`Total`/`PerSplit`/`Timeline`) | shapes **what the buffer keeps** |
| **cadence** | `cadence` (`Instant`/`PerSplit`/`EndOnly`) | decides **when to flush + POST** |
| display_targets | — | back/web only; app ignores it |

**Cardinal rule:** every sample is the **absolute cumulative value** the wasm reported,
never a `+1`. The back derives deltas. Don't try to diff on the app side.

Wire contract (must match the back exactly): the enums serialize **PascalCase**
(`CounterMode { Total, PerSplit, Timeline }`, `CounterCadence { Instant, PerSplit,
EndOnly }`), and the ingest field is **`at_ms`** — plain serde enums with **no
`#[serde(rename_all)]`** in `api/counter_config.rs` ("to match the back's PascalCase
wire format").

## The pipeline

```
wasm set_variable(key=value)
  → timer.rs: build CounterSample { value, split_index, at_ms }
  → resolve_action(config) → Drop | Buffer(mode) | Post
      Post   → post_player_counter now (Instant)
      Buffer → counter_buffers[name].record(sample)   (PerSplit / EndOnly / unknown)
  → flush trigger → flush_counter_buffers → one batch POST per counter
```

### 1. Emit point — `autosplit/timer.rs::set_variable`

Parses the value to `i64`, computes `at_ms` relative to `race_start_at` (via
`clock_offset_ms`), builds a `CounterSample`, looks up the counter's config by name in
the in-hand snapshot, and dispatches through `resolve_action`:

```rust
match crate::counter::resolve_action(cfg.as_ref()) {
    CounterAction::Drop        => None,                       // disabled → dropped
    CounterAction::Buffer(mode)=> { buffers.entry(name)…record(sample); None }
    CounterAction::Post        => Some((lobby_id, sample)),   // only Instant POSTs here
}
```

Bails early if there's no `race_start_at` or no active `lobby`. Only the `Post` branch
spawns `api::lobby::post_player_counter`.

### 2. Config fetch — `api/counter_config.rs::fetch_counter_config`

`GET /api/v1/games/{game_id}/counters`, **cached by `updated_at`** against a local file
(same pattern as `fetch_game_autosplitter`): if the cached stamp equals the payload's
`counter_config_updated_at`, deserialize the cached `Vec<CounterConfig>`; else GET and
rewrite the cache. Returns `None` when the stamp is `None`. Called once at race start in
`ws/handler.rs::init_lobby_resources`, stored in `SharedState.counter_config:
Option<Vec<CounterConfig>>` (`state.rs`). The stamp arrives on the lobby setup payload
(`LobbyCurrentResponse.counter_config_updated_at` → `LobbySetup`).

`CounterConfig` (app-side model) = `{ counter_name, enabled, mode, cadence, label, icon,
display_order }` — a subset of the back's `GameCounterConfig` (no ids/timestamps/targets).

### 3. Buffers — `counter.rs`

Per-counter buffers in `SharedState.counter_buffers: HashMap<String, CounterBuffer>`
(cleared on lobby setup **and** lobby close). The buffer is **mode-shaped**:

| `CounterBuffer` variant | Keeps | For mode |
| --- | --- | --- |
| `Total(Option<Sample>)` | latest sample wins | Total |
| `PerSplit { per_split: BTreeMap<u32,Sample>, no_split }` | latest per split; null split → latest | PerSplit |
| `Timeline(Vec<Sample>)` | every sample appended | Timeline |

`record()` updates the variant; `drain()` empties it into a `Vec<CounterSample>` for one
batch POST. `resolve_action` (the dispatch brain):

```rust
None                         => Buffer(Total)   // unknown counter → buffered, one POST at finish
Some(c) if !c.enabled        => Drop
Some(c) if c.cadence==Instant=> Post
Some(c)                      => Buffer(c.mode)
```

An **unknown** counter (no config yet) is buffered as `Total` and flushed at finish — it
never floods, matching the back's auto-discovery default (`Total` / `EndOnly`).

### 4. Flush — `counter.rs::flush_counter_buffers`

Drains each buffer (optionally filtered to a single cadence) and POSTs one batch per
counter. `flush_all_counter_buffers` = flush everything. **Cadence is event-driven — there
is no interval/timer flush.** Triggers:

| Cadence | Trigger site |
| --- | --- |
| `Instant` | POSTed immediately in `set_variable` (never buffered) |
| `PerSplit` | on split advance — `autosplit/split.rs`, `flush_counter_buffers(…, Some(PerSplit))` |
| `EndOnly` / all | on final split (`split.rs`) and before finish/forfeit POSTs |

> ⚠️ **The finish/forfeit path MUST flush all buffers before the terminal POST**, or an
> `EndOnly` counter (and the last split of any buffered counter) is lost. Call sites:
> `commands/lobby_commands.rs` finish (before `post_player_finished`) and forfeit (before
> `post_player_forfeited`), plus the final-split flush in `split.rs`.
>
> ⚠️ **Server-initiated forfeit (WS `PlayerResult`) is inherently unflushable** — the app
> didn't initiate it, so a buffered (esp. `EndOnly`) counter loses its unflushed data
> there. Known limitation; surfaced in the back's data-loss note.

### 5. POST — `api/lobby.rs::post_player_counter`

`POST /api/v1/lobby/{lobby_id}/counter`, body `SubmitCounterBody { counter_name, samples:
Vec<CounterSample> }`, where `CounterSample { value: i64, split_index: Option<u32>,
at_ms: u64 }`. A single Instant event is just `samples.len() == 1`; a flushed buffer is
the whole batch in one request.

## Gotchas

- **Don't diff on the app side** — always send the absolute cumulative `value`.
- **Buffers are memory-only** — a crash loses the unflushed buffer. That's why `PerSplit`
  flushes each split boundary and `EndOnly` is reserved for cheap counters.
- **Snapshot, not live** — `counter_config` is fetched once at race start; an admin change
  mid-race has no effect until the next race (mirrors the back's snapshot).
- **Enums must stay PascalCase** — if the back renames a mode/cadence, this side breaks
  silently (serde just fails to match). Keep `api/counter_config.rs` in sync.
- **Tests:** `src-tauri/src/counter/tests.rs` covers drop/instant/buffer resolution,
  PerSplit latest-wins, and null-split-degrades-to-latest. Gate: `rtk cargo test` in
  `src-tauri/`.
