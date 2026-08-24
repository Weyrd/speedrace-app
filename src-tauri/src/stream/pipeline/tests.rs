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
fn amf_live_args_force_idr_and_avoid_constrained_baseline() {
    let args = live_encoder_args(Encoder::Amf, 60, 6000);
    let idr_at = args.iter().position(|a| a == "-forced_idr");
    assert_eq!(
        idr_at.and_then(|i| args.get(i + 1)),
        Some(&"1".to_string()),
        "WHEP viewers joining mid-stream need a real IDR, not just a periodic I-frame"
    );
    let profile_at = args.iter().position(|a| a == "-profile:v");
    assert_eq!(
        profile_at.and_then(|i| args.get(i + 1)),
        Some(&"main".to_string()),
        "constrained_baseline + AMF's auto CABAC produces an SPS that lies about the bitstream"
    );
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
