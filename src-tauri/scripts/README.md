# FFmpeg sidecar

The app streams via ffmpeg's native `-f whip` muxer, which requires **ffmpeg 8.x with a real
DTLS-SRTP backend** (GnuTLS, OpenSSL, or mbedTLS). We ship our own **minimal from-source
build** (~10 MB exe, `--enable-gnutls`, GPLv3) as a Tauri sidecar — hosted as a pinned GitHub
**prerelease** asset on this repo, downloaded by `get-ffmpeg.ps1`, reproduced by
`build-ffmpeg.ps1`. Current pin: `ffmpeg-min-2` (git master `9bc73ba344`), verified against
MediaMTX live.

> **Do not substitute a Windows SChannel build** (BtbN's default `win64-gpl` is SChannel-only).
> SChannel compiles the whip muxer but its DTLS handshake fails against MediaMTX at runtime
> (`SEC_E_ALGORITHM_MISMATCH 0x80090331` / "DTLS session failed"). Sanity-check any binary:
> `ffmpeg -buildconf` must show `--enable-gnutls`/`--enable-openssl`/`--enable-mbedtls`, and
> `ffmpeg -protocols` must list **both** `dtls` and `srtp`.

## Getting the sidecar: `get-ffmpeg.ps1` (run once after clone)

```powershell
# from src-tauri/scripts/
.\get-ffmpeg.ps1           # download + SHA256-verify + install the sidecar
.\get-ffmpeg.ps1 -Force    # re-download even if already present
```

It installs `../binaries/ffmpeg-<target-triple>.exe` (gitignored), which
`tauri.windows.conf.json` → `bundle.externalBin: ["binaries/ffmpeg"]` bundles next to the app
exe (Windows-only on purpose: streaming is `#[cfg(windows)]`, and keeping `externalBin` out of
the base `tauri.conf.json` lets the macOS/Linux CI legs build without a sidecar).
`binaries/` is not committed, so **every fresh clone must run this once** before
`pnpm tauri dev`/`build`. CI is automatic: both workflows (`release.yml`, `dev.yml`) run this
script on the Windows leg before `tauri-action`. The Rust side spawns the binary directly via
`tokio::process` (`stream/ffmpeg.rs::resolve_ffmpeg_path` finds it next to the exe, or in
`binaries/` in dev) — no `tauri-plugin-shell` involved.

The pin lives in `get-ffmpeg.ps1` (`$Url` + `$ExpectedSha256`), pointing at an asset on a
dedicated `ffmpeg-min-N` **prerelease** tag. Prerelease is load-bearing: the app updater
resolves `releases/latest`, and a normal release here would shadow it and break auto-updates.
Bumps get a new tag + new hash — existing assets are never overwritten.

**Not a general-purpose ffmpeg**: everything the app doesn't use is compiled out (no h264/aac
decoders, no gdigrab/dshow, most formats absent). If the streaming pipeline gains a codec,
filter, muxer, or protocol, the build must gain it too — see below. The component inventory
comes from `docs/SPEC_FFMPEG.md` and `src/stream/pipeline.rs`.

## Rebuilding: `build-ffmpeg.ps1` (only for bumps, never per-clone/CI)

### Prerequisites (Windows)

- Git in PATH
- ~10 GB free disk (build toolchain + intermediates)
- media-autobuild_suite cloned:
  `git clone https://github.com/m-ab-s/media-autobuild_suite.git C:\media-autobuild_suite`

### Build

```powershell
.\build-ffmpeg.ps1 -SuitePath "C:\media-autobuild_suite"
# the suite compiles in its own mintty window (the console launcher exiting
# nonzero is normal); when the mintty window finishes:
.\build-ffmpeg.ps1 -SuitePath "C:\media-autobuild_suite" -InstallOnly
```

**No prompts**: the script seeds the suite's answer file from the checked-in snapshot
(`scripts/media-autobuild_suite.ini` — GPLv3, 64-bit only, x264 8-bit lib, static ffmpeg,
every optional tool declined). GPLv3 is deliberate: it keeps `gmp` for GnuTLS, and the suite
disables OpenSSL under GPL, which is why the DTLS backend is GnuTLS. If the suite ever asks
a *new* question interactively, answer it, then refresh the snapshot from
`C:\media-autobuild_suite\build\media-autobuild_suite.ini`. Build time: ~30-60 min first run
(MSYS2 bootstrap + toolchain), ~5-10 min on rebuilds (ccache enabled).

The script writes `build/ffmpeg_options.txt` (our pinned configure flags, BOM-less — configure
rejects a BOM as an unknown option), runs the suite, and `-InstallOnly` copies the output to
`src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe`. No UPX on purpose: packed exes trigger
AV false positives on end-user machines.

### Verify

```powershell
$ffmpeg = ".\src-tauri\binaries\ffmpeg-x86_64-pc-windows-msvc.exe"

# License + DTLS backend: expect --enable-gnutls and NO --enable-nonfree
& $ffmpeg -hide_banner -buildconf

# WHIP transport: expect BOTH dtls and srtp
& $ffmpeg -hide_banner -protocols

# Everything the pipeline spawns
& $ffmpeg -hide_banner -muxers 2>&1   | Select-String "whip|mp4|mpjpeg|mjpeg"
& $ffmpeg -hide_banner -encoders 2>&1 | Select-String "libx264|libopus|aac|mjpeg|nvenc|amf"
& $ffmpeg -hide_banner -filters 2>&1  | Select-String "ddagrab|hwdownload|scale|aresample|split"

# Smoke tests (lavfi indev + the app's real capture/encode paths)
& $ffmpeg -f lavfi -i "anullsrc=r=48000:cl=stereo" -t 1 -c:a aac -f mp4 -y $env:TEMP\smoke.mp4
& $ffmpeg -f lavfi -i "ddagrab=output_idx=0:framerate=5" -frames:v 1 `
  -vf "hwdownload,format=bgra,scale=320:-2" -c:v mjpeg -f mjpeg -y $env:TEMP\thumb.mjpeg
```

Then the **runtime drills** from `docs/SPEC_FFMPEG.md` with this exe in `binaries/`:
preview (monitor + window), thumbnails, Publish → live → ready against MediaMTX (the DTLS
handshake is the real gate), ranked MP4 playable, stop → preview restart.

### Ship a new pin

1. Zip the exe (as `ffmpeg.exe` inside the archive) and upload it as an asset on a new
   `ffmpeg-min-N` release, **marked `prerelease`**. Attach `ffmpeg_options.txt` and point the
   release body at the exact upstream ffmpeg source commit (GPL corresponding-source
   obligation) — `ffmpeg -version` prints it.
2. Re-pin `get-ffmpeg.ps1`: `$Url` → the new asset, `$ExpectedSha256` → the zip's hash.
3. Fresh-clone test: `get-ffmpeg.ps1 -Force`, hash passes, launch the app, start a preview.

## Troubleshooting

| Problem                       | Fix                                                                                     |
| ----------------------------- | --------------------------------------------------------------------------------------- |
| Script says "exit code 1" but a mintty window is compiling | Normal — the suite detaches the compile into mintty. Wait for it, then re-run with `-InstallOnly`. |
| `configure: Unknown option "?"` | BOM in `ffmpeg_options.txt` — the script writes it BOM-less; don't rewrite it with PS5 `Set-Content -Encoding UTF8` |
| `nvenc requested ... ffnvcodec` | `--enable-ffnvcodec` missing — with `--disable-autodetect` the suite won't install nv-codec-headers on its own |
| `-f lavfi` input fails: no decoder for wrapped_avframe / pcm_u8 | lavfi wraps its outputs in those codecs; both decoders must stay enabled |
| Binary too large (>30 MB)     | Confirm the suite used our `build\ffmpeg_options.txt` (script rewrites it each run)     |
| WHIP muxer missing            | GnuTLS not linked or dtls protocol disabled; check `-buildconf` and `-protocols`        |
| DTLS handshake fails live     | Wrong TLS backend (SChannel) — must be GnuTLS/OpenSSL/mbedTLS, see the warning up top   |

## Appendix: what's in the build and why

| Category      | Components                                                       | Purpose                                         |
| ------------- | ---------------------------------------------------------------- | ----------------------------------------------- |
| **Encoders**  | libx264, libopus, aac, mjpeg (+ h264_nvenc, h264_amf)            | WHIP + MP4 + preview/thumb JPEG + hw encode     |
| **Decoders**  | rawvideo, pcm_f32le, wrapped_avframe, pcm_u8, h264, aac          | Raw pipes + lavfi wrapping + replay head trim   |
| **Muxers**    | whip, mp4, mpjpeg, mjpeg, segment                                | Live + segmented replay + preview + thumb       |
| **Inputs**    | lavfi indev, rawvideo/pcm_f32le/concat/mov demuxers              | Capture + raw pipes + replay assembly           |
| **Filters**   | ddagrab, hwdownload, format, scale, aresample, anullsrc, color, (a)split | Capture → scale → dual output + gap filler |
| **Protocols** | file, pipe, http(s), tcp, udp, tls, dtls, srtp, rtp, crypto      | Pipes + WHIP POST + WebRTC transport            |
| **BSFs**      | h264_mp4toannexb, aac_adtstoasc, extract_extradata               | Payload/container conversion                    |
| **Hw/TLS**    | d3d11va (ddagrab dep), ffnvcodec + amf headers, GnuTLS           | Capture + future hw encode + DTLS               |

Deliberately absent: gdigrab/dshow (dead since the WGC rework), qsv/libmfx (deprecated
upstream, would need libvpl), dxva2, the `null` muxer, UPX. The h264/aac decoders and the
mov/concat demuxers **are** present — the replay is written as segments and reassembled at
upload time, which needs to read them back and re-encode the head.

Licensing: `--enable-gpl` + GnuTLS, `--enable-nonfree` is never allowed (it would make the
binary legally non-redistributable), and the suite's GPL license choice disables OpenSSL anyway.
HW encoder headers are build-time only; at runtime NVENC/AMF use the user's GPU driver.

> **Known discrepancy.** `build-ffmpeg.ps1` asks for `--enable-version3`, but the suite does not
> pass it through — shipped binaries report `--enable-gpl` alone (true of `min-1` and `min-2`).
> `min-1`'s release notes nevertheless claimed GPLv3. Since GnuTLS pulls in gmp (LGPLv3), whether
> the build *should* be version3 needs a decision; until then release notes must describe what
> `-buildconf` actually reports, not what the script requests.

## Fallback: Gyan prebuilt full build

If the minimal pin is ever broken, the previously shipped Gyan GPL `full_build` (231 MB,
same GnuTLS backend, proven against MediaMTX) can be re-pinned in `get-ffmpeg.ps1`:
`https://github.com/GyanD/codexffmpeg/releases/download/8.1.2/ffmpeg-8.1.2-full_build.zip`
(sha256 `b8cdefab5f50590a076c27c2b56b0294a0e6154faded28ba1ba05ebc4f801f57`).

---

# OBS game-capture helpers

Exclusive-fullscreen games (Celeste etc.) are not DWM-composited, so WGC window capture returns
black and ddagrab loses the display at the mode flip. The only way to grab such a game's pixels
**without reading the whole screen** is graphics-API hook injection — the OBS Game Capture model.
We reuse OBS Studio's proven `win-capture` binaries; our own Rust client
(`src/stream/gamecapture/`) drives them over the documented shared-memory/event protocol.

`get-game-capture.ps1` (run once after clone, like `get-ffmpeg.ps1`) downloads the pinned OBS
portable zip, SHA256-verifies it, and extracts six files into `binaries/gamecapture/`
(shipped via `tauri.conf.json` → `bundle.resources`):
`graphics-hook{32,64}.dll`, `inject-helper{32,64}.exe`, `get-graphics-offsets{32,64}.exe`.
Both bitnesses ship — a 64-bit host must still capture 32-bit games.

**Current pin:** OBS **32.1.2** (hook ABI `HOOK_VER 1.8`). The `struct hook_info` layout mirrored
in `gamecapture/protocol.rs` is the OBS win-capture ABI (`sizeof == 648`, guarded by a
compile-time assert). It is stable across OBS releases but not guaranteed forever — **when you
bump the pin, re-verify `protocol.rs` against `shared/obs-hook-config/graphics-hook-info.h` at
the new tag.** Asset digests: `gh api repos/obsproject/obs-studio/releases/tags/<tag>
--jq '.assets[] | "\(.name) \(.digest)"'`.

**Licensing (GPLv2).** These are unmodified OBS Studio binaries redistributed as separate helper
executables + an injected DLL — mere aggregation; our app links none of their code. Corresponding
source is OBS's own public tag, which is also where `get-game-capture.ps1` downloads from:
`https://github.com/obsproject/obs-studio/tree/32.1.2` (this pointer is the written source offer).
Keep this reference and the pin in sync. Note: a **user-facing** third-party-notices surface for
the shipped app (covering both this and the GPLv3 ffmpeg sidecar) is a separate, broader
compliance task not specific to this feature.
