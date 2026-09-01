use super::*;

impl WinitApp {
    pub(super) fn stop_viewer_playback(&mut self) {
        self.reset_viewer_playback();
        self.select.session_mode = SessionMode::Autoplay;
        self.viewer_waiting = true;
        if let Some(snapshot) = &mut self.play.last_play_snapshot {
            // 通常の Result 遷移用 fade を解除し、最終 Play frame を待機画面にする。
            snapshot.fadeout_elapsed_ms = None;
        }
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
        if !self.start_boot_chart_with_presentation(
            imported.chart_id,
            true,
            true,
            Some(start_time.0),
            Some(bms_random_seed),
            PlayEntryPresentation::ViewerSeek,
        ) {
            self.viewer_waiting = true;
            bail!("viewer chart is unavailable for the active session mode");
        }
        Ok(())
    }

    /// ViewerのPlay/待機画面を明示的に抜け、通常のSelect画面を表示する。
    /// 起動時間短縮のため省略したSelect資源は、この操作が行われた時点で初めて初期化する。
    pub(super) fn leave_viewer_for_select(&mut self, reason: &'static str) -> bool {
        if !self.viewer_mode
            || (!self.viewer_waiting
                && self.play.active_play.is_none()
                && self.play.pending_play_start.is_none()
                && self.play.play_ending.is_none())
        {
            return false;
        }

        tracing::info!(reason, "leaving viewer playback for select");
        if self.play.active_play.is_some() || self.play.pending_play_start.is_some() {
            self.notify_obs_play_ended();
        }
        self.reset_viewer_playback();
        self.viewer_waiting = false;
        self.initialize_viewer_select_runtime();
        self.play.last_play_snapshot = None;
        self.clear_play_meta_image_state();
        self.reload_select_items();
        self.sync_select_holds_from_pressed_controls();
        self.reload_skin_for_scene_entry(SkinKind::Select);
        self.restart_select_scene_timers();
        self.request_redraw();
        true
    }

    /// 最終Play frameで待機中のE1/E2/E3物理holdを退出操作へ同期する。
    pub(super) fn sync_viewer_wait_exit_holds(&mut self) -> bool {
        if !self.viewer_mode || !self.viewer_waiting {
            return false;
        }
        let e1_held = self.input.select_e_action_holds.contains(&InputActionConfig::E1);
        let e2_held = self.input.select_e_action_holds.contains(&InputActionConfig::E2);
        let e3_held = self.input.select_e_action_holds.contains(&InputActionConfig::E3);
        self.play.play_e1_held = e1_held;
        self.play.play_e2_held = e2_held;
        self.play.play_e3_held = e3_held;
        self.update_play_exit_hold_timer();
        if play_exit_chord_pressed(e2_held, e3_held) {
            return self.leave_viewer_for_select("E2+E3 pressed while viewer waiting");
        }
        false
    }

    fn initialize_viewer_select_runtime(&mut self) {
        if self.viewer_select_initialized {
            return;
        }
        self.viewer_select_initialized = true;
        self.skin.skin_catalog = scan_skin_catalog(&self.boot.app_paths);
        self.audio.system_sound_catalog = system_sound_catalog_from_boot(&self.boot);
        self.refresh_player_stats_snapshot();
        self.select.difficulty_tables = match self.boot.library_db.list_difficulty_tables() {
            Ok(tables) => tables,
            Err(error) => {
                tracing::warn!(%error, "failed to list difficulty tables after leaving viewer");
                Vec::new()
            }
        };
        match SelectFolderSummaryRuntime::new(
            self.boot.app_paths.library_db.clone(),
            self.boot.profile_paths.score_db.clone(),
            &self.select.folder_stack,
            self.boot.profile_config.play.ln_mode_policy,
            self.boot.profile_config.play.rule_mode,
        ) {
            Ok(runtime) => self.select.select_folder_summaries = runtime,
            Err(error) => {
                tracing::warn!(%error, "failed to start select folder summary worker after leaving viewer");
            }
        }
        if self.audio.system_audio.is_some()
            && self.audio.system_sound.is_none()
            && self.audio.pending_system_sound.is_none()
        {
            self.start_system_sound_load();
        }
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
    }
}
