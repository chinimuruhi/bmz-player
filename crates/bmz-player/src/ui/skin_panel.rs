use super::*;

/// スキン設定パネルからのアクション要求。
pub(super) struct SkinPanelActions {
    /// 「保存」ボタンが押された (profile.toml へ書き出し)。
    pub(super) save: bool,
    /// 「リセット」ボタンが押された (profile.toml の値へ戻す)。
    pub(super) reset: bool,
    /// パネル内のスキン設定変更に対して必要な反映対象。
    pub(super) reload: SkinReloadRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SkinSlot {
    Select,
    Decide,
    Play4,
    Play5,
    Play6,
    Play7,
    Play8,
    Play9,
    Play10,
    Play14,
    Battle5,
    Battle7,
    Result,
    CourseResult,
}

impl SkinSlot {
    /// locale を切り替えても egui の永続 widget ID が変わらないよう、
    /// i18n 前に ID へ使われていた日本語ラベルを固定 salt として維持する。
    const fn path_combo_id(self) -> &'static str {
        match self {
            Self::Select => "選曲",
            Self::Decide => "決定",
            Self::Play4 => "プレイ (4K)",
            Self::Play5 => "プレイ (5K)",
            Self::Play6 => "プレイ (6K)",
            Self::Play7 => "プレイ (7K)",
            Self::Play8 => "プレイ (8K)",
            Self::Play9 => "プレイ (9K)",
            Self::Play10 => "プレイ (10K)",
            Self::Play14 => "プレイ (14K)",
            Self::Battle5 => "プレイ (5K BATTLE)",
            Self::Battle7 => "プレイ (7K BATTLE)",
            Self::Result => "リザルト",
            Self::CourseResult => "コースリザルト",
        }
    }

    const fn defs_header_id(self) -> &'static str {
        match self {
            Self::Select => "選曲スキン",
            Self::Decide => "決定スキン",
            Self::Play4 => "プレイスキン (4K)",
            Self::Play5 => "プレイスキン (5K)",
            Self::Play6 => "プレイスキン (6K)",
            Self::Play7 => "プレイスキン (7K)",
            Self::Play8 => "プレイスキン (8K)",
            Self::Play9 => "プレイスキン (9K)",
            Self::Play10 => "プレイスキン (10K)",
            Self::Play14 => "プレイスキン (14K)",
            Self::Battle5 => "プレイスキン (5K BATTLE)",
            Self::Battle7 => "プレイスキン (7K BATTLE)",
            Self::Result => "リザルトスキン",
            Self::CourseResult => "コースリザルトスキン",
        }
    }
}

pub(super) fn skin_scene_label(slot: SkinSlot, text: Localizer) -> String {
    match slot {
        SkinSlot::Select => tr!(text, "skin-scene-select"),
        SkinSlot::Decide => tr!(text, "skin-scene-decide"),
        SkinSlot::Play4 => tr!(text, "skin-scene-play", "keys" => "4K"),
        SkinSlot::Play5 => tr!(text, "skin-scene-play", "keys" => "5K"),
        SkinSlot::Play6 => tr!(text, "skin-scene-play", "keys" => "6K"),
        SkinSlot::Play7 => tr!(text, "skin-scene-play", "keys" => "7K"),
        SkinSlot::Play8 => tr!(text, "skin-scene-play", "keys" => "8K"),
        SkinSlot::Play9 => tr!(text, "skin-scene-play", "keys" => "9K"),
        SkinSlot::Play10 => tr!(text, "skin-scene-play", "keys" => "10K"),
        SkinSlot::Play14 => tr!(text, "skin-scene-play", "keys" => "14K"),
        SkinSlot::Battle5 => tr!(text, "skin-scene-play", "keys" => "5K BATTLE"),
        SkinSlot::Battle7 => tr!(text, "skin-scene-play", "keys" => "7K BATTLE"),
        SkinSlot::Result => tr!(text, "skin-scene-result"),
        SkinSlot::CourseResult => tr!(text, "skin-scene-course-result"),
    }
}

pub(super) fn skin_scene_defs_label(slot: SkinSlot, text: Localizer) -> String {
    tr!(text, "skin-scene-options", "scene" => skin_scene_label(slot, text))
}

pub(super) fn skin_reload_request_from_diff(
    before: &SkinConfig,
    after: &SkinConfig,
) -> SkinReloadRequest {
    let mut request = SkinReloadRequest::default();
    let select_offsets_changed = before.select_offsets != after.select_offsets;
    let decide_offsets_changed = before.decide_offsets != after.decide_offsets;
    let play4_offsets_changed = before.play4_offsets != after.play4_offsets;
    let play5_offsets_changed = before.play5_offsets != after.play5_offsets;
    let play6_offsets_changed = before.play6_offsets != after.play6_offsets;
    let play7_offsets_changed = before.play7_offsets != after.play7_offsets;
    let play8_offsets_changed = before.play8_offsets != after.play8_offsets;
    let play9_offsets_changed = before.play9_offsets != after.play9_offsets;
    let play10_offsets_changed = before.play10_offsets != after.play10_offsets;
    let play14_offsets_changed = before.play14_offsets != after.play14_offsets;
    let battle5_offsets_changed = before.battle5_offsets != after.battle5_offsets;
    let battle7_offsets_changed = before.battle7_offsets != after.battle7_offsets;
    let result_offsets_changed = before.result_offsets != after.result_offsets;
    let course_result_offsets_changed = before.course_result_offsets != after.course_result_offsets;
    if before.select != after.select
        || before.select_options != after.select_options
        || before.select_files != after.select_files
        || select_offsets_changed
    {
        request.select = true;
    }
    if before.decide != after.decide
        || before.decide_options != after.decide_options
        || before.decide_files != after.decide_files
        || decide_offsets_changed
    {
        request.decide = true;
    }
    if before.play4 != after.play4
        || before.play4_options != after.play4_options
        || before.play4_files != after.play4_files
        || play4_offsets_changed
    {
        request.play4 = true;
    }
    if before.play5 != after.play5
        || before.play5_options != after.play5_options
        || before.play5_files != after.play5_files
        || play5_offsets_changed
    {
        request.play5 = true;
    }
    if before.play6 != after.play6
        || before.play6_options != after.play6_options
        || before.play6_files != after.play6_files
        || play6_offsets_changed
    {
        request.play6 = true;
    }
    if before.play7 != after.play7
        || before.play7_options != after.play7_options
        || before.play7_files != after.play7_files
        || play7_offsets_changed
    {
        request.play7 = true;
    }
    if before.play8 != after.play8
        || before.play8_options != after.play8_options
        || before.play8_files != after.play8_files
        || play8_offsets_changed
    {
        request.play8 = true;
    }
    if before.play9 != after.play9
        || before.play9_options != after.play9_options
        || before.play9_files != after.play9_files
        || play9_offsets_changed
    {
        request.play9 = true;
    }
    if before.play10 != after.play10
        || before.play10_options != after.play10_options
        || before.play10_files != after.play10_files
        || play10_offsets_changed
    {
        request.play10 = true;
    }
    if before.play14 != after.play14
        || before.play14_options != after.play14_options
        || before.play14_files != after.play14_files
        || play14_offsets_changed
    {
        request.play14 = true;
    }
    if before.battle5 != after.battle5
        || before.battle5_options != after.battle5_options
        || before.battle5_files != after.battle5_files
        || battle5_offsets_changed
    {
        request.play10 = true;
    }
    if before.battle7 != after.battle7
        || before.battle7_options != after.battle7_options
        || before.battle7_files != after.battle7_files
        || battle7_offsets_changed
    {
        request.play14 = true;
    }
    if before.result != after.result
        || before.result_options != after.result_options
        || before.result_files != after.result_files
        || result_offsets_changed
    {
        request.result = true;
    }
    if before.course_result != after.course_result
        || before.course_result_options != after.course_result_options
        || before.course_result_files != after.course_result_files
        || course_result_offsets_changed
    {
        request.course_result = true;
    }
    request.offsets = select_offsets_changed
        || decide_offsets_changed
        || play4_offsets_changed
        || play5_offsets_changed
        || play6_offsets_changed
        || play7_offsets_changed
        || play8_offsets_changed
        || play9_offsets_changed
        || play10_offsets_changed
        || play14_offsets_changed
        || battle5_offsets_changed
        || battle7_offsets_changed
        || result_offsets_changed
        || course_result_offsets_changed;
    request
}

pub(super) fn skin_path_combo(
    ui: &mut egui::Ui,
    skin: &mut SkinConfig,
    slot: SkinSlot,
    label: &str,
    candidates: &[SkinCandidate],
    show_bundled_origin: bool,
    text: Localizer,
) -> bool {
    ui.label(label);
    let current = skin_slot_path(skin, slot).to_string();
    let mut selected = current.clone();
    let selected_text = skin_candidate_label(candidates, &current, show_bundled_origin, text);
    egui::ComboBox::from_id_salt(("skin_path_combo", slot.path_combo_id()))
        .selected_text(selected_text)
        .width(320.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut selected, String::new(), tr!(text, "skin-default"));
            for candidate in candidates {
                let response = ui.selectable_value(
                    &mut selected,
                    candidate.path.clone(),
                    skin_candidate_display(candidate, show_bundled_origin, text),
                );
                match candidate.origin {
                    SkinCandidateOrigin::Bundled if show_bundled_origin => {
                        response.on_hover_text(tr!(text, "skin-origin-bundled-help"));
                    }
                    SkinCandidateOrigin::Bundled => {}
                    SkinCandidateOrigin::User => {
                        response.on_hover_text(tr!(text, "skin-origin-user-help"));
                    }
                    SkinCandidateOrigin::External => {
                        response.on_hover_text(tr!(text, "skin-origin-external-help"));
                    }
                }
            }
        });
    let combo_changed = selected != current;
    if combo_changed {
        save_skin_slot_history(skin, slot);
        *skin_slot_path_mut(skin, slot) = selected;
        restore_skin_slot_history(skin, slot);
    }
    let mut edited_path = skin_slot_path(skin, slot).to_string();
    let text_changed = ui.text_edit_singleline(&mut edited_path).changed();
    if text_changed {
        save_skin_slot_history(skin, slot);
        *skin_slot_path_mut(skin, slot) = edited_path;
        restore_skin_slot_history(skin, slot);
    }
    combo_changed || text_changed
}

pub(super) fn skin_candidate_label(
    candidates: &[SkinCandidate],
    current: &str,
    show_bundled_origin: bool,
    text: Localizer,
) -> String {
    if current.is_empty() {
        return tr!(text, "skin-default");
    }
    candidates
        .iter()
        .find(|candidate| candidate.path == current)
        .map(|candidate| skin_candidate_display(candidate, show_bundled_origin, text))
        .unwrap_or_else(|| current.to_string())
}

pub(super) fn skin_candidate_display(
    candidate: &SkinCandidate,
    show_bundled_origin: bool,
    text: Localizer,
) -> String {
    let label = skin_candidate_origin_label(candidate.origin, show_bundled_origin, text);
    let text = if candidate.name.is_empty() {
        candidate.path.clone()
    } else {
        format!("{} ({})", candidate.name, candidate.path)
    };
    if let Some(label) = label { format!("{label} {text}") } else { text }
}

pub(super) fn skin_candidate_origin_label(
    origin: SkinCandidateOrigin,
    show_bundled_origin: bool,
    text: Localizer,
) -> Option<String> {
    match origin {
        SkinCandidateOrigin::Bundled if show_bundled_origin => {
            Some(tr!(text, "skin-origin-bundled"))
        }
        SkinCandidateOrigin::Bundled => None,
        SkinCandidateOrigin::User => Some(tr!(text, "skin-origin-user")),
        SkinCandidateOrigin::External => Some(tr!(text, "skin-origin-external")),
    }
}

pub(super) fn show_bundled_skin_origin(app_paths: &AppPaths, skin_catalog: &SkinCatalog) -> bool {
    !app_paths.hides_bundled_skin_label() && skin_catalog_has_non_bundled_candidate(skin_catalog)
}

pub(super) fn skin_catalog_has_non_bundled_candidate(skin_catalog: &SkinCatalog) -> bool {
    let groups: [&[SkinCandidate]; 14] = [
        &skin_catalog.select,
        &skin_catalog.decide,
        &skin_catalog.play4,
        &skin_catalog.play5,
        &skin_catalog.play6,
        &skin_catalog.play7,
        &skin_catalog.play8,
        &skin_catalog.play9,
        &skin_catalog.play10,
        &skin_catalog.play14,
        &skin_catalog.battle5,
        &skin_catalog.battle7,
        &skin_catalog.result,
        &skin_catalog.course_result,
    ];
    groups.iter().any(|candidates| {
        candidates.iter().any(|candidate| candidate.origin != SkinCandidateOrigin::Bundled)
    })
}

pub(super) fn skin_slot_path(skin: &SkinConfig, slot: SkinSlot) -> &str {
    match slot {
        SkinSlot::Select => &skin.select,
        SkinSlot::Decide => &skin.decide,
        SkinSlot::Play4 => &skin.play4,
        SkinSlot::Play5 => &skin.play5,
        SkinSlot::Play6 => &skin.play6,
        SkinSlot::Play7 => &skin.play7,
        SkinSlot::Play8 => &skin.play8,
        SkinSlot::Play9 => &skin.play9,
        SkinSlot::Play10 => &skin.play10,
        SkinSlot::Play14 => &skin.play14,
        SkinSlot::Battle5 => &skin.battle5,
        SkinSlot::Battle7 => &skin.battle7,
        SkinSlot::Result => &skin.result,
        SkinSlot::CourseResult => &skin.course_result,
    }
}

pub(super) fn skin_slot_path_mut(skin: &mut SkinConfig, slot: SkinSlot) -> &mut String {
    match slot {
        SkinSlot::Select => &mut skin.select,
        SkinSlot::Decide => &mut skin.decide,
        SkinSlot::Play4 => &mut skin.play4,
        SkinSlot::Play5 => &mut skin.play5,
        SkinSlot::Play6 => &mut skin.play6,
        SkinSlot::Play7 => &mut skin.play7,
        SkinSlot::Play8 => &mut skin.play8,
        SkinSlot::Play9 => &mut skin.play9,
        SkinSlot::Play10 => &mut skin.play10,
        SkinSlot::Play14 => &mut skin.play14,
        SkinSlot::Battle5 => &mut skin.battle5,
        SkinSlot::Battle7 => &mut skin.battle7,
        SkinSlot::Result => &mut skin.result,
        SkinSlot::CourseResult => &mut skin.course_result,
    }
}

pub(super) fn skin_slot_options_mut(
    skin: &mut SkinConfig,
    slot: SkinSlot,
) -> &mut BTreeMap<String, String> {
    match slot {
        SkinSlot::Select => &mut skin.select_options,
        SkinSlot::Decide => &mut skin.decide_options,
        SkinSlot::Play4 => &mut skin.play4_options,
        SkinSlot::Play5 => &mut skin.play5_options,
        SkinSlot::Play6 => &mut skin.play6_options,
        SkinSlot::Play7 => &mut skin.play7_options,
        SkinSlot::Play8 => &mut skin.play8_options,
        SkinSlot::Play9 => &mut skin.play9_options,
        SkinSlot::Play10 => &mut skin.play10_options,
        SkinSlot::Play14 => &mut skin.play14_options,
        SkinSlot::Battle5 => &mut skin.battle5_options,
        SkinSlot::Battle7 => &mut skin.battle7_options,
        SkinSlot::Result => &mut skin.result_options,
        SkinSlot::CourseResult => &mut skin.course_result_options,
    }
}

pub(super) fn skin_slot_files_mut(
    skin: &mut SkinConfig,
    slot: SkinSlot,
) -> &mut BTreeMap<String, String> {
    match slot {
        SkinSlot::Select => &mut skin.select_files,
        SkinSlot::Decide => &mut skin.decide_files,
        SkinSlot::Play4 => &mut skin.play4_files,
        SkinSlot::Play5 => &mut skin.play5_files,
        SkinSlot::Play6 => &mut skin.play6_files,
        SkinSlot::Play7 => &mut skin.play7_files,
        SkinSlot::Play8 => &mut skin.play8_files,
        SkinSlot::Play9 => &mut skin.play9_files,
        SkinSlot::Play10 => &mut skin.play10_files,
        SkinSlot::Play14 => &mut skin.play14_files,
        SkinSlot::Battle5 => &mut skin.battle5_files,
        SkinSlot::Battle7 => &mut skin.battle7_files,
        SkinSlot::Result => &mut skin.result_files,
        SkinSlot::CourseResult => &mut skin.course_result_files,
    }
}

pub(super) fn skin_slot_offsets_mut(
    skin: &mut SkinConfig,
    slot: SkinSlot,
) -> &mut Vec<SkinOffsetConfig> {
    match slot {
        SkinSlot::Select => &mut skin.select_offsets,
        SkinSlot::Decide => &mut skin.decide_offsets,
        SkinSlot::Play4 => &mut skin.play4_offsets,
        SkinSlot::Play5 => &mut skin.play5_offsets,
        SkinSlot::Play6 => &mut skin.play6_offsets,
        SkinSlot::Play7 => &mut skin.play7_offsets,
        SkinSlot::Play8 => &mut skin.play8_offsets,
        SkinSlot::Play9 => &mut skin.play9_offsets,
        SkinSlot::Play10 => &mut skin.play10_offsets,
        SkinSlot::Play14 => &mut skin.play14_offsets,
        SkinSlot::Battle5 => &mut skin.battle5_offsets,
        SkinSlot::Battle7 => &mut skin.battle7_offsets,
        SkinSlot::Result => &mut skin.result_offsets,
        SkinSlot::CourseResult => &mut skin.course_result_offsets,
    }
}

pub(super) fn skin_slot_history_key(slot: SkinSlot, path: &str) -> String {
    let slot_name = match slot {
        SkinSlot::Select => "select",
        SkinSlot::Decide => "decide",
        SkinSlot::Play4 => "play4",
        SkinSlot::Play5 => "play5",
        SkinSlot::Play6 => "play6",
        SkinSlot::Play7 => "play7",
        SkinSlot::Play8 => "play8",
        SkinSlot::Play9 => "play9",
        SkinSlot::Play10 => "play10",
        SkinSlot::Play14 => "play14",
        SkinSlot::Battle5 => "battle5",
        SkinSlot::Battle7 => "battle7",
        SkinSlot::Result => "result",
        SkinSlot::CourseResult => "course_result",
    };
    format!("{slot_name}::{path}")
}

pub(super) fn save_skin_slot_history(skin: &mut SkinConfig, slot: SkinSlot) {
    let path = skin_slot_path(skin, slot).trim().to_string();
    if path.is_empty() {
        return;
    }
    let options = skin_slot_options_mut(skin, slot).clone();
    let files = skin_slot_files_mut(skin, slot).clone();
    let offsets = skin_slot_offsets_mut(skin, slot).clone();
    skin.history.insert(
        skin_slot_history_key(slot, &path),
        SkinHistoryEntryConfig { options, files, offsets },
    );
}

pub(super) fn restore_skin_slot_history(skin: &mut SkinConfig, slot: SkinSlot) {
    let path = skin_slot_path(skin, slot).trim().to_string();
    let history_key = skin_slot_history_key(slot, &path);
    let Some(entry) =
        skin.history.get(&history_key).cloned().or_else(|| skin.history.get(&path).cloned())
    else {
        skin_slot_options_mut(skin, slot).clear();
        skin_slot_files_mut(skin, slot).clear();
        skin_slot_offsets_mut(skin, slot).clear();
        return;
    };
    skin.history.entry(history_key).or_insert_with(|| entry.clone());
    *skin_slot_options_mut(skin, slot) = entry.options;
    *skin_slot_files_mut(skin, slot) = entry.files;
    *skin_slot_offsets_mut(skin, slot) = entry.offsets;
}

/// プロファイルのスキン設定 (`SkinConfig`) を編集するパネル。
pub(super) fn build_skin_panel(
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
                select_root.as_deref(),
                &mut skin.select_options,
                &mut skin.select_files,
                &mut skin.select_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Decide,
                &skin_meta.decide,
                decide_root.as_deref(),
                &mut skin.decide_options,
                &mut skin.decide_files,
                &mut skin.decide_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play4,
                &skin_meta.play4,
                play4_root.as_deref(),
                &mut skin.play4_options,
                &mut skin.play4_files,
                &mut skin.play4_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play5,
                &skin_meta.play5,
                play5_root.as_deref(),
                &mut skin.play5_options,
                &mut skin.play5_files,
                &mut skin.play5_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play6,
                &skin_meta.play6,
                play6_root.as_deref(),
                &mut skin.play6_options,
                &mut skin.play6_files,
                &mut skin.play6_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play7,
                &skin_meta.play7,
                play7_root.as_deref(),
                &mut skin.play7_options,
                &mut skin.play7_files,
                &mut skin.play7_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play8,
                &skin_meta.play8,
                play8_root.as_deref(),
                &mut skin.play8_options,
                &mut skin.play8_files,
                &mut skin.play8_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play9,
                &skin_meta.play9,
                play9_root.as_deref(),
                &mut skin.play9_options,
                &mut skin.play9_files,
                &mut skin.play9_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play10,
                &skin_meta.play10,
                play10_root.as_deref(),
                &mut skin.play10_options,
                &mut skin.play10_files,
                &mut skin.play10_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Play14,
                &skin_meta.play14,
                play14_root.as_deref(),
                &mut skin.play14_options,
                &mut skin.play14_files,
                &mut skin.play14_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Battle5,
                &skin_meta.battle5,
                battle5_root.as_deref(),
                &mut skin.battle5_options,
                &mut skin.battle5_files,
                &mut skin.battle5_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Battle7,
                &skin_meta.battle7,
                battle7_root.as_deref(),
                &mut skin.battle7_options,
                &mut skin.battle7_files,
                &mut skin.battle7_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::Result,
                &skin_meta.result,
                result_root.as_deref(),
                &mut skin.result_options,
                &mut skin.result_files,
                &mut skin.result_offsets,
                text,
            );
            changed |= build_scene_skin_defs(
                ui,
                SkinSlot::CourseResult,
                &skin_meta.course_result,
                course_result_root.as_deref(),
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

/// 1 シーン分のスキン設定可能項目を折りたたみ表示・編集する。
///
/// - property: ComboBox で選択肢を選び `options` へ書き込む。
/// - filepath: `path` グロブにマッチするファイルを ComboBox で選び `files` へ書き込む。
/// - offset: 宣言された要素ごとに x/y/w/h/r/a を編集し `offsets` (名前単位) へ反映。
pub(super) fn build_scene_skin_defs(
    ui: &mut egui::Ui,
    slot: SkinSlot,
    defs: &SceneSkinDefs,
    skin_root: Option<&Path>,
    options: &mut BTreeMap<String, String>,
    files: &mut BTreeMap<String, String>,
    offsets: &mut Vec<SkinOffsetConfig>,
    text: Localizer,
) -> bool {
    let mut changed = sync_skin_offsets_with_defs(&defs.offset, offsets);
    egui::CollapsingHeader::new(skin_scene_defs_label(slot, text))
        .id_salt(slot.defs_header_id())
        .show(ui, |ui| {
            if defs.is_empty() {
                ui.label(tr!(text, "skin-no-settings"));
                return;
            }
            let _ = fill_missing_skin_defaults(defs, skin_root, options, files);
            if !defs.property.is_empty() {
                ui.strong(tr!(text, "skin-options"));
                // property / filepath は同名 (例: "シャッター") を持ちうるので、egui の
                // ComboBox ID 衝突を防ぐためにカテゴリで名前空間を切る。
                ui.push_id("property", |ui| {
                    for prop in &defs.property {
                        let mut selected = options
                            .get(&prop.name)
                            .cloned()
                            .unwrap_or_else(|| property_default(prop));
                        let before = selected.clone();
                        egui::ComboBox::from_label(&prop.name).selected_text(&selected).show_ui(
                            ui,
                            |ui| {
                                for item in &prop.item {
                                    ui.selectable_value(
                                        &mut selected,
                                        item.name.clone(),
                                        &item.name,
                                    );
                                }
                            },
                        );
                        if selected != before {
                            options.insert(prop.name.clone(), selected);
                            changed = true;
                        }
                    }
                });
            }
            if !defs.filepath.is_empty() {
                ui.strong(tr!(text, "skin-file-selection"));
                ui.push_id("filepath", |ui| {
                    for filepath in &defs.filepath {
                        let mut selected = files.get(&filepath.name).cloned().unwrap_or_default();
                        let before = selected.clone();
                        let display = if selected.is_empty() {
                            tr!(text, "skin-file-none")
                        } else if selected == RANDOM_FILE_SELECTION {
                            tr!(text, "skin-file-random")
                        } else {
                            filepath_selection_label(&selected).to_string()
                        };
                        egui::ComboBox::from_label(&filepath.name).selected_text(display).show_ui(
                            ui,
                            |ui| {
                                // beatoraja 同様、具体ファイルに加えて「ランダム」を選べる。
                                // ランダム選択時は毎ロードで候補からランダムに解決する。
                                ui.selectable_value(
                                    &mut selected,
                                    RANDOM_FILE_SELECTION.to_string(),
                                    tr!(text, "skin-file-random"),
                                );
                                // 候補列挙は ComboBox を開いたときだけ行う (毎フレームの fs 走査を回避)。
                                let candidates = match skin_root {
                                    Some(root) => glob_candidates(root, &filepath.path),
                                    None => Vec::new(),
                                };
                                if let Some(normalized) =
                                    normalize_filepath_selection(&selected, &candidates)
                                {
                                    selected = normalized;
                                }
                                if candidates.is_empty() {
                                    ui.label(tr!(text, "skin-file-no-candidates"));
                                }
                                for candidate in candidates {
                                    let label = filepath_selection_label(&candidate);
                                    ui.selectable_value(&mut selected, candidate.clone(), label);
                                }
                            },
                        );
                        if selected != before {
                            files.insert(filepath.name.clone(), selected);
                            changed = true;
                        }
                    }
                });
            }
            if !defs.offset.is_empty() {
                ui.strong(tr!(text, "skin-offset-elements"));
                for (offset_index, offset_def) in defs.offset.iter().enumerate() {
                    ui.push_id((offset_index, offset_def.id, offset_def.name.as_str()), |ui| {
                        ui.label(format!(
                            "{} [{}] — id {}",
                            offset_def.name, offset_def.category, offset_def.id
                        ));
                        let existing = offsets
                            .iter()
                            .find(|offset| {
                                offset.name.as_deref() == Some(offset_def.name.as_str())
                                    && offset.id == offset_def.id
                            })
                            .or_else(|| {
                                offsets.iter().find(|offset| {
                                    offset.name.as_deref() == Some(offset_def.name.as_str())
                                })
                            })
                            .or_else(|| {
                                offsets.iter().find(|offset| {
                                    offset.name.is_none() && offset.id == offset_def.id
                                })
                            })
                            .cloned();
                        let mut value = existing.unwrap_or(SkinOffsetConfig {
                            name: Some(offset_def.name.clone()),
                            id: offset_def.id,
                            ..Default::default()
                        });
                        value.name = Some(offset_def.name.clone());
                        value.id = offset_def.id;
                        let before = value.clone();
                        ui.horizontal(|ui| {
                            changed |= add_offset_drag_values(ui, offset_def, &mut value, text);
                        });
                        if value != before {
                            changed |= update_skin_offset_value(offsets, offset_def, value);
                        }
                    });
                }
            }
            if !defs.is_empty() && ui.button(tr!(text, "skin-reset-defaults")).clicked() {
                changed |= reset_scene_skin_to_defaults(defs, skin_root, options, files, offsets);
            }
        });
    changed
}

/// 現在のスキン定義に合わせ、offset 設定を名前優先で正規化する。
///
/// 旧設定は名前を持たないため ID で移行する。同じ旧 ID を複数の異なる名前が
/// 使用する場合は値をそれぞれへ複製し、以後は独立して編集できるようにする。
/// 同名定義が複数 ID にある場合は、beatoraja と同様に最初の同名設定を共有する。
pub(super) fn sync_skin_offsets_with_defs(
    defs: &[SkinOffsetDef],
    offsets: &mut Vec<SkinOffsetConfig>,
) -> bool {
    if defs.is_empty() || offsets.is_empty() {
        return false;
    }

    let previous = offsets.clone();
    let mut synced = Vec::with_capacity(previous.len().max(defs.len()));
    for offset_def in defs {
        let source = previous
            .iter()
            .find(|offset| offset.name.as_deref() == Some(offset_def.name.as_str()))
            .or_else(|| {
                previous.iter().find(|offset| offset.name.is_none() && offset.id == offset_def.id)
            });
        if let Some(source) = source {
            let mut value = source.clone();
            value.name = Some(offset_def.name.clone());
            value.id = offset_def.id;
            synced.push(value);
        }
    }

    let declared_names: std::collections::HashSet<&str> =
        defs.iter().map(|offset| offset.name.as_str()).collect();
    let declared_ids: std::collections::HashSet<i32> =
        defs.iter().map(|offset| offset.id).collect();
    synced.extend(
        previous
            .iter()
            .filter(|offset| match offset.name.as_deref() {
                Some(name) => !declared_names.contains(name),
                None => !declared_ids.contains(&offset.id),
            })
            .cloned(),
    );

    if synced == previous {
        false
    } else {
        *offsets = synced;
        true
    }
}

/// 同名 offset の値を全定義へ反映する。ID は各定義のものを維持する。
pub(super) fn update_skin_offset_value(
    offsets: &mut Vec<SkinOffsetConfig>,
    offset_def: &SkinOffsetDef,
    value: SkinOffsetConfig,
) -> bool {
    let mut found_named = false;
    for entry in
        offsets.iter_mut().filter(|offset| offset.name.as_deref() == Some(offset_def.name.as_str()))
    {
        let id = entry.id;
        *entry = value.clone();
        entry.name = Some(offset_def.name.clone());
        entry.id = id;
        found_named = true;
    }
    if found_named {
        return true;
    }

    if let Some(entry) =
        offsets.iter_mut().find(|offset| offset.name.is_none() && offset.id == offset_def.id)
    {
        *entry = value;
        entry.name = Some(offset_def.name.clone());
        entry.id = offset_def.id;
    } else {
        offsets.push(value);
    }
    true
}

/// 1 シーン分の options / files / 当該 offset 名をスキン定義の factory default へ戻す。
pub(super) fn reset_scene_skin_to_defaults(
    defs: &SceneSkinDefs,
    skin_root: Option<&Path>,
    options: &mut BTreeMap<String, String>,
    files: &mut BTreeMap<String, String>,
    offsets: &mut Vec<SkinOffsetConfig>,
) -> bool {
    if defs.is_empty() {
        return false;
    }
    let previous_options = options.clone();
    let previous_files = files.clone();
    let previous_offsets = offsets.clone();
    options.clear();
    files.clear();
    let scene_offset_names: std::collections::HashSet<&str> =
        defs.offset.iter().map(|offset| offset.name.as_str()).collect();
    let scene_offset_ids: std::collections::HashSet<i32> =
        defs.offset.iter().map(|offset| offset.id).collect();
    offsets.retain(|offset| match offset.name.as_deref() {
        Some(name) => !scene_offset_names.contains(name),
        None => !scene_offset_ids.contains(&offset.id),
    });
    let _ = fill_missing_skin_defaults(defs, skin_root, options, files);
    *options != previous_options || *files != previous_files || *offsets != previous_offsets
}

pub(super) fn fill_missing_skin_defaults(
    defs: &SceneSkinDefs,
    skin_root: Option<&Path>,
    options: &mut BTreeMap<String, String>,
    files: &mut BTreeMap<String, String>,
) -> bool {
    let mut changed = false;
    for prop in &defs.property {
        let current = options.get(&prop.name).map(String::as_str);
        if current.is_none() || !property_selection_is_valid(prop, current.unwrap_or_default()) {
            let default = property_default(prop);
            if current != Some(default.as_str()) {
                options.insert(prop.name.clone(), default);
                changed = true;
            }
        }
    }
    let Some(skin_root) = skin_root else {
        return changed;
    };
    for filepath in &defs.filepath {
        let candidates = glob_candidates(skin_root, &filepath.path);
        let current = files.get(&filepath.name).map(|value| value.replace('\\', "/"));
        // beatoraja は保存済み filepath を候補内に存在するか検証せず尊重する。
        // BMZ 旧版の相対パス保存も含め、空でなければここでは置き換えない。
        if current.as_ref().is_some_and(|selected| !selected.is_empty()) {
            continue;
        }
        if let Some(default) = filepath_default(filepath, &candidates) {
            if current.as_deref() != Some(default.as_str()) {
                files.insert(filepath.name.clone(), default);
                changed = true;
            }
        } else if current.as_deref() != Some("") {
            files.insert(filepath.name.clone(), String::new());
            changed = true;
        }
    }
    changed
}

pub(super) fn add_offset_drag_values(
    ui: &mut egui::Ui,
    def: &SkinOffsetDef,
    value: &mut SkinOffsetConfig,
    text: Localizer,
) -> bool {
    let mut changed = false;
    let mut any = false;
    if def.x {
        changed |= ui.add(egui::DragValue::new(&mut value.x).prefix("x:")).changed();
        any = true;
    }
    if def.y {
        changed |= ui.add(egui::DragValue::new(&mut value.y).prefix("y:")).changed();
        any = true;
    }
    if def.w {
        changed |= ui.add(egui::DragValue::new(&mut value.w).prefix("w:")).changed();
        any = true;
    }
    if def.h {
        changed |= ui.add(egui::DragValue::new(&mut value.h).prefix("h:")).changed();
        any = true;
    }
    if def.r {
        changed |= ui.add(egui::DragValue::new(&mut value.r).prefix("r:")).changed();
        any = true;
    }
    if def.a {
        changed |= ui.add(egui::DragValue::new(&mut value.a).prefix("a:")).changed();
        any = true;
    }
    if !any {
        ui.label(tr!(text, "skin-offset-no-adjustable-values"));
    }
    changed
}

/// スキンパス文字列からスキンルートディレクトリ (親ディレクトリ) を得る。
pub(super) fn skin_root_path(app_paths: &AppPaths, skin_path: &str) -> Option<PathBuf> {
    let trimmed = skin_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = app_paths.resolve_path_ref(trimmed).ok()?;
    if path.is_dir() { Some(path) } else { path.parent().map(Path::to_path_buf) }
}

/// `pattern` (スキンルート相対、末尾要素にワイルドカード `*` を 1 個まで) に
/// マッチするファイルの相対パス一覧を返す。
///
/// beatoraja の `path|filter|` 形式の `|...|` 接尾辞 (lanecover などの
/// アセット用途タグ) は対象ファイル名には含まれないので、列挙前に取り除く。
pub(super) fn glob_candidates(root: &Path, pattern: &str) -> Vec<String> {
    let pattern = pattern.replace('\\', "/");
    let pattern = pattern.split_once('|').map_or(pattern.as_str(), |(path, _)| path).to_string();
    let (dir_part, name_part) = match pattern.rfind('/') {
        Some(index) => (&pattern[..=index], &pattern[index + 1..]),
        None => ("", pattern.as_str()),
    };
    let Some((prefix, suffix)) = name_part.split_once('*') else {
        // ワイルドカード無し: パターンそのものを唯一の候補とする。
        return vec![pattern.clone()];
    };
    let dir = root.join(dir_part);
    let mut candidates = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.len() >= prefix.len() + suffix.len()
                && name.starts_with(prefix)
                && name.ends_with(suffix)
            {
                candidates.push(format!("{dir_part}{name}"));
            }
        }
    }
    candidates.sort();
    candidates
}

pub(super) fn normalize_filepath_selection(
    selected: &str,
    candidates: &[String],
) -> Option<String> {
    if selected.is_empty() || selected == RANDOM_FILE_SELECTION {
        return None;
    }
    let normalized = selected.replace('\\', "/");
    if candidates.iter().any(|candidate| candidate == &normalized) {
        return (normalized != selected).then_some(normalized);
    }
    if normalized.contains('/') {
        return None;
    }
    candidates
        .iter()
        .find(|candidate| {
            filepath_selection_label(candidate).eq_ignore_ascii_case(normalized.as_str())
        })
        .cloned()
}

pub(super) fn filepath_selection_label(value: &str) -> &str {
    let slash = value.rfind('/').into_iter().chain(value.rfind('\\')).max();
    match slash {
        Some(index) if index + 1 < value.len() => &value[index + 1..],
        _ => value,
    }
}

/// property の既定選択肢名。beatoraja と同じく `def` が item name と一致する
/// ときだけ採用し、未指定/不一致なら先頭 item を使う。
pub(super) fn property_default(prop: &SkinPropertyDef) -> String {
    prop.item
        .iter()
        .find(|item| !prop.def.is_empty() && item.name == prop.def)
        .or_else(|| prop.item.first())
        .map(|item| item.name.clone())
        .unwrap_or_default()
}

pub(super) fn property_selection_is_valid(prop: &SkinPropertyDef, selected: &str) -> bool {
    if let Ok(op) = selected.parse::<i32>() {
        return prop.item.iter().any(|item| item.op == op);
    }
    prop.item.iter().any(|item| item.name == selected)
}

pub(super) fn filepath_default(
    filepath: &SkinFilepathDef,
    candidates: &[String],
) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    // def が "Random" のときは具体ファイルへ固定せず、ランダム番兵を既定にする
    // (beatoraja の def="Random" 相当)。
    if filepath.def.eq_ignore_ascii_case(RANDOM_FILE_SELECTION) {
        return Some(RANDOM_FILE_SELECTION.to_string());
    }
    if !filepath.def.is_empty()
        && let Some(candidate) =
            candidates.iter().find(|candidate| filename_matches_def(candidate, &filepath.def))
    {
        return Some(candidate.clone());
    }
    if filepath.def.is_empty()
        && let Some(candidate) =
            candidates.iter().find(|candidate| filename_matches_def(candidate, "default"))
    {
        return Some(candidate.clone());
    }
    candidates.first().cloned()
}

pub(super) fn filename_matches_def(candidate: &str, def: &str) -> bool {
    let file_name = Path::new(candidate).file_name().and_then(|name| name.to_str()).unwrap_or("");
    if file_name.eq_ignore_ascii_case(def) {
        return true;
    }
    let stem = Path::new(file_name).file_stem().and_then(|stem| stem.to_str()).unwrap_or(file_name);
    if stem.eq_ignore_ascii_case(def) {
        return true;
    }
    filepath_def_acronym(def).is_some_and(|acronym| {
        let stem_lower = stem.to_ascii_lowercase();
        let acronym_lower = acronym.to_ascii_lowercase();
        stem_lower == acronym_lower || stem_lower.starts_with(&acronym_lower)
    })
}

pub(super) fn filepath_def_acronym(def: &str) -> Option<String> {
    if !def.contains('-') {
        return None;
    }
    let acronym = def
        .split('-')
        .filter_map(|part| part.chars().find(|ch| ch.is_ascii_alphanumeric()))
        .collect::<String>();
    (!acronym.is_empty()).then_some(acronym)
}
