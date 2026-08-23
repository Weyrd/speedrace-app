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
    assert_eq!(block.out_time_us, Some(1_000_000));
}

#[test]
fn parser_resets_between_blocks() {
    let mut p = ProgressParser::default();
    p.feed("frame=1");
    p.feed("out_time_us=500000");
    p.feed("progress=continue");
    p.feed("fps=60.0");
    let block = p.feed("progress=continue").expect("block should complete");
    assert_eq!(block.frame, None);
    assert_eq!(block.out_time_us, None);
}

#[test]
fn realtime_speed_from_healthy_deltas_ignores_the_cumulative_speed_field() {
    let mut w = RealtimeWatchdog::default();
    let t0 = Instant::now();
    assert!(w.observe(t0, 0).is_none());
    let t1 = t0 + Duration::from_secs(1);
    let speed = w
        .observe(t1, 950_000)
        .expect("second sample yields a speed");
    assert!(
        speed > 0.9 && speed < 1.05,
        "expected a near-realtime speed, got {speed}"
    );
}

#[test]
fn realtime_speed_flags_an_encoder_that_cannot_keep_up() {
    let mut w = RealtimeWatchdog::default();
    let t0 = Instant::now();
    w.observe(t0, 0);
    let t1 = t0 + Duration::from_secs(1);
    let speed = w
        .observe(t1, 194_000)
        .expect("second sample yields a speed");
    assert!(speed < MIN_REALTIME_SPEED, "expected a slow speed, got {speed}");
}

#[test]
fn a_healthy_machine_never_dips_below_the_slow_threshold() {
    let mut w = RealtimeWatchdog::default();
    let mut t = Instant::now();
    w.observe(t, 0);
    for out_time_us in (960_000..=4_800_000).step_by(960_000) {
        t += Duration::from_secs(1);
        let speed = w.observe(t, out_time_us).expect("a speed each second");
        assert!(
            speed >= MIN_REALTIME_SPEED,
            "0.96x sustained should never be flagged as slow, got {speed}"
        );
    }
}

#[test]
fn a_genuinely_overloaded_machine_stays_flagged() {
    let mut w = RealtimeWatchdog::default();
    let mut t = Instant::now();
    w.observe(t, 0);
    for out_time_us in (350_000..=1_750_000).step_by(350_000) {
        t += Duration::from_secs(1);
        let speed = w.observe(t, out_time_us).expect("a speed each second");
        assert!(
            speed < MIN_REALTIME_SPEED,
            "0.35x sustained should stay flagged as slow, got {speed}"
        );
    }
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
