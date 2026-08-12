use std::path::{Path, PathBuf};

use bmz_core::course::{
    CourseClassConstraint, CourseConstraints, CourseDefinition, CourseEntry, CourseGaugeConstraint,
    CourseJudgeConstraint, CourseKind, CourseLnConstraint, CourseSpeedConstraint, CourseTrophy,
};

use super::{
    CourseEditorAction, CourseEditorData, CourseEditorUiState, Localizer, PANEL_VIEWPORT_MARGIN,
};

const LOCAL_COURSE_SOURCE: &str = "bmz:local";

pub(super) fn build_course_editor_panel(
    ctx: &egui::Context,
    open: &mut bool,
    state: &mut CourseEditorUiState,
    data: &CourseEditorData,
    profile_root: &Path,
    editable: bool,
    text: Localizer,
) -> Option<CourseEditorAction> {
    if !*open {
        return None;
    }
    if state.export_path.is_empty() {
        state.export_path = profile_root.join("courses.json").display().to_string();
    }
    if state.import_path.is_empty() {
        state.import_path = profile_root.join("courses.json").display().to_string();
    }

    let mut action = None;
    egui::Window::new(text.text("course-editor-title"))
        .id(egui::Id::new("bmz_course_editor"))
        .open(open)
        .default_pos(egui::pos2(420.0, 24.0))
        .default_size(egui::vec2(760.0, 760.0))
        .constrain_to(ctx.content_rect().shrink(PANEL_VIEWPORT_MARGIN))
        .show(ctx, |ui| {
            if !editable {
                ui.colored_label(
                    egui::Color32::LIGHT_YELLOW,
                    text.text("course-editor-select-only"),
                );
                return;
            }

            build_course_picker(ui, state, data, text);
            let Some(draft) = state.draft.as_mut() else {
                return;
            };
            let is_new = state.selected_course_id.is_none();
            let mut reset_draft = false;

            egui::ScrollArea::vertical().show(ui, |ui| {
                build_identity_editor(ui, draft, is_new, text);
                ui.separator();
                build_constraints_editor(ui, draft, text);
                ui.separator();
                build_entry_editor(ui, draft, &mut state.search_query, data, text);
                ui.separator();
                build_trophy_editor(ui, draft, text);
                ui.separator();

                ui.horizontal_wrapped(|ui| {
                    if ui.button(text.text("course-editor-save")).clicked() {
                        action =
                            Some(CourseEditorAction::Save(normalize_definition(draft.clone())));
                    }
                    if ui.button(text.text("course-editor-test-play")).clicked() {
                        action = Some(CourseEditorAction::SaveAndTest(normalize_definition(
                            draft.clone(),
                        )));
                    }
                    let local_selected = state.selected_course_id.is_some_and(|id| {
                        data.courses
                            .iter()
                            .any(|course| course.id == id && course.source == LOCAL_COURSE_SOURCE)
                    });
                    if ui
                        .add_enabled(
                            local_selected,
                            egui::Button::new(text.text("course-editor-delete")),
                        )
                        .clicked()
                        && let Some(id) = state.selected_course_id
                    {
                        action = Some(CourseEditorAction::Delete(id));
                        reset_draft = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(text.text("course-editor-export-path"));
                    ui.text_edit_singleline(&mut state.export_path);
                    if ui.button(text.text("course-editor-export")).clicked() {
                        action = Some(CourseEditorAction::Export {
                            path: PathBuf::from(state.export_path.trim()),
                            definition: normalize_definition(draft.clone()),
                        });
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(text.text("course-editor-import-path"));
                    ui.text_edit_singleline(&mut state.import_path);
                    if ui.button(text.text("course-editor-import")).clicked() {
                        action = Some(CourseEditorAction::Import {
                            path: PathBuf::from(state.import_path.trim()),
                        });
                    }
                });
                if !state.status.is_empty() {
                    let color = if state.error {
                        egui::Color32::LIGHT_RED
                    } else {
                        egui::Color32::LIGHT_GREEN
                    };
                    ui.colored_label(color, &state.status);
                }
            });
            if reset_draft {
                state.selected_course_id = None;
                state.draft = None;
            }
        });
    action
}

fn build_course_picker(
    ui: &mut egui::Ui,
    state: &mut CourseEditorUiState,
    data: &CourseEditorData,
    text: Localizer,
) {
    let selected_title = state
        .selected_course_id
        .and_then(|id| data.courses.iter().find(|course| course.id == id))
        .map(|course| course.definition.title.clone())
        .unwrap_or_else(|| text.text("course-editor-new"));
    let mut selected = None;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("course_editor_course")
            .selected_text(&selected_title)
            .width(360.0)
            .show_ui(ui, |ui| {
                for course in &data.courses {
                    let source =
                        if course.source == LOCAL_COURSE_SOURCE { "LOCAL" } else { "IMPORT" };
                    if ui
                        .selectable_label(
                            state.selected_course_id == Some(course.id),
                            format!("[{}] {}", source, course.definition.title),
                        )
                        .clicked()
                    {
                        selected = Some(course.id);
                    }
                }
            });
        if ui.button(text.text("course-editor-new")).clicked() {
            state.selected_course_id = None;
            state.draft = Some(new_definition(&data.courses));
            state.status.clear();
        }
    });
    if let Some(id) = selected
        && let Some(course) = data.courses.iter().find(|course| course.id == id)
    {
        state.selected_course_id = Some(id);
        state.draft = Some(course.definition.clone());
        state.status.clear();
    }
    if state.draft.is_none() {
        state.draft = Some(new_definition(&data.courses));
    }
}

fn build_identity_editor(
    ui: &mut egui::Ui,
    draft: &mut CourseDefinition,
    key_editable: bool,
    text: Localizer,
) {
    egui::Grid::new("course_editor_identity").num_columns(2).show(ui, |ui| {
        ui.label(text.text("course-editor-name"));
        ui.text_edit_singleline(&mut draft.title);
        ui.end_row();
        ui.label(text.text("course-editor-key"));
        ui.add_enabled(key_editable, egui::TextEdit::singleline(&mut draft.key));
        ui.end_row();
        ui.label(text.text("course-editor-kind"));
        let old_kind = draft.kind;
        egui::ComboBox::from_id_salt("course_editor_kind")
            .selected_text(kind_label(draft.kind))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut draft.kind, CourseKind::Course, "COURSE");
                ui.selectable_value(&mut draft.kind, CourseKind::Dan, "DAN / GRADE");
            });
        if old_kind != draft.kind {
            draft.constraints.class = match draft.kind {
                CourseKind::Course => CourseClassConstraint::None,
                CourseKind::Dan => CourseClassConstraint::Grade,
            };
        }
        ui.end_row();
        ui.label(text.text("course-editor-release"));
        ui.checkbox(&mut draft.release, "");
        ui.end_row();
    });
}

fn build_constraints_editor(ui: &mut egui::Ui, draft: &mut CourseDefinition, text: Localizer) {
    ui.heading(text.text("course-editor-constraints"));
    egui::Grid::new("course_editor_constraints").num_columns(2).striped(true).show(ui, |ui| {
        ui.label("CLASS");
        combo_class(ui, &mut draft.constraints.class);
        ui.end_row();
        ui.label("SPEED");
        combo_speed(ui, &mut draft.constraints.speed);
        ui.end_row();
        ui.label("JUDGE");
        combo_judge(ui, &mut draft.constraints.judge);
        ui.end_row();
        ui.label("GAUGE");
        combo_gauge(ui, &mut draft.constraints.gauge);
        ui.end_row();
        ui.label("LN");
        combo_ln(ui, &mut draft.constraints.ln);
        ui.end_row();
    });
    draft.kind = CourseDefinition::derive_kind_from_constraints(&draft.constraints);
}

fn build_entry_editor(
    ui: &mut egui::Ui,
    draft: &mut CourseDefinition,
    search_query: &mut String,
    data: &CourseEditorData,
    text: Localizer,
) {
    ui.heading(text.text("course-editor-entries"));
    let mut move_entry = None;
    let mut remove_entry = None;
    for (index, entry) in draft.entries.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("{}.", index + 1));
            ui.label(&entry.title_hint);
            if ui.small_button("↑").clicked() && index > 0 {
                move_entry = Some((index, index - 1));
            }
            if ui.small_button("↓").clicked() && index + 1 < draft.entries.len() {
                move_entry = Some((index, index + 1));
            }
            if ui.small_button("×").clicked() {
                remove_entry = Some(index);
            }
        });
    }
    if let Some((from, to)) = move_entry {
        draft.entries.swap(from, to);
    }
    if let Some(index) = remove_entry {
        draft.entries.remove(index);
    }

    ui.horizontal(|ui| {
        ui.label(text.text("course-editor-chart-search"));
        ui.text_edit_singleline(search_query);
    });
    egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
        for chart in &data.charts {
            ui.horizontal(|ui| {
                if ui.small_button("+").clicked() {
                    draft.entries.push(CourseEntry {
                        title_hint: chart.title.clone(),
                        md5: Some(chart.md5.clone()),
                        sha256: Some(chart.sha256.clone()),
                        chart_id: Some(chart.chart_id),
                    });
                }
                ui.label(format!(
                    "{} / {}  ☆{}  [{}]",
                    chart.title, chart.artist, chart.play_level, chart.mode
                ));
            });
        }
    });
}

fn build_trophy_editor(ui: &mut egui::Ui, draft: &mut CourseDefinition, text: Localizer) {
    ui.heading(text.text("course-editor-trophies"));
    let mut remove = None;
    for (index, trophy) in draft.trophies.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut trophy.name);
            ui.label("MISS ≤");
            ui.add(egui::DragValue::new(&mut trophy.max_miss_rate).range(0.0..=100.0).suffix("%"));
            ui.label("SCORE ≥");
            ui.add(egui::DragValue::new(&mut trophy.min_score_rate).range(0.0..=100.0).suffix("%"));
            if ui.small_button("×").clicked() {
                remove = Some(index);
            }
        });
    }
    if let Some(index) = remove {
        draft.trophies.remove(index);
    }
    if ui.button(text.text("course-editor-add-trophy")).clicked() {
        draft.trophies.push(CourseTrophy {
            name: format!("trophy{}", draft.trophies.len() + 1),
            max_miss_rate: 5.0,
            min_score_rate: 70.0,
        });
    }
}

fn new_definition(courses: &[crate::storage::library_db::StoredCourse]) -> CourseDefinition {
    let key = (1..)
        .map(|index| format!("local-course-{index}"))
        .find(|candidate| {
            courses.iter().all(|course| course.definition.key.as_str() != candidate.as_str())
        })
        .expect("course key sequence is finite in practice");
    CourseDefinition {
        key,
        title: "New Course".to_string(),
        kind: CourseKind::Course,
        entries: Vec::new(),
        constraints: CourseConstraints::default(),
        trophies: Vec::new(),
        release: true,
    }
}

fn normalize_definition(mut definition: CourseDefinition) -> CourseDefinition {
    definition.title = definition.title.trim().to_string();
    if definition.title.is_empty() {
        definition.title = "No Course Title".to_string();
    }
    definition.key = definition.key.trim().to_string();
    if definition.key.is_empty() {
        definition.key = definition.title.to_ascii_lowercase().replace(' ', "-");
    }
    definition.kind = CourseDefinition::derive_kind_from_constraints(&definition.constraints);
    definition.constraints.source_constraints = constraint_names(&definition.constraints);
    definition
}

pub(crate) fn constraint_names(constraints: &CourseConstraints) -> Vec<String> {
    let mut names = Vec::new();
    match constraints.class {
        CourseClassConstraint::None => {}
        CourseClassConstraint::Grade => names.push("grade"),
        CourseClassConstraint::GradeMirrorAllowed => names.push("grade_mirror"),
        CourseClassConstraint::GradeRandomAllowed => names.push("grade_random"),
    }
    if constraints.speed == CourseSpeedConstraint::NoSpeed {
        names.push("no_speed");
    }
    match constraints.judge {
        CourseJudgeConstraint::Normal => {}
        CourseJudgeConstraint::NoGood => names.push("no_good"),
        CourseJudgeConstraint::NoGreat => names.push("no_great"),
    }
    match constraints.gauge {
        CourseGaugeConstraint::Default => {}
        CourseGaugeConstraint::Lr2 => names.push("gauge_lr2"),
        CourseGaugeConstraint::Keys5 => names.push("gauge_5k"),
        CourseGaugeConstraint::Keys7 => names.push("gauge_7k"),
        CourseGaugeConstraint::Keys9 => names.push("gauge_9k"),
        CourseGaugeConstraint::Keys24 => {}
    }
    match constraints.ln {
        CourseLnConstraint::Default => {}
        CourseLnConstraint::Ln => names.push("ln"),
        CourseLnConstraint::Cn => names.push("cn"),
        CourseLnConstraint::Hcn => names.push("hcn"),
    }
    names.into_iter().map(str::to_string).collect()
}

fn kind_label(kind: CourseKind) -> &'static str {
    match kind {
        CourseKind::Course => "COURSE",
        CourseKind::Dan => "DAN / GRADE",
    }
}

macro_rules! enum_combo {
    ($name:ident, $id:literal, $ty:ty, [$($value:path => $label:literal),+ $(,)?]) => {
        fn $name(ui: &mut egui::Ui, current: &mut $ty) {
            let selected = match *current { $($value => $label),+ };
            egui::ComboBox::from_id_salt($id).selected_text(selected).show_ui(ui, |ui| {
                $(ui.selectable_value(current, $value, $label);)+
            });
        }
    };
}

enum_combo!(combo_class, "course_class", CourseClassConstraint, [
    CourseClassConstraint::None => "NONE",
    CourseClassConstraint::Grade => "GRADE",
    CourseClassConstraint::GradeMirrorAllowed => "GRADE MIRROR",
    CourseClassConstraint::GradeRandomAllowed => "GRADE RANDOM",
]);
enum_combo!(combo_speed, "course_speed", CourseSpeedConstraint, [
    CourseSpeedConstraint::Free => "FREE",
    CourseSpeedConstraint::NoSpeed => "NO SPEED",
]);
enum_combo!(combo_judge, "course_judge", CourseJudgeConstraint, [
    CourseJudgeConstraint::Normal => "NORMAL",
    CourseJudgeConstraint::NoGood => "NO GOOD",
    CourseJudgeConstraint::NoGreat => "NO GREAT",
]);
fn combo_gauge(ui: &mut egui::Ui, current: &mut CourseGaugeConstraint) {
    let selected = match *current {
        CourseGaugeConstraint::Default => "DEFAULT",
        CourseGaugeConstraint::Lr2 => "LR2",
        CourseGaugeConstraint::Keys5 => "5KEYS",
        CourseGaugeConstraint::Keys7 => "7KEYS",
        CourseGaugeConstraint::Keys9 => "9KEYS",
        CourseGaugeConstraint::Keys24 => "UNSUPPORTED 24KEYS",
    };
    egui::ComboBox::from_id_salt("course_gauge").selected_text(selected).show_ui(ui, |ui| {
        ui.selectable_value(current, CourseGaugeConstraint::Default, "DEFAULT");
        ui.selectable_value(current, CourseGaugeConstraint::Lr2, "LR2");
        ui.selectable_value(current, CourseGaugeConstraint::Keys5, "5KEYS");
        ui.selectable_value(current, CourseGaugeConstraint::Keys7, "7KEYS");
        ui.selectable_value(current, CourseGaugeConstraint::Keys9, "9KEYS");
    });
}
enum_combo!(combo_ln, "course_ln", CourseLnConstraint, [
    CourseLnConstraint::Default => "DEFAULT",
    CourseLnConstraint::Ln => "LN",
    CourseLnConstraint::Cn => "CN",
    CourseLnConstraint::Hcn => "HCN",
]);
