use super::*;

pub(super) fn build_select_course_builder_panel(
    ctx: &egui::Context,
    data: &mut SelectCourseBuilderData<'_>,
    text: Localizer,
) -> Option<SelectCourseBuilderAction> {
    let mut action = None;
    egui::Window::new(text.text("select-course-builder-title"))
        .id(egui::Id::new("bmz_select_course_builder"))
        .default_pos(egui::pos2(24.0, 24.0))
        .default_width(440.0)
        .collapsible(false)
        .resizable(true)
        .constrain_to(ctx.content_rect().shrink(PANEL_VIEWPORT_MARGIN))
        .show(ctx, |ui| {
            ui.label(text.text("select-course-builder-help"));
            ui.separator();
            egui::Grid::new("select_course_builder_identity").num_columns(2).show(ui, |ui| {
                ui.label(text.text("course-editor-name"));
                ui.text_edit_singleline(&mut data.definition.title);
                ui.end_row();
                ui.label(text.text("select-course-builder-mode"));
                ui.label(data.key_mode.map(bmz_core::lane::KeyMode::as_str).unwrap_or("-"));
                ui.end_row();
            });
            ui.separator();

            let mut args = FluentArgs::new();
            args.set("count", data.definition.entries.len() as i64);
            args.set("max", data.max_entries as i64);
            ui.heading(text.format("select-course-builder-entries", &args));
            if data.definition.entries.is_empty() {
                ui.weak(text.text("select-course-builder-empty"));
            } else {
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    for (index, entry) in data.definition.entries.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}.", index + 1));
                            ui.label(&entry.title_hint);
                            if ui.small_button("×").clicked() {
                                action = Some(SelectCourseBuilderAction::Remove(index));
                            }
                        });
                    }
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !data.definition.entries.is_empty(),
                        egui::Button::new(text.text("select-course-builder-undo")),
                    )
                    .clicked()
                {
                    action = Some(SelectCourseBuilderAction::Undo);
                }
                if ui
                    .add_enabled(
                        !data.definition.entries.is_empty(),
                        egui::Button::new(text.text("course-editor-save")),
                    )
                    .clicked()
                {
                    action = Some(SelectCourseBuilderAction::Save);
                }
                if ui.button(text.text("select-course-builder-cancel")).clicked() {
                    action = Some(SelectCourseBuilderAction::Cancel);
                }
            });
        });
    action
}
