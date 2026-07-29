use super::*;

/// `SkinDocument` (bmz-skin-document) に対する描画評価の拡張 trait。
///
/// document スキーマ本体は `bmz-skin-document` crate へ移動したため、
/// `SkinDrawState` / `SkinRenderItem` 等の描画型に依存する評価メソッドは
/// foreign type への inherent impl ができず、この拡張 trait で提供する。
/// 実装は `impl SkinDocumentRenderExt for SkinDocument` の 1 つだけを想定する。
///
/// 旧 inherent impl の private ヘルパーメソッドも trait へ機械的に移した
/// ため、`SkinRuntimeGraphs` 等の crate 内 private 型がシグネチャに現れる。
/// これらは外部から呼べない (引数型を名指しできない) ので lint を許可する。
#[allow(private_interfaces)]
pub trait SkinDocumentRenderExt {
    fn static_image_render_items(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem>;

    fn static_render_items(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
        text_state: &SkinTextState<'_>,
    ) -> Vec<SkinRenderItem>;

    fn static_render_items_with_graphs(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
        text_state: &SkinTextState<'_>,
        runtime_graphs: SkinRuntimeGraphs<'_>,
    ) -> Vec<SkinRenderItem>;

    fn static_render_items_with_graphs_cached(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
        text_state: &SkinTextState<'_>,
        runtime_graphs: SkinRuntimeGraphs<'_>,
        cache: Option<&mut ResultRenderCache>,
    ) -> Vec<SkinRenderItem>;

    fn static_render_items_split(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
        text_state: &SkinTextState<'_>,
    ) -> (Vec<SkinRenderItem>, Vec<SkinRenderItem>, Vec<SkinRenderItem>);

    fn static_render_items_split_with_graphs(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
        text_state: &SkinTextState<'_>,
        runtime_graphs: SkinRuntimeGraphs<'_>,
        cache: Option<&mut ResultRenderCache>,
    ) -> (Vec<SkinRenderItem>, Vec<SkinRenderItem>, Vec<SkinRenderItem>);

    fn result_judge_pie_destination_item(
        &self,
        destination: &SkinDestinationDef,
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;

    fn destination_looks_like_pre_notes_judge_line(
        &self,
        destination: &SkinDestinationDef,
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        next_destination: Option<&SkinDestinationDef>,
    ) -> bool;

    fn disappear_line_for_lane_cover_clip(&self) -> Option<(i32, bool)>;

    fn should_clip_image_at_disappear_line(
        &self,
        destination: &SkinDestinationDef,
        image: &SkinImageDef,
    ) -> bool;

    fn should_skip_lift_lane_cover_render(
        &self,
        destination: &SkinDestinationDef,
        image: &SkinImageDef,
    ) -> bool;

    fn link_lift_for_lane_cover_clip(
        &self,
        destination: &SkinDestinationDef,
        image: &SkinImageDef,
        link_lift: bool,
    ) -> bool;

    fn resolve_destination_items(
        &self,
        destination_index: usize,
        destination: &SkinDestinationDef,
        context: DestinationResolveContext<'_, '_>,
    ) -> Option<Vec<SkinRenderItem>>;

    fn resolve_offset_destination_items(
        &self,
        destination: &SkinDestinationDef,
        offset: (i32, i32),
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        text_state: &SkinTextState<'_>,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<Vec<SkinRenderItem>>;

    fn select_render_items(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        snapshot: &SelectSnapshot,
    ) -> Vec<SkinRenderItem>;

    fn select_render_items_with_dynamic_timers(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        snapshot: &SelectSnapshot,
        dynamic_timers: Option<&mut DynamicTimerRuntime>,
        settings_dest_index: &crate::select_settings_dest::SelectSettingsDestIndex,
        lua_draw_runtime: Option<Arc<dyn SkinLuaDrawRuntime>>,
    ) -> Vec<SkinRenderItem>;

    fn select_draw_state<'a>(
        &self,
        snapshot: &'a SelectSnapshot,
        dynamic_timers: Option<&mut DynamicTimerRuntime>,
    ) -> (SkinDrawState, Option<&'a SelectRowSnapshot>);

    fn select_click_hit(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        snapshot: &SelectSnapshot,
        settings_dest_index: &crate::select_settings_dest::SelectSettingsDestIndex,
        x: f32,
        y: f32,
    ) -> Option<SkinClickHit>;

    fn result_click_hit(&self, state: &SkinDrawState, x: f32, y: f32) -> Option<SkinClickHit>;

    fn result_slider_hit(&self, state: &SkinDrawState, x: f32, y: f32) -> Option<SkinSliderHit>;

    fn select_slider_hit(
        &self,
        snapshot: &SelectSnapshot,
        settings_dest_index: &crate::select_settings_dest::SelectSettingsDestIndex,
        x: f32,
        y: f32,
    ) -> Option<SkinSliderHit>;

    fn select_click_hits(
        &self,
        _sources: &HashMap<String, SkinDocumentTexture>,
        snapshot: &SelectSnapshot,
        settings_dest_index: &crate::select_settings_dest::SelectSettingsDestIndex,
    ) -> Vec<SkinClickHit>;

    fn select_songlist_click_hits(
        &self,
        snapshot: &SelectSnapshot,
        enabled_options: &[i32],
        state: &SkinDrawState,
    ) -> Vec<SkinClickHit>;

    fn apply_select_songlist_render_row_state(state: &mut SkinDrawState, row: &SelectRowSnapshot);

    fn apply_select_songlist_click_row_state(state: &mut SkinDrawState, row: &SelectRowSnapshot);

    fn click_target_for_destination(
        &self,
        destination: &SkinDestinationDef,
        images: &HashMap<&str, &SkinImageDef>,
    ) -> Option<SkinClickTarget>;

    fn destination_click_rect(
        &self,
        destination: &SkinDestinationDef,
        enabled_options: &[i32],
        state: &SkinDrawState,
    ) -> Option<Rect>;

    fn destination_slider_hit(
        &self,
        slider: &SkinSliderDef,
        destination: &SkinDestinationDef,
        enabled_options: &[i32],
        state: &SkinDrawState,
        x: f32,
        y: f32,
    ) -> Option<SkinSliderHit>;

    fn select_songlist_items(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        snapshot: &SelectSnapshot,
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem>;

    fn apply_select_songlist_scroll_to_frame(
        &self,
        frame: &mut ResolvedSkinFrame,
        songlist: &SkinSongListDef,
        slot: i32,
        enabled_options: &[i32],
        state: &SkinDrawState,
        direction: i32,
        progress: f32,
    );

    fn select_songlist_all_child_items(
        &self,
        entries: &[DestinationListEntry],
        row: &SelectRowSnapshot,
        row_origin: (i32, i32),
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem>;

    fn select_folder_distribution_graph_render_items(
        &self,
        row: &SelectRowSnapshot,
        graph: &SkinGraphDef,
        destination: &SkinDestinationDef,
        row_origin: (i32, i32),
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem>;

    fn select_songlist_level_items(
        &self,
        entries: &[DestinationListEntry],
        row: &SelectRowSnapshot,
        row_origin: (i32, i32),
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem>;

    fn select_songlist_child_items_by_index(
        &self,
        entries: &[DestinationListEntry],
        index: usize,
        row_origin: (i32, i32),
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem>;

    fn select_songlist_text_items(
        &self,
        row: &SelectRowSnapshot,
        row_origin: (i32, i32),
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem>;

    fn select_bar_item(
        &self,
        row: &SelectRowSnapshot,
        destination: &SkinDestinationDef,
        frame: ResolvedSkinFrame,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;

    fn note_image_render_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;

    fn note_ln_start_render_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        mode: LongNoteMode,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;

    fn note_ln_end_render_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        mode: LongNoteMode,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;

    fn ln_body_image_id<'a>(
        &self,
        note: &'a SkinNoteSetDef,
        index: usize,
        pressing: bool,
    ) -> Option<&'a String>;

    fn hcn_body_image_id<'a>(
        &self,
        note: &'a SkinNoteSetDef,
        index: usize,
        state: LongBodyState,
    ) -> Option<&'a String>;

    fn note_long_body_render_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        mode: LongNoteMode,
        state: LongBodyState,
        draw_state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;

    fn note_mine_render_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;

    fn note_height_for_lane(&self, lane: Lane, key_mode: KeyMode) -> Option<f32>;

    fn note_part_render_item(
        &self,
        image_id: &str,
        rect: Rect,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;

    fn note_group_render_items(
        &self,
        note_y: f32,
        key_mode: KeyMode,
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem>;

    fn note_line_render_items(
        &self,
        destinations: &[SkinDestinationDef],
        note_y: f32,
        key_mode: KeyMode,
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem>;

    fn note_lane_area(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        enabled_options: &[i32],
    ) -> Option<Rect>;

    fn primary_note_lane_height_px(&self) -> Option<i32>;

    fn apply_notes_offset_to_rect(&self, rect: Rect, state: &SkinDrawState) -> Rect;

    fn gauge_render_items(
        &self,
        gauge: f32,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<Vec<SkinRenderItem>>;

    fn destination_uses_skin_gauge_bar_render(&self, destination: &SkinDestinationDef) -> bool;

    fn destination_uses_skin_gauge_overlay_render(&self, destination: &SkinDestinationDef) -> bool;

    fn skin_gauge_for_destination(&self, destination: &SkinDestinationDef)
    -> Option<&SkinGaugeDef>;

    fn resolve_gauge_destination_items(
        &self,
        destination: &SkinDestinationDef,
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<Vec<SkinRenderItem>>;

    fn judge_render_items(
        &self,
        judge: &str,
        combo: u32,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<Vec<SkinRenderItem>>;

    fn judge_render_items_with_offsets(
        &self,
        judge: &str,
        combo: u32,
        elapsed_ms: i32,
        skin_offsets: &SkinOffsetValues,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<Vec<SkinRenderItem>>;

    fn judge_render_items_for_def(
        &self,
        judge: &SkinJudgeDef,
        judge_index: usize,
        combo: u32,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
    ) -> Option<Vec<SkinRenderItem>>;

    fn beatoraja_judge_number_dst_x(dst_w: i32, digit: i32) -> i32;

    fn apply_beatoraja_judge_number_dst_x(frame: &mut ResolvedSkinFrame, digit: i32);

    fn value_number_length(&self, value_id: &str, number: i64, frame: ResolvedSkinFrame) -> i32;

    fn judge_image_render_item(
        &self,
        judge: &str,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;

    fn value_number_render_items(
        &self,
        value_id: &str,
        number: i64,
        base_frame: ResolvedSkinFrame,
        frame: ResolvedSkinFrame,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
        compact_digits: bool,
        align_override: Option<i32>,
        signed_render: SignedNumberRender,
    ) -> Vec<SkinRenderItem>;

    fn value_digit_texture_region(
        value: &SkinValueDef,
        digit: u32,
        elapsed_ms: i32,
        source_size: SkinImageSize,
        cell_width_px: f32,
        cell_height_px: f32,
        divx: i32,
        divy: i32,
    ) -> TextureRegion;

    fn gauge_image_render_item(
        &self,
        image_id: &str,
        rect: Rect,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
        tint: Color,
        blend: BlendMode,
        linear_filter: bool,
    ) -> Option<SkinRenderItem>;

    #[cfg(test)]
    fn text_render_item(
        &self,
        text: &SkinTextDef,
        frame: ResolvedSkinFrame,
        state: &SkinTextState<'_>,
    ) -> Option<SkinRenderItem>;

    fn text_render_item_with_draw_state(
        &self,
        text: &SkinTextDef,
        frame: ResolvedSkinFrame,
        draw_state: Option<&SkinDrawState>,
        state: &SkinTextState<'_>,
    ) -> Option<SkinRenderItem>;

    fn hiterror_visualizer_render_items(
        &self,
        visualizer: &SkinHitErrorVisualizerDef,
        destination: &SkinDestinationDef,
        frame: ResolvedSkinFrame,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem>;

    fn gaugegraph_render_items(
        &self,
        destination_index: usize,
        graph: &SkinGaugeGraphDef,
        destination: &SkinDestinationDef,
        frame: ResolvedSkinFrame,
        state: &SkinDrawState,
        points: &[crate::snapshot::ResultGaugeGraphPoint],
        cache: Option<&mut ResultRenderCache>,
    ) -> Vec<SkinRenderItem>;

    fn timing_visualizer_render_items(
        &self,
        visualizer: &SkinTimingVisualizerDef,
        destination: &SkinDestinationDef,
        frame: ResolvedSkinFrame,
        state: &SkinDrawState,
        timing_points: &[crate::snapshot::ResultTimingPoint],
    ) -> Vec<SkinRenderItem>;

    fn timing_distribution_graph_render_items(
        &self,
        graph: &SkinTimingDistributionGraphDef,
        destination: &SkinDestinationDef,
        frame: ResolvedSkinFrame,
        state: &SkinDrawState,
        timing_points: &[crate::snapshot::ResultTimingPoint],
        timing_distribution: &crate::snapshot::ResultTimingDistribution,
    ) -> Vec<SkinRenderItem>;

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
    ) -> Vec<SkinRenderItem>;

    fn density_judgegraph_render_items(
        &self,
        graph: &SkinJudgeGraphDef,
        destination: &SkinDestinationDef,
        frame: ResolvedSkinFrame,
        density: &[u8],
    ) -> Vec<SkinRenderItem>;

    fn select_note_distribution_graph_render_items(
        &self,
        row: &SelectRowSnapshot,
        graph: &SkinJudgeGraphDef,
        destination: &SkinDestinationDef,
        row_origin: (i32, i32),
        enabled_options: &[i32],
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem>;

    fn select_bpmgraph_row_render_items(
        &self,
        row: &SelectRowSnapshot,
        graph: &SkinBpmGraphDef,
        destination: &SkinDestinationDef,
        row_origin: (i32, i32),
        enabled_options: &[i32],
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem>;

    fn bpmgraph_render_items_with_segments(
        &self,
        graph: &SkinBpmGraphDef,
        destination: &SkinDestinationDef,
        frame: ResolvedSkinFrame,
        state: &SkinDrawState,
        segments: &[crate::chart_graph::BpmGraphSegment],
    ) -> Vec<SkinRenderItem>;

    fn direct_source_image_render_item(
        &self,
        destination: &SkinDestinationDef,
        frame: ResolvedSkinFrame,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;

    fn slider_render_item(
        &self,
        slider: &SkinSliderDef,
        destination: &SkinDestinationDef,
        frame: ResolvedSkinFrame,
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;

    fn hidden_cover_render_item(
        &self,
        cover: &SkinHiddenCoverDef,
        destination: &SkinDestinationDef,
        frame: ResolvedSkinFrame,
        force_lift_cover: bool,
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;

    fn graph_render_item(
        &self,
        graph: &SkinGraphDef,
        frame: ResolvedSkinFrame,
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem>;
}

#[allow(private_interfaces)]
impl SkinDocumentRenderExt for SkinDocument {
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

    fn select_render_items(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        snapshot: &SelectSnapshot,
    ) -> Vec<SkinRenderItem> {
        self.select_render_items_with_dynamic_timers(
            sources,
            snapshot,
            None,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            None,
        )
    }

    fn select_render_items_with_dynamic_timers(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        snapshot: &SelectSnapshot,
        dynamic_timers: Option<&mut DynamicTimerRuntime>,
        settings_dest_index: &crate::select_settings_dest::SelectSettingsDestIndex,
        lua_draw_runtime: Option<Arc<dyn SkinLuaDrawRuntime>>,
    ) -> Vec<SkinRenderItem> {
        let (mut state, selected_row) = self.select_draw_state(snapshot, dynamic_timers);
        let text = SkinTextState {
            player_name: &snapshot.player_name,
            title: select_detail_title(snapshot, selected_row),
            subtitle: select_detail_subtitle(snapshot, selected_row),
            artist: select_detail_artist(snapshot, selected_row),
            genre: select_detail_genre(snapshot, selected_row),
            difficulty_name: if snapshot.in_settings {
                ""
            } else {
                selected_row.map(|row| row.difficulty_name.as_str()).unwrap_or_default()
            },
            play_level: selected_row.map(|row| row.play_level.as_str()).unwrap_or_default(),
            target: if snapshot.in_settings { "" } else { &snapshot.target },
            select_arrange: &snapshot.arrange,
            select_arrange_2p: &snapshot.arrange_2p,
            select_gauge: &snapshot.gauge,
            select_gauge_auto_shift: &snapshot.gauge_auto_shift,
            select_bottom_shiftable_gauge: &snapshot.bottom_shiftable_gauge,
            select_double_option: &snapshot.double_option,
            select_hs_fix: &snapshot.hs_fix,
            select_assist: &snapshot.assist,
            select_mode: &snapshot.select_mode,
            select_sort: &snapshot.select_sort,
            select_ln_mode: &snapshot.select_ln_mode,
            select_bga: &snapshot.bga,
            select_judge_timing_auto_adjust: if snapshot.judge_timing_auto_adjust {
                "ON"
            } else {
                "OFF"
            },
            current_folder: &snapshot.current_folder,
            table_level: selected_row
                .map(|row| {
                    if row.table_text_secondary.is_empty() {
                        row.table_level.as_str()
                    } else {
                        row.table_text_secondary.as_str()
                    }
                })
                .unwrap_or_default(),
            table_text_primary: selected_row
                .map(|row| row.table_text_primary.as_str())
                .unwrap_or_default(),
            table_text_secondary: selected_row
                .map(|row| row.table_text_secondary.as_str())
                .unwrap_or_default(),
            table_text_fallback: selected_row
                .map(|row| row.table_text_fallback.as_str())
                .unwrap_or_default(),
            course_titles: selected_row
                .map(|row| string_array_refs(&row.course_titles))
                .unwrap_or_default(),
            search_word: &snapshot.search_word,
            search_word_alpha: snapshot.search_word_alpha,
            search_caret_byte_index: snapshot.search_caret_byte_index,
            rival: snapshot.rival.as_ref().map(|rival| rival.display_name.as_str()).unwrap_or(""),
            ir_ranking: &snapshot.ir,
            ..SkinTextState::default()
        };

        let images = self.image_map();
        let values: HashMap<&str, &SkinValueDef> =
            self.value.iter().map(|value| (value.id.as_str(), value)).collect();
        let enabled_options = self.enabled_options();
        if let Some(runtime) = lua_draw_runtime {
            state.lua_runtime = Some(SkinLuaRuntimeContext {
                runtime,
                enabled_options: Arc::from(enabled_options.clone()),
                text_values: Arc::new(lua_main_state_text_values(&state, &text)),
            });
        }
        let destinations = self.all_destinations(&enabled_options);
        let has_nearest_f_diff_rank_destination =
            nearest_f_diff_rank_destination_available(&destinations);
        let mut items = Vec::new();
        for (destination_index, destination) in destinations.into_iter().enumerate() {
            if destination.id == self.songlist.as_ref().map(|list| list.id.as_str()).unwrap_or("") {
                items.extend(self.select_songlist_items(
                    sources,
                    snapshot,
                    &images,
                    &enabled_options,
                    &state,
                ));
                continue;
            }
            if !crate::select_settings_dest::test_select_destination_visible(
                settings_dest_index,
                destination,
                &enabled_options,
                &state,
                snapshot,
                selected_row,
                eval_skin_draw_condition,
                |ops, enabled_options, state| {
                    if ops.len() == destination.op.len() && ops.iter().eq(destination.op.iter()) {
                        destination_ops_match(
                            destination,
                            enabled_options,
                            state,
                            has_nearest_f_diff_rank_destination,
                        )
                    } else {
                        test_skin_ops(ops, enabled_options, state)
                    }
                },
            ) {
                continue;
            }
            if let (Some(row), Some(judge_graph)) = (
                selected_row.filter(|row| select_row_shows_score_decorations(row)),
                self.judgegraph.iter().find(|graph| graph.id == destination.id),
            ) {
                items.extend(self.select_note_distribution_graph_render_items(
                    row,
                    judge_graph,
                    destination,
                    (0, 0),
                    &enabled_options,
                    &state,
                ));
                continue;
            }
            if let (Some(row), Some(bpm_graph)) = (
                selected_row.filter(|row| select_row_shows_score_decorations(row)),
                self.bpmgraph.iter().find(|graph| graph.id == destination.id),
            ) {
                let Some(elapsed) = skin_timer_elapsed_ms(destination.timer, &state) else {
                    continue;
                };
                let Some(mut frame) =
                    resolve_destination_frame(destination, elapsed, &enabled_options, &state)
                else {
                    continue;
                };
                apply_skin_offset_to_frame(destination, &mut frame, &state, false);
                if !destination_mouse_rect_contains(destination, frame, &state) {
                    continue;
                }
                items.extend(self.bpmgraph_render_items_with_segments(
                    bpm_graph,
                    destination,
                    frame,
                    &state,
                    &row.chart_bpm_graph_segments,
                ));
                continue;
            }
            if let Some(resolved) = self.resolve_destination_items(
                destination_index,
                destination,
                DestinationResolveContext {
                    images: &images,
                    values: &values,
                    enabled_options: &enabled_options,
                    state: &state,
                    text_state: &text,
                    sources,
                    runtime_graphs: SkinRuntimeGraphs::from_document(self),
                    has_nearest_f_diff_rank_destination,
                    cache: None,
                },
            ) {
                items.extend(resolved);
            }
        }
        items
    }

    fn select_draw_state<'a>(
        &self,
        snapshot: &'a SelectSnapshot,
        dynamic_timers: Option<&mut DynamicTimerRuntime>,
    ) -> (SkinDrawState, Option<&'a SelectRowSnapshot>) {
        let selected_row = snapshot.rows.iter().find(|row| row.index == snapshot.selected_index);
        let mouse_position = snapshot.mouse_position.map(|(x, y)| {
            (x.clamp(0.0, 1.0) * self.w as f32, (1.0 - y.clamp(0.0, 1.0)) * self.h as f32)
        });
        let duration_green_ms = snapshot.note_display_duration_ms;
        let elapsed_ms = (snapshot.time.0 / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        let mut state = SkinDrawState {
            elapsed_ms,
            start_input_ms: skin_start_input_elapsed_ms(elapsed_ms, self.input),
            current_fps: snapshot.current_fps,
            operating_time_ms: snapshot.operating_time_ms,
            logical_input_held: snapshot.skin_input.held,
            skin_offsets: snapshot.skin_offsets,
            select_bar_elapsed_ms: (snapshot.selection_time.0 / 1_000)
                .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            select_option_panel_elapsed_ms: (snapshot.option_panel_time.0 / 1_000)
                .clamp(i32::MIN as i64, i32::MAX as i64)
                as i32,
            select_option_panel_off_elapsed_ms: snapshot.option_panel_off_times.map(|elapsed| {
                elapsed.map(|elapsed| {
                    (elapsed.0 / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32
                })
            }),
            select_option_panel: snapshot.option_panel,
            select_arrange_index: select_arrange_index(&snapshot.arrange),
            select_arrange_2p_index: select_arrange_index(&snapshot.arrange_2p),
            select_extended_arrange_index: extended_arrange_index(&snapshot.arrange),
            select_extended_arrange_2p_index: extended_arrange_index(&snapshot.arrange_2p),
            select_double_option_index: select_double_option_index(&snapshot.double_option),
            select_hs_fix_index: select_hs_fix_index(&snapshot.hs_fix),
            select_gauge_index: select_gauge_index(&snapshot.gauge),
            select_gauge_auto_shift_index: select_gauge_auto_shift_index(
                &snapshot.gauge_auto_shift,
            ),
            select_bottom_shiftable_gauge_index: select_bottom_shiftable_gauge_index(
                &snapshot.bottom_shiftable_gauge,
            ),
            select_target_index: play_target_image_index(&snapshot.target),
            select_bga_index: select_bga_index(&snapshot.bga),
            judge_timing_offset_ms: snapshot.judge_timing_offset_ms,
            judge_timing_auto_adjust: snapshot.judge_timing_auto_adjust,
            lanecover_enabled: snapshot.lanecover_enabled,
            lift_enabled: snapshot.lift_enabled,
            hidden_enabled: snapshot.hidden_enabled,
            hispeed_auto_adjust: snapshot.hispeed_auto_adjust,
            player_stats: snapshot.player_stats.clone(),
            select_assist_index: select_assist_index(&snapshot.assist),
            select_mode_index: select_mode_index(&snapshot.select_mode),
            select_sort_index: select_sort_index(&snapshot.select_sort),
            select_ln_mode_index: select_ln_mode_index(&snapshot.select_ln_mode),
            select_judge_algorithm_index: select_judge_algorithm_index(&snapshot.judge_algorithm),
            hispeed: snapshot.hispeed,
            total_duration_ms: duration_green_ms
                .map(green_duration_to_duration)
                .unwrap_or(0)
                .min(i32::MAX as i64) as i32,
            duration_green_ms,
            result_grade_diff_display: snapshot.grade_diff_display,
            select_scroll_progress: select_scroll_progress(snapshot),
            select_master_volume: snapshot.master_volume,
            select_key_volume: snapshot.key_volume,
            select_bgm_volume: snapshot.bgm_volume,
            select_has_banner: snapshot.banner_image,
            select_has_document: selected_row.is_some_and(|row| row.has_document),
            has_stagefile: snapshot.stage_background,
            has_backbmp: snapshot.backbmp_image,
            select_folder_song_count: selected_row.and_then(select_row_folder_song_count),
            select_screen: true,
            select_play_level: selected_row.map(select_row_level_number).unwrap_or(0),
            play_level: selected_row.map(select_row_level_number).unwrap_or(0),
            table_song: selected_row.is_some_and(|row| !row.table_text_primary.is_empty()),
            min_bpm: selected_row.map(|row| row.min_bpm).unwrap_or(0.0),
            max_bpm: selected_row.map(|row| row.max_bpm).unwrap_or(0.0),
            has_bpm_stop: selected_row
                .map(|row| row.chart_bpm_graph_segments.iter().any(|s| s.is_stop))
                .unwrap_or(false),
            main_bpm: selected_row.map(|row| row.chart_main_bpm).unwrap_or(0.0),
            difficulty: selected_row.map(select_row_difficulty_code).unwrap_or(0),
            judge_rank: selected_row.and_then(|row| row.judge_rank),
            select_ex_score: selected_row.and_then(|row| row.ex_score),
            select_replay_slots: selected_row.map(|row| row.replay_slots).unwrap_or([false; 4]),
            select_replay_index: selected_row.and_then(select_row_replay_index),
            select_clear_index: selected_row.map(select_row_clear_index).unwrap_or(0) as i64,
            select_favorite_song: selected_row.is_some_and(|row| row.favorite_song),
            select_favorite_chart: selected_row.is_some_and(|row| row.favorite_chart),
            select_replay_slot_rule_indices: snapshot.replay_slot_rule_indices,
            select_folder_lamp_counts: selected_row
                .map(|row| row.folder_lamp_counts)
                .unwrap_or([0; 11]),
            select_row_kind: selected_row.map(|row| row.kind).unwrap_or(SelectRowKind::Song),
            select_course_constraints: selected_row
                .map(|row| row.course_constraints)
                .unwrap_or_default(),
            select_is_folder: selected_row.is_some_and(|row| row.is_folder),
            select_in_library: selected_row.is_none_or(|row| row.in_library),
            select_total_notes: selected_row.map(|row| row.total_notes).unwrap_or(0),
            select_chart_normal_notes: selected_row.map(|row| row.chart_normal_notes).unwrap_or(0),
            select_chart_long_notes: selected_row.map(|row| row.chart_long_notes).unwrap_or(0),
            select_chart_scratch_notes: selected_row
                .map(|row| row.chart_scratch_notes)
                .unwrap_or(0),
            select_chart_long_scratch_notes: selected_row
                .map(|row| row.chart_long_scratch_notes)
                .unwrap_or(0),
            select_chart_mine_notes: selected_row.map(|row| row.chart_mine_notes).unwrap_or(0),
            select_chart_density: selected_row.map(|row| row.chart_density).unwrap_or(0.0),
            select_chart_peak_density: selected_row
                .map(|row| row.chart_peak_density)
                .unwrap_or(0.0),
            select_chart_end_density: selected_row.map(|row| row.chart_end_density).unwrap_or(0.0),
            select_chart_total_gauge: selected_row.map(|row| row.chart_total_gauge).unwrap_or(0.0),
            select_chart_main_bpm: selected_row.map(|row| row.chart_main_bpm).unwrap_or(0.0),
            select_bpm: selected_row.map(|row| row.initial_bpm).unwrap_or(0.0),
            select_min_bpm: selected_row.map(|row| row.min_bpm).unwrap_or(0.0),
            select_max_bpm: selected_row.map(|row| row.max_bpm).unwrap_or(0.0),
            select_length_ms: selected_row.map(|row| row.length_ms).unwrap_or(0),
            select_play_count: selected_row.map(|row| row.play_count).unwrap_or(0),
            select_clear_count: selected_row.map(|row| row.clear_count).unwrap_or(0),
            select_bp: selected_row.and_then(|row| row.bp),
            select_cb: selected_row.and_then(|row| row.cb),
            judge_counts: selected_row.map(|row| row.judge_counts).unwrap_or_default(),
            fast_slow_counts: selected_row.and_then(|row| row.fast_slow_counts),
            max_combo: selected_row.and_then(|row| row.max_combo).unwrap_or(0),
            total_notes: selected_row.map(|row| row.total_notes).unwrap_or(0),
            past_notes: selected_row.map(|row| row.total_notes).unwrap_or(0),
            gauge: selected_row.and_then(|row| row.gauge_value).unwrap_or(0.0),
            gauge_auto_shift: snapshot.gauge_auto_shift != "OFF",
            ex_score: selected_row.and_then(|row| row.ex_score).unwrap_or(0),
            in_settings: snapshot.in_settings,
            settings_editing: snapshot.settings_editing,
            select_chart_key_mode: selected_row.and_then(|row| row.chart_key_mode),
            random_lane_refs: selected_row
                .and_then(|row| row.chart_key_mode)
                .map_or([0; SKIN_RANDOM_LANE_REF_COUNT], |key_mode| {
                    random_lane_refs(&snapshot.lane_shuffle_pattern, key_mode)
                }),
            mouse_x: mouse_position.map(|position| position.0),
            mouse_y: mouse_position.map(|position| position.1),
            ir_ranking: snapshot.ir.clone(),
            rival_ex_score: snapshot.rival.as_ref().map(|rival| i64::from(rival.ex_score)),
            rival_max_combo: snapshot.rival.as_ref().map(|rival| i64::from(rival.max_combo)),
            rival_bp: snapshot.rival.as_ref().map(|rival| i64::from(rival.bp)),
            rival_judge_counts: snapshot.rival.as_ref().and_then(|rival| {
                rival.judge_counts.map(|counts| {
                    [counts.pgreat, counts.great, counts.good, counts.bad, counts.poor]
                })
            }),
            ..SkinDrawState::default()
        };
        if let Some(runtime) = dynamic_timers {
            let now_ms = state.elapsed_ms;
            runtime.advance(self, &mut state, now_ms);
        }
        (state, selected_row)
    }

    fn select_click_hit(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        snapshot: &SelectSnapshot,
        settings_dest_index: &crate::select_settings_dest::SelectSettingsDestIndex,
        x: f32,
        y: f32,
    ) -> Option<SkinClickHit> {
        self.select_click_hits(sources, snapshot, settings_dest_index)
            .into_iter()
            .rev()
            .find(|hit| rect_contains(hit.rect, x, y))
    }

    fn result_click_hit(&self, state: &SkinDrawState, x: f32, y: f32) -> Option<SkinClickHit> {
        let enabled_options = self.enabled_options();
        let images = self.image_map();
        let destinations = self.all_destinations(&enabled_options);
        let has_nearest_f_diff_rank_destination =
            nearest_f_diff_rank_destination_available(&destinations);
        destinations
            .into_iter()
            .filter(|destination| {
                destination_ops_match(
                    destination,
                    &enabled_options,
                    state,
                    has_nearest_f_diff_rank_destination,
                ) && eval_skin_draw_condition(&destination.draw, state)
            })
            .filter_map(|destination| {
                Some(SkinClickHit {
                    target: self.click_target_for_destination(destination, &images)?,
                    rect: self.destination_click_rect(destination, &enabled_options, state)?,
                })
            })
            .rev()
            .find(|hit| rect_contains(hit.rect, x, y))
    }

    fn result_slider_hit(&self, state: &SkinDrawState, x: f32, y: f32) -> Option<SkinSliderHit> {
        let enabled_options = self.enabled_options();
        let destinations = self.all_destinations(&enabled_options);
        let has_nearest_f_diff_rank_destination =
            nearest_f_diff_rank_destination_available(&destinations);
        destinations
            .into_iter()
            .filter(|destination| {
                destination_ops_match(
                    destination,
                    &enabled_options,
                    state,
                    has_nearest_f_diff_rank_destination,
                ) && eval_skin_draw_condition(&destination.draw, state)
            })
            .filter_map(|destination| {
                let slider = self.slider.iter().find(|slider| slider.id == destination.id)?;
                (slider.slider_type == 8).then_some(())?;
                self.destination_slider_hit(slider, destination, &enabled_options, state, x, y)
            })
            .next_back()
    }

    fn select_slider_hit(
        &self,
        snapshot: &SelectSnapshot,
        settings_dest_index: &crate::select_settings_dest::SelectSettingsDestIndex,
        x: f32,
        y: f32,
    ) -> Option<SkinSliderHit> {
        let (state, selected_row) = self.select_draw_state(snapshot, None);
        let enabled_options = self.enabled_options();
        self.all_destinations(&enabled_options)
            .into_iter()
            .filter_map(|destination| {
                if !crate::select_settings_dest::test_select_destination_visible(
                    settings_dest_index,
                    destination,
                    &enabled_options,
                    &state,
                    snapshot,
                    selected_row,
                    eval_skin_draw_condition,
                    test_skin_ops,
                ) {
                    return None;
                }
                let slider = self.slider.iter().find(|slider| slider.id == destination.id)?;
                self.destination_slider_hit(slider, destination, &enabled_options, &state, x, y)
            })
            .next_back()
    }

    fn select_click_hits(
        &self,
        _sources: &HashMap<String, SkinDocumentTexture>,
        snapshot: &SelectSnapshot,
        settings_dest_index: &crate::select_settings_dest::SelectSettingsDestIndex,
    ) -> Vec<SkinClickHit> {
        let (state, selected_row) = self.select_draw_state(snapshot, None);
        let enabled_options = self.enabled_options();
        let images = self.image_map();
        let mut hits = Vec::new();
        for destination in self.all_destinations(&enabled_options) {
            if destination.id == self.songlist.as_ref().map(|list| list.id.as_str()).unwrap_or("") {
                hits.extend(self.select_songlist_click_hits(snapshot, &enabled_options, &state));
                continue;
            }
            if !crate::select_settings_dest::test_select_destination_visible(
                settings_dest_index,
                destination,
                &enabled_options,
                &state,
                snapshot,
                selected_row,
                eval_skin_draw_condition,
                test_skin_ops,
            ) {
                continue;
            }
            let Some(target) = self.click_target_for_destination(destination, &images) else {
                continue;
            };
            let Some(rect) = self.destination_click_rect(destination, &enabled_options, &state)
            else {
                continue;
            };
            hits.push(SkinClickHit { target, rect });
        }
        hits
    }

    fn select_songlist_click_hits(
        &self,
        snapshot: &SelectSnapshot,
        enabled_options: &[i32],
        state: &SkinDrawState,
    ) -> Vec<SkinClickHit> {
        let Some(songlist) = &self.songlist else {
            return Vec::new();
        };
        let selected_row_position =
            select_snapshot_selected_row_position(&snapshot.rows, snapshot.selected_index) as i32;
        let mut hits = Vec::new();
        let mut row_state = state.clone();
        for (row_position, row) in snapshot.rows.iter().enumerate() {
            let offset = row_position as i32 - selected_row_position;
            let slot = songlist.center + offset;
            if !songlist.clickable.contains(&slot) || slot < 0 {
                continue;
            }
            let selected = row_position as i32 == selected_row_position;
            let row_destinations = if selected { &songlist.liston } else { &songlist.listoff };
            let Some(row_destination) =
                destination_entry_at(row_destinations, slot as usize, enabled_options)
            else {
                continue;
            };
            Self::apply_select_songlist_click_row_state(&mut row_state, row);
            let elapsed = skin_timer_elapsed_ms(row_destination.timer, state).unwrap_or(0);
            let Some(mut frame) =
                resolve_destination_frame(row_destination, elapsed, enabled_options, &row_state)
            else {
                continue;
            };
            self.apply_select_songlist_scroll_to_frame(
                &mut frame,
                songlist,
                slot,
                enabled_options,
                &row_state,
                snapshot.bar_scroll_direction,
                snapshot.bar_scroll_progress,
            );
            apply_skin_offset_to_frame(row_destination, &mut frame, &row_state, false);
            if !destination_mouse_rect_contains(row_destination, frame, &row_state) {
                continue;
            }
            let rect = normalize_skin_frame_rect(frame, self.w, self.h);
            if rect.width <= 0.0 || rect.height <= 0.0 {
                continue;
            }
            hits.push(SkinClickHit {
                target: SkinClickTarget::SelectRow { row_index: row.index },
                rect,
            });
        }
        hits
    }

    fn apply_select_songlist_render_row_state(state: &mut SkinDrawState, row: &SelectRowSnapshot) {
        state.select_play_level = select_row_level_number(row);
        state.play_level = select_row_level_number(row);
        state.table_song = !row.table_text_primary.is_empty();
        state.difficulty = select_row_difficulty_code(row);
        state.judge_rank = row.judge_rank;
        state.select_ex_score = row.ex_score;
        state.select_replay_slots = row.replay_slots;
        state.select_replay_index = select_row_replay_index(row);
        state.select_clear_index = select_row_clear_index(row) as i64;
        state.select_favorite_song = row.favorite_song;
        state.select_favorite_chart = row.favorite_chart;
        state.select_folder_lamp_counts = row.folder_lamp_counts;
        state.select_row_kind = row.kind;
        state.select_course_constraints = row.course_constraints;
        state.select_is_folder = row.is_folder;
        state.select_in_library = row.in_library;
        state.select_total_notes = row.total_notes;
        state.select_chart_normal_notes = row.chart_normal_notes;
        state.select_chart_long_notes = row.chart_long_notes;
        state.select_chart_scratch_notes = row.chart_scratch_notes;
        state.select_chart_long_scratch_notes = row.chart_long_scratch_notes;
        state.select_chart_mine_notes = row.chart_mine_notes;
        state.select_chart_density = row.chart_density;
        state.select_chart_peak_density = row.chart_peak_density;
        state.select_chart_end_density = row.chart_end_density;
        state.select_chart_total_gauge = row.chart_total_gauge;
        state.select_chart_main_bpm = row.chart_main_bpm;
        state.select_bpm = row.initial_bpm;
        state.select_min_bpm = row.min_bpm;
        state.select_max_bpm = row.max_bpm;
        state.min_bpm = row.min_bpm;
        state.max_bpm = row.max_bpm;
        state.main_bpm = row.chart_main_bpm;
        state.select_length_ms = row.length_ms;
        state.select_play_count = row.play_count;
        state.select_clear_count = row.clear_count;
        state.select_bp = row.bp;
        state.select_cb = row.cb;
        state.max_combo = row.max_combo.unwrap_or(0);
        state.total_notes = row.total_notes;
        state.gauge = row.gauge_value.unwrap_or(0.0);
        state.ex_score = row.ex_score.unwrap_or(0);
        state.select_chart_key_mode = row.chart_key_mode;
    }

    fn apply_select_songlist_click_row_state(state: &mut SkinDrawState, row: &SelectRowSnapshot) {
        Self::apply_select_songlist_render_row_state(state, row);
    }

    fn click_target_for_destination(
        &self,
        destination: &SkinDestinationDef,
        images: &HashMap<&str, &SkinImageDef>,
    ) -> Option<SkinClickTarget> {
        if destination.clickable == Some(false) {
            return None;
        }
        if let Some(event_id) = destination.act {
            return Some(SkinClickTarget::Event { event_id, click: destination.click });
        }
        if let Some(image) = images.get(destination.id.as_str())
            && destination.clickable.or(image.clickable).unwrap_or(image.act.is_some())
            && let Some(event_id) = image.act
        {
            return Some(SkinClickTarget::Event { event_id, click: image.click });
        }
        let imageset = self.imageset.iter().find(|set| set.id == destination.id)?;
        destination
            .clickable
            .or(imageset.clickable)
            .unwrap_or(imageset.act.is_some())
            .then_some(imageset.act)
            .flatten()
            .map(|event_id| SkinClickTarget::Event { event_id, click: imageset.click })
    }

    fn destination_click_rect(
        &self,
        destination: &SkinDestinationDef,
        enabled_options: &[i32],
        state: &SkinDrawState,
    ) -> Option<Rect> {
        let elapsed = skin_timer_elapsed_ms(destination.timer, state)?;
        let mut frame = resolve_destination_frame(destination, elapsed, enabled_options, state)?;
        apply_skin_offset_to_frame(destination, &mut frame, state, false);
        if !destination_mouse_rect_contains(destination, frame, state) {
            return None;
        }
        let rect = normalize_skin_frame_rect(frame, self.w, self.h);
        if rect.width <= 0.0 || rect.height <= 0.0 { None } else { Some(rect) }
    }

    fn destination_slider_hit(
        &self,
        slider: &SkinSliderDef,
        destination: &SkinDestinationDef,
        enabled_options: &[i32],
        state: &SkinDrawState,
        x: f32,
        y: f32,
    ) -> Option<SkinSliderHit> {
        if !slider.changeable || !matches!(slider.slider_type, 1 | 8 | 17..=19) {
            return None;
        }
        let elapsed = skin_timer_elapsed_ms(destination.timer, state)?;
        let mut frame = resolve_destination_frame(destination, elapsed, enabled_options, state)?;
        apply_skin_offset_to_frame(destination, &mut frame, state, false);
        if !destination_mouse_rect_contains(destination, frame, state) {
            return None;
        }
        let mouse_x = x.clamp(0.0, 1.0) * self.w as f32;
        let mouse_y = (1.0 - y.clamp(0.0, 1.0)) * self.h as f32;
        let value = if slider.slider_type == 1 {
            scroll_slider_value_at(slider, frame, mouse_x, mouse_y)?
        } else {
            slider_value_at(slider, frame, mouse_x, mouse_y)?
        };
        Some(SkinSliderHit { slider_type: slider.slider_type, value })
    }

    fn select_songlist_items(
        &self,
        sources: &HashMap<String, SkinDocumentTexture>,
        snapshot: &SelectSnapshot,
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem> {
        let Some(songlist) = &self.songlist else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let selected_row_position =
            select_snapshot_selected_row_position(&snapshot.rows, snapshot.selected_index) as i32;
        let mut row_state = state.clone();
        for (row_position, row) in snapshot.rows.iter().enumerate() {
            let offset = row_position as i32 - selected_row_position;
            let slot = songlist.center + offset;
            if slot < 0 {
                continue;
            }
            let selected = row_position as i32 == selected_row_position;
            let row_destinations = if selected { &songlist.liston } else { &songlist.listoff };
            let Some(row_destination) =
                destination_entry_at(row_destinations, slot as usize, enabled_options)
            else {
                continue;
            };
            Self::apply_select_songlist_render_row_state(&mut row_state, row);
            let elapsed = skin_timer_elapsed_ms(row_destination.timer, state).unwrap_or(0);
            let Some(mut row_frame) =
                resolve_destination_frame(row_destination, elapsed, enabled_options, &row_state)
            else {
                continue;
            };
            self.apply_select_songlist_scroll_to_frame(
                &mut row_frame,
                songlist,
                slot,
                enabled_options,
                &row_state,
                snapshot.bar_scroll_direction,
                snapshot.bar_scroll_progress,
            );
            let row_origin = (row_frame.x, row_frame.y);
            apply_skin_offset_to_frame(row_destination, &mut row_frame, state, false);
            if let Some(item) = self.select_bar_item(row, row_destination, row_frame, sources) {
                items.push(item);
            }
            if select_row_shows_lamp(row) {
                let clear_index = select_row_clear_index(row);
                items.extend(self.select_songlist_child_items_by_index(
                    &songlist.lamp,
                    clear_index,
                    row_origin,
                    images,
                    enabled_options,
                    &row_state,
                    sources,
                ));
            }
            if select_row_shows_score_decorations(row) {
                if select_row_shows_level(row) {
                    items.extend(self.select_songlist_level_items(
                        &songlist.level,
                        row,
                        row_origin,
                        images,
                        enabled_options,
                        &row_state,
                        sources,
                    ));
                }
                for label_index in select_row_label_indices(row) {
                    items.extend(self.select_songlist_child_items_by_index(
                        &songlist.label,
                        label_index,
                        row_origin,
                        images,
                        enabled_options,
                        &row_state,
                        sources,
                    ));
                }
                if select_row_shows_course_trophy(row)
                    && let Some(trophy_index) = select_row_trophy_index(row)
                {
                    items.extend(self.select_songlist_child_items_by_index(
                        &songlist.trophy,
                        trophy_index,
                        row_origin,
                        images,
                        enabled_options,
                        &row_state,
                        sources,
                    ));
                }
                items.extend(self.select_songlist_all_child_items(
                    &songlist.judgegraph,
                    row,
                    row_origin,
                    images,
                    enabled_options,
                    &row_state,
                    sources,
                ));
                items.extend(self.select_songlist_all_child_items(
                    &songlist.bpmgraph,
                    row,
                    row_origin,
                    images,
                    enabled_options,
                    &row_state,
                    sources,
                ));
            }
            if select_row_shows_folder_distribution(row) {
                items.extend(self.select_songlist_all_child_items(
                    &songlist.graph,
                    row,
                    row_origin,
                    images,
                    enabled_options,
                    &row_state,
                    sources,
                ));
            }
            items.extend(self.select_songlist_text_items(
                row,
                row_origin,
                images,
                enabled_options,
                &row_state,
                sources,
            ));
        }
        items
    }

    fn apply_select_songlist_scroll_to_frame(
        &self,
        frame: &mut ResolvedSkinFrame,
        songlist: &SkinSongListDef,
        slot: i32,
        enabled_options: &[i32],
        state: &SkinDrawState,
        direction: i32,
        progress: f32,
    ) {
        let direction = direction.signum();
        let progress = progress.clamp(0.0, 1.0);
        if direction == 0 || progress <= 0.0 {
            return;
        }
        let next_slot = slot + direction;
        if next_slot < 0 {
            return;
        }
        let next_selected = next_slot == songlist.center;
        let next_destinations = if next_selected { &songlist.liston } else { &songlist.listoff };
        let Some(next_destination) =
            destination_entry_at(next_destinations, next_slot as usize, enabled_options)
        else {
            return;
        };
        let elapsed = skin_timer_elapsed_ms(next_destination.timer, state).unwrap_or(0);
        let Some(next_frame) =
            resolve_destination_frame(next_destination, elapsed, enabled_options, state)
        else {
            return;
        };
        frame.x += ((next_frame.x - frame.x) as f32 * progress).round() as i32;
        frame.y += ((next_frame.y - frame.y) as f32 * progress).round() as i32;
    }

    fn select_songlist_all_child_items(
        &self,
        entries: &[DestinationListEntry],
        row: &SelectRowSnapshot,
        row_origin: (i32, i32),
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem> {
        let mut items = Vec::new();
        for destination in destination_entries(entries, enabled_options) {
            if let Some(judge_graph) =
                self.judgegraph.iter().find(|graph| graph.id == destination.id)
            {
                items.extend(self.select_note_distribution_graph_render_items(
                    row,
                    judge_graph,
                    destination,
                    row_origin,
                    enabled_options,
                    state,
                ));
                continue;
            }
            if let Some(bpm_graph) = self.bpmgraph.iter().find(|graph| graph.id == destination.id) {
                items.extend(self.select_bpmgraph_row_render_items(
                    row,
                    bpm_graph,
                    destination,
                    row_origin,
                    enabled_options,
                    state,
                ));
                continue;
            }
            if select_row_shows_folder_distribution(row)
                && let Some(graph) = self.graph.iter().find(|graph| graph.id == destination.id)
            {
                items.extend(self.select_folder_distribution_graph_render_items(
                    row,
                    graph,
                    destination,
                    row_origin,
                    enabled_options,
                    state,
                    sources,
                ));
                continue;
            }
            if let Some(mut resolved) = self.resolve_offset_destination_items(
                destination,
                row_origin,
                images,
                enabled_options,
                state,
                &SkinTextState::default(),
                sources,
            ) {
                items.append(&mut resolved);
            }
        }
        items
    }

    fn select_folder_distribution_graph_render_items(
        &self,
        row: &SelectRowSnapshot,
        graph: &SkinGraphDef,
        destination: &SkinDestinationDef,
        row_origin: (i32, i32),
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem> {
        let Some(source) = sources.get(&graph.src) else {
            return Vec::new();
        };
        if !test_skin_ops(&destination.op, enabled_options, state)
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

        let total: u32 = row.folder_lamp_counts.iter().sum();
        if total == 0 {
            return Vec::new();
        }

        let dst = normalize_skin_frame_rect(frame, self.w, self.h);
        let source_w = source.source_size.width.max(1.0);
        let source_h = source.source_size.height.max(1.0);
        let cell_w = skin_grid_cell_size(graph.w, graph.divx.max(11));
        let cell_h = skin_grid_cell_size(graph.h, graph.divy);
        if cell_w <= 0 || cell_h <= 0 {
            return Vec::new();
        }
        let animation_rows = graph.divy.max(1);
        let animation_row = if graph.cycle > 0 && animation_rows > 1 {
            (elapsed.rem_euclid(graph.cycle) * animation_rows / graph.cycle).min(animation_rows - 1)
        } else {
            0
        };

        let mut items = Vec::new();
        let mut filled = 0.0;
        for lamp_index in (0..row.folder_lamp_counts.len()).rev() {
            let count = row.folder_lamp_counts[lamp_index];
            if count == 0 {
                continue;
            }
            let width = dst.width * (count as f32 / total as f32);
            if width <= 0.0 {
                continue;
            }
            let rect = Rect { x: dst.x + filled, width, ..dst };
            let source_x = graph.x + cell_w * lamp_index as i32;
            let source_y = graph.y + cell_h * animation_row;
            let uv = TextureRegion {
                x: source_x as f32 / source_w,
                y: source_y as f32 / source_h,
                width: cell_w as f32 / source_w,
                height: cell_h as f32 / source_h,
            };
            items.push(SkinRenderItem::Image {
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
            });
            filled += width;
        }
        items
    }

    fn select_songlist_level_items(
        &self,
        entries: &[DestinationListEntry],
        row: &SelectRowSnapshot,
        row_origin: (i32, i32),
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem> {
        let level_index = select_row_difficulty_code(row).clamp(0, i64::MAX) as usize;
        self.select_songlist_child_items_by_index(
            entries,
            level_index,
            row_origin,
            images,
            enabled_options,
            state,
            sources,
        )
    }

    fn select_songlist_child_items_by_index(
        &self,
        entries: &[DestinationListEntry],
        index: usize,
        row_origin: (i32, i32),
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem> {
        let mut items = Vec::new();
        let Some(destination) = destination_entry_at(entries, index, enabled_options) else {
            return items;
        };
        if let Some(mut resolved) = self.resolve_offset_destination_items(
            destination,
            row_origin,
            images,
            enabled_options,
            state,
            &SkinTextState::default(),
            sources,
        ) {
            items.append(&mut resolved);
        }
        items
    }

    fn select_songlist_text_items(
        &self,
        row: &SelectRowSnapshot,
        row_origin: (i32, i32),
        images: &HashMap<&str, &SkinImageDef>,
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem> {
        let Some(songlist) = &self.songlist else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let text_state = SkinTextState {
            bar_text: &row.title,
            table_level: if row.table_text_secondary.is_empty() {
                &row.table_level
            } else {
                &row.table_text_secondary
            },
            table_text_primary: &row.table_text_primary,
            table_text_secondary: &row.table_text_secondary,
            table_text_fallback: &row.table_text_fallback,
            ..SkinTextState::default()
        };
        let destinations = destination_entries(&songlist.text, enabled_options);
        let Some(destination) = select_row_slot_with_fallbacks(
            &destinations,
            select_row_bar_text_index(row),
            select_row_bar_text_fallback_indices(row),
        )
        .copied() else {
            return items;
        };
        {
            if let Some(mut resolved) = self.resolve_offset_destination_items(
                destination,
                row_origin,
                images,
                enabled_options,
                state,
                &text_state,
                sources,
            ) {
                items.append(&mut resolved);
            }
        }
        items
    }

    fn select_bar_item(
        &self,
        row: &SelectRowSnapshot,
        destination: &SkinDestinationDef,
        frame: ResolvedSkinFrame,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem> {
        let imageset = self.imageset.iter().find(|set| set.id == destination.id)?;
        let image_index = select_row_bar_image_index(row);
        let image_id = select_row_slot_with_fallbacks(
            &imageset.images,
            image_index,
            select_row_bar_image_fallback_indices(row),
        )?;
        let image = self.image.iter().find(|image| image.id == *image_id)?;
        let source = resolve_document_source(sources, &image.src)?;
        let elapsed =
            skin_timer_elapsed_ms(destination.timer, &SkinDrawState::default()).unwrap_or(0);
        let (rect, uv) = stretch_skin_image_geometry(
            destination.stretch,
            normalize_skin_frame_rect(frame, self.w, self.h),
            skin_image_texture_region(image, source.source_size, elapsed),
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

    fn note_image_render_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem> {
        let note = self.note.as_ref()?;
        let image_id = note.note.get(beatoraja_note_index(lane, key_mode))?;
        self.note_part_render_item(image_id, rect, 0, sources)
    }

    /// LN START（ヘッドキャップ）画像を描画する。
    /// HCN モードでは `hcnstart`（beatoraja: `longImage[5]`）を優先し、
    /// `lnstart` → `note` の順にフォールバックする。
    fn note_ln_start_render_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        mode: LongNoteMode,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem> {
        let note = self.note.as_ref()?;
        let index = beatoraja_note_index(lane, key_mode);
        let hcn = (mode == LongNoteMode::Hcn).then(|| note.hcnstart.get(index)).flatten();
        let image_id = hcn.or_else(|| note.lnstart.get(index)).or_else(|| note.note.get(index))?;
        self.note_part_render_item(image_id, rect, 0, sources)
    }

    /// LN END（テールキャップ）画像を描画する。
    /// HCN モードでは `hcnend`（beatoraja: `longImage[4]`）を優先し、
    /// `lnend` → `note` の順にフォールバックする。
    fn note_ln_end_render_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        mode: LongNoteMode,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem> {
        let note = self.note.as_ref()?;
        let index = beatoraja_note_index(lane, key_mode);
        let hcn = (mode == LongNoteMode::Hcn).then(|| note.hcnend.get(index)).flatten();
        let image_id = hcn.or_else(|| note.lnend.get(index)).or_else(|| note.note.get(index))?;
        self.note_part_render_item(image_id, rect, 0, sources)
    }

    /// LN/CN 用の胴体画像 id を選択する。
    /// 新形式 (`lnbodyActive` 定義あり): 押下中=`lnbodyActive`, 非押下=`lnbody`。
    /// 旧形式: 押下中=`lnbody` (longImage\[2\]), 非押下=`lnactive` (longImage\[3\])。
    fn ln_body_image_id<'a>(
        &self,
        note: &'a SkinNoteSetDef,
        index: usize,
        pressing: bool,
    ) -> Option<&'a String> {
        if !note.lnbody_active.is_empty() {
            if pressing {
                note.lnbody_active.get(index).or_else(|| note.lnbody.get(index))
            } else {
                note.lnbody.get(index).or_else(|| note.lnbody_active.get(index))
            }
        } else if pressing {
            note.lnbody.get(index).or_else(|| note.lnactive.get(index))
        } else {
            note.lnactive.get(index).or_else(|| note.lnbody.get(index))
        }
    }

    /// HCN 用の胴体画像 id を選択する。beatoraja `JsonPlaySkinObjectLoader` の
    /// longImage 割り当てに準拠:
    /// 新形式 (`hcnbodyActive` 定義あり): \[6\]=`hcnbodyActive` \[7\]=`hcnbody`
    /// \[8\]=`hcnbodyReactive` \[9\]=`hcnbodyMiss`。
    /// 旧形式: \[6\]=`hcnbody` \[7\]=`hcnactive` \[8\]=`hcndamage` \[9\]=`hcnreactive`。
    fn hcn_body_image_id<'a>(
        &self,
        note: &'a SkinNoteSetDef,
        index: usize,
        state: LongBodyState,
    ) -> Option<&'a String> {
        let new_format = !note.hcnbody_active.is_empty();
        let primary = match state {
            LongBodyState::Processing => {
                if new_format {
                    note.hcnbody_active.get(index)
                } else {
                    note.hcnbody.get(index)
                }
            }
            LongBodyState::Inactive => {
                if new_format {
                    note.hcnbody.get(index)
                } else {
                    note.hcnactive.get(index)
                }
            }
            LongBodyState::HcnActive => {
                if new_format {
                    note.hcnbody_reactive.get(index)
                } else {
                    note.hcndamage.get(index)
                }
            }
            LongBodyState::HcnDamage => {
                if new_format {
                    note.hcnbody_miss.get(index)
                } else {
                    note.hcnreactive.get(index)
                }
            }
        };
        // 状態別画像が無い場合は HCN の基本 2 状態 → LN 胴体の順にフォールバック。
        primary
            .or_else(|| {
                if new_format {
                    if state.is_processing() {
                        note.hcnbody_active.get(index).or_else(|| note.hcnbody.get(index))
                    } else {
                        note.hcnbody.get(index)
                    }
                } else if state.is_processing() {
                    note.hcnbody.get(index).or_else(|| note.hcnactive.get(index))
                } else {
                    note.hcnactive.get(index).or_else(|| note.hcnbody.get(index))
                }
            })
            .or_else(|| self.ln_body_image_id(note, index, state.is_processing()))
    }

    /// ロングノート胴体画像を描画する。`mode` と `state` の組み合わせで
    /// beatoraja `drawLongNote` の longImage 選択を再現する。
    /// 該当画像が無ければ LN 胴体 → `note` の順にフォールバックする。
    fn note_long_body_render_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        mode: LongNoteMode,
        state: LongBodyState,
        draw_state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem> {
        let note = self.note.as_ref()?;
        let index = beatoraja_note_index(lane, key_mode);
        let image_id = if mode == LongNoteMode::Hcn {
            self.hcn_body_image_id(note, index, state)
        } else {
            self.ln_body_image_id(note, index, state.is_processing())
        }
        .or_else(|| note.note.get(index))?;
        let image = self.image.iter().find(|image| image.id == *image_id)?;
        // LR2 `SRC_LN_BODY` uses the lane HOLD timer for the processing image,
        // while the inactive copy has no timer and must remain on frame zero.
        let elapsed_ms = skin_timer_elapsed_ms(image.timer, draw_state).unwrap_or(0);
        self.note_part_render_item(image_id, rect, elapsed_ms, sources)
    }

    /// Mine ノート画像（`note.mine`）を描画する。スキンが `mine` を定義していない、
    /// または該当レーンの index が空なら `None` を返し、呼び出し側でフォールバックを
    /// 使う想定。
    fn note_mine_render_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem> {
        let note = self.note.as_ref()?;
        let image_id = note.mine.get(beatoraja_note_index(lane, key_mode))?;
        self.note_part_render_item(image_id, rect, 0, sources)
    }

    fn note_height_for_lane(&self, lane: Lane, key_mode: KeyMode) -> Option<f32> {
        let note = self.note.as_ref()?;
        let index = beatoraja_note_index(lane, key_mode);
        if let Some(size) = note.size.get(index).copied().filter(|size| *size > 0) {
            return Some(size as f32 / self.h.max(1) as f32);
        }
        let image_id = note.note.get(index)?;
        let image = self.image.iter().find(|image| image.id == *image_id)?;
        let divy = image.divy.max(1);
        Some((image.h.max(1) as f32 / divy as f32) / self.h.max(1) as f32)
    }

    fn note_part_render_item(
        &self,
        image_id: &str,
        rect: Rect,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem> {
        let image = self.image.iter().find(|image| image.id == image_id)?;
        let source = resolve_document_source(sources, &image.src)?;
        Some(SkinRenderItem::Image {
            texture: source.texture,
            rect,
            uv: skin_image_texture_region(image, source.source_size, elapsed_ms),
            tint: Color::rgb(1.0, 1.0, 1.0),
            blend: BlendMode::Normal,
            scale: SkinImageScale::Stretch,
            border: None,
            source_size: Some(source.source_size),
            linear_filter: false,
        })
    }

    fn note_group_render_items(
        &self,
        note_y: f32,
        key_mode: KeyMode,
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem> {
        let Some(note) = self.note.as_ref() else {
            return Vec::new();
        };
        self.note_line_render_items(&note.group, note_y, key_mode, state, sources)
    }

    fn note_line_render_items(
        &self,
        destinations: &[SkinDestinationDef],
        note_y: f32,
        key_mode: KeyMode,
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Vec<SkinRenderItem> {
        let images = self.image_map();
        let enabled_options = self.enabled_options();
        let Some(area) = self.note_lane_area(Lane::Key1, key_mode, &enabled_options) else {
            return Vec::new();
        };
        let canvas_h = self.h.max(1) as f32;
        let bottom_y = note_progress_to_y(area, note_y, state, canvas_h);
        let judge_bottom_px = canvas_h * (1.0 - note_judge_bottom_y(area, state, canvas_h));
        let timeline_bottom_px = canvas_h * (1.0 - bottom_y);
        let mut items = Vec::new();
        for destination in destinations {
            if !test_skin_ops(&destination.op, &enabled_options, state)
                || !eval_skin_draw_condition(&destination.draw, state)
            {
                continue;
            }
            let Some(elapsed) = skin_timer_elapsed_ms(destination.timer, state) else {
                continue;
            };
            let Some(mut frame) =
                resolve_destination_frame(destination, elapsed, &enabled_options, state)
            else {
                continue;
            };
            frame.y += (timeline_bottom_px - judge_bottom_px).round() as i32;
            apply_bar_line_skin_offsets_to_frame(destination, &mut frame, state);
            let Some(image) = images.get(destination.id.as_str()) else {
                continue;
            };
            let Some(source) = resolve_document_source(sources, &image.src) else {
                continue;
            };
            let pixel_rect = skin_image_pixel_rect(image, &images);
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
            let item = skin_image_item_for_frame(
                source.texture,
                rect,
                uv,
                frame,
                destination.center,
                if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal },
                Some(source.source_size),
                destination.filter != 0,
            );
            items.push(item);
        }
        items
    }

    /// `note.dst` の中から有効な条件に一致するエントリを探し、
    /// 指定レーンのノートエリア矩形（正規化座標）を返す。
    /// ノートエリアはレーン列全体を表す。Y軸: 上端=ノートが最も早い時点、下端=判定ライン。
    ///
    /// note.dst の解釈は2通り:
    /// 1. `load_beatoraja_json` 経由で読んだ場合: `expand_json_skin_value` により条件ブロックが
    ///    展開済みで、dst はレーン順の Frame エントリ列になっている。
    ///    → 全 Frame をフラット配列として `lane_idx` 番目を使う。
    /// 2. 直接 JSON パースした場合: Conditional エントリの frames 配列がレーン対応を持つ。
    ///    → 条件を満たす Conditional を探し、その frames[lane_idx] を使う。
    fn note_lane_area(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        enabled_options: &[i32],
    ) -> Option<Rect> {
        let note = self.note.as_ref()?;
        let lane_idx = beatoraja_note_index(lane, key_mode);
        let canvas_w = self.w as f32;
        let canvas_h = self.h as f32;

        // 全エントリを展開してフラット化。Conditional は条件が合うものだけ展開する。
        let mut flat: Vec<SkinAnimationDef> = Vec::new();
        for entry in &note.dst {
            match entry {
                SkinDstEntry::Frame(f) => flat.push(*f),
                SkinDstEntry::Conditional { if_ops, frames } => {
                    if test_skin_dst_if(if_ops, enabled_options) {
                        flat.extend_from_slice(frames);
                    }
                }
            }
        }

        let frame = flat.get(lane_idx)?;
        if let (Some(x), Some(y), Some(w), Some(h)) = (frame.x, frame.y, frame.w, frame.h) {
            Some(normalize_skin_frame_rect(
                ResolvedSkinFrame { x, y, w, h, ..ResolvedSkinFrame::default() },
                canvas_w as u32,
                canvas_h as u32,
            ))
        } else {
            None
        }
    }

    fn primary_note_lane_height_px(&self) -> Option<i32> {
        let enabled_options = self.enabled_options();
        self.note_lane_area(Lane::Scratch, KeyMode::K7, &enabled_options)
            .or_else(|| self.note_lane_area(Lane::Key1, KeyMode::K7, &enabled_options))
            .map(|area| (area.height * self.h.max(1) as f32).round() as i32)
            .filter(|height| *height > 0)
    }

    fn apply_notes_offset_to_rect(&self, rect: Rect, state: &SkinDrawState) -> Rect {
        let Some(offset) = state.skin_offsets.get(OFFSET_NOTES_1P) else {
            return rect;
        };
        let canvas_w = self.w.max(1) as f32;
        let canvas_h = self.h.max(1) as f32;
        let offset_w = offset.w as f32 / canvas_w;
        let offset_h = offset.h as f32 / canvas_h;
        Rect {
            x: rect.x + offset.x as f32 / canvas_w - offset_w / 2.0,
            y: rect.y - offset.y as f32 / canvas_h - offset_h / 2.0,
            width: rect.width + offset_w,
            height: rect.height + offset_h,
        }
    }

    fn gauge_render_items(
        &self,
        gauge: f32,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<Vec<SkinRenderItem>> {
        let state = SkinDrawState { elapsed_ms, gauge, ..SkinDrawState::default() };
        let enabled_options = self.enabled_options();
        let destination =
            self.all_destinations(&enabled_options).into_iter().find(|destination| {
                self.destination_uses_skin_gauge_bar_render(destination)
                    && destination.timer.is_none()
                    && test_skin_ops(&destination.op, &enabled_options, &state)
                    && eval_skin_draw_condition(&destination.draw, &state)
            })?;
        self.resolve_gauge_destination_items(destination, &enabled_options, &state, sources)
    }

    fn destination_uses_skin_gauge_bar_render(&self, destination: &SkinDestinationDef) -> bool {
        self.skin_gauge_for_destination(destination).is_some()
            && destination.draw.trim().is_empty()
            && destination.blend != 2
    }

    fn destination_uses_skin_gauge_overlay_render(&self, destination: &SkinDestinationDef) -> bool {
        self.skin_gauge_for_destination(destination).is_some()
            && (!destination.draw.trim().is_empty() || destination.blend == 2)
    }

    fn skin_gauge_for_destination(
        &self,
        destination: &SkinDestinationDef,
    ) -> Option<&SkinGaugeDef> {
        self.gauges
            .iter()
            .find(|gauge| gauge.id == destination.id)
            .or_else(|| self.gauge.as_ref().filter(|gauge| gauge.id == destination.id))
    }

    fn resolve_gauge_destination_items(
        &self,
        destination: &SkinDestinationDef,
        enabled_options: &[i32],
        state: &SkinDrawState,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<Vec<SkinRenderItem>> {
        let gauge_def = self.skin_gauge_for_destination(destination)?;
        let elapsed_ms = skin_timer_elapsed_ms(destination.timer, state)?;
        let mut frame = resolve_destination_frame(destination, elapsed_ms, enabled_options, state)?;
        apply_skin_offset_to_frame(destination, &mut frame, state, false);
        let reverse_parts = skin_gauge_reverse_parts(frame);
        let rect = normalize_skin_frame_rect(frame, self.w, self.h);
        let parts = gauge_def.parts.max(1);
        let max = state.gauge_max.max(1.0);
        let border = state.gauge_border;
        let notes = skin_gauge_notes_count(state.gauge, parts, max);
        let animation = skin_gauge_animation_index(gauge_def, state);
        let exgauge = skin_gauge_node_base(state.gauge_type);
        let anim_type = gauge_def.gauge_type;
        let base_color = skin_gauge_frame_color(frame);
        let blend = skin_gauge_destination_blend(destination);
        let mut items = Vec::new();
        for part in 1..=parts {
            let part_border = part as f32 * max / parts as f32;
            let node_index = skin_gauge_sprite_node_index(
                exgauge,
                part,
                notes,
                animation,
                border,
                part_border,
                gauge_def.nodes.len(),
                anim_type,
            );
            let node_id = gauge_def.nodes.get(node_index)?;
            let part_rect = skin_gauge_part_rect(rect, parts, part, reverse_parts);
            if let Some(item) = self.gauge_image_render_item(
                node_id,
                part_rect,
                elapsed_ms,
                sources,
                base_color,
                blend,
                destination.filter != 0,
            ) {
                items.push(item);
            }
            if anim_type == SKIN_GAUGE_ANIM_FLICKERING
                && notes > 0
                && part == notes
                && let Some(tip_index) = skin_gauge_flicker_tip_node_index(
                    exgauge,
                    border,
                    part_border,
                    gauge_def.nodes.len(),
                )
                && let Some(tip_id) = gauge_def.nodes.get(tip_index)
            {
                let flicker_alpha = skin_gauge_flicker_alpha(animation, gauge_def.cycle);
                let flicker_color = Color::rgba(
                    base_color.r,
                    base_color.g,
                    base_color.b,
                    base_color.a * flicker_alpha,
                );
                if let Some(item) = self.gauge_image_render_item(
                    tip_id,
                    part_rect,
                    elapsed_ms,
                    sources,
                    flicker_color,
                    blend,
                    destination.filter != 0,
                ) {
                    items.push(item);
                }
            }
        }
        Some(items)
    }

    fn judge_render_items(
        &self,
        judge: &str,
        combo: u32,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<Vec<SkinRenderItem>> {
        self.judge_render_items_with_offsets(
            judge,
            combo,
            elapsed_ms,
            &SkinOffsetValues::default(),
            sources,
        )
    }

    fn judge_render_items_with_offsets(
        &self,
        judge: &str,
        combo: u32,
        elapsed_ms: i32,
        skin_offsets: &SkinOffsetValues,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<Vec<SkinRenderItem>> {
        let judge_image_index = judge_image_index(judge)?;
        let judge_def = self.judge.first()?;
        let state = SkinDrawState { skin_offsets: *skin_offsets, ..SkinDrawState::default() };
        self.judge_render_items_for_def(
            judge_def,
            judge_image_index,
            combo,
            elapsed_ms,
            sources,
            &state,
        )
    }

    fn judge_render_items_for_def(
        &self,
        judge: &SkinJudgeDef,
        judge_index: usize,
        combo: u32,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
        state: &SkinDrawState,
    ) -> Option<Vec<SkinRenderItem>> {
        let image_destination = judge.images.get(judge_index)?;
        let enabled_options = self.enabled_options();
        let mut image_frame = resolve_destination_frame_until_end(
            image_destination,
            elapsed_ms,
            &enabled_options,
            state,
        )?;
        let offset_state = SkinDrawState {
            skin_offsets: state.skin_offsets,
            offset_lift_px: state.offset_lift_px,
            offset_lanecover_px: state.offset_lanecover_px,
            ..SkinDrawState::default()
        };
        // OFFSET_JUDGE_1P (id 32) は beatoraja では明示注入されず、destination の
        // `offsets` フィールドで宣言されたぶんだけ適用される。ここで重ねて
        // 注入すると、`offsets: [32]` を持つ skin (beatoraja 標準形) で
        // 二重適用になり、判定文字とコンボ数の Y が乖離する原因になる。
        apply_skin_offset_to_frame(image_destination, &mut image_frame, &offset_state, false);
        // beatoraja はコンボ数字をシフト前の判定文字 X を基準に配置する。
        let image_frame_for_numbers = image_frame;
        if judge.shift
            && combo > 0
            && let Some(number_destination) = judge.numbers.get(judge_index)
            && let Some(number_frame) = resolve_destination_frame_until_end(
                number_destination,
                elapsed_ms,
                &enabled_options,
                state,
            )
        {
            image_frame.x -=
                self.value_number_length(&number_destination.id, combo as i64, number_frame) / 2;
        }
        let image = self.image.iter().find(|image| image.id == image_destination.id)?;
        let source = resolve_document_source(sources, &image.src)?;
        let uv = skin_image_texture_region(image, source.source_size, elapsed_ms);
        let (rect, uv) = stretch_skin_image_geometry(
            image_destination.stretch,
            normalize_skin_frame_rect(image_frame, self.w, self.h),
            uv,
            source.source_size,
            self.w,
            self.h,
        );
        let mut items = vec![skin_image_item_for_frame(
            source.texture,
            rect,
            uv,
            image_frame,
            image_destination.center,
            BlendMode::Normal,
            Some(source.source_size),
            image_destination.filter != 0,
        )];
        if combo > 0
            && let Some(number_destination) = judge.numbers.get(judge_index)
            && let Some(mut number_frame) = resolve_destination_frame_until_end(
                number_destination,
                elapsed_ms,
                &enabled_options,
                state,
            )
        {
            // beatoraja は SkinNumber に `setRelative(true)` を立てるため、
            // destination の offsets を適用しても x/y は移動せず w/h/r/a だけ
            // 加算される。これにより combo digit の最終位置は
            // base_frame.y (= 適用後 image_frame.y) + number_frame.y_orig となり、
            // 判定文字と同じ量だけ y シフトする (中心アンカー伸縮)。
            apply_skin_offset_to_frame_relative(
                number_destination,
                &mut number_frame,
                &offset_state,
            );
            let judge_align = self
                .value
                .iter()
                .find(|value| value.id == number_destination.id)
                .map_or(2, |value| value.judge_align.unwrap_or(2));
            if let Some(value) = self.value.iter().find(|value| value.id == number_destination.id)
                && judge_align == 2
            {
                Self::apply_beatoraja_judge_number_dst_x(&mut number_frame, value.digit);
            }
            let signed_render =
                if self.value.iter().find(|value| value.id == number_destination.id).is_some_and(
                    |value| ref_id_is_signed(value.ref_id) || value_layout_is_signed(value),
                ) {
                    SignedNumberRender::Signed(SignedNumberRowOrder::PositiveFirst)
                } else {
                    SignedNumberRender::Unsigned
                };
            items.extend(self.value_number_render_items(
                &number_destination.id,
                combo as i64,
                image_frame_for_numbers,
                number_frame,
                elapsed_ms,
                sources,
                false,
                Some(judge_align),
                signed_render,
            ));
        }
        Some(items)
    }

    /// beatoraja `JsonPlaySkinObjectLoader` が judge number の各 dst に適用する X 補正。
    fn beatoraja_judge_number_dst_x(dst_w: i32, digit: i32) -> i32 {
        dst_w.saturating_mul(digit.max(0)) / 2
    }

    fn apply_beatoraja_judge_number_dst_x(frame: &mut ResolvedSkinFrame, digit: i32) {
        frame.x -= Self::beatoraja_judge_number_dst_x(frame.w, digit);
    }

    fn value_number_length(&self, value_id: &str, number: i64, frame: ResolvedSkinFrame) -> i32 {
        let Some(value) = self.value.iter().find(|value| value.id == value_id) else {
            return 0;
        };
        let max_digits = value.digit.max(0) as usize;
        let padding = number_padding(value);
        let digits = if ref_id_is_signed(value.ref_id) || value_layout_is_signed(value) {
            display_signed_number_digits(
                number,
                max_digits,
                signed_value_padding(value, padding),
                value.divx.max(1) as u32,
            )
        } else {
            display_number_digits(number, max_digits, padding)
        };
        if digits.is_empty() { 0 } else { digits.len() as i32 * (frame.w + value.space) }
    }

    fn judge_image_render_item(
        &self,
        judge: &str,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
    ) -> Option<SkinRenderItem> {
        self.judge_render_items(judge, 0, elapsed_ms, sources)?.into_iter().next()
    }

    fn value_number_render_items(
        &self,
        value_id: &str,
        number: i64,
        base_frame: ResolvedSkinFrame,
        frame: ResolvedSkinFrame,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
        compact_digits: bool,
        align_override: Option<i32>,
        signed_render: SignedNumberRender,
    ) -> Vec<SkinRenderItem> {
        let Some(value) = self.value.iter().find(|value| value.id == value_id) else {
            return Vec::new();
        };
        let Some(source) = sources.get(&value.src) else {
            return Vec::new();
        };
        let divx = value.divx.max(1);
        let divy = value.divy.max(1);
        let source_width_px =
            if value.w == -1 { source.source_size.width.round() as i32 } else { value.w };
        let source_height_px =
            if value.h == -1 { source.source_size.height.round() as i32 } else { value.h };
        let cell_width_px = (source_width_px / divx) as f32;
        let cell_height_px = (source_height_px / divy) as f32;
        if cell_width_px <= 0.0 || cell_height_px <= 0.0 {
            return Vec::new();
        }
        let padding = number_padding(value);
        let max_digits = value.digit.max(0) as usize;
        let digits = match signed_render {
            SignedNumberRender::Signed(row_order) => display_signed_number_digits_with_row_order(
                number,
                max_digits,
                signed_value_padding(value, padding),
                divx as u32,
                row_order,
            ),
            SignedNumberRender::Unsigned => display_number_digits(number, max_digits, padding),
        };
        // 桁間スペース (space フィールド、px 単位)
        let digit_step = frame.w + value.space;
        // 先頭の空き桁数 (align のためのオフセット計算に使用)
        let shiftbase = max_digits.saturating_sub(digits.len());
        // align=0: 右寄せ (デフォルト), align=1: 左寄せ, align=2: 中央
        let align = align_override.unwrap_or(value.align);
        let shift = match align {
            1 => digit_step * shiftbase as i32,
            2 => digit_step * shiftbase as i32 / 2,
            _ => 0,
        };

        digits
            .into_iter()
            .enumerate()
            .map(|(index, digit)| {
                let digit_position = if compact_digits { index } else { shiftbase + index } as i32;
                let rect = normalize_skin_frame_rect(
                    ResolvedSkinFrame {
                        x: base_frame.x + frame.x + digit_step * digit_position - shift,
                        y: base_frame.y + frame.y,
                        w: frame.w,
                        h: frame.h,
                        ..frame
                    },
                    self.w,
                    self.h,
                );
                let uv = Self::value_digit_texture_region(
                    value,
                    digit.into(),
                    elapsed_ms,
                    source.source_size,
                    cell_width_px,
                    cell_height_px,
                    divx,
                    divy,
                );
                let tint = Color::rgba(
                    frame.r as f32 / 255.0,
                    frame.g as f32 / 255.0,
                    frame.b as f32 / 255.0,
                    frame.a as f32 / 255.0,
                );
                SkinRenderItem::Image {
                    texture: source.texture,
                    rect,
                    uv,
                    tint,
                    blend: BlendMode::Normal,
                    scale: SkinImageScale::Stretch,
                    border: None,
                    source_size: Some(source.source_size),
                    linear_filter: false,
                }
            })
            .collect()
    }

    fn value_digit_texture_region(
        value: &SkinValueDef,
        digit: u32,
        elapsed_ms: i32,
        source_size: SkinImageSize,
        cell_width_px: f32,
        cell_height_px: f32,
        divx: i32,
        divy: i32,
    ) -> TextureRegion {
        let source_width = source_size.width.max(1.0);
        let source_height = source_size.height.max(1.0);
        let digit_column = digit as i32 % divx;
        let digit_row = digit as i32 / divx;
        let animation_rows = divy.saturating_sub(digit_row).max(1);
        let animation_row = if value.cycle > 0 && animation_rows > 1 {
            (elapsed_ms.rem_euclid(value.cycle) * animation_rows / value.cycle)
                .min(animation_rows - 1)
        } else {
            0
        };
        let source_row = (digit_row + animation_row).min(divy - 1);
        TextureRegion {
            x: (value.x as f32 + cell_width_px * digit_column as f32) / source_width,
            y: (value.y as f32 + cell_height_px * source_row as f32) / source_height,
            width: cell_width_px / source_width,
            height: cell_height_px / source_height,
        }
    }

    fn gauge_image_render_item(
        &self,
        image_id: &str,
        rect: Rect,
        elapsed_ms: i32,
        sources: &HashMap<String, SkinDocumentTexture>,
        tint: Color,
        blend: BlendMode,
        linear_filter: bool,
    ) -> Option<SkinRenderItem> {
        let image = self.image.iter().find(|image| image.id == image_id)?;
        let source = resolve_document_source(sources, &image.src)?;
        let uv = skin_image_texture_region(image, source.source_size, elapsed_ms);
        let (rect, uv) =
            stretch_skin_image_geometry(0, rect, uv, source.source_size, self.w, self.h);
        Some(SkinRenderItem::Image {
            texture: source.texture,
            rect,
            uv,
            tint,
            blend,
            scale: SkinImageScale::Stretch,
            border: None,
            source_size: Some(source.source_size),
            linear_filter,
        })
    }

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
            let clamped = sample
                .clamp(-visualizer.judge_width_millis as i64, visualizer.judge_width_millis as i64)
                as f32;
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
                rect: Rect { x, y: rect.y + canvas_h - bar_h, width: line_width, height: bar_h },
                color: Color::rgba(line_color.r, line_color.g, line_color.b, alpha * frame_alpha),
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
            .or_else(|| uncached_filtered_points.as_deref().filter(|filtered| !filtered.is_empty()))
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
        let line_w = (visualizer.line_width.clamp(1, 4) as f32 / self.w.max(1) as f32).max(0.001);
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
            let height = if visualizer.draw_decay == 1 { rect.height * age } else { rect.height };
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
            let rects =
                if let Some(cache) = cache { cache.cached_rect_batch(key, build) } else { build() };
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
            let rects =
                if let Some(cache) = cache { cache.cached_rect_batch(key, build) } else { build() };
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
}
