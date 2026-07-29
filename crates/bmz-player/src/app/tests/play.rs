use super::*;

#[test]
fn smoke_play_frame_counter_only_exits_at_the_requested_count() {
    assert_eq!(count_smoke_play_frame(0, 3), (1, false));
    assert_eq!(count_smoke_play_frame(2, 3), (3, true));
    assert_eq!(count_smoke_play_frame(u32::MAX, 1), (u32::MAX, true));
}

#[test]
fn player_name_and_fps_are_applied_to_every_scene() {
    let mut scenes = [
        AppSceneSnapshot::Select(SelectSnapshot::default()),
        AppSceneSnapshot::Play(RenderSnapshot::default()),
        bmz_render::sample::sample_result_scene(),
    ];

    for scene in &mut scenes {
        apply_skin_runtime_info_to_scene(scene, "Test Player", 237);
        match scene {
            AppSceneSnapshot::Select(snapshot) => {
                assert_eq!(snapshot.player_name, "Test Player");
                assert_eq!(snapshot.current_fps, 237);
            }
            AppSceneSnapshot::Decide(snapshot) | AppSceneSnapshot::Play(snapshot) => {
                assert_eq!(snapshot.player_name, "Test Player");
                assert_eq!(snapshot.current_fps, 237);
            }
            AppSceneSnapshot::Result(snapshot) => {
                assert_eq!(snapshot.player_name, "Test Player");
                assert_eq!(snapshot.current_fps, 237);
            }
        }
    }
}

#[test]
fn active_play_visual_offset_sync_preserves_auto_adjusted_value() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);

    sync_active_play_visual_offset_to_profile(&mut profile, 1_000, true);

    assert_eq!(profile.judge.visual_offset_us, 1_000);
    assert_eq!(crate::config::play::play_offsets_from_profile(&profile).visual_offset_us, 1_000);

    sync_active_play_visual_offset_to_profile(&mut profile, 2_000, false);
    assert_eq!(profile.judge.visual_offset_us, 1_000);
}

#[test]
fn pending_play_uses_preload_input_before_session_install() {
    use bmz_core::input::InputKind;
    use bmz_gameplay::input::backend::InputBackend;

    let preload_input = SharedInputBackend::default();
    assert!(play_input_backend_for_context(None, false, None, Some(&preload_input)).is_none());

    let selected = play_input_backend_for_context(None, true, None, Some(&preload_input)).unwrap();
    crate::input::winit::handle_key_parts(
        &selected,
        PhysicalKey::Code(KeyCode::KeyZ),
        ElementState::Pressed,
        false,
    );

    let events = preload_input.clone().drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, InputKind::Press);
}

#[test]
fn pending_play_input_updates_keybeam_before_session_install() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let binding = crate::config::play::lane_binding_for_chart_with_slots(
        &profile.input,
        KeyMode::K7,
        Default::default(),
    );
    let mut visual = PendingPlayVisualInput::new(KeyMode::K7, binding, false);
    let press = physical_key_to_device_input(
        PhysicalKey::Code(KeyCode::KeyZ),
        ElementState::Pressed,
        false,
    )
    .unwrap();

    visual.apply_event(&press, TimeUs(100_000));
    let mut snapshot = RenderSnapshot::default();
    crate::screens::play_snapshot::refresh_pending_play_input_visuals(
        &mut snapshot,
        visual.key_mode,
        visual.lane_keyon_started_at,
        visual.lane_keyoff_started_at,
        visual.lane_scratch_angle_delta_ms,
        TimeUs(150_000),
    );

    assert_eq!(snapshot.keyon_ms[Lane::Key1.index()], Some(50));
    assert_eq!(snapshot.keyoff_ms[Lane::Key1.index()], None);

    let release = physical_key_to_device_input(
        PhysicalKey::Code(KeyCode::KeyZ),
        ElementState::Released,
        false,
    )
    .unwrap();
    visual.apply_event(&release, TimeUs(160_000));
    crate::screens::play_snapshot::refresh_pending_play_input_visuals(
        &mut snapshot,
        visual.key_mode,
        visual.lane_keyon_started_at,
        visual.lane_keyoff_started_at,
        visual.lane_scratch_angle_delta_ms,
        TimeUs(175_000),
    );
    assert_eq!(snapshot.keyon_ms[Lane::Key1.index()], None);
    assert_eq!(snapshot.keyoff_ms[Lane::Key1.index()], Some(15));
}

#[test]
fn pending_play_input_state_hands_off_without_resetting_keybeam_timer() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let binding = crate::config::play::lane_binding_for_chart_with_slots(
        &profile.input,
        KeyMode::K7,
        Default::default(),
    );
    let mut visual = PendingPlayVisualInput::new(KeyMode::K7, binding, false);
    let press = physical_key_to_device_input(
        PhysicalKey::Code(KeyCode::KeyZ),
        ElementState::Pressed,
        false,
    )
    .unwrap();
    visual.apply_event(&press, TimeUs(100_000));
    let input = SharedInputBackend::default();
    input.push_shared_event(press);
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(app_test_chart()),
        &profile,
        crate::screens::play_session::PlaySessionOptions::default(),
    );

    handoff_pending_play_visual_input(&mut session, &input, &visual);
    let mut snapshot = RenderSnapshot { play_elapsed_time: TimeUs(150_000), ..Default::default() };
    crate::screens::play_snapshot::refresh_play_skin_visuals_with_input_elapsed(
        &mut snapshot,
        &session,
        TimeUs(150_000),
    );

    assert_eq!(session.lane_keyon_started_at[Lane::Key1.index()], Some(TimeUs(100_000)));
    assert_eq!(snapshot.keyon_ms[Lane::Key1.index()], Some(50));
    assert!(input.clone().drain_events().is_empty());
}

#[test]
fn pending_play_input_suppresses_human_keybeam_for_full_autoplay() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let binding = crate::config::play::lane_binding_for_chart_with_slots(
        &profile.input,
        KeyMode::K7,
        Default::default(),
    );
    let mut visual = PendingPlayVisualInput::new(KeyMode::K7, binding, true);
    let press = physical_key_to_device_input(
        PhysicalKey::Code(KeyCode::KeyZ),
        ElementState::Pressed,
        false,
    )
    .unwrap();

    visual.apply_event(&press, TimeUs(100_000));

    assert_eq!(visual.lane_keyon_started_at[Lane::Key1.index()], None);
}

#[test]
fn play_control_hold_state_rebuilds_from_pressed_controls() {
    let input = crate::config::play_input::default_profile_input();
    let play_input = play_option_input_for(&input, KeyMode::K7);
    let keyboard =
        |control: &str| (W_KEYBOARD_DEVICE_ID, PhysicalControl::KeyboardKey(control.to_string()));
    let pressed = HashSet::from([keyboard("Q"), keyboard("W"), keyboard("E")]);

    assert_eq!(
        play_control_hold_state_from_pressed_inputs(&pressed, &play_input),
        (true, true, true)
    );

    let pressed = HashSet::from([keyboard("Q")]);
    assert_eq!(
        play_control_hold_state_from_pressed_inputs(&pressed, &play_input),
        (true, false, false)
    );

    let pressed = HashSet::from([keyboard("W")]);
    assert_eq!(
        play_control_hold_state_from_pressed_inputs(&pressed, &play_input),
        (false, true, false)
    );
}

#[test]
fn play_control_hold_state_keeps_legacy_and_default_e1_fallbacks() {
    let mut legacy_input = crate::config::play_input::default_profile_input();
    legacy_input.ui.bindings.retain(|entry| entry.action != Some(InputActionConfig::E1));
    legacy_input.start_key = Some("E".to_string());
    let legacy_play_input = play_option_input_for(&legacy_input, KeyMode::K7);
    let legacy_pressed =
        HashSet::from([(W_KEYBOARD_DEVICE_ID, PhysicalControl::KeyboardKey("E".to_string()))]);
    assert_eq!(
        play_control_hold_state_from_pressed_inputs(&legacy_pressed, &legacy_play_input),
        (true, false, true)
    );

    legacy_input.start_key = None;
    let fallback_play_input = play_option_input_for(&legacy_input, KeyMode::K7);
    let fallback_pressed =
        HashSet::from([(W_KEYBOARD_DEVICE_ID, PhysicalControl::KeyboardKey("Q".to_string()))]);
    assert_eq!(
        play_control_hold_state_from_pressed_inputs(&fallback_pressed, &fallback_play_input),
        (true, false, false)
    );
}

#[test]
fn play_ready_is_blocked_while_e1_or_e2_is_held() {
    assert!(!play_ready_blocked_by_control_holds(false, false));
    assert!(play_ready_blocked_by_control_holds(true, false));
    assert!(play_ready_blocked_by_control_holds(false, true));
    assert!(play_ready_blocked_by_control_holds(true, true));
}

#[test]
fn play_ready_waits_one_second_after_last_e1_or_e2_hold() {
    let last_control_hold_at = Instant::now();

    assert!(play_ready_blocked_by_recent_control_hold(
        Some(last_control_hold_at),
        last_control_hold_at + Duration::from_millis(999)
    ));
    assert!(play_ready_blocked_by_recent_control_hold(
        Some(last_control_hold_at),
        last_control_hold_at + Duration::from_secs(1)
    ));
    assert!(!play_ready_blocked_by_recent_control_hold(
        Some(last_control_hold_at),
        last_control_hold_at + Duration::from_millis(1_001)
    ));
}

#[test]
fn play_ready_has_no_release_delay_without_prior_control_hold() {
    assert!(!play_ready_blocked_by_recent_control_hold(None, Instant::now()));
}

#[test]
fn play_analog_lane_cover_delta_maps_scratch_bindings() {
    let gamepad_keys =
        SelectKeyBindings::from_profile(&ProfileConfig::new_default("default", "Default", 1).input);

    assert_eq!(play_analog_lane_cover_delta("Axis1", 4, &gamepad_keys), Some(-4));
    assert_eq!(play_analog_lane_cover_delta("Axis1", -4, &gamepad_keys), Some(4));
    assert_eq!(play_analog_lane_cover_delta("Axis2", -4, &gamepad_keys), None);
    assert_eq!(play_analog_lane_cover_delta("Axis1", 0, &gamepad_keys), None);
}

#[test]
fn play_analog_green_number_uses_opposite_direction_from_lane_cover() {
    assert_eq!(green_number_change_from_analog_steps(1), GreenNumberChange::Up);
    assert_eq!(green_number_change_from_analog_steps(-1), GreenNumberChange::Down);
}

#[test]
fn play_exit_hold_timer_uses_beatoraja_default_duration() {
    let default_hold = Duration::from_millis(1_000);
    let start = Instant::now();
    let mut held_since = None;

    update_play_exit_hold_started_at(&mut held_since, true, false, start);
    assert!(held_since.is_none());

    update_play_exit_hold_started_at(&mut held_since, true, true, start);
    assert_eq!(held_since, Some(start));
    assert!(!play_exit_hold_elapsed(held_since, start + default_hold / 2, default_hold));
    assert!(play_exit_hold_elapsed(held_since, start + default_hold, default_hold));

    update_play_exit_hold_started_at(&mut held_since, false, true, start + default_hold);
    assert!(held_since.is_none());
}

#[test]
fn decide_control_action_skips_with_1p_and_2p_decide_keys() {
    let keys = select_keys_with_full_2p_bindings();

    assert_eq!(decide_control_action("Z", &keys), Some(DecideAction::Confirm));
    assert_eq!(decide_control_action("M", &keys), Some(DecideAction::Confirm));
    assert_eq!(decide_control_action("P2K7", &keys), Some(DecideAction::Confirm));
    assert_eq!(decide_control_action("S", &keys), None);
    assert_eq!(decide_control_action("P2K6", &keys), None);
}

#[test]
fn decide_cancel_chord_accepts_e1_e2_and_e2_e3() {
    assert!(decide_cancel_chord_pressed(true, true, false));
    assert!(decide_cancel_chord_pressed(false, true, true));
    assert!(decide_cancel_chord_pressed(true, true, true));
    assert!(!decide_cancel_chord_pressed(true, false, true));
    assert!(!decide_cancel_chord_pressed(false, true, false));
}

#[test]
fn decide_fadeout_scene_elapsed_enters_scene_tail_on_early_skip() {
    let elapsed = decide_fadeout_scene_elapsed(
        Duration::from_millis(100),
        Duration::from_millis(250),
        Duration::from_millis(2500),
        Duration::from_millis(1000),
        DecideFadeoutSceneTiming::DefaultTail,
    );

    assert_eq!(elapsed, Duration::from_millis(1750));
}

#[test]
fn decide_fadeout_scene_elapsed_stretches_detected_tail_fadeout() {
    let elapsed = decide_fadeout_scene_elapsed(
        Duration::from_millis(100),
        Duration::from_millis(500),
        Duration::from_millis(2500),
        Duration::from_millis(1000),
        DecideFadeoutSceneTiming::TailStart(Duration::from_millis(2300)),
    );

    assert_eq!(elapsed, Duration::from_millis(2400));
}

#[test]
fn decide_fadeout_scene_elapsed_stays_direct_when_timer_fadeout_exists() {
    let elapsed = decide_fadeout_scene_elapsed(
        Duration::from_millis(100),
        Duration::from_millis(0),
        Duration::from_millis(2500),
        Duration::from_millis(500),
        DecideFadeoutSceneTiming::DirectOnly,
    );

    assert_eq!(elapsed, Duration::from_millis(100));
}

#[test]
fn decide_fadeout_scene_elapsed_does_not_rewind_auto_fadeout() {
    let elapsed = decide_fadeout_scene_elapsed(
        Duration::from_millis(2500),
        Duration::from_millis(250),
        Duration::from_millis(2500),
        Duration::from_millis(1000),
        DecideFadeoutSceneTiming::DefaultTail,
    );

    assert_eq!(elapsed, Duration::from_millis(2750));
}

#[test]
fn decide_scene_fadeout_tail_start_detects_scene_end_black_fade() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 6,
                "w": 1920,
                "h": 1080,
                "scene": 2500,
                "fadeout": 1000,
                "destination": [
                    { "id": -110, "loop": 800, "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 1920, "h": 1080, "a": 255 },
                        { "time": 800, "a": 0 }
                    ] },
                    { "id": -110, "loop": 2500, "dst": [
                        { "time": 2300, "x": 0, "y": 0, "w": 1920, "h": 1080, "a": 0 },
                        { "time": 2500, "a": 255 }
                    ] }
                ]
            }
            "#,
    )
    .unwrap();

    assert_eq!(decide_scene_fadeout_tail_start(Some(&document)), Some(2300));
}

#[test]
fn decide_scene_fadeout_tail_start_ignores_scene_tail_when_timer_fadeout_exists() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 6,
                "w": 1920,
                "h": 1080,
                "scene": 2500,
                "fadeout": 500,
                "destination": [
                    { "id": -110, "loop": 2000, "dst": [
                        { "time": 1500, "x": 0, "y": 0, "w": 1920, "h": 1080, "a": 0 },
                        { "time": 2000, "a": 255 }
                    ] },
                    { "id": -110, "loop": 500, "timer": 2, "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 1920, "h": 1080, "a": 0 },
                        { "time": 500, "a": 255 }
                    ] }
                ]
            }
            "#,
    )
    .unwrap();

    assert!(document_has_fadeout_timer_black(&document));
    assert_eq!(decide_fadeout_scene_timing(Some(&document)), DecideFadeoutSceneTiming::DirectOnly);
    assert_eq!(decide_scene_fadeout_tail_start(Some(&document)), None);
}

#[test]
fn bga_option_cycles_on_auto_off() {
    assert!(matches!(cycle_bga_option(BgaModeConfig::On), BgaModeConfig::Auto));
    assert!(matches!(cycle_bga_option(BgaModeConfig::Auto), BgaModeConfig::Off));
    assert!(matches!(cycle_bga_option(BgaModeConfig::Off), BgaModeConfig::On));
}

#[test]
fn retry_preload_always_builds_fresh_audio_for_the_retried_chart() {
    assert_eq!(
        retry_preload_kind(ResultRetryMode::SameArrange, true),
        RetryPreloadKind::CachedChartWithFreshAudio
    );
    assert_eq!(
        retry_preload_kind(ResultRetryMode::SameArrange, false),
        RetryPreloadKind::ReimportedChartWithFreshAudio
    );
    assert_eq!(
        retry_preload_kind(ResultRetryMode::DifferentArrange, true),
        RetryPreloadKind::ReimportedChartWithFreshAudio
    );
    assert_eq!(
        retry_preload_kind(ResultRetryMode::DifferentArrange, false),
        RetryPreloadKind::ReimportedChartWithFreshAudio
    );
}

#[test]
fn hispeed_action_maps_left_and_right_presses() {
    assert_eq!(
        hispeed_action(PhysicalKey::Code(KeyCode::ArrowLeft), ElementState::Pressed, false),
        Some(HispeedChange::Down)
    );
    assert_eq!(
        hispeed_action(PhysicalKey::Code(KeyCode::ArrowRight), ElementState::Pressed, false),
        Some(HispeedChange::Up)
    );
}

#[test]
fn hispeed_action_rejects_releases_and_other_keys() {
    assert_eq!(
        hispeed_action(PhysicalKey::Code(KeyCode::ArrowLeft), ElementState::Released, false),
        None
    );
    assert_eq!(
        hispeed_action(PhysicalKey::Code(KeyCode::ArrowUp), ElementState::Pressed, false),
        None
    );
}

#[test]
fn adjusted_hispeed_uses_configured_step_and_clamps_range() {
    assert_eq!(adjusted_hispeed(2.0, HispeedChange::Up, 0.25), 2.25);
    assert_eq!(adjusted_hispeed(2.0, HispeedChange::Down, 0.25), 1.75);
    assert_eq!(adjusted_hispeed(2.0, HispeedChange::Up, 0.5), 2.5);
    assert_eq!(adjusted_hispeed(10.0, HispeedChange::Up, 0.5), 10.0);
    assert_eq!(adjusted_hispeed(0.5, HispeedChange::Down, 0.5), 0.5);
}

#[test]
fn pending_hispeed_changes_use_displayed_mode_without_mutating_profile() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let profile_hispeed = profile.lane.hispeed;
    let mut lane = PendingPlayLaneState {
        hispeed: 2.0,
        hispeed_mode: HispeedMode::Floating,
        target_green_number: 300,
        lane_cover: 0.0,
        lift: 0.0,
        lane_cover_visible: true,
        lane_cover_changing: false,
        hsfix_base_bpm: 120.0,
        hispeed_auto_adjust: false,
    };

    assert!(apply_pending_play_lane_action_to_state(
        &mut lane,
        PlayLaneAction::Hispeed(HispeedChange::Up),
        &profile,
        120.0,
        false,
    ));

    assert_eq!(lane.hispeed, 2.5);
    assert_eq!(lane.target_green_number, 300);
    assert_eq!(profile.lane.hispeed, profile_hispeed);
}

#[test]
fn pending_green_number_change_switches_displayed_state_to_floating() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut lane = PendingPlayLaneState {
        hispeed: 2.0,
        hispeed_mode: HispeedMode::Normal,
        target_green_number: 300,
        lane_cover: 0.0,
        lift: 0.0,
        lane_cover_visible: true,
        lane_cover_changing: true,
        hsfix_base_bpm: 120.0,
        hispeed_auto_adjust: false,
    };

    assert!(apply_pending_play_lane_action_to_state(
        &mut lane,
        PlayLaneAction::GreenNumberDelta(1),
        &profile,
        120.0,
        false,
    ));

    assert_eq!(lane.hispeed_mode, HispeedMode::Floating);
    assert_eq!(lane.target_green_number, 601);
    let expected =
        crate::screens::play_snapshot::hispeed_for_green_number_values(601.0, 1.0, 120.0, 1.0);
    assert!((lane.hispeed - expected).abs() < 0.000_1, "hispeed={}", lane.hispeed);
}

#[test]
fn pending_lane_state_matches_no_speed_control_rules() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut lane = PendingPlayLaneState {
        hispeed: 2.0,
        hispeed_mode: HispeedMode::Floating,
        target_green_number: 300,
        lane_cover: 0.0,
        lift: 0.0,
        lane_cover_visible: true,
        lane_cover_changing: true,
        hsfix_base_bpm: 120.0,
        hispeed_auto_adjust: false,
    };

    assert!(!apply_pending_play_lane_action_to_state(
        &mut lane,
        PlayLaneAction::Hispeed(HispeedChange::Up),
        &profile,
        120.0,
        true,
    ));
    assert!(apply_pending_play_lane_action_to_state(
        &mut lane,
        PlayLaneAction::LaneCoverDelta(-LANE_COVER_STEP),
        &profile,
        120.0,
        true,
    ));
    assert_eq!(lane.hispeed, 2.0);
    assert!((lane.lane_cover - LANE_COVER_STEP).abs() < f32::EPSILON);
}

#[test]
fn pending_lane_actions_replay_once_on_loaded_session() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.lane.hispeed_mode = HispeedModeConfig::Floating;
    profile.lane.target_green_number = 300;
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(app_test_chart()),
        &profile,
        crate::screens::play_session::PlaySessionOptions::default(),
    );
    let initial_hispeed = session.hispeed;
    let hispeed_step = hispeed_step_for_profile(&profile, session.hispeed_mode);

    replay_pending_play_lane_actions(
        &mut session,
        &[PlayLaneAction::Hispeed(HispeedChange::Up)],
        &profile,
        false,
    );

    assert_eq!(session.hispeed, initial_hispeed + hispeed_step);
    replay_pending_play_lane_actions(
        &mut session,
        &[PlayLaneAction::LaneCoverDelta(-LANE_COVER_STEP)],
        &profile,
        false,
    );
    assert!((session.lane_cover - LANE_COVER_STEP).abs() < f32::EPSILON);
}

#[test]
fn floating_hispeed_recalculation_uses_hsfix_base_before_chart_start() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.lane.hispeed_mode = HispeedModeConfig::Floating;
    profile.lane.target_green_number = 300;
    let mut chart = app_test_chart();
    chart.metadata.initial_bpm = 120.0;
    chart.timing_events.push(bmz_chart::model::TimingEvent {
        tick: bmz_core::time::ChartTick(48),
        time: TimeUs(1_000_000),
        kind: bmz_chart::model::TimingEventKind::BpmChange { bpm: 240.0 },
    });
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(chart),
        &profile,
        crate::screens::play_session::PlaySessionOptions {
            hs_fix: HsFixOption::MaxBpm,
            ..Default::default()
        },
    );
    session.lane_cover = 0.25;

    reset_floating_hispeed_if_enabled(&mut session, false);

    assert_eq!(session.hsfix_base_bpm, 240.0);
    assert!((session.hispeed - 1.5).abs() < 0.000_1, "hispeed={}", session.hispeed);
}

#[test]
fn floating_hispeed_recalculation_uses_current_bpm_after_chart_start() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.lane.hispeed_mode = HispeedModeConfig::Floating;
    profile.lane.hispeed_auto_adjust = true;
    profile.lane.target_green_number = 300;
    let mut chart = app_test_chart();
    chart.metadata.initial_bpm = 120.0;
    chart.timing_events.push(bmz_chart::model::TimingEvent {
        tick: bmz_core::time::ChartTick(48),
        time: TimeUs(1_000_000),
        kind: bmz_chart::model::TimingEventKind::BpmChange { bpm: 240.0 },
    });
    let frame = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(chart),
        &profile,
        crate::screens::play_session::PlaySessionOptions {
            hs_fix: HsFixOption::MaxBpm,
            ..Default::default()
        },
    );
    session.audio_clock = bmz_audio::clock::AudioClock::with_position(48_000, 0, 0, frame, true);

    apply_lane_cover_step_to_session(&mut session, -0.25, false);

    assert_eq!(session.hsfix_base_bpm, 240.0);
    assert!((session.hispeed - 3.0).abs() < 0.000_1, "hispeed={}", session.hispeed);
}

#[test]
fn lane_cover_change_uses_hsfix_base_when_hispeed_auto_adjust_is_off() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.lane.hispeed_mode = HispeedModeConfig::Floating;
    profile.lane.hispeed_auto_adjust = false;
    profile.lane.target_green_number = 300;
    let mut chart = app_test_chart();
    chart.metadata.initial_bpm = 120.0;
    chart.timing_events.push(bmz_chart::model::TimingEvent {
        tick: bmz_core::time::ChartTick(48),
        time: TimeUs(1_000_000),
        kind: bmz_chart::model::TimingEventKind::BpmChange { bpm: 240.0 },
    });
    let frame = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(chart),
        &profile,
        crate::screens::play_session::PlaySessionOptions {
            hs_fix: HsFixOption::MaxBpm,
            ..Default::default()
        },
    );
    session.audio_clock = bmz_audio::clock::AudioClock::with_position(48_000, 0, 0, frame, true);

    apply_lane_cover_step_to_session(&mut session, -0.25, false);

    assert!(!session.hispeed_auto_adjust);
    assert!((session.hispeed - 1.5).abs() < 0.000_1, "hispeed={}", session.hispeed);
}

#[test]
fn egui_lane_profile_cover_change_keeps_runtime_nhs_hispeed() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let before = profile.lane.clone();
    let mut edited = profile.lane.clone();
    edited.sudden = 250;
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(app_test_chart()),
        &profile,
        crate::screens::play_session::PlaySessionOptions::default(),
    );
    session.hispeed = 3.5;

    assert!(apply_profile_lane_settings_to_session(&mut session, &before, &edited, false));
    assert!((session.hispeed - 3.5).abs() < f32::EPSILON);
    assert!((session.lane_cover - 0.25).abs() < f32::EPSILON);
}

#[test]
fn egui_lane_profile_target_change_recalculates_fhs_hispeed() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.lane.hispeed_mode = HispeedModeConfig::Floating;
    let before = profile.lane.clone();
    let mut edited = profile.lane.clone();
    edited.target_green_number = 320;
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(app_test_chart()),
        &profile,
        crate::screens::play_session::PlaySessionOptions {
            hs_fix: HsFixOption::StartBpm,
            ..Default::default()
        },
    );

    assert!(apply_profile_lane_settings_to_session(&mut session, &before, &edited, false));
    assert_eq!(session.hispeed_mode, HispeedMode::Floating);
    assert_eq!(session.target_green_number, 320);
    assert!((session.hispeed - 3.75).abs() < 0.000_1, "hispeed={}", session.hispeed);
}

#[test]
fn chart_started_for_system_sound_waits_until_running_clock_reaches_zero() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let frame = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(app_test_chart()),
        &profile,
        crate::screens::play_session::PlaySessionOptions::default(),
    );

    assert!(!chart_started_for_system_sound(&session));

    session.audio_clock =
        bmz_audio::clock::AudioClock::with_position(48_000, 0, -1_000_000, frame.clone(), true);
    assert!(!chart_started_for_system_sound(&session));

    frame.store(48_000, std::sync::atomic::Ordering::Relaxed);
    assert!(chart_started_for_system_sound(&session));
}

#[test]
fn lane_cover_step_moves_one_profile_unit() {
    assert!((LANE_COVER_STEP - 0.001).abs() < f32::EPSILON);
}

#[test]
fn lane_cover_step_accelerates_on_key_repeat() {
    assert_eq!(
        lane_cover_step(PhysicalKey::Code(KeyCode::ArrowUp), ElementState::Pressed, false),
        Some(0.001)
    );
    assert_eq!(
        lane_cover_step(PhysicalKey::Code(KeyCode::ArrowUp), ElementState::Pressed, true),
        Some(0.01)
    );
    assert_eq!(
        lane_cover_step(PhysicalKey::Code(KeyCode::ArrowDown), ElementState::Pressed, true),
        Some(-0.01)
    );
}

#[test]
fn lane_cover_step_clamps_sudden_and_lift_to_combined_range() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(app_test_chart()),
        &profile,
        crate::screens::play_session::PlaySessionOptions::default(),
    );

    session.lift = 0.2;
    session.lane_cover = 0.79;
    session.lane_cover_visible = true;
    assert!(apply_lane_cover_step_to_session(&mut session, -0.02, false));
    assert!((session.lane_cover - 0.8).abs() < 0.000_01);

    session.lane_cover = 0.3;
    session.lift = 0.69;
    session.lane_cover_visible = false;
    assert!(apply_lane_cover_step_to_session(&mut session, 0.02, false));
    assert!((session.lift - 0.7).abs() < 0.000_01);
}

#[test]
fn play_start_double_press_registers_within_window() {
    let mut last = None;
    let t0 = Instant::now();
    assert!(!register_play_start_double_press(&mut last, t0));
    assert_eq!(last, Some(t0));

    let t1 = t0 + Duration::from_millis(200);
    assert!(register_play_start_double_press(&mut last, t1));
    assert_eq!(last, None);
}

#[test]
fn play_start_double_press_expires_outside_window() {
    let mut last = None;
    let t0 = Instant::now();
    assert!(!register_play_start_double_press(&mut last, t0));

    let t1 = t0 + PLAY_START_DOUBLE_PRESS_WINDOW + Duration::from_millis(1);
    assert!(!register_play_start_double_press(&mut last, t1));
    assert_eq!(last, Some(t1));
}

#[test]
fn toggle_lane_cover_visibility_flips_sudden_display() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(app_test_chart()),
        &profile,
        crate::screens::play_session::PlaySessionOptions::default(),
    );
    session.lane_cover_visible = true;

    toggle_lane_cover_visibility(&mut session, false);
    assert!(!session.lane_cover_visible);

    toggle_lane_cover_visibility(&mut session, false);
    assert!(session.lane_cover_visible);
}

#[test]
fn green_number_step_switches_normal_hispeed_to_floating() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(app_test_chart()),
        &profile,
        crate::screens::play_session::PlaySessionOptions::default(),
    );

    assert!(apply_green_number_step_to_session(&mut session, 1, false));

    assert_eq!(session.hispeed_mode, HispeedMode::Floating);
    assert_eq!(session.target_green_number, 601);
    assert!(session.hispeed < 2.0);
}

#[test]
fn green_number_step_respects_no_speed_constraint() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(app_test_chart()),
        &profile,
        crate::screens::play_session::PlaySessionOptions::default(),
    );

    assert!(!apply_green_number_step_to_session(&mut session, 1, true));

    assert_eq!(session.hispeed_mode, HispeedMode::Normal);
    assert_eq!(session.target_green_number, 300);
    assert_eq!(session.hispeed, 2.0);
}

#[test]
fn floating_hispeed_change_keeps_target_green_during_play() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.lane.hispeed_mode = HispeedModeConfig::Floating;
    profile.lane.target_green_number = 300;
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(app_test_chart()),
        &profile,
        crate::screens::play_session::PlaySessionOptions {
            hs_fix: HsFixOption::StartBpm,
            ..Default::default()
        },
    );

    let hispeed = session.hispeed;
    apply_hispeed_change_to_session(&mut session, HispeedChange::Up, 0.5);

    assert_eq!(session.hispeed, hispeed + 0.5);
    assert_eq!(session.target_green_number, 300);
}

#[test]
fn e1_hispeed_change_keeps_target_green_during_play() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.lane.hispeed_mode = HispeedModeConfig::Floating;
    profile.lane.target_green_number = 300;
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(app_test_chart()),
        &profile,
        crate::screens::play_session::PlaySessionOptions {
            hs_fix: HsFixOption::StartBpm,
            ..Default::default()
        },
    );

    assert!(apply_play_option_control_to_session(
        &mut session,
        PlayOptionControl::Hispeed(HispeedChange::Up),
        false,
        0.5,
    ));

    assert_eq!(session.target_green_number, 300);
}

#[test]
fn active_lane_state_keeps_green_number_captured_when_switching_to_fhs() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = crate::screens::play_session::build_game_session(
        std::sync::Arc::new(app_test_chart()),
        &profile,
        crate::screens::play_session::PlaySessionOptions::default(),
    );
    let frame = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    session.audio_clock = bmz_audio::clock::AudioClock::with_position(48_000, 0, 0, frame, true);
    let expected_target = current_green_number(&session, session.audio_clock.now());
    assert_ne!(expected_target, session.target_green_number);

    assert!(apply_play_option_control_to_session(
        &mut session,
        PlayOptionControl::ToggleHispeedMode,
        false,
        0.25,
    ));
    assert_eq!(session.hispeed_mode, HispeedMode::Floating);
    assert_eq!(session.target_green_number, expected_target);

    // NHSへ戻ってHSを変更しても、終了時の現在緑数字でtargetを上書きしない。
    session.hispeed = 1.0;
    assert!(apply_play_option_control_to_session(
        &mut session,
        PlayOptionControl::ToggleHispeedMode,
        false,
        0.25,
    ));
    let state = active_lane_state_for_session(&session);

    assert_eq!(state.hispeed_mode, HispeedMode::Normal);
    assert_eq!(state.target_green_number, expected_target);
}

#[test]
fn play_option_control_maps_seven_key_lane_and_scratch_targets() {
    let input = crate::config::play_input::default_profile_input();
    let keys = SelectKeyBindings::from_profile(&input);
    let play_input = play_option_input_for(&input, KeyMode::K7);

    assert_eq!(
        keyboard_play_option("W", true, true, &keys, &play_input, &input),
        Some(PlayOptionControl::ToggleHispeedMode)
    );
    assert_eq!(
        keyboard_play_option("Z", true, false, &keys, &play_input, &input),
        Some(PlayOptionControl::Hispeed(HispeedChange::Down))
    );
    assert_eq!(
        keyboard_play_option("V", true, false, &keys, &play_input, &input),
        Some(PlayOptionControl::Hispeed(HispeedChange::Down))
    );
    assert_eq!(
        keyboard_play_option("S", true, false, &keys, &play_input, &input),
        Some(PlayOptionControl::Hispeed(HispeedChange::Up))
    );
    assert_eq!(
        keyboard_play_option("F", true, false, &keys, &play_input, &input),
        Some(PlayOptionControl::Hispeed(HispeedChange::Up))
    );
    assert_eq!(
        keyboard_play_option("LShift", true, false, &keys, &play_input, &input),
        Some(PlayOptionControl::LaneCover(LaneCoverChange::Up))
    );
    assert_eq!(
        keyboard_play_option("LControl", true, false, &keys, &play_input, &input),
        Some(PlayOptionControl::LaneCover(LaneCoverChange::Down))
    );
}

#[test]
fn play_option_control_maps_scratch_for_scratchless_key_modes() {
    let input = crate::config::play_input::default_profile_input();
    let keys = SelectKeyBindings::from_profile(&input);

    for key_mode in [KeyMode::K4, KeyMode::K6, KeyMode::K8, KeyMode::K9] {
        let play_input = play_option_input_for(&input, key_mode);
        assert_eq!(
            keyboard_play_option("LShift", true, false, &keys, &play_input, &input),
            Some(PlayOptionControl::LaneCover(LaneCoverChange::Up)),
            "{} Scratch Up",
            key_mode.as_str(),
        );
        assert_eq!(
            keyboard_play_option("LControl", true, false, &keys, &play_input, &input),
            Some(PlayOptionControl::LaneCover(LaneCoverChange::Down)),
            "{} Scratch Down",
            key_mode.as_str(),
        );
        assert_eq!(
            keyboard_play_option("LShift", false, true, &keys, &play_input, &input),
            Some(PlayOptionControl::GreenNumber(GreenNumberChange::Up)),
            "{} Scratch Up green number",
            key_mode.as_str(),
        );
        assert_eq!(
            keyboard_play_option("LControl", false, true, &keys, &play_input, &input),
            Some(PlayOptionControl::GreenNumber(GreenNumberChange::Down)),
            "{} Scratch Down green number",
            key_mode.as_str(),
        );
    }
}

#[test]
fn play_option_control_maps_e2_to_mode_specific_green_number_direction() {
    let input = crate::config::play_input::default_profile_input();
    let keys = SelectKeyBindings::from_profile(&input);
    let play_input = play_option_input_for(&input, KeyMode::K7);

    assert_eq!(
        keyboard_play_option("Z", false, true, &keys, &play_input, &input),
        Some(PlayOptionControl::GreenNumber(GreenNumberChange::Down))
    );
    assert_eq!(
        keyboard_play_option("S", false, true, &keys, &play_input, &input),
        Some(PlayOptionControl::GreenNumber(GreenNumberChange::Up))
    );
    assert_eq!(
        keyboard_play_option("LShift", false, true, &keys, &play_input, &input),
        Some(PlayOptionControl::GreenNumber(GreenNumberChange::Up))
    );
    assert_eq!(
        keyboard_play_option("LControl", false, true, &keys, &play_input, &input),
        Some(PlayOptionControl::GreenNumber(GreenNumberChange::Down))
    );
    assert_eq!(keyboard_play_option("Z", true, true, &keys, &play_input, &input), None);
}

#[test]
fn play_option_control_applies_eight_key_default_and_override() {
    let mut input = crate::config::play_input::default_profile_input();
    let keys = SelectKeyBindings::from_profile(&input);
    let play_input = play_option_input_for(&input, KeyMode::K8);

    assert_eq!(
        keyboard_play_option("Z", true, false, &keys, &play_input, &input),
        Some(PlayOptionControl::Hispeed(HispeedChange::Up))
    );
    assert!(crate::config::play_input::set_eight_key_hispeed_direction(
        &mut input,
        LaneConfig::Key1,
        HispeedDirectionConfig::Down,
    ));
    assert_eq!(
        keyboard_play_option("Z", true, false, &keys, &play_input, &input),
        Some(PlayOptionControl::Hispeed(HispeedChange::Down))
    );
}

#[test]
fn play_option_control_distinguishes_two_player_gamepads() {
    let mut input = crate::config::play_input::default_profile_input();
    input.play.insert(
        KeyMode::K14.play_map_key().to_string(),
        crate::config::profile_config::PlayModeInputConfig {
            inherit: None,
            bindings: vec![
                crate::config::play_input::gamepad_play_binding_for_device(
                    "gamepad1",
                    "Button1",
                    LaneConfig::Key1,
                ),
                crate::config::play_input::gamepad_play_binding_for_device(
                    "gamepad2",
                    "Button1",
                    LaneConfig::Key9,
                ),
            ],
            ..Default::default()
        },
    );
    let slots = crate::input::gamepad::GamepadSlotMap::from_device_ids([
        Some(DeviceId(11)),
        Some(DeviceId(22)),
    ]);
    let play_input = PlayOptionInput::new(
        KeyMode::K14,
        crate::config::play::lane_binding_for_chart_with_slots(&input, KeyMode::K14, slots),
        &input,
        slots,
    );
    let control = PhysicalControl::GamepadButton("Button1".to_string());

    assert_eq!(
        play_option_control_for_input(
            DeviceId(11),
            &control,
            true,
            false,
            Some(&play_input),
            &input,
        ),
        Some(PlayOptionControl::Hispeed(HispeedChange::Down))
    );
    assert_eq!(
        play_option_control_for_input(
            DeviceId(22),
            &control,
            true,
            false,
            Some(&play_input),
            &input,
        ),
        Some(PlayOptionControl::Hispeed(HispeedChange::Up))
    );
}

#[test]
fn bounce_bypass_requires_synthesized_axis_bound_to_profile_scratch_lane() {
    let mut input = crate::config::play_input::default_profile_input();
    input.play.insert(
        KeyMode::K14.play_map_key().to_string(),
        crate::config::profile_config::PlayModeInputConfig {
            inherit: None,
            bindings: vec![
                crate::config::play_input::gamepad_play_binding_for_device(
                    "gamepad1",
                    "Axis1+",
                    LaneConfig::Scratch,
                ),
                crate::config::play_input::gamepad_play_binding_for_device(
                    "gamepad1",
                    "Axis2+",
                    LaneConfig::Key1,
                ),
                crate::config::play_input::gamepad_play_binding_for_device(
                    "gamepad1",
                    "Axis3+",
                    LaneConfig::Scratch2,
                ),
            ],
            ..Default::default()
        },
    );
    let slots = crate::input::gamepad::GamepadSlotMap::from_device_ids([Some(DeviceId(11)), None]);
    let binding =
        crate::config::play::lane_binding_for_chart_with_slots(&input, KeyMode::K14, slots);
    let event = |name: &str, device_id, synthesized_analog_axis| {
        crate::input::gamepad::GamepadButtonEvent {
            name: name.to_string(),
            device_id,
            pressed: true,
            timestamp: bmz_gameplay::input::backend::DeviceTimestamp::MonotonicNs(1),
            synthesized_analog_axis,
        }
    };

    assert!(should_bypass_analog_scratch_bounce(
        &event("Axis1+", DeviceId(11), true),
        Some(&binding),
    ));
    assert!(!should_bypass_analog_scratch_bounce(
        &event("Axis2+", DeviceId(11), true),
        Some(&binding),
    ));
    assert!(should_bypass_analog_scratch_bounce(
        &event("Axis3+", DeviceId(11), true),
        Some(&binding),
    ));
    assert!(!should_bypass_analog_scratch_bounce(
        &event("Axis1+", DeviceId(11), false),
        Some(&binding),
    ));
    assert!(!should_bypass_analog_scratch_bounce(
        &event("Axis1+", DeviceId(22), true),
        Some(&binding),
    ));
    assert!(!should_bypass_analog_scratch_bounce(&event("Axis1+", DeviceId(11), true), None,));
}

#[test]
fn play_option_control_prioritizes_two_player_lane_over_other_devices_e2_button() {
    let mut input = crate::config::play_input::default_profile_input();
    input.ui.bindings.retain(|entry| {
        entry.action != Some(InputActionConfig::E2)
            || !crate::config::play_input::is_gamepad_device(&entry.device)
    });
    input.ui.bindings.push(crate::config::profile_config::BindingConfigEntry {
        device: "gamepad1".to_string(),
        control: "Button10".to_string(),
        keyboard_slot: None,
        lane: None,
        action: Some(InputActionConfig::E2),
        scratch: None,
    });
    input.play.insert(
        KeyMode::K14.play_map_key().to_string(),
        crate::config::profile_config::PlayModeInputConfig {
            inherit: None,
            bindings: vec![
                crate::config::play_input::gamepad_play_binding_for_device(
                    "gamepad1",
                    "Button1",
                    LaneConfig::Key1,
                ),
                crate::config::play_input::gamepad_play_binding_for_device(
                    "gamepad2",
                    "Button10",
                    LaneConfig::Key9,
                ),
            ],
            ..Default::default()
        },
    );
    let slots = crate::input::gamepad::GamepadSlotMap::from_device_ids([
        Some(DeviceId(11)),
        Some(DeviceId(22)),
    ]);
    let play_input = PlayOptionInput::new(
        KeyMode::K14,
        crate::config::play::lane_binding_for_chart_with_slots(&input, KeyMode::K14, slots),
        &input,
        slots,
    );
    let control = PhysicalControl::GamepadButton("Button10".to_string());

    assert_eq!(
        play_option_control_for_input(
            DeviceId(11),
            &control,
            true,
            true,
            Some(&play_input),
            &input,
        ),
        Some(PlayOptionControl::ToggleHispeedMode)
    );
    assert_eq!(
        play_option_control_for_input(
            DeviceId(22),
            &control,
            true,
            false,
            Some(&play_input),
            &input,
        ),
        Some(PlayOptionControl::Hispeed(HispeedChange::Up))
    );
    assert_eq!(
        play_option_control_for_input(
            DeviceId(22),
            &control,
            false,
            true,
            Some(&play_input),
            &input,
        ),
        Some(PlayOptionControl::GreenNumber(GreenNumberChange::Up))
    );

    let p2_lane_pressed = HashSet::from([(DeviceId(22), control.clone())]);
    assert_eq!(
        play_control_hold_state_from_pressed_inputs(&p2_lane_pressed, &play_input),
        (false, false, false)
    );
    let p1_e2_pressed = HashSet::from([(DeviceId(11), control)]);
    assert_eq!(
        play_control_hold_state_from_pressed_inputs(&p1_e2_pressed, &play_input),
        (false, true, false)
    );
}

#[test]
fn floating_hispeed_formula_uses_green_number_and_lane_cover() {
    assert_eq!(hispeed_for_green_number_values(300.0, 1.0, 120.0, 1.0), 4.0);
    assert_eq!(hispeed_for_green_number_values(300.0, 0.5, 120.0, 1.0), 2.0);
    assert_eq!(hispeed_for_green_number_values(300.0, 1.0, 240.0, 1.0), 2.0);
    assert_eq!(hispeed_for_green_number_values(300.0, 1.0, 120.0, 2.0), 2.0);
    assert!(
        (hispeed_for_green_number_values(295.0, 0.93, 120.0, 1.0) - 3.783_051).abs() < 0.000_01
    );
}

#[test]
fn green_number_change_uses_the_displayed_integer_duration() {
    assert_eq!(green_number_from_display_duration(500.0), 300);
    assert_eq!(green_number_from_display_duration(500.6), 301);
}

#[test]
fn active_lane_state_saves_current_green_number_for_nhs() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);

    apply_current_play_options_to_profile(
        &mut profile,
        Some(2.0),
        Some(ActiveLaneState {
            lane_cover: 0.0,
            lift: 0.0,
            hispeed_mode: HispeedMode::Normal,
            target_green_number: 600,
        }),
        CurrentPlayOptions {
            arrange: ArrangeOption::Normal,
            arrange_2p: ArrangeOption::Normal,
            target: TargetOption::None,
            gauge: GaugeTypeConfig::Normal,
            gauge_auto_shift: GaugeAutoShiftConfig::Off,
            bottom_shiftable_gauge: BottomShiftableGaugeConfig::Easy,
            double_option: DoubleOption::Off,
            hs_fix: HsFixOption::Off,
            session_mode: SessionMode::Normal,
        },
        42,
    );

    assert_eq!(profile.lane.hispeed_mode, HispeedModeConfig::Normal);
    assert_eq!(profile.lane.target_green_number, 600);
}

#[test]
fn normal_hispeed_rounding_restores_quarter_steps() {
    assert_eq!(clamp_hispeed_for_profile(3.783_051, HispeedModeConfig::Normal, 0.25), 3.75);
}

#[test]
fn custom_hispeed_step_preserves_non_quarter_profile_values() {
    assert_eq!(clamp_hispeed_for_profile(2.3, HispeedModeConfig::Normal, 0.3), 2.3);
    assert_eq!(clamp_hispeed_for_profile(2.37, HispeedModeConfig::Floating, 0.5), 2.37);
}

#[test]
fn gauge_option_cycle_includes_auto_shift() {
    assert_eq!(cycle_gauge_option(GaugeTypeConfig::ExHard), GaugeTypeConfig::Hazard);
    assert_eq!(
        cycle_gauge_auto_shift_option(GaugeAutoShiftConfig::Off),
        GaugeAutoShiftConfig::Continue
    );
    assert_eq!(gauge_auto_shift_as_str(GaugeAutoShiftConfig::BestClear), "BEST CLEAR");
    assert_eq!(
        cycle_bottom_shiftable_gauge_with_direction(BottomShiftableGaugeConfig::Normal, 1),
        BottomShiftableGaugeConfig::AssistEasy
    );
    assert_eq!(bottom_shiftable_gauge_as_str(BottomShiftableGaugeConfig::Easy), "EASY");
    assert_eq!(cycle_gauge_option(GaugeTypeConfig::AutoShift), GaugeTypeConfig::Hazard);
}

#[test]
fn apply_current_play_options_updates_profile_defaults() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);

    apply_current_play_options_to_profile(
        &mut profile,
        Some(3.37),
        Some(ActiveLaneState {
            lane_cover: 0.42,
            lift: 0.1,
            hispeed_mode: HispeedMode::Floating,
            target_green_number: 280,
        }),
        CurrentPlayOptions {
            arrange: ArrangeOption::Mirror,
            arrange_2p: ArrangeOption::Random,
            target: TargetOption::RankAaa,
            gauge: GaugeTypeConfig::Hard,
            gauge_auto_shift: GaugeAutoShiftConfig::BestClear,
            bottom_shiftable_gauge: BottomShiftableGaugeConfig::Normal,
            double_option: DoubleOption::Flip,
            hs_fix: HsFixOption::MainBpm,
            session_mode: SessionMode::Autoplay,
        },
        42,
    );

    assert_eq!(profile.lane.hispeed, 3.37);
    assert_eq!(profile.lane.sudden, 420);
    assert_eq!(profile.lane.lift, 100);
    assert_eq!(profile.lane.hispeed_mode, HispeedModeConfig::Floating);
    assert_eq!(profile.lane.target_green_number, 280);
    assert!(matches!(profile.play.random, RandomOptionConfig::Mirror));
    assert!(matches!(profile.play.random2, RandomOptionConfig::Random));
    assert!(matches!(profile.play.target, TargetOptionConfig::RankAaa));
    assert!(matches!(profile.play.gauge, GaugeTypeConfig::Hard));
    assert!(matches!(profile.play.gauge_auto_shift, GaugeAutoShiftConfig::BestClear));
    assert!(matches!(profile.play.bottom_shiftable_gauge, BottomShiftableGaugeConfig::Normal));
    assert!(matches!(profile.play.double_option, DoubleOptionConfig::Flip));
    assert!(matches!(profile.play.hs_fix, HsFixConfig::MainBpm));
    assert!(profile.play.auto_play);
    assert!(matches!(profile.play.assist, AssistOptionConfig::None));
    assert_eq!(profile.updated_at, 42);
}

#[test]
fn profile_play_option_changes_disable_random_and_autoplay_without_rollback() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.play.random = RandomOptionConfig::Random;
    profile.play.random2 = RandomOptionConfig::Mirror;
    profile.play.session_mode = None;
    profile.play.auto_play = true;
    let before = profile.play.clone();
    let current = select_play_options_from_profile(&before);

    profile.play.random = RandomOptionConfig::Off;
    profile.play.random2 = RandomOptionConfig::Off;
    profile.play.auto_play = false;
    let synced = merge_changed_select_play_options_from_profile(current, &before, &profile.play);

    assert_eq!(synced.arrange, ArrangeOption::Normal);
    assert_eq!(synced.arrange_2p, ArrangeOption::Normal);
    assert_eq!(synced.session_mode, SessionMode::Normal);

    apply_current_play_options_to_profile(&mut profile, None, None, synced, 42);
    assert_eq!(profile.play.random, RandomOptionConfig::Off);
    assert_eq!(profile.play.random2, RandomOptionConfig::Off);
    assert!(!profile.play.auto_play);
}

#[test]
fn session_mode_profile_migrates_legacy_autoplay_and_persists_battle() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.play.session_mode = None;
    profile.play.auto_play = true;
    assert_eq!(session_mode_from_profile(&profile.play), SessionMode::Autoplay);

    let mut options = select_play_options_from_profile(&profile.play);
    options.session_mode = SessionMode::GhostBattle;
    apply_current_play_options_to_profile(&mut profile, None, None, options, 2);

    assert_eq!(profile.play.session_mode, Some(SessionMode::GhostBattle));
    assert!(!profile.play.auto_play);
    let serialized = toml::to_string(&profile).unwrap();
    assert!(serialized.contains(r#"session_mode = "GhostBattle""#));
}

#[test]
fn profile_random_change_preserves_cli_autoplay_runtime_option() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let before = profile.play.clone();
    let mut current = select_play_options_from_profile(&before);
    current.session_mode = SessionMode::Autoplay;

    let mut after = before.clone();
    after.random = RandomOptionConfig::Mirror;
    let synced = merge_changed_select_play_options_from_profile(current, &before, &after);

    assert_eq!(synced.arrange, ArrangeOption::Mirror);
    assert_eq!(synced.session_mode, SessionMode::Autoplay);
}

#[test]
fn apply_lane_state_preserves_lift_amount_while_lift_is_disabled() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.lane.lift = 240;
    profile.lane.lift_enabled = false;

    apply_lane_state_to_profile(
        &mut profile,
        None,
        Some(ActiveLaneState {
            lane_cover: 0.3,
            lift: 0.0,
            hispeed_mode: HispeedMode::Normal,
            target_green_number: 300,
        }),
    );

    assert_eq!(profile.lane.lift, 240);
    assert!(!profile.lane.lift_enabled);
}

#[test]
fn arrange_option_maps_profile_random_defaults() {
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::Off), ArrangeOption::Normal);
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::Mirror), ArrangeOption::Mirror);
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::Random), ArrangeOption::Random);
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::RRandom), ArrangeOption::RRandom);
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::SRandom), ArrangeOption::SRandom);
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::Spiral), ArrangeOption::Spiral);
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::HRandom), ArrangeOption::HRandom);
    assert_eq!(
        arrange_option_from_profile(RandomOptionConfig::AllScratch),
        ArrangeOption::AllScratch
    );
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::RandomEx), ArrangeOption::RandomEx);
    assert_eq!(
        arrange_option_from_profile(RandomOptionConfig::SRandomEx),
        ArrangeOption::SRandomEx
    );
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::FRandom), ArrangeOption::FRandom);
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::MFRandom), ArrangeOption::MFRandom);
    assert!(matches!(random_config_from_arrange(ArrangeOption::Normal), RandomOptionConfig::Off));
    assert!(matches!(
        random_config_from_arrange(ArrangeOption::Mirror),
        RandomOptionConfig::Mirror
    ));
    assert!(matches!(
        random_config_from_arrange(ArrangeOption::Random),
        RandomOptionConfig::Random
    ));
    assert!(matches!(
        random_config_from_arrange(ArrangeOption::RRandom),
        RandomOptionConfig::RRandom
    ));
    assert!(matches!(
        random_config_from_arrange(ArrangeOption::SRandom),
        RandomOptionConfig::SRandom
    ));
    assert!(matches!(
        random_config_from_arrange(ArrangeOption::Spiral),
        RandomOptionConfig::Spiral
    ));
    assert!(matches!(
        random_config_from_arrange(ArrangeOption::HRandom),
        RandomOptionConfig::HRandom
    ));
    assert!(matches!(
        random_config_from_arrange(ArrangeOption::AllScratch),
        RandomOptionConfig::AllScratch
    ));
    assert!(matches!(
        random_config_from_arrange(ArrangeOption::RandomEx),
        RandomOptionConfig::RandomEx
    ));
    assert!(matches!(
        random_config_from_arrange(ArrangeOption::SRandomEx),
        RandomOptionConfig::SRandomEx
    ));
    assert!(matches!(
        random_config_from_arrange(ArrangeOption::FRandom),
        RandomOptionConfig::FRandom
    ));
    assert!(matches!(
        random_config_from_arrange(ArrangeOption::MFRandom),
        RandomOptionConfig::MFRandom
    ));
}

#[test]
fn play_scene_keeps_decide_bgm_until_chart_start() {
    use crate::system_sound::SoundType;

    let sounds = system_bgm_stop_targets_on_scene_enter(AppSceneKind::Play);

    assert!(sounds.contains(&SoundType::Select));
    assert!(!sounds.contains(&SoundType::Decide));
}

#[test]
fn non_play_scene_stops_all_transition_bgms() {
    use crate::system_sound::SoundType;

    for scene in [AppSceneKind::Select, AppSceneKind::Decide, AppSceneKind::Result] {
        let sounds = system_bgm_stop_targets_on_scene_enter(scene);
        assert!(sounds.contains(&SoundType::Select), "scene={scene:?}");
        assert!(sounds.contains(&SoundType::Decide), "scene={scene:?}");
    }
}

#[test]
fn left_overlay_hides_toast_while_screenshot_pending() {
    let toast = Some(("スクリーンショットを保存しました", Duration::from_millis(100)));
    assert_eq!(resolve_left_overlay_text(true, toast, "SCAN 1 / 2"), "SCAN 1 / 2");
    assert_eq!(
        resolve_left_overlay_text(false, toast, "SCAN 1 / 2"),
        "スクリーンショットを保存しました"
    );
}

#[test]
fn clear_rank_separates_unowned_from_noplay() {
    // 所持済み・スコア無し → NoPlay = 0。
    let noplay = select_chart_row(1);
    assert!(noplay.in_library());
    assert_eq!(clear_rank(&noplay), 0);

    // 難易度表エントリだがローカル未所持 → NoPlay より下位の -1。
    let mut unowned = select_chart_row(2);
    unowned.chart = None;
    unowned.entry_sha256 = Some([2u8; 32]);
    assert!(!unowned.in_library());
    assert_eq!(clear_rank(&unowned), -1);

    assert!(clear_rank(&unowned) < clear_rank(&noplay));
}
