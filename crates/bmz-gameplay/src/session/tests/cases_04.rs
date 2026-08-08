use super::*;

#[test]
fn result_settles_after_slow_window_without_waiting_for_late_bgm() {
    let mut chart = chart_with_keysound();
    chart.bgm_events.push(SoundEvent {
        tick: ChartTick(384),
        time: TimeUs(2_000_000),
        sound: SoundId(7),
    });
    let mut session = session_with_autoplay(chart);
    let outcome = session.judge.process_input(&session.chart, human_press(TimeUs(0)));

    assert!(outcome.events.iter().any(|event| event.judge == Judge::PGreat));
    assert!(session.judge.is_exhausted(&session.chart));
    assert!(!session.bgm_scheduler.is_done(&session.chart));
    // テスト用窓の最大 SLOW 側は Empty Poor の 200ms。境界上ではまだ受付中。
    assert!(!result_is_settled(&session, TimeUs(200_000)));
    assert!(result_is_settled(&session, TimeUs(200_001)));
    assert!(should_finish(&session, TimeUs(200_001)));
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
