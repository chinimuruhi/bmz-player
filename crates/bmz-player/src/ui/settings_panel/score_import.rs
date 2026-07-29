use super::*;

pub(in crate::ui) fn build_score_import_section(
    ui: &mut egui::Ui,
    path: &mut String,
    kind: &mut ScoreImportKind,
    device_type: &mut InputDeviceKind,
    status: &str,
    error: &str,
    request: &mut Option<ScoreImportRequest>,
    text: Localizer,
) {
    egui::CollapsingHeader::new(tr!(text, "settings-score-import-title"))
        .id_salt("settings_score_import")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("DB");
                ui.add(
                    egui::TextEdit::singleline(path)
                        .desired_width(260.0)
                        .hint_text("score.db / scoredatalog.db / LR2 score db"),
                );
            });
            ui.horizontal(|ui| {
                if ui.button(tr!(text, "common-choose-file")).clicked()
                    && let Some(file) =
                        rfd::FileDialog::new().add_filter("SQLite DB", &["db"]).pick_file()
                {
                    *path = file.to_string_lossy().into_owned();
                }
                egui::ComboBox::from_id_salt("score_import_kind")
                    .selected_text(kind.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            kind,
                            ScoreImportKind::Lr2,
                            ScoreImportKind::Lr2.label(),
                        );
                        ui.selectable_value(
                            kind,
                            ScoreImportKind::Beatoraja,
                            ScoreImportKind::Beatoraja.label(),
                        );
                        ui.selectable_value(
                            kind,
                            ScoreImportKind::Lr2Oraja,
                            ScoreImportKind::Lr2Oraja.label(),
                        );
                        ui.selectable_value(
                            kind,
                            ScoreImportKind::Lr2OrajaDx,
                            ScoreImportKind::Lr2OrajaDx.label(),
                        );
                    });
            });
            ui.horizontal(|ui| {
                ui.label(tr!(text, "settings-score-import-device"));
                ui.selectable_value(
                    device_type,
                    InputDeviceKind::Keyboard,
                    tr!(text, "settings-input-keyboard"),
                );
                ui.selectable_value(
                    device_type,
                    InputDeviceKind::Controller,
                    tr!(text, "settings-score-import-controller"),
                );
            });
            if ui.button(tr!(text, "settings-score-import-button")).clicked() {
                let trimmed = path.trim();
                if trimmed.is_empty() {
                    *request = None;
                } else {
                    *request = Some(ScoreImportRequest {
                        path: PathBuf::from(trimmed),
                        kind: *kind,
                        device_type: *device_type,
                    });
                }
            }
            if !status.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_GREEN, status);
            }
            if !error.is_empty() {
                ui.colored_label(egui::Color32::RED, error);
            }
        });
}
