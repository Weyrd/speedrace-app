// App-side early-start detection: the three scenarios we discussed all reduce to computing the
// absolute instant a run began, from which the back derives head_start = race_start_at − run_start.

use momentum_app_lib::early_start::{parse_livesplit_time_ms, run_start_from_elapsed};

fn head_start(race_start_ms: i64, run_start_ms: i64) -> i64 {
    (race_start_ms - run_start_ms).max(0)
}

const GUN: i64 = 100_000; // shared-clock instant the race clock fires

#[test]
fn parses_livesplit_time_formats() {
    assert_eq!(parse_livesplit_time_ms("0.00"), Some(0));
    assert_eq!(parse_livesplit_time_ms("1.50"), Some(1_500));
    assert_eq!(parse_livesplit_time_ms("0:30.00"), Some(30_000));
    assert_eq!(parse_livesplit_time_ms("1:23.45"), Some(83_450));
    assert_eq!(parse_livesplit_time_ms("1:02:03.45"), Some(3_723_450));
    assert_eq!(parse_livesplit_time_ms("-"), None, "no current time");
    assert_eq!(parse_livesplit_time_ms(""), None);
    assert_eq!(parse_livesplit_time_ms("nope"), None);
}

#[test]
fn scenario_started_early_during_countdown() {
    // Triggers the game start 1.5s before the gun; at the gun the timer reads 1.50s.
    let elapsed = parse_livesplit_time_ms("1.50").unwrap();
    let run_start = run_start_from_elapsed(GUN, elapsed);
    assert_eq!(head_start(GUN, run_start), 1_500, "1.5s early -> penalized");
}

#[test]
fn scenario_already_started_while_waiting() {
    // Timer already running in WaitingForStart; sampled 3s before the gun it reads 4s elapsed,
    // so the run truly began 7s before the gun.
    let sampled_at = GUN - 3_000;
    let elapsed = parse_livesplit_time_ms("4.00").unwrap();
    let run_start = run_start_from_elapsed(sampled_at, elapsed);
    assert_eq!(head_start(GUN, run_start), 7_000, "back-dating survives sampling in the lobby");
}

#[test]
fn scenario_game_already_running_when_app_starts() {
    // LiveSplit connects mid-run 1s before the gun and the timer already reads 30s.
    let sampled_at = GUN - 1_000;
    let elapsed = parse_livesplit_time_ms("0:30.00").unwrap();
    let run_start = run_start_from_elapsed(sampled_at, elapsed);
    assert_eq!(head_start(GUN, run_start), 31_000, "a mid-run attach is caught and ruinous");
}

#[test]
fn scenario_clean_start_at_the_gun() {
    // Started at the gun; a poll ~120ms later reads 0.12s -> run_start back to the gun -> no head start.
    let sampled_at = GUN + 120;
    let elapsed = parse_livesplit_time_ms("0.12").unwrap();
    let run_start = run_start_from_elapsed(sampled_at, elapsed);
    assert_eq!(head_start(GUN, run_start), 0, "poll delay is compensated by elapsed");
}
