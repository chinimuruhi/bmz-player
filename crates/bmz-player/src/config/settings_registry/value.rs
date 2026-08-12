use super::*;

pub fn settings_adjust_step(id: SettingsEntryId) -> i32 {
    match id {
        SettingsEntryId::InputOffsetMs | SettingsEntryId::VisualOffsetMs => 1,
        SettingsEntryId::Sudden | SettingsEntryId::Lift | SettingsEntryId::Hidden => 25,
        SettingsEntryId::TargetGreenNumber => 10,
        SettingsEntryId::MisslayerDurationMs => 50,
        SettingsEntryId::AnalogScratchThreshold1P | SettingsEntryId::AnalogScratchThreshold2P => 10,
        SettingsEntryId::RandomMixMaxBpm | SettingsEntryId::RandomMixMinBpm => 10,
        _ => 1,
    }
}

pub fn format_settings_value(profile: &ProfileConfig, id: SettingsEntryId) -> String {
    match id {
        SettingsEntryId::NormalizeChartVolume => {
            format_bool_on_off(profile.audio_mix.normalize_chart_volume)
        }
        SettingsEntryId::MasterVolume => format!("{}", profile.audio_mix.master_volume),
        SettingsEntryId::KeyVolume => format!("{}", profile.audio_mix.key_volume),
        SettingsEntryId::BgmVolume => format!("{}", profile.audio_mix.bgm_volume),
        SettingsEntryId::PreviewVolume => format!("{}", profile.audio_mix.preview_volume),
        SettingsEntryId::SystemBgmVolume => format!("{}", profile.audio_mix.system_bgm_volume),
        SettingsEntryId::SystemSeVolume => format!("{}", profile.audio_mix.system_se_volume),
        SettingsEntryId::InputOffsetMs => {
            format!("{} ms", profile.judge.input_offset_us / 1_000)
        }
        SettingsEntryId::VisualOffsetMs => {
            format!("{} ms", profile.judge.visual_offset_us / 1_000)
        }
        SettingsEntryId::VisualOffsetAutoAdjust => {
            format_bool_on_off(profile.judge.visual_offset_auto_adjust)
        }
        SettingsEntryId::JudgeAlgorithm => format_judge_algorithm(profile.judge.judge_algorithm),
        SettingsEntryId::RuleMode => format_rule_mode(profile.play.rule_mode),
        SettingsEntryId::LnModePolicy => profile.play.ln_mode_policy.display_label().to_string(),
        SettingsEntryId::Gauge => format_gauge(profile.play.gauge),
        SettingsEntryId::GaugeAutoShift => format_gauge_auto_shift(profile.play.gauge_auto_shift),
        SettingsEntryId::BottomShiftableGauge => {
            format_bottom_shiftable_gauge(profile.play.bottom_shiftable_gauge)
        }
        SettingsEntryId::Random => format_random(profile.play.random),
        SettingsEntryId::Random2 => format_random(profile.play.random2),
        SettingsEntryId::DoubleOption => format_double_option(profile.play.double_option),
        SettingsEntryId::HsFix => format_hs_fix(profile.play.hs_fix),
        SettingsEntryId::Target => format_target(profile.play.target),
        SettingsEntryId::GradeDiffDisplay => {
            format_grade_diff_display(profile.play.grade_diff_display)
        }
        SettingsEntryId::LaneEffect => format_lane_effect(profile.play.lane_effect),
        SettingsEntryId::Assist => format_assist(profile.play.assist),
        SettingsEntryId::BgaMode => format_bga_mode(profile.play.bga),
        SettingsEntryId::BgaExpand => format_bga_expand(profile.play.bga_expand),
        SettingsEntryId::AutoPlay => format_bool_on_off(profile.play.auto_play),
        SettingsEntryId::ShowLnTailCap => format_bool_on_off(profile.play.show_ln_tail_cap),
        SettingsEntryId::MisslayerDurationMs => {
            format!("{} ms", profile.play.misslayer_duration_ms)
        }
        SettingsEntryId::Hispeed => format!("{:.2}", profile.lane.hispeed),
        SettingsEntryId::HispeedMode => format_hispeed_mode(profile.lane.hispeed_mode),
        SettingsEntryId::HispeedStepNhs => format!("{:.2}", profile.lane.hispeed_step_nhs),
        SettingsEntryId::HispeedStepFhs => format!("{:.2}", profile.lane.hispeed_step_fhs),
        SettingsEntryId::Sudden => format_lane_unit(profile.lane.sudden),
        SettingsEntryId::Lift => format_lane_unit(profile.lane.lift),
        SettingsEntryId::Hidden => format_lane_unit(profile.lane.hidden),
        SettingsEntryId::TargetGreenNumber => format!("{}", profile.lane.target_green_number),
        SettingsEntryId::SelectInputMode => {
            profile.input.select_input_mode.display_label().to_string()
        }
        SettingsEntryId::AnalogScratch1P => {
            format_bool_on_off(profile.input.gamepad1.analog_scratch)
        }
        SettingsEntryId::AnalogScratchSensitivity1P => {
            format!("{:.1}", profile.input.gamepad1.analog_scratch_sensitivity)
        }
        SettingsEntryId::AnalogScratchThreshold1P => {
            format!("{} ticks", profile.input.gamepad1.analog_scratch_threshold)
        }
        SettingsEntryId::AnalogScratch2P => {
            format_bool_on_off(profile.input.gamepad2.analog_scratch)
        }
        SettingsEntryId::AnalogScratchSensitivity2P => {
            format!("{:.1}", profile.input.gamepad2.analog_scratch_sensitivity)
        }
        SettingsEntryId::AnalogScratchThreshold2P => {
            format!("{} ticks", profile.input.gamepad2.analog_scratch_threshold)
        }
        SettingsEntryId::AnalogTicksPerScroll => {
            format!("{} ticks", profile.input.analog_ticks_per_scroll)
        }
        SettingsEntryId::KeyboardReleaseBounceMs => {
            format!("{} ms", profile.input.keyboard_release_bounce_ms)
        }
        SettingsEntryId::ControllerReleaseBounceMs => {
            format!("{} ms", profile.input.controller_release_bounce_ms)
        }
        id @ (SettingsEntryId::Hispeed8Key1
        | SettingsEntryId::Hispeed8Key2
        | SettingsEntryId::Hispeed8Key3
        | SettingsEntryId::Hispeed8Key4
        | SettingsEntryId::Hispeed8Key5
        | SettingsEntryId::Hispeed8Key6
        | SettingsEntryId::Hispeed8Key7
        | SettingsEntryId::Hispeed8Key8) => {
            let lane = eight_key_hispeed_lane(id).expect("guarded 8K hispeed setting");
            format_hispeed_direction(
                crate::config::play_input::hispeed_direction_for_lane(
                    &profile.input,
                    KeyMode::K8,
                    crate::config::play::lane_from_config(lane),
                )
                .expect("8K key lane has a hispeed direction"),
            )
        }
        SettingsEntryId::SelectRandomSelect => format_bool_on_off(profile.select.random_select),
        SettingsEntryId::RandomMixTargetLevel => {
            format_random_mix_level(profile.select.random_mix.target_level, "OFF")
        }
        SettingsEntryId::RandomMixMaxLevel => {
            format_random_mix_level(profile.select.random_mix.max_level, "NO LIMIT")
        }
        SettingsEntryId::RandomMixMinLevel => {
            format_random_mix_level(profile.select.random_mix.min_level, "NO LIMIT")
        }
        SettingsEntryId::RandomMixBpmRange => {
            let value = profile.select.random_mix.bpm_range;
            if value == 0 { "NO LIMIT".to_string() } else { format!("+- {value} BPM") }
        }
        SettingsEntryId::RandomMixMaxBpm => {
            format_random_mix_bpm(profile.select.random_mix.max_bpm)
        }
        SettingsEntryId::RandomMixMinBpm => {
            format_random_mix_bpm(profile.select.random_mix.min_bpm)
        }
        SettingsEntryId::RandomMixStages => {
            let value = profile.select.random_mix.stages;
            if value == 0 { "RANDOM".to_string() } else { format!("{value} STAGE") }
        }
        SettingsEntryId::ReplayAutoSave => format_bool_on_off(profile.replay.auto_save),
        SettingsEntryId::ReplaySlot1Rule => format_replay_slot_rule(profile.replay.slot_rules[0]),
        SettingsEntryId::ReplaySlot2Rule => format_replay_slot_rule(profile.replay.slot_rules[1]),
        SettingsEntryId::ReplaySlot3Rule => format_replay_slot_rule(profile.replay.slot_rules[2]),
        SettingsEntryId::ReplaySlot4Rule => format_replay_slot_rule(profile.replay.slot_rules[3]),
    }
}

fn format_random_mix_level(value: u32, zero: &str) -> String {
    if value == 0 { zero.to_string() } else { format!("LEVEL {value}") }
}

fn format_random_mix_bpm(value: u32) -> String {
    if value == 0 { "NO LIMIT".to_string() } else { format!("{value} BPM") }
}
