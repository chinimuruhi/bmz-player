pub(in crate::app) fn apply_pending_play_lane_action_to_state(
    lane: &mut PendingPlayLaneState,
    action: PlayLaneAction,
    profile: &ProfileConfig,
    now_bpm: f32,
    speed_locked: bool,
) -> bool {
    match action {
        PlayLaneAction::ToggleHispeedMode => match lane.hispeed_mode {
            HispeedMode::Normal => {
                lane.target_green_number = lane.current_green_number(now_bpm);
                lane.hispeed_mode = HispeedMode::Floating;
            }
            HispeedMode::Floating => {
                lane.hispeed = lane.hispeed.clamp(0.5, 10.0);
                lane.hispeed_mode = HispeedMode::Normal;
            }
        },
        PlayLaneAction::Hispeed(change) => {
            if speed_locked {
                return false;
            }
            let step = hispeed_step_for_profile(profile, lane.hispeed_mode);
            lane.hispeed = adjusted_hispeed(lane.hispeed, change, step);
        }
        PlayLaneAction::LaneCoverDelta(delta) => {
            if lane.lane_cover_visible {
                lane.lane_cover = (lane.lane_cover - delta)
                    .clamp(0.0, crate::config::play::lane_cover_max_for_lift(lane.lift));
                lane.refresh_cover_hispeed(now_bpm, speed_locked);
            } else {
                lane.lift = (lane.lift + delta).clamp(0.0, (1.0 - lane.lane_cover).clamp(0.0, 1.0));
                if lane.hispeed_auto_adjust {
                    lane.refresh_floating_hispeed(now_bpm, speed_locked);
                }
            }
        }
        PlayLaneAction::GreenNumberDelta(delta) => {
            if speed_locked {
                return false;
            }
            let current = match lane.hispeed_mode {
                HispeedMode::Normal => lane.current_green_number(now_bpm),
                HispeedMode::Floating => lane.target_green_number,
            };
            lane.target_green_number = adjusted_green_number(current, delta);
            lane.hispeed_mode = HispeedMode::Floating;
            lane.refresh_floating_hispeed(now_bpm, speed_locked);
        }
        PlayLaneAction::ToggleLaneCoverVisibility => {
            let was_visible = lane.lane_cover_visible;
            lane.lane_cover_visible = !lane.lane_cover_visible;
            if !was_visible && lane.lane_cover_visible {
                lane.refresh_cover_hispeed(now_bpm, speed_locked);
            }
        }
    }
    true
}

pub(in crate::app) fn sync_active_play_visual_offset_to_profile(
    profile: &mut ProfileConfig,
    visual_offset_us: i64,
    auto_adjust_active: bool,
) {
    if !auto_adjust_active || profile.judge.visual_offset_us == visual_offset_us {
        return;
    }
    profile.judge.visual_offset_us = visual_offset_us;
    profile.updated_at = now_unix_seconds();
}

pub(in crate::app) fn apply_hispeed_change_to_session(
    session: &mut bmz_gameplay::session::GameSession,
    change: HispeedChange,
    step: f32,
) {
    session.hispeed = adjusted_hispeed(session.hispeed, change, step);
}

pub(in crate::app) fn apply_play_lane_action_to_session(
    session: &mut bmz_gameplay::session::GameSession,
    action: PlayLaneAction,
    speed_locked: bool,
    hispeed_step: f32,
) -> bool {
    match action {
        PlayLaneAction::ToggleHispeedMode => {
            match session.hispeed_mode {
                HispeedMode::Normal => {
                    let now = session.audio_clock.now();
                    session.target_green_number = current_green_number(session, now);
                    session.hispeed_mode = HispeedMode::Floating;
                }
                HispeedMode::Floating => {
                    session.hispeed = session.hispeed.clamp(0.5, 10.0);
                    session.hispeed_mode = HispeedMode::Normal;
                }
            }
            true
        }
        PlayLaneAction::Hispeed(change) => {
            if speed_locked {
                return false;
            }
            apply_hispeed_change_to_session(session, change, hispeed_step);
            true
        }
        PlayLaneAction::LaneCoverDelta(delta) => {
            apply_lane_cover_step_to_session(session, delta, speed_locked)
        }
        PlayLaneAction::GreenNumberDelta(delta) => {
            apply_green_number_step_to_session(session, delta, speed_locked)
        }
        PlayLaneAction::ToggleLaneCoverVisibility => {
            toggle_lane_cover_visibility(session, speed_locked);
            true
        }
    }
}

#[cfg(test)]
pub(in crate::app) fn apply_play_option_control_to_session(
    session: &mut bmz_gameplay::session::GameSession,
    action: PlayOptionControl,
    speed_locked: bool,
    hispeed_step: f32,
) -> bool {
    apply_play_lane_action_to_session(
        session,
        lane_action_from_option(action, false).expect("button option always maps to a lane action"),
        speed_locked,
        hispeed_step,
    )
}

pub(in crate::app) fn replay_pending_play_lane_actions(
    session: &mut bmz_gameplay::session::GameSession,
    actions: &[PlayLaneAction],
    profile: &ProfileConfig,
    speed_locked: bool,
) {
    for &action in actions {
        let step = hispeed_step_for_profile(profile, session.hispeed_mode);
        let _ = apply_play_lane_action_to_session(session, action, speed_locked, step);
    }
}

pub(in crate::app) fn handoff_pending_play_visual_input(
    session: &mut bmz_gameplay::session::GameSession,
    input: &SharedInputBackend,
    visual_input: &PendingPlayVisualInput,
) {
    let mut input = input.clone();
    let _ = input.drain_events();
    visual_input.clone().apply_to_session(session);
}

pub(in crate::app) fn apply_green_number_step_to_session(
    session: &mut bmz_gameplay::session::GameSession,
    delta: i32,
    speed_locked: bool,
) -> bool {
    if speed_locked {
        return false;
    }
    let current = match session.hispeed_mode {
        HispeedMode::Normal => current_green_number(session, session.audio_clock.now()),
        HispeedMode::Floating => session.target_green_number,
    };
    session.target_green_number = adjusted_green_number(current, delta);
    session.hispeed_mode = HispeedMode::Floating;
    let now = session.audio_clock.now();
    session.hispeed =
        hispeed_for_green_number(session, active_lane_cover_for_hispeed(session), now);
    true
}

pub(in crate::app) fn apply_lane_cover_step_to_session(
    session: &mut bmz_gameplay::session::GameSession,
    delta: f32,
    speed_locked: bool,
) -> bool {
    if session.lane_cover_visible {
        session.lane_cover = (session.lane_cover - delta)
            .clamp(0.0, crate::config::play::lane_cover_max_for_lift(session.lift));
        if session.hispeed_mode == HispeedMode::Floating && !speed_locked {
            let now = session.audio_clock.now();
            session.hispeed = if session.hispeed_auto_adjust {
                hispeed_for_green_number(session, session.lane_cover, now)
            } else {
                hispeed_for_green_number_at_bpm(
                    session,
                    session.lane_cover,
                    now,
                    session.hsfix_base_bpm,
                )
            };
        }
    } else {
        session.lift =
            (session.lift + delta).clamp(0.0, (1.0 - session.lane_cover).clamp(0.0, 1.0));
        if session.hispeed_auto_adjust
            && session.hispeed_mode == HispeedMode::Floating
            && !speed_locked
        {
            let now = session.audio_clock.now();
            session.hispeed = hispeed_for_green_number(session, 0.0, now);
        }
    }
    true
}

pub(in crate::app) fn reset_floating_hispeed_if_enabled(
    session: &mut bmz_gameplay::session::GameSession,
    speed_locked: bool,
) {
    if session.hispeed_mode == HispeedMode::Floating && !speed_locked {
        let now = session.audio_clock.now();
        let lane_cover = active_lane_cover_for_hispeed(session);
        session.hispeed = if session.hispeed_auto_adjust {
            hispeed_for_green_number(session, lane_cover, now)
        } else {
            hispeed_for_green_number_at_bpm(session, lane_cover, now, session.hsfix_base_bpm)
        };
    }
}

/// Start / E1 の連続押し間隔を判定する。2回目なら true を返しタイムスタンプをクリアする。
pub(in crate::app) fn register_play_start_double_press(
    last_press_at: &mut Option<Instant>,
    now: Instant,
) -> bool {
    let is_double = last_press_at
        .is_some_and(|prev| now.duration_since(prev) <= PLAY_START_DOUBLE_PRESS_WINDOW);
    if is_double {
        *last_press_at = None;
        true
    } else {
        *last_press_at = Some(now);
        false
    }
}

pub(in crate::app) fn toggle_lane_cover_visibility(
    session: &mut bmz_gameplay::session::GameSession,
    speed_locked: bool,
) {
    let was_visible = session.lane_cover_visible;
    session.lane_cover_visible = !session.lane_cover_visible;
    if !was_visible && session.lane_cover_visible {
        reset_floating_hispeed_if_enabled(session, speed_locked);
    }
}
use super::*;
