use super::play::{
    CONSTANT_FADE_MAX_MS, CONSTANT_FADE_MIN_MS, NOTE_DISPLAY_DURATION_MAX_MS,
    NOTE_DISPLAY_DURATION_MIN_MS, TARGET_GREEN_NUMBER_MAX, TARGET_GREEN_NUMBER_MIN, clamp_hispeed,
};
use super::profile_config::{
    AssistOptionConfig, BgaExpandConfig, BgaModeConfig, BottomShiftableGaugeConfig,
    DifficultyTableLevelDisplay, DoubleOptionConfig, GaugeAutoShiftConfig, GaugeTypeConfig,
    HISPEED_STEP_MAX, HISPEED_STEP_MIN, HispeedDirectionConfig, HispeedModeConfig, HsFixConfig,
    JudgeAlgorithmConfig, LaneConfig, LaneEffectConfig, ProfileConfig, RELEASE_BOUNCE_MS_MAX,
    RandomOptionConfig, ReplaySlotRule, SelectInputModeConfig, TargetOptionConfig,
    default_hispeed_step_fhs, default_hispeed_step_nhs, normalize_hispeed_step,
};
use bmz_core::lane::KeyMode;
use bmz_gameplay::rule::RuleMode;

use crate::ln_policy::LnPolicySetting;

/// ゲーム内設定で編集可能な profile.toml 項目。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsEntryId {
    NormalizeChartVolume,
    NormalizeSystemBgmVolume,
    MasterVolume,
    KeyVolume,
    BgmVolume,
    PreviewVolume,
    SystemBgmVolume,
    SystemSeVolume,
    InputOffsetMs,
    VisualOffsetMs,
    VisualOffsetAutoAdjust,
    JudgeAlgorithm,
    RuleMode,
    LnModePolicy,
    Gauge,
    GaugeAutoShift,
    BottomShiftableGauge,
    Random,
    Random2,
    DoubleOption,
    HsFix,
    Target,
    LaneEffect,
    Assist,
    BgaMode,
    BgaExpand,
    AutoPlay,
    MisslayerDurationMs,
    ShowLnTailCap,
    GuideSe,
    Hispeed,
    HispeedMode,
    HispeedStepNhs,
    HispeedStepFhs,
    Sudden,
    Lift,
    Hidden,
    TargetGreenNumber,
    NoteDisplayDurationMs,
    Constant,
    ConstantFadeMs,
    SelectInputMode,
    AnalogScratch1P,
    AnalogScratchSensitivity1P,
    AnalogScratchThreshold1P,
    AnalogScratch2P,
    AnalogScratchSensitivity2P,
    AnalogScratchThreshold2P,
    AnalogTicksPerScroll,
    KeyboardReleaseBounceMs,
    ControllerReleaseBounceMs,
    Hispeed8Key1,
    Hispeed8Key2,
    Hispeed8Key3,
    Hispeed8Key4,
    Hispeed8Key5,
    Hispeed8Key6,
    Hispeed8Key7,
    Hispeed8Key8,
    DifficultyTableLevelDisplay,
    SelectRandomSelect,
    RandomMixTargetLevel,
    RandomMixMaxLevel,
    RandomMixMinLevel,
    RandomMixBpmRange,
    RandomMixMaxBpm,
    RandomMixMinBpm,
    RandomMixStages,
    ReplayAutoSave,
    ReplaySlot1Rule,
    ReplaySlot2Rule,
    ReplaySlot3Rule,
    ReplaySlot4Rule,
}

impl SettingsEntryId {
    pub const VOLUME_ENTRIES: &'static [Self] = &[
        Self::NormalizeChartVolume,
        Self::NormalizeSystemBgmVolume,
        Self::MasterVolume,
        Self::KeyVolume,
        Self::BgmVolume,
        Self::PreviewVolume,
        Self::SystemBgmVolume,
        Self::SystemSeVolume,
    ];

    pub const JUDGE_ENTRIES: &'static [Self] = &[
        Self::InputOffsetMs,
        Self::VisualOffsetMs,
        Self::VisualOffsetAutoAdjust,
        Self::JudgeAlgorithm,
    ];

    // `Assist` は設定値を保持したまま、仕様確定までUIからのみ除外する。
    pub const PLAY_ENTRIES: &'static [Self] = &[
        Self::Gauge,
        Self::RuleMode,
        Self::LnModePolicy,
        Self::GaugeAutoShift,
        Self::BottomShiftableGauge,
        Self::Random,
        Self::Random2,
        Self::DoubleOption,
        Self::HsFix,
        Self::Target,
        Self::LaneEffect,
        Self::BgaMode,
        Self::BgaExpand,
        Self::AutoPlay,
        Self::MisslayerDurationMs,
        Self::ShowLnTailCap,
        Self::GuideSe,
    ];

    pub const DISPLAY_ENTRIES: &'static [Self] = &[
        Self::Hispeed,
        Self::HispeedMode,
        Self::HispeedStepNhs,
        Self::HispeedStepFhs,
        Self::Sudden,
        Self::Lift,
        Self::Hidden,
        Self::TargetGreenNumber,
        Self::NoteDisplayDurationMs,
        Self::Constant,
        Self::ConstantFadeMs,
    ];

    pub const INPUT_ENTRIES: &'static [Self] = &[
        Self::SelectInputMode,
        Self::AnalogScratch1P,
        Self::AnalogScratchSensitivity1P,
        Self::AnalogScratchThreshold1P,
        Self::AnalogScratch2P,
        Self::AnalogScratchSensitivity2P,
        Self::AnalogScratchThreshold2P,
        Self::AnalogTicksPerScroll,
        Self::KeyboardReleaseBounceMs,
        Self::ControllerReleaseBounceMs,
    ];

    pub const SELECT_ENTRIES: &'static [Self] = &[
        Self::DifficultyTableLevelDisplay,
        Self::SelectRandomSelect,
        Self::RandomMixTargetLevel,
        Self::RandomMixMaxLevel,
        Self::RandomMixMinLevel,
        Self::RandomMixBpmRange,
        Self::RandomMixMaxBpm,
        Self::RandomMixMinBpm,
        Self::RandomMixStages,
    ];

    pub const HISPEED_8K_ENTRIES: &'static [Self] = &[
        Self::Hispeed8Key1,
        Self::Hispeed8Key2,
        Self::Hispeed8Key3,
        Self::Hispeed8Key4,
        Self::Hispeed8Key5,
        Self::Hispeed8Key6,
        Self::Hispeed8Key7,
        Self::Hispeed8Key8,
    ];

    pub const REPLAY_ENTRIES: &'static [Self] = &[
        Self::ReplayAutoSave,
        Self::ReplaySlot1Rule,
        Self::ReplaySlot2Rule,
        Self::ReplaySlot3Rule,
        Self::ReplaySlot4Rule,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::NormalizeChartVolume => "NORMALIZE",
            Self::NormalizeSystemBgmVolume => "SYS BGM NORM",
            Self::MasterVolume => "MASTER",
            Self::KeyVolume => "KEY",
            Self::BgmVolume => "BGM",
            Self::PreviewVolume => "PREVIEW",
            Self::SystemBgmVolume => "SYS BGM",
            Self::SystemSeVolume => "SYS SE",
            Self::InputOffsetMs => "INPUT OFFSET",
            Self::VisualOffsetMs => "VISUAL OFFSET",
            Self::VisualOffsetAutoAdjust => "AUTO ADJUST",
            Self::JudgeAlgorithm => "JUDGE ALGO",
            Self::RuleMode => "RULE MODE",
            Self::LnModePolicy => "LN MODE",
            Self::Gauge => "GAUGE",
            Self::GaugeAutoShift => "GAUGE SHIFT",
            Self::BottomShiftableGauge => "GAS BOTTOM",
            Self::Random => "RANDOM",
            Self::Random2 => "RANDOM 2P",
            Self::DoubleOption => "DP OPTION",
            Self::HsFix => "HS-FIX",
            Self::Target => "TARGET",
            Self::LaneEffect => "LANE FX",
            Self::Assist => "ASSIST",
            Self::BgaMode => "BGA",
            Self::BgaExpand => "BGA FIT",
            Self::AutoPlay => "AUTO PLAY",
            Self::MisslayerDurationMs => "MISSLAYER",
            Self::ShowLnTailCap => "LN TAIL CAP",
            Self::GuideSe => "GUIDE SE",
            Self::Hispeed => "HISPEED",
            Self::HispeedMode => "HS MODE",
            Self::HispeedStepNhs => "HS STEP NHS",
            Self::HispeedStepFhs => "HS STEP FHS",
            Self::Sudden => "SUDDEN+",
            Self::Lift => "LIFT",
            Self::Hidden => "HIDDEN",
            Self::TargetGreenNumber => "GREEN NO.",
            Self::NoteDisplayDurationMs => "DURATION",
            Self::Constant => "CONSTANT",
            Self::ConstantFadeMs => "CONSTANT FADE",
            Self::SelectInputMode => "SELECT INPUT",
            Self::AnalogScratch1P => "1P ANALOG SCRATCH",
            Self::AnalogScratchSensitivity1P => "1P ANALOG SENS",
            Self::AnalogScratchThreshold1P => "1P ANALOG STOP",
            Self::AnalogScratch2P => "2P ANALOG SCRATCH",
            Self::AnalogScratchSensitivity2P => "2P ANALOG SENS",
            Self::AnalogScratchThreshold2P => "2P ANALOG STOP",
            Self::AnalogTicksPerScroll => "ANALOG SCROLL",
            Self::KeyboardReleaseBounceMs => "KEYBOARD BOUNCE",
            Self::ControllerReleaseBounceMs => "CONTROLLER BOUNCE",
            Self::Hispeed8Key1 => "KEY 1 HS DIRECTION",
            Self::Hispeed8Key2 => "KEY 2 HS DIRECTION",
            Self::Hispeed8Key3 => "KEY 3 HS DIRECTION",
            Self::Hispeed8Key4 => "KEY 4 HS DIRECTION",
            Self::Hispeed8Key5 => "KEY 5 HS DIRECTION",
            Self::Hispeed8Key6 => "KEY 6 HS DIRECTION",
            Self::Hispeed8Key7 => "KEY 7 HS DIRECTION",
            Self::Hispeed8Key8 => "KEY 8 HS DIRECTION",
            Self::DifficultyTableLevelDisplay => "TABLE LEVEL DISPLAY",
            Self::SelectRandomSelect => "RANDOM SELECT",
            Self::RandomMixTargetLevel => "MIX TARGET LEVEL",
            Self::RandomMixMaxLevel => "MIX MAX LEVEL",
            Self::RandomMixMinLevel => "MIX MIN LEVEL",
            Self::RandomMixBpmRange => "MIX BPM RANGE",
            Self::RandomMixMaxBpm => "MIX MAX BPM",
            Self::RandomMixMinBpm => "MIX MIN BPM",
            Self::RandomMixStages => "MIX STAGES",
            Self::ReplayAutoSave => "REPLAY SAVE",
            Self::ReplaySlot1Rule => "REPLAY 1",
            Self::ReplaySlot2Rule => "REPLAY 2",
            Self::ReplaySlot3Rule => "REPLAY 3",
            Self::ReplaySlot4Rule => "REPLAY 4",
        }
    }
}

/// 設定値 1 ステップの増減量。
mod adjust;
mod support;
mod value;

pub use adjust::{adjust_settings_value, eight_key_hispeed_lane};
pub use value::{format_settings_value, settings_adjust_step};

use support::*;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::profile_config::ProfileConfig;

    #[test]
    fn adjust_volume_clamps_to_range() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert!(profile.audio_mix.normalize_chart_volume);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::NormalizeChartVolume, 1));
        assert!(!profile.audio_mix.normalize_chart_volume);
        assert_eq!(format_settings_value(&profile, SettingsEntryId::NormalizeChartVolume), "OFF");
        assert!(profile.audio_mix.normalize_system_bgm_volume);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::NormalizeSystemBgmVolume, 1,));
        assert!(!profile.audio_mix.normalize_system_bgm_volume);
        assert_eq!(
            format_settings_value(&profile, SettingsEntryId::NormalizeSystemBgmVolume),
            "OFF"
        );
        profile.audio_mix.master_volume = 98;
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::MasterVolume, 5));
        assert_eq!(profile.audio_mix.master_volume, 100);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::MasterVolume, -200));
        assert_eq!(profile.audio_mix.master_volume, 0);
    }

    #[test]
    fn adjust_judge_offset_in_millisecond_steps() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::InputOffsetMs, 3));
        assert_eq!(profile.judge.input_offset_us, 3_000);
    }

    #[test]
    fn visual_offset_auto_adjust_toggles() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert!(SettingsEntryId::JUDGE_ENTRIES.contains(&SettingsEntryId::VisualOffsetAutoAdjust));
        assert_eq!(format_settings_value(&profile, SettingsEntryId::VisualOffsetAutoAdjust), "OFF");
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::VisualOffsetAutoAdjust, 1));
        assert!(profile.judge.visual_offset_auto_adjust);
        assert_eq!(format_settings_value(&profile, SettingsEntryId::VisualOffsetAutoAdjust), "ON");
    }

    #[test]
    fn cycle_judge_algorithm_uses_beatoraja_order() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);

        assert_eq!(format_settings_value(&profile, SettingsEntryId::JudgeAlgorithm), "COMBO");
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::JudgeAlgorithm, 1));
        assert_eq!(profile.judge.judge_algorithm, JudgeAlgorithmConfig::Duration);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::JudgeAlgorithm, 1));
        assert_eq!(profile.judge.judge_algorithm, JudgeAlgorithmConfig::Lowest);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::JudgeAlgorithm, 1));
        assert_eq!(profile.judge.judge_algorithm, JudgeAlgorithmConfig::Combo);
        assert_eq!(format_settings_value(&profile, SettingsEntryId::JudgeAlgorithm), "COMBO");
    }

    #[test]
    fn adjust_hispeed_uses_mode_specific_steps() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::Hispeed, 1));
        assert!((profile.lane.hispeed - 2.25).abs() < f32::EPSILON);

        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::Hispeed, 1));
        assert!((profile.lane.hispeed - 2.75).abs() < f32::EPSILON);
    }

    #[test]
    fn adjust_hispeed_step_settings_increments_by_five_hundredths() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert_eq!(format_settings_value(&profile, SettingsEntryId::HispeedStepNhs), "0.25");
        assert_eq!(format_settings_value(&profile, SettingsEntryId::HispeedStepFhs), "0.50");

        assert!(adjust_settings_value(&mut profile, SettingsEntryId::HispeedStepNhs, 1));
        assert!((profile.lane.hispeed_step_nhs - 0.30).abs() < f32::EPSILON);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::HispeedStepFhs, -1));
        assert!((profile.lane.hispeed_step_fhs - 0.45).abs() < f32::EPSILON);
    }

    #[test]
    fn adjust_lane_cover_and_lift_keep_combined_range() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        profile.lane.sudden = 900;
        profile.lane.lift = 200;

        assert!(adjust_settings_value(&mut profile, SettingsEntryId::Sudden, 1));
        assert_eq!(profile.lane.sudden, 800);

        profile.lane.sudden = 300;
        profile.lane.lift = 700;
        assert!(!adjust_settings_value(&mut profile, SettingsEntryId::Lift, 1));
        assert_eq!(profile.lane.lift, 700);
    }

    #[test]
    fn cycle_gauge_wraps() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        profile.play.gauge = GaugeTypeConfig::Hazard;
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::Gauge, 1));
        assert_eq!(profile.play.gauge, GaugeTypeConfig::AutoShift);
    }

    #[test]
    fn cycle_rule_mode_and_format_value() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert_eq!(format_settings_value(&profile, SettingsEntryId::RuleMode), "BEATORAJA");

        assert!(adjust_settings_value(&mut profile, SettingsEntryId::RuleMode, 1));
        assert_eq!(profile.play.rule_mode, RuleMode::Lr2Oraja);
        assert_eq!(format_settings_value(&profile, SettingsEntryId::RuleMode), "LR2ORAJA");

        assert!(adjust_settings_value(&mut profile, SettingsEntryId::RuleMode, 1));
        assert_eq!(profile.play.rule_mode, RuleMode::Dx);
        assert_eq!(format_settings_value(&profile, SettingsEntryId::RuleMode), "DX");
    }

    #[test]
    fn cycle_hs_fix_uses_beatoraja_order() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert_eq!(format_settings_value(&profile, SettingsEntryId::HsFix), "OFF");

        assert!(adjust_settings_value(&mut profile, SettingsEntryId::HsFix, 1));
        assert_eq!(profile.play.hs_fix, HsFixConfig::StartBpm);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::HsFix, 1));
        assert_eq!(profile.play.hs_fix, HsFixConfig::MaxBpm);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::HsFix, 1));
        assert_eq!(profile.play.hs_fix, HsFixConfig::MainBpm);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::HsFix, 1));
        assert_eq!(profile.play.hs_fix, HsFixConfig::MinBpm);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::HsFix, 1));
        assert_eq!(profile.play.hs_fix, HsFixConfig::Off);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::HsFix, -1));
        assert_eq!(profile.play.hs_fix, HsFixConfig::MinBpm);
    }

    #[test]
    fn auto_play_toggles() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert!(!profile.play.auto_play);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::AutoPlay, 1));
        assert!(profile.play.auto_play);
    }

    #[test]
    fn cycle_ln_mode_policy_and_hispeed_mode() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert_eq!(format_settings_value(&profile, SettingsEntryId::LnModePolicy), "AUTO(LN)");
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::LnModePolicy, 1));
        assert_eq!(profile.play.ln_mode_policy, crate::ln_policy::LnPolicySetting::AutoCn);

        assert_eq!(format_settings_value(&profile, SettingsEntryId::HispeedMode), "NORMAL");
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::HispeedMode, 1));
        assert_eq!(
            profile.lane.hispeed_mode,
            crate::config::profile_config::HispeedModeConfig::Floating
        );
    }

    #[test]
    fn adjust_green_number_misslayer_and_analog_settings() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        profile.lane.target_green_number = 5_995;
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::TargetGreenNumber, 10));
        assert_eq!(profile.lane.target_green_number, 6_000);
        assert_eq!(profile.lane.note_display_duration_ms, 10_000);

        assert!(adjust_settings_value(&mut profile, SettingsEntryId::NoteDisplayDurationMs, -10,));
        assert_eq!(profile.lane.note_display_duration_ms, 9_990);
        assert_eq!(profile.lane.target_green_number, 5_994);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::Constant, 1));
        assert!(profile.lane.constant_enabled);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::GuideSe, 1));
        assert!(profile.play.guide_se);

        profile.play.misslayer_duration_ms = 4_980;
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::MisslayerDurationMs, 50));
        assert_eq!(profile.play.misslayer_duration_ms, 5_000);

        assert!(adjust_settings_value(
            &mut profile,
            SettingsEntryId::AnalogScratchSensitivity1P,
            1,
        ));
        assert!((profile.input.gamepad1.analog_scratch_sensitivity - 1.1).abs() < f32::EPSILON);

        profile.input.gamepad1.analog_scratch_threshold = 995;
        assert!(
            adjust_settings_value(&mut profile, SettingsEntryId::AnalogScratchThreshold1P, 10,)
        );
        assert_eq!(profile.input.gamepad1.analog_scratch_threshold, 1_000);
        assert_eq!(
            format_settings_value(&profile, SettingsEntryId::AnalogScratchThreshold1P),
            "1000 ticks"
        );

        assert!(adjust_settings_value(&mut profile, SettingsEntryId::AnalogScratch2P, 1));
        assert!(!profile.input.gamepad2.analog_scratch);

        assert!(adjust_settings_value(&mut profile, SettingsEntryId::KeyboardReleaseBounceMs, 5,));
        assert_eq!(profile.input.keyboard_release_bounce_ms, 5);
        assert_eq!(
            format_settings_value(&profile, SettingsEntryId::KeyboardReleaseBounceMs),
            "5 ms"
        );

        profile.input.controller_release_bounce_ms = RELEASE_BOUNCE_MS_MAX;
        assert!(!adjust_settings_value(
            &mut profile,
            SettingsEntryId::ControllerReleaseBounceMs,
            1,
        ));
        assert_eq!(profile.input.controller_release_bounce_ms, RELEASE_BOUNCE_MS_MAX);
    }

    #[test]
    fn cycle_input_and_replay_settings() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert_eq!(format_settings_value(&profile, SettingsEntryId::SelectInputMode), "7K/14K");
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::SelectInputMode, 1));
        assert_eq!(
            profile.input.select_input_mode,
            crate::config::profile_config::SelectInputModeConfig::Key9
        );

        assert!(profile.replay.auto_save);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::ReplayAutoSave, 1));
        assert!(!profile.replay.auto_save);

        assert_eq!(
            format_settings_value(&profile, SettingsEntryId::ReplaySlot2Rule),
            "SCORE UPDATE"
        );
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::ReplaySlot2Rule, 1));
        assert_eq!(
            profile.replay.slot_rules[1],
            crate::config::profile_config::ReplaySlotRule::BpUpdate
        );
    }

    #[test]
    fn random_mix_settings_use_lr2_ranges_and_labels() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert_eq!(format_settings_value(&profile, SettingsEntryId::RandomMixTargetLevel), "OFF");
        assert_eq!(
            format_settings_value(&profile, SettingsEntryId::RandomMixBpmRange),
            "+- 10 BPM"
        );
        assert_eq!(format_settings_value(&profile, SettingsEntryId::RandomMixStages), "5 STAGE");

        assert_eq!(settings_adjust_step(SettingsEntryId::RandomMixMaxBpm), 10);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::RandomMixTargetLevel, 120,));
        assert_eq!(profile.select.random_mix.target_level, 99);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::RandomMixMaxBpm, 1_000,));
        assert_eq!(profile.select.random_mix.max_bpm, 990);
        assert!(adjust_settings_value(&mut profile, SettingsEntryId::RandomMixStages, -5,));
        assert_eq!(format_settings_value(&profile, SettingsEntryId::RandomMixStages), "RANDOM");
    }

    #[test]
    fn difficulty_table_level_display_cycles_between_table_and_chart() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert_eq!(
            format_settings_value(&profile, SettingsEntryId::DifficultyTableLevelDisplay),
            "TABLE LEVEL"
        );

        assert!(adjust_settings_value(
            &mut profile,
            SettingsEntryId::DifficultyTableLevelDisplay,
            1,
        ));
        assert_eq!(
            profile.select.difficulty_table_level_display,
            DifficultyTableLevelDisplay::Chart
        );
        assert_eq!(
            format_settings_value(&profile, SettingsEntryId::DifficultyTableLevelDisplay),
            "CHART LEVEL"
        );
    }

    #[test]
    fn eight_key_hispeed_direction_rows_toggle_independently() {
        let mut profile = ProfileConfig::new_default("default", "Default", 0);
        assert_eq!(format_settings_value(&profile, SettingsEntryId::Hispeed8Key1), "UP");
        assert_eq!(format_settings_value(&profile, SettingsEntryId::Hispeed8Key2), "DOWN");

        assert!(adjust_settings_value(&mut profile, SettingsEntryId::Hispeed8Key1, 1));
        assert_eq!(format_settings_value(&profile, SettingsEntryId::Hispeed8Key1), "DOWN");
        assert_eq!(format_settings_value(&profile, SettingsEntryId::Hispeed8Key2), "DOWN");
        assert_eq!(
            profile.input.play[KeyMode::K8.play_map_key()].hispeed.get(&LaneConfig::Key1),
            Some(&HispeedDirectionConfig::Down),
        );

        assert!(adjust_settings_value(&mut profile, SettingsEntryId::Hispeed8Key1, -1));
        assert!(profile.input.play[KeyMode::K8.play_map_key()].hispeed.is_empty());
    }
}
