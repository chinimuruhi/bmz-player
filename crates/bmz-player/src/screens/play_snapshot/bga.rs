use super::*;

pub(super) fn current_poor_bga_frame(
    cache: &PlayRenderSnapshotCache,
    render_now: TimeUs,
    recent_judgements: &[JudgementEvent],
    bga_frames: &BgaFrameCatalog,
    duration_us: i64,
) -> Option<DisplayBgaFrame> {
    if duration_us <= 0 {
        return None;
    }

    let judgement = recent_judgements.iter().rev().find(|event| {
        matches!(event.judge, Judge::Bad | Judge::Poor)
            && render_now.0 >= event.time.0
            && render_now.0 < event.time.0 + duration_us
    })?;
    current_bga_frame(cache, judgement.time, BgaEventKind::Poor, bga_frames)
}

pub(super) fn note_display_duration_ms(
    session: &GameSession,
    now_bpm: f32,
    scroll_multiplier: f32,
) -> i32 {
    let lane_cover = if session.lane_cover_visible {
        crate::config::play::clamp_lane_cover_for_lift(session.lane_cover, session.lift)
    } else {
        0.0
    };
    display_duration_ms_for_bpm_hispeed(
        now_bpm,
        session.hispeed,
        lane_cover,
        session.lift,
        scroll_multiplier,
    )
    .round()
    .clamp(0.0, i32::MAX as f32) as i32
}

pub(crate) fn display_duration_ms_for_bpm_hispeed(
    now_bpm: f32,
    hispeed: f32,
    lane_cover: f32,
    lift: f32,
    scroll_multiplier: f32,
) -> f32 {
    let visible_max = crate::config::play::visible_lane_fraction(lane_cover, lift);
    if scroll_multiplier <= 0.0 {
        return 0.0;
    }
    BEATORAJA_DURATION_BPM_FACTOR_MS / now_bpm.max(1.0) / hispeed.max(0.01) / scroll_multiplier
        * visible_max
}

pub(crate) fn hispeed_for_green_number_values(
    target_green: f32,
    visible_max: f32,
    now_bpm: f64,
    scroll_multiplier: f32,
) -> f32 {
    BEATORAJA_DURATION_BPM_FACTOR_MS * visible_max.clamp(0.0, 1.0) * 0.6
        / (target_green.max(1.0) * now_bpm.max(1.0) as f32 * scroll_multiplier.max(0.01))
}

pub(super) fn current_keybound_bga_frame(
    session: &GameSession,
    cache: &PlayRenderSnapshotCache,
    render_now: TimeUs,
    bga_frames: &BgaFrameCatalog,
) -> Option<DisplayBgaFrame> {
    let asset = bmz_chart::bga_keybound::keybound_bga_asset_at_time(
        &session.chart,
        render_now,
        session.lane_keyon_started_at,
    )?;
    let mut frame = bga_frames.get(&asset).copied()?;
    let tint = bga_tint_at_time(cache, BgaEventKind::Layer, render_now);
    frame.tint_r = tint.r;
    frame.tint_g = tint.g;
    frame.tint_b = tint.b;
    frame.tint_a = tint.a;
    Some(frame)
}

pub(super) fn current_bga_frame(
    cache: &PlayRenderSnapshotCache,
    render_now: TimeUs,
    kind: BgaEventKind,
    bga_frames: &BgaFrameCatalog,
) -> Option<DisplayBgaFrame> {
    let events = cache.bga_events.events(kind);
    let end = events.partition_point(|event| event.time <= render_now);
    let event = events[..end].last()?;
    let asset = event.asset?;
    let mut frame = bga_frames.get(&asset).copied()?;
    let tint = bga_tint_at_time(cache, kind, render_now);
    frame.tint_r = tint.r;
    frame.tint_g = tint.g;
    frame.tint_b = tint.b;
    frame.tint_a = tint.a;
    Some(frame)
}

pub(super) fn bga_tint_at_time(
    cache: &PlayRenderSnapshotCache,
    kind: BgaEventKind,
    render_now: TimeUs,
) -> bmz_chart::bga::BgaTint {
    let opacity = bga_opacity_at_time(cache, kind, render_now);
    let (alpha, red, green, blue) = bga_argb_at_time(cache, kind, render_now);
    bmz_chart::bga::BgaTint {
        r: red as f32 / 255.0,
        g: green as f32 / 255.0,
        b: blue as f32 / 255.0,
        a: (opacity as f32 / 255.0) * (alpha as f32 / 255.0),
    }
}

pub(super) fn bga_opacity_at_time(
    cache: &PlayRenderSnapshotCache,
    kind: BgaEventKind,
    render_now: TimeUs,
) -> u8 {
    let events = cache.bga_events.opacity_events(kind);
    let end = events.partition_point(|event| event.time <= render_now);
    events[..end].last().map_or(0xFF, |event| event.opacity)
}

pub(super) fn bga_argb_at_time(
    cache: &PlayRenderSnapshotCache,
    kind: BgaEventKind,
    render_now: TimeUs,
) -> (u8, u8, u8, u8) {
    let events = cache.bga_events.argb_events(kind);
    let end = events.partition_point(|event| event.time <= render_now);
    events[..end]
        .last()
        .map_or((0xFF, 0xFF, 0xFF, 0xFF), |event| (event.alpha, event.red, event.green, event.blue))
}

pub fn display_bga_frame(id: BgaAssetId, width: u32, height: u32) -> DisplayBgaFrame {
    DisplayBgaFrame::opaque(bga_texture_id(id), width.max(1) as f32, height.max(1) as f32)
}

pub fn display_video_bga_frame(id: BgaAssetId, width: u32, height: u32) -> DisplayBgaFrame {
    DisplayBgaFrame::opaque_video(bga_texture_id(id), width.max(1) as f32, height.max(1) as f32)
}

pub fn bga_texture_id(id: BgaAssetId) -> u32 {
    CHART_BGA_TEXTURE_BASE + id.0
}
