use super::*;

pub fn is_gamepad_device(device: &str) -> bool {
    gamepad_player_index(device).is_some() || device.trim().eq_ignore_ascii_case("gamepad")
}

pub(super) fn binding_device_from_config(device: &str, slots: GamepadSlotMap) -> Option<DeviceId> {
    gamepad_player_index(device).and_then(|index| slots.device_id_for_player(index))
}

pub fn gamepad_player_index(device: &str) -> Option<u32> {
    let lower = device.trim().to_ascii_lowercase();
    let suffix = lower.strip_prefix("gamepad")?;
    if suffix.is_empty() {
        return None;
    }
    suffix.parse::<u32>().ok().filter(|index| *index > 0)
}

pub(super) fn control_from_config(device: &str, control: &str) -> PhysicalControl {
    match device.to_ascii_lowercase().as_str() {
        device if is_gamepad_device(device) => PhysicalControl::GamepadButton(control.to_string()),
        "hid" => control
            .parse::<u32>()
            .map(PhysicalControl::HidButton)
            .unwrap_or_else(|_| PhysicalControl::KeyboardKey(control.to_string())),
        _ => PhysicalControl::KeyboardKey(control.to_string()),
    }
}
