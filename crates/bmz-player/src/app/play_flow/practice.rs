use super::*;

impl WinitApp {
    pub(super) fn route_practice_gamepad_control(&mut self, button: &str, pressed: bool) -> bool {
        let is_config = self
            .play
            .practice_session
            .as_ref()
            .is_some_and(|practice| practice.phase == PracticePhase::Config);
        if !is_config {
            return false;
        }
        if !pressed {
            return true;
        }

        enum Action {
            Move(bool),
            Adjust(bool),
            Start,
            Leave,
            Ignore,
        }
        let action = if self.select.select_keys.is_enter(button) {
            Action::Start
        } else if self.select.select_keys.is_back(button) {
            Action::Leave
        } else if self.select.select_keys.is_select_previous(button)
            || matches!(button, "DPadUp" | "DPadNorth")
        {
            Action::Move(false)
        } else if self.select.select_keys.is_select_next(button)
            || matches!(button, "DPadDown" | "DPadSouth")
        {
            Action::Move(true)
        } else if self.select.select_keys.is_target_previous(button)
            || matches!(button, "DPadLeft" | "DPadWest")
        {
            Action::Adjust(false)
        } else if self.select.select_keys.is_target_next(button)
            || matches!(button, "DPadRight" | "DPadEast")
        {
            Action::Adjust(true)
        } else {
            Action::Ignore
        };

        match action {
            Action::Move(forward) => {
                if let Some(practice) = &mut self.play.practice_session {
                    crate::screens::practice::move_practice_cursor(
                        &mut practice.cursor,
                        practice.is_double,
                        forward,
                    );
                }
            }
            Action::Adjust(increment) => {
                if let Some(practice) = &mut self.play.practice_session {
                    crate::screens::practice::adjust_practice_selected_field(
                        &mut practice.property,
                        practice.cursor,
                        practice.is_double,
                        increment,
                        practice.max_end_time_ms,
                    );
                }
            }
            Action::Start => self.start_practice_round(),
            Action::Leave => self.leave_practice(),
            Action::Ignore => {}
        }
        true
    }

    pub(super) fn start_chart(&mut self, chart_id: i64) {
        self.select.autoplay_folder = None;
        let mut options = self.play_start_options();
        if !self.prepare_session_mode_or_show_error(chart_id, &mut options) {
            return;
        }
        self.begin_decide_for_chart(chart_id, options);
    }

    pub(super) fn prepare_session_mode_or_show_error(
        &mut self,
        chart_id: i64,
        options: &mut PlayStartOptions,
    ) -> bool {
        if let Err(error) = self.prepare_session_mode_for_chart(chart_id, options) {
            tracing::warn!(
                chart_id,
                session_mode = options.session_mode.as_str(),
                error = %format_error_chain(&error),
                "session mode is unavailable for selected chart"
            );
            let text = Localizer::new(self.boot.profile_config.ui.locale());
            self.show_left_overlay_toast(text.text("toast-session-mode-unavailable"));
            false
        } else {
            true
        }
    }

    pub(super) fn prepare_session_mode_for_chart(
        &self,
        chart_id: i64,
        options: &mut PlayStartOptions,
    ) -> Result<()> {
        if !options.session_mode.is_battle() {
            return Ok(());
        }
        let chart = crate::screens::play_session::load_source_chart_for_chart(
            &self.boot.library_db,
            chart_id,
            None,
        )?;
        if !matches!(chart.metadata.key_mode, KeyMode::K5 | KeyMode::K7) {
            anyhow::bail!("battle session currently supports only 5K and 7K charts");
        }
        options.double_option = DoubleOption::Off;
        if options.session_mode == SessionMode::AutoplayBattle {
            options.autoplay = true;
            return Ok(());
        }

        let score_key = crate::storage::score_db::ScoreKey::with_options(
            chart.identity.file_sha256,
            crate::ln_policy::score_ln_policy_for_chart(
                self.boot.profile_config.play.ln_mode_policy,
                &chart,
            ),
            crate::select_options::DoubleOptionScoreBucket::Off,
            self.boot.profile_config.play.rule_mode,
        );
        let best = self
            .boot
            .score_db
            .best_scores_for_charts(&[score_key])?
            .into_iter()
            .next()
            .context("self-best score is not available")?;
        if best.replay_path.is_empty() {
            anyhow::bail!("self-best score has no full replay");
        }
        let replay_path = self.boot.profile_paths.root_dir.join(&best.replay_path);
        let replay = load_replay_for_chart_policy_and_double_option(
            &replay_path,
            chart.identity.file_sha256,
            best.ln_policy,
            crate::select_options::DoubleOptionScoreBucket::Off,
        )?;
        if replay.uses_legacy_seed_scheme() {
            anyhow::bail!("legacy replay seed scheme is not supported by ghost battle");
        }
        if replay.events.is_empty() {
            anyhow::bail!("self-best score has no full input replay");
        }
        let replay_s_random_scheme = replay.effective_s_random_scheme()?;
        let events = replay
            .events
            .iter()
            .filter_map(|event| {
                second_player_lane(event.lane)
                    .map(|lane| bmz_core::replay::ReplayEvent { lane, ..*event })
            })
            .collect();
        options.autoplay = false;
        options.replay_player = Some(bmz_gameplay::replay::ReplayPlayer { events, next_index: 0 });
        options.arrange_2p = replay.arrange_option();
        options.arrange_seed_2p = replay.arrange_seed;
        options.s_random_scheme_2p = Some(replay_s_random_scheme);
        options.bms_random_seed = None;
        options.bms_random_choices = replay.bms_random_choices;
        Ok(())
    }

    pub(super) fn enter_practice(&mut self, chart_id: i64, cli: PracticeCliOverrides) {
        let defaults = match self.load_practice_defaults_for_chart(chart_id, &cli) {
            Ok(defaults) => defaults,
            Err(error) => {
                tracing::error!(%error, chart_id, "failed to load practice configuration");
                return;
            }
        };
        self.play.practice_session = Some(PracticeSession {
            chart_id,
            chart_title: defaults.title,
            chart_sha256: defaults.sha256,
            property: defaults.property,
            phase: PracticePhase::Config,
            max_end_time_ms: defaults.max_end_time_ms,
            last_graph: defaults.graph,
            graph_start_time_ms: 0,
            is_double: defaults.is_double,
            cursor: 0,
        });
        self.result.finished_play = None;
        self.play.play_ending = None;
        self.result.result_exit = None;
        self.clear_active_course_state();

        let preload_options = PlayStartOptions {
            autoplay: false,
            practice_mode: false,
            arrange: ArrangeOption::Normal,
            ..Default::default()
        };
        self.start_play_preload(chart_id, preload_options.clone());
        self.enter_play_scene(chart_id, preload_options, self.decide_snapshot_for_chart(chart_id));
        tracing::info!(chart_id, "practice configuration screen ready");
    }

    pub(super) fn load_practice_defaults_for_chart(
        &self,
        chart_id: i64,
        cli: &PracticeCliOverrides,
    ) -> Result<PracticeChartDefaults> {
        let Some(path) = self.boot.library_db.primary_chart_file_path(chart_id)? else {
            anyhow::bail!("chart file not found for chart id {chart_id}");
        };
        let import = bmz_chart::import::import_bms_chart(Path::new(&path), None, true)
            .with_context(|| format!("import chart for practice defaults: {path}"))?;
        let property = load_practice_property(
            &self.boot.profile_paths,
            &import.chart.identity.file_sha256,
            &import.chart,
            self.select.gauge_option,
            cli,
        )?;
        let title = if import.chart.metadata.title.is_empty() {
            format!("chart {chart_id}")
        } else {
            import.chart.metadata.title.clone()
        };
        let graph = crate::screens::result_model::ResultGraphCollector::default()
            .snapshot_for_result_parts(&import.chart, &Default::default(), None);
        let max_end_time_ms = crate::screens::practice::default_end_time_ms(&import.chart);
        let is_double = matches!(import.chart.metadata.key_mode, KeyMode::K10 | KeyMode::K14);
        Ok(PracticeChartDefaults {
            property,
            title,
            sha256: import.chart.identity.file_sha256,
            graph: std::sync::Arc::new(graph),
            max_end_time_ms,
            is_double,
        })
    }

    pub(super) fn practice_media_ready(&self) -> bool {
        self.play.practice_session.is_some()
            && self.play.preloaded_play_session.is_some()
            && self.play.pending_play_preload.is_none()
    }

    pub(super) fn leave_practice(&mut self) {
        if let Some(practice) = &self.play.practice_session {
            let _ = save_practice_property(
                &self.boot.profile_paths,
                &practice.chart_sha256,
                &practice.property,
            );
        }
        if !self.commit_active_play_lane_state_to_profile() {
            self.commit_pending_play_lane_state_to_profile();
        }
        self.play.practice_session = None;
        self.select.autoplay_folder = None;
        self.play.practice_chart_zero_time = None;
        self.play.active_play = None;
        self.play.pending_play_start = None;
        self.play.preloaded_play_session = None;
        self.invalidate_play_preload();
        self.play.play_ending = None;
        self.result.finished_play = None;
        self.play.play_ready_sound_started_at = None;
        self.play.play_ready_last_control_hold_at = None;
        self.audio.draining_audio = None;
        self.clear_play_meta_image_state();
        self.play.last_play_snapshot = None;
        self.reload_select_items();
        let now = Instant::now();
        self.select.select_scene_started_at = now;
        self.restart_select_bar_timer_without_scroll(now);
        tracing::info!("left practice mode");
    }

    pub(super) fn start_practice_round(&mut self) {
        if !self.practice_media_ready() {
            tracing::debug!("practice start ignored: media not ready");
            return;
        }
        let (chart_id, property, chart_sha256) = {
            let Some(practice) = &mut self.play.practice_session else {
                return;
            };
            if let Some(preloaded) = &self.play.preloaded_play_session {
                clamp_practice_property(&mut practice.property, &preloaded.preloaded.chart);
                practice.max_end_time_ms =
                    crate::screens::practice::default_end_time_ms(&preloaded.preloaded.chart);
            }
            (practice.chart_id, practice.property.clone(), practice.chart_sha256)
        };
        if let Err(error) =
            save_practice_property(&self.boot.profile_paths, &chart_sha256, &property)
        {
            tracing::warn!(%error, "failed to save practice property");
        }
        self.play.practice_chart_zero_time =
            Some(practice_chart_zero_time(&property, self.play_skin_playstart_offset()));
        if let Some(practice) = &mut self.play.practice_session {
            practice.phase = PracticePhase::Playing;
        }

        let chart_zero = self.play.practice_chart_zero_time.unwrap_or(TimeUs(0));
        let preloaded = match self.play.preloaded_play_session.take() {
            Some(preloaded) => preloaded,
            None => {
                tracing::error!(chart_id, "practice start without preloaded session");
                self.play.practice_chart_zero_time = None;
                if let Some(practice) = &mut self.play.practice_session {
                    practice.phase = PracticePhase::Config;
                }
                return;
            }
        };

        let app_config = self.play_session_app_config();
        let mut session_options = play_session_options_from_start(
            &app_config,
            PlayStartOptions {
                autoplay: false,
                practice_mode: true,
                playback_rate_percent: property.playback_rate_percent,
                gauge_auto_shift: GaugeAutoShiftConfig::Off,
                arrange: property.arrange,
                arrange_2p: property.arrange_2p,
                double_option: if property.dp_flip {
                    DoubleOption::Flip
                } else {
                    DoubleOption::Off
                },
                chart_zero_time: chart_zero,
                ..Default::default()
            },
        );
        session_options.ln_policy_setting = self.boot.profile_config.play.ln_mode_policy;
        let prepared = build_practice_prepared_from_preloaded(
            preloaded.preloaded,
            &self.boot.profile_config,
            &property,
            session_options,
            Box::new(preloaded.input.clone()),
        );
        let prepared_winit = crate::screens::play_start::PreparedInputPlaySession {
            prepared,
            input: preloaded.input,
        };
        match self.open_prepared_winit_play_session(prepared_winit) {
            Ok(active_play) => {
                self.install_active_play(chart_id, active_play);
                self.play.pending_play_start = None;
                tracing::info!(chart_id, "practice round started");
            }
            Err(error) => {
                tracing::error!(%error, chart_id, "failed to open practice play session");
                self.play.practice_chart_zero_time = None;
                if let Some(practice) = &mut self.play.practice_session {
                    practice.phase = PracticePhase::Config;
                }
            }
        }
    }

    pub(super) fn finish_practice_round(&mut self) {
        let (chart_id, chart_sha256, property) = {
            let Some(practice) = &self.play.practice_session else {
                return;
            };
            (practice.chart_id, practice.chart_sha256, practice.property.clone())
        };
        if let Err(error) =
            save_practice_property(&self.boot.profile_paths, &chart_sha256, &property)
        {
            tracing::warn!(%error, "failed to save practice property after round");
        }
        self.commit_active_play_lane_state_to_profile();
        if let Some(started) = self.play.active_play.take() {
            let graph = started.running.result_graph.snapshot_for_session(&started.running.session);
            if let Some(practice) = &mut self.play.practice_session {
                practice.last_graph = std::sync::Arc::new(graph);
                practice.graph_start_time_ms = property.start_time_ms;
            }
            let mut audio = started.running.audio;
            audio.mark_draining();
            self.audio.draining_audio = Some(audio);
        }
        self.clear_play_control_holds();
        self.play.play_ending = None;
        self.result.finished_play = None;
        self.play.play_ready_sound_started_at = None;
        self.play.play_ready_last_control_hold_at = None;
        self.play.practice_chart_zero_time = None;
        if let Some(practice) = &mut self.play.practice_session {
            practice.phase = PracticePhase::Config;
        }

        let mut preload_options = PlayStartOptions {
            autoplay: false,
            practice_mode: false,
            arrange: ArrangeOption::Normal,
            ..Default::default()
        };
        self.invalidate_play_preload();
        self.start_play_preload(chart_id, preload_options.clone());
        let key_mode = self.play_skin_key_mode_for_chart(chart_id, &preload_options);
        let play_config_key_mode = self.key_mode_for_chart(chart_id);
        self.boot.profile_config.activate_play_mode(play_config_key_mode);
        self.select.hs_fix_option =
            hs_fix_option_from_profile(self.boot.profile_config.play.hs_fix);
        preload_options.hs_fix = self.select.hs_fix_option;
        let mut session_options = play_session_options_from_start(
            &self.play_session_app_config(),
            preload_options.clone(),
        );
        session_options.play_config_key_mode = Some(play_config_key_mode);
        let mut snapshot = self
            .play
            .last_play_snapshot
            .clone()
            .unwrap_or_else(|| self.decide_snapshot_for_chart(chart_id));
        crate::screens::play_session::apply_placeholder_session_visuals(
            &mut snapshot,
            &self.boot.profile_config,
            key_mode,
            &session_options,
        );
        let mut pending_play_start = PendingPlayStart::from_snapshot(
            chart_id,
            preload_options,
            &snapshot,
            &self.boot.profile_config,
            key_mode,
            play_config_key_mode,
            session_options.gamepad_slots,
        );
        pending_play_start.lane.lane_cover_changing = self.play_lane_value_changing();
        pending_play_start.lane.apply_to_snapshot(&mut snapshot);
        self.play.play_option_input = Some(PlayOptionInput::new(
            key_mode,
            pending_play_start.visual_input.binding.clone(),
            &self.boot.profile_config.input,
            session_options.gamepad_slots,
        ));
        self.play.last_play_snapshot = Some(snapshot);
        self.play.pending_play_start = Some(pending_play_start);
        tracing::info!(chart_id, "practice round finished; back to configuration");
    }
}
