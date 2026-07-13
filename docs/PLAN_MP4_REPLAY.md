# Plan — MP4 replay recording

Phase 2 of the streaming work (`README_STREAM_V2.md`). **Not yet implemented.** Goal: for
**ranked** races, automatically write a local MP4 VOD of the run to disk (evidence / the racer's
own replay) while they stream, without needing OBS.

---

## Locked decisions (from the user)

1. **Quality = match the stream (720p).** The replay reuses the same 1280×720 encode target as
   the WHIP output — one scale, minimal extra CPU. No separate high-res branch.
2. **Enabled automatically for ranked races only.** Casual races do not record in v1. There is no
   manual "record" toggle — recording is driven by the lobby's `race_type`. (Interpreted as
   VOD-evidence for ranked; revisit if a casual override is wanted.)
3. **User controls the storage folder** — see where replays are saved, open the folder, and
   change it.
4. **Auto-delete replays older than 1 week**, with a **settings toggle** to enable/disable that
   cleanup.

Because ranked-ness decides recording and recording starts at "Start stream" (StreamSetup, before
the race), the app must know `race_type` at lobby-setup time — which it does **not** today. That
makes Step 0 a real cross-repo change, same pattern as `whep_url` in Phase 1.

---

## Core mechanism: a second output on the *same* ffmpeg process

`pipeline::build_args` today ends with one output: `-f whip <url>`. Replay adds a **second output
file** to the same command — one ffmpeg process, two outputs. Deliberate: the WHIP stream must not
be disturbed by recording, and a single process shares one capture and keeps the existing
supervisor, graceful-`q` stop, and Job Object exactly as they are.

Not `tee`: the `tee` muxer needs identical codecs on every branch, but WHIP mandates **Opus** and
MP4 needs **AAC**. Different audio ⇒ two full output blocks, each with its own audio encoder.
Since quality matches the stream, video is encoded per-output at the same 720p target (a second
cheap x264 `veryfast` pass; note the ~2× encode cost — the reason NVENC is the natural follow-up).

Arg shape (replay block appended only when the race is ranked):

```
… <inputs, -map, -vf/-af as today, output 1 = WHIP unchanged, incl. -ts_buffer_size 4194304> …
# output 2 — MP4 replay (ranked only)
-map 0:v -map 1:a
-c:v libx264 -preset veryfast -profile:v high -pix_fmt yuv420p -g {2*fps} -r {fps} -b:v {kbps}k
-c:a aac -b:a 160k -ar 48000 -ac 2
-movflags +frag_keyframe+empty_moov  {out_path}.mp4
```

- **Crash safety via fragmented MP4** (`+frag_keyframe+empty_moov`): a hard-killed ffmpeg still
  leaves a playable file — no moov repair, no next-launch recovery code. Graceful `q` finalizes
  cleanly regardless. (Plays in VLC/YouTube; if an editor complains we switch to plain MP4 +
  `-c copy` remux on next launch.)
- Replay output drops `zerolatency`/`baseline` (those are live-latency tradeoffs) for `-profile:v
  high` + normal GOP — better VOD at the same size.

## Two single-process constraints to accept

1. **The replay spans the whole stream session** (Start → Stop), not just the race — trimming to
   the race would need restarting ffmpeg, which drops the live WHIP stream. The racer trims later;
   the run itself is always fully captured.
2. **A mid-race reconnect makes a new file.** The supervisor restarts ffmpeg up to 3× on mid-race
   death; each restart is a new process ⇒ a new segment. Suffix them `…_pt2.mp4` so both survive.

---

## Step 0 — momentum-back: expose `race_type` to the app (cross-repo)

The `Lobby` domain already has `race_type: RaceType` (`domain/lobby.rs`) — it's just not sent.

- `src/ws/messages.rs`: add `race_type: RaceType` to `AppEvent::LobbySetup` (~l.61).
- Fill it at every LobbySetup construction site (the lobby entity is in scope — mirror how
  `whep_url` was threaded). No Mongo change, no migration: it's an existing field, serialization-only.
- If a `LobbyCurrentDto` also feeds the app's hydrate path, add `race_type` there too.
- Gate: `CARGO_TARGET_DIR=target-gate cargo check` in momentum-back (dev server locks the exe).

App mirror:
- `src-tauri/src/models/lobby.rs` `LobbySetup` += `#[serde(default)] pub race_type: RaceType`
  (define a matching `RaceType` const-object-style enum on the app side; Rust enum + serde
  `rename_all="snake_case"`). Default = casual so an old back can't accidentally force recording.
- Frontend types + state lobby: add `race_type` so the UI can also badge "recording" if wanted.

## Step 1 — settings (`replay_dir`, `replay_autodelete`)

No `replay_enabled` — recording is decided by `race_type`. Settings hold only:

- `src-tauri/src/settings.rs`: `stream_replay_dir` (string; default = Windows **Videos** known
  folder + `\Momentum`), `stream_replay_autodelete` (bool, default the user's call — propose
  **on**). Move the growing `load/save_stream_settings` tuple to a small struct now that it's
  several fields. Add a `retention_days = 7` constant (fixed, not user-facing in v1).
- Thread through `StreamSettings` (mod.rs), `StreamSettingsDto` + get/set commands
  (`stream_commands.rs`), `src/types.ts`, `useStreamSettings.ts`.

## Step 2 — pipeline + per-race output path

- `src-tauri/src/stream/pipeline.rs`: `build_args` takes `replay_path: Option<&Path>` and appends
  output 2 only when `Some`. Quality matches the stream, so the existing single `-vf` chain feeds
  both outputs — no `-filter_complex` split needed.
- `src-tauri/src/stream/mod.rs::start`: compute the replay path only when `lobby.race_type ==
  Ranked` (and directory creatable); else `None`. Create `replay_dir` if missing; filename
  `momentum_{game}_{yyyymmdd-hhmmss}.mp4` (sanitize game name). Pass into `supervise` →
  `build_args`. On mid-race restart append the `_pt{n}` suffix.

## Step 3 — supervisor + reveal

- `src-tauri/src/stream/ffmpeg.rs::supervise` just forwards the replay path into `build_args` — no
  new failure branch (a replay-write error surfaces through the existing stderr tail + death path).
  Log the final path on stop.
- `reveal_replay(path)` command via `tauri_plugin_opener` (already a dep) — "Show in folder".

## Step 4 — auto-delete (retention sweep)

- On app startup (in `lib.rs` setup, spawned so it never blocks launch): if
  `stream_replay_autodelete` is on, scan `replay_dir` for `*.mp4` with mtime older than
  `retention_days` and delete them. Best-effort, log failures, never panic. A startup sweep is
  enough — no background timer needed for a desktop app.

## Step 5 — UI (settings + post-race)

- `src/components/SettingsPanel.tsx`: a "Replays" section — current folder path (read-only row) +
  **Open folder** (`reveal_replay` on the dir) + **Change folder** (needs a folder picker →
  **add `tauri-plugin-dialog`**, a new dep + capability; it's the standard Tauri way, low risk) +
  an **Auto-delete after 1 week** toggle (`stream_replay_autodelete`). Persist via
  `useSetStreamSettings`.
- `src/components/Finished.tsx` (race-finished screen): if a replay was written this session, show
  "Replay saved" + **Show in folder** (`reveal_replay`). Only for ranked; honest empty state otherwise.
- i18n (`src/locales/{en,fr}/settings.json` + `app.json`): `replay_title`, `replay_folder`,
  `replay_open_folder`, `replay_change_folder`, `replay_autodelete`, `replay_saved`,
  `replay_show_in_folder`. FR: "rediffusion" (confirm wording), infinitive buttons, no em-dashes,
  "race" not "course".

Gates: back `cargo check` (Step 0); app `cargo check` (Steps 1-4); `pnpm build` (Step 5).

---

## Failure drills

- Ranked race → stream → race → stop: one playable 720p MP4 spanning the run in `replay_dir`.
- Casual race: **no** file written; WHIP unaffected (drives the race_type gate).
- Hard-kill ffmpeg.exe mid-record: the fragmented MP4 left behind still plays.
- Mid-race reconnect: two segments (`…`, `…_pt2`), both playable.
- Change folder → next ranked run writes there. Open folder reveals it.
- Auto-delete on: a >7-day-old `.mp4` is gone after next launch; a fresh one survives.
  Auto-delete off: nothing deleted.
- Disk-full / bad `replay_dir`: WHIP survives; replay error logged, not fatal.
- Old back (no `race_type`): app defaults to casual ⇒ no recording (fails safe, never force-records).

## Sequencing

Build in order with gates: Step 0 (back + app mirror) → 1 (settings) → 2-3 (pipeline/supervisor)
→ 4 (retention) → 5 (UI). Pause after Step 3 to confirm a ranked run actually produces a playable
MP4 alongside a working stream before doing the retention sweep and settings UI.
