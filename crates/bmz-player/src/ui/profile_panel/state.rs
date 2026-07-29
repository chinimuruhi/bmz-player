pub(super) struct ProfileSettingsPanelActions {
    pub(super) save: bool,
    pub(super) save_app_config: bool,
}

pub(super) fn scene_restricts_settings(scene: &str) -> bool {
    matches!(scene, "Decide" | "Play")
}

pub(super) fn restore_restricted_profile_settings(
    profile: &mut ProfileConfig,
    mut readonly: ProfileConfig,
) {
    readonly.audio_mix = profile.audio_mix.clone();
    readonly.judge = profile.judge.clone();
    readonly.lane = profile.lane.clone();
    readonly.input = profile.input.clone();
    *profile = readonly;
}
