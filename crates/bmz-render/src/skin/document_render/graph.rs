macro_rules! skin_document_render_graph_methods {
    () => {
        #[cfg(test)]
        fn text_render_item(
            &self,
            text: &SkinTextDef,
            frame: ResolvedSkinFrame,
            state: &SkinTextState<'_>,
        ) -> Option<SkinRenderItem> {
            self.text_render_item_with_draw_state(text, frame, None, state)
        }

        fn text_render_item_with_draw_state(
            &self,
            text: &SkinTextDef,
            frame: ResolvedSkinFrame,
            draw_state: Option<&SkinDrawState>,
            state: &SkinTextState<'_>,
        ) -> Option<SkinRenderItem> {
            let content = skin_state_text_with_draw_state(text, draw_state, state);
            let rect = normalize_skin_frame_rect(frame, self.w, self.h);
            // beatoraja は dst.x を align 基準点として扱う（align=1=center なら
            // dst.x がテキストの中央, align=2=right なら dst.x がテキストの右端）。
            // bmz の renderer は origin を「テキストボックスの左端」として扱うので、
            // align に応じて origin.x を平行移動してから渡す。
            let origin_x = match text.align {
                1 => rect.x - rect.width / 2.0,
                2 => rect.x - rect.width,
                _ => rect.x,
            };
            // beatoraja `STRING_SEARCHWORD` (ref=30) は placeholder 状態で
            // messageFontColor=GRAY (半透明) になる。bmz では state から渡される
            // multiplier を skin 由来の alpha に掛け合わせて同様の見た目を再現する。
            let mut alpha = frame.a as f32 / 255.0;
            if text.ref_id == 30 {
                alpha *= state.search_word_alpha.clamp(0.0, 1.0);
            }
            let mut color = Color::rgba(
                frame.r as f32 / 255.0,
                frame.g as f32 / 255.0,
                frame.b as f32 / 255.0,
                alpha,
            );
            if text.judge_color
                && let Some(draw_state) = draw_state
                && let Some(region) = text.judge_region
                && let Some(judge_color) = skin_judge_region_color(draw_state, region, alpha)
            {
                color = judge_color;
            }
            if text.judge_timing_color
                && let Some(draw_state) = draw_state
                && let Some(region) = text.judge_timing_region
                && let Some(judge_color) = skin_judge_timing_color(draw_state, region, alpha)
            {
                color = judge_color;
            }
            let caret = if text.ref_id == 30 {
                state.search_caret_byte_index.map(|byte_index| TextCaret { byte_index, color })
            } else {
                None
            };
            if content.is_empty() && caret.is_none() {
                return None;
            }
            Some(SkinRenderItem::Text {
                origin: Point { x: origin_x, y: rect.y },
                text: content,
                style: TextStyle {
                    font_id: (!text.font.is_empty()).then(|| text.font.clone()),
                    size: frame.h.abs().max(text.size).max(1) as f32 / self.h.max(1) as f32,
                    bitmap_size: skin_text_bitmap_size(text, &self.font, self.h, frame.h),
                    color,
                    layer: TextLayer::Ui,
                    align: skin_text_align(text.align),
                    max_width: frame.w.abs() as f32 / self.w.max(1) as f32,
                    overflow: skin_text_overflow(text.overflow),
                    wrapping: text.wrapping,
                    outline: skin_text_outline(text, self.h),
                    shadow: skin_text_shadow(text, self.w, self.h),
                },
                caret,
                blend: BlendMode::Normal,
            })
        }

        fn hiterror_visualizer_render_items(
            &self,
            visualizer: &SkinHitErrorVisualizerDef,
            destination: &SkinDestinationDef,
            frame: ResolvedSkinFrame,
            state: &SkinDrawState,
        ) -> Vec<SkinRenderItem> {
            if visualizer.hiterror_mode == 0 {
                return Vec::new();
            }
            let rect = normalize_skin_frame_rect(frame, self.w, self.h);
            let frame_alpha = frame.a as f32 / 255.0;
            let blend = if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal };
            let window = visualizer.window_length.clamp(1, 100) as usize;
            let width = visualizer.width.max(1) as f32;
            let line_width = visualizer.line_width.clamp(1, 4) as f32;
            let center_ms = visualizer.judge_width_millis.max(1) as f32;
            let judge_width_rate = width / (center_ms * 2.0 + 1.0);
            let line_color =
                skin_hex_color(&visualizer.line_color).unwrap_or(Color::rgba(0.6, 0.8, 1.0, 0.5));
            let center_color =
                skin_hex_color(&visualizer.center_color).unwrap_or(Color::rgba(1.0, 1.0, 1.0, 1.0));
            let canvas_h = rect.height.max(1.0);
            let mut items = Vec::new();
            let center_x = rect.x + rect.width / 2.0 - line_width / 2.0;
            items.push(SkinRenderItem::Rect {
                rect: Rect { x: center_x, y: rect.y, width: line_width, height: canvas_h },
                color: center_color.with_alpha(center_color.a * frame_alpha),
                blend,
            });
            let index = state.hit_error_ring_index;
            let recent = &state.hit_error_ring;
            for i in 1..=window {
                let ring_index = (index as i64 - window as i64 + i as i64)
                    .rem_euclid(bmz_gameplay::hit_error::HIT_ERROR_RING_LEN as i64)
                    as usize;
                let sample = recent[ring_index];
                if sample == bmz_gameplay::hit_error::HIT_ERROR_EMPTY {
                    continue;
                }
                let clamped = sample.clamp(
                    -visualizer.judge_width_millis as i64,
                    visualizer.judge_width_millis as i64,
                ) as f32;
                let x = rect.x + width / 2.0 - line_width / 2.0 - clamped * judge_width_rate;
                let alpha = if visualizer.color_mode == 0 {
                    line_color.a * (i as f32 / (window as f32 / 2.0)).min(1.0)
                } else {
                    line_color.a
                };
                let bar_h = if visualizer.draw_decay != 0 {
                    canvas_h * i as f32 / window as f32
                } else {
                    canvas_h
                };
                items.push(SkinRenderItem::Rect {
                    rect: Rect {
                        x,
                        y: rect.y + canvas_h - bar_h,
                        width: line_width,
                        height: bar_h,
                    },
                    color: Color::rgba(
                        line_color.r,
                        line_color.g,
                        line_color.b,
                        alpha * frame_alpha,
                    ),
                    blend,
                });
            }
            items
        }

        fn gaugegraph_render_items(
            &self,
            destination_index: usize,
            graph: &SkinGaugeGraphDef,
            destination: &SkinDestinationDef,
            frame: ResolvedSkinFrame,
            state: &SkinDrawState,
            points: &[crate::snapshot::ResultGaugeGraphPoint],
            mut cache: Option<&mut ResultRenderCache>,
        ) -> Vec<SkinRenderItem> {
            let cached_points = state
                .result_gauge_graph_type
                .and_then(|gauge_type| cache.as_deref_mut()?.cached_gauge_points(gauge_type));
            let graph_revision = cached_points
                .as_ref()
                .map(|(revision, _)| *revision)
                .or_else(|| cache.as_deref().and_then(ResultRenderCache::gauge_graph_revision));
            let uncached_filtered_points = if cached_points.is_none() {
                state.result_gauge_graph_type.map(|gauge_type| {
                    points
                        .iter()
                        .copied()
                        .filter(|point| point.gauge_type == gauge_type)
                        .collect::<Vec<_>>()
                })
            } else {
                None
            };
            let points = cached_points
                .as_ref()
                .map(|(_, points)| points.as_ref())
                .or_else(|| {
                    uncached_filtered_points.as_deref().filter(|filtered| !filtered.is_empty())
                })
                .unwrap_or(points);
            if points.is_empty() {
                return Vec::new();
            }
            let rect = normalize_skin_frame_rect(frame, self.w, self.h);
            let frame_alpha = frame.a as f32 / 255.0;
            let max = points
                .iter()
                .find_map(|point| (point.max > 0.0).then_some(point.max))
                .unwrap_or(state.gauge_max)
                .max(1.0);
            let display_gauge_type = state.result_gauge_graph_type.unwrap_or_else(|| {
                points.last().map(|point| point.gauge_type).unwrap_or(state.gauge_type)
            });
            let border = points.first().map(|point| point.border).unwrap_or(state.gauge_border);
            let color_index = gaugegraph_color_index(display_gauge_type);
            let colors = gaugegraph_colors(graph, color_index, frame_alpha);
            let line_w = (2.0 / self.w.max(1) as f32).max(0.001);
            let line_h = (2.0 / self.h.max(1) as f32).max(0.001);
            let render_progress = (state.elapsed_ms.max(0) as f32 / 1500.0).clamp(0.0, 1.0);
            let build = || {
                gaugegraph_rect_batch(
                    points,
                    rect,
                    max,
                    border,
                    colors,
                    line_w,
                    line_h,
                    render_progress,
                    destination.blend == 2,
                )
            };
            let completed = render_progress >= 1.0;
            let key = graph_revision.map(|graph_revision| ResultGaugeGraphRectBatchCacheKey {
                destination_index,
                frame,
                graph_revision,
                display_gauge_type,
                gauge_max_bits: max.to_bits(),
                gauge_border_bits: border.to_bits(),
            });
            let rects = if completed {
                if let (Some(cache), Some(key)) = (cache, key) {
                    cache.cached_gauge_rect_batch(key, build)
                } else {
                    build()
                }
            } else {
                build()
            };
            let batch_cache = completed
                .then(|| key.and_then(|key| result_gauge_graph_rect_batch_cache(key, &rects)))
                .flatten();
            rect_batch_render_items(rects, batch_cache)
        }

        fn timing_visualizer_render_items(
            &self,
            visualizer: &SkinTimingVisualizerDef,
            destination: &SkinDestinationDef,
            frame: ResolvedSkinFrame,
            state: &SkinDrawState,
            timing_points: &[crate::snapshot::ResultTimingPoint],
        ) -> Vec<SkinRenderItem> {
            if timing_points.is_empty() {
                return Vec::new();
            }
            let rect = normalize_skin_frame_rect(frame, self.w, self.h);
            let frame_alpha = frame.a as f32 / 255.0;
            let blend = if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal };
            let width = visualizer.width.max(1) as f32;
            let center_ms = visualizer.judge_width_millis.max(1) as f32;
            let line_w =
                (visualizer.line_width.clamp(1, 4) as f32 / self.w.max(1) as f32).max(0.001);
            let judge_width_rate = width / (center_ms * 2.0 + 1.0);
            let center_color = timing_color(&visualizer.center_color, frame_alpha);
            let base_line_color = timing_color(&visualizer.line_color, frame_alpha);
            let mut items = Vec::new();
            items.extend(timing_judge_band_items(
                rect,
                center_ms,
                frame_alpha,
                blend,
                timing_visualizer_judge_colors(visualizer),
                state,
            ));
            let center_x = rect.x + rect.width / 2.0 - line_w / 2.0;
            items.push(SkinRenderItem::Rect {
                rect: Rect { x: center_x, y: rect.y, width: line_w, height: rect.height },
                color: center_color,
                blend,
            });

            let window = timing_points.len().min(bmz_gameplay::hit_error::HIT_ERROR_RING_LEN);
            for (index, point) in timing_points.iter().rev().take(window).enumerate() {
                let delta_ms = point.delta_us as f32 / 1_000.0;
                if delta_ms.abs() > center_ms {
                    continue;
                }
                let x = rect.x + rect.width / 2.0 - line_w / 2.0
                    + delta_ms * judge_width_rate / width * rect.width;
                let age = (window - index) as f32 / window.max(1) as f32;
                let alpha = if visualizer.draw_decay == 1 { age } else { 1.0 };
                let color = judge_timing_color(point.judge, visualizer, base_line_color)
                    .with_alpha(base_line_color.a * alpha);
                let height =
                    if visualizer.draw_decay == 1 { rect.height * age } else { rect.height };
                items.push(SkinRenderItem::Rect {
                    rect: Rect { x, y: rect.y + rect.height - height, width: line_w, height },
                    color,
                    blend,
                });
            }
            items
        }

        fn timing_distribution_graph_render_items(
            &self,
            graph: &SkinTimingDistributionGraphDef,
            destination: &SkinDestinationDef,
            frame: ResolvedSkinFrame,
            state: &SkinDrawState,
            timing_points: &[crate::snapshot::ResultTimingPoint],
            timing_distribution: &crate::snapshot::ResultTimingDistribution,
        ) -> Vec<SkinRenderItem> {
            let fallback_distribution;
            let distribution = if timing_distribution.total() > 0 || timing_points.is_empty() {
                timing_distribution
            } else {
                fallback_distribution = skin_timing_distribution_from_points(timing_points);
                &fallback_distribution
            };
            let rect = normalize_skin_frame_rect(frame, self.w, self.h);
            let frame_alpha = frame.a as f32 / 255.0;
            let blend = if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal };
            let width = graph.width.max(1);
            let line_px = graph.line_width.clamp(1, width);
            let buckets = (width / line_px).max(1) as usize;
            let center = buckets / 2;
            let mut counts = vec![0u32; buckets];
            for (bucket_index, count) in counts.iter_mut().enumerate() {
                let timing_ms = bucket_index as i32 - center as i32;
                if -distribution.range_ms < timing_ms && timing_ms < distribution.range_ms {
                    let source_index = (timing_ms + distribution.range_ms) as usize;
                    if let Some(source_count) = distribution.counts.get(source_index) {
                        *count = *source_count;
                    }
                }
            }
            let max_count = beatoraja_timing_distribution_max(distribution) as f32;
            let bar_w = (rect.width / buckets.max(1) as f32).max(1.0 / self.w.max(1) as f32);
            let mut items = timing_judge_band_items(
                rect,
                center as f32,
                frame_alpha,
                blend,
                timing_distribution_judge_colors(graph),
                state,
            );
            items.reserve(buckets.saturating_add(3));
            let graph_color = timing_color(&graph.graph_color, frame_alpha);
            for (index, count) in counts.into_iter().enumerate() {
                if count == 0 {
                    continue;
                }
                let height = rect.height * count as f32 / max_count;
                items.push(SkinRenderItem::Rect {
                    rect: Rect {
                        x: rect.x + index as f32 * bar_w,
                        y: rect.y + rect.height - height,
                        width: bar_w,
                        height,
                    },
                    color: graph_color,
                    blend,
                });
            }
            let stats = distribution.stats();
            if graph.draw_average == 1
                && let Some((average_ms, _)) = stats
            {
                let color = timing_color(&graph.average_color, frame_alpha);
                let x = timing_distribution_x(rect, center, average_ms);
                items.push(SkinRenderItem::Rect {
                    rect: Rect { x, y: rect.y, width: bar_w.max(0.001), height: rect.height },
                    color,
                    blend,
                });
            }
            if graph.draw_dev == 1
                && let Some((average_ms, stddev_ms)) = stats
            {
                let color = timing_color(&graph.dev_color, frame_alpha);
                for x in [
                    timing_distribution_x(rect, center, average_ms + stddev_ms),
                    timing_distribution_x(rect, center, average_ms - stddev_ms),
                ] {
                    items.push(SkinRenderItem::Rect {
                        rect: Rect { x, y: rect.y, width: bar_w.max(0.001), height: rect.height },
                        color,
                        blend,
                    });
                }
            }
            items
        }

        fn judgegraph_render_items(
            &self,
            destination_index: usize,
            graph: &SkinJudgeGraphDef,
            destination: &SkinDestinationDef,
            frame: ResolvedSkinFrame,
            elapsed_ms: i32,
            state: &SkinDrawState,
            runtime_graphs: SkinRuntimeGraphs<'_>,
            cache: Option<&mut ResultRenderCache>,
        ) -> Vec<SkinRenderItem> {
            let graph_type = graph.graph_type();
            let pms_colors = state.key_mode == KeyMode::K9;
            if graph_type == 1 && !runtime_graphs.result_judge_graph_buckets.is_empty() {
                let key = result_note_graph_cache_key(
                    destination_index,
                    ResultRectBatchKind::Judge,
                    runtime_graphs.result_judge_graph_buckets,
                    graph,
                    frame,
                    state,
                    elapsed_ms,
                );
                let build = || {
                    stacked_result_note_graph_rect_batch(
                        runtime_graphs.result_judge_graph_buckets,
                        &result_judge_graph_colors(frame.a as f32 / 255.0, pms_colors),
                        graph,
                        destination,
                        frame,
                        self.w,
                        self.h,
                        elapsed_ms,
                    )
                };
                let rects = if let Some(cache) = cache {
                    cache.cached_rect_batch(key, build)
                } else {
                    build()
                };
                return rect_batch_render_items(
                    rects,
                    result_note_graph_rect_batch_cache(key, graph, frame, self.w, self.h),
                );
            }
            if graph_type == 2 && !runtime_graphs.result_early_late_graph_buckets.is_empty() {
                let key = result_note_graph_cache_key(
                    destination_index,
                    ResultRectBatchKind::EarlyLate,
                    runtime_graphs.result_early_late_graph_buckets,
                    graph,
                    frame,
                    state,
                    elapsed_ms,
                );
                let build = || {
                    stacked_result_note_graph_rect_batch(
                        runtime_graphs.result_early_late_graph_buckets,
                        &result_early_late_graph_colors(frame.a as f32 / 255.0, pms_colors),
                        graph,
                        destination,
                        frame,
                        self.w,
                        self.h,
                        elapsed_ms,
                    )
                };
                let rects = if let Some(cache) = cache {
                    cache.cached_rect_batch(key, build)
                } else {
                    build()
                };
                return rect_batch_render_items(
                    rects,
                    result_note_graph_rect_batch_cache(key, graph, frame, self.w, self.h),
                );
            }
            self.density_judgegraph_render_items(
                graph,
                destination,
                frame,
                runtime_graphs.play_judge_graph_density,
            )
        }

        fn density_judgegraph_render_items(
            &self,
            graph: &SkinJudgeGraphDef,
            destination: &SkinDestinationDef,
            frame: ResolvedSkinFrame,
            density: &[u8],
        ) -> Vec<SkinRenderItem> {
            if density.is_empty() {
                return Vec::new();
            }
            let rect = normalize_skin_frame_rect(frame, self.w, self.h);
            let frame_alpha = frame.a as f32 / 255.0;
            let blend = if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal };
            let max_density = density.iter().copied().max().unwrap_or(1).max(1) as f32;
            let count = density.len().max(1) as f32;
            let pixel_w = 1.0 / self.w.max(1) as f32;
            let gap = if graph.no_gap != 0 || graph.no_gap_x != 0 { 0.0 } else { pixel_w };
            let bar_w = ((rect.width - gap * (count - 1.0)).max(pixel_w) / count).max(pixel_w);
            let color = Color::rgba(0.75, 0.85, 1.0, 0.85 * frame_alpha);
            let mut items = Vec::new();
            for (index, value) in density.iter().enumerate() {
                if *value == 0 {
                    continue;
                }
                let x = rect.x + index as f32 * (bar_w + gap);
                let height = rect.height * (*value as f32 / max_density);
                items.push(SkinRenderItem::Rect {
                    rect: Rect { x, y: rect.y + rect.height - height, width: bar_w, height },
                    color,
                    blend,
                });
            }
            items
        }

        fn select_note_distribution_graph_render_items(
            &self,
            row: &SelectRowSnapshot,
            graph: &SkinJudgeGraphDef,
            destination: &SkinDestinationDef,
            row_origin: (i32, i32),
            enabled_options: &[i32],
            state: &SkinDrawState,
        ) -> Vec<SkinRenderItem> {
            if row.chart_distribution.is_empty()
                || !test_skin_ops(&destination.op, enabled_options, state)
                || !eval_skin_draw_condition(&destination.draw, state)
                || graph.graph_type() != 0
            {
                return Vec::new();
            }
            let Some(elapsed) = skin_timer_elapsed_ms(destination.timer, state) else {
                return Vec::new();
            };
            let Some(mut frame) =
                resolve_destination_frame(destination, elapsed, enabled_options, state)
            else {
                return Vec::new();
            };
            frame.x += row_origin.0;
            frame.y += row_origin.1;
            apply_skin_offset_to_frame(destination, &mut frame, state, false);
            if !destination_mouse_rect_contains(destination, frame, state) {
                return Vec::new();
            }

            let rect = normalize_skin_frame_rect(frame, self.w, self.h);
            if rect.width <= 0.0 || rect.height <= 0.0 {
                return Vec::new();
            }
            let frame_alpha = frame.a as f32 / 255.0;
            let blend = if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal };
            let max_density = select_note_distribution_max_density(&row.chart_distribution) as f32;
            let count = row.chart_distribution.len().max(1) as f32;
            let pixel_w = 1.0 / self.w.max(1) as f32;
            let pixel_h = 1.0 / self.h.max(1) as f32;
            let gap_x = if graph.no_gap_x != 0 { 0.0 } else { pixel_w };
            let gap_y = if graph.no_gap != 0 { 0.0 } else { pixel_h };
            let bar_w = ((rect.width - gap_x * (count - 1.0)).max(pixel_w) / count).max(pixel_w);
            let colors = note_distribution_colors(frame_alpha);
            let mut items = Vec::new();
            if graph.back_tex_off == 0 {
                items.extend(select_note_distribution_background_items(
                    rect,
                    row.chart_distribution.len(),
                    max_density as u32,
                    frame_alpha,
                    blend,
                    pixel_w,
                    pixel_h,
                ));
            }
            let reveal = if graph.delay > 0 {
                (elapsed as f32 / graph.delay as f32).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let reveal_right = rect.x + rect.width * reveal;

            for (index, second) in row.chart_distribution.iter().enumerate() {
                let x = rect.x + index as f32 * (bar_w + gap_x);
                if x >= reveal_right {
                    break;
                }
                let visible_bar_w = bar_w.min((reveal_right - x).max(0.0));
                if visible_bar_w <= 0.0 {
                    continue;
                }
                let values = second.values();
                let iter: Box<dyn Iterator<Item = (usize, u16)>> = if graph.order_reverse != 0 {
                    Box::new(values.into_iter().enumerate().rev())
                } else {
                    Box::new(values.into_iter().enumerate())
                };
                let mut y_cursor = rect.y + rect.height;
                for (series, value) in iter {
                    if value == 0 {
                        continue;
                    }
                    let height = (rect.height * (value as f32 / max_density) - gap_y).max(pixel_h);
                    y_cursor -= height;
                    items.push(SkinRenderItem::Rect {
                        rect: Rect { x, y: y_cursor, width: visible_bar_w, height },
                        color: colors[series],
                        blend,
                    });
                    y_cursor -= gap_y;
                    if y_cursor <= rect.y {
                        break;
                    }
                }
            }

            items
        }

        fn select_bpmgraph_row_render_items(
            &self,
            row: &SelectRowSnapshot,
            graph: &SkinBpmGraphDef,
            destination: &SkinDestinationDef,
            row_origin: (i32, i32),
            enabled_options: &[i32],
            state: &SkinDrawState,
        ) -> Vec<SkinRenderItem> {
            if row.chart_bpm_graph_segments.is_empty()
                || !test_skin_ops(&destination.op, enabled_options, state)
                || !eval_skin_draw_condition(&destination.draw, state)
            {
                return Vec::new();
            }
            let Some(elapsed) = skin_timer_elapsed_ms(destination.timer, state) else {
                return Vec::new();
            };
            let Some(mut frame) =
                resolve_destination_frame(destination, elapsed, enabled_options, state)
            else {
                return Vec::new();
            };
            frame.x += row_origin.0;
            frame.y += row_origin.1;
            apply_skin_offset_to_frame(destination, &mut frame, state, false);
            if !destination_mouse_rect_contains(destination, frame, state) {
                return Vec::new();
            }
            self.bpmgraph_render_items_with_segments(
                graph,
                destination,
                frame,
                state,
                &row.chart_bpm_graph_segments,
            )
        }

        fn bpmgraph_render_items_with_segments(
            &self,
            graph: &SkinBpmGraphDef,
            destination: &SkinDestinationDef,
            frame: ResolvedSkinFrame,
            state: &SkinDrawState,
            segments: &[crate::chart_graph::BpmGraphSegment],
        ) -> Vec<SkinRenderItem> {
            if segments.is_empty() {
                return Vec::new();
            }
            let rect = normalize_skin_frame_rect(frame, self.w, self.h);
            let frame_alpha = frame.a as f32 / 255.0;
            let blend = if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal };
            let main_bpm = state.main_bpm.max(1.0);
            let canvas_w = self.w.max(1) as f32;
            let canvas_h = self.h.max(1) as f32;
            // lineWidth は canvas pixel 単位。正規化座標系に変換する。
            // 未指定 (0) のときは beatoraja デフォルトの 2 を使う。
            let canvas_line_px = if graph.line_width > 0 { graph.line_width } else { 2 } as f32;
            let line_w = canvas_line_px / canvas_w;
            let line_h = canvas_line_px / canvas_h;
            // beatoraja デフォルト色: main=緑, min=青, max=赤, other=黄, stop=紫, transition=灰
            let main_color = skin_hex_color(&graph.main_bpm_color)
                .unwrap_or(Color::rgba(0.0, 1.0, 0.0, 1.0))
                .with_alpha(frame_alpha);
            let min_color = skin_hex_color(&graph.min_bpm_color)
                .unwrap_or(Color::rgba(0.0, 0.0, 1.0, 1.0))
                .with_alpha(frame_alpha);
            let max_color = skin_hex_color(&graph.max_bpm_color)
                .unwrap_or(Color::rgba(1.0, 0.0, 0.0, 1.0))
                .with_alpha(frame_alpha);
            let other_color = skin_hex_color(&graph.other_bpm_color)
                .unwrap_or(Color::rgba(1.0, 1.0, 0.0, 1.0))
                .with_alpha(frame_alpha);
            let stop_color = skin_hex_color(&graph.stop_line_color)
                .unwrap_or(Color::rgba(1.0, 0.0, 1.0, 1.0))
                .with_alpha(frame_alpha);
            let transition_color = skin_hex_color(&graph.transition_line_color)
                .unwrap_or(Color::rgba(0.5, 0.5, 0.5, 1.0))
                .with_alpha(frame_alpha);
            // beatoraja: log10(bpm/mainbpm) を [log10(1/8), log10(8)] に正規化。
            // ratio=0 → グラフ上部 (低BPM / stop)、ratio=1 → グラフ下部 (高BPM)。
            let min_log: f32 = (1.0_f32 / 8.0).log10();
            let max_log: f32 = 8.0_f32.log10();
            let log_range = max_log - min_log;
            // bpm=0 (stop) は min 側にクランプされグラフ上部に描画される。
            let bpm_to_ratio = |bpm: f32| -> f32 {
                let r = (bpm / main_bpm).clamp(1.0 / 8.0, 8.0);
                ((r.log10() - min_log) / log_range).clamp(0.0, 1.0)
            };
            // ratio=0 → top (rect.y + rect.height)、ratio=1 → bottom (rect.y)
            let ratio_to_y =
                |ratio: f32| -> f32 { rect.y + rect.height * (1.0 - ratio) - line_h / 2.0 };
            let mut items = Vec::new();
            let mut prev_ratio: Option<f32> = None;
            for segment in segments {
                let x0 = rect.x + segment.start_ratio.clamp(0.0, 1.0) * rect.width;
                let x1 = rect.x + segment.end_ratio.clamp(0.0, 1.0) * rect.width;
                let bpm = if segment.is_stop { 0.0 } else { segment.bpm };
                let cur_ratio = bpm_to_ratio(bpm);
                // BPM変化点を transitionLineColor の縦線で繋ぐ (beatoraja 互換)。
                if let Some(prev) = prev_ratio {
                    let y_prev = ratio_to_y(prev);
                    let y_cur = ratio_to_y(cur_ratio);
                    let height = (y_prev - y_cur).abs() - line_h;
                    if height > 0.0 {
                        let y_bottom = y_prev.min(y_cur) + line_h;
                        items.push(SkinRenderItem::Rect {
                            rect: Rect { x: x0 - line_w / 2.0, y: y_bottom, width: line_w, height },
                            color: transition_color,
                            blend,
                        });
                    }
                }
                let y = ratio_to_y(cur_ratio);
                let color = if segment.is_stop {
                    stop_color
                } else if (segment.bpm - state.main_bpm).abs() < 0.5 {
                    main_color
                } else if (segment.bpm - state.min_bpm).abs() < 0.5 {
                    min_color
                } else if (segment.bpm - state.max_bpm).abs() < 0.5 {
                    max_color
                } else {
                    other_color
                };
                items.push(SkinRenderItem::Rect {
                    rect: Rect { x: x0, y, width: (x1 - x0).max(line_w), height: line_h },
                    color,
                    blend,
                });
                prev_ratio = Some(cur_ratio);
            }
            items
        }

        fn direct_source_image_render_item(
            &self,
            destination: &SkinDestinationDef,
            frame: ResolvedSkinFrame,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<SkinRenderItem> {
            let source_id = beatoraja_direct_image_source_id(&destination.id)?;
            let source = resolve_document_source(sources, &source_id)?;
            let uv = TextureRegion { x: 0.0, y: 0.0, width: 1.0, height: 1.0 };
            let (rect, uv) = stretch_skin_image_geometry(
                destination.stretch,
                normalize_skin_frame_rect(frame, self.w, self.h),
                uv,
                source.source_size,
                self.w,
                self.h,
            );
            Some(skin_image_item_for_frame(
                source.texture,
                rect,
                uv,
                frame,
                destination.center,
                if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal },
                Some(source.source_size),
                destination.filter != 0,
            ))
        }

        fn slider_render_item(
            &self,
            slider: &SkinSliderDef,
            destination: &SkinDestinationDef,
            frame: ResolvedSkinFrame,
            state: &SkinDrawState,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<SkinRenderItem> {
            let progress = skin_slider_progress(slider, state)?;
            let source = sources.get(&slider.src)?;
            let source_width = source.source_size.width.max(1.0);
            let source_height = source.source_size.height.max(1.0);
            let mut frame = frame;
            let offset = (slider.range as f32 * progress).round() as i32;
            match slider.angle {
                0 => frame.y += offset,
                1 => frame.x += offset,
                2 => frame.y -= offset,
                3 => frame.x -= offset,
                _ => {}
            }
            let mut uv = TextureRegion {
                x: slider.x as f32 / source_width,
                y: slider.y as f32 / source_height,
                width: slider.w as f32 / source_width,
                height: slider.h as f32 / source_height,
            };
            if slider.slider_type == 4
                && let Some((disappear_line, link_lift)) = self.disappear_line_for_lane_cover_clip()
            {
                clip_skin_cover_to_disappear_line(
                    &mut frame,
                    &mut uv,
                    disappear_line,
                    link_lift,
                    state,
                );
                if frame.h <= 0 {
                    return None;
                }
            }
            let (rect, uv) = stretch_skin_image_geometry(
                destination.stretch,
                normalize_skin_frame_rect(frame, self.w, self.h),
                uv,
                source.source_size,
                self.w,
                self.h,
            );
            Some(SkinRenderItem::Image {
                texture: source.texture,
                rect,
                uv,
                tint: Color::rgba(
                    frame.r as f32 / 255.0,
                    frame.g as f32 / 255.0,
                    frame.b as f32 / 255.0,
                    frame.a as f32 / 255.0,
                ),
                blend: if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal },
                scale: SkinImageScale::Stretch,
                border: None,
                source_size: Some(source.source_size),
                linear_filter: destination.filter != 0,
            })
        }

        fn hidden_cover_render_item(
            &self,
            cover: &SkinHiddenCoverDef,
            destination: &SkinDestinationDef,
            frame: ResolvedSkinFrame,
            force_lift_cover: bool,
            state: &SkinDrawState,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<SkinRenderItem> {
            let is_lift_cover = force_lift_cover
                || is_lift_lane_cover_id(&cover.id)
                || is_lift_lane_cover_id(&destination.id);
            if is_lift_cover {
                if state.offset_lift_px <= 0 {
                    return None;
                }
            } else if state.hidden_cover <= 0.0 {
                return None;
            }
            let source = sources.get(&cover.src)?;
            let source_width = source.source_size.width.max(1.0);
            let source_height = source.source_size.height.max(1.0);
            let mut frame = frame;
            let mut uv = TextureRegion {
                x: cover.x as f32 / source_width,
                y: cover.y as f32 / source_height,
                width: cover.w as f32 / source_width,
                height: cover.h as f32 / source_height,
            };
            clip_skin_cover_to_disappear_line(
                &mut frame,
                &mut uv,
                cover.disappear_line,
                cover.is_disappear_line_link_lift,
                state,
            );
            if frame.h <= 0 {
                return None;
            }
            let (rect, uv) = stretch_skin_image_geometry(
                destination.stretch,
                normalize_skin_frame_rect(frame, self.w, self.h),
                uv,
                source.source_size,
                self.w,
                self.h,
            );
            Some(SkinRenderItem::Image {
                texture: source.texture,
                rect,
                uv,
                tint: Color::rgba(
                    frame.r as f32 / 255.0,
                    frame.g as f32 / 255.0,
                    frame.b as f32 / 255.0,
                    frame.a as f32 / 255.0,
                ),
                blend: if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal },
                scale: SkinImageScale::Stretch,
                border: None,
                source_size: Some(source.source_size),
                linear_filter: destination.filter != 0,
            })
        }

        fn graph_render_item(
            &self,
            graph: &SkinGraphDef,
            frame: ResolvedSkinFrame,
            state: &SkinDrawState,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<SkinRenderItem> {
            let source = sources.get(&graph.src)?;
            let (fill_multiplier, uv_ratio) = graph_fill_dimensions(graph, state);
            let fill_from_right = frame.w < 0;
            let source_w = source.source_size.width.max(1.0);
            let source_h = source.source_size.height.max(1.0);
            let base_uv = TextureRegion {
                x: graph.x as f32 / source_w,
                y: graph.y as f32 / source_h,
                width: graph.w as f32 / source_w,
                height: graph.h as f32 / source_h,
            };
            let dst = normalize_skin_frame_rect(frame, self.w, self.h);
            let (rect, uv) = if graph.angle == 1 {
                // vertical: fill from bottom up
                let clipped_h = dst.height * fill_multiplier;
                let uv_offset = base_uv.height * (1.0 - uv_ratio);
                (
                    Rect { y: dst.y + dst.height - clipped_h, height: clipped_h, ..dst },
                    TextureRegion {
                        y: base_uv.y + uv_offset,
                        height: base_uv.height * uv_ratio,
                        ..base_uv
                    },
                )
            } else {
                // horizontal: positive destinations fill from left. beatoraja keeps a
                // negative destination width and therefore fills leftwards from the
                // destination x; after rect normalization that is the right edge.
                let clipped_w = dst.width * fill_multiplier;
                (
                    Rect {
                        x: if fill_from_right { dst.x + dst.width - clipped_w } else { dst.x },
                        width: clipped_w,
                        ..dst
                    },
                    TextureRegion { width: base_uv.width * uv_ratio, ..base_uv },
                )
            };
            if rect.width <= 0.0 || rect.height <= 0.0 {
                return None;
            }
            Some(SkinRenderItem::Image {
                texture: source.texture,
                rect,
                uv,
                tint: Color::rgba(
                    frame.r as f32 / 255.0,
                    frame.g as f32 / 255.0,
                    frame.b as f32 / 255.0,
                    frame.a as f32 / 255.0,
                ),
                blend: BlendMode::Normal,
                scale: SkinImageScale::Stretch,
                border: None,
                source_size: Some(source.source_size),
                linear_filter: false,
            })
        }
    };
}

pub(super) use skin_document_render_graph_methods;
