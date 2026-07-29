use super::*;

pub(super) fn scene_kind(scene: &AppSceneSnapshot) -> AppSceneKind {
    match scene {
        AppSceneSnapshot::Select(_) => AppSceneKind::Select,
        AppSceneSnapshot::Decide(_) => AppSceneKind::Decide,
        AppSceneSnapshot::Play(_) => AppSceneKind::Play,
        AppSceneSnapshot::Result(_) => AppSceneKind::Result,
    }
}

pub(super) fn window_title_for_scene(scene_kind: AppSceneKind) -> &'static str {
    match scene_kind {
        AppSceneKind::Select => "bmz-player - Select",
        AppSceneKind::Decide => "bmz-player - Decide",
        AppSceneKind::Play => "bmz-player - Play",
        AppSceneKind::Result => "bmz-player - Result",
    }
}

pub(super) fn discord_key_mode_label(key_mode: KeyMode) -> String {
    let value = key_mode.as_str().strip_suffix('K').unwrap_or(key_mode.as_str());
    format!("{value}Keys")
}

pub(super) fn discord_join_metadata(first: &str, second: &str, separator: &str) -> Option<String> {
    let first = first.trim();
    let second = second.trim();
    match (first.is_empty(), second.is_empty()) {
        (true, true) => None,
        (false, true) => Some(first.to_string()),
        (true, false) => Some(second.to_string()),
        (false, false) => Some(format!("{first}{separator}{second}")),
    }
}

pub(super) fn physical_key_name(physical_key: PhysicalKey) -> Option<String> {
    use bmz_gameplay::input::backend::PhysicalControl;
    match physical_key_to_control(physical_key)? {
        PhysicalControl::KeyboardKey(name) => Some(name),
        _ => None,
    }
}

pub(super) fn physical_control_name(control: &PhysicalControl) -> Option<&str> {
    match control {
        PhysicalControl::KeyboardKey(name) | PhysicalControl::GamepadButton(name) => {
            Some(name.as_str())
        }
        PhysicalControl::HidButton(_) => None,
    }
}

pub(super) fn result_panel_for_control(control: &PhysicalControl) -> Option<i32> {
    match physical_control_name(control)? {
        "ArrowLeft" => Some(2),
        "ArrowRight" => Some(1),
        _ => None,
    }
}

pub(super) fn digit_to_replay_slot(physical_key: PhysicalKey) -> Option<u8> {
    match physical_key {
        PhysicalKey::Code(KeyCode::Digit1) => Some(0),
        PhysicalKey::Code(KeyCode::Digit2) => Some(1),
        PhysicalKey::Code(KeyCode::Digit3) => Some(2),
        PhysicalKey::Code(KeyCode::Digit4) => Some(3),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TargetCycle {
    Previous,
    Next,
}

pub(super) fn target_cycle_from_key(physical_key: PhysicalKey) -> Option<TargetCycle> {
    match physical_key {
        PhysicalKey::Code(KeyCode::ArrowUp) => Some(TargetCycle::Next),
        PhysicalKey::Code(KeyCode::ArrowDown) => Some(TargetCycle::Previous),
        _ => None,
    }
}

pub(super) fn target_cycle_from_control(
    control: &str,
    bindings: &SelectKeyBindings,
) -> Option<TargetCycle> {
    if control == "ScratchUp" || bindings.is_target_previous(control) {
        Some(TargetCycle::Next)
    } else if control == "ScratchDown" || bindings.is_target_next(control) {
        Some(TargetCycle::Previous)
    } else {
        None
    }
}

pub(super) fn select_option_lane_for_gamepad(
    input: &ProfileInputConfig,
    slots: crate::input::gamepad::GamepadSlotMap,
    device: DeviceId,
    control: &str,
) -> Option<Lane> {
    crate::config::play::lane_binding_for_chart_with_slots(input, KeyMode::K14, slots)
        .resolve(device, &PhysicalControl::GamepadButton(control.to_string()))
}
