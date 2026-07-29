use super::*;

pub(in crate::ui) fn build_settings_panel(
    ctx: &egui::Context,
    window: &Window,
    open: &mut bool,
    config: &mut AppConfig,
    profile: &mut ProfileConfig,
    show_fps: &mut bool,
    editable: bool,
    difficulty_tables: &[DifficultyTableRecord],
    text: Localizer,
    mut state: SettingsPanelState<'_>,
) -> SettingsPanelActions {
    let mut save_clicked = false;
    let mut obs_enabled_changed = false;
    let mut save_profile = false;
    let mut rescan_clicked = false;
    let mut check_update_clicked = false;
    let mut song_scan_requests = Vec::new();
    let mut table_fetch_urls = Vec::new();
    let mut score_import_request = None;
    let mut apply_audio = false;
    localized_sized_panel_window(
        "app_settings_panel",
        tr!(text, "settings-app-title"),
        ctx,
        open,
        440.0,
        520.0,
        egui::pos2(16.0, 320.0),
    )
    .show(ctx, |ui| {
        if !editable {
            ui.label(tr!(text, "settings-disabled-during-play"));
            ui.separator();
        }
        ui.add_enabled_ui(editable, |ui| {
            scrollable_window_content(ui, |ui| {
                build_library_settings_sections(
                    ui,
                    config,
                    difficulty_tables,
                    text,
                    &mut state,
                    &mut save_clicked,
                    &mut rescan_clicked,
                    &mut song_scan_requests,
                    &mut table_fetch_urls,
                    &mut score_import_request,
                );
                build_audio_video_settings_sections(
                    ui,
                    window,
                    config,
                    profile,
                    show_fps,
                    text,
                    &mut state,
                    &mut apply_audio,
                    &mut save_profile,
                    &mut obs_enabled_changed,
                );
                build_integration_settings_sections(
                    ui,
                    config,
                    text,
                    &mut state,
                    &mut save_clicked,
                    &mut check_update_clicked,
                );
                if ui.button(tr!(text, "settings-save")).clicked() {
                    save_clicked = true;
                }
            });
        });
    });
    SettingsPanelActions {
        save: save_clicked || apply_audio,
        obs_enabled_changed,
        save_profile,
        check_update: check_update_clicked,
        rescan: rescan_clicked,
        song_scan_requests,
        table_fetch_urls,
        score_import_request,
        apply_audio,
    }
}
