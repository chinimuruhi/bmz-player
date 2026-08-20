use super::*;

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
    assert_eq!(adjusted_hispeed(20.0, HispeedChange::Up, 0.5), 20.0);
    assert_eq!(adjusted_hispeed(0.01, HispeedChange::Down, 0.5), 0.01);
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
fn pending_lane_state_preserves_sub_one_hsfix_bpm() {
    let mut snapshot =
        RenderSnapshot { hispeed_mode_index: 1, min_bpm: 0.96, ..Default::default() };
    let mut lane = PendingPlayLaneState::from_snapshot(&snapshot, 300, HsFixOption::MinBpm, false);
    assert_eq!(lane.hsfix_base_bpm, 0.96);

    snapshot.min_bpm = 0.5;
    lane.sync_chart_bpm(&snapshot, HsFixOption::MinBpm);
    assert_eq!(lane.hsfix_base_bpm, 0.5);
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
fn pending_lane_state_rejects_all_no_speed_controls() {
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

    for action in [
        PlayLaneAction::ToggleHispeedMode,
        PlayLaneAction::Hispeed(HispeedChange::Up),
        PlayLaneAction::LaneCoverDelta(-LANE_COVER_STEP),
        PlayLaneAction::GreenNumberDelta(1),
        PlayLaneAction::ToggleLaneCoverVisibility,
    ] {
        assert!(
            !apply_pending_play_lane_action_to_state(&mut lane, action, &profile, 120.0, true,)
        );
    }
    assert_eq!(lane.hispeed_mode, HispeedMode::Floating);
    assert_eq!(lane.hispeed, 2.0);
    assert_eq!(lane.target_green_number, 300);
    assert_eq!(lane.lane_cover, 0.0);
    assert_eq!(lane.lift, 0.0);
    assert!(lane.lane_cover_visible);
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
