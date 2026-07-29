#[allow(clippy::too_many_arguments)]
pub(in crate::ui) fn build_profile_settings_panel(
    ctx: &egui::Context,
    open: &mut bool,
    profile: &mut ProfileConfig,
    app_config: &mut AppConfig,
    show_fps: &mut bool,
    ir_login: &mut IrLoginUiState,
    ir_device_key: &mut IrDeviceKeyUiState,
    profile_manager: &mut ProfileManagerUiState,
    profile_root: &std::path::Path,
    unrestricted: bool,
    mut text: Localizer,
) -> ProfileSettingsPanelActions {
    let mut save_clicked = false;
    let mut save_app_config = false;
    // ログインタスクの完了を反映。provider 設定が更新されたら保存する。
    save_clicked |= ir_login.poll(profile, text);
    ir_device_key.poll(text);
    let readonly_profile = (!unrestricted).then(|| profile.clone());
    let readonly_app_config = (!unrestricted).then(|| app_config.clone());
    localized_sized_panel_window(
        "profile_settings_panel",
        tr!(text, "profile-settings-title"),
        ctx,
        open,
        460.0,
        560.0,
        egui::pos2(476.0, 320.0),
    )
    .show(ctx, |ui| {
        scrollable_window_content(ui, |ui| {
            if !unrestricted {
                ui.label(tr!(text, "profile-settings-restricted"));
                ui.separator();
            }
            egui::CollapsingHeader::new(tr!(text, "profile-basic-title"))
                .id_salt("profile_basic")
                .default_open(true)
                .show(ui, |ui| {
                    if !unrestricted {
                        ui.disable();
                    }
                    ui.horizontal(|ui| {
                        ui.label(tr!(text, "profile-display-name"));
                        ui.text_edit_singleline(&mut profile.display_name);
                    });
                    ui.horizontal(|ui| {
                        ui.label("ID");
                        ui.monospace(&profile.id);
                    });
                });

            save_app_config |= build_profile_manager_section(
                ui,
                app_config,
                profile,
                profile_manager,
                unrestricted,
                text,
            );

            egui::CollapsingHeader::new(tr!(text, "profile-volume-title"))
                .id_salt("profile_volume")
                .default_open(true)
                .show(ui, |ui| {
                    ui.checkbox(
                        &mut profile.audio_mix.normalize_chart_volume,
                        tr!(text, "profile-volume-normalize"),
                    );
                    volume_slider(
                        ui,
                        &mut profile.audio_mix.master_volume,
                        &tr!(text, "profile-volume-master"),
                    );
                    volume_slider(
                        ui,
                        &mut profile.audio_mix.key_volume,
                        &tr!(text, "profile-volume-keysound"),
                    );
                    volume_slider(ui, &mut profile.audio_mix.bgm_volume, "BGM");
                    volume_slider(
                        ui,
                        &mut profile.audio_mix.preview_volume,
                        &tr!(text, "profile-volume-preview"),
                    );
                    volume_slider(
                        ui,
                        &mut profile.audio_mix.system_bgm_volume,
                        &tr!(text, "profile-volume-system-bgm"),
                    );
                    volume_slider(
                        ui,
                        &mut profile.audio_mix.system_se_volume,
                        &tr!(text, "profile-volume-system-se"),
                    );
                    ui.label(tr!(text, "profile-volume-help"));
                });

            egui::CollapsingHeader::new(tr!(text, "profile-judge-title"))
                .id_salt("profile_judge")
                .show(ui, |ui| {
                    offset_ms_slider(
                        ui,
                        &mut profile.judge.input_offset_us,
                        &tr!(text, "profile-judge-input-offset"),
                    );
                    offset_ms_slider(
                        ui,
                        &mut profile.judge.visual_offset_us,
                        &tr!(text, "profile-judge-visual-offset"),
                    );
                    ui.checkbox(
                        &mut profile.judge.visual_offset_auto_adjust,
                        tr!(text, "profile-judge-auto-adjust"),
                    );
                    egui::ComboBox::new(
                        "profile_judge_algorithm",
                        tr!(text, "profile-judge-algorithm"),
                    )
                    .selected_text(judge_algorithm_label(profile.judge.judge_algorithm))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut profile.judge.judge_algorithm,
                            JudgeAlgorithmConfig::Combo,
                            judge_algorithm_label(JudgeAlgorithmConfig::Combo),
                        );
                        ui.selectable_value(
                            &mut profile.judge.judge_algorithm,
                            JudgeAlgorithmConfig::Duration,
                            judge_algorithm_label(JudgeAlgorithmConfig::Duration),
                        );
                        ui.selectable_value(
                            &mut profile.judge.judge_algorithm,
                            JudgeAlgorithmConfig::Lowest,
                            judge_algorithm_label(JudgeAlgorithmConfig::Lowest),
                        );
                    });
                    egui::ComboBox::new(
                        "profile_fast_slow_scope",
                        tr!(text, "profile-fast-slow-mode"),
                    )
                    .selected_text(fast_slow_scope_label(
                        text,
                        profile.judge.fast_slow_display_scope,
                    ))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut profile.judge.fast_slow_display_scope,
                            FastSlowDisplayScope::Auto,
                            fast_slow_scope_label(text, FastSlowDisplayScope::Auto),
                        );
                        ui.selectable_value(
                            &mut profile.judge.fast_slow_display_scope,
                            FastSlowDisplayScope::ThresholdMs,
                            fast_slow_scope_label(text, FastSlowDisplayScope::ThresholdMs),
                        );
                    });
                    if profile.judge.fast_slow_display_scope == FastSlowDisplayScope::ThresholdMs {
                        ui.add(
                            egui::Slider::new(
                                &mut profile.judge.fast_slow_display_threshold_ms,
                                0..=50,
                            )
                            .text(tr!(text, "profile-fast-slow-threshold")),
                        );
                        ui.label(tr!(text, "profile-fast-slow-threshold-help"));
                    }
                });

            egui::CollapsingHeader::new(tr!(text, "profile-play-title"))
                .id_salt("profile_play")
                .show(ui, |ui| {
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
                        .selected_text(bottom_shiftable_gauge_label(
                            profile.play.bottom_shiftable_gauge,
                        ))
                        .show_ui(ui, |ui| {
                            for (value, label) in [
                                (BottomShiftableGaugeConfig::AssistEasy, "ASSIST EASY"),
                                (BottomShiftableGaugeConfig::Easy, "EASY"),
                                (BottomShiftableGaugeConfig::Normal, "NORMAL"),
                            ] {
                                ui.selectable_value(
                                    &mut profile.play.bottom_shiftable_gauge,
                                    value,
                                    label,
                                );
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
                    egui::ComboBox::new(
                        "profile_result_diff",
                        tr!(text, "profile-play-result-diff"),
                    )
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
                    egui::ComboBox::new(
                        "profile_lane_effect",
                        tr!(text, "profile-play-lane-effect"),
                    )
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
                    egui::ComboBox::new(
                        "profile_bga_expand",
                        tr!(text, "profile-play-bga-display"),
                    )
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
                    let mut session_mode =
                        profile.play.session_mode.unwrap_or(if profile.play.auto_play {
                            SessionMode::Autoplay
                        } else {
                            SessionMode::Normal
                        });
                    egui::ComboBox::new(
                        "profile_session_mode",
                        tr!(text, "profile-play-session-mode"),
                    )
                    .selected_text(session_mode.as_str())
                    .show_ui(ui, |ui| {
                        for value in SessionMode::VALUES {
                            ui.selectable_value(&mut session_mode, value, value.as_str());
                        }
                    });
                    profile.play.session_mode = Some(session_mode);
                    profile.play.auto_play = session_mode.primary_autoplay();
                    ui.checkbox(
                        &mut profile.play.show_ln_tail_cap,
                        tr!(text, "profile-play-ln-tail-cap"),
                    );
                    ui.add(
                        egui::Slider::new(&mut profile.play.misslayer_duration_ms, 0..=5000)
                            .text(tr!(text, "profile-play-miss-layer-duration")),
                    );
                    ui.add(
                        egui::Slider::new(&mut profile.play.play_exit_hold_ms, 100..=5000)
                            .text(tr!(text, "profile-play-exit-hold-duration")),
                    );
                });

            egui::CollapsingHeader::new(tr!(text, "profile-display-title"))
                .id_salt("profile_display")
                .show(ui, |ui| {
                    let hispeed_step = match profile.lane.hispeed_mode {
                        HispeedModeConfig::Normal => normalize_hispeed_step(
                            profile.lane.hispeed_step_nhs,
                            default_hispeed_step_nhs(),
                        ),
                        HispeedModeConfig::Floating => normalize_hispeed_step(
                            profile.lane.hispeed_step_fhs,
                            default_hispeed_step_fhs(),
                        ),
                    };
                    ui.add(
                        egui::Slider::new(&mut profile.lane.hispeed, 0.5..=10.0)
                            .step_by(hispeed_step as f64)
                            .text(tr!(text, "profile-display-hispeed")),
                    );
                    egui::ComboBox::new(
                        "profile_hispeed_mode",
                        tr!(text, "profile-display-hispeed-mode"),
                    )
                    .selected_text(hispeed_mode_label(profile.lane.hispeed_mode))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut profile.lane.hispeed_mode,
                            HispeedModeConfig::Normal,
                            hispeed_mode_label(HispeedModeConfig::Normal),
                        );
                        ui.selectable_value(
                            &mut profile.lane.hispeed_mode,
                            HispeedModeConfig::Floating,
                            hispeed_mode_label(HispeedModeConfig::Floating),
                        );
                    });
                    ui.add(
                        egui::Slider::new(
                            &mut profile.lane.hispeed_step_nhs,
                            HISPEED_STEP_MIN..=HISPEED_STEP_MAX,
                        )
                        .step_by(0.05)
                        .text(tr!(text, "profile-display-nhs-step")),
                    );
                    ui.add(
                        egui::Slider::new(
                            &mut profile.lane.hispeed_step_fhs,
                            HISPEED_STEP_MIN..=HISPEED_STEP_MAX,
                        )
                        .step_by(0.05)
                        .text(tr!(text, "profile-display-fhs-step")),
                    );
                    ui.label(tr!(text, "profile-display-step-range"));
                    let sudden_max =
                        crate::config::play::lane_unit_max_for_other(profile.lane.lift);
                    lane_unit_slider_with_max(ui, &mut profile.lane.sudden, "SUDDEN+", sudden_max);
                    let lift_max =
                        crate::config::play::lane_unit_max_for_other(profile.lane.sudden);
                    ui.checkbox(
                        &mut profile.lane.lift_enabled,
                        tr!(text, "profile-display-lift-enabled"),
                    );
                    lane_unit_slider_with_max(ui, &mut profile.lane.lift, "LIFT", lift_max);
                    ui.checkbox(
                        &mut profile.lane.hispeed_auto_adjust,
                        tr!(text, "profile-display-auto-adjust-hispeed"),
                    );
                    lane_unit_slider(ui, &mut profile.lane.hidden, "HIDDEN");
                    ui.add(
                        egui::Slider::new(
                            &mut profile.lane.target_green_number,
                            TARGET_GREEN_NUMBER_MIN..=TARGET_GREEN_NUMBER_MAX,
                        )
                        .text(tr!(text, "profile-display-green-number")),
                    );
                });

            egui::CollapsingHeader::new(tr!(text, "profile-input-title"))
                .id_salt("profile_input")
                .show(ui, |ui| {
                    ui.add(
                        egui::Slider::new(&mut profile.input.analog_scratch_sensitivity, 0.1..=5.0)
                            .text(tr!(text, "profile-input-analog-sensitivity")),
                    );
                    ui.add(
                        egui::Slider::new(&mut profile.input.analog_scratch_threshold, 1..=1000)
                            .text(tr!(text, "profile-input-analog-stop-threshold")),
                    );
                    ui.add(
                        egui::Slider::new(
                            &mut profile.input.keyboard_release_bounce_ms,
                            0..=RELEASE_BOUNCE_MS_MAX,
                        )
                        .text(tr!(text, "profile-input-keyboard-release-bounce-ms")),
                    );
                    ui.add(
                        egui::Slider::new(
                            &mut profile.input.controller_release_bounce_ms,
                            0..=RELEASE_BOUNCE_MS_MAX,
                        )
                        .text(tr!(text, "profile-input-controller-release-bounce-ms")),
                    );
                    ui.label(tr!(text, "profile-input-release-bounce-help"));
                    ui.label(tr!(text, "profile-input-key-bindings-help"));
                });

            egui::CollapsingHeader::new(tr!(text, "profile-replay-title"))
                .id_salt("profile_replay")
                .show(ui, |ui| {
                    if !unrestricted {
                        ui.disable();
                    }
                    ui.checkbox(
                        &mut profile.replay.auto_save,
                        tr!(text, "profile-replay-auto-save"),
                    );
                    ui.checkbox(&mut profile.replay.compress, tr!(text, "profile-replay-compress"));
                    for (index, rule) in profile.replay.slot_rules.iter_mut().enumerate() {
                        egui::ComboBox::new(
                            ("profile_replay_slot", index),
                            tr!(text, "profile-replay-slot", "number" => index + 1),
                        )
                        .selected_text(replay_slot_rule_label(*rule))
                        .show_ui(ui, |ui| {
                            for value in [
                                ReplaySlotRule::Disabled,
                                ReplaySlotRule::Always,
                                ReplaySlotRule::ScoreUpdate,
                                ReplaySlotRule::BpUpdate,
                                ReplaySlotRule::MaxComboUpdate,
                                ReplaySlotRule::ClearUpdate,
                            ] {
                                ui.selectable_value(rule, value, replay_slot_rule_label(value));
                            }
                        });
                    }
                });

            egui::CollapsingHeader::new(tr!(text, "profile-system-sound-title"))
                .id_salt("profile_system_sound")
                .show(ui, |ui| {
                    if !unrestricted {
                        ui.disable();
                    }
                    system_sound_path_row(
                        ui,
                        text,
                        &tr!(text, "profile-system-sound-bgm-root"),
                        &mut profile.system_sound.bgm_dir,
                    );
                    system_sound_path_row(
                        ui,
                        text,
                        &tr!(text, "profile-system-sound-se-root"),
                        &mut profile.system_sound.se_dir,
                    );
                    system_sound_path_row(
                        ui,
                        text,
                        &tr!(text, "profile-system-sound-fallback"),
                        &mut profile.system_sound.default_sound_dir,
                    );
                    ui.label(tr!(text, "profile-system-sound-rescan-help"));
                });

            egui::CollapsingHeader::new(tr!(text, "profile-ir-title")).id_salt("profile_ir").show(
                ui,
                |ui| {
                    if !unrestricted {
                        ui.disable();
                    }
                    sync_ir_provider_roles(&mut profile.ir);
                    let primary_options: Vec<_> = profile
                        .ir
                        .providers
                        .iter()
                        .filter_map(|provider| {
                            crate::ir::provider_key::configured_provider_key(provider).map(
                                |provider_key| {
                                    (
                                        provider_key.to_string(),
                                        ir_primary_provider_label(provider, provider_key),
                                    )
                                },
                            )
                        })
                        .collect();
                    let mut selected_primary = profile.ir.primary_provider.clone();
                    let selected_primary_text = primary_options
                        .iter()
                        .find(|(provider_key, _)| provider_key == &profile.ir.primary_provider)
                        .map(|(_, label)| label.clone())
                        .unwrap_or_else(|| {
                            if profile.ir.primary_provider.is_empty() {
                                tr!(text, "profile-ir-unset")
                            } else {
                                profile.ir.primary_provider.clone()
                            }
                        });
                    egui::ComboBox::new("profile_primary_ir", tr!(text, "profile-ir-primary"))
                        .selected_text(selected_primary_text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut selected_primary,
                                String::new(),
                                tr!(text, "profile-ir-unset"),
                            );
                            for (provider_key, label) in &primary_options {
                                ui.selectable_value(
                                    &mut selected_primary,
                                    provider_key.clone(),
                                    label,
                                );
                            }
                        });
                    if selected_primary != profile.ir.primary_provider {
                        profile.ir.primary_provider = selected_primary;
                        sync_ir_provider_roles(&mut profile.ir);
                    }
                    ui.checkbox(
                        &mut profile.ir.prefetch_global_ranking_on_score_submit,
                        tr!(text, "profile-ir-prefetch-global"),
                    );
                    egui::ComboBox::new(
                        "profile_ir_credential_store",
                        tr!(text, "profile-ir-credential-store"),
                    )
                    .selected_text(match profile.ir.credential_store {
                        IrCredentialStoreConfig::File => tr!(text, "profile-ir-credential-file"),
                        IrCredentialStoreConfig::Os => tr!(text, "profile-ir-credential-os"),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut profile.ir.credential_store,
                            IrCredentialStoreConfig::File,
                            tr!(text, "profile-ir-credential-file"),
                        );
                        ui.selectable_value(
                            &mut profile.ir.credential_store,
                            IrCredentialStoreConfig::Os,
                            tr!(text, "profile-ir-credential-os"),
                        );
                    });
                    ui.checkbox(
                        &mut profile.ir.prefetch_rival_ranking_on_score_submit,
                        tr!(text, "profile-ir-prefetch-rival"),
                    );
                    let mut remove_index = None;
                    let mut logged_out_provider_key = None;
                    for (index, provider) in profile.ir.providers.iter_mut().enumerate() {
                        ui.push_id(("ir_provider", index), |ui| {
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut provider.enabled, "");
                                ui.label(tr!(text, "profile-ir-provider", "number" => index + 1));
                                if ui.button(tr!(text, "common-delete")).clicked() {
                                    remove_index = Some(index);
                                }
                            });
                            let provider_key =
                                crate::ir::provider_key::configured_provider_key(provider)
                                    .map(str::to_string);
                            let endpoint_editable = provider_key.is_none() && !ir_login.busy;
                            let mut preset = classify_ir_provider_preset(provider);
                            let previous_preset = preset;
                            ui.add_enabled_ui(endpoint_editable, |ui| {
                                egui::ComboBox::new(
                                    ("profile_ir_provider_preset", index),
                                    tr!(text, "profile-ir-provider-kind"),
                                )
                                .selected_text(ir_provider_preset_label(text, preset))
                                .show_ui(ui, |ui| {
                                    for value in [
                                        IrProviderPreset::BmzIr,
                                        IrProviderPreset::RianIr,
                                        IrProviderPreset::Other,
                                    ] {
                                        ui.selectable_value(
                                            &mut preset,
                                            value,
                                            ir_provider_preset_label(text, value),
                                        );
                                    }
                                });
                            });
                            if preset != previous_preset {
                                apply_ir_provider_preset(provider, preset);
                                if preset == IrProviderPreset::Other {
                                    provider.provider =
                                        crate::ir::bmz_official::BMZ_IR_PROVIDER.to_string();
                                    provider.base_url.clear();
                                }
                            }
                            match preset {
                                IrProviderPreset::BmzIr | IrProviderPreset::RianIr => {
                                    ui.horizontal(|ui| {
                                        ui.label(tr!(text, "profile-ir-base-url"));
                                        ui.add_enabled(
                                            false,
                                            egui::TextEdit::singleline(&mut provider.base_url)
                                                .desired_width(300.0),
                                        );
                                        ui.hyperlink_to(
                                            tr!(text, "profile-ir-open-browser"),
                                            provider.base_url.clone(),
                                        );
                                    });
                                }
                                IrProviderPreset::Other => {
                                    ui.add_enabled_ui(endpoint_editable, |ui| {
                                        let mut family =
                                            ir_provider_family(&provider.provider).to_string();
                                        let previous_family = family.clone();
                                        egui::ComboBox::new(
                                            ("profile_ir_provider_protocol", index),
                                            tr!(text, "profile-ir-provider-protocol"),
                                        )
                                        .selected_text(family.clone())
                                        .show_ui(
                                            ui,
                                            |ui| {
                                                ui.selectable_value(
                                                    &mut family,
                                                    crate::ir::bmz_official::BMZ_IR_PROVIDER
                                                        .to_string(),
                                                    crate::ir::bmz_official::BMZ_IR_PROVIDER,
                                                );
                                                ui.selectable_value(
                                                    &mut family,
                                                    crate::ir::rian_ir::RIAN_IR_PROVIDER
                                                        .to_string(),
                                                    crate::ir::rian_ir::RIAN_IR_PROVIDER,
                                                );
                                            },
                                        );
                                        if family != previous_family {
                                            provider.provider = family;
                                        }
                                        ir_provider_text_row(
                                            ui,
                                            &tr!(text, "profile-ir-base-url"),
                                            &mut provider.base_url,
                                        );
                                    });
                                }
                            }
                            if !endpoint_editable && provider_key.is_some() {
                                ui.small(tr!(text, "profile-ir-logout-to-change"));
                            }
                            let row_target = IrProviderUiTarget::new(
                                provider.provider.clone(),
                                provider.base_url.clone(),
                            );
                            let is_rian = crate::ir::rian_ir::is_rian_ir_config(provider);
                            let provider_key_text = provider_key
                                .clone()
                                .unwrap_or_else(|| tr!(text, "profile-ir-key-after-login"));
                            ui.horizontal(|ui| {
                                ui.label("Key");
                                ui.monospace(&provider_key_text);
                            });
                            ui.horizontal(|ui| {
                                ui.label(if is_rian {
                                    tr!(text, "profile-ir-login-id")
                                } else {
                                    tr!(text, "profile-ir-email")
                                });
                                ui.text_edit_singleline(&mut ir_login.email);
                            });
                            ui.horizontal(|ui| {
                                ui.label(tr!(text, "profile-ir-password"));
                                ui.add(
                                    egui::TextEdit::singleline(&mut ir_login.password)
                                        .password(true),
                                );
                            });
                            ui.horizontal(|ui| {
                                let can_login = !ir_login.busy
                                    && normalized_ir_base_url(&provider.base_url).is_some()
                                    && !ir_login.email.is_empty()
                                    && !ir_login.password.is_empty();
                                if ui
                                    .add_enabled(
                                        can_login,
                                        egui::Button::new(tr!(text, "profile-ir-login")),
                                    )
                                    .clicked()
                                {
                                    ir_login.start_login(
                                        profile_root.to_path_buf(),
                                        provider.provider.clone(),
                                        provider.base_url.clone(),
                                    );
                                }
                                let login_busy =
                                    ir_login.busy_target.as_ref().is_some_and(|target| {
                                        target.matches(&provider.provider, &provider.base_url)
                                    });
                                if login_busy {
                                    ui.spinner();
                                }
                                if ui.button(tr!(text, "profile-ir-logout")).clicked() {
                                    let result = provider_key
                                        .as_deref()
                                        .map(|provider_key| {
                                            crate::ir::credentials::delete_credentials(
                                                profile_root,
                                                provider_key,
                                            )
                                        })
                                        .transpose();
                                    match result {
                                        Ok(_) => {
                                            provider.enabled = false;
                                            logged_out_provider_key = provider_key.clone();
                                            provider.provider_key.clear();
                                            provider.account_id.clear();
                                            provider.account_display_name.clear();
                                            provider.last_login_at = None;
                                            ir_login.message = Some(IrProviderUiMessage {
                                                target: row_target.clone(),
                                                ok: true,
                                                text: tr!(text, "profile-ir-logout-success"),
                                            });
                                            save_clicked = true;
                                        }
                                        Err(error) => {
                                            ir_login.message = Some(IrProviderUiMessage {
                                                target: row_target.clone(),
                                                ok: false,
                                                text: format!("{error:#}"),
                                            });
                                        }
                                    }
                                }
                            });
                            ui.horizontal(|ui| {
                                let busy = ir_device_key.busy_provider.as_deref()
                                    == provider_key.as_deref();
                                let can_rotate = !busy
                                    && !provider.base_url.is_empty()
                                    && provider_key.is_some()
                                    && !is_rian;
                                if ui
                                    .add_enabled(
                                        can_rotate,
                                        egui::Button::new(tr!(
                                            text,
                                            "profile-ir-device-key-rotate"
                                        )),
                                    )
                                    .clicked()
                                {
                                    ir_device_key.start_rotate(
                                        profile_root.to_path_buf(),
                                        provider.provider.clone(),
                                        provider_key.clone().unwrap_or_default(),
                                        provider.base_url.clone(),
                                    );
                                }
                                if busy {
                                    ui.spinner();
                                }
                            });
                            if let Some(message) = &ir_login.message
                                && message.target.matches(&provider.provider, &provider.base_url)
                            {
                                let color = if message.ok {
                                    egui::Color32::LIGHT_GREEN
                                } else {
                                    egui::Color32::LIGHT_RED
                                };
                                ui.colored_label(color, message.text.clone());
                            }
                            if let Some(message) = &ir_device_key.message
                                && message.target.matches(&provider.provider, &provider.base_url)
                            {
                                let color = if message.ok {
                                    egui::Color32::LIGHT_GREEN
                                } else {
                                    egui::Color32::LIGHT_RED
                                };
                                ui.colored_label(color, message.text.clone());
                            }
                            egui::ComboBox::new(
                                ("profile_ir_send_policy", index),
                                tr!(text, "profile-ir-send-policy"),
                            )
                            .selected_text(ir_send_policy_label(provider.send_policy))
                            .show_ui(ui, |ui| {
                                for value in [
                                    IrSendPolicyConfig::UpdateScore,
                                    IrSendPolicyConfig::Always,
                                    IrSendPolicyConfig::CompleteSong,
                                ] {
                                    ui.selectable_value(
                                        &mut provider.send_policy,
                                        value,
                                        ir_send_policy_label(value),
                                    );
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label(tr!(text, "profile-ir-last-login"));
                                ui.monospace(format_optional_timestamp(provider.last_login_at));
                            });
                            ui.horizontal(|ui| {
                                ui.label(tr!(text, "profile-ir-last-success"));
                                ui.monospace(format_optional_timestamp(provider.last_success_at));
                            });
                        });
                    }
                    if logged_out_provider_key
                        .as_deref()
                        .is_some_and(|key| profile.ir.primary_provider == key)
                    {
                        profile.ir.primary_provider.clear();
                        sync_ir_provider_roles(&mut profile.ir);
                    }
                    if let Some(index) = remove_index {
                        profile.ir.providers.remove(index);
                    }
                    if ui.button(tr!(text, "profile-ir-add-provider")).clicked() {
                        profile.ir.providers.push(IrProviderConfig {
                            provider: crate::ir::bmz_official::BMZ_IR_PROVIDER.to_string(),
                            provider_key: String::new(),
                            base_url: crate::ir::bmz_official::BMZ_IR_DEFAULT_BASE_URL.to_string(),
                            enabled: false,
                            account_display_name: String::new(),
                            account_id: String::new(),
                            send_policy: IrSendPolicyConfig::default(),
                            role: IrProviderRoleConfig::default(),
                            last_login_at: None,
                            last_success_at: None,
                        });
                    }
                },
            );

            egui::CollapsingHeader::new(tr!(text, "profile-ui-title")).id_salt("profile_ui").show(
                ui,
                |ui| {
                    if !unrestricted {
                        ui.disable();
                    }
                    let current_locale = profile.ui.locale();
                    let mut selected_locale = current_locale;
                    egui::ComboBox::new("profile_ui_language", tr!(text, "profile-ui-language"))
                        .selected_text(selected_locale.native_name())
                        .show_ui(ui, |ui| {
                            for locale in AppLocale::SUPPORTED {
                                ui.selectable_value(
                                    &mut selected_locale,
                                    locale,
                                    locale.native_name(),
                                );
                            }
                        });
                    if selected_locale != current_locale {
                        profile.ui.set_locale(selected_locale);
                        text = Localizer::new(selected_locale);
                        save_clicked = true;
                    }
                    ui.horizontal(|ui| {
                        ui.label(tr!(text, "profile-ui-theme-unimplemented"));
                        ui.text_edit_singleline(&mut profile.ui.theme);
                    });
                    if ui.checkbox(show_fps, tr!(text, "settings-show-fps")).changed() {
                        profile.ui.show_fps = *show_fps;
                    }
                    ui.checkbox(
                        &mut profile.ui.confirm_on_exit,
                        tr!(text, "profile-ui-confirm-exit-unimplemented"),
                    );
                },
            );

            ui.separator();
            if ui.button(tr!(text, "settings-save")).clicked() {
                save_clicked = true;
            }
        });
    });
    if let Some(readonly) = readonly_profile {
        restore_restricted_profile_settings(profile, readonly);
    }
    if let Some(readonly) = readonly_app_config {
        *app_config = readonly;
        save_app_config = false;
    }
    ProfileSettingsPanelActions { save: save_clicked, save_app_config }
}
use super::*;
