/// プロファイルのスキン設定 (`SkinConfig`) を編集するパネル。
pub(in crate::ui) fn build_skin_panel(
    ctx: &egui::Context,
    open: &mut bool,
    skin: &mut SkinConfig,
    skin_meta: &SkinConfigMeta,
    skin_catalog: &SkinCatalog,
    app_paths: &AppPaths,
    path_cache: &mut SkinUiPathCache,
    text: Localizer,
) -> SkinPanelActions {
    let mut save_clicked = false;
    let mut reset_clicked = false;
    let mut reload = SkinReloadRequest::default();
    let show_bundled_origin = show_bundled_skin_origin(app_paths, skin_catalog);
    localized_sized_panel_window(
        "スキン設定",
        tr!(text, "skin-title"),
        ctx,
        open,
        440.0,
        560.0,
        egui::pos2(16.0, 480.0),
    )
    .show(ctx, |ui| {
        scrollable_window_content(ui, |ui| {
            ui.label(tr!(text, "skin-description"));
            egui::Grid::new("skin_grid").num_columns(2).show(ui, |ui| {
                for (slot, candidates) in [
                    (SkinSlot::Select, skin_catalog.select.as_slice()),
                    (SkinSlot::Decide, skin_catalog.decide.as_slice()),
                    (SkinSlot::Play4, skin_catalog.play4.as_slice()),
                    (SkinSlot::Play5, skin_catalog.play5.as_slice()),
                    (SkinSlot::Play6, skin_catalog.play6.as_slice()),
                    (SkinSlot::Play7, skin_catalog.play7.as_slice()),
                    (SkinSlot::Play8, skin_catalog.play8.as_slice()),
                    (SkinSlot::Play9, skin_catalog.play9.as_slice()),
                    (SkinSlot::Play10, skin_catalog.play10.as_slice()),
                    (SkinSlot::Play14, skin_catalog.play14.as_slice()),
                    (SkinSlot::Battle5, skin_catalog.battle5.as_slice()),
                    (SkinSlot::Battle7, skin_catalog.battle7.as_slice()),
                    (SkinSlot::Result, skin_catalog.result.as_slice()),
                    (SkinSlot::CourseResult, skin_catalog.course_result.as_slice()),
                ] {
                    if skin_path_combo(
                        ui,
                        skin,
                        slot,
                        &skin_scene_label(slot, text),
                        candidates,
                        show_bundled_origin,
                        text,
                    ) {
                        // path ごとの履歴復元は options / files と offset をまとめて差し替える。
                        request_skin_reload(&mut reload, slot, true);
                    }
                    ui.end_row();
                }
            });
            ui.separator();
            ui.label(tr!(text, "skin-loaded-options-description"));
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Select,
                &skin_meta.select,
                &skin.select,
                app_paths,
                path_cache,
                &mut skin.select_options,
                &mut skin.select_files,
                &mut skin.select_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Decide,
                &skin_meta.decide,
                &skin.decide,
                app_paths,
                path_cache,
                &mut skin.decide_options,
                &mut skin.decide_files,
                &mut skin.decide_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Play4,
                &skin_meta.play4,
                &skin.play4,
                app_paths,
                path_cache,
                &mut skin.play4_options,
                &mut skin.play4_files,
                &mut skin.play4_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Play5,
                &skin_meta.play5,
                &skin.play5,
                app_paths,
                path_cache,
                &mut skin.play5_options,
                &mut skin.play5_files,
                &mut skin.play5_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Play6,
                &skin_meta.play6,
                &skin.play6,
                app_paths,
                path_cache,
                &mut skin.play6_options,
                &mut skin.play6_files,
                &mut skin.play6_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Play7,
                &skin_meta.play7,
                &skin.play7,
                app_paths,
                path_cache,
                &mut skin.play7_options,
                &mut skin.play7_files,
                &mut skin.play7_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Play8,
                &skin_meta.play8,
                &skin.play8,
                app_paths,
                path_cache,
                &mut skin.play8_options,
                &mut skin.play8_files,
                &mut skin.play8_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Play9,
                &skin_meta.play9,
                &skin.play9,
                app_paths,
                path_cache,
                &mut skin.play9_options,
                &mut skin.play9_files,
                &mut skin.play9_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Play10,
                &skin_meta.play10,
                &skin.play10,
                app_paths,
                path_cache,
                &mut skin.play10_options,
                &mut skin.play10_files,
                &mut skin.play10_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Play14,
                &skin_meta.play14,
                &skin.play14,
                app_paths,
                path_cache,
                &mut skin.play14_options,
                &mut skin.play14_files,
                &mut skin.play14_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Battle5,
                &skin_meta.battle5,
                &skin.battle5,
                app_paths,
                path_cache,
                &mut skin.battle5_options,
                &mut skin.battle5_files,
                &mut skin.battle5_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Battle7,
                &skin_meta.battle7,
                &skin.battle7,
                app_paths,
                path_cache,
                &mut skin.battle7_options,
                &mut skin.battle7_files,
                &mut skin.battle7_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::Result,
                &skin_meta.result,
                &skin.result,
                app_paths,
                path_cache,
                &mut skin.result_options,
                &mut skin.result_files,
                &mut skin.result_offsets,
                text,
            );
            build_scene_skin_defs_with_reload(
                &mut reload,
                ui,
                SkinSlot::CourseResult,
                &skin_meta.course_result,
                &skin.course_result,
                app_paths,
                path_cache,
                &mut skin.course_result_options,
                &mut skin.course_result_files,
                &mut skin.course_result_offsets,
                text,
            );
            ui.separator();
            ui.label(tr!(text, "skin-save-reset-help"));
            ui.horizontal(|ui| {
                if ui.button(tr!(text, "skin-save")).clicked() {
                    save_clicked = true;
                }
                if ui.button(tr!(text, "skin-reset")).clicked() {
                    reset_clicked = true;
                }
            });
        });
    });
    SkinPanelActions { save: save_clicked, reset: reset_clicked, reload }
}

#[allow(clippy::too_many_arguments)]
fn build_scene_skin_defs_with_reload(
    reload: &mut SkinReloadRequest,
    ui: &mut egui::Ui,
    slot: SkinSlot,
    defs: &SceneSkinDefs,
    skin_path: &str,
    app_paths: &AppPaths,
    path_cache: &mut SkinUiPathCache,
    options: &mut BTreeMap<String, String>,
    files: &mut BTreeMap<String, String>,
    offsets: &mut Vec<SkinOffsetConfig>,
    text: Localizer,
) {
    let edit = build_scene_skin_defs(
        ui, slot, defs, skin_path, app_paths, path_cache, options, files, offsets, text,
    );
    if edit.changed {
        request_skin_reload(reload, slot, edit.offsets_changed);
    }
}
use super::*;
