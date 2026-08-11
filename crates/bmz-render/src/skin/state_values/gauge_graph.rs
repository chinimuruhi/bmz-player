use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct GaugeGraphColors {
    pub(super) graph_bg: Color,
    pub(super) graph_line: Color,
    pub(super) border_bg: Color,
    pub(super) border_line: Color,
    pub(super) course_section_line: Color,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GaugeGraphLayout {
    pub(super) rect: Rect,
    pub(super) max: f32,
    pub(super) border: f32,
    pub(super) colors: GaugeGraphColors,
    pub(super) line_width: f32,
    pub(super) line_height: f32,
    pub(super) render_progress: f32,
    pub(super) additive: bool,
}

#[derive(Debug, Clone, Copy)]
struct GaugeGraphSegment {
    from: [f32; 2],
    to: [f32; 2],
    line_size: [f32; 2],
    color: Color,
}

pub(super) fn is_additive_black(color: Color) -> bool {
    color.r == 0.0 && color.g == 0.0 && color.b == 0.0
}

pub(super) fn gaugegraph_color_index(gauge_type: i32) -> usize {
    const TYPE_TABLE: [usize; 10] = [0, 1, 2, 3, 4, 5, 3, 4, 5, 3];
    TYPE_TABLE.get(gauge_type.max(0) as usize).copied().unwrap_or(3)
}

pub(super) fn gaugegraph_colors(
    graph: &SkinGaugeGraphDef,
    color_index: usize,
    frame_alpha: f32,
) -> GaugeGraphColors {
    let colors = if graph.color.is_empty() {
        gaugegraph_default_color_strings(graph)
    } else {
        gaugegraph_explicit_color_strings(graph)
    };
    let with_frame_alpha = |value: &str, fallback: Color| {
        let color = skin_hex_color(value).unwrap_or(fallback);
        color.with_alpha(color.a * frame_alpha)
    };
    GaugeGraphColors {
        border_line: with_frame_alpha(&colors[color_index][0], Color::rgb(0.0, 0.0, 0.0)),
        border_bg: with_frame_alpha(&colors[color_index][1], Color::rgb(0.0, 0.0, 0.0)),
        graph_line: with_frame_alpha(&colors[color_index][2], Color::rgb(0.0, 0.0, 0.0)),
        graph_bg: with_frame_alpha(&colors[color_index][3], Color::rgb(0.0, 0.0, 0.0)),
        course_section_line: Color::rgba(1.0, 1.0, 1.0, frame_alpha),
    }
}

pub(super) fn gaugegraph_explicit_color_strings(graph: &SkinGaugeGraphDef) -> [[String; 4]; 6] {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            graph.color.get(row * 4 + column).cloned().unwrap_or_else(|| "000000".to_string())
        })
    })
}

pub(super) fn gaugegraph_default_color_strings(graph: &SkinGaugeGraphDef) -> [[String; 4]; 6] {
    let mut colors = [
        [
            graph.borderline_color.clone(),
            graph.border_color.clone(),
            graph.assist_clear_line_color.clone(),
            graph.assist_clear_bg_color.clone(),
        ],
        [
            graph.borderline_color.clone(),
            graph.border_color.clone(),
            graph.assist_and_easy_fail_line_color.clone(),
            graph.assist_and_easy_fail_bg_color.clone(),
        ],
        [
            graph.borderline_color.clone(),
            graph.border_color.clone(),
            graph.groove_fail_line_color.clone(),
            graph.groove_fail_bg_color.clone(),
        ],
        [
            graph.groove_clear_and_hard_line_color.clone(),
            graph.groove_clear_and_hard_bg_color.clone(),
            graph.groove_clear_and_hard_line_color.clone(),
            graph.groove_clear_and_hard_bg_color.clone(),
        ],
        [
            graph.ex_hard_line_color.clone(),
            graph.ex_hard_bg_color.clone(),
            graph.ex_hard_line_color.clone(),
            graph.ex_hard_bg_color.clone(),
        ],
        [
            graph.hazard_line_color.clone(),
            graph.hazard_bg_color.clone(),
            graph.hazard_line_color.clone(),
            graph.hazard_bg_color.clone(),
        ],
    ];
    for row in &mut colors {
        for color in row {
            if color.is_empty() {
                *color = "000000".to_string();
            }
        }
    }
    colors
}

pub(super) fn gaugegraph_y(rect: Rect, gauge: f32, max: f32) -> f32 {
    rect.y + rect.height * (1.0 - (gauge / max).clamp(0.0, 1.0))
}

pub(super) fn gaugegraph_sample_ratio(index: usize, sample_count: usize) -> f32 {
    if sample_count == 0 { 0.0 } else { index as f32 / sample_count as f32 }
}

pub(super) fn gaugegraph_rect_batch(
    points: &[crate::snapshot::ResultGaugeGraphPoint],
    layout: GaugeGraphLayout,
) -> Arc<[RectCommand]> {
    let GaugeGraphLayout {
        rect,
        max,
        border,
        colors,
        line_width,
        line_height,
        render_progress,
        additive,
    } = layout;
    let border_y = rect.y + rect.height * (1.0 - (border / max).clamp(0.0, 1.0));
    let render_x = rect.x + rect.width * render_progress;
    let mut rects = Vec::with_capacity(points.len().saturating_mul(3).saturating_add(3));
    // Additive black is a no-op in beatoraja. RectBatch has no blend field,
    // so emitting it as a normal rectangle would cover an earlier graph.
    if !additive || !is_additive_black(colors.graph_bg) {
        rects.push(RectCommand { rect, color: colors.graph_bg });
    }
    if border_y > rect.y && (!additive || !is_additive_black(colors.border_bg)) {
        rects.push(RectCommand {
            rect: Rect { x: rect.x, y: rect.y, width: rect.width, height: border_y - rect.y },
            color: colors.border_bg,
        });
    }
    let sample_count = points.len();
    for (index, pair) in points.windows(2).enumerate() {
        let from = pair[0];
        let to = pair[1];
        let x1 = rect.x + gaugegraph_sample_ratio(index, sample_count) * rect.width;
        if x1 > render_x {
            break;
        }
        let x2 =
            (rect.x + gaugegraph_sample_ratio(index + 1, sample_count) * rect.width).min(render_x);
        let y1 = gaugegraph_y(rect, from.value, max);
        let y2 = gaugegraph_y(rect, to.value, max);
        if to.course_section_start {
            rects.push(RectCommand {
                rect: Rect { x: x1, y: rect.y, width: line_width * 0.5, height: rect.height },
                color: colors.course_section_line,
            });
        }
        if (x2 - x1).abs() <= f32::EPSILON {
            continue;
        }
        if from.value < border && to.value < border {
            push_gaugegraph_segment(
                &mut rects,
                GaugeGraphSegment {
                    from: [x1, y1],
                    to: [x2, y2],
                    line_size: [line_width, line_height],
                    color: colors.graph_line,
                },
            );
        } else if from.value >= border && to.value >= border {
            push_gaugegraph_segment(
                &mut rects,
                GaugeGraphSegment {
                    from: [x1, y1],
                    to: [x2, y2],
                    line_size: [line_width, line_height],
                    color: colors.border_line,
                },
            );
        } else {
            let split_x = if (to.value - from.value).abs() <= f32::EPSILON {
                x1
            } else {
                x1 + (x2 - x1) * ((border - from.value) / (to.value - from.value)).clamp(0.0, 1.0)
            };
            let graph_color =
                if from.value < border { colors.graph_line } else { colors.border_line };
            let border_color =
                if from.value < border { colors.border_line } else { colors.graph_line };
            push_gaugegraph_segment(
                &mut rects,
                GaugeGraphSegment {
                    from: [x1, y1],
                    to: [split_x, border_y],
                    line_size: [line_width, line_height],
                    color: graph_color,
                },
            );
            push_gaugegraph_segment(
                &mut rects,
                GaugeGraphSegment {
                    from: [split_x, border_y],
                    to: [x2, y2],
                    line_size: [line_width, line_height],
                    color: border_color,
                },
            );
        }
    }
    if points.len() == 1 {
        let y = gaugegraph_y(rect, points[0].value, max);
        let color = if points[0].value < border { colors.graph_line } else { colors.border_line };
        rects.push(RectCommand {
            rect: Rect {
                x: rect.x,
                y,
                width: (render_x - rect.x).max(line_width),
                height: line_height,
            },
            color,
        });
    } else if let Some(last) = points.last().copied() {
        let x1 = rect.x
            + gaugegraph_sample_ratio(sample_count.saturating_sub(1), sample_count) * rect.width;
        let x2 = render_x;
        if x2 > x1 {
            let y = gaugegraph_y(rect, last.value, max);
            let color = if last.value < border { colors.graph_line } else { colors.border_line };
            push_gaugegraph_segment(
                &mut rects,
                GaugeGraphSegment {
                    from: [x1, y],
                    to: [x2, y],
                    line_size: [line_width, line_height],
                    color,
                },
            );
        }
    }
    Arc::from(rects)
}

fn push_gaugegraph_segment(rects: &mut Vec<RectCommand>, segment: GaugeGraphSegment) {
    let [x1, y1] = segment.from;
    let [x2, y2] = segment.to;
    let [line_width, line_height] = segment.line_size;
    let width = (x2 - x1).max(line_width);
    rects.push(RectCommand {
        rect: Rect {
            x: x1,
            y: y1.min(y2),
            width: line_width,
            height: (y2 - y1).abs() + line_height,
        },
        color: segment.color,
    });
    rects.push(RectCommand {
        rect: Rect { x: x1, y: y2, width, height: line_height },
        color: segment.color,
    });
}
