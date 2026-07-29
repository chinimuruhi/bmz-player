use super::*;

pub(super) fn result_skin_click_action(event_id: i32) -> Option<ResultSkinClickAction> {
    match event_id {
        SKIN_EVENT_RESULT_PANEL_IR => Some(ResultSkinClickAction::SetPanel(1)),
        SKIN_EVENT_RESULT_PANEL_GRAPH => Some(ResultSkinClickAction::SetPanel(2)),
        SKIN_EVENT_IR_SCOPE_GLOBAL => Some(ResultSkinClickAction::SelectIrScope(
            crate::screens::result_ir::ResultRankingTab::Global,
        )),
        SKIN_EVENT_IR_SCOPE_RIVAL => Some(ResultSkinClickAction::SelectIrScope(
            crate::screens::result_ir::ResultRankingTab::SelfAndRivals,
        )),
        SKIN_EVENT_IR_SCOPE_TOGGLE => Some(ResultSkinClickAction::ToggleIrScope),
        SKIN_EVENT_DAILY_STATISTICS_RESET => Some(ResultSkinClickAction::ResetDailyStatistics),
        90 => Some(ResultSkinClickAction::ToggleFavoriteChart),
        19 => Some(ResultSkinClickAction::SaveReplay(0)),
        316..=318 => Some(ResultSkinClickAction::SaveReplay((event_id - 315) as u8)),
        _ => None,
    }
}

/// コース曲間の中間リザルトかどうか。active_course を保持したまま finished_play
/// だけが立ち、finished_course はまだ無い状態を指す。中間リザルトでは retry を
/// 無効化し、次の曲へ進むだけにする (beatoraja MusicResult のコース分岐相当)。
pub(super) fn is_course_intermediate_result(
    active_course: bool,
    finished_course: bool,
    finished_play: bool,
) -> bool {
    active_course && finished_play && !finished_course
}

pub(super) fn toggled_result_panel(
    current: i32,
    supported: bool,
    ir_available: bool,
) -> Option<i32> {
    if !ir_available {
        return None;
    }
    let requested = match current {
        1 => 2,
        2 => 1,
        _ => return None,
    };
    selected_result_panel(current, requested, supported, ir_available)
}

pub(super) fn selected_result_panel(
    current: i32,
    requested: i32,
    supported: bool,
    ir_available: bool,
) -> Option<i32> {
    if !supported || current == requested {
        return None;
    }
    match requested {
        1 if ir_available => Some(1),
        2 if matches!(current, 1 | 2) => Some(2),
        _ => None,
    }
}

pub(super) fn result_failed_for_skin_ops(
    display_clear_type: ClearType,
    raw_clear_type: Option<ClearType>,
) -> bool {
    matches!(raw_clear_type.unwrap_or(display_clear_type), ClearType::Failed | ClearType::NoPlay)
}

pub(super) fn course_intermediate_exit_action_for_state(
    failed: bool,
    has_next_chart: bool,
) -> ResultExitAction {
    if failed || !has_next_chart {
        ResultExitAction::FinishCourse
    } else {
        ResultExitAction::AdvanceCourse
    }
}

pub(super) fn should_show_course_stage_result(
    failed: bool,
    has_next_entry: bool,
    has_next_chart: bool,
) -> bool {
    failed || has_next_chart || !has_next_entry
}

pub(super) fn result_skin_signature_for_config(
    skin: &crate::config::profile_config::SkinConfig,
    slot: ResultSkinSlot,
    mut runtime_state: bmz_skin::LuaLoadRuntimeState,
) -> ResultSkinSignature {
    runtime_state.offset_values.clear();
    runtime_state.offset_id_values.clear();
    match slot {
        ResultSkinSlot::Normal => {
            apply_skin_offsets_to_lua_runtime_state(&mut runtime_state, &skin.result_offsets);
            (
                slot,
                skin.result.trim().to_string(),
                skin.result_options.clone(),
                skin.result_files.clone(),
                runtime_state,
            )
        }
        ResultSkinSlot::Course => {
            apply_skin_offsets_to_lua_runtime_state(
                &mut runtime_state,
                &skin.course_result_offsets,
            );
            (
                slot,
                skin.course_result.trim().to_string(),
                skin.course_result_options.clone(),
                skin.course_result_files.clone(),
                runtime_state,
            )
        }
    }
}

pub(super) fn result_lua_runtime_number_values_for_summary(
    summary: &ResultSummary,
) -> BTreeMap<i32, i32> {
    let mut number_values = BTreeMap::new();
    let value = |value: u32| i32::try_from(value).unwrap_or(i32::MAX);
    let difference = |current: u32, previous: u32| value(current).saturating_sub(value(previous));
    number_values.insert(71, value(summary.ex_score));
    number_values.insert(74, value(summary.total_notes));
    number_values.insert(101, value(summary.ex_score));
    number_values.insert(171, value(summary.ex_score));
    number_values.insert(107, summary.gauge_value.floor().clamp(0.0, i32::MAX as f32) as i32);
    number_values.insert(110, value(summary.judge_counts.pgreat));
    number_values.insert(111, value(summary.judge_counts.great));
    number_values.insert(112, value(summary.judge_counts.good));
    number_values.insert(113, value(summary.judge_counts.bad));
    number_values.insert(114, value(summary.judge_counts.poor));
    let previous_best_ex_score = summary.previous_best_ex_score.unwrap_or(0);
    number_values.insert(150, value(previous_best_ex_score));
    number_values.insert(170, value(previous_best_ex_score));
    number_values.insert(152, difference(summary.ex_score, previous_best_ex_score));
    number_values.insert(172, difference(summary.ex_score, previous_best_ex_score));
    if let Some(target_ex_score) = summary.target_ex_score {
        number_values.insert(121, value(target_ex_score));
        number_values.insert(151, value(target_ex_score));
        number_values.insert(153, difference(summary.ex_score, target_ex_score));
    }
    number_values.insert(177, value(summary.bp));
    let counts = summary.fast_slow_counts;
    number_values.insert(410, value(counts.fast_pgreat));
    number_values.insert(411, value(counts.slow_pgreat));
    number_values.insert(412, value(counts.fast_great));
    number_values.insert(413, value(counts.slow_great));
    number_values.insert(414, value(counts.fast_good));
    number_values.insert(415, value(counts.slow_good));
    number_values.insert(416, value(counts.fast_bad));
    number_values.insert(417, value(counts.slow_bad));
    number_values.insert(418, value(counts.fast_poor));
    number_values.insert(419, value(counts.slow_poor));
    number_values.insert(420, value(summary.judge_counts.empty_poor));
    number_values.insert(421, value(counts.fast_empty_poor));
    number_values.insert(422, value(counts.slow_empty_poor));
    number_values.insert(
        423,
        value(
            counts
                .fast_great
                .saturating_add(counts.fast_good)
                .saturating_add(counts.fast_bad)
                .saturating_add(counts.fast_poor)
                .saturating_add(counts.fast_empty_poor),
        ),
    );
    number_values.insert(
        424,
        value(
            counts
                .slow_great
                .saturating_add(counts.slow_good)
                .saturating_add(counts.slow_bad)
                .saturating_add(counts.slow_poor)
                .saturating_add(counts.slow_empty_poor),
        ),
    );
    number_values.insert(425, i32::try_from(summary.cb).unwrap_or(i32::MAX));
    number_values.insert(
        426,
        value(summary.judge_counts.poor.saturating_add(summary.judge_counts.empty_poor)),
    );
    number_values.insert(
        427,
        value(
            summary
                .judge_counts
                .bad
                .saturating_add(summary.judge_counts.poor)
                .saturating_add(summary.judge_counts.empty_poor),
        ),
    );
    number_values.insert(370, summary.clear_type as i32);
    number_values.insert(371, summary.previous_best_clear_type.unwrap_or(ClearType::NoPlay) as i32);
    if let Some((average_timing_ms, _)) = summary.graph.timing_distribution.stats() {
        number_values.insert(374, average_timing_ms as i32);
        number_values.insert(375, (average_timing_ms * 100.0) as i32 % 100);
    }
    if let Some(previous_best_bp) = summary.previous_best_bp
        && let (Ok(current), Ok(previous)) =
            (i32::try_from(summary.bp), i32::try_from(previous_best_bp))
    {
        number_values.insert(178, current.saturating_sub(previous));
    }
    number_values
}

pub(super) fn apply_course_mode_lua_options(
    runtime_state: &mut bmz_skin::LuaLoadRuntimeState,
    stage: Option<CourseStageMarker>,
) {
    runtime_state.option_values.insert(290, true);
    for option in [280, 281, 282, 283, 289] {
        runtime_state.option_values.insert(option, false);
    }
    let option = match stage {
        Some(CourseStageMarker::Stage1) => Some(280),
        Some(CourseStageMarker::Stage2) => Some(281),
        Some(CourseStageMarker::Stage3) => Some(282),
        Some(CourseStageMarker::Stage4) => Some(283),
        Some(CourseStageMarker::Final) => Some(289),
        None => None,
    };
    if let Some(option) = option {
        runtime_state.option_values.insert(option, true);
    }
}

pub(super) fn apply_course_result_lua_load_state(
    runtime_state: &mut bmz_skin::LuaLoadRuntimeState,
    course: &CourseResultSummary,
) {
    for (index, title) in course.course_titles.iter().enumerate() {
        runtime_state.text_values.insert(150 + index as i32, title.clone());
    }
    let course_result = course_result_skin_snapshot(course);
    runtime_state.number_values.insert(
        bmz_render::skin::SKIN_REF_BMZ_COURSE_STAGE_COUNT,
        course_result.stage_count as i32,
    );
    for (index, stage) in course_result.stages.iter().enumerate() {
        let index = index as i32;
        runtime_state.number_values.insert(
            bmz_render::skin::SKIN_REF_BMZ_COURSE_STAGE_EX_BASE + index,
            i32::try_from(stage.ex_score).unwrap_or(i32::MAX),
        );
        runtime_state.number_values.insert(
            bmz_render::skin::SKIN_REF_BMZ_COURSE_STAGE_GAUGE_BASE + index,
            stage.gauge.floor() as i32,
        );
        runtime_state.number_values.insert(
            bmz_render::skin::SKIN_REF_BMZ_COURSE_STAGE_BP_BASE + index,
            i32::try_from(stage.bp).unwrap_or(i32::MAX),
        );
        runtime_state.number_values.insert(
            bmz_render::skin::SKIN_REF_BMZ_COURSE_STAGE_RATE_BASE + index,
            i32::try_from(stage.rate_basis_points).unwrap_or(i32::MAX),
        );
    }

    // WMII stores these values from each intermediate MusicResult in
    // `skin/WMII_FHD/result/courseData.json`, then reads them from CourseResult.
    // Lua filesystem writes are intentionally disabled in BMZ, so expose the
    // equivalent attempt data as an app-owned, read-only virtual file.
    let songs = course
        .entry_summaries
        .iter()
        .enumerate()
        .map(|(index, summary)| {
            let max_ex_score = summary.total_notes.saturating_mul(2);
            let rate = if max_ex_score > 0 {
                f64::from(summary.ex_score) / f64::from(max_ex_score)
            } else {
                0.0
            };
            serde_json::json!({
                "stage": index + 1,
                "score": summary.ex_score,
                "gauge": summary.gauge_value.floor(),
                "miss": summary.bp,
                "rate": rate,
            })
        })
        .collect::<Vec<_>>();
    let course_data = serde_json::json!({ "songs": songs }).to_string();
    runtime_state
        .virtual_io_files
        .insert("skin/WMII_FHD/result/courseData.json".to_string(), course_data);
}

pub(super) fn apply_result_summary_lua_load_state(
    runtime_state: &mut bmz_skin::LuaLoadRuntimeState,
    summary: &ResultSummary,
    table_primary: &str,
    table_level: &str,
    table_full: &str,
) {
    let full_title = if summary.subtitle.is_empty() {
        summary.title.clone()
    } else {
        format!("{} {}", summary.title, summary.subtitle)
    };
    let full_artist = if summary.subartist.is_empty() {
        summary.artist.clone()
    } else {
        format!("{} {}", summary.artist, summary.subartist)
    };
    runtime_state.text_values.extend([
        (1, summary.target_name.clone()),
        (3, summary.target_name.clone()),
        (10, summary.title.clone()),
        (11, summary.subtitle.clone()),
        (12, full_title),
        (13, summary.genre.clone()),
        (14, summary.artist.clone()),
        (15, summary.subartist.clone()),
        (16, full_artist),
        (1001, table_primary.to_string()),
        (1002, table_level.to_string()),
        (1003, table_full.to_string()),
    ]);
    for option in 180..=184 {
        runtime_state.option_values.insert(option, false);
    }
    if let Some(option) = result_judge_rank_option_id(summary.judge_rank) {
        runtime_state.option_values.insert(option, true);
    }
    runtime_state.event_index_values.insert(
        308,
        i32::try_from(result_long_note_mode_index(summary.long_note_mode)).unwrap_or_default(),
    );
    runtime_state.event_index_values.insert(
        42,
        i32::try_from(bmz_render::skin::select_arrange_index(&summary.arrange)).unwrap_or_default(),
    );
    runtime_state.event_index_values.insert(
        43,
        i32::try_from(bmz_render::skin::select_arrange_index(&summary.arrange_2p))
            .unwrap_or_default(),
    );
    runtime_state.event_index_values.insert(
        344,
        i32::try_from(bmz_render::skin::extended_arrange_index(&summary.arrange))
            .unwrap_or_default(),
    );
    runtime_state.event_index_values.insert(
        345,
        i32::try_from(bmz_render::skin::extended_arrange_index(&summary.arrange_2p))
            .unwrap_or_default(),
    );
}

pub(super) fn result_judge_rank_option_id(judge_rank: Option<i32>) -> Option<i32> {
    let Some(rank) = judge_rank else {
        return Some(182);
    };
    match rank {
        0 | 10..=34 => Some(180),
        1 | 35..=59 => Some(181),
        2 | 60..=84 => Some(182),
        3 | 85..=109 => Some(183),
        4 | 110.. => Some(184),
        _ => None,
    }
}

pub(super) fn result_long_note_mode_index(mode: bmz_chart::model::LongNoteMode) -> usize {
    match mode {
        bmz_chart::model::LongNoteMode::Ln => 0,
        bmz_chart::model::LongNoteMode::Cn => 1,
        bmz_chart::model::LongNoteMode::Hcn => 2,
    }
}

pub(super) fn result_ir_skin_name(
    ir_config: &crate::config::profile_config::IrConfig,
) -> Option<&str> {
    let provider = crate::ir::provider_key::primary_provider_config(ir_config)?;
    crate::ir::provider_key::configured_provider_display_name(provider)
}

pub(super) fn lua_runtime_state_for_result(
    table_song: bool,
    ir_name: Option<&str>,
    score_save_enabled: bool,
    key_mode: KeyMode,
    mut number_values: BTreeMap<i32, i32>,
    player_name: &str,
) -> bmz_skin::LuaLoadRuntimeState {
    let ir_online = ir_name.is_some();
    let mut option_values = BTreeMap::new();
    option_values.insert(1008, table_song);
    option_values.insert(50, !ir_online);
    option_values.insert(51, ir_online);
    option_values.insert(60, !score_save_enabled);
    option_values.insert(61, score_save_enabled);
    for option in 160..=164 {
        option_values.insert(option, result_key_mode_option_matches(option, key_mode));
    }
    extend_bmz_key_mode_lua_state(&mut number_values, &mut option_values, key_mode);
    bmz_skin::LuaLoadRuntimeState {
        number_values,
        text_values: BTreeMap::from([
            (2, player_name.to_string()),
            (1020, ir_name.unwrap_or_default().to_string()),
        ]),
        option_values,
        ..Default::default()
    }
}

pub(super) fn lua_runtime_state_for_play(
    options: &PlayStartOptions,
    profile_autoplay: bool,
    key_mode: KeyMode,
    player_name: &str,
) -> bmz_skin::LuaLoadRuntimeState {
    let replay_playback = options.replay_player.is_some();
    let autoplay = !replay_playback && (profile_autoplay || options.autoplay);
    let score_save_enabled = !autoplay && !replay_playback && !options.practice_mode;
    let mut option_values = BTreeMap::from([
        (32, !autoplay),
        (33, autoplay),
        (60, !score_save_enabled),
        (61, score_save_enabled),
        (82, !autoplay && !replay_playback),
        (84, replay_playback),
        (1080, options.practice_mode),
    ]);
    let mut number_values = BTreeMap::new();
    extend_bmz_key_mode_lua_state(&mut number_values, &mut option_values, key_mode);
    bmz_skin::LuaLoadRuntimeState {
        number_values,
        text_values: BTreeMap::from([(2, player_name.to_string())]),
        option_values,
        ..Default::default()
    }
}

pub(super) fn lua_runtime_state_for_player(player_name: &str) -> bmz_skin::LuaLoadRuntimeState {
    bmz_skin::LuaLoadRuntimeState {
        text_values: BTreeMap::from([(2, player_name.to_string())]),
        ..bmz_skin::LuaLoadRuntimeState::default()
    }
}

pub(super) fn result_key_mode_option_matches(option: i32, key_mode: KeyMode) -> bool {
    match option {
        160 => matches!(key_mode, KeyMode::K7 | KeyMode::K8),
        161 => key_mode == KeyMode::K5,
        162 => key_mode == KeyMode::K14,
        163 => key_mode == KeyMode::K10,
        164 => key_mode == KeyMode::K9,
        _ => false,
    }
}

pub(super) fn extend_bmz_key_mode_lua_state(
    number_values: &mut BTreeMap<i32, i32>,
    option_values: &mut BTreeMap<i32, bool>,
    key_mode: KeyMode,
) {
    number_values.insert(SKIN_REF_BMZ_KEY_MODE, bmz_key_mode_number(key_mode));
    number_values.insert(SKIN_REF_BMZ_ACTIVE_LANE_COUNT, key_mode.lane_count() as i32);
    for option in SKIN_OPTION_BMZ_KEY_MODE_BASE
        ..SKIN_OPTION_BMZ_KEY_MODE_BASE + SKIN_OPTION_BMZ_KEY_MODE_COUNT as i32
    {
        option_values.insert(option, bmz_key_mode_option_matches(option, key_mode));
    }
    for option in
        [SKIN_OPTION_BMZ_NO_SCRATCH, SKIN_OPTION_BMZ_SINGLE_PLAY, SKIN_OPTION_BMZ_DOUBLE_PLAY]
    {
        option_values.insert(option, bmz_key_mode_option_matches(option, key_mode));
    }
}

pub(super) fn bmz_key_mode_number(key_mode: KeyMode) -> i32 {
    match key_mode {
        KeyMode::K4 => 4,
        KeyMode::K5 => 5,
        KeyMode::K6 => 6,
        KeyMode::K7 => 7,
        KeyMode::K8 => 8,
        KeyMode::K9 => 9,
        KeyMode::K10 => 10,
        KeyMode::K14 => 14,
    }
}

pub(super) fn bmz_key_mode_option_matches(option: i32, key_mode: KeyMode) -> bool {
    match option - SKIN_OPTION_BMZ_KEY_MODE_BASE {
        0 => key_mode == KeyMode::K4,
        1 => key_mode == KeyMode::K5,
        2 => key_mode == KeyMode::K6,
        3 => key_mode == KeyMode::K7,
        4 => key_mode == KeyMode::K8,
        5 => key_mode == KeyMode::K9,
        6 => key_mode == KeyMode::K10,
        7 => key_mode == KeyMode::K14,
        _ if option == SKIN_OPTION_BMZ_NO_SCRATCH => {
            matches!(key_mode, KeyMode::K4 | KeyMode::K6 | KeyMode::K8 | KeyMode::K9)
        }
        _ if option == SKIN_OPTION_BMZ_SINGLE_PLAY => matches!(key_mode, KeyMode::K5 | KeyMode::K7),
        _ if option == SKIN_OPTION_BMZ_DOUBLE_PLAY => {
            matches!(key_mode, KeyMode::K10 | KeyMode::K14)
        }
        _ => false,
    }
}

/// リザルト画面で押すと終了アニメーションを開始するレーン。
/// BMZ では Key1/3/5/7 を「次へ進む」、Key2/4/6 を「戻る/変更」系に寄せるため、
/// beatoraja と異なり Key2 は終了開始に使わない。
/// Key6 は CHANGE_GRAPH、scratch は無割り当てなので開始しない。
pub(super) fn lane_starts_result_exit(lane: Lane) -> bool {
    matches!(lane, Lane::Key1 | Lane::Key3 | Lane::Key4 | Lane::Key5 | Lane::Key7)
}

pub(super) fn lane_skips_result_exit(lane: Lane) -> bool {
    matches!(lane, Lane::Key1 | Lane::Key3 | Lane::Key8 | Lane::Key10 | Lane::Key12 | Lane::Key14)
}

pub(super) fn retry_preload_kind(
    mode: ResultRetryMode,
    cached_chart_available: bool,
) -> RetryPreloadKind {
    match mode {
        ResultRetryMode::SameArrange if cached_chart_available => {
            RetryPreloadKind::CachedChartWithFreshAudio
        }
        ResultRetryMode::SameArrange | ResultRetryMode::DifferentArrange => {
            RetryPreloadKind::ReimportedChartWithFreshAudio
        }
    }
}

/// フェードアウト終了時の Key5/Key7 押下状態から遷移を決める。
/// beatoraja 準拠: Key5=別配置 (REPLAY_DIFFERENT)、Key7=同配置 (REPLAY_SAME)。
/// - Key7 押下 (両押し含む) → 同配置 (SameArrange)
/// - Key5 のみ押下 → 別配置 (DifferentArrange)
/// - どちらも非押下 → None (選曲へ戻る)
///
/// beatoraja は両押し時に index の若い Key5 (DIFFERENT) を優先するが、
/// 本実装はユーザー仕様として両押しを SameArrange とする。
pub(super) fn result_action_for_held_lanes(
    key5_held: bool,
    key7_held: bool,
) -> Option<ResultRetryMode> {
    match (key5_held, key7_held) {
        (_, true) => Some(ResultRetryMode::SameArrange),
        (true, false) => Some(ResultRetryMode::DifferentArrange),
        (false, false) => None,
    }
}

pub(super) fn skin_duration_ms(ms: i32) -> Duration {
    Duration::from_millis(ms.max(0) as u64)
}

pub(super) fn result_input_duration_for_document(document: Option<&SkinDocument>) -> Duration {
    document.map(|document| skin_duration_ms(document.input)).unwrap_or_default()
}

pub(super) fn result_panel_supported(document: &SkinDocument) -> bool {
    document.result_panel_default.is_some()
        && document
            .destination
            .iter()
            .flat_map(destination_entry_values)
            .any(|destination| destination.draw.contains("result_panel("))
}

#[cfg(test)]
pub(super) fn result_scene_duration_for_document(document: Option<&SkinDocument>) -> Duration {
    document
        .map(|document| skin_duration_ms(document.scene))
        .unwrap_or(FALLBACK_RESULT_SCENE_DURATION)
}

pub(super) fn result_auto_exit_duration_for_document(
    document: Option<&SkinDocument>,
    is_course_intermediate: bool,
    course_intermediate_auto_advance: bool,
) -> Option<Duration> {
    if is_course_intermediate {
        if !course_intermediate_auto_advance {
            return None;
        }
        return Some(
            document
                .and_then(|document| (document.scene > 0).then(|| skin_duration_ms(document.scene)))
                .unwrap_or(FALLBACK_RESULT_SCENE_DURATION),
        );
    }

    match document {
        Some(document) if document.scene > 0 => Some(skin_duration_ms(document.scene)),
        Some(_) => None,
        None => Some(FALLBACK_RESULT_SCENE_DURATION),
    }
}

pub(super) fn decide_fadeout_scene_elapsed(
    fadeout_started_elapsed: Duration,
    fadeout_elapsed: Duration,
    scene_duration: Duration,
    fadeout_duration: Duration,
    timing: DecideFadeoutSceneTiming,
) -> Duration {
    let direct_elapsed = fadeout_started_elapsed.saturating_add(fadeout_elapsed);
    let tail_elapsed = match timing {
        DecideFadeoutSceneTiming::DirectOnly => direct_elapsed,
        DecideFadeoutSceneTiming::TailStart(tail_start) if fadeout_duration > Duration::ZERO => {
            let tail_start = tail_start.min(scene_duration);
            let tail_duration = scene_duration.saturating_sub(tail_start);
            if tail_duration > Duration::ZERO {
                let scaled = scale_duration(
                    fadeout_elapsed.min(fadeout_duration),
                    tail_duration,
                    fadeout_duration,
                );
                tail_start.saturating_add(scaled).min(scene_duration)
            } else {
                scene_duration
            }
        }
        DecideFadeoutSceneTiming::TailStart(_) => scene_duration,
        DecideFadeoutSceneTiming::DefaultTail => {
            let tail_start = scene_duration.checked_sub(fadeout_duration).unwrap_or_default();
            tail_start.saturating_add(fadeout_elapsed).min(scene_duration)
        }
    };
    direct_elapsed.max(tail_elapsed)
}

pub(super) fn scale_duration(
    value: Duration,
    numerator: Duration,
    denominator: Duration,
) -> Duration {
    if denominator == Duration::ZERO {
        return Duration::ZERO;
    }
    let micros = value
        .as_micros()
        .saturating_mul(numerator.as_micros())
        .checked_div(denominator.as_micros())
        .unwrap_or(0);
    Duration::from_micros(micros.min(u64::MAX as u128) as u64)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DecideFadeoutSceneTiming {
    /// `timer=2` が fadeout を担う skin。scene 時刻を終端へ飛ばすと
    /// timer なしの終了演出まで同時に進み、暗転が即飽和する。
    DirectOnly,
    /// timer=2 が無い skin 向け。従来通り fadeout 中は scene 末尾へ寄せる。
    DefaultTail,
    /// m-select のように scene 末尾の黒フェードを fadeout として使う skin。
    TailStart(Duration),
}

pub(super) fn decide_fadeout_scene_timing(
    document: Option<&SkinDocument>,
) -> DecideFadeoutSceneTiming {
    let Some(document) = document else {
        return DecideFadeoutSceneTiming::DefaultTail;
    };
    if document_has_fadeout_timer_black(document) {
        return DecideFadeoutSceneTiming::DirectOnly;
    }
    decide_scene_fadeout_tail_start(Some(document))
        .map(skin_duration_ms)
        .map_or(DecideFadeoutSceneTiming::DefaultTail, DecideFadeoutSceneTiming::TailStart)
}

pub(super) fn decide_scene_fadeout_tail_start(document: Option<&SkinDocument>) -> Option<i32> {
    let document = document?;
    if document.scene <= 0 || document.w == 0 || document.h == 0 {
        return None;
    }
    if document_has_fadeout_timer_black(document) {
        return None;
    }
    document
        .destination
        .iter()
        .flat_map(destination_entry_values)
        .filter_map(|destination| {
            if destination.id != "-110" || destination.timer.is_some() {
                return None;
            }
            scene_black_fade_tail_start(destination.dst.iter().flat_map(dst_entry_frames), document)
        })
        .max()
}

pub(super) fn document_has_fadeout_timer_black(document: &SkinDocument) -> bool {
    document.destination.iter().flat_map(destination_entry_values).any(|destination| {
        destination.id == "-110"
            && destination.timer == Some(2)
            && black_fade_start(destination.dst.iter().flat_map(dst_entry_frames), document, 0)
                .is_some()
    })
}

pub(super) fn destination_entry_values(
    entry: &DestinationListEntry,
) -> &[bmz_render::skin::SkinDestinationDef] {
    match entry {
        DestinationListEntry::Single(destination) => std::slice::from_ref(destination),
        DestinationListEntry::Conditional { destinations, .. } => destinations.as_slice(),
    }
}

pub(super) fn dst_entry_frames(entry: &SkinDstEntry) -> &[SkinAnimationDef] {
    match entry {
        SkinDstEntry::Frame(frame) => std::slice::from_ref(frame),
        SkinDstEntry::Conditional { frames, .. } => frames.as_slice(),
    }
}

pub(super) fn scene_black_fade_tail_start<'a>(
    frames: impl Iterator<Item = &'a SkinAnimationDef>,
    document: &SkinDocument,
) -> Option<i32> {
    black_fade_start(frames, document, document.scene)
}

pub(super) fn black_fade_start<'a>(
    frames: impl Iterator<Item = &'a SkinAnimationDef>,
    document: &SkinDocument,
    min_end_time: i32,
) -> Option<i32> {
    let mut resolved = ResolvedTailFrame::default();
    let mut previous: Option<ResolvedTailFrame> = None;
    let mut start = None;
    for frame in frames {
        resolved.apply(frame);
        let Some(previous_frame) = previous else {
            previous = Some(resolved);
            continue;
        };
        if resolved.time >= min_end_time
            && previous_frame.time < resolved.time
            && previous_frame.a < resolved.a
            && previous_frame.is_fullscreen(document)
        {
            start = Some(previous_frame.time);
        }
        previous = Some(resolved);
    }
    start
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ResolvedTailFrame {
    time: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    a: i32,
}

impl Default for ResolvedTailFrame {
    fn default() -> Self {
        Self { time: 0, x: 0, y: 0, w: 0, h: 0, a: 255 }
    }
}

impl ResolvedTailFrame {
    fn apply(&mut self, frame: &SkinAnimationDef) {
        if let Some(time) = frame.time {
            self.time = time;
        }
        if let Some(x) = frame.x {
            self.x = x;
        }
        if let Some(y) = frame.y {
            self.y = y;
        }
        if let Some(w) = frame.w {
            self.w = w;
        }
        if let Some(h) = frame.h {
            self.h = h;
        }
        if let Some(a) = frame.a {
            self.a = a;
        }
    }

    fn is_fullscreen(self, document: &SkinDocument) -> bool {
        let width = document.w as i32;
        let height = document.h as i32;
        self.x <= width / 20
            && self.y <= height / 20
            && self.w >= width * 9 / 10
            && self.h >= height * 9 / 10
    }
}
