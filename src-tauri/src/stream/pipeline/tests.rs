use super::*;

#[test]
fn scale_tail_caps_both_dimensions_to_the_requested_box() {
    assert_eq!(
        scale_tail(1080),
        "scale='min(iw,1920)':'min(ih,1080)':force_original_aspect_ratio=decrease:force_divisible_by=2:flags=bilinear,format=yuv420p"
    );
    assert_eq!(
        scale_tail(720),
        "scale='min(iw,1280)':'min(ih,720)':force_original_aspect_ratio=decrease:force_divisible_by=2:flags=bilinear,format=yuv420p"
    );
}

#[test]
fn scale_tail_never_upscales_and_stays_even() {
    let tail = scale_tail(1080);
    assert!(tail.contains("force_original_aspect_ratio=decrease"));
    assert!(tail.contains("force_divisible_by=2"));
    assert!(tail.contains("min(iw,1920)"));
    assert!(tail.contains("min(ih,1080)"));
}

#[test]
fn amf_live_args_use_lowlatency_and_avoid_the_broken_ultralowlatency_preset() {
    let args = live_encoder_args(Encoder::Amf, 60, 6000);
    assert!(
        !args.contains(&"ultralowlatency".to_string()),
        "ultralowlatency skips periodic IDRs, leaving late WHEP joiners on a black screen"
    );
    assert!(!args.contains(&"-header_spacing".to_string()));
    assert!(!args.contains(&"-async_depth".to_string()));
    let usage_at = args.iter().position(|a| a == "-usage");
    assert_eq!(usage_at.and_then(|i| args.get(i + 1)), Some(&"lowlatency".to_string()));
    let idr_at = args.iter().position(|a| a == "-forced_idr");
    assert_eq!(idr_at.and_then(|i| args.get(i + 1)), Some(&"1".to_string()));
    let profile_at = args.iter().position(|a| a == "-profile:v");
    assert_eq!(profile_at.and_then(|i| args.get(i + 1)), Some(&"main".to_string()));
}

#[test]
fn x264_and_nvenc_live_profile_is_unchanged() {
    assert_eq!(
        live_encoder_args(Encoder::X264, 60, 6000)
            .windows(2)
            .find(|w| w[0] == "-profile:v")
            .map(|w| w[1].as_str()),
        Some("baseline")
    );
    assert!(!live_encoder_args(Encoder::Nvenc, 60, 6000).contains(&"-forced_idr".to_string()));
}

fn test_settings() -> StreamSettings {
    StreamSettings {
        source: CaptureSource::Monitor { index: 0 },
        bitrate_kbps: 3000,
        framerate: 60,
        resolution: 720,
    }
}

#[test]
fn amf_whip_output_forces_periodic_key_frames_but_other_encoders_dont() {
    let preview_path = std::path::Path::new("live_preview.jpg");
    let amf_args = build_args(
        &test_settings(),
        "https://example.invalid/whip",
        &AudioSource::Silent,
        None,
        None,
        Encoder::Amf,
        Some(preview_path),
    )
    .expect("valid args");
    assert!(
        amf_args.windows(2).any(|w| w[0] == "-force_key_frames"),
        "ffmpeg only submits AV_PICTURE_TYPE_I frames (which forced_idr needs) via -force_key_frames"
    );

    let x264_args = build_args(
        &test_settings(),
        "https://example.invalid/whip",
        &AudioSource::Silent,
        None,
        None,
        Encoder::X264,
        Some(preview_path),
    )
    .expect("valid args");
    assert!(!x264_args.contains(&"-force_key_frames".to_string()));
}

#[test]
fn live_preview_output_is_only_added_when_debug_is_enabled() {
    let with_debug = build_args(
        &test_settings(),
        "https://example.invalid/whip",
        &AudioSource::Silent,
        None,
        None,
        Encoder::X264,
        Some(std::path::Path::new("live_preview.jpg")),
    )
    .expect("valid args");
    assert!(with_debug.contains(&"image2".to_string()));

    let without_debug = build_args(
        &test_settings(),
        "https://example.invalid/whip",
        &AudioSource::Silent,
        None,
        None,
        Encoder::X264,
        None,
    )
    .expect("valid args");
    assert!(!without_debug.contains(&"image2".to_string()));
}
