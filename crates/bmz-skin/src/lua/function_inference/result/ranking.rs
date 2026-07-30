use super::*;

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
