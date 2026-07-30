use super::*;

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
