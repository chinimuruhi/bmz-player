use super::*;

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
