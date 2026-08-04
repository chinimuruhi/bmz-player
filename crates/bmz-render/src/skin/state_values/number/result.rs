use super::*;

pub(in crate::skin) fn ir_ranking_entry(
    ranking: &crate::scene::ResultIrSnapshot,
    index: i32,
) -> Option<crate::scene::ResultIrRankingEntrySnapshot> {
    ranking.entries.get(usize::try_from(index).ok()?).copied().filter(|entry| {
        entry.rank.is_some() || entry.ex_score.is_some() || !entry.player_name.as_str().is_empty()
    })
}

pub(in crate::skin) fn ir_ranking_score_and_max(
    state: &SkinDrawState,
    slot: i32,
) -> Option<(i64, i64)> {
    if !(1..=10).contains(&slot) {
        return None;
    }
    let score = ir_ranking_entry(&state.ir_ranking, slot - 1)?.ex_score?;
    let max_score = i64::from(state.select_total_notes.max(state.total_notes).checked_mul(2)?);
    Some((score, max_score))
}

pub(in crate::skin) fn ir_ranking_score_rate_parts(
    state: &SkinDrawState,
    slot: i32,
) -> Option<(i64, i64)> {
    let (score, max_score) = ir_ranking_score_and_max(state, slot)?;
    if score <= 0 || max_score <= 0 {
        return Some((0, 0));
    }
    let scaled = i128::from(score).checked_mul(10_000)?.checked_div(i128::from(max_score))?;
    Some((i64::try_from(scaled / 100).ok()?, i64::try_from(scaled % 100).ok()?))
}

pub(in crate::skin) fn ir_total_clear_count(
    ranking: &crate::scene::ResultIrSnapshot,
) -> Option<i64> {
    let total = ranking.total_player?;
    let clear_rate = ranking.clear_rate?;
    Some((total * clear_rate + 50) / 100)
}

pub(in crate::skin) fn result_grade_diff_number(state: &SkinDrawState) -> Option<i64> {
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

pub(in crate::skin) fn grade_diff_score_available(state: &SkinDrawState) -> bool {
    !state.select_screen
        || state.select_play_count > 0
        || state.select_ex_score.is_some_and(|score| score > 0)
}

pub(in crate::skin) fn next_rank_diff(state: &SkinDrawState) -> Option<i64> {
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

/// Computes the forward difference used by WMII PLAY's Lua `next_rank_info`.
/// It differs from BMZ's generic nearest-rank display because WMII always
/// targets the next higher boundary and has a separate MAX- boundary.
pub(in crate::skin) fn wmii_next_rank_diff(state: &SkinDrawState) -> Option<i64> {
    let ex_score = i64::from(state.ex_score);
    let total_notes = i64::from(state.total_notes);
    let max_score = total_notes.checked_mul(2)?;
    if max_score <= 0 {
        return None;
    }
    let ex_score = ex_score.clamp(0, max_score);
    for (numerator, denominator) in [
        (6_i64, 27_i64),
        (9, 27),
        (12, 27),
        (15, 27),
        (18, 27),
        (21, 27),
        (24, 27),
        (17, 18),
        (1, 1),
    ] {
        let threshold = div_ceil(max_score * numerator, denominator);
        if ex_score < threshold {
            return Some(threshold - ex_score);
        }
    }
    Some(0)
}

pub(in crate::skin) fn wmii_next_rank_stage(state: &SkinDrawState) -> Option<i32> {
    let ex_score = i64::from(state.ex_score);
    let total_notes = i64::from(state.total_notes);
    let max_score = total_notes.checked_mul(2)?;
    if max_score <= 0 {
        return None;
    }
    let ex_score = ex_score.clamp(0, max_score);
    for (numerator, denominator, stage) in [
        (6_i64, 27_i64, 7_i32),
        (9, 27, 6),
        (12, 27, 5),
        (15, 27, 4),
        (18, 27, 3),
        (21, 27, 2),
        (24, 27, 1),
        (17, 18, 8),
        (1, 1, 0),
    ] {
        let threshold = div_ceil(max_score * numerator, denominator);
        if ex_score < threshold {
            return Some(stage);
        }
    }
    Some(0)
}

pub(in crate::skin) fn next_rank_grade(state: &SkinDrawState) -> Option<&'static str> {
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

pub(in crate::skin) fn next_rank_grade_for_step(rank_step: i32) -> Option<&'static str> {
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
pub(in crate::skin) struct NearestGradeDiff {
    pub(in crate::skin) grade: &'static str,
    pub(in crate::skin) value: i64,
}

impl NearestGradeDiff {
    fn label(self) -> String {
        format!("{}{:+}", self.grade, self.value)
    }
}

pub(in crate::skin) fn nearest_grade_diff(state: &SkinDrawState) -> Option<NearestGradeDiff> {
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

pub(in crate::skin) fn nearest_grade_diff_for_state(
    state: &SkinDrawState,
) -> Option<NearestGradeDiff> {
    if state.result_grade_diff_f_fallback_to_e {
        return nearest_grade_diff_for_destination(state, false);
    }
    nearest_grade_diff(state)
}

pub(in crate::skin) fn projected_score_at_progress(final_score: u32, state: &SkinDrawState) -> u32 {
    if state.total_notes == 0 {
        return final_score;
    }
    let past_notes = state.past_notes.min(state.total_notes);
    ((final_score as u64 * past_notes as u64) / state.total_notes as u64) as u32
}

pub(in crate::skin) fn result_or_select_length_ms(state: &SkinDrawState) -> i64 {
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

pub(in crate::skin) fn projected_best_score_at_progress(state: &SkinDrawState) -> Option<u32> {
    state.projected_best_ex_score.or_else(|| {
        result_mybest_ex_score(state)
            .map(|score| projected_score_at_progress(score, state))
            .or_else(|| state.result_failed.is_some().then_some(0))
    })
}

pub(in crate::skin) fn div_ceil(numerator: i64, denominator: i64) -> i64 {
    if denominator <= 0 {
        return 0;
    }
    numerator.div_euclid(denominator) + i64::from(numerator.rem_euclid(denominator) != 0)
}

pub(in crate::skin) fn rank_threshold(max_score: u32, rank_step: u32) -> u32 {
    div_ceil(rank_step as i64 * max_score as i64, 27).clamp(0, u32::MAX as i64) as u32
}

pub(in crate::skin) fn judge_rank_option_matches(op: i32, judge_rank: Option<i32>) -> bool {
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

pub(in crate::skin) fn judge_rate_int(count: u32, total_notes: u32) -> Option<i64> {
    if total_notes == 0 {
        return None;
    }
    Some(count as i64 * 100 / total_notes as i64)
}

pub(in crate::skin) fn poor_plus_miss(counts: DisplayJudgeCounts) -> u32 {
    counts.poor.saturating_add(counts.empty_poor)
}

pub(in crate::skin) fn bad_plus_poor_plus_miss(counts: DisplayJudgeCounts) -> u32 {
    counts.bad.saturating_add(poor_plus_miss(counts))
}

pub(in crate::skin) fn score_rate_parts(ex_score: u32, total_notes: u32) -> (u32, u32) {
    if total_notes == 0 {
        return (0, 0);
    }
    // beatoraja ScoreDataProperty: rateInt=(int)(rate*100), rateAfterDot=((int)(rate*10000))%100
    let rate_scaled = ex_score.saturating_mul(10000) / total_notes.saturating_mul(2).max(1);
    (rate_scaled / 100, rate_scaled % 100)
}

pub(in crate::skin) fn current_score_rate_notes(state: &SkinDrawState) -> Option<u32> {
    if state.past_notes > 0 {
        Some(state.past_notes)
    } else if state.total_notes > 0 || state.select_total_notes > 0 {
        Some(0)
    } else {
        None
    }
}

pub(in crate::skin) fn current_score_rate_value(state: &SkinDrawState) -> f32 {
    match current_score_rate_notes(state) {
        Some(0) => 1.0,
        Some(notes) => state.ex_score as f32 / notes.saturating_mul(2).max(1) as f32,
        None => 0.0,
    }
}

pub(in crate::skin) fn current_score_rate_parts(state: &SkinDrawState) -> (u32, u32) {
    match current_score_rate_notes(state) {
        Some(0) => (100, 0),
        Some(notes) => score_rate_parts(state.ex_score, notes),
        None => (0, 0),
    }
}
