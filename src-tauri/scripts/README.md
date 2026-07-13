# FFmpeg sidecar

## Current: prebuilt Gyan build via `get-ffmpeg.ps1` (run once after clone)

The app streams via ffmpeg's native `-f whip` muxer, which requires **ffmpeg 8.x with a real
DTLS-SRTP backend** (GnuTLS, OpenSSL, or mbedTLS). We ship a prebuilt **Gyan GPL `full_build`
static win64** build (`--enable-gnutls`) as a Tauri sidecar instead of building our own.

> **Do not substitute a Windows SChannel build** (BtbN's default `win64-gpl` is SChannel-only).
> SChannel compiles the whip muxer but its DTLS handshake fails against MediaMTX at runtime
> (`SEC_E_ALGORITHM_MISMATCH 0x80090331` / "DTLS session failed"). Sanity-check any binary:
> `ffmpeg -buildconf` must show `--enable-gnutls`/`--enable-openssl`/`--enable-mbedtls`, and
> `ffmpeg -protocols` must list **both** `dtls` and `srtp`.

```powershell
# from src-tauri/scripts/
.\get-ffmpeg.ps1           # download + SHA256-verify + install the sidecar
.\get-ffmpeg.ps1 -Force    # re-download even if already present
```

It installs `../binaries/ffmpeg-<target-triple>.exe` (gitignored), which
`tauri.conf.json` → `bundle.externalBin: ["binaries/ffmpeg"]` bundles next to the app exe.
`binaries/` is not committed, so **every fresh clone must run this once** before
`pnpm tauri dev`/`build`.

The pin lives in `get-ffmpeg.ps1` (`$Url` + `$ExpectedSha256`). Gyan's GitHub release assets
are immutable per tag, so the hash is stable. To bump ffmpeg, point `$Url` at a newer
`GyanD/codexffmpeg` `full_build.zip`, run `-Force`, re-verify the DTLS backend (above), and
update `$ExpectedSha256` to the printed hash. (Gyan mirrors on GitHub because gyan.dev itself
is DNS-blocked on some ISPs.)

**GPL / redistribution**: the Gyan `full_build` links x264 (GPL). Bundling it makes our
installer GPL-encumbered; we accept that and the heavier updater artifacts (the full build is
~240 MB — a minimal GnuTLS build would trim this later). Corresponding source: the upstream
ffmpeg tag matching the version string the script prints (e.g. `8.1.x`) at
<https://github.com/GyanD/codexffmpeg>.

---

## Legacy: minimal self-built sidecar (`build-ffmpeg.ps1`)

> **STATUS: NOT CURRENTLY USED.** Kept only as a future size optimization (a from-source
> ~15–20 MB build vs the ~240 MB prebuilt). It is *not* wired into the app and has known
> bugs — see the banner in `build-ffmpeg.ps1` and `README_FFMPEG_MINIMAL_BUILD.md`. Do
> not run it for Phase 1.

`build-ffmpeg.ps1` is a one-shot builder that produces a minimal FFmpeg sidecar via
media-autobuild_suite. This section explains how the generated binary would be added to
the repo.

What you get

- `src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe` (expected ~15–20 MB stripped)
- Optional UPX-compressed variant: ~8–12 MB (smaller, may be flagged by AV)

Recommended repository options

1. Git LFS (recommended)

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

2. Commit directly (only if < ~20MB and you don't rebuild often)

- If you choose to commit the stripped binary directly (no LFS), it's acceptable for a single ~15–20MB file. Add it to `.gitignore` later if you want to stop tracking rebuilds.

3. Ignore binary (rebuild on demand)

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
