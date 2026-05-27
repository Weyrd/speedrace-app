# ffmpeg Sidecar - Build & Integration

The Momentum desktop app bundles a **custom minimal ffmpeg** (~15–20MB instead of ~130MB) as a Tauri sidecar. It handles screen capture → WHIP live stream + local MP4 replay simultaneously.

This folder contains the build script and this guide.

---

## Quick Start

### Prerequisites (on Windows)

- Git installed and in PATH
- ~10GB free disk space (for build toolchain + intermediate files)
- Optional: `winget install upx` for extra compression (~50% smaller binary)

### Step 1 - Clone media-autobuild_suite

```powershell
git clone https://github.com/m-ab-s/media-autobuild_suite.git C:\media-autobuild_suite
```

You can put it anywhere. The script will ask for the path.

### Step 2 - Run the build script

```powershell
cd momentum-app
.\src-tauri\scripts\build-ffmpeg.ps1
```

Or skip the prompt:

```powershell
.\src-tauri\scripts\build-ffmpeg.ps1 -SuitePath "C:\media-autobuild_suite"
```

**First run is interactive** - media-autobuild_suite will ask setup questions (MSYS2 install location, toolchain, etc.). Accept defaults. Subsequent runs are fully automatic.

Build time: **~30–60min** first run (downloads + compiles x264, opus, openssl, ffmpeg), **~5–10min** on rebuilds.

### Step 3 - Done

The script places the binary at:

```
src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe
```

No further steps needed - Tauri picks it up automatically on the next build.

---

## What the Script Does

1. `git pull` on media-autobuild_suite (gets latest build recipes)
2. Writes `build/ffmpeg_options.txt` with our minimal configure flags
3. Launches the build (compiles all dependencies from source)
4. Locates the output `ffmpeg.exe`
5. Strips debug symbols (if `strip` is in PATH)
6. Compresses with UPX (if installed)
7. Copies the final binary to `src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe`

---

## Integrating into the Tauri App

### tauri.conf.json

Add `externalBin` to the bundle config:

```json
{
  "bundle": {
    "active": true,
    "targets": "all",
    "externalBin": ["binaries/ffmpeg"],
    "icon": [...]
  }
}
```

Tauri automatically resolves the platform-specific binary name. On Windows it looks for `binaries/ffmpeg-x86_64-pc-windows-msvc.exe`.

### Rust side - spawning the sidecar

```rust
use tauri::Manager;
use tauri_plugin_shell::ShellExt;

// From any Tauri command or setup hook:
let sidecar = app.shell().sidecar("ffmpeg").unwrap();
let (mut rx, child) = sidecar
    .args(["-f", "lavfi", "-i", "ddagrab=0,hwdownload,format=bgra", ...])
    .spawn()
    .expect("failed to spawn ffmpeg sidecar");
```

### Required Tauri plugin

The `shell` plugin is needed for sidecar support:

```toml
# src-tauri/Cargo.toml
[dependencies]
tauri-plugin-shell = "2"
```

```json
// src-tauri/capabilities/default.json - add shell permission
{
  "permissions": ["shell:allow-spawn", "shell:allow-stdin-write"]
}
```

### .gitignore

The binary is large (~15–20MB). Either:

**Option A** - Gitignore it (recommended, rebuild when needed):

```gitignore
# src-tauri/.gitignore
binaries/ffmpeg-*.exe
```

**Option B** - Use Git LFS:

```bash
git lfs track "src-tauri/binaries/ffmpeg-*.exe"
```

---

## Verification

After building, run these to confirm everything is included:

```powershell
$ffmpeg = ".\src-tauri\binaries\ffmpeg-x86_64-pc-windows-msvc.exe"

# Core encoders
& $ffmpeg -hide_banner -encoders 2>&1 | Select-String "libx264|h264_nvenc|h264_qsv|h264_amf|libopus|aac"

# WHIP + MP4 muxers
& $ffmpeg -hide_banner -muxers 2>&1 | Select-String "whip|mp4"

# Screen capture + scaling filters
& $ffmpeg -hide_banner -filters 2>&1 | Select-String "ddagrab|hwdownload|scale|format"

# Protocols (pipe for audio stdin, http for WHIP)
& $ffmpeg -hide_banner -protocols 2>&1 | Select-String "pipe|file|http|https|udp"

# Smoke test
& $ffmpeg -f lavfi -i "testsrc=duration=1:size=1280x720:rate=30" -c:v libx264 -f null -
```

---

## Rebuilding / Updating ffmpeg

Just re-run the script. It `git pull`s the suite and rewrites the options file, so you always get the latest ffmpeg source with our pinned flags.

```powershell
.\src-tauri\scripts\build-ffmpeg.ps1 -SuitePath "C:\media-autobuild_suite"
```

---

## Troubleshooting

| Problem                            | Fix                                                                                                        |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Build fails on first run           | Normal - media-autobuild_suite prompts setup questions. Re-run after setup completes.                      |
| `strip` not found                  | Add MSYS2's `mingw64/bin` to your Windows PATH                                                             |
| Binary too large (>30MB)           | Ensure the build used our `ffmpeg_options.txt` - check `C:\media-autobuild_suite\build\ffmpeg_options.txt` |
| `ddagrab` filter missing           | You need ffmpeg 7.0+. The suite should build latest by default.                                            |
| WHIP muxer missing                 | OpenSSL must be linked. Should be automatic with the suite.                                                |
| AV software quarantines the binary | Happens with UPX. Rebuild without UPX (remove it from PATH before running script).                         |

---

---

## Appendix: Technical Specifications

> Reference for what's included in the minimal build and why.

### Included Components

| Category          | Components                                                              | Purpose                                    |
| ----------------- | ----------------------------------------------------------------------- | ------------------------------------------ |
| **Encoders**      | libx264, h264_nvenc, h264_qsv, h264_amf, libopus, aac                   | H.264 video (SW + HW) + audio for WHIP/MP4 |
| **Decoders**      | rawvideo, pcm_f32le, h264, aac                                          | Raw inputs + remux support                 |
| **Muxers**        | whip, mp4, mov, null                                                    | Live WebRTC output + replay file           |
| **Demuxers**      | lavfi, rawvideo, pcm_f32le, mov, concat                                 | Virtual input + raw audio + crash remux    |
| **Filters**       | ddagrab, hwdownload, format, scale, scale_cuda, aresample, split/asplit | Screen capture → process → dual output     |
| **Protocols**     | pipe, file, http, https, udp, tcp, tls, crypto, rtp, srtp               | Audio stdin + disk + WebRTC transport      |
| **BSFs**          | h264_mp4toannexb, aac_adtstoasc                                         | Container format conversion                |
| **Input devices** | gdigrab, dshow                                                          | Fallback screen capture                    |

### Size Breakdown

| Component                                             | Size         |
| ----------------------------------------------------- | ------------ |
| libx264 (static)                                      | ~4MB         |
| libopus (static)                                      | ~500KB       |
| ffmpeg core (lavformat, lavcodec, lavutil, lavfilter) | ~8–12MB      |
| OpenSSL (DTLS/TLS for WHIP)                           | ~3MB         |
| NVENC/QSV/AMF headers                                 | ~200KB       |
| **Total (stripped, static)**                          | **~15–20MB** |
| **Total (+ UPX)**                                     | **~8–12MB**  |

### Important Constraints

- **WHIP muxer requires OpenSSL** - WebRTC needs DTLS/SRTP. Always linked.
- **ddagrab requires ffmpeg ≥7.0** - The suite builds latest by default.
- **HW encoder headers are build-time only** - NVENC/QSV/AMF use the user's GPU driver at runtime. If no compatible GPU, software fallback is automatic.
- **Static linking** - Single `.exe`, no DLL dependencies. Portable.

### Raw Configure Flags

These are what `build-ffmpeg.ps1` writes to `ffmpeg_options.txt`:

```
--enable-gpl --enable-nonfree --enable-version3
--disable-everything --disable-doc --disable-autodetect --disable-programs --disable-avdevice
--enable-ffmpeg --enable-network --enable-small
--enable-libx264 --enable-libopus --enable-openssl
--enable-d3d11va --enable-dxva2 --enable-nvenc --enable-amf --enable-libmfx
--enable-encoder=libx264,h264_nvenc,h264_qsv,h264_amf,h264_mf,libopus,aac,pcm_f32le
--enable-decoder=rawvideo,pcm_f32le,pcm_s16le,h264,aac
--enable-muxer=whip,mp4,mov,null,rawvideo
--enable-demuxer=lavfi,rawvideo,pcm_f32le,pcm_s16le,mov,concat
--enable-parser=h264,aac,opus
--enable-protocol=file,pipe,http,https,udp,tcp,tls,crypto,rtp,srtp
--enable-filter=ddagrab,hwdownload,hwupload,format,scale,scale_cuda,null,anull,aresample,aformat,abuffer,buffer,abuffersink,buffersink,setpts,asetpts,trim,atrim,split,asplit
--enable-bsf=h264_mp4toannexb,aac_adtstoasc,null
--enable-indev=gdigrab,dshow
--enable-optimizations --enable-stripping --disable-debug
```
