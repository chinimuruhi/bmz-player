pub(in crate::app) fn active_lane_cover_for_hispeed(
    session: &bmz_gameplay::session::GameSession,
) -> f32 {
    if session.lane_cover_visible {
        crate::config::play::clamp_lane_cover_for_lift(session.lane_cover, session.lift)
    } else {
        0.0
    }
}

pub(in crate::app) fn current_green_number(
    session: &bmz_gameplay::session::GameSession,
    now: TimeUs,
) -> u32 {
    let total = note_display_duration_ms_for_hispeed(
        session,
        session.hispeed,
        active_lane_cover_for_hispeed(session),
        now,
    );
    green_number_from_display_duration(total)
}

pub(in crate::app) fn adjusted_green_number(current: u32, delta: i32) -> u32 {
    let next = current as i64 + delta as i64;
    next.clamp(TARGET_GREEN_NUMBER_MIN as i64, TARGET_GREEN_NUMBER_MAX as i64) as u32
}

pub(in crate::app) fn green_number_from_display_duration(duration_ms: f32) -> u32 {
    let displayed_duration_ms = duration_ms.round().clamp(0.0, i32::MAX as f32) as i32;
    bmz_render::skin::duration_to_green_number_ms(displayed_duration_ms)
        .clamp(TARGET_GREEN_NUMBER_MIN as i32, TARGET_GREEN_NUMBER_MAX as i32) as u32
}

pub(in crate::app) fn instant_elapsed_us_u64(start: Instant) -> u64 {
    start.elapsed().as_micros().min(u64::MAX as u128) as u64
}

pub(in crate::app) fn instant_duration_us_u64(start: Instant, end: Instant) -> u64 {
    end.saturating_duration_since(start).as_micros().min(u64::MAX as u128) as u64
}

pub(in crate::app) fn duration_us_u64(duration: Duration) -> u64 {
    duration.as_micros().min(u64::MAX as u128) as u64
}

pub(in crate::app) fn count_smoke_play_frame(
    rendered_frames: u32,
    exit_after_frames: u32,
) -> (u32, bool) {
    let frames = rendered_frames.saturating_add(1);
    (frames, frames >= exit_after_frames)
}

pub(in crate::app) fn note_display_duration_ms_for_hispeed(
    session: &bmz_gameplay::session::GameSession,
    hispeed: f32,
    lane_cover: f32,
    now: TimeUs,
) -> f32 {
    let now_bpm = floating_hispeed_target_bpm(session, now);
    let scroll_multiplier = crate::screens::play_snapshot::current_scroll_multiplier(
        &session.chart,
        &session.timing_map,
        now,
    );
    crate::screens::play_snapshot::display_duration_ms_for_bpm_hispeed(
        now_bpm as f32,
        hispeed,
        lane_cover,
        session.lift,
        scroll_multiplier,
    )
}

pub(in crate::app) fn hispeed_for_green_number(
    session: &bmz_gameplay::session::GameSession,
    lane_cover: f32,
    now: TimeUs,
) -> f32 {
    hispeed_for_green_number_at_bpm(
        session,
        lane_cover,
        now,
        floating_hispeed_target_bpm(session, now),
    )
}

pub(in crate::app) fn hispeed_for_green_number_at_bpm(
    session: &bmz_gameplay::session::GameSession,
    lane_cover: f32,
    now: TimeUs,
    target_bpm: f64,
) -> f32 {
    let target_green = session.target_green_number.max(1) as f32;
    let visible_max = crate::config::play::visible_lane_fraction(lane_cover, session.lift);
    let scroll_multiplier = crate::screens::play_snapshot::current_scroll_multiplier(
        &session.chart,
        &session.timing_map,
        now,
    );
    let hispeed = hispeed_for_green_number_values(
        target_green,
        visible_max,
        target_bpm.max(1.0),
        scroll_multiplier,
    );
    clamp_hispeed(hispeed)
}

pub(in crate::app) fn floating_hispeed_target_bpm(
    session: &bmz_gameplay::session::GameSession,
    now: TimeUs,
) -> f64 {
    if session.audio_clock.running && now.0 >= 0 {
        session.timing_map.bpm_at_time(now).max(1.0)
    } else {
        session.hsfix_base_bpm.max(1.0)
    }
}

pub(in crate::app) fn chart_play_has_started(session: &bmz_gameplay::session::GameSession) -> bool {
    session.audio_clock.running && session.audio_clock.now().0 >= 0
}

pub(in crate::app) fn hispeed_for_green_number_values(
    target_green: f32,
    visible_max: f32,
    now_bpm: f64,
    scroll_multiplier: f32,
) -> f32 {
    crate::screens::play_snapshot::hispeed_for_green_number_values(
        target_green,
        visible_max,
        now_bpm,
        scroll_multiplier,
    )
}
use super::*;
