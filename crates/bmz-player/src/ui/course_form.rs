use bmz_core::course::{
    CourseClassConstraint, CourseDefinition, CourseGaugeConstraint, CourseJudgeConstraint,
    CourseLnConstraint, CourseSpeedConstraint, CourseTrophy,
};

use super::Localizer;

pub(super) fn build_course_constraints_editor(
    ui: &mut egui::Ui,
    draft: &mut CourseDefinition,
    text: Localizer,
    id_salt: &'static str,
) {
    ui.heading(text.text("course-editor-constraints"));
    egui::Grid::new((id_salt, "constraints")).num_columns(2).striped(true).show(ui, |ui| {
        ui.label("CLASS");
        combo_class(ui, &mut draft.constraints.class, (id_salt, "class"));
        ui.end_row();
        ui.label("SPEED");
        combo_speed(ui, &mut draft.constraints.speed, (id_salt, "speed"));
        ui.end_row();
        ui.label("JUDGE");
        combo_judge(ui, &mut draft.constraints.judge, (id_salt, "judge"));
        ui.end_row();
        ui.label("GAUGE");
        combo_gauge(ui, &mut draft.constraints.gauge, (id_salt, "gauge"));
        ui.end_row();
        ui.label("LN");
        combo_ln(ui, &mut draft.constraints.ln, (id_salt, "ln"));
        ui.end_row();
    });
    draft.kind = CourseDefinition::derive_kind_from_constraints(&draft.constraints);
}

pub(super) fn build_course_trophy_editor(
    ui: &mut egui::Ui,
    draft: &mut CourseDefinition,
    text: Localizer,
    id_salt: &'static str,
) {
    egui::CollapsingHeader::new(text.text("course-editor-trophies"))
        .id_salt((id_salt, "trophies"))
        .default_open(false)
        .show(ui, |ui| {
            let mut remove = None;
            for (index, trophy) in draft.trophies.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut trophy.name);
                    ui.label("MISS ≤");
                    ui.add(
                        egui::DragValue::new(&mut trophy.max_miss_rate)
                            .range(0.0..=100.0)
                            .suffix("%"),
                    );
                    ui.label("SCORE ≥");
                    ui.add(
                        egui::DragValue::new(&mut trophy.min_score_rate)
                            .range(0.0..=100.0)
                            .suffix("%"),
                    );
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
        });
}

fn combo_class(ui: &mut egui::Ui, current: &mut CourseClassConstraint, id: impl std::hash::Hash) {
    let selected = match *current {
        CourseClassConstraint::None => "NONE",
        CourseClassConstraint::Grade => "GRADE",
        CourseClassConstraint::GradeMirrorAllowed => "GRADE MIRROR",
        CourseClassConstraint::GradeRandomAllowed => "GRADE RANDOM",
    };
    egui::ComboBox::from_id_salt(id).selected_text(selected).show_ui(ui, |ui| {
        ui.selectable_value(current, CourseClassConstraint::None, "NONE");
        ui.selectable_value(current, CourseClassConstraint::Grade, "GRADE");
        ui.selectable_value(current, CourseClassConstraint::GradeMirrorAllowed, "GRADE MIRROR");
        ui.selectable_value(current, CourseClassConstraint::GradeRandomAllowed, "GRADE RANDOM");
    });
}

fn combo_speed(ui: &mut egui::Ui, current: &mut CourseSpeedConstraint, id: impl std::hash::Hash) {
    let selected = match *current {
        CourseSpeedConstraint::Free => "FREE",
        CourseSpeedConstraint::NoSpeed => "NO SPEED",
    };
    egui::ComboBox::from_id_salt(id).selected_text(selected).show_ui(ui, |ui| {
        ui.selectable_value(current, CourseSpeedConstraint::Free, "FREE");
        ui.selectable_value(current, CourseSpeedConstraint::NoSpeed, "NO SPEED");
    });
}

fn combo_judge(ui: &mut egui::Ui, current: &mut CourseJudgeConstraint, id: impl std::hash::Hash) {
    let selected = match *current {
        CourseJudgeConstraint::Normal => "NORMAL",
        CourseJudgeConstraint::NoGood => "NO GOOD",
        CourseJudgeConstraint::NoGreat => "NO GREAT",
    };
    egui::ComboBox::from_id_salt(id).selected_text(selected).show_ui(ui, |ui| {
        ui.selectable_value(current, CourseJudgeConstraint::Normal, "NORMAL");
        ui.selectable_value(current, CourseJudgeConstraint::NoGood, "NO GOOD");
        ui.selectable_value(current, CourseJudgeConstraint::NoGreat, "NO GREAT");
    });
}

fn combo_gauge(ui: &mut egui::Ui, current: &mut CourseGaugeConstraint, id: impl std::hash::Hash) {
    let selected = match *current {
        CourseGaugeConstraint::Default => "DEFAULT",
        CourseGaugeConstraint::Lr2 => "LR2",
        CourseGaugeConstraint::Keys5 => "5KEYS",
        CourseGaugeConstraint::Keys7 => "7KEYS",
        CourseGaugeConstraint::Keys9 => "9KEYS",
        CourseGaugeConstraint::Keys24 => "UNSUPPORTED 24KEYS",
    };
    egui::ComboBox::from_id_salt(id).selected_text(selected).show_ui(ui, |ui| {
        ui.selectable_value(current, CourseGaugeConstraint::Default, "DEFAULT");
        ui.selectable_value(current, CourseGaugeConstraint::Lr2, "LR2");
        ui.selectable_value(current, CourseGaugeConstraint::Keys5, "5KEYS");
        ui.selectable_value(current, CourseGaugeConstraint::Keys7, "7KEYS");
        ui.selectable_value(current, CourseGaugeConstraint::Keys9, "9KEYS");
    });
}

fn combo_ln(ui: &mut egui::Ui, current: &mut CourseLnConstraint, id: impl std::hash::Hash) {
    let selected = match *current {
        CourseLnConstraint::Default => "DEFAULT",
        CourseLnConstraint::Ln => "LN",
        CourseLnConstraint::Cn => "CN",
        CourseLnConstraint::Hcn => "HCN",
    };
    egui::ComboBox::from_id_salt(id).selected_text(selected).show_ui(ui, |ui| {
        ui.selectable_value(current, CourseLnConstraint::Default, "DEFAULT");
        ui.selectable_value(current, CourseLnConstraint::Ln, "LN");
        ui.selectable_value(current, CourseLnConstraint::Cn, "CN");
        ui.selectable_value(current, CourseLnConstraint::Hcn, "HCN");
    });
}
