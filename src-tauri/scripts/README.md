# FFmpeg build — quick README

This folder contains `build-ffmpeg.ps1`, a one-shot builder that produces a minimal FFmpeg sidecar for the Momentum Tauri app.

This short README explains how to add the generated binary to the repo and the recommended workflow.

What you get
- `src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe` (expected ~15–20 MB stripped)
- Optional UPX-compressed variant: ~8–12 MB (smaller, may be flagged by AV)

Recommended repository options

1) Git LFS (recommended)

- Pros: avoids bloating `.git` history, transparent for collaborators.
- Cons: needs LFS enabled for each dev/machine.

Commands to set up and commit the binary:

```bash
# once per machine
git lfs install

# in repo (track the ffmpeg binary)
git lfs track "src-tauri/binaries/ffmpeg-*.exe"

# commit attributes and binary
git add .gitattributes
git add src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe
git commit -m "feat: add ffmpeg sidecar (LFS)"
git push origin main
```

2) Commit directly (only if < ~20MB and you don't rebuild often)

- If you choose to commit the stripped binary directly (no LFS), it's acceptable for a single ~15–20MB file. Add it to `.gitignore` later if you want to stop tracking rebuilds.

3) Ignore binary (rebuild on demand)

- Add to `.gitignore`:

```gitignore
src-tauri/binaries/ffmpeg-*.exe
```

- Useful if you prefer each developer to build locally, but requires build time and MSYS2 setup.

UPX note

- If `upx` is installed and used by the script, the binary will be ~8–12MB.
- UPX reduces download size but may increase startup time slightly and sometimes trigger AV false positives.
- If you plan to store the UPX version in the repo, consider using Git LFS as well.

Tauri integration reminder

- `tauri.conf.json` should include an `externalBin` mapping under `bundle`. Example:

```json
"bundle": {
  "externalBin": ["binaries/ffmpeg"]
}
```

- The builder copies `ffmpeg-x86_64-pc-windows-msvc.exe` to `src-tauri/binaries`.
- Tauri resolves the correct platform binary name automatically at runtime.

Which to choose?

- Default: use Git LFS. It's simple, safe, and keeps the repo small.
- If you prefer not to install LFS and the binary is UPX-compressed (~8–12MB), you can commit it directly.

Questions or want me to add a `Makefile`/npm script to automate the commit/push step? Ask and I can add it.
