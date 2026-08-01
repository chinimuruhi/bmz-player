/// プロファイルのスキン設定 (`SkinConfig`) を編集するパネル。
pub(in crate::ui) fn build_skin_panel(
    ctx: &egui::Context,
    open: &mut bool,
    skin: &mut SkinConfig,
    skin_meta: &SkinConfigMeta,
    skin_catalog: &SkinCatalog,
    app_paths: &AppPaths,
    text: Localizer,
) -> SkinPanelActions {
    let mut save_clicked = false;
    let mut reset_clicked = false;
    let mut changed = false;
    let before_skin = skin.clone();
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
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Select,
                    &skin_scene_label(SkinSlot::Select, text),
                    &skin_catalog.select,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Decide,
                    &skin_scene_label(SkinSlot::Decide, text),
                    &skin_catalog.decide,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Play4,
                    &skin_scene_label(SkinSlot::Play4, text),
                    &skin_catalog.play4,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Play5,
                    &skin_scene_label(SkinSlot::Play5, text),
                    &skin_catalog.play5,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Play6,
                    &skin_scene_label(SkinSlot::Play6, text),
                    &skin_catalog.play6,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Play7,
                    &skin_scene_label(SkinSlot::Play7, text),
                    &skin_catalog.play7,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Play8,
                    &skin_scene_label(SkinSlot::Play8, text),
                    &skin_catalog.play8,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Play9,
                    &skin_scene_label(SkinSlot::Play9, text),
                    &skin_catalog.play9,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Play10,
                    &skin_scene_label(SkinSlot::Play10, text),
                    &skin_catalog.play10,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Play14,
                    &skin_scene_label(SkinSlot::Play14, text),
                    &skin_catalog.play14,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Battle5,
                    &skin_scene_label(SkinSlot::Battle5, text),
                    &skin_catalog.battle5,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Battle7,
                    &skin_scene_label(SkinSlot::Battle7, text),
                    &skin_catalog.battle7,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::Result,
                    &skin_scene_label(SkinSlot::Result, text),
                    &skin_catalog.result,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
                changed |= skin_path_combo(
                    ui,
                    skin,
                    SkinSlot::CourseResult,
                    &skin_scene_label(SkinSlot::CourseResult, text),
                    &skin_catalog.course_result,
                    show_bundled_origin,
                    text,
                );
                ui.end_row();
            });
            ui.separator();
            ui.label(tr!(text, "skin-loaded-options-description"));
            let select_root = skin_root_path(app_paths, &skin.select);
            let decide_root = skin_root_path(app_paths, &skin.decide);
            let play4_root = skin_root_path(app_paths, &skin.play4);
            let play5_root = skin_root_path(app_paths, &skin.play5);
            let play6_root = skin_root_path(app_paths, &skin.play6);
            let play7_root = skin_root_path(app_paths, &skin.play7);
            let play8_root = skin_root_path(app_paths, &skin.play8);
            let play9_root = skin_root_path(app_paths, &skin.play9);
            let play10_root = skin_root_path(app_paths, &skin.play10);
            let play14_root = skin_root_path(app_paths, &skin.play14);
            let battle5_root = skin_root_path(app_paths, &skin.battle5);
            let battle7_root = skin_root_path(app_paths, &skin.battle7);
            let result_root = skin_root_path(app_paths, &skin.result);
            let course_result_root = skin_root_path(app_paths, &skin.course_result);
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Select,
                &skin_meta.select,
                select_root.as_ref(),
                &mut skin.select_options,
                &mut skin.select_files,
                &mut skin.select_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Decide,
                &skin_meta.decide,
                decide_root.as_ref(),
                &mut skin.decide_options,
                &mut skin.decide_files,
                &mut skin.decide_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play4,
                &skin_meta.play4,
                play4_root.as_ref(),
                &mut skin.play4_options,
                &mut skin.play4_files,
                &mut skin.play4_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play5,
                &skin_meta.play5,
                play5_root.as_ref(),
                &mut skin.play5_options,
                &mut skin.play5_files,
                &mut skin.play5_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play6,
                &skin_meta.play6,
                play6_root.as_ref(),
                &mut skin.play6_options,
                &mut skin.play6_files,
                &mut skin.play6_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play7,
                &skin_meta.play7,
                play7_root.as_ref(),
                &mut skin.play7_options,
                &mut skin.play7_files,
                &mut skin.play7_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play8,
                &skin_meta.play8,
                play8_root.as_ref(),
                &mut skin.play8_options,
                &mut skin.play8_files,
                &mut skin.play8_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play9,
                &skin_meta.play9,
                play9_root.as_ref(),
                &mut skin.play9_options,
                &mut skin.play9_files,
                &mut skin.play9_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play10,
                &skin_meta.play10,
                play10_root.as_ref(),
                &mut skin.play10_options,
                &mut skin.play10_files,
                &mut skin.play10_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play14,
                &skin_meta.play14,
                play14_root.as_ref(),
                &mut skin.play14_options,
                &mut skin.play14_files,
                &mut skin.play14_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Battle5,
                &skin_meta.battle5,
                battle5_root.as_ref(),
                &mut skin.battle5_options,
                &mut skin.battle5_files,
                &mut skin.battle5_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Battle7,
                &skin_meta.battle7,
                battle7_root.as_ref(),
                &mut skin.battle7_options,
                &mut skin.battle7_files,
                &mut skin.battle7_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Result,
                &skin_meta.result,
                result_root.as_ref(),
                &mut skin.result_options,
                &mut skin.result_files,
                &mut skin.result_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::CourseResult,
                &skin_meta.course_result,
                course_result_root.as_ref(),
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
    let reload = if changed {
        skin_reload_request_from_diff(&before_skin, skin)
    } else {
        Default::default()
    };
    SkinPanelActions { save: save_clicked, reset: reset_clicked, reload }
}
use super::*;
