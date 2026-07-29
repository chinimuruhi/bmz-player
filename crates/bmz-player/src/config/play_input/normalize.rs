use super::*;

pub fn normalize_profile_input(input: &mut ProfileInputConfig) {
    if !input.legacy_bindings.is_empty() {
        let (ui, play) = migrate_legacy_bindings(&input.legacy_bindings);
        if input.ui.bindings.is_empty() && !ui.is_empty() {
            input.ui.bindings = ui;
        }
        if input.play.is_empty() && !play.is_empty() {
            input.play = play;
        }
        input.legacy_bindings.clear();
    }
    normalize_play_map_keys(&mut input.play);
    if input.ui.bindings.is_empty() {
        input.ui.bindings = crate::config::profile_config::default_ui_bindings();
    }
}

pub fn default_profile_input() -> ProfileInputConfig {
    let mut play = BTreeMap::new();
    play.insert(
        KeyMode::K7.play_map_key().to_string(),
        PlayModeInputConfig {
            inherit: None,
            bindings: default_play_7k_bindings(),
            ..Default::default()
        },
    );
    ProfileInputConfig {
        scratch_mode: crate::config::profile_config::ScratchInputMode::Normal,
        select_input_mode: crate::config::profile_config::SelectInputModeConfig::Key7Key14,
        start_key: None,
        ui: crate::config::profile_config::UiInputConfig {
            bindings: crate::config::profile_config::default_ui_bindings(),
        },
        play,
        legacy_bindings: Vec::new(),
        analog_scratch_sensitivity: 1.0,
        analog_scratch_timeout_ms: 500,
        analog_scratch_threshold: 100,
        analog_ticks_per_scroll: 3,
        keyboard_release_bounce_ms: 0,
        controller_release_bounce_ms: 0,
    }
}

pub fn normalize_play_map_keys(play: &mut BTreeMap<String, PlayModeInputConfig>) {
    let old = std::mem::take(play);
    for (key, value) in old {
        play.insert(normalize_play_map_key(&key), value);
    }
}

pub fn normalize_play_map_key(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}

pub fn migrate_legacy_bindings(
    legacy: &[BindingConfigEntry],
) -> (Vec<BindingConfigEntry>, BTreeMap<String, PlayModeInputConfig>) {
    let mut ui_bindings = Vec::new();
    let mut play_7k = Vec::new();
    let mut play_14k = Vec::new();

    for entry in legacy {
        if entry.action.is_some() {
            ui_bindings.push(entry.clone());
            continue;
        }
        let Some(lane) = entry.lane else { continue };
        match lane {
            LaneConfig::Scratch
            | LaneConfig::Key1
            | LaneConfig::Key2
            | LaneConfig::Key3
            | LaneConfig::Key4
            | LaneConfig::Key5
            | LaneConfig::Key6
            | LaneConfig::Key7 => play_7k.push(entry.clone()),
            LaneConfig::Scratch2
            | LaneConfig::Key8
            | LaneConfig::Key9
            | LaneConfig::Key10
            | LaneConfig::Key11
            | LaneConfig::Key12
            | LaneConfig::Key13
            | LaneConfig::Key14 => play_14k.push(entry.clone()),
        }
    }

    let mut play = BTreeMap::new();
    if !play_7k.is_empty() {
        play.insert(
            KeyMode::K7.play_map_key().to_string(),
            PlayModeInputConfig { inherit: None, bindings: play_7k, ..Default::default() },
        );
    }
    if !play_14k.is_empty() {
        play.insert(
            KeyMode::K14.play_map_key().to_string(),
            PlayModeInputConfig { inherit: None, bindings: play_14k, ..Default::default() },
        );
    }
    (ui_bindings, play)
}
