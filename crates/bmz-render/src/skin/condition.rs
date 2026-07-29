use super::*;

pub(super) fn eval_skin_draw_condition(condition: &str, state: &SkinDrawState) -> bool {
    let condition = condition.trim();
    if condition.is_empty() {
        return true;
    }
    if let Some(callback_id) =
        condition.strip_prefix(LUA_DRAW_CALLBACK_PREFIX).and_then(|id| id.parse::<usize>().ok())
    {
        let Some(runtime) = state.lua_runtime.as_ref() else {
            return false;
        };
        return runtime.runtime.evaluate_draw(
            callback_id,
            state,
            &runtime.enabled_options,
            &runtime.text_values,
        );
    }

    condition.split("||").flat_map(|segment| segment.split(" or ")).any(|branch| {
        branch
            .split("&&")
            .flat_map(|segment| segment.split(" and "))
            .all(|term| eval_skin_draw_term(term.trim(), state).unwrap_or(false))
    })
}

pub(super) fn eval_skin_draw_term(term: &str, state: &SkinDrawState) -> Option<bool> {
    if let Some(flag_id) = parse_runtime_flag_operand(term) {
        return Some(*state.runtime_flags.get(&flag_id).unwrap_or(&false));
    }
    if let Some(flag_id) = term.strip_prefix("not ").and_then(parse_runtime_flag_operand) {
        return Some(!state.runtime_flags.get(&flag_id).copied().unwrap_or(false));
    }
    if term == "select_score_available()" {
        return Some(
            state.select_screen
                && select_score_metadata_available(state)
                && state.select_ex_score.is_some(),
        );
    }
    if let Some(panel) = parse_result_panel_predicate(term) {
        return state.result_panel.map(|current| current == panel);
    }
    if let Some((slot, lower, upper)) = parse_ir_score_rate_band_predicate(term) {
        let score = ir_ranking_entry(&state.ir_ranking, slot - 1)?.ex_score?;
        let max_score =
            i64::from(state.select_total_notes.max(state.total_notes)).checked_mul(2)?;
        if max_score <= 0 {
            return Some(false);
        }
        let score = score.clamp(0, max_score);
        return Some(score * 9 >= max_score * lower && score * 9 < max_score * upper);
    }
    if let Some((slot, lower, upper)) = parse_ir_score_rate_range_predicate(term) {
        let score = ir_ranking_entry(&state.ir_ranking, slot - 1)?.ex_score?;
        let max_score =
            i64::from(state.select_total_notes.max(state.total_notes)).checked_mul(2)?;
        if max_score <= 0 {
            return Some(false);
        }
        let score = score.clamp(0, max_score);
        return Some(score * 1000 > max_score * lower && score * 1000 <= max_score * upper);
    }
    if let Some(slot) = parse_ir_ranking_user_predicate(term) {
        let entry = ir_ranking_entry(&state.ir_ranking, slot - 1)?;
        let user_name = state.ir_ranking.user_name.as_str();
        return Some(!user_name.is_empty() && entry.player_name.as_str() == user_name);
    }
    if let Some((lower, upper)) = parse_score_rate_band_predicate(term) {
        let score = i64::from(state.select_ex_score.unwrap_or(state.ex_score));
        let total_notes = i64::from(state.select_total_notes.max(state.total_notes));
        let max_score = total_notes.checked_mul(2)?;
        if max_score <= 0 {
            return Some(false);
        }
        let score = score.clamp(0, max_score);
        return Some(score * 9 >= max_score * lower && score * 9 < max_score * upper);
    }
    if let Some((grade, sign)) = parse_nearest_rank_predicate(term) {
        let diff = nearest_grade_diff(state)?;
        return Some(diff.grade == grade && nearest_rank_sign_matches(diff.value, sign));
    }
    if let Some(sign) = parse_nearest_rank_sign_predicate(term) {
        return nearest_grade_diff(state).map(|diff| nearest_rank_sign_matches(diff.value, sign));
    }
    if let Some((mode, digits)) = parse_gauge_value_digits_predicate(term) {
        let value = match mode {
            "percent" => clamped_gauge_value(state) * 100.0 / state.gauge_max.max(1.0),
            "amount" => clamped_gauge_value(state),
            _ => return None,
        };
        let actual_digits = if value.floor() >= 100.0 {
            3
        } else if value.floor() >= 10.0 {
            2
        } else {
            1
        };
        return Some(actual_digits == digits);
    }
    if let Some((group, part, below_border)) = parse_gauge_lead_glow_predicate(term) {
        return Some(eval_gauge_lead_glow_predicate(group, part, below_border, state));
    }
    if let Some((graph_kind, lane, slot, kind)) = parse_keylogger_draw_predicate(term) {
        let actual = match graph_kind {
            "judge" => state.keylogger_event_judge.get(lane)?.get(slot).copied()?,
            "fastslow" => state.keylogger_event_fast_slow.get(lane)?.get(slot).copied()?,
            _ => return None,
        };
        return Some(actual == kind);
    }
    if let Some(option_id) = parse_skin_option_operand(term) {
        return Some(test_skin_op(option_id, &[], state));
    }
    if let Some(option_id) = term.strip_prefix('!').and_then(parse_skin_option_operand) {
        return Some(!test_skin_op(option_id, &[], state));
    }
    let operators = [">=", "<=", "==", "!=", ">", "<"];
    for operator in operators {
        let Some(index) = term.find(operator) else {
            continue;
        };
        let left = term[..index].trim();
        let right = term[index + operator.len()..].trim();
        let left = eval_skin_draw_operand(left, state)?;
        let right = eval_skin_draw_operand(right, state)?;
        return Some(match operator {
            ">=" => left >= right,
            "<=" => left <= right,
            "==" => (left - right).abs() < f32::EPSILON,
            "!=" => (left - right).abs() >= f32::EPSILON,
            ">" => left > right,
            "<" => left < right,
            _ => false,
        });
    }
    None
}

pub(super) fn parse_result_panel_predicate(term: &str) -> Option<i32> {
    let panel = term.strip_prefix("result_panel(")?.strip_suffix(')')?.trim().parse().ok()?;
    (0..=2).contains(&panel).then_some(panel)
}

pub(super) fn parse_score_rate_band_predicate(term: &str) -> Option<(i64, i64)> {
    let inner = term.strip_prefix("score_rate_band(")?.strip_suffix(')')?;
    let (lower, upper) = inner.split_once(',')?;
    let lower = lower.trim().parse::<i64>().ok()?;
    let upper = upper.trim().parse::<i64>().ok()?;
    (0 <= lower && lower < upper && upper <= 10).then_some((lower, upper))
}

pub(super) fn parse_ir_score_rate_band_predicate(term: &str) -> Option<(i32, i64, i64)> {
    let inner = term.strip_prefix("ir_score_rate_band(")?.strip_suffix(')')?;
    let mut args = inner.split(',').map(str::trim);
    let slot = args.next()?.parse::<i32>().ok()?;
    let lower = args.next()?.parse::<i64>().ok()?;
    let upper = args.next()?.parse::<i64>().ok()?;
    if args.next().is_some()
        || !(1..=10).contains(&slot)
        || !(0 <= lower && lower < upper && upper <= 10)
    {
        return None;
    }
    Some((slot, lower, upper))
}

pub(super) fn parse_ir_score_rate_range_predicate(term: &str) -> Option<(i32, i64, i64)> {
    let inner = term.strip_prefix("ir_score_rate_range(")?.strip_suffix(')')?;
    let mut args = inner.split(',').map(str::trim);
    let slot = args.next()?.parse::<i32>().ok()?;
    let lower = args.next()?.parse::<i64>().ok()?;
    let upper = args.next()?.parse::<i64>().ok()?;
    if args.next().is_some()
        || !(1..=10).contains(&slot)
        || !(-10 <= lower && lower < upper && upper <= 1000)
    {
        return None;
    }
    Some((slot, lower, upper))
}

pub(super) fn parse_ir_ranking_user_predicate(term: &str) -> Option<i32> {
    let slot = term.strip_prefix("ir_ranking_user(")?.strip_suffix(')')?.trim().parse().ok()?;
    (1..=10).contains(&slot).then_some(slot)
}

pub(super) fn parse_nearest_rank_predicate(term: &str) -> Option<(&str, &str)> {
    let inner = term.strip_prefix("nearest_rank(")?.strip_suffix(')')?;
    let (grade, sign) = inner.split_once(',')?;
    let grade = grade.trim();
    let sign = sign.trim();
    (matches!(grade, "F" | "E" | "D" | "C" | "B" | "A" | "AA" | "AAA" | "MAX")
        && matches!(sign, "plus" | "minus"))
    .then_some((grade, sign))
}

pub(super) fn parse_nearest_rank_sign_predicate(term: &str) -> Option<&str> {
    let sign = term.strip_prefix("nearest_rank_sign(")?.strip_suffix(')')?.trim();
    matches!(sign, "plus" | "minus").then_some(sign)
}

pub(super) fn nearest_rank_sign_matches(value: i64, sign: &str) -> bool {
    match sign {
        "plus" => value >= 0,
        "minus" => value < 0,
        _ => false,
    }
}

pub(super) fn parse_gauge_value_digits_predicate(term: &str) -> Option<(&str, usize)> {
    let inner = term.strip_prefix("gauge_value_digits(")?.strip_suffix(')')?;
    let (mode, digits) = inner.split_once(',')?;
    let mode = mode.trim();
    let digits = digits.trim().parse::<usize>().ok()?;
    (matches!(mode, "percent" | "amount") && (1..=3).contains(&digits)).then_some((mode, digits))
}

pub(super) fn parse_gauge_lead_glow_predicate(term: &str) -> Option<(&str, i32, bool)> {
    let inner = term.strip_prefix("gauge_lead_glow(")?.strip_suffix(')')?;
    let mut args = inner.split(',').map(str::trim);
    let group = args.next()?;
    let part = args.next()?.parse::<i32>().ok()?;
    let below_border = match args.next()? {
        "above" => false,
        "below" => true,
        _ => return None,
    };
    if args.next().is_some()
        || !(1..=24).contains(&part)
        || !matches!(group, "assist_easy" | "easy" | "groove" | "hard" | "exhard" | "hazard")
    {
        return None;
    }
    Some((group, part, below_border))
}

pub(super) fn eval_gauge_lead_glow_predicate(
    group: &str,
    part: i32,
    below_border: bool,
    state: &SkinDrawState,
) -> bool {
    let actual_group = match state.gauge_type {
        0 => "assist_easy",
        1 => "easy",
        2 => "groove",
        3 | 6 => "hard",
        4 | 7 => "exhard",
        5 | 8 => "hazard",
        _ => "groove",
    };
    if actual_group != group || state.gauge <= 0.0 {
        return false;
    }
    let max = state.gauge_max.max(1.0);
    let border = match (max > 100.0, state.gauge_type) {
        (true, 0) => 65.0,
        (true, 1 | 2) => 85.0,
        (false, 0) => 60.0,
        (false, 1 | 2) => 80.0,
        _ => 0.0,
    };
    let lead = ((state.gauge * 24.0 / max).floor() as i32).clamp(1, 24);
    lead == part && ((part as f32 * max / 24.0) < border) == below_border
}

pub(super) fn parse_keylogger_draw_predicate(term: &str) -> Option<(&str, usize, usize, u8)> {
    let inner = term.strip_prefix("keylogger_")?.strip_suffix(')')?;
    let (graph_kind, args) = inner.split_once('(')?;
    let mut args = args.split(',');
    let lane = args.next()?.trim().parse::<usize>().ok()?.checked_sub(1)?;
    let slot = args.next()?.trim().parse::<usize>().ok()?.checked_sub(1)?;
    let kind_name = args.next()?.trim();
    if args.next().is_some() || lane >= LANE_COUNT || slot >= 16 {
        return None;
    }
    let kind = match (graph_kind, kind_name) {
        ("judge", "none") | ("fastslow", "none") => 0,
        ("judge", "cool") | ("fastslow", "cool") => 1,
        ("judge", "great") | ("fastslow", "fast") => 2,
        ("judge", "good") | ("fastslow", "slow") => 3,
        ("judge", "bad") => 4,
        _ => return None,
    };
    Some((graph_kind, lane, slot, kind))
}

pub(super) fn destination_timer_elapsed_ms(
    destination: &SkinDestinationDef,
    state: &SkinDrawState,
) -> Option<i32> {
    if let Some(rest) = destination.timer_expr.strip_prefix("bmz:keylogger_event:") {
        let mut parts = rest.split(':');
        let lane = parts.next()?.parse::<usize>().ok()?.checked_sub(1)?;
        let slot = parts.next()?.parse::<usize>().ok()?.checked_sub(1)?;
        if parts.next().is_some() {
            return None;
        }
        return state.keylogger_event_ms.get(lane)?.get(slot).copied().flatten();
    }
    skin_timer_elapsed_ms(destination.timer, state)
}

pub(super) fn eval_skin_draw_operand(operand: &str, state: &SkinDrawState) -> Option<f32> {
    eval_skin_draw_atom_operand(operand, state)
        .or_else(|| eval_skin_draw_sum_operand(operand, state))
}

pub(super) fn eval_skin_draw_sum_operand(operand: &str, state: &SkinDrawState) -> Option<f32> {
    let mut total = 0.0;
    let mut sign = 1.0;
    let mut start = 0;
    let mut saw_operator = false;

    for (index, ch) in operand.char_indices() {
        if index == 0 || !matches!(ch, '+' | '-') {
            continue;
        }
        let term = operand[start..index].trim();
        if term.is_empty() {
            return None;
        }
        total += sign * eval_skin_draw_atom_operand(term, state)?;
        sign = if ch == '-' { -1.0 } else { 1.0 };
        start = index + ch.len_utf8();
        saw_operator = true;
    }

    if !saw_operator {
        return None;
    }
    let term = operand[start..].trim();
    if term.is_empty() {
        return None;
    }
    total += sign * eval_skin_draw_atom_operand(term, state)?;
    Some(total)
}

pub(super) fn eval_skin_draw_atom_operand(operand: &str, state: &SkinDrawState) -> Option<f32> {
    if let Some(flag_id) = parse_runtime_flag_operand(operand) {
        return Some(if state.runtime_flags.get(&flag_id).copied().unwrap_or(false) {
            1.0
        } else {
            0.0
        });
    }
    if let Some(ref_id) = parse_skin_float_number_operand(operand) {
        return skin_state_float_number(ref_id, state);
    }
    if let Some(event_id) = parse_skin_event_index_operand(operand) {
        return Some(skin_state_event_index(event_id, state) as f32);
    }
    if let Some(ref_id) = parse_skin_number_operand(operand) {
        return skin_state_number(ref_id, state).map(|value| value as f32);
    }
    if let Some(timer_id) = parse_skin_timer_operand(operand) {
        return Some(skin_timer_elapsed_ms(Some(timer_id), state).unwrap_or(i32::MIN) as f32);
    }
    if let Some(timer_id) = parse_skin_keybeam_operand(operand, "keybeam_hold") {
        return keybeam_lane_for_keyon_timer(timer_id)
            .map(|lane| i32::from(state.keybeam_hold_active[lane]) as f32);
    }
    if let Some(timer_id) = parse_skin_keybeam_operand(operand, "keybeam_fade") {
        return keybeam_lane_for_keyoff_timer(timer_id)
            .map(|lane| i32::from(state.keybeam_fade_active[lane]) as f32);
    }
    match operand {
        "gauge()" | "gauge" => Some(state.gauge),
        "gauge_type()" | "gauge_type" => Some(state.gauge_type as f32),
        "gauge_auto_shift()" | "gauge_auto_shift" => {
            Some(if state.gauge_auto_shift { 1.0 } else { 0.0 })
        }
        "gauge_auto_shift_mode()" | "gauge_auto_shift_mode" => {
            Some(state.select_gauge_auto_shift_index as f32)
        }
        "timer_off" | "timer_off_value" => Some(i32::MIN as f32),
        value => value.parse::<f32>().ok(),
    }
}

pub(super) fn parse_runtime_flag_operand(operand: &str) -> Option<i32> {
    let inner = operand.strip_prefix("runtime_flag(")?.strip_suffix(')')?.trim();
    inner.parse().ok()
}

pub(super) fn parse_skin_number_operand(operand: &str) -> Option<i32> {
    let inner = operand.strip_prefix("number(")?.strip_suffix(')')?.trim();
    inner.parse::<i32>().ok()
}

pub(super) fn parse_skin_float_number_operand(operand: &str) -> Option<i32> {
    let inner = operand.strip_prefix("float_number(")?.strip_suffix(')')?.trim();
    inner.parse::<i32>().ok()
}

pub(super) fn parse_skin_event_index_operand(operand: &str) -> Option<i32> {
    let inner = operand.strip_prefix("event_index(")?.strip_suffix(')')?.trim();
    inner.parse::<i32>().ok()
}

pub(super) fn parse_skin_keybeam_operand(operand: &str, name: &str) -> Option<i32> {
    let inner = operand.strip_prefix(name)?.strip_prefix('(')?.strip_suffix(')')?.trim();
    inner.parse::<i32>().ok()
}

pub(super) fn keybeam_lane_for_keyon_timer(timer: i32) -> Option<usize> {
    match timer {
        100..=107 => Some((timer - 100) as usize),
        108..=109 => Some(Lane::Key8.index() + (timer - 108) as usize),
        110 => Some(Lane::Scratch2.index()),
        111..=117 => Some(Lane::Key8.index() + (timer - 111) as usize),
        _ => None,
    }
}

pub(super) fn keybeam_lane_for_keyoff_timer(timer: i32) -> Option<usize> {
    match timer {
        120..=127 => Some((timer - 120) as usize),
        128..=129 => Some(Lane::Key8.index() + (timer - 128) as usize),
        130 => Some(Lane::Scratch2.index()),
        131..=137 => Some(Lane::Key8.index() + (timer - 131) as usize),
        _ => None,
    }
}

pub(super) fn skin_state_event_index(event_id: i32, state: &SkinDrawState) -> i32 {
    match event_id {
        40 => state.select_gauge_index as i32,
        41 => state.select_target_index as i32,
        42 => arrange_ref_index(state) as i32,
        43 => arrange_2p_ref_index(state) as i32,
        54 => state.select_double_option_index as i32,
        55 if state.select_screen => state.select_hs_fix_index as i32,
        72 => state.select_bga_index as i32,
        73 => state.select_assist_index as i32,
        75 => i32::from(state.judge_timing_auto_adjust),
        78 => state.select_gauge_auto_shift_index as i32,
        308 if state.result_ln_mode_index.is_some() => {
            state.result_ln_mode_index.unwrap_or_default() as i32
        }
        308 => state.select_ln_mode_index as i32,
        340 => state.select_judge_algorithm_index as i32,
        341 => state.select_bottom_shiftable_gauge_index as i32,
        344 => extended_arrange_ref_index(state) as i32,
        345 => extended_arrange_2p_ref_index(state) as i32,
        1900 => skin_hispeed_mode_index(state),
        SKIN_REF_BMZ_KEY_MODE => effective_skin_key_mode(state).map_or(0, skin_key_mode_number),
        SKIN_REF_BMZ_SELECT_SETTINGS_ROW_KIND => {
            select_settings_row_kind_index(state.select_row_kind)
        }
        SKIN_EVENT_HSFIX => state.hsfix_index,
        _ => skin_random_lane_ref_number(event_id, state)
            .and_then(|value| i32::try_from(value).ok())
            .or_else(|| skin_state_lane_judge_event_index(event_id, state))
            .unwrap_or(0),
    }
}

pub(super) fn skin_state_lane_judge_event_index(
    event_id: i32,
    state: &SkinDrawState,
) -> Option<i32> {
    let lane = match event_id {
        500 => Lane::Scratch,
        501..=509 => Lane::from_pms_key((event_id - 500) as u8)?,
        510 => Lane::Scratch2,
        511 => Lane::Key8,
        512 => Lane::Key9,
        513 => Lane::Key10,
        514 => Lane::Key11,
        515 => Lane::Key12,
        516 => Lane::Key13,
        517 => Lane::Key14,
        _ => return None,
    };
    Some(match state.lane_judge[lane.index()] {
        None => 0,
        Some(0) => 1,
        Some(1) => 2,
        Some(2) => 4,
        Some(3) => 6,
        Some(4) => 7,
        Some(5) => 8,
        Some(_) => 0,
    })
}

/// beatoraja `main_state.float_number(ref)`。BARGRAPH / SLIDER 系の比率 0.0-1.0。
pub(super) fn skin_state_float_number(ref_id: i32, state: &SkinDrawState) -> Option<f32> {
    let is_play = !state.select_screen && state.result_failed.is_none();
    match ref_id {
        // `MainStateAccessor.float_number()` calls beatoraja's getRateProperty(),
        // so only SLIDER/BARGRAPH RateType IDs belong to this namespace.
        1 => Some(if state.select_screen {
            state.select_scroll_progress.clamp(0.0, 1.0)
        } else {
            0.0
        }),
        4 | 5 => Some(if is_play && state.lanecover_enabled {
            bmz_lane_cover_for_lift(state.lane_cover, state.lift)
        } else {
            0.0
        }),
        6 | 101 => Some(if is_play { state.play_progress.clamp(0.0, 1.0) } else { 0.0 }),
        // Skin configuration scroll position is not exposed by BMZ yet.
        7 => Some(0.0),
        8 => Some(ir_ranking_scroll_progress(state)),
        17 => Some(state.select_master_volume.clamp(0.0, 1.0)),
        18 => Some(state.select_key_volume.clamp(0.0, 1.0)),
        19 => Some(state.select_bgm_volume.clamp(0.0, 1.0)),
        102 => Some(state.resource_load_progress.clamp(0.0, 1.0)),
        103 => Some(select_level_rate(state, None)),
        105..=109 => Some(select_level_rate(state, Some(ref_id - 104))),
        110..=115 => Some(graph_value(ref_id, state)),
        140..=145 | 147 => Some(if state.select_screen { graph_value(ref_id, state) } else { 0.0 }),
        285..=289 => {
            let notes = state.select_total_notes.max(state.total_notes);
            let count = state.rival_judge_counts?[usize::try_from(ref_id - 285).ok()?];
            (notes > 0).then_some(count as f32 / notes as f32)
        }
        _ => None,
    }
}

pub(super) fn select_level_rate(state: &SkinDrawState, difficulty: Option<i32>) -> f32 {
    if !state.select_screen
        || difficulty.is_some_and(|difficulty| state.difficulty != i64::from(difficulty))
    {
        return 0.0;
    }
    // This intentionally follows beatoraja's current switch fallthrough: every supported mode
    // ends with maxLevel=10.
    (state.select_play_level as f32 / 10.0).max(0.0)
}

pub(super) fn ir_ranking_scroll_progress(state: &SkinDrawState) -> f32 {
    if state.ir_ranking.scroll_max == 0 {
        0.0
    } else {
        (state.ir_ranking.scroll_offset as f32 / state.ir_ranking.scroll_max as f32).clamp(0.0, 1.0)
    }
}

pub(super) fn parse_skin_option_operand(operand: &str) -> Option<i32> {
    let inner = operand.strip_prefix("option(")?.strip_suffix(')')?.trim();
    inner.parse::<i32>().ok()
}

pub(super) fn parse_skin_timer_operand(operand: &str) -> Option<i32> {
    let inner = operand.strip_prefix("timer(")?.strip_suffix(')')?.trim();
    inner.parse::<i32>().ok()
}

pub(super) fn select_chart_notes_total_formula(notes: u32) -> f64 {
    let notes = f64::from(notes);
    if notes <= 0.0 {
        return 0.0;
    }
    7.605 * notes / (0.01 * notes + 6.5)
}

pub(super) fn default_chart_total_count_value(state: &SkinDrawState) -> f32 {
    let notes = state.select_total_notes.max(state.total_notes);
    let total = state.select_chart_total_gauge.max(0.0) as f64;
    (select_chart_notes_total_formula(notes) - total) as f32
}

pub(super) fn default_chart_gauge_graph_value(state: &SkinDrawState) -> f32 {
    (default_chart_total_count_value(state) * 0.75).max(0.0)
}

pub(super) fn course_clear_rate_value(state: &SkinDrawState) -> f32 {
    let notes = f64::from(state.total_notes);
    if notes <= 0.0 {
        return 0.0;
    }
    let pgreat = f64::from(state.judge_counts.pgreat);
    let great = f64::from(state.judge_counts.great);
    let good = f64::from(state.judge_counts.good);
    let bad = f64::from(state.judge_counts.bad);
    let poor_and_miss =
        f64::from(state.judge_counts.poor.saturating_add(state.judge_counts.empty_poor));
    let progress = (pgreat + great + good + bad + poor_and_miss).min(notes);
    if progress <= 0.0 {
        return 0.0;
    }

    // WMII course result's "CLEAR RATE": course progress contributes 40%,
    // while its judgement-quality formula contributes the remaining 60%.
    let x = 8.0 * (pgreat + great) + 2.0 * good - (68.0 * bad + 100.0 * poor_and_miss);
    let mut y = 100.0 * x / (6.0 * notes);
    if y >= 0.0 {
        y /= 2.0;
    }
    let performance = (0.6 * (y + 50.0)).clamp(0.0, 60.0);
    (40.0 * progress / notes + performance) as f32
}

pub(super) fn clamped_gauge_value(state: &SkinDrawState) -> f32 {
    if state.gauge_max <= 0.0 { 0.0 } else { state.gauge.clamp(0.0, state.gauge_max) }
}

pub(super) fn skin_builtin_value_f32(expr: &str, state: &SkinDrawState) -> Option<f32> {
    if let Some(slot) = expr.trim().strip_prefix("bmz:ir_score_rate:") {
        let slot = slot.parse::<i32>().ok()?;
        if !(1..=10).contains(&slot) {
            return None;
        }
        let (score, max_score) = ir_ranking_score_and_max(state, slot)?;
        return Some(if max_score == 0 { 0.0 } else { score as f32 / max_score as f32 });
    }
    if expr.trim() == "bmz:keylogger_nps" {
        return Some(state.keylogger_nps as f32);
    }
    if let Some(value) = keylogger_graph_value(expr.trim(), state) {
        return Some(value);
    }
    match expr.trim() {
        SKIN_EXPR_ADJUSTED_COVER => state.adjusted_cover_progress,
        SKIN_EXPR_ADJUSTED_RATE => state.adjusted_rate,
        SKIN_EXPR_ADJUSTED_RATE_ADOT => state.adjusted_rate_adot.map(|value| value as f32),
        SKIN_EXPR_FS_THRESHOLD => Some(state.fs_threshold_ms as f32),
        SKIN_EXPR_DEFAULT_CHART_TOTAL_COUNT => Some(default_chart_total_count_value(state)),
        SKIN_EXPR_DEFAULT_CHART_GAUGE => Some(default_chart_gauge_graph_value(state)),
        SKIN_EXPR_COURSE_CLEAR_RATE => Some(course_clear_rate_value(state)),
        SKIN_EXPR_GAUGE_PERCENT_INTEGER => {
            Some((clamped_gauge_value(state) * 100.0 / state.gauge_max.max(1.0)).floor())
        }
        SKIN_EXPR_GAUGE_PERCENT_FRACTION => {
            Some((clamped_gauge_value(state) * 10_000.0 / state.gauge_max.max(1.0)).floor() % 100.0)
        }
        SKIN_EXPR_GAUGE_AMOUNT_INTEGER => Some(clamped_gauge_value(state).floor()),
        SKIN_EXPR_GAUGE_AMOUNT_FRACTION => {
            Some((clamped_gauge_value(state) * 100.0).floor() % 100.0)
        }
        _ => None,
    }
}

pub(super) fn keylogger_graph_value(expr: &str, state: &SkinDrawState) -> Option<f32> {
    let rest = expr.strip_prefix("bmz:keylogger_graph:")?;
    let mut parts = rest.split(':');
    let graph_kind = parts.next()?;
    let lane = parts.next()?.parse::<usize>().ok()?.checked_sub(1)?;
    let layer = parts.next()?;
    if parts.next().is_some() || lane >= LANE_COUNT {
        return None;
    }
    match graph_kind {
        "judge" => {
            let start = match layer {
                "cool" => 0,
                "great" => 1,
                "good" => 2,
                "bad" => 3,
                _ => return None,
            };
            let denominator_start = usize::from(state.keylogger_exclude_cool);
            let max = state
                .keylogger_judge_counts
                .iter()
                .map(|counts| counts[denominator_start..].iter().sum::<u32>())
                .max()
                .unwrap_or(0);
            let count = state.keylogger_judge_counts[lane][start..].iter().sum::<u32>();
            Some(if max == 0 { 0.0 } else { count as f32 / max as f32 })
        }
        "fastslow" => {
            let start = match layer {
                "cool" => 0,
                "fast" => 1,
                "slow" => 2,
                _ => return None,
            };
            let denominator_start = usize::from(state.keylogger_exclude_cool);
            let max = state
                .keylogger_fast_slow_counts
                .iter()
                .map(|counts| counts[denominator_start..].iter().sum::<u32>())
                .max()
                .unwrap_or(0);
            let count = state.keylogger_fast_slow_counts[lane][start..].iter().sum::<u32>();
            Some(if max == 0 { 0.0 } else { count as f32 / max as f32 })
        }
        _ => None,
    }
}

pub(super) fn skin_builtin_value_i64(expr: &str, state: &SkinDrawState) -> Option<i64> {
    if let Some(slot) = expr.trim().strip_prefix("bmz:ir_score_rate_integer:") {
        let slot = slot.parse::<i32>().ok()?;
        return ir_ranking_score_rate_parts(state, slot).map(|parts| parts.0);
    }
    if let Some(slot) = expr.trim().strip_prefix("bmz:ir_score_rate_fraction:") {
        let slot = slot.parse::<i32>().ok()?;
        return ir_ranking_score_rate_parts(state, slot).map(|parts| parts.1);
    }
    if let Some(slot) = expr.trim().strip_prefix("bmz:ir_score_diff:") {
        let slot = slot.parse::<i32>().ok()?;
        if !(1..=10).contains(&slot) {
            return None;
        }
        let local_best = skin_state_number(170, state)?.max(skin_state_number(171, state)?);
        let ranking_score = ir_ranking_entry(&state.ir_ranking, slot - 1)?.ex_score?;
        return local_best.checked_sub(ranking_score);
    }
    if expr.trim() == "bmz:nearest_rank_diff_abs" {
        return nearest_grade_diff(state).map(|diff| diff.value.abs());
    }
    let number = skin_builtin_value_f32(expr, state)?;
    Some(match expr.trim() {
        SKIN_EXPR_DEFAULT_CHART_TOTAL_COUNT | SKIN_EXPR_DEFAULT_CHART_GAUGE => {
            number.round() as i64
        }
        _ => integer_property_value(number),
    })
}

pub(super) fn skin_value_number(value: &SkinValueDef, state: &SkinDrawState) -> Option<i64> {
    if value.id == "Number_Todayplayednotes" {
        return Some(player_stat_u64(daily_completed_notes(&state.player_stats.daily)));
    }
    if !value.expr.trim().is_empty() {
        return skin_state_number_expr(&value.expr, state);
    }
    if !value.value_expr.trim().is_empty() {
        if let Some(number) = skin_builtin_value_i64(&value.value_expr, state) {
            return Some(number);
        }
        return skin_state_float_expr(&value.value_expr, state).map(integer_property_value);
    }
    skin_state_number(value.ref_id, state)
}

pub(super) fn integer_property_value(value: f32) -> i64 {
    value as i64
}

pub(super) fn skin_value_number_for_destination(
    value: &SkinValueDef,
    state: &SkinDrawState,
    has_nearest_f_diff_rank_destination: bool,
) -> Option<i64> {
    if value.ref_id == 154
        && value.expr.trim().is_empty()
        && value.value_expr.trim().is_empty()
        && state.result_grade_diff_display == ResultGradeDiffDisplay::Nearest
        && !has_nearest_f_diff_rank_destination
    {
        return nearest_grade_diff_for_destination(state, false).map(|diff| diff.value);
    }
    if value.ref_id == 0 && value.expr.trim().is_empty() && value.value_expr.trim().is_empty() {
        return Some(if state.play_level != 0 {
            state.play_level
        } else {
            state.select_play_level
        });
    }
    skin_value_number(value, state)
}

pub(super) fn skin_state_number_expr(expr: &str, state: &SkinDrawState) -> Option<i64> {
    let normalized = expr.replace('+', " + ").replace('-', " - ");
    let mut sign = 1_i64;
    let mut total = 0_i64;
    let mut expecting_value = true;
    for token in normalized.split_whitespace() {
        match token {
            "+" if expecting_value => sign = 1,
            "-" if expecting_value => sign = -1,
            "+" if !expecting_value => {
                sign = 1;
                expecting_value = true;
            }
            "-" if !expecting_value => {
                sign = -1;
                expecting_value = true;
            }
            value => {
                if !expecting_value {
                    return None;
                }
                let term = skin_state_number_expr_term(value, state)?;
                total += sign * term;
                sign = 1;
                expecting_value = false;
            }
        }
    }
    if expecting_value {
        return None;
    }
    Some(total)
}

pub(super) fn skin_state_number_expr_term(term: &str, state: &SkinDrawState) -> Option<i64> {
    if let Some(ref_id) = parse_skin_number_operand(term) {
        return skin_state_number(ref_id, state);
    }
    if let Some((coefficient, operand)) = term.split_once('*') {
        let coefficient = coefficient.parse::<i64>().ok()?;
        let ref_id = parse_skin_number_operand(operand.trim())?;
        return skin_state_number(ref_id, state).map(|value| coefficient * value);
    }
    term.parse::<i64>().ok()
}

pub(super) fn skin_state_float_expr(expr: &str, state: &SkinDrawState) -> Option<f32> {
    let expr = strip_wrapping_parentheses(expr.trim());
    if expr.is_empty() {
        return None;
    }
    if let Some(inner) = expr.strip_prefix("floor(").and_then(|value| value.strip_suffix(')')) {
        return skin_state_float_expr(inner.trim(), state).map(f32::floor);
    }
    if let Some(inner) = expr.strip_prefix("max(0,").and_then(|value| value.strip_suffix(')')) {
        return skin_state_float_expr(inner.trim(), state).map(|value| value.max(0.0));
    }
    skin_state_additive_float_expr(expr, state)
}

pub(super) fn strip_wrapping_parentheses(mut expr: &str) -> &str {
    loop {
        let trimmed = expr.trim();
        if !outer_parentheses_wrap_expression(trimmed) {
            return trimmed;
        }
        expr = &trimmed[1..trimmed.len() - 1];
    }
}

pub(super) fn outer_parentheses_wrap_expression(expr: &str) -> bool {
    if !expr.starts_with('(') || !expr.ends_with(')') {
        return false;
    }
    let mut depth = 0_i32;
    let last_index = expr.len() - 1;
    for (index, ch) in expr.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && index < last_index {
                    return false;
                }
            }
            _ => {}
        }
        if depth < 0 {
            return false;
        }
    }
    depth == 0
}

pub(super) fn skin_state_additive_float_expr(expr: &str, state: &SkinDrawState) -> Option<f32> {
    let mut depth = 0_i32;
    let mut sign = 1.0_f32;
    let mut start = 0_usize;
    let mut total = 0.0_f32;

    for (index, ch) in expr.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '+' | '-' if depth == 0 => {
                let term = expr[start..index].trim();
                if term.is_empty() {
                    sign = if ch == '-' { -1.0 } else { 1.0 };
                    start = index + ch.len_utf8();
                    continue;
                }
                total += sign * skin_state_float_mul_div_expr(term, state)?;
                sign = if ch == '-' { -1.0 } else { 1.0 };
                start = index + ch.len_utf8();
            }
            _ => {}
        }
        if depth < 0 {
            return None;
        }
    }
    if depth != 0 {
        return None;
    }
    let term = expr[start..].trim();
    if term.is_empty() {
        return None;
    }
    total += sign * skin_state_float_mul_div_expr(term, state)?;
    Some(total)
}

pub(super) fn skin_state_float_mul_div_expr(expr: &str, state: &SkinDrawState) -> Option<f32> {
    let mut depth = 0_i32;
    let mut start = 0_usize;
    let mut value: Option<f32> = None;
    let mut operator = '*';

    for (index, ch) in expr.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '*' | '/' if depth == 0 => {
                let factor = skin_state_float_expr_term(expr[start..index].trim(), state)?;
                value = Some(apply_float_mul_div(value, factor, operator));
                operator = ch;
                start = index + ch.len_utf8();
            }
            _ => {}
        }
        if depth < 0 {
            return None;
        }
    }
    if depth != 0 {
        return None;
    }

    let factor = skin_state_float_expr_term(expr[start..].trim(), state)?;
    Some(apply_float_mul_div(value, factor, operator))
}

pub(super) fn apply_float_mul_div(current: Option<f32>, factor: f32, operator: char) -> f32 {
    let Some(current) = current else { return factor };
    match operator {
        '*' => current * factor,
        '/' if factor.abs() < f32::EPSILON => 0.0,
        '/' => current / factor,
        _ => current,
    }
}

pub(super) fn skin_state_float_expr_term(term: &str, state: &SkinDrawState) -> Option<f32> {
    let term = term.trim();
    let stripped = strip_wrapping_parentheses(term);
    if stripped.len() != term.len() {
        return skin_state_float_expr(stripped, state);
    }
    if term.starts_with("floor(") || term.starts_with("max(0,") {
        return skin_state_float_expr(term, state);
    }
    if let Some(ref_id) = parse_skin_float_number_operand(term) {
        return skin_state_float_number(ref_id, state);
    }
    if let Some(event_id) = parse_skin_event_index_operand(term) {
        return Some(skin_state_event_index(event_id, state) as f32);
    }
    if let Some(ref_id) = parse_skin_number_operand(term) {
        return skin_state_number(ref_id, state).map(|value| value as f32);
    }
    if let Some(option_id) = parse_skin_option_operand(term) {
        return Some(if test_skin_op(option_id, &[], state) { 1.0 } else { 0.0 });
    }
    term.parse::<f32>().ok()
}

/// destination の全 option 条件を現在の描画状態に対して評価する。
pub fn test_skin_ops(ops: &[i32], enabled_options: &[i32], state: &SkinDrawState) -> bool {
    ops.iter().all(|op| test_skin_op(*op, enabled_options, state))
}

pub(super) fn destination_ops_match(
    destination: &SkinDestinationDef,
    enabled_options: &[i32],
    state: &SkinDrawState,
    has_nearest_f_diff_rank_destination: bool,
) -> bool {
    if is_grade_diff_rank_destination(destination, state) {
        return destination.op.iter().all(|&op| {
            test_grade_diff_rank_op(
                destination,
                op,
                enabled_options,
                state,
                has_nearest_f_diff_rank_destination,
            )
        });
    }
    test_skin_ops(&destination.op, enabled_options, state)
}

pub(super) fn test_grade_diff_rank_op(
    destination: &SkinDestinationDef,
    op: i32,
    enabled_options: &[i32],
    state: &SkinDrawState,
    has_nearest_f_diff_rank_destination: bool,
) -> bool {
    if op < 0 {
        return op.checked_neg().is_some_and(|positive| {
            !test_grade_diff_rank_op(
                destination,
                positive,
                enabled_options,
                state,
                has_nearest_f_diff_rank_destination,
            )
        });
    }
    match op {
        300..=307 => grade_diff_rank_destination_matches(
            destination,
            op,
            state,
            has_nearest_f_diff_rank_destination,
        ),
        _ => test_skin_op(op, enabled_options, state),
    }
}

pub(super) fn test_skin_op(op: i32, enabled_options: &[i32], state: &SkinDrawState) -> bool {
    if op < 0 {
        return op
            .checked_neg()
            .is_some_and(|positive| !test_skin_op(positive, enabled_options, state));
    }
    match op {
        40 => !state.bga_enabled,
        41 => state.bga_enabled,
        1901 => skin_hispeed_mode_is_floating(state),
        SKIN_OPTION_BMZ_RESULT_IR_SCOPE_GLOBAL => {
            state.ir_ranking.scope == crate::scene::ResultIrScope::Global
        }
        SKIN_OPTION_BMZ_RESULT_IR_SCOPE_RIVAL => {
            state.ir_ranking.scope == crate::scene::ResultIrScope::Rival
        }
        SKIN_OPTION_BMZ_RESULT_IR_SCOPE_GLOBAL_SUPPORTED => state.ir_ranking.global_scope_supported,
        SKIN_OPTION_BMZ_RESULT_IR_SCOPE_RIVAL_SUPPORTED => state.ir_ranking.rival_scope_supported,
        SKIN_OPTION_BMZ_INPUT_BASE..=SKIN_OPTION_BMZ_INPUT_LAST => {
            state.logical_input_held[(op - SKIN_OPTION_BMZ_INPUT_BASE) as usize]
        }
        1 => matches!(
            state.select_row_kind,
            SelectRowKind::Folder
                | SelectRowKind::TableFolder
                | SelectRowKind::SearchFolder
                | SelectRowKind::Command
                | SelectRowKind::Container
                | SelectRowKind::SettingsRoot
                | SelectRowKind::SettingsFolder
                | SelectRowKind::SettingsBack
                | SelectRowKind::SettingsClose
        ),
        SKIN_OPTION_BMZ_SETTINGS_FOLDER => matches!(
            state.select_row_kind,
            SelectRowKind::SettingsRoot | SelectRowKind::SettingsFolder
        ),
        SKIN_OPTION_BMZ_SETTINGS_BACK => state.select_row_kind == SelectRowKind::SettingsBack,
        SKIN_OPTION_BMZ_SETTINGS_CLOSE => state.select_row_kind == SelectRowKind::SettingsClose,
        2 => select_song_detail_row(state),
        3 => state.select_row_kind == SelectRowKind::Course,
        1030 => state.select_row_kind == SelectRowKind::Executable,
        1031 => state.select_row_kind == SelectRowKind::RandomCourse,
        1008 => state.table_song,
        1002..=1017 => gradebar_constraint_op_matches(op, state),
        5 => {
            !state.in_settings
                && (matches!(state.select_row_kind, SelectRowKind::Executable)
                    || (state.select_in_library
                        && !state.select_is_folder
                        && matches!(
                            state.select_row_kind,
                            SelectRowKind::Song
                                | SelectRowKind::Course
                                | SelectRowKind::RandomCourse
                        )))
        }
        // OPTION_OFFLINE / OPTION_ONLINE. beatoraja は設定済み IR 接続の有無を
        // 返す。結果スキンでは 51 が IR 送信完了/失敗の timer 173/174 を
        // 描画する前提条件としても使われる。
        50 => matches!(state.ir_ranking.state, crate::scene::ResultIrState::Offline),
        51 => !matches!(state.ir_ranking.state, crate::scene::ResultIrState::Offline),
        21 => state.select_option_panel == 1,
        22 => state.select_option_panel == 2,
        23 => state.select_option_panel == 3,
        160..=164 => select_key_mode_option_matches(op, state),
        1160 | 1161 => select_key_mode_option_matches(op, state),
        SKIN_OPTION_BMZ_KEY_MODE_BASE..=SKIN_OPTION_BMZ_KEY_MODE_LAST => {
            select_key_mode_option_matches(op, state)
        }
        SKIN_OPTION_BMZ_NO_SCRATCH | SKIN_OPTION_BMZ_SINGLE_PLAY | SKIN_OPTION_BMZ_DOUBLE_PLAY => {
            select_key_mode_option_matches(op, state)
        }
        196 | 197 | 198 | 1196..=1208 if state.result_failed.is_some() => {
            result_replay_op_matches(op, state)
        }
        126..=131 | 1128..=1131 if state.result_failed.is_some() => {
            result_arrange_op_matches(op, state)
        }
        196 | 197 | 198 | 1196..=1208 => select_replay_op_matches(op, state),
        200..=207 => select_rank_op_matches(op, state),
        300..=318 if state.result_failed.is_some() => result_rank_op_matches(op, state),
        300..=307 => select_small_rank_op_matches(op, state),
        320..=327 => best_rank_op_matches(op, state),
        // OPTION_NO_LN / OPTION_LN. Resultでは、選曲設定ではなく
        // LN policy / course constraint適用後の実効譜面を使う。
        172 if state.result_has_long_notes.is_some() => {
            !state.result_has_long_notes.unwrap_or_default()
        }
        173 if state.result_has_long_notes.is_some() => {
            state.result_has_long_notes.unwrap_or_default()
        }
        170 => !state.has_bga,
        171 => state.has_bga,
        // SongDataBooleanProperty returns false for both branches without a selected song.
        174 => select_song_option_matches(state) && !state.select_has_document,
        175 => select_song_option_matches(state) && state.select_has_document,
        // OPTION_BPMCHANGE (BPM変化あり) / OPTION_BPMSTOP (STOP命令あり)
        177 => state.min_bpm < state.max_bpm,
        1177 => state.has_bpm_stop,
        // OPTION_NOW_LOADING / OPTION_LOADED
        80 => !state.skin_loaded,
        81 => state.skin_loaded,
        // OPTION_NO_STAGEFILE / OPTION_STAGEFILE
        190 => !state.has_stagefile,
        191 => state.has_stagefile,
        // OPTION_NO_BANNER / OPTION_BANNER (192/193)
        192 => select_banner_option_matches(false, state),
        193 => select_banner_option_matches(true, state),
        // OPTION_NO_BACKBMP / OPTION_BACKBMP
        194 => !state.has_backbmp,
        195 => state.has_backbmp,
        // OPTION_LANECOVER1_CHANGING / OPTION_LANECOVER1_ON / OPTION_LIFT1_ON / OPTION_HIDDEN1_ON
        270 => state.lane_cover_changing,
        271 => state.lanecover_enabled,
        272 => state.lift_enabled,
        273 => state.hidden_enabled,
        // OPTION_1P_0_9 .. OPTION_1P_100. beatoraja evaluates these only on
        // BMSPlayer and compares the displayed gauge value with its configured maximum.
        230..=240 => gauge_range_option_matches(op, state),
        // Result judgement-existence options. EmptyPoor is beatoraja's MISS bucket.
        2241 if state.result_failed.is_some() => state.judge_counts.pgreat > 0,
        2242 if state.result_failed.is_some() => state.judge_counts.great > 0,
        2243 if state.result_failed.is_some() => state.judge_counts.good > 0,
        2244 if state.result_failed.is_some() => state.judge_counts.bad > 0,
        2245 if state.result_failed.is_some() => state.judge_counts.poor > 0,
        2246 if state.result_failed.is_some() => state.judge_counts.empty_poor > 0,
        2241..=2246 => false,
        // Result/update comparison options. In play skins these are often reused
        // as target-reached draw conditions.
        330 => state.previous_best_ex_score.is_some_and(|best| state.ex_score > best),
        1330 => state.previous_best_ex_score.is_some_and(|best| state.ex_score == best),
        331 => state.previous_best_max_combo.is_some_and(|best| state.max_combo > best),
        1331 => state.previous_best_max_combo.is_some_and(|best| state.max_combo == best),
        332 => state.previous_best_bp.is_some_and(|best| current_bp(state) < best),
        1332 => state.previous_best_bp.is_some_and(|best| current_bp(state) == best),
        335 => state.previous_best_ex_score.is_some_and(|best| {
            score_rate_cmp_value(state.ex_score, state.total_notes)
                > score_rate_cmp_value(best, state.total_notes)
        }),
        1335 => state.previous_best_ex_score.is_some_and(|best| {
            score_rate_cmp_value(state.ex_score, state.total_notes)
                == score_rate_cmp_value(best, state.total_notes)
        }),
        336 => state.target_ex_score.is_some_and(|target| state.ex_score > target),
        1336 => state.target_ex_score.is_some_and(|target| state.ex_score == target),
        350 => true,
        351 => false,
        352 => state.target_ex_score.is_some_and(|target| state.ex_score > target),
        353 => state.target_ex_score.is_some_and(|target| state.ex_score < target),
        354 => state.target_ex_score.is_some_and(|target| state.ex_score == target),
        // OPTION_GAUGE_GROOVE / OPTION_GAUGE_HARD / OPTION_GAUGE_EX.
        // beatoraja uses the current gauge type index: 0..2 are groove-family,
        // 3+ are hard-family, and 1046 is true for assist/easy/ex variants.
        42 => state.gauge_type <= 2,
        43 => state.gauge_type >= 3,
        1046 => matches!(state.gauge_type, 0 | 1 | 4 | 5 | 7 | 8),
        // OPTION_NOT_COMPARE_RIVAL / OPTION_COMPARE_RIVAL。
        624 => state.rival_ex_score.is_none(),
        625 => state.rival_ex_score.is_some(),
        // OPTION_IR_LOADING / LOADED / NOPLAYER / FAILED (601..604)。
        601 => matches!(state.ir_ranking.state, crate::scene::ResultIrState::Loading),
        602 => matches!(state.ir_ranking.state, crate::scene::ResultIrState::Loaded),
        603 => {
            matches!(state.ir_ranking.state, crate::scene::ResultIrState::Loaded)
                && state.ir_ranking.total_player == Some(0)
        }
        604 => matches!(state.ir_ranking.state, crate::scene::ResultIrState::Failed),
        // beatoraja MusicSelector: ranking object生成前はWAITING。
        606 => matches!(state.ir_ranking.state, crate::scene::ResultIrState::Waiting),
        // BooleanPropertyFactory のIR_BUSYは現行beatorajaでFAILと同条件。
        608 => matches!(state.ir_ranking.state, crate::scene::ResultIrState::Failed),
        // BANNED / ACCESSING は現行beatorajaにもproperty実装がない。
        605 | 607 => false,
        // OPTION_DIFFICULTY0..5. 0 は UNKNOWN/OTHER、1..5 は BMS #DIFFICULTY。
        150 => state.difficulty <= 0 || state.difficulty > 5,
        151..=155 => state.difficulty == i64::from(op - 150),
        // OPTION_JUDGE_VERYHARD..VERYEASY (180..184)
        180..=184 => {
            !(state.select_screen && state.in_settings)
                && select_chart_metadata_available(state)
                && judge_rank_option_matches(op, state.judge_rank)
        }
        // OPTION_RESULT_CLEAR=90, OPTION_RESULT_FAIL=91
        // Result 画面以外 (result_failed == None) では両方 false。
        90 => state.result_failed == Some(false),
        91 => state.result_failed == Some(true),
        // OPTION_AUTOPLAYOFF / OPTION_AUTOPLAYON
        32 => !state.autoplay,
        33 => state.autoplay,
        // PlayerResource.updateScore. Select など対象外 scene では両方 false。
        60 => state.score_save_enabled == Some(false),
        61 => state.score_save_enabled == Some(true),
        // BMSPlayer play mode. beatoraja では 82 は PLAY/PRACTICE、84 は REPLAY。
        82 => state.play_screen && !state.autoplay && !state.replay_playback,
        84 => state.play_screen && state.replay_playback,
        1080 => state.play_screen && state.practice_mode,
        // OPTION_1P/2P/3P_PERFECT and EARLY/LATE judge-detail conditions.
        // beatoraja maps FAST/EARLY to positive recent judge timing, LATE/SLOW to negative.
        // judge_timing_sign is None when FAST/SLOW display is suppressed (Auto mode hides PGREAT,
        // ThresholdMs mode hides below the threshold), so no extra judge_index guard is needed.
        241 => state.judge_index[0] == Some(0),
        1242 => state.judge_timing_sign[0] == Some(1),
        1243 => state.judge_timing_sign[0] == Some(-1),
        261 => state.judge_index[1] == Some(0),
        1262 => state.judge_timing_sign[1] == Some(1),
        1263 => state.judge_timing_sign[1] == Some(-1),
        361 => state.judge_index[2] == Some(0),
        1362 => state.judge_timing_sign[2] == Some(1),
        1363 => state.judge_timing_sign[2] == Some(-1),
        // OPTION_COURSE_STAGE1..4 / OPTION_COURSE_STAGE_FINAL
        280 => state.course_stage == Some(CourseStageMarker::Stage1),
        281 => state.course_stage == Some(CourseStageMarker::Stage2),
        282 => state.course_stage == Some(CourseStageMarker::Stage3),
        283 => state.course_stage == Some(CourseStageMarker::Stage4),
        289 => state.course_stage == Some(CourseStageMarker::Final),
        // OPTION_MODE_COURSE
        290 => state.course_stage.is_some(),
        // beatoraja defines OPTION_MODE_NONSTOP / EXPERT / GRADE (291..293)
        // but does not expose BooleanProperty handlers for them.  Return
        // false here instead of falling through to skin property defaults.
        291..=293 => false,
        value => test_json_option_number(value, enabled_options),
    }
}

pub(super) fn gauge_range_option_matches(op: i32, state: &SkinDrawState) -> bool {
    if !state.play_screen || state.gauge_max <= 0.0 {
        return false;
    }
    let range = (op - 230) as f32;
    let value = state.gauge / state.gauge_max;
    value >= range * 0.1 && value < (range + 1.0) * 0.1
}

pub(super) fn gradebar_constraint_op_matches(op: i32, state: &SkinDrawState) -> bool {
    if state.select_row_kind != SelectRowKind::Course {
        return false;
    }
    let constraints = state.select_course_constraints;
    match op {
        1002 => constraints.class,
        1003 => constraints.mirror,
        1004 => constraints.random,
        1005 => constraints.no_speed,
        1006 => constraints.no_good,
        1007 => constraints.no_great,
        1010 => constraints.gauge_lr2,
        1011 => constraints.gauge_5k,
        1012 => constraints.gauge_7k,
        1013 => constraints.gauge_9k,
        1014 => constraints.gauge_24k,
        1015 => constraints.ln,
        1016 => constraints.cn,
        1017 => constraints.hcn,
        _ => false,
    }
}
