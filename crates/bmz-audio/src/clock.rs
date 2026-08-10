use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use bmz_core::time::TimeUs;

#[derive(Debug, Clone)]
pub struct AudioClock {
    pub sample_rate: u32,
    pub start_output_frame: u64,
    pub chart_zero_time_us: i64,
    pub current_frame: Arc<AtomicU64>,
    pub running: bool,
    started_at: Option<Instant>,
}

impl AudioClock {
    pub fn stopped(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            start_output_frame: 0,
            chart_zero_time_us: 0,
            current_frame: Arc::new(AtomicU64::new(0)),
            running: false,
            started_at: None,
        }
    }

    pub fn with_position(
        sample_rate: u32,
        start_output_frame: u64,
        chart_zero_time_us: i64,
        current_frame: Arc<AtomicU64>,
        running: bool,
    ) -> Self {
        Self {
            sample_rate,
            start_output_frame,
            chart_zero_time_us,
            current_frame,
            running,
            started_at: None,
        }
    }

    /// Starts the chart clock from a monotonic timestamp.
    ///
    /// Output-frame coordinates remain available for audio scheduling, while gameplay time
    /// advances continuously from [`Instant`] instead of stepping once per audio callback.
    pub fn start(&mut self, chart_zero_time: TimeUs) {
        self.start_at(chart_zero_time, Instant::now());
    }

    pub fn start_at(&mut self, chart_zero_time: TimeUs, started_at: Instant) {
        self.chart_zero_time_us = chart_zero_time.0;
        self.start_output_frame = self.current_frame.load(Ordering::Relaxed);
        self.running = true;
        self.started_at = Some(started_at);
    }

    pub fn pause(&mut self) {
        self.running = false;
        self.started_at = None;
    }

    pub fn now(&self) -> TimeUs {
        self.now_at(Instant::now())
    }

    pub fn now_at(&self, frame_at: Instant) -> TimeUs {
        if !self.running {
            return TimeUs(self.chart_zero_time_us);
        }

        if let Some(started_at) = self.started_at {
            let elapsed_us =
                frame_at.saturating_duration_since(started_at).as_micros().min(i64::MAX as u128)
                    as i64;
            return TimeUs(self.chart_zero_time_us.saturating_add(elapsed_us));
        }

        // Tests and clocks restored from an explicit output position do not have a monotonic
        // anchor. Preserve the old hardware-frame calculation for those snapshots.
        let frame = self.current_frame.load(Ordering::Relaxed);
        let delta_frames = frame.saturating_sub(self.start_output_frame);
        let delta_us = frame_to_us(delta_frames, self.sample_rate);
        TimeUs(self.chart_zero_time_us + delta_us)
    }

    pub fn time_to_output_frame(&self, time: TimeUs) -> u64 {
        let delta_us = (time.0 - self.chart_zero_time_us).max(0) as u128;
        let delta_frames = delta_us * self.sample_rate as u128 / 1_000_000u128;
        self.start_output_frame + delta_frames as u64
    }

    /// Returns hardware-paced output time elapsed since a chart timestamp.
    ///
    /// Time before `since`, such as the READY margin before chart time zero, is excluded.
    pub fn elapsed_since(&self, since: TimeUs) -> TimeUs {
        if !self.running {
            return TimeUs(0);
        }

        TimeUs(self.now().0.saturating_sub(since.0).max(0))
    }
}

pub fn frame_to_us(frame: u64, sample_rate: u32) -> i64 {
    ((frame as u128 * 1_000_000u128) / sample_rate as u128) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn stopped_clock_reports_chart_zero_time() {
        let clock = AudioClock::stopped(48_000);

        assert_eq!(clock.now(), TimeUs(0));
        assert_eq!(clock.time_to_output_frame(TimeUs(1_000_000)), 48_000);
        assert_eq!(clock.elapsed_since(TimeUs(0)), TimeUs(0));
    }

    #[test]
    fn elapsed_since_excludes_negative_chart_margin() {
        let current_frame = Arc::new(AtomicU64::new(192_000));
        let clock =
            AudioClock::with_position(48_000, 96_000, -1_000_000, Arc::clone(&current_frame), true);

        assert_eq!(clock.time_to_output_frame(TimeUs(0)), 144_000);
        assert_eq!(clock.elapsed_since(TimeUs(0)), TimeUs(1_000_000));
    }

    #[test]
    fn running_clock_advances_from_frame_instant_between_audio_callbacks() {
        let current_frame = Arc::new(AtomicU64::new(256));
        let started_at = Instant::now();
        let mut clock = AudioClock::with_position(48_000, 0, 0, Arc::clone(&current_frame), false);
        clock.start_at(TimeUs(-1_000_000), started_at);

        assert_eq!(clock.now_at(started_at), TimeUs(-1_000_000));
        assert_eq!(clock.now_at(started_at + Duration::from_micros(4_167)), TimeUs(-995_833));
        assert_eq!(current_frame.load(Ordering::Relaxed), 256);
    }
}
