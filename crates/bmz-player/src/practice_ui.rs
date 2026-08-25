//! egui overlay for practice configuration (pre-play).

use egui::{Context, RichText};

use crate::i18n::Localizer;
use crate::screens::practice::{PracticeGaugeType, PracticeGraphType, PracticeProperty};
use crate::select_options::ArrangeOption;
use bmz_gameplay::gauge::GaugeProperty;
use bmz_render::snapshot::ResultGraphSnapshot;

pub struct PracticePanelContext<'a> {
    pub property: &'a mut PracticeProperty,
    pub graph: &'a ResultGraphSnapshot,
    pub graph_start_time_ms: u32,
    pub is_double: bool,
    pub cursor: &'a mut usize,
    pub chart_title: &'a str,
    pub media_ready: bool,
    pub max_end_time_ms: u32,
    /// Surface 左上原点の正規化座標。beatoraja skin の practice destination 由来。
    pub default_position: Option<(f32, f32)>,
}

pub struct PracticePanelOutput {
    pub start_play: bool,
    pub leave: bool,
}

pub fn build_practice_panel(
    ctx: &Context,
    practice: &mut PracticePanelContext<'_>,
    text: Localizer,
) -> PracticePanelOutput {
    let mut start_play = false;
    let mut leave = false;
    let field_count = crate::screens::practice::practice_field_count(practice.is_double);
    if ctx.input(|input| input.key_pressed(egui::Key::ArrowDown)) {
        *practice.cursor = (*practice.cursor + 1) % field_count;
    }
    if ctx.input(|input| input.key_pressed(egui::Key::ArrowUp)) {
        *practice.cursor = (*practice.cursor + field_count - 1) % field_count;
    }
    let decrement = ctx.input(|input| input.key_pressed(egui::Key::ArrowLeft));
    let increment = ctx.input(|input| input.key_pressed(egui::Key::ArrowRight));
    if decrement || increment {
        crate::screens::practice::adjust_practice_selected_field(
            practice.property,
            *practice.cursor,
            practice.is_double,
            increment,
            practice.max_end_time_ms,
        );
    }

    let mut window = egui::Window::new(text.text("practice-title"))
        .id(egui::Id::new("practice_config_panel"))
        .order(egui::Order::Foreground)
        .movable(true)
        .resizable(false)
        .collapsible(false)
        .default_width(360.0)
        .frame(
            egui::Frame::window(ctx.global_style().as_ref())
                .fill(egui::Color32::from_rgba_unmultiplied(16, 20, 32, 230)),
        );
    let screen = ctx.content_rect();
    let default_position = practice
        .default_position
        .map(|(x, y)| {
            egui::pos2(screen.left() + x * screen.width(), screen.top() + y * screen.height())
        })
        .unwrap_or_else(|| egui::pos2(screen.left() + 12.0, screen.top() + 12.0));
    window = window.default_pos(default_position);
    window.show(ctx, |ui| {
        ui.set_min_width(360.0);
        ui.label(RichText::new(practice.chart_title).weak());
        ui.separator();

        practice_field_label(ui, practice.cursor, 0, text.text("practice-start-time"));

        ui.horizontal(|ui| {
            time_ms_field(
                ui,
                &mut practice.property.start_time_ms,
                practice.max_end_time_ms.saturating_sub(3000),
            );
        });
        practice_field_label(ui, practice.cursor, 1, text.text("practice-end-time"));
        ui.horizontal(|ui| {
            time_ms_field(ui, &mut practice.property.end_time_ms, practice.max_end_time_ms);
        });

        practice_field_label(ui, practice.cursor, 2, text.text("practice-gauge"));
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("practice_gauge")
                .selected_text(gauge_label(text, practice.property.gauge))
                .show_ui(ui, |ui| {
                    for gauge in practice_gauges() {
                        ui.selectable_value(
                            &mut practice.property.gauge,
                            gauge,
                            gauge_label(text, gauge),
                        );
                    }
                });
        });
        practice_field_label(ui, practice.cursor, 3, text.text("practice-gauge-category"));
        ui.horizontal(|ui| {
            let category = practice.property.gauge_category.get_or_insert(GaugeProperty::SevenKeys);
            let previous_category = *category;
            egui::ComboBox::from_id_salt("practice_gauge_category")
                .selected_text(gauge_category_label(*category))
                .show_ui(ui, |ui| {
                    for value in practice_gauge_categories() {
                        ui.selectable_value(category, value, gauge_category_label(value));
                    }
                });
            let selected_category = *category;
            if selected_category != previous_category {
                practice.property.start_gauge =
                    crate::screens::practice::practice_gauge_initial_value(
                        practice.property.gauge,
                        selected_category,
                    );
            }
        });
        practice_field_label(ui, practice.cursor, 4, text.text("practice-gauge-percent"));
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut practice.property.start_gauge).range(1..=100).speed(0.2),
            );
        });
        practice_field_label(ui, practice.cursor, 5, text.text("practice-judge-rank"));
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut practice.property.judgerank).range(1..=400).speed(0.5),
            );
        });
        practice_field_label(ui, practice.cursor, 6, text.text("practice-total"));
        if let Some(total) = practice.property.total.as_mut() {
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(total).range(10.0..=5000.0).speed(1.0));
            });
        }
        practice_field_label(ui, practice.cursor, 7, text.text("practice-frequency"));
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut practice.property.playback_rate_percent)
                    .range(50..=200)
                    .suffix(" %"),
            );
        });
        practice_field_label(ui, practice.cursor, 8, text.text("practice-graph-type"));
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("practice_graph_type")
                .selected_text(graph_type_label(practice.property.graph_type))
                .show_ui(ui, |ui| {
                    for graph_type in [
                        PracticeGraphType::NoteType,
                        PracticeGraphType::Judge,
                        PracticeGraphType::EarlyLate,
                    ] {
                        ui.selectable_value(
                            &mut practice.property.graph_type,
                            graph_type,
                            graph_type_label(graph_type),
                        );
                    }
                });
        });
        practice_field_label(ui, practice.cursor, 9, text.text("practice-arrange"));
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("practice_arrange")
                .selected_text(arrange_label(text, practice.property.arrange))
                .show_ui(ui, |ui| {
                    for arrange in ArrangeOption::VALUES {
                        ui.selectable_value(
                            &mut practice.property.arrange,
                            arrange,
                            arrange_label(text, arrange),
                        );
                    }
                });
        });
        if practice.is_double {
            practice_field_label(ui, practice.cursor, 10, text.text("practice-arrange-2p"));
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("practice_arrange_2p")
                    .selected_text(arrange_label(text, practice.property.arrange_2p))
                    .show_ui(ui, |ui| {
                        for arrange in ArrangeOption::VALUES {
                            ui.selectable_value(
                                &mut practice.property.arrange_2p,
                                arrange,
                                arrange_label(text, arrange),
                            );
                        }
                    });
            });
            practice_field_label(ui, practice.cursor, 11, text.text("practice-dp-option"));
            ui.checkbox(&mut practice.property.dp_flip, "FLIP");
        }

        let last_field = if practice.is_double { 11 } else { 9 };
        *practice.cursor = (*practice.cursor).min(last_field);
        draw_practice_graph(ui, practice);

        ui.separator();
        if practice.media_ready {
            ui.colored_label(egui::Color32::LIGHT_GREEN, text.text("practice-ready-hint"));
        } else {
            ui.colored_label(egui::Color32::YELLOW, text.text("practice-media-loading"));
        }

        ui.horizontal(|ui| {
            if ui.button(text.text("practice-start-play")).clicked() {
                start_play = true;
            }
            if ui.button(text.text("practice-back-to-select")).clicked() {
                leave = true;
            }
        });
    });

    if ctx.input(|input| input.key_pressed(egui::Key::Enter)) && practice.media_ready {
        start_play = true;
    }
    if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        leave = true;
    }

    PracticePanelOutput { start_play, leave }
}

fn time_ms_field(ui: &mut egui::Ui, value: &mut u32, max_ms: u32) {
    let mut ms = i64::from(*value);
    if ui.add(egui::DragValue::new(&mut ms).range(0..=i64::from(max_ms)).speed(50.0)).changed() {
        *value = u32::try_from(ms).unwrap_or(max_ms);
    }
    ui.label(format_time_ms(*value));
}

fn format_time_ms(ms: u32) -> String {
    let minutes = ms / 60_000;
    let seconds = (ms / 1000) % 60;
    let tenths = (ms / 100) % 10;
    format!("{minutes:02}:{seconds:02}.{tenths}")
}

fn practice_gauges() -> [PracticeGaugeType; 9] {
    PracticeGaugeType::VALUES
}

fn gauge_label(text: Localizer, gauge: PracticeGaugeType) -> String {
    text.text(match gauge {
        PracticeGaugeType::AssistEasy => "practice-gauge-assist-easy",
        PracticeGaugeType::Easy => "practice-gauge-easy",
        PracticeGaugeType::Normal => "practice-gauge-normal",
        PracticeGaugeType::Hard => "practice-gauge-hard",
        PracticeGaugeType::ExHard => "practice-gauge-ex-hard",
        PracticeGaugeType::Hazard => "practice-gauge-hazard",
        PracticeGaugeType::Class => "practice-gauge-class",
        PracticeGaugeType::ExClass => "practice-gauge-ex-class",
        PracticeGaugeType::ExHardClass => "practice-gauge-ex-hard-class",
        PracticeGaugeType::AutoShift => "practice-gauge-auto-shift",
    })
}

fn practice_gauge_categories() -> [GaugeProperty; 4] {
    [GaugeProperty::FiveKeys, GaugeProperty::SevenKeys, GaugeProperty::Pms, GaugeProperty::Lr2]
}

fn gauge_category_label(category: GaugeProperty) -> &'static str {
    match category {
        GaugeProperty::FiveKeys => "5KEYS",
        GaugeProperty::SevenKeys => "7KEYS",
        GaugeProperty::Pms => "PMS",
        GaugeProperty::Keyboard => "KEYBOARD",
        GaugeProperty::Lr2 => "LR2",
    }
}

fn graph_type_label(graph_type: PracticeGraphType) -> &'static str {
    match graph_type {
        PracticeGraphType::NoteType => "NOTETYPE",
        PracticeGraphType::Judge => "JUDGE",
        PracticeGraphType::EarlyLate => "EARLYLATE",
    }
}

fn practice_field_label(ui: &mut egui::Ui, cursor: &mut usize, index: usize, label: String) {
    if ui.selectable_label(*cursor == index, label).clicked() {
        *cursor = index;
    }
}

fn draw_practice_graph(ui: &mut egui::Ui, practice: &PracticePanelContext<'_>) {
    let desired = egui::vec2(ui.available_width().max(320.0), 120.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(8, 12, 20));
    let buckets: Vec<[u32; 10]> = match practice.property.graph_type {
        PracticeGraphType::NoteType => practice
            .graph
            .note_graph_buckets
            .iter()
            .map(|bucket| {
                let mut values = [0; 10];
                values[..7].copy_from_slice(&bucket.values);
                values
            })
            .collect(),
        PracticeGraphType::Judge => practice
            .graph
            .judge_graph_buckets
            .iter()
            .map(|bucket| {
                let mut values = [0; 10];
                values[..6].copy_from_slice(&bucket.values);
                values
            })
            .collect(),
        PracticeGraphType::EarlyLate => {
            practice.graph.early_late_graph_buckets.iter().map(|bucket| bucket.values).collect()
        }
    };
    if buckets.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "NO DATA",
            egui::FontId::proportional(14.0),
            egui::Color32::GRAY,
        );
        return;
    }
    let start = (practice.property.start_time_ms.saturating_sub(practice.graph_start_time_ms)
        / 1000) as usize;
    let end = (practice
        .property
        .end_time_ms
        .saturating_sub(practice.graph_start_time_ms)
        .saturating_add(999)
        / 1000) as usize;
    let visible =
        &buckets[start.min(buckets.len())..end.min(buckets.len()).max(start.min(buckets.len()))];
    if visible.is_empty() {
        return;
    }
    let max_total =
        visible.iter().map(|bucket| bucket.iter().copied().sum::<u32>()).max().unwrap_or(1).max(1)
            as f32;
    let colors = [
        egui::Color32::from_rgb(90, 100, 120),
        egui::Color32::from_rgb(80, 210, 255),
        egui::Color32::from_rgb(255, 220, 80),
        egui::Color32::from_rgb(120, 220, 120),
        egui::Color32::from_rgb(255, 150, 70),
        egui::Color32::from_rgb(245, 80, 90),
        egui::Color32::from_rgb(210, 70, 210),
        egui::Color32::from_rgb(70, 160, 255),
        egui::Color32::from_rgb(255, 100, 180),
        egui::Color32::from_rgb(150, 80, 220),
    ];
    let width = rect.width() / visible.len() as f32;
    for (index, bucket) in visible.iter().enumerate() {
        let mut bottom = rect.bottom();
        for (state, count) in bucket.iter().enumerate() {
            if *count == 0 {
                continue;
            }
            let height = rect.height() * *count as f32 / max_total;
            let bar = egui::Rect::from_min_max(
                egui::pos2(rect.left() + index as f32 * width, bottom - height),
                egui::pos2(rect.left() + (index + 1) as f32 * width, bottom),
            );
            painter.rect_filled(bar, 0.0, colors[state]);
            bottom -= height;
        }
    }
}

fn arrange_label(text: Localizer, arrange: ArrangeOption) -> String {
    text.text(match arrange {
        ArrangeOption::Normal => "practice-arrange-normal",
        ArrangeOption::Mirror => "practice-arrange-mirror",
        ArrangeOption::Random => "practice-arrange-random",
        ArrangeOption::RRandom => "practice-arrange-r-random",
        ArrangeOption::SRandom => "practice-arrange-s-random",
        ArrangeOption::Spiral => "practice-arrange-spiral",
        ArrangeOption::HRandom => "practice-arrange-h-random",
        ArrangeOption::AllScratch => "practice-arrange-all-scratch",
        ArrangeOption::RandomEx => "practice-arrange-random-ex",
        ArrangeOption::SRandomEx => "practice-arrange-s-random-ex",
        ArrangeOption::FRandom => "practice-arrange-f-random",
        ArrangeOption::MFRandom => "practice-arrange-mf-random",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::AppLocale;

    #[test]
    fn practice_labels_resolve_for_every_locale() {
        let keys = [
            "practice-title",
            "practice-start-time",
            "practice-end-time",
            "practice-gauge",
            "practice-gauge-percent",
            "practice-judge-rank",
            "practice-arrange",
            "practice-total",
            "practice-ready-hint",
            "practice-media-loading",
            "practice-start-play",
            "practice-back-to-select",
        ];
        for locale in AppLocale::SUPPORTED {
            let text = Localizer::new(locale);
            for key in keys {
                assert_ne!(text.text(key), key, "{} is missing {key}", locale.code());
            }
            for gauge in practice_gauges() {
                assert!(!gauge_label(text, gauge).starts_with("practice-"));
            }
            for arrange in ArrangeOption::VALUES {
                assert!(!arrange_label(text, arrange).starts_with("practice-"));
            }
        }
    }

    #[test]
    fn time_format_is_locale_neutral() {
        assert_eq!(format_time_ms(0), "00:00.0");
        assert_eq!(format_time_ms(125_678), "02:05.6");
    }
}
