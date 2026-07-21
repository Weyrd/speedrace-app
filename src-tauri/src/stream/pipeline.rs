use super::{AudioSource, CaptureSource, Encoder, ReplayRun, StreamSettings};

const AUDIO_FILTER: &str = "aresample=async=1:first_pts=0";

fn owned(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn live_encoder_args(enc: Encoder, fps: u32, kbps: u32) -> Vec<String> {
    let mut a = vec!["-c:v".into(), enc.name().to_string()];
    a.extend(match enc {
        Encoder::X264 => owned(&["-preset", "veryfast", "-tune", "zerolatency"]),
        Encoder::Nvenc => owned(&[
            "-preset",
            "p1",
            "-tune",
            "ull",
            "-rc",
            "cbr",
            "-zerolatency",
            "1",
            "-delay",
            "0",
        ]),
        Encoder::Amf => owned(&[
            "-usage",
            "ultralowlatency",
            "-quality",
            "speed",
            "-rc",
            "cbr",
        ]),
    });
    a.extend(owned(&[
        "-profile:v",
        match enc {
            Encoder::Amf => "constrained_baseline",
            _ => "baseline",
        },
        "-bf",
        "0",
    ]));
    let (maxrate, bufsize) = match enc {
        Encoder::X264 => (kbps * 5 / 4, kbps * 2),
        _ => (kbps, kbps),
    };
    a.extend(owned(&[
        "-g",
        &(2 * fps).to_string(),
        "-r",
        &fps.to_string(),
        "-b:v",
        &format!("{kbps}k"),
        "-maxrate",
        &format!("{maxrate}k"),
        "-bufsize",
        &format!("{bufsize}k"),
    ]));
    a
}

pub(crate) fn replay_encoder_args(enc: Encoder, fps: u32, kbps: u32) -> Vec<String> {
    let mut a = vec!["-c:v".into(), enc.name().to_string()];
    a.extend(match enc {
        Encoder::X264 => owned(&["-preset", "veryfast"]),
        Encoder::Nvenc => owned(&[
            "-preset",
            "p5",
            "-tune",
            "hq",
            "-rc",
            "vbr",
            "-maxrate",
            &format!("{}k", kbps * 5 / 4),
            "-bufsize",
            &format!("{}k", kbps * 2),
        ]),
        Encoder::Amf => owned(&[
            "-usage",
            "transcoding",
            "-quality",
            "quality",
            "-rc",
            "vbr_peak",
            "-maxrate",
            &format!("{}k", kbps * 5 / 4),
            "-bufsize",
            &format!("{}k", kbps * 2),
        ]),
    });
    a.extend(owned(&[
        "-profile:v",
        "high",
        "-pix_fmt",
        "yuv420p",
        "-g",
        &(2 * fps).to_string(),
        "-r",
        &fps.to_string(),
        "-b:v",
        &format!("{kbps}k"),
    ]));
    a
}

fn scale_tail(resolution: u32) -> String {
    let width = (resolution.max(360) * 16 / 9) & !1;
    format!("scale={width}:-2:flags=bilinear,format=yuv420p")
}

pub const PREVIEW_FPS: u32 = 15;
const PREVIEW_TAIL: &str = "scale=640:-2:flags=bilinear,format=yuvj420p";

// Rust WGC capture (window or monitor)
pub struct VideoPipe<'a> {
    pub path: &'a str,
    pub width: u32,
    pub height: u32,
}

fn video_filter(piped: bool, tail: &str) -> String {
    if piped {
        tail.to_string()
    } else {
        format!("hwdownload,format=bgra,{tail}")
    }
}

fn push_video_input(
    a: &mut Vec<String>,
    source: &CaptureSource,
    fps: u32,
    video_pipe: Option<&VideoPipe>,
) -> Result<(), String> {
    let mut push = |s: &str| a.push(s.to_string());
    if let Some(pipe) = video_pipe {
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
        return Ok(());
    }
    match source {
        CaptureSource::Monitor { index } => {
            push("-f");
            push("lavfi");
            push("-i");
            push(&format!("ddagrab=output_idx={index}:framerate={fps}"));
            Ok(())
        }
        CaptureSource::Window { .. } => Err("window capture needs a video pipe".into()),
    }
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

    let vf = video_filter(video_pipe.is_some(), PREVIEW_TAIL);
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
    replay: Option<&ReplayRun>,
    video_pipe: Option<&VideoPipe>,
    encoder: Encoder,
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

    let vf = video_filter(video_pipe.is_some(), &scale_tail(settings.resolution));
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

    let (vmap, amap) = if replay.is_some() {
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
    if replay.is_none() {
        push("-vf");
        push(&vf);
        push("-af");
        push(AUDIO_FILTER);
    }
    for s in live_encoder_args(encoder, fps, kbps) {
        push(&s);
    }
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
    if let Some(run) = replay {
        push("-map");
        push("[vr]");
        push("-map");
        push("[ar]");
        for s in replay_encoder_args(encoder, fps, kbps) {
            push(&s);
        }
        push("-c:a");
        push("aac");
        push("-b:a");
        push("160k");
        push("-ar");
        push("48000");
        push("-ac");
        push("2");
        push("-f");
        push("segment");
        push("-segment_time");
        push(&super::SEGMENT_SECS.to_string());
        push("-segment_format");
        push("mp4");
        push("-segment_format_options");
        push("movflags=+frag_keyframe+empty_moov");
        push("-reset_timestamps");
        push("1");
        push("-segment_list");
        push(&run.list.to_string_lossy());
        push("-segment_list_type");
        push("csv");
        push("-segment_list_flags");
        push("+live");
        push(&run.pattern.to_string_lossy());
    }

    Ok(a)
}
