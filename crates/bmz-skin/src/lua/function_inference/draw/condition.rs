use super::*;

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
