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
