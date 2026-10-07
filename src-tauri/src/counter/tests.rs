use super::*;

fn sample(value: i64, split_index: Option<u32>, at_ms: u64) -> CounterSample {
    CounterSample {
        value,
        split_index,
        at_ms,
    }
}

fn cfg(enabled: bool, mode: CounterMode, cadence: CounterCadence) -> CounterConfig {
    CounterConfig {
        counter_name: "deaths".into(),
        enabled,
        mode,
        cadence,
        label: None,
        icon: None,
        display_order: 0,
    }
}

#[test]
fn total_collapses_to_latest() {
    let mut buf = CounterBuffer::for_mode(CounterMode::Total);
    buf.record(sample(1, Some(0), 10));
    buf.record(sample(2, Some(0), 20));
    buf.record(sample(3, Some(1), 30));
    let drained = buf.drain();
    assert_eq!(drained, vec![sample(3, Some(1), 30)]);
    assert!(buf.drain().is_empty());
}

#[test]
fn per_split_keeps_latest_per_split() {
    let mut buf = CounterBuffer::for_mode(CounterMode::PerSplit);
    buf.record(sample(5, Some(0), 10));
    buf.record(sample(8, Some(0), 20));
    buf.record(sample(12, Some(1), 30));
    let drained = buf.drain();
    assert_eq!(
        drained,
        vec![sample(8, Some(0), 20), sample(12, Some(1), 30)]
    );
}

#[test]
fn per_split_none_degrades_to_latest() {
    let mut buf = CounterBuffer::for_mode(CounterMode::PerSplit);
    buf.record(sample(1, None, 10));
    buf.record(sample(2, None, 20));
    assert_eq!(buf.drain(), vec![sample(2, None, 20)]);
}

#[test]
fn timeline_preserves_order() {
    let mut buf = CounterBuffer::for_mode(CounterMode::Timeline);
    buf.record(sample(1, Some(0), 10));
    buf.record(sample(2, Some(0), 20));
    buf.record(sample(3, Some(1), 30));
    let drained = buf.drain();
    assert_eq!(
        drained,
        vec![
            sample(1, Some(0), 10),
            sample(2, Some(0), 20),
            sample(3, Some(1), 30)
        ]
    );
    assert!(buf.drain().is_empty());
}

#[test]
fn action_no_config_buffers_total() {
    assert_eq!(
        resolve_action(None),
        CounterAction::Buffer(CounterMode::Total)
    );
}

#[test]
fn unknown_counter_accumulates_and_drains_to_latest_total() {
    let CounterAction::Buffer(mode) = resolve_action(None) else {
        panic!("unknown counter should buffer, not post per-event");
    };
    let mut buf = CounterBuffer::for_mode(mode);
    buf.record(sample(1, Some(0), 10));
    buf.record(sample(2, Some(0), 20));
    buf.record(sample(3, Some(1), 30));
    assert_eq!(buf.drain(), vec![sample(3, Some(1), 30)]);
    assert!(buf.drain().is_empty());
}

#[test]
fn action_disabled_drops() {
    let c = cfg(false, CounterMode::Timeline, CounterCadence::Instant);
    assert_eq!(resolve_action(Some(&c)), CounterAction::Drop);
}

#[test]
fn action_instant_posts() {
    let c = cfg(true, CounterMode::Timeline, CounterCadence::Instant);
    assert_eq!(resolve_action(Some(&c)), CounterAction::Post);
}

#[test]
fn action_buffered_reports_mode() {
    let c = cfg(true, CounterMode::PerSplit, CounterCadence::PerSplit);
    assert_eq!(
        resolve_action(Some(&c)),
        CounterAction::Buffer(CounterMode::PerSplit)
    );
    let c = cfg(true, CounterMode::Timeline, CounterCadence::EndOnly);
    assert_eq!(
        resolve_action(Some(&c)),
        CounterAction::Buffer(CounterMode::Timeline)
    );
}
