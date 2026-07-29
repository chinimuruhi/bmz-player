/// 1 シーン分のスキン設定可能項目を折りたたみ表示・編集する。
///
/// - property: ComboBox で選択肢を選び `options` へ書き込む。
/// - filepath: `path` グロブにマッチするファイルを ComboBox で選び `files` へ書き込む。
/// - offset: 宣言された要素ごとに x/y/w/h/r/a を編集し `offsets` (名前単位) へ反映。
pub(in crate::ui) fn build_scene_skin_defs(
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
pub(in crate::ui) fn sync_skin_offsets_with_defs(
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
pub(in crate::ui) fn update_skin_offset_value(
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
pub(in crate::ui) fn reset_scene_skin_to_defaults(
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

pub(in crate::ui) fn fill_missing_skin_defaults(
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

pub(in crate::ui) fn add_offset_drag_values(
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
use super::*;
