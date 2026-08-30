use super::*;

impl WinitApp {
    pub(super) fn stop_viewer_playback(&mut self) {
        self.reset_viewer_playback();
        self.select.session_mode = SessionMode::Autoplay;
        self.viewer_waiting = true;
    }

    pub(super) fn play_viewer_chart(&mut self, path: &Path, measure: u32) -> Result<()> {
        let path = path
            .canonicalize()
            .with_context(|| format!("failed to resolve viewer chart: {}", path.display()))?;
        if !crate::storage::scan::is_chart_file(&path) {
            bail!("unsupported viewer chart extension: {}", path.display());
        }

        // RANDOM分岐と実プレイで同じ譜面を使うため、importとPlayStartOptionsへ同じseedを渡す。
        // importが失敗した場合は現在の再生を維持し、成功してから差し替える。
        let bms_random_seed = crate::random_option_seed::fresh_bms_random_seed();
        let imported = crate::storage::import::import_chart_file(
            &mut self.boot.library_db,
            &path,
            None,
            Some(bms_random_seed),
            now_unix_seconds(),
        )?;
        let start_time = bootstrap::viewer_measure_start_time(&imported.chart, measure)?;

        tracing::info!(
            chart_id = imported.chart_id,
            path = %path.display(),
            measure,
            start_time_us = start_time.0,
            "replacing external viewer chart"
        );
        self.reset_viewer_playback();
        self.select.session_mode = SessionMode::Autoplay;
        self.viewer_waiting = false;
        if !self.start_boot_chart(
            imported.chart_id,
            true,
            true,
            Some(start_time.0),
            Some(bms_random_seed),
        ) {
            self.viewer_waiting = true;
            bail!("viewer chart is unavailable for the active session mode");
        }
        Ok(())
    }

    fn reset_viewer_playback(&mut self) {
        self.stop_select_preview();
        if let Some(manager) = &self.audio.system_sound {
            manager.stop_all_bgm();
        }
        if let Some(audio) = &self.result.result_skin_audio {
            audio.stop_all();
        }
        self.play.pending_decide = None;
        self.result.finished_play = None;
        self.result.result_exit = None;
        self.abort_pending_play_start();
        self.audio.draining_audio = None;
        self.play.play_media_cache = None;
        self.play.last_play_snapshot = None;
        self.clear_play_meta_image_state();
    }
}
