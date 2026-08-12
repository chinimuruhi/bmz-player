use super::*;

pub const PLAY_MODE_CONFIG_MODES: [KeyMode; 8] = [
    KeyMode::K4,
    KeyMode::K5,
    KeyMode::K6,
    KeyMode::K7,
    KeyMode::K8,
    KeyMode::K9,
    KeyMode::K10,
    KeyMode::K14,
];

impl ProfileConfig {
    /// Returns the effective settings for `key_mode`. The active mode is read
    /// from the legacy-compatible editable mirror so unsaved UI changes are
    /// immediately visible to play/session construction.
    pub fn play_mode_config(&self, key_mode: KeyMode) -> PlayModeConfig {
        if key_mode == self.active_play_mode {
            return self.editable_play_mode_config();
        }
        self.play_mode
            .get(key_mode.play_map_key())
            .cloned()
            .unwrap_or_else(|| self.editable_play_mode_config())
    }

    /// Stores the editable mirror and replaces it with `key_mode` settings.
    pub fn activate_play_mode(&mut self, key_mode: KeyMode) {
        if key_mode == self.active_play_mode {
            return;
        }
        self.sync_active_play_mode();
        let next = self
            .play_mode
            .get(key_mode.play_map_key())
            .cloned()
            .unwrap_or_else(|| self.editable_play_mode_config());
        self.apply_editable_play_mode_config(&next);
        self.active_play_mode = key_mode;
    }

    /// Copies the editable legacy-compatible fields into the persistent map.
    pub fn sync_active_play_mode(&mut self) {
        self.play_mode.insert(
            self.active_play_mode.play_map_key().to_string(),
            self.editable_play_mode_config(),
        );
    }

    /// Migrates old profiles by copying their single set of values to every
    /// supported key mode, then restores the K7 editable mirror.
    pub fn normalize_play_mode_configs(&mut self) {
        let legacy = self.editable_play_mode_config();
        for key_mode in PLAY_MODE_CONFIG_MODES {
            self.play_mode
                .entry(key_mode.play_map_key().to_string())
                .or_insert_with(|| legacy.clone());
        }
        self.active_play_mode = KeyMode::K7;
        let mode7 = self.play_mode.get(KeyMode::K7.play_map_key()).cloned().unwrap_or(legacy);
        self.apply_editable_play_mode_config(&mode7);
    }

    fn editable_play_mode_config(&self) -> PlayModeConfig {
        PlayModeConfig {
            hispeed: self.lane.hispeed,
            hispeed_mode: self.lane.hispeed_mode,
            hs_fix: self.play.hs_fix,
            lane_effect: self.play.lane_effect,
            sudden: self.lane.sudden,
            lift: self.lane.lift,
            lift_enabled: self.lane.lift_enabled,
            hispeed_auto_adjust: self.lane.hispeed_auto_adjust,
            hidden: self.lane.hidden,
            target_green_number: self.lane.target_green_number,
            visual_offset_us: self.judge.visual_offset_us,
        }
    }

    fn apply_editable_play_mode_config(&mut self, config: &PlayModeConfig) {
        self.lane.hispeed = config.hispeed;
        self.lane.hispeed_mode = config.hispeed_mode;
        self.play.hs_fix = config.hs_fix;
        self.play.lane_effect = config.lane_effect;
        self.lane.sudden = config.sudden;
        self.lane.lift = config.lift;
        self.lane.lift_enabled = config.lift_enabled;
        self.lane.hispeed_auto_adjust = config.hispeed_auto_adjust;
        self.lane.hidden = config.hidden;
        self.lane.target_green_number = config.target_green_number;
        self.judge.visual_offset_us = config.visual_offset_us;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_play_settings_are_copied_to_every_key_mode() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed = 3.25;
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.play.hs_fix = HsFixConfig::MainBpm;
        profile.play.lane_effect = LaneEffectConfig::HiddenSudden;
        profile.lane.sudden = 420;
        profile.lane.hidden = 120;
        profile.lane.lift = 180;
        profile.lane.lift_enabled = true;
        profile.lane.hispeed_auto_adjust = true;
        profile.lane.target_green_number = 275;
        profile.judge.visual_offset_us = -7_000;

        profile.normalize_play_mode_configs();

        assert_eq!(profile.play_mode.len(), PLAY_MODE_CONFIG_MODES.len());
        let expected = profile.play_mode_config(KeyMode::K7);
        for key_mode in PLAY_MODE_CONFIG_MODES {
            assert_eq!(profile.play_mode_config(key_mode), expected, "{}", key_mode.as_str());
        }
    }

    #[test]
    fn switching_key_modes_preserves_independent_play_settings() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.normalize_play_mode_configs();

        profile.lane.hispeed = 2.75;
        profile.lane.target_green_number = 290;
        profile.lane.sudden = 350;
        profile.play.lane_effect = LaneEffectConfig::Sudden;
        profile.play.hs_fix = HsFixConfig::StartBpm;
        profile.judge.visual_offset_us = 4_000;

        profile.activate_play_mode(KeyMode::K14);
        profile.lane.hispeed = 4.25;
        profile.lane.target_green_number = 240;
        profile.lane.sudden = 610;
        profile.lane.hidden = 210;
        profile.lane.lift = 90;
        profile.lane.lift_enabled = true;
        profile.play.lane_effect = LaneEffectConfig::HiddenSudden;
        profile.play.hs_fix = HsFixConfig::MaxBpm;
        profile.judge.visual_offset_us = -11_000;

        profile.activate_play_mode(KeyMode::K7);
        assert_eq!(profile.lane.hispeed, 2.75);
        assert_eq!(profile.lane.target_green_number, 290);
        assert_eq!(profile.lane.sudden, 350);
        assert_eq!(profile.play.lane_effect, LaneEffectConfig::Sudden);
        assert_eq!(profile.play.hs_fix, HsFixConfig::StartBpm);
        assert_eq!(profile.judge.visual_offset_us, 4_000);

        profile.activate_play_mode(KeyMode::K14);
        assert_eq!(profile.lane.hispeed, 4.25);
        assert_eq!(profile.lane.target_green_number, 240);
        assert_eq!(profile.lane.sudden, 610);
        assert_eq!(profile.lane.hidden, 210);
        assert_eq!(profile.lane.lift, 90);
        assert!(profile.lane.lift_enabled);
        assert_eq!(profile.play.lane_effect, LaneEffectConfig::HiddenSudden);
        assert_eq!(profile.play.hs_fix, HsFixConfig::MaxBpm);
        assert_eq!(profile.judge.visual_offset_us, -11_000);
    }
}
