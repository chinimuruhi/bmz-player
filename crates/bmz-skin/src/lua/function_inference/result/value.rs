use super::*;

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
