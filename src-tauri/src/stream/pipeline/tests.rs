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
