use super::*;

pub(in crate::lua) fn infer_constant_number_at_load(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    main_state_probe.lock().ok()?.end_recording();
    match function.call::<Value>(()).ok()? {
        Value::Integer(value) => Some(value.to_string()),
        Value::Number(value) if value.is_finite() => Some(value.to_string()),
        _ => None,
    }
}

pub(in crate::lua) fn infer_constant_integer_at_load(
    function: &Function,
    _main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i64> {
    // `act` is an input callback. Calling it in the skin's live Lua environment
    // can mutate globals used by later draw conversion (WMII switches Expand_op
    // from GRAPH to IR this way). Evaluate serializable constant callbacks in an
    // isolated Lua state so conversion has no observable side effects.
    let isolated = Lua::new();
    let dumped = function.dump(true);
    let isolated_function = isolated.load(&dumped).into_function().ok()?;
    match isolated_function.call::<Value>(()).ok()? {
        Value::Integer(value) => Some(value),
        Value::Number(value) if value.is_finite() && value.fract() == 0.0 => Some(value as i64),
        _ => None,
    }
}

pub(in crate::lua) fn infer_result_panel_act_at_load(
    lua: &Lua,
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i64> {
    if let Some(current) =
        lua.globals().raw_get::<Value>("Expand_op").ok().and_then(lua_result_panel_value)
    {
        // WMII の tab callback は `Expand_op = 1/2` だけを行う。元の Lua state を
        // 実行時まで保持せず、隔離 state で代入先を観測して BMZ 内部 event に変換する。
        let isolated = Lua::new();
        isolated.globals().raw_set("Expand_op", current).ok()?;
        let dumped = function.dump(true);
        let isolated_function = isolated.load(&dumped).into_function().ok()?;
        if !matches!(isolated_function.call::<Value>(()).ok()?, Value::Nil) {
            return None;
        }
        let panel = isolated.globals().raw_get::<Value>("Expand_op").ok()?;
        return result_panel_event(lua_result_panel_value(panel)?);
    }

    // Luxe Flat keeps the active tab in a local closure upvalue instead of the
    // global used by WMII. Preserve upvalue names in the dumped callback, seed
    // its isolated copy, and observe only the resulting `result_mode` value.
    let (upvalue_index, current_mode) = lua_result_mode_upvalue(lua, function)?;
    record_local_result_panel_default(main_state_probe, current_mode)?;
    let isolated = Lua::new();
    let dumped = function.dump(false);
    let isolated_function = isolated.load(&dumped).into_function().ok()?;
    if !set_lua_integer_upvalue(&isolated, &isolated_function, upvalue_index, current_mode)
        || !matches!(isolated_function.call::<Value>(()).ok()?, Value::Nil)
    {
        return None;
    }
    let (_, mode) = lua_result_mode_upvalue(&isolated, &isolated_function)?;
    result_panel_event(result_panel_from_local_mode(mode)?)
}

pub(in crate::lua) fn result_panel_event(panel: i32) -> Option<i64> {
    match panel {
        1 => Some(i64::from(SKIN_EVENT_RESULT_PANEL_IR)),
        2 => Some(i64::from(SKIN_EVENT_RESULT_PANEL_GRAPH)),
        _ => None,
    }
}

pub(in crate::lua) fn collect_number_refs(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<Vec<i32>> {
    let mut calls = Vec::new();
    // Lua の `or` / `and` 短絡評価で片方の number() だけ呼ばれることがあるため、
    // 複数の probe 値で実行して ref を集める。
    for default_value in [5, 0, -1] {
        {
            main_state_probe.lock().ok()?.begin_number_call_recording(default_value);
        }
        let _ = function.call::<Value>(()).ok();
        {
            let mut probe = main_state_probe.lock().ok()?;
            calls.extend(probe.number_calls.iter().copied());
            probe.end_recording();
        }
    }
    calls.sort_unstable();
    calls.dedup();
    Some(calls)
}

pub(in crate::lua) fn collect_number_refs_with_option(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    option_id: i32,
) -> Option<Vec<i32>> {
    collect_number_refs_with_option_value(function, main_state_probe, option_id, true)
}

pub(in crate::lua) fn collect_number_refs_with_option_value(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    option_id: i32,
    option_value: bool,
) -> Option<Vec<i32>> {
    let mut calls = Vec::new();
    for default_value in [5, 0, -1] {
        {
            main_state_probe.lock().ok()?.begin_number_call_recording_with_option_value(
                default_value,
                option_id,
                option_value,
            );
        }
        let _ = function.call::<Value>(()).ok();
        {
            let mut probe = main_state_probe.lock().ok()?;
            calls.extend(probe.number_calls.iter().copied());
            probe.end_recording();
        }
    }
    calls.sort_unstable();
    calls.dedup();
    Some(calls)
}

pub(in crate::lua) fn call_draw_with_numbers(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    values: BTreeMap<i32, i32>,
) -> Option<bool> {
    {
        main_state_probe.lock().ok()?.begin_number_recording_with_values(values);
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(in crate::lua) fn call_draw_with_numbers_and_timers(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    values: BTreeMap<i32, i32>,
    timers: BTreeMap<i32, i32>,
) -> Option<bool> {
    {
        main_state_probe.lock().ok()?.begin_number_timer_recording_with_values(values, timers);
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        Value::Nil => Some(false),
        _ => None,
    }
}

pub(in crate::lua) fn call_draw_with_number_option(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    number_ref: i32,
    number_value: i32,
    option_id: i32,
    option_value: bool,
) -> Option<bool> {
    {
        main_state_probe.lock().ok()?.begin_number_recording_with_values_and_options(
            BTreeMap::from([(number_ref, number_value)]),
            BTreeMap::from([(option_id, option_value)]),
        );
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(in crate::lua) fn call_number_float_with_values(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    values: BTreeMap<i32, i32>,
) -> Option<f64> {
    call_number_float_raw_with_values(function, main_state_probe, values)
        .filter(|value| value.is_finite())
}

pub(in crate::lua) fn call_number_float_raw_with_values(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    values: BTreeMap<i32, i32>,
) -> Option<f64> {
    {
        main_state_probe.lock().ok()?.begin_number_recording_with_values(values);
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Integer(value) => Some(value as f64),
        Value::Number(value) => Some(value),
        _ => None,
    }
}

pub(in crate::lua) fn call_number_float_with_values_and_options(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    values: BTreeMap<i32, i32>,
    options: BTreeMap<i32, bool>,
) -> Option<f64> {
    {
        main_state_probe
            .lock()
            .ok()?
            .begin_number_recording_with_values_and_options(values, options);
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Integer(value) => Some(value as f64),
        Value::Number(value) if value.is_finite() => Some(value),
        _ => None,
    }
}

pub(in crate::lua) fn call_draw_with_numbers_and_options(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    values: BTreeMap<i32, i32>,
    options: BTreeMap<i32, bool>,
) -> Option<bool> {
    main_state_probe.lock().ok()?.begin_number_recording_with_values_and_options(values, options);
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        Value::Nil => Some(false),
        _ => None,
    }
}

pub(in crate::lua) fn verify_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    refs: &[i32],
    expected: impl Fn(&BTreeMap<i32, i32>) -> bool,
) -> bool {
    // Keep consecutive values through one past the largest threshold inferred
    // by `infer_two_number_compare_and`. Without 4/6, an always-false draw can
    // spuriously match `left > right and right >= 4/5` because the verifier has
    // no sampled pair that can satisfy those predicates.
    let samples = [-1, 0, 1, 2, 3, 4, 5, 6];
    for &left in &samples {
        for &right in &samples {
            let mut values = BTreeMap::new();
            if refs.len() == 1 {
                values.insert(refs[0], left);
            } else if refs.len() >= 2 {
                values.insert(refs[0], left);
                values.insert(refs[1], right);
                for extra in refs.iter().skip(2) {
                    values.insert(*extra, 0);
                }
            }
            let Some(got) = call_draw_with_numbers(function, main_state_probe, values.clone())
            else {
                return false;
            };
            if got != expected(&values) {
                return false;
            }
        }
    }
    true
}

pub(in crate::lua) fn infer_or_of_number_gt_zero(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let refs = collect_number_refs(function, main_state_probe)?;
    if refs.is_empty() {
        return None;
    }
    let all_zero = refs.iter().copied().map(|ref_id| (ref_id, 0)).collect::<BTreeMap<_, _>>();
    if call_draw_with_numbers(function, main_state_probe, all_zero) != Some(false) {
        return None;
    }
    let mut terms = Vec::new();
    for ref_id in &refs {
        let mut only_positive = refs.iter().copied().map(|id| (id, 0)).collect::<BTreeMap<_, _>>();
        only_positive.insert(*ref_id, 5);
        if call_draw_with_numbers(function, main_state_probe, only_positive) == Some(true) {
            terms.push(format!("number({ref_id}) > 0"));
        }
    }
    if terms.is_empty() {
        return None;
    }
    let condition = terms.join(" or ");
    verify_draw_condition(function, main_state_probe, &refs, |values| {
        refs.iter().any(|ref_id| values.get(ref_id).copied().unwrap_or(0) > 0)
    })
    .then_some(condition)
}

pub(in crate::lua) fn infer_or_of_number_lt_zero(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let refs = collect_number_refs(function, main_state_probe)?;
    if refs.is_empty() {
        return None;
    }
    if refs.len() == 1 {
        let ref_id = refs[0];
        let condition = format!("number({ref_id}) < 0");
        return verify_draw_condition(function, main_state_probe, &refs, |values| {
            values.get(&ref_id).copied().unwrap_or(0) < 0
        })
        .then_some(condition);
    }
    let all_zero = refs.iter().copied().map(|ref_id| (ref_id, 0)).collect::<BTreeMap<_, _>>();
    if call_draw_with_numbers(function, main_state_probe, all_zero) != Some(false) {
        return None;
    }
    let mut terms = Vec::new();
    for ref_id in &refs {
        let mut only_negative = refs.iter().copied().map(|id| (id, 0)).collect::<BTreeMap<_, _>>();
        only_negative.insert(*ref_id, -1);
        if call_draw_with_numbers(function, main_state_probe, only_negative) == Some(true) {
            terms.push(format!("number({ref_id}) < 0"));
        }
    }
    if terms.is_empty() {
        return None;
    }
    let condition = terms.join(" or ");
    verify_draw_condition(function, main_state_probe, &refs, |values| {
        refs.iter().any(|ref_id| values.get(ref_id).copied().unwrap_or(0) < 0)
    })
    .then_some(condition)
}

pub(in crate::lua) fn infer_result_average_timing_sign_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let refs = collect_number_refs(function, main_state_probe)?;
    if refs.as_slice() != [374, 375] {
        return None;
    }

    let samples = [(0, 0), (0, 34), (0, -34), (1, 0), (-1, 0), (12, 34), (-12, -34)];
    let observed = samples
        .iter()
        .map(|(integer, afterdot)| {
            call_draw_with_numbers(
                function,
                main_state_probe,
                BTreeMap::from([(374, *integer), (375, *afterdot)]),
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let expected_negative = samples
        .iter()
        .map(|(integer, afterdot)| *integer as f64 + *afterdot as f64 * 0.01 < 0.0)
        .collect::<Vec<_>>();
    if observed == expected_negative {
        return Some("number(374) < 0 or number(375) < 0".to_string());
    }

    let expected_non_negative = samples
        .iter()
        .map(|(integer, afterdot)| *integer as f64 + *afterdot as f64 * 0.01 >= 0.0)
        .collect::<Vec<_>>();
    if observed == expected_non_negative {
        return Some("number(374) >= 0 and number(375) >= 0".to_string());
    }

    let expected_positive = samples
        .iter()
        .map(|(integer, afterdot)| *integer as f64 + *afterdot as f64 * 0.01 > 0.0)
        .collect::<Vec<_>>();
    (observed == expected_positive).then(|| "number(374) > 0 or number(375) > 0".to_string())
}

pub(in crate::lua) fn infer_or_of_number_eq_zero(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let refs = collect_number_refs(function, main_state_probe)?;
    if refs.len() < 2 {
        return None;
    }
    let all_positive = refs.iter().copied().map(|ref_id| (ref_id, 5)).collect::<BTreeMap<_, _>>();
    if call_draw_with_numbers(function, main_state_probe, all_positive) != Some(false) {
        return None;
    }
    let mut terms = Vec::new();
    for ref_id in &refs {
        let mut only_zero = refs.iter().copied().map(|id| (id, 5)).collect::<BTreeMap<_, _>>();
        only_zero.insert(*ref_id, 0);
        if call_draw_with_numbers(function, main_state_probe, only_zero) == Some(true) {
            terms.push(format!("number({ref_id}) == 0"));
        }
    }
    if terms.is_empty() {
        return None;
    }
    let condition = terms.join(" or ");
    verify_draw_condition(function, main_state_probe, &refs, |values| {
        refs.iter().any(|ref_id| values.get(ref_id).copied().unwrap_or(0) == 0)
    })
    .then_some(condition)
}

pub(in crate::lua) fn infer_two_number_compare_and(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let refs = collect_number_refs(function, main_state_probe)?;
    if refs.len() != 2 {
        return None;
    }
    let (left, right) = (refs[0], refs[1]);
    for threshold in 0..=5 {
        for &(flip_left, flip_right) in &[(left, right), (right, left)] {
            let condition = format!(
                "number({flip_left}) < number({flip_right}) and number({flip_right}) >= {threshold}"
            );
            if verify_draw_condition(function, main_state_probe, &refs, |values| {
                let a = values.get(&flip_left).copied().unwrap_or(0);
                let b = values.get(&flip_right).copied().unwrap_or(0);
                a < b && b >= threshold
            }) {
                return Some(condition);
            }
            let gt_condition = format!(
                "number({flip_left}) > number({flip_right}) and number({flip_right}) >= {threshold}"
            );
            if verify_draw_condition(function, main_state_probe, &refs, |values| {
                let a = values.get(&flip_left).copied().unwrap_or(0);
                let b = values.get(&flip_right).copied().unwrap_or(0);
                a > b && b >= threshold
            }) {
                return Some(gt_condition);
            }
        }
    }
    None
}

pub(in crate::lua) fn infer_number_eq_zero_with_constant_tail(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let refs = collect_number_refs(function, main_state_probe)?;
    if refs.len() != 1 {
        return None;
    }
    let ref_id = refs[0];
    let zero = call_draw_with_numbers(function, main_state_probe, BTreeMap::from([(ref_id, 0)]))?;
    let nonzero =
        call_draw_with_numbers(function, main_state_probe, BTreeMap::from([(ref_id, 5)]))?;
    if zero && !nonzero {
        return Some(format!("number({ref_id}) == 0"));
    }
    if !zero && nonzero {
        return Some(format!("number({ref_id}) != 0"));
    }
    None
}

pub(in crate::lua) fn infer_gauge_type_imageset_ref(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i32> {
    {
        main_state_probe.lock().ok()?.begin_gauge_type_call_recording(0);
    }
    let _ = function.call::<Value>(()).ok();
    let (gauge_calls, number_calls) = {
        let mut probe = main_state_probe.lock().ok()?;
        let gauge_calls = probe.gauge_type_calls;
        let number_calls = probe.number_calls.clone();
        probe.end_recording();
        (gauge_calls, number_calls)
    };
    (gauge_calls > 0 && number_calls.is_empty()).then_some(SKIN_REF_PLAY_GAUGE_TYPE)
}

pub(in crate::lua) fn infer_course_table_text_expr(
    function: &Function,
    object_id: Option<&str>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    if object_id == Some("table") {
        return Some(SKIN_EXPR_COURSE_TABLE_TEXT.to_string());
    }

    let option_calls = collect_option_calls(function, main_state_probe)?;
    if !option_calls.contains(&290) {
        return None;
    }

    {
        main_state_probe.lock().ok()?.begin_number_call_recording(0);
    }
    let _ = function.call::<Value>(()).ok();
    let text_calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.text_calls.clone();
        probe.end_recording();
        calls
    };
    if text_calls.iter().any(|ref_id| (1001..=1003).contains(ref_id)) {
        Some(SKIN_EXPR_COURSE_TABLE_TEXT.to_string())
    } else {
        None
    }
}

pub(in crate::lua) fn infer_main_state_text_ref(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i32> {
    {
        main_state_probe.lock().ok()?.begin_number_call_recording(0);
    }
    let _ = function.call::<Value>(()).ok();
    let text_calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.text_calls.clone();
        probe.end_recording();
        calls
    };
    single_number_call(&text_calls)
}

pub(in crate::lua) fn infer_text_concat_expr(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    main_state_probe.lock().ok()?.begin_number_call_recording(0);
    let result = function.call::<Value>(()).ok();
    let text_calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.text_calls.clone();
        probe.end_recording();
        calls
    };
    if text_calls != [1001, 1002] {
        return None;
    }
    let Value::String(text) = result? else {
        return None;
    };
    (text.to_string_lossy() == "Text1001 Text1002").then(|| "bmz:text_concat:1001:1002".to_string())
}

pub(in crate::lua) fn infer_nearest_rank_diff_value_expr(
    function: &Function,
    object_id: Option<&str>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let refs = collect_number_refs(function, main_state_probe)?;
    let supported = match object_id {
        Some("diff_rank") => refs == [71, 74],
        Some("rank_diff_count") => {
            refs.contains(&71)
                && refs.contains(&74)
                && refs.iter().all(|ref_id| matches!(ref_id, 71 | 74 | 170 | 271))
        }
        _ => false,
    };
    if !supported {
        return None;
    }
    for total_notes in [9, 10, 37] {
        for ex_score in 0..=total_notes * 2 {
            let values = refs
                .iter()
                .copied()
                .map(|ref_id| {
                    let value = match ref_id {
                        71 => ex_score,
                        74 => total_notes,
                        _ => 0,
                    };
                    (ref_id, value)
                })
                .collect();
            let actual = call_number_float_with_values(function, main_state_probe, values)?;
            let expected = match object_id {
                Some("rank_diff_count") => {
                    luxe_flat_nearest_rank_diff(ex_score, total_notes)? as f64
                }
                _ => wmii_nearest_rank(ex_score, total_notes)?.2 as f64,
            };
            if !approx_float_eq(actual, expected) {
                return None;
            }
        }
    }
    Some("bmz:nearest_rank_diff_abs".to_string())
}

pub(in crate::lua) fn luxe_flat_nearest_rank_diff(ex_score: i32, total_notes: i32) -> Option<i32> {
    let max = total_notes.checked_mul(2)?;
    if max <= 0 {
        return None;
    }
    let ex_score = ex_score.clamp(0, max);
    if ex_score >= max {
        return Some(0);
    }
    const BOUNDARIES: [i32; 9] = [0, 2, 3, 4, 5, 6, 7, 8, 9];
    let current =
        BOUNDARIES.iter().rposition(|boundary| ex_score * 9 >= *boundary * max).unwrap_or(0);
    let lower = BOUNDARIES[current];
    let upper = *BOUNDARIES.get(current + 1)?;
    let lower_score = (lower * max + 8) / 9;
    let upper_score = (upper * max + 8) / 9;
    if ex_score * 18 < (lower + upper) * max {
        Some((ex_score - lower_score).max(0))
    } else {
        Some((upper_score - ex_score).max(0))
    }
}

pub(in crate::lua) fn infer_result_score_draw(
    function: &Function,
    object_id: Option<&str>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    match object_id? {
        "scoreGraph" => infer_score_rate_band(function, main_state_probe),
        id if id.starts_with("ir_scoreGraph") => {
            infer_ir_score_rate_band(function, id, main_state_probe)
        }
        id if modern_chic_ir_ranking_graph(id).is_some() => {
            infer_modern_chic_ir_score_rate_band(function, id, main_state_probe)
        }
        "irYouFrame" => infer_ir_ranking_user_draw(function, main_state_probe),
        id if id.starts_with("nextRank") => {
            let grade = id.strip_prefix("nextRank")?;
            for sign in ["plus", "minus"] {
                if verify_nearest_rank_draw(function, main_state_probe, Some(grade), sign) {
                    return Some(format!("nearest_rank({grade},{sign})"));
                }
            }
            None
        }
        id if luxe_flat_nearest_rank_destination(id).is_some() => {
            let (grade, sign) = luxe_flat_nearest_rank_destination(id)?;
            Some(format!("nearest_rank({grade},{sign})"))
        }
        "diff_plus" => verify_nearest_rank_draw(function, main_state_probe, None, "plus")
            .then(|| "nearest_rank_sign(plus)".to_string()),
        "diff_minus" => verify_nearest_rank_draw(function, main_state_probe, None, "minus")
            .then(|| "nearest_rank_sign(minus)".to_string()),
        "diff_rank" => ["plus", "minus"].into_iter().find_map(|sign| {
            verify_nearest_rank_draw(function, main_state_probe, None, sign)
                .then(|| format!("nearest_rank_sign({sign})"))
        }),
        _ => None,
    }
}

pub(in crate::lua) fn luxe_flat_nearest_rank_destination(
    id: &str,
) -> Option<(&'static str, &'static str)> {
    let suffix = id.strip_prefix("rank_diff_")?;
    let (grade, sign) = suffix.rsplit_once('_')?;
    let grade = match grade {
        "f" => "F",
        "e" => "E",
        "d" => "D",
        "c" => "C",
        "b" => "B",
        "a" => "A",
        "aa" => "AA",
        "aaa" => "AAA",
        "max" => "MAX",
        _ => return None,
    };
    let sign = match sign {
        "plus" => "plus",
        "minus" => "minus",
        _ => return None,
    };
    Some((grade, sign))
}

pub(in crate::lua) fn infer_result_panel_draw_condition(
    lua: &Lua,
    function: &Function,
    object_id: Option<&str>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    const ALWAYS_TRUE: &str = "number(0) >= 0";
    const ALWAYS_FALSE: &str = "number(0) < 0";

    let globals = lua.globals();
    let global_original = globals
        .raw_get::<Value>("Expand_op")
        .ok()
        .filter(|value| lua_result_panel_value(value.clone()).is_some());
    let local_original = if global_original.is_none() {
        let (index, mode) = lua_result_mode_upvalue(lua, function)?;
        record_local_result_panel_default(main_state_probe, mode)?;
        Some((index, mode))
    } else {
        None
    };

    let mut conditions = Vec::with_capacity(3);
    for panel in 0..=2 {
        let state_updated = if global_original.is_some() {
            globals.raw_set("Expand_op", panel).is_ok()
        } else if let Some((index, _)) = local_original {
            // Luxe Flat: result_mode 0=GRAPH, 1=IR. Use 2 for the inactive
            // BMZ panel state so neither equality branch is selected.
            let mode = match panel {
                1 => 1,
                2 => 0,
                _ => 2,
            };
            set_lua_integer_upvalue(lua, function, index, mode)
        } else {
            false
        };
        if !state_updated {
            restore_result_panel_probe_state(
                lua,
                function,
                global_original.as_ref(),
                local_original,
            );
            return None;
        }
        let specialized = infer_result_score_draw(function, object_id, main_state_probe);
        conditions.push(if result_score_draw_object(object_id) {
            specialized.or_else(|| infer_constant_draw_at_load(function, main_state_probe))
        } else {
            specialized.or_else(|| infer_boolean_predicate(function, main_state_probe, object_id))
        });
    }
    restore_result_panel_probe_state(lua, function, global_original.as_ref(), local_original);

    if conditions.windows(2).all(|pair| pair[0] == pair[1]) {
        return None;
    }

    let branches = conditions
        .into_iter()
        .enumerate()
        .flat_map(|(panel, condition)| match condition.as_deref() {
            None | Some(ALWAYS_FALSE) => Vec::new(),
            Some(ALWAYS_TRUE) => vec![format!("result_panel({panel})")],
            Some(condition) => condition
                .split(" or ")
                .map(|branch| format!("result_panel({panel}) and {branch}"))
                .collect(),
        })
        .collect::<Vec<_>>();
    (!branches.is_empty()).then(|| branches.join(" or "))
}

pub(in crate::lua) fn restore_result_panel_probe_state(
    lua: &Lua,
    function: &Function,
    global_original: Option<&Value>,
    local_original: Option<(i32, i32)>,
) {
    if let Some(original) = global_original {
        let _ = lua.globals().raw_set("Expand_op", original.clone());
    } else if let Some((index, mode)) = local_original {
        let _ = set_lua_integer_upvalue(lua, function, index, mode);
    }
}

pub(in crate::lua) fn result_score_draw_object(object_id: Option<&str>) -> bool {
    object_id.is_some_and(|id| {
        id == "scoreGraph"
            || id.starts_with("ir_scoreGraph")
            || id == "irYouFrame"
            || id.starts_with("nextRank")
            || matches!(id, "diff_plus" | "diff_minus" | "diff_rank")
    })
}

pub(in crate::lua) fn ir_ranking_slot_from_id(id: &str, prefix: &str) -> Option<i32> {
    let slot = id.strip_prefix(prefix)?.parse::<i32>().ok()?;
    (1..=10).contains(&slot).then_some(slot)
}

pub(in crate::lua) fn modern_chic_ir_ranking_graph(id: &str) -> Option<(i32, &'static str)> {
    let suffix = id.strip_prefix("s_rankingGraph")?;
    let digit_start = suffix.find(|character: char| character.is_ascii_digit())?;
    let (rank, slot) = suffix.split_at(digit_start);
    let rank = match rank {
        "AAA" => "AAA",
        "AA" => "AA",
        "A" => "A",
        "B" => "B",
        "C" => "C",
        "D" => "D",
        "E" => "E",
        "F" => "F",
        _ => return None,
    };
    let slot = slot.parse::<i32>().ok()?;
    (1..=10).contains(&slot).then_some((slot, rank))
}

pub(in crate::lua) fn collect_text_refs(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<Vec<i32>> {
    main_state_probe.lock().ok()?.begin_number_call_recording(0);
    let _ = function.call::<Value>(()).ok();
    let mut calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.text_calls.clone();
        probe.end_recording();
        calls
    };
    calls.sort_unstable();
    calls.dedup();
    Some(calls)
}

pub(in crate::lua) fn infer_ir_ranking_name_ref(
    function: &Function,
    object_id: Option<&str>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i32> {
    let slot = ir_ranking_slot_from_id(object_id?, "ir_username")?;
    let expected_ref = 119 + slot;
    let refs = collect_text_refs(function, main_state_probe)?;
    (refs.contains(&expected_ref)
        && refs.iter().all(|ref_id| matches!(*ref_id, 1021) || *ref_id == expected_ref))
    .then_some(expected_ref)
}

pub(in crate::lua) fn infer_ir_ranking_user_draw(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let refs = collect_text_refs(function, main_state_probe)?;
    let ranking_ref = refs.iter().copied().find(|ref_id| (120..=129).contains(ref_id))?;
    if !refs.iter().all(|ref_id| matches!(*ref_id, 1021) || *ref_id == ranking_ref) {
        return None;
    }
    let own = call_draw_with_text_values(
        function,
        main_state_probe,
        BTreeMap::from([(ranking_ref, "same".to_string()), (1021, "same".to_string())]),
    )?;
    let other = call_draw_with_text_values(
        function,
        main_state_probe,
        BTreeMap::from([(ranking_ref, "ranking".to_string()), (1021, "player".to_string())]),
    )?;
    (own && !other).then(|| format!("ir_ranking_user({})", ranking_ref - 119))
}

pub(in crate::lua) fn call_draw_with_text_values(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    values: BTreeMap<i32, String>,
) -> Option<bool> {
    main_state_probe.lock().ok()?.begin_text_recording_with_values(values);
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(in crate::lua) fn infer_ir_ranking_score_value_expr(
    function: &Function,
    object_id: Option<&str>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let object_id = object_id?;
    let modern_chic_slot = modern_chic_ir_ranking_graph(object_id).map(|(slot, _)| slot);
    let slot = ir_ranking_slot_from_id(object_id, "ir_scoreGraph").or(modern_chic_slot)?;
    let score_ref = 379 + slot;
    if collect_number_refs(function, main_state_probe)? != [74, score_ref] {
        return None;
    }
    let mut samples = vec![(100, 0), (100, 123), (100, 200), (2151, 4155)];
    if modern_chic_slot.is_some() {
        samples.insert(0, (100, i32::MIN));
    }
    for (notes, score) in samples {
        let actual = call_number_float_with_values(
            function,
            main_state_probe,
            BTreeMap::from([(74, notes), (score_ref, score)]),
        )?;
        let expected = if score == i32::MIN { 0.0 } else { score as f64 / (notes * 2) as f64 };
        if !approx_float_eq(actual, expected) {
            return None;
        }
    }
    Some(format!("bmz:ir_score_rate:{slot}"))
}

pub(in crate::lua) fn infer_ir_ranking_score_diff_value_expr(
    function: &Function,
    object_id: Option<&str>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let slot = ir_ranking_slot_from_id(object_id?, "ir_diff_score")?;
    let ranking_ref = 379 + slot;
    if collect_number_refs(function, main_state_probe)? != [170, 171, ranking_ref] {
        return None;
    }
    for (old_score, new_score, ranking_score) in
        [(0, 0, 0), (2293, 2284, 2293), (2200, 2284, 2293), (2300, 2284, 2293)]
    {
        let actual = call_number_expr_with_values(
            function,
            main_state_probe,
            BTreeMap::from([(170, old_score), (171, new_score), (ranking_ref, ranking_score)]),
        )?;
        let expected = old_score.max(new_score) - ranking_score;
        if actual != i64::from(expected) {
            return None;
        }
    }
    Some(format!("bmz:ir_score_diff:{slot}"))
}

pub(in crate::lua) fn infer_ir_ranking_score_rate_value_expr(
    function: &Function,
    object_id: Option<&str>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let object_id = object_id?;
    let (slot, part) = if let Some(slot) = object_id.strip_prefix("ir_scorerate_dot") {
        (slot.parse::<i32>().ok()?, "fraction")
    } else {
        let slot = object_id.strip_prefix("ir_scorerate")?;
        (slot.parse::<i32>().ok()?, "integer")
    };
    if !(1..=10).contains(&slot) {
        return None;
    }
    let score_ref = 379 + slot;
    if collect_number_refs(function, main_state_probe)? != [74, score_ref] {
        return None;
    }
    for (notes, score) in [(0, 0), (100, 0), (100, 123), (100, 200), (2151, 4155)] {
        let actual = call_number_float_with_values(
            function,
            main_state_probe,
            BTreeMap::from([(74, notes), (score_ref, score)]),
        )?;
        let expected = if notes <= 0 || score <= 0 {
            0.0
        } else if part == "integer" {
            (score as f64 / (notes * 2) as f64 * 100.0).floor()
        } else {
            (score as f64 / (notes * 2) as f64 * 10_000.0) % 100.0
        };
        if !approx_float_eq(actual, expected) {
            return None;
        }
    }
    Some(format!("bmz:ir_score_rate_{part}:{slot}"))
}

pub(in crate::lua) fn infer_ir_score_rate_band(
    function: &Function,
    object_id: &str,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let slot = ir_ranking_slot_from_id(object_id, "ir_scoreGraph")?;
    let score_ref = 379 + slot;
    if collect_number_refs(function, main_state_probe)? != [74, score_ref] {
        return None;
    }
    for lower in 0..=9 {
        for upper in lower + 1..=10 {
            let mut matches = true;
            'samples: for total_notes in [9, 10, 37] {
                let max = total_notes * 2;
                for ex_score in 0..=max {
                    let actual = call_draw_with_numbers(
                        function,
                        main_state_probe,
                        BTreeMap::from([(74, total_notes), (score_ref, ex_score)]),
                    );
                    let expected = 9 * ex_score >= lower * max && 9 * ex_score < upper * max;
                    if actual != Some(expected) {
                        matches = false;
                        break 'samples;
                    }
                }
            }
            if matches {
                return Some(format!("ir_score_rate_band({slot},{lower},{upper})"));
            }
        }
    }
    None
}

pub(in crate::lua) fn modern_chic_ir_rate_bounds(rank: &str) -> Option<(i64, i64)> {
    match rank {
        "AAA" => Some((888, 1000)),
        "AA" => Some((777, 888)),
        "A" => Some((666, 777)),
        "B" => Some((555, 666)),
        "C" => Some((444, 555)),
        "D" => Some((333, 444)),
        "E" => Some((222, 333)),
        "F" => Some((-10, 222)),
        _ => None,
    }
}

pub(in crate::lua) fn infer_modern_chic_ir_score_rate_band(
    function: &Function,
    object_id: &str,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let (slot, rank) = modern_chic_ir_ranking_graph(object_id)?;
    let score_ref = 379 + slot;
    if collect_number_refs(function, main_state_probe)? != [74, score_ref]
        || collect_option_calls(function, main_state_probe)? != [51]
    {
        return None;
    }
    let (lower, upper) = modern_chic_ir_rate_bounds(rank)?;
    for online in [false, true] {
        for total_notes in [10, 37] {
            let max_score = total_notes * 2;
            for ex_score in 0..=max_score {
                let actual = call_draw_with_numbers_and_options(
                    function,
                    main_state_probe,
                    BTreeMap::from([(74, total_notes), (score_ref, ex_score)]),
                    BTreeMap::from([(51, online)]),
                )?;
                let expected = online
                    && i64::from(ex_score) * 1000 > lower * i64::from(max_score)
                    && i64::from(ex_score) * 1000 <= upper * i64::from(max_score);
                if actual != expected {
                    return None;
                }
            }
        }
    }
    Some(format!("option(51) and ir_score_rate_range({slot},{lower},{upper})"))
}

pub(in crate::lua) fn infer_score_rate_band(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    if collect_number_refs(function, main_state_probe)? != [71, 74] {
        return None;
    }
    for lower in 0..=9 {
        for upper in lower + 1..=10 {
            let mut matches = true;
            'samples: for total_notes in [9, 10, 37] {
                let max = total_notes * 2;
                for ex_score in 0..=max {
                    let actual = call_draw_with_numbers(
                        function,
                        main_state_probe,
                        BTreeMap::from([(71, ex_score), (74, total_notes)]),
                    );
                    let expected = 9 * ex_score >= lower * max && 9 * ex_score < upper * max;
                    if actual != Some(expected) {
                        matches = false;
                        break 'samples;
                    }
                }
            }
            if matches {
                return Some(format!("score_rate_band({lower},{upper})"));
            }
        }
    }
    None
}

pub(in crate::lua) fn verify_nearest_rank_draw(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    grade: Option<&str>,
    sign: &str,
) -> bool {
    if collect_number_refs(function, main_state_probe).as_deref() != Some(&[71, 74]) {
        return false;
    }
    for total_notes in [9, 10, 37] {
        for ex_score in 0..=total_notes * 2 {
            let Some((actual_grade, actual_sign, _)) = wmii_nearest_rank(ex_score, total_notes)
            else {
                return false;
            };
            let expected = grade.is_none_or(|grade| grade == actual_grade) && sign == actual_sign;
            if call_draw_with_numbers(
                function,
                main_state_probe,
                BTreeMap::from([(71, ex_score), (74, total_notes)]),
            ) != Some(expected)
            {
                return false;
            }
        }
    }
    true
}

pub(in crate::lua) fn wmii_nearest_rank(
    ex_score: i32,
    total_notes: i32,
) -> Option<(&'static str, &'static str, i32)> {
    let max = total_notes.checked_mul(2)?;
    if max <= 0 {
        return None;
    }
    let ex_score = ex_score.clamp(0, max);
    const RANKS: [(&str, i32); 9] = [
        ("F", 0),
        ("E", 2),
        ("D", 3),
        ("C", 4),
        ("B", 5),
        ("A", 6),
        ("AA", 7),
        ("AAA", 8),
        ("MAX", 9),
    ];
    if ex_score >= max {
        return Some(("MAX", "plus", 0));
    }
    let current = RANKS.iter().rposition(|(_, ninths)| ex_score * 9 >= ninths * max).unwrap_or(0);
    let (grade, lower) = RANKS[current];
    let (next_grade, upper) = RANKS.get(current + 1).copied().unwrap_or((grade, lower));
    let lower_score = (lower * max + 8) / 9;
    let upper_score = (upper * max + 8) / 9;
    let lower_diff = (ex_score - lower_score).max(0);
    let upper_diff = (upper_score - ex_score).max(0);
    if lower_diff <= upper_diff {
        Some((grade, "plus", lower_diff))
    } else {
        Some((next_grade, "minus", upper_diff))
    }
}

pub(in crate::lua) fn call_draw_with_float_and_number(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    float_ref: i32,
    float_value: f64,
    number_ref: i32,
    number_value: i32,
) -> Option<bool> {
    {
        main_state_probe.lock().ok()?.begin_draw_probe(
            BTreeMap::from([(number_ref, number_value)]),
            BTreeMap::from([(float_ref, float_value)]),
        );
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(in crate::lua) fn infer_float_number_and_number_and_draw(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let float_refs = collect_float_number_refs(function, main_state_probe)?;
    let number_refs = collect_number_refs(function, main_state_probe)?;
    if float_refs.len() != 1 || number_refs.len() != 1 {
        return None;
    }
    let float_ref = float_refs[0];
    let number_ref = number_refs[0];
    let zero_zero =
        call_draw_with_float_and_number(function, main_state_probe, float_ref, 0.0, number_ref, 0);
    let zero_pos =
        call_draw_with_float_and_number(function, main_state_probe, float_ref, 0.0, number_ref, 5);
    let pos_pos =
        call_draw_with_float_and_number(function, main_state_probe, float_ref, 1.0, number_ref, 5);
    if zero_pos == Some(true) && zero_zero == Some(false) && pos_pos == Some(false) {
        return Some(format!("float_number({float_ref}) == 0 && number({number_ref}) != 0"));
    }
    if pos_pos == Some(true) && zero_pos == Some(false) && zero_zero == Some(false) {
        return Some(format!("float_number({float_ref}) != 0 && number({number_ref}) != 0"));
    }
    if zero_zero == Some(true) && zero_pos == Some(false) && pos_pos == Some(false) {
        return Some(format!("number({number_ref}) == 0"));
    }
    None
}

pub(in crate::lua) fn collect_float_number_refs(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<Vec<i32>> {
    let mut calls = Vec::new();
    for float_value in [0.0_f64, 1.0] {
        {
            main_state_probe
                .lock()
                .ok()?
                .begin_draw_probe(BTreeMap::new(), BTreeMap::from([(113, float_value)]));
        }
        let _ = function.call::<Value>(()).ok();
        {
            let mut probe = main_state_probe.lock().ok()?;
            calls.extend(probe.float_number_calls.iter().copied());
            probe.end_recording();
        }
    }
    calls.sort_unstable();
    calls.dedup();
    (!calls.is_empty()).then_some(calls)
}

pub(in crate::lua) fn format_number_sum_expr(refs: &[i32]) -> String {
    refs.iter().map(|ref_id| format!("number({ref_id})")).collect::<Vec<_>>().join("+")
}
