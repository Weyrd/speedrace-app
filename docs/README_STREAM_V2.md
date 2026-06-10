# Stream V2 - ffmpeg Sidecar: Live WHIP + Local Replay

## Summary

Replace the webview-based WebRTC WHIP stream with an **ffmpeg sidecar** that:

1. Captures a **screen or application window** (DXGI Desktop Duplication) + system audio (WASAPI loopback)
2. Outputs **two streams simultaneously** from a single capture:
   - **Live (WHIP)**: Low quality, high framerate (target **60fps**), ultra-lightweight - viewers see smooth motion with minimal bandwidth
   - **Replay (MP4)**: Minimum 720p60, well-compressed for long runs (3–4 hours), YouTube-ready

> **Not a camera/webcam capture.** This captures what's on screen (full monitor or a specific application window). The user picks which screen/app to capture in the UI.

The webview no longer handles `getDisplayMedia` or WebRTC. The Rust `FfmpegStreamHandle` replaces `WhipStreamHandle` behind the existing `StreamHandle` trait - no changes to `commands.rs` or backend.

---

## Quality Philosophy

| Output           | Goal                           | Framerate               | Resolution                         | Bitrate/Quality        | Rationale                                                                                                 |
| ---------------- | ------------------------------ | ----------------------- | ---------------------------------- | ---------------------- | --------------------------------------------------------------------------------------------------------- |
| **WHIP (live)**  | Smooth, lightweight, real-time | **60fps** (hard target) | 720p or lower                      | 1500–2500 kbps         | Viewers need fluidity, not pixel-perfect quality. Lower resolution = less bandwidth = more stable WebRTC. |
| **MP4 (replay)** | Archival, YouTube-uploadable   | **60fps** minimum       | **720p** minimum (native if ≥720p) | CRF 26–28, slow preset | Long runs (3–4h) must stay reasonable in file size. CRF 26 at 720p60 ≈ 1.5–2.5 GB/hour.                   |

### File size estimates (720p60, CRF 26, slow preset)

| Duration | Estimated size |
| -------- | -------------- |
| 1 hour   | ~1.5–2.5 GB    |
| 3 hours  | ~4.5–7.5 GB    |
| 4 hours  | ~6–10 GB       |

With CRF 28 (more compression, slightly lower quality): roughly 30–40% smaller.
With 1080p: roughly 2× larger than 720p.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Tauri App (Rust)                                       │
│                                                         │
│  ┌─────────────────┐     ┌────────────────────────┐    │
│  │  WASAPI Loopback │────▶│  Named Pipe (audio)    │    │
│  │  (cpal crate)    │     │  (raw f32le PCM)       │    │
│  └─────────────────┘     └──────────┬─────────────┘    │
│                                      │                  │
│  ┌─────────────────┐     ┌──────────┼─────────────┐    │
│  │  WGC capture     │────▶│  Named Pipe (video)    │    │
│  │  (window only)   │     │  (raw BGRA frames)     │    │
│  └─────────────────┘     └──────────┼─────────────┘    │
│   (monitor target skips this — ffmpeg ddagrab instead)  │
│                                      │                  │
│  ┌───────────────────────────────────▼──────────────┐   │
│  │  ffmpeg sidecar                                   │   │
│  │                                                   │   │
│  │  Video in (monitor): -f lavfi "ddagrab=0,..."    │   │
│  │  Video in (window):  -f rawvideo -pix_fmt bgra   │   │
│  │                      -i \\.\pipe\momentum_video  │   │
│  │  Audio in: -f f32le -ar 48000 -ac 2              │   │
│  │            -i \\.\pipe\momentum_audio            │   │
│  │                                                   │   │
│  │  Output 1 (LIVE):                                 │   │
│  │    -c:v libx264 -preset veryfast -tune zerolatency│   │
│  │    -bf 0 -profile:v baseline -b:v {stream_bitrate}│   │
│  │    -c:a libopus -ar 48000 -ac 2                   │   │
│  │    -f whip "{mediamtx_whip_url}"                  │   │
│  │                                                   │   │
│  │  Output 2 (REPLAY):                              │   │
│  │    -c:v libx264 -preset medium -crf {rec_crf}    │   │
│  │    -profile:v high                                │   │
│  │    -c:a aac -b:a 192k                            │   │
│  │    "{replay_path}.mp4"                           │   │
│  └───────────────────────────────────────────────────┘   │
│                                                         │
│  FfmpegStreamHandle                                     │
│    - spawns ffmpeg child process                        │
│    - spawns cpal audio capture thread                   │
│    - monitors stderr for progress/errors                │
│    - stop() → SIGTERM → graceful WHIP teardown + MP4   │
│              finalization (moov atom written)            │
└─────────────────────────────────────────────────────────┘
```

---

## Why this approach

| Criteria                               | ffmpeg tee/multi-output | MediaRecorder (browser API) | ffmpeg record-only |
| -------------------------------------- | ----------------------- | --------------------------- | ------------------ |
| Single capture source                  | ✅                      | ✅                          | ❌ (two captures)  |
| Low-latency live (WHIP)                | ✅ native `-f whip`     | ✅ existing WhipClient      | N/A                |
| YouTube-ready MP4 (H.264+AAC)          | ✅                      | ❌ (WebM/VP9)               | ✅                 |
| Quality control (bitrate, CRF, preset) | ✅ full                 | ❌ limited                  | ✅                 |
| Hardware encoding (NVENC/QSV)          | ✅                      | ❌                          | ✅                 |
| Cross-platform (macOS later)           | ✅ (AVFoundation)       | Partial                     | ✅                 |
| No extra dependencies                  | ❌ (sidecar)            | ✅                          | ❌ (sidecar)       |
| Configurable stream vs replay quality  | ✅                      | ❌                          | N/A                |

**ffmpeg multi-output wins** because:

- WHIP requires Opus audio, YouTube requires AAC → can't use simple `tee` → need separate outputs (ffmpeg handles this natively)
- One screen capture, two encodes with different optimization targets
- Produces a YouTube-ready MP4 with no post-processing
- The existing `StreamHandle` trait was designed for this swap

---

## ffmpeg Pipeline - Windows

### Minimal command (software encoding)

```bash
ffmpeg \
  -f lavfi -i "ddagrab=output_idx=0,hwdownload,format=bgra" \
  -f f32le -ar 48000 -ac 2 -i pipe:0 \
  \
  # Output 1: WHIP live - low quality, 60fps, ultra-light
  -map 0:v -map 1:a \
  -c:v libx264 -preset ultrafast -tune zerolatency -bf 0 -profile:v baseline \
  -r 60 -g 120 -b:v 2000k \
  -vf "scale=1280:720:flags=fast_bilinear" \
  -c:a libopus -ar 48000 -ac 2 -b:a 64k \
  -f whip "http://mediamtx:8889/{lobby_id}/whip" \
  \
  # Output 2: Replay MP4 - 720p60 minimum, compressed for long runs
  -map 0:v -map 1:a \
  -c:v libx264 -preset slow -crf 26 -profile:v high \
  -r 60 \
  -vf "scale='if(gte(iw,1280),iw,1280)':'if(gte(ih,720),ih,720)':flags=lanczos" \
  -c:a aac -ar 48000 -ac 2 -b:a 128k \
  -movflags +faststart \
  "{replay_dir}/{lobby_id}_{timestamp}.mp4"
```

### With NVIDIA hardware encoding (much lower CPU)

```bash
ffmpeg \
  -f lavfi -i "ddagrab=output_idx=0" \
  -f f32le -ar 48000 -ac 2 -i pipe:0 \
  \
  # Output 1: WHIP live - NVENC low-latency, 60fps, 720p downscale
  -map 0:v -map 1:a \
  -c:v h264_nvenc -preset p3 -tune ll -profile:v baseline -bf 0 \
  -r 60 -b:v 2000k \
  -vf "scale_cuda=1280:720" \
  -c:a libopus -ar 48000 -ac 2 -b:a 64k \
  -f whip "http://mediamtx:8889/{lobby_id}/whip" \
  \
  # Output 2: Replay - NVENC quality mode, 60fps, compressed
  -map 0:v -map 1:a \
  -c:v h264_nvenc -preset p6 -profile:v high -cq 28 -b:v 0 \
  -r 60 \
  -c:a aac -ar 48000 -ac 2 -b:a 128k \
  -movflags +faststart \
  "{replay_dir}/{lobby_id}_{timestamp}.mp4"
```

### Fallback: gdigrab (older systems without DXGI DDA)

```bash
ffmpeg \
  -f gdigrab -framerate 60 -i desktop \
  -f f32le -ar 48000 -ac 2 -i pipe:0 \
  ... (same outputs)
```

---

## Video Capture: Screen/App Only (no camera)

This captures **what is displayed on screen** - either a full monitor or a specific application window. There is no webcam/camera capture; this is a speedrun proof-of-play recording.

The user selects what to capture in the settings UI before going live:

- **Full screen** - captures everything on the selected monitor (`ddagrab`, in-ffmpeg)
- **Specific window** - captures a single application, e.g. the game window (Windows Graphics Capture, in-Rust → piped to ffmpeg)

The two targets use **different capture engines**, because the constraints differ:

| Target | Engine | Why |
| --- | --- | --- |
| Full monitor | DXGI Desktop Duplication (`ddagrab`) | ffmpeg captures it directly, GPU zero-copy into NVENC. Follows whatever is on the monitor, including a game that returns to fullscreen. |
| Single window / game | **Windows Graphics Capture (WGC)** | Captures a specific `HWND` at the DWM compositor level. Game-only, GPU-accelerated, **works with fullscreen and borderless windows**, no DLL injection. |

> **Why not `gdigrab` for windows?** `gdigrab` is GDI/BitBlt-based and **cannot capture GPU-accelerated or fullscreen DirectX/Vulkan/OpenGL windows** — it returns black or stale frames, the exact failure mode of the v1 webview `getDisplayMedia` window capture. It is kept only as a last-resort fallback for legacy desktop apps. WGC is the correct primitive for game-only capture.

### Full screen — Desktop Duplication API (ddagrab)

- Available in ffmpeg 6.0+ (standard in Windows builds from gyan.dev / BtbN)
- Uses DXGI Desktop Duplication API - GPU-accelerated, very low overhead
- `output_idx=0` captures primary monitor (configurable)
- Outputs D3D11 hardware frames → can be passed directly to NVENC for zero-copy
- Supports 60fps capture natively

```
ddagrab=output_idx=0    # Primary monitor
ddagrab=output_idx=1    # Secondary monitor
```

### Single window — Windows Graphics Capture (WGC)

This is the **Discord / modern-OBS "Window Capture" approach without process injection**. ffmpeg has no WGC input device, so the capture runs in Rust (via the `windows` crate) and pipes raw frames into ffmpeg.

**Capture loop (Rust):**

1. Resolve the target `HWND` (chosen in settings) and create a `GraphicsCaptureItem` via `IGraphicsCaptureItemInterop::CreateForWindow`.
2. Create a `Direct3D11CaptureFramePool` (free-threaded) + `GraphicsCaptureSession` bound to a shared D3D11 device.
3. Set `IsBorderRequired = false` (removes the yellow capture border; Win11 / recent Win10) and `IsCursorCaptureEnabled` per setting.
4. On each `FrameArrived`, copy the BGRA `ID3D11Texture2D` to a CPU-readable **staging texture**, `Map` it, and write the raw BGRA bytes to a **named pipe** (`\\.\pipe\momentum_video`).
5. On window resize, call `framePool.Recreate(...)` with the new dimensions.

**ffmpeg side — read raw frames from the pipe:**

```bash
ffmpeg \
  -f rawvideo -pix_fmt bgra -s {win_w}x{win_h} -framerate 60 -i \\.\pipe\momentum_video \
  -f f32le -ar 48000 -ac 2 -i \\.\pipe\momentum_audio \
  ... (same dual WHIP + MP4 outputs as the ddagrab pipeline)
```

Notes:
- Both video (WGC) and audio (WASAPI loopback) now arrive over **named pipes** so ffmpeg's `stdin` stays free; the previous "audio via `pipe:0`/stdin" wording in the diagrams becomes a named pipe for symmetry.
- Window dimensions are read once at start and on resize; the `-s` flag must match what the capture thread writes. Send a new resolution by restarting the pipe segment or padding to a fixed canvas — simplest is to lock capture to the window's client size at "Go Live" and recreate on significant resize.
- WGC requires **Windows 10 1903+**. If unavailable, fall back to **monitor capture** (`ddagrab`), not `gdigrab` — monitor capture is more robust than GDI window capture for games.

### Fullscreen / alt-tab behavior (the v1 pain point)

- **Exclusive fullscreen + window capture is fundamentally fragile.** WGC handles it far better than gdigrab/`getDisplayMedia`, but the most bulletproof advice for users is still **run the game in Borderless / Windowed-Fullscreen** — visually identical to fullscreen, but a normal window that captures cleanly and survives alt-tab.
- **Monitor capture (`ddagrab`) sidesteps the issue entirely**: it shows whatever is on the monitor, so a game that drops out of fullscreen during source selection and returns afterward is captured correctly. The settings UI should nudge users toward monitor capture when in doubt.

The app enumerates available monitors and windows via Windows API and presents them in the settings UI.

---

## Audio Capture: WASAPI Loopback via cpal

ffmpeg on Windows has no built-in system audio loopback input. The cleanest solution:

1. **Rust side** uses the `cpal` crate with WASAPI backend in **loopback mode**
2. Captures the default audio output device (system sounds + game audio)
3. Pipes raw PCM (`f32le`, 48kHz, stereo) to ffmpeg's stdin

```rust
// Pseudocode
let host = cpal::host_from_id(cpal::HostId::Wasapi)?;
let device = host.default_output_device()?; // loopback of the output
let config = StreamConfig { sample_rate: 48000, channels: 2, .. };

let stream = device.build_input_stream(
    &config,
    move |data: &[f32], _| {
        // Write raw f32le PCM to ffmpeg's stdin pipe
        ffmpeg_stdin.write_all(bytemuck::cast_slice(data));
    },
    |err| eprintln!("audio capture error: {}", err),
    None, // No timeout
)?;
stream.play()?;
```

**Why not "Stereo Mix"?**

- Not available on all systems
- Must be manually enabled in Windows Sound settings
- Poor UX for non-technical users

**Why cpal loopback?**

- Zero user configuration needed
- Works on all Windows 10+ systems
- Pure Rust, compiles into the app
- Guaranteed to capture exactly what the user hears

---

## Tauri Integration

### Sidecar bundling

In `tauri.conf.json`:

```json
{
  "bundle": {
    "externalBin": ["binaries/ffmpeg"]
  }
}
```

Place the ffmpeg binary at:

```
src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe
```

Tauri resolves the correct binary per platform at runtime via `app.shell().sidecar("ffmpeg")`.

### FfmpegStreamHandle

```rust
// src-tauri/src/stream/ffmpeg.rs

pub struct FfmpegStreamHandle {
    child: Child,           // ffmpeg process
    audio_stream: cpal::Stream, // WASAPI loopback capture
    is_live: AtomicBool,
    replay_path: PathBuf,
}

impl StreamHandle for FfmpegStreamHandle {
    fn is_live(&self) -> bool {
        self.is_live.load(Ordering::Relaxed)
    }

    fn stop(&self) {
        // 1. Send 'q' to ffmpeg stdin (graceful quit)
        //    This makes ffmpeg:
        //    - Send WHIP DELETE to teardown the WebRTC session
        //    - Finalize MP4 (write moov atom)
        // 2. Wait up to 5s for process exit
        // 3. If still running, SIGTERM
        // 4. Stop cpal audio stream
        // 5. Set is_live = false
    }
}
```

### StreamConfig (subset of app settings - stream-specific)

```rust
pub struct StreamConfig {
    // Capture source
    pub monitor_index: u32,              // 0 = primary
    pub capture_target: CaptureTarget,   // FullScreen | Window(window_id)

    // Live stream (WHIP) - priority: framerate > quality
    pub stream_framerate: u32,           // target 60
    pub stream_resolution: (u32, u32),   // 1280x720 default (downscale if native is higher)
    pub stream_bitrate_kbps: u32,        // 1500–2500, default 2000

    // Replay (MP4) - priority: compression > quality, minimum 720p60
    pub replay_framerate: u32,           // minimum 60
    pub replay_resolution: ReplayResolution, // Native | 720p | 1080p
    pub replay_crf: u8,                  // 24–30, default 26 (good compression)
    pub replay_preset: String,           // "slow" default (better compression ratio)

    // Hardware encoding (auto-detected, user can override)
    pub encoder: EncoderChoice,          // Auto | Nvenc | Qsv | Amf | Software
}

pub enum CaptureTarget {
    FullScreen,
    Window(String), // window handle/title
}

pub enum ReplayResolution {
    Native,     // whatever the capture source provides (if ≥720p)
    Res720p,    // force 1280x720
    Res1080p,   // force 1920x1080
}

pub enum EncoderChoice {
    Auto,       // NVENC > QSV > AMF > Software
    Nvenc,
    Qsv,
    Amf,
    Software,
}
```

---

## Lifecycle

### Start flow

```
User clicks "Go Live" in StreamSetup
    → Tauri command: start_stream(lobby_id, config)
    → Rust:
        1. Build ffmpeg argument list from StreamConfig + lobby WHIP URL
        2. Spawn cpal WASAPI loopback → pipe to ffmpeg stdin
        3. Spawn ffmpeg sidecar with constructed args
        4. Monitor ffmpeg stderr for "Output #0" (WHIP connected) confirmation
        5. Notify backend via WS: stream_live
        6. Return FfmpegStreamHandle to state
        7. Frontend transitions to Racing/WaitingForStart screen
```

### Stop flow (normal finish or user stop)

```
Backend sends race_finish via WS / User clicks Stop
    → Tauri: stop_stream()
    → Rust:
        1. Write 'q' to ffmpeg stdin
        2. ffmpeg gracefully:
           - Tears down WHIP session (HTTP DELETE)
           - Finalizes MP4 (writes moov atom, faststart)
        3. Wait for process exit (timeout 10s)
        4. Stop cpal audio stream
        5. replay_path is now a complete, playable MP4
        6. Emit event: stream_stopped { replay_path }
```

### Crash/forfeit flow

```
WebSocket disconnect / forfeit
    → Rust:
        1. SIGTERM to ffmpeg (or taskkill on Windows)
        2. MP4 may be incomplete (no moov atom)
           → Run quick remux: ffmpeg -i broken.mp4 -c copy fixed.mp4
           → Or accept partial loss (forfeit = no upload anyway)
        3. Cleanup
```

---

## File Structure (new/modified files)

```
src-tauri/
├── binaries/
│   └── ffmpeg-x86_64-pc-windows-msvc.exe   ← bundled sidecar
├── Cargo.toml                               ← add: cpal, bytemuck, dirs
└── src/
    ├── settings/
    │   ├── mod.rs                           ← NEW: AppSettings, load/save, Tauri commands
    │   ├── schema.rs                        ← NEW: All settings structs + defaults
    │   └── migration.rs                     ← NEW: Schema version upgrades
    └── stream/
        ├── mod.rs                           ← keep StreamHandle trait
        ├── handler.rs                       ← remove WhipStreamHandle
        ├── ffmpeg.rs                        ← NEW: FfmpegStreamHandle
        ├── audio_capture.rs                 ← NEW: cpal WASAPI loopback → named pipe
        ├── window_capture.rs               ← NEW: Windows Graphics Capture (WGC) → raw BGRA pipe
        ├── pipeline.rs                      ← NEW: build ffmpeg CLI args from settings (ddagrab vs rawvideo pipe)
        └── hardware_detect.rs              ← NEW: probe NVENC/QSV availability

src/
├── stores/
│   └── settings.ts                         ← NEW: reactive settings store (Zustand)
├── components/
│   ├── settings/
│   │   ├── SettingsModal.tsx               ← NEW: tabbed settings modal
│   │   ├── GeneralSettings.tsx             ← NEW: language, theme, startup
│   │   ├── StreamSettings.tsx              ← NEW: capture/stream/replay config
│   │   └── AdvancedSettings.tsx            ← NEW: encoder, debug
│   └── StreamSetup.tsx                     ← simplified (no more getDisplayMedia)
├── stream/
│   └── whip.ts                             ← DELETE (no longer needed)
└── i18n/                                    ← already exists, add settings translations
```

---

## Hardware Encoding Detection

At app startup or first stream, probe available encoders:

```rust
// Run: ffmpeg -encoders 2>&1 | grep h264
// Look for:
//   h264_nvenc  - NVIDIA GPU
//   h264_qsv   - Intel QuickSync
//   h264_amf   - AMD AMF

pub fn detect_hw_encoders(ffmpeg_path: &Path) -> HwEncoders {
    let output = Command::new(ffmpeg_path)
        .args(["-hide_banner", "-encoders"])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    HwEncoders {
        nvenc: stdout.contains("h264_nvenc"),
        qsv: stdout.contains("h264_qsv"),
        amf: stdout.contains("h264_amf"),
    }
}
```

Auto-select the best encoder: NVENC > QSV > AMF > libx264 (software fallback).

---

## Settings Module (app-wide, extensible)

The app needs a **robust, extensible settings system** - not just stream config. Stream settings are one category among many (language, theme, keybinds, etc.). The module is designed to grow.

### Architecture

```
src-tauri/src/settings/
├── mod.rs              ← AppSettings struct, load/save, Tauri commands
├── schema.rs           ← All settings categories with serde defaults
└── migration.rs        ← Handle schema changes between app versions

src/
├── stores/
│   └── settings.ts     ← Frontend reactive store (syncs with Rust via IPC)
└── components/
    └── settings/
        ├── SettingsModal.tsx       ← Modal container with tabs/categories
        ├── GeneralSettings.tsx     ← Language, theme, startup behavior
        ├── StreamSettings.tsx      ← All capture/stream/replay config
        └── AdvancedSettings.tsx    ← Encoder override, debug flags
```

### Settings schema (Rust - persisted as JSON file)

```rust
// src-tauri/src/settings/schema.rs

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AppSettings {
    pub version: u32,               // schema version for migrations
    pub general: GeneralSettings,
    pub stream: StreamSettings,
    // Future categories:
    // pub keybinds: KeybindSettings,
    // pub notifications: NotificationSettings,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct GeneralSettings {
    pub language: String,            // "en", "fr", etc. (i18next locale code)
    pub theme: Theme,                // System | Light | Dark
    pub minimize_to_tray: bool,      // default true
    pub launch_on_startup: bool,     // default false
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct StreamSettings {
    // Capture
    pub monitor_index: u32,
    pub capture_target: CaptureTarget,

    // WHIP (live) - lightweight, max fluidity
    pub stream_framerate: u32,           // default: 60
    pub stream_resolution: (u32, u32),   // default: (1280, 720)
    pub stream_bitrate_kbps: u32,        // default: 2000

    // Replay (MP4) - compressed, archival quality
    pub replay_framerate: u32,           // default: 60, minimum enforced: 60
    pub replay_resolution: ReplayResolution, // default: Native (min 720p enforced)
    pub replay_compression: ReplayCompression, // default: Balanced
    pub replay_dir: PathBuf,             // default: ~/Videos/Momentum/

    // Encoder
    pub encoder: EncoderChoice,          // default: Auto
}

/// Replay compression presets (maps to CRF + preset internally)
#[derive(Serialize, Deserialize, Clone)]
pub enum ReplayCompression {
    Light,      // CRF 22, preset medium  → ~2.5–4 GB/h at 720p60 (better quality)
    Balanced,   // CRF 26, preset slow    → ~1.5–2.5 GB/h at 720p60 (recommended)
    Heavy,      // CRF 28, preset slower  → ~1–1.8 GB/h at 720p60 (smaller files)
}
```

### Default implementation

```rust
impl Default for StreamSettings {
    fn default() -> Self {
        Self {
            monitor_index: 0,
            capture_target: CaptureTarget::FullScreen,
            stream_framerate: 60,
            stream_resolution: (1280, 720),
            stream_bitrate_kbps: 2000,
            replay_framerate: 60,
            replay_resolution: ReplayResolution::Native,
            replay_compression: ReplayCompression::Balanced,
            replay_dir: dirs::video_dir().unwrap_or_default().join("Momentum"),
            encoder: EncoderChoice::Auto,
        }
    }
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            theme: Theme::System,
            minimize_to_tray: true,
            launch_on_startup: false,
        }
    }
}
```

### Storage

- Persisted as `settings.json` in the Tauri app data directory (`app.path().app_data_dir()`)
- Loaded at startup, written on every change (debounced 500ms)
- If file is missing or invalid → use `Default::default()` and write it fresh

### Tauri IPC commands

```rust
#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String>;

#[tauri::command]
async fn update_settings(state: State<'_, AppState>, settings: AppSettings) -> Result<(), String>;

#[tauri::command]
async fn get_available_monitors() -> Result<Vec<MonitorInfo>, String>;

#[tauri::command]
async fn get_available_encoders(state: State<'_, AppState>) -> Result<HwEncoders, String>;
```

### Frontend store (reactive)

```typescript
// src/stores/settings.ts
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

interface SettingsStore {
  settings: AppSettings | null;
  load: () => Promise<void>;
  update: (patch: Partial<AppSettings>) => Promise<void>;
}
```

The settings modal renders category tabs. Each tab is a standalone component that reads/writes from the store. Adding a new category = add a new sub-struct to `AppSettings` + a new tab component.

### Stream settings UI

| Setting                | Control                       | Default            | Notes                                                     |
| ---------------------- | ----------------------------- | ------------------ | --------------------------------------------------------- |
| **Capture source**     | Dropdown (monitors + windows) | Primary monitor    | Refreshed on open                                         |
| **Stream framerate**   | 60 (locked)                   | 60                 | Always 60fps for WHIP - non-negotiable for smooth viewing |
| **Stream quality**     | Slider: Light / Medium        | Light (2000k)      | Lower = more stable WebRTC                                |
| **Replay framerate**   | 60 (locked)                   | 60                 | Minimum 60fps enforced                                    |
| **Replay resolution**  | 720p / 1080p / Native         | Native             | Minimum 720p enforced                                     |
| **Replay compression** | Light / Balanced / Heavy      | Balanced           | Shows estimated GB/hour                                   |
| **Replay folder**      | Folder picker                 | ~/Videos/Momentum/ |                                                           |
| **Encoder**            | Auto / NVENC / QSV / Software | Auto               | Greyed-out options if not available                       |

---

## Dependencies

### Rust (Cargo.toml additions)

```toml
[dependencies]
cpal = "0.15"          # WASAPI audio capture
bytemuck = "1"         # Safe cast f32 slices to bytes
dirs = "5"             # OS-standard directories (Videos, AppData, etc.)

[target.'cfg(windows)'.dependencies]
# Windows Graphics Capture (WGC) for game-only / single-window capture.
# Pulls in Graphics.Capture, Graphics.DirectX.Direct3D11, Win32.Graphics.Direct3D11.
windows = { version = "0.58", features = [
  "Graphics_Capture",
  "Graphics_DirectX_Direct3D11",
  "Win32_Graphics_Direct3D11",
  "Win32_Graphics_Dxgi",
  "Win32_System_WinRT_Graphics_Capture",
  "Win32_Foundation",
] }
```

### ffmpeg binary

- Source: [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds) (GPL builds include all codecs)
- Required build flags: `--enable-libx264 --enable-libopus`
- Size: ~80MB uncompressed, ~30MB compressed in the installer
- The binary is NOT compiled from source - it's a pre-built static binary bundled as a Tauri sidecar

---

## Migration from v1

| Component       | v1 (current)                       | v2 (this)                                          |
| --------------- | ---------------------------------- | -------------------------------------------------- |
| Screen capture  | `getDisplayMedia` in webview       | `ddagrab` (monitor) / **WGC** (window) via ffmpeg  |
| Audio capture   | Browser audio track                | WASAPI loopback via cpal                           |
| Live stream     | WhipClient (TypeScript WebRTC)     | ffmpeg `-f whip`         |
| Local recording | ❌ none                            | ffmpeg MP4 output        |
| Stream control  | Frontend → Tauri IPC (notify only) | Rust spawns/kills ffmpeg |
| Frontend role   | Capture + WebRTC + UI              | UI only                  |

### What gets deleted

- `src/stream/whip.ts`
- `getDisplayMedia` calls in StreamSetup
- WhipClient references in components

### What stays the same

- `StreamHandle` trait interface
- Backend WS events (`stream_live`, etc.)
- SSE to website viewers (they still consume WHEP from MediaMTX)
- MediaMTX configuration (WHIP ingest → WHEP playback)

---

## Risks & Mitigations

| Risk                                         | Mitigation                                                 |
| -------------------------------------------- | ---------------------------------------------------------- |
| ffmpeg sidecar increases app size (+80MB)    | Compress with UPX, or use ffmpeg-light custom build        |
| ddagrab not available on old Windows         | Fallback to gdigrab (detect at runtime)                    |
| WGC unavailable (< Win10 1903) for window capture | Fall back to monitor capture (`ddagrab`), not gdigrab — more robust for games |
| Exclusive-fullscreen game won't window-capture cleanly | Recommend Borderless-Fullscreen in the UI; WGC handles it, monitor capture sidesteps it |
| WASAPI loopback fails (no output device)     | Detect and warn user, allow mic-only fallback              |
| MP4 incomplete if crash/force-kill           | Post-process with `ffmpeg -i broken.mp4 -c copy fixed.mp4` |
| WHIP handshake fails                         | Retry logic with exponential backoff, surface error to UI  |
| User has no GPU (software encoding too slow) | Reduce resolution/framerate automatically, warn user       |

---

## Implementation Order

1. **Settings module** (`settings/`) - schema, load/save, defaults, Tauri commands (needed by everything else)
2. **Audio capture module** (`audio_capture.rs`) - cpal WASAPI loopback → named pipe
3. **Window capture module** (`window_capture.rs`) - WGC → raw BGRA → named pipe (Windows-only). Start with monitor (`ddagrab`) working end-to-end first, then add WGC window capture.
4. **Pipeline builder** (`pipeline.rs`) - construct ffmpeg args from StreamSettings, branching on `CaptureTarget` (ddagrab vs `-f rawvideo` pipe)
5. **Hardware detection** (`hardware_detect.rs`) - probe available encoders
6. **FfmpegStreamHandle** (`ffmpeg.rs`) - spawn, monitor, stop (owns the WGC capture thread when window-targeted)
7. **Bundle ffmpeg binary** - add to Tauri sidecar config
8. **Frontend settings store + UI** - Zustand store, SettingsModal with tabs; capture-source dropdown + Borderless-Fullscreen tip
9. **Simplify StreamSetup** - remove getDisplayMedia, WhipClient; wire to settings
10. **Integration test** - verify WHIP connects to MediaMTX + MP4 is valid (both monitor and window targets)
