use super::*;

/// 指定スロットへキーボード / コントローラー割り当てを更新する。
pub fn apply_play_binding(
    input: &mut ProfileInputConfig,
    key_mode: KeyMode,
    target: KeyBindingTarget,
    control: &str,
) -> Result<(), crate::config::play_input::InheritError> {
    if let KeyBindingTarget::Action { action, slot } = target {
        apply_action_binding(input, action, slot, control);
        return Ok(());
    }

    let lane = lane_for_target(target);
    if !key_mode.active_lanes().contains(&lane_from_config(lane)) {
        return Ok(());
    }

    let slot = target.slot();
    let mut bindings = resolve_play_bindings(input, key_mode)?;
    remove_control_from_device(&mut bindings, slot.device(), control);

    match target {
        KeyBindingTarget::Key { lane, slot } => {
            let keyboard = read_lane_keyboard_slots(&bindings, lane);
            let primary = keyboard.primary;
            let secondary = keyboard.secondary;

            match slot {
                KeyBindingSlot::KeyboardPrimary => {
                    remove_lane_device_bindings(&mut bindings, lane, "keyboard");
                    write_lane_keyboard_bindings(
                        &mut bindings,
                        lane,
                        Some(control),
                        secondary.as_deref(),
                    );
                }
                KeyBindingSlot::KeyboardSecondary => {
                    remove_lane_device_bindings(&mut bindings, lane, "keyboard");
                    write_lane_keyboard_bindings(
                        &mut bindings,
                        lane,
                        primary.as_deref(),
                        Some(control),
                    );
                }
                KeyBindingSlot::Controller
                | KeyBindingSlot::Controller1P
                | KeyBindingSlot::Controller2P => {
                    write_lane_gamepad_bindings_for_device(
                        &mut bindings,
                        lane,
                        slot.device(),
                        &[control.to_string()],
                    );
                }
            }
        }
        KeyBindingTarget::Scratch { lane, direction, slot } => match slot {
            KeyBindingSlot::KeyboardPrimary | KeyBindingSlot::KeyboardSecondary => {
                let mut keyboard = read_scratch_keyboard_slots(&bindings, lane);
                keyboard.set(direction, slot, Some(control.to_string()));
                write_scratch_keyboard_bindings(&mut bindings, lane, &keyboard);
            }
            KeyBindingSlot::Controller
            | KeyBindingSlot::Controller1P
            | KeyBindingSlot::Controller2P => {
                let mut gamepad =
                    read_scratch_gamepad_slots_for_device(&bindings, lane, slot.device());
                gamepad.set(direction, Some(control.to_string()));
                write_scratch_gamepad_bindings_for_device(
                    &mut bindings,
                    lane,
                    slot.device(),
                    &gamepad,
                );
            }
        },
        KeyBindingTarget::Action { .. } => unreachable!("action binding is handled above"),
    }

    persist_bindings(input, key_mode, bindings)
}

/// 指定スロットの割り当てを削除する。
pub fn clear_play_binding(
    input: &mut ProfileInputConfig,
    key_mode: KeyMode,
    target: KeyBindingTarget,
) -> Result<(), crate::config::play_input::InheritError> {
    if let KeyBindingTarget::Action { action, slot } = target {
        clear_action_binding(input, action, slot);
        return Ok(());
    }

    let lane = lane_for_target(target);
    if !key_mode.active_lanes().contains(&lane_from_config(lane)) {
        return Ok(());
    }

    let mut bindings = resolve_play_bindings(input, key_mode)?;

    match target {
        KeyBindingTarget::Key { lane, slot } => {
            let keyboard = read_lane_keyboard_slots(&bindings, lane);
            let primary = keyboard.primary;
            let secondary = keyboard.secondary;

            match slot {
                KeyBindingSlot::KeyboardPrimary => {
                    remove_lane_device_bindings(&mut bindings, lane, "keyboard");
                    write_lane_keyboard_bindings(&mut bindings, lane, None, secondary.as_deref());
                }
                KeyBindingSlot::KeyboardSecondary => {
                    remove_lane_device_bindings(&mut bindings, lane, "keyboard");
                    write_lane_keyboard_bindings(&mut bindings, lane, primary.as_deref(), None);
                }
                KeyBindingSlot::Controller
                | KeyBindingSlot::Controller1P
                | KeyBindingSlot::Controller2P => {
                    write_lane_gamepad_bindings_for_device(&mut bindings, lane, slot.device(), &[]);
                }
            }
        }
        KeyBindingTarget::Scratch { lane, direction, slot } => match slot {
            KeyBindingSlot::KeyboardPrimary | KeyBindingSlot::KeyboardSecondary => {
                let mut keyboard = read_scratch_keyboard_slots(&bindings, lane);
                keyboard.set(direction, slot, None);
                write_scratch_keyboard_bindings(&mut bindings, lane, &keyboard);
            }
            KeyBindingSlot::Controller
            | KeyBindingSlot::Controller1P
            | KeyBindingSlot::Controller2P => {
                let mut gamepad =
                    read_scratch_gamepad_slots_for_device(&bindings, lane, slot.device());
                gamepad.set(direction, None);
                write_scratch_gamepad_bindings_for_device(
                    &mut bindings,
                    lane,
                    slot.device(),
                    &gamepad,
                );
            }
        },
        KeyBindingTarget::Action { .. } => unreachable!("action binding is handled above"),
    }

    persist_bindings(input, key_mode, bindings)
}

pub fn snapshot_play_mode_config(
    input: &ProfileInputConfig,
    key_mode: KeyMode,
) -> Option<PlayModeInputConfig> {
    input.play.get(key_mode.play_map_key()).cloned()
}

pub fn restore_play_mode_config(
    input: &mut ProfileInputConfig,
    key_mode: KeyMode,
    config: Option<PlayModeInputConfig>,
) {
    match config {
        Some(config) => {
            input.play.insert(key_mode.play_map_key().to_string(), config);
        }
        None => {
            input.play.remove(key_mode.play_map_key());
        }
    }
}

pub(super) fn ensure_play_mode_config(
    input: &mut ProfileInputConfig,
    key_mode: KeyMode,
) -> &mut PlayModeInputConfig {
    input.play.entry(key_mode.play_map_key().to_string()).or_insert_with(|| PlayModeInputConfig {
        inherit: None,
        bindings: default_play_bindings(key_mode),
        ..Default::default()
    })
}
