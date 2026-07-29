use super::*;

pub fn default_play_bindings(key_mode: KeyMode) -> Vec<BindingConfigEntry> {
    match key_mode {
        KeyMode::K7 => default_play_7k_bindings(),
        KeyMode::K8 => default_play_8k_bindings(),
        KeyMode::K14 => default_play_14k_bindings(),
        KeyMode::K9 => default_play_9k_bindings(),
        KeyMode::K5 | KeyMode::K4 | KeyMode::K6 | KeyMode::K10 => Vec::new(),
    }
}

pub fn default_play_7k_bindings() -> Vec<BindingConfigEntry> {
    let mut bindings = default_play_7k_keyboard_bindings();
    bindings.extend(default_play_7k_gamepad_bindings());
    bindings
}

pub fn default_play_7k_keyboard_bindings() -> Vec<BindingConfigEntry> {
    vec![
        scratch_play_binding("LShift", LaneConfig::Scratch, ScratchDirectionConfig::Up),
        scratch_play_binding("LControl", LaneConfig::Scratch, ScratchDirectionConfig::Down),
        play_binding("Z", LaneConfig::Key1),
        play_binding("S", LaneConfig::Key2),
        play_binding("X", LaneConfig::Key3),
        play_binding("D", LaneConfig::Key4),
        play_binding("C", LaneConfig::Key5),
        play_binding("F", LaneConfig::Key6),
        play_binding("V", LaneConfig::Key7),
    ]
}

pub fn default_play_7k_gamepad_bindings() -> Vec<BindingConfigEntry> {
    vec![
        gamepad_scratch_play_binding_for_device(
            "gamepad",
            "Axis1+",
            LaneConfig::Scratch,
            ScratchDirectionConfig::Up,
        ),
        gamepad_scratch_play_binding_for_device(
            "gamepad",
            "Axis1-",
            LaneConfig::Scratch,
            ScratchDirectionConfig::Down,
        ),
        gamepad_play_binding("Button1", LaneConfig::Key1),
        gamepad_play_binding("Button2", LaneConfig::Key2),
        gamepad_play_binding("Button3", LaneConfig::Key3),
        gamepad_play_binding("Button4", LaneConfig::Key4),
        gamepad_play_binding("Button5", LaneConfig::Key5),
        gamepad_play_binding("Button6", LaneConfig::Key6),
        gamepad_play_binding("Button7", LaneConfig::Key7),
    ]
}

pub fn default_play_14k_bindings() -> Vec<BindingConfigEntry> {
    let mut bindings = vec![
        scratch_play_binding("LShift", LaneConfig::Scratch, ScratchDirectionConfig::Up),
        scratch_play_binding("LControl", LaneConfig::Scratch, ScratchDirectionConfig::Down),
        play_binding("Z", LaneConfig::Key1),
        play_binding("S", LaneConfig::Key2),
        play_binding("X", LaneConfig::Key3),
        play_binding("D", LaneConfig::Key4),
        play_binding("C", LaneConfig::Key5),
        play_binding("F", LaneConfig::Key6),
        play_binding("V", LaneConfig::Key7),
        scratch_play_binding("RShift", LaneConfig::Scratch2, ScratchDirectionConfig::Up),
        scratch_play_binding("RControl", LaneConfig::Scratch2, ScratchDirectionConfig::Down),
        play_binding("M", LaneConfig::Key8),
        play_binding("K", LaneConfig::Key9),
        play_binding("Comma", LaneConfig::Key10),
        play_binding("L", LaneConfig::Key11),
        play_binding("Period", LaneConfig::Key12),
        play_binding("Semicolon", LaneConfig::Key13),
        play_binding("Slash", LaneConfig::Key14),
    ];
    bindings.extend([
        gamepad_scratch_play_binding_for_device(
            "gamepad1",
            "Axis1+",
            LaneConfig::Scratch,
            ScratchDirectionConfig::Up,
        ),
        gamepad_scratch_play_binding_for_device(
            "gamepad1",
            "Axis1-",
            LaneConfig::Scratch,
            ScratchDirectionConfig::Down,
        ),
        gamepad_play_binding_for_device("gamepad1", "Button1", LaneConfig::Key1),
        gamepad_play_binding_for_device("gamepad1", "Button2", LaneConfig::Key2),
        gamepad_play_binding_for_device("gamepad1", "Button3", LaneConfig::Key3),
        gamepad_play_binding_for_device("gamepad1", "Button4", LaneConfig::Key4),
        gamepad_play_binding_for_device("gamepad1", "Button5", LaneConfig::Key5),
        gamepad_play_binding_for_device("gamepad1", "Button6", LaneConfig::Key6),
        gamepad_play_binding_for_device("gamepad1", "Button7", LaneConfig::Key7),
    ]);
    bindings.extend([
        gamepad_scratch_play_binding_for_device(
            "gamepad2",
            "Axis1-",
            LaneConfig::Scratch2,
            ScratchDirectionConfig::Up,
        ),
        gamepad_scratch_play_binding_for_device(
            "gamepad2",
            "Axis1+",
            LaneConfig::Scratch2,
            ScratchDirectionConfig::Down,
        ),
        gamepad_play_binding_for_device("gamepad2", "Button1", LaneConfig::Key8),
        gamepad_play_binding_for_device("gamepad2", "Button2", LaneConfig::Key9),
        gamepad_play_binding_for_device("gamepad2", "Button3", LaneConfig::Key10),
        gamepad_play_binding_for_device("gamepad2", "Button4", LaneConfig::Key11),
        gamepad_play_binding_for_device("gamepad2", "Button5", LaneConfig::Key12),
        gamepad_play_binding_for_device("gamepad2", "Button6", LaneConfig::Key13),
        gamepad_play_binding_for_device("gamepad2", "Button7", LaneConfig::Key14),
    ]);
    bindings
}

pub fn default_play_9k_bindings() -> Vec<BindingConfigEntry> {
    vec![
        play_binding("Z", LaneConfig::Key1),
        play_binding("S", LaneConfig::Key2),
        play_binding("X", LaneConfig::Key3),
        play_binding("D", LaneConfig::Key4),
        play_binding("C", LaneConfig::Key5),
        play_binding("F", LaneConfig::Key6),
        play_binding("V", LaneConfig::Key7),
        play_binding("G", LaneConfig::Key8),
        play_binding("B", LaneConfig::Key9),
    ]
}

pub fn default_play_8k_bindings() -> Vec<BindingConfigEntry> {
    vec![
        play_binding("Z", LaneConfig::Key1),
        play_binding("S", LaneConfig::Key2),
        play_binding("X", LaneConfig::Key3),
        play_binding("D", LaneConfig::Key4),
        play_binding("C", LaneConfig::Key5),
        play_binding("F", LaneConfig::Key6),
        play_binding("V", LaneConfig::Key7),
        play_binding("G", LaneConfig::Key8),
    ]
}

pub fn play_binding(control: &str, lane: LaneConfig) -> BindingConfigEntry {
    BindingConfigEntry {
        device: "keyboard".to_string(),
        control: control.to_string(),
        keyboard_slot: None,
        lane: Some(lane),
        action: None,
        scratch: None,
    }
}

pub fn scratch_play_binding(
    control: &str,
    lane: LaneConfig,
    scratch: ScratchDirectionConfig,
) -> BindingConfigEntry {
    let mut entry = play_binding(control, lane);
    entry.scratch = Some(scratch);
    entry
}

pub fn gamepad_play_binding(control: &str, lane: LaneConfig) -> BindingConfigEntry {
    gamepad_play_binding_for_device("gamepad", control, lane)
}

pub fn gamepad_play_binding_for_device(
    device: &str,
    control: &str,
    lane: LaneConfig,
) -> BindingConfigEntry {
    BindingConfigEntry {
        device: device.to_string(),
        control: control.to_string(),
        keyboard_slot: None,
        lane: Some(lane),
        action: None,
        scratch: None,
    }
}

pub fn gamepad_scratch_play_binding_for_device(
    device: &str,
    control: &str,
    lane: LaneConfig,
    scratch: ScratchDirectionConfig,
) -> BindingConfigEntry {
    let mut entry = gamepad_play_binding_for_device(device, control, lane);
    entry.scratch = Some(scratch);
    entry
}
