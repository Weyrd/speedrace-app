use super::*;

fn buffered(i: u32) -> BufferedEarlySplit {
    BufferedEarlySplit {
        lobby_id: "l".into(),
        split_index: i,
        segment_name: "s".into(),
        is_final: false,
    }
}

#[test]
fn reset_clears_run_start_so_a_restart_can_re_arm() {
    let mut g = GlobalState::new();
    g.run_start_instant = Some(1);
    g.run_active = true;
    reset_run_start(&mut g);
    assert!(g.run_start_instant.is_none());
    assert!(!g.run_active);
}

#[test]
fn reset_rolls_back_split_index_for_dropped_early_splits_only() {
    let mut g = GlobalState::new();
    g.current_split_index = 3;
    g.pending_early_splits = vec![buffered(1), buffered(2)];
    reset_run_start(&mut g);
    assert_eq!(g.current_split_index, 1);
    assert!(g.pending_early_splits.is_empty());
}

#[test]
fn reset_keeps_split_index_when_splits_were_already_sent() {
    let mut g = GlobalState::new();
    g.current_split_index = 2;
    reset_run_start(&mut g);
    assert_eq!(g.current_split_index, 2);
}
