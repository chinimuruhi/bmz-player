use super::*;

pub(in crate::lua) fn infer_main_state_number_ref(
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
pub(in crate::lua) fn infer_gated_number_ref(
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

pub(in crate::lua) fn infer_main_state_number_expr(
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

pub(in crate::lua) fn call_number_expr_with_values(
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

pub(in crate::lua) fn format_number_expr(constant: i64, terms: &[(i32, i64)]) -> String {
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

pub(in crate::lua) fn infer_main_state_draw_condition(
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

pub(in crate::lua) fn single_number_call(calls: &[i32]) -> Option<i32> {
    let first = *calls.first()?;
    calls.iter().all(|call| *call == first).then_some(first)
}

pub(in crate::lua) const ARRANGE_EVENT_INDEX_SAMPLES: [i32; 12] =
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

pub(in crate::lua) fn call_draw_with_number(
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

pub(in crate::lua) fn infer_main_state_event_index_draw_condition(
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

pub(in crate::lua) fn call_draw_with_event_index(
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

pub(in crate::lua) fn infer_main_state_event_index_options_draw_condition(
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

pub(in crate::lua) fn call_draw_with_event_index_options(
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

pub(in crate::lua) fn option_truth_table_cubes(
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

pub(in crate::lua) fn infer_main_state_option_draw_condition(
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

pub(in crate::lua) fn infer_main_state_option_number_draw_condition(
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

pub(in crate::lua) fn call_draw_with_option(
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

pub(in crate::lua) fn infer_main_state_timer_option_draw_condition(
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

pub(in crate::lua) fn infer_main_state_two_options_timer_draw_condition(
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

pub(in crate::lua) fn infer_end_of_note_shadow_draw_condition(
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

pub(in crate::lua) fn infer_os_clock_after_draw_condition(
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

pub(in crate::lua) fn infer_os_clock_after_option_draw_condition(
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

pub(in crate::lua) fn call_draw_with_os_clock_options(
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

pub(in crate::lua) fn collect_timer_refs(
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

pub(in crate::lua) fn infer_all_timers_off_draw_condition(
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

pub(in crate::lua) fn call_timer_function_with_values(
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

pub(in crate::lua) fn call_timer_function_with_values_at_time(
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

pub(in crate::lua) fn event_index_calls_with_timer_values(
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

pub(in crate::lua) fn call_draw_with_timer_event(
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

pub(in crate::lua) fn keybeam_hold_timer_for_keyon_timer(timer_id: i32) -> Option<i32> {
    match timer_id {
        100..=109 => Some(timer_id - 30),
        110..=117 => Some(timer_id - 30),
        _ => None,
    }
}

pub(in crate::lua) fn is_keybeam_keyoff_timer(timer_id: i32) -> bool {
    matches!(timer_id, 120..=137)
}

pub(in crate::lua) fn infer_keybeam_timer_event_draw_condition(
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

pub(in crate::lua) fn infer_timer_function_ref(
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
pub(in crate::lua) fn infer_fixed_delay_timer(
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
pub(in crate::lua) fn infer_custom_timer_alias(
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

pub(in crate::lua) fn call_draw_with_timer_option(
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

pub(in crate::lua) fn call_draw_with_timer_options(
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

pub(in crate::lua) fn infer_main_state_gauge_type_draw_condition(
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

pub(in crate::lua) fn call_draw_with_gauge_type(
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

pub(in crate::lua) fn infer_judge_fast_slow_draw_condition(
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

pub(in crate::lua) fn unique_numbers_in_order(values: &[i32]) -> Vec<i32> {
    let mut unique = Vec::new();
    for value in values {
        if !unique.contains(value) {
            unique.push(*value);
        }
    }
    unique
}

pub(in crate::lua) fn is_constant_boolean_condition(condition: &str) -> bool {
    matches!(condition, "number(0) >= 0" | "number(0) < 0")
}
