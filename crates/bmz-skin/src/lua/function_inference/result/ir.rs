use super::*;

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
