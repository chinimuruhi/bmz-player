use std::collections::HashSet;

use bmz_gameplay::input::backend::{DeviceId, PhysicalControl};
use bmz_gameplay::input::bounce::InputBounceFilter;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::PhysicalKey;

use crate::config::profile_config::InputActionConfig;
use crate::input::winit::{W_KEYBOARD_DEVICE_ID, physical_key_to_control};

/// keyboard/gamepadに依存しないapp controlの押下・解放イベント。
pub(super) struct ControlInputEvent {
    pub(super) device: DeviceId,
    pub(super) name: Option<String>,
    pub(super) physical: Option<PhysicalControl>,
    pub(super) pressed: bool,
    pub(super) repeat: bool,
}

impl ControlInputEvent {
    pub(super) fn keyboard(event: &KeyEvent) -> Self {
        Self::keyboard_parts(event.physical_key, event.state, event.repeat)
    }

    pub(super) fn keyboard_parts(
        physical_key: PhysicalKey,
        state: ElementState,
        repeat: bool,
    ) -> Self {
        let physical = physical_key_to_control(physical_key);
        let name = (!repeat)
            .then(|| match physical.as_ref()? {
                PhysicalControl::KeyboardKey(name) => Some(name.clone()),
                _ => None,
            })
            .flatten();
        Self {
            device: W_KEYBOARD_DEVICE_ID,
            name,
            physical,
            pressed: state == ElementState::Pressed,
            repeat,
        }
    }

    pub(super) fn gamepad(device: DeviceId, name: &str, pressed: bool) -> Self {
        Self {
            device,
            name: Some(name.to_string()),
            physical: Some(PhysicalControl::GamepadButton(name.to_string())),
            pressed,
            repeat: false,
        }
    }
}

/// app全体で共有する押下集合とkeyboard bounce状態。
#[derive(Default)]
pub(super) struct AppInputRuntime {
    pub(super) start_held: bool,
    pub(super) select_held: bool,
    pub(super) select_e_action_holds: HashSet<InputActionConfig>,
    pub(super) pressed_controls: HashSet<String>,
    pub(super) pressed_play_inputs: HashSet<(DeviceId, PhysicalControl)>,
    pub(super) raw_input_pressed_keys: HashSet<PhysicalKey>,
    pub(super) window_input_pressed_keys: HashSet<PhysicalKey>,
    pub(super) app_bounce_filter: InputBounceFilter,
    pub(super) raw_bounce_filter: InputBounceFilter,
}

impl AppInputRuntime {
    pub(super) fn track_control(&mut self, event: &ControlInputEvent) {
        if let Some(name) = event.name.as_deref() {
            if event.pressed {
                self.pressed_controls.insert(name.to_string());
            } else {
                self.pressed_controls.remove(name);
            }
        }
        if let Some(physical) = event.physical.as_ref() {
            let input = (event.device, physical.clone());
            if event.pressed {
                self.pressed_play_inputs.insert(input);
            } else {
                self.pressed_play_inputs.remove(&input);
            }
        }
    }

    pub(super) fn select_e_action_held(&self) -> bool {
        !self.select_e_action_holds.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_control_press_and_release_in_shared_state() {
        let mut runtime = AppInputRuntime::default();
        let gamepad = ControlInputEvent::gamepad(DeviceId(2), "ButtonSouth", true);
        runtime.track_control(&gamepad);
        assert!(runtime.pressed_controls.contains("ButtonSouth"));
        assert!(
            runtime.pressed_play_inputs.contains(&(
                DeviceId(2),
                PhysicalControl::GamepadButton("ButtonSouth".to_string())
            ))
        );

        let released = ControlInputEvent::gamepad(DeviceId(2), "ButtonSouth", false);
        runtime.track_control(&released);
        assert!(!runtime.pressed_controls.contains("ButtonSouth"));
        assert!(runtime.pressed_play_inputs.is_empty());
    }
}
