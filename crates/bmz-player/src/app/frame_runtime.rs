use std::time::{Duration, Instant};

use bmz_render::renderer::RenderFrameTimings;

use crate::i18n::{FluentArgs, Localizer};

/// beatoraja の `Gdx.graphics.getFramesPerSecond()` と同様、1 秒ごとに
/// 確定したフレーム数を skin の NUMBER_CURRENT_FPS (20) と右上表示へ渡す。
pub(super) struct SkinFpsCounter {
    window_started_at: Instant,
    frames: u32,
    current: u32,
}

impl SkinFpsCounter {
    pub(super) fn new(now: Instant) -> Self {
        Self { window_started_at: now, frames: 0, current: 0 }
    }

    pub(super) fn record_frame(&mut self, now: Instant) {
        self.frames = self.frames.saturating_add(1);
        if now.duration_since(self.window_started_at) >= Duration::from_secs(1) {
            self.current = self.frames;
            self.frames = 0;
            self.window_started_at = now;
        }
    }

    pub(super) fn current(&self) -> u32 {
        self.current
    }
}

#[derive(Debug, Default)]
pub(super) struct FramePacer {
    next_frame_at: Option<Instant>,
    fps: Option<u32>,
}

impl FramePacer {
    pub(super) fn delay(&self, now: Instant, fps: u32, skip_wait: bool) -> Duration {
        if skip_wait || fps == 0 || self.fps != Some(fps) {
            return Duration::ZERO;
        }
        self.next_frame_at
            .and_then(|deadline| deadline.checked_duration_since(now))
            .unwrap_or_default()
    }

    pub(super) fn record_frame_started(&mut self, now: Instant, fps: u32, rebase: bool) {
        if fps == 0 {
            self.next_frame_at = None;
            self.fps = None;
            return;
        }

        let budget = frame_budget(fps);
        let fps_changed = self.fps != Some(fps);
        let next_frame_at = if rebase || fps_changed {
            now + budget
        } else if let Some(previous_deadline) = self.next_frame_at {
            let scheduled = previous_deadline + budget;
            if scheduled > now { scheduled } else { now + budget }
        } else {
            now + budget
        };
        self.next_frame_at = Some(next_frame_at);
        self.fps = Some(fps);
    }

    pub(super) fn next_deadline(&self, now: Instant, fps: u32, skip_wait: bool) -> Option<Instant> {
        let delay = self.delay(now, fps, skip_wait);
        if delay.is_zero() { None } else { now.checked_add(delay) }
    }
}

fn frame_budget(fps: u32) -> Duration {
    debug_assert!(fps > 0);
    Duration::from_secs_f64(1.0 / f64::from(fps)).max(Duration::from_nanos(1))
}

pub(super) fn fps_overlay_text(show_fps: bool, current_fps: u32, text: Localizer) -> String {
    if !show_fps || current_fps == 0 {
        return String::new();
    }
    let mut args = FluentArgs::new();
    args.set("fps", i64::from(current_fps));
    text.format("fps-overlay", &args)
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SkinVideoFrameProfile {
    pub(super) poll_us: u128,
    pub(super) upload_us: u128,
    pub(super) opened: u32,
    pub(super) active_sources: u32,
    pub(super) visible_sources: u32,
    pub(super) uploaded_frames: u32,
}

#[derive(Debug, Default)]
pub(super) struct SceneFrameProfiler {
    frames: u32,
    video_us: u128,
    video_poll_us: u128,
    video_upload_us: u128,
    video_opened: u128,
    video_active_sources: u128,
    video_visible_sources: u128,
    video_uploaded_frames: u128,
    snapshot_us: u128,
    render_us: u128,
    plan_us: u128,
    draw_us: u128,
    text_us: u128,
    geometry_us: u128,
    upload_us: u128,
    submit_us: u128,
    surface_us: u128,
    bind_us: u128,
    encode_us: u128,
    queue_us: u128,
    present_us: u128,
    commands: u128,
    steps: u128,
    rect_steps: u128,
    image_steps: u128,
    text_steps: u128,
    rect_instances: u128,
    image_instances: u128,
    text_instances: u128,
    total_redraw_us: u128,
    input_us: u128,
    background_us: u128,
    transition_us: u128,
    egui_us: u128,
    advance_active_play_us: u128,
    post_scene_us: u128,
    total_redraw_samples_us: Vec<u64>,
}

const FRAME_PROFILE_SAMPLE_CAPACITY: usize = 120;

#[derive(Debug, Clone, Copy)]
pub(super) struct PlayLoopFrameTimings {
    pub(super) total_redraw_us: u64,
    pub(super) input_us: u64,
    pub(super) background_us: u64,
    pub(super) transition_us: u64,
    pub(super) egui_us: u64,
    pub(super) advance_active_play_us: u64,
    pub(super) post_scene_us: u64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SceneFrameProfileSample {
    pub(super) kind: FrameProfileKind,
    pub(super) video_us: u128,
    pub(super) video_profile: SkinVideoFrameProfile,
    pub(super) snapshot_us: u128,
    pub(super) render_us: u128,
    pub(super) render_timings: Option<RenderFrameTimings>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrameProfileKind {
    Select,
    Play,
    Result,
}

impl SceneFrameProfiler {
    const LOG_EVERY_FRAMES: u32 = 120;

    pub(super) fn record(
        &mut self,
        profile: FrameProfileKind,
        video_us: u128,
        video_profile: SkinVideoFrameProfile,
        snapshot_us: u128,
        render_us: u128,
        timings: Option<RenderFrameTimings>,
        play_loop: Option<PlayLoopFrameTimings>,
    ) {
        self.frames += 1;
        self.video_us += video_us;
        self.video_poll_us += video_profile.poll_us;
        self.video_upload_us += video_profile.upload_us;
        self.video_opened += video_profile.opened as u128;
        self.video_active_sources += video_profile.active_sources as u128;
        self.video_visible_sources += video_profile.visible_sources as u128;
        self.video_uploaded_frames += video_profile.uploaded_frames as u128;
        self.snapshot_us += snapshot_us;
        self.render_us += render_us;
        if let Some(timings) = timings {
            self.plan_us += timings.plan_us;
            self.draw_us += timings.draw_us;
            self.text_us += timings.text_us;
            self.geometry_us += timings.geometry_us;
            self.upload_us += timings.upload_us;
            self.submit_us += timings.submit_us;
            self.surface_us += timings.surface_us;
            self.bind_us += timings.bind_us;
            self.encode_us += timings.encode_us;
            self.queue_us += timings.queue_us;
            self.present_us += timings.present_us;
            self.commands += timings.commands as u128;
            self.steps += timings.steps as u128;
            self.rect_steps += timings.rect_steps as u128;
            self.image_steps += timings.image_steps as u128;
            self.text_steps += timings.text_steps as u128;
            self.rect_instances += timings.rect_instances as u128;
            self.image_instances += timings.image_instances as u128;
            self.text_instances += timings.text_instances as u128;
        }
        if let Some(timings) = play_loop {
            self.total_redraw_us += u128::from(timings.total_redraw_us);
            self.input_us += u128::from(timings.input_us);
            self.background_us += u128::from(timings.background_us);
            self.transition_us += u128::from(timings.transition_us);
            self.egui_us += u128::from(timings.egui_us);
            self.advance_active_play_us += u128::from(timings.advance_active_play_us);
            self.post_scene_us += u128::from(timings.post_scene_us);
            if self.total_redraw_samples_us.len() < FRAME_PROFILE_SAMPLE_CAPACITY {
                self.total_redraw_samples_us.push(timings.total_redraw_us);
            }
        }
        if self.frames >= Self::LOG_EVERY_FRAMES {
            self.log_and_reset(profile);
        }
    }

    fn log_and_reset(&mut self, profile: FrameProfileKind) {
        let frames = self.frames.max(1) as u128;
        let commands = (self.commands / frames) as u64;
        let steps = (self.steps / frames) as u64;
        let rect_steps = (self.rect_steps / frames) as u64;
        let image_steps = (self.image_steps / frames) as u64;
        let text_steps = (self.text_steps / frames) as u64;
        let rect_instances = (self.rect_instances / frames) as u64;
        let image_instances = (self.image_instances / frames) as u64;
        let text_instances = (self.text_instances / frames) as u64;
        let video_ms = fmt_profile_ms(self.video_us, frames);
        let video_poll_ms = fmt_profile_ms(self.video_poll_us, frames);
        let video_upload_ms = fmt_profile_ms(self.video_upload_us, frames);
        let video_opened = self.video_opened as u64;
        let video_active_sources = (self.video_active_sources / frames) as u64;
        let video_visible_sources = (self.video_visible_sources / frames) as u64;
        let video_uploaded_frames = self.video_uploaded_frames as u64;
        let video_upload_frame_ms =
            fmt_profile_ms(self.video_upload_us, self.video_uploaded_frames.max(1));
        let snapshot_ms = fmt_profile_ms(self.snapshot_us, frames);
        let render_ms = fmt_profile_ms(self.render_us, frames);
        let plan_ms = fmt_profile_ms(self.plan_us, frames);
        let draw_ms = fmt_profile_ms(self.draw_us, frames);
        let text_ms = fmt_profile_ms(self.text_us, frames);
        let geometry_ms = fmt_profile_ms(self.geometry_us, frames);
        let upload_ms = fmt_profile_ms(self.upload_us, frames);
        let submit_ms = fmt_profile_ms(self.submit_us, frames);
        let surface_ms = fmt_profile_ms(self.surface_us, frames);
        let bind_ms = fmt_profile_ms(self.bind_us, frames);
        let encode_ms = fmt_profile_ms(self.encode_us, frames);
        let queue_ms = fmt_profile_ms(self.queue_us, frames);
        let present_ms = fmt_profile_ms(self.present_us, frames);
        let total_redraw_ms = fmt_profile_ms(self.total_redraw_us, frames);
        let input_ms = fmt_profile_ms(self.input_us, frames);
        let background_ms = fmt_profile_ms(self.background_us, frames);
        let transition_ms = fmt_profile_ms(self.transition_us, frames);
        let egui_ms = fmt_profile_ms(self.egui_us, frames);
        let advance_active_play_ms = fmt_profile_ms(self.advance_active_play_us, frames);
        let post_scene_ms = fmt_profile_ms(self.post_scene_us, frames);
        let total_redraw_percentiles = frame_duration_percentiles(&self.total_redraw_samples_us);
        match profile {
            FrameProfileKind::Select => {
                tracing::debug!(
                    target: "bmz_player::select_profile",
                    frames = self.frames,
                    video_ms,
                    video_poll_ms,
                    video_upload_ms,
                    video_upload_frame_ms,
                    video_opened,
                    video_active_sources,
                    video_visible_sources,
                    video_uploaded_frames,
                    snapshot_ms,
                    render_ms,
                    plan_ms,
                    draw_ms,
                    text_ms,
                    geometry_ms,
                    upload_ms,
                    submit_ms,
                    surface_ms,
                    bind_ms,
                    encode_ms,
                    queue_ms,
                    present_ms,
                    commands,
                    steps,
                    rect_steps,
                    image_steps,
                    text_steps,
                    rect_instances,
                    image_instances,
                    text_instances,
                    "select frame profile"
                );
            }
            FrameProfileKind::Play => {
                tracing::debug!(
                    target: "bmz_player::play_profile",
                    frames = self.frames,
                    video_ms,
                    video_poll_ms,
                    video_upload_ms,
                    video_upload_frame_ms,
                    video_opened,
                    video_active_sources,
                    video_visible_sources,
                    video_uploaded_frames,
                    snapshot_ms,
                    render_ms,
                    plan_ms,
                    draw_ms,
                    text_ms,
                    geometry_ms,
                    upload_ms,
                    submit_ms,
                    surface_ms,
                    bind_ms,
                    encode_ms,
                    queue_ms,
                    present_ms,
                    total_redraw_ms,
                    total_redraw_p95_ms = total_redraw_percentiles
                        .map(|value| fmt_profile_us_ms(value.p95_us)),
                    total_redraw_p99_ms = total_redraw_percentiles
                        .map(|value| fmt_profile_us_ms(value.p99_us)),
                    total_redraw_max_ms = total_redraw_percentiles
                        .map(|value| fmt_profile_us_ms(value.max_us)),
                    input_ms,
                    background_ms,
                    transition_ms,
                    egui_ms,
                    advance_active_play_ms,
                    post_scene_ms,
                    commands,
                    steps,
                    rect_steps,
                    image_steps,
                    text_steps,
                    rect_instances,
                    image_instances,
                    text_instances,
                    "play frame profile"
                );
            }
            FrameProfileKind::Result => {
                tracing::debug!(
                    target: "bmz_player::result_profile",
                    frames = self.frames,
                    video_ms,
                    video_poll_ms,
                    video_upload_ms,
                    video_upload_frame_ms,
                    video_opened,
                    video_active_sources,
                    video_visible_sources,
                    video_uploaded_frames,
                    snapshot_ms,
                    render_ms,
                    plan_ms,
                    draw_ms,
                    text_ms,
                    geometry_ms,
                    upload_ms,
                    submit_ms,
                    surface_ms,
                    bind_ms,
                    encode_ms,
                    queue_ms,
                    present_ms,
                    commands,
                    steps,
                    rect_steps,
                    image_steps,
                    text_steps,
                    rect_instances,
                    image_instances,
                    text_instances,
                    "result frame profile"
                );
            }
        }
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrameDurationPercentiles {
    p95_us: u64,
    p99_us: u64,
    max_us: u64,
}

fn fmt_profile_ms(total_us: u128, frames: u128) -> String {
    format!("{:.3}", total_us as f64 / frames as f64 / 1000.0)
}

fn fmt_profile_us_ms(us: u64) -> String {
    format!("{:.3}", us as f64 / 1000.0)
}

fn frame_duration_percentiles(samples: &[u64]) -> Option<FrameDurationPercentiles> {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let len = sorted.len();
    (len > 0).then(|| FrameDurationPercentiles {
        p95_us: sorted[(len * 95).div_ceil(100).saturating_sub(1)],
        p99_us: sorted[(len * 99).div_ceil(100).saturating_sub(1)],
        max_us: *sorted.last().expect("non-empty sample list"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::AppLocale;

    #[test]
    fn frame_duration_percentiles_use_nearest_rank() {
        let samples = [1_000, 2_000, 3_000, 4_000, 5_000, 6_000, 7_000, 8_000, 9_000, 10_000];

        assert_eq!(
            frame_duration_percentiles(&samples),
            Some(FrameDurationPercentiles { p95_us: 10_000, p99_us: 10_000, max_us: 10_000 })
        );
        assert_eq!(frame_duration_percentiles(&[]), None);
    }

    #[test]
    fn skin_fps_updates_only_after_each_one_second_window() {
        let started_at = Instant::now();
        let mut fps = SkinFpsCounter::new(started_at);

        for elapsed_ms in [0, 250, 500, 750] {
            fps.record_frame(started_at + Duration::from_millis(elapsed_ms));
            assert_eq!(fps.current(), 0);
        }

        fps.record_frame(started_at + Duration::from_secs(1));
        assert_eq!(fps.current(), 5);
        assert_eq!(fps_overlay_text(true, fps.current(), Localizer::new(AppLocale::Ja)), "FPS 5");
        fps.record_frame(started_at + Duration::from_millis(1_250));
        assert_eq!(fps.current(), 5);
        fps.record_frame(started_at + Duration::from_secs(2));
        assert_eq!(fps.current(), 2);
    }

    #[test]
    fn frame_pacer_waits_for_every_light_frame_without_alternating() {
        let started_at = Instant::now();
        let budget = frame_budget(120);
        let work = Duration::from_micros(500);
        let mut pacer = FramePacer::default();

        assert_eq!(pacer.delay(started_at, 120, false), Duration::ZERO);
        pacer.record_frame_started(started_at, 120, false);

        let mut previous_frame = started_at;
        for _ in 0..4 {
            let redraw_arrived_at = previous_frame + work;
            let delay = pacer.delay(redraw_arrived_at, 120, false);
            assert_eq!(delay, budget - work);

            let frame_started = redraw_arrived_at + delay;
            assert_eq!(frame_started.duration_since(previous_frame), budget);
            pacer.record_frame_started(frame_started, 120, false);
            previous_frame = frame_started;
        }
    }

    #[test]
    fn frame_pacer_exposes_the_next_deadline_without_blocking() {
        let started_at = Instant::now();
        let budget = frame_budget(120);
        let work = Duration::from_micros(500);
        let mut pacer = FramePacer::default();
        pacer.record_frame_started(started_at, 120, false);

        assert_eq!(pacer.next_deadline(started_at + work, 120, false), Some(started_at + budget));
        assert_eq!(pacer.next_deadline(started_at + budget, 120, false), None);
        assert_eq!(pacer.next_deadline(started_at + work, 120, true), None);
        assert_eq!(pacer.next_deadline(started_at + work, 0, false), None);
    }

    #[test]
    fn frame_pacer_rebases_after_a_missed_deadline() {
        let started_at = Instant::now();
        let budget = frame_budget(120);
        let work = Duration::from_micros(500);
        let mut pacer = FramePacer::default();
        pacer.record_frame_started(started_at, 120, false);

        let late_frame = started_at + budget + budget;
        assert_eq!(pacer.delay(late_frame, 120, false), Duration::ZERO);
        pacer.record_frame_started(late_frame, 120, false);

        let next_redraw = late_frame + work;
        assert_eq!(pacer.delay(next_redraw, 120, false), budget - work);
    }

    #[test]
    fn frame_pacer_rebases_when_fps_changes_or_wait_is_skipped() {
        let started_at = Instant::now();
        let work = Duration::from_micros(500);
        let mut pacer = FramePacer::default();
        pacer.record_frame_started(started_at, 120, false);

        let fps_changed_at = started_at + work;
        assert_eq!(pacer.delay(fps_changed_at, 60, false), Duration::ZERO);
        pacer.record_frame_started(fps_changed_at, 60, false);
        assert_eq!(pacer.delay(fps_changed_at + work, 60, false), frame_budget(60) - work);

        let skipped_at = fps_changed_at + Duration::from_millis(2);
        assert_eq!(pacer.delay(skipped_at, 60, true), Duration::ZERO);
        pacer.record_frame_started(skipped_at, 60, true);
        assert_eq!(pacer.delay(skipped_at + work, 60, false), frame_budget(60) - work);

        pacer.record_frame_started(skipped_at + work, 0, false);
        assert_eq!(pacer.delay(skipped_at + work, 0, false), Duration::ZERO);
        assert_eq!(pacer.delay(skipped_at + work, 120, false), Duration::ZERO);
    }

    #[test]
    fn fps_overlay_text_uses_skin_fps_value() {
        let text = Localizer::new(AppLocale::Ja);
        assert_eq!(fps_overlay_text(true, 237, text), "FPS 237");
        assert_eq!(fps_overlay_text(false, 237, text), "");
        assert_eq!(fps_overlay_text(true, 0, text), "");
    }
}
