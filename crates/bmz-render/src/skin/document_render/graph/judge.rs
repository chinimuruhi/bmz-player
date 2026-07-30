macro_rules! skin_document_render_graph_judge_methods {
    () => {
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
    };
}

pub(in crate::skin::document_render) use skin_document_render_graph_judge_methods;
