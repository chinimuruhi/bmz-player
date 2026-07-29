use super::*;

pub(super) fn register_runtime_draw_path(
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    field_path: &str,
) -> Result<usize> {
    let mut probe =
        main_state_probe.lock().map_err(|_| anyhow!("main_state probe lock poisoned"))?;
    let callback_id = probe.runtime_draw_paths.len();
    probe.runtime_draw_paths.push(field_path.to_string());
    Ok(callback_id)
}

pub(super) fn infer_gauge_value_digit_draw_condition(
    function: &Function,
    object_id: Option<&str>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let id = object_id?;
    if matches!(
        id,
        "Number_Remaingauge_Max_1"
            | "Number_Remaingauge_Max_00"
            | "Number_Remaingauge_Normal"
            | "Parts_Text_Remaingauge_Dot"
            | "Number_Remaingauge_Afterdot"
    ) {
        let below_max =
            call_draw_with_numbers(function, main_state_probe, BTreeMap::from([(107, 99)]))?;
        let at_max =
            call_draw_with_numbers(function, main_state_probe, BTreeMap::from([(107, 100)]))?;
        return match (below_max, at_max) {
            (false, true) => Some("number(107) == 100".to_string()),
            (true, false) => Some("number(107) < 100".to_string()),
            _ => None,
        };
    }
    let mut probe = main_state_probe.lock().ok()?;
    let mode = match id {
        "val-gauge-percent-integer" | "val-gauge-percent-fraction" | "gauge-value-percent" => {
            probe.gauge_value_overlay_mode = Some("percent");
            "percent"
        }
        "val-gauge-amount-integer" | "val-gauge-amount-fraction" => {
            probe.gauge_value_overlay_mode = Some("amount");
            "amount"
        }
        "gauge-value-dot" => probe.gauge_value_overlay_mode?,
        _ => return None,
    };
    let occurrence = probe.gauge_value_destination_occurrences.entry(id.to_string()).or_default();
    *occurrence += 1;
    let digits = ((*occurrence - 1) % 3) + 1;
    Some(format!("gauge_value_digits({mode},{digits})"))
}

pub(super) fn infer_select_score_available_draw_condition(
    lua: &Lua,
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let globals = lua.globals();
    #[derive(Clone, Copy)]
    enum ScoreGuard {
        Global(bool),
        Upvalue(i32, bool),
    }
    let guard = match globals.get::<Value>("flag_score").ok() {
        Some(Value::Boolean(original)) => ScoreGuard::Global(original),
        _ => {
            let (index, original) = lua_flag_score_upvalue(lua, function)?;
            ScoreGuard::Upvalue(index, original)
        }
    };
    let original = match guard {
        ScoreGuard::Global(value) | ScoreGuard::Upvalue(_, value) => value,
    };
    let set_guard = |value: bool| match guard {
        ScoreGuard::Global(_) => globals.set("flag_score", value).is_ok(),
        ScoreGuard::Upvalue(index, _) => set_lua_boolean_upvalue(lua, function, index, value),
    };
    let evaluate = |value: bool| -> Option<bool> {
        set_guard(value).then_some(())?;
        main_state_probe.lock().ok()?.end_recording();
        function.call::<bool>(()).ok()
    };
    let when_unavailable = evaluate(false);
    let when_available = evaluate(true);
    let _ = set_guard(original);

    match (when_unavailable, when_available) {
        (Some(false), Some(true)) => Some("select_score_available()".to_string()),
        _ => None,
    }
}

pub(super) fn repair_result_table_title_text(path: &str, object: &mut JsonMap<String, JsonValue>) {
    if !path.contains(".text[") || object.get("id").and_then(JsonValue::as_str) != Some("title") {
        return;
    }
    let expected = format!(
        "{LUA_TEXT_REF_SENTINEL_PREFIX}1002{LUA_TEXT_REF_SENTINEL_SUFFIX} \
         {LUA_TEXT_REF_SENTINEL_PREFIX}1001{LUA_TEXT_REF_SENTINEL_SUFFIX} "
    );
    if object.get("constantText").and_then(JsonValue::as_str) != Some(expected.as_str()) {
        return;
    }
    object.remove("constantText");
    object.insert(
        "value_expr".to_string(),
        JsonValue::String(SKIN_EXPR_RESULT_TABLE_TITLE.to_string()),
    );
}

/// ModernChic Result builds `bottomCourse` by evaluating
/// `main_state.text(FULLTITLE)` while the Lua skin is loaded.  The value is
/// runtime state in beatoraja, so replace the empty load-time stub with the
/// corresponding text ref rather than permanently baking in an empty string.
pub(super) fn repair_result_course_title_text(path: &str, object: &mut JsonMap<String, JsonValue>) {
    if !path.contains(".text[")
        || object.get("id").and_then(JsonValue::as_str) != Some("bottomCourse")
        || object.get("constantText").and_then(JsonValue::as_str) != Some("")
    {
        return;
    }
    object.remove("constantText");
    object.insert("ref".to_string(), JsonValue::Number(JsonNumber::from(12)));
}

/// Some select skins keep the score-rate digits visible for both songs and
/// courses, but gate the shared decimal-point/percent sprite with SONGBAR only.
/// Grade bars expose the same score-rate refs, so extend that punctuation
/// destination to GRADEBAR without changing the third-party skin file.
pub(super) fn repair_select_score_rate_punctuation(root: &mut JsonMap<String, JsonValue>) {
    let has_value = |id: &str, ref_id: i64| {
        root.get("value").and_then(JsonValue::as_array).is_some_and(|values| {
            values.iter().any(|value| {
                value.get("id").and_then(JsonValue::as_str) == Some(id)
                    && value.get("ref").and_then(JsonValue::as_i64) == Some(ref_id)
            })
        })
    };
    if !has_value("scorerate_count", 102) || !has_value("scorerate_dot_count", 103) {
        return;
    }
    let Some(destinations) = root.get_mut("destination").and_then(JsonValue::as_array_mut) else {
        return;
    };
    for destination in destinations {
        let Some(object) = destination.as_object_mut() else {
            continue;
        };
        if object.get("id").and_then(JsonValue::as_str) == Some("score_per")
            && object.get("op")
                == Some(&JsonValue::Array(vec![JsonValue::Number(JsonNumber::from(2))]))
        {
            object.remove("op");
            object.insert(
                "draw".to_string(),
                JsonValue::String("option(2) or option(3)".to_string()),
            );
        }
    }
}

#[cfg(test)]
mod result_course_title_repair_tests {
    use super::*;

    #[test]
    fn replaces_empty_load_time_course_title_with_full_title_ref() {
        let mut object = JsonMap::from_iter([
            ("id".to_string(), JsonValue::String("bottomCourse".to_string())),
            ("constantText".to_string(), JsonValue::String(String::new())),
        ]);

        repair_result_course_title_text("$.text[2]", &mut object);

        assert_eq!(object.get("ref"), Some(&JsonValue::Number(JsonNumber::from(12))));
        assert!(!object.contains_key("constantText"));
    }

    #[test]
    fn preserves_unrelated_empty_constant_text() {
        let mut object = JsonMap::from_iter([
            ("id".to_string(), JsonValue::String("other".to_string())),
            ("constantText".to_string(), JsonValue::String(String::new())),
        ]);

        repair_result_course_title_text("$.text[2]", &mut object);

        assert_eq!(object.get("constantText"), Some(&JsonValue::String(String::new())));
        assert!(!object.contains_key("ref"));
    }
}

#[cfg(test)]
mod select_score_rate_punctuation_repair_tests {
    use super::*;

    #[test]
    fn extends_song_score_rate_punctuation_to_course_rows() {
        let mut root = serde_json::from_value::<JsonMap<String, JsonValue>>(serde_json::json!({
            "value": [
                { "id": "scorerate_count", "ref": 102 },
                { "id": "scorerate_dot_count", "ref": 103 }
            ],
            "destination": [{ "id": "score_per", "op": [2], "dst": [] }]
        }))
        .unwrap();

        repair_select_score_rate_punctuation(&mut root);
        let object = root["destination"][0].as_object().unwrap();

        assert_eq!(object.get("draw").and_then(JsonValue::as_str), Some("option(2) or option(3)"));
        assert!(!object.contains_key("op"));
    }

    #[test]
    fn preserves_unrelated_song_only_destination() {
        let mut root = serde_json::from_value::<JsonMap<String, JsonValue>>(serde_json::json!({
            "value": [{ "id": "other", "ref": 102 }],
            "destination": [{ "id": "score_per", "op": [2], "dst": [] }]
        }))
        .unwrap();

        repair_select_score_rate_punctuation(&mut root);
        let object = root["destination"][0].as_object().unwrap();

        assert!(!object.contains_key("draw"));
        assert!(object.contains_key("op"));
    }
}

pub(super) fn keylogger_graph_value_expr_from_id(id: &str) -> Option<String> {
    let rest = id.strip_prefix("keylogger-graph-")?;
    let mut parts = rest.split('-');
    let graph_kind = parts.next()?;
    let lane = parts.next()?.parse::<usize>().ok()?;
    let layer = parts.next()?;
    if parts.next().is_some()
        || !matches!(graph_kind, "judge" | "fastslow")
        || lane == 0
        || !matches!(layer, "cool" | "great" | "good" | "bad" | "fast" | "slow")
    {
        return None;
    }
    Some(format!("bmz:keylogger_graph:{graph_kind}:{lane}:{layer}"))
}

pub(super) fn milliondollar_fast_slow_graph_value_expr_from_id(id: &str) -> Option<String> {
    let numerator = match id {
        "Graph_Totalfastslow_Fast" => {
            "option(928)*number(423)+(1-option(928))*(number(423)+number(410))"
        }
        "Graph_Totalfastslow_Slow" => {
            "option(928)*number(424)+(1-option(928))*(number(424)+number(411))"
        }
        _ => return None,
    };
    Some(format!(
        "({numerator})/(number(110)+number(111)+number(112)+number(113)+number(114)+number(420))"
    ))
}

pub(super) fn parse_keylogger_destination_id(
    id: &str,
) -> Option<(&'static str, usize, Option<&str>)> {
    if let Some(rest) = id.strip_prefix("keylogger-note-judge-") {
        let (lane, kind) = rest.split_once('-')?;
        return Some(("judge", lane.parse().ok()?, Some(kind)));
    }
    if let Some(rest) = id.strip_prefix("keylogger-note-fastslow-") {
        let (lane, kind) = rest.split_once('-')?;
        return Some(("fastslow", lane.parse().ok()?, Some(kind)));
    }
    let lane = id.strip_prefix("keylogger-note-")?.parse().ok()?;
    Some(("plain", lane, None))
}

pub(super) fn lua_object_id(entries: &[(Value, Value)]) -> Option<String> {
    entries.iter().find_map(|(key, value)| {
        if !matches!(key, Value::String(key) if key.to_string_lossy() == "id") {
            return None;
        }
        match value {
            Value::String(value) => Some(value.to_string_lossy()),
            Value::Integer(value) => Some(value.to_string()),
            Value::Number(value) if value.is_finite() => Some(value.to_string()),
            _ => None,
        }
    })
}

pub(super) fn infer_main_state_number_ref(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i32> {
    const SENTINEL: i32 = 1_000_000;
    {
        main_state_probe.lock().ok()?.begin_number_recording(SENTINEL);
    }
    let result = function.call::<Value>(()).ok();
    let calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.number_calls.clone();
        probe.end_recording();
        calls
    };
    let ref_id = single_number_call(&calls)?;
    match result? {
        Value::Integer(value) if value == i64::from(SENTINEL + ref_id) => Some(ref_id),
        Value::Number(value) if (value - f64::from(SENTINEL + ref_id)).abs() < f64::EPSILON => {
            Some(ref_id)
        }
        _ => None,
    }
}

/// Rm-skin `getDummyNumber(ref)` — `number(101) < 1` なら 0、でなければ `number(ref)`。
pub(super) fn infer_gated_number_ref(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i32> {
    const GATE_REF: i32 = 101;
    let refs = collect_number_refs(function, main_state_probe)?;
    if !refs.contains(&GATE_REF) {
        return None;
    }
    let target = if refs.len() == 1 {
        GATE_REF
    } else if refs.len() == 2 {
        if refs[0] == GATE_REF && refs[1] == GATE_REF {
            GATE_REF
        } else {
            refs.iter().copied().find(|ref_id| *ref_id != GATE_REF)?
        }
    } else {
        return None;
    };
    let gated_off =
        call_number_expr_with_values(function, main_state_probe, BTreeMap::from([(GATE_REF, 0)]))?;
    if gated_off != 0 {
        return None;
    }
    let mut open_values = BTreeMap::from([(GATE_REF, 5), (target, 7)]);
    if target == GATE_REF {
        open_values.insert(GATE_REF, 7);
    }
    let open_on = call_number_expr_with_values(function, main_state_probe, open_values.clone())?;
    if open_on != 7 {
        return None;
    }
    open_values.insert(target, 0);
    let open_zero = call_number_expr_with_values(function, main_state_probe, open_values)?;
    if open_zero != 0 {
        return None;
    }
    Some(target)
}

pub(super) fn infer_main_state_number_expr(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    {
        main_state_probe.lock().ok()?.begin_number_call_recording(0);
    }
    let _ = function.call::<Value>(()).ok();
    let calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.number_calls.clone();
        probe.end_recording();
        calls
    };
    let mut refs = calls;
    refs.sort_unstable();
    refs.dedup();
    if refs.is_empty() || refs.len() > 12 {
        return None;
    }
    let baseline = call_number_expr_with_values(function, main_state_probe, BTreeMap::new())?;
    let mut terms = Vec::new();
    for ref_id in refs {
        let value = call_number_expr_with_values(
            function,
            main_state_probe,
            BTreeMap::from([(ref_id, 1)]),
        )?;
        let coefficient = value - baseline;
        if coefficient != 0 {
            terms.push((ref_id, coefficient));
        }
    }
    if terms.is_empty() {
        return None;
    }
    Some(format_number_expr(baseline, &terms))
}

pub(super) fn call_number_expr_with_values(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    values: BTreeMap<i32, i32>,
) -> Option<i64> {
    {
        main_state_probe.lock().ok()?.begin_number_recording_with_values(values);
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Integer(value) => Some(value),
        Value::Number(value) if value.is_finite() && value.fract() == 0.0 => Some(value as i64),
        _ => None,
    }
}

pub(super) fn format_number_expr(constant: i64, terms: &[(i32, i64)]) -> String {
    let mut parts = Vec::new();
    if constant != 0 {
        parts.push(constant.to_string());
    }
    for (ref_id, coefficient) in terms {
        let sign = if *coefficient < 0 { "-" } else { "+" };
        let magnitude = coefficient.unsigned_abs();
        let term = if magnitude == 1 {
            format!("number({ref_id})")
        } else {
            format!("{magnitude}*number({ref_id})")
        };
        if parts.is_empty() {
            parts.push(if *coefficient < 0 { format!("-{term}") } else { term });
        } else {
            parts.push(format!("{sign} {term}"));
        }
    }
    parts.join(" ")
}

pub(super) fn infer_main_state_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    {
        main_state_probe.lock().ok()?.begin_number_call_recording(1);
    }
    let _ = function.call::<Value>(()).ok();
    let calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.number_calls.clone();
        probe.end_recording();
        calls
    };
    let ref_id = single_number_call(&calls)?;
    let samples = [-8, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 99];
    let observed = samples
        .iter()
        .map(|sample| call_draw_with_number(function, main_state_probe, ref_id, *sample))
        .collect::<Option<Vec<_>>>()?;

    let candidates = [
        ("== 0", samples.iter().map(|value| *value == 0).collect::<Vec<_>>()),
        ("< 0", samples.iter().map(|value| *value < 0).collect::<Vec<_>>()),
        ("> 0", samples.iter().map(|value| *value > 0).collect::<Vec<_>>()),
        ("!= 0", samples.iter().map(|value| *value != 0).collect::<Vec<_>>()),
        (">= 0", samples.iter().map(|value| *value >= 0).collect::<Vec<_>>()),
        ("<= 0", samples.iter().map(|value| *value <= 0).collect::<Vec<_>>()),
    ];
    if let Some(condition) = candidates.into_iter().find_map(|(operator, expected)| {
        (observed == expected).then(|| format!("number({ref_id}) {operator}"))
    }) {
        return Some(condition);
    }

    for members in [&[1, 3, 5, 7][..], &[2, 4, 6][..]] {
        let expected = samples.iter().map(|value| members.contains(value)).collect::<Vec<_>>();
        if observed == expected {
            return Some(
                members
                    .iter()
                    .map(|value| format!("number({ref_id}) == {value}"))
                    .collect::<Vec<_>>()
                    .join(" or "),
            );
        }
    }
    None
}

pub(super) fn single_number_call(calls: &[i32]) -> Option<i32> {
    let first = *calls.first()?;
    calls.iter().all(|call| *call == first).then_some(first)
}

pub(super) const ARRANGE_EVENT_INDEX_SAMPLES: [i32; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

pub(super) fn call_draw_with_number(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    ref_id: i32,
    value: i32,
) -> Option<bool> {
    {
        main_state_probe.lock().ok()?.begin_number_recording_with_value(ref_id, value);
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(super) fn infer_main_state_event_index_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    {
        main_state_probe.lock().ok()?.begin_event_index_call_recording(0);
    }
    let _ = function.call::<Value>(()).ok();
    let calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.event_index_calls.clone();
        probe.end_recording();
        calls
    };
    let event_id = single_number_call(&calls)?;
    let samples = ARRANGE_EVENT_INDEX_SAMPLES;
    let observed = samples
        .iter()
        .map(|sample| call_draw_with_event_index(function, main_state_probe, event_id, *sample))
        .collect::<Option<Vec<_>>>()?;
    let enabled = samples
        .iter()
        .zip(observed)
        .filter_map(|(value, enabled)| enabled.then_some(*value))
        .collect::<Vec<_>>();
    if enabled.is_empty() || enabled.len() == samples.len() {
        return None;
    }
    Some(
        enabled
            .into_iter()
            .map(|value| format!("event_index({event_id}) == {value}"))
            .collect::<Vec<_>>()
            .join(" or "),
    )
}

pub(super) fn call_draw_with_event_index(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    event_id: i32,
    value: i32,
) -> Option<bool> {
    {
        main_state_probe.lock().ok()?.begin_event_index_recording_with_value(event_id, value);
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(super) fn infer_main_state_event_index_options_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    {
        main_state_probe.lock().ok()?.begin_event_index_call_recording(0);
    }
    let _ = function.call::<Value>(()).ok();
    let event_calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.event_index_calls.clone();
        probe.end_recording();
        calls
    };
    let event_id = single_number_call(&event_calls)?;
    let samples = ARRANGE_EVENT_INDEX_SAMPLES;

    let mut option_ids = Vec::new();
    for event_value in samples {
        {
            main_state_probe.lock().ok()?.begin_event_index_options_recording_with_values(
                event_id,
                event_value,
                BTreeMap::new(),
                false,
            );
        }
        let result = function.call::<Value>(()).ok();
        let mut probe = main_state_probe.lock().ok()?;
        let only_event_and_options = probe.number_calls.is_empty()
            && probe.timer_calls.is_empty()
            && probe.float_number_calls.is_empty()
            && probe.gauge_type_calls == 0
            && probe.event_index_calls.iter().all(|call| *call == event_id);
        option_ids.extend(probe.option_calls.iter().copied());
        probe.end_recording();
        if !only_event_and_options || !matches!(result, Some(Value::Boolean(_))) {
            return None;
        }
    }
    option_ids.sort_unstable();
    option_ids.dedup();
    if option_ids.is_empty() || option_ids.len() > 2 {
        return None;
    }

    let assignment_count = 1usize << option_ids.len();
    let mut branches = Vec::new();
    let mut observed_patterns = Vec::new();
    let mut saw_option_dependent_pattern = false;
    for event_value in samples {
        let mut truth_table = Vec::with_capacity(assignment_count);
        for assignment in 0..assignment_count {
            let option_values = option_ids
                .iter()
                .enumerate()
                .map(|(index, option_id)| (*option_id, assignment & (1 << index) != 0))
                .collect();
            truth_table.push(call_draw_with_event_index_options(
                function,
                main_state_probe,
                event_id,
                event_value,
                option_values,
            )?);
        }
        saw_option_dependent_pattern |= truth_table.windows(2).any(|values| values[0] != values[1]);
        let option_cubes = option_truth_table_cubes(&option_ids, &truth_table)?;
        for cube in option_cubes {
            let mut terms = vec![format!("event_index({event_id}) == {event_value}")];
            terms.extend(cube);
            branches.push(terms.join(" and "));
        }
        observed_patterns.push(truth_table);
    }

    let saw_event_dependent_pattern =
        observed_patterns.windows(2).any(|values| values[0] != values[1]);
    if branches.is_empty() || !saw_option_dependent_pattern || !saw_event_dependent_pattern {
        return None;
    }
    Some(branches.join(" or "))
}

pub(super) fn call_draw_with_event_index_options(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    event_id: i32,
    event_value: i32,
    option_values: BTreeMap<i32, bool>,
) -> Option<bool> {
    {
        main_state_probe.lock().ok()?.begin_event_index_options_recording_with_values(
            event_id,
            event_value,
            option_values,
            false,
        );
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(super) fn option_truth_table_cubes(
    option_ids: &[i32],
    truth_table: &[bool],
) -> Option<Vec<Vec<String>>> {
    match (option_ids, truth_table) {
        ([], [false]) => Some(Vec::new()),
        ([], [true]) => Some(vec![Vec::new()]),
        ([_], [false, false]) => Some(Vec::new()),
        ([_], [true, true]) => Some(vec![Vec::new()]),
        ([option], [false, true]) => Some(vec![vec![format!("option({option})")]]),
        ([option], [true, false]) => Some(vec![vec![format!("!option({option})")]]),
        ([_, _], [false, false, false, false]) => Some(Vec::new()),
        ([_, _], [true, true, true, true]) => Some(vec![Vec::new()]),
        ([a, _], [false, true, false, true]) => Some(vec![vec![format!("option({a})")]]),
        ([a, _], [true, false, true, false]) => Some(vec![vec![format!("!option({a})")]]),
        ([_, b], [false, false, true, true]) => Some(vec![vec![format!("option({b})")]]),
        ([_, b], [true, true, false, false]) => Some(vec![vec![format!("!option({b})")]]),
        ([a, b], [false, false, false, true]) => {
            Some(vec![vec![format!("option({a})"), format!("option({b})")]])
        }
        ([a, b], [false, true, false, false]) => {
            Some(vec![vec![format!("option({a})"), format!("!option({b})")]])
        }
        ([a, b], [false, false, true, false]) => {
            Some(vec![vec![format!("!option({a})"), format!("option({b})")]])
        }
        ([a, b], [true, false, false, false]) => {
            Some(vec![vec![format!("!option({a})"), format!("!option({b})")]])
        }
        ([a, b], [false, true, true, true]) => {
            Some(vec![vec![format!("option({a})")], vec![format!("option({b})")]])
        }
        ([a, b], [true, true, false, true]) => {
            Some(vec![vec![format!("option({a})")], vec![format!("!option({b})")]])
        }
        ([a, b], [true, false, true, true]) => {
            Some(vec![vec![format!("!option({a})")], vec![format!("option({b})")]])
        }
        ([a, b], [true, true, true, false]) => {
            Some(vec![vec![format!("!option({a})")], vec![format!("!option({b})")]])
        }
        ([a, b], [false, true, true, false]) => Some(vec![
            vec![format!("option({a})"), format!("!option({b})")],
            vec![format!("!option({a})"), format!("option({b})")],
        ]),
        ([a, b], [true, false, false, true]) => Some(vec![
            vec![format!("!option({a})"), format!("!option({b})")],
            vec![format!("option({a})"), format!("option({b})")],
        ]),
        _ => None,
    }
}

pub(super) fn infer_main_state_option_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    {
        main_state_probe.lock().ok()?.begin_option_call_recording(true);
    }
    let _ = function.call::<Value>(()).ok();
    let calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.option_calls.clone();
        probe.end_recording();
        calls
    };
    let option_id = single_number_call(&calls)?;
    let off = call_draw_with_option(function, main_state_probe, option_id, false)?;
    let on = call_draw_with_option(function, main_state_probe, option_id, true)?;
    match (off, on) {
        (false, true) => Some(format!("option({option_id})")),
        (true, false) => Some(format!("!option({option_id})")),
        _ => None,
    }
}

pub(super) fn infer_main_state_option_number_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let option_id = single_number_call(&collect_option_calls(function, main_state_probe)?)?;
    let mut number_refs =
        collect_number_refs_with_option_value(function, main_state_probe, option_id, true)?;
    number_refs.extend(collect_number_refs_with_option_value(
        function,
        main_state_probe,
        option_id,
        false,
    )?);
    number_refs.sort_unstable();
    number_refs.dedup();
    let number_ref = single_number_call(&number_refs)?;

    let false_zero =
        call_draw_with_number_option(function, main_state_probe, number_ref, 0, option_id, false)?;
    let false_nonzero =
        call_draw_with_number_option(function, main_state_probe, number_ref, 5, option_id, false)?;
    let true_zero =
        call_draw_with_number_option(function, main_state_probe, number_ref, 0, option_id, true)?;
    let true_nonzero =
        call_draw_with_number_option(function, main_state_probe, number_ref, 5, option_id, true)?;

    match (false_zero, false_nonzero, true_zero, true_nonzero) {
        (false, false, false, true) => {
            Some(format!("option({option_id}) && number({number_ref}) != 0"))
        }
        (false, false, true, false) => {
            Some(format!("option({option_id}) && number({number_ref}) == 0"))
        }
        (false, true, false, false) => {
            Some(format!("!option({option_id}) && number({number_ref}) != 0"))
        }
        (true, false, false, false) => {
            Some(format!("!option({option_id}) && number({number_ref}) == 0"))
        }
        _ => None,
    }
}

pub(super) fn call_draw_with_option(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    option_id: i32,
    value: bool,
) -> Option<bool> {
    {
        main_state_probe.lock().ok()?.begin_option_recording_with_value(option_id, value);
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(super) fn infer_main_state_timer_option_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    {
        main_state_probe.lock().ok()?.begin_timer_option_call_recording();
    }
    let _ = function.call::<Value>(()).ok();
    let (timer_calls, option_calls) = {
        let mut probe = main_state_probe.lock().ok()?;
        let timer_calls = probe.timer_calls.clone();
        let option_calls = probe.option_calls.clone();
        probe.end_recording();
        (timer_calls, option_calls)
    };
    let timer_id = single_number_call(&timer_calls)?;
    let option_id = single_number_call(&option_calls)?;
    let samples =
        [(i32::MIN, false), (i32::MIN, true), (0, false), (0, true), (100, false), (100, true)];
    let observed = samples
        .iter()
        .map(|(timer_value, option_value)| {
            call_draw_with_timer_option(
                function,
                main_state_probe,
                timer_id,
                *timer_value,
                option_id,
                *option_value,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let candidates = [
        (
            format!("timer({timer_id}) == timer_off and option({option_id})"),
            samples
                .iter()
                .map(|(timer_value, option_value)| *timer_value == i32::MIN && *option_value)
                .collect::<Vec<_>>(),
        ),
        (
            format!("timer({timer_id}) != timer_off and option({option_id})"),
            samples
                .iter()
                .map(|(timer_value, option_value)| *timer_value != i32::MIN && *option_value)
                .collect::<Vec<_>>(),
        ),
        (
            format!("timer({timer_id}) > 0 and option({option_id})"),
            samples
                .iter()
                .map(|(timer_value, option_value)| *timer_value > 0 && *option_value)
                .collect::<Vec<_>>(),
        ),
    ];
    candidates
        .into_iter()
        .find_map(|(condition, expected)| (observed == expected).then_some(condition))
}

pub(super) fn infer_main_state_two_options_timer_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let mut option_calls = collect_option_calls(function, main_state_probe)?;
    option_calls.sort_unstable();
    option_calls.dedup();
    if option_calls.len() != 2 {
        return None;
    }
    let option_a = option_calls[0];
    let option_b = option_calls[1];

    // Force both option branches open so a timer hidden behind Lua's short-circuit
    // evaluation is recorded as well.
    let timer_id = {
        let mut probe = main_state_probe.lock().ok()?;
        probe.begin_timer_options_recording_with_values(
            BTreeMap::new(),
            BTreeMap::from([(option_a, false), (option_b, true)]),
        );
        drop(probe);
        let _ = function.call::<Value>(()).ok();
        let mut probe = main_state_probe.lock().ok()?;
        let timer_calls = probe.timer_calls.clone();
        probe.end_recording();
        single_number_call(&timer_calls)?
    };

    let samples = [
        (false, false, i32::MIN),
        (false, false, 100),
        (false, true, i32::MIN),
        (false, true, 100),
        (true, false, i32::MIN),
        (true, false, 100),
        (true, true, i32::MIN),
        (true, true, 100),
    ];
    let observed = samples
        .iter()
        .map(|(a, b, timer)| {
            call_draw_with_timer_options(
                function,
                main_state_probe,
                timer_id,
                *timer,
                [(option_a, *a), (option_b, *b)],
            )
        })
        .collect::<Option<Vec<_>>>()?;
    let expected =
        samples.iter().map(|(a, b, timer)| *a || (*b && *timer == i32::MIN)).collect::<Vec<_>>();
    (observed == expected).then(|| {
        format!("option({option_a}) or option({option_b}) and timer({timer_id}) == timer_off")
    })
}

pub(super) fn infer_end_of_note_shadow_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let timers = collect_timer_refs(function, main_state_probe)?;
    let timer_id = single_number_call(&timers)?;
    if !matches!(timer_id, 143 | 144) {
        return None;
    }

    let refs = collect_number_refs(function, main_state_probe)?;
    if refs.as_slice() != REMAIN_NOTE_REFS {
        return None;
    }

    let samples = [
        (i32::MIN, BTreeMap::from([(106, 0), (110, 0), (111, 0), (112, 0), (113, 0), (114, 0)])),
        (i32::MIN, BTreeMap::from([(106, 5), (110, 5), (111, 0), (112, 0), (113, 0), (114, 0)])),
        (i32::MIN, BTreeMap::from([(106, 5), (110, 2), (111, 1), (112, 1), (113, 0), (114, 0)])),
        (0, BTreeMap::from([(106, 5), (110, 5), (111, 0), (112, 0), (113, 0), (114, 0)])),
        (100, BTreeMap::from([(106, 0), (110, 0), (111, 0), (112, 0), (113, 0), (114, 0)])),
    ];
    for (timer_value, values) in samples {
        let expected = timer_value == i32::MIN && remain_notes_value(&values) == 0;
        let actual = call_draw_with_numbers_and_timers(
            function,
            main_state_probe,
            values,
            BTreeMap::from([(timer_id, timer_value)]),
        )?;
        if actual != expected {
            return None;
        }
    }

    Some(format!("timer({timer_id}) == timer_off and {} == 0", remain_notes_numerator_expr()))
}

pub(super) fn infer_os_clock_after_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let mut first_true_ms = None;
    let mut saw_clock = false;
    let mut saw_false = false;
    for elapsed_ms in (0..=10_000).step_by(100) {
        {
            main_state_probe.lock().ok()?.begin_os_clock_recording(elapsed_ms as f64 / 1000.0);
        }
        let result = function.call::<Value>(()).ok();
        let (clock_calls, value) = {
            let mut probe = main_state_probe.lock().ok()?;
            let clock_calls = probe.os_clock_calls;
            probe.end_recording();
            let value = match result? {
                Value::Boolean(value) => value,
                _ => return None,
            };
            (clock_calls, value)
        };
        saw_clock |= clock_calls > 0;
        if value {
            first_true_ms = Some(elapsed_ms);
            break;
        }
        saw_false = true;
    }
    let first_true_ms = first_true_ms?;
    (saw_clock && saw_false).then(|| format!("timer(0) >= {first_true_ms}"))
}

pub(super) fn infer_os_clock_after_option_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let mut first_option_call_ms = None;
    let mut saw_clock = false;
    let mut saw_false_before_option = false;
    for elapsed_ms in (0..=10_000).step_by(100) {
        {
            main_state_probe.lock().ok()?.begin_os_clock_recording(elapsed_ms as f64 / 1000.0);
        }
        let result = function.call::<Value>(()).ok();
        let (clock_calls, option_calls, value) = {
            let mut probe = main_state_probe.lock().ok()?;
            let clock_calls = probe.os_clock_calls;
            let option_calls = probe.option_calls.clone();
            probe.end_recording();
            let value = match result? {
                Value::Boolean(value) => value,
                _ => return None,
            };
            (clock_calls, option_calls, value)
        };
        saw_clock |= clock_calls > 0;
        if option_calls.is_empty() {
            if !value {
                saw_false_before_option = true;
            }
            continue;
        }
        first_option_call_ms = Some(elapsed_ms);
        break;
    }
    let first_option_ms = first_option_call_ms?;
    if !saw_clock || !saw_false_before_option {
        return None;
    }

    let mut option_ids = Vec::<i32>::new();
    for _ in 0..16 {
        let known_true = option_ids.iter().map(|&option_id| (option_id, true)).collect::<Vec<_>>();
        let (calls, value) = call_draw_with_os_clock_options(
            function,
            main_state_probe,
            first_option_ms,
            &known_true,
            false,
        )?;
        let next_option_id = calls.into_iter().find(|call| !option_ids.contains(call));
        if let Some(option_id) = next_option_id {
            option_ids.push(option_id);
            continue;
        }
        if value && !option_ids.is_empty() {
            let mut condition = format!("timer(0) >= {first_option_ms}");
            for option_id in option_ids {
                condition.push_str(&format!(" and option({option_id})"));
            }
            return Some(condition);
        }
        return None;
    }
    None
}

pub(super) fn call_draw_with_os_clock_options(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    elapsed_ms: i32,
    option_values: &[(i32, bool)],
    default_option_value: bool,
) -> Option<(Vec<i32>, bool)> {
    {
        main_state_probe.lock().ok()?.begin_os_clock_options_recording(
            elapsed_ms as f64 / 1000.0,
            option_values,
            default_option_value,
        );
    }
    let result = function.call::<Value>(()).ok();
    let (calls, value) = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.option_calls.clone();
        probe.end_recording();
        let value = match result? {
            Value::Boolean(value) => value,
            _ => return None,
        };
        (calls, value)
    };
    Some((calls, value))
}

pub(super) fn collect_timer_refs(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<Vec<i32>> {
    {
        main_state_probe.lock().ok()?.begin_timer_call_recording(i32::MIN);
    }
    let _ = function.call::<Value>(()).ok();
    let calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.timer_calls.clone();
        probe.end_recording();
        calls
    };
    let mut timers = calls;
    timers.sort_unstable();
    timers.dedup();
    Some(timers)
}

pub(super) fn infer_all_timers_off_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let timers = collect_timer_refs(function, main_state_probe)?;
    if !(2..=4).contains(&timers.len()) {
        return None;
    }

    for active_mask in 0..(1_usize << timers.len()) {
        let values = timers
            .iter()
            .enumerate()
            .map(|(index, timer_id)| {
                let value =
                    if active_mask & (1 << index) == 0 { i32::MIN } else { 100 + index as i32 };
                (*timer_id, value)
            })
            .collect::<BTreeMap<_, _>>();
        let actual =
            call_draw_with_numbers_and_timers(function, main_state_probe, BTreeMap::new(), values)?;
        if actual != (active_mask == 0) {
            return None;
        }
    }

    Some(
        timers
            .iter()
            .map(|timer_id| format!("timer({timer_id}) == timer_off"))
            .collect::<Vec<_>>()
            .join(" and "),
    )
}

pub(super) fn call_timer_function_with_values(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    timer_values: BTreeMap<i32, i32>,
) -> Option<i32> {
    {
        main_state_probe.lock().ok()?.begin_timer_recording_with_values(timer_values);
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Integer(value) => i32::try_from(value).ok(),
        Value::Number(value) if value.is_finite() && value.fract() == 0.0 => {
            i32::try_from(value as i64).ok()
        }
        _ => None,
    }
}

pub(super) fn call_timer_function_with_values_at_time(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    timer_values: BTreeMap<i32, i32>,
    time_value_us: i32,
) -> Option<i32> {
    {
        let mut probe = main_state_probe.lock().ok()?;
        probe.begin_timer_recording_with_values(timer_values);
        probe.time_value_us = time_value_us;
    }
    let result = function.call::<Value>(()).ok();
    {
        let mut probe = main_state_probe.lock().ok()?;
        probe.time_value_us = 1_000_000;
        probe.end_recording();
    }
    match result? {
        Value::Integer(value) => i32::try_from(value).ok(),
        Value::Number(value) if value.is_finite() && value.fract() == 0.0 => {
            i32::try_from(value as i64).ok()
        }
        _ => None,
    }
}

pub(super) fn event_index_calls_with_timer_values(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    timer_values: BTreeMap<i32, i32>,
) -> Option<Vec<i32>> {
    {
        main_state_probe.lock().ok()?.begin_timer_recording_with_values(timer_values);
    }
    let _ = function.call::<Value>(()).ok();
    let calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.event_index_calls.clone();
        probe.end_recording();
        calls
    };
    Some(calls)
}

pub(super) fn call_draw_with_timer_event(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    timer_values: BTreeMap<i32, i32>,
    event_id: i32,
    event_value: i32,
) -> Option<bool> {
    {
        main_state_probe.lock().ok()?.begin_timer_event_recording_with_values(
            timer_values,
            event_id,
            event_value,
        );
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(super) fn keybeam_hold_timer_for_keyon_timer(timer_id: i32) -> Option<i32> {
    match timer_id {
        100..=109 => Some(timer_id - 30),
        110..=117 => Some(timer_id - 30),
        _ => None,
    }
}

pub(super) fn is_keybeam_keyoff_timer(timer_id: i32) -> bool {
    matches!(timer_id, 120..=137)
}

pub(super) fn infer_keybeam_timer_event_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let timers = collect_timer_refs(function, main_state_probe)?;
    for keyon_timer in timers.iter().copied() {
        let Some(hold_timer) = keybeam_hold_timer_for_keyon_timer(keyon_timer) else {
            continue;
        };
        if !timers.contains(&hold_timer) {
            continue;
        }

        let active_timers = BTreeMap::from([(keyon_timer, 1)]);
        let event_calls =
            event_index_calls_with_timer_values(function, main_state_probe, active_timers.clone())?;
        let event_id = single_number_call(&event_calls)?;
        let samples = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let observed = samples
            .iter()
            .map(|sample| {
                call_draw_with_timer_event(
                    function,
                    main_state_probe,
                    active_timers.clone(),
                    event_id,
                    *sample,
                )
            })
            .collect::<Option<Vec<_>>>()?;
        let enabled = samples
            .iter()
            .zip(observed)
            .filter_map(|(value, enabled)| enabled.then_some(*value))
            .collect::<Vec<_>>();
        if enabled.is_empty() || enabled.len() == samples.len() {
            continue;
        }

        let prefix =
            format!("timer({keyon_timer}) != timer_off and timer({hold_timer}) == timer_off and ");
        return Some(
            enabled
                .into_iter()
                .map(|value| format!("{prefix}event_index({event_id}) == {value}"))
                .collect::<Vec<_>>()
                .join(" or "),
        );
    }
    None
}

pub(super) fn infer_timer_function_ref(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i32> {
    let timers = collect_timer_refs(function, main_state_probe)?;
    for timer_id in timers.into_iter().filter(|timer_id| is_keybeam_keyoff_timer(*timer_id)) {
        let sample = main_state_probe.lock().ok()?.time_value_us.saturating_sub(1);
        if call_timer_function_with_values(
            function,
            main_state_probe,
            BTreeMap::from([(timer_id, sample)]),
        ) == Some(sample)
        {
            return Some(timer_id);
        }
    }
    None
}

/// `source timer timestamp + fixed delay` を返し、delay到達前はtimer-offとなる
/// custom timerだけを限定的にIR化する。
pub(super) fn infer_fixed_delay_timer(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<(i32, i32)> {
    let timers = collect_timer_refs(function, main_state_probe)?;
    let source_timer = *timers.as_slice().first()?;
    if timers.len() != 1 {
        return None;
    }
    let source_time_us = 100_000;
    let returned_start = call_timer_function_with_values_at_time(
        function,
        main_state_probe,
        BTreeMap::from([(source_timer, source_time_us)]),
        i32::MAX / 2,
    )?;
    let delay_us = returned_start.checked_sub(source_time_us)?;
    if delay_us <= 0 || delay_us % 1_000 != 0 {
        return None;
    }
    let delay_ms = delay_us / 1_000;
    if delay_ms > 60_000 {
        return None;
    }
    let before = returned_start.checked_sub(1)?;
    if call_timer_function_with_values_at_time(
        function,
        main_state_probe,
        BTreeMap::from([(source_timer, source_time_us)]),
        before,
    ) != Some(TIMER_OFF_VALUE)
        || call_timer_function_with_values_at_time(
            function,
            main_state_probe,
            BTreeMap::from([(source_timer, source_time_us)]),
            returned_start,
        ) != Some(returned_start)
        || call_timer_function_with_values_at_time(
            function,
            main_state_probe,
            BTreeMap::from([(source_timer, source_time_us)]),
            returned_start.saturating_add(123_000),
        ) != Some(returned_start)
        || call_timer_function_with_values_at_time(
            function,
            main_state_probe,
            BTreeMap::new(),
            returned_start.saturating_add(123_000),
        ) != Some(TIMER_OFF_VALUE)
    {
        return None;
    }
    Some((source_timer, delay_ms))
}

/// 既存 timer の値をそのまま返す custom timer を別 ID の alias としてIR化する。
pub(super) fn infer_custom_timer_alias(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i32> {
    let timers = collect_timer_refs(function, main_state_probe)?;
    let source_timer = *timers.as_slice().first()?;
    if timers.len() != 1 {
        return None;
    }

    for sample in [123_456, 765_432] {
        if call_timer_function_with_values(
            function,
            main_state_probe,
            BTreeMap::from([(source_timer, sample)]),
        ) != Some(sample)
        {
            return None;
        }
    }
    if call_timer_function_with_values(
        function,
        main_state_probe,
        BTreeMap::from([(source_timer, TIMER_OFF_VALUE)]),
    ) != Some(TIMER_OFF_VALUE)
    {
        return None;
    }

    Some(source_timer)
}

pub(super) fn call_draw_with_timer_option(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    timer_id: i32,
    timer_value: i32,
    option_id: i32,
    option_value: bool,
) -> Option<bool> {
    {
        main_state_probe.lock().ok()?.begin_timer_option_recording_with_values(
            timer_id,
            timer_value,
            option_id,
            option_value,
        );
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(super) fn call_draw_with_timer_options(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    timer_id: i32,
    timer_value: i32,
    options: [(i32, bool); 2],
) -> Option<bool> {
    {
        main_state_probe.lock().ok()?.begin_timer_options_recording_with_values(
            BTreeMap::from([(timer_id, timer_value)]),
            BTreeMap::from(options),
        );
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(super) fn infer_main_state_gauge_type_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    {
        main_state_probe.lock().ok()?.begin_gauge_type_call_recording(0);
    }
    let _ = function.call::<Value>(()).ok();
    let calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.gauge_type_calls;
        probe.end_recording();
        calls
    };
    if calls == 0 {
        return None;
    }
    // beatoraja の gauge id 0..=8 を網羅。6/7/8 (CLASS / EXCLASS / EXHARDCLASS) を
    // 含めることで段位ゲージ用の skin 条件 (例: `gauge_type() >= 6`) を取りこぼさない。
    let samples = [0, 1, 2, 3, 4, 5, 6, 7, 8];
    let observed = samples
        .iter()
        .map(|value| call_draw_with_gauge_type(function, main_state_probe, *value))
        .collect::<Option<Vec<_>>>()?;
    let enabled = samples
        .iter()
        .zip(observed)
        .filter_map(|(value, is_enabled)| is_enabled.then_some(*value))
        .collect::<Vec<_>>();
    if enabled.is_empty() {
        return None;
    }
    Some(
        enabled
            .into_iter()
            .map(|value| format!("gauge_type() == {value}"))
            .collect::<Vec<_>>()
            .join(" or "),
    )
}

pub(super) fn call_draw_with_gauge_type(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    value: i32,
) -> Option<bool> {
    {
        main_state_probe.lock().ok()?.begin_gauge_type_recording_with_value(value);
    }
    let result = function.call::<Value>(()).ok();
    main_state_probe.lock().ok()?.end_recording();
    match result? {
        Value::Boolean(value) => Some(value),
        _ => None,
    }
}

pub(super) fn infer_judge_fast_slow_draw_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    object_id: Option<&str>,
) -> Option<String> {
    let object_id = object_id?;
    let suffix = object_id.rsplit_once('_')?.1;
    if !matches!(suffix, "N" | "F" | "S") {
        return None;
    }

    {
        main_state_probe.lock().ok()?.begin_number_call_recording(0);
    }
    let _ = function.call::<Value>(()).ok();
    let calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = unique_numbers_in_order(&probe.number_calls);
        probe.end_recording();
        calls
    };
    if calls.len() != 3 {
        return None;
    }
    let total = calls[0];
    let fast = calls[1];
    let slow = calls[2];

    match suffix {
        "N" if object_id == "PF_N" => {
            Some(format!("number({fast}) == number({slow}) or number({total}) == number({fast})"))
        }
        "N" => Some(format!("number({fast}) == number({slow})")),
        "F" if object_id == "PF_F" => {
            Some(format!("number({fast}) > number({slow}) and number({slow}) >= 1"))
        }
        "F" => Some(format!("number({fast}) > number({slow})")),
        "S" => Some(format!("number({slow}) > number({fast})")),
        _ => None,
    }
}

pub(super) fn unique_numbers_in_order(values: &[i32]) -> Vec<i32> {
    let mut unique = Vec::new();
    for value in values {
        if !unique.contains(value) {
            unique.push(*value);
        }
    }
    unique
}

pub(super) fn is_constant_boolean_condition(condition: &str) -> bool {
    matches!(condition, "number(0) >= 0" | "number(0) < 0")
}

/// `CUSTOMS.some_flag` のようなトップレベル bool 参照を宣言的 runtime flag へ写す。
///
/// 任意テーブルやネストした値は触らず、各 bool を一つずつ反転して callback の結果が
/// 反転・復元する単一依存だけを受理する。これにより描画中の Lua 実行を避けつつ、
/// MILLIONDOLLAR の表示切替 callback を Rust 側の状態へ移せる。
pub(super) fn infer_runtime_boolean_field_observe(
    lua: &Lua,
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let baseline = match function.call::<Value>(()).ok()? {
        Value::Boolean(value) => value,
        _ => return None,
    };
    let customs = lua.globals().get::<Table>("CUSTOMS").ok()?;
    let bool_fields = customs
        .clone()
        .pairs::<Value, Value>()
        .filter_map(|entry| {
            let (key, value) = entry.ok()?;
            let Value::String(key) = key else {
                return None;
            };
            let Value::Boolean(value) = value else {
                return None;
            };
            Some((key.to_str().ok()?.to_string(), value))
        })
        .collect::<Vec<_>>();

    let mut candidates = Vec::new();
    for (field, initial) in bool_fields {
        customs.set(field.as_str(), !initial).ok()?;
        let flipped = function.call::<Value>(()).ok();
        customs.set(field.as_str(), initial).ok()?;
        let restored = function.call::<Value>(()).ok();
        if matches!(flipped, Some(Value::Boolean(value)) if value != baseline)
            && matches!(restored, Some(Value::Boolean(value)) if value == baseline)
        {
            candidates.push((field, initial));
        }
    }
    let [(field, initial)] = candidates.as_slice() else {
        return None;
    };

    let flag_id = {
        let mut probe = main_state_probe.lock().ok()?;
        if let Some(flag) =
            probe.runtime_flags.iter().find(|flag| flag.table == "CUSTOMS" && flag.field == *field)
        {
            flag.id
        } else {
            let id = probe.next_runtime_flag_id;
            probe.next_runtime_flag_id += 1;
            probe.runtime_flags.push(LuaRuntimeFlagProbe {
                id,
                table: "CUSTOMS".to_string(),
                field: field.clone(),
                initial: *initial,
            });
            id
        }
    };
    Some(if baseline == *initial {
        format!("runtime_flag({flag_id})")
    } else {
        format!("not runtime_flag({flag_id})")
    })
}

pub(super) fn lua_runtime_scalar(value: Value) -> Option<LuaRuntimeScalar> {
    match value {
        Value::Boolean(value) => Some(LuaRuntimeScalar::Boolean(value)),
        Value::Integer(value) => Some(LuaRuntimeScalar::Integer(value)),
        Value::Number(value) if value.is_finite() => Some(LuaRuntimeScalar::Number(value)),
        Value::String(value) => Some(LuaRuntimeScalar::String(value.as_bytes().to_vec())),
        _ => None,
    }
}

pub(super) fn lua_audio_action_to_json(action: LuaAudioActionProbe) -> JsonValue {
    let action_name = match action.action {
        LuaAudioActionKindProbe::Play => "play",
        LuaAudioActionKindProbe::Loop => "loop",
        LuaAudioActionKindProbe::Stop => "stop",
    };
    let volume =
        JsonNumber::from_f64(action.volume.clamp(0.0, 1.0)).unwrap_or_else(|| JsonNumber::from(0));
    JsonValue::Object(JsonMap::from_iter([
        ("action".to_string(), JsonValue::String(action_name.to_string())),
        ("path".to_string(), JsonValue::String(action.path)),
        ("volume".to_string(), JsonValue::Number(volume)),
    ]))
}

/// timer が off のとき false、on のとき true になる単一 timer 条件だけを受理する。
pub(super) fn infer_timer_on_condition(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i32> {
    let timers = collect_timer_refs(function, main_state_probe)?;
    let [timer_id] = timers.as_slice() else { return None };
    let off = call_draw_with_numbers_and_timers(
        function,
        main_state_probe,
        BTreeMap::new(),
        BTreeMap::from([(*timer_id, TIMER_OFF_VALUE)]),
    )?;
    let on = call_draw_with_numbers_and_timers(
        function,
        main_state_probe,
        BTreeMap::new(),
        BTreeMap::from([(*timer_id, 123_456)]),
    )?;
    (!off && on).then_some(*timer_id)
}

/// `customEvents.action` を一度だけ sandbox 内で呼び、音声命令以外に
/// `CUSTOMS` の scalar 状態を変えない callback を宣言データへ落とす。
pub(super) fn infer_custom_audio_event_action(
    lua: &Lua,
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<Vec<LuaAudioActionProbe>> {
    let customs = lua.globals().get::<Table>("CUSTOMS").ok();
    let before = customs.as_ref().and_then(customs_scalar_snapshot);
    {
        let mut probe = main_state_probe.lock().ok()?;
        probe.audio_actions.clear();
        probe.capture_audio_actions = true;
    }
    let result = function.call::<Value>(()).ok();
    let actions = {
        let mut probe = main_state_probe.lock().ok()?;
        probe.capture_audio_actions = false;
        probe.take_audio_actions()
    };
    let after = customs.as_ref().and_then(customs_scalar_snapshot);
    if !matches!(result, Some(Value::Nil))
        || actions.is_empty()
        || before.is_some() && before != after
    {
        return None;
    }
    Some(actions)
}

pub(super) fn customs_scalar_snapshot(
    customs: &Table,
) -> Option<BTreeMap<String, LuaRuntimeScalar>> {
    let mut snapshot = BTreeMap::new();
    for entry in customs.clone().pairs::<Value, Value>() {
        let (key, value) = entry.ok()?;
        let Value::String(key) = key else {
            continue;
        };
        let Some(value) = lua_runtime_scalar(value) else {
            continue;
        };
        snapshot.insert(key.to_str().ok()?.to_string(), value);
    }
    Some(snapshot)
}

pub(super) fn restore_customs_scalar_snapshot(
    lua: &Lua,
    customs: &Table,
    snapshot: &BTreeMap<String, LuaRuntimeScalar>,
) -> mlua::Result<()> {
    for (field, value) in snapshot {
        match value {
            LuaRuntimeScalar::Boolean(value) => customs.set(field.as_str(), *value)?,
            LuaRuntimeScalar::Integer(value) => customs.set(field.as_str(), *value)?,
            LuaRuntimeScalar::Number(value) => customs.set(field.as_str(), *value)?,
            LuaRuntimeScalar::String(value) => {
                customs.set(field.as_str(), lua.create_string(value.as_slice())?)?
            }
        }
    }
    Ok(())
}

/// 登録済み `CUSTOMS` bool だけを反転し、二回呼ぶと全 scalar が復元する act を
/// `runtimeEvent` へ変換する。外部副作用を持つ任意 callback は対象にしない。
pub(super) fn infer_runtime_toggle_act(
    lua: &Lua,
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i64> {
    let registered = main_state_probe.lock().ok()?.runtime_flags.clone();
    if registered.is_empty() || registered.iter().any(|flag| flag.table != "CUSTOMS") {
        return None;
    }
    let customs = lua.globals().get::<Table>("CUSTOMS").ok()?;
    let before = customs_scalar_snapshot(&customs)?;
    let first_result = function.call::<Value>(()).ok();
    let after_first = customs_scalar_snapshot(&customs)?;

    let mut changed_flag_ids = Vec::new();
    let mut safe = matches!(first_result, Some(Value::Nil));
    for (field, before_value) in &before {
        let after_value = after_first.get(field);
        if after_value == Some(before_value) {
            continue;
        }
        let (LuaRuntimeScalar::Boolean(before_bool), Some(LuaRuntimeScalar::Boolean(after_bool))) =
            (before_value, after_value)
        else {
            safe = false;
            continue;
        };
        if *after_bool == *before_bool {
            safe = false;
            continue;
        }
        let Some(flag) = registered.iter().find(|flag| flag.field == *field) else {
            safe = false;
            continue;
        };
        changed_flag_ids.push(flag.id);
    }
    if before.len() != after_first.len() || changed_flag_ids.is_empty() {
        safe = false;
    }

    let second_result = function.call::<Value>(()).ok();
    let after_second = customs_scalar_snapshot(&customs);
    let _ = restore_customs_scalar_snapshot(lua, &customs, &before);
    if !safe || !matches!(second_result, Some(Value::Nil)) || after_second.as_ref() != Some(&before)
    {
        return None;
    }

    changed_flag_ids.sort_unstable();
    changed_flag_ids.dedup();
    let event_id = {
        let mut probe = main_state_probe.lock().ok()?;
        if let Some(event_id) = probe.runtime_event_ids_by_flags.get(&changed_flag_ids) {
            *event_id
        } else {
            let event_id = probe.next_runtime_event_id;
            probe.next_runtime_event_id -= 1;
            probe.runtime_event_ids_by_flags.insert(changed_flag_ids.clone(), event_id);
            probe.runtime_events.push((event_id, changed_flag_ids));
            event_id
        }
    };
    Some(i64::from(event_id))
}

/// Starseeker 等が `return is_gauge_iidx` / `return not is_gauge_iidx` と書くが
/// グローバルを定義しないスキン向け。ロード時に真偽を切り替えて EX-HARD/HAZARD 相当へ写す。
pub(super) fn infer_is_gauge_iidx_global_observe(lua: &Lua, function: &Function) -> Option<String> {
    let globals = lua.globals();
    let previous = globals.get::<Value>("is_gauge_iidx").ok();
    let selected_gauge_display = globals
        .get::<Table>("skin_config")
        .ok()
        .and_then(|skin_config| skin_config.get::<Table>("option").ok())
        .and_then(|option| option.get::<i64>("グルーヴゲージ表示").ok());

    fn observe_truth(function: &Function) -> Option<bool> {
        match function.call::<Value>(()).ok()? {
            Value::Boolean(value) => Some(value),
            Value::Nil => Some(false),
            _ => None,
        }
    }

    globals.set("is_gauge_iidx", false).ok()?;
    let when_false = observe_truth(function)?;
    globals.set("is_gauge_iidx", true).ok()?;
    let when_true = observe_truth(function)?;

    if let Some(value) = previous {
        globals.set("is_gauge_iidx", value).ok()?;
    } else {
        globals.raw_remove("is_gauge_iidx").ok()?;
    }

    match (when_false, when_true) {
        (false, true) if selected_gauge_display == Some(930) => Some("number(0) < 0".to_string()),
        (true, false) if selected_gauge_display == Some(930) => Some("number(0) >= 0".to_string()),
        (false, true) => Some("gauge_type() == 4 or gauge_type() == 5".to_string()),
        (true, false) => Some("gauge_type() != 4 and gauge_type() != 5".to_string()),
        _ => None,
    }
}

pub(super) fn infer_boolean_predicate(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    object_id: Option<&str>,
) -> Option<String> {
    // PeacefulPlay のキービーム関数は closure 内に前フレーム状態を持つ。
    // 汎用probeを先に走らせるとその状態が変化し、特に最後のLane9が
    // 定数falseへ畳み込まれるため、対象objectは専用推論を最初に行う。
    if object_id.is_some_and(|id| id.starts_with("key-beam-"))
        && let Some(predicate) =
            infer_keybeam_timer_event_draw_condition(function, main_state_probe)
    {
        return Some(predicate);
    }
    if let Some(predicate) = infer_all_timers_off_draw_condition(function, main_state_probe) {
        return Some(predicate);
    }
    // Probe short-circuit option/timer predicates before simpler single-option
    // inference can collapse them to the first branch alone.
    if let Some(predicate) =
        infer_main_state_two_options_timer_draw_condition(function, main_state_probe)
    {
        return Some(predicate);
    }
    let refs = collect_number_refs(function, main_state_probe).unwrap_or_default();
    infer_result_average_timing_sign_draw_condition(function, main_state_probe)
        .or_else(|| {
            if refs.len() >= 2 {
                infer_or_of_number_gt_zero(function, main_state_probe)
                    .or_else(|| infer_or_of_number_eq_zero(function, main_state_probe))
                    .or_else(|| infer_or_of_number_lt_zero(function, main_state_probe))
                    .or_else(|| infer_two_number_compare_and(function, main_state_probe))
            } else {
                None
            }
        })
        .or_else(|| infer_float_number_and_number_and_draw(function, main_state_probe))
        .or_else(|| infer_main_state_event_index_options_draw_condition(function, main_state_probe))
        .or_else(|| infer_main_state_option_number_draw_condition(function, main_state_probe))
        .or_else(|| infer_main_state_draw_condition(function, main_state_probe))
        .or_else(|| infer_main_state_event_index_draw_condition(function, main_state_probe))
        .or_else(|| infer_main_state_option_draw_condition(function, main_state_probe))
        .or_else(|| infer_main_state_gauge_type_draw_condition(function, main_state_probe))
        .or_else(|| infer_keybeam_timer_event_draw_condition(function, main_state_probe))
        .or_else(|| infer_main_state_timer_option_draw_condition(function, main_state_probe))
        .or_else(|| infer_end_of_note_shadow_draw_condition(function, main_state_probe))
        .or_else(|| infer_os_clock_after_draw_condition(function, main_state_probe))
        .or_else(|| infer_os_clock_after_option_draw_condition(function, main_state_probe))
        .or_else(|| infer_judge_fast_slow_draw_condition(function, main_state_probe, object_id))
        .or_else(|| infer_or_of_number_gt_zero(function, main_state_probe))
        .or_else(|| infer_or_of_number_eq_zero(function, main_state_probe))
        .or_else(|| infer_or_of_number_lt_zero(function, main_state_probe))
        .or_else(|| infer_two_number_compare_and(function, main_state_probe))
        .or_else(|| infer_number_eq_zero_with_constant_tail(function, main_state_probe))
        .or_else(|| infer_constant_draw_at_load(function, main_state_probe))
}

/// `skin_config.option` のみ等、ロード時に結果が決まる draw function を畳み込む。
pub(super) fn infer_constant_draw_at_load(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    main_state_probe.lock().ok()?.end_recording();
    // A single successful call is not evidence of a constant: closures may
    // count invocations or mutate module/upvalue state. Repeated disagreement
    // keeps the function on the runtime fallback path.
    let call = || match function.call::<Value>(()).ok()? {
        Value::Boolean(value) => Some(value),
        _ => None,
    };
    let first = call()?;
    let second = call()?;
    let third = call()?;
    if first != second || second != third {
        return None;
    }
    if first { Some("number(0) >= 0".to_string()) } else { Some("number(0) < 0".to_string()) }
}

pub(super) fn infer_constant_text_at_load(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    main_state_probe.lock().ok()?.end_recording();
    match function.call::<Value>(()).ok()? {
        Value::String(value) => Some(value.to_string_lossy()),
        Value::Integer(value) => Some(value.to_string()),
        Value::Number(value) if value.is_finite() => Some(value.to_string()),
        Value::Boolean(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn infer_constant_text_ref_at_load(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i32> {
    let text = infer_constant_text_at_load(function, main_state_probe)?;
    let ref_id = text
        .strip_prefix(LUA_TEXT_REF_SENTINEL_PREFIX)?
        .strip_suffix(LUA_TEXT_REF_SENTINEL_SUFFIX)?
        .parse::<i32>()
        .ok()?;
    (1001..=1003).contains(&ref_id).then_some(ref_id)
}

pub(super) fn repair_keybeam_destination_draws(
    root: &mut JsonMap<String, JsonValue>,
) -> BTreeSet<usize> {
    let mut repaired = BTreeSet::new();
    let Some(destinations) = root.get_mut("destination").and_then(JsonValue::as_array_mut) else {
        return repaired;
    };
    for index in 0..destinations.len().saturating_sub(1) {
        let Some((hold_draw, fade_draw, fade_timer)) =
            keybeam_draw_replacements(&destinations[index], &destinations[index + 1])
        else {
            continue;
        };
        if let JsonValue::Object(destination) = &mut destinations[index] {
            destination.insert("draw".to_string(), JsonValue::String(hold_draw));
        }
        if let JsonValue::Object(destination) = &mut destinations[index + 1] {
            destination
                .insert("timer".to_string(), JsonValue::Number(JsonNumber::from(fade_timer)));
            destination.insert("draw".to_string(), JsonValue::String(fade_draw));
        }
        // Lua table path is 1-based, while the converted JSON array is 0-based.
        repaired.insert(index + 1);
        repaired.insert(index + 2);
    }
    repaired
}

pub(super) fn keybeam_draw_replacements(
    hold: &JsonValue,
    fade: &JsonValue,
) -> Option<(String, String, i32)> {
    let hold = hold.as_object()?;
    let fade = fade.as_object()?;
    let hold_id = json_string_field(hold, "id")?;
    if !hold_id.starts_with("key-beam-") || hold_id != json_string_field(fade, "id")? {
        return None;
    }
    if json_i32_field(hold, "timer").is_some() || json_i32_field(hold, "loop") == Some(-1) {
        return None;
    }
    if json_i32_field(fade, "loop") != Some(-1) {
        return None;
    }
    let inferred_fade_draw = json_string_field(fade, "draw")?;
    let fade_timer = json_i32_field(fade, "timer")
        .filter(|timer| is_keybeam_keyoff_timer(*timer))
        .or_else(|| keybeam_keyoff_timer_from_draw(inferred_fade_draw))?;
    let fallback_draw;
    let fade_draw = if inferred_fade_draw.contains("event_index(") {
        inferred_fade_draw
    } else {
        fallback_draw = keybeam_judge_draw_from_id(hold_id, fade_timer)?;
        &fallback_draw
    };
    let keyon_timer = keybeam_keyon_timer_for_keyoff_timer(fade_timer)?;
    let hold_timer = keybeam_hold_timer_for_keyon_timer(keyon_timer)?;
    let hold_draw = keybeam_hold_draw_from_fade_draw(fade_draw, keyon_timer, hold_timer)?;
    let fade_draw = fade_draw
        .split(" or ")
        .map(str::trim)
        .map(|branch| format!("keybeam_fade({fade_timer}) != 0 and {branch}"))
        .collect::<Vec<_>>()
        .join(" or ");
    Some((hold_draw, fade_draw, fade_timer))
}

pub(super) fn keybeam_judge_draw_from_id(id: &str, keyoff_timer: i32) -> Option<String> {
    let event_id = keyoff_timer.checked_add(380)?;
    let values: &[i32] = if id.ends_with("-pgreat") {
        &[1]
    } else if id.ends_with("-great") {
        &[2, 3]
    } else if id.ends_with("-good") {
        &[4, 5]
    } else if id.ends_with("-other") {
        &[0, 6, 7, 8, 9]
    } else {
        return None;
    };
    Some(
        values
            .iter()
            .map(|value| format!("event_index({event_id}) == {value}"))
            .collect::<Vec<_>>()
            .join(" or "),
    )
}

pub(super) fn json_string_field<'a>(
    object: &'a JsonMap<String, JsonValue>,
    key: &str,
) -> Option<&'a str> {
    object.get(key)?.as_str()
}

pub(super) fn json_i32_field(object: &JsonMap<String, JsonValue>, key: &str) -> Option<i32> {
    i32::try_from(object.get(key)?.as_i64()?).ok()
}

pub(super) fn keybeam_keyoff_timer_from_draw(draw: &str) -> Option<i32> {
    let event_ids = draw
        .split("event_index(")
        .skip(1)
        .filter_map(|tail| tail.split_once(')')?.0.trim().parse::<i32>().ok())
        .collect::<BTreeSet<_>>();
    let event_id = (event_ids.len() == 1).then(|| *event_ids.first().unwrap())?;
    (500..=517).contains(&event_id).then_some(event_id - 380)
}

pub(super) fn keybeam_keyon_timer_for_keyoff_timer(timer_id: i32) -> Option<i32> {
    match timer_id {
        120..=137 => Some(timer_id - 20),
        _ => None,
    }
}

pub(super) fn keybeam_hold_draw_from_fade_draw(
    fade_draw: &str,
    keyon_timer: i32,
    _hold_timer: i32,
) -> Option<String> {
    let prefix = format!("keybeam_hold({keyon_timer}) != 0 and ");
    let branches = fade_draw
        .split(" or ")
        .map(str::trim)
        .filter(|branch| branch.contains("event_index("))
        .map(|branch| format!("{prefix}{branch}"))
        .collect::<Vec<_>>();
    (!branches.is_empty()).then(|| branches.join(" or "))
}

pub(super) fn infer_constant_number_at_load(
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

pub(super) fn infer_constant_integer_at_load(
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

pub(super) fn infer_result_panel_act_at_load(
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

pub(super) fn result_panel_event(panel: i32) -> Option<i64> {
    match panel {
        1 => Some(i64::from(SKIN_EVENT_RESULT_PANEL_IR)),
        2 => Some(i64::from(SKIN_EVENT_RESULT_PANEL_GRAPH)),
        _ => None,
    }
}

pub(super) fn collect_number_refs(
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

pub(super) fn collect_number_refs_with_option(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    option_id: i32,
) -> Option<Vec<i32>> {
    collect_number_refs_with_option_value(function, main_state_probe, option_id, true)
}

pub(super) fn collect_number_refs_with_option_value(
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

pub(super) fn call_draw_with_numbers(
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

pub(super) fn call_draw_with_numbers_and_timers(
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

pub(super) fn call_draw_with_number_option(
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

pub(super) fn call_number_float_with_values(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    values: BTreeMap<i32, i32>,
) -> Option<f64> {
    call_number_float_raw_with_values(function, main_state_probe, values)
        .filter(|value| value.is_finite())
}

pub(super) fn call_number_float_raw_with_values(
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

pub(super) fn call_number_float_with_values_and_options(
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

pub(super) fn call_draw_with_numbers_and_options(
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

pub(super) fn verify_draw_condition(
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

pub(super) fn infer_or_of_number_gt_zero(
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

pub(super) fn infer_or_of_number_lt_zero(
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

pub(super) fn infer_result_average_timing_sign_draw_condition(
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

pub(super) fn infer_or_of_number_eq_zero(
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

pub(super) fn infer_two_number_compare_and(
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

pub(super) fn infer_number_eq_zero_with_constant_tail(
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

pub(super) fn infer_gauge_type_imageset_ref(
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

pub(super) fn infer_course_table_text_expr(
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

pub(super) fn infer_main_state_text_ref(
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

pub(super) fn infer_text_concat_expr(
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

pub(super) fn infer_nearest_rank_diff_value_expr(
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

pub(super) fn luxe_flat_nearest_rank_diff(ex_score: i32, total_notes: i32) -> Option<i32> {
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

pub(super) fn infer_result_score_draw(
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

pub(super) fn luxe_flat_nearest_rank_destination(id: &str) -> Option<(&'static str, &'static str)> {
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

pub(super) fn infer_result_panel_draw_condition(
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

pub(super) fn restore_result_panel_probe_state(
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

pub(super) fn result_score_draw_object(object_id: Option<&str>) -> bool {
    object_id.is_some_and(|id| {
        id == "scoreGraph"
            || id.starts_with("ir_scoreGraph")
            || id == "irYouFrame"
            || id.starts_with("nextRank")
            || matches!(id, "diff_plus" | "diff_minus" | "diff_rank")
    })
}

pub(super) fn ir_ranking_slot_from_id(id: &str, prefix: &str) -> Option<i32> {
    let slot = id.strip_prefix(prefix)?.parse::<i32>().ok()?;
    (1..=10).contains(&slot).then_some(slot)
}

pub(super) fn modern_chic_ir_ranking_graph(id: &str) -> Option<(i32, &'static str)> {
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

pub(super) fn collect_text_refs(
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

pub(super) fn infer_ir_ranking_name_ref(
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

pub(super) fn infer_ir_ranking_user_draw(
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

pub(super) fn call_draw_with_text_values(
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

pub(super) fn infer_ir_ranking_score_value_expr(
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

pub(super) fn infer_ir_ranking_score_diff_value_expr(
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

pub(super) fn infer_ir_ranking_score_rate_value_expr(
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

pub(super) fn infer_ir_score_rate_band(
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

pub(super) fn modern_chic_ir_rate_bounds(rank: &str) -> Option<(i64, i64)> {
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

pub(super) fn infer_modern_chic_ir_score_rate_band(
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

pub(super) fn infer_score_rate_band(
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

pub(super) fn verify_nearest_rank_draw(
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

pub(super) fn wmii_nearest_rank(
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

pub(super) fn call_draw_with_float_and_number(
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

pub(super) fn infer_float_number_and_number_and_draw(
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

pub(super) fn collect_float_number_refs(
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

pub(super) fn format_number_sum_expr(refs: &[i32]) -> String {
    refs.iter().map(|ref_id| format!("number({ref_id})")).collect::<Vec<_>>().join("+")
}

pub(super) fn infer_slider_value_expr(
    function: &Function,
    object_id: Option<&str>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    match object_id {
        Some("adjustedcover") | Some("adjusted-cover") | Some("adjusted_cover") => {
            Some(SKIN_EXPR_ADJUSTED_COVER.to_string())
        }
        _ => infer_hsfix_dependent_float(function, main_state_probe)
            .map(|_| SKIN_EXPR_ADJUSTED_COVER.to_string()),
    }
}

pub(super) fn infer_bmz_builtin_value_expr(
    function: &Function,
    object_id: Option<&str>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    match object_id {
        Some("adjusted-rate-num") | Some("adjusted_rate_num") => {
            Some(SKIN_EXPR_ADJUSTED_RATE.to_string())
        }
        Some("adjusted-rate-adot-num") | Some("adjusted_rate_adot_num") => {
            Some(SKIN_EXPR_ADJUSTED_RATE_ADOT.to_string())
        }
        Some("threshold-num") | Some("threshold_num") | Some("fs-threshold") => {
            Some(SKIN_EXPR_FS_THRESHOLD.to_string())
        }
        Some("courseClearRate") | Some("course-clear-rate") | Some("course_clear_rate") => {
            Some(SKIN_EXPR_COURSE_CLEAR_RATE.to_string())
        }
        Some("val-gauge-percent-integer") => Some(SKIN_EXPR_GAUGE_PERCENT_INTEGER.to_string()),
        Some("val-gauge-percent-fraction") => Some(SKIN_EXPR_GAUGE_PERCENT_FRACTION.to_string()),
        Some("val-gauge-amount-integer") => Some(SKIN_EXPR_GAUGE_AMOUNT_INTEGER.to_string()),
        Some("val-gauge-amount-fraction") => Some(SKIN_EXPR_GAUGE_AMOUNT_FRACTION.to_string()),
        _ => {
            let refs = collect_number_refs(function, main_state_probe)?;
            if refs.iter().any(|ref_id| matches!(ref_id, 160 | 90 | 91 | 314 | 14)) {
                infer_hsfix_dependent_float(function, main_state_probe).map(|_| {
                    if object_id.is_some_and(|id| id.contains("adot") || id.contains("dot")) {
                        SKIN_EXPR_ADJUSTED_RATE_ADOT.to_string()
                    } else {
                        SKIN_EXPR_ADJUSTED_RATE.to_string()
                    }
                })
            } else if collect_option_calls(function, main_state_probe)
                .is_some_and(|options| options.iter().any(|option| (180..=183).contains(option)))
            {
                Some(SKIN_EXPR_FS_THRESHOLD.to_string())
            } else {
                None
            }
        }
    }
}

pub(super) fn infer_hsfix_dependent_float(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<f64> {
    let number_refs = collect_number_refs(function, main_state_probe)?;
    let float_refs = collect_float_number_refs(function, main_state_probe)?;
    if number_refs.iter().any(|ref_id| matches!(ref_id, 160 | 90 | 91))
        || float_refs.iter().any(|ref_id| matches!(ref_id, 14 | 314))
    {
        Some(0.0)
    } else {
        None
    }
}

pub(super) fn collect_option_calls(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<Vec<i32>> {
    {
        main_state_probe.lock().ok()?.begin_number_call_recording(0);
    }
    let _ = function.call::<Value>(()).ok();
    let calls = {
        let mut probe = main_state_probe.lock().ok()?;
        let calls = probe.option_calls.clone();
        probe.end_recording();
        calls
    };
    (!calls.is_empty()).then_some(calls)
}

pub(super) fn infer_value_float_expr(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    infer_remain_rate_scaled(function, main_state_probe)
        .or_else(|| infer_number_scalar_multiply(function, main_state_probe))
        .or_else(|| infer_option_weighted_number_sum(function, main_state_probe))
        .or_else(|| infer_weighted_number_ratio_scaled(function, main_state_probe))
        .or_else(|| infer_division_of_number_sums(function, main_state_probe))
}

pub(super) const REMAIN_NOTE_REFS: [i32; 6] = [106, 110, 111, 112, 113, 114];

pub(super) fn remain_notes_numerator_expr() -> String {
    "number(106)-number(110)-number(111)-number(112)-number(113)-number(114)".to_string()
}

pub(super) fn remain_notes_value(values: &BTreeMap<i32, i32>) -> i32 {
    REMAIN_NOTE_REFS
        .iter()
        .map(|ref_id| {
            let value = values.get(ref_id).copied().unwrap_or(0);
            if *ref_id == 106 { value } else { -value }
        })
        .sum()
}

pub(super) fn infer_remain_rate_scaled(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let refs = collect_number_refs(function, main_state_probe)?;
    if refs.len() != 6 || !refs.iter().all(|ref_id| REMAIN_NOTE_REFS.contains(ref_id)) {
        return None;
    }
    let mut probe_values = BTreeMap::from([(106, 10)]);
    for ref_id in REMAIN_NOTE_REFS {
        probe_values.entry(ref_id).or_insert(0);
    }
    let scale_sample =
        call_number_float_with_values(function, main_state_probe, probe_values.clone())?;
    let scale = scale_sample.round();
    if (scale - 100.0).abs() > 0.5 && (scale - 10000.0).abs() > 0.5 {
        return None;
    }
    let numerator = remain_notes_numerator_expr();
    let expr = format!("({numerator})/number(106)*{}", scale as i64);
    let expected = |values: &BTreeMap<i32, i32>| {
        let remain: f64 = REMAIN_NOTE_REFS
            .iter()
            .map(|ref_id| {
                let value = values.get(ref_id).copied().unwrap_or(0) as f64;
                if *ref_id == 106 { value } else { -value }
            })
            .sum();
        let total = values.get(&106).copied().unwrap_or(0) as f64;
        if total.abs() < f64::EPSILON { 0.0 } else { remain / total * scale }
    };
    for test_values in [
        probe_values.clone(),
        BTreeMap::from([(106, 20), (110, 5)]),
        BTreeMap::from([(106, 30), (110, 10), (111, 5)]),
    ] {
        let actual =
            call_number_float_with_values(function, main_state_probe, test_values.clone())?;
        if !approx_float_eq(actual, expected(&test_values)) {
            return None;
        }
    }
    Some(expr)
}

pub(super) fn infer_number_scalar_multiply(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let refs = collect_number_refs(function, main_state_probe)?;
    if refs.len() != 1 {
        return None;
    }
    let ref_id = refs[0];
    let baseline = call_number_float_with_values(function, main_state_probe, BTreeMap::new())?;
    let at_one =
        call_number_float_with_values(function, main_state_probe, BTreeMap::from([(ref_id, 1)]))?;
    let coefficient = at_one - baseline;
    if coefficient.abs() < f64::EPSILON {
        return None;
    }
    let at_three =
        call_number_float_with_values(function, main_state_probe, BTreeMap::from([(ref_id, 3)]))?;
    if !approx_float_eq(at_three - baseline, coefficient * 3.0) {
        return None;
    }
    Some(format!("{coefficient}*number({ref_id})"))
}

pub(super) fn infer_option_weighted_number_sum(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let options = collect_option_calls(function, main_state_probe)?;
    if options.is_empty() || options.len() > 12 {
        return None;
    }

    let mut refs = Vec::new();
    for option_id in &options {
        refs.extend(collect_number_refs_with_option(function, main_state_probe, *option_id)?);
    }
    refs.sort_unstable();
    refs.dedup();
    if refs.is_empty() || refs.len() > 16 {
        return None;
    }

    let mut terms = Vec::new();
    for option_id in &options {
        let option_values = BTreeMap::from([(*option_id, true)]);
        let zero_values = refs.iter().copied().map(|ref_id| (ref_id, 0)).collect();
        let baseline = call_number_float_with_values_and_options(
            function,
            main_state_probe,
            zero_values,
            option_values.clone(),
        )?;
        for ref_id in &refs {
            let mut values = refs.iter().copied().map(|id| (id, 0)).collect::<BTreeMap<_, _>>();
            values.insert(*ref_id, 1);
            let at_one = call_number_float_with_values_and_options(
                function,
                main_state_probe,
                values,
                option_values.clone(),
            )?;
            let coefficient = at_one - baseline;
            if coefficient.abs() > f64::EPSILON {
                terms.push(format!("{coefficient}*option({option_id})*number({ref_id})"));
            }
        }
    }
    if terms.is_empty() {
        return None;
    }

    for option_id in &options {
        let option_values = BTreeMap::from([(*option_id, true)]);
        for sample in [1, 3, 7] {
            let values = refs.iter().copied().map(|ref_id| (ref_id, sample)).collect();
            let actual = call_number_float_with_values_and_options(
                function,
                main_state_probe,
                values,
                option_values.clone(),
            )?;
            let expected = evaluate_option_weighted_number_terms(
                &terms,
                *option_id,
                &refs.iter().copied().map(|ref_id| (ref_id, sample)).collect(),
            )?;
            if !approx_float_eq(actual, expected) {
                return None;
            }
        }
    }

    Some(terms.join("+"))
}

pub(super) fn evaluate_option_weighted_number_terms(
    terms: &[String],
    active_option: i32,
    values: &BTreeMap<i32, i32>,
) -> Option<f64> {
    let mut total = 0.0;
    for term in terms {
        let mut factors = term.split('*');
        let coefficient = factors.next()?.parse::<f64>().ok()?;
        let option = factors.next()?.trim();
        let number = factors.next()?.trim();
        if factors.next().is_some() {
            return None;
        }
        let option_id = option.strip_prefix("option(")?.strip_suffix(')')?.parse::<i32>().ok()?;
        let ref_id = number.strip_prefix("number(")?.strip_suffix(')')?.parse::<i32>().ok()?;
        if option_id == active_option {
            total += coefficient * f64::from(values.get(&ref_id).copied().unwrap_or(0));
        }
    }
    Some(total)
}

pub(super) fn infer_weighted_number_ratio_scaled(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let refs = collect_number_refs(function, main_state_probe)?;
    if refs.len() < 2 || refs.len() > 16 {
        return None;
    }
    refs.iter().find_map(|denominator_ref| {
        infer_weighted_number_ratio_scaled_with_denominator(
            function,
            main_state_probe,
            &refs,
            *denominator_ref,
        )
    })
}

pub(super) fn infer_weighted_number_ratio_scaled_with_denominator(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    refs: &[i32],
    denominator_ref: i32,
) -> Option<String> {
    const PROBE_DENOMINATOR: i32 = 1000;
    let mut base_values =
        refs.iter().copied().map(|ref_id| (ref_id, 0)).collect::<BTreeMap<_, _>>();
    base_values.insert(denominator_ref, PROBE_DENOMINATOR);
    let baseline = call_number_float_with_values(function, main_state_probe, base_values.clone())?;
    if !approx_float_eq(baseline, 0.0) {
        return None;
    }

    let mut terms = Vec::new();
    for ref_id in refs.iter().copied().filter(|ref_id| *ref_id != denominator_ref) {
        let mut values = base_values.clone();
        values.insert(ref_id, 1);
        let at_one = call_number_float_with_values(function, main_state_probe, values)?;
        if at_one - baseline < 1.0 {
            continue;
        }
        let coefficient = ((at_one - baseline) * f64::from(PROBE_DENOMINATOR)).round() as i64;
        if coefficient <= 0 {
            continue;
        }
        terms.push((ref_id, coefficient));
    }
    if terms.is_empty() {
        return None;
    }

    let test_cases = [
        refs.iter().copied().map(|ref_id| (ref_id, 0)).collect::<BTreeMap<_, _>>(),
        terms
            .iter()
            .map(|(ref_id, _)| (*ref_id, 1))
            .chain(std::iter::once((denominator_ref, PROBE_DENOMINATOR)))
            .collect::<BTreeMap<_, _>>(),
        terms
            .iter()
            .map(|(ref_id, _)| (*ref_id, 3))
            .chain(std::iter::once((denominator_ref, PROBE_DENOMINATOR)))
            .collect::<BTreeMap<_, _>>(),
        terms
            .iter()
            .map(|(ref_id, _)| (*ref_id, 1))
            .chain(std::iter::once((denominator_ref, 74)))
            .collect::<BTreeMap<_, _>>(),
    ];
    for values in test_cases {
        let expected = weighted_ratio_floor(&terms, denominator_ref, &values) as f64;
        let actual = match call_number_float_with_values(function, main_state_probe, values) {
            Some(value) if value.is_finite() => value,
            _ if expected.abs() < f64::EPSILON => 0.0,
            _ => return None,
        };
        if !approx_float_eq(actual, expected) {
            return None;
        }
    }

    let numerator = terms
        .iter()
        .map(|(ref_id, coefficient)| {
            if *coefficient == 1 {
                format!("number({ref_id})")
            } else {
                format!("{coefficient}*number({ref_id})")
            }
        })
        .collect::<Vec<_>>()
        .join("+");
    Some(format!("floor(({numerator})/number({denominator_ref}))"))
}

pub(super) fn weighted_ratio_floor(
    terms: &[(i32, i64)],
    denominator_ref: i32,
    values: &BTreeMap<i32, i32>,
) -> i64 {
    let denominator = values.get(&denominator_ref).copied().unwrap_or(0);
    if denominator <= 0 {
        return 0;
    }
    let numerator = terms
        .iter()
        .map(|(ref_id, coefficient)| {
            coefficient.saturating_mul(i64::from(values.get(ref_id).copied().unwrap_or(0)))
        })
        .sum::<i64>();
    numerator / i64::from(denominator)
}

pub(super) fn fast_slow_ref_set() -> BTreeMap<i32, ()> {
    FAST_SLOW_FAST_REFS.into_iter().chain(FAST_SLOW_SLOW_REFS).map(|ref_id| (ref_id, ())).collect()
}

pub(super) fn infer_fast_slow_ratio_graph_type(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<i32> {
    let refs = collect_number_refs(function, main_state_probe)?;
    let expected = fast_slow_ref_set();
    if refs.len() != expected.len() || !refs.iter().all(|ref_id| expected.contains_key(ref_id)) {
        return None;
    }
    let fast_set: BTreeMap<i32, ()> =
        FAST_SLOW_FAST_REFS.into_iter().map(|ref_id| (ref_id, ())).collect();
    let slow_set: BTreeMap<i32, ()> =
        FAST_SLOW_SLOW_REFS.into_iter().map(|ref_id| (ref_id, ())).collect();
    if verify_fast_slow_ratio(function, main_state_probe, &refs, &fast_set) {
        return Some(148);
    }
    if verify_fast_slow_ratio(function, main_state_probe, &refs, &slow_set) {
        return Some(149);
    }
    None
}

pub(super) fn approx_float_eq(actual: f64, expected: f64) -> bool {
    if expected.abs() < f64::EPSILON && (!actual.is_finite() || actual.abs() < f64::EPSILON) {
        return true;
    }
    (actual - expected).abs() <= 0.02
}

pub(super) fn verify_fast_slow_ratio(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    refs: &[i32],
    numerator_refs: &BTreeMap<i32, ()>,
) -> bool {
    let ratio = |values: &BTreeMap<i32, i32>| {
        let num: f64 = numerator_refs
            .keys()
            .map(|ref_id| values.get(ref_id).copied().unwrap_or(0) as f64)
            .sum();
        let den: f64 =
            refs.iter().map(|ref_id| values.get(ref_id).copied().unwrap_or(0) as f64).sum();
        if den.abs() < f64::EPSILON { 0.0 } else { num / den }
    };
    let all_zero: BTreeMap<i32, i32> = refs.iter().copied().map(|ref_id| (ref_id, 0)).collect();
    let all_one: BTreeMap<i32, i32> = refs.iter().copied().map(|ref_id| (ref_id, 1)).collect();
    let mut numerator_only = all_zero.clone();
    for ref_id in numerator_refs.keys() {
        numerator_only.insert(*ref_id, 5);
    }
    let mut complement_only =
        refs.iter().copied().map(|ref_id| (ref_id, 5)).collect::<BTreeMap<_, _>>();
    for ref_id in numerator_refs.keys() {
        complement_only.insert(*ref_id, 0);
    }
    let ratio_all_one = ratio(&all_one);
    let ratio_numerator_only = ratio(&numerator_only);
    let ratio_complement_only = ratio(&complement_only);
    for (values, expected) in [
        (all_zero, 0.0),
        (all_one, ratio_all_one),
        (numerator_only, ratio_numerator_only),
        (complement_only, ratio_complement_only),
    ] {
        let actual = match call_number_float_with_values(function, main_state_probe, values) {
            Some(value) if value.is_finite() => value,
            _ if expected.abs() < f64::EPSILON => 0.0,
            _ => return false,
        };
        if !approx_float_eq(actual, expected) {
            return false;
        }
    }
    true
}

pub(super) fn infer_division_of_number_sums(
    function: &Function,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
) -> Option<String> {
    let refs = collect_number_refs(function, main_state_probe)?;
    if refs.len() < 2 || refs.len() > 24 {
        return None;
    }
    let zero_values = refs.iter().copied().map(|ref_id| (ref_id, 0)).collect::<BTreeMap<_, _>>();
    // Lua の 0/0 は NaN になる。beatoraja の graph 描画では非有限値が実質0幅に
    // なるため、比率推論でも全ゼロ入力だけは0として扱う。
    let baseline =
        call_number_float_raw_with_values(function, main_state_probe, zero_values.clone())?;
    let baseline = if baseline.is_finite() { baseline } else { 0.0 };
    let mut numerator_refs = Vec::new();
    for ref_id in &refs {
        let mut values = zero_values.clone();
        values.insert(*ref_id, 5);
        let value = call_number_float_with_values(function, main_state_probe, values)?;
        if value > baseline + f64::EPSILON {
            numerator_refs.push(*ref_id);
        }
    }
    if numerator_refs.is_empty() {
        return None;
    }
    let numerator = format_number_sum_expr(&numerator_refs);
    let denominator = format_number_sum_expr(&refs);
    let expr = format!("({numerator})/({denominator})");
    let expected_ratio = |values: &BTreeMap<i32, i32>| {
        let num: f64 = numerator_refs
            .iter()
            .map(|ref_id| values.get(ref_id).copied().unwrap_or(0) as f64)
            .sum();
        let den: f64 =
            refs.iter().map(|ref_id| values.get(ref_id).copied().unwrap_or(0) as f64).sum();
        if den.abs() < f64::EPSILON { 0.0 } else { num / den }
    };
    let mut numerator_only = zero_values.clone();
    for ref_id in &numerator_refs {
        numerator_only.insert(*ref_id, 5);
    }
    let mut denominator_only =
        refs.iter().copied().map(|ref_id| (ref_id, 5)).collect::<BTreeMap<_, _>>();
    for ref_id in &numerator_refs {
        denominator_only.insert(*ref_id, 0);
    }
    let test_cases = [
        zero_values,
        refs.iter().copied().map(|id| (id, 1)).collect(),
        refs.iter().copied().map(|id| (id, 3)).collect(),
        numerator_only,
        denominator_only,
    ];
    for values in test_cases {
        let expected = expected_ratio(&values);
        let actual = call_number_float_raw_with_values(function, main_state_probe, values)?;
        let actual = if actual.is_finite() {
            actual
        } else if expected.abs() < f64::EPSILON {
            0.0
        } else {
            return None;
        };
        if !approx_float_eq(actual, expected) {
            return None;
        }
    }
    Some(expr)
}
