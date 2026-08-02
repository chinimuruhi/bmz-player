use super::*;

#[test]
fn mine_hit_emits_event_with_damage() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8.0);
    let mut engine = JudgeEngine::new(windows());

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_000_000)));

    assert_eq!(outcome.mine_hits.len(), 1);
    assert_eq!(outcome.mine_hits[0].damage, 8.0);
    assert_eq!(outcome.mine_hits[0].note_id, NoteId(7));
    // Mine ヒットは通常判定とは別ベクタに入る。スコア対象ノーツが無いので
    // events は空、consumed_input も false のまま。
    assert!(outcome.events.is_empty());
    assert!(!outcome.consumed_input);
}

#[test]
fn mine_does_not_hit_outside_window() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8.0);
    let mut engine = JudgeEngine::new(windows());

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_100_000)));
    assert!(outcome.mine_hits.is_empty());
}

#[test]
fn mine_hit_does_not_double_fire() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8.0);
    let mut engine = JudgeEngine::new(windows());

    let first = engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    let second = engine.process_input(&chart, press_at(TimeUs(1_000_000)));

    assert_eq!(first.mine_hits.len(), 1);
    assert!(second.mine_hits.is_empty(), "same Mine must not fire twice");
}

#[test]
fn mine_pass_hits_when_lane_is_held() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8.0);
    let mut engine = JudgeEngine::new(windows());
    let mut lane_keyon_started_at = [None; LANE_COUNT];
    lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(900_000));

    let outcome = engine.process_mine_passes(&chart, TimeUs(1_000_000), &lane_keyon_started_at);

    assert_eq!(outcome.mine_hits.len(), 1);
    assert_eq!(outcome.mine_hits[0].note_id, NoteId(7));
    assert_eq!(outcome.mine_hits[0].damage, 8.0);
}

#[test]
fn mine_pass_without_pressed_lane_is_skipped() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8.0);
    let mut engine = JudgeEngine::new(windows());
    let lane_keyon_started_at = [None; LANE_COUNT];

    let outcome = engine.process_mine_passes(&chart, TimeUs(1_000_000), &lane_keyon_started_at);

    assert!(outcome.mine_hits.is_empty());
}

#[test]
fn mine_pass_ignores_key_pressed_after_mine_time() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8.0);
    let mut engine = JudgeEngine::new(windows());
    let mut lane_keyon_started_at = [None; LANE_COUNT];
    lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(1_050_000));

    let outcome = engine.process_mine_passes(&chart, TimeUs(1_100_000), &lane_keyon_started_at);

    assert!(outcome.mine_hits.is_empty());
}

#[test]
fn mine_does_not_hit_after_it_already_passed_unpressed() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8.0);
    let mut engine = JudgeEngine::new(windows());
    let lane_keyon_started_at = [None; LANE_COUNT];
    engine.process_mine_passes(&chart, TimeUs(1_000_000), &lane_keyon_started_at);

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_100_000)));

    assert!(outcome.mine_hits.is_empty());
}

#[test]
fn miss_is_reported_after_bad_window() {
    let chart = chart_with_tap(TimeUs(1_000_000));
    let mut engine = JudgeEngine::new(windows());

    let still_candidate = engine.process_misses(&chart, TimeUs(1_110_000));
    let missed = engine.process_misses(&chart, TimeUs(1_130_000));

    assert!(still_candidate.events.is_empty());
    assert_eq!(missed.events.len(), 1);
    assert_eq!(missed.events[0].judge, Judge::Poor);
    assert_eq!(missed.events[0].side, TimingSide::Slow);
    assert_eq!(missed.events[0].note_id, Some(NoteId(1)));
    assert_eq!(engine.lanes[Lane::Key1.index()].next_note_index, 1);
}
