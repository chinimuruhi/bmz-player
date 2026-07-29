use bmz_chart::model::{ChartMetadata, LongNotePair, LongNoteStyle, SoundAssetRef, SoundEvent};
use bmz_core::chart::ChartIdentity;
use bmz_core::input::InputSource;

use super::*;

fn windows() -> JudgeWindow {
    JudgeWindow::symmetric(16_000, 40_000, 80_000, 120_000, 500_000, 200_000, 16_000)
}

fn chart_with_tap(time: TimeUs) -> PlayableChart {
    chart_with_lane_tap(Lane::Key1, time)
}

fn chart_with_lane_tap(lane: Lane, time: TimeUs) -> PlayableChart {
    let note = NoteEvent {
        id: NoteId(1),
        lane,
        kind: NoteKind::Tap,
        tick: Default::default(),
        time,
        sound: None,
        damage: None,
    };
    let mut lane_notes = std::array::from_fn(|_| Vec::new());
    lane_notes[lane.index()].push(note);

    PlayableChart {
        identity: ChartIdentity { file_md5: [0; 16], file_sha256: [0; 32] },
        metadata: ChartMetadata::default(),
        lane_notes,
        long_notes: Vec::new(),
        bgm_events: Vec::<SoundEvent>::new(),
        bga_events: Vec::new(),
        timing_events: Vec::new(),
        scroll_events: Vec::new(),
        speed_events: Vec::new(),
        judge_rank_events: Vec::new(),
        bgm_volume_events: Vec::new(),
        key_volume_events: Vec::new(),
        text_events: Vec::new(),
        bga_opacity_events: Vec::new(),
        bga_argb_events: Vec::new(),
        swbga_definitions: Vec::new(),
        bga_keybound_events: Vec::new(),
        bga_asset_by_bmp_key: std::collections::HashMap::new(),
        bar_lines: Vec::new(),
        sounds: Vec::<SoundAssetRef>::new(),
        bga_assets: Vec::new(),
        total_notes: 1,
        end_time: time,
    }
}

fn chart_with_two_taps(first_time: TimeUs, second_time: TimeUs) -> PlayableChart {
    let lane = Lane::Key1;
    let first = NoteEvent {
        id: NoteId(1),
        lane,
        kind: NoteKind::Tap,
        tick: Default::default(),
        time: first_time,
        sound: None,
        damage: None,
    };
    let second = NoteEvent {
        id: NoteId(2),
        lane,
        kind: NoteKind::Tap,
        tick: Default::default(),
        time: second_time,
        sound: None,
        damage: None,
    };
    let mut chart = chart_with_tap(first_time);
    chart.lane_notes[lane.index()] = vec![first, second];
    chart.total_notes = 2;
    chart.end_time = second_time;
    chart
}

fn chart_with_long_start(time: TimeUs, end_time: TimeUs) -> PlayableChart {
    chart_with_lane_long_start(Lane::Key1, time, end_time)
}

fn chart_with_lane_long_start(lane: Lane, time: TimeUs, end_time: TimeUs) -> PlayableChart {
    let start = NoteEvent {
        id: NoteId(1),
        lane,
        kind: NoteKind::LongStart,
        tick: Default::default(),
        time,
        sound: None,
        damage: None,
    };
    let end = NoteEvent {
        id: NoteId(2),
        lane,
        kind: NoteKind::LongEnd,
        tick: Default::default(),
        time: end_time,
        sound: None,
        damage: None,
    };
    let mut chart = chart_with_tap(time);
    chart.metadata.long_note_mode = LongNoteMode::Ln;
    chart.lane_notes[lane.index()] = vec![start, end];
    chart.long_notes = vec![LongNotePair {
        lane,
        style: LongNoteStyle::ChannelPair,
        mode: None,
        start_note_id: NoteId(1),
        end_note_id: NoteId(2),
        start_tick: Default::default(),
        end_tick: Default::default(),
        start_time: time,
        end_time,
        sound: None,
    }];
    chart
}

fn press_at(time: TimeUs) -> InputEvent {
    press_lane_at(Lane::Key1, time)
}

fn press_lane_at(lane: Lane, time: TimeUs) -> InputEvent {
    InputEvent {
        source: InputSource::Human,
        lane,
        kind: InputKind::Press,
        time,
        device_kind: bmz_core::input::InputDeviceKind::Keyboard,
        scratch_direction: None,
    }
}

fn release_at(time: TimeUs) -> InputEvent {
    release_lane_at(Lane::Key1, time)
}

fn release_lane_at(lane: Lane, time: TimeUs) -> InputEvent {
    InputEvent {
        source: InputSource::Human,
        lane,
        kind: InputKind::Release,
        time,
        device_kind: bmz_core::input::InputDeviceKind::Keyboard,
        scratch_direction: None,
    }
}

#[test]
fn normal_window_consumes_note() {
    let chart = chart_with_tap(TimeUs(1_000_000));
    let mut engine = JudgeEngine::new(windows());

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_030_000)));

    assert!(outcome.consumed_input);
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(outcome.events[0].judge, Judge::Great);
    assert_eq!(outcome.events[0].side, TimingSide::Slow);
    assert_eq!(outcome.events[0].note_id, Some(NoteId(1)));
    assert_eq!(engine.lanes[Lane::Key1.index()].next_note_index, 1);
}

#[test]
fn slow_empty_poor_does_not_consume_note() {
    let chart = chart_with_tap(TimeUs(1_000_000));
    let mut engine = JudgeEngine::new(windows());

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_150_000)));

    assert!(!outcome.consumed_input);
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(outcome.events[0].judge, Judge::EmptyPoor);
    assert_eq!(outcome.events[0].side, TimingSide::Slow);
    assert_eq!(outcome.events[0].note_id, None);
    assert_eq!(
        outcome.keysounds,
        vec![KeySoundEvent { note_id: NoteId(1), time: TimeUs(1_150_000) }]
    );
    assert_eq!(engine.lanes[Lane::Key1.index()].next_note_index, 0);
}

#[test]
fn fast_empty_poor_does_not_consume_note() {
    let chart = chart_with_tap(TimeUs(1_000_000));
    let mut engine = JudgeEngine::new(windows());

    let outcome = engine.process_input(&chart, press_at(TimeUs(700_000)));

    assert!(!outcome.consumed_input);
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(outcome.events[0].judge, Judge::EmptyPoor);
    assert_eq!(outcome.events[0].side, TimingSide::Fast);
    assert_eq!(outcome.events[0].note_id, None);
    assert_eq!(
        outcome.keysounds,
        vec![KeySoundEvent { note_id: NoteId(1), time: TimeUs(700_000) }]
    );
    assert_eq!(engine.lanes[Lane::Key1.index()].next_note_index, 0);
}

#[test]
fn outside_empty_poor_windows_is_unjudged() {
    let chart = chart_with_tap(TimeUs(1_000_000));
    let mut engine = JudgeEngine::new(windows());

    let too_late = engine.process_input(&chart, press_at(TimeUs(1_250_000)));
    let too_early = engine.process_input(&chart, press_at(TimeUs(400_000)));

    assert!(too_late.events.is_empty());
    assert!(!too_late.consumed_input);
    assert!(too_early.events.is_empty());
    assert!(!too_early.consumed_input);
    assert_eq!(engine.lanes[Lane::Key1.index()].next_note_index, 0);
}

#[test]
fn double_press_after_normal_judge_is_slow_empty_poor() {
    let chart = chart_with_tap(TimeUs(1_000_000));
    let mut engine = JudgeEngine::new(windows());

    let first = engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    let second = engine.process_input(&chart, press_at(TimeUs(1_005_000)));

    assert_eq!(first.events[0].judge, Judge::PGreat);
    assert_eq!(first.events[0].note_id, Some(NoteId(1)));
    assert!(!second.consumed_input);
    assert_eq!(second.events.len(), 1);
    assert_eq!(second.events[0].judge, Judge::EmptyPoor);
    assert_eq!(second.events[0].side, TimingSide::Slow);
    assert_eq!(second.events[0].note_id, None);
    assert_eq!(
        second.keysounds,
        vec![KeySoundEvent { note_id: NoteId(1), time: TimeUs(1_005_000) }]
    );
    assert_eq!(engine.lanes[Lane::Key1.index()].next_note_index, 1);
}

#[test]
fn beatoraja_7k_double_press_after_slow_empty_poor_window_is_unjudged() {
    let chart = chart_with_tap(TimeUs(1_000_000));
    let mut engine = JudgeEngine::new(
        crate::judge::window::beatoraja_note_judge_window_for_keymode(bmz_core::lane::KeyMode::K7),
    );

    let first = engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    let second = engine.process_input(&chart, press_at(TimeUs(1_151_000)));

    assert_eq!(first.events[0].judge, Judge::PGreat);
    assert!(second.events.is_empty());
    assert!(!second.consumed_input);
}

#[test]
fn beatoraja_7k_late_bad_is_not_missed_before_late_bad_end() {
    let chart = chart_with_tap(TimeUs(1_000_000));
    let mut engine = JudgeEngine::new(
        crate::judge::window::beatoraja_note_judge_window_for_keymode(bmz_core::lane::KeyMode::K7),
    );

    let missed = engine.process_misses(&chart, TimeUs(1_260_000));
    let outcome = engine.process_input(&chart, press_at(TimeUs(1_260_000)));

    assert!(missed.events.is_empty());
    assert!(outcome.consumed_input);
    assert_eq!(outcome.events[0].judge, Judge::Bad);
    assert_eq!(outcome.events[0].side, TimingSide::Slow);
    assert_eq!(outcome.events[0].delta, TimeUs(260_000));
}

#[test]
fn beatoraja_7k_early_beyond_bad_is_fast_empty_poor() {
    let chart = chart_with_tap(TimeUs(1_000_000));
    let mut engine = JudgeEngine::new(
        crate::judge::window::beatoraja_note_judge_window_for_keymode(bmz_core::lane::KeyMode::K7),
    );

    let outcome = engine.process_input(&chart, press_at(TimeUs(740_000)));

    assert!(!outcome.consumed_input);
    assert_eq!(outcome.events[0].judge, Judge::EmptyPoor);
    assert_eq!(outcome.events[0].side, TimingSide::Fast);
    assert_eq!(outcome.events[0].delta, TimeUs(-260_000));
}

#[test]
fn beatoraja_pms_bad_does_not_consume_and_can_be_rejudged() {
    let chart = chart_with_tap(TimeUs(1_000_000));
    let mut engine = JudgeEngine::new_with_window_set_algorithm_and_keymode(
        crate::judge::window::beatoraja_judge_windows_for_keymode(KeyMode::K9),
        RuleMode::Beatoraja,
        JudgeAlgorithm::Combo,
        KeyMode::K9,
    );

    let bad = engine.process_input(&chart, press_at(TimeUs(820_000)));

    assert!(bad.consumed_input);
    assert_eq!(bad.events[0].judge, Judge::Bad);
    assert_eq!(bad.events[0].note_id, Some(NoteId(1)));
    assert!(engine.bad_attempted_notes.contains(&NoteId(1)));
    assert_eq!(engine.judged_notes.get(&NoteId(1)), None);

    let great = engine.process_input(&chart, press_at(TimeUs(1_050_000)));

    assert!(!engine.bad_attempted_notes.contains(&NoteId(1)));
    assert_eq!(engine.judged_notes.get(&NoteId(1)), Some(&Judge::Great));

    assert!(great.consumed_input);
    assert_eq!(great.events[0].judge, Judge::Great);
    assert_eq!(great.events[0].note_id, Some(NoteId(1)));
    assert_eq!(engine.lanes[Lane::Key1.index()].next_note_index, 1);
}

#[test]
fn beatoraja_pms_bad_attempt_miss_consumes_without_extra_poor_event() {
    let chart = chart_with_tap(TimeUs(1_000_000));
    let mut engine = JudgeEngine::new_with_window_set_algorithm_and_keymode(
        crate::judge::window::beatoraja_judge_windows_for_keymode(KeyMode::K9),
        RuleMode::Beatoraja,
        JudgeAlgorithm::Combo,
        KeyMode::K9,
    );

    let bad = engine.process_input(&chart, press_at(TimeUs(820_000)));
    assert_eq!(bad.events[0].judge, Judge::Bad);
    assert!(engine.bad_attempted_notes.contains(&NoteId(1)));

    let missed = engine.process_misses(&chart, TimeUs(1_184_000));

    assert!(missed.events.is_empty());
    assert!(!engine.bad_attempted_notes.contains(&NoteId(1)));
    assert_eq!(engine.judged_notes.get(&NoteId(1)), Some(&Judge::Poor));
    assert_eq!(engine.lanes[Lane::Key1.index()].next_note_index, 1);
}

#[test]
fn dx_9key_bad_does_not_consume_and_can_be_rejudged() {
    let chart = chart_with_tap(TimeUs(1_000_000));
    let mut engine = JudgeEngine::new_with_window_set_algorithm_and_keymode(
        crate::judge::window::dx_pop_judge_windows(),
        RuleMode::Dx,
        JudgeAlgorithm::Combo,
        KeyMode::K9,
    );

    let bad = engine.process_input(&chart, press_at(TimeUs(900_000)));
    assert_eq!(bad.events[0].judge, Judge::Bad);
    assert!(engine.bad_attempted_notes.contains(&NoteId(1)));
    assert_eq!(engine.judged_notes.get(&NoteId(1)), None);

    let pgreat = engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    assert_eq!(pgreat.events[0].judge, Judge::PGreat);
    assert!(!engine.bad_attempted_notes.contains(&NoteId(1)));
    assert_eq!(engine.judged_notes.get(&NoteId(1)), Some(&Judge::PGreat));
}

#[test]
fn combo_candidate_prefers_later_combo_note_over_slow_bad() {
    let chart = chart_with_two_taps(TimeUs(1_000_000), TimeUs(1_100_000));
    let mut engine = JudgeEngine::new(windows());

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_100_000)));
    let missed = engine.process_misses(&chart, TimeUs(1_130_000));

    assert_eq!(outcome.events[0].note_id, Some(NoteId(2)));
    assert_eq!(outcome.events[0].judge, Judge::PGreat);
    assert_eq!(missed.events[0].note_id, Some(NoteId(1)));
    assert_eq!(missed.events[0].judge, Judge::Poor);
}

#[test]
fn duration_candidate_prefers_closest_note() {
    let chart = chart_with_two_taps(TimeUs(1_000_000), TimeUs(1_040_000));
    let mut engine = JudgeEngine::new_with_window_set_and_algorithm(
        JudgeWindows::uniform(windows()),
        RuleMode::Beatoraja,
        JudgeAlgorithm::Duration,
    );

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_030_000)));

    assert_eq!(outcome.events[0].note_id, Some(NoteId(2)));
    assert_eq!(outcome.events[0].judge, Judge::PGreat);
    assert_eq!(outcome.events[0].delta, TimeUs(-10_000));
}

#[test]
fn lowest_candidate_keeps_first_note() {
    let chart = chart_with_two_taps(TimeUs(1_000_000), TimeUs(1_040_000));
    let mut engine = JudgeEngine::new_with_window_set_and_algorithm(
        JudgeWindows::uniform(windows()),
        RuleMode::Beatoraja,
        JudgeAlgorithm::Lowest,
    );

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_030_000)));

    assert_eq!(outcome.events[0].note_id, Some(NoteId(1)));
    assert_eq!(outcome.events[0].judge, Judge::Great);
    assert_eq!(outcome.events[0].delta, TimeUs(30_000));
}

#[test]
fn score_candidate_uses_great_threshold_instead_of_duration() {
    let chart = chart_with_two_taps(TimeUs(1_000_000), TimeUs(1_150_000));
    let mut engine = JudgeEngine::new_with_window_set_and_algorithm(
        JudgeWindows::uniform(windows()),
        RuleMode::Beatoraja,
        JudgeAlgorithm::Score,
    );

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_100_000)));

    assert_eq!(outcome.events[0].note_id, Some(NoteId(1)));
    assert_eq!(outcome.events[0].judge, Judge::Bad);
    assert_eq!(outcome.events[0].delta, TimeUs(100_000));
}

#[test]
fn lr2oraja_multi_bad_adds_preceding_bad_before_selected_note() {
    let chart = chart_with_two_taps(TimeUs(1_000_000), TimeUs(1_090_000));
    let mut engine = JudgeEngine::new_with_rule_mode(
        crate::judge::window::lr2oraja_note_judge_window(),
        RuleMode::Lr2Oraja,
    );

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_150_000)));

    assert!(outcome.consumed_input);
    assert_eq!(outcome.events.len(), 2);
    assert_eq!(outcome.events[0].note_id, Some(NoteId(1)));
    assert_eq!(outcome.events[0].judge, Judge::Bad);
    assert_eq!(outcome.events[0].delta, TimeUs(150_000));
    assert_eq!(outcome.events[1].note_id, Some(NoteId(2)));
    assert_eq!(outcome.events[1].judge, Judge::Great);
    assert_eq!(outcome.events[1].delta, TimeUs(60_000));
    assert_eq!(
        outcome.keysounds,
        vec![KeySoundEvent { note_id: NoteId(2), time: TimeUs(1_150_000) }]
    );
    assert_eq!(engine.lanes[Lane::Key1.index()].next_note_index, 2);
}

#[test]
fn dx_mode_adds_lr2oraja_multi_bad() {
    let chart = chart_with_two_taps(TimeUs(1_000_000), TimeUs(1_090_000));
    let mut engine =
        JudgeEngine::new_with_rule_mode(crate::judge::window::dx_note_judge_window(), RuleMode::Dx);

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_150_000)));

    assert!(outcome.consumed_input);
    assert_eq!(outcome.events.len(), 2);
    assert_eq!(outcome.events[0].note_id, Some(NoteId(1)));
    assert_eq!(outcome.events[0].judge, Judge::Bad);
    assert_eq!(outcome.events[1].note_id, Some(NoteId(2)));
    assert_eq!(outcome.events[1].judge, Judge::Good);
    assert_eq!(engine.lanes[Lane::Key1.index()].next_note_index, 2);
}

#[test]
fn dx_9key_multi_bad_does_not_consume_the_bad_note() {
    let chart = chart_with_two_taps(TimeUs(1_000_000), TimeUs(1_050_000));
    let mut engine = JudgeEngine::new_with_window_set_algorithm_and_keymode(
        crate::judge::window::dx_pop_judge_windows(),
        RuleMode::Dx,
        JudgeAlgorithm::Combo,
        KeyMode::K9,
    );

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_100_000)));

    assert_eq!(outcome.events.len(), 2);
    assert_eq!(outcome.events[0].note_id, Some(NoteId(1)));
    assert_eq!(outcome.events[0].judge, Judge::Bad);
    assert!(engine.bad_attempted_notes.contains(&NoteId(1)));
    assert_eq!(engine.judged_notes.get(&NoteId(1)), None);
    assert_eq!(engine.judged_notes.get(&NoteId(2)), Some(&Judge::Great));

    let missed = engine.process_misses(&chart, TimeUs(1_100_001));
    assert!(missed.events.is_empty());
    assert_eq!(engine.judged_notes.get(&NoteId(1)), Some(&Judge::Poor));
}

#[test]
fn beatoraja_mode_does_not_add_lr2oraja_multi_bad() {
    let chart = chart_with_two_taps(TimeUs(1_000_000), TimeUs(1_090_000));
    let mut engine = JudgeEngine::new_with_rule_mode(
        crate::judge::window::lr2oraja_note_judge_window(),
        RuleMode::Beatoraja,
    );

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_150_000)));

    assert_eq!(outcome.events.len(), 1);
    assert_eq!(outcome.events[0].note_id, Some(NoteId(2)));
    assert_eq!(outcome.events[0].judge, Judge::Great);
    assert_eq!(engine.lanes[Lane::Key1.index()].next_note_index, 0);
}

#[test]
fn lr2oraja_multi_bad_keeps_following_bad_when_selected_note_is_bad() {
    let chart = chart_with_two_taps(TimeUs(1_000_000), TimeUs(1_260_000));
    let mut engine = JudgeEngine::new_with_rule_mode(
        crate::judge::window::lr2oraja_note_judge_window(),
        RuleMode::Lr2Oraja,
    );

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_130_000)));

    assert!(outcome.consumed_input);
    assert_eq!(outcome.events.len(), 2);
    assert_eq!(outcome.events[0].note_id, Some(NoteId(2)));
    assert_eq!(outcome.events[0].judge, Judge::Bad);
    assert_eq!(outcome.events[0].delta, TimeUs(-130_000));
    assert_eq!(outcome.events[1].note_id, Some(NoteId(1)));
    assert_eq!(outcome.events[1].judge, Judge::Bad);
    assert_eq!(outcome.events[1].delta, TimeUs(130_000));
    assert_eq!(
        outcome.keysounds,
        vec![KeySoundEvent { note_id: NoteId(1), time: TimeUs(1_130_000) }]
    );
    assert_eq!(engine.lanes[Lane::Key1.index()].next_note_index, 2);
}

#[test]
fn scratch_press_uses_scratch_window() {
    let chart = chart_with_lane_tap(Lane::Scratch, TimeUs(1_000_000));
    let mut engine = JudgeEngine::new_with_window_set(
        crate::judge::window::beatoraja_judge_windows_for_keymode(bmz_core::lane::KeyMode::K7),
        RuleMode::Beatoraja,
    );

    let outcome = engine.process_input(&chart, press_lane_at(Lane::Scratch, TimeUs(1_065_000)));

    assert_eq!(outcome.events[0].judge, Judge::Great);
    assert_eq!(outcome.events[0].side, TimingSide::Slow);
}

#[test]
fn cn_release_uses_long_note_end_window() {
    let mut window_set = JudgeWindows::uniform(windows());
    window_set.long_note_end =
        JudgeWindow::symmetric(120_000, 160_000, 200_000, 220_000, 0, 0, 16_000);
    let mut chart = chart_with_long_start(TimeUs(1_000_000), TimeUs(2_000_000));
    chart.long_notes[0].mode = Some(LongNoteMode::Cn);
    let mut engine = JudgeEngine::new_with_window_set(window_set, RuleMode::Beatoraja);

    let press = engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    let release = engine.process_input(&chart, release_at(TimeUs(2_150_000)));

    assert_eq!(press.events[0].judge, Judge::PGreat);
    assert_eq!(release.events[0].judge, Judge::Great);
}

#[test]
fn dx_9key_ln_early_bad_release_can_be_cancelled_during_margin() {
    let chart = chart_with_long_start(TimeUs(1_000_000), TimeUs(2_000_000));
    let mut engine = JudgeEngine::new_with_window_set_algorithm_and_keymode(
        crate::judge::window::dx_pop_judge_windows(),
        RuleMode::Dx,
        JudgeAlgorithm::Combo,
        KeyMode::K9,
    );

    let press = engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    assert_eq!(press.events[0].judge, Judge::PGreat);

    let release = engine.process_input(&chart, release_at(TimeUs(1_750_000)));
    assert!(release.events.is_empty());
    assert!(
        engine.lanes[Lane::Key1.index()]
            .active_long
            .is_some_and(|active| active.pending_release.is_some())
    );

    let before_margin = engine.process_misses(&chart, TimeUs(1_900_000));
    assert!(before_margin.events.is_empty());
    let repress = engine.process_input(&chart, press_at(TimeUs(1_940_000)));
    assert!(repress.events.is_empty());
    assert!(repress.consumed_input);

    let end = engine.process_misses(&chart, TimeUs(2_000_001));
    assert_eq!(end.events.len(), 1);
    assert_eq!(end.events[0].note_id, Some(NoteId(1)));
    assert_eq!(end.events[0].judge, Judge::PGreat);
}

#[test]
fn dx_9key_cn_early_bad_release_finalizes_after_margin() {
    let mut chart = chart_with_long_start(TimeUs(1_000_000), TimeUs(2_000_000));
    chart.long_notes[0].mode = Some(LongNoteMode::Cn);
    let mut engine = JudgeEngine::new_with_window_set_algorithm_and_keymode(
        crate::judge::window::dx_pop_judge_windows(),
        RuleMode::Dx,
        JudgeAlgorithm::Combo,
        KeyMode::K9,
    );

    engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    let release = engine.process_input(&chart, release_at(TimeUs(1_750_000)));
    assert!(release.events.is_empty());

    let finalized = engine.process_misses(&chart, TimeUs(1_950_000));
    assert_eq!(finalized.events.len(), 1);
    assert_eq!(finalized.events[0].note_id, Some(NoteId(2)));
    assert_eq!(finalized.events[0].judge, Judge::Bad);
    assert_eq!(engine.judged_notes.get(&NoteId(2)), Some(&Judge::Bad));
}

#[test]
fn missed_cn_head_marks_both_head_and_tail_poor() {
    let mut chart = chart_with_long_start(TimeUs(1_000_000), TimeUs(2_000_000));
    chart.long_notes[0].mode = Some(LongNoteMode::Cn);
    let mut engine = JudgeEngine::new(windows());

    let missed = engine.process_misses(&chart, TimeUs(1_120_001));

    assert_eq!(missed.events.len(), 2);
    assert_eq!(missed.events[0].note_id, Some(NoteId(1)));
    assert_eq!(missed.events[1].note_id, Some(NoteId(2)));
    assert!(missed.events.iter().all(|event| event.judge == Judge::Poor));
    assert_eq!(engine.judged_notes.get(&NoteId(1)), Some(&Judge::Poor));
    assert_eq!(engine.judged_notes.get(&NoteId(2)), Some(&Judge::Poor));
}

#[test]
fn held_cn_tail_miss_uses_normal_note_bad_window() {
    let mut window_set = JudgeWindows::uniform(windows());
    window_set.long_note_end =
        JudgeWindow::symmetric(120_000, 160_000, 200_000, 220_000, 0, 0, 16_000);
    let mut chart = chart_with_long_start(TimeUs(1_000_000), TimeUs(2_000_000));
    chart.long_notes[0].mode = Some(LongNoteMode::Cn);
    let mut engine = JudgeEngine::new_with_window_set(window_set, RuleMode::Beatoraja);

    engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    let inside_note_bad = engine.process_misses(&chart, TimeUs(2_120_000));
    assert!(inside_note_bad.events.is_empty());
    let missed = engine.process_misses(&chart, TimeUs(2_120_001));
    assert_eq!(missed.events.len(), 1);
    assert_eq!(missed.events[0].note_id, Some(NoteId(2)));
    assert_eq!(missed.events[0].judge, Judge::Poor);
    assert_eq!(engine.judged_notes.get(&NoteId(2)), Some(&Judge::Poor));
}

#[test]
fn lr2oraja_derived_modes_suppress_late_bad_on_long_note_start() {
    let chart = chart_with_long_start(TimeUs(1_000_000), TimeUs(2_000_000));
    let input = press_at(TimeUs(1_100_000));

    let mut beatoraja = JudgeEngine::new(windows());
    let beatoraja_outcome = beatoraja.process_input(&chart, input);
    assert_eq!(beatoraja_outcome.events[0].judge, Judge::Bad);
    assert_eq!(beatoraja.lanes[Lane::Key1.index()].next_note_index, 2);

    let mut lr2oraja = JudgeEngine::new_with_rule_mode(windows(), RuleMode::Lr2Oraja);
    let lr2oraja_outcome = lr2oraja.process_input(&chart, input);
    assert!(lr2oraja_outcome.events.is_empty());
    assert!(!lr2oraja_outcome.consumed_input);
    assert_eq!(lr2oraja.lanes[Lane::Key1.index()].next_note_index, 0);

    let mut dx = JudgeEngine::new_with_rule_mode(windows(), RuleMode::Dx);
    let dx_outcome = dx.process_input(&chart, input);
    assert!(dx_outcome.events.is_empty());
    assert!(!dx_outcome.consumed_input);
    assert_eq!(dx.lanes[Lane::Key1.index()].next_note_index, 0);
}

#[test]
fn defined_cn_pair_judges_release_even_when_chart_default_is_ln() {
    let mut chart = chart_with_long_start(TimeUs(1_000_000), TimeUs(2_000_000));
    chart.metadata.long_note_mode = LongNoteMode::Ln;
    chart.long_notes[0].mode = Some(LongNoteMode::Cn);
    let mut engine = JudgeEngine::new(windows());

    let press = engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    let release = engine.process_input(&chart, release_at(TimeUs(2_000_000)));

    assert_eq!(press.events[0].judge, Judge::PGreat);
    assert_eq!(release.events.len(), 1);
    assert_eq!(release.events[0].note_id, Some(NoteId(2)));
    assert_eq!(release.events[0].judge, Judge::PGreat);
}

#[test]
fn ln_start_defers_scoring_until_end() {
    let chart = chart_with_long_start(TimeUs(1_000_000), TimeUs(2_000_000));
    let mut engine = JudgeEngine::new(windows());

    let press = engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    let end = engine.process_misses(&chart, TimeUs(2_000_001));

    assert_eq!(press.events[0].note_id, Some(NoteId(1)));
    assert_eq!(press.events[0].judge, Judge::PGreat);
    assert!(!press.events[0].affects_score);
    assert_eq!(end.events[0].note_id, Some(NoteId(1)));
    assert_eq!(end.events[0].judge, Judge::PGreat);
    assert!(end.events[0].affects_score);
}

#[test]
fn ln_early_release_scores_once_with_combined_judge() {
    let chart = chart_with_long_start(TimeUs(1_000_000), TimeUs(2_000_000));
    let mut engine = JudgeEngine::new(windows());

    let press = engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    let release = engine.process_input(&chart, release_at(TimeUs(1_900_000)));

    assert!(!press.events[0].affects_score);
    assert_eq!(release.events[0].note_id, Some(NoteId(1)));
    assert_eq!(release.events[0].judge, Judge::Bad);
    assert_eq!(release.events[0].side, TimingSide::Fast);
    assert_eq!(release.events[0].delta, TimeUs(-100_000));
    assert!(release.events[0].affects_score);
}

#[test]
fn defined_hcn_pair_judges_early_release_even_when_chart_default_is_ln() {
    // 早離し後の減衰は judge engine ではなく session 側の passing ベース
    // (update_hcn_lane_timers / apply_hcn_gauge) で処理される。
    let mut chart = chart_with_long_start(TimeUs(1_000_000), TimeUs(2_000_000));
    chart.metadata.long_note_mode = LongNoteMode::Ln;
    chart.long_notes[0].mode = Some(LongNoteMode::Hcn);
    let mut engine = JudgeEngine::new(windows());

    let press = engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    let release = engine.process_input(&chart, release_at(TimeUs(1_500_000)));

    assert_eq!(press.events[0].judge, Judge::PGreat);
    assert_eq!(release.events[0].note_id, Some(NoteId(2)));
    assert_eq!(release.events[0].judge, Judge::Poor);
    assert_eq!(engine.judged_notes.get(&NoteId(2)), Some(&Judge::Poor));
}

fn chart_with_mine(time: TimeUs, damage: u16) -> PlayableChart {
    let lane = Lane::Key1;
    let note = NoteEvent {
        id: NoteId(7),
        lane,
        kind: NoteKind::Mine,
        tick: Default::default(),
        time,
        sound: None,
        damage: Some(damage),
    };
    let mut lane_notes = std::array::from_fn(|_| Vec::new());
    lane_notes[lane.index()].push(note);
    PlayableChart {
        identity: ChartIdentity { file_md5: [0; 16], file_sha256: [0; 32] },
        metadata: ChartMetadata::default(),
        lane_notes,
        long_notes: Vec::new(),
        bgm_events: Vec::<SoundEvent>::new(),
        bga_events: Vec::new(),
        timing_events: Vec::new(),
        scroll_events: Vec::new(),
        speed_events: Vec::new(),
        judge_rank_events: Vec::new(),
        bgm_volume_events: Vec::new(),
        key_volume_events: Vec::new(),
        text_events: Vec::new(),
        bga_opacity_events: Vec::new(),
        bga_argb_events: Vec::new(),
        swbga_definitions: Vec::new(),
        bga_keybound_events: Vec::new(),
        bga_asset_by_bmp_key: std::collections::HashMap::new(),
        bar_lines: Vec::new(),
        sounds: Vec::<SoundAssetRef>::new(),
        bga_assets: Vec::new(),
        total_notes: 0,
        end_time: time,
    }
}

#[test]
fn mine_hit_emits_event_with_damage() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8);
    let mut engine = JudgeEngine::new(windows());

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_000_000)));

    assert_eq!(outcome.mine_hits.len(), 1);
    assert_eq!(outcome.mine_hits[0].damage, 8);
    assert_eq!(outcome.mine_hits[0].note_id, NoteId(7));
    // Mine ヒットは通常判定とは別ベクタに入る。スコア対象ノーツが無いので
    // events は空、consumed_input も false のまま。
    assert!(outcome.events.is_empty());
    assert!(!outcome.consumed_input);
}

#[test]
fn mine_does_not_hit_outside_window() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8);
    let mut engine = JudgeEngine::new(windows());

    let outcome = engine.process_input(&chart, press_at(TimeUs(1_100_000)));
    assert!(outcome.mine_hits.is_empty());
}

#[test]
fn mine_hit_does_not_double_fire() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8);
    let mut engine = JudgeEngine::new(windows());

    let first = engine.process_input(&chart, press_at(TimeUs(1_000_000)));
    let second = engine.process_input(&chart, press_at(TimeUs(1_000_000)));

    assert_eq!(first.mine_hits.len(), 1);
    assert!(second.mine_hits.is_empty(), "same Mine must not fire twice");
}

#[test]
fn mine_pass_hits_when_lane_is_held() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8);
    let mut engine = JudgeEngine::new(windows());
    let mut lane_keyon_started_at = [None; LANE_COUNT];
    lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(900_000));

    let outcome = engine.process_mine_passes(&chart, TimeUs(1_000_000), &lane_keyon_started_at);

    assert_eq!(outcome.mine_hits.len(), 1);
    assert_eq!(outcome.mine_hits[0].note_id, NoteId(7));
    assert_eq!(outcome.mine_hits[0].damage, 8);
}

#[test]
fn mine_pass_without_pressed_lane_is_skipped() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8);
    let mut engine = JudgeEngine::new(windows());
    let lane_keyon_started_at = [None; LANE_COUNT];

    let outcome = engine.process_mine_passes(&chart, TimeUs(1_000_000), &lane_keyon_started_at);

    assert!(outcome.mine_hits.is_empty());
}

#[test]
fn mine_pass_ignores_key_pressed_after_mine_time() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8);
    let mut engine = JudgeEngine::new(windows());
    let mut lane_keyon_started_at = [None; LANE_COUNT];
    lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(1_050_000));

    let outcome = engine.process_mine_passes(&chart, TimeUs(1_100_000), &lane_keyon_started_at);

    assert!(outcome.mine_hits.is_empty());
}

#[test]
fn mine_does_not_hit_after_it_already_passed_unpressed() {
    let chart = chart_with_mine(TimeUs(1_000_000), 8);
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
