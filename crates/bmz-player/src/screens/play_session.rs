use anyhow::{Context, Result, bail};
use bmz_audio::clock::AudioClock;
use bmz_audio::engine::AudioEngine;
use bmz_audio::ffmpeg_loader::FfmpegSampleLoader;
use bmz_audio::loader::{
    LoadedSampleReport, SampleLoader, load_chart_samples, load_chart_samples_with_progress,
};
use bmz_audio::loudness::{analyze_chart_loudness, play_normalization_gain_for_loudness};
use bmz_chart::import::{
    BmsRandomSource, ImportResult, import_bms_chart, import_bms_chart_with_random_source,
};
use bmz_chart::model::{BgaAssetRef, NoteEvent, NoteKind, PlayableChart, TimingEventKind};
use bmz_chart::start_margin::apply_start_note_margin;
use bmz_core::clear::GaugeType;
use bmz_core::ids::NoteId;
use bmz_core::lane::{KeyMode, LANE_COUNT, Lane};
use bmz_core::time::TimeUs;
use bmz_gameplay::autoplay::AutoplayController;
use bmz_gameplay::gauge::{
    GaugeAutoShiftMode, GaugeCarryValue, GaugeProperty, GaugeState,
    gauge_total_for_chart_and_rule_mode,
};
use bmz_gameplay::hit_error::HitErrorRing;
use bmz_gameplay::input::backend::{InputBackend, NullInputBackend};
use bmz_gameplay::input::bounce::InputBounceFilter;
use bmz_gameplay::input::system::InputSystem;
use bmz_gameplay::input::translator::DefaultInputTranslator;
use bmz_gameplay::judge::engine::JudgeEngine;
use bmz_gameplay::judge::model::{JudgeAlgorithm, JudgeWindow, JudgeWindows};
use bmz_gameplay::judge::window::{
    judge_percent_at_time_for_keymode, judge_windows_for_keymode_and_rule_mode,
    judge_windows_for_rule_mode_and_keymode,
};
use bmz_gameplay::replay::{ReplayPlayer, ReplayRecorder};
use bmz_gameplay::rule::RuleMode;
use bmz_gameplay::score::{ScoreState, scored_note_count};
use bmz_gameplay::session::{
    BgmScheduler, GameSession, HispeedMode, InputOffsetAutoAdjustState, PlaySkinOffset, PlayState,
};
use std::sync::Arc;

use crate::config::play::{
    audio_mix_from_profile, bottom_shiftable_gauge_from_config, gauge_auto_shift_from_config,
    gauge_type_from_config, input_bounce_config_from_profile, lane_binding_for_chart_with_slots,
    lane_unit_to_f32, play_offsets_from_profile,
};
use crate::config::profile_config::{
    BgaExpandConfig, BgaModeConfig, JudgeAlgorithmConfig, LaneEffectConfig, ProfileConfig,
};
use crate::input::gamepad::GamepadSlotMap;
use crate::ln_policy::{
    LnPolicySetting, apply_ln_policy_to_chart, force_ln_mode_for_chart, score_ln_policy_for_chart,
};
use crate::random_option_seed::{JavaRandom, RandomOptionSeed, RandomOptionSeeds};
use crate::screens::practice::{
    PracticeProperty, apply_practice_property, apply_practice_start_gauge,
};
use crate::select_options::{ArrangeOption, DoubleOption, HsFixOption, SessionMode, TargetOption};
use crate::skin_loader::play_skin_selection_for_session;
use crate::storage::library_db::ChartNormalizationAnalysis;
use crate::storage::library_db::LibraryDatabase;
use crate::storage::score_db::ScoreKey;

#[derive(Debug, Clone)]
pub struct PlaySessionOptions {
    pub session_mode: SessionMode,
    pub autoplay: bool,
    /// Practice section play: no score / replay persistence (like autoplay).
    pub practice_mode: bool,
    pub replay_player: Option<ReplayPlayer>,
    pub sample_rate: u32,
    pub gauge_override: Option<GaugeType>,
    pub gauge_auto_shift: GaugeAutoShiftMode,
    pub bottom_shiftable_gauge: GaugeType,
    pub arrange: ArrangeOption,
    pub arrange_2p: ArrangeOption,
    pub double_option: DoubleOption,
    pub hs_fix: HsFixOption,
    pub target: TargetOption,
    /// beatoraja-compatible 24-bit RANDOM option seed for the 1P side.
    pub arrange_seed: Option<i64>,
    /// beatoraja-compatible 24-bit RANDOM option seed for the 2P side.
    pub arrange_seed_2p: Option<i64>,
    /// Fresh play 用 Random Trainer seed。7K の通常 RANDOM だけで 1P seed より優先する。
    pub random_trainer_seed: Option<i64>,
    /// Replay v3 and older used one unrestricted i64 seed with SplitMix64.
    pub legacy_arrange_seed: bool,
    /// Independent seed used only while selecting BMS `#RANDOM` branches.
    pub bms_random_seed: Option<u64>,
    /// Recorded `#RANDOM` decisions, in source order, for exact replay.
    pub bms_random_choices: Option<Vec<i32>>,
    pub arrange_pattern: Option<Vec<u8>>,
    /// When set, overrides the gauge's starting value.  Used to carry the
    /// gauge between charts during a course.
    pub initial_gauge_value: Option<f32>,
    /// Per-gauge starting values for course carry.  This preserves auto-shift
    /// gauges independently, so depleted higher gauges stay depleted.
    pub initial_gauge_values: Option<Vec<GaugeCarryValue>>,
    /// Course-mode combo carried from the previous chart. Score storage still
    /// starts from zero; this affects rendered combo/max combo only.
    pub initial_course_combo: Option<u32>,
    /// Course judge constraint forwarded from CourseJudgeConstraint.
    /// `NoGood` zeroes the good window, `NoGreat` zeroes great and good
    /// windows; the next judge band kicks in immediately.
    pub judge_constraint: bmz_core::course::CourseJudgeConstraint,
    /// Course-forced long-note mode (Ln/Cn/Hcn).  `None` keeps the chart's
    /// declared mode.
    pub ln_mode_override: Option<bmz_chart::model::LongNoteMode>,
    pub ln_policy_setting: LnPolicySetting,
    pub rule_mode: RuleMode,
    /// 段位ゲージ用の `GaugeProperty` 上書き。コース時に
    /// `apply_course_constraints` が `CourseGaugeConstraint::Lr2/Keys5/...` を
    /// 解釈して設定する。`None` の場合はチャートの `KeyMode` から自動推定する。
    pub gauge_property: Option<GaugeProperty>,
    /// 論理 `gamepad1`/`gamepad2` → 物理 gilrs id の対応。プレイ開始時に固定する。
    pub gamepad_slots: GamepadSlotMap,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppliedArrange {
    pub arrange: ArrangeOption,
    pub arrange_2p: ArrangeOption,
    pub double_option: DoubleOption,
    /// 1P option seed. New plays always use the beatoraja 24-bit range.
    pub seed: Option<i64>,
    /// Independent 2P option seed for DP charts.
    pub seed_2p: Option<i64>,
    /// True only when replaying the pre-v4 SplitMix64 seed format.
    pub legacy_seed: bool,
    /// BMS `#RANDOM` decisions applied before the arrange modifier.
    pub bms_random_choices: Vec<i32>,
    pub pattern: Option<Vec<u8>>,
}

impl AppliedArrange {
    pub fn packed_beatoraja_seed_from_sides(&self) -> Option<i64> {
        if self.legacy_seed {
            return None;
        }
        let p1 = RandomOptionSeed::new(u32::try_from(self.seed?).ok()?)?;
        let seeds = if let Some(seed_2p) = self.seed_2p {
            let p2 = RandomOptionSeed::new(u32::try_from(seed_2p).ok()?)?;
            RandomOptionSeeds::double(p1, p2)
        } else {
            RandomOptionSeeds::single(p1)
        };
        i64::try_from(seeds.pack()).ok()
    }

    pub fn packed_beatoraja_seed(&self, key_mode: KeyMode) -> Option<i64> {
        if self.legacy_seed {
            return None;
        }
        let packed = self.packed_beatoraja_seed_from_sides()?;
        let has_p2 = self.seed_2p.is_some();
        (has_p2 == matches!(key_mode, KeyMode::K10 | KeyMode::K14)).then_some(packed)
    }
}

pub struct PreparedPlaySession {
    pub session: GameSession,
    pub audio: AudioEngine,
    pub sample_report: Vec<LoadedSampleReport>,
    pub applied_arrange: AppliedArrange,
    pub score_key: ScoreKey,
    pub target_option: TargetOption,
    pub target: String,
    pub practice_mode: bool,
}

pub struct PreloadedPlaySession {
    pub chart: Arc<PlayableChart>,
    pub audio: AudioEngine,
    pub sample_report: Vec<LoadedSampleReport>,
    pub chart_normalization_gain: f32,
    pub applied_arrange: AppliedArrange,
    pub score_key: ScoreKey,
}

impl Default for PlaySessionOptions {
    fn default() -> Self {
        Self {
            session_mode: SessionMode::Normal,
            autoplay: false,
            practice_mode: false,
            replay_player: None,
            sample_rate: 48_000,
            gauge_override: None,
            gauge_auto_shift: GaugeAutoShiftMode::Off,
            bottom_shiftable_gauge: GaugeType::AssistEasy,
            arrange: ArrangeOption::Normal,
            arrange_2p: ArrangeOption::Normal,
            double_option: DoubleOption::Off,
            hs_fix: HsFixOption::Off,
            target: TargetOption::None,
            arrange_seed: None,
            arrange_seed_2p: None,
            random_trainer_seed: None,
            legacy_arrange_seed: false,
            bms_random_seed: None,
            bms_random_choices: None,
            arrange_pattern: None,
            initial_gauge_value: None,
            initial_gauge_values: None,
            initial_course_combo: None,
            judge_constraint: bmz_core::course::CourseJudgeConstraint::Normal,
            ln_mode_override: None,
            ln_policy_setting: LnPolicySetting::AutoLn,
            rule_mode: RuleMode::Beatoraja,
            gauge_property: None,
            gamepad_slots: GamepadSlotMap::default(),
        }
    }
}

mod arrange;
mod build;
mod preload;

pub use arrange::{apply_arrange, apply_arrange_pair, generate_arrange_seed};
pub use build::{
    apply_placeholder_session_visuals, build_game_session, build_game_session_with_input_backend,
};
pub use preload::{
    build_audio_engine_for_chart, build_practice_prepared_from_preloaded,
    build_prepared_play_session_from_preloaded, load_chart_bga_assets_for_chart,
    load_game_session_for_chart, load_game_session_for_chart_with_input_backend,
    load_prepared_play_session_for_chart, load_prepared_play_session_for_chart_with_input_backend,
    load_source_chart_for_chart, preload_play_session_for_chart,
    preload_play_session_for_chart_with_callbacks, preload_play_session_for_chart_with_progress,
    preload_play_session_reloading_audio_with_progress, scored_note_count_for_chart,
};

use arrange::*;
#[cfg(test)]
use build::*;
#[cfg(test)]
use preload::*;

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::sync::Arc;

    use bmz_audio::loader::LoadedSampleStatus;
    use bmz_chart::hash::compute_chart_identity;
    use bmz_chart::model::{ChartMetadata, PlayableChart, SoundAssetRef};
    use bmz_core::clear::GaugeType;
    use bmz_core::ids::{NoteId, SoundId};
    use bmz_core::input::InputKind;
    use bmz_core::lane::{KeyMode, Lane};
    use bmz_core::time::TimeUs;
    use bmz_gameplay::input::backend::{
        BufferedInputBackend, DeviceId, DeviceInputEvent, DeviceTimestamp, PhysicalControl,
    };
    use bmz_gameplay::input::translator::InputTimingContext;
    use bmz_gameplay::rule::RuleMode;
    use rusqlite::Connection;

    use super::*;
    use crate::config::profile_config::HispeedModeConfig;
    use crate::storage::common::configure_connection;
    use crate::storage::library_db::{ChartImportRecord, LibraryDatabase};
    use crate::storage::migration::{LIBRARY_MIGRATIONS, run_migrations};

    #[test]
    fn build_game_session_uses_profile_play_settings() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.play.auto_play = true;
        profile.judge.input_offset_us = 123;
        let chart = Arc::new(chart());

        let session = build_game_session(chart, &profile, PlaySessionOptions::default());

        assert_eq!(session.state, PlayState::Ready);
        assert_eq!(session.gauge.selected, GaugeType::Normal);
        assert!(session.autoplay.is_some());
        assert_eq!(session.offsets.input_offset_us, 123);
        assert!((session.audio_mix.master_volume - 0.5).abs() < 1e-6);
        assert_eq!(session.audio_clock.sample_rate, 48_000);
        assert_eq!(session.hispeed, 2.0);
        assert_eq!(session.hidden_cover, 0.0);
        assert!(session.bga_enabled);
        assert_eq!(session.poor_bga_duration_us, 500_000);
        assert_eq!(session.bga_stretch, 1);
    }

    #[test]
    fn build_game_session_uses_visual_offset_auto_adjust_from_profile() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.judge.visual_offset_auto_adjust = true;
        let session =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert!(session.input_offset_auto_adjust_enabled);
        assert!(session.input_offset_auto_adjust.is_some());
    }

    #[test]
    fn ghost_battle_keeps_primary_input_offset_auto_adjust_enabled() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.judge.visual_offset_auto_adjust = true;
        let mut battle_chart = chart();
        battle_chart.metadata.key_mode = KeyMode::K14;
        let session = build_game_session(
            Arc::new(battle_chart),
            &profile,
            PlaySessionOptions {
                session_mode: SessionMode::GhostBattle,
                replay_player: Some(ReplayPlayer::default()),
                ..PlaySessionOptions::default()
            },
        );

        assert!(session.input_offset_auto_adjust.is_some());
        assert!(session.replay_lane_mask.is_some());
        assert_eq!(session.primary_key_mode, KeyMode::K7);
    }

    #[test]
    fn build_game_session_uses_release_bounce_settings_from_profile() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.input.keyboard_release_bounce_ms = 3;
        profile.input.controller_release_bounce_ms = 8;

        let session =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert_eq!(
            session.input_system.bounce_filter.config(),
            bmz_gameplay::input::bounce::InputBounceConfig {
                keyboard_threshold_us: 3_000,
                controller_threshold_us: 8_000,
            }
        );
    }

    #[test]
    fn build_game_session_applies_judge_algorithm_from_profile() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.judge.judge_algorithm = JudgeAlgorithmConfig::Duration;

        let duration =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
        assert_eq!(duration.judge.algorithm, JudgeAlgorithm::Duration);
    }

    #[test]
    fn placeholder_session_visuals_use_visual_offset_for_skin_judge_timing() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.judge.input_offset_us = 3_000;
        profile.judge.visual_offset_us = 4_000;
        profile.judge.visual_offset_auto_adjust = true;
        let options = PlaySessionOptions::default();
        let mut snapshot = bmz_render::snapshot::RenderSnapshot::default();

        apply_placeholder_session_visuals(&mut snapshot, &profile, KeyMode::K7, &options);

        assert_eq!(snapshot.judge_timing_offset_ms, 4);
        assert!(snapshot.judge_timing_auto_adjust);
    }

    #[test]
    fn placeholder_session_visuals_preserve_preloaded_meta_images() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let options = PlaySessionOptions::default();
        let stagefile_size = bmz_render::skin::SkinImageSize { width: 320.0, height: 240.0 };
        let mut snapshot = bmz_render::snapshot::RenderSnapshot {
            stagefile_background: true,
            stagefile_image_size: Some(stagefile_size),
            backbmp_background: true,
            ..Default::default()
        };

        apply_placeholder_session_visuals(&mut snapshot, &profile, KeyMode::K7, &options);

        assert!(snapshot.stagefile_background);
        assert_eq!(snapshot.stagefile_image_size, Some(stagefile_size));
        assert!(snapshot.backbmp_background);
    }

    #[test]
    fn placeholder_session_visuals_initialize_floating_hispeed_for_ready_display() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.lane.target_green_number = 300;
        // Stale value from a different BPM should not leak into READY display.
        profile.lane.hispeed = 4.0;
        let options =
            PlaySessionOptions { hs_fix: HsFixOption::StartBpm, ..PlaySessionOptions::default() };
        let mut snapshot =
            bmz_render::snapshot::RenderSnapshot { now_bpm: 240.0, ..Default::default() };

        apply_placeholder_session_visuals(&mut snapshot, &profile, KeyMode::K7, &options);

        assert!((snapshot.hispeed - 2.0).abs() < f32::EPSILON);
        assert_eq!(snapshot.hispeed_mode_index, 1);
        assert_eq!(snapshot.note_display_duration_ms, 500);
    }

    #[test]
    fn placeholder_session_visuals_use_hsfix_to_select_hispeed_mode() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.lane.hispeed = 4.0;
        profile.lane.target_green_number = 300;
        let options = PlaySessionOptions::default();
        let mut snapshot =
            bmz_render::snapshot::RenderSnapshot { now_bpm: 240.0, ..Default::default() };

        apply_placeholder_session_visuals(&mut snapshot, &profile, KeyMode::K7, &options);

        assert_eq!(snapshot.hispeed, 4.0);
        assert_eq!(snapshot.hispeed_mode_index, 0);
    }

    #[test]
    fn placeholder_session_visuals_match_session_bga_modes() {
        for (mode, profile_autoplay, option_autoplay, replay, expected) in [
            (BgaModeConfig::On, false, false, false, true),
            (BgaModeConfig::Auto, false, false, false, false),
            (BgaModeConfig::Auto, false, true, false, true),
            (BgaModeConfig::Auto, false, false, true, true),
            (BgaModeConfig::Auto, true, false, false, true),
            (BgaModeConfig::Off, true, true, false, false),
        ] {
            let mut profile = ProfileConfig::new_default("default", "Default", 1);
            profile.play.bga = mode;
            profile.play.auto_play = profile_autoplay;
            let options = PlaySessionOptions {
                autoplay: option_autoplay,
                replay_player: replay.then(ReplayPlayer::default),
                ..PlaySessionOptions::default()
            };
            let mut snapshot = bmz_render::snapshot::RenderSnapshot::default();

            apply_placeholder_session_visuals(&mut snapshot, &profile, KeyMode::K7, &options);
            let session = build_game_session(Arc::new(chart()), &profile, options);

            assert_eq!(snapshot.bga_enabled, expected, "mode={mode:?}");
            assert_eq!(snapshot.bga_enabled, session.bga_enabled, "mode={mode:?}");
        }
    }

    #[test]
    fn placeholder_session_visuals_expose_score_save_and_play_modes() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        for (options, save, replay, practice) in [
            (PlaySessionOptions::default(), true, false, false),
            (
                PlaySessionOptions { autoplay: true, ..PlaySessionOptions::default() },
                false,
                false,
                false,
            ),
            (
                PlaySessionOptions {
                    replay_player: Some(ReplayPlayer::default()),
                    ..PlaySessionOptions::default()
                },
                false,
                true,
                false,
            ),
            (
                PlaySessionOptions { practice_mode: true, ..PlaySessionOptions::default() },
                false,
                false,
                true,
            ),
        ] {
            let mut snapshot = bmz_render::snapshot::RenderSnapshot::default();
            apply_placeholder_session_visuals(&mut snapshot, &profile, KeyMode::K7, &options);
            assert_eq!(snapshot.score_save_enabled, save);
            assert_eq!(snapshot.replay_playback, replay);
            assert_eq!(snapshot.practice_mode, practice);
        }

        let mut ghost_snapshot = bmz_render::snapshot::RenderSnapshot::default();
        apply_placeholder_session_visuals(
            &mut ghost_snapshot,
            &profile,
            KeyMode::K7,
            &PlaySessionOptions {
                session_mode: SessionMode::GhostBattle,
                replay_player: Some(ReplayPlayer::default()),
                ..PlaySessionOptions::default()
            },
        );
        assert!(!ghost_snapshot.replay_playback);
        assert!(ghost_snapshot.score_save_enabled);
    }

    #[test]
    fn placeholder_session_visuals_match_session_bga_expand() {
        for (expand, expected) in [
            (BgaExpandConfig::Full, 0),
            (BgaExpandConfig::KeepAspect, 1),
            (BgaExpandConfig::Off, 8),
        ] {
            let mut profile = ProfileConfig::new_default("default", "Default", 1);
            profile.play.bga_expand = expand;
            let options = PlaySessionOptions::default();
            let mut snapshot = bmz_render::snapshot::RenderSnapshot::default();

            apply_placeholder_session_visuals(&mut snapshot, &profile, KeyMode::K7, &options);
            let session = build_game_session(Arc::new(chart()), &profile, options);

            assert_eq!(snapshot.bga_stretch, expected, "expand={expand:?}");
            assert_eq!(snapshot.bga_stretch, session.bga_stretch, "expand={expand:?}");
        }
    }

    fn class_gauge_values(session: &GameSession) -> [f32; 6] {
        session
            .gauge
            .gauges
            .iter()
            .find(|g| g.definition.gauge_type == GaugeType::Class)
            .map(|g| g.definition.values)
            .expect("Class gauge present")
    }

    #[test]
    fn build_game_session_picks_gauge_property_from_chart_keymode() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let mut chart_k5 = chart();
        chart_k5.metadata.key_mode = KeyMode::K5;
        let mut chart_k7 = chart();
        chart_k7.metadata.key_mode = KeyMode::K7;

        let session_k5 =
            build_game_session(Arc::new(chart_k5), &profile, PlaySessionOptions::default());
        let session_k7 =
            build_game_session(Arc::new(chart_k7), &profile, PlaySessionOptions::default());

        // FIVEKEYS CLASS: PG/GR=0.01, BAD=-0.5。SEVENKEYS CLASS: PG=0.15, BAD=-1.5。
        assert_eq!(class_gauge_values(&session_k5)[0], 0.01);
        assert_eq!(class_gauge_values(&session_k5)[3], -0.5);
        assert_eq!(class_gauge_values(&session_k7)[0], 0.15);
        assert_eq!(class_gauge_values(&session_k7)[3], -1.5);
    }

    #[test]
    fn build_game_session_uses_gauge_property_override() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        // チャートは K7 だが、option で LR2 を強制する。
        let options =
            PlaySessionOptions { gauge_property: Some(GaugeProperty::Lr2), ..Default::default() };
        let session = build_game_session(Arc::new(chart()), &profile, options);

        // LR2 CLASS: BAD=-2.0、PG=0.10。
        assert_eq!(class_gauge_values(&session)[3], -2.0);
        assert_eq!(class_gauge_values(&session)[0], 0.10);
    }

    #[test]
    fn build_game_session_applies_lr2oraja_rule_mode() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.play.rule_mode = RuleMode::Lr2Oraja;

        let session =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert_eq!(session.rule_mode, RuleMode::Lr2Oraja);
        assert_eq!(session.base_judge_window.pgreat_us, 21_000);
        assert_eq!(session.base_judge_window.empty_poor_slow_us, 0);
        let hard = session
            .gauge
            .gauges
            .iter()
            .find(|g| g.definition.gauge_type == GaugeType::Hard)
            .expect("Hard gauge present");
        assert_eq!(hard.definition.guts, &[(32.0, 0.6)]);
        assert_eq!(hard.definition.death, 2.0);
    }

    #[test]
    fn build_game_session_applies_dx_rule_mode() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.play.rule_mode = RuleMode::Dx;

        let session =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert_eq!(session.rule_mode, RuleMode::Dx);
        assert_eq!(session.base_judge_window.pgreat_us, 16_666);
        assert_eq!(session.judge.windows.pgreat_us, 16_666);
        let hard = session
            .gauge
            .gauges
            .iter()
            .find(|g| g.definition.gauge_type == GaugeType::Hard)
            .expect("Hard gauge present");
        assert_eq!(hard.definition.values, [0.16, 0.16, 0.0, -4.5, -9.0, -4.5]);
    }

    #[test]
    fn build_game_session_applies_dx_9key_pop_rules() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.play.rule_mode = RuleMode::Dx;
        let mut chart = chart();
        chart.metadata.key_mode = KeyMode::K9;

        let session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());

        assert_eq!(session.base_judge_window.pgreat_us, 25_000);
        assert_eq!(session.base_judge_window.good_us, 87_500);
        assert_eq!(session.judge.window_set.long_note_end.good_us, 217_000);
        assert_eq!(session.judge.window_set.long_note_release_margin_us, 200_000);
        assert!(session.score.empty_poor_breaks_combo);
        let normal = session
            .gauge
            .gauges
            .iter()
            .find(|g| g.definition.gauge_type == GaugeType::Normal)
            .expect("Normal gauge present");
        assert_eq!((normal.definition.min, normal.definition.max), (2.0, 120.0));
        assert_eq!((normal.definition.init, normal.definition.border), (30.0, 85.0));
        assert_eq!(normal.definition.values[3..], [-2.04, -6.0, -6.0]);
    }

    #[test]
    fn build_game_session_sets_empty_poor_combo_policy_from_keymode() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let mut chart_k5 = chart();
        chart_k5.metadata.key_mode = KeyMode::K5;
        let mut chart_k7 = chart();
        chart_k7.metadata.key_mode = KeyMode::K7;

        let session_k5 =
            build_game_session(Arc::new(chart_k5), &profile, PlaySessionOptions::default());
        let session_k7 =
            build_game_session(Arc::new(chart_k7), &profile, PlaySessionOptions::default());

        assert!(session_k5.score.empty_poor_breaks_combo);
        assert!(!session_k7.score.empty_poor_breaks_combo);
    }

    #[test]
    fn mirror_permutation_k9_reverses_all_nine_keys() {
        let perm = mirror_permutation(KeyMode::K9);
        assert_eq!(perm[Lane::Key1 as usize], Lane::Key9 as usize);
        assert_eq!(perm[Lane::Key9 as usize], Lane::Key1 as usize);
        assert_eq!(perm[Lane::Key5 as usize], Lane::Key5 as usize);
    }

    #[test]
    fn arrange_lane_groups_cover_no_scratch_keymodes() {
        for key_mode in [KeyMode::K4, KeyMode::K6, KeyMode::K8, KeyMode::K9] {
            let expected: Vec<usize> =
                key_mode.active_lanes().iter().map(|&lane| lane.index()).collect();

            assert_eq!(arrange_lane_groups(key_mode, false), vec![expected.clone()]);
            assert_eq!(arrange_lane_groups(key_mode, true), vec![expected]);
        }
    }

    #[test]
    fn mirror_permutation_reverses_no_scratch_keymodes() {
        for key_mode in [KeyMode::K4, KeyMode::K6, KeyMode::K8, KeyMode::K9] {
            let perm = mirror_permutation(key_mode);
            let active = key_mode.active_lanes();

            for (source, dest) in active.iter().zip(active.iter().rev()) {
                assert_eq!(
                    perm[source.index()],
                    dest.index(),
                    "mirror should reverse {} lane {:?}",
                    key_mode.as_str(),
                    source
                );
            }
        }
    }

    #[test]
    fn random_lane_permutation_k9_preserves_active_lanes() {
        let perm = random_lane_permutation(42, KeyMode::K9, false, false);
        let active: HashSet<_> =
            KeyMode::K9.active_lanes().iter().map(|&lane| lane as usize).collect();
        let mapped: HashSet<_> =
            KeyMode::K9.active_lanes().iter().map(|&lane| perm[lane as usize]).collect();
        assert_eq!(active, mapped);
    }

    #[test]
    fn random_permutations_preserve_no_scratch_active_lanes() {
        for key_mode in [KeyMode::K4, KeyMode::K6, KeyMode::K8, KeyMode::K9] {
            let active: HashSet<_> =
                key_mode.active_lanes().iter().map(|&lane| lane.index()).collect();
            for perm in [
                random_lane_permutation(42, key_mode, false, false),
                random_lane_permutation(42, key_mode, true, false),
                rotate_lane_permutation(42, key_mode, false, false),
                rotate_lane_permutation(42, key_mode, true, false),
            ] {
                let mapped: HashSet<_> =
                    key_mode.active_lanes().iter().map(|&lane| perm[lane.index()]).collect();
                assert_eq!(
                    active,
                    mapped,
                    "random permutation should stay inside {} active lanes",
                    key_mode.as_str()
                );
            }
        }
    }

    #[test]
    fn f_random_groups_keep_odd_center_lane_fixed() {
        assert_eq!(
            f_random_lane_groups(KeyMode::K7),
            vec![
                vec![Lane::Key1.index(), Lane::Key2.index(), Lane::Key3.index()],
                vec![Lane::Key5.index(), Lane::Key6.index(), Lane::Key7.index()],
            ]
        );
        assert_eq!(
            f_random_lane_groups(KeyMode::K5),
            vec![
                vec![Lane::Key1.index(), Lane::Key2.index()],
                vec![Lane::Key4.index(), Lane::Key5.index()],
            ]
        );
        assert_eq!(
            f_random_lane_groups(KeyMode::K9),
            vec![
                vec![
                    Lane::Key1.index(),
                    Lane::Key2.index(),
                    Lane::Key3.index(),
                    Lane::Key4.index(),
                ],
                vec![
                    Lane::Key6.index(),
                    Lane::Key7.index(),
                    Lane::Key8.index(),
                    Lane::Key9.index(),
                ],
            ]
        );
    }

    #[test]
    fn f_random_groups_split_even_key_modes_into_halves() {
        assert_eq!(
            f_random_lane_groups(KeyMode::K4),
            vec![
                vec![Lane::Key1.index(), Lane::Key2.index()],
                vec![Lane::Key3.index(), Lane::Key4.index()],
            ]
        );
        assert_eq!(
            f_random_lane_groups(KeyMode::K8),
            vec![
                vec![
                    Lane::Key1.index(),
                    Lane::Key2.index(),
                    Lane::Key3.index(),
                    Lane::Key4.index(),
                ],
                vec![
                    Lane::Key5.index(),
                    Lane::Key6.index(),
                    Lane::Key7.index(),
                    Lane::Key8.index(),
                ],
            ]
        );
    }

    #[test]
    fn f_random_keeps_7k_center_lane_in_place() {
        let mut chart = chart();
        chart.metadata.key_mode = KeyMode::K7;
        chart.lane_notes[Lane::Key4.index()].push(note(1, Lane::Key4, 1_000_000));

        let applied = apply_arrange(&mut chart, ArrangeOption::FRandom, Some(42), None);

        assert_eq!(applied.arrange, ArrangeOption::FRandom);
        assert_eq!(applied.seed, Some(42));
        assert_eq!(chart.lane_notes[Lane::Key4.index()][0].lane, Lane::Key4);
        assert_eq!(chart.lane_notes[Lane::Key4.index()][0].id, NoteId(1));
    }

    #[test]
    fn mf_random_applies_mirror_after_f_random() {
        let f_random = f_random_lane_permutation(42, KeyMode::K7, ArrangeOption::FRandom, false);
        let mf_random = f_random_lane_permutation(42, KeyMode::K7, ArrangeOption::MFRandom, false);
        let mirror = mirror_permutation(KeyMode::K7);

        assert_eq!(mf_random, compose_lane_permutations(&f_random, &mirror));
        assert_eq!(mf_random[Lane::Key4.index()], Lane::Key4.index());
    }

    #[test]
    fn scratch_required_arrange_falls_back_to_normal_without_scratch_lane() {
        for key_mode in [KeyMode::K4, KeyMode::K6, KeyMode::K8, KeyMode::K9] {
            for arrange in
                [ArrangeOption::AllScratch, ArrangeOption::RandomEx, ArrangeOption::SRandomEx]
            {
                let mut chart = chart();
                chart.metadata.key_mode = key_mode;
                chart.lane_notes[Lane::Key1.index()].push(note(1, Lane::Key1, 1_000_000));
                let before = lanes_for_notes(&chart);

                let applied = apply_arrange(&mut chart, arrange, Some(7), None);

                assert_eq!(applied.arrange, ArrangeOption::Normal);
                assert_eq!(applied.seed, Some(7));
                assert_eq!(applied.pattern, None);
                assert_eq!(lanes_for_notes(&chart), before);
            }
        }
    }

    #[test]
    fn scratch_required_arrange_ignores_replay_pattern_without_scratch_lane() {
        for key_mode in [KeyMode::K4, KeyMode::K6, KeyMode::K8, KeyMode::K9] {
            let mut chart = chart();
            chart.metadata.key_mode = key_mode;
            chart.lane_notes[Lane::Key1.index()].push(note(1, Lane::Key1, 1_000_000));
            let before = lanes_for_notes(&chart);

            let mut pattern: Vec<u8> = (0u8..LANE_COUNT as u8).collect();
            pattern[Lane::Key1.index()] = Lane::Key2.index() as u8;
            pattern[Lane::Key2.index()] = Lane::Key1.index() as u8;

            let applied =
                apply_arrange(&mut chart, ArrangeOption::RandomEx, Some(7), Some(&pattern));

            assert_eq!(applied.arrange, ArrangeOption::Normal);
            assert_eq!(applied.seed, Some(7));
            assert_eq!(applied.pattern, None);
            assert_eq!(lanes_for_notes(&chart), before);
        }
    }

    #[test]
    fn note_arrange_keeps_no_scratch_modes_inside_active_lanes() {
        for key_mode in [KeyMode::K4, KeyMode::K6, KeyMode::K8, KeyMode::K9] {
            for arrange in [ArrangeOption::SRandom, ArrangeOption::Spiral, ArrangeOption::HRandom] {
                let mut chart = chart();
                chart.metadata.key_mode = key_mode;
                for (index, &lane) in key_mode.active_lanes().iter().enumerate() {
                    chart.lane_notes[lane.index()].push(note(
                        (index + 1) as u32,
                        lane,
                        1_000_000 + index as i64 * 1_000,
                    ));
                }

                apply_arrange(&mut chart, arrange, Some(7), None);

                let active: HashSet<_> =
                    key_mode.active_lanes().iter().map(|&lane| lane.index()).collect();
                for note in chart.lane_notes.iter().flatten() {
                    assert!(
                        active.contains(&note.lane.index()),
                        "{arrange:?} should keep {} note {:?} inside active lanes",
                        key_mode.as_str(),
                        note.id
                    );
                }
            }
        }
    }

    #[test]
    fn splitmix64_matches_known_seed_zero_outputs() {
        let mut rng = SplitMix64::new(0);

        assert_eq!(rng.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(rng.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(rng.next_u64(), 0x06C4_5D18_8009_454F);
    }

    #[test]
    fn random_lane_shuffle_matches_beatoraja_java_fixture() {
        // java.util.Random(42) + LaneRandomShuffleModifier's remove-at-index loop.
        let lanes = vec![0, 1, 2, 3, 4, 5, 6];
        let mut perm: Vec<usize> = (0..LANE_COUNT).collect();
        let mut rng = ArrangeRng::new(42, false);

        shuffle_lane_group(&mut rng, &lanes, &mut perm, false);

        assert_eq!(&perm[..7], &[1, 4, 5, 0, 2, 6, 3]);
    }

    #[test]
    fn random_trainer_seed_only_overrides_fresh_7k_random() {
        let trainer_seed = Some(322);
        let normal_seed = Some(42);
        let recorded_pattern = [0, 7, 6, 5, 4, 3, 2, 1];

        assert_eq!(
            effective_arrange_seed(
                KeyMode::K7,
                ArrangeOption::Random,
                normal_seed,
                trainer_seed,
                None,
            ),
            trainer_seed
        );
        assert_eq!(
            effective_arrange_seed(
                KeyMode::K5,
                ArrangeOption::Random,
                normal_seed,
                trainer_seed,
                None,
            ),
            normal_seed
        );
        assert_eq!(
            effective_arrange_seed(
                KeyMode::K7,
                ArrangeOption::Mirror,
                normal_seed,
                trainer_seed,
                None,
            ),
            normal_seed
        );
        assert_eq!(
            effective_arrange_seed(
                KeyMode::K7,
                ArrangeOption::Random,
                normal_seed,
                trainer_seed,
                Some(&recorded_pattern),
            ),
            normal_seed,
            "a replay or same-arrange retry pattern must take priority"
        );
    }

    #[test]
    fn random_trainer_compatible_seed_applies_requested_7k_order() {
        let mut chart = chart();
        chart.metadata.key_mode = KeyMode::K7;
        let seed = crate::random_trainer::seed_for_lane_order([2, 1, 4, 3, 6, 5, 7])
            .expect("known lane order must resolve");

        let applied =
            apply_arrange(&mut chart, ArrangeOption::Random, Some(i64::from(seed.value())), None);
        let pattern = applied.pattern.expect("RANDOM must record its lane permutation");

        assert_eq!(applied.seed, Some(322));
        assert_eq!(&pattern[..8], &[0, 2, 1, 4, 3, 6, 5, 7]);
        assert_eq!(pattern[Lane::Scratch.index()], Lane::Scratch.index() as u8);
        assert_eq!(
            &pattern[Lane::Key8.index()..],
            &(Lane::Key8.index() as u8..LANE_COUNT as u8).collect::<Vec<_>>()
        );
    }

    #[test]
    fn apply_arrange_random_moves_notes_between_lanes() {
        use bmz_chart::model::{NoteEvent, NoteKind};
        use bmz_core::time::ChartTick;

        let mut chart = chart();
        chart.metadata.key_mode = KeyMode::K7;
        chart.lane_notes[Lane::Key1.index()].push(NoteEvent {
            id: NoteId(1),
            lane: Lane::Key1,
            kind: NoteKind::Tap,
            tick: ChartTick(0),
            time: TimeUs(1_000_000),
            sound: None,
            damage: None,
        });

        let applied = apply_arrange(&mut chart, ArrangeOption::Random, Some(42), None);

        assert_eq!(applied.arrange, ArrangeOption::Random);
        assert_ne!(applied.pattern, Some((0u8..LANE_COUNT as u8).collect()));
        assert!(chart.lane_notes[Lane::Key1.index()].is_empty());
        assert!(chart.lane_notes.iter().enumerate().any(|(lane_index, notes)| lane_index
            != Lane::Key1.index()
            && notes.iter().any(|note| note.id == NoteId(1) && note.lane.index() == lane_index)));
    }

    #[test]
    fn rotate_random_uses_non_identity_lane_rotation() {
        let perm = rotate_lane_permutation(7, KeyMode::K7, false, false);
        let key_lanes: Vec<usize> = (Lane::Key1.index()..=Lane::Key7.index()).collect();
        let mapped: HashSet<_> = key_lanes.iter().map(|&lane| perm[lane]).collect();

        assert_eq!(mapped, key_lanes.iter().copied().collect());
        assert!(key_lanes.iter().any(|&lane| perm[lane] != lane));
        assert_eq!(perm[Lane::Scratch.index()], Lane::Scratch.index());
    }

    #[test]
    fn random_ex_includes_scratch_lane() {
        let mut chart = chart();
        chart.metadata.key_mode = KeyMode::K7;
        chart.lane_notes[Lane::Scratch.index()].push(note(1, Lane::Scratch, 1_000_000));

        let applied = apply_arrange(&mut chart, ArrangeOption::RandomEx, Some(1), None);

        assert_eq!(applied.arrange, ArrangeOption::RandomEx);
        assert!(chart.lane_notes.iter().enumerate().any(|(lane_index, notes)| lane_index
            != Lane::Scratch.index()
            && notes.iter().any(|note| note.id == NoteId(1) && note.lane.index() == lane_index)));
    }

    #[test]
    fn random2_arranges_only_dp_second_player_lanes() {
        let mut chart = chart();
        chart.metadata.key_mode = KeyMode::K14;
        chart.lane_notes[Lane::Key1.index()].push(note(1, Lane::Key1, 1_000_000));
        chart.lane_notes[Lane::Key8.index()].push(note(2, Lane::Key8, 1_000_000));

        let applied = apply_arrange_pair(
            &mut chart,
            ArrangeOption::Normal,
            ArrangeOption::Mirror,
            Some(1),
            Some(2),
            false,
            None,
        );

        assert_eq!(applied.arrange, ArrangeOption::Normal);
        assert_eq!(applied.seed, Some(1));
        assert_eq!(applied.seed_2p, Some(2));
        assert_eq!(applied.packed_beatoraja_seed(KeyMode::K14), Some(1 + (2 << 24)));
        assert_eq!(chart.lane_notes[Lane::Key1.index()][0].id, NoteId(1));
        assert!(chart.lane_notes[Lane::Key8.index()].is_empty());
        assert!(
            chart.lane_notes[Lane::Key14.index()]
                .iter()
                .any(|note| note.id == NoteId(2) && note.lane == Lane::Key14)
        );
    }

    #[test]
    fn recorded_sp_pattern_does_not_gain_a_second_player_seed() {
        let mut chart = chart();
        chart.metadata.key_mode = KeyMode::K7;
        let pattern: Vec<u8> = (0..LANE_COUNT as u8).collect();

        let applied = apply_arrange_pair(
            &mut chart,
            ArrangeOption::Random,
            ArrangeOption::Normal,
            Some(1),
            None,
            false,
            Some(&pattern),
        );

        assert_eq!(applied.seed, Some(1));
        assert_eq!(applied.seed_2p, None);
        assert_eq!(applied.packed_beatoraja_seed(KeyMode::K7), Some(1));
    }

    #[test]
    fn double_option_flip_swaps_dp_player_lanes() {
        let mut chart = chart();
        chart.metadata.key_mode = KeyMode::K14;
        chart.lane_notes[Lane::Scratch.index()].push(note(1, Lane::Scratch, 1_000_000));
        chart.lane_notes[Lane::Key1.index()].push(note(2, Lane::Key1, 1_000_000));
        chart.lane_notes[Lane::Scratch2.index()].push(note(3, Lane::Scratch2, 1_000_000));
        chart.lane_notes[Lane::Key8.index()].push(note(4, Lane::Key8, 1_000_000));

        apply_double_option(&mut chart, DoubleOption::Flip);

        assert!(
            chart.lane_notes[Lane::Scratch2.index()]
                .iter()
                .any(|note| note.id == NoteId(1) && note.lane == Lane::Scratch2)
        );
        assert!(
            chart.lane_notes[Lane::Key8.index()]
                .iter()
                .any(|note| note.id == NoteId(2) && note.lane == Lane::Key8)
        );
        assert!(
            chart.lane_notes[Lane::Scratch.index()]
                .iter()
                .any(|note| note.id == NoteId(3) && note.lane == Lane::Scratch)
        );
        assert!(
            chart.lane_notes[Lane::Key1.index()]
                .iter()
                .any(|note| note.id == NoteId(4) && note.lane == Lane::Key1)
        );
    }

    #[test]
    fn double_option_battle_duplicates_sp_lanes_as_dp() {
        let mut chart = chart();
        chart.metadata.key_mode = KeyMode::K7;
        chart.total_notes = 2;
        chart.lane_notes[Lane::Scratch.index()].push(note(1, Lane::Scratch, 1_000_000));
        chart.lane_notes[Lane::Key1.index()].push(note(2, Lane::Key1, 1_010_000));

        apply_double_option(&mut chart, DoubleOption::Battle);

        assert_eq!(chart.metadata.key_mode, KeyMode::K14);
        assert_eq!(chart.total_notes, 4);
        assert!(
            chart.lane_notes[Lane::Scratch.index()]
                .iter()
                .any(|note| note.id == NoteId(1) && note.lane == Lane::Scratch)
        );
        assert!(
            chart.lane_notes[Lane::Scratch2.index()]
                .iter()
                .any(|note| note.id != NoteId(1) && note.lane == Lane::Scratch2)
        );
        assert!(
            chart.lane_notes[Lane::Key1.index()]
                .iter()
                .any(|note| note.id == NoteId(2) && note.lane == Lane::Key1)
        );
        assert!(
            chart.lane_notes[Lane::Key8.index()]
                .iter()
                .any(|note| note.id != NoteId(2) && note.lane == Lane::Key8)
        );
    }

    #[test]
    fn s_random_is_reproducible_from_seed() {
        let mut first = chart_with_two_notes_same_lane();
        let mut second = chart_with_two_notes_same_lane();

        let first_applied = apply_arrange(&mut first, ArrangeOption::SRandom, Some(99), None);
        let _second_applied = apply_arrange(&mut second, ArrangeOption::SRandom, Some(99), None);

        assert_eq!(first_applied.pattern, None);
        assert_eq!(lanes_for_notes(&first), lanes_for_notes(&second));
    }

    #[test]
    fn s_random_keeps_long_note_end_on_start_lane() {
        use bmz_chart::model::{LongNoteMode, LongNotePair, LongNoteStyle};
        use bmz_core::time::ChartTick;

        let mut chart = chart();
        chart.metadata.key_mode = KeyMode::K7;
        chart.lane_notes[Lane::Key1.index()].push(NoteEvent {
            kind: NoteKind::LongStart,
            tick: ChartTick(0),
            ..note(1, Lane::Key1, 1_000_000)
        });
        chart.lane_notes[Lane::Key1.index()].push(NoteEvent {
            kind: NoteKind::LongEnd,
            tick: ChartTick(48),
            ..note(2, Lane::Key1, 2_000_000)
        });
        chart.long_notes.push(LongNotePair {
            lane: Lane::Key1,
            style: LongNoteStyle::ChannelPair,
            mode: Some(LongNoteMode::Cn),
            start_note_id: NoteId(1),
            end_note_id: NoteId(2),
            start_tick: ChartTick(0),
            end_tick: ChartTick(48),
            start_time: TimeUs(1_000_000),
            end_time: TimeUs(2_000_000),
            sound: None,
        });

        apply_arrange(&mut chart, ArrangeOption::SRandom, Some(5), None);

        let start_lane = chart
            .lane_notes
            .iter()
            .flatten()
            .find(|note| note.id == NoteId(1))
            .map(|note| note.lane)
            .expect("start note");
        let end_lane = chart
            .lane_notes
            .iter()
            .flatten()
            .find(|note| note.id == NoteId(2))
            .map(|note| note.lane)
            .expect("end note");
        assert_eq!(start_lane, end_lane);
        assert_eq!(chart.long_notes[0].lane, start_lane);
    }

    #[test]
    fn build_game_session_enables_gauge_auto_shift_from_profile() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.play.gauge_auto_shift =
            crate::config::profile_config::GaugeAutoShiftConfig::BestClear;
        let chart = Arc::new(chart());

        let session = build_game_session(chart, &profile, PlaySessionOptions::default());

        assert!(session.gauge.auto_shift);
        assert_eq!(session.gauge.auto_shift_mode, GaugeAutoShiftMode::BestClear);
        assert_eq!(session.gauge.selected, GaugeType::Hazard);
    }

    #[test]
    fn build_game_session_uses_hidden_cover_only_for_hidden_effects() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hidden = 400;
        profile.play.lane_effect = LaneEffectConfig::Off;
        let off = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        profile.play.lane_effect = LaneEffectConfig::Hidden;
        let hidden = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert_eq!(off.hidden_cover, 0.0);
        assert_eq!(hidden.hidden_cover, 0.4);
    }

    #[test]
    fn build_game_session_maps_lane_cover_and_lift_skin_options_from_values() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.play.lane_effect = LaneEffectConfig::Off;
        profile.lane.sudden = 290;
        profile.lane.lift = 222;

        let session =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert!(session.lanecover_enabled);
        assert!(session.lift_enabled);

        profile.lane.sudden = 0;
        profile.lane.lift = 0;
        let disabled =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert!(!disabled.lanecover_enabled);
        assert!(disabled.lift_enabled);

        profile.lane.lift = 222;
        profile.lane.lift_enabled = false;
        let lift_disabled =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert_eq!(lift_disabled.lift, 0.0);
        assert!(!lift_disabled.lift_enabled);

        profile.play.lane_effect = LaneEffectConfig::Sudden;
        let sudden_option =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert!(sudden_option.lanecover_enabled);
    }

    #[test]
    fn build_game_session_clamps_lane_cover_to_remaining_lift_range() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.sudden = 900;
        profile.lane.lift = 200;

        let session =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert!((session.lane_cover - 0.8).abs() < 0.000_01);
        assert!((session.lift - 0.2).abs() < 0.000_01);
        assert!(session.lanecover_enabled);
    }

    #[test]
    fn build_game_session_clamps_profile_misslayer_duration() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.play.misslayer_duration_ms = 12_000;

        let session =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert_eq!(session.poor_bga_duration_us, 5_000_000);
    }

    #[test]
    fn build_game_session_maps_profile_bga_expand() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);

        profile.play.bga_expand = BgaExpandConfig::Full;
        let full = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
        profile.play.bga_expand = BgaExpandConfig::KeepAspect;
        let keep = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
        profile.play.bga_expand = BgaExpandConfig::Off;
        let off = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert_eq!(full.bga_stretch, 0);
        assert_eq!(keep.bga_stretch, 1);
        assert_eq!(off.bga_stretch, 8);
    }

    #[test]
    fn build_game_session_maps_profile_bga_mode() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);

        profile.play.bga = BgaModeConfig::Off;
        let off = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        profile.play.bga = BgaModeConfig::Auto;
        let auto_human =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
        let auto_autoplay = build_game_session(
            Arc::new(chart()),
            &profile,
            PlaySessionOptions { autoplay: true, ..PlaySessionOptions::default() },
        );
        let auto_replay = build_game_session(
            Arc::new(chart()),
            &profile,
            PlaySessionOptions {
                replay_player: Some(ReplayPlayer::default()),
                ..PlaySessionOptions::default()
            },
        );

        assert!(!off.bga_enabled);
        assert!(!auto_human.bga_enabled);
        assert!(auto_autoplay.bga_enabled);
        assert!(auto_replay.bga_enabled);
    }

    #[test]
    fn build_game_session_copies_selected_play_slot_offsets() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.skin.play7_offsets.push(crate::config::profile_config::SkinOffsetConfig {
            name: None,
            id: 42,
            x: 1,
            y: 2,
            w: 3,
            h: 4,
            r: 5,
            a: -6,
        });
        profile.skin.play14_offsets.push(crate::config::profile_config::SkinOffsetConfig {
            name: None,
            id: 42,
            h: 99,
            ..Default::default()
        });

        let session =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert_eq!(
            session.skin_offsets,
            vec![PlaySkinOffset { id: 42, x: 1, y: 2, w: 3, h: 4, r: 5, a: -6 }]
        );
    }

    #[test]
    fn build_game_session_uses_active_offsets_instead_of_skin_history() {
        use crate::config::profile_config::{SkinHistoryEntryConfig, SkinOffsetConfig};

        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.skin.play7 = "data/skins/ECFN/play/play7.luaskin".to_string();
        profile.skin.play7_offsets = vec![SkinOffsetConfig { id: 30, h: 12, ..Default::default() }];
        profile.skin.history.insert(
            profile.skin.play7.clone(),
            SkinHistoryEntryConfig {
                offsets: vec![SkinOffsetConfig { id: 30, h: 48, ..Default::default() }],
                ..Default::default()
            },
        );

        let session =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert_eq!(
            session.skin_offsets,
            vec![PlaySkinOffset { id: 30, x: 0, y: 0, w: 0, h: 12, r: 0, a: 0 }]
        );
    }

    #[test]
    fn build_game_session_keeps_active_offsets_with_empty_skin_history() {
        use crate::config::profile_config::{SkinHistoryEntryConfig, SkinOffsetConfig};

        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.skin.play7 = "resource:skins/ECFN/play/play7.luaskin".to_string();
        profile.skin.play7_offsets = vec![
            SkinOffsetConfig { id: 43, a: 180, ..Default::default() },
            SkinOffsetConfig { id: 44, a: 110, ..Default::default() },
        ];
        profile.skin.history.insert(profile.skin.play7.clone(), SkinHistoryEntryConfig::default());

        let session =
            build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert_eq!(
            session.skin_offsets,
            vec![
                PlaySkinOffset { id: 43, x: 0, y: 0, w: 0, h: 0, r: 0, a: 180 },
                PlaySkinOffset { id: 44, x: 0, y: 0, w: 0, h: 0, r: 0, a: 110 },
            ]
        );
    }

    #[test]
    fn build_game_session_clamps_profile_hispeed() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed = 11.0;
        let high = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
        profile.lane.hispeed = 0.25;
        let low = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

        assert_eq!(high.hispeed, 10.0);
        assert_eq!(low.hispeed, 0.5);
    }

    #[test]
    fn build_game_session_initializes_floating_hispeed_for_chart_bpm() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.lane.target_green_number = 300;
        // Stale value from a 120 BPM chart with green number 300.
        profile.lane.hispeed = 4.0;
        let mut fast_chart = chart();
        fast_chart.metadata.initial_bpm = 240.0;

        let session = build_game_session(
            Arc::new(fast_chart),
            &profile,
            PlaySessionOptions { hs_fix: HsFixOption::StartBpm, ..PlaySessionOptions::default() },
        );

        assert_eq!(session.hispeed_mode, HispeedMode::Floating);
        assert_eq!(session.target_green_number, 300);
        assert!((session.hispeed - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn build_game_session_uses_hsfix_to_select_hispeed_mode() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.lane.hispeed = 4.0;
        profile.lane.target_green_number = 300;
        let normal = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
        assert_eq!(normal.hispeed_mode, HispeedMode::Normal);
        assert_eq!(normal.hispeed, 4.0);

        profile.lane.hispeed_mode = HispeedModeConfig::Normal;
        let floating = build_game_session(
            Arc::new(chart()),
            &profile,
            PlaySessionOptions { hs_fix: HsFixOption::StartBpm, ..PlaySessionOptions::default() },
        );
        assert_eq!(floating.hispeed_mode, HispeedMode::Floating);
        assert!((floating.hispeed - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn build_game_session_initializes_floating_hispeed_for_hsfix_base_bpm() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.lane.target_green_number = 300;
        let mut bpm_chart = chart();
        bpm_chart.metadata.initial_bpm = 120.0;
        bpm_chart.timing_events.push(bmz_chart::model::TimingEvent {
            tick: bmz_core::time::ChartTick(48),
            time: TimeUs(1_000_000),
            kind: TimingEventKind::BpmChange { bpm: 240.0 },
        });

        let session = build_game_session(
            Arc::new(bpm_chart),
            &profile,
            PlaySessionOptions { hs_fix: HsFixOption::MaxBpm, ..PlaySessionOptions::default() },
        );

        assert_eq!(session.hsfix_base_bpm, 240.0);
        assert_eq!(session.hsfix_index, 2);
        assert!((session.hispeed - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn main_bpm_uses_bpm_with_most_notes() {
        let mut bpm_chart = chart();
        bpm_chart.timing_events.push(bmz_chart::model::TimingEvent {
            tick: bmz_core::time::ChartTick(48),
            time: TimeUs(1_000_000),
            kind: TimingEventKind::BpmChange { bpm: 180.0 },
        });
        bpm_chart.lane_notes[Lane::Key1.index()].push(note(1, Lane::Key1, 0));
        bpm_chart.lane_notes[Lane::Key2.index()].push(note(2, Lane::Key2, 1_100_000));
        bpm_chart.lane_notes[Lane::Key3.index()].push(note(3, Lane::Key3, 1_200_000));
        let timing_map = bmz_chart::timing::TimingMap::from_chart_timing_events(
            bpm_chart.metadata.initial_bpm,
            &bpm_chart.timing_events,
        );

        assert_eq!(hsfix_base_bpm_for_chart(&bpm_chart, &timing_map, HsFixOption::MainBpm), 180.0);
    }

    #[test]
    fn build_game_session_accepts_custom_input_backend() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let mut backend = BufferedInputBackend::default();
        backend.push(DeviceInputEvent {
            device: DeviceId(1),
            control: PhysicalControl::KeyboardKey("Z".to_string()),
            kind: InputKind::Press,
            timestamp: DeviceTimestamp::Unknown,
            bounce_policy: Default::default(),
        });
        let chart = Arc::new(chart());
        let mut session = build_game_session_with_input_backend(
            chart,
            &profile,
            PlaySessionOptions::default(),
            Box::new(backend),
        );
        let ctx = InputTimingContext {
            audio_clock: &session.audio_clock,
            offsets: session.offsets,
            timestamp_anchor: None,
        };

        let inputs = session.input_system.collect_game_inputs(&ctx);

        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].lane, Lane::Key1);
    }

    #[test]
    fn load_game_session_for_chart_imports_linked_file() {
        let path = write_temp_bms(
            "\
#TITLE Linked
#BPM 120
#00011:01
",
        );
        let imported = import_bms_chart(&path, None, true).unwrap();
        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut library_db = LibraryDatabase::from_connection(conn);
        let chart_id = library_db
            .upsert_chart_import(&ChartImportRecord {
                root_id: None,
                file_path: &path,
                file_size: 1,
                modified_at: 1,
                scanned_at: 1,
                chart: &imported.chart,
            })
            .unwrap();
        let profile = ProfileConfig::new_default("default", "Default", 1);

        let session = load_game_session_for_chart(
            &library_db,
            chart_id,
            &profile,
            PlaySessionOptions::default(),
        )
        .unwrap();

        assert_eq!(session.chart.metadata.title, "Linked");

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn load_transformed_chart_applies_start_note_margin() {
        let path = write_temp_bms(
            "\
#TITLE Early Note
#BPM 120
#00011:01
#00201:01
",
        );
        let imported = import_bms_chart(&path, None, true).unwrap();
        let source_first =
            imported.chart.lane_notes.iter().flatten().map(|note| note.time.0).min().unwrap();
        assert_eq!(source_first, 0);

        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut library_db = LibraryDatabase::from_connection(conn);
        let chart_id = library_db
            .upsert_chart_import(&ChartImportRecord {
                root_id: None,
                file_path: &path,
                file_size: 1,
                modified_at: 1,
                scanned_at: 1,
                chart: &imported.chart,
            })
            .unwrap();

        let transformed =
            load_transformed_chart_for_play(&library_db, chart_id, &PlaySessionOptions::default())
                .unwrap();
        let play_first =
            transformed.chart.lane_notes.iter().flatten().map(|note| note.time.0).min().unwrap();
        assert_eq!(play_first, 1_000_000);

        let source = load_source_chart_for_chart(&library_db, chart_id, None).unwrap();
        let source_first_again =
            source.lane_notes.iter().flatten().map(|note| note.time.0).min().unwrap();
        assert_eq!(source_first_again, 0, "source chart must stay unshifted");

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn load_game_session_counts_cn_ends_from_source_chart() {
        let path = write_temp_bms(
            "\
#TITLE Source CN
#BPM 120
#LNMODE 2
#LNOBJ ZZ
#00011:01ZZ
",
        );
        let imported = import_bms_chart(&path, None, true).unwrap();
        assert_eq!(imported.chart.total_notes, 1);
        assert_eq!(imported.chart.long_notes.len(), 1);
        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut library_db = LibraryDatabase::from_connection(conn);
        let chart_id = library_db
            .upsert_chart_import(&ChartImportRecord {
                root_id: None,
                file_path: &path,
                file_size: 1,
                modified_at: 1,
                scanned_at: 1,
                chart: &imported.chart,
            })
            .unwrap();
        let stored = library_db.list_charts_by_ids(&[chart_id]).unwrap().remove(0);
        assert_eq!(stored.total_notes, 1);
        assert_eq!(stored.ln_counts.defined_cn_pairs, 1);
        assert_eq!(stored.scored_total_notes_for_setting(LnPolicySetting::AutoLn), 2);
        library_db
            .conn()
            .execute(
                "UPDATE charts SET total_notes = 999, mode = '14K' WHERE id = ?1",
                rusqlite::params![chart_id],
            )
            .unwrap();
        let source_chart = load_source_chart_for_chart(&library_db, chart_id, None).unwrap();
        assert_eq!(source_chart.metadata.key_mode, KeyMode::K5);
        assert_eq!(source_chart.identity.file_sha256, imported.chart.identity.file_sha256);
        assert_eq!(
            scored_note_count_for_chart(&library_db, chart_id, &PlaySessionOptions::default())
                .unwrap(),
            2,
            "course pre-count must ignore stale library totals"
        );
        let force_ln = PlaySessionOptions {
            ln_mode_override: Some(bmz_chart::model::LongNoteMode::Ln),
            ..Default::default()
        };
        assert_eq!(scored_note_count_for_chart(&library_db, chart_id, &force_ln).unwrap(), 1);
        let battle =
            PlaySessionOptions { double_option: DoubleOption::Battle, ..Default::default() };
        assert_eq!(scored_note_count_for_chart(&library_db, chart_id, &battle).unwrap(), 4);
        let profile = ProfileConfig::new_default("default", "Default", 1);

        let session = load_game_session_for_chart(
            &library_db,
            chart_id,
            &profile,
            PlaySessionOptions::default(),
        )
        .unwrap();

        assert_eq!(session.chart.total_notes, 1);
        assert_eq!(session.scored_total_notes, 2);

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn load_prepared_play_session_for_chart_loads_audio_samples() {
        let (path, wav_path) = write_temp_bms_with_wav(
            "\
#TITLE Prepared
#BPM 120
#WAV01 test.wav
#00011:01
",
        );
        let imported = import_bms_chart(&path, None, true).unwrap();
        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut library_db = LibraryDatabase::from_connection(conn);
        let chart_id = library_db
            .upsert_chart_import(&ChartImportRecord {
                root_id: None,
                file_path: &path,
                file_size: 1,
                modified_at: 1,
                scanned_at: 1,
                chart: &imported.chart,
            })
            .unwrap();
        let profile = ProfileConfig::new_default("default", "Default", 1);

        let prepared = load_prepared_play_session_for_chart(
            &library_db,
            chart_id,
            &profile,
            PlaySessionOptions::default(),
        )
        .unwrap();

        assert_eq!(prepared.session.chart.metadata.title, "Prepared");
        assert_eq!(prepared.audio.mixer.output_sample_rate, 48_000);
        assert!(matches!(prepared.sample_report[0].status, LoadedSampleStatus::Loaded));
        assert!(prepared.audio.samples.get(SoundId(0)).is_some());

        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(wav_path).unwrap();
    }

    #[test]
    fn preload_reports_applied_arrange_before_audio_progress() {
        let (path, wav_path) = write_temp_bms_with_wav(
            "\
#TITLE Arrange preview
#BPM 120
#WAV01 test.wav
#00011:01
",
        );
        let imported = import_bms_chart(&path, None, true).unwrap();
        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut library_db = LibraryDatabase::from_connection(conn);
        let chart_id = library_db
            .upsert_chart_import(&ChartImportRecord {
                root_id: None,
                file_path: &path,
                file_size: 1,
                modified_at: 1,
                scanned_at: 1,
                chart: &imported.chart,
            })
            .unwrap();
        let reported_arrange = RefCell::new(None);

        let preloaded = preload_play_session_for_chart_with_callbacks(
            &library_db,
            chart_id,
            PlaySessionOptions {
                arrange: ArrangeOption::Random,
                arrange_seed: Some(42),
                ..Default::default()
            },
            |arrange| {
                *reported_arrange.borrow_mut() = Some(arrange.clone());
            },
            |_, _| {
                assert!(
                    reported_arrange.borrow().is_some(),
                    "arrange must be available before WAV progress"
                );
            },
        )
        .unwrap();

        let reported_arrange = reported_arrange.into_inner().expect("reported arrange");
        assert_eq!(reported_arrange.pattern, preloaded.applied_arrange.pattern);
        assert_eq!(reported_arrange.arrange, ArrangeOption::Random);

        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(wav_path).unwrap();
    }

    #[test]
    fn retry_audio_reload_preserves_bgm_and_keysound_asset_mapping() {
        let (path, bgm_path, key_path) = write_temp_bms_with_two_wavs(
            "\
#TITLE Retry audio
#BPM 120
#WAV01 bgm.wav
#WAV02 key.wav
#00001:01
#00011:02
",
        );
        let imported = import_bms_chart(&path, None, true).unwrap();
        let chart = Arc::new(imported.chart);
        let score_key =
            ScoreKey::new(chart.identity.file_sha256, crate::ln_policy::LnScorePolicy::AutoLn);
        let mut progress = Vec::new();

        let preloaded = preload_play_session_reloading_audio_with_progress(
            Arc::clone(&chart),
            48_000,
            0.75,
            normal_applied_arrange(0, false),
            score_key,
            |loaded, total| progress.push((loaded, total)),
        );

        assert!(Arc::ptr_eq(&preloaded.chart, &chart));
        assert_eq!(preloaded.chart_normalization_gain, 0.75);
        assert_eq!(preloaded.score_key, score_key);
        assert!(
            preloaded
                .sample_report
                .iter()
                .all(|report| matches!(report.status, LoadedSampleStatus::Loaded))
        );
        let bgm_id = preloaded
            .chart
            .sounds
            .iter()
            .find(|asset| asset.path == bgm_path)
            .map(|asset| asset.id)
            .expect("BGM asset");
        let key_id = preloaded
            .chart
            .sounds
            .iter()
            .find(|asset| asset.path == key_path)
            .map(|asset| asset.id)
            .expect("keysound asset");
        assert_eq!(preloaded.chart.bgm_events.first().map(|event| event.sound), Some(bgm_id));
        assert_eq!(
            preloaded.chart.lane_notes.iter().flatten().find_map(|note| note.sound),
            Some(key_id)
        );
        assert!(preloaded.audio.samples.get(bgm_id).unwrap().frames[0] > 0.4);
        assert!(preloaded.audio.samples.get(key_id).unwrap().frames[0] < -0.4);
        assert_eq!(progress.last(), Some(&(2, 2)));

        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(bgm_path).unwrap();
        std::fs::remove_file(key_path).unwrap();
    }

    fn chart() -> PlayableChart {
        PlayableChart {
            identity: compute_chart_identity(b"session"),
            metadata: ChartMetadata {
                title: "session".to_string(),
                initial_bpm: 120.0,
                total: Some(160.0),
                ..Default::default()
            },
            lane_notes: std::array::from_fn(|_| Vec::new()),
            long_notes: Vec::new(),
            bgm_events: Vec::new(),
            bga_events: Vec::new(),
            timing_events: Vec::new(),

            scroll_events: Vec::new(),

            speed_events: Vec::new(),
            judge_rank_events: Vec::new(),
            bgm_volume_events: Vec::new(),
            key_volume_events: Vec::new(),
            text_events: Vec::new(),
            bga_opacity_events: Vec::new(),
            bga_argb_events: Vec::new(),
            swbga_definitions: Vec::new(),
            bga_keybound_events: Vec::new(),
            bga_asset_by_bmp_key: std::collections::HashMap::new(),
            bar_lines: Vec::new(),
            sounds: Vec::<SoundAssetRef>::new(),
            bga_assets: Vec::new(),
            total_notes: 1,
            end_time: TimeUs(0),
        }
    }

    fn note(id: u32, lane: Lane, time_us: i64) -> NoteEvent {
        use bmz_core::time::ChartTick;

        NoteEvent {
            id: NoteId(id),
            lane,
            kind: NoteKind::Tap,
            tick: ChartTick((time_us / 1_000) as u64),
            time: TimeUs(time_us),
            sound: None,
            damage: None,
        }
    }

    fn chart_with_two_notes_same_lane() -> PlayableChart {
        let mut chart = chart();
        chart.metadata.key_mode = KeyMode::K7;
        chart.lane_notes[Lane::Key1.index()].push(note(1, Lane::Key1, 1_000_000));
        chart.lane_notes[Lane::Key1.index()].push(note(2, Lane::Key1, 1_020_000));
        chart
    }

    fn lanes_for_notes(chart: &PlayableChart) -> Vec<(NoteId, Lane)> {
        let mut lanes: Vec<_> =
            chart.lane_notes.iter().flatten().map(|note| (note.id, note.lane)).collect();
        lanes.sort_by_key(|(id, _)| *id);
        lanes
    }

    fn write_temp_bms(text: &str) -> std::path::PathBuf {
        let stamp =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir()
            .join(format!("bmz-play-session-{}-{stamp}.bms", std::process::id()));
        std::fs::write(&path, text).unwrap();
        path
    }

    fn write_temp_bms_with_wav(text: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let stamp =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir();
        let bms_path = dir.join(format!("bmz-prepared-session-{}-{stamp}.bms", std::process::id()));
        let wav_name = format!("bmz-prepared-session-{}-{stamp}.wav", std::process::id());
        let wav_path = dir.join(&wav_name);
        std::fs::write(&bms_path, text.replace("test.wav", &wav_name)).unwrap();
        std::fs::write(
            &wav_path,
            [wav_header(1, 1, 48_000, 16, 2).as_slice(), &[0x00, 0x40]].concat(),
        )
        .unwrap();
        (bms_path, wav_path)
    }

    fn write_temp_bms_with_two_wavs(
        text: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let stamp =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir();
        let prefix = format!("bmz-retry-audio-{}-{stamp}", std::process::id());
        let bms_path = dir.join(format!("{prefix}.bms"));
        let bgm_path = dir.join(format!("{prefix}-bgm.wav"));
        let key_path = dir.join(format!("{prefix}-key.wav"));
        let bms = text
            .replace("bgm.wav", bgm_path.file_name().unwrap().to_str().unwrap())
            .replace("key.wav", key_path.file_name().unwrap().to_str().unwrap());
        std::fs::write(&bms_path, bms).unwrap();
        std::fs::write(
            &bgm_path,
            [wav_header(1, 1, 48_000, 16, 2).as_slice(), &16_384_i16.to_le_bytes()].concat(),
        )
        .unwrap();
        std::fs::write(
            &key_path,
            [wav_header(1, 1, 48_000, 16, 2).as_slice(), &(-16_384_i16).to_le_bytes()].concat(),
        )
        .unwrap();
        (bms_path, bgm_path, key_path)
    }

    fn wav_header(
        format: u16,
        channels: u16,
        sample_rate: u32,
        bits: u16,
        data_len: u32,
    ) -> Vec<u8> {
        let byte_rate = sample_rate * channels as u32 * bits as u32 / 8;
        let block_align = channels * bits / 8;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16_u32.to_le_bytes());
        out.extend_from_slice(&format.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        out
    }
}
