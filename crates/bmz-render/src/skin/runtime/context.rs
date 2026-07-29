use super::*;

/// Renderer-facing interface for Lua draw sidecars. Implementations own the VM
/// outside `SkinDocument`; the renderer only supplies a read-only frame state.
pub trait SkinLuaDrawRuntime: std::fmt::Debug + Send + Sync {
    fn evaluate_draw(
        &self,
        callback_id: usize,
        state: &SkinDrawState,
        enabled_options: &[i32],
        text_values: &BTreeMap<i32, String>,
    ) -> bool;
}

#[derive(Clone)]
pub struct SkinLuaRuntimeContext {
    pub(in crate::skin) runtime: Arc<dyn SkinLuaDrawRuntime>,
    pub(in crate::skin) enabled_options: Arc<[i32]>,
    pub(in crate::skin) text_values: Arc<BTreeMap<i32, String>>,
}

impl std::fmt::Debug for SkinLuaRuntimeContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SkinLuaRuntimeContext")
            .field("enabled_options", &self.enabled_options)
            .field("text_value_count", &self.text_values.len())
            .finish_non_exhaustive()
    }
}

impl PartialEq for SkinLuaRuntimeContext {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.runtime, &other.runtime)
            && self.enabled_options == other.enabled_options
            && self.text_values == other.text_values
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JudgeRegionState {
    pub judge_ms: [Option<i32>; MAX_JUDGE_REGIONS],
    pub judge_index: [Option<usize>; MAX_JUDGE_REGIONS],
    pub judge_combo: [u32; MAX_JUDGE_REGIONS],
    pub judge_timing_sign: [Option<i8>; MAX_JUDGE_REGIONS],
    /// 領域別の最新判定タイミングずれ ms (VALUE_JUDGE_1P/2P/3P_DURATION=525/526/527 に使用)。
    /// 符号は 押下時刻 - note時刻 (FAST=負)。None なら非表示。
    pub judge_timing_ms: [Option<i32>; MAX_JUDGE_REGIONS],
}

/// レーン index から判定領域 index へ (beatoraja `JudgeManager.updateMicro` 同式)。
pub fn lane_judge_region(lane_index: usize, lane_count: usize, region_count: usize) -> usize {
    if lane_count == 0 || region_count == 0 {
        return 0;
    }
    let region = lane_index * region_count / lane_count;
    region.min(region_count.saturating_sub(1))
}

/// `recent_judgements` から領域別の判定 timer / 画像 index を構築する。
pub fn build_judge_region_state(
    recent_judgements: &[crate::snapshot::DisplayJudgement],
    render_now_us: i64,
    region_count: usize,
) -> JudgeRegionState {
    let mut judge_ms = [None; MAX_JUDGE_REGIONS];
    let mut judge_index = [None; MAX_JUDGE_REGIONS];
    let mut judge_combo = [0; MAX_JUDGE_REGIONS];
    let mut judge_timing_sign = [None; MAX_JUDGE_REGIONS];
    let mut judge_timing_ms = [None; MAX_JUDGE_REGIONS];
    let region_count = region_count.min(MAX_JUDGE_REGIONS);
    for judgement in recent_judgements.iter().rev() {
        let region = lane_judge_region(judgement.lane.index(), LANE_COUNT, region_count);
        if judge_ms[region].is_some() {
            continue;
        }
        judge_ms[region] = Some(
            ((render_now_us - judgement.time.0) / 1_000).clamp(i32::MIN as i64, i32::MAX as i64)
                as i32,
        );
        judge_index[region] = Some(judge_image_index_for_judge(judgement.judge));
        judge_combo[region] = judgement.combo;
        judge_timing_sign[region] = judgement.side.map(|side| match side {
            TimingSide::Fast => 1,
            TimingSide::Slow => -1,
        });
        if !judgement.timing_ms_suppressed {
            judge_timing_ms[region] =
                Some((judgement.delta_us / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32);
        }
    }
    JudgeRegionState { judge_ms, judge_index, judge_combo, judge_timing_sign, judge_timing_ms }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinClickTarget {
    Event { event_id: i32, click: i32 },
    SelectRow { row_index: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinClickHit {
    pub target: SkinClickTarget,
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinSliderHit {
    pub slider_type: i32,
    pub value: f32,
}

#[derive(Debug, Clone)]
pub struct SkinContext {
    manifest: SkinManifest,
    document: Option<SkinDocument>,
    lua_draw_runtime: Option<Arc<dyn SkinLuaDrawRuntime>>,
    document_sources: HashMap<String, SkinDocumentTexture>,
    select_settings_dest_index: Arc<crate::select_settings_dest::SelectSettingsDestIndex>,
    result_render_cache: Arc<Mutex<ResultRenderCache>>,
}

impl PartialEq for SkinContext {
    fn eq(&self, other: &Self) -> bool {
        self.manifest == other.manifest
            && self.document == other.document
            && match (&self.lua_draw_runtime, &other.lua_draw_runtime) {
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
            && self.document_sources == other.document_sources
            && self.select_settings_dest_index == other.select_settings_dest_index
    }
}

pub(in crate::skin) const RESULT_RENDER_CACHE_MAX_ENTRIES: usize = 64;
static NEXT_RESULT_GAUGE_GRAPH_REVISION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Default)]
pub(in crate::skin) struct ResultRenderCache {
    planning: Option<ResultPlanningCache>,
    rect_batches: HashMap<ResultRectBatchCacheKey, Arc<[RectCommand]>>,
    gauge_graph: Option<ResultGaugeGraphCache>,
    gauge_rect_batches: HashMap<ResultGaugeGraphRectBatchCacheKey, Arc<[RectCommand]>>,
}

impl ResultRenderCache {
    pub(in crate::skin) fn cached_planning(
        &mut self,
        document: &SkinDocument,
    ) -> ResultPlanningCache {
        if let Some(planning) = &self.planning {
            return planning.clone();
        }
        let enabled_options = Arc::<[i32]>::from(document.enabled_options());
        let mut destinations = Vec::new();
        for (entry_index, entry) in document.destination.iter().enumerate() {
            match entry {
                DestinationListEntry::Single(_) => {
                    destinations.push(ResultDestinationRef::Single { entry_index });
                }
                DestinationListEntry::Conditional { if_ops, destinations: entries } => {
                    if test_skin_dst_if(if_ops, &enabled_options) {
                        destinations.extend(entries.iter().enumerate().map(
                            |(destination_index, _)| ResultDestinationRef::Conditional {
                                entry_index,
                                destination_index,
                            },
                        ));
                    }
                }
            }
        }
        let has_nearest_f_diff_rank_destination = destinations
            .iter()
            .filter_map(|destination| destination.resolve(document))
            .any(|destination| destination.id == "RANK_s_F");
        let planning = ResultPlanningCache {
            enabled_options,
            destinations: Arc::from(destinations),
            has_nearest_f_diff_rank_destination,
        };
        self.planning = Some(planning.clone());
        planning
    }

    pub(in crate::skin) fn cached_rect_batch(
        &mut self,
        key: ResultRectBatchCacheKey,
        build: impl FnOnce() -> Arc<[RectCommand]>,
    ) -> Arc<[RectCommand]> {
        if let Some(rects) = self.rect_batches.get(&key) {
            return Arc::clone(rects);
        }
        let rects = build();
        if self.rect_batches.len() >= RESULT_RENDER_CACHE_MAX_ENTRIES {
            self.rect_batches.clear();
        }
        self.rect_batches.insert(key, Arc::clone(&rects));
        rects
    }

    pub(in crate::skin) fn prepare_gauge_graph(
        &mut self,
        graph: &Arc<crate::snapshot::ResultGraphSnapshot>,
    ) {
        if self.gauge_graph.as_ref().is_some_and(|cached| Arc::ptr_eq(&cached.graph, graph)) {
            return;
        }
        let revision = NEXT_RESULT_GAUGE_GRAPH_REVISION.fetch_add(1, Ordering::Relaxed);
        self.gauge_graph = Some(ResultGaugeGraphCache {
            graph: Arc::clone(graph),
            revision,
            points_by_type: HashMap::new(),
        });
        if self.gauge_rect_batches.len() >= RESULT_RENDER_CACHE_MAX_ENTRIES {
            self.gauge_rect_batches.clear();
        }
    }

    pub(in crate::skin) fn cached_gauge_points(
        &mut self,
        gauge_type: i32,
    ) -> Option<(u64, Arc<[crate::snapshot::ResultGaugeGraphPoint]>)> {
        let cached = self.gauge_graph.as_mut()?;
        let points = cached
            .points_by_type
            .entry(gauge_type)
            .or_insert_with(|| {
                let filtered = cached
                    .graph
                    .gauge_points
                    .iter()
                    .copied()
                    .filter(|point| point.gauge_type == gauge_type)
                    .collect::<Vec<_>>();
                if filtered.is_empty() {
                    Arc::from(cached.graph.gauge_points.as_slice())
                } else {
                    Arc::from(filtered)
                }
            })
            .clone();
        Some((cached.revision, points))
    }

    pub(in crate::skin) fn gauge_graph_revision(&self) -> Option<u64> {
        self.gauge_graph.as_ref().map(|cached| cached.revision)
    }

    pub(in crate::skin) fn cached_gauge_rect_batch(
        &mut self,
        key: ResultGaugeGraphRectBatchCacheKey,
        build: impl FnOnce() -> Arc<[RectCommand]>,
    ) -> Arc<[RectCommand]> {
        if let Some(rects) = self.gauge_rect_batches.get(&key) {
            return Arc::clone(rects);
        }
        let rects = build();
        if self.gauge_rect_batches.len() >= RESULT_RENDER_CACHE_MAX_ENTRIES {
            self.gauge_rect_batches.clear();
        }
        self.gauge_rect_batches.insert(key, Arc::clone(&rects));
        rects
    }
}

#[derive(Debug)]
pub(in crate::skin) struct ResultGaugeGraphCache {
    graph: Arc<crate::snapshot::ResultGraphSnapshot>,
    revision: u64,
    points_by_type: HashMap<i32, Arc<[crate::snapshot::ResultGaugeGraphPoint]>>,
}

#[derive(Debug, Clone)]
pub(in crate::skin) struct ResultPlanningCache {
    pub(in crate::skin) enabled_options: Arc<[i32]>,
    pub(in crate::skin) destinations: Arc<[ResultDestinationRef]>,
    pub(in crate::skin) has_nearest_f_diff_rank_destination: bool,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::skin) enum ResultDestinationRef {
    Single { entry_index: usize },
    Conditional { entry_index: usize, destination_index: usize },
}

impl ResultDestinationRef {
    pub(in crate::skin) fn resolve(self, document: &SkinDocument) -> Option<&SkinDestinationDef> {
        match (self, document.destination.get(self.entry_index())) {
            (
                ResultDestinationRef::Single { .. },
                Some(DestinationListEntry::Single(destination)),
            ) => Some(destination),
            (
                ResultDestinationRef::Conditional { destination_index, .. },
                Some(DestinationListEntry::Conditional { destinations, .. }),
            ) => destinations.get(destination_index),
            _ => None,
        }
    }

    fn entry_index(self) -> usize {
        match self {
            ResultDestinationRef::Single { entry_index }
            | ResultDestinationRef::Conditional { entry_index, .. } => entry_index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::skin) struct ResultRectBatchCacheKey {
    pub(in crate::skin) destination_index: usize,
    pub(in crate::skin) kind: ResultRectBatchKind,
    pub(in crate::skin) frame: ResolvedSkinFrame,
    pub(in crate::skin) key_mode: KeyMode,
    pub(in crate::skin) judge_rank: Option<i32>,
    pub(in crate::skin) visible_len: usize,
    pub(in crate::skin) data_hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::skin) enum ResultRectBatchKind {
    Judge,
    EarlyLate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::skin) struct ResultGaugeGraphRectBatchCacheKey {
    pub(in crate::skin) destination_index: usize,
    pub(in crate::skin) frame: ResolvedSkinFrame,
    pub(in crate::skin) graph_revision: u64,
    pub(in crate::skin) display_gauge_type: i32,
    pub(in crate::skin) gauge_max_bits: u32,
    pub(in crate::skin) gauge_border_bits: u32,
}

impl Default for SkinContext {
    fn default() -> Self {
        Self {
            manifest: default_skin_manifest(),
            document: None,
            lua_draw_runtime: None,
            document_sources: HashMap::new(),
            select_settings_dest_index: Arc::new(
                crate::select_settings_dest::SelectSettingsDestIndex::default(),
            ),
            result_render_cache: Arc::new(Mutex::new(ResultRenderCache::default())),
        }
    }
}

impl SkinContext {
    pub fn from_manifest(manifest: SkinManifest) -> Self {
        Self {
            manifest,
            document: None,
            lua_draw_runtime: None,
            document_sources: HashMap::new(),
            select_settings_dest_index: Arc::new(
                crate::select_settings_dest::SelectSettingsDestIndex::default(),
            ),
            result_render_cache: Arc::new(Mutex::new(ResultRenderCache::default())),
        }
    }

    pub fn from_manifest_and_document(
        manifest: SkinManifest,
        document: SkinDocument,
        document_sources: impl IntoIterator<Item = SkinDocumentTexture>,
    ) -> Self {
        let select_settings_dest_index =
            Arc::new(crate::select_settings_dest::build_select_settings_dest_index(&document));
        Self {
            manifest,
            document: Some(document),
            lua_draw_runtime: None,
            document_sources: document_sources
                .into_iter()
                .map(|source| (source.source_id.clone(), source))
                .collect(),
            select_settings_dest_index,
            result_render_cache: Arc::new(Mutex::new(ResultRenderCache::default())),
        }
    }

    pub fn manifest(&self) -> &SkinManifest {
        &self.manifest
    }

    pub fn document(&self) -> Option<&SkinDocument> {
        self.document.as_ref()
    }

    pub fn set_lua_draw_runtime(&mut self, runtime: Option<Arc<dyn SkinLuaDrawRuntime>>) {
        self.lua_draw_runtime = runtime;
    }

    fn state_with_lua_runtime(
        &self,
        state: &SkinDrawState,
        text: &SkinTextState<'_>,
    ) -> SkinDrawState {
        let mut state = state.clone();
        let Some(runtime) = self.lua_draw_runtime.as_ref() else {
            return state;
        };
        let enabled_options: Arc<[i32]> = self
            .document
            .as_ref()
            .map(|document| Arc::from(document.enabled_options()))
            .unwrap_or_else(|| Arc::from([]));
        state.lua_runtime = Some(SkinLuaRuntimeContext {
            runtime: Arc::clone(runtime),
            enabled_options,
            text_values: Arc::new(lua_main_state_text_values(&state, text)),
        });
        state
    }

    pub fn set_user_selected_options(&mut self, enabled_options: Vec<i32>) -> bool {
        let Some(document) = &mut self.document else {
            return false;
        };
        document.user_selected_options = Some(enabled_options);
        true
    }

    pub fn with_play_graphs(
        &self,
        judge_graph_density: Vec<u8>,
        bpm_graph_segments: Vec<crate::chart_graph::BpmGraphSegment>,
    ) -> Self {
        let mut cloned = self.clone();
        if let Some(document) = &mut cloned.document {
            document.play_judge_graph_density = judge_graph_density;
            document.play_bpm_graph_segments = bpm_graph_segments;
        }
        cloned
    }

    pub fn with_result_graphs(&self, graph: &crate::snapshot::ResultGraphSnapshot) -> Self {
        let mut cloned = self.clone();
        if let Some(document) = &mut cloned.document {
            document.play_judge_graph_density = graph.judge_graph_density.clone();
            document.play_bpm_graph_segments = graph.bpm_graph_segments.clone();
            document.result_gauge_graph_points = graph.gauge_points.clone();
            document.result_timing_points = graph.timing_points.clone();
            document.result_judge_graph_buckets = graph.judge_graph_buckets.clone();
            document.result_early_late_graph_buckets = graph.early_late_graph_buckets.clone();
            document.result_timing_distribution = graph.timing_distribution.clone();
        }
        cloned
    }

    pub fn static_document_items(&self) -> Vec<SkinRenderItem> {
        self.static_document_items_for_state(&SkinDrawState::default())
    }

    pub fn static_document_items_for_state(&self, state: &SkinDrawState) -> Vec<SkinRenderItem> {
        self.static_document_items_for_state_and_text(state, &SkinTextState::default())
    }

    pub fn static_document_items_for_state_and_text(
        &self,
        state: &SkinDrawState,
        text: &SkinTextState<'_>,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = &self.document else {
            return Vec::new();
        };
        let runtime_sources = static_runtime_document_sources(&self.document_sources, state);
        let state = self.state_with_lua_runtime(state, text);
        document.static_render_items(&runtime_sources, &state, text)
    }

    pub fn static_document_items_for_result_state_and_text(
        &self,
        graph: &Arc<crate::snapshot::ResultGraphSnapshot>,
        state: &SkinDrawState,
        text: &SkinTextState<'_>,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = &self.document else {
            return Vec::new();
        };
        let runtime_sources = static_runtime_document_sources(&self.document_sources, state);
        let state = self.state_with_lua_runtime(state, text);
        // A runtime callback may execute arbitrary bounded Lua. Do not hold the
        // result cache lock across that call.
        if self.lua_draw_runtime.is_none()
            && let Ok(mut cache) = self.result_render_cache.lock()
        {
            cache.prepare_gauge_graph(graph);
            return document.static_render_items_with_graphs_cached(
                &runtime_sources,
                &state,
                text,
                SkinRuntimeGraphs::from_result_graph(graph.as_ref()),
                Some(&mut cache),
            );
        }
        document.static_render_items_with_graphs(
            &runtime_sources,
            &state,
            text,
            SkinRuntimeGraphs::from_result_graph(graph.as_ref()),
        )
    }

    pub fn select_document_items(&self, snapshot: &SelectSnapshot) -> Vec<SkinRenderItem> {
        self.select_document_items_with_dynamic_timers(snapshot, None)
    }

    pub fn select_document_items_with_dynamic_timers(
        &self,
        snapshot: &SelectSnapshot,
        dynamic_timers: Option<&mut DynamicTimerRuntime>,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = &self.document else {
            return Vec::new();
        };
        let runtime_sources = select_runtime_document_sources(&self.document_sources, snapshot);
        document.select_render_items_with_dynamic_timers(
            &runtime_sources,
            snapshot,
            dynamic_timers,
            &self.select_settings_dest_index,
            self.lua_draw_runtime.clone(),
        )
    }

    pub fn select_click_hit(
        &self,
        snapshot: &SelectSnapshot,
        x: f32,
        y: f32,
    ) -> Option<SkinClickHit> {
        let document = self.document.as_ref()?;
        document.select_click_hit(
            &self.document_sources,
            snapshot,
            &self.select_settings_dest_index,
            x,
            y,
        )
    }

    pub fn result_click_hit(&self, state: &SkinDrawState, x: f32, y: f32) -> Option<SkinClickHit> {
        self.document.as_ref()?.result_click_hit(state, x, y)
    }

    pub fn result_slider_hit(
        &self,
        state: &SkinDrawState,
        x: f32,
        y: f32,
    ) -> Option<SkinSliderHit> {
        self.document.as_ref()?.result_slider_hit(state, x, y)
    }

    pub fn select_slider_hit(
        &self,
        snapshot: &SelectSnapshot,
        x: f32,
        y: f32,
    ) -> Option<SkinSliderHit> {
        let document = self.document.as_ref()?;
        document.select_slider_hit(snapshot, &self.select_settings_dest_index, x, y)
    }

    /// 静的 destination を `{"id":"notes"}` マーカーと `timer: 3` (FAILED) で分割して返す。
    /// `.0` はノーツ背面、`.1` はノーツ前面、`.2` は閉店/暗転オーバーレイ（最前面）。
    pub fn static_document_items_split_for_state_and_text(
        &self,
        state: &SkinDrawState,
        text: &SkinTextState<'_>,
    ) -> (Vec<SkinRenderItem>, Vec<SkinRenderItem>, Vec<SkinRenderItem>) {
        let Some(document) = &self.document else {
            return (Vec::new(), Vec::new(), Vec::new());
        };
        let runtime_sources = static_runtime_document_sources(&self.document_sources, state);
        let state = self.state_with_lua_runtime(state, text);
        document.static_render_items_split(&runtime_sources, &state, text)
    }

    pub fn static_document_play_items_split_for_state_and_text(
        &self,
        state: &SkinDrawState,
        text: &SkinTextState<'_>,
        play_judge_graph_density: &[u8],
        play_bpm_graph_segments: &[crate::chart_graph::BpmGraphSegment],
    ) -> (Vec<SkinRenderItem>, Vec<SkinRenderItem>, Vec<SkinRenderItem>) {
        let Some(document) = &self.document else {
            return (Vec::new(), Vec::new(), Vec::new());
        };
        let runtime_sources = static_runtime_document_sources(&self.document_sources, state);
        let state = self.state_with_lua_runtime(state, text);
        document.static_render_items_split_with_graphs(
            &runtime_sources,
            &state,
            text,
            SkinRuntimeGraphs::from_document_with_play_graphs(
                document,
                play_judge_graph_density,
                play_bpm_graph_segments,
            ),
            None,
        )
    }

    pub fn document_note_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
    ) -> Option<SkinRenderItem> {
        let document = self.document.as_ref()?;
        document.note_image_render_item(lane, key_mode, rect, &self.document_sources)
    }

    pub fn document_ln_start_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        mode: LongNoteMode,
    ) -> Option<SkinRenderItem> {
        let document = self.document.as_ref()?;
        document.note_ln_start_render_item(lane, key_mode, rect, mode, &self.document_sources)
    }

    pub fn document_ln_end_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        mode: LongNoteMode,
    ) -> Option<SkinRenderItem> {
        let document = self.document.as_ref()?;
        document.note_ln_end_render_item(lane, key_mode, rect, mode, &self.document_sources)
    }

    /// ロングノート胴体（`note.lnbody` 系 / `note.hcnbody` 系）を指定矩形に伸縮描画する。
    pub fn document_long_body_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
        mode: LongNoteMode,
        state: LongBodyState,
        draw_state: &SkinDrawState,
    ) -> Option<SkinRenderItem> {
        let document = self.document.as_ref()?;
        document.note_long_body_render_item(
            lane,
            key_mode,
            rect,
            mode,
            state,
            draw_state,
            &self.document_sources,
        )
    }

    /// Mine ノート（`note.mine`）を指定矩形に描画する。スキン側に定義が無ければ
    /// `None` を返すため、呼び出し側はデフォルトテクスチャ等のフォールバックへ
    /// 落ちる。
    pub fn document_mine_item(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        rect: Rect,
    ) -> Option<SkinRenderItem> {
        let document = self.document.as_ref()?;
        document.note_mine_render_item(lane, key_mode, rect, &self.document_sources)
    }

    pub fn document_note_height(&self, lane: Lane, key_mode: KeyMode) -> Option<f32> {
        let document = self.document.as_ref()?;
        document.note_height_for_lane(lane, key_mode)
    }

    pub fn document_note_expansion_scale(&self, state: &SkinDrawState) -> (f32, f32) {
        let Some(note) = self.document.as_ref().and_then(|document| document.note.as_ref()) else {
            return (1.0, 1.0);
        };
        let elapsed = state.quarter_note_elapsed_ms.unwrap_or(i32::MAX).max(0) as f32;
        let pulse = if elapsed < 9.0 {
            elapsed / 9.0
        } else if elapsed <= 159.0 {
            (159.0 - elapsed) / 150.0
        } else {
            0.0
        };
        let width = note.expansionrate.first().copied().unwrap_or(100) as f32 / 100.0;
        let height = note.expansionrate.get(1).copied().unwrap_or(100) as f32 / 100.0;
        (1.0 + (width - 1.0) * pulse, 1.0 + (height - 1.0) * pulse)
    }

    pub fn document_bar_line_items(
        &self,
        note_y: f32,
        key_mode: KeyMode,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = self.document.as_ref() else {
            return Vec::new();
        };
        let state = self.state_with_lua_runtime(state, &SkinTextState::default());
        document.note_group_render_items(note_y, key_mode, &state, &self.document_sources)
    }

    pub fn document_bpm_line_items(
        &self,
        note_y: f32,
        key_mode: KeyMode,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = self.document.as_ref() else {
            return Vec::new();
        };
        let Some(note) = document.note.as_ref() else {
            return Vec::new();
        };
        let state = self.state_with_lua_runtime(state, &SkinTextState::default());
        document.note_line_render_items(&note.bpm, note_y, key_mode, &state, &self.document_sources)
    }

    pub fn document_stop_line_items(
        &self,
        note_y: f32,
        key_mode: KeyMode,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = self.document.as_ref() else {
            return Vec::new();
        };
        let Some(note) = document.note.as_ref() else {
            return Vec::new();
        };
        let state = self.state_with_lua_runtime(state, &SkinTextState::default());
        document.note_line_render_items(
            &note.stop,
            note_y,
            key_mode,
            &state,
            &self.document_sources,
        )
    }

    pub fn document_time_line_items(
        &self,
        note_y: f32,
        key_mode: KeyMode,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem> {
        let Some(document) = self.document.as_ref() else {
            return Vec::new();
        };
        let Some(note) = document.note.as_ref() else {
            return Vec::new();
        };
        let state = self.state_with_lua_runtime(state, &SkinTextState::default());
        document.note_line_render_items(
            &note.time,
            note_y,
            key_mode,
            &state,
            &self.document_sources,
        )
    }

    pub fn document_gauge_items(&self, gauge: f32, elapsed_ms: i32) -> Option<Vec<SkinRenderItem>> {
        let document = self.document.as_ref()?;
        document.gauge_render_items(gauge, elapsed_ms, &self.document_sources)
    }

    pub fn timer_animation_duration_ms(&self, timer: i32) -> i32 {
        self.document.as_ref().map_or(0, |document| {
            let enabled_options = document.enabled_options();
            document
                .all_destinations(&enabled_options)
                .into_iter()
                .filter(|destination| destination.timer == Some(timer))
                .filter_map(|destination| {
                    flatten_dst_entries(&destination.dst, &enabled_options)
                        .into_iter()
                        .map(|frame| frame.time.unwrap_or(0))
                        .max()
                })
                .max()
                .unwrap_or(0)
                .max(0)
        })
    }

    pub fn document_judge_items(
        &self,
        judge: &str,
        combo: u32,
        elapsed_ms: i32,
        skin_offsets: &SkinOffsetValues,
        region: usize,
    ) -> Option<Vec<SkinRenderItem>> {
        let document = self.document.as_ref()?;
        let judge_image_index = judge_image_index(judge)?;
        let judge_def = document
            .judge
            .iter()
            .find(|j| j.index == region as i32)
            .or_else(|| document.judge.first())?;
        let state = SkinDrawState { skin_offsets: *skin_offsets, ..SkinDrawState::default() };
        document.judge_render_items_for_def(
            judge_def,
            judge_image_index,
            combo,
            elapsed_ms,
            &self.document_sources,
            &state,
        )
    }

    pub fn apply_play_skin_global_offset(
        &self,
        items: Vec<SkinRenderItem>,
        state: &SkinDrawState,
    ) -> Vec<SkinRenderItem> {
        if self.document.is_none() {
            return items;
        }
        items.into_iter().map(|item| apply_all_offset_to_render_item(item, state)).collect()
    }

    pub fn apply_play_skin_global_offset_to_item(
        &self,
        item: SkinRenderItem,
        state: &SkinDrawState,
    ) -> SkinRenderItem {
        if self.document.is_none() {
            return item;
        }
        apply_all_offset_to_render_item(item, state)
    }

    /// beatoraja スキンの `note.dst` からレーンのノートエリアを取得し、
    /// `note_y`（0.0=判定ライン, 1.0=最上部）に対応するノート矩形を返す。
    /// `note_height` は正規化座標での高さ。ドキュメントスキンが無い場合は `None`。
    pub fn note_rect_for_progress(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        note_y: f32,
        note_height: f32,
        state: &SkinDrawState,
    ) -> Option<Rect> {
        let document = self.document.as_ref()?;
        let enabled_options = document.enabled_options();
        let area = document.note_lane_area(lane, key_mode, &enabled_options)?;
        let canvas_h = document.h.max(1) as f32;
        let bottom_y = note_progress_to_y(area, note_y, state, canvas_h);
        let rect =
            Rect { x: area.x, y: bottom_y - note_height, width: area.width, height: note_height };
        Some(document.apply_notes_offset_to_rect(rect, state))
    }

    pub fn missed_note_rect_for_fall(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        fall: f32,
        note_height: f32,
        state: &SkinDrawState,
    ) -> Option<Rect> {
        let document = self.document.as_ref()?;
        let note = document.note.as_ref()?;
        if note.dst2 == i32::MIN {
            return None;
        }
        let enabled_options = document.enabled_options();
        let area = document.note_lane_area(lane, key_mode, &enabled_options)?;
        let canvas_h = document.h.max(1) as f32;
        let judge_bottom = note_judge_bottom_y(area, state, canvas_h);
        let target_bottom = (canvas_h - note.dst2 as f32) / canvas_h;
        let bottom_y = judge_bottom + (target_bottom - judge_bottom) * fall.clamp(0.0, 1.0);
        let rect =
            Rect { x: area.x, y: bottom_y - note_height, width: area.width, height: note_height };
        Some(document.apply_notes_offset_to_rect(rect, state))
    }

    /// ロングノート胴体の矩形を計算する。`head_y`/`tail_y` は `VisibleNote::y` と同じ
    /// 正規化座標（0.0=判定ライン, 1.0=最奥）。
    pub fn note_body_rect(
        &self,
        lane: Lane,
        key_mode: KeyMode,
        head_y: f32,
        tail_y: f32,
        state: &SkinDrawState,
    ) -> Option<Rect> {
        let document = self.document.as_ref()?;
        let enabled_options = document.enabled_options();
        let area = document.note_lane_area(lane, key_mode, &enabled_options)?;
        let canvas_h = document.h.max(1) as f32;
        let note_height = document.note_height_for_lane(lane, key_mode)?;
        let head_bottom = note_progress_to_y(area, head_y, state, canvas_h);
        let tail_bottom = note_progress_to_y(area, tail_y, state, canvas_h);
        // beatoraja の drawLongNote に合わせる:
        //   body = [dsty+scale, dsty+dy]  (LibGDX y-up)
        //       = [tail_bottom, head_bottom - note_height]  (y-down)
        // 胴体は tail キャップの下端から head キャップの上端まで、キャップと重ならない。
        let top = head_bottom.min(tail_bottom);
        let bottom = head_bottom.max(tail_bottom) - note_height;
        Some(document.apply_notes_offset_to_rect(
            Rect { x: area.x, y: top, width: area.width, height: bottom - top },
            state,
        ))
    }
}

pub(in crate::skin) fn select_runtime_document_sources(
    base_sources: &HashMap<String, SkinDocumentTexture>,
    snapshot: &SelectSnapshot,
) -> HashMap<String, SkinDocumentTexture> {
    let mut sources = base_sources.clone();
    if snapshot.stage_background
        && let Some(source_size) = snapshot.stage_image_size
    {
        insert_runtime_document_source(&mut sources, "100", SELECT_STAGE_TEXTURE, source_size);
    }
    if snapshot.backbmp_image
        && let Some(source_size) = snapshot.backbmp_image_size
    {
        insert_runtime_document_source(&mut sources, "101", PLAY_BACKBMP_TEXTURE, source_size);
    }
    if snapshot.banner_image
        && let Some(source_size) = snapshot.banner_image_size
    {
        insert_runtime_document_source(&mut sources, "102", SELECT_BANNER_TEXTURE, source_size);
    }
    sources
}

pub(in crate::skin) fn static_runtime_document_sources(
    base_sources: &HashMap<String, SkinDocumentTexture>,
    state: &SkinDrawState,
) -> HashMap<String, SkinDocumentTexture> {
    let mut sources = base_sources.clone();
    if state.has_stagefile
        && let Some(source_size) = state.stagefile_image_size
    {
        insert_runtime_document_source(&mut sources, "100", SELECT_STAGE_TEXTURE, source_size);
    }
    if state.has_backbmp {
        insert_runtime_document_source(
            &mut sources,
            "101",
            PLAY_BACKBMP_TEXTURE,
            SkinImageSize { width: 1.0, height: 1.0 },
        );
    }
    sources
}

pub(in crate::skin) fn insert_runtime_document_source(
    sources: &mut HashMap<String, SkinDocumentTexture>,
    source_id: &str,
    texture: TextureId,
    source_size: SkinImageSize,
) {
    sources.insert(
        source_id.to_string(),
        SkinDocumentTexture {
            source_id: source_id.to_string(),
            texture: SkinTextureId(texture.0),
            source_size,
        },
    );
}
