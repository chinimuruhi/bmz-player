use bmz_gameplay::input::backend::PhysicalControl;

use super::input_runtime::ControlInputEvent;
use super::{LANE_COVER_REPEAT_STEP, LANE_COVER_STEP};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HispeedChange {
    Down,
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum PlayLaneAction {
    ToggleHispeedMode,
    Hispeed(HispeedChange),
    LaneCoverDelta(f32),
    AnalogLaneCoverDelta(f32),
    GreenNumberDelta(i32),
    ToggleLaneCoverVisibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlayLaneTarget {
    Sudden,
    Lift,
    Hidden,
}

impl PlayLaneTarget {
    pub(super) const fn toggled_lift_hidden(self) -> Self {
        match self {
            Self::Hidden => Self::Lift,
            Self::Sudden | Self::Lift => Self::Hidden,
        }
    }
}

pub(super) fn resolved_play_lane_target(
    sudden_enabled: bool,
    lane_cover_visible: bool,
    lift_enabled: bool,
    hidden_enabled: bool,
    preferred: PlayLaneTarget,
) -> Option<PlayLaneTarget> {
    if sudden_enabled && lane_cover_visible {
        return Some(PlayLaneTarget::Sudden);
    }
    match (lift_enabled, hidden_enabled) {
        (true, true) => Some(match preferred {
            PlayLaneTarget::Hidden => PlayLaneTarget::Hidden,
            PlayLaneTarget::Sudden | PlayLaneTarget::Lift => PlayLaneTarget::Lift,
        }),
        (true, false) => Some(PlayLaneTarget::Lift),
        (false, true) => Some(PlayLaneTarget::Hidden),
        (false, false) => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlayOptionControl {
    ToggleHispeedMode,
    Hispeed(HispeedChange),
    LaneCover(LaneCoverChange),
    GreenNumber(GreenNumberChange),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlayAnalogOptionMode {
    LaneCover,
    GreenNumber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LaneCoverChange {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GreenNumberChange {
    Up,
    Down,
}

pub(super) fn keyboard_lane_action(event: &ControlInputEvent) -> Option<PlayLaneAction> {
    if !event.pressed {
        return None;
    }
    let PhysicalControl::KeyboardKey(control) = event.physical.as_ref()? else {
        return None;
    };
    let lane_cover_step = if event.repeat { LANE_COVER_REPEAT_STEP } else { LANE_COVER_STEP };
    match control.as_str() {
        "ArrowLeft" => Some(PlayLaneAction::Hispeed(HispeedChange::Down)),
        "ArrowRight" => Some(PlayLaneAction::Hispeed(HispeedChange::Up)),
        "ArrowUp" => Some(PlayLaneAction::LaneCoverDelta(lane_cover_step)),
        "ArrowDown" => Some(PlayLaneAction::LaneCoverDelta(-lane_cover_step)),
        _ => None,
    }
}

pub(super) fn lane_action_from_option(
    action: PlayOptionControl,
    is_axis: bool,
) -> Option<PlayLaneAction> {
    match action {
        PlayOptionControl::ToggleHispeedMode => Some(PlayLaneAction::ToggleHispeedMode),
        PlayOptionControl::Hispeed(change) => Some(PlayLaneAction::Hispeed(change)),
        PlayOptionControl::LaneCover(_) if is_axis => None,
        PlayOptionControl::LaneCover(LaneCoverChange::Up) => {
            Some(PlayLaneAction::LaneCoverDelta(LANE_COVER_STEP))
        }
        PlayOptionControl::LaneCover(LaneCoverChange::Down) => {
            Some(PlayLaneAction::LaneCoverDelta(-LANE_COVER_STEP))
        }
        PlayOptionControl::GreenNumber(_) if is_axis => None,
        PlayOptionControl::GreenNumber(GreenNumberChange::Up) => {
            Some(PlayLaneAction::GreenNumberDelta(1))
        }
        PlayOptionControl::GreenNumber(GreenNumberChange::Down) => {
            Some(PlayLaneAction::GreenNumberDelta(-1))
        }
    }
}

#[cfg(test)]
mod tests {
    use winit::event::ElementState;
    use winit::keyboard::{KeyCode, PhysicalKey};

    use super::*;

    fn keyboard(code: KeyCode, repeat: bool) -> ControlInputEvent {
        ControlInputEvent::keyboard_parts(PhysicalKey::Code(code), ElementState::Pressed, repeat)
    }

    #[test]
    fn keyboard_arrows_map_to_shared_lane_actions() {
        assert_eq!(
            keyboard_lane_action(&keyboard(KeyCode::ArrowLeft, false)),
            Some(PlayLaneAction::Hispeed(HispeedChange::Down))
        );
        assert_eq!(
            keyboard_lane_action(&keyboard(KeyCode::ArrowUp, false)),
            Some(PlayLaneAction::LaneCoverDelta(LANE_COVER_STEP))
        );
        assert_eq!(
            keyboard_lane_action(&keyboard(KeyCode::ArrowDown, true)),
            Some(PlayLaneAction::LaneCoverDelta(-LANE_COVER_REPEAT_STEP))
        );
    }

    #[test]
    fn axis_button_events_do_not_duplicate_analog_lane_changes() {
        assert_eq!(
            lane_action_from_option(PlayOptionControl::LaneCover(LaneCoverChange::Up), true,),
            None
        );
        assert_eq!(
            lane_action_from_option(PlayOptionControl::GreenNumber(GreenNumberChange::Down), true,),
            None
        );
        assert_eq!(
            lane_action_from_option(PlayOptionControl::Hispeed(HispeedChange::Up), true,),
            Some(PlayLaneAction::Hispeed(HispeedChange::Up))
        );
    }
}
