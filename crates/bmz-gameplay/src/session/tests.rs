use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use bmz_chart::model::{ChartMetadata, NoteEvent, NoteKind, SoundAssetRef, SoundEvent};
use bmz_core::chart::ChartIdentity;
use bmz_core::ids::{NoteId, SoundId};
use bmz_core::input::{InputDeviceKind, InputSource};
use bmz_core::judge::TimingSide;
use bmz_core::lane::Lane;
use bmz_core::time::{ChartTick, TimeUs};

use crate::input::backend::NullInputBackend;
use crate::input::binding::LaneBinding;
use crate::input::system::InputSystem;
use crate::input::translator::DefaultInputTranslator;
use crate::judge::model::JudgeWindow;

use super::*;
use crate::score::scored_note_count;

#[derive(Default)]
struct TestAudio {
    scheduled: Vec<ScheduledSound>,
}

impl AudioScheduler for TestAudio {
    fn schedule(&mut self, sound: ScheduledSound) {
        self.scheduled.push(sound);
    }
}

#[test]
fn audio_mix_toggle_preserves_chart_normalization_gain() {
    let mut mix = PlayAudioMix {
        master_volume: 1.0,
        chart_normalization_gain: 0.25,
        normalize_chart_volume: true,
        key_volume: 1.0,
        bgm_volume: 1.0,
    };

    assert_eq!(mix.effective_normalization_gain(), 0.25);
    mix.normalize_chart_volume = false;
    assert_eq!(mix.effective_normalization_gain(), 1.0);
    assert_eq!(mix.chart_normalization_gain, 0.25);
    mix.normalize_chart_volume = true;
    assert_eq!(mix.effective_normalization_gain(), 0.25);
}

#[test]
fn display_only_opponent_judgement_does_not_change_primary_score_or_gauge() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.autoplay = None;
    session.display_only_lane_mask[Lane::Key8.index()] = true;
    session.opponent_score = Some(ScoreState::default());
    session.opponent_gauge = Some(session.gauge.clone());
    let primary_gauge = session.gauge.current().value;

    let events = apply_judge_outcome(
        &mut session,
        JudgeOutcome {
            events: vec![JudgementEvent {
                note_id: Some(NoteId(1)),
                lane: Lane::Key8,
                judge: Judge::PGreat,
                side: TimingSide::Fast,
                delta: TimeUs(0),
                time: TimeUs(1_000_000),
                affects_score: true,
            }],
            keysound_volumes: vec![(SoundId(7), 0.0)],
            ..JudgeOutcome::default()
        },
    );

    assert_eq!(session.score.ex_score(), 0);
    assert_eq!(session.score.past_notes, 0);
    assert_eq!(session.gauge.current().value, primary_gauge);
    assert_eq!(session.opponent_score.as_ref().unwrap().ex_score(), 2);
    assert_eq!(session.opponent_score.as_ref().unwrap().past_notes, 1);
    assert!(session.pending_keysound_volumes.is_empty());
    assert!(!events[0].affects_score);

    update_recent_judgements(&mut session, &events, TimeUs(1_000_000));
    assert_eq!(session.recent_judgements, events);
}

#[test]
fn display_only_opponent_hcn_updates_only_opponent_gauge() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.gauge.set_initial_value(50.0);
    session.opponent_gauge = Some(session.gauge.clone());
    session.display_only_lane_mask[Lane::Key8.index()] = true;
    session.lane_hcn_timer[Lane::Key8.index()] =
        Some(HcnLaneTimer { inclease: true, since: TimeUs(0), passing_count_us: 0 });
    session.last_hcn_gauge_at = Some(TimeUs(0));

    apply_hcn_gauge(&mut session, TimeUs(500_000));

    assert_eq!(session.gauge.current().value, 50.0);
    assert!(session.opponent_gauge.as_ref().unwrap().current().value > 50.0);
    assert_eq!(session.opponent_gauge_increase_started_at, Some(TimeUs(500_000)));
}

#[test]
fn advance_session_frame_schedules_autoplay_keysounds() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.audio_mix.master_volume = 0.5;
    session.audio_mix.key_volume = 0.25;
    session.audio_mix.chart_normalization_gain = 0.5;
    session.audio_mix.normalize_chart_volume = true;
    let mut audio = TestAudio::default();

    let frame = advance_session_frame(&mut session, &mut audio);

    assert_eq!(frame.judgements.len(), 1);
    assert_eq!(audio.scheduled.len(), 1);
    assert_eq!(audio.scheduled[0].sound_id, SoundId(7));
    assert_eq!(audio.scheduled[0].start_frame, 0);
    assert_eq!(audio.scheduled[0].volume, 0.0625);
    assert_eq!(audio.scheduled[0].restart_policy, RestartPolicy::StopSameSound);
    assert_eq!(session.recent_judgements.len(), 1);
}

#[test]
fn advance_session_frame_keeps_ready_until_chart_zero() {
    let mut session = session_with_autoplay(chart_with_keysound());
    let current_frame = Arc::new(AtomicU64::new(0));
    session.audio_clock =
        AudioClock::with_position(48_000, 0, -1_000_000, current_frame.clone(), true);
    let mut audio = TestAudio::default();

    let ready_frame = advance_session_frame(&mut session, &mut audio);

    assert_eq!(ready_frame.state, PlayState::Ready);
    assert!(ready_frame.judgements.is_empty());
    assert!(audio.scheduled.is_empty());
    assert_eq!(session.score.past_notes, 0);

    current_frame.store(48_000, std::sync::atomic::Ordering::Relaxed);
    let playing_frame = advance_session_frame(&mut session, &mut audio);

    assert_eq!(playing_frame.state, PlayState::Playing);
    assert_eq!(playing_frame.judgements.len(), 1);
    assert_eq!(audio.scheduled.len(), 1);
}

#[test]
fn session_frame_drains_ordered_skin_input_and_judgement_events() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.autoplay = None;

    process_session_input(&mut session, human_press(TimeUs(0)));
    process_session_input(&mut session, human_release(TimeUs(10_000)));
    session.state = PlayState::Finished;

    let mut audio = TestAudio::default();
    let frame = advance_session_frame(&mut session, &mut audio);

    assert_eq!(frame.skin_events.len(), 3);
    assert_eq!(frame.skin_events.iter().map(|event| event.sequence).collect::<Vec<_>>(), [0, 1, 2]);
    assert!(matches!(
        frame.skin_events[0].kind,
        SkinRuntimeEventKind::Input(InputEvent { kind: InputKind::Press, .. })
    ));
    assert!(matches!(
        frame.skin_events[1].kind,
        SkinRuntimeEventKind::Judgement(JudgementEvent { judge: Judge::PGreat, .. })
    ));
    assert!(matches!(
        frame.skin_events[2].kind,
        SkinRuntimeEventKind::Input(InputEvent { kind: InputKind::Release, .. })
    ));
    assert!(session.pending_skin_events.is_empty());

    let next_frame = advance_session_frame(&mut session, &mut audio);
    assert!(next_frame.skin_events.is_empty());
}

#[test]
fn auto_key_release_emits_skin_release_event() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(0));
    session.lane_auto_release_at[Lane::Key1.index()] = Some(TimeUs(80_000));

    apply_auto_key_release(&mut session, TimeUs(80_000));

    assert!(matches!(
        session.pending_skin_events.as_slice(),
        [SkinRuntimeEvent {
            sequence: 0,
            kind: SkinRuntimeEventKind::Input(InputEvent {
                lane: Lane::Key1,
                kind: InputKind::Release,
                time: TimeUs(80_000),
                source: InputSource::Auto,
                ..
            }),
        }]
    ));
}

#[test]
fn empty_poor_schedules_target_note_keysound() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.autoplay = None;
    let mut audio = TestAudio::default();

    let judgements = process_session_input(&mut session, human_press(TimeUs(150_000)));
    schedule_keysounds(&mut session, &mut audio);

    assert_eq!(judgements.len(), 1);
    assert_eq!(judgements[0].judge, Judge::EmptyPoor);
    assert_eq!(judgements[0].note_id, None);
    assert_eq!(audio.scheduled.len(), 1);
    assert_eq!(audio.scheduled[0].sound_id, SoundId(7));
}

#[test]
fn unjudged_press_after_empty_poor_window_uses_previous_playable_keysound() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.autoplay = None;
    let mut audio = TestAudio::default();

    let judgements = process_session_input(&mut session, human_press(TimeUs(800_000)));
    schedule_keysounds(&mut session, &mut audio);

    assert!(judgements.is_empty());
    assert_eq!(audio.scheduled.len(), 1);
    assert_eq!(audio.scheduled[0].sound_id, SoundId(7));
}

#[test]
fn unjudged_press_after_empty_poor_window_prefers_previous_invisible_keysound() {
    let mut session = session_with_autoplay(chart_with_invisible_keysound());
    session.autoplay = None;
    let mut audio = TestAudio::default();

    let judgements = process_session_input(&mut session, human_press(TimeUs(800_000)));
    schedule_keysounds(&mut session, &mut audio);

    assert!(judgements.is_empty());
    assert_eq!(audio.scheduled.len(), 1);
    assert_eq!(audio.scheduled[0].sound_id, SoundId(8));
}

#[test]
fn ln_release_does_not_replay_start_keysound_when_end_has_no_sound() {
    let mut session = session_with_autoplay(ln_chart_with_start_sound_and_end_sound(None));
    session.autoplay = None;
    let mut audio = TestAudio::default();

    process_session_input(&mut session, human_press(TimeUs(0)));
    schedule_keysounds(&mut session, &mut audio);
    assert_eq!(audio.scheduled.len(), 1);
    assert_eq!(audio.scheduled[0].sound_id, SoundId(7));
    audio.scheduled.clear();

    process_session_input(&mut session, human_release(TimeUs(1_000_000)));
    schedule_keysounds(&mut session, &mut audio);

    assert!(audio.scheduled.is_empty());
}

#[test]
fn ln_release_plays_end_keysound_when_end_has_sound() {
    let mut session =
        session_with_autoplay(ln_chart_with_start_sound_and_end_sound(Some(SoundId(9))));
    session.autoplay = None;
    let mut audio = TestAudio::default();

    process_session_input(&mut session, human_press(TimeUs(0)));
    schedule_keysounds(&mut session, &mut audio);
    audio.scheduled.clear();

    process_session_input(&mut session, human_release(TimeUs(1_000_000)));
    schedule_keysounds(&mut session, &mut audio);

    assert_eq!(audio.scheduled.len(), 1);
    assert_eq!(audio.scheduled[0].sound_id, SoundId(9));
}

#[test]
fn early_bad_ln_release_mutes_held_start_keysound() {
    let mut session = session_with_autoplay(ln_chart_with_start_sound_and_end_sound(None));
    session.autoplay = None;

    process_session_input(&mut session, human_press(TimeUs(0)));
    session.pending_keysounds.clear();
    session.pending_keysound_volumes.clear();

    let judgements = process_session_input(&mut session, human_release(TimeUs(700_000)));

    assert_eq!(judgements.len(), 1);
    assert_eq!(judgements[0].judge, Judge::Bad);
    assert_eq!(session.pending_keysound_volumes, vec![(SoundId(7), 0.0)]);
}

#[test]
fn input_offset_auto_adjust_increases_after_ten_late_judgements() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.input_offset_auto_adjust = Some(InputOffsetAutoAdjustState::default());

    let events = vec![judgement_event(Judge::Great, 2_000); 10];
    apply_input_offset_auto_adjust(&mut session, &events);

    assert_eq!(session.offsets.visual_offset_us, 1_000);
    assert_eq!(session.offsets.input_offset_us, 0);
    assert_eq!(session.input_offset_auto_adjust, Some(InputOffsetAutoAdjustState::default()));
}

#[test]
fn input_offset_auto_adjust_decreases_after_ten_early_judgements() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.input_offset_auto_adjust = Some(InputOffsetAutoAdjustState::default());

    let events = vec![judgement_event(Judge::Good, -2_000); 10];
    apply_input_offset_auto_adjust(&mut session, &events);

    assert_eq!(session.offsets.visual_offset_us, -1_000);
    assert_eq!(session.offsets.input_offset_us, 0);
}

#[test]
fn input_offset_auto_adjust_ignores_poor_and_empty_poor() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.input_offset_auto_adjust = Some(InputOffsetAutoAdjustState::default());

    let mut events = vec![judgement_event(Judge::Poor, 30_000); 10];
    events.extend(vec![judgement_event(Judge::EmptyPoor, 30_000); 10]);
    apply_input_offset_auto_adjust(&mut session, &events);

    assert_eq!(session.offsets.visual_offset_us, 0);
    assert_eq!(session.offsets.input_offset_us, 0);
    assert_eq!(session.input_offset_auto_adjust.unwrap().count, 0);
}

#[test]
fn advance_session_frame_starts_full_combo_timer_after_last_note() {
    let mut session = session_with_autoplay(chart_with_keysound());
    let mut audio = TestAudio::default();

    advance_session_frame(&mut session, &mut audio);

    assert_eq!(session.full_combo_started_at, Some(TimeUs(0)));
}

#[test]
fn scored_note_count_uses_effective_long_note_mode() {
    let mut chart = chart_with_hcn_long_note();
    assert_eq!(scored_note_count(&chart), 2);

    chart.long_notes[0].mode = Some(LongNoteMode::Cn);
    assert_eq!(scored_note_count(&chart), 2);

    chart.long_notes[0].mode = Some(LongNoteMode::Ln);
    assert_eq!(scored_note_count(&chart), 1);

    chart.long_notes[0].mode = None;
    chart.metadata.long_note_mode = LongNoteMode::Hcn;
    assert_eq!(scored_note_count(&chart), 2);
}

#[test]
fn full_combo_timer_waits_for_cn_end_judgement() {
    let mut chart = chart_with_hcn_long_note();
    chart.long_notes[0].mode = Some(LongNoteMode::Cn);
    let mut session = session_with_autoplay(chart);

    let start_events = apply_judge_outcome(
        &mut session,
        JudgeOutcome { events: vec![judgement_event(Judge::PGreat, 0)], ..Default::default() },
    );
    update_full_combo_timer(&mut session, &start_events);

    assert_eq!(session.scored_total_notes, 2);
    assert_eq!(session.score.past_notes, 1);
    assert_eq!(session.full_combo_started_at, None);

    let mut end_event = judgement_event(Judge::PGreat, 0);
    end_event.note_id = Some(NoteId(2));
    end_event.time = TimeUs(1_000_000);
    let end_events = apply_judge_outcome(
        &mut session,
        JudgeOutcome { events: vec![end_event], ..Default::default() },
    );
    update_full_combo_timer(&mut session, &end_events);

    assert_eq!(session.score.past_notes, 2);
    assert_eq!(session.full_combo_started_at, Some(TimeUs(1_000_000)));
}

#[test]
fn hard_gauge_zero_moves_session_to_failed() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.state = PlayState::Playing;
    session.gauge = GaugeState::new(bmz_core::clear::GaugeType::Hard, 160.0, 1000);
    session
        .gauge
        .gauges
        .iter_mut()
        .find(|gauge| gauge.definition.gauge_type == bmz_core::clear::GaugeType::Hard)
        .unwrap()
        .value = 0.0;

    update_failed_state_from_gauge(&mut session);

    assert_eq!(session.state, PlayState::Failed);
}

#[test]
fn gauge_increase_timer_starts_when_judge_raises_gauge() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.gauge.set_initial_value(50.0);

    apply_judge_outcome(
        &mut session,
        JudgeOutcome {
            events: vec![JudgementEvent {
                time: TimeUs(123_000),
                ..judgement_event(Judge::PGreat, 0)
            }],
            ..Default::default()
        },
    );

    assert_eq!(session.gauge_increase_started_at, Some(TimeUs(123_000)));
}

#[test]
fn gauge_max_timer_tracks_current_max_state() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.gauge.set_initial_value(99.0);

    update_gauge_max_timer(&mut session, TimeUs(25_000));
    assert_eq!(session.gauge_max_started_at, None);

    session.gauge.set_initial_value(100.0);
    update_gauge_max_timer(&mut session, TimeUs(50_000));
    assert_eq!(session.gauge_max_started_at, Some(TimeUs(50_000)));

    update_gauge_max_timer(&mut session, TimeUs(75_000));
    assert_eq!(session.gauge_max_started_at, Some(TimeUs(50_000)));

    session.gauge.set_initial_value(99.0);
    update_gauge_max_timer(&mut session, TimeUs(100_000));
    assert_eq!(session.gauge_max_started_at, None);
}

#[test]
fn course_combo_carry_extends_display_combo_without_changing_score_max() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.course_combo_carry = 100;
    session.course_combo_carry_active = true;
    session.course_max_combo = 100;

    apply_judge_outcome(
        &mut session,
        JudgeOutcome { events: vec![judgement_event(Judge::PGreat, 0)], ..Default::default() },
    );

    assert_eq!(session.score.combo, 1);
    assert_eq!(session.score.max_combo, 1);
    assert_eq!(session.display_combo(), 101);
    assert_eq!(session.display_max_combo(), 101);
}

#[test]
fn course_combo_carry_resets_on_combo_break() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.course_combo_carry = 100;
    session.course_combo_carry_active = true;
    session.course_max_combo = 100;

    apply_judge_outcome(
        &mut session,
        JudgeOutcome { events: vec![judgement_event(Judge::PGreat, 0)], ..Default::default() },
    );
    apply_judge_outcome(
        &mut session,
        JudgeOutcome { events: vec![judgement_event(Judge::Bad, 0)], ..Default::default() },
    );
    apply_judge_outcome(
        &mut session,
        JudgeOutcome { events: vec![judgement_event(Judge::Great, 0)], ..Default::default() },
    );

    assert!(!session.course_combo_carry_active);
    assert_eq!(session.score.combo, 1);
    assert_eq!(session.score.max_combo, 1);
    assert_eq!(session.display_combo(), 1);
    assert_eq!(session.display_max_combo(), 101);
}

#[test]
fn auto_shift_hard_zero_falls_back_without_failed_state() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.state = PlayState::Playing;
    session.gauge = GaugeState::new_auto_shift(160.0, 1000);

    session.gauge.apply_judge(Judge::Poor, 7.0);
    update_failed_state_from_gauge(&mut session);

    assert_eq!(session.state, PlayState::Playing);
    assert_eq!(session.gauge.selected, bmz_core::clear::GaugeType::Hard);
}

/// Key1 に HCN ロングノート (0s 〜 1s, キー音 SoundId(7)) を持つ譜面。
fn chart_with_hcn_long_note() -> PlayableChart {
    let mut chart = chart_with_keysound();
    chart.lane_notes = std::array::from_fn(|_| Vec::new());
    chart.lane_notes[Lane::Key1.index()].push(NoteEvent {
        id: NoteId(1),
        lane: Lane::Key1,
        kind: NoteKind::LongStart,
        tick: ChartTick(0),
        time: TimeUs(0),
        sound: Some(SoundId(7)),
        damage: None,
    });
    chart.lane_notes[Lane::Key1.index()].push(NoteEvent {
        id: NoteId(2),
        lane: Lane::Key1,
        kind: NoteKind::LongEnd,
        tick: ChartTick(192),
        time: TimeUs(1_000_000),
        sound: None,
        damage: None,
    });
    chart.long_notes.push(bmz_chart::model::LongNotePair {
        lane: Lane::Key1,
        style: bmz_chart::model::LongNoteStyle::ChannelPair,
        mode: Some(LongNoteMode::Hcn),
        start_note_id: NoteId(1),
        end_note_id: NoteId(2),
        start_tick: ChartTick(0),
        end_tick: ChartTick(192),
        start_time: TimeUs(0),
        end_time: TimeUs(1_000_000),
        sound: Some(SoundId(7)),
    });
    chart
}

fn human_press(time: TimeUs) -> InputEvent {
    InputEvent {
        lane: Lane::Key1,
        kind: InputKind::Press,
        time,
        source: InputSource::Human,
        device_kind: InputDeviceKind::Keyboard,
        scratch_direction: None,
    }
}

fn human_release(time: TimeUs) -> InputEvent {
    InputEvent {
        lane: Lane::Key1,
        kind: InputKind::Release,
        time,
        source: InputSource::Human,
        device_kind: InputDeviceKind::Keyboard,
        scratch_direction: None,
    }
}

fn chart_with_invisible_keysound() -> PlayableChart {
    let mut chart = chart_with_keysound();
    chart.lane_notes[Lane::Key1.index()].push(NoteEvent {
        id: NoteId(2),
        lane: Lane::Key1,
        kind: NoteKind::Invisible,
        tick: ChartTick(96),
        time: TimeUs(500_000),
        sound: Some(SoundId(8)),
        damage: None,
    });
    chart.sounds.push(SoundAssetRef { id: SoundId(8), path: "hidden.wav".into() });
    chart.end_time = TimeUs(500_000);
    chart
}

fn ln_chart_with_start_sound_and_end_sound(end_sound: Option<SoundId>) -> PlayableChart {
    let mut chart = chart_with_hcn_long_note();
    chart.metadata.long_note_mode = LongNoteMode::Ln;
    chart.long_notes[0].mode = Some(LongNoteMode::Ln);
    chart.lane_notes[Lane::Key1.index()][1].sound = end_sound;
    if let Some(sound_id) = end_sound {
        chart
            .sounds
            .push(SoundAssetRef { id: sound_id, path: format!("sound-{}.wav", sound_id.0).into() });
    }
    chart
}

#[test]
fn hcn_gauge_increases_while_passing_and_pressed_until_end() {
    let mut session = session_with_autoplay(chart_with_hcn_long_note());
    session.gauge.set_initial_value(50.0);
    session.judge.judged_notes.insert(NoteId(1), Judge::PGreat);
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(0));

    update_hcn_lane_timers(&mut session, TimeUs(0));
    apply_hcn_gauge(&mut session, TimeUs(0));
    update_hcn_lane_timers(&mut session, TimeUs(500_000));
    apply_hcn_gauge(&mut session, TimeUs(500_000));
    let mid = session.gauge.current().value;
    // 終端通過後は passing が外れ、ゲージは変化しない。
    update_hcn_lane_timers(&mut session, TimeUs(2_000_000));
    apply_hcn_gauge(&mut session, TimeUs(2_000_000));
    update_hcn_lane_timers(&mut session, TimeUs(3_000_000));
    apply_hcn_gauge(&mut session, TimeUs(3_000_000));

    assert!(mid > 50.0);
    assert!((session.gauge.current().value - mid).abs() < f32::EPSILON);
}

#[test]
fn hcn_passing_waits_until_the_head_is_judged() {
    let mut session = session_with_autoplay(chart_with_hcn_long_note());

    update_hcn_lane_timers(&mut session, TimeUs(100_000));
    assert!(session.lane_hcn_timer[Lane::Key1.index()].is_none());

    session.judge.judged_notes.insert(NoteId(1), Judge::Poor);
    update_hcn_lane_timers(&mut session, TimeUs(100_000));
    assert!(session.lane_hcn_timer[Lane::Key1.index()].is_some());
}

#[test]
fn hcn_gauge_recovers_when_pressed_after_missed_start() {
    let mut session = session_with_autoplay(chart_with_hcn_long_note());
    session.gauge.set_initial_value(50.0);
    // 始端を見逃し (POOR) て離している → 減衰。
    session.judge.judged_notes.insert(NoteId(1), Judge::Poor);

    update_hcn_lane_timers(&mut session, TimeUs(100_000));
    apply_hcn_gauge(&mut session, TimeUs(100_000));
    // 250ms 経過 → mpassingcount が -200ms を下回り減衰 1 tick。
    update_hcn_lane_timers(&mut session, TimeUs(350_000));
    apply_hcn_gauge(&mut session, TimeUs(350_000));
    let drained = session.gauge.current().value;
    assert!(drained < 50.0);

    // 途中から押し直すと回復に転じる (beatoraja passing ベース)。
    // カウンタは反転でリセットされないため、残り -50ms を打ち消して
    // +200ms を超えるまで押し続けると回復 1 tick が入る。
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(350_000));
    update_hcn_lane_timers(&mut session, TimeUs(800_000));
    apply_hcn_gauge(&mut session, TimeUs(800_000));

    assert!(session.gauge.current().value > drained);
}

#[test]
fn hcn_keysound_mutes_on_release_after_early_end_judge() {
    let mut session = session_with_autoplay(chart_with_hcn_long_note());
    session.judge.judged_notes.insert(NoteId(1), Judge::PGreat);
    // 早離しで終端が BAD 判定済み、キーは離している。
    session.judge.judged_notes.insert(NoteId(2), Judge::Bad);

    update_hcn_lane_timers(&mut session, TimeUs(500_000));
    assert_eq!(session.pending_keysound_volumes, vec![(SoundId(7), 0.0)]);

    // 押し直すと元の音量へ復帰する。同一状態の継続では再送しない。
    session.pending_keysound_volumes.clear();
    update_hcn_lane_timers(&mut session, TimeUs(600_000));
    assert!(session.pending_keysound_volumes.is_empty());
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(700_000));
    update_hcn_lane_timers(&mut session, TimeUs(700_000));
    assert_eq!(session.pending_keysound_volumes.len(), 1);
    assert_eq!(session.pending_keysound_volumes[0].0, SoundId(7));
    assert!(session.pending_keysound_volumes[0].1 > 0.0);
}

#[test]
fn hcn_keysound_volume_untouched_while_end_unjudged() {
    let mut session = session_with_autoplay(chart_with_hcn_long_note());
    session.judge.judged_notes.insert(NoteId(1), Judge::PGreat);
    // 終端未判定で離していても音量は触らない (beatoraja: pair state > 3 のみ)。
    update_hcn_lane_timers(&mut session, TimeUs(500_000));
    assert!(session.pending_keysound_volumes.is_empty());
}

#[test]
fn ln_mode_scores_once_at_long_note_end() {
    let mut chart = chart_with_hcn_long_note();
    chart.metadata.long_note_mode = LongNoteMode::Ln;
    chart.long_notes[0].mode = Some(LongNoteMode::Ln);
    let mut session = session_with_autoplay(chart);
    session.autoplay = None;

    let press = session.judge.process_input(
        &session.chart,
        InputEvent {
            lane: Lane::Key1,
            kind: InputKind::Press,
            time: TimeUs(0),
            source: InputSource::Human,
            device_kind: InputDeviceKind::Keyboard,
            scratch_direction: None,
        },
    );
    apply_judge_outcome(&mut session, press);
    assert_eq!(session.score.past_notes, 0);
    assert_eq!(session.score.combo, 0);

    let end = session.judge.process_misses(&session.chart, TimeUs(1_000_001));
    apply_judge_outcome(&mut session, end);

    assert_eq!(session.score.past_notes, 1);
    assert_eq!(session.score.combo, 1);
    assert_eq!(session.score.ex_score(), 2);
}

fn chart_with_mine(time: TimeUs, damage: u16) -> PlayableChart {
    let mut chart = chart_with_keysound();
    chart.lane_notes = std::array::from_fn(|_| Vec::new());
    chart.lane_notes[Lane::Key1.index()].push(NoteEvent {
        id: NoteId(7),
        lane: Lane::Key1,
        kind: NoteKind::Mine,
        tick: ChartTick(0),
        time,
        sound: None,
        damage: Some(damage),
    });
    chart.total_notes = 0;
    chart.end_time = time;
    chart
}

#[test]
fn process_mine_passes_applies_damage_for_held_human_lane() {
    let mut session = session_with_autoplay(chart_with_mine(TimeUs(1_000_000), 8));
    session.autoplay = None;
    session.gauge.set_initial_value(50.0);
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(900_000));

    let events = process_mine_passes(&mut session, TimeUs(1_000_000));

    assert!(events.is_empty());
    assert_eq!(session.pending_mine_hits.len(), 1);
    assert_eq!(session.pending_mine_hits[0].note_id, NoteId(7));
    assert!((session.gauge.current().value - 42.0).abs() < f32::EPSILON);
}

#[test]
fn process_mine_passes_ignores_autoplay_lane() {
    let mut session = session_with_autoplay(chart_with_mine(TimeUs(1_000_000), 8));
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(900_000));

    process_mine_passes(&mut session, TimeUs(1_000_000));

    assert!(session.pending_mine_hits.is_empty());
    assert!((session.gauge.current().value - 20.0).abs() < f32::EPSILON);
}

#[test]
fn advance_session_frame_schedules_bgm_with_mix_volume() {
    let mut session = session_with_autoplay(chart_with_bgm());
    session.audio_mix.master_volume = 0.5;
    session.audio_mix.bgm_volume = 0.75;
    session.audio_mix.chart_normalization_gain = 0.5;
    session.audio_mix.normalize_chart_volume = true;
    let mut audio = TestAudio::default();

    advance_session_frame(&mut session, &mut audio);

    assert_eq!(audio.scheduled.len(), 1);
    assert_eq!(audio.scheduled[0].sound_id, SoundId(3));
    assert_eq!(audio.scheduled[0].volume, 0.1875);
    assert_eq!(audio.scheduled[0].restart_policy, RestartPolicy::StopSameSound);
}

#[test]
fn advance_session_frame_applies_chart_volume_channels() {
    let mut chart = chart_with_keysound();
    chart.key_volume_events.push(bmz_chart::model::ChartVolumeEvent {
        tick: ChartTick(0),
        time: TimeUs(0),
        value: 128,
    });
    let mut session = session_with_autoplay(chart);
    session.audio_mix.master_volume = 1.0;
    session.audio_mix.key_volume = 1.0;
    let mut audio = TestAudio::default();

    advance_session_frame(&mut session, &mut audio);

    let expected = 128.0 / 255.0;
    assert!((audio.scheduled[0].volume - expected).abs() < 0.001);

    let mut bgm_chart = chart_with_bgm();
    bgm_chart.bgm_volume_events.push(bmz_chart::model::ChartVolumeEvent {
        tick: ChartTick(0),
        time: TimeUs(0),
        value: 64,
    });
    let mut bgm_session = session_with_autoplay(bgm_chart);
    bgm_session.audio_mix.master_volume = 1.0;
    bgm_session.audio_mix.bgm_volume = 1.0;
    let mut bgm_audio = TestAudio::default();

    advance_session_frame(&mut bgm_session, &mut bgm_audio);

    let expected_bgm = 64.0 / 255.0;
    assert!((bgm_audio.scheduled[0].volume - expected_bgm).abs() < 0.001);
}

#[test]
fn update_recent_judgements_expires_old_events() {
    let mut session = session_with_autoplay(chart_with_keysound());
    let event = JudgementEvent {
        note_id: Some(NoteId(1)),
        lane: Lane::Key1,
        judge: Judge::PGreat,
        side: TimingSide::Slow,
        delta: TimeUs(0),
        time: TimeUs(0),
        affects_score: true,
    };

    update_recent_judgements(&mut session, &[event], TimeUs(0));
    update_recent_judgements(&mut session, &[], TimeUs(JUDGEMENT_DISPLAY_US + 1));

    assert!(session.recent_judgements.is_empty());
}

#[test]
fn advance_session_frame_skips_human_inputs_when_replay_active() {
    use crate::input::backend::{
        BufferedInputBackend, DeviceId, DeviceInputEvent, DeviceTimestamp, PhysicalControl,
    };
    use crate::input::binding::{BindingEntry, LaneBinding};

    let chart = chart_with_keysound();
    let mut session = session_with_autoplay(chart);
    // 入力バインディングを設定して Z キーを Key1 にマップ
    let mut backend = BufferedInputBackend::default();
    backend.push(DeviceInputEvent {
        device: DeviceId(1),
        control: PhysicalControl::KeyboardKey("Z".to_string()),
        kind: InputKind::Press,
        timestamp: DeviceTimestamp::Unknown,
        bounce_policy: Default::default(),
    });
    session.input_system = InputSystem {
        backend: Box::new(backend),
        translator: Box::new(DefaultInputTranslator {
            binding: LaneBinding {
                entries: vec![BindingEntry {
                    device: None,
                    control: PhysicalControl::KeyboardKey("Z".to_string()),
                    lane: Lane::Key1,
                    scratch_direction: None,
                }],
            },
        }),
        bounce_filter: Default::default(),
    };
    session.replay_player = Some(crate::replay::ReplayPlayer::default());
    session.autoplay = None;
    let mut audio = TestAudio::default();

    advance_session_frame(&mut session, &mut audio);

    // 人間入力は judge にも recorder にも渡らない
    assert_eq!(session.score.judges.fast_pgreat + session.score.judges.slow_pgreat, 0);
    assert_eq!(session.score.judges.fast_great + session.score.judges.slow_great, 0);
    assert!(session.replay_recorder.events.is_empty());
    // recent_inputs だけは Press が反映される (視覚エフェクト用)
    assert_eq!(session.recent_inputs.len(), 1);
}

#[test]
fn advance_session_frame_skips_human_inputs_when_autoplay_active() {
    use crate::input::backend::{
        BufferedInputBackend, DeviceId, DeviceInputEvent, DeviceTimestamp, PhysicalControl,
    };
    use crate::input::binding::{BindingEntry, LaneBinding};

    let chart = chart_with_keysound();
    let mut session = session_with_autoplay(chart);
    let mut backend = BufferedInputBackend::default();
    backend.push(DeviceInputEvent {
        device: DeviceId(1),
        control: PhysicalControl::KeyboardKey("Z".to_string()),
        kind: InputKind::Press,
        timestamp: DeviceTimestamp::Unknown,
        bounce_policy: Default::default(),
    });
    session.input_system = InputSystem {
        backend: Box::new(backend),
        translator: Box::new(DefaultInputTranslator {
            binding: LaneBinding {
                entries: vec![BindingEntry {
                    device: None,
                    control: PhysicalControl::KeyboardKey("Z".to_string()),
                    lane: Lane::Key1,
                    scratch_direction: None,
                }],
            },
        }),
        bounce_filter: Default::default(),
    };
    let mut audio = TestAudio::default();

    advance_session_frame(&mut session, &mut audio);

    // オートプレイ中は人間入力を recorder に渡さない。
    assert!(session.replay_recorder.events.is_empty());
    // 人間のキー入力は recent_inputs(キービーム)にも反映されない。
    // recent_inputs に乗るのは autoplay のノーツ処理入力のみ。
    assert!(session.recent_inputs.iter().all(|i| i.source == InputSource::Auto));
    assert!(
        session.recent_inputs.iter().all(|i| i.source != InputSource::Human),
        "human key press must not produce a keybeam during autoplay",
    );
}

#[test]
fn process_autoplay_inputs_flashes_keybeam_on_note_processing() {
    let mut session = session_with_autoplay(chart_with_keysound());

    // chart_with_keysound のノーツは time=0 / Key1。audio_now=0 で処理される。
    let judgements = process_autoplay_inputs(&mut session, TimeUs(0));

    assert!(!judgements.is_empty(), "autoplay should judge the note");
    // ノーツ処理に伴って autoplay 入力が recent_inputs に積まれる(キービーム発火)。
    assert_eq!(session.recent_inputs.len(), 1);
    assert_eq!(session.recent_inputs[0].lane, Lane::Key1);
    assert_eq!(session.recent_inputs[0].source, InputSource::Auto);
}

#[test]
fn update_lane_key_states_press_sets_keyon_clears_keyoff() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.lane_keyoff_started_at[Lane::Key1.index()] = Some(TimeUs(1_000));

    let inputs = [InputEvent {
        lane: Lane::Key1,
        kind: InputKind::Press,
        time: TimeUs(5_000),
        source: InputSource::Human,
        device_kind: InputDeviceKind::Keyboard,
        scratch_direction: None,
    }];
    update_lane_key_states(&mut session, &inputs);

    assert_eq!(session.lane_keyon_started_at[Lane::Key1.index()], Some(TimeUs(5_000)));
    assert_eq!(session.lane_keyoff_started_at[Lane::Key1.index()], None);
    // Human source の Press は自動 release を予約しない (押し続け対応)。
    assert_eq!(session.lane_auto_release_at[Lane::Key1.index()], None);
}

#[test]
fn update_lane_key_states_tracks_scratch_direction_until_release() {
    let mut session = session_with_autoplay(chart_with_keysound());
    let press = [InputEvent {
        lane: Lane::Scratch,
        kind: InputKind::Press,
        time: TimeUs(5_000),
        source: InputSource::Human,
        device_kind: InputDeviceKind::Controller,
        scratch_direction: Some(ScratchDirection::Up),
    }];

    update_lane_key_states(&mut session, &press);

    assert_eq!(session.lane_scratch_direction[Lane::Scratch.index()], Some(ScratchDirection::Up));

    let release = [InputEvent {
        lane: Lane::Scratch,
        kind: InputKind::Release,
        time: TimeUs(10_000),
        source: InputSource::Human,
        device_kind: InputDeviceKind::Controller,
        scratch_direction: Some(ScratchDirection::Up),
    }];
    update_lane_key_states(&mut session, &release);

    assert_eq!(session.lane_scratch_direction[Lane::Scratch.index()], None);
}

#[test]
fn update_scratch_angle_phase_accumulates_directionless_scratch_until_release() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.scratch_angle_last_render_at = Some(TimeUs(0));
    session.lane_keyon_started_at[Lane::Scratch.index()] = Some(TimeUs(0));

    update_scratch_angle_phase(&mut session, TimeUs(1_000_000));

    assert_eq!(session.lane_scratch_angle_delta_ms[Lane::Scratch.index()], 160);

    update_lane_key_states(
        &mut session,
        &[InputEvent {
            lane: Lane::Scratch,
            kind: InputKind::Release,
            time: TimeUs(1_000_000),
            source: InputSource::Human,
            device_kind: InputDeviceKind::Keyboard,
            scratch_direction: None,
        }],
    );
    update_scratch_angle_phase(&mut session, TimeUs(1_500_000));

    assert_eq!(session.lane_scratch_angle_delta_ms[Lane::Scratch.index()], 160);
}

#[test]
fn rebase_pre_ready_visual_times_keeps_timer_elapsed_across_clock_reset() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.scratch_angle_last_render_at = Some(TimeUs(5_000_000));
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(2_000_000));
    session.lane_keyoff_started_at[Lane::Key2.index()] = Some(TimeUs(3_000_000));
    session.recent_inputs.push(InputEvent {
        lane: Lane::Key1,
        kind: InputKind::Press,
        time: TimeUs(4_900_000),
        source: InputSource::Human,
        device_kind: InputDeviceKind::Keyboard,
        scratch_direction: None,
    });

    rebase_pre_ready_visual_times(&mut session, TimeUs(-1_000_000));

    assert_eq!(session.lane_keyon_started_at[Lane::Key1.index()], Some(TimeUs(-4_000_000)));
    assert_eq!(session.lane_keyoff_started_at[Lane::Key2.index()], Some(TimeUs(-3_000_000)));
    assert_eq!(session.recent_inputs[0].time, TimeUs(-1_100_000));
    assert_eq!(session.scratch_angle_last_render_at, Some(TimeUs(-1_000_000)));

    // READY/PLAYに移行しても押下状態は保たれ、実際のReleaseでのみ解除される。
    update_lane_key_states(
        &mut session,
        &[InputEvent {
            lane: Lane::Key1,
            kind: InputKind::Release,
            time: TimeUs(-500_000),
            source: InputSource::Human,
            device_kind: InputDeviceKind::Keyboard,
            scratch_direction: None,
        }],
    );
    assert_eq!(session.lane_keyon_started_at[Lane::Key1.index()], None);
    assert_eq!(session.lane_keyoff_started_at[Lane::Key1.index()], Some(TimeUs(-500_000)));
}

#[test]
fn sync_input_timestamp_anchor_tracks_running_audio_clock() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.input_timestamp_anchor =
        Some(InputTimestampAnchor { monotonic_ns: 123, audio_time: TimeUs(456) });

    sync_input_timestamp_anchor(&mut session, TimeUs(1_234_567));

    assert!(session.input_timestamp_anchor.is_none());

    session.audio_clock.running = true;
    sync_input_timestamp_anchor(&mut session, TimeUs(1_234_567));

    let anchor = session.input_timestamp_anchor.unwrap();
    assert_eq!(anchor.audio_time, TimeUs(1_234_567));
    assert!(anchor.monotonic_ns <= monotonic_timestamp_ns());
}

#[test]
fn process_human_inputs_uses_monotonic_event_time() {
    use crate::input::backend::{
        BufferedInputBackend, DeviceId, DeviceInputEvent, DeviceTimestamp, PhysicalControl,
    };
    use crate::input::binding::{BindingEntry, LaneBinding};

    let mut session = session_with_autoplay(chart_with_bgm());
    session.autoplay = None;
    session.audio_clock.running = true;
    session.audio_clock.current_frame = Arc::new(AtomicU64::new(48_000));
    session.input_timestamp_anchor =
        Some(InputTimestampAnchor { monotonic_ns: 2_000_000, audio_time: TimeUs(1_000_000) });
    let mut backend = BufferedInputBackend::default();
    backend.push(DeviceInputEvent {
        device: DeviceId(1),
        control: PhysicalControl::KeyboardKey("Z".to_string()),
        kind: InputKind::Press,
        timestamp: DeviceTimestamp::MonotonicNs(1_500_000),
        bounce_policy: Default::default(),
    });
    session.input_system = InputSystem {
        backend: Box::new(backend),
        translator: Box::new(DefaultInputTranslator {
            binding: LaneBinding {
                entries: vec![BindingEntry {
                    device: None,
                    control: PhysicalControl::KeyboardKey("Z".to_string()),
                    lane: Lane::Key1,
                    scratch_direction: None,
                }],
            },
        }),
        bounce_filter: Default::default(),
    };

    let judgements = process_human_inputs(&mut session);

    assert!(judgements.is_empty());
    assert_eq!(session.replay_recorder.events[0].time, TimeUs(999_500));
    assert_eq!(session.lane_keyon_started_at[Lane::Key1.index()], Some(TimeUs(999_500)));
}

#[test]
fn drain_pre_ready_visual_inputs_updates_lane_key_states_without_judging() {
    use crate::input::backend::{
        BufferedInputBackend, DeviceId, DeviceInputEvent, DeviceTimestamp, PhysicalControl,
    };
    use crate::input::binding::{BindingEntry, LaneBinding};

    let mut session = session_with_autoplay(chart_with_keysound());
    session.autoplay = None;
    let mut backend = BufferedInputBackend::default();
    backend.push(DeviceInputEvent {
        device: DeviceId(1),
        control: PhysicalControl::KeyboardKey("Z".to_string()),
        kind: InputKind::Press,
        timestamp: DeviceTimestamp::Unknown,
        bounce_policy: Default::default(),
    });
    session.input_system = InputSystem {
        backend: Box::new(backend),
        translator: Box::new(DefaultInputTranslator {
            binding: LaneBinding {
                entries: vec![BindingEntry {
                    device: None,
                    control: PhysicalControl::KeyboardKey("Z".to_string()),
                    lane: Lane::Key1,
                    scratch_direction: None,
                }],
            },
        }),
        bounce_filter: Default::default(),
    };

    drain_pre_ready_visual_inputs(&mut session, TimeUs(2_000_000));

    assert_eq!(session.lane_keyon_started_at[Lane::Key1.index()], Some(TimeUs(2_000_000)));
    assert_eq!(session.recent_inputs.len(), 1);
    assert_eq!(session.score.past_notes, 0);
}

#[test]
fn drain_pre_ready_visual_inputs_advances_scratch_phase() {
    use crate::input::backend::{
        BufferedInputBackend, DeviceId, DeviceInputEvent, DeviceTimestamp, PhysicalControl,
    };
    use crate::input::binding::{BindingEntry, LaneBinding};

    let mut session = session_with_autoplay(chart_with_keysound());
    session.autoplay = None;
    let mut backend = BufferedInputBackend::default();
    backend.push(DeviceInputEvent {
        device: DeviceId(1),
        control: PhysicalControl::GamepadButton("scratch-up".to_string()),
        kind: InputKind::Press,
        timestamp: DeviceTimestamp::Unknown,
        bounce_policy: Default::default(),
    });
    session.input_system = InputSystem {
        backend: Box::new(backend),
        translator: Box::new(DefaultInputTranslator {
            binding: LaneBinding {
                entries: vec![BindingEntry {
                    device: None,
                    control: PhysicalControl::GamepadButton("scratch-up".to_string()),
                    lane: Lane::Scratch,
                    scratch_direction: Some(ScratchDirection::Up),
                }],
            },
        }),
        bounce_filter: Default::default(),
    };

    drain_pre_ready_visual_inputs(&mut session, TimeUs(2_000_000));
    drain_pre_ready_visual_inputs(&mut session, TimeUs(2_500_000));

    assert_eq!(session.lane_scratch_angle_delta_ms[Lane::Scratch.index()], 1_000);
    assert_eq!(session.scratch_angle_last_render_at, Some(TimeUs(2_500_000)));
    assert_eq!(session.score.past_notes, 0);
    assert!(session.replay_recorder.events.is_empty());
}

#[test]
fn drain_pre_ready_visual_inputs_discards_human_inputs_during_full_autoplay() {
    use crate::input::backend::{
        BufferedInputBackend, DeviceId, DeviceInputEvent, DeviceTimestamp, PhysicalControl,
    };
    use crate::input::binding::{BindingEntry, LaneBinding};

    let mut session = session_with_autoplay(chart_with_keysound());
    let mut backend = BufferedInputBackend::default();
    backend.push(DeviceInputEvent {
        device: DeviceId(1),
        control: PhysicalControl::KeyboardKey("Z".to_string()),
        kind: InputKind::Press,
        timestamp: DeviceTimestamp::Unknown,
        bounce_policy: Default::default(),
    });
    session.input_system = InputSystem {
        backend: Box::new(backend),
        translator: Box::new(DefaultInputTranslator {
            binding: LaneBinding {
                entries: vec![BindingEntry {
                    device: None,
                    control: PhysicalControl::KeyboardKey("Z".to_string()),
                    lane: Lane::Key1,
                    scratch_direction: None,
                }],
            },
        }),
        bounce_filter: Default::default(),
    };

    drain_pre_ready_visual_inputs(&mut session, TimeUs(2_000_000));

    assert_eq!(session.lane_keyon_started_at[Lane::Key1.index()], None);
    assert!(session.recent_inputs.is_empty());
    assert_eq!(session.score.past_notes, 0);
    assert!(session.replay_recorder.events.is_empty());
}

#[test]
fn update_lane_key_states_release_transitions_to_keyoff() {
    let mut session = session_with_autoplay(chart_with_keysound());
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(1_000));

    let inputs = [InputEvent {
        lane: Lane::Key1,
        kind: InputKind::Release,
        time: TimeUs(10_000),
        source: InputSource::Human,
        device_kind: InputDeviceKind::Keyboard,
        scratch_direction: None,
    }];
    update_lane_key_states(&mut session, &inputs);

    assert_eq!(session.lane_keyon_started_at[Lane::Key1.index()], None);
    assert_eq!(session.lane_keyoff_started_at[Lane::Key1.index()], Some(TimeUs(10_000)));
}

#[test]
fn update_lane_key_states_autoplay_press_schedules_auto_release() {
    let mut session = session_with_autoplay(chart_with_keysound());

    let inputs = [InputEvent {
        lane: Lane::Key1,
        kind: InputKind::Press,
        time: TimeUs(5_000),
        source: InputSource::Auto,
        device_kind: InputDeviceKind::Keyboard,
        scratch_direction: None,
    }];
    update_lane_key_states(&mut session, &inputs);

    // Auto は AUTO_KEYBEAM_DURATION_US (80ms) 後に自動 release が予約される。
    assert_eq!(
        session.lane_auto_release_at[Lane::Key1.index()],
        Some(TimeUs(5_000 + AUTO_KEYBEAM_DURATION_US))
    );
}

#[test]
fn apply_auto_key_release_transitions_after_duration() {
    let mut session = session_with_autoplay(chart_with_keysound());
    let inputs = [InputEvent {
        lane: Lane::Key1,
        kind: InputKind::Press,
        time: TimeUs(0),
        source: InputSource::Auto,
        device_kind: InputDeviceKind::Keyboard,
        scratch_direction: None,
    }];
    update_lane_key_states(&mut session, &inputs);

    // 期限前: 何も起きない。
    apply_auto_key_release(&mut session, TimeUs(AUTO_KEYBEAM_DURATION_US - 1));
    assert!(session.lane_keyon_started_at[Lane::Key1.index()].is_some());
    assert!(session.lane_keyoff_started_at[Lane::Key1.index()].is_none());

    // 期限到達: keyon → keyoff へ遷移。
    apply_auto_key_release(&mut session, TimeUs(AUTO_KEYBEAM_DURATION_US));
    assert!(session.lane_keyon_started_at[Lane::Key1.index()].is_none());
    assert_eq!(
        session.lane_keyoff_started_at[Lane::Key1.index()],
        Some(TimeUs(AUTO_KEYBEAM_DURATION_US))
    );
    assert!(session.lane_auto_release_at[Lane::Key1.index()].is_none());
}

#[test]
fn update_recent_inputs_keeps_presses_and_expires_old_events() {
    let mut session = session_with_autoplay(chart_with_keysound());
    let inputs = [
        InputEvent {
            lane: Lane::Key1,
            kind: InputKind::Press,
            time: TimeUs(10_000),
            source: InputSource::Human,
            device_kind: InputDeviceKind::Keyboard,
            scratch_direction: None,
        },
        InputEvent {
            lane: Lane::Key2,
            kind: InputKind::Release,
            time: TimeUs(20_000),
            source: InputSource::Human,
            device_kind: InputDeviceKind::Keyboard,
            scratch_direction: None,
        },
    ];

    update_recent_inputs(&mut session, &inputs, TimeUs(10_000));
    assert_eq!(session.recent_inputs.len(), 1);
    assert_eq!(session.recent_inputs[0].lane, Lane::Key1);
    update_recent_inputs(&mut session, &[], TimeUs(10_000 + INPUT_DISPLAY_US + 1));

    assert!(session.recent_inputs.is_empty());
}

fn session_with_autoplay(chart: PlayableChart) -> GameSession {
    let chart = Arc::new(chart);
    let timing_map =
        TimingMap::from_chart_timing_events(chart.metadata.initial_bpm, &chart.timing_events);
    GameSession {
        chart: Arc::clone(&chart),
        primary_key_mode: chart.metadata.key_mode,
        scored_total_notes: scored_note_count(&chart),
        timing_map,
        audio_clock: AudioClock {
            sample_rate: 48_000,
            start_output_frame: 0,
            chart_zero_time_us: 0,
            current_frame: Arc::new(AtomicU64::new(0)),
            running: false,
        },
        input_system: InputSystem {
            backend: Box::new(NullInputBackend),
            translator: Box::new(DefaultInputTranslator {
                binding: LaneBinding { entries: Vec::new() },
            }),
            bounce_filter: Default::default(),
        },
        judge: JudgeEngine::new(JudgeWindow::symmetric(
            16_000, 40_000, 80_000, 120_000, 500_000, 200_000, 16_000,
        )),
        base_judge_window: JudgeWindow::symmetric(
            16_000, 40_000, 80_000, 120_000, 500_000, 200_000, 16_000,
        ),
        base_judge_windows: JudgeWindows::uniform(JudgeWindow::symmetric(
            16_000, 40_000, 80_000, 120_000, 500_000, 200_000, 16_000,
        )),
        rule_mode: RuleMode::Beatoraja,
        score: ScoreState::default(),
        opponent_score: None,
        course_combo_carry: 0,
        course_combo_carry_active: false,
        course_max_combo: 0,
        gauge: GaugeState::new(bmz_core::clear::GaugeType::Normal, 160.0, chart.total_notes),
        opponent_gauge: None,
        replay_recorder: ReplayRecorder::default(),
        replay_player: None,
        replay_lane_mask: None,
        display_only_lane_mask: [false; LANE_COUNT],
        autoplay: Some(AutoplayController::default()),
        recent_inputs: Vec::new(),
        lane_keyon_started_at: Default::default(),
        lane_keyoff_started_at: Default::default(),
        lane_scratch_direction: Default::default(),
        lane_scratch_angle_delta_ms: Default::default(),
        scratch_angle_last_render_at: None,
        lane_auto_release_at: Default::default(),
        recent_judgements: Vec::new(),
        pending_skin_events: Vec::new(),
        next_skin_event_sequence: 0,
        result_judgements: Default::default(),
        hit_error_ring: HitErrorRing::default(),
        gauge_increase_started_at: None,
        opponent_gauge_increase_started_at: None,
        gauge_max_started_at: None,
        opponent_gauge_max_started_at: None,
        full_combo_started_at: None,
        opponent_full_combo_started_at: None,
        bgm_scheduler: BgmScheduler::default(),
        offsets: PlayOffsets { input_offset_us: 0, visual_offset_us: 0 },
        input_offset_auto_adjust_enabled: false,
        input_offset_auto_adjust: None,
        audio_mix: PlayAudioMix {
            master_volume: 1.0,
            chart_normalization_gain: 1.0,
            normalize_chart_volume: true,
            key_volume: 1.0,
            bgm_volume: 1.0,
        },
        hispeed: 2.0,
        hispeed_mode: HispeedMode::Normal,
        target_green_number: 300,
        hsfix_base_bpm: 120.0,
        lift: 0.0,
        lane_cover: 0.0,
        lane_cover_visible: true,
        lane_cover_changing: false,
        lanecover_enabled: false,
        lift_enabled: true,
        hidden_enabled: false,
        hispeed_auto_adjust: false,
        hidden_cover: 0.0,
        skin_offsets: Vec::new(),
        bga_enabled: true,
        poor_bga_duration_us: 500_000,
        bga_stretch: 1,
        show_ln_tail_cap: false,
        lane_hcn_timer: [None; LANE_COUNT],
        lane_hcn_keysound_muted: [None; LANE_COUNT],
        pending_keysounds: Vec::new(),
        pending_keysound_volumes: Vec::new(),
        hsfix_index: 0,
        input_timestamp_anchor: None,
        pending_mine_hits: Vec::new(),
        state: PlayState::Ready,
        last_hcn_gauge_at: None,
    }
}

fn chart_with_keysound() -> PlayableChart {
    let note = NoteEvent {
        id: NoteId(1),
        lane: Lane::Key1,
        kind: NoteKind::Tap,
        tick: ChartTick(0),
        time: TimeUs(0),
        sound: Some(SoundId(7)),
        damage: None,
    };
    let mut lane_notes = std::array::from_fn(|_| Vec::new());
    lane_notes[Lane::Key1.index()].push(note);

    PlayableChart {
        identity: ChartIdentity { file_md5: [0; 16], file_sha256: [0; 32] },
        metadata: ChartMetadata::default(),
        lane_notes,
        long_notes: Vec::new(),
        bgm_events: Vec::new(),
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
        sounds: vec![SoundAssetRef { id: SoundId(7), path: "sound.wav".into() }],
        bga_assets: Vec::new(),
        total_notes: 1,
        end_time: TimeUs(0),
    }
}

fn chart_with_bgm() -> PlayableChart {
    PlayableChart {
        identity: ChartIdentity { file_md5: [0; 16], file_sha256: [0; 32] },
        metadata: ChartMetadata::default(),
        lane_notes: std::array::from_fn(|_| Vec::new()),
        long_notes: Vec::new(),
        bgm_events: vec![SoundEvent { tick: ChartTick(0), time: TimeUs(0), sound: SoundId(3) }],
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
        sounds: vec![SoundAssetRef { id: SoundId(3), path: "bgm.wav".into() }],
        bga_assets: Vec::new(),
        total_notes: 0,
        end_time: TimeUs(0),
    }
}

fn judgement_event(judge: Judge, delta_us: i64) -> JudgementEvent {
    JudgementEvent {
        note_id: Some(NoteId(1)),
        lane: Lane::Key1,
        judge,
        side: if delta_us < 0 { TimingSide::Fast } else { TimingSide::Slow },
        delta: TimeUs(delta_us),
        time: TimeUs(0),
        affects_score: true,
    }
}
