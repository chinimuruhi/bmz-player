use super::*;

pub(in crate::ui::profile_panel) fn build_profile_play_section(
    ui: &mut egui::Ui,
    section: &mut ProfileSectionContext<'_>,
) {
    let profile = &mut *section.profile;
    let unrestricted = section.unrestricted;
    let text = section.text;
    egui::CollapsingHeader::new(tr!(text, "profile-play-title")).id_salt("profile_play").show(
        ui,
        |ui| {
            if !unrestricted {
                ui.disable();
            }
            egui::ComboBox::new("profile_rule", tr!(text, "profile-play-rule"))
                .selected_text(rule_mode_label(profile.play.rule_mode))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut profile.play.rule_mode,
                        RuleMode::Beatoraja,
                        rule_mode_label(RuleMode::Beatoraja),
                    );
                    ui.selectable_value(
                        &mut profile.play.rule_mode,
                        RuleMode::Lr2Oraja,
                        rule_mode_label(RuleMode::Lr2Oraja),
                    );
                    ui.selectable_value(
                        &mut profile.play.rule_mode,
                        RuleMode::Dx,
                        rule_mode_label(RuleMode::Dx),
                    );
                });
            egui::ComboBox::new("profile_ln_mode", tr!(text, "profile-play-ln-mode"))
                .selected_text(profile.play.ln_mode_policy.display_label())
                .show_ui(ui, |ui| {
                    for value in LnPolicySetting::ORDER {
                        ui.selectable_value(
                            &mut profile.play.ln_mode_policy,
                            value,
                            value.display_label(),
                        );
                    }
                });
            egui::ComboBox::new("profile_gauge", tr!(text, "profile-play-gauge"))
                .selected_text(gauge_label(profile.play.gauge))
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        (GaugeTypeConfig::AssistEasy, "ASSIST EASY"),
                        (GaugeTypeConfig::Easy, "EASY"),
                        (GaugeTypeConfig::Normal, "NORMAL"),
                        (GaugeTypeConfig::Hard, "HARD"),
                        (GaugeTypeConfig::ExHard, "EX HARD"),
                        (GaugeTypeConfig::Hazard, "HAZARD"),
                        (GaugeTypeConfig::AutoShift, "AUTO SHIFT"),
                    ] {
                        ui.selectable_value(&mut profile.play.gauge, value, label);
                    }
                });
            egui::ComboBox::new(
                "profile_gauge_auto_shift",
                tr!(text, "profile-play-gauge-auto-shift"),
            )
            .selected_text(gauge_auto_shift_label(profile.play.gauge_auto_shift))
            .show_ui(ui, |ui| {
                for (value, label) in [
                    (GaugeAutoShiftConfig::Off, "OFF"),
                    (GaugeAutoShiftConfig::Continue, "CONTINUE"),
                    (GaugeAutoShiftConfig::HardToGroove, "HARD->GROOVE"),
                    (GaugeAutoShiftConfig::BestClear, "BEST CLEAR"),
                    (GaugeAutoShiftConfig::SelectToUnder, "SELECT UNDER"),
                ] {
                    ui.selectable_value(&mut profile.play.gauge_auto_shift, value, label);
                }
            });
            egui::ComboBox::new("profile_gas_floor", tr!(text, "profile-play-gas-floor"))
                .selected_text(bottom_shiftable_gauge_label(profile.play.bottom_shiftable_gauge))
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        (BottomShiftableGaugeConfig::AssistEasy, "ASSIST EASY"),
                        (BottomShiftableGaugeConfig::Easy, "EASY"),
                        (BottomShiftableGaugeConfig::Normal, "NORMAL"),
                    ] {
                        ui.selectable_value(&mut profile.play.bottom_shiftable_gauge, value, label);
                    }
                });
            egui::ComboBox::new("profile_random", tr!(text, "profile-play-random"))
                .selected_text(random_label(profile.play.random))
                .show_ui(ui, |ui| {
                    for (value, label) in random_options() {
                        ui.selectable_value(&mut profile.play.random, value, label);
                    }
                });
            egui::ComboBox::new("profile_random_2p", tr!(text, "profile-play-random-2p"))
                .selected_text(random_label(profile.play.random2))
                .show_ui(ui, |ui| {
                    for (value, label) in random_options() {
                        ui.selectable_value(&mut profile.play.random2, value, label);
                    }
                });
            egui::ComboBox::new("profile_dp_option", tr!(text, "profile-play-dp-option"))
                .selected_text(double_option_label(profile.play.double_option))
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        (DoubleOptionConfig::Off, "OFF"),
                        (DoubleOptionConfig::Flip, "FLIP"),
                        (DoubleOptionConfig::Battle, "BATTLE"),
                        (DoubleOptionConfig::BattleAutoScratch, "BATTLE AS"),
                    ] {
                        ui.selectable_value(&mut profile.play.double_option, value, label);
                    }
                });
            egui::ComboBox::from_label("HS-FIX")
                .selected_text(hs_fix_label(profile.play.hs_fix))
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        (HsFixConfig::Off, "OFF"),
                        (HsFixConfig::StartBpm, "START BPM"),
                        (HsFixConfig::MaxBpm, "MAX BPM"),
                        (HsFixConfig::MainBpm, "MAIN BPM"),
                        (HsFixConfig::MinBpm, "MIN BPM"),
                    ] {
                        ui.selectable_value(&mut profile.play.hs_fix, value, label);
                    }
                });
            egui::ComboBox::new("profile_target", tr!(text, "profile-play-target"))
                .selected_text(target_label(profile.play.target))
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        (TargetOptionConfig::None, "NONE"),
                        (TargetOptionConfig::RankA, "RANK_A"),
                        (TargetOptionConfig::RankAaMinus, "RANK_AA-"),
                        (TargetOptionConfig::RankAa, "RANK_AA"),
                        (TargetOptionConfig::RankAaaMinus, "RANK_AAA-"),
                        (TargetOptionConfig::RankAaa, "RANK_AAA"),
                        (TargetOptionConfig::RankMaxMinus, "RANK_MAX-"),
                        (TargetOptionConfig::Max, "MAX"),
                        (TargetOptionConfig::RankNext, "RANK_NEXT"),
                        (TargetOptionConfig::IrTop, "IR_TOP"),
                        (TargetOptionConfig::IrNext, "IR_NEXT"),
                        (TargetOptionConfig::RivalTop, "RIVAL TOP"),
                        (TargetOptionConfig::RivalNext, "RIVAL NEXT"),
                    ] {
                        ui.selectable_value(&mut profile.play.target, value, label);
                    }
                });
            egui::ComboBox::new("profile_result_diff", tr!(text, "profile-play-result-diff"))
                .selected_text(grade_diff_display_label(profile.play.grade_diff_display))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut profile.play.grade_diff_display,
                        ResultGradeDiffDisplay::Next,
                        grade_diff_display_label(ResultGradeDiffDisplay::Next),
                    );
                    ui.selectable_value(
                        &mut profile.play.grade_diff_display,
                        ResultGradeDiffDisplay::Nearest,
                        grade_diff_display_label(ResultGradeDiffDisplay::Nearest),
                    );
                });
            egui::ComboBox::new("profile_lane_effect", tr!(text, "profile-play-lane-effect"))
                .selected_text(lane_effect_label(profile.play.lane_effect))
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        (LaneEffectConfig::Off, "OFF"),
                        (LaneEffectConfig::Hidden, "HIDDEN"),
                        (LaneEffectConfig::Sudden, "SUDDEN"),
                        (LaneEffectConfig::HiddenSudden, "HIDDEN+SUDDEN"),
                    ] {
                        ui.selectable_value(&mut profile.play.lane_effect, value, label);
                    }
                });
            egui::ComboBox::from_label("BGA")
                .selected_text(bga_mode_label(profile.play.bga))
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        (BgaModeConfig::On, "ON"),
                        (BgaModeConfig::Auto, "AUTO"),
                        (BgaModeConfig::Off, "OFF"),
                    ] {
                        ui.selectable_value(&mut profile.play.bga, value, label);
                    }
                });
            egui::ComboBox::new("profile_bga_expand", tr!(text, "profile-play-bga-display"))
                .selected_text(bga_expand_label(profile.play.bga_expand))
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        (BgaExpandConfig::KeepAspect, "KEEP ASPECT"),
                        (BgaExpandConfig::Full, "FULL"),
                        (BgaExpandConfig::Off, "OFF"),
                    ] {
                        ui.selectable_value(&mut profile.play.bga_expand, value, label);
                    }
                });
            let mut session_mode = profile.play.session_mode.unwrap_or(if profile.play.auto_play {
                SessionMode::Autoplay
            } else {
                SessionMode::Normal
            });
            egui::ComboBox::new("profile_session_mode", tr!(text, "profile-play-session-mode"))
                .selected_text(session_mode.as_str())
                .show_ui(ui, |ui| {
                    for value in SessionMode::VALUES {
                        ui.selectable_value(&mut session_mode, value, value.as_str());
                    }
                });
            profile.play.session_mode = Some(session_mode);
            profile.play.auto_play = session_mode.primary_autoplay();
            ui.checkbox(&mut profile.play.show_ln_tail_cap, tr!(text, "profile-play-ln-tail-cap"));
            ui.add(
                egui::Slider::new(&mut profile.play.misslayer_duration_ms, 0..=5000)
                    .text(tr!(text, "profile-play-miss-layer-duration")),
            );
            ui.add(
                egui::Slider::new(&mut profile.play.play_exit_hold_ms, 100..=5000)
                    .text(tr!(text, "profile-play-exit-hold-duration")),
            );
        },
    );
}
