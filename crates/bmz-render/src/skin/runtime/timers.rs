use super::*;

/// `dynamicTimer` observe 条件のエッジ検出用ランタイム。Renderer が保持する。
#[derive(Debug, Clone)]
pub struct DynamicTimerRuntime {
    runtime_flags: HashMap<i32, bool>,
    runtime_flags_initialized: bool,
    starts: [Option<i32>; SKIN_DYNAMIC_TIMER_COUNT],
    logical_input_initialized: bool,
    logical_input_held: [bool; SKIN_BMZ_INPUT_COUNT],
    logical_input_starts: [Option<i32>; SKIN_BMZ_INPUT_COUNT],
    keybeam_keyon_starts: [Option<i32>; LANE_COUNT],
    keybeam_keyoff_starts: [Option<i32>; LANE_COUNT],
    keybeam_suppressed: [bool; LANE_COUNT],
    keybeam_fade_allowed: [bool; LANE_COUNT],
    key_logger: KeyLoggerRuntime,
    judge_lane: JudgeLaneRuntime,
}

#[derive(Debug, Clone, Default)]
pub(in crate::skin) struct KeyLoggerRuntime {
    last_sequence: Option<u64>,
    last_now_us: Option<i64>,
    press_history_us: VecDeque<i64>,
    judge_counts: [[u32; 4]; LANE_COUNT],
    fast_slow_counts: [[u32; 3]; LANE_COUNT],
    event_started_ms: [[Option<i32>; 16]; LANE_COUNT],
    event_started_us: [[Option<i64>; 16]; LANE_COUNT],
    event_judge: [[u8; 16]; LANE_COUNT],
    event_fast_slow: [[u8; 16]; LANE_COUNT],
    next_event_slot: [usize; LANE_COUNT],
}

/// BMZ拡張の判定領域×Scratch/鍵盤別の最新判定。
///
/// 標準判定表示の800msリストとは別にrenderer runtimeへラッチし、destinationの
/// timer/loopだけで各チャンネルの表示時間を決められるようにする。
#[derive(Debug, Clone)]
struct JudgeLaneRuntime {
    last_now_us: Option<i64>,
    region_count: Option<usize>,
    region_started_us: [Option<i64>; MAX_JUDGE_REGIONS],
    region_judge_index: [Option<usize>; MAX_JUDGE_REGIONS],
    region_combo: [u32; MAX_JUDGE_REGIONS],
    region_timing_sign: [Option<i8>; MAX_JUDGE_REGIONS],
    region_timing_ms: [Option<i32>; MAX_JUDGE_REGIONS],
    started_us: [Option<i64>; SKIN_BMZ_JUDGE_LANE_COUNT],
    judge_index: [Option<usize>; SKIN_BMZ_JUDGE_LANE_COUNT],
    timing_sign: [Option<i8>; SKIN_BMZ_JUDGE_LANE_COUNT],
    timing_ms: [Option<i32>; SKIN_BMZ_JUDGE_LANE_COUNT],
}

impl Default for JudgeLaneRuntime {
    fn default() -> Self {
        Self {
            last_now_us: None,
            region_count: None,
            region_started_us: [None; MAX_JUDGE_REGIONS],
            region_judge_index: [None; MAX_JUDGE_REGIONS],
            region_combo: [0; MAX_JUDGE_REGIONS],
            region_timing_sign: [None; MAX_JUDGE_REGIONS],
            region_timing_ms: [None; MAX_JUDGE_REGIONS],
            started_us: [None; SKIN_BMZ_JUDGE_LANE_COUNT],
            judge_index: [None; SKIN_BMZ_JUDGE_LANE_COUNT],
            timing_sign: [None; SKIN_BMZ_JUDGE_LANE_COUNT],
            timing_ms: [None; SKIN_BMZ_JUDGE_LANE_COUNT],
        }
    }
}

impl Default for DynamicTimerRuntime {
    fn default() -> Self {
        Self {
            runtime_flags: HashMap::new(),
            runtime_flags_initialized: false,
            starts: [None; SKIN_DYNAMIC_TIMER_COUNT],
            logical_input_initialized: false,
            logical_input_held: [false; SKIN_BMZ_INPUT_COUNT],
            logical_input_starts: [None; SKIN_BMZ_INPUT_COUNT],
            keybeam_keyon_starts: [None; LANE_COUNT],
            keybeam_keyoff_starts: [None; LANE_COUNT],
            keybeam_suppressed: [false; LANE_COUNT],
            keybeam_fade_allowed: [false; LANE_COUNT],
            key_logger: KeyLoggerRuntime::default(),
            judge_lane: JudgeLaneRuntime::default(),
        }
    }
}

impl DynamicTimerRuntime {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// スキン install / scene 再入場時に、timer と runtime flag を初期状態へ戻す。
    pub fn reset_for_document(&mut self, document: Option<&SkinDocument>) {
        self.reset();
        if let Some(document) = document {
            self.initialize_runtime_flags(document);
        }
    }

    /// 宣言済み runtime event を dispatch する。対象 event がなければ false。
    pub fn dispatch_runtime_event(&mut self, document: &SkinDocument, event_id: i32) -> bool {
        self.ensure_runtime_flags(document);
        let mut handled = false;
        for event in document.runtime_events.iter().filter(|event| event.id == event_id) {
            handled = true;
            for flag_id in &event.toggle_flags {
                let flag = self.runtime_flags.entry(*flag_id).or_insert(false);
                *flag = !*flag;
            }
        }
        handled
    }

    /// observe 条件を評価し、`state.dynamic_timer_ms` を更新する。
    pub fn advance(&mut self, document: &SkinDocument, state: &mut SkinDrawState, now_ms: i32) {
        self.ensure_runtime_flags(document);
        self.advance_logical_inputs(document, state.logical_input_held, now_ms);
        state.runtime_flags.clone_from(&self.runtime_flags);
        state.logical_input_press_ms =
            self.logical_input_starts.map(|start| start.map(|start| now_ms.saturating_sub(start)));
        self.advance_keybeam(state, now_ms);
        self.key_logger.write_state(state, now_ms);
        self.judge_lane.write_state(state);
        state.keylogger_exclude_cool = !document.graph.iter().any(|graph| {
            graph.id.starts_with("keylogger-graph-judge-") && graph.id.ends_with("-cool")
        });
        state.fixed_delay_timer_ms.clear();
        for def in &document.fixed_delay_timers {
            let Some(source_elapsed) = skin_timer_elapsed_ms(Some(def.source_timer), state) else {
                continue;
            };
            if source_elapsed >= def.delay_ms {
                state
                    .fixed_delay_timer_ms
                    .insert(def.id, source_elapsed.saturating_sub(def.delay_ms));
            }
        }
        for def in &document.dynamic_timers {
            let idx = def.id.saturating_sub(SKIN_DYNAMIC_TIMER_BASE) as usize;
            if idx >= SKIN_DYNAMIC_TIMER_COUNT {
                continue;
            }
            if eval_skin_draw_condition(&def.observe, state) {
                let start = self.starts[idx].get_or_insert(now_ms);
                state.dynamic_timer_ms[idx] = Some(now_ms.saturating_sub(*start));
            } else {
                self.starts[idx] = None;
                state.dynamic_timer_ms[idx] = None;
            }
        }
    }

    fn ensure_runtime_flags(&mut self, document: &SkinDocument) {
        if !self.runtime_flags_initialized {
            self.initialize_runtime_flags(document);
        }
    }

    fn advance_logical_inputs(
        &mut self,
        document: &SkinDocument,
        held: [bool; SKIN_BMZ_INPUT_COUNT],
        now_ms: i32,
    ) {
        if !self.logical_input_initialized {
            self.logical_input_initialized = true;
            self.logical_input_held = held;
            return;
        }
        for (index, &is_held) in held.iter().enumerate() {
            if is_held && !self.logical_input_held[index] {
                self.logical_input_starts[index] = Some(now_ms);
                for event in document.runtime_events.iter().filter(|event| {
                    event.trigger_action.is_some_and(|action| action.index() == index)
                }) {
                    for flag_id in &event.toggle_flags {
                        let flag = self.runtime_flags.entry(*flag_id).or_insert(false);
                        *flag = !*flag;
                    }
                }
            }
        }
        self.logical_input_held = held;
    }

    fn initialize_runtime_flags(&mut self, document: &SkinDocument) {
        self.runtime_flags =
            document.runtime_flags.iter().map(|flag| (flag.id, flag.initial)).collect();
        self.runtime_flags_initialized = true;
    }

    pub fn ingest_skin_events(
        &mut self,
        events: &[SkinRuntimeEvent],
        key_mode: KeyMode,
        now_us: i64,
    ) {
        self.key_logger.ingest(events, key_mode, now_us);
    }

    pub(crate) fn ingest_judge_lane_state(
        &mut self,
        judgements: &[crate::snapshot::DisplayJudgement],
        region_count: usize,
        now_us: i64,
    ) {
        self.judge_lane.ingest(judgements, region_count, now_us);
    }

    fn advance_keybeam(&mut self, state: &mut SkinDrawState, now_ms: i32) {
        for lane in 0..LANE_COUNT {
            let keyon_start = state.keyon_ms[lane].map(|elapsed| now_ms.saturating_sub(elapsed));
            let keyoff_start = state.keyoff_ms[lane].map(|elapsed| now_ms.saturating_sub(elapsed));

            if keyon_start.is_some() && keyon_start != self.keybeam_keyon_starts[lane] {
                self.keybeam_suppressed[lane] = false;
                self.keybeam_fade_allowed[lane] = false;
            }
            if state.hold_ms[lane].is_some() && state.keyon_ms[lane].is_some() {
                self.keybeam_suppressed[lane] = true;
            }

            state.keybeam_hold_active[lane] =
                state.keyon_ms[lane].is_some() && !self.keybeam_suppressed[lane];
            if keyoff_start.is_some() && keyoff_start != self.keybeam_keyoff_starts[lane] {
                self.keybeam_fade_allowed[lane] = !self.keybeam_suppressed[lane];
                self.keybeam_suppressed[lane] = false;
            }
            state.keybeam_fade_active[lane] =
                keyoff_start.is_some() && self.keybeam_fade_allowed[lane];
            self.keybeam_keyon_starts[lane] = keyon_start;
            self.keybeam_keyoff_starts[lane] = keyoff_start;
        }
    }
}

impl JudgeLaneRuntime {
    fn ingest(
        &mut self,
        judgements: &[crate::snapshot::DisplayJudgement],
        region_count: usize,
        now_us: i64,
    ) {
        let region_count = region_count.clamp(1, MAX_JUDGE_REGIONS);
        if self.last_now_us.is_some_and(|last| now_us < last)
            || self.region_count.is_some_and(|last| last != region_count)
        {
            *self = Self::default();
        }
        self.last_now_us = Some(now_us);
        self.region_count = Some(region_count);

        // recent_judgements は発生順なので、同時刻の同一チャンネルは後の判定を採用する。
        for judgement in judgements {
            let region = lane_judge_region(judgement.lane.index(), LANE_COUNT, region_count);
            if self.region_started_us[region].is_none_or(|last| judgement.time.0 >= last) {
                self.region_started_us[region] = Some(judgement.time.0);
                self.region_judge_index[region] =
                    Some(judge_image_index_for_judge(judgement.judge));
                self.region_combo[region] = judgement.combo;
                self.region_timing_sign[region] = judgement.side.map(|side| match side {
                    TimingSide::Fast => 1,
                    TimingSide::Slow => -1,
                });
                self.region_timing_ms[region] = (!judgement.timing_ms_suppressed).then_some(
                    (judgement.delta_us / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                );
            }
            let lane_kind = usize::from(!matches!(judgement.lane, Lane::Scratch | Lane::Scratch2));
            let slot = region * 2 + lane_kind;
            if self.started_us[slot].is_some_and(|last| judgement.time.0 < last) {
                continue;
            }
            self.started_us[slot] = Some(judgement.time.0);
            self.judge_index[slot] = Some(judge_image_index_for_judge(judgement.judge));
            self.timing_sign[slot] = judgement.side.map(|side| match side {
                TimingSide::Fast => 1,
                TimingSide::Slow => -1,
            });
            self.timing_ms[slot] = (!judgement.timing_ms_suppressed).then_some(
                (judgement.delta_us / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            );
        }
    }

    fn write_state(&self, state: &mut SkinDrawState) {
        let Some(now_us) = self.last_now_us else {
            return;
        };
        state.judge_ms = self.region_started_us.map(|started| {
            started.map(|started| {
                (now_us.saturating_sub(started) / 1_000).clamp(i32::MIN as i64, i32::MAX as i64)
                    as i32
            })
        });
        state.judge_index = self.region_judge_index;
        state.judge_combo = self.region_combo;
        state.judge_timing_sign = self.region_timing_sign;
        state.judge_timing_ms = self.region_timing_ms;
        state.judge_lane_ms = self.started_us.map(|started| {
            started.map(|started| {
                (now_us.saturating_sub(started) / 1_000).clamp(i32::MIN as i64, i32::MAX as i64)
                    as i32
            })
        });
        state.judge_lane_index = self.judge_index;
        state.judge_lane_timing_sign = self.timing_sign;
        state.judge_lane_timing_ms = self.timing_ms;
    }
}

impl KeyLoggerRuntime {
    pub(in crate::skin) fn ingest(
        &mut self,
        events: &[SkinRuntimeEvent],
        key_mode: KeyMode,
        now_us: i64,
    ) {
        if self.last_now_us.is_some_and(|last| now_us < last) {
            *self = Self::default();
        }
        self.last_now_us = Some(now_us);
        let active_lanes = key_mode.active_lanes();
        for event in events {
            if self.last_sequence.is_some_and(|last| event.sequence <= last) {
                continue;
            }
            self.last_sequence = Some(event.sequence);
            match &event.kind {
                SkinRuntimeEventKind::Input(input) if input.kind == InputKind::Press => {
                    let Some(lane) =
                        active_lanes.iter().position(|candidate| *candidate == input.lane)
                    else {
                        continue;
                    };
                    self.press_history_us.push_back(input.time.0);
                    let slot = self.next_event_slot[lane];
                    self.event_started_ms[lane][slot] =
                        Some((input.time.0 / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32);
                    self.event_started_us[lane][slot] = Some(input.time.0);
                    self.event_judge[lane][slot] = 0;
                    self.event_fast_slow[lane][slot] = 0;
                    self.next_event_slot[lane] = (slot + 1) % 16;
                }
                SkinRuntimeEventKind::Judgement(judgement) => {
                    let Some(lane) =
                        active_lanes.iter().position(|candidate| *candidate == judgement.lane)
                    else {
                        continue;
                    };
                    let judge = match judgement.judge {
                        Judge::PGreat => 0,
                        Judge::Great => 1,
                        Judge::Good => 2,
                        Judge::Bad | Judge::Poor | Judge::EmptyPoor => 3,
                    };
                    self.judge_counts[lane][judge] =
                        self.judge_counts[lane][judge].saturating_add(1);
                    let side = match judgement.judge {
                        Judge::PGreat => Some(0),
                        _ => match judgement.side {
                            TimingSide::Fast => Some(1),
                            TimingSide::Slow => Some(2),
                        },
                    };
                    if let Some(side) = side {
                        self.fast_slow_counts[lane][side] =
                            self.fast_slow_counts[lane][side].saturating_add(1);
                    }
                    let slot = (self.next_event_slot[lane] + 15) % 16;
                    if self.event_started_us[lane][slot] == Some(judgement.time.0) {
                        self.event_judge[lane][slot] = (judge + 1) as u8;
                        self.event_fast_slow[lane][slot] = side.map_or(0, |side| (side + 1) as u8);
                    }
                }
                _ => {}
            }
        }
        let keep_from = now_us.saturating_sub(1_000_000);
        while self.press_history_us.front().is_some_and(|time| *time < keep_from) {
            self.press_history_us.pop_front();
        }
    }

    pub(in crate::skin) fn write_state(&self, state: &mut SkinDrawState, now_ms: i32) {
        state.keylogger_nps = self.press_history_us.len().min(999) as u32;
        state.keylogger_judge_counts = self.judge_counts;
        state.keylogger_fast_slow_counts = self.fast_slow_counts;
        state.keylogger_event_judge = self.event_judge;
        state.keylogger_event_fast_slow = self.event_fast_slow;
        for lane in 0..LANE_COUNT {
            for slot in 0..16 {
                state.keylogger_event_ms[lane][slot] =
                    self.event_started_ms[lane][slot].map(|started| now_ms.saturating_sub(started));
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::skin) struct SkinRuntimeGraphs<'a> {
    pub(in crate::skin) play_judge_graph_density: &'a [u8],
    pub(in crate::skin) play_bpm_graph_segments: &'a [crate::chart_graph::BpmGraphSegment],
    pub(in crate::skin) result_gauge_graph_points: &'a [crate::snapshot::ResultGaugeGraphPoint],
    pub(in crate::skin) result_timing_points: &'a [crate::snapshot::ResultTimingPoint],
    pub(in crate::skin) result_judge_graph_buckets: &'a [crate::snapshot::ResultJudgeGraphBucket],
    pub(in crate::skin) result_note_graph_buckets: &'a [crate::snapshot::ResultNoteGraphBucket],
    pub(in crate::skin) result_early_late_graph_buckets:
        &'a [crate::snapshot::ResultEarlyLateGraphBucket],
    pub(in crate::skin) result_timing_distribution: &'a crate::snapshot::ResultTimingDistribution,
}

impl<'a> SkinRuntimeGraphs<'a> {
    pub(in crate::skin) fn from_document(document: &'a SkinDocument) -> Self {
        Self {
            play_judge_graph_density: &document.play_judge_graph_density,
            play_bpm_graph_segments: &document.play_bpm_graph_segments,
            result_gauge_graph_points: &document.result_gauge_graph_points,
            result_timing_points: &document.result_timing_points,
            result_judge_graph_buckets: &document.result_judge_graph_buckets,
            result_note_graph_buckets: &document.result_note_graph_buckets,
            result_early_late_graph_buckets: &document.result_early_late_graph_buckets,
            result_timing_distribution: &document.result_timing_distribution,
        }
    }

    pub(in crate::skin) fn from_document_with_play_graphs(
        document: &'a SkinDocument,
        play_judge_graph_density: &'a [u8],
        play_bpm_graph_segments: &'a [crate::chart_graph::BpmGraphSegment],
    ) -> Self {
        Self {
            play_judge_graph_density,
            play_bpm_graph_segments,
            result_gauge_graph_points: &document.result_gauge_graph_points,
            result_timing_points: &document.result_timing_points,
            result_judge_graph_buckets: &document.result_judge_graph_buckets,
            result_note_graph_buckets: &document.result_note_graph_buckets,
            result_early_late_graph_buckets: &document.result_early_late_graph_buckets,
            result_timing_distribution: &document.result_timing_distribution,
        }
    }

    pub(in crate::skin) fn from_result_graph(
        graph: &'a crate::snapshot::ResultGraphSnapshot,
    ) -> Self {
        Self {
            play_judge_graph_density: &graph.judge_graph_density,
            play_bpm_graph_segments: &graph.bpm_graph_segments,
            result_gauge_graph_points: &graph.gauge_points,
            result_timing_points: &graph.timing_points,
            result_judge_graph_buckets: &graph.judge_graph_buckets,
            result_note_graph_buckets: &graph.note_graph_buckets,
            result_early_late_graph_buckets: &graph.early_late_graph_buckets,
            result_timing_distribution: &graph.timing_distribution,
        }
    }
}

pub(in crate::skin) struct DestinationResolveContext<'a, 'text> {
    pub(in crate::skin) images: &'a HashMap<&'a str, &'a SkinImageDef>,
    pub(in crate::skin) values: &'a HashMap<&'a str, &'a SkinValueDef>,
    pub(in crate::skin) enabled_options: &'a [i32],
    pub(in crate::skin) state: &'a SkinDrawState,
    pub(in crate::skin) text_state: &'a SkinTextState<'text>,
    pub(in crate::skin) sources: &'a HashMap<String, SkinDocumentTexture>,
    pub(in crate::skin) runtime_graphs: SkinRuntimeGraphs<'a>,
    pub(in crate::skin) has_nearest_f_diff_rank_destination: bool,
    pub(in crate::skin) cache: Option<&'a mut ResultRenderCache>,
}

/// beatoraja `PlaySkin.judgeregion` 上限 (TIMER_JUDGE_1P/2P/3P = 46/47/247)。
pub const MAX_JUDGE_REGIONS: usize = 3;
pub(in crate::skin) const LUA_DRAW_CALLBACK_PREFIX: &str = "bmz:lua_draw_callback:";
