use super::*;

pub(super) fn best_rank_op_matches(op: i32, state: &SkinDrawState) -> bool {
    if state.in_settings {
        return false;
    }
    let Some(rank) = rank_index(result_mybest_ex_score(state), state.total_notes) else {
        return false;
    };
    op == 320 + rank as i32
}

/// 現在のランク判定の基準値 (ex_score, notes)。
/// Play 画面では beatoraja の `qualifyNowRank` と同じく past notes を分母にする。
pub(super) fn current_rank_inputs(state: &SkinDrawState) -> (Option<u32>, u32) {
    if state.result_failed.is_some() {
        (Some(state.ex_score), state.total_notes)
    } else if state.select_screen {
        (state.select_ex_score, state.select_total_notes)
    } else if let Some(notes) = current_score_rate_notes(state) {
        (Some(state.ex_score), notes)
    } else {
        (Some(state.ex_score), state.total_notes)
    }
}

pub(super) fn current_rank_index(state: &SkinDrawState) -> Option<usize> {
    let (ex_score, total_notes) = current_rank_inputs(state);
    if !state.select_screen
        && state.result_failed.is_none()
        && total_notes == 0
        && current_score_rate_notes(state) == Some(0)
    {
        return rank_index(Some(2), 1);
    }
    rank_index(ex_score, total_notes)
}

pub(super) fn rank_index(ex_score: Option<u32>, total_notes: u32) -> Option<usize> {
    let ex_score = ex_score?;
    let max_score = total_notes.saturating_mul(2);
    if max_score == 0 {
        return None;
    }
    let score = ex_score.min(max_score) as u64;
    let max = max_score as u64;
    let rank = if score * 9 >= max * 8 {
        0
    } else if score * 9 >= max * 7 {
        1
    } else if score * 9 >= max * 6 {
        2
    } else if score * 9 >= max * 5 {
        3
    } else if score * 9 >= max * 4 {
        4
    } else if score * 9 >= max * 3 {
        5
    } else if score * 9 >= max * 2 {
        6
    } else {
        7
    };
    Some(rank)
}
