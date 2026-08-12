use super::*;

pub fn default_bindings() -> Vec<BindingConfigEntry> {
    let mut bindings = default_play_lane_bindings();
    bindings.extend(default_ui_bindings());
    bindings
}

pub fn default_ui_bindings() -> Vec<BindingConfigEntry> {
    default_keyboard_bindings()
        .into_iter()
        .filter(|entry| entry.action.is_some())
        .chain(default_gamepad_bindings().into_iter().filter(|entry| entry.action.is_some()))
        .collect()
}

fn default_play_lane_bindings() -> Vec<BindingConfigEntry> {
    default_keyboard_bindings()
        .into_iter()
        .filter(|entry| entry.lane.is_some())
        .chain(default_gamepad_bindings().into_iter().filter(|entry| entry.lane.is_some()))
        .collect()
}

pub fn default_keyboard_bindings() -> Vec<BindingConfigEntry> {
    vec![
        scratch_binding("LShift", LaneConfig::Scratch, ScratchDirectionConfig::Up),
        scratch_binding("LControl", LaneConfig::Scratch, ScratchDirectionConfig::Down),
        binding("Z", LaneConfig::Key1),
        binding("S", LaneConfig::Key2),
        binding("X", LaneConfig::Key3),
        binding("D", LaneConfig::Key4),
        binding("C", LaneConfig::Key5),
        binding("F", LaneConfig::Key6),
        binding("V", LaneConfig::Key7),
        action_binding("Q", InputActionConfig::E1),
        action_binding("Z", InputActionConfig::SelectEnter),
        action_binding("X", InputActionConfig::SelectEnter),
        action_binding("C", InputActionConfig::SelectEnter),
        action_binding("V", InputActionConfig::SelectEnter),
        action_binding("W", InputActionConfig::E2),
        action_binding("E", InputActionConfig::E3),
        action_binding("R", InputActionConfig::E4),
        action_binding("Z", InputActionConfig::SelectOptionArrange),
        action_binding("X", InputActionConfig::SelectOptionGauge),
        action_binding("C", InputActionConfig::SelectOptionAssist),
        action_binding("Z", InputActionConfig::SelectOptionBga),
        action_binding("F8", InputActionConfig::SelectFavoriteSong),
        action_binding("F9", InputActionConfig::SelectFavoriteChart),
        action_binding("Numpad8", InputActionConfig::SelectSameFolder),
        action_binding("Numpad0", InputActionConfig::SelectDifficultyFilter),
        action_binding("Numpad4", InputActionConfig::SelectReplayCycle),
        action_binding("Numpad5", InputActionConfig::SelectReplayPlay),
    ]
}

pub fn default_gamepad_bindings() -> Vec<BindingConfigEntry> {
    vec![
        gamepad_scratch_binding("Axis1+", LaneConfig::Scratch, ScratchDirectionConfig::Up),
        gamepad_scratch_binding("Axis1-", LaneConfig::Scratch, ScratchDirectionConfig::Down),
        gamepad_binding("Button1", LaneConfig::Key1),
        gamepad_binding("Button2", LaneConfig::Key2),
        gamepad_binding("Button3", LaneConfig::Key3),
        gamepad_binding("Button4", LaneConfig::Key4),
        gamepad_binding("Button5", LaneConfig::Key5),
        gamepad_binding("Button6", LaneConfig::Key6),
        gamepad_binding("Button7", LaneConfig::Key7),
        gamepad_action_binding("Button9", InputActionConfig::E1),
        gamepad_action_binding("Button1", InputActionConfig::SelectEnter),
        gamepad_action_binding("Button10", InputActionConfig::E2),
        gamepad_action_binding("Button11", InputActionConfig::E3),
        gamepad_action_binding("Button12", InputActionConfig::E4),
        gamepad_action_binding("Button1", InputActionConfig::SelectOptionArrange),
        gamepad_action_binding("Button3", InputActionConfig::SelectOptionGauge),
        gamepad_action_binding("Button5", InputActionConfig::SelectOptionAssist),
        gamepad_action_binding("Button1", InputActionConfig::SelectOptionBga),
    ]
}

fn binding(control: &str, lane: LaneConfig) -> BindingConfigEntry {
    BindingConfigEntry {
        device: "keyboard".to_string(),
        control: control.to_string(),
        keyboard_slot: None,
        lane: Some(lane),
        action: None,
        scratch: None,
    }
}

fn scratch_binding(
    control: &str,
    lane: LaneConfig,
    scratch: ScratchDirectionConfig,
) -> BindingConfigEntry {
    let mut entry = binding(control, lane);
    entry.scratch = Some(scratch);
    entry
}

fn gamepad_binding(control: &str, lane: LaneConfig) -> BindingConfigEntry {
    BindingConfigEntry {
        device: "gamepad".to_string(),
        control: control.to_string(),
        keyboard_slot: None,
        lane: Some(lane),
        action: None,
        scratch: None,
    }
}

fn gamepad_scratch_binding(
    control: &str,
    lane: LaneConfig,
    scratch: ScratchDirectionConfig,
) -> BindingConfigEntry {
    let mut entry = gamepad_binding(control, lane);
    entry.scratch = Some(scratch);
    entry
}

fn action_binding(control: &str, action: InputActionConfig) -> BindingConfigEntry {
    BindingConfigEntry {
        device: "keyboard".to_string(),
        control: control.to_string(),
        keyboard_slot: None,
        lane: None,
        action: Some(action),
        scratch: None,
    }
}

fn gamepad_action_binding(control: &str, action: InputActionConfig) -> BindingConfigEntry {
    BindingConfigEntry {
        device: "gamepad".to_string(),
        control: control.to_string(),
        keyboard_slot: None,
        lane: None,
        action: Some(action),
        scratch: None,
    }
}
