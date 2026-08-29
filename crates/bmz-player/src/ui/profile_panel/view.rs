use super::*;

pub(in crate::ui) fn build_profile_settings_panel(
    context: ProfileSettingsPanelContext<'_>,
) -> ProfileSettingsPanelActions {
    let ProfileSettingsPanelContext {
        ctx,
        open,
        profile,
        app_config,
        show_fps,
        ir_login,
        ir_device_key,
        profile_manager,
        key_config,
        profile_root,
        unrestricted,
        text,
    } = context;

    if !*open || !unrestricted {
        key_config.listening = None;
    }

    // ログインタスクの完了を反映。provider 設定が更新されたら保存する。
    let save_clicked = ir_login.poll(profile, text);
    ir_device_key.poll(text);
    let readonly_profile = (!unrestricted).then(|| profile.clone());
    let readonly_app_config = (!unrestricted).then(|| app_config.clone());
    let mut section = ProfileSectionContext {
        profile,
        app_config,
        show_fps,
        ir_login,
        ir_device_key,
        profile_manager,
        key_config,
        profile_root,
        unrestricted,
        text,
        save_clicked,
        save_app_config: false,
        key_config_action: None,
    };

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
            if !section.unrestricted {
                ui.label(tr!(section.text, "profile-settings-restricted"));
                ui.separator();
            }
            build_profile_basic_section(ui, &mut section);
            section.save_app_config |= build_profile_manager_section(
                ui,
                section.app_config,
                section.profile,
                section.profile_manager,
                section.unrestricted,
                section.text,
            );
            build_profile_volume_section(ui, &mut section);
            build_profile_judge_section(ui, &mut section);
            build_profile_play_section(ui, &mut section);
            build_profile_display_section(ui, &mut section);
            build_profile_select_section(ui, &mut section);
            build_profile_input_section(ui, &mut section);
            build_profile_key_config_section(ui, &mut section);
            build_profile_replay_section(ui, &mut section);
            build_profile_system_sound_section(ui, &mut section);
            build_profile_ir_section(ui, &mut section);
            build_profile_ui_section(ui, &mut section);

            ui.separator();
            if ui.button(tr!(section.text, "settings-save")).clicked() {
                section.save_clicked = true;
            }
        });
    });

    if let Some(readonly) = readonly_profile {
        restore_restricted_profile_settings(section.profile, readonly);
    }
    if let Some(readonly) = readonly_app_config {
        *section.app_config = readonly;
        section.save_app_config = false;
    }
    if !*open {
        section.key_config.listening = None;
    }
    ProfileSettingsPanelActions {
        save: section.save_clicked,
        save_app_config: section.save_app_config,
        key_config_action: section.key_config_action,
    }
}
