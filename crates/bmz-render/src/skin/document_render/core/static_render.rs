macro_rules! skin_document_render_core_static_methods {
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
            let destinations = if planning.is_none() {
                self.all_destinations(enabled_options)
            } else {
                Vec::new()
            };
            let destination_count = planning
                .as_ref()
                .map_or(destinations.len(), |planning| planning.destinations.len());
            let has_nearest_f_diff_rank_destination = planning.as_ref().map_or_else(
                || nearest_f_diff_rank_destination_available(&destinations),
                |planning| planning.has_nearest_f_diff_rank_destination,
            );
            let state =
                apply_nearest_f_diff_rank_fallback(state, has_nearest_f_diff_rank_destination);
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
    };
}

pub(in crate::skin::document_render) use skin_document_render_core_static_methods;
