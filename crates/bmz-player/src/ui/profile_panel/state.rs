pub(in crate::ui) struct ProfileSettingsPanelActions {
    pub(in crate::ui) save: bool,
    pub(in crate::ui) save_app_config: bool,
}

pub(in crate::ui) fn scene_restricts_settings(scene: &str) -> bool {
    matches!(scene, "Decide" | "Play")
}

pub(in crate::ui) fn restore_restricted_profile_settings(
    profile: &mut ProfileConfig,
    mut readonly: ProfileConfig,
) {
    readonly.audio_mix = profile.audio_mix.clone();
    readonly.judge = profile.judge.clone();
    readonly.lane = profile.lane.clone();
    readonly.input = profile.input.clone();
    *profile = readonly;
}
use super::*;
