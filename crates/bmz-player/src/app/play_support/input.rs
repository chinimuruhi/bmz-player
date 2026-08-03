#[cfg(test)]
pub(in crate::app) fn hispeed_action(
    physical_key: PhysicalKey,
    state: ElementState,
    repeat: bool,
) -> Option<HispeedChange> {
    match keyboard_lane_action(&ControlInputEvent::keyboard_parts(physical_key, state, repeat)) {
        Some(PlayLaneAction::Hispeed(change)) => Some(change),
        _ => None,
    }
}

pub(in crate::app) fn play_option_control_for_input(
    device: DeviceId,
    control: &PhysicalControl,
    e1_held: bool,
    e2_held: bool,
    play_input: Option<&PlayOptionInput>,
    profile_input: &ProfileInputConfig,
) -> Option<PlayOptionControl> {
    let play_input = play_input?;
    let resolved = play_input.resolve_entry(device, control);
    if e1_held && resolved.is_none() && play_input.is_action(device, control, InputActionConfig::E2)
    {
        return Some(PlayOptionControl::ToggleHispeedMode);
    }
    if e1_held == e2_held {
        return None;
    }

    let resolved = resolved?;
    if let Some(direction) = crate::config::play_input::hispeed_direction_for_lane(
        profile_input,
        play_input.key_mode,
        resolved.lane,
    ) {
        return match (e1_held, direction) {
            (true, HispeedDirectionConfig::Down) => {
                Some(PlayOptionControl::Hispeed(HispeedChange::Down))
            }
            (true, HispeedDirectionConfig::Up) => {
                Some(PlayOptionControl::Hispeed(HispeedChange::Up))
            }
            (false, HispeedDirectionConfig::Down) => {
                Some(PlayOptionControl::GreenNumber(GreenNumberChange::Down))
            }
            (false, HispeedDirectionConfig::Up) => {
                Some(PlayOptionControl::GreenNumber(GreenNumberChange::Up))
            }
        };
    }

    let control_name = physical_control_name(control);
    let scratch_direction = resolved.scratch_direction.or_else(|| {
        control_name.and_then(|control| {
            if is_scratch_up_control(control) {
                Some(ScratchDirection::Up)
            } else if is_scratch_down_control(control) {
                Some(ScratchDirection::Down)
            } else {
                None
            }
        })
    })?;
    match (e1_held, scratch_direction) {
        (true, ScratchDirection::Up) => Some(PlayOptionControl::LaneCover(LaneCoverChange::Up)),
        (true, ScratchDirection::Down) => Some(PlayOptionControl::LaneCover(LaneCoverChange::Down)),
        (false, ScratchDirection::Up) => {
            Some(PlayOptionControl::GreenNumber(GreenNumberChange::Up))
        }
        (false, ScratchDirection::Down) => {
            Some(PlayOptionControl::GreenNumber(GreenNumberChange::Down))
        }
    }
}

pub(in crate::app) fn visual_offset_delta_control(
    control: &str,
    bindings: &SelectKeyBindings,
) -> Option<i32> {
    if bindings.is_ui_key5(control) {
        Some(-1)
    } else if bindings.is_ui_key7(control) {
        Some(1)
    } else {
        None
    }
}

pub(in crate::app) fn green_number_delta_control(
    control: &str,
    bindings: &SelectKeyBindings,
) -> Option<i32> {
    if bindings.is_ui_key4(control) {
        Some(-1)
    } else if bindings.is_ui_key6(control) {
        Some(1)
    } else {
        None
    }
}

#[cfg(test)]
pub(in crate::app) fn lane_cover_step(
    physical_key: PhysicalKey,
    state: ElementState,
    repeat: bool,
) -> Option<f32> {
    match keyboard_lane_action(&ControlInputEvent::keyboard_parts(physical_key, state, repeat)) {
        Some(PlayLaneAction::LaneCoverDelta(delta)) => Some(delta),
        _ => None,
    }
}

pub(in crate::app) fn lane_cover_change_step(change: LaneCoverChange) -> f32 {
    match change {
        LaneCoverChange::Up => LANE_COVER_STEP,
        LaneCoverChange::Down => -LANE_COVER_STEP,
    }
}

/// アナログスクラッチによる緑数字操作は、レーンカバー操作とは増減方向が逆。
/// 正の step (Scratch Down) で緑数字を上げ、負の step (Scratch Up) で下げる。
pub(in crate::app) fn green_number_change_from_analog_steps(steps: i32) -> GreenNumberChange {
    if steps > 0 { GreenNumberChange::Up } else { GreenNumberChange::Down }
}

pub(in crate::app) fn green_number_change_step(change: GreenNumberChange) -> i32 {
    match change {
        GreenNumberChange::Up => 1,
        GreenNumberChange::Down => -1,
    }
}

pub(in crate::app) fn hispeed_step_for_profile(profile: &ProfileConfig, mode: HispeedMode) -> f32 {
    match mode {
        HispeedMode::Normal => {
            normalize_hispeed_step(profile.lane.hispeed_step_nhs, default_hispeed_step_nhs())
        }
        HispeedMode::Floating => {
            normalize_hispeed_step(profile.lane.hispeed_step_fhs, default_hispeed_step_fhs())
        }
    }
}

pub(in crate::app) fn adjusted_hispeed(current: f32, change: HispeedChange, step: f32) -> f32 {
    let delta = match change {
        HispeedChange::Down => -step,
        HispeedChange::Up => step,
    };
    clamp_hispeed(current + delta)
}
use super::*;
