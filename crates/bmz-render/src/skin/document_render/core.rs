macro_rules! skin_document_render_core_methods {
    () => {
    fn static_image_render_items(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem> {
        self.static_render_items(sources, state, &SkinTextState::default())
    }

    fn static_render_items(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
        text_state: &SkinTextState<'_>,
    ) -> Vec<SkinRenderItem> {
        self.static_render_items_with_graphs(
            sources,
            state,
            text_state,
            SkinRuntimeGraphs::from_document(self),
        )
    }

    fn static_render_items_with_graphs(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
        text_state: &SkinTextState<'_>,
        runtime_graphs: SkinRuntimeGraphs<'_>,
    ) -> Vec<SkinRenderItem> {
        self.static_render_items_with_graphs_cached(
            sources,
            state,
            text_state,
            runtime_graphs,
            None,
        )
    }

    fn static_render_items_with_graphs_cached(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
        text_state: &SkinTextState<'_>,
        runtime_graphs: SkinRuntimeGraphs<'_>,
        cache: Option<&mut ResultRenderCache>,
    ) -> Vec<SkinRenderItem> {
        let (mut behind, front, failed_overlay) = self.static_render_items_split_with_graphs(
            sources,
            state,
            text_state,
            runtime_graphs,
            cache,
        );
        behind.extend(front);
        behind.extend(failed_overlay);
        behind
    }

    /// 静的 destination を `{"id":"notes"}` マーカーと `timer: 3` で3分割して描画アイテムを返す。
    /// 戻り値 `.0` はノーツより背面、`.1` はノーツより前面、`.2` は FAILED オーバーレイ。
    fn static_render_items_split(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
        text_state: &SkinTextState<'_>,
    ) -> (Vec<SkinRenderItem>, Vec<SkinRenderItem>, Vec<SkinRenderItem>) {
        self.static_render_items_split_with_graphs(
            sources,
            state,
            text_state,
            SkinRuntimeGraphs::from_document(self),
            None,
        )
    }

    fn static_render_items_split_with_graphs(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
        text_state: &SkinTextState<'_>,
        runtime_graphs: SkinRuntimeGraphs<'_>,
        mut cache: Option<&mut ResultRenderCache>,
    ) -> (Vec<SkinRenderItem>, Vec<SkinRenderItem>, Vec<SkinRenderItem>) {
        let images = self.image_map();
        let values: HashMap<&str, &SkinValueDef> =
            self.value.iter().map(|value| (value.id.as_str(), value)).collect();
        let planning = cache.as_deref_mut().map(|cache| cache.cached_planning(self));
        let enabled_options_storage =
            if planning.is_none() { self.enabled_options() } else { Vec::new() };
        let enabled_options: &[i32] =
            planning.as_ref().map_or(enabled_options_storage.as_slice(), |planning| {
                planning.enabled_options.as_ref()
            });
        let mut behind = Vec::new();
        let mut front = Vec::new();
        let mut failed_overlay = Vec::new();
        let mut after_notes_marker = false;
        let destinations =
            if planning.is_none() { self.all_destinations(enabled_options) } else { Vec::new() };
        let destination_count =
            planning.as_ref().map_or(destinations.len(), |planning| planning.destinations.len());
        let has_nearest_f_diff_rank_destination = planning.as_ref().map_or_else(
            || nearest_f_diff_rank_destination_available(&destinations),
            |planning| planning.has_nearest_f_diff_rank_destination,
        );
        let state = apply_nearest_f_diff_rank_fallback(state, has_nearest_f_diff_rank_destination);
        let state = state.as_ref();
        for index in 0..destination_count {
            let Some(destination) = planning
                .as_ref()
                .and_then(|planning| planning.destinations.get(index).copied())
                .and_then(|destination| destination.resolve(self))
                .or_else(|| destinations.get(index).copied())
            else {
                continue;
            };
            // `{"id":"notes"}` はノーツ描画位置マーカー。以降の destination はノーツ前面に積む。
            if destination.id == "notes" {
                after_notes_marker = true;
                continue;
            }
            if !destination.op.is_empty()
                && !destination_ops_match(
                    destination,
                    enabled_options,
                    state,
                    has_nearest_f_diff_rank_destination,
                )
            {
                continue;
            }
            if !destination.draw.trim().is_empty()
                && !eval_skin_draw_condition(&destination.draw, state)
            {
                continue;
            }
            if let Some(item) = self.result_judge_pie_destination_item(
                destination,
                &images,
                enabled_options,
                state,
                sources,
            ) {
                let target = destination_render_layer(
                    destination.timer,
                    after_notes_marker,
                    &mut behind,
                    &mut front,
                    &mut failed_overlay,
                );
                target.push(item);
                continue;
            }
            if self.destination_uses_skin_gauge_bar_render(destination) {
                if let Some(items) = self.resolve_gauge_destination_items(
                    destination,
                    enabled_options,
                    state,
                    sources,
                ) {
                    let target = destination_render_layer(
                        destination.timer,
                        after_notes_marker,
                        &mut behind,
                        &mut front,
                        &mut failed_overlay,
                    );
                    target.extend(items);
                }
                continue;
            }
            if let Some(items) = self.resolve_destination_items(
                index,
                destination,
                DestinationResolveContext {
                    images: &images,
                    values: &values,
                    enabled_options,
                    state,
                    text_state,
                    sources,
                    runtime_graphs,
                    has_nearest_f_diff_rank_destination,
                    cache: cache.as_deref_mut(),
                },
            ) {
                let after_notes_marker = after_notes_marker
                    || self.destination_looks_like_pre_notes_judge_line(
                        destination,
                        &images,
                        enabled_options,
                        state,
                        planning
                            .as_ref()
                            .and_then(|planning| planning.destinations.get(index + 1).copied())
                            .and_then(|destination| destination.resolve(self))
                            .or_else(|| destinations.get(index + 1).copied()),
                    );
                let target = destination_render_layer(
                    destination.timer,
                    after_notes_marker,
                    &mut behind,
                    &mut front,
                    &mut failed_overlay,
                );
                target.extend(items);
            }
        }
        (behind, front, failed_overlay)
    }

    fn result_judge_pie_destination_item(
        &self,
        destination: &SkinDestinationDef,
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem> {
        if state.result_failed.is_none() || destination.id != "judge_graph" {
            return None;
        }
        let elapsed = skin_timer_elapsed_ms(destination.timer, state)?;
        let mut frame = resolve_destination_frame(destination, elapsed, enabled_options, state)?;
        let image = skin_image_for_destination_id(destination.id.as_str(), images)?;
        let is_hidden_cover_destination = self
            .hidden_cover
            .iter()
            .any(|cover| cover.id == destination.id && !is_lift_lane_cover_id(&cover.id));
        apply_skin_offset_to_frame(destination, &mut frame, state, is_hidden_cover_destination);
        if !destination_mouse_rect_contains(destination, frame, state) {
            return None;
        }
        let (r, g, b) = result_judge_pie_segment_color(destination, image, frame, state)?;
        frame.r = r;
        frame.g = g;
        frame.b = b;
        let source = resolve_document_source(sources, &image.src)?;
        let pixel_rect = skin_image_pixel_rect(image, images);
        let uv = skin_image_texture_region_for_state(
            image,
            source.source_size,
            elapsed,
            Some(state),
            pixel_rect,
        );
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

    fn destination_looks_like_pre_notes_judge_line(
        &self,
        destination: &SkinDestinationDef,
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        next_destination: Option<&SkinDestinationDef>,
    ) -> bool {
        if !matches!(next_destination, Some(next) if next.id == "notes")
            || destination.timer.is_some()
            || !destination_uses_lift_offset_only(destination)
            || skin_image_for_destination_id(destination.id.as_str(), images).is_none()
        {
            return false;
        }
        let Some(frame) = resolve_destination_frame(destination, 0, enabled_options, state) else {
            return false;
        };
        if frame.w < 100 || frame.h <= 0 || frame.h > 48 {
            return false;
        }
        let Some(note) = &self.note else {
            return false;
        };
        flatten_dst_entries(&note.dst, enabled_options).into_iter().any(|note_frame| {
            let Some(note_y) = note_frame.y else {
                return false;
            };
            frame.y >= note_y && frame.y <= note_y.saturating_add(64)
        })
    }

    /// `hiddenCover.disapearLine` をレーンカバー系 (HIDDEN / SUDDEN+ / LIFT) のクロップ境界として使う。
    fn disappear_line_for_lane_cover_clip(&self) -> Option<(i32, bool)> {
        let cover = self.hidden_cover.first()?;
        (cover.disappear_line > 0)
            .then_some((cover.disappear_line, cover.is_disappear_line_link_lift))
    }

    fn should_clip_image_at_disappear_line(
        &self,
        destination: &SkinDestinationDef,
        image: &SkinImageDef,
    ) -> bool {
        if self.hidden_cover.is_empty() {
            return false;
        }
        if is_lift_lane_cover_id(&destination.id) || is_lift_lane_cover_id(&image.id) {
            return true;
        }
        destination_uses_lift_offset_only(destination)
            && self.hidden_cover.iter().any(|cover| cover.src == image.src)
    }

    /// `liftcover` 系 ID のみ。`offset: 3` だけの destination (判定線・数値表示など) は対象外。
    fn should_skip_lift_lane_cover_render(
        &self,
        destination: &SkinDestinationDef,
        image: &SkinImageDef,
    ) -> bool {
        is_lift_lane_cover_id(&destination.id) || is_lift_lane_cover_id(&image.id)
    }

    /// LIFT 用 image は `offset: 3` で既にリフト分だけ動くため、`hiddenCover` の
    /// `isDisappearLineLinkLift` は二重適用しない。
    fn link_lift_for_lane_cover_clip(
        &self,
        destination: &SkinDestinationDef,
        image: &SkinImageDef,
        link_lift: bool,
    ) -> bool {
        if is_lift_lane_cover_id(&destination.id)
            || is_lift_lane_cover_id(&image.id)
            || destination_uses_lift_offset_only(destination)
        {
            return false;
        }
        link_lift
    }

    fn resolve_destination_items(
        &self,
        destination_index: usize,
        destination: &SkinDestinationDef,
        context: DestinationResolveContext<'_, '_>,
    ) -> Option<Vec<SkinRenderItem>> {
        let DestinationResolveContext {
            images,
            values,
            enabled_options,
            state,
            text_state,
            sources,
            runtime_graphs,
            has_nearest_f_diff_rank_destination,
            cache,
        } = context;
        let state = apply_nearest_f_diff_rank_fallback(state, has_nearest_f_diff_rank_destination);
        let state = state.as_ref();
        if let Some(judge_def) = self.judge.iter().find(|judge| judge.id == destination.id) {
            let region = judge_def.index.clamp(0, MAX_JUDGE_REGIONS as i32 - 1) as usize;
            let elapsed = state.judge_ms[region]?;
            let judge_image_index = state.judge_index[region]?;
            return self.judge_render_items_for_def(
                judge_def,
                judge_image_index,
                state.judge_combo[region],
                elapsed,
                sources,
                state,
            );
        }

        let value_for_destination = values.get(destination.id.as_str()).copied();
        let elapsed = destination_timer_elapsed_ms(destination, state).or_else(|| {
            value_for_destination
                .filter(|value| pre_ready_lane_cover_value_destination(destination, value, state))
                .map(|_| 0)
        })?;
        let mut frame = resolve_destination_frame(destination, elapsed, enabled_options, state)?;
        let is_hidden_cover_destination = self
            .hidden_cover
            .iter()
            .any(|cover| cover.id == destination.id && !is_lift_lane_cover_id(&cover.id));
        let is_lift_cover_destination =
            self.lift_cover.iter().any(|cover| cover.id == destination.id);
        apply_skin_offset_to_frame(destination, &mut frame, state, is_hidden_cover_destination);
        if is_lift_cover_destination && !destination_uses_skin_offset(destination, 3) {
            apply_skin_offset_ids_to_frame(&[3], &mut frame, state, false);
        }
        if !destination_mouse_rect_contains(destination, frame, state) {
            return None;
        }
        if let Some(panel) = self.panel.iter().find(|panel| panel.id == destination.id) {
            return Some(skin_panel_render_items(panel, destination, frame, self.w, self.h));
        }
        if let Some(visualizer) =
            self.hiterror_visualizer.iter().find(|visualizer| visualizer.id == destination.id)
        {
            return Some(self.hiterror_visualizer_render_items(
                visualizer,
                destination,
                frame,
                state,
            ));
        }
        if let Some(visualizer) =
            self.timingvisualizer.iter().find(|visualizer| visualizer.id == destination.id)
        {
            return Some(self.timing_visualizer_render_items(
                visualizer,
                destination,
                frame,
                state,
                runtime_graphs.result_timing_points,
            ));
        }
        if let Some(graph) =
            self.timingdistributiongraph.iter().find(|graph| graph.id == destination.id)
        {
            return Some(self.timing_distribution_graph_render_items(
                graph,
                destination,
                frame,
                state,
                runtime_graphs.result_timing_points,
                runtime_graphs.result_timing_distribution,
            ));
        }
        if let Some(gauge_graph) = self.gaugegraph.iter().find(|graph| graph.id == destination.id) {
            return Some(self.gaugegraph_render_items(
                destination_index,
                gauge_graph,
                destination,
                frame,
                state,
                runtime_graphs.result_gauge_graph_points,
                cache,
            ));
        }
        if let Some(judge_graph) = self.judgegraph.iter().find(|graph| graph.id == destination.id) {
            return Some(self.judgegraph_render_items(
                destination_index,
                judge_graph,
                destination,
                frame,
                elapsed,
                state,
                runtime_graphs,
                cache,
            ));
        }
        if let Some(bpm_graph) = self.bpmgraph.iter().find(|graph| graph.id == destination.id) {
            return Some(self.bpmgraph_render_items_with_segments(
                bpm_graph,
                destination,
                frame,
                state,
                runtime_graphs.play_bpm_graph_segments,
            ));
        }
        if let Some(item) = self.direct_source_image_render_item(destination, frame, sources) {
            return Some(vec![item]);
        }
        if let Some(image) = skin_image_for_destination_id(destination.id.as_str(), images) {
            if self.should_skip_lift_lane_cover_render(destination, image)
                && state.offset_lift_px == 0
            {
                return None;
            }
            if let Some((r, g, b)) =
                result_judge_pie_segment_color(destination, image, frame, state)
            {
                frame.r = r;
                frame.g = g;
                frame.b = b;
            }
            let source = resolve_document_source(sources, &image.src)?;
            let pixel_rect = skin_image_pixel_rect(image, images);
            let mut uv = skin_image_texture_region_for_state(
                image,
                source.source_size,
                elapsed,
                Some(state),
                pixel_rect,
            );
            if self.should_clip_image_at_disappear_line(destination, image)
                && let Some((disappear_line, link_lift)) = self.disappear_line_for_lane_cover_clip()
            {
                clip_skin_cover_to_disappear_line(
                    &mut frame,
                    &mut uv,
                    disappear_line,
                    self.link_lift_for_lane_cover_clip(destination, image, link_lift),
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
            return Some(vec![skin_image_item_for_frame(
                source.texture,
                rect,
                uv,
                frame,
                destination.center,
                if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal },
                Some(source.source_size),
                destination.filter != 0,
            )]);
        }

        if self.bga.as_ref().is_some_and(|bga| bga.id == destination.id) {
            return (state.has_bga && state.bga_enabled).then(|| {
                let rect = normalize_skin_frame_rect(frame, self.w, self.h);
                let blend = if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal };
                let destination_tint = Color::rgba(1.0, 1.0, 1.0, frame.a as f32 / 255.0);
                let stretch =
                    if destination.stretch < 0 { state.bga_stretch } else { destination.stretch };
                let mut items = Vec::new();
                if let Some(bga) = state.bga_poor {
                    let tint = multiply_bga_tints(destination_tint, bga);
                    items.push(bga_image_item(
                        bga,
                        stretch,
                        rect,
                        tint,
                        blend,
                        self.w,
                        self.h,
                        destination.filter != 0,
                    ));
                } else if let Some(bga) = state.bga_base {
                    let tint = multiply_bga_tints(destination_tint, bga);
                    items.push(bga_image_item(
                        bga,
                        stretch,
                        rect,
                        tint,
                        blend,
                        self.w,
                        self.h,
                        destination.filter != 0,
                    ));
                }
                // Layer / Layer2 は beatoraja の TYPE_LAYER と同様、黒ピクセルを
                // 透過させて Base に重ねる。例外として:
                //   - Add 指定時はクロマキー不要 (黒は加算寄与ゼロ)
                //   - 動画 BGA Layer は beatoraja でも `ffmpeg.frag` を使い
                //     クロマキーをかけない
                let layer_blend_for = |bga: SkinBgaFrame| {
                    if matches!(blend, BlendMode::Add) || bga.is_video {
                        blend
                    } else {
                        BlendMode::LayerMask
                    }
                };
                if state.bga_poor.is_none()
                    && let Some(bga) = state.bga_layer
                {
                    let tint = multiply_bga_tints(destination_tint, bga);
                    items.push(bga_image_item(
                        bga,
                        stretch,
                        rect,
                        tint,
                        layer_blend_for(bga),
                        self.w,
                        self.h,
                        destination.filter != 0,
                    ));
                }
                if state.bga_poor.is_none()
                    && let Some(bga) = state.bga_layer2
                {
                    let tint = multiply_bga_tints(destination_tint, bga);
                    items.push(bga_image_item(
                        bga,
                        stretch,
                        rect,
                        tint,
                        layer_blend_for(bga),
                        self.w,
                        self.h,
                        destination.filter != 0,
                    ));
                }
                if items.is_empty() {
                    items.push(SkinRenderItem::Rect {
                        rect,
                        color: Color::rgba(0.0, 0.0, 0.0, frame.a as f32 / 255.0),
                        blend,
                    });
                }
                items
            });
        }

        // imageset (キービーム・ボム等) を destination 自身のタイマー駆動で描画する。
        // timer が非アクティブな destination は上の skin_timer_elapsed_ms で除外済み。
        if let Some(imageset) = self.imageset.iter().find(|set| set.id == destination.id) {
            let image_id = if let Some(index) = skin_state_imageset_index(imageset.ref_id, state) {
                imageset.images.get(index.min(imageset.images.len().saturating_sub(1))).cloned()
            } else {
                let judge_index = imageset_ref_lane(imageset.ref_id)
                    .and_then(|lane| state.lane_judge[lane.index()]);
                imageset_image_for_index(imageset, judge_index)
            }?;
            let image = images.get(image_id.as_str())?;
            let source = resolve_document_source(sources, &image.src)?;
            let pixel_rect = skin_image_pixel_rect(image, images);
            let (rect, uv) = stretch_skin_image_geometry(
                destination.stretch,
                normalize_skin_frame_rect(frame, self.w, self.h),
                skin_image_texture_region_for_state(
                    image,
                    source.source_size,
                    elapsed,
                    Some(state),
                    pixel_rect,
                ),
                source.source_size,
                self.w,
                self.h,
            );
            return Some(vec![skin_image_item_for_frame(
                source.texture,
                rect,
                uv,
                frame,
                destination.center,
                if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal },
                Some(source.source_size),
                destination.filter != 0,
            )]);
        }

        if let Some(value) = value_for_destination {
            let number = skin_value_number_for_destination(
                value,
                state,
                has_nearest_f_diff_rank_destination,
            )?;
            let signed_render = signed_number_render_for_value(value, state);
            return Some(self.value_number_render_items(
                &value.id,
                number,
                ResolvedSkinFrame::default(),
                frame,
                elapsed,
                sources,
                false,
                None,
                signed_render,
            ));
        }

        if let Some(graph) = self.graph.iter().find(|graph| graph.id == destination.id) {
            return self.graph_render_item(graph, frame, state, sources).map(|item| vec![item]);
        }

        if let Some(text) = self.text.iter().find(|text| text.id == destination.id)
            && let Some(item) =
                self.text_render_item_with_draw_state(text, frame, Some(state), text_state)
        {
            return Some(vec![item]);
        }

        if let Some(slider) = self.slider.iter().find(|slider| slider.id == destination.id)
            && let Some(item) = self.slider_render_item(slider, destination, frame, state, sources)
        {
            return Some(vec![item]);
        }

        if self.destination_uses_skin_gauge_overlay_render(destination) {
            return self.resolve_gauge_destination_items(
                destination,
                enabled_options,
                state,
                sources,
            );
        }

        if let Some(item) = special_image_render_item(destination, frame, self.w, self.h) {
            return Some(vec![item]);
        }

        if let Some(lift_cover) = self.lift_cover.iter().find(|cover| cover.id == destination.id) {
            return self
                .hidden_cover_render_item(lift_cover, destination, frame, true, state, sources)
                .map(|item| vec![item]);
        }
        let hidden_cover = self.hidden_cover.iter().find(|cover| cover.id == destination.id)?;
        self.hidden_cover_render_item(hidden_cover, destination, frame, false, state, sources)
            .map(|item| vec![item])
    }

    fn resolve_offset_destination_items(
        &self,
        destination: &SkinDestinationDef,
        offset: (i32, i32),
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        text_state: &SkinTextState<'_>,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<Vec<SkinRenderItem>> {
        let destinations = self.all_destinations(enabled_options);
        let has_nearest_f_diff_rank_destination =
            nearest_f_diff_rank_destination_available(&destinations);
        let state = apply_nearest_f_diff_rank_fallback(state, has_nearest_f_diff_rank_destination);
        let state = state.as_ref();
        if !destination_ops_match(
            destination,
            enabled_options,
            state,
            has_nearest_f_diff_rank_destination,
        ) || !eval_skin_draw_condition(&destination.draw, state)
        {
            return None;
        }
        let elapsed = skin_timer_elapsed_ms(destination.timer, state)?;
        let mut frame = resolve_destination_frame(destination, elapsed, enabled_options, state)?;
        frame.x += offset.0;
        frame.y += offset.1;
        apply_skin_offset_to_frame(destination, &mut frame, state, false);

        if let Some(panel) = self.panel.iter().find(|panel| panel.id == destination.id) {
            return Some(skin_panel_render_items(panel, destination, frame, self.w, self.h));
        }

        if let Some(image) = skin_image_for_destination_id(destination.id.as_str(), images) {
            if self.should_skip_lift_lane_cover_render(destination, image)
                && state.offset_lift_px == 0
            {
                return None;
            }
            let source = resolve_document_source(sources, &image.src)?;
            let pixel_rect = skin_image_pixel_rect(image, images);
            let mut uv = skin_image_texture_region_for_state(
                image,
                source.source_size,
                elapsed,
                Some(state),
                pixel_rect,
            );
            if self.should_clip_image_at_disappear_line(destination, image)
                && let Some((disappear_line, link_lift)) = self.disappear_line_for_lane_cover_clip()
            {
                clip_skin_cover_to_disappear_line(
                    &mut frame,
                    &mut uv,
                    disappear_line,
                    self.link_lift_for_lane_cover_clip(destination, image, link_lift),
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
            return Some(vec![skin_image_item_for_frame(
                source.texture,
                rect,
                uv,
                frame,
                destination.center,
                if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal },
                Some(source.source_size),
                destination.filter != 0,
            )]);
        }

        if let Some(value) = self.value.iter().find(|value| value.id == destination.id) {
            let number = skin_value_number_for_destination(
                value,
                state,
                has_nearest_f_diff_rank_destination,
            )?;
            let signed_render = signed_number_render_for_value(value, state);
            return Some(self.value_number_render_items(
                &value.id,
                number,
                ResolvedSkinFrame::default(),
                frame,
                elapsed,
                sources,
                false,
                None,
                signed_render,
            ));
        }

        if let Some(graph) = self.graph.iter().find(|graph| graph.id == destination.id)
            && let Some(item) = self.graph_render_item(graph, frame, state, sources)
        {
            return Some(vec![item]);
        }

        if let Some(text) = self.text.iter().find(|text| text.id == destination.id)
            && let Some(item) =
                self.text_render_item_with_draw_state(text, frame, Some(state), text_state)
        {
            return Some(vec![item]);
        }

        None
    }
    };
}

pub(super) use skin_document_render_core_methods;
