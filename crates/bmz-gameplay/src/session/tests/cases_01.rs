use super::*;

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
