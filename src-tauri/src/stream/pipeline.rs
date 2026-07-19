use super::{AudioSource, CaptureSource, StreamSettings};
use std::path::Path;

const AUDIO_FILTER: &str = "aresample=async=1:first_pts=0";

fn scale_tail(resolution: u32) -> String {
    let width = resolution.max(360) * 16 / 9 & !1;
    format!("scale={width}:-2:flags=bilinear,format=yuv420p")
}

pub const PREVIEW_FPS: u32 = 15;
const PREVIEW_TAIL: &str = "scale=640:-2:flags=bilinear,format=yuvj420p";

// Window capture
pub struct VideoPipe<'a> {
    pub path: &'a str,
    pub width: u32,
    pub height: u32,
}

fn video_filter(source: &CaptureSource, tail: &str) -> String {
    match source {
        CaptureSource::Monitor { .. } => format!("hwdownload,format=bgra,{tail}"),
        CaptureSource::Window { .. } => tail.to_string(),
    }
}

fn push_video_input(
    a: &mut Vec<String>,
    source: &CaptureSource,
    fps: u32,
    video_pipe: Option<&VideoPipe>,
) -> Result<(), String> {
    let mut push = |s: &str| a.push(s.to_string());
    match source {
        CaptureSource::Monitor { index } => {
            push("-f");
            push("lavfi");
            push("-i");
            push(&format!("ddagrab=output_idx={index}:framerate={fps}"));
        }
        CaptureSource::Window { .. } => {
            let pipe = video_pipe.ok_or("window capture needs a video pipe")?;
            push("-f");
            push("rawvideo");
            push("-pix_fmt");
            push("bgra");
            push("-video_size");
            push(&format!("{}x{}", pipe.width, pipe.height));
            push("-framerate");
            push(&fps.to_string());
            push("-thread_queue_size");
            push("64");
            push("-i");
            push(pipe.path);
        }
    }
    Ok(())
}

pub fn build_preview_args(
    source: &CaptureSource,
    video_pipe: Option<&VideoPipe>,
) -> Result<Vec<String>, String> {
    let mut a: Vec<String> = Vec::new();

    for s in ["-hide_banner", "-loglevel", "error", "-nostats"] {
        a.push(s.to_string());
    }

    push_video_input(&mut a, source, PREVIEW_FPS, video_pipe)?;

    let vf = video_filter(source, PREVIEW_TAIL);
    let mut push = |s: &str| a.push(s.to_string());
    push("-vf");
    push(&vf);
    push("-q:v");
    push("7");
    push("-r");
    push(&PREVIEW_FPS.to_string());
    push("-f");
    push("mpjpeg");
    push("pipe:1");

    Ok(a)
}

pub fn build_args(
    settings: &StreamSettings,
    whip_url: &str,
    audio: &AudioSource,
    replay_path: Option<&Path>,
    video_pipe: Option<&VideoPipe>,
) -> Result<Vec<String>, String> {
    let fps = settings.framerate.max(1);
    let kbps = settings.bitrate_kbps.max(500);
    let mut a: Vec<String> = Vec::new();

    for s in [
        "-hide_banner",
        "-loglevel",
        "info",
        "-nostats",
        "-progress",
        "pipe:1",
        "-stats_period",
        "1",
    ] {
        a.push(s.to_string());
    }

    push_video_input(&mut a, &settings.source, fps, video_pipe)?;

    let vf = video_filter(&settings.source, &scale_tail(settings.resolution));
    let mut push = |s: &str| a.push(s.to_string());

    // Audio input
    match audio {
        #[cfg(windows)]
        AudioSource::Pipe(path) => {
            push("-thread_queue_size");
            push("512");
            push("-f");
            push("f32le");
            push("-ar");
            push("48000");
            push("-ac");
            push("2");
            push("-channel_layout");
            push("stereo");
            push("-i");
            push(path);
        }
        AudioSource::Silent => {
            push("-f");
            push("lavfi");
            push("-i");
            push("anullsrc=r=48000:cl=stereo");
        }
    }

    let (vmap, amap) = if replay_path.is_some() {
        push("-filter_complex");
        push(&format!(
            "[0:v]{vf},split=2[vw][vr];[1:a]{AUDIO_FILTER},asplit=2[aw][ar]"
        ));
        ("[vw]", "[aw]")
    } else {
        ("0:v", "1:a")
    };

    // Output 1 - WHIP (live)
    push("-map");
    push(vmap);
    push("-map");
    push(amap);
    if replay_path.is_none() {
        push("-vf");
        push(&vf);
        push("-af");
        push(AUDIO_FILTER);
    }
    push("-c:v");
    push("libx264");
    push("-preset");
    push("veryfast");
    push("-tune");
    push("zerolatency");
    push("-profile:v");
    push("baseline");
    push("-bf");
    push("0");
    push("-g");
    push(&(2 * fps).to_string());
    push("-r");
    push(&fps.to_string());
    push("-b:v");
    push(&format!("{kbps}k"));
    push("-maxrate");
    push(&format!("{}k", kbps * 5 / 4));
    push("-bufsize");
    push(&format!("{}k", kbps * 2));
    push("-c:a");
    push("libopus");
    push("-b:a");
    push("96k");
    push("-ar");
    push("48000");
    push("-ac");
    push("2");
    push("-ts_buffer_size");
    push("4194304");
    push("-f");
    push("whip");
    push(whip_url);

    // MP4 replay VOD
    if let Some(path) = replay_path {
        push("-map");
        push("[vr]");
        push("-map");
        push("[ar]");
        push("-c:v");
        push("libx264");
        push("-preset");
        push("veryfast");
        push("-profile:v");
        push("high");
        push("-pix_fmt");
        push("yuv420p");
        push("-g");
        push(&(2 * fps).to_string());
        push("-r");
        push(&fps.to_string());
        push("-b:v");
        push(&format!("{kbps}k"));
        push("-c:a");
        push("aac");
        push("-b:a");
        push("160k");
        push("-ar");
        push("48000");
        push("-ac");
        push("2");
        push("-movflags");
        push("+frag_keyframe+empty_moov");
        push(&path.to_string_lossy());
    }

    Ok(a)
}
