use super::*;

const VIEWER_SHORT_SEEK_US: i64 = 3_000_000;
const VIEWER_SNAPPED_SEEK_US: i64 = 5_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewerSeek {
    Measures(i32),
    Seconds(i64),
    SnappedSeconds(i64),
    Home,
}

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
        self.load_viewer_chart(&path, measure, bms_random_seed, None)
    }

    fn load_viewer_chart(
        &mut self,
        path: &Path,
        measure: u32,
        bms_random_seed: u64,
        preserved_options: Option<PlayStartOptions>,
    ) -> Result<()> {
        let imported = crate::storage::import::import_chart_file(
            &mut self.boot.library_db,
            path,
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
        let mut options = preserved_options.unwrap_or_else(|| self.play_start_options());
        options.score_save_disabled = true;
        options.bms_random_seed = Some(bms_random_seed);
        if !self.prepare_session_mode_or_show_error(imported.chart_id, &mut options) {
            self.viewer_waiting = true;
            bail!("viewer chart is unavailable for the active session mode");
        }
        self.play.practice_chart_zero_time = Some(start_time);
        self.start_play_preload(imported.chart_id, options.clone());
        self.begin_preloaded_play_scene_with_presentation(
            imported.chart_id,
            options,
            PlayEntryPresentation::ViewerSeek,
        );
        self.viewer_chart_path = Some(path.to_path_buf());
        self.viewer_bms_random_seed = Some(bms_random_seed);
        Ok(())
    }

    pub(super) fn route_viewer_keyboard(&mut self, event: &winit::event::KeyEvent) -> bool {
        if !self.viewer_mode
            || self.viewer_waiting
            || self.play.active_play.is_none()
            || event.state != ElementState::Pressed
        {
            return false;
        }

        let shift = self.viewer_modifier_held(["LShift", "RShift"]);
        let control = self.viewer_modifier_held(["LControl", "RControl"]);
        let seek = viewer_keyboard_seek(event.physical_key, shift, control);
        if let Some(seek) = seek {
            if let Err(error) = self.seek_active_viewer(seek) {
                tracing::warn!(%error, ?seek, "viewer keyboard seek failed");
                self.show_left_overlay_toast(format!("Viewer seek failed: {error:#}"));
            }
            return true;
        }

        if event.repeat {
            return matches!(event.physical_key, PhysicalKey::Code(KeyCode::F5 | KeyCode::Escape));
        }
        match event.physical_key {
            PhysicalKey::Code(KeyCode::F5) => {
                if let Err(error) = self.reload_active_viewer(shift) {
                    tracing::warn!(%error, reroll = shift, "viewer reload failed");
                    self.show_left_overlay_toast(format!("Viewer reload failed: {error:#}"));
                }
                true
            }
            PhysicalKey::Code(KeyCode::Escape) => {
                self.stop_viewer_playback();
                true
            }
            _ => false,
        }
    }

    pub(super) fn route_viewer_mouse_wheel(&mut self, delta: MouseScrollDelta) -> bool {
        if !self.viewer_mode || self.viewer_waiting || self.play.active_play.is_none() {
            return false;
        }
        let Some(direction) = viewer_wheel_direction(delta) else {
            return true;
        };
        let shift = self.viewer_modifier_held(["LShift", "RShift"]);
        let control = self.viewer_modifier_held(["LControl", "RControl"]);
        let seek = if control {
            ViewerSeek::SnappedSeconds(i64::from(direction) * VIEWER_SNAPPED_SEEK_US)
        } else if shift {
            ViewerSeek::Measures(direction * 4)
        } else {
            ViewerSeek::Measures(direction)
        };
        if let Err(error) = self.seek_active_viewer(seek) {
            tracing::warn!(%error, ?seek, "viewer mouse-wheel seek failed");
            self.show_left_overlay_toast(format!("Viewer seek failed: {error:#}"));
        }
        true
    }

    fn viewer_modifier_held<const N: usize>(&self, names: [&str; N]) -> bool {
        names.into_iter().any(|name| self.input.pressed_controls.contains(name))
    }

    fn seek_active_viewer(&mut self, seek: ViewerSeek) -> Result<()> {
        self.commit_active_play_lane_state_to_profile();
        let (
            chart,
            input,
            sample_rate,
            normalization_gain,
            assist_runtime,
            playback_rate_percent,
            current_time,
        ) = {
            let active = self.play.active_play.as_ref().context("viewer play is not active")?;
            (
                Arc::clone(&active.running.session.chart),
                active.input.clone(),
                active.running.audio.engine.output_sample_rate(),
                active.running.session.audio_mix.chart_normalization_gain,
                active.running.session.assist,
                active.running.playback_rate_percent,
                active.running.session.audio_clock.now(),
            )
        };
        let target = viewer_seek_target(&chart, current_time, seek);
        let mut options = self.active_play_retry_options(ResultRetryMode::SameArrange);
        options.playback_rate_percent = playback_rate_percent;
        let mut session_options =
            play_session_options_from_start(&self.play_session_app_config(), options);
        session_options.sample_rate = sample_rate;
        session_options.assist_runtime = assist_runtime;
        session_options.ln_policy_setting = self.boot.profile_config.play.ln_mode_policy;
        session_options.rule_mode = self.boot.profile_config.play.rule_mode;
        let mut session = crate::screens::play_session::build_game_session_with_input_backend(
            chart,
            &self.boot.profile_config,
            session_options,
            Box::new(input),
        );
        session.audio_mix.chart_normalization_gain = normalization_gain;

        let (carryover_count, feedback) = {
            let active = self.play.active_play.as_mut().context("viewer play ended during seek")?;
            active.running.session = session;
            active.running.play_duration_ms = None;
            active.running.pending_audio.clear();
            active.running.pending_keysound_volumes.clear();
            active.running.finished = None;
            active.running.pending_finished = None;
            active.running.finish_error = None;
            active.running.result_graph =
                crate::screens::result_model::ResultGraphCollector::default();
            active.running.failed_video_bga.clear();
            crate::video_bga::prepare_reused_video_decoders(&mut active.running.video_bga_decoders);
            let carryover_count = active.running.start_viewer_seek(target)?;
            let feedback = viewer_seek_feedback(&active.running.session.chart, target);
            (carryover_count, feedback)
        };
        self.play.play_entry_presentation = PlayEntryPresentation::ViewerSeek;
        self.play.play_ready_sound_started_at = Some(Instant::now());
        self.play.play_ending = None;
        self.result.finished_play = None;
        self.viewer_waiting = false;
        tracing::info!(target_time_us = target.0, carryover_count, ?seek, "viewer seek applied");
        self.show_left_overlay_toast(feedback);
        Ok(())
    }

    fn reload_active_viewer(&mut self, reroll: bool) -> Result<()> {
        let path = self.viewer_chart_path.clone().context("viewer chart path is unavailable")?;
        let active = self.play.active_play.as_ref().context("viewer play is not active")?;
        let measure = viewer_measure_at_time(
            &active.running.session.chart,
            active.running.session.audio_clock.now(),
        );
        let seed = if reroll {
            crate::random_option_seed::fresh_bms_random_seed()
        } else {
            self.viewer_bms_random_seed
                .unwrap_or_else(crate::random_option_seed::fresh_bms_random_seed)
        };
        let options =
            (!reroll).then(|| self.active_play_retry_options(ResultRetryMode::SameArrange));
        self.load_viewer_chart(&path, measure, seed, options)?;
        self.show_left_overlay_toast(if reroll {
            format!("Reloaded measure {measure} with a new RANDOM")
        } else {
            format!("Reloaded measure {measure}")
        });
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
        self.viewer_chart_path = None;
        self.viewer_bms_random_seed = None;
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

fn viewer_wheel_direction(delta: MouseScrollDelta) -> Option<i32> {
    let amount = match delta {
        MouseScrollDelta::LineDelta(_, y) => f64::from(y),
        MouseScrollDelta::PixelDelta(position) => position.y,
    };
    (amount != 0.0).then_some(if amount > 0.0 { 1 } else { -1 })
}

fn viewer_keyboard_seek(key: PhysicalKey, shift: bool, control: bool) -> Option<ViewerSeek> {
    match key {
        PhysicalKey::Code(KeyCode::ArrowLeft) => Some(if shift {
            ViewerSeek::Seconds(-VIEWER_SHORT_SEEK_US)
        } else {
            ViewerSeek::Measures(-1)
        }),
        PhysicalKey::Code(KeyCode::ArrowRight) => Some(if shift {
            ViewerSeek::Seconds(VIEWER_SHORT_SEEK_US)
        } else {
            ViewerSeek::Measures(1)
        }),
        PhysicalKey::Code(KeyCode::ArrowUp) => Some(if control {
            ViewerSeek::SnappedSeconds(VIEWER_SNAPPED_SEEK_US)
        } else if shift {
            ViewerSeek::Measures(4)
        } else {
            ViewerSeek::Measures(1)
        }),
        PhysicalKey::Code(KeyCode::ArrowDown) => Some(if control {
            ViewerSeek::SnappedSeconds(-VIEWER_SNAPPED_SEEK_US)
        } else if shift {
            ViewerSeek::Measures(-4)
        } else {
            ViewerSeek::Measures(-1)
        }),
        PhysicalKey::Code(KeyCode::Home) => Some(ViewerSeek::Home),
        _ => None,
    }
}

fn viewer_measure_at_time(chart: &PlayableChart, time: TimeUs) -> u32 {
    chart.bar_lines.iter().take_while(|bar| bar.time <= time).last().map_or(0, |bar| bar.measure)
}

fn viewer_seek_target(chart: &PlayableChart, current: TimeUs, seek: ViewerSeek) -> TimeUs {
    let clamp = |time: TimeUs| TimeUs(time.0.clamp(0, chart.end_time.0.max(0)));
    match seek {
        ViewerSeek::Home => chart.bar_lines.first().map_or(TimeUs(0), |bar| bar.time),
        ViewerSeek::Seconds(delta) => clamp(TimeUs(current.0.saturating_add(delta))),
        ViewerSeek::SnappedSeconds(delta) => {
            let raw = clamp(TimeUs(current.0.saturating_add(delta)));
            chart.bar_lines.iter().take_while(|bar| bar.time <= raw).last().map_or_else(
                || chart.bar_lines.first().map_or(TimeUs(0), |bar| bar.time),
                |bar| bar.time,
            )
        }
        ViewerSeek::Measures(delta) => {
            let bars = &chart.bar_lines;
            if bars.is_empty() {
                return TimeUs(0);
            }
            let current_index = bars.partition_point(|bar| bar.time <= current).saturating_sub(1);
            let target_index = if delta < 0 {
                current_index.saturating_sub(delta.unsigned_abs() as usize)
            } else {
                current_index.saturating_add(delta as usize).min(bars.len() - 1)
            };
            bars[target_index].time
        }
    }
}

fn viewer_seek_feedback(chart: &PlayableChart, target: TimeUs) -> String {
    let measure = viewer_measure_at_time(chart, target);
    format!("Measure {measure}  {:.3}s", target.0 as f64 / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bmz_chart::model::BarLine;
    use bmz_core::time::ChartTick;

    fn chart_with_bars() -> PlayableChart {
        let mut chart = crate::app::tests::app_test_chart();
        chart.bar_lines = (0..=5)
            .map(|measure| BarLine {
                measure,
                tick: ChartTick(u64::from(measure) * 192),
                time: TimeUs(1_000_000 + i64::from(measure) * 2_000_000),
            })
            .collect();
        chart.end_time = TimeUs(12_000_000);
        chart
    }

    #[test]
    fn measure_seek_uses_containing_bar_and_clamps() {
        let chart = chart_with_bars();
        assert_eq!(
            viewer_seek_target(&chart, TimeUs(4_500_000), ViewerSeek::Measures(1)),
            TimeUs(5_000_000)
        );
        assert_eq!(
            viewer_seek_target(&chart, TimeUs(4_500_000), ViewerSeek::Measures(-9)),
            TimeUs(1_000_000)
        );
    }

    #[test]
    fn left_right_bind_measure_and_shift_three_second_seeks() {
        assert_eq!(
            viewer_keyboard_seek(PhysicalKey::Code(KeyCode::ArrowLeft), false, false),
            Some(ViewerSeek::Measures(-1))
        );
        assert_eq!(
            viewer_keyboard_seek(PhysicalKey::Code(KeyCode::ArrowRight), false, false),
            Some(ViewerSeek::Measures(1))
        );
        assert_eq!(
            viewer_keyboard_seek(PhysicalKey::Code(KeyCode::ArrowLeft), true, false),
            Some(ViewerSeek::Seconds(-3_000_000))
        );
        assert_eq!(
            viewer_keyboard_seek(PhysicalKey::Code(KeyCode::ArrowRight), true, false),
            Some(ViewerSeek::Seconds(3_000_000))
        );
    }

    #[test]
    fn second_seek_is_exact_and_snapped_seek_uses_previous_bar() {
        let chart = chart_with_bars();
        assert_eq!(
            viewer_seek_target(&chart, TimeUs(4_500_000), ViewerSeek::Seconds(3_000_000)),
            TimeUs(7_500_000)
        );
        assert_eq!(
            viewer_seek_target(&chart, TimeUs(4_500_000), ViewerSeek::SnappedSeconds(5_000_000)),
            TimeUs(9_000_000)
        );
    }
}
