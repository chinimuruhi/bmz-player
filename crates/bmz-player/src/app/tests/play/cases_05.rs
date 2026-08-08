use super::*;
use crate::app::scene_state::session_mode_overlay_suffix;

#[test]
fn session_mode_overlay_suffix_distinguishes_all_modes() {
    assert_eq!(session_mode_overlay_suffix(SessionMode::Normal), None);
    assert_eq!(session_mode_overlay_suffix(SessionMode::Autoplay), Some("autoplay"));
    assert_eq!(session_mode_overlay_suffix(SessionMode::AutoplayBattle), Some("auto battle"));
    assert_eq!(session_mode_overlay_suffix(SessionMode::GhostBattle), Some("battle"));
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
fn returning_to_select_reshuffles_system_sound_sets() {
    assert!(!should_shuffle_system_sound_sets_on_scene_enter(None, AppSceneKind::Select));
    assert!(!should_shuffle_system_sound_sets_on_scene_enter(
        Some(AppSceneKind::Select),
        AppSceneKind::Select,
    ));
    for previous in [AppSceneKind::Decide, AppSceneKind::Play, AppSceneKind::Result] {
        assert!(
            should_shuffle_system_sound_sets_on_scene_enter(Some(previous), AppSceneKind::Select),
            "previous={previous:?}"
        );
    }
    for next in [AppSceneKind::Decide, AppSceneKind::Play, AppSceneKind::Result] {
        assert!(
            !should_shuffle_system_sound_sets_on_scene_enter(Some(AppSceneKind::Select), next,)
        );
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
