use super::*;

#[test]
fn parses_block_and_ignores_unknown_fields() {
    let mut p = ProgressParser::default();
    assert!(p.feed("frame=42").is_none());
    assert!(p.feed("fps=30.0").is_none());
    assert!(p.feed("bitrate=N/A").is_none());
    assert!(p.feed("total_size=N/A").is_none());
    assert!(p.feed("out_time_us=1000000").is_none());
    assert!(p.feed("speed=0.961x").is_none());
    let block = p.feed("progress=continue").expect("block should complete");
    assert_eq!(block.frame, Some(42));
}

#[test]
fn parser_resets_between_blocks() {
    let mut p = ProgressParser::default();
    p.feed("frame=1");
    p.feed("progress=continue");
    p.feed("fps=60.0");
    let block = p.feed("progress=continue").expect("block should complete");
    assert_eq!(block.frame, None);
}

#[test]
fn hw_encoder_failed_ignores_unrelated_death_causes() {
    let tail = vec![
        "Connection timed out".to_string(),
        "WHIP muxer: udp connect failed".to_string(),
    ];
    assert!(!hw_encoder_failed(&tail));
}

#[test]
fn hw_encoder_failed_recognizes_a_real_hardware_failure() {
    let tail = vec!["[h264_nvenc] No capable devices found".to_string()];
    assert!(hw_encoder_failed(&tail));
}
