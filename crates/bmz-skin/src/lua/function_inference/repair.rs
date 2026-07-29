use super::*;

pub(in crate::lua) fn register_runtime_draw_path(
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    field_path: &str,
) -> Result<usize> {
    let mut probe =
        main_state_probe.lock().map_err(|_| anyhow!("main_state probe lock poisoned"))?;
    let callback_id = probe.runtime_draw_paths.len();
    probe.runtime_draw_paths.push(field_path.to_string());
    Ok(callback_id)
}

pub(in crate::lua) fn infer_gauge_value_digit_draw_condition(
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

pub(in crate::lua) fn infer_select_score_available_draw_condition(
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

pub(in crate::lua) fn repair_result_table_title_text(
    path: &str,
    object: &mut JsonMap<String, JsonValue>,
) {
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
pub(in crate::lua) fn repair_result_course_title_text(
    path: &str,
    object: &mut JsonMap<String, JsonValue>,
) {
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
pub(in crate::lua) fn repair_select_score_rate_punctuation(root: &mut JsonMap<String, JsonValue>) {
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

pub(in crate::lua) fn keylogger_graph_value_expr_from_id(id: &str) -> Option<String> {
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

pub(in crate::lua) fn milliondollar_fast_slow_graph_value_expr_from_id(id: &str) -> Option<String> {
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

pub(in crate::lua) fn parse_keylogger_destination_id(
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

pub(in crate::lua) fn lua_object_id(entries: &[(Value, Value)]) -> Option<String> {
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
