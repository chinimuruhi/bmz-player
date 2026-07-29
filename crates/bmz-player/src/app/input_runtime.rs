use std::collections::HashSet;

use bmz_gameplay::input::backend::{DeviceId, DeviceInputEvent, PhysicalControl};
use bmz_gameplay::input::bounce::{InputBounceConfig, InputBounceFilter};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::PhysicalKey;

use crate::config::profile_config::InputActionConfig;
use crate::input::winit::{
    W_KEYBOARD_DEVICE_ID, physical_key_to_control, physical_key_to_device_input,
};

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
    raw_input_pressed_keys: HashSet<PhysicalKey>,
    window_input_pressed_keys: HashSet<PhysicalKey>,
    app_bounce_filter: InputBounceFilter,
    raw_bounce_filter: InputBounceFilter,
    discard_gamepad_output_until_resynced: bool,
}

pub(super) struct InputReleaseBatch {
    pub(super) raw_keyboard: Vec<DeviceInputEvent>,
    pub(super) window_keyboard: Vec<DeviceInputEvent>,
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

    pub(super) fn accept_app_event(
        &mut self,
        config: InputBounceConfig,
        event: DeviceInputEvent,
    ) -> Option<DeviceInputEvent> {
        self.app_bounce_filter.set_config(config);
        self.app_bounce_filter.accept(event)
    }

    /// UI がゲーム入力をブロックしていても、受理済みキーのReleaseは通す。
    /// バウンスとして抑制したPressは押下集合へ追加しない。
    pub(super) fn raw_keyboard_transition(
        &mut self,
        config: InputBounceConfig,
        physical_key: PhysicalKey,
        state: ElementState,
        gameplay_blocked: bool,
    ) -> Option<DeviceInputEvent> {
        let tracked = self.raw_input_pressed_keys.contains(&physical_key);
        if matches!(state, ElementState::Pressed) && (gameplay_blocked || tracked) {
            return None;
        }
        if matches!(state, ElementState::Released) && !tracked {
            return None;
        }
        let event = physical_key_to_device_input(physical_key, state, false)?;
        self.raw_bounce_filter.set_config(config);
        let event = self.raw_bounce_filter.accept(event)?;
        match state {
            ElementState::Pressed => {
                self.raw_input_pressed_keys.insert(physical_key);
            }
            ElementState::Released => {
                self.raw_input_pressed_keys.remove(&physical_key);
            }
        }
        Some(event)
    }

    pub(super) fn discard_raw_keyboard_transition(
        &mut self,
        physical_key: PhysicalKey,
        state: ElementState,
    ) {
        if state == ElementState::Released {
            self.raw_input_pressed_keys.remove(&physical_key);
        }
        self.raw_bounce_filter.clear();
    }

    pub(super) fn track_window_keyboard(
        &mut self,
        physical_key: PhysicalKey,
        state: ElementState,
        repeat: bool,
        gameplay_enabled: bool,
        has_play_context: bool,
    ) {
        if !gameplay_enabled || repeat {
            return;
        }
        match state {
            ElementState::Pressed if has_play_context => {
                self.window_input_pressed_keys.insert(physical_key);
            }
            ElementState::Released => {
                self.window_input_pressed_keys.remove(&physical_key);
            }
            ElementState::Pressed => {}
        }
    }

    pub(super) fn handle_focus_lost(&mut self) -> InputReleaseBatch {
        self.discard_gamepad_output_until_resynced = true;
        self.pressed_controls.clear();
        self.pressed_play_inputs.clear();
        InputReleaseBatch {
            raw_keyboard: self.take_raw_keyboard_releases(),
            window_keyboard: self.take_window_keyboard_releases(),
        }
    }

    pub(super) fn should_discard_gamepad_output(&mut self, focused: bool) -> bool {
        if !focused {
            return true;
        }
        std::mem::take(&mut self.discard_gamepad_output_until_resynced)
    }

    fn take_raw_keyboard_releases(&mut self) -> Vec<DeviceInputEvent> {
        let releases = release_events(&mut self.raw_input_pressed_keys);
        self.raw_bounce_filter.clear();
        releases
    }

    fn take_window_keyboard_releases(&mut self) -> Vec<DeviceInputEvent> {
        let releases = release_events(&mut self.window_input_pressed_keys);
        self.app_bounce_filter.clear();
        releases
    }
}

pub(super) fn should_route_gamepad_event_while_discarding(pressed: bool) -> bool {
    !pressed
}

fn release_events(pressed_keys: &mut HashSet<PhysicalKey>) -> Vec<DeviceInputEvent> {
    std::mem::take(pressed_keys)
        .into_iter()
        .filter_map(|physical_key| {
            physical_key_to_device_input(physical_key, ElementState::Released, false)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use bmz_core::input::InputKind;
    use winit::keyboard::KeyCode;

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

    #[test]
    fn raw_input_blocking_drops_new_presses_but_keeps_tracked_release() {
        let key = PhysicalKey::Code(KeyCode::KeyZ);
        let mut runtime = AppInputRuntime::default();

        let press = runtime
            .raw_keyboard_transition(
                InputBounceConfig::default(),
                key,
                ElementState::Pressed,
                false,
            )
            .unwrap();
        assert_eq!(press.kind, InputKind::Press);
        assert!(
            runtime
                .raw_keyboard_transition(
                    InputBounceConfig::default(),
                    key,
                    ElementState::Pressed,
                    true,
                )
                .is_none()
        );
        let release = runtime
            .raw_keyboard_transition(
                InputBounceConfig::default(),
                key,
                ElementState::Released,
                true,
            )
            .unwrap();
        assert_eq!(release.kind, InputKind::Release);
        assert!(runtime.raw_input_pressed_keys.is_empty());
    }

    #[test]
    fn bounced_raw_press_does_not_restore_pressed_state() {
        let key = PhysicalKey::Code(KeyCode::KeyZ);
        let config =
            InputBounceConfig { keyboard_threshold_us: 1_000_000, controller_threshold_us: 0 };
        let mut runtime = AppInputRuntime::default();

        assert!(
            runtime.raw_keyboard_transition(config, key, ElementState::Pressed, false).is_some()
        );
        assert!(
            runtime.raw_keyboard_transition(config, key, ElementState::Released, false).is_some()
        );
        assert!(
            runtime.raw_keyboard_transition(config, key, ElementState::Pressed, false).is_none()
        );
        assert!(runtime.raw_input_pressed_keys.is_empty());
        assert!(
            runtime.raw_keyboard_transition(config, key, ElementState::Released, false).is_none()
        );
    }

    #[test]
    fn focus_loss_releases_keyboard_state_and_resyncs_gamepad_once() {
        let raw_key = PhysicalKey::Code(KeyCode::KeyZ);
        let window_key = PhysicalKey::Code(KeyCode::KeyX);
        let mut runtime = AppInputRuntime::default();
        runtime
            .raw_keyboard_transition(
                InputBounceConfig::default(),
                raw_key,
                ElementState::Pressed,
                false,
            )
            .unwrap();
        runtime.track_window_keyboard(window_key, ElementState::Pressed, false, true, true);

        let releases = runtime.handle_focus_lost();

        assert_eq!(releases.raw_keyboard.len(), 1);
        assert_eq!(releases.raw_keyboard[0].kind, InputKind::Release);
        assert_eq!(releases.window_keyboard.len(), 1);
        assert_eq!(releases.window_keyboard[0].kind, InputKind::Release);
        assert!(runtime.should_discard_gamepad_output(false));
        assert!(runtime.should_discard_gamepad_output(true));
        assert!(!runtime.should_discard_gamepad_output(true));
        assert!(!should_route_gamepad_event_while_discarding(true));
        assert!(should_route_gamepad_event_while_discarding(false));
    }
}
