# Momentum App

Racer-side desktop client of the Momentum speedrun platform — Tauri 2, React 18 + Vite + Tailwind v4 frontend, Rust backend in `src-tauri/`. Product overview: @README.md.

Detailed conventions (IPC contract, Phase state machine, add-a-command/event checklists, anti-patterns) live in the `momentum-app` skill (`.claude/skills/momentum-app/SKILL.md`) — read it before touching `src/` or `src-tauri/`.

Sibling repos (local checkouts): `../momentum-back` (API + WebSocket server) and `../momentum-web` (web frontend). When an API or WS contract is in doubt, check the server side there instead of guessing.

## Commands

```bash
pnpm tauri dev      # full app (frontend + Rust backend)
pnpm dev            # Vite frontend only (port 1420)
pnpm build          # tsc + vite build (frontend type-check)
pnpm tauri build    # production bundle
```

Rust: `cargo check` / `cargo clippy` from `src-tauri/`. There is no test suite and no JS linter.

## Docs

- `docs/SPEC_FFMPEG.md` — the streaming spec (preview, Publish, WHIP, capture, replay) with the why behind each decision. Read it before touching `src/stream/`, `src-tauri/src/stream/` or stream commands.
- `src-tauri/scripts/README_FFMPEG_MINIMAL_BUILD.md` — the bundled minimal FFmpeg sidecar: what it does, how it's built and integrated. `src-tauri/scripts/README.md` covers the build script workflow itself.

## Rules

- **Ask, don't guess.** If a request is ambiguous, state the possible interpretations and ask before coding. Push back when a simpler approach exists.
- **Minimum, surgical code.** Solve only what was asked — no speculative features or single-use abstractions. Touch only what you must; don't refactor or "improve" adjacent code.
- **Surface conflicts, don't average them.** If two existing patterns contradict, pick one (more recent / more used), say why, and flag the other for cleanup.
- **Fail loud.** Never report "done" if anything was skipped or unverified. Surface uncertainty instead of hiding it.
- **Never commit or push** unless explicitly asked.
- **Border radius: `rounded-sm` only.** Never `rounded-md` or `rounded-lg` (nor their per-corner variants). Keep corners consistently small.
- **Comments: short, meaningful, rare.** Use `//` only — never `///` or `—` or `;` doc-comments in
  Rust unless documenting a public API on purpose. One line when it explains _why_,
  not _what_. No multi-line comment blocks, no banners, no commenting the obvious.

## Do not touch without asking

- `.env` — never modify it or print its values.
- Updater/signing config in `src-tauri/tauri.conf.json` — a mistake here breaks auto-updates for existing users.
