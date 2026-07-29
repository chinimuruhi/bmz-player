use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct GaugeGraphColors {
    pub(super) graph_bg: Color,
    pub(super) graph_line: Color,
    pub(super) border_bg: Color,
    pub(super) border_line: Color,
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

#[allow(clippy::too_many_arguments)]
pub(super) fn gaugegraph_rect_batch(
    points: &[crate::snapshot::ResultGaugeGraphPoint],
    rect: Rect,
    max: f32,
    border: f32,
    colors: GaugeGraphColors,
    line_w: f32,
    line_h: f32,
    render_progress: f32,
    additive: bool,
) -> Arc<[RectCommand]> {
    let border_y = rect.y + rect.height * (1.0 - (border / max).clamp(0.0, 1.0));
    let render_x = rect.x + rect.width * render_progress;
    let mut rects = Vec::with_capacity(points.len().saturating_mul(2).saturating_add(3));
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
        if (x2 - x1).abs() <= f32::EPSILON {
            continue;
        }
        if from.value < border && to.value < border {
            push_gaugegraph_segment(&mut rects, x1, x2, y1, y2, line_w, line_h, colors.graph_line);
        } else if from.value >= border && to.value >= border {
            push_gaugegraph_segment(&mut rects, x1, x2, y1, y2, line_w, line_h, colors.border_line);
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
                x1,
                split_x,
                y1,
                border_y,
                line_w,
                line_h,
                graph_color,
            );
            push_gaugegraph_segment(
                &mut rects,
                split_x,
                x2,
                border_y,
                y2,
                line_w,
                line_h,
                border_color,
            );
        }
    }
    if points.len() == 1 {
        let y = gaugegraph_y(rect, points[0].value, max);
        let color = if points[0].value < border { colors.graph_line } else { colors.border_line };
        rects.push(RectCommand {
            rect: Rect { x: rect.x, y, width: (render_x - rect.x).max(line_w), height: line_h },
            color,
        });
    } else if let Some(last) = points.last().copied() {
        let x1 = rect.x
            + gaugegraph_sample_ratio(sample_count.saturating_sub(1), sample_count) * rect.width;
        let x2 = render_x;
        if x2 > x1 {
            let y = gaugegraph_y(rect, last.value, max);
            let color = if last.value < border { colors.graph_line } else { colors.border_line };
            push_gaugegraph_segment(&mut rects, x1, x2, y, y, line_w, line_h, color);
        }
    }
    Arc::from(rects)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_gaugegraph_segment(
    rects: &mut Vec<RectCommand>,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
    line_w: f32,
    line_h: f32,
    color: Color,
) {
    let width = (x2 - x1).max(line_w);
    rects.push(RectCommand {
        rect: Rect { x: x1, y: y1.min(y2), width: line_w, height: (y2 - y1).abs() + line_h },
        color,
    });
    rects.push(RectCommand { rect: Rect { x: x1, y: y2, width, height: line_h }, color });
}

pub(super) fn skin_timing_distribution_from_points(
    points: &[crate::snapshot::ResultTimingPoint],
) -> crate::snapshot::ResultTimingDistribution {
    let mut distribution = crate::snapshot::ResultTimingDistribution::default();
    for point in points {
        distribution.add((point.delta_us / 1_000) as i32);
    }
    distribution
}

pub(super) fn beatoraja_timing_distribution_max(
    distribution: &crate::snapshot::ResultTimingDistribution,
) -> u32 {
    let mut max = 10;
    for count in &distribution.counts {
        if max < *count {
            max = (count / 10) * 10 + 10;
        }
    }
    max
}

pub(super) fn timing_color(value: &str, frame_alpha: f32) -> Color {
    skin_hex_color(value)
        .or_else(|| skin_hex_color("FF0000FF"))
        .unwrap_or(Color::rgb(1.0, 0.0, 0.0))
        .with_alpha(frame_alpha)
}

pub(super) fn select_note_distribution_max_density(
    distribution: &[crate::scene::SelectChartDistributionSecond],
) -> u32 {
    let peak = distribution.iter().map(|second| second.total()).max().unwrap_or(0);
    if peak <= 20 { 20 } else { ((peak / 10) * 10 + 10).clamp(20, 100) }
}

pub(super) fn select_note_distribution_background_items(
    rect: Rect,
    seconds: usize,
    max_density: u32,
    frame_alpha: f32,
    blend: BlendMode,
    pixel_w: f32,
    pixel_h: f32,
) -> Vec<SkinRenderItem> {
    let mut items = vec![SkinRenderItem::Rect {
        rect,
        color: Color::rgba(0.0, 0.0, 0.0, 0.8 * frame_alpha),
        blend,
    }];

    for density in (10..max_density).step_by(10) {
        let y = rect.y + rect.height - rect.height * density as f32 / max_density.max(1) as f32;
        items.push(SkinRenderItem::Rect {
            rect: Rect { x: rect.x, y, width: rect.width, height: pixel_h },
            color: Color::rgba(0.007 * density as f32, 0.007 * density as f32, 0.0, frame_alpha),
            blend,
        });
    }

    for second in 0..seconds {
        let color = if second % 60 == 0 {
            Some(Color::rgba(0.25, 0.25, 0.25, frame_alpha))
        } else if second % 10 == 0 {
            Some(Color::rgba(0.125, 0.125, 0.125, frame_alpha))
        } else {
            None
        };
        if let Some(color) = color {
            let x = rect.x + rect.width * second as f32 / seconds.max(1) as f32;
            items.push(SkinRenderItem::Rect {
                rect: Rect { x, y: rect.y, width: pixel_w, height: rect.height },
                color,
                blend,
            });
        }
    }
    items
}

pub(super) fn note_distribution_colors(alpha: f32) -> [Color; 7] {
    [
        Color::rgba(0.27, 1.0, 0.27, alpha),
        Color::rgba(0.13, 0.53, 0.13, alpha),
        Color::rgba(1.0, 0.27, 0.27, alpha),
        Color::rgba(0.27, 0.27, 1.0, alpha),
        Color::rgba(0.13, 0.13, 0.53, alpha),
        Color::rgba(0.80, 0.80, 0.80, alpha),
        Color::rgba(0.53, 0.0, 0.0, alpha),
    ]
}

pub(super) fn result_judge_graph_colors(alpha: f32, pms: bool) -> [Color; 6] {
    if pms {
        return [
            Color::rgba(0.33, 0.33, 0.33, alpha),
            Color::rgba(1.0, 0.37, 0.69, alpha),
            Color::rgba(1.0, 0.75, 0.20, alpha),
            Color::rgba(0.86, 0.27, 0.24, alpha),
            Color::rgba(0.42, 0.78, 1.0, alpha),
            Color::rgba(0.42, 0.78, 1.0, alpha),
        ];
    }
    [
        Color::rgba(0.33, 0.33, 0.33, alpha),
        Color::rgba(0.0, 0.53, 1.0, alpha),
        Color::rgba(0.0, 1.0, 0.53, alpha),
        Color::rgba(1.0, 1.0, 0.0, alpha),
        Color::rgba(1.0, 0.53, 0.0, alpha),
        Color::rgba(1.0, 0.0, 0.0, alpha),
    ]
}

pub(super) fn result_early_late_graph_colors(alpha: f32, pms: bool) -> [Color; 10] {
    if pms {
        return [
            Color::rgba(0.33, 0.33, 0.33, alpha),
            Color::rgba(1.0, 0.37, 0.69, alpha),
            Color::rgba(0.0, 0.53, 1.0, alpha),
            Color::rgba(0.0, 0.4, 0.8, alpha),
            Color::rgba(0.0, 0.27, 0.53, alpha),
            Color::rgba(0.0, 0.13, 0.27, alpha),
            Color::rgba(1.0, 0.53, 0.0, alpha),
            Color::rgba(0.8, 0.4, 0.0, alpha),
            Color::rgba(0.53, 0.27, 0.0, alpha),
            Color::rgba(0.27, 0.13, 0.0, alpha),
        ];
    }
    [
        Color::rgba(0.33, 0.33, 0.33, alpha),
        Color::rgba(0.27, 1.0, 0.27, alpha),
        Color::rgba(0.0, 0.53, 1.0, alpha),
        Color::rgba(0.0, 0.4, 0.8, alpha),
        Color::rgba(0.0, 0.27, 0.53, alpha),
        Color::rgba(0.0, 0.13, 0.27, alpha),
        Color::rgba(1.0, 0.53, 0.0, alpha),
        Color::rgba(0.8, 0.4, 0.0, alpha),
        Color::rgba(0.53, 0.27, 0.0, alpha),
        Color::rgba(0.27, 0.13, 0.0, alpha),
    ]
}

pub(super) trait ResultNoteGraphBucket<const N: usize> {
    fn values(&self) -> [u32; N];
}

impl<const N: usize> ResultNoteGraphBucket<N> for [u32; N] {
    fn values(&self) -> [u32; N] {
        *self
    }
}

impl ResultNoteGraphBucket<6> for crate::snapshot::ResultJudgeGraphBucket {
    fn values(&self) -> [u32; 6] {
        self.values
    }
}

impl ResultNoteGraphBucket<10> for crate::snapshot::ResultEarlyLateGraphBucket {
    fn values(&self) -> [u32; 10] {
        self.values
    }
}

pub(super) fn stacked_result_note_graph_rect_batch<const N: usize, B: ResultNoteGraphBucket<N>>(
    buckets: &[B],
    colors: &[Color; N],
    graph: &SkinJudgeGraphDef,
    destination: &SkinDestinationDef,
    frame: ResolvedSkinFrame,
    canvas_w: u32,
    canvas_h: u32,
    elapsed_ms: i32,
) -> Arc<[RectCommand]> {
    if buckets.is_empty() {
        return Arc::from([]);
    }
    let rect = normalize_skin_frame_rect(frame, canvas_w, canvas_h);
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return Arc::from([]);
    }
    let frame_alpha = frame.a as f32 / 255.0;
    let blend = if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal };
    let max_stack =
        buckets.iter().map(|bucket| bucket.values().into_iter().sum::<u32>()).max().unwrap_or(0);
    let graph_max = beatoraja_note_graph_max(max_stack);
    let visible_len = result_note_graph_visible_len(buckets.len(), graph, elapsed_ms);
    let background_items = if graph.back_tex_off == 0 {
        result_note_graph_background_item_count(buckets.len(), graph_max)
    } else {
        0
    };
    let chip_items = buckets
        .iter()
        .take(visible_len)
        .map(|bucket| bucket.values().into_iter().sum::<u32>().min(graph_max) as usize)
        .sum::<usize>();
    let mut rects = Vec::with_capacity(background_items.saturating_add(chip_items));
    if graph.back_tex_off == 0 {
        push_result_note_graph_background(
            &mut rects,
            rect,
            buckets.len(),
            graph_max,
            frame_alpha,
            blend,
        );
    }
    if visible_len == 0 {
        return Arc::from(rects);
    }
    let bucket_w = rect.width / buckets.len().max(1) as f32;
    let chip_w = bucket_w * if graph.no_gap_x != 0 { 1.0 } else { 0.8 };
    let unit_h = rect.height / graph_max.max(1) as f32;
    let chip_h = unit_h * if graph.no_gap != 0 { 1.0 } else { 0.8 };

    for (second, bucket) in buckets.iter().take(visible_len).enumerate() {
        let x = rect.x + second as f32 * bucket_w;
        let mut drawn = 0_u32;
        let values = bucket.values();
        if graph.order_reverse != 0 {
            for (series, value) in values.into_iter().enumerate().rev() {
                push_result_note_graph_chips(
                    &mut rects,
                    rect,
                    x,
                    chip_w,
                    unit_h,
                    chip_h,
                    graph_max,
                    &mut drawn,
                    value,
                    colors[series],
                    blend,
                );
            }
        } else {
            for (series, value) in values.into_iter().enumerate() {
                push_result_note_graph_chips(
                    &mut rects,
                    rect,
                    x,
                    chip_w,
                    unit_h,
                    chip_h,
                    graph_max,
                    &mut drawn,
                    value,
                    colors[series],
                    blend,
                );
            }
        }
    }
    Arc::from(rects)
}

pub(super) fn rect_batch_render_items(
    rects: Arc<[RectCommand]>,
    cache: Option<RectBatchCache>,
) -> Vec<SkinRenderItem> {
    if rects.is_empty() { Vec::new() } else { vec![SkinRenderItem::RectBatch { rects, cache }] }
}

pub(super) fn result_note_graph_cache_key<const N: usize, B: ResultNoteGraphBucket<N>>(
    destination_index: usize,
    kind: ResultRectBatchKind,
    buckets: &[B],
    graph: &SkinJudgeGraphDef,
    frame: ResolvedSkinFrame,
    state: &SkinDrawState,
    elapsed_ms: i32,
) -> ResultRectBatchCacheKey {
    ResultRectBatchCacheKey {
        destination_index,
        kind,
        frame,
        key_mode: state.key_mode,
        judge_rank: state.judge_rank,
        visible_len: result_note_graph_visible_len(buckets.len(), graph, elapsed_ms),
        data_hash: result_note_graph_data_hash(buckets, graph),
    }
}

pub(super) fn result_note_graph_rect_batch_cache(
    key: ResultRectBatchCacheKey,
    graph: &SkinJudgeGraphDef,
    frame: ResolvedSkinFrame,
    canvas_w: u32,
    canvas_h: u32,
) -> Option<RectBatchCache> {
    if graph.back_tex_off == 0 {
        return None;
    }
    let bounds = normalize_skin_frame_rect(frame, canvas_w, canvas_h);
    if bounds.width <= f32::EPSILON || bounds.height <= f32::EPSILON {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    "result-note-graph-rect-batch".hash(&mut hasher);
    key.hash(&mut hasher);
    Some(RectBatchCache { key: RectBatchCacheKey(hasher.finish()), bounds })
}

pub(super) fn result_gauge_graph_rect_batch_cache(
    key: ResultGaugeGraphRectBatchCacheKey,
    rects: &[RectCommand],
) -> Option<RectBatchCache> {
    let first = rects.first()?.rect;
    let bounds = rects.iter().skip(1).fold(first, |bounds, command| {
        let left = bounds.x.min(command.rect.x);
        let top = bounds.y.min(command.rect.y);
        let right = (bounds.x + bounds.width).max(command.rect.x + command.rect.width);
        let bottom = (bounds.y + bounds.height).max(command.rect.y + command.rect.height);
        Rect { x: left, y: top, width: right - left, height: bottom - top }
    });
    if bounds.width <= f32::EPSILON || bounds.height <= f32::EPSILON {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    "result-gauge-graph-rect-batch".hash(&mut hasher);
    key.hash(&mut hasher);
    Some(RectBatchCache { key: RectBatchCacheKey(hasher.finish()), bounds })
}

pub(super) fn result_note_graph_data_hash<const N: usize, B: ResultNoteGraphBucket<N>>(
    buckets: &[B],
    graph: &SkinJudgeGraphDef,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    graph.graph_type().hash(&mut hasher);
    graph.back_tex_off.hash(&mut hasher);
    graph.delay.hash(&mut hasher);
    graph.order_reverse.hash(&mut hasher);
    graph.no_gap.hash(&mut hasher);
    graph.no_gap_x.hash(&mut hasher);
    buckets.len().hash(&mut hasher);
    for bucket in buckets {
        bucket.values().hash(&mut hasher);
    }
    hasher.finish()
}

pub(super) fn result_note_graph_visible_len(
    bucket_count: usize,
    graph: &SkinJudgeGraphDef,
    elapsed_ms: i32,
) -> usize {
    let render_ratio = if graph.delay > 0 {
        (elapsed_ms as f32 / graph.delay as f32).clamp(0.0, 1.0)
    } else {
        1.0
    };
    ((bucket_count as f32) * render_ratio).ceil() as usize
}

pub(super) fn beatoraja_note_graph_max(max_stack: u32) -> u32 {
    if max_stack <= 20 { 20 } else { ((max_stack / 10) * 10 + 10).min(100) }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_result_note_graph_chips(
    rects: &mut Vec<RectCommand>,
    rect: Rect,
    x: f32,
    chip_w: f32,
    unit_h: f32,
    chip_h: f32,
    graph_max: u32,
    drawn: &mut u32,
    value: u32,
    color: Color,
    _blend: BlendMode,
) {
    for _ in 0..value {
        if *drawn >= graph_max {
            break;
        }
        let y = rect.y + rect.height - (*drawn as f32 + 1.0) * unit_h;
        rects.push(RectCommand { rect: Rect { x, y, width: chip_w, height: chip_h }, color });
        *drawn = (*drawn).saturating_add(1);
    }
}

pub(super) fn push_result_note_graph_background(
    rects: &mut Vec<RectCommand>,
    rect: Rect,
    bucket_count: usize,
    graph_max: u32,
    frame_alpha: f32,
    _blend: BlendMode,
) {
    rects.push(RectCommand { rect, color: Color::rgba(0.0, 0.0, 0.0, 0.8 * frame_alpha) });
    for count in (10..graph_max).step_by(10) {
        let band_y =
            rect.y + rect.height * (1.0 - (count + 10).min(graph_max) as f32 / graph_max as f32);
        let band_h = rect.height * 10.0 / graph_max as f32;
        rects.push(RectCommand {
            rect: Rect { x: rect.x, y: band_y, width: rect.width, height: band_h },
            color: Color::rgba(0.007 * count as f32, 0.007 * count as f32, 0.0, frame_alpha),
        });
    }
    let line_w = (rect.width / (bucket_count.max(1) * 5) as f32).max(0.0005);
    for second in 0..bucket_count {
        let color = if second % 60 == 0 {
            Some(Color::rgba(0.25, 0.25, 0.25, frame_alpha))
        } else if second % 10 == 0 {
            Some(Color::rgba(0.125, 0.125, 0.125, frame_alpha))
        } else {
            None
        };
        if let Some(color) = color {
            rects.push(RectCommand {
                rect: Rect {
                    x: rect.x + second as f32 * rect.width / bucket_count.max(1) as f32,
                    y: rect.y,
                    width: line_w,
                    height: rect.height,
                },
                color,
            });
        }
    }
}

pub(super) fn result_note_graph_background_item_count(
    bucket_count: usize,
    graph_max: u32,
) -> usize {
    let band_count = (10..graph_max).step_by(10).count();
    let line_count = (0..bucket_count).filter(|second| second % 10 == 0).count();
    1 + band_count + line_count
}

pub(super) fn timing_visualizer_judge_colors(visualizer: &SkinTimingVisualizerDef) -> [Color; 5] {
    [
        timing_color(&visualizer.pg_color, 1.0),
        timing_color(&visualizer.gr_color, 1.0),
        timing_color(&visualizer.gd_color, 1.0),
        timing_color(&visualizer.bd_color, 1.0),
        if visualizer.transparent == 1 {
            Color::rgba(0.0, 0.0, 0.0, 0.0)
        } else {
            timing_color(&visualizer.pr_color, 1.0)
        },
    ]
}

pub(super) fn timing_distribution_judge_colors(
    graph: &SkinTimingDistributionGraphDef,
) -> [Color; 5] {
    [
        timing_color(&graph.pg_color, 1.0),
        timing_color(&graph.gr_color, 1.0),
        timing_color(&graph.gd_color, 1.0),
        timing_color(&graph.bd_color, 1.0),
        timing_color(&graph.pr_color, 1.0),
    ]
}

pub(super) fn judge_timing_color(
    judge: Judge,
    visualizer: &SkinTimingVisualizerDef,
    fallback: Color,
) -> Color {
    match judge {
        Judge::PGreat => timing_color(&visualizer.pg_color, 1.0),
        Judge::Great => timing_color(&visualizer.gr_color, 1.0),
        Judge::Good => timing_color(&visualizer.gd_color, 1.0),
        Judge::Bad => timing_color(&visualizer.bd_color, 1.0),
        Judge::Poor | Judge::EmptyPoor if visualizer.transparent == 1 => {
            Color::rgba(0.0, 0.0, 0.0, 0.0)
        }
        Judge::Poor | Judge::EmptyPoor => timing_color(&visualizer.pr_color, 1.0),
    }
    .with_alpha(fallback.a)
}

pub(super) fn timing_judge_band_items(
    rect: Rect,
    center_ms: f32,
    frame_alpha: f32,
    blend: BlendMode,
    colors: [Color; 5],
    state: &SkinDrawState,
) -> Vec<SkinRenderItem> {
    let areas = beatoraja_timing_judge_areas(state);
    let mut items = Vec::new();
    let mut inner_late_ms = 0.0;
    let mut inner_early_ms = 0.0;
    for (area, color) in areas.into_iter().zip(colors) {
        let late_ms = area.late_ms.clamp(-center_ms, center_ms);
        let early_ms = area.early_ms.clamp(-center_ms, center_ms);
        push_timing_judge_band_rect(
            &mut items,
            rect,
            center_ms,
            late_ms,
            inner_late_ms,
            color,
            frame_alpha,
            blend,
        );
        push_timing_judge_band_rect(
            &mut items,
            rect,
            center_ms,
            inner_early_ms,
            early_ms,
            color,
            frame_alpha,
            blend,
        );
        inner_late_ms = inner_late_ms.min(late_ms);
        inner_early_ms = inner_early_ms.max(early_ms);
    }
    items
}

pub(super) fn push_timing_judge_band_rect(
    items: &mut Vec<SkinRenderItem>,
    rect: Rect,
    center_ms: f32,
    start_ms: f32,
    end_ms: f32,
    color: Color,
    frame_alpha: f32,
    blend: BlendMode,
) {
    if end_ms <= start_ms {
        return;
    }
    let x1 = rect.x + ((start_ms + center_ms) / (center_ms * 2.0)) * rect.width;
    let x2 = rect.x + ((end_ms + center_ms) / (center_ms * 2.0)) * rect.width;
    items.push(SkinRenderItem::Rect {
        rect: Rect { x: x1, y: rect.y, width: (x2 - x1).max(0.0), height: rect.height },
        color: color.with_alpha(color.a * frame_alpha * 0.25),
        blend,
    });
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TimingJudgeArea {
    pub(super) late_ms: f32,
    pub(super) early_ms: f32,
}

pub(super) fn beatoraja_timing_judge_areas(state: &SkinDrawState) -> [TimingJudgeArea; 5] {
    let base = bmz_gameplay::judge::window::beatoraja_note_judge_window_for_keymode(state.key_mode);
    let percent = beatoraja_judge_rank_percent_for_mode(state.key_mode, state.judge_rank);
    let window = bmz_gameplay::judge::window::beatoraja_judge_window_for_rank_and_keymode(
        base,
        percent,
        state.key_mode,
    );
    timing_judge_areas_from_window(window)
}

pub(super) fn timing_judge_areas_from_window(
    window: bmz_gameplay::judge::model::JudgeWindow,
) -> [TimingJudgeArea; 5] {
    [
        symmetric_timing_judge_area(window.pgreat_us),
        symmetric_timing_judge_area(window.great_us),
        symmetric_timing_judge_area(window.good_us),
        TimingJudgeArea {
            late_ms: -window.bad_fast_us as f32 / 1_000.0,
            early_ms: window.bad_slow_us as f32 / 1_000.0,
        },
        TimingJudgeArea {
            late_ms: -window.empty_poor_fast_us as f32 / 1_000.0,
            early_ms: window.empty_poor_slow_us as f32 / 1_000.0,
        },
    ]
}

pub(super) fn symmetric_timing_judge_area(us: i64) -> TimingJudgeArea {
    let ms = us as f32 / 1_000.0;
    TimingJudgeArea { late_ms: -ms, early_ms: ms }
}

pub(super) fn beatoraja_judge_rank_percent_for_mode(
    key_mode: KeyMode,
    judge_rank: Option<i32>,
) -> i32 {
    let Some(rank) = judge_rank else {
        return 100;
    };
    if rank >= 10 {
        return rank;
    }
    let table =
        if key_mode == KeyMode::K9 { [33, 50, 70, 100, 133] } else { [25, 50, 75, 100, 125] };
    table.get(rank as usize).copied().unwrap_or(table[2])
}

pub(super) fn timing_distribution_x(rect: Rect, center: usize, value_ms: f32) -> f32 {
    let span = (center.max(1) * 2) as f32;
    rect.x + ((center as f32 + value_ms) / span).clamp(0.0, 1.0) * rect.width
}
