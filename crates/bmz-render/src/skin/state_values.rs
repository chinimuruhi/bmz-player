use super::*;

pub(super) fn select_settings_screen_number_hidden(ref_id: i32) -> bool {
    matches!(
        ref_id,
        45..=49 | 71 | 72 | 74 | 77 | 78 | 90 | 91 | 92 | 1163 | 1164 | 121 | 150 | 170 | 350 | 370
    )
}

pub(super) fn select_volume_number(volume: f32) -> i64 {
    (volume.clamp(0.0, 1.0) * 100.0 + 0.0001) as i64
}

pub(super) fn select_settings_screen_number(ref_id: i32, state: &SkinDrawState) -> Option<i64> {
    match ref_id {
        96 if state.select_row_kind == SelectRowKind::Config => {
            Some(if state.play_level != 0 { state.play_level } else { state.select_play_level })
        }
        57 => Some(select_volume_number(state.select_master_volume)),
        58 => Some(select_volume_number(state.select_key_volume)),
        59 => Some(select_volume_number(state.select_bgm_volume)),
        12 => Some(state.judge_timing_offset_ms as i64),
        _ => None,
    }
}

pub(super) fn select_chart_metadata_available(state: &SkinDrawState) -> bool {
    !state.select_screen
        || (state.select_row_kind == SelectRowKind::Song
            && !state.select_is_folder
            && state.select_in_library)
}

pub(super) fn select_score_metadata_available(state: &SkinDrawState) -> bool {
    !state.select_screen
        || (matches!(state.select_row_kind, SelectRowKind::Song | SelectRowKind::Course)
            && !state.select_is_folder
            && state.select_in_library)
}

pub(super) fn select_chart_score_number_requires_song(ref_id: i32) -> bool {
    matches!(
        ref_id,
        45..=49
            | 71
            | 72
            | 74..=89
            | 100..=116
            | 150..=158
            | 170..=178
            | 183
            | 184
            | 400
            | 410..=427
    )
}

pub(super) fn select_chart_detail_number_requires_song(ref_id: i32) -> bool {
    matches!(ref_id, 350..=368 | 1163 | 1164)
}

pub(super) fn player_stat_u64(value: u64) -> i64 {
    value.min(i64::MAX as u64) as i64
}

pub(super) fn player_total_pgreat(stats: &PlayerStatsSnapshot) -> u64 {
    stats.fast_pgreat.saturating_add(stats.slow_pgreat)
}

pub(super) fn player_total_great(stats: &PlayerStatsSnapshot) -> u64 {
    stats.fast_great.saturating_add(stats.slow_great)
}

pub(super) fn player_total_good(stats: &PlayerStatsSnapshot) -> u64 {
    stats.fast_good.saturating_add(stats.slow_good)
}

pub(super) fn player_total_bad(stats: &PlayerStatsSnapshot) -> u64 {
    stats.fast_bad.saturating_add(stats.slow_bad)
}

pub(super) fn player_total_poor(stats: &PlayerStatsSnapshot) -> u64 {
    stats.fast_poor.saturating_add(stats.slow_poor)
}

pub(super) fn player_total_play_notes(stats: &PlayerStatsSnapshot) -> u64 {
    player_total_pgreat(stats)
        .saturating_add(player_total_great(stats))
        .saturating_add(player_total_good(stats))
        .saturating_add(player_total_bad(stats))
}

pub(super) fn daily_play_notes(stats: &DailyPlayerStatsSnapshot) -> u64 {
    stats.pgreat.saturating_add(stats.great).saturating_add(stats.good).saturating_add(stats.bad)
}

pub(super) fn daily_completed_notes(stats: &DailyPlayerStatsSnapshot) -> u64 {
    daily_play_notes(stats).saturating_add(stats.poor).saturating_add(stats.empty_poor)
}

pub(super) fn daily_ex_score(stats: &DailyPlayerStatsSnapshot) -> u64 {
    stats.pgreat.saturating_mul(2).saturating_add(stats.great)
}

pub(super) fn daily_max_ex_score(stats: &DailyPlayerStatsSnapshot) -> u64 {
    daily_play_notes(stats).saturating_mul(2)
}

pub(super) fn daily_rate_basis_points(stats: &DailyPlayerStatsSnapshot) -> u64 {
    let max = daily_max_ex_score(stats);
    daily_ex_score(stats).saturating_mul(10_000).checked_div(max).unwrap_or(0)
}

pub(super) fn daily_rank_index(stats: &DailyPlayerStatsSnapshot) -> i64 {
    let max = daily_max_ex_score(stats);
    if max == 0 {
        return 7;
    }
    let scaled = daily_ex_score(stats).saturating_mul(9);
    (2..=8)
        .rev()
        .find(|threshold| scaled >= max.saturating_mul(*threshold))
        .map_or(7, |threshold| 8 - threshold as i64)
}

pub(super) fn daily_rank_label(stats: &DailyPlayerStatsSnapshot) -> &'static str {
    ["AAA", "AA", "A", "B", "C", "D", "E", "F"][daily_rank_index(stats) as usize]
}

pub(super) fn rounded_percent(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        return 0.0;
    }
    ((numerator as f64 / denominator as f64) * 10_000.0).round() / 100.0
}

pub(super) fn m_select_daily_rank(stats: &DailyPlayerStatsSnapshot) -> &'static str {
    daily_rank_label(stats)
}

pub(super) fn m_select_daily_stats_text(
    id: &str,
    stats: &DailyPlayerStatsSnapshot,
) -> Option<String> {
    let notes = daily_play_notes(stats);
    let judge = |count: u64| format!("{}  ({}%)", count, rounded_percent(count, notes));
    Some(match id {
        "defaultNotesProcessingCounter_exscore" => {
            stats.pgreat.saturating_mul(2).saturating_add(stats.great).to_string()
        }
        "defaultNotesProcessingCounter_pg" => judge(stats.pgreat),
        "defaultNotesProcessingCounter_gr" => judge(stats.great),
        "defaultNotesProcessingCounter_gd" => judge(stats.good),
        "defaultNotesProcessingCounter_bd" => judge(stats.bad),
        "defaultNotesProcessingCounter_pr" => judge(stats.poor),
        "defaultNotesProcessingCounter_notes" | "defaultNotesProcessingCounter_stroke" => {
            notes.to_string()
        }
        "defaultNotesProcessingCounter_cp" => {
            format!("{}/{}", stats.clear_count, stats.play_count)
        }
        "defaultNotesProcessingCounter_rank" => m_select_daily_rank(stats).to_string(),
        "defaultNotesProcessingCounter_rate" => rounded_percent(
            stats.pgreat.saturating_mul(2).saturating_add(stats.great),
            notes.saturating_mul(2),
        )
        .to_string(),
        _ => return None,
    })
}

pub(super) fn operating_time_seconds(state: &SkinDrawState) -> i64 {
    i64::from(state.operating_time_ms.max(0)) / 1_000
}

pub(super) fn select_folder_lamp_counts_available(state: &SkinDrawState) -> bool {
    state.select_screen
        && matches!(
            state.select_row_kind,
            SelectRowKind::Folder | SelectRowKind::SearchFolder | SelectRowKind::TableFolder
        )
}

pub(super) fn select_row_folder_song_count(row: &SelectRowSnapshot) -> Option<u32> {
    (row.is_folder
        && matches!(
            row.kind,
            SelectRowKind::Folder | SelectRowKind::SearchFolder | SelectRowKind::TableFolder
        ))
    .then(|| row.folder_lamp_counts.iter().copied().sum())
}

pub(super) fn select_folder_lamp_count(ref_id: i32, counts: &[u32; 11]) -> Option<i64> {
    let index = match ref_id {
        320 => 0,
        321 => 1,
        322 => 2,
        323 => 3,
        324 => 4,
        325 => 5,
        326 => 6,
        327 => 7,
        328 => 8,
        329 => 9,
        330 => 10,
        _ => return None,
    };
    Some(counts[index] as i64)
}

pub(super) fn skin_state_number(ref_id: i32, state: &SkinDrawState) -> Option<i64> {
    if select_folder_lamp_counts_available(state)
        && let Some(value) = select_folder_lamp_count(ref_id, &state.select_folder_lamp_counts)
    {
        return Some(value);
    }

    if state.select_screen
        && select_chart_score_number_requires_song(ref_id)
        && !select_score_metadata_available(state)
    {
        return None;
    }

    if state.select_screen
        && select_chart_detail_number_requires_song(ref_id)
        && !select_chart_metadata_available(state)
    {
        return None;
    }

    if state.select_screen && state.in_settings {
        if let Some(value) = select_settings_screen_number(ref_id, state) {
            return Some(value);
        }
        if select_settings_screen_number_hidden(ref_id) {
            return None;
        }
    }
    match ref_id {
        // Lua draw 畳み込みのプレースホルダ (`number(0) >= 0` 等)
        0 => Some(0),
        17 => Some(player_stat_u64(state.player_stats.playtime_seconds / 3600)),
        18 => Some(player_stat_u64((state.player_stats.playtime_seconds / 60) % 60)),
        19 => Some(player_stat_u64(state.player_stats.playtime_seconds % 60)),
        20 => Some(i64::from(state.current_fps)),
        21..=26 => current_datetime_number(ref_id),
        27 => Some(operating_time_seconds(state) / 3_600),
        28 => Some((operating_time_seconds(state) / 60) % 60),
        29 => Some(operating_time_seconds(state) % 60),
        161 => Some(i64::from(state.play_timer_ms.unwrap_or(0).max(0) / 60_000)),
        162 => Some(i64::from((state.play_timer_ms.unwrap_or(0).max(0) / 1_000) % 60)),
        42 => Some(arrange_ref_index(state) as i64),
        43 => Some(arrange_2p_ref_index(state) as i64),
        344 => Some(extended_arrange_ref_index(state) as i64),
        345 => Some(extended_arrange_2p_ref_index(state) as i64),
        54 if state.select_screen => Some(state.select_double_option_index as i64),
        55 if state.select_screen => Some(state.select_hs_fix_index as i64),
        11 if state.select_screen => Some(state.select_mode_index as i64),
        12 if state.select_screen && state.select_option_panel == 3 => {
            Some(state.judge_timing_offset_ms as i64)
        }
        12 if state.select_screen => Some(state.select_sort_index as i64),
        300 if state.select_screen => state.select_folder_song_count.map(i64::from),
        30 => Some(player_stat_u64(state.player_stats.play_count)),
        31 => Some(player_stat_u64(state.player_stats.clear_count)),
        32 => Some(player_stat_u64(
            state.player_stats.play_count.saturating_sub(state.player_stats.clear_count),
        )),
        33 => Some(player_stat_u64(player_total_pgreat(&state.player_stats))),
        34 => Some(player_stat_u64(player_total_great(&state.player_stats))),
        35 => Some(player_stat_u64(player_total_good(&state.player_stats))),
        36 => Some(player_stat_u64(player_total_bad(&state.player_stats))),
        37 => Some(player_stat_u64(player_total_poor(&state.player_stats))),
        45..=49 | 96 => {
            Some(if state.play_level != 0 { state.play_level } else { state.select_play_level })
        }
        370 => Some(state.select_clear_index),
        92 if state.select_screen => {
            if !select_chart_metadata_available(state) {
                return None;
            }
            Some(select_chart_main_bpm(state).unwrap_or(state.select_bpm).round() as i64)
        }
        92 => Some(state.main_bpm.round() as i64),
        100 => Some(skin_point_score(state) as i64),
        71 | 101 | 171 => Some(state.ex_score as i64),
        72 => Some(state.total_notes as i64 * 2),
        74 | 106 => Some(state.total_notes.max(state.select_total_notes) as i64),
        333 => Some(player_stat_u64(player_total_play_notes(&state.player_stats))),
        350 if state.select_screen => Some(select_chart_normal_notes(state) as i64),
        351 if state.select_screen => Some(state.select_chart_long_notes as i64),
        352 if state.select_screen => Some(state.select_chart_scratch_notes as i64),
        353 if state.select_screen => Some(state.select_chart_long_scratch_notes as i64),
        354 if state.select_screen => Some(state.select_chart_mine_notes as i64),
        360 if state.select_screen => Some(state.select_chart_peak_density.floor() as i64),
        361 if state.select_screen => Some(decimal_afterdot(state.select_chart_peak_density)),
        362 if state.select_screen => Some(state.select_chart_end_density.floor() as i64),
        363 if state.select_screen => Some(decimal_afterdot(state.select_chart_end_density)),
        364 if state.select_screen => Some(state.select_chart_density.floor() as i64),
        365 if state.select_screen => Some(decimal_afterdot(state.select_chart_density)),
        // beatoraja chart_totalgauge(368): effective BMS #TOTAL from SongInformation.
        368 => Some(state.select_chart_total_gauge.floor() as i64),
        75 | 105 | 174 => Some(state.max_combo as i64),
        76 if state.select_screen => state.select_bp.map(|count| count as i64).or(Some(0)),
        76 if state.result_failed.is_some() => Some(current_bp(state) as i64),
        76 => Some((state.judge_counts.bad + state.judge_counts.poor) as i64),
        77 if state.select_screen => Some(state.select_play_count as i64),
        77 => Some(state.select_target_index as i64),
        78 if state.select_screen => Some(state.select_clear_count as i64),
        78 => Some(state.select_gauge_auto_shift_index as i64),
        79 if state.select_screen
            && (state.select_ex_score.is_some()
                || state.select_play_count > 0
                || state.select_clear_count > 0) =>
        {
            Some(state.select_play_count.saturating_sub(state.select_clear_count) as i64)
        }
        341 => Some(state.select_bottom_shiftable_gauge_index as i64),
        342 => Some(i64::from(state.hispeed_auto_adjust)),
        80 | 110 => Some(state.judge_counts.pgreat as i64),
        81 | 111 => Some(state.judge_counts.great as i64),
        82 | 112 => Some(state.judge_counts.good as i64),
        83 | 113 => Some(state.judge_counts.bad as i64),
        84 | 114 => Some(state.judge_counts.poor as i64),
        85 => judge_rate_int(state.judge_counts.pgreat, state.total_notes),
        86 => judge_rate_int(state.judge_counts.great, state.total_notes),
        87 => judge_rate_int(state.judge_counts.good, state.total_notes),
        88 => judge_rate_int(state.judge_counts.bad, state.total_notes),
        89 => judge_rate_int(state.judge_counts.poor, state.total_notes),
        102 => Some(current_score_rate_parts(state).0 as i64),
        103 => Some(current_score_rate_parts(state).1 as i64),
        115 | 155 => Some(score_rate_parts(state.ex_score, state.total_notes).0 as i64),
        116 | 156 => Some(score_rate_parts(state.ex_score, state.total_notes).1 as i64),
        104 => Some(state.combo as i64),
        107 => Some(state.gauge.floor() as i64),
        407 => Some(gauge_after_dot(state.gauge) as i64),
        163 => Some((state.timeleft_ms / 60_000) as i64),
        164 => Some(((state.timeleft_ms / 1_000) % 60) as i64),
        165 => Some((state.resource_load_progress.clamp(0.0, 1.0) * 100.0) as i64),
        1163 => Some(result_or_select_length_ms(state) / 60_000),
        1164 => Some((result_or_select_length_ms(state) / 1_000) % 60),
        310 => Some(state.hispeed.floor() as i64),
        311 => Some(((state.hispeed * 100.0) as i64) % 100),
        1900 => Some(skin_hispeed_mode_index(state) as i64),
        1901 => Some(i64::from(skin_hispeed_mode_is_floating(state))),
        1902 => Some(skin_target_green_number(state)),
        SKIN_REF_BMZ_SELECT_SETTINGS_ROW_KIND => {
            Some(i64::from(select_settings_row_kind_index(state.select_row_kind)))
        }
        1930 => Some(player_stat_u64(state.player_stats.daily.play_count)),
        1931 => Some(player_stat_u64(state.player_stats.daily.clear_count)),
        1932 => Some(player_stat_u64(state.player_stats.daily.pgreat)),
        1933 => Some(player_stat_u64(state.player_stats.daily.great)),
        1934 => Some(player_stat_u64(state.player_stats.daily.good)),
        1935 => Some(player_stat_u64(state.player_stats.daily.bad)),
        1936 => Some(player_stat_u64(state.player_stats.daily.poor)),
        1937 => Some(player_stat_u64(state.player_stats.daily.empty_poor)),
        1938 => Some(player_stat_u64(daily_play_notes(&state.player_stats.daily))),
        1939 => Some(player_stat_u64(daily_completed_notes(&state.player_stats.daily))),
        1940 => Some(player_stat_u64(daily_ex_score(&state.player_stats.daily))),
        1941 => Some(player_stat_u64(daily_max_ex_score(&state.player_stats.daily))),
        1942 => Some(player_stat_u64(daily_rate_basis_points(&state.player_stats.daily))),
        1943 => Some(daily_rank_index(&state.player_stats.daily)),
        1944 => Some(player_stat_u64(state.player_stats.daily.score_update_count)),
        1945 => Some(player_stat_u64(state.player_stats.daily.clear_update_count)),
        1946 => Some(player_stat_u64(state.player_stats.daily.miss_count_update_count)),
        SKIN_REF_BMZ_COURSE_STAGE_COUNT => Some(i64::from(state.course_result.stage_count)),
        id if (SKIN_REF_BMZ_COURSE_STAGE_EX_BASE
            ..SKIN_REF_BMZ_COURSE_STAGE_EX_BASE + SKIN_BMZ_COURSE_STAGE_COUNT as i32)
            .contains(&id) =>
        {
            Some(
                state.course_result.stages[(ref_id - SKIN_REF_BMZ_COURSE_STAGE_EX_BASE) as usize]
                    .ex_score as i64,
            )
        }
        id if (SKIN_REF_BMZ_COURSE_STAGE_GAUGE_BASE
            ..SKIN_REF_BMZ_COURSE_STAGE_GAUGE_BASE + SKIN_BMZ_COURSE_STAGE_COUNT as i32)
            .contains(&id) =>
        {
            Some(
                state.course_result.stages[(ref_id - SKIN_REF_BMZ_COURSE_STAGE_GAUGE_BASE) as usize]
                    .gauge
                    .floor() as i64,
            )
        }
        id if (SKIN_REF_BMZ_COURSE_STAGE_BP_BASE
            ..SKIN_REF_BMZ_COURSE_STAGE_BP_BASE + SKIN_BMZ_COURSE_STAGE_COUNT as i32)
            .contains(&id) =>
        {
            Some(
                state.course_result.stages[(ref_id - SKIN_REF_BMZ_COURSE_STAGE_BP_BASE) as usize].bp
                    as i64,
            )
        }
        id if (SKIN_REF_BMZ_COURSE_STAGE_RATE_BASE
            ..SKIN_REF_BMZ_COURSE_STAGE_RATE_BASE + SKIN_BMZ_COURSE_STAGE_COUNT as i32)
            .contains(&id) =>
        {
            Some(
                state.course_result.stages[(ref_id - SKIN_REF_BMZ_COURSE_STAGE_RATE_BASE) as usize]
                    .rate_basis_points as i64,
            )
        }
        SKIN_REF_BMZ_KEY_MODE => effective_skin_key_mode(state).map(skin_key_mode_number_i64),
        SKIN_REF_BMZ_ACTIVE_LANE_COUNT => {
            effective_skin_key_mode(state).map(|mode| mode.lane_count() as i64)
        }
        312 => {
            if state.select_screen && !duration_refs_available(state) {
                return None;
            }
            Some(state_duration_number_ms(state))
        }
        313 => {
            if state.select_screen && !duration_refs_available(state) {
                return None;
            }
            Some(state_duration_green_number_ms(state))
        }
        1312..=1327 => lane_cover_duration_number(ref_id, state),
        308 if state.select_screen => Some(state.select_ln_mode_index as i64),
        340 if state.select_screen => Some(state.select_judge_algorithm_index as i64),
        // BPM 系: NUMBER_MAXBPM=90, NUMBER_MINBPM=91, NUMBER_NOWBPM=160
        90 => {
            if !select_chart_metadata_available(state) {
                return None;
            }
            Some(if state.max_bpm > 0.0 { state.max_bpm } else { state.select_max_bpm }.round()
                as i64)
        }
        91 => {
            if !select_chart_metadata_available(state) {
                return None;
            }
            Some(if state.min_bpm > 0.0 { state.min_bpm } else { state.select_min_bpm }.round()
                as i64)
        }
        160 => {
            if !select_chart_metadata_available(state) {
                return None;
            }
            Some(if state.now_bpm > 0.0 { state.now_bpm } else { state.select_bpm }.round() as i64)
        }
        // レーンカバー: NUMBER_LANECOVER1=14 (0-1000)
        14 => Some((bmz_lane_cover_for_lift(state.lane_cover, state.lift) * 1000.0).round() as i64),
        // リフト: NUMBER_LIFT1=314 (0-1000)
        314 => Some((state.lift.clamp(0.0, 1.0) * 1000.0).round() as i64),
        // 選曲画面の音量表示: MASTER/KEY/BGM volume (0-100)
        57 => Some(select_volume_number(state.select_master_volume)),
        58 => Some(select_volume_number(state.select_key_volume)),
        59 => Some(select_volume_number(state.select_bgm_volume)),
        // 判定タイミングずれ: VALUE_JUDGE_1P/2P/3P_DURATION=525/526/527 (ms、符号付き)
        // beatoraja getRecentJudgeTiming は note時刻 - 押下時刻 (FAST=正)。
        // bmz の judge_timing_ms は 押下時刻 - note時刻 (FAST=負) なので符号を反転する。
        525 => state.judge_timing_ms[0].map(|ms| -(ms as i64)),
        526 => state.judge_timing_ms[1].map(|ms| -(ms as i64)),
        527 => state.judge_timing_ms[2].map(|ms| -(ms as i64)),
        // 判定タイミングオフセット設定値 (NUMBER_JUDGETIMING=12)
        12 => Some(state.judge_timing_offset_ms as i64),
        // Result judgement duration / timing distribution stats.
        372 => state.average_duration_us.map(|value| value / 1_000),
        373 => state.average_duration_us.map(|value| (value / 10) % 100),
        374 => state.average_timing_ms.map(|value| value as i64),
        375 => state.average_timing_ms.map(timing_afterdot),
        376 => state.stddev_timing_ms.map(|value| value as i64),
        377 => state.stddev_timing_ms.map(|value| ((value.abs() * 100.0) as i64) % 100),
        SKIN_REF_BMZ_RESULT_IR_SCOPE => Some(state.ir_ranking.scope.index()),
        SKIN_REF_BMZ_RESULT_IR_SCOPE_TOTAL => state.ir_ranking.total_player,
        // IR numbers (beatoraja NUMBER_IR_*)。Offline / 未取得時は
        // beatoraja の Integer.MIN_VALUE と同じく値なしにする。
        179 => state.ir_ranking.rank,
        180 | 200 => state.ir_ranking.total_player,
        181 => state.ir_ranking.clear_rate,
        182 => state.ir_ranking.previous_rank,
        226 => ir_total_clear_count(&state.ir_ranking),
        227 => state.ir_ranking.clear_rate,
        241 => state.ir_ranking.clear_rate.map(|_| 0),
        380..=389 => {
            ir_ranking_entry(&state.ir_ranking, ref_id - 380).and_then(|entry| entry.ex_score)
        }
        390..=399 => ir_ranking_entry(&state.ir_ranking, ref_id - 390).and_then(|entry| entry.rank),
        201..=242 => None,
        // NUMBER_RIVAL_SCORE / MAXCOMBO / MISSCOUNT (IR ライバルベスト)。
        271 => state.rival_ex_score,
        275 => state.rival_max_combo,
        276 => state.rival_bp,
        280..=284 => {
            state.rival_judge_counts.map(|counts| i64::from(counts[(ref_id - 280) as usize]))
        }
        285..=289 => {
            let notes = state.select_total_notes.max(state.total_notes);
            let count = state.rival_judge_counts?[(ref_id - 285) as usize];
            (notes > 0).then_some(i64::from(count) * 100 / i64::from(notes))
        }
        // ベストスコア / ターゲットスコア (DB から供給、未取得時は None)
        150 | 170 => projected_best_score_at_progress(state).map(|s| s as i64),
        121 | 151 => state.target_ex_score.map(|s| projected_score_at_progress(s, state) as i64),
        122 | 123 | 135 | 136 | 157 | 158 => {
            state.target_ex_score.map(|target| score_rate_parts(target, state.total_notes)).map(
                |parts| (if matches!(ref_id, 122 | 135 | 157) { parts.0 } else { parts.1 }) as i64,
            )
        }
        183 | 184 => result_mybest_ex_score_display(state).map(|best| {
            let parts = score_rate_parts(best, state.total_notes);
            (if ref_id == 183 { parts.0 } else { parts.1 }) as i64
        }),
        400 => state.judge_rank.map(|rank| rank as i64),
        154 => result_grade_diff_number(state),
        // NUMBER_DIFF_HIGHSCORE=152, NUMBER_DIFF_HIGHSCORE2=172 (符号付き、ex_score - best)
        152 | 172 => {
            projected_best_score_at_progress(state).map(|best| state.ex_score as i64 - best as i64)
        }
        // NUMBER_DIFF_TARGETSCORE=153 (符号付き、ex_score - target)
        153 => state.target_ex_score.map(|target| {
            state.ex_score as i64 - projected_score_at_progress(target, state) as i64
        }),
        // NUMBER_TARGET_MAXCOMBO=173 is the old/my-best score on Result.
        173 if state.result_failed.is_some() => {
            state.previous_best_max_combo.filter(|combo| *combo > 0).map(|combo| combo as i64)
        }
        // NUMBER_DIFF_MAXCOMBO=175 (current - old/my-best).
        175 if state.result_failed.is_some() => state
            .previous_best_max_combo
            .filter(|combo| *combo > 0)
            .map(|previous| state.max_combo as i64 - previous as i64),
        // NUMBER_TARGET_MISSCOUNT=176 (Result では old/mybest min_bp)
        176 => result_mybest_bp(state).map(|c| c as i64),
        // NUMBER_MISSCOUNT2=177 (Result では今回の min_bp)
        177 => Some(current_bp(state) as i64),
        // NUMBER_DIFF_MISSCOUNT=178 (符号付き、今回 min_bp - old/mybest min_bp)
        178 => result_diff_misscount(state),
        // NUMBER_TARGET_CLEAR=371
        371 if state.result_failed.is_some() => result_mybest_clear_index_display(state),
        371 => result_mybest_clear_index(state).or(state.target_clear_index),
        // Fast/Slow split (PGREAT/GREAT/GOOD/BAD/POOR)
        410 | 411 if state.autoplay => Some(0),
        410 => state.fast_slow_counts.map(|c| c.fast_pgreat as i64),
        411 => state.fast_slow_counts.map(|c| c.slow_pgreat as i64),
        412 => state.fast_slow_counts.map(|c| c.fast_great as i64),
        413 => state.fast_slow_counts.map(|c| c.slow_great as i64),
        414 => state.fast_slow_counts.map(|c| c.fast_good as i64),
        415 => state.fast_slow_counts.map(|c| c.slow_good as i64),
        416 => state.fast_slow_counts.map(|c| c.fast_bad as i64),
        417 => state.fast_slow_counts.map(|c| c.slow_bad as i64),
        418 => state.fast_slow_counts.map(|c| c.fast_poor as i64),
        419 => state.fast_slow_counts.map(|c| c.slow_poor as i64),
        420 => Some(state.judge_counts.empty_poor as i64),
        421 => state.fast_slow_counts.map(|c| c.fast_empty_poor as i64),
        422 => state.fast_slow_counts.map(|c| c.slow_empty_poor as i64),
        // NUMBER_TOTALEARLY=423, NUMBER_TOTALLATE=424
        423 => state.fast_slow_counts.map(|c| c.fast_total() as i64),
        424 => state.fast_slow_counts.map(|c| c.slow_total() as i64),
        425 | 427 if state.select_screen => state.select_cb.map(|count| count as i64).or(Some(0)),
        425 | 427 if state.result_failed.is_some() => Some(current_cb(state) as i64),
        425 => Some((state.judge_counts.bad + state.judge_counts.poor) as i64),
        426 => Some(poor_plus_miss(state.judge_counts) as i64),
        427 => Some(bad_plus_poor_plus_miss(state.judge_counts) as i64),
        ref_id if random_lane_ref_slot(ref_id).is_some() => {
            skin_random_lane_ref_number(ref_id, state)
        }
        _ => None,
    }
}

pub(super) fn skin_hispeed_mode_index(state: &SkinDrawState) -> i32 {
    state.hispeed_mode_index.clamp(0, 1)
}

pub(super) fn skin_hispeed_mode_is_floating(state: &SkinDrawState) -> bool {
    skin_hispeed_mode_index(state) == 1
}

pub(super) fn skin_target_green_number(state: &SkinDrawState) -> i64 {
    if skin_hispeed_mode_is_floating(state) && state.target_green_number > 0 {
        i64::from(state.target_green_number)
    } else {
        state_duration_green_number_ms(state)
    }
}

pub(super) fn lane_cover_duration_number(ref_id: i32, state: &SkinDrawState) -> Option<i64> {
    if state.select_screen && !duration_refs_available(state) {
        return None;
    }
    let offset = ref_id.checked_sub(1312)?;
    let green = offset % 2 == 1;
    let cover = offset % 4 < 2;
    let mode = offset / 4;
    let duration = if mode == 0 {
        current_lane_cover_duration_number_ms(cover, state)
            .or_else(|| lane_cover_duration_number_ms_for_bpm(cover, mode, state))?
    } else {
        lane_cover_duration_number_ms_for_bpm(cover, mode, state)?
    };
    Some(if green { duration_to_green_number_ms_i64(duration) } else { duration })
}

pub(super) fn current_lane_cover_duration_number_ms(
    cover: bool,
    state: &SkinDrawState,
) -> Option<i64> {
    if !duration_refs_available(state) {
        return None;
    }
    let cover_on_visible = bmz_visible_lane_fraction(state.lane_cover, state.lift);
    let target_visible =
        if cover { cover_on_visible } else { bmz_visible_lane_fraction(0.0, state.lift) };
    let duration = state.total_duration_ms.max(0) as i64;
    let duration = if duration > 0 { duration } else { state_duration_number_ms(state) };
    if cover {
        return Some(duration);
    }
    if cover_on_visible > f32::EPSILON {
        return Some(
            (duration.max(0) as f64 * target_visible as f64 / cover_on_visible as f64)
                .round()
                .max(0.0) as i64,
        );
    }
    None
}

pub(super) fn lane_cover_duration_number_ms_for_bpm(
    cover: bool,
    mode: i32,
    state: &SkinDrawState,
) -> Option<i64> {
    let target_bpm = match mode {
        0 => bpm_value_or_select(state.now_bpm, state.select_bpm),
        1 => bpm_value_or_select(state.main_bpm, state.select_bpm),
        2 => bpm_value_or_select(state.min_bpm, state.select_min_bpm),
        3 => bpm_value_or_select(state.max_bpm, state.select_max_bpm),
        _ => return None,
    }?;
    let visible = if cover {
        bmz_visible_lane_fraction(state.lane_cover, state.lift)
    } else {
        bmz_visible_lane_fraction(0.0, state.lift)
    };
    let duration = 240_000.0 / target_bpm as f64 / state.hispeed.max(0.01) as f64 * visible as f64;
    Some(duration.round().max(0.0) as i64)
}

pub(super) fn bmz_lane_cover_for_lift(lane_cover: f32, lift: f32) -> f32 {
    lane_cover.clamp(0.0, (1.0 - lift.clamp(0.0, 1.0)).clamp(0.0, 1.0))
}

pub(super) fn bmz_visible_lane_fraction(lane_cover: f32, lift: f32) -> f32 {
    (1.0 - bmz_lane_cover_for_lift(lane_cover, lift) - lift.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

pub(super) fn duration_refs_available(state: &SkinDrawState) -> bool {
    state.duration_green_ms.is_some_and(|value| value > 0) || state.total_duration_ms > 0
}

pub(super) fn state_duration_number_ms(state: &SkinDrawState) -> i64 {
    state
        .duration_green_ms
        .map(green_duration_to_duration)
        .unwrap_or_else(|| state.total_duration_ms.max(0) as i64)
}

pub(super) fn state_duration_green_number_ms(state: &SkinDrawState) -> i64 {
    state
        .duration_green_ms
        .map(|value| value.max(0) as i64)
        .unwrap_or_else(|| duration_to_green_number_ms(state.total_duration_ms) as i64)
}

pub(super) fn green_duration_to_duration(green_duration_ms: i32) -> i64 {
    let green = green_duration_ms.max(0) as i64;
    (green.saturating_mul(5).saturating_add(1)) / 3
}

pub fn green_duration_to_duration_i32(green_duration_ms: i32) -> i32 {
    green_duration_to_duration(green_duration_ms).min(i32::MAX as i64) as i32
}

pub fn duration_to_green_number_ms(duration_ms: i32) -> i32 {
    let duration = duration_ms.max(0) as i64;
    duration_to_green_number_ms_i64(duration) as i32
}

pub(super) fn duration_to_green_number_ms_i64(duration_ms: i64) -> i64 {
    duration_ms.max(0).saturating_mul(3).saturating_add(2).saturating_div(5).min(i32::MAX as i64)
}

pub(super) fn bpm_value_or_select(value: f32, fallback: f32) -> Option<f32> {
    if value > 0.0 {
        Some(value)
    } else if fallback > 0.0 {
        Some(fallback)
    } else {
        None
    }
}

pub(super) fn ir_ranking_entry(
    ranking: &crate::scene::ResultIrSnapshot,
    index: i32,
) -> Option<crate::scene::ResultIrRankingEntrySnapshot> {
    ranking.entries.get(usize::try_from(index).ok()?).copied().filter(|entry| {
        entry.rank.is_some() || entry.ex_score.is_some() || !entry.player_name.as_str().is_empty()
    })
}

pub(super) fn ir_ranking_score_and_max(state: &SkinDrawState, slot: i32) -> Option<(i64, i64)> {
    if !(1..=10).contains(&slot) {
        return None;
    }
    let score = ir_ranking_entry(&state.ir_ranking, slot - 1)?.ex_score?;
    let max_score = i64::from(state.select_total_notes.max(state.total_notes).checked_mul(2)?);
    Some((score, max_score))
}

pub(super) fn ir_ranking_score_rate_parts(state: &SkinDrawState, slot: i32) -> Option<(i64, i64)> {
    let (score, max_score) = ir_ranking_score_and_max(state, slot)?;
    if score <= 0 || max_score <= 0 {
        return Some((0, 0));
    }
    let scaled = i128::from(score).checked_mul(10_000)?.checked_div(i128::from(max_score))?;
    Some((i64::try_from(scaled / 100).ok()?, i64::try_from(scaled % 100).ok()?))
}

pub(super) fn ir_total_clear_count(ranking: &crate::scene::ResultIrSnapshot) -> Option<i64> {
    let total = ranking.total_player?;
    let clear_rate = ranking.clear_rate?;
    Some((total * clear_rate + 50) / 100)
}

pub(super) fn result_grade_diff_number(state: &SkinDrawState) -> Option<i64> {
    if !grade_diff_score_available(state) {
        return None;
    }
    match state.result_grade_diff_display {
        ResultGradeDiffDisplay::Next => next_rank_diff(state),
        ResultGradeDiffDisplay::Nearest => {
            nearest_grade_diff_for_state(state).map(|diff| diff.value)
        }
    }
}

pub(crate) fn result_grade_diff_label(state: &SkinDrawState) -> Option<String> {
    if !grade_diff_score_available(state) {
        return None;
    }
    match state.result_grade_diff_display {
        ResultGradeDiffDisplay::Next => next_rank_diff(state).map(|value| format!("{value:+}")),
        ResultGradeDiffDisplay::Nearest => nearest_grade_diff(state).map(|diff| diff.label()),
    }
}

pub(super) fn grade_diff_score_available(state: &SkinDrawState) -> bool {
    !state.select_screen
        || state.select_play_count > 0
        || state.select_ex_score.is_some_and(|score| score > 0)
}

pub(super) fn next_rank_diff(state: &SkinDrawState) -> Option<i64> {
    let ex_score = state.select_ex_score.unwrap_or(state.ex_score) as i64;
    let total_notes = state.select_total_notes.max(state.total_notes) as i64;
    let max_score = total_notes.checked_mul(2)?;
    if max_score <= 0 {
        return None;
    }
    let ex_score = ex_score.clamp(0, max_score);
    for rank_step in (0..=24).step_by(3) {
        let threshold = div_ceil(rank_step as i64 * max_score, 27);
        if ex_score < threshold {
            return Some(ex_score - threshold);
        }
    }
    Some(ex_score - max_score)
}

pub(super) fn next_rank_grade(state: &SkinDrawState) -> Option<&'static str> {
    let ex_score = state.select_ex_score.unwrap_or(state.ex_score) as i64;
    let total_notes = state.select_total_notes.max(state.total_notes) as i64;
    let max_score = total_notes.checked_mul(2)?;
    if max_score <= 0 {
        return None;
    }
    let ex_score = ex_score.clamp(0, max_score);
    for rank_step in (3..=24).step_by(3) {
        let threshold = div_ceil(rank_step as i64 * max_score, 27);
        if ex_score < threshold {
            return next_rank_grade_for_step(rank_step);
        }
    }
    Some("MAX")
}

pub(super) fn next_rank_grade_for_step(rank_step: i32) -> Option<&'static str> {
    match rank_step {
        3 => Some("E"),
        6 => Some("D"),
        9 => Some("C"),
        12 => Some("B"),
        15 => Some("A"),
        18 => Some("AA"),
        21 => Some("AAA"),
        24 => Some("AAA"),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NearestGradeDiff {
    pub(super) grade: &'static str,
    pub(super) value: i64,
}

impl NearestGradeDiff {
    fn label(self) -> String {
        format!("{}{:+}", self.grade, self.value)
    }
}

pub(super) fn nearest_grade_diff(state: &SkinDrawState) -> Option<NearestGradeDiff> {
    let score = state.select_ex_score.unwrap_or(state.ex_score) as i64;
    let total_notes = state.select_total_notes.max(state.total_notes) as i64;
    let max = total_notes.checked_mul(2)?;
    if max <= 0 {
        return None;
    }
    let score = score.clamp(0, max);
    if score * 9 < max * 2 {
        return Some(if score * 18 < max * 2 {
            NearestGradeDiff { grade: "F", value: score }
        } else {
            NearestGradeDiff { grade: "E", value: -div_ceil(max * 2 - score * 9, 9) }
        });
    }
    for (lower_step, plus_grade, minus_grade, half_step, upper_step) in [
        (2, "E", "D", 5, 3),
        (3, "D", "C", 7, 4),
        (4, "C", "B", 9, 5),
        (5, "B", "A", 11, 6),
        (6, "A", "AA", 13, 7),
        (7, "AA", "AAA", 15, 8),
    ] {
        if score * 9 < max * upper_step {
            return Some(if score * 18 < max * half_step {
                NearestGradeDiff {
                    grade: plus_grade,
                    value: (score - div_ceil(max * lower_step, 9)).max(0),
                }
            } else {
                NearestGradeDiff {
                    grade: minus_grade,
                    value: -div_ceil(max * upper_step - score * 9, 9),
                }
            });
        }
    }
    if score * 18 < max * 17 {
        Some(NearestGradeDiff { grade: "AAA", value: (score - div_ceil(max * 8, 9)).max(0) })
    } else if score < max {
        Some(NearestGradeDiff { grade: "MAX", value: -(max - score) })
    } else {
        Some(NearestGradeDiff { grade: "MAX", value: 0 })
    }
}

pub(super) fn nearest_grade_diff_for_state(state: &SkinDrawState) -> Option<NearestGradeDiff> {
    if state.result_grade_diff_f_fallback_to_e {
        return nearest_grade_diff_for_destination(state, false);
    }
    nearest_grade_diff(state)
}

pub(super) fn projected_score_at_progress(final_score: u32, state: &SkinDrawState) -> u32 {
    if state.total_notes == 0 {
        return final_score;
    }
    let past_notes = state.past_notes.min(state.total_notes);
    ((final_score as u64 * past_notes as u64) / state.total_notes as u64) as u32
}

pub(super) fn result_or_select_length_ms(state: &SkinDrawState) -> i64 {
    if state.result_failed.is_some() {
        if state.result_duration_ms > 0 {
            state.result_duration_ms as i64
        } else {
            state.total_duration_ms.max(0) as i64
        }
    } else {
        state.select_length_ms.max(0)
    }
}

pub(super) fn projected_best_score_at_progress(state: &SkinDrawState) -> Option<u32> {
    state.projected_best_ex_score.or_else(|| {
        result_mybest_ex_score(state)
            .map(|score| projected_score_at_progress(score, state))
            .or_else(|| state.result_failed.is_some().then_some(0))
    })
}

pub(super) fn div_ceil(numerator: i64, denominator: i64) -> i64 {
    if denominator <= 0 {
        return 0;
    }
    numerator.div_euclid(denominator) + i64::from(numerator.rem_euclid(denominator) != 0)
}

pub(super) fn rank_threshold(max_score: u32, rank_step: u32) -> u32 {
    div_ceil(rank_step as i64 * max_score as i64, 27).clamp(0, u32::MAX as i64) as u32
}

pub(super) fn judge_rank_option_matches(op: i32, judge_rank: Option<i32>) -> bool {
    let Some(rank) = judge_rank else {
        return op == 182;
    };
    match op {
        180 => rank == 0 || (10..35).contains(&rank),
        181 => rank == 1 || (35..60).contains(&rank),
        182 => rank == 2 || (60..85).contains(&rank),
        183 => rank == 3 || (85..110).contains(&rank),
        184 => rank == 4 || rank >= 110,
        _ => false,
    }
}

pub(super) fn judge_rate_int(count: u32, total_notes: u32) -> Option<i64> {
    if total_notes == 0 {
        return None;
    }
    Some(count as i64 * 100 / total_notes as i64)
}

pub(super) fn poor_plus_miss(counts: DisplayJudgeCounts) -> u32 {
    counts.poor.saturating_add(counts.empty_poor)
}

pub(super) fn bad_plus_poor_plus_miss(counts: DisplayJudgeCounts) -> u32 {
    counts.bad.saturating_add(poor_plus_miss(counts))
}

pub(super) fn score_rate_parts(ex_score: u32, total_notes: u32) -> (u32, u32) {
    if total_notes == 0 {
        return (0, 0);
    }
    // beatoraja ScoreDataProperty: rateInt=(int)(rate*100), rateAfterDot=((int)(rate*10000))%100
    let rate_scaled = ex_score.saturating_mul(10000) / total_notes.saturating_mul(2).max(1);
    (rate_scaled / 100, rate_scaled % 100)
}

pub(super) fn current_score_rate_notes(state: &SkinDrawState) -> Option<u32> {
    if state.past_notes > 0 {
        Some(state.past_notes)
    } else if state.total_notes > 0 || state.select_total_notes > 0 {
        Some(0)
    } else {
        None
    }
}

pub(super) fn current_score_rate_value(state: &SkinDrawState) -> f32 {
    match current_score_rate_notes(state) {
        Some(0) => 1.0,
        Some(notes) => state.ex_score as f32 / notes.saturating_mul(2).max(1) as f32,
        None => 0.0,
    }
}

pub(super) fn current_score_rate_parts(state: &SkinDrawState) -> (u32, u32) {
    match current_score_rate_notes(state) {
        Some(0) => (100, 0),
        Some(notes) => score_rate_parts(state.ex_score, notes),
        None => (0, 0),
    }
}

pub(super) fn skin_image_texture_region(
    image: &SkinImageDef,
    source_size: SkinImageSize,
    elapsed_ms: i32,
) -> TextureRegion {
    skin_image_texture_region_for_state(
        image,
        source_size,
        elapsed_ms,
        None,
        (image.x, image.y, image.w, image.h),
    )
}

pub(super) fn pre_ready_lane_cover_value_destination(
    destination: &SkinDestinationDef,
    value: &SkinValueDef,
    state: &SkinDrawState,
) -> bool {
    destination.timer == Some(40)
        && state.ready_timer_ms.is_none()
        && state.lane_cover_changing
        && destination.op.contains(&270)
        && skin_value_is_lane_cover_number(value)
}

pub(super) fn skin_value_is_lane_cover_number(value: &SkinValueDef) -> bool {
    matches!(value.ref_id, 14 | 312 | 313 | 1312..=1327)
        || skin_expr_references_lane_cover_number(&value.expr)
        || skin_expr_references_lane_cover_number(&value.value_expr)
}

pub(super) fn skin_expr_references_lane_cover_number(expr: &str) -> bool {
    ["number(14)", "number(312)", "number(313)"].iter().any(|needle| expr.contains(needle))
        || (1312..=1327).any(|ref_id| expr.contains(&format!("number({ref_id})")))
}

/// Starseeker 閉店の `src = 0, x = 0, y = 0` sentinel は `system` の黒 1px
/// (`black` image と同じ UV) を指す。ECFN の判定ラインなど、`src = 0` でも
/// 明示的な crop 座標を持つ画像はそのまま扱う。
pub(super) fn skin_image_pixel_rect(
    image: &SkinImageDef,
    images: &HashMap<&str, &SkinImageDef>,
) -> (i32, i32, i32, i32) {
    if image.src == "0"
        && image.x == 0
        && image.y == 0
        && let Some(black) = images.get("black")
    {
        return (black.x, black.y, black.w, black.h);
    }
    (image.x, image.y, image.w, image.h)
}

/// `image.ref_id` が指定されている場合、`SkinDrawState` から ref 値を引いて
/// 行インデックス（divy 方向）として使う。divx 方向は cycle 経過時間でアニメ。
/// ref 未指定なら従来通り全フレームを cycle で順次再生する。
pub(super) fn skin_image_texture_region_for_state(
    image: &SkinImageDef,
    source_size: SkinImageSize,
    elapsed_ms: i32,
    state: Option<&SkinDrawState>,
    pixel_rect: (i32, i32, i32, i32),
) -> TextureRegion {
    let source_width = source_size.width.max(1.0);
    let source_height = source_size.height.max(1.0);
    let (px, py, pw, ph) = resolve_skin_image_pixel_rect(pixel_rect, source_width, source_height);
    let divx = image.divx.max(1);
    let divy = image.divy.max(1);
    let frame_count = divx * divy;

    // ref_id / act が指定されている画像は「状態値 = 行」「cycle = 列のサブアニメ」と解釈する。
    // 値が解決できない場合 (state 未提供 or 値 None) は行 0 にフォールバックし、
    // 全フレームを順次再生する cycle モードへは落とさない（高速点滅を防ぐため）。
    let frame_index = if image.ref_id != 0 || image.act.is_some() {
        let row = state
            .and_then(|s| {
                if image.ref_id != 0 {
                    skin_image_ref_number(image.ref_id, s)
                } else {
                    image.act.map(|event_id| skin_state_event_index(event_id, s) as i64)
                }
            })
            .unwrap_or(0);
        let max_row = if image.len > 0 { image.len.min(divy) } else { divy };
        let row = row.clamp(0, (max_row - 1).max(0) as i64) as i32;
        let col = if image.cycle > 0 && divx > 1 {
            (elapsed_ms.rem_euclid(image.cycle) * divx / image.cycle).min(divx - 1)
        } else {
            0
        };
        row * divx + col
    } else if image.cycle > 0 && frame_count > 1 {
        (elapsed_ms.rem_euclid(image.cycle) * frame_count / image.cycle).min(frame_count - 1)
    } else {
        0
    };

    let cell_width = pw as f32 / divx as f32;
    let cell_height = ph as f32 / divy as f32;
    let source_column = frame_index % divx;
    let source_row = frame_index / divx;
    TextureRegion {
        x: (px as f32 + cell_width * source_column as f32) / source_width,
        y: (py as f32 + cell_height * source_row as f32) / source_height,
        width: cell_width / source_width,
        height: cell_height / source_height,
    }
}

pub(super) fn skin_image_ref_number(ref_id: i32, state: &SkinDrawState) -> Option<i64> {
    skin_image_index_number(ref_id, state)
}

pub(super) fn arrange_ref_index(state: &SkinDrawState) -> usize {
    if state.result_failed.is_some() {
        state.result_arrange_index
    } else {
        state.select_arrange_index
    }
}

pub(super) fn arrange_2p_ref_index(state: &SkinDrawState) -> usize {
    if state.result_failed.is_some() {
        state.result_arrange_2p_index
    } else {
        state.select_arrange_2p_index
    }
}

pub(super) fn extended_arrange_ref_index(state: &SkinDrawState) -> usize {
    if state.result_failed.is_some() {
        state.result_extended_arrange_index
    } else {
        state.select_extended_arrange_index
    }
}

pub(super) fn extended_arrange_2p_ref_index(state: &SkinDrawState) -> usize {
    if state.result_failed.is_some() {
        state.result_extended_arrange_2p_index
    } else {
        state.select_extended_arrange_2p_index
    }
}

pub(super) fn random_lane_ref_slot(ref_id: i32) -> Option<usize> {
    match ref_id {
        450..=466 | 469 => Some((ref_id - SKIN_RANDOM_LANE_REF_BASE) as usize),
        _ => None,
    }
}

pub(super) fn skin_random_lane_ref_number(ref_id: i32, state: &SkinDrawState) -> Option<i64> {
    let slot = random_lane_ref_slot(ref_id)?;
    Some(state.random_lane_refs[slot] as i64)
}

pub(super) fn resolve_skin_image_pixel_rect(
    pixel_rect: (i32, i32, i32, i32),
    source_width: f32,
    source_height: f32,
) -> (i32, i32, i32, i32) {
    let (px, py, pw, ph) = pixel_rect;
    let resolved_w =
        if pw < 0 { (source_width.round() as i32).saturating_sub(px).max(0) } else { pw };
    let resolved_h =
        if ph < 0 { (source_height.round() as i32).saturating_sub(py).max(0) } else { ph };
    (px, py, resolved_w, resolved_h)
}

pub(super) fn gauge_after_dot(gauge: f32) -> u32 {
    if gauge > 0.0 && gauge < 0.1 { 1 } else { ((gauge.max(0.0) * 10.0) as u32) % 10 }
}

pub(super) fn timing_afterdot(value: f32) -> i64 {
    let afterdot = ((value.abs() * 100.0) as i64) % 100;
    if value < 0.0 { -afterdot } else { afterdot }
}

pub(super) fn decimal_afterdot(value: f32) -> i64 {
    ((value.abs() * 100.0) as i64) % 100
}

pub(super) fn select_chart_normal_notes(state: &SkinDrawState) -> u32 {
    if state.select_chart_normal_notes > 0 {
        state.select_chart_normal_notes
    } else {
        state.select_total_notes
    }
}

pub(super) fn select_chart_main_bpm(state: &SkinDrawState) -> Option<f32> {
    (state.select_chart_main_bpm > 0.0).then_some(state.select_chart_main_bpm)
}

pub(super) fn current_bp(state: &SkinDrawState) -> u32 {
    if state.result_failed.is_some() {
        return state.result_bp.unwrap_or(state.judge_counts.bad + state.judge_counts.poor);
    }
    state.judge_counts.bad + state.judge_counts.poor
}

pub(super) fn current_cb(state: &SkinDrawState) -> u32 {
    if state.result_failed.is_some() {
        return state.result_cb.unwrap_or(state.judge_counts.bad + state.judge_counts.poor);
    }
    state.judge_counts.bad + state.judge_counts.poor
}

pub(super) fn result_mybest_bp(state: &SkinDrawState) -> Option<u32> {
    if state.result_failed.is_some() { state.previous_best_bp } else { state.best_bp }
}

pub(super) fn result_diff_misscount(state: &SkinDrawState) -> Option<i64> {
    state.result_failed?;
    let previous = result_mybest_bp(state)?;
    Some(i64::from(current_bp(state)) - i64::from(previous))
}

pub(super) fn result_mybest_ex_score(state: &SkinDrawState) -> Option<u32> {
    if state.result_failed.is_some() { state.previous_best_ex_score } else { state.best_ex_score }
}

pub(super) fn result_mybest_clear_index(state: &SkinDrawState) -> Option<i64> {
    if state.result_failed.is_some() {
        state.previous_best_clear_index
    } else {
        state.best_clear_index
    }
}

/// Result 画面の MYBEST 表示用。過去ベストが無い初プレイは 0 (NOPLAY) を返す。
pub(super) fn result_mybest_ex_score_display(state: &SkinDrawState) -> Option<u32> {
    result_mybest_ex_score(state).or_else(|| state.result_failed.is_some().then_some(0))
}

pub(super) fn result_mybest_clear_index_display(state: &SkinDrawState) -> Option<i64> {
    result_mybest_clear_index(state).or_else(|| state.result_failed.is_some().then_some(0))
}

pub(super) fn skin_point_score(state: &SkinDrawState) -> u32 {
    let total_notes = state.total_notes;
    if total_notes == 0 {
        return 0;
    }
    let counts = state.judge_counts;
    let numerator = match state.key_mode {
        KeyMode::K5 | KeyMode::K10 => {
            100_000_u64 * u64::from(counts.pgreat)
                + 100_000_u64 * u64::from(counts.great)
                + 50_000_u64 * u64::from(counts.good)
        }
        KeyMode::K7 | KeyMode::K14 | KeyMode::K4 | KeyMode::K6 | KeyMode::K8 => {
            150_000_u64 * u64::from(counts.pgreat)
                + 100_000_u64 * u64::from(counts.great)
                + 20_000_u64 * u64::from(counts.good)
                + 50_000_u64 * u64::from(state.max_combo)
        }
        KeyMode::K9 => {
            100_000_u64 * u64::from(counts.pgreat)
                + 70_000_u64 * u64::from(counts.great)
                + 40_000_u64 * u64::from(counts.good)
        }
    };
    (numerator / u64::from(total_notes)).min(u64::from(u32::MAX)) as u32
}

pub(super) fn score_rate_cmp_value(ex_score: u32, total_notes: u32) -> u32 {
    if total_notes == 0 { 0 } else { ex_score.saturating_mul(1000) / total_notes.max(1) }
}

/// Returns the graph bar fill ratio (0.0-1.0) for a given `BARGRAPH_*` type.
pub(super) fn graph_value(graph_type: i32, state: &SkinDrawState) -> f32 {
    match graph_type {
        101 => state.play_progress, // BARGRAPH_MUSIC_PROGRESS: elapsed / total playtime
        102 => 1.0,                 // BARGRAPH_LOAD_PROGRESS: always complete during play
        110 => {
            // BARGRAPH_SCORERATE: ex_score / max_ex_score
            let max = (state.total_notes * 2) as f32;
            if max > 0.0 { state.ex_score as f32 / max } else { 0.0 }
        }
        111 => current_score_rate_value(state),
        // BARGRAPH_RATE_PGREAT..RATE_EXSCORE: judge count / past_notes (or total_notes)
        140 => judge_rate(state.judge_counts.pgreat, state.past_notes),
        141 => judge_rate(state.judge_counts.great, state.past_notes),
        142 => judge_rate(state.judge_counts.good, state.past_notes),
        143 => judge_rate(state.judge_counts.bad, state.past_notes),
        144 => judge_rate(state.judge_counts.poor, state.past_notes),
        145 => judge_rate(state.max_combo, state.total_notes),
        146 => {
            // BARGRAPH_RATE_SCORE: (pgreat + great*0.5) / total_notes
            let max = (state.past_notes * 2) as f32;
            if max > 0.0 {
                (state.judge_counts.pgreat * 2 + state.judge_counts.great) as f32 / max
            } else {
                0.0
            }
        }
        147 => {
            // BARGRAPH_RATE_EXSCORE: ex_score so far / (past_notes * 2)
            let notes = if state.select_screen {
                state.select_total_notes.max(state.total_notes)
            } else {
                state.past_notes
            };
            let max = (notes * 2) as f32;
            if max > 0.0 { state.ex_score as f32 / max } else { 0.0 }
        }
        // BARGRAPH_BESTSCORERATE_NOW (112): best score at current progress / max_ex_score.
        // When a beatoraja ghost is available, use its per-note progression instead of a
        // linear projection from the final best score.
        112 => {
            let max = (state.total_notes * 2) as f32;
            if max > 0.0 {
                projected_best_score_at_progress(state).unwrap_or(0) as f32 / max
            } else {
                0.0
            }
        }
        // BARGRAPH_BESTSCORERATE (113): best_ex_score / (total_notes * 2)
        113 => {
            let max = (state.total_notes * 2) as f32;
            if max > 0.0 { result_mybest_ex_score(state).unwrap_or(0) as f32 / max } else { 0.0 }
        }
        // BARGRAPH_TARGETSCORERATE_NOW (114): target_ex_score * past_notes / (total_notes^2 * 2)
        114 => {
            let max = (state.total_notes as f64).powi(2) * 2.0;
            if max > 0.0 {
                (state.target_ex_score.unwrap_or(0) as f64 * state.past_notes as f64 / max) as f32
            } else {
                0.0
            }
        }
        // BARGRAPH_TARGETSCORERATE (115): target_ex_score / (total_notes * 2)
        115 => {
            let max = (state.total_notes * 2) as f32;
            if max > 0.0 { state.target_ex_score.unwrap_or(0) as f32 / max } else { 0.0 }
        }
        -1 => (state.select_clear_index as f32 / 10.0).clamp(0.0, 1.0),
        -2 => {
            let total_notes = state.select_total_notes.max(state.total_notes);
            let max = (total_notes * 2) as f32;
            if max > 0.0 { state.ex_score as f32 / max } else { 0.0 }
        }
        17 => state.select_master_volume.clamp(0.0, 1.0),
        18 => state.select_key_volume.clamp(0.0, 1.0),
        19 => state.select_bgm_volume.clamp(0.0, 1.0),
        // Lua fast/slow 比率 graph (ECFN select 等)
        148 => fast_slow_ratio_fast(state),
        149 => fast_slow_ratio_slow(state),
        _ => 0.0,
    }
}

pub(super) fn graph_raw_value(graph: &SkinGraphDef, state: &SkinDrawState) -> f32 {
    if !graph.value_expr.trim().is_empty() {
        if let Some(value) = skin_builtin_value_f32(&graph.value_expr, state) {
            return value;
        }
        skin_state_float_expr(&graph.value_expr, state).unwrap_or(0.0)
    } else {
        graph_value(graph.graph_type, state)
    }
}

/// Returns (fill multiplier on dst extent, UV clip ratio 0.0-1.0).
pub(super) fn graph_fill_dimensions(graph: &SkinGraphDef, state: &SkinDrawState) -> (f32, f32) {
    let raw = graph_raw_value(graph, state).max(0.0);
    if !graph.value_expr.trim().is_empty() {
        // beatoraja Lua graph: rendered size = dst.w * value (value is pixel multiplier).
        let max = graph.max.max(1) as f32;
        return (raw, (raw / max).clamp(0.0, 1.0));
    }
    if graph.is_ref_num && graph.max > graph.min {
        let ratio = ((raw - graph.min as f32) / (graph.max - graph.min) as f32).clamp(0.0, 1.0);
        return (ratio, ratio);
    }
    let ratio = raw.clamp(0.0, 1.0);
    (ratio, ratio)
}

pub(super) fn skin_grid_cell_size(size: i32, divisions: i32) -> i32 {
    let divisions = divisions.max(1);
    size / divisions
}

pub(super) fn fast_slow_ratio_fast(state: &SkinDrawState) -> f32 {
    let Some(counts) = state.fast_slow_counts else {
        return 0.0;
    };
    let fast = fast_slow_ratio_fast_total(counts);
    let slow = fast_slow_ratio_slow_total(counts);
    let total = fast + slow;
    if total == 0 { 0.0 } else { fast as f32 / total as f32 }
}

pub(super) fn fast_slow_ratio_slow(state: &SkinDrawState) -> f32 {
    let Some(counts) = state.fast_slow_counts else {
        return 0.0;
    };
    let fast = fast_slow_ratio_fast_total(counts);
    let slow = fast_slow_ratio_slow_total(counts);
    let total = fast + slow;
    if total == 0 { 0.0 } else { slow as f32 / total as f32 }
}

pub(super) fn fast_slow_ratio_fast_total(counts: crate::snapshot::FastSlowJudgeCounts) -> u32 {
    counts.fast_pgreat
        + counts.fast_great
        + counts.fast_good
        + counts.fast_bad
        + counts.fast_poor
        + counts.fast_empty_poor
}

pub(super) fn fast_slow_ratio_slow_total(counts: crate::snapshot::FastSlowJudgeCounts) -> u32 {
    counts.slow_pgreat
        + counts.slow_great
        + counts.slow_good
        + counts.slow_bad
        + counts.slow_poor
        + counts.slow_empty_poor
}

pub(super) fn skin_frame_expr_value(expr: SkinFrameExpr, state: &SkinDrawState) -> Option<i32> {
    match expr {
        SkinFrameExpr::FastSlowBreakdownHeight(ref_id) => fast_slow_breakdown_height(ref_id, state),
    }
}

pub(super) fn fast_slow_breakdown_height(ref_id: i32, state: &SkinDrawState) -> Option<i32> {
    const REFS: [i32; 12] = [422, 419, 417, 415, 413, 411, 410, 412, 414, 416, 418, 421];
    if !REFS.contains(&ref_id) {
        return None;
    }
    let values = REFS.map(|candidate| skin_state_number(candidate, state).unwrap_or(0).max(0));
    let max = values.into_iter().max().unwrap_or(0);
    if max <= 0 {
        return Some(0);
    }
    let value = skin_state_number(ref_id, state).unwrap_or(0).max(0);
    Some((value as f32 / max as f32 * 100.0).round() as i32)
}

pub(super) fn judge_rate(count: u32, total: u32) -> f32 {
    if total > 0 { count as f32 / total as f32 } else { 0.0 }
}

pub(super) fn skin_slider_progress(slider: &SkinSliderDef, state: &SkinDrawState) -> Option<f32> {
    if !slider.value_expr.trim().is_empty()
        && let Some(progress) = skin_builtin_value_f32(&slider.value_expr, state)
    {
        return Some(progress.clamp(0.0, 1.0));
    }
    skin_slider_progress_by_type(slider.slider_type, state)
}

pub(super) fn skin_slider_progress_by_type(slider_type: i32, state: &SkinDrawState) -> Option<f32> {
    match slider_type {
        1 => Some(state.select_scroll_progress.clamp(0.0, 1.0)),
        4 | 5 => {
            let lane_cover = bmz_lane_cover_for_lift(state.lane_cover, state.lift);
            (lane_cover > 0.0).then_some(lane_cover)
        }
        6 => Some(state.play_progress.clamp(0.0, 1.0)),
        8 => Some(ir_ranking_scroll_progress(state)),
        17 => Some(state.select_master_volume.clamp(0.0, 1.0)),
        18 => Some(state.select_key_volume.clamp(0.0, 1.0)),
        19 => Some(state.select_bgm_volume.clamp(0.0, 1.0)),
        _ => None,
    }
}

pub(super) fn skin_timer_elapsed_ms(timer: Option<i32>, state: &SkinDrawState) -> Option<i32> {
    match timer {
        None => Some(state.elapsed_ms),
        Some(0) => Some(state.elapsed_ms),
        Some(1) => state.start_input_ms,
        Some(2) => state.fadeout_ms,
        Some(3) => state.failed_ms,
        Some(SKIN_TIMER_BMZ_INPUT_BASE..=SKIN_TIMER_BMZ_INPUT_LAST) => {
            state.logical_input_press_ms[(timer.unwrap() - SKIN_TIMER_BMZ_INPUT_BASE) as usize]
        }
        Some(150) => state.result_graph_begin_ms,
        Some(151) => state.result_graph_end_ms,
        Some(152) => state.result_update_score_ms,
        // TIMER_IR_CONNECT_BEGIN/SUCCESS/FAIL.
        Some(172) => state.ir_ranking.connect_begin_ms,
        Some(173) => state.ir_ranking.connect_success_ms,
        Some(174) => state.ir_ranking.connect_fail_ms,
        Some(40) => state.ready_timer_ms,
        Some(41) => state.play_timer_ms,
        Some(140) => state.rhythm_timer_ms,
        Some(42) => state.gauge_increase_ms,
        Some(43) => state.gauge_increase_2p_ms,
        Some(44) => state.gauge_max_ms,
        Some(45) => state.gauge_max_2p_ms,
        Some(11) => Some(state.select_bar_elapsed_ms),
        Some(21..=26) => (state.select_option_panel == (timer.unwrap() - 20) as u8)
            .then_some(state.select_option_panel_elapsed_ms),
        Some(31..=36) => state.select_option_panel_off_elapsed_ms[(timer.unwrap() - 31) as usize],
        Some(348..=352) => score_target_timer_elapsed_ms(timer.unwrap(), state),
        Some(46) => state.judge_ms[0],
        Some(47) => state.judge_ms[1],
        Some(247) => state.judge_ms[2],
        Some(446) => state.judge_ms[0],
        Some(447) => state.judge_ms[1],
        Some(448) => state.judge_ms[2],
        Some(48) => state.full_combo_ms,
        Some(49) => state.full_combo_2p_ms,
        Some(908) => state.music_end_ms,
        Some(50..=57) => state.bomb_ms[(timer.unwrap() - 50) as usize],
        Some(58..=59) => state.bomb_ms[Lane::Key8.index() + (timer.unwrap() - 58) as usize],
        // 2P bomb: timer 60=Scratch2, 61-67=Key8-14
        Some(60) => state.bomb_ms[Lane::Scratch2.index()],
        Some(61..=67) => state.bomb_ms[Lane::Key8.index() + (timer.unwrap() - 61) as usize],
        // 1P hold: timer 70=Scratch, 71-77=Key1-7
        Some(70..=77) => state.hold_ms[(timer.unwrap() - 70) as usize],
        Some(78..=79) => state.hold_ms[Lane::Key8.index() + (timer.unwrap() - 78) as usize],
        // 2P hold: timer 80=Scratch2, 81-87=Key8-14
        Some(80) => state.hold_ms[Lane::Scratch2.index()],
        Some(81..=87) => state.hold_ms[Lane::Key8.index() + (timer.unwrap() - 81) as usize],
        Some(100..=107) => state.keyon_ms[(timer.unwrap() - 100) as usize],
        Some(108..=109) => state.keyon_ms[Lane::Key8.index() + (timer.unwrap() - 108) as usize],
        // 2P keyon: timer 110=Scratch2, 111-117=Key8-14
        Some(110) => state.keyon_ms[Lane::Scratch2.index()],
        Some(111..=117) => state.keyon_ms[Lane::Key8.index() + (timer.unwrap() - 111) as usize],
        Some(120..=127) => state.keyoff_ms[(timer.unwrap() - 120) as usize],
        Some(128..=129) => state.keyoff_ms[Lane::Key8.index() + (timer.unwrap() - 128) as usize],
        // 2P keyoff: timer 130=Scratch2, 131-137=Key8-14
        Some(130) => state.keyoff_ms[Lane::Scratch2.index()],
        Some(131..=137) => state.keyoff_ms[Lane::Key8.index() + (timer.unwrap() - 131) as usize],
        Some(143) => state.end_of_note_ms,
        Some(144) => state.end_of_note_2p_ms,
        // 1P HCN active: timer 250=Scratch, 251-257=Key1-7
        Some(250..=257) => state.hcn_active_ms[(timer.unwrap() - 250) as usize],
        Some(258..=259) => {
            state.hcn_active_ms[Lane::Key8.index() + (timer.unwrap() - 258) as usize]
        }
        // 2P HCN active: timer 260=Scratch2, 261-267=Key8-14
        Some(260) => state.hcn_active_ms[Lane::Scratch2.index()],
        Some(261..=267) => {
            state.hcn_active_ms[Lane::Key8.index() + (timer.unwrap() - 261) as usize]
        }
        // 1P HCN damage: timer 270=Scratch, 271-277=Key1-7
        Some(270..=277) => state.hcn_damage_ms[(timer.unwrap() - 270) as usize],
        Some(278..=279) => {
            state.hcn_damage_ms[Lane::Key8.index() + (timer.unwrap() - 278) as usize]
        }
        // 2P HCN damage: timer 280=Scratch2, 281-287=Key8-14
        Some(280) => state.hcn_damage_ms[Lane::Scratch2.index()],
        Some(281..=287) => {
            state.hcn_damage_ms[Lane::Key8.index() + (timer.unwrap() - 281) as usize]
        }
        Some(id)
            if (SKIN_DYNAMIC_TIMER_BASE
                ..SKIN_DYNAMIC_TIMER_BASE + SKIN_DYNAMIC_TIMER_COUNT as i32)
                .contains(&id) =>
        {
            let idx = (id - SKIN_DYNAMIC_TIMER_BASE) as usize;
            state.dynamic_timer_ms[idx]
        }
        Some(id) => state.fixed_delay_timer_ms.get(&id).copied(),
    }
}

/// beatoraja の各 scene が TIMER_STARTINPUT を開始する条件と経過時間。
/// `now > skin.input` の厳密な不等号も合わせる。
pub fn skin_start_input_elapsed_ms(elapsed_ms: i32, input_ms: i32) -> Option<i32> {
    (elapsed_ms > input_ms).then_some(elapsed_ms.saturating_sub(input_ms))
}

pub(super) fn skin_text_align(align: i32) -> TextAlign {
    match align {
        1 => TextAlign::Center,
        2 => TextAlign::Right,
        _ => TextAlign::Left,
    }
}

pub(super) fn skin_text_bitmap_size(
    text: &SkinTextDef,
    fonts: &[SkinFontDef],
    skin_height: u32,
    frame_h: i32,
) -> Option<f32> {
    if text.font.is_empty() {
        return None;
    }
    let font_id = text.font.rsplit_once(':').map_or(text.font.as_str(), |(_, id)| id);
    let font = fonts.iter().find(|font| font.id == text.font || font.id == font_id)?;
    let extension = Path::new(&font.path).extension()?.to_str()?;
    if !extension.eq_ignore_ascii_case("fnt") {
        return None;
    }
    let bitmap_size = if text.size > 0 { text.size } else { frame_h.abs().max(1) };
    Some(bitmap_size as f32 / skin_height.max(1) as f32)
}

pub(super) fn skin_text_overflow(overflow: i32) -> TextOverflow {
    match overflow {
        1 => TextOverflow::Shrink,
        2 => TextOverflow::Truncate,
        _ => TextOverflow::Overflow,
    }
}

pub(super) fn skin_text_shadow(
    text: &SkinTextDef,
    skin_width: u32,
    skin_height: u32,
) -> Option<TextShadow> {
    let color = skin_hex_color(&text.shadow_color)?;
    if color.a <= 0.0 {
        return None;
    }
    Some(TextShadow {
        color,
        offset: Point {
            x: text.shadow_offset_x / skin_width.max(1) as f32,
            y: text.shadow_offset_y / skin_height.max(1) as f32,
        },
    })
}

pub(super) fn skin_text_outline(text: &SkinTextDef, skin_height: u32) -> Option<TextOutline> {
    if text.outline_width <= 0.0 {
        return None;
    }
    let color = skin_hex_color(&text.outline_color)?;
    if color.a <= 0.0 {
        return None;
    }
    Some(TextOutline { color, width: text.outline_width / skin_height.max(1) as f32 })
}

pub(super) fn skin_hex_color(value: &str) -> Option<Color> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    let a =
        if hex.len() == 8 { u8::from_str_radix(&hex[6..8], 16).ok()? as f32 / 255.0 } else { 1.0 };
    Some(Color::rgba(r, g, b, a))
}

pub(super) fn skin_panel_render_items(
    panel: &SkinPanelDef,
    destination: &SkinDestinationDef,
    frame: ResolvedSkinFrame,
    canvas_width: u32,
    canvas_height: u32,
) -> Vec<SkinRenderItem> {
    let rect = normalize_skin_frame_rect(frame, canvas_width, canvas_height);
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return Vec::new();
    }

    let blend = if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal };
    let tint = |value: &str| {
        let color = skin_hex_color(value)?;
        Some(Color::rgba(
            color.r * frame.r.clamp(0, 255) as f32 / 255.0,
            color.g * frame.g.clamp(0, 255) as f32 / 255.0,
            color.b * frame.b.clamp(0, 255) as f32 / 255.0,
            color.a * frame.a.clamp(0, 255) as f32 / 255.0,
        ))
    };

    let mut items = Vec::with_capacity(5);
    if let Some(color) = tint(&panel.color)
        && color.a > 0.0
    {
        items.push(SkinRenderItem::Rect { rect, color, blend });
    }

    let Some(border_color) = tint(&panel.border_color).filter(|color| color.a > 0.0) else {
        return items;
    };
    if panel.border_width <= 0.0 {
        return items;
    }
    let border_x = (panel.border_width / canvas_width.max(1) as f32).min(rect.width * 0.5);
    let border_y = (panel.border_width / canvas_height.max(1) as f32).min(rect.height * 0.5);
    if border_x <= 0.0 || border_y <= 0.0 {
        return items;
    }
    items.extend([
        SkinRenderItem::Rect {
            rect: Rect { height: border_y, ..rect },
            color: border_color,
            blend,
        },
        SkinRenderItem::Rect {
            rect: Rect { y: rect.y + rect.height - border_y, height: border_y, ..rect },
            color: border_color,
            blend,
        },
        SkinRenderItem::Rect {
            rect: Rect {
                y: rect.y + border_y,
                width: border_x,
                height: (rect.height - border_y * 2.0).max(0.0),
                ..rect
            },
            color: border_color,
            blend,
        },
        SkinRenderItem::Rect {
            rect: Rect {
                x: rect.x + rect.width - border_x,
                y: rect.y + border_y,
                width: border_x,
                height: (rect.height - border_y * 2.0).max(0.0),
            },
            color: border_color,
            blend,
        },
    ]);
    items
}

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

/// Rm-skin `text id="table"` と beatoraja `TEXT_TABLE1..3` (1001..1003) の表示ロジック。
pub fn format_rm_skin_course_table_text(
    course_stage: Option<CourseStageMarker>,
    primary: &str,
    secondary: &str,
    fallback: &str,
) -> String {
    if let Some(stage) = course_stage {
        return match stage {
            CourseStageMarker::Final => "COURSE : STAGE FINAL".to_string(),
            CourseStageMarker::Stage1 => "COURSE : STAGE 1".to_string(),
            CourseStageMarker::Stage2 => "COURSE : STAGE 2".to_string(),
            CourseStageMarker::Stage3 => "COURSE : STAGE 3".to_string(),
            CourseStageMarker::Stage4 => "COURSE : STAGE 4".to_string(),
        };
    }

    // Lua: `not tx1 or tx1 == "" and not tx2 or tx2 == ""`
    let use_fallback = secondary.is_empty() || (primary.is_empty() && secondary.is_empty());
    if use_fallback {
        if fallback.is_empty() {
            return "# No-Table".to_string();
        }
        return fallback.to_string();
    }

    if primary.is_empty() { format!(" > {secondary}") } else { format!("{primary} > {secondary}") }
}

#[cfg(test)]
pub(super) fn skin_state_text(text: &SkinTextDef, state: &SkinTextState<'_>) -> String {
    skin_state_text_with_draw_state(text, None, state)
}

pub(super) fn skin_state_text_with_draw_state(
    text: &SkinTextDef,
    draw_state: Option<&SkinDrawState>,
    state: &SkinTextState<'_>,
) -> String {
    if let Some(draw_state) = draw_state
        && let Some(value) = m_select_daily_stats_text(&text.id, &draw_state.player_stats.daily)
    {
        return value;
    }
    if text.value_expr.trim() == "bmz:text_concat:1001:1002" {
        return format!("{} {}", state.table_text_primary, state.table_level);
    }
    if text.value_expr.trim() == SKIN_EXPR_RESULT_TABLE_TITLE {
        return format!(
            "{} {} {}",
            state.table_level,
            state.table_text_primary,
            full_label(state.title, state.subtitle)
        );
    }
    if !text.constant_text.is_empty() {
        return text.constant_text.clone();
    }
    if let Some(ref_id) = text.number_ref {
        let Some(value) = draw_state.and_then(|state| skin_state_number(ref_id, state)) else {
            return String::new();
        };
        return format!("{}{}{}", text.prefix, value, text.suffix);
    }
    if let Some(region) = text.judge_region {
        let Some(state) = draw_state else {
            return String::new();
        };
        let Some(value) = skin_judge_region_text(state, region) else {
            return String::new();
        };
        return format!("{}{}{}", text.prefix, value, text.suffix);
    }
    if let Some(region) = text.judge_timing_region {
        let Some(state) = draw_state else {
            return String::new();
        };
        let Some(value) = skin_judge_timing_text(state, region) else {
            return String::new();
        };
        return format!("{}{}{}", text.prefix, value, text.suffix);
    }
    if text.value_expr.trim() == SKIN_EXPR_COURSE_TABLE_TEXT {
        return format_rm_skin_course_table_text(
            state.course_stage,
            state.table_text_primary,
            state.table_text_secondary,
            state.table_text_fallback,
        );
    }
    if text.id == "table" {
        return format_rm_skin_course_table_text(
            state.course_stage,
            state.table_text_primary,
            state.table_text_secondary,
            state.table_text_fallback,
        );
    }
    if text.id.contains("bartext") {
        return state.bar_text.to_string();
    }
    if text.id == "table_level" {
        return state.table_level.to_string();
    }
    if text.id == "difficulty" || text.id == "difficulty_name" {
        return state.difficulty_name.to_string();
    }
    if text.id == "level" || text.id == "play_level" {
        return state.play_level.to_string();
    }
    if matches!(text.id.as_str(), "grade_diff" | "gradediff" | "dj_level_diff") {
        return state.grade_diff.to_string();
    }
    match text.id.as_str() {
        "bmz_select_arrange" => return state.select_arrange.to_string(),
        "bmz_select_arrange_2p" => return state.select_arrange_2p.to_string(),
        "bmz_select_target" => return select_target_name(state.target),
        "bmz_select_gauge" => return state.select_gauge.to_string(),
        "bmz_select_gauge_auto_shift" => return state.select_gauge_auto_shift.to_string(),
        "bmz_select_bottom_shiftable_gauge" => {
            return state.select_bottom_shiftable_gauge.to_string();
        }
        "bmz_select_double_option" => return state.select_double_option.to_string(),
        "bmz_select_hs_fix" => return state.select_hs_fix.to_string(),
        "bmz_select_assist" => return state.select_assist.to_string(),
        "bmz_select_mode" => return state.select_mode.to_string(),
        "bmz_select_sort" => return state.select_sort.to_string(),
        "bmz_select_ln_mode" => return state.select_ln_mode.to_string(),
        "bmz_select_bga" => return state.select_bga.to_string(),
        "bmz_select_judge_timing_auto_adjust" => {
            return state.select_judge_timing_auto_adjust.to_string();
        }
        _ => {}
    }
    skin_main_state_text(text.ref_id, draw_state, state)
}

pub(super) fn skin_main_state_text(
    ref_id: i32,
    draw_state: Option<&SkinDrawState>,
    state: &SkinTextState<'_>,
) -> String {
    match ref_id {
        1 => {
            if state.rival.is_empty() {
                select_play_target_name(state.target)
            } else {
                state.rival.to_string()
            }
        }
        2 => state.player_name.to_string(),
        3 => select_target_name(state.target),
        10 => state.title.to_string(),
        11 => state.subtitle.to_string(),
        12 => full_label(state.title, state.subtitle),
        13 => state.genre.to_string(),
        14 => state.artist.to_string(),
        15 => state.subartist.to_string(),
        16 => full_label(state.artist, state.subartist),
        17 => state.table_level.to_string(),
        30 => state.search_word.to_string(),
        120..=129 => ir_ranking_entry(state.ir_ranking, ref_id - 120)
            .map(|entry| entry.player_name.as_str().to_string())
            .unwrap_or_default(),
        150..=159 => state.course_titles[(ref_id - 150) as usize].to_string(),
        SKIN_REF_BMZ_RESULT_IR_SCOPE => {
            draw_state.map(|state| state.ir_ranking.scope.label().to_string()).unwrap_or_default()
        }
        SKIN_TEXT_BMZ_DAILY_RANK => draw_state
            .map(|state| daily_rank_label(&state.player_stats.daily).to_string())
            .unwrap_or_default(),
        SKIN_TEXT_BMZ_DAILY_RECENT_BASE..=SKIN_TEXT_BMZ_DAILY_RECENT_LAST => draw_state
            .map(|state| {
                state.player_stats.daily.recent_titles
                    [(ref_id - SKIN_TEXT_BMZ_DAILY_RECENT_BASE) as usize]
                    .clone()
            })
            .unwrap_or_default(),
        1900 => draw_state
            .map(|state| {
                if skin_hispeed_mode_is_floating(state) { "FHS" } else { "NHS" }.to_string()
            })
            .unwrap_or_default(),
        // beatoraja StringPropertyFactory: 1001=tablename, 1002=tablelevel,
        // 1003=tablefull.  Rm-skin's combined table label is handled above by
        // id/value_expr, so direct numeric refs follow the beatoraja mapping.
        1001 => state.table_text_primary.to_string(),
        1002 => state.table_level.to_string(),
        1003 => state.table_text_fallback.to_string(),
        1010 => format!("bmz-player {}", env!("CARGO_PKG_VERSION")),
        1020 => {
            if matches!(state.ir_ranking.state, crate::scene::ResultIrState::Offline) {
                String::new()
            } else {
                state.ir_ranking.provider_name.as_str().to_string()
            }
        }
        1021 => state.ir_ranking.user_name.as_str().to_string(),
        200..=209 => select_target_name_by_offset(state.target, ref_id - 210),
        210..=219 => select_target_name_by_offset(state.target, ref_id - 209),
        1000 => state.current_folder.to_string(),
        _ => String::new(),
    }
}

pub(super) fn lua_main_state_text_values(
    draw_state: &SkinDrawState,
    text_state: &SkinTextState<'_>,
) -> BTreeMap<i32, String> {
    let mut refs = vec![
        1,
        2,
        3,
        10,
        11,
        12,
        13,
        14,
        15,
        16,
        17,
        30,
        1000,
        1001,
        1002,
        1003,
        1010,
        1020,
        1021,
        1900,
        SKIN_TEXT_BMZ_DAILY_RANK,
    ];
    refs.extend(120..=129);
    refs.extend(150..=159);
    refs.extend(200..=219);
    refs.extend(SKIN_TEXT_BMZ_DAILY_RECENT_BASE..=SKIN_TEXT_BMZ_DAILY_RECENT_LAST);
    refs.into_iter()
        .map(|ref_id| (ref_id, skin_main_state_text(ref_id, Some(draw_state), text_state)))
        .collect()
}

pub fn lua_main_state_option(
    option_id: i32,
    enabled_options: &[i32],
    state: &SkinDrawState,
) -> bool {
    test_skin_op(option_id, enabled_options, state)
}

pub fn lua_main_state_number(ref_id: i32, state: &SkinDrawState) -> i64 {
    skin_state_number(ref_id, state).unwrap_or_default()
}

pub fn lua_main_state_float(ref_id: i32, state: &SkinDrawState) -> f64 {
    f64::from(skin_state_float_number(ref_id, state).unwrap_or_default())
}

pub fn lua_main_state_timer(timer_id: i32, state: &SkinDrawState) -> Option<i32> {
    skin_timer_elapsed_ms(Some(timer_id), state)
}

pub fn lua_main_state_event_index(event_id: i32, state: &SkinDrawState) -> i32 {
    skin_state_event_index(event_id, state)
}

pub(super) const SELECT_TARGET_IDS: [&str; 13] = [
    "NONE",
    "RANK_A",
    "RANK_AA-",
    "RANK_AA",
    "RANK_AAA-",
    "RANK_AAA",
    "RANK_MAX-",
    "MAX",
    "RANK_NEXT",
    "IR_TOP",
    "IR_NEXT",
    "RIVAL TOP",
    "RIVAL NEXT",
];
pub(super) const SELECT_TARGET_NAMES: [&str; 13] = [
    "NO TARGET",
    "RANK A",
    "RANK AA-",
    "RANK AA",
    "RANK AAA-",
    "RANK AAA",
    "RANK MAX-",
    "MAX",
    "NEXT RANK",
    "IR TOP",
    "IR NEXT",
    "RIVAL TOP",
    "RIVAL NEXT",
];
