use super::*;

pub(in crate::ui::profile_panel) fn build_profile_display_section(
    ui: &mut egui::Ui,
    section: &mut ProfileSectionContext<'_>,
) {
    let profile = &mut *section.profile;
    let text = section.text;
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
                egui::Slider::new(
                    &mut profile.lane.hispeed,
                    crate::config::play::HISPEED_MIN..=crate::config::play::HISPEED_MAX,
                )
                .step_by(hispeed_step as f64)
                .text(tr!(text, "profile-display-hispeed")),
            );
            egui::ComboBox::new("profile_hispeed_mode", tr!(text, "profile-display-hispeed-mode"))
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
            let sudden_max = crate::config::play::lane_unit_max_for_other(profile.lane.lift);
            lane_unit_slider_with_max(ui, &mut profile.lane.sudden, "SUDDEN+", sudden_max);
            let lift_max = crate::config::play::lane_unit_max_for_other(profile.lane.sudden);
            ui.checkbox(&mut profile.lane.lift_enabled, tr!(text, "profile-display-lift-enabled"));
            lane_unit_slider_with_max(ui, &mut profile.lane.lift, "LIFT", lift_max);
            ui.checkbox(
                &mut profile.lane.hispeed_auto_adjust,
                tr!(text, "profile-display-auto-adjust-hispeed"),
            );
            lane_unit_slider(ui, &mut profile.lane.hidden, "HIDDEN");
            let green_changed = ui
                .add(
                    egui::Slider::new(
                        &mut profile.lane.target_green_number,
                        TARGET_GREEN_NUMBER_MIN..=TARGET_GREEN_NUMBER_MAX,
                    )
                    .text(tr!(text, "profile-display-green-number")),
                )
                .changed();
            if green_changed {
                profile.lane.note_display_duration_ms =
                    crate::config::play::duration_ms_from_green_number(
                        profile.lane.target_green_number,
                    )
                    .clamp(
                        crate::config::play::NOTE_DISPLAY_DURATION_MIN_MS,
                        crate::config::play::NOTE_DISPLAY_DURATION_MAX_MS,
                    );
            }
            let duration_changed = ui
                .add(
                    egui::Slider::new(
                        &mut profile.lane.note_display_duration_ms,
                        crate::config::play::NOTE_DISPLAY_DURATION_MIN_MS
                            ..=crate::config::play::NOTE_DISPLAY_DURATION_MAX_MS,
                    )
                    .text(tr!(text, "profile-display-note-duration")),
                )
                .changed();
            if duration_changed {
                profile.lane.target_green_number =
                    crate::config::play::green_number_from_duration_ms(
                        profile.lane.note_display_duration_ms,
                    )
                    .clamp(TARGET_GREEN_NUMBER_MIN, TARGET_GREEN_NUMBER_MAX);
            }
            ui.checkbox(&mut profile.lane.constant_enabled, tr!(text, "profile-display-constant"));
            ui.add_enabled(
                profile.lane.constant_enabled,
                egui::Slider::new(
                    &mut profile.lane.constant_fade_ms,
                    crate::config::play::CONSTANT_FADE_MIN_MS
                        ..=crate::config::play::CONSTANT_FADE_MAX_MS,
                )
                .text(tr!(text, "profile-display-constant-fade")),
            );
        });
}
