use super::*;

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
