use super::*;

/// 設定値を 1 ステップ変更する。変更があった場合 `true`。
pub fn adjust_settings_value(profile: &mut ProfileConfig, id: SettingsEntryId, delta: i32) -> bool {
    if delta == 0 {
        return false;
    }
    match id {
        SettingsEntryId::NormalizeChartVolume => {
            profile.audio_mix.normalize_chart_volume = !profile.audio_mix.normalize_chart_volume;
            true
        }
        SettingsEntryId::MasterVolume => {
            adjust_u32(&mut profile.audio_mix.master_volume, delta, 0, 100)
        }
        SettingsEntryId::KeyVolume => adjust_u32(&mut profile.audio_mix.key_volume, delta, 0, 100),
        SettingsEntryId::BgmVolume => adjust_u32(&mut profile.audio_mix.bgm_volume, delta, 0, 100),
        SettingsEntryId::PreviewVolume => {
            adjust_u32(&mut profile.audio_mix.preview_volume, delta, 0, 100)
        }
        SettingsEntryId::SystemBgmVolume => {
            adjust_u32(&mut profile.audio_mix.system_bgm_volume, delta, 0, 100)
        }
        SettingsEntryId::SystemSeVolume => {
            adjust_u32(&mut profile.audio_mix.system_se_volume, delta, 0, 100)
        }
        SettingsEntryId::InputOffsetMs => {
            adjust_offset_ms(&mut profile.judge.input_offset_us, delta)
        }
        SettingsEntryId::VisualOffsetMs => {
            adjust_offset_ms(&mut profile.judge.visual_offset_us, delta)
        }
        SettingsEntryId::VisualOffsetAutoAdjust => {
            profile.judge.visual_offset_auto_adjust = !profile.judge.visual_offset_auto_adjust;
            true
        }
        SettingsEntryId::JudgeAlgorithm => {
            cycle_enum(delta, profile.judge.judge_algorithm, cycle_judge_algorithm)
                .map(|next| profile.judge.judge_algorithm = next)
                .is_some()
        }
        SettingsEntryId::RuleMode => cycle_enum(delta, profile.play.rule_mode, cycle_rule_mode)
            .map(|next| profile.play.rule_mode = next)
            .is_some(),
        SettingsEntryId::LnModePolicy => {
            cycle_enum(delta, profile.play.ln_mode_policy, cycle_ln_mode_policy)
                .map(|next| profile.play.ln_mode_policy = next)
                .is_some()
        }
        SettingsEntryId::Gauge => cycle_enum(delta, profile.play.gauge, cycle_gauge)
            .map(|next| profile.play.gauge = next)
            .is_some(),
        SettingsEntryId::GaugeAutoShift => {
            cycle_enum(delta, profile.play.gauge_auto_shift, cycle_gauge_auto_shift)
                .map(|next| profile.play.gauge_auto_shift = next)
                .is_some()
        }
        SettingsEntryId::BottomShiftableGauge => {
            cycle_enum(delta, profile.play.bottom_shiftable_gauge, cycle_bottom_shiftable_gauge)
                .map(|next| profile.play.bottom_shiftable_gauge = next)
                .is_some()
        }
        SettingsEntryId::Random => cycle_enum(delta, profile.play.random, cycle_random)
            .map(|next| profile.play.random = next)
            .is_some(),
        SettingsEntryId::Random2 => cycle_enum(delta, profile.play.random2, cycle_random)
            .map(|next| profile.play.random2 = next)
            .is_some(),
        SettingsEntryId::DoubleOption => {
            cycle_enum(delta, profile.play.double_option, cycle_double_option)
                .map(|next| profile.play.double_option = next)
                .is_some()
        }
        SettingsEntryId::HsFix => cycle_enum(delta, profile.play.hs_fix, cycle_hs_fix)
            .map(|next| profile.play.hs_fix = next)
            .is_some(),
        SettingsEntryId::Target => cycle_enum(delta, profile.play.target, cycle_target)
            .map(|next| profile.play.target = next)
            .is_some(),
        SettingsEntryId::GradeDiffDisplay => {
            cycle_enum(delta, profile.play.grade_diff_display, cycle_grade_diff_display)
                .map(|next| profile.play.grade_diff_display = next)
                .is_some()
        }
        SettingsEntryId::LaneEffect => {
            cycle_enum(delta, profile.play.lane_effect, cycle_lane_effect)
                .map(|next| profile.play.lane_effect = next)
                .is_some()
        }
        SettingsEntryId::Assist => cycle_enum(delta, profile.play.assist, cycle_assist)
            .map(|next| profile.play.assist = next)
            .is_some(),
        SettingsEntryId::BgaMode => cycle_enum(delta, profile.play.bga, cycle_bga_mode)
            .map(|next| profile.play.bga = next)
            .is_some(),
        SettingsEntryId::BgaExpand => cycle_enum(delta, profile.play.bga_expand, cycle_bga_expand)
            .map(|next| profile.play.bga_expand = next)
            .is_some(),
        SettingsEntryId::AutoPlay => {
            if delta == 0 {
                false
            } else {
                profile.play.auto_play = !profile.play.auto_play;
                true
            }
        }
        SettingsEntryId::MisslayerDurationMs => {
            adjust_u32(&mut profile.play.misslayer_duration_ms, delta, 0, 5000)
        }
        SettingsEntryId::ShowLnTailCap => {
            if delta == 0 {
                false
            } else {
                profile.play.show_ln_tail_cap = !profile.play.show_ln_tail_cap;
                true
            }
        }
        SettingsEntryId::Hispeed => {
            let (step, default) = match profile.lane.hispeed_mode {
                HispeedModeConfig::Normal => {
                    (profile.lane.hispeed_step_nhs, default_hispeed_step_nhs())
                }
                HispeedModeConfig::Floating => {
                    (profile.lane.hispeed_step_fhs, default_hispeed_step_fhs())
                }
            };
            adjust_hispeed(&mut profile.lane.hispeed, delta, step, default)
        }
        SettingsEntryId::HispeedMode => {
            cycle_enum(delta, profile.lane.hispeed_mode, cycle_hispeed_mode)
                .map(|next| profile.lane.hispeed_mode = next)
                .is_some()
        }
        SettingsEntryId::HispeedStepNhs => {
            adjust_hispeed_step(&mut profile.lane.hispeed_step_nhs, delta)
        }
        SettingsEntryId::HispeedStepFhs => {
            adjust_hispeed_step(&mut profile.lane.hispeed_step_fhs, delta)
        }
        SettingsEntryId::Sudden => adjust_u32(
            &mut profile.lane.sudden,
            delta,
            0,
            crate::config::play::lane_unit_max_for_other(profile.lane.lift),
        ),
        SettingsEntryId::Lift => adjust_u32(
            &mut profile.lane.lift,
            delta,
            0,
            crate::config::play::lane_unit_max_for_other(profile.lane.sudden),
        ),
        SettingsEntryId::Hidden => adjust_u32(&mut profile.lane.hidden, delta, 0, 1000),
        SettingsEntryId::TargetGreenNumber => adjust_u32(
            &mut profile.lane.target_green_number,
            delta,
            TARGET_GREEN_NUMBER_MIN,
            TARGET_GREEN_NUMBER_MAX,
        ),
        SettingsEntryId::SelectInputMode => {
            cycle_enum(delta, profile.input.select_input_mode, cycle_select_input_mode)
                .map(|next| profile.input.select_input_mode = next)
                .is_some()
        }
        SettingsEntryId::AnalogScratch1P => {
            profile.input.gamepad1.analog_scratch = !profile.input.gamepad1.analog_scratch;
            true
        }
        SettingsEntryId::AnalogScratchSensitivity1P => adjust_f32_tenths(
            &mut profile.input.gamepad1.analog_scratch_sensitivity,
            delta,
            0.1,
            5.0,
        ),
        SettingsEntryId::AnalogScratchThreshold1P => {
            adjust_u32(&mut profile.input.gamepad1.analog_scratch_threshold, delta, 1, 1000)
        }
        SettingsEntryId::AnalogScratch2P => {
            profile.input.gamepad2.analog_scratch = !profile.input.gamepad2.analog_scratch;
            true
        }
        SettingsEntryId::AnalogScratchSensitivity2P => adjust_f32_tenths(
            &mut profile.input.gamepad2.analog_scratch_sensitivity,
            delta,
            0.1,
            5.0,
        ),
        SettingsEntryId::AnalogScratchThreshold2P => {
            adjust_u32(&mut profile.input.gamepad2.analog_scratch_threshold, delta, 1, 1000)
        }
        SettingsEntryId::AnalogTicksPerScroll => {
            adjust_u32(&mut profile.input.analog_ticks_per_scroll, delta, 1, 100)
        }
        SettingsEntryId::KeyboardReleaseBounceMs => adjust_u32(
            &mut profile.input.keyboard_release_bounce_ms,
            delta,
            0,
            RELEASE_BOUNCE_MS_MAX,
        ),
        SettingsEntryId::ControllerReleaseBounceMs => adjust_u32(
            &mut profile.input.controller_release_bounce_ms,
            delta,
            0,
            RELEASE_BOUNCE_MS_MAX,
        ),
        id @ (SettingsEntryId::Hispeed8Key1
        | SettingsEntryId::Hispeed8Key2
        | SettingsEntryId::Hispeed8Key3
        | SettingsEntryId::Hispeed8Key4
        | SettingsEntryId::Hispeed8Key5
        | SettingsEntryId::Hispeed8Key6
        | SettingsEntryId::Hispeed8Key7
        | SettingsEntryId::Hispeed8Key8) => {
            let lane = eight_key_hispeed_lane(id).expect("guarded 8K hispeed setting");
            let current = crate::config::play_input::hispeed_direction_for_lane(
                &profile.input,
                KeyMode::K8,
                crate::config::play::lane_from_config(lane),
            )
            .expect("8K key lane has a hispeed direction");
            let next = match current {
                HispeedDirectionConfig::Down => HispeedDirectionConfig::Up,
                HispeedDirectionConfig::Up => HispeedDirectionConfig::Down,
            };
            crate::config::play_input::set_eight_key_hispeed_direction(
                &mut profile.input,
                lane,
                next,
            )
        }
        SettingsEntryId::SelectRandomSelect => {
            if delta == 0 {
                false
            } else {
                profile.select.random_select = !profile.select.random_select;
                true
            }
        }
        SettingsEntryId::RandomMixTargetLevel => {
            adjust_u32(&mut profile.select.random_mix.target_level, delta, 0, 99)
        }
        SettingsEntryId::RandomMixMaxLevel => {
            adjust_u32(&mut profile.select.random_mix.max_level, delta, 0, 99)
        }
        SettingsEntryId::RandomMixMinLevel => {
            adjust_u32(&mut profile.select.random_mix.min_level, delta, 0, 99)
        }
        SettingsEntryId::RandomMixBpmRange => {
            adjust_u32(&mut profile.select.random_mix.bpm_range, delta, 0, 99)
        }
        SettingsEntryId::RandomMixMaxBpm => {
            adjust_u32(&mut profile.select.random_mix.max_bpm, delta, 0, 990)
        }
        SettingsEntryId::RandomMixMinBpm => {
            adjust_u32(&mut profile.select.random_mix.min_bpm, delta, 0, 990)
        }
        SettingsEntryId::RandomMixStages => {
            adjust_u32(&mut profile.select.random_mix.stages, delta, 0, 5)
        }
        SettingsEntryId::ReplayAutoSave => {
            if delta == 0 {
                false
            } else {
                profile.replay.auto_save = !profile.replay.auto_save;
                true
            }
        }
        SettingsEntryId::ReplaySlot1Rule => {
            adjust_replay_slot_rule(&mut profile.replay.slot_rules[0], delta)
        }
        SettingsEntryId::ReplaySlot2Rule => {
            adjust_replay_slot_rule(&mut profile.replay.slot_rules[1], delta)
        }
        SettingsEntryId::ReplaySlot3Rule => {
            adjust_replay_slot_rule(&mut profile.replay.slot_rules[2], delta)
        }
        SettingsEntryId::ReplaySlot4Rule => {
            adjust_replay_slot_rule(&mut profile.replay.slot_rules[3], delta)
        }
    }
}

pub const fn eight_key_hispeed_lane(id: SettingsEntryId) -> Option<LaneConfig> {
    match id {
        SettingsEntryId::Hispeed8Key1 => Some(LaneConfig::Key1),
        SettingsEntryId::Hispeed8Key2 => Some(LaneConfig::Key2),
        SettingsEntryId::Hispeed8Key3 => Some(LaneConfig::Key3),
        SettingsEntryId::Hispeed8Key4 => Some(LaneConfig::Key4),
        SettingsEntryId::Hispeed8Key5 => Some(LaneConfig::Key5),
        SettingsEntryId::Hispeed8Key6 => Some(LaneConfig::Key6),
        SettingsEntryId::Hispeed8Key7 => Some(LaneConfig::Key7),
        SettingsEntryId::Hispeed8Key8 => Some(LaneConfig::Key8),
        _ => None,
    }
}
