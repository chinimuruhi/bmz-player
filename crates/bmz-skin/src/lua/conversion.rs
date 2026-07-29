use super::*;

pub(super) fn lua_value_to_log_string(value: &Value) -> String {
    match value {
        Value::Nil => "nil".to_string(),
        Value::Boolean(value) => value.to_string(),
        Value::Integer(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.to_string_lossy(),
        Value::Table(_) => "<table>".to_string(),
        Value::Function(_) => "<function>".to_string(),
        Value::Thread(_) => "<thread>".to_string(),
        Value::UserData(_) => "<userdata>".to_string(),
        Value::LightUserData(_) => "<lightuserdata>".to_string(),
        Value::Error(error) => format!("<error:{error}>"),
        Value::Other(_) => "<other>".to_string(),
    }
}

pub(super) fn infer_m_select_result_graph_height_expr(path: &str) -> Option<String> {
    const DESTINATION_FIRST: i64 = 40;
    const FAST_SLOW_REFS: [i32; 12] = [422, 419, 417, 415, 413, 411, 410, 412, 414, 416, 418, 421];
    let destination_index = lua_path_array_index(path, "$.destination[")?;
    let dst_index = lua_path_array_index(path, "].dst[")?;
    if dst_index != 3 {
        return None;
    }
    let ref_index = usize::try_from(destination_index - DESTINATION_FIRST).ok()?;
    let ref_id = *FAST_SLOW_REFS.get(ref_index)?;
    Some(format!("{SKIN_EXPR_FAST_SLOW_BREAKDOWN_HEIGHT}({ref_id})"))
}

pub(super) fn lua_path_array_index(path: &str, marker: &str) -> Option<i64> {
    let (_, rest) = path.split_once(marker)?;
    let (index, _) = rest.split_once(']')?;
    index.parse().ok()
}

pub(super) fn lua_value_to_json(
    lua: &Lua,
    value: Value,
    path: &str,
    depth: usize,
    warnings: &mut Vec<String>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    instruction_budget: &LuaInstructionBudget,
    table_budget: &mut TableBudget,
) -> Result<JsonValue> {
    if depth > LUA_MAX_TABLE_DEPTH {
        bail!("lua table nesting is too deep at {path}");
    }

    Ok(match value {
        Value::Nil => JsonValue::Null,
        Value::Boolean(value) => JsonValue::Bool(value),
        Value::Integer(value) => JsonValue::Number(JsonNumber::from(value)),
        Value::Number(value) => match JsonNumber::from_f64(value) {
            Some(number) => JsonValue::Number(number),
            None => {
                warnings.push(format!("non-finite lua number converted to 0 at {path}"));
                JsonValue::Number(JsonNumber::from(0))
            }
        },
        Value::String(value) => JsonValue::String(value.to_string_lossy()),
        Value::Table(table) => lua_table_to_json(
            lua,
            table,
            path,
            depth + 1,
            warnings,
            main_state_probe,
            instruction_budget,
            table_budget,
        )?,
        Value::Function(_) => {
            warnings.push(format!("skipping function at {path}"));
            JsonValue::Null
        }
        Value::Thread(_) => {
            warnings.push(format!("skipping thread at {path}"));
            JsonValue::Null
        }
        Value::UserData(_) | Value::LightUserData(_) => {
            warnings.push(format!("skipping userdata at {path}"));
            JsonValue::Null
        }
        Value::Error(error) => {
            warnings.push(format!("skipping lua error value at {path}: {error}"));
            JsonValue::Null
        }
        Value::Other(_) => {
            warnings.push(format!("skipping unsupported lua value at {path}"));
            JsonValue::Null
        }
    })
}

pub(super) fn peaceful_gauge_lead_glow_id(id: &str) -> Option<(&str, bool)> {
    let (group, side) = id.strip_prefix("gauge-lead-glow-")?.rsplit_once('-')?;
    if !matches!(group, "assist_easy" | "easy" | "groove" | "hard" | "exhard" | "hazard") {
        return None;
    }
    Some((
        group,
        match side {
            "above" => false,
            "below" => true,
            _ => return None,
        },
    ))
}

pub(super) fn is_peaceful_gauge_lead_glow_destination(entries: &[(Value, Value)]) -> bool {
    let Some(Value::Table(dst)) = entries.iter().find_map(|(key, value)| {
        matches!(key, Value::String(key) if key.as_bytes() == b"dst").then_some(value)
    }) else {
        return false;
    };
    let frames =
        [1, 2, 3].into_iter().map(|index| dst.get::<Table>(index).ok()).collect::<Option<Vec<_>>>();
    let Some(frames) = frames else { return false };
    let expected = [(0, 0), (750, 255), (1500, 0)];
    let rect = frames[0]
        .get::<f64>("x")
        .ok()
        .zip(frames[0].get::<f64>("y").ok())
        .zip(frames[0].get::<f64>("w").ok())
        .zip(frames[0].get::<f64>("h").ok());
    frames.iter().zip(expected).all(|(frame, (time, alpha))| {
        frame.get::<i32>("time").ok() == Some(time)
            && frame.get::<i32>("a").ok() == Some(alpha)
            && frame
                .get::<f64>("x")
                .ok()
                .zip(frame.get::<f64>("y").ok())
                .zip(frame.get::<f64>("w").ok())
                .zip(frame.get::<f64>("h").ok())
                == rect
    })
}

pub(super) fn lua_table_to_json(
    lua: &Lua,
    table: Table,
    path: &str,
    depth: usize,
    warnings: &mut Vec<String>,
    main_state_probe: &Arc<Mutex<MainStateProbe>>,
    instruction_budget: &LuaInstructionBudget,
    table_budget: &mut TableBudget,
) -> Result<JsonValue> {
    let mut entries = Vec::new();
    for pair in table.pairs::<Value, Value>() {
        entries.push(pair?);
    }
    table_budget.consume(entries.len(), path)?;

    if entries.is_empty() {
        return Ok(JsonValue::Array(Vec::new()));
    }

    let mut integer_keys = Vec::new();
    let mut has_non_integer_key = false;
    for (key, value) in &entries {
        if matches!(value, Value::Nil) {
            continue;
        }
        match key {
            Value::Integer(index) if *index > 0 => integer_keys.push(*index),
            _ => has_non_integer_key = true,
        }
    }
    integer_keys.sort_unstable();
    let is_array = !has_non_integer_key
        && integer_keys.iter().enumerate().all(|(offset, index)| *index == offset as i64 + 1);

    if is_array {
        let mut values = Vec::new();
        entries.sort_by_key(|(key, _)| match key {
            Value::Integer(index) => *index,
            _ => i64::MAX,
        });
        for (index, (_, value)) in entries.into_iter().enumerate() {
            values.push(lua_value_to_json(
                lua,
                value,
                &format!("{path}[{}]", index + 1),
                depth,
                warnings,
                main_state_probe,
                instruction_budget,
                table_budget,
            )?);
        }
        return Ok(JsonValue::Array(values));
    }

    if !integer_keys.is_empty() {
        warnings.push(format!("mixed lua table converted to object at {path}"));
    }
    let object_id = lua_object_id(&entries);
    let gauge_lead_glow_destination = object_id
        .as_deref()
        .filter(|_| path.contains(".destination["))
        .and_then(|id| peaceful_gauge_lead_glow_id(id))
        .filter(|_| is_peaceful_gauge_lead_glow_destination(&entries))
        .and_then(|(group, below_border)| {
            let mut probe = main_state_probe.lock().ok()?;
            let occurrence = probe
                .gauge_lead_glow_occurrences
                .entry(object_id.as_deref()?.to_string())
                .or_default();
            let part = *occurrence + 1;
            *occurrence += 1;
            Some((group.to_string(), below_border, part))
        });
    let keylogger_destination = object_id.as_deref().and_then(parse_keylogger_destination_id);
    let keylogger_slot = if path.contains(".destination[") && keylogger_destination.is_some() {
        object_id.as_deref().and_then(|id| {
            let mut probe = main_state_probe.lock().ok()?;
            let occurrence =
                probe.keylogger_destination_occurrences.entry(id.to_string()).or_default();
            let slot = *occurrence % 16 + 1;
            *occurrence += 1;
            Some(slot)
        })
    } else {
        None
    };
    let mut object = JsonMap::new();
    for (key, value) in entries {
        let key = lua_key_to_json_key(key, path, warnings)?;
        if matches!(value, Value::Nil) {
            continue;
        }
        if key == "draw"
            && let Value::Boolean(value) = &value
        {
            object.insert(
                key.clone(),
                JsonValue::String(if *value {
                    "number(0) >= 0".to_string()
                } else {
                    "number(0) < 0".to_string()
                }),
            );
            tracing::debug!(field_path = %format!("{path}.{key}"), classification = "STATIC", "classified Lua draw condition");
            continue;
        }
        if let Value::Function(function) = &value {
            // Inference deliberately invokes one callback several times with
            // different probe values. Give each callback a fresh bounded
            // slice while retaining the separate total inference cap.
            instruction_budget.begin_callback();
            if key == "value" {
                let is_graph = path.contains(".graph[");
                if matches!(object_id.as_deref(), Some("val-hits-per-sec")) {
                    object.insert(
                        "value_expr".to_string(),
                        JsonValue::String("bmz:keylogger_nps".to_string()),
                    );
                    continue;
                }
                if is_graph
                    && let Some(value_expr) =
                        object_id.as_deref().and_then(keylogger_graph_value_expr_from_id)
                {
                    object.insert("value_expr".to_string(), JsonValue::String(value_expr));
                    continue;
                }
                if is_graph
                    && let Some(value_expr) = object_id
                        .as_deref()
                        .and_then(milliondollar_fast_slow_graph_value_expr_from_id)
                {
                    // MILLIONDOLLAR keeps these two values in CUSTOMS, which
                    // is evaluated while the skin is loaded.  Preserve their
                    // runtime main_state/option dependencies as an expression
                    // instead of freezing the load-time value.
                    object.insert("value_expr".to_string(), JsonValue::String(value_expr));
                    continue;
                }
                if !is_graph
                    && path.contains(".imageset[")
                    && let Some(ref_id) = infer_gauge_type_imageset_ref(function, main_state_probe)
                {
                    object.insert("ref".to_string(), JsonValue::Number(JsonNumber::from(ref_id)));
                    continue;
                }
                if !is_graph
                    && path.contains(".text[")
                    && let Some(ref_id) =
                        infer_ir_ranking_name_ref(function, object_id.as_deref(), main_state_probe)
                {
                    object.insert("ref".to_string(), JsonValue::Number(JsonNumber::from(ref_id)));
                    continue;
                }
                if !is_graph
                    && path.contains(".text[")
                    && let Some(value_expr) = infer_course_table_text_expr(
                        function,
                        object_id.as_deref(),
                        main_state_probe,
                    )
                {
                    object.insert("value_expr".to_string(), JsonValue::String(value_expr));
                    continue;
                }
                if !is_graph
                    && path.contains(".text[")
                    && let Some(value_expr) = infer_text_concat_expr(function, main_state_probe)
                {
                    object.insert("value_expr".to_string(), JsonValue::String(value_expr));
                    continue;
                }
                if !is_graph
                    && path.contains(".text[")
                    && let Some(ref_id) = infer_main_state_text_ref(function, main_state_probe)
                {
                    object.insert("ref".to_string(), JsonValue::Number(JsonNumber::from(ref_id)));
                    continue;
                }
                if !is_graph
                    && path.contains(".slider[")
                    && let Some(value_expr) =
                        infer_slider_value_expr(function, object_id.as_deref(), main_state_probe)
                {
                    object.insert("value_expr".to_string(), JsonValue::String(value_expr));
                    continue;
                }
                if !is_graph
                    && let Some(value_expr) = infer_nearest_rank_diff_value_expr(
                        function,
                        object_id.as_deref(),
                        main_state_probe,
                    )
                {
                    object.insert("value_expr".to_string(), JsonValue::String(value_expr));
                    continue;
                }
                if !is_graph
                    && let Some(value_expr) = infer_ir_ranking_score_diff_value_expr(
                        function,
                        object_id.as_deref(),
                        main_state_probe,
                    )
                {
                    object.insert("value_expr".to_string(), JsonValue::String(value_expr));
                    continue;
                }
                if !is_graph
                    && let Some(value_expr) = infer_ir_ranking_score_rate_value_expr(
                        function,
                        object_id.as_deref(),
                        main_state_probe,
                    )
                {
                    object.insert("value_expr".to_string(), JsonValue::String(value_expr));
                    continue;
                }
                if !is_graph
                    && let Some(value_expr) = infer_bmz_builtin_value_expr(
                        function,
                        object_id.as_deref(),
                        main_state_probe,
                    )
                {
                    object.insert("value_expr".to_string(), JsonValue::String(value_expr));
                    continue;
                }
                if !is_graph
                    && let Some(ref_id) = infer_gated_number_ref(function, main_state_probe)
                {
                    object.insert("ref".to_string(), JsonValue::Number(JsonNumber::from(ref_id)));
                    continue;
                }
                if !is_graph
                    && let Some(ref_id) = infer_main_state_number_ref(function, main_state_probe)
                {
                    object.insert("ref".to_string(), JsonValue::Number(JsonNumber::from(ref_id)));
                    continue;
                }
                if is_graph
                    && let Some(value_expr) = infer_ir_ranking_score_value_expr(
                        function,
                        object_id.as_deref(),
                        main_state_probe,
                    )
                {
                    object.insert("value_expr".to_string(), JsonValue::String(value_expr));
                } else if is_graph
                    && let Some(graph_type) =
                        infer_fast_slow_ratio_graph_type(function, main_state_probe)
                {
                    object.insert(
                        "type".to_string(),
                        JsonValue::Number(JsonNumber::from(graph_type)),
                    );
                } else if !is_graph
                    && let Some(expr) = infer_main_state_number_expr(function, main_state_probe)
                {
                    object.insert("expr".to_string(), JsonValue::String(expr));
                } else if is_graph && matches!(object_id.as_deref(), Some("default_chart_gauge")) {
                    object.insert(
                        "value_expr".to_string(),
                        JsonValue::String("bmz:default_chart_gauge".to_string()),
                    );
                } else if !is_graph
                    && matches!(object_id.as_deref(), Some("default_chart_total_count"))
                {
                    object.insert(
                        "value_expr".to_string(),
                        JsonValue::String("bmz:default_chart_total_count".to_string()),
                    );
                } else if let Some(value_expr) = infer_value_float_expr(function, main_state_probe)
                {
                    object.insert("value_expr".to_string(), JsonValue::String(value_expr));
                } else if path.contains(".text[")
                    && let Some(ref_id) =
                        infer_constant_text_ref_at_load(function, main_state_probe)
                {
                    object.insert("ref".to_string(), JsonValue::Number(JsonNumber::from(ref_id)));
                } else if path.contains(".text[")
                    && let Some(text) = infer_constant_text_at_load(function, main_state_probe)
                {
                    object.insert("constantText".to_string(), JsonValue::String(text));
                } else if let Some(value_expr) =
                    infer_constant_number_at_load(function, main_state_probe)
                {
                    object.insert("value_expr".to_string(), JsonValue::String(value_expr));
                } else if matches!(object_id.as_deref(), Some("Number_Info_Level_Insane")) {
                    // MILLIONDOLLAR parses digits from table level text. Non-table
                    // results legitimately receive an empty string and return nil;
                    // the matching destination is hidden, so keep an inert value.
                    object.insert("value_expr".to_string(), JsonValue::String("0".to_string()));
                } else {
                    warnings.push(format!("skipping unsupported value function at {path}.{key}"));
                }
                continue;
            }
            if key == "act" {
                let event_id = infer_constant_integer_at_load(function, main_state_probe)
                    .or_else(|| infer_runtime_toggle_act(lua, function, main_state_probe))
                    .or_else(|| infer_result_panel_act_at_load(lua, function, main_state_probe));
                if let Some(event_id) = event_id {
                    object.insert(key.clone(), JsonValue::Number(JsonNumber::from(event_id)));
                    continue;
                }
            }
            if key == "action"
                && path.contains(".customEvents[")
                && let Some(actions) =
                    infer_custom_audio_event_action(lua, function, main_state_probe)
            {
                object.insert(
                    "audioActions".to_string(),
                    JsonValue::Array(actions.into_iter().map(lua_audio_action_to_json).collect()),
                );
                object.insert("once".to_string(), JsonValue::Bool(true));
                continue;
            }
            if key == "condition"
                && path.contains(".customEvents[")
                && let Some(timer_id) = infer_timer_on_condition(function, main_state_probe)
            {
                object.insert("timer".to_string(), JsonValue::Number(JsonNumber::from(timer_id)));
                continue;
            }
            if key == "draw" {
                if let Some(draw) = infer_result_panel_draw_condition(
                    lua,
                    function,
                    object_id.as_deref(),
                    main_state_probe,
                ) {
                    object.insert(key.clone(), JsonValue::String(draw));
                    tracing::debug!(field_path = %format!("{path}.{key}"), classification = "COMPILED", "classified Lua draw condition");
                    continue;
                }
                if let Some(draw) =
                    infer_result_score_draw(function, object_id.as_deref(), main_state_probe)
                {
                    object.insert(key.clone(), JsonValue::String(draw));
                    tracing::debug!(field_path = %format!("{path}.{key}"), classification = "COMPILED", "classified Lua draw condition");
                    continue;
                }
                if let Some((group, below_border, part)) = &gauge_lead_glow_destination {
                    object.insert(
                        key.clone(),
                        JsonValue::String(format!(
                            "gauge_lead_glow({group},{part},{})",
                            if *below_border { "below" } else { "above" }
                        )),
                    );
                    tracing::debug!(field_path = %format!("{path}.{key}"), classification = "COMPILED", "classified Lua draw condition");
                    continue;
                }
                if let (Some((graph_kind, lane, Some(kind))), Some(slot)) =
                    (keylogger_destination, keylogger_slot)
                {
                    object.insert(
                        key.clone(),
                        JsonValue::String(format!("keylogger_{graph_kind}({lane},{slot},{kind})")),
                    );
                    tracing::debug!(field_path = %format!("{path}.{key}"), classification = "COMPILED", "classified Lua draw condition");
                    continue;
                }
                if let Some(draw) = infer_gauge_value_digit_draw_condition(
                    function,
                    object_id.as_deref(),
                    main_state_probe,
                ) {
                    object.insert(key.clone(), JsonValue::String(draw));
                    tracing::debug!(field_path = %format!("{path}.{key}"), classification = "COMPILED", "classified Lua draw condition");
                    continue;
                }
                if let Some(draw) =
                    infer_select_score_available_draw_condition(lua, function, main_state_probe)
                {
                    object.insert(key.clone(), JsonValue::String(draw));
                    tracing::debug!(field_path = %format!("{path}.{key}"), classification = "COMPILED", "classified Lua draw condition");
                    continue;
                }
                if let Some(draw) =
                    infer_boolean_predicate(function, main_state_probe, object_id.as_deref())
                {
                    object.insert(key.clone(), JsonValue::String(draw));
                    tracing::debug!(field_path = %format!("{path}.{key}"), classification = "COMPILED", "classified Lua draw condition");
                } else {
                    let field_path = format!("{path}.{key}");
                    let callback_id = register_runtime_draw_path(main_state_probe, &field_path)?;
                    object.insert(
                        key.clone(),
                        JsonValue::String(format!("{LUA_DRAW_CALLBACK_PREFIX}{callback_id}")),
                    );
                    tracing::debug!(%field_path, callback_id, classification = "RUNTIME", "classified Lua draw condition");
                }
                continue;
            }
            if key == "timer" {
                if let (Some((_, lane, _)), Some(slot)) = (keylogger_destination, keylogger_slot) {
                    object.insert(
                        "timer_expr".to_string(),
                        JsonValue::String(format!("bmz:keylogger_event:{lane}:{slot}")),
                    );
                    continue;
                }
                if path.contains(".customTimers[")
                    && let Some(id) = object_id.as_deref().and_then(|id| id.parse::<i32>().ok())
                    && let Some((source_timer, delay_ms)) =
                        infer_fixed_delay_timer(function, main_state_probe).or_else(|| {
                            infer_custom_timer_alias(function, main_state_probe)
                                .map(|source_timer| (source_timer, 0))
                        })
                {
                    if let Ok(mut probe) = main_state_probe.lock()
                        && !probe.fixed_delay_timers.iter().any(|(existing, _, _)| *existing == id)
                    {
                        probe.fixed_delay_timers.push((id, source_timer, delay_ms));
                    }
                    continue;
                }
                let map: Table = lua.globals().get("bmz_timer_fn_map")?;
                if let Ok(timer_id) = map.get::<i32>(function.clone()) {
                    object.insert(key.clone(), JsonValue::Number(JsonNumber::from(timer_id)));
                    continue;
                }
                if let Some(timer_id) = infer_timer_function_ref(function, main_state_probe) {
                    object.insert(key.clone(), JsonValue::Number(JsonNumber::from(timer_id)));
                    continue;
                }
                if path.contains(".customTimers[") {
                    let id = object_id.as_deref().unwrap_or("unknown");
                    warnings.push(format!(
                        "skipping unsupported custom timer function id {id} at {path}.{key}"
                    ));
                    continue;
                }
            }
        }
        if is_unsupported_json_field_value(&value) {
            if should_silently_skip_loader_field(&key, &value) {
                continue;
            }
            warnings.push(format!("skipping unsupported field `{key}` at {path}"));
            continue;
        }
        if key == "h"
            && let Value::Number(number) = &value
            && !number.is_finite()
            && let Some(expr) = infer_m_select_result_graph_height_expr(path)
        {
            object.insert(key.clone(), JsonValue::Number(JsonNumber::from(0)));
            object.insert("h_expr".to_string(), JsonValue::String(expr));
            continue;
        }
        object.insert(
            key.clone(),
            lua_value_to_json(
                lua,
                value,
                &format!("{path}.{key}"),
                depth,
                warnings,
                main_state_probe,
                instruction_budget,
                table_budget,
            )?,
        );
    }
    repair_result_table_title_text(path, &mut object);
    repair_result_course_title_text(path, &mut object);
    Ok(JsonValue::Object(object))
}
