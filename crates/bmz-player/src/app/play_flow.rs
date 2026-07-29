use super::*;

impl WinitApp {
    pub(super) fn start_chart(&mut self, chart_id: i64) {
        self.autoplay_folder = None;
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
        // SessionMode の battle 表示は通常の譜面オプション BATTLE と独立させる。
        // スコアキーは 1P の OFF bucket を使い、2P側だけを表示専用にする。
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
        let max_end_time_ms = defaults.property.end_time_ms;
        self.practice_session = Some(PracticeSession {
            chart_id,
            chart_title: defaults.title,
            chart_sha256: defaults.sha256,
            property: defaults.property,
            phase: PracticePhase::Config,
            max_end_time_ms,
        });
        self.finished_play = None;
        self.play_ending = None;
        self.result_exit = None;
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
            self.gauge_option,
            cli,
        )?;
        let title = if import.chart.metadata.title.is_empty() {
            format!("chart {chart_id}")
        } else {
            import.chart.metadata.title.clone()
        };
        Ok(PracticeChartDefaults { property, title, sha256: import.chart.identity.file_sha256 })
    }

    pub(super) fn practice_media_ready(&self) -> bool {
        self.practice_session.is_some()
            && self.preloaded_play_session.is_some()
            && self.pending_play_preload.is_none()
    }

    pub(super) fn leave_practice(&mut self) {
        if let Some(practice) = &self.practice_session {
            let _ = save_practice_property(
                &self.boot.profile_paths,
                &practice.chart_sha256,
                &practice.property,
            );
        }
        if !self.commit_active_play_lane_state_to_profile() {
            self.commit_pending_play_lane_state_to_profile();
        }
        self.practice_session = None;
        self.autoplay_folder = None;
        self.practice_chart_zero_time = None;
        self.active_play = None;
        self.pending_play_start = None;
        self.preloaded_play_session = None;
        self.invalidate_play_preload();
        self.play_ending = None;
        self.finished_play = None;
        self.play_ready_sound_started_at = None;
        self.play_ready_last_control_hold_at = None;
        self.draining_audio = None;
        self.clear_play_meta_image_state();
        self.last_play_snapshot = None;
        self.reload_select_items();
        let now = Instant::now();
        self.select_scene_started_at = now;
        self.restart_select_bar_timer_without_scroll(now);
        tracing::info!("left practice mode");
    }

    pub(super) fn start_practice_round(&mut self) {
        if !self.practice_media_ready() {
            tracing::debug!("practice start ignored: media not ready");
            return;
        }
        let (chart_id, property, chart_sha256) = {
            let Some(practice) = &mut self.practice_session else {
                return;
            };
            if let Some(preloaded) = &self.preloaded_play_session {
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
        self.practice_chart_zero_time =
            Some(practice_chart_zero_time(&property, self.play_skin_playstart_offset()));
        if let Some(practice) = &mut self.practice_session {
            practice.phase = PracticePhase::Playing;
        }

        let chart_zero = self.practice_chart_zero_time.unwrap_or(TimeUs(0));
        let preloaded = match self.preloaded_play_session.take() {
            Some(preloaded) => preloaded,
            None => {
                tracing::error!(chart_id, "practice start without preloaded session");
                self.practice_chart_zero_time = None;
                if let Some(practice) = &mut self.practice_session {
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
                gauge: Some(property.gauge),
                gauge_auto_shift: GaugeAutoShiftConfig::Off,
                arrange: property.arrange,
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
                self.pending_play_start = None;
                tracing::info!(chart_id, "practice round started");
            }
            Err(error) => {
                tracing::error!(%error, chart_id, "failed to open practice play session");
                self.practice_chart_zero_time = None;
                if let Some(practice) = &mut self.practice_session {
                    practice.phase = PracticePhase::Config;
                }
            }
        }
    }

    pub(super) fn finish_practice_round(&mut self) {
        let (chart_id, chart_sha256, property) = {
            let Some(practice) = &self.practice_session else {
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
        if let Some(started) = self.active_play.take() {
            let mut audio = started.running.audio;
            audio.mark_draining();
            self.draining_audio = Some(audio);
        }
        self.clear_play_control_holds();
        self.play_ending = None;
        self.finished_play = None;
        self.play_ready_sound_started_at = None;
        self.play_ready_last_control_hold_at = None;
        self.practice_chart_zero_time = None;
        if let Some(practice) = &mut self.practice_session {
            practice.phase = PracticePhase::Config;
        }

        let preload_options = PlayStartOptions {
            autoplay: false,
            practice_mode: false,
            arrange: ArrangeOption::Normal,
            ..Default::default()
        };
        self.invalidate_play_preload();
        self.start_play_preload(chart_id, preload_options.clone());
        let key_mode = self.play_skin_key_mode_for_chart(chart_id, &preload_options);
        let session_options = play_session_options_from_start(
            &self.play_session_app_config(),
            preload_options.clone(),
        );
        let mut snapshot = self
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
            session_options.gamepad_slots,
        );
        pending_play_start.lane.lane_cover_changing = self.play_lane_value_changing();
        pending_play_start.lane.apply_to_snapshot(&mut snapshot);
        self.play_option_input = Some(PlayOptionInput::new(
            key_mode,
            pending_play_start.visual_input.binding.clone(),
            &self.boot.profile_config.input,
            session_options.gamepad_slots,
        ));
        self.last_play_snapshot = Some(snapshot);
        self.pending_play_start = Some(pending_play_start);
        tracing::info!(chart_id, "practice round finished; back to configuration");
    }

    pub(super) fn begin_decide_for_chart(&mut self, chart_id: i64, options: PlayStartOptions) {
        let snapshot = self.decide_snapshot_for_chart(chart_id);
        self.begin_decide_for_chart_with_snapshot(chart_id, options, snapshot, None);
    }

    pub(super) fn begin_course_decide_for_chart(
        &mut self,
        chart_id: i64,
        options: PlayStartOptions,
        course_title: &str,
    ) {
        let snapshot = self.decide_snapshot_for_chart(chart_id);
        let title_override =
            DecideTitleOverride { title: course_title.to_string(), subtitle: String::new() };
        self.begin_decide_for_chart_with_snapshot(
            chart_id,
            options,
            snapshot,
            Some(title_override),
        );
    }

    pub(super) fn begin_decide_for_chart_with_snapshot(
        &mut self,
        chart_id: i64,
        options: PlayStartOptions,
        mut snapshot: RenderSnapshot,
        title_override: Option<DecideTitleOverride>,
    ) {
        // Pre-import placeholder only: account for course LN overrides and
        // Battle here. The running session replaces this with a count derived
        // from the imported source chart after preload.
        if let Ok(charts) = self.boot.library_db.list_charts_by_ids(&[chart_id])
            && let Some(chart) = charts.first()
        {
            let policy = match options.ln_mode_override {
                Some(bmz_chart::model::LongNoteMode::Ln) => {
                    crate::ln_policy::LnScorePolicy::ForceLn
                }
                Some(bmz_chart::model::LongNoteMode::Cn) => {
                    crate::ln_policy::LnScorePolicy::ForceCn
                }
                Some(bmz_chart::model::LongNoteMode::Hcn) => {
                    crate::ln_policy::LnScorePolicy::ForceHcn
                }
                None => crate::ln_policy::score_ln_policy(
                    self.boot.profile_config.play.ln_mode_policy,
                    chart.ln_profile,
                ),
            };
            let multiplier = match options
                .double_option
                .normalize_for_key_mode(KeyMode::from_str_opt(&chart.mode).unwrap_or_default())
            {
                DoubleOption::Battle | DoubleOption::BattleAutoScratch => 2,
                DoubleOption::Off | DoubleOption::Flip => 1,
            };
            snapshot.total_notes = chart.scored_total_notes(policy).saturating_mul(multiplier);
        }
        self.ensure_skin_ready(SkinKind::Decide);
        // Play 画面へ入ってから stagefile / backbmp の有無が切り替わると、ロード演出中に
        // 代替タイトルから曲画像へ差し替わって見える。Decide 中に先行ロードし、
        // Play の最初の snapshot から同じ runtime image 100 / 101 を使えるようにする。
        self.prepare_play_meta_image_textures(chart_id);
        // Play スキンは裏で decode+upload を進めるが、Decide 入場では待たない。
        // 実際の Play 入場 (`start_chart_with_options`) で `ensure_skin_ready` が保険として残る。
        let play_skin_key_mode = self.play_skin_key_mode_for_chart(chart_id, &options);
        let play_skin_runtime_state = lua_runtime_state_for_play(
            &options,
            self.boot.profile_config.play.auto_play,
            play_skin_key_mode,
            &self.boot.profile_config.display_name,
        );
        self.spawn_play_skin_decode_for(play_skin_key_mode, play_skin_runtime_state);
        self.start_play_preload(chart_id, options.clone());
        let now = Instant::now();
        self.pending_decide = Some(DecideTransition {
            chart_id,
            options,
            started_at: now,
            fadeout_started_at: None,
            cancel: false,
            snapshot,
            title_override,
        });
    }

    pub(super) fn start_play_preload(&mut self, chart_id: i64, options: PlayStartOptions) {
        self.play_preload_generation = self.play_preload_generation.wrapping_add(1);
        let generation = self.play_preload_generation;
        self.preloaded_play_session = None;
        let bga_options = options.clone();
        let (tx, rx) = mpsc::channel();
        let library_db_path = self.boot.app_paths.library_db.clone();
        let app_config = self.play_session_app_config();
        let ln_policy_setting = self.boot.profile_config.play.ln_mode_policy;
        let rule_mode = self.boot.profile_config.play.rule_mode;
        let input = SharedInputBackend::default();
        let preload_input = input.clone();
        let audio_progress = Arc::new(AtomicU32::new(0));
        let worker_audio_progress = Arc::clone(&audio_progress);
        let applied_arrange = Arc::new(OnceLock::new());
        let worker_applied_arrange = Arc::clone(&applied_arrange);
        thread::Builder::new()
            .name(format!("play-preload-{chart_id}"))
            .spawn(move || {
                let result = (|| -> Result<PreloadedInputPlaySession> {
                    let library_db =
                        crate::storage::library_db::LibraryDatabase::open(&library_db_path)?;
                    let mut session_options =
                        crate::screens::play_start::play_session_options_from_start(
                            &app_config,
                            options,
                        );
                    session_options.ln_policy_setting = ln_policy_setting;
                    session_options.rule_mode = rule_mode;
                    let preloaded =
                        crate::screens::play_session::preload_play_session_for_chart_with_callbacks(
                            &library_db,
                            chart_id,
                            session_options.clone(),
                            |arrange| {
                                let _ = worker_applied_arrange.set(arrange.clone());
                            },
                            |loaded, total| {
                                worker_audio_progress.store(
                                    resource_load_progress_units(loaded, total),
                                    Ordering::Relaxed,
                                );
                            },
                        )?;
                    Ok(PreloadedInputPlaySession {
                        chart_id,
                        preloaded,
                        input: preload_input,
                        session_options,
                    })
                })()
                .map_err(|error| format!("{error:#}"));
                let _ = tx.send(PlayPreloadResult { generation, chart_id, result });
            })
            .expect("failed to spawn play preload thread");
        self.pending_play_preload = Some(PendingPlayPreload {
            generation,
            chart_id,
            input,
            audio_progress,
            applied_arrange,
            rx,
        });
        tracing::info!(chart_id, generation, "play preload started");
        self.start_chart_bga_texture_preload(chart_id, bga_options);
    }

    pub(super) fn invalidate_play_preload(&mut self) {
        self.play_preload_generation = self.play_preload_generation.wrapping_add(1);
        self.pending_play_preload = None;
        // 裏で完成して退避していた結果も無効化する (decide キャンセル / 譜面差し替え)。
        self.preloaded_play_session = None;
        self.invalidate_chart_bga_texture_preload();
    }

    /// select_items に持っている `ChartListItem.mode` から KeyMode を引く。
    /// コース行から開始した譜面など select_items に Chart 行が無い場合は DB を参照し、
    /// 未知 / 見つからない場合だけデフォルトの 7K を返す。
    pub(super) fn key_mode_for_chart(&self, chart_id: i64) -> KeyMode {
        if let Some(key_mode) = self
            .select_items
            .iter()
            .find_map(|item| match item {
                SelectItem::Chart(row) => row.chart.as_ref().and_then(|chart| {
                    (chart.chart_id == chart_id).then(|| KeyMode::from_str_opt(&chart.mode))
                }),
                _ => None,
            })
            .flatten()
        {
            return key_mode;
        }
        match self.boot.library_db.list_charts_by_ids(&[chart_id]) {
            Ok(mut charts) => charts
                .pop()
                .and_then(|chart| KeyMode::from_str_opt(&chart.mode))
                .unwrap_or_default(),
            Err(error) => {
                tracing::warn!(chart_id, %error, "failed to load chart key_mode for play skin");
                KeyMode::default()
            }
        }
    }

    pub(super) fn play_skin_key_mode_for_chart(
        &self,
        chart_id: i64,
        options: &PlayStartOptions,
    ) -> KeyMode {
        play_skin_key_mode_for_options(
            self.key_mode_for_chart(chart_id),
            options.double_option,
            options.session_mode,
        )
    }

    pub(super) fn open_prepared_winit_play_session(
        &self,
        prepared: PreparedInputPlaySession,
    ) -> Result<StartedInputPlaySession> {
        let runtime = self.audio_runtime.as_ref().context("audio output is not available")?;
        open_prepared_winit_play_session(&self.boot.score_db, runtime, prepared)
    }

    pub(super) fn play_output_sample_rate(&self) -> u32 {
        self.audio_runtime
            .as_ref()
            .map(AudioRuntime::sample_rate)
            .unwrap_or(self.boot.app_config.audio.sample_rate)
    }

    pub(super) fn play_session_app_config(&self) -> AppConfig {
        let mut app_config = self.boot.app_config.clone();
        app_config.audio.sample_rate = self.play_output_sample_rate();
        app_config.input.gamepad_slot_runtime_device_ids =
            resolve_gamepad_runtime_slots(&app_config.input, self.gamepad.as_deref())
                .map(|id| id.map(|id| id.0));
        app_config
    }

    /// ウィンドウと renderer surface の準備後、初回シーン描画に合わせて共有
    /// cpal ストリームを開く。
    /// 起動ロード中に音声デバイスを start して、デバイス側の初期化音が先に鳴るのを避ける。
    /// scene transition sound の発火前に system audio を用意し、PulseAudio backend で
    /// corked stream の内部
    /// worker だけが動き続ける状態を避ける。
    pub(super) fn ensure_audio_output(&mut self) {
        if self.audio_runtime.is_some() || self.audio_output_open_attempted {
            return;
        }
        self.audio_output_open_attempted = true;

        match AudioRuntime::open(&self.boot.app_config.audio) {
            Ok(runtime) => {
                self.install_system_audio(&runtime, None);
                if let Err(error) = runtime.play() {
                    tracing::warn!(%error, "failed to start shared audio output stream");
                }
                self.audio_runtime = Some(runtime);
                tracing::info!("audio output opened after window initialization");
            }
            Err(error) => {
                tracing::warn!(%error, "failed to open shared audio output; running without audio");
            }
        }
    }

    pub(super) fn log_audio_diagnostics(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.audio_diagnostics_last_log_at) < AUDIO_DIAGNOSTICS_LOG_INTERVAL {
            return;
        }
        self.audio_diagnostics_last_log_at = now;

        if self.audio_runtime.is_none() {
            self.audio_diagnostics_last = None;
            return;
        };
        let snapshot = self.collect_audio_diagnostics();
        let Some(previous) = self.audio_diagnostics_last.replace(snapshot) else {
            return;
        };
        if snapshot.callback_count < previous.callback_count {
            return;
        }

        let callbacks = snapshot.callback_count - previous.callback_count;
        if callbacks == 0 {
            return;
        }
        let rendered_frames = snapshot.rendered_frames.saturating_sub(previous.rendered_frames);
        let stream_errors = snapshot.stream_error_count.saturating_sub(previous.stream_error_count);
        let source_lock_misses =
            snapshot.source_lock_miss_count.saturating_sub(previous.source_lock_miss_count);
        let engine_lock_misses =
            snapshot.engine_lock_miss_count.saturating_sub(previous.engine_lock_miss_count);
        let engine_lock_miss_callbacks = snapshot
            .engine_lock_miss_callback_count
            .saturating_sub(previous.engine_lock_miss_callback_count);
        let system_engine_lock_misses = snapshot
            .system_engine_lock_miss_count
            .saturating_sub(previous.system_engine_lock_miss_count);
        let play_engine_lock_misses = snapshot
            .play_engine_lock_miss_count
            .saturating_sub(previous.play_engine_lock_miss_count);
        let draining_engine_lock_misses = snapshot
            .draining_engine_lock_miss_count
            .saturating_sub(previous.draining_engine_lock_miss_count);
        let other_engine_lock_misses = snapshot
            .other_engine_lock_miss_count
            .saturating_sub(previous.other_engine_lock_miss_count);
        let clipped_samples =
            snapshot.clipped_sample_count.saturating_sub(previous.clipped_sample_count);
        let command_drops =
            snapshot.command_dropped_count.saturating_sub(previous.command_dropped_count);
        let command_drain_lock_misses = snapshot
            .command_drain_lock_miss_count
            .saturating_sub(previous.command_drain_lock_miss_count);
        let command_engine_lock_misses = snapshot
            .command_engine_lock_miss_count
            .saturating_sub(previous.command_engine_lock_miss_count);
        let commands_submitted =
            snapshot.command_submitted_count.saturating_sub(previous.command_submitted_count);
        let commands_drained =
            snapshot.command_drained_count.saturating_sub(previous.command_drained_count);
        let commands_coalesced =
            snapshot.command_coalesced_count.saturating_sub(previous.command_coalesced_count);

        let sample_rate =
            self.audio_runtime.as_ref().map(AudioRuntime::sample_rate).unwrap_or(1).max(1);
        let avg_callback_frames = rendered_frames as f64 / callbacks as f64;
        let callback_budget_ns =
            ((avg_callback_frames / f64::from(sample_rate)) * 1_000_000_000.0).round() as u64;
        let callback_over_budget =
            callback_budget_ns > 0 && snapshot.max_callback_ns > callback_budget_ns;
        let suspected_cause = classify_audio_output_issue(
            stream_errors,
            source_lock_misses,
            engine_lock_misses,
            command_drops,
            command_drain_lock_misses,
            command_engine_lock_misses,
            callback_over_budget,
            clipped_samples,
            self.select_assets.generated_preview_loading(),
        );

        if stream_errors == 0
            && source_lock_misses == 0
            && engine_lock_misses == 0
            && command_drops == 0
            && command_engine_lock_misses == 0
            && clipped_samples == 0
            && !callback_over_budget
        {
            return;
        }

        tracing::warn!(
            callbacks,
            rendered_frames,
            avg_callback_frames,
            sample_rate,
            stream_errors,
            source_lock_misses,
            engine_lock_misses,
            engine_lock_miss_callbacks,
            system_engine_lock_misses,
            play_engine_lock_misses,
            draining_engine_lock_misses,
            other_engine_lock_misses,
            commands_submitted,
            commands_drained,
            commands_coalesced,
            command_drops,
            command_drain_lock_misses,
            command_engine_lock_misses,
            command_queue_max_depth = snapshot.command_queue_max_depth,
            suspected_cause = suspected_cause.as_str(),
            generated_preview_loading = self.select_assets.generated_preview_loading(),
            select_preview_playing = self.select_assets.preview_playing(),
            select_preview_fade = select_preview_fade_name(self.select_assets.preview_fade()),
            select_preview_factor =
                select_preview_fade_factor(self.select_assets.preview_fade(), now),
            clipped_samples,
            peak_abs = snapshot.peak_abs,
            max_callback_us = snapshot.max_callback_ns / 1_000,
            callback_budget_us = callback_budget_ns / 1_000,
            "audio output diagnostics reported possible dropout or clipping",
        );
    }

    pub(super) fn log_input_diagnostics(&mut self) {
        let diagnostics = last_input_collection_diagnostics();
        if diagnostics.sequence == 0 || diagnostics.sequence == self.input_diagnostics_last_sequence
        {
            return;
        }
        self.input_diagnostics_last_sequence = diagnostics.sequence;
        if diagnostics.drained_events == 0 {
            return;
        }

        tracing::debug!(
            target: "bmz_player::input_profile",
            sequence = diagnostics.sequence,
            drained_events = diagnostics.drained_events,
            translated_events = diagnostics.translated_events,
            dropped_events = diagnostics.dropped_events,
            timestamped_events = diagnostics.timestamped_events,
            min_event_age_us = ?diagnostics.min_event_age_us,
            max_event_age_us = ?diagnostics.max_event_age_us,
            max_future_event_us = ?diagnostics.max_future_event_us,
            "play input collection diagnostics"
        );
    }

    pub(super) fn collect_audio_diagnostics(&self) -> AudioOutputDiagnostics {
        let mut snapshot =
            self.audio_runtime.as_ref().map(AudioRuntime::take_diagnostics).unwrap_or_default();
        if let Some(system_audio) = &self.system_audio {
            snapshot.add_command_queue(system_audio.command_diagnostics());
        }
        if let Some(active_play) = &self.active_play {
            snapshot.add_command_queue(active_play.running.audio.command_diagnostics());
        }
        if let Some(draining_audio) = &self.draining_audio {
            snapshot.add_command_queue(draining_audio.command_diagnostics());
        }
        snapshot
    }

    pub(super) fn install_system_audio(
        &mut self,
        runtime: &AudioRuntime,
        system_engine: Option<bmz_audio::command::AudioEngineHandle>,
    ) {
        let system_audio = match system_engine {
            Some(engine) => crate::audio::SystemAudio::reattach(runtime, engine),
            None => crate::audio::SystemAudio::open(runtime),
        };

        if self.system_sound.is_none() {
            self.system_sound = Some(system_sound_manager_from_boot(&self.boot, &system_audio));
        }
        if !self.select_assets.has_preview() {
            self.select_assets.install_preview(SelectChartPreview::new(system_audio.engine()));
        }
        self.system_audio = Some(system_audio);
    }

    /// 設定パネルの「適用」で、現在の `AppConfig` の音声設定を使って共有 cpal
    /// ストリームを開き直す。ASIO は排他なので新ストリームを開く前に旧ストリームを
    /// 完全に閉じる。プレイ中・プレイ開始待ち中はストリーム差し替えが危険なため何もしない。
    pub(super) fn reopen_audio_output(&mut self) {
        if self.active_play.is_some() || self.pending_play_start.is_some() {
            tracing::warn!("ignoring audio apply while a play session is active");
            return;
        }

        // SystemSoundManager / SelectChartPreview と共有しているシステムエンジン
        // Arc を保持し、新ストリームへそのまま載せ替える(samples を再ロードしない)。
        let system_engine = self.system_audio.as_ref().map(crate::audio::SystemAudio::engine);

        // 旧ストリームを参照する全ハンドルを drop し、ASIO デバイスを解放する。
        self.draining_audio = None;
        self.system_audio = None;
        self.audio_runtime = None;

        match AudioRuntime::open(&self.boot.app_config.audio) {
            Ok(runtime) => {
                self.install_system_audio(&runtime, system_engine);
                if let Err(error) = runtime.play() {
                    tracing::warn!(%error, "failed to start shared audio output stream");
                }
                self.audio_runtime = Some(runtime);
                tracing::info!("audio output reopened with current settings");
            }
            Err(error) => {
                tracing::error!(
                    %error,
                    "failed to reopen audio output; audio disabled until restart"
                );
            }
        }
    }

    pub(super) fn decide_snapshot_for_chart(&self, chart_id: i64) -> RenderSnapshot {
        let mut snapshot = RenderSnapshot::default();
        let metadata = chart_snapshot_metadata_for_chart(
            &self.select_items,
            chart_id,
            |chart_id| {
                self.boot
                .library_db
                .list_charts_by_ids(&[chart_id])
                .map_err(|error| {
                    tracing::warn!(%error, chart_id, "failed to load chart metadata for play snapshot");
                    error
                })
                .ok()
                .and_then(|mut charts| charts.pop())
            },
        );
        if let Some((chart, best_ex_score)) = metadata {
            let total_notes =
                chart.scored_total_notes_for_setting(self.boot.profile_config.play.ln_mode_policy);
            apply_chart_metadata_to_snapshot(&mut snapshot, &chart, total_notes, best_ex_score);
        }
        let (primary, secondary, fallback) = self.table_text_context_for_chart(chart_id).as_tuple();
        snapshot.table_text_primary = primary;
        snapshot.table_text_secondary = secondary;
        snapshot.table_text_fallback = fallback;
        snapshot
    }

    pub(super) fn start_chart_with_options(
        &mut self,
        chart_id: i64,
        mut options: PlayStartOptions,
    ) {
        self.last_play_was_autoplay = options.autoplay;
        self.ensure_skin_ready(SkinKind::Decide);
        let play_skin_key_mode = self.play_skin_key_mode_for_chart(chart_id, &options);
        let play_skin_runtime_state = lua_runtime_state_for_play(
            &options,
            self.boot.profile_config.play.auto_play,
            play_skin_key_mode,
            &self.boot.profile_config.display_name,
        );
        self.spawn_play_skin_decode_for(play_skin_key_mode, play_skin_runtime_state);
        self.ensure_skin_ready(SkinKind::Play);
        self.invalidate_play_preload();
        if self.play_media_cache.as_ref().is_some_and(|cache| cache.chart_id != chart_id) {
            self.play_media_cache = None;
        }
        self.play_ending = None;
        self.result_exit = None;
        self.result_key5_held = false;
        self.result_key7_held = false;
        self.play_ready_sound_started_at = None;
        self.play_ready_last_control_hold_at = None;
        self.decide_sound_stopped_for_chart_start = false;
        if options.chart_zero_time == TimeUs(0) {
            options.chart_zero_time = self.play_skin_playstart_offset();
        }
        // 新しいプレイの音声出力を開く前に、前曲の余韻再生を止めて出力を解放する。
        self.draining_audio = None;

        // Decide 演出中に preload worker が完成させていればそれを使う。
        // 譜面/音源は別スレッドでロード済みなので、ここでは音声出力 open 等の軽量処理だけ。
        // バッファが無ければ (course モード / preload 不発時) 従来通り main で同期ロードする。
        let opened = match self.preloaded_play_session.take() {
            Some(preloaded) => {
                tracing::debug!(chart_id, "using buffered play preload");
                let prepared =
                    prepare_winit_play_session_from_preloaded(&self.boot.profile_config, preloaded);
                self.open_prepared_winit_play_session(prepared)
            }
            None => {
                let app_config = self.play_session_app_config();
                prepare_play_session_for_chart_with_winit_input(
                    &self.boot.library_db,
                    &app_config,
                    &self.boot.profile_config,
                    chart_id,
                    options.clone(),
                )
                .and_then(|prepared| self.open_prepared_winit_play_session(prepared))
            }
        };
        match opened {
            Ok(active_play) => {
                self.enter_play_scene(
                    chart_id,
                    options.clone(),
                    self.decide_snapshot_for_chart(chart_id),
                );
                self.install_active_play(chart_id, active_play);
            }
            Err(error) => {
                tracing::error!(chart_id, %error, "failed to start play");
            }
        }
    }

    pub(super) fn play_skin_playstart_offset(&self) -> TimeUs {
        let playstart_ms =
            self.renderer.play_skin_document().map(|document| document.playstart).unwrap_or(0);
        TimeUs(-i64::from(playstart_ms.max(0)) * 1_000)
    }

    pub(super) fn play_skin_ready_delay(&self) -> Duration {
        let ready_delay_ms = self.renderer.play_skin_document().map_or(0, |document| {
            document.loadstart.max(0).saturating_add(document.loadend.max(0))
        });
        skin_duration_ms(ready_delay_ms)
    }

    pub(super) fn clear_play_meta_image_state(&mut self) {
        self.clear_play_stagefile_state();
        self.clear_play_backbmp_state();
    }

    pub(super) fn clear_play_stagefile_state(&mut self) {
        self.play_stagefile_source = None;
        self.play_stagefile_loaded = false;
        self.play_stagefile_size = None;
    }

    pub(super) fn clear_play_backbmp_state(&mut self) {
        self.play_backbmp_source = None;
        self.play_backbmp_loaded = false;
    }

    pub(super) fn prepare_play_meta_image_textures(&mut self, chart_id: i64) {
        let chart = self
            .boot
            .library_db
            .list_charts_by_ids(&[chart_id])
            .ok()
            .and_then(|mut charts| charts.pop());
        let Some(chart) = chart else {
            self.clear_play_meta_image_state();
            return;
        };
        self.sync_play_stagefile_texture(&chart.folder_path, &chart.stage_file);
        self.sync_play_backbmp_texture(&chart.folder_path, &chart.backbmp_file);
    }

    pub(super) fn sync_play_stagefile_texture(&mut self, folder: &str, relative: &str) {
        let stagefile_key = format!("{folder}|{relative}");
        if self.play_stagefile_source.as_deref() == Some(stagefile_key.as_str()) {
            return;
        }
        self.play_stagefile_source = Some(stagefile_key);
        self.play_stagefile_size =
            load_chart_meta_texture(&mut self.renderer, SELECT_STAGE_TEXTURE, folder, relative);
        self.play_stagefile_loaded = self.play_stagefile_size.is_some();
    }

    pub(super) fn sync_play_backbmp_texture(&mut self, folder: &str, relative: &str) {
        let backbmp_key = format!("{folder}|{relative}");
        if self.play_backbmp_source.as_deref() == Some(backbmp_key.as_str()) {
            return;
        }
        self.play_backbmp_source = Some(backbmp_key);
        self.play_backbmp_loaded =
            load_chart_meta_texture(&mut self.renderer, PLAY_BACKBMP_TEXTURE, folder, relative)
                .is_some();
    }

    pub(super) fn enter_play_scene(
        &mut self,
        chart_id: i64,
        options: PlayStartOptions,
        mut snapshot: RenderSnapshot,
    ) {
        // リザルトの非同期 IR state は今回の試行だけを表す。retry 中にも残すと
        // 同じ chart hash の前回スコアを次の Result で表示し得るため、Play へ
        // 入る時点で直ちに手放す（バックグラウンド送信自体は継続する）。
        self.result_ir = None;
        self.play_ending = None;
        self.result_exit = None;
        self.play_ready_sound_started_at = None;
        self.play_ready_last_control_hold_at = None;
        self.decide_sound_stopped_for_chart_start = false;
        self.active_play = None;
        self.clear_play_control_holds();
        // begin_decide_for_chart_with_snapshot で先行ロードした stagefile / backbmp は保持する。
        // boot / retry など Decide を通らない経路でも、この呼び出しで補完する。
        self.prepare_play_meta_image_textures(chart_id);
        self.finished_play = None;
        self.draining_audio = None;
        self.play_scene_started_at = Instant::now();
        snapshot.arrange = options.arrange.as_str().to_string();
        snapshot.arrange_2p = options.arrange_2p.as_str().to_string();
        snapshot.play_elapsed_time = TimeUs(0);
        snapshot.ready_elapsed_time = None;
        snapshot.time = self.play_skin_playstart_offset();
        snapshot.stagefile_background = self.play_stagefile_loaded;
        snapshot.stagefile_image_size = self.play_stagefile_size;
        snapshot.backbmp_background = self.play_backbmp_loaded;
        // preload 完了で install_active_play がフル snapshot に置き換えるまでの間、
        // 初期ゲージや緑数字が空表示にならないようセッション開始時相当の値を埋める。
        let key_mode = self.play_skin_key_mode_for_chart(chart_id, &options);
        let session_options =
            play_session_options_from_start(&self.play_session_app_config(), options.clone());
        crate::screens::play_session::apply_placeholder_session_visuals(
            &mut snapshot,
            &self.boot.profile_config,
            key_mode,
            &session_options,
        );
        // 譜面変換はWAVロードより先に完了する。preload workerが先行公開した
        // 実配置を使い、Play入場直後のロード画面からRANDOM refを表示する。
        if let Some(applied_arrange) = self.play_preload_applied_arrange(chart_id) {
            apply_play_arrange_to_snapshot(&mut snapshot, &applied_arrange);
        }
        let pending_play_start = PendingPlayStart::from_snapshot(
            chart_id,
            options,
            &snapshot,
            &self.boot.profile_config,
            key_mode,
            session_options.gamepad_slots,
        );
        pending_play_start.lane.apply_to_snapshot(&mut snapshot);
        self.play_option_input = Some(PlayOptionInput::new(
            key_mode,
            pending_play_start.visual_input.binding.clone(),
            &self.boot.profile_config.input,
            session_options.gamepad_slots,
        ));
        self.capture_play_table_text_for_chart(chart_id);
        self.apply_course_skin_context(&mut snapshot);
        self.apply_play_table_text(&mut snapshot);
        self.last_play_snapshot = Some(snapshot.clone());
        self.pending_play_start = Some(pending_play_start);
        self.sync_play_control_holds_from_pressed_controls();
        self.last_started_chart_id = Some(chart_id);
    }

    /// FAST/SLOW 表示モード (Auto / ThresholdMs) を snapshot へ適用する。
    /// プレイ snapshot を `last_play_snapshot` に入れる全パスで呼ぶこと。
    pub(super) fn apply_profile_fast_slow_filter(&self, snapshot: &mut RenderSnapshot) {
        apply_fast_slow_display_filter(
            snapshot,
            self.boot.profile_config.judge.fast_slow_display_threshold_ms,
            self.boot.profile_config.judge.fast_slow_display_scope,
        );
    }

    pub(super) fn install_active_play(
        &mut self,
        chart_id: i64,
        mut active_play: StartedInputPlaySession,
    ) {
        self.last_play_was_autoplay = active_play
            .running
            .session
            .autoplay
            .as_ref()
            .is_some_and(|autoplay| autoplay.is_full());
        if let Some(pending) =
            self.pending_play_start.as_ref().filter(|pending| pending.chart_id == chart_id)
        {
            let speed_locked = self.active_course.as_ref().is_some_and(|course| {
                course.definition.constraints.speed
                    == bmz_core::course::CourseSpeedConstraint::NoSpeed
            });
            replay_pending_play_lane_actions(
                &mut active_play.running.session,
                &pending.lane_actions,
                &self.boot.profile_config,
                speed_locked,
            );
            // pending 中の入力は表示状態へ反映済み。共有 backend に残った同じイベントを
            // 再処理すると key-on/off が install 時刻へずれるため、ここで一度だけ破棄し、
            // placeholder の表示状態を実セッションへ引き継ぐ。
            handoff_pending_play_visual_input(
                &mut active_play.running.session,
                &active_play.input,
                &pending.visual_input,
            );
        }
        active_play.running.session.lane_cover_changing = self.play_lane_value_changing();
        let active_bga_assets = &active_play.running.session.chart.bga_assets;
        let preload_matches_active_chart =
            self.bga_preload.matches_chart(chart_id, active_bga_assets);
        if self.bga_preload.chart_id == Some(chart_id) && !preload_matches_active_chart {
            tracing::warn!(
                chart_id,
                preloaded_assets = self.bga_preload.assets.as_ref().map_or(0, Vec::len),
                active_assets = active_bga_assets.len(),
                "discarding BGA preload because its asset manifest does not match the active chart"
            );
        }
        active_play.running.bga_frames = if preload_matches_active_chart {
            self.bga_preload.frames.clone()
        } else {
            self.start_chart_bga_texture_load_for_chart(
                chart_id,
                &active_play.running.session.chart,
            )
        };
        if let Some(cache) = self.play_media_cache.as_mut()
            && cache.chart_id == chart_id
        {
            let mut videos = std::mem::take(&mut cache.video_bga_decoders);
            if !videos.is_empty() {
                crate::video_bga::prepare_reused_video_decoders(&mut videos);
                active_play.running.video_bga_decoders = videos;
                tracing::info!(
                    chart_id,
                    decoders = active_play.running.video_bga_decoders.len(),
                    "installed reused video BGA decoders"
                );
            }
        }
        let chart = &active_play.running.session.chart;
        let folder = chart_asset_folder(chart)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.sync_play_stagefile_texture(&folder, &chart.metadata.stage_file);
        self.sync_play_backbmp_texture(&folder, &chart.metadata.backbmp_file);
        let render_now = self.play_skin_playstart_offset();
        let mut snapshot = build_render_snapshot_with_target_and_bga_frames_cached(
            &active_play.running.session,
            render_now,
            &active_play.running.session.recent_judgements,
            active_play.running.best_ex_score,
            active_play.running.best_ghost.as_deref(),
            active_play.running.target_ex_score,
            &active_play.running.bga_frames,
            &active_play.running.render_snapshot_cache,
        );
        self.apply_profile_fast_slow_filter(&mut snapshot);
        // READY前から実際の配置をスキンへ渡す。arrange名だけでは
        // RANDOM lane ref (450..469) を解決できないため、確定patternも必要。
        apply_play_arrange_to_snapshot(&mut snapshot, &active_play.running.applied_arrange);
        snapshot.target = active_play.running.target.clone();
        snapshot.stagefile_background = self.play_stagefile_loaded;
        snapshot.stagefile_image_size = self.play_stagefile_size;
        snapshot.backbmp_background = self.play_backbmp_loaded;
        let play_elapsed_time = self.play_elapsed_time();
        snapshot.play_elapsed_time = play_elapsed_time;
        snapshot.ready_elapsed_time = self.play_ready_sound_started_at.map(elapsed_since);
        self.apply_course_skin_context(&mut snapshot);
        self.apply_play_table_text(&mut snapshot);
        crate::screens::play_snapshot::refresh_play_skin_visuals_with_input_elapsed(
            &mut snapshot,
            &active_play.running.session,
            play_elapsed_time,
        );
        self.last_play_snapshot = Some(snapshot);
        self.active_play = Some(active_play);
        // preload 経路では Play シーンへの遷移後にここで曲メタデータが確定する。
        // 曲情報なしで送った Presence を実際の譜面情報で置き換える。
        self.publish_discord_presence_for_scene(AppSceneKind::Play);
        self.update_play_exit_hold_timer();
    }

    pub(super) fn start_chart_bga_texture_preload(
        &mut self,
        chart_id: i64,
        options: PlayStartOptions,
    ) {
        let generation = self.bga_preload.begin_unresolved(chart_id);
        let Some(uploader) = self.renderer.gpu_uploader() else {
            tracing::warn!(chart_id, "skipping BGA preload because GPU uploader is unavailable");
            self.bga_preload.status = BgaImageLoadStatus::skipped(generation, chart_id);
            return;
        };

        let library_db_path = self.boot.app_paths.library_db.clone();
        let app_config = self.play_session_app_config();
        thread::Builder::new()
            .name(format!("bga-image-load-{chart_id}"))
            .spawn({
                let (tx, rx) = bounded_gpu_upload_channel(MAX_PENDING_BGA_TEXTURE_UPLOADS);
                self.bga_preload.rx = Some(rx);
                move || {
                    let session_options =
                        crate::screens::play_start::play_session_options_from_start(
                            &app_config,
                            options,
                        );
                    let assets = (|| -> Result<Vec<bmz_chart::model::BgaAssetRef>> {
                        let library_db =
                            crate::storage::library_db::LibraryDatabase::open(&library_db_path)?;
                        crate::screens::play_session::load_chart_bga_assets_for_chart(
                            &library_db,
                            chart_id,
                            &session_options,
                        )
                    })();
                    chart_bga_texture_preload_worker(generation, chart_id, assets, tx, uploader);
                }
            })
            .expect("failed to spawn BGA image load thread");
        tracing::info!(chart_id, generation, "BGA image preload started");
    }

    pub(super) fn invalidate_chart_bga_texture_preload(&mut self) {
        self.bga_preload.invalidate();
    }

    pub(super) fn start_chart_bga_texture_load_for_chart(
        &mut self,
        chart_id: i64,
        chart: &PlayableChart,
    ) -> BgaFrameCatalog {
        let generation = self.bga_preload.begin_chart(chart_id, chart.bga_assets.clone());
        let static_asset_count = chart
            .bga_assets
            .iter()
            .filter(|asset| asset.kind == bmz_chart::model::BgaAssetKind::Static)
            .count();
        if static_asset_count == 0 {
            self.bga_preload.status = BgaImageLoadStatus::ready(generation, chart_id);
            return BgaFrameCatalog::new();
        }
        let Some(uploader) = self.renderer.gpu_uploader() else {
            tracing::warn!("loading BGA images synchronously because GPU uploader is unavailable");
            let frames = load_chart_bga_textures(&mut self.renderer, chart);
            self.bga_preload.completed_assets = self.bga_preload.total_assets;
            self.bga_preload.status = BgaImageLoadStatus::ready(generation, chart_id);
            return frames;
        };

        let assets = chart.bga_assets.clone();
        let (tx, rx) = bounded_gpu_upload_channel(MAX_PENDING_BGA_TEXTURE_UPLOADS);
        thread::Builder::new()
            .name("bga-image-load".to_string())
            .spawn(move || chart_bga_texture_load_worker(generation, assets, tx, uploader))
            .expect("failed to spawn BGA image load thread");
        self.bga_preload.rx = Some(rx);
        tracing::info!(chart_id, generation, "BGA image preload started");
        BgaFrameCatalog::new()
    }

    pub(super) fn poll_chart_bga_texture_load(&mut self) {
        let Some(rx) = self.bga_preload.rx.take() else {
            return;
        };
        let mut keep_rx = true;
        for _ in 0..MAX_BGA_TEXTURE_RESULTS_PER_REDRAW {
            match rx.try_recv() {
                Ok(PendingBgaImageResult::Manifest { generation, assets }) => {
                    if generation != self.bga_preload.generation {
                        continue;
                    }
                    self.bga_preload.total_assets = assets
                        .iter()
                        .filter(|asset| asset.kind == bmz_chart::model::BgaAssetKind::Static)
                        .count()
                        .min(u32::MAX as usize)
                        as u32;
                    self.bga_preload.completed_assets = 0;
                    self.bga_preload.assets = Some(assets);
                }
                Ok(PendingBgaImageResult::Loaded(image)) => {
                    if image.generation != self.bga_preload.generation {
                        continue;
                    }
                    self.bga_preload.completed_assets =
                        self.bga_preload.completed_assets.saturating_add(1);
                    self.renderer.insert_prepared_texture(image.texture_id, image.prepared);
                    self.bga_preload.frames.insert(
                        image.asset_id,
                        display_bga_frame(image.asset_id, image.width, image.height),
                    );
                    if let Some(active_play) = &mut self.active_play {
                        active_play.running.bga_frames.insert(
                            image.asset_id,
                            display_bga_frame(image.asset_id, image.width, image.height),
                        );
                    }
                    tracing::info!(
                        asset_id = image.asset_id.0,
                        texture_id = image.texture_id.0,
                        width = image.width,
                        height = image.height,
                        file_bytes = image.file_bytes,
                        rgba_bytes = image.rgba_bytes,
                        decode_us = image.decode_us,
                        upload_us = image.upload_us,
                        async_load = true,
                        path = %image.path.display(),
                        "loaded BGA image"
                    );
                }
                Ok(PendingBgaImageResult::Failed {
                    generation,
                    asset_id,
                    path,
                    file_bytes,
                    decode_us,
                    error,
                }) => {
                    if generation != self.bga_preload.generation {
                        continue;
                    }
                    self.bga_preload.completed_assets =
                        self.bga_preload.completed_assets.saturating_add(1);
                    tracing::warn!(
                        asset_id = asset_id.0,
                        file_bytes,
                        decode_us,
                        async_load = true,
                        path = %path.display(),
                        error,
                        "skipping unreadable BGA image"
                    );
                }
                Ok(PendingBgaImageResult::PreloadFailed { generation, chart_id, error }) => {
                    if generation != self.bga_preload.generation {
                        continue;
                    }
                    self.bga_preload.status = BgaImageLoadStatus::failed(generation, chart_id);
                    tracing::warn!(chart_id, error, "BGA image preload failed");
                    keep_rx = false;
                    break;
                }
                Ok(PendingBgaImageResult::Finished { generation, stats }) => {
                    if generation == self.bga_preload.generation {
                        self.bga_preload.completed_assets = self.bga_preload.total_assets;
                        if let Some(chart_id) = self.bga_preload.chart_id {
                            self.bga_preload.status =
                                BgaImageLoadStatus::ready(generation, chart_id);
                        }
                        tracing::info!(
                            chart_bga_assets = stats.chart_bga_assets,
                            static_assets = stats.static_assets,
                            skipped_non_static = stats.skipped_non_static,
                            loaded_assets = stats.loaded_assets,
                            failed_assets = stats.failed_assets,
                            total_file_bytes = stats.total_file_bytes,
                            loaded_file_bytes = stats.loaded_file_bytes,
                            rgba_bytes = stats.rgba_bytes,
                            decode_us = stats.decode_us,
                            upload_us = stats.upload_us,
                            total_us = stats.total_us,
                            async_load = true,
                            "chart BGA image load timing"
                        );
                    }
                    keep_rx = false;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if let Some(chart_id) = self.bga_preload.chart_id {
                        self.bga_preload.status =
                            BgaImageLoadStatus::failed(self.bga_preload.generation, chart_id);
                    }
                    keep_rx = false;
                    break;
                }
            }
        }
        if keep_rx {
            self.bga_preload.rx = Some(rx);
        }
    }

    pub(super) fn poll_play_preload(&mut self) {
        // 1) preload worker からの結果を受け取り (Decide 演出中でも受信して退避する)。
        if let Some(pending) = &self.pending_play_preload {
            match pending.rx.try_recv() {
                Ok(result) => {
                    self.pending_play_preload = None;
                    if result.generation != self.play_preload_generation {
                        tracing::debug!(
                            chart_id = result.chart_id,
                            generation = result.generation,
                            current_generation = self.play_preload_generation,
                            "discarding stale play preload result"
                        );
                        if self.pending_play_start.is_some() {
                            tracing::warn!(
                                chart_id = result.chart_id,
                                generation = result.generation,
                                current_generation = self.play_preload_generation,
                                "aborting pending play start after stale preload result"
                            );
                            self.abort_pending_play_start();
                            return;
                        }
                    } else {
                        match result.result {
                            Ok(prepared) => {
                                tracing::info!(
                                    chart_id = result.chart_id,
                                    generation = result.generation,
                                    "play preload ready (buffered)"
                                );
                                self.preloaded_play_session = Some(prepared);
                            }
                            Err(error) => {
                                // preload 全体の失敗は譜面パース不能など再生不能なケースのみ
                                // (個別音源の欠落は load_chart_samples が warning で続行する)。
                                // Play 画面へ入場済みなら選曲へ戻す。course モード等の
                                // start_chart_with_options 経路は同期 fallback で再試行される。
                                tracing::error!(
                                    chart_id = result.chart_id,
                                    error,
                                    "play preload failed"
                                );
                                if self.pending_play_start.is_some() {
                                    self.abort_pending_play_start();
                                    return;
                                }
                            }
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    tracing::warn!(
                        chart_id = pending.chart_id,
                        generation = pending.generation,
                        "play preload worker disconnected"
                    );
                    self.pending_play_preload = None;
                    if self.pending_play_start.is_some() {
                        self.abort_pending_play_start();
                        return;
                    }
                }
            }
        }

        // 2) Play 入場が確定 (pending_play_start) しており、バッファに preload があれば install。
        if self
            .practice_session
            .as_ref()
            .is_some_and(|practice| practice.phase == PracticePhase::Config)
        {
            return;
        }
        let Some(play_start) = self.pending_play_start.as_ref() else {
            return;
        };
        let Some(prepared) = self.preloaded_play_session.take() else {
            return;
        };
        let chart_id = play_start.chart_id;
        let start_options = play_start.options.clone();
        let opened = if preloaded_matches_start(&prepared, chart_id, &start_options) {
            let prepared =
                prepare_winit_play_session_from_preloaded(&self.boot.profile_config, prepared);
            self.open_prepared_winit_play_session(prepared)
        } else {
            tracing::warn!(chart_id, "discarding mismatched play preload");
            let app_config = self.play_session_app_config();
            prepare_play_session_for_chart_with_winit_input(
                &self.boot.library_db,
                &app_config,
                &self.boot.profile_config,
                chart_id,
                start_options,
            )
            .and_then(|prepared| self.open_prepared_winit_play_session(prepared))
        };
        match opened {
            Ok(active_play) => {
                tracing::info!(chart_id, "play preload installed");
                self.install_active_play(chart_id, active_play);
                // スキン宣言のロード演出時間を既に超えていれば、同一フレーム内で
                // READY を開始して op 80→81 切り替えと timer 40 発火を揃える
                // (次フレームの advance_active_play まで待つと 1 フレーム
                // 曲名表示が途切れる)。
                self.maybe_start_ready_phase();
            }
            Err(error) => {
                tracing::error!(chart_id, %error, "failed to open preloaded play audio");
                self.abort_pending_play_start();
            }
        }
    }

    pub(super) fn abort_pending_play_start(&mut self) {
        if !self.commit_active_play_lane_state_to_profile() {
            self.commit_pending_play_lane_state_to_profile();
        }
        self.pending_play_start = None;
        self.active_play = None;
        self.decide_sound_stopped_for_chart_start = true;
        self.clear_play_meta_image_state();
        self.last_play_snapshot = None;
        // An audio-open / audio-start failure bounces the user back to the
        // select screen.  If they were in a course at the time, the course
        // session is no longer valid — otherwise the next chart they pick
        // would be treated as the next entry of a stale course (route
        // through advance_course_after_finish with mismatched chart_id).
        self.clear_active_course_state();
        self.autoplay_folder = None;
        self.play_media_cache = None;
        let now = Instant::now();
        self.select_scene_started_at = now;
        self.restart_select_bar_timer_without_scroll(now);
    }

    /// Clears any active course session and the cached finished-course
    /// summary.  Call from any path that returns to the select screen
    /// without completing the course naturally.
    pub(super) fn clear_active_course_state(&mut self) {
        if self.active_course.is_some() || self.finished_course.is_some() {
            tracing::info!(
                had_active = self.active_course.is_some(),
                had_finished = self.finished_course.is_some(),
                "clearing course session state (abort or cancel)"
            );
        }
        self.active_course = None;
        self.clear_finished_course();
    }

    pub(super) fn play_start_options(&self) -> PlayStartOptions {
        // beatoraja assigns a 24-bit seed even to NORMAL/MIRROR. Generate both
        // sides here so preload, retry, replay and IR all observe one stable pair.
        let option_seeds = crate::random_option_seed::RandomOptionSeeds::fresh(true);
        let random_trainer_seed = self.random_trainer.arrange_seed(option_seeds.p1);
        PlayStartOptions {
            session_mode: self.session_mode,
            autoplay: self.session_mode.primary_autoplay(),
            gauge: Some(self.gauge_option),
            gauge_auto_shift: self.gauge_auto_shift_option,
            bottom_shiftable_gauge: self.bottom_shiftable_gauge_option,
            arrange: self.arrange_option,
            arrange_2p: self.arrange_option_2p,
            double_option: self.double_option,
            hs_fix: self.hs_fix_option,
            target: self.target_option,
            arrange_seed: Some(i64::from(option_seeds.p1.value())),
            arrange_seed_2p: option_seeds.p2.map(|seed| i64::from(seed.value())),
            random_trainer_seed,
            bms_random_seed: Some(crate::random_option_seed::fresh_bms_random_seed()),
            ..Default::default()
        }
    }

    pub(super) fn refresh_play_target_from_source(&mut self) {
        let source = self
            .active_play
            .as_ref()
            .map(|active| {
                (
                    active.running.score_key,
                    active.running.target_option,
                    active.running.best_ex_score,
                )
            })
            .or_else(|| {
                self.preloaded_play_session.as_ref().map(|preloaded| {
                    (preloaded.preloaded.score_key, preloaded.session_options.target, None)
                })
            });
        let Some((score_key, target, local_best_ex_score)) = source else {
            return;
        };
        if !target.uses_ir_ranking() {
            return;
        }

        let context = select_ir_cache_context(
            self.boot.profile_config.play.ln_mode_policy,
            score_key.ln_policy,
            score_key.double_option,
            score_key.rule_mode,
        );
        self.select_ir.update(
            &self.boot.profile_config.ir,
            &self.boot.profile_paths.root_dir,
            &context,
            score_key.ln_policy,
            score_key.double_option,
            score_key.rule_mode,
            Some(score_key.chart_sha256),
        );
        let resolved = self.select_ir.target_ex_score_for(
            &self.boot.profile_config.ir,
            Some(score_key.chart_sha256),
            target,
            local_best_ex_score,
        );
        if let Some(active) = &mut self.active_play
            && active.running.score_key == score_key
            && active.running.target_option == target
        {
            active.running.target_ex_score = resolved;
        }
    }

    /// リプレイファイル (例: `bmz ir replay` でダウンロードした IR リプレイ) を
    /// 直接指定して再生する。譜面はファイル内の chart_sha256 から library を引く。
    pub(super) fn try_start_replay_from_file(&mut self, path: &std::path::Path) -> bool {
        let replay_file = match crate::storage::replay::load_replay(path) {
            Ok(file) => file,
            Err(error) => {
                tracing::warn!(%error, path = %path.display(), "replay file load failed");
                return false;
            }
        };
        let Ok(sha) = crate::storage::common::hex_to_hash::<32>(&replay_file.chart_sha256) else {
            tracing::warn!(sha = %replay_file.chart_sha256, "replay file has invalid chart sha256");
            return false;
        };
        let Some(chart_id) = self.boot.library_db.chart_id_by_sha256(sha).ok().flatten() else {
            tracing::warn!(
                sha = %replay_file.chart_sha256,
                "replay chart is not in the library; load the song first"
            );
            return false;
        };
        let player = bmz_gameplay::replay::ReplayPlayer {
            events: replay_file.events.clone(),
            next_index: 0,
        };
        let options = PlayStartOptions {
            session_mode: SessionMode::Normal,
            autoplay: false,
            practice_mode: false,
            replay_player: Some(player),
            chart_zero_time: TimeUs(0),
            gauge: Some(self.gauge_option),
            gauge_auto_shift: self.gauge_auto_shift_option,
            bottom_shiftable_gauge: self.bottom_shiftable_gauge_option,
            arrange: replay_file.arrange_option(),
            arrange_2p: replay_file.arrange_2p_option(),
            double_option: replay_file.double_option(),
            hs_fix: HsFixOption::Off,
            target: self.target_option,
            arrange_seed: replay_file.arrange_seed,
            arrange_seed_2p: replay_file.arrange_seed_2p,
            random_trainer_seed: None,
            legacy_arrange_seed: replay_file.uses_legacy_seed_scheme(),
            bms_random_seed: None,
            bms_random_choices: replay_file.bms_random_choices.clone(),
            arrange_pattern: replay_file.lane_shuffle_pattern.clone(),
            initial_gauge_value: None,
            initial_gauge_values: None,
            initial_course_combo: None,
            judge_constraint: bmz_core::course::CourseJudgeConstraint::Normal,
            ln_mode_override: None,
            course_gauge_override: None,
            course_gauge_property_override: None,
        };
        self.start_chart_with_options(chart_id, options);
        true
    }

    pub(super) fn start_replay_chart_with_options(
        &mut self,
        chart_id: i64,
        options: PlayStartOptions,
        show_decide: bool,
    ) {
        if show_decide {
            self.begin_decide_for_chart(chart_id, options);
        } else {
            self.start_chart_with_options(chart_id, options);
        }
    }

    pub(super) fn try_start_replay_for_chart(
        &mut self,
        chart_id: i64,
        slot: u8,
        show_decide: bool,
    ) -> bool {
        let chart = match crate::screens::play_session::load_source_chart_for_chart(
            &self.boot.library_db,
            chart_id,
            None,
        ) {
            Ok(chart) => chart,
            Err(error) => {
                tracing::warn!(chart_id, %error, "replay start failed: source chart load failed");
                return false;
            }
        };
        let sha = chart.identity.file_sha256;
        let key_mode = chart.metadata.key_mode;
        let key = crate::storage::score_db::ScoreKey::with_options(
            sha,
            crate::ln_policy::score_ln_policy_for_chart(
                self.boot.profile_config.play.ln_mode_policy,
                &chart,
            ),
            self.double_option.normalize_for_key_mode(key_mode).score_bucket(),
            self.boot.profile_config.play.rule_mode,
        );
        let Some(slot_record) = self.boot.score_db.replay_slot(key, slot).ok().flatten() else {
            tracing::info!(slot, "no replay saved for slot");
            return false;
        };
        let abs_path = self.boot.profile_paths.root_dir.join(&slot_record.replay_path);
        let replay_file = match load_replay_for_chart_policy_and_double_option(
            &abs_path,
            sha,
            slot_record.ln_policy,
            slot_record.double_option,
        ) {
            Ok(file) => file,
            Err(error) => {
                tracing::warn!(%error, path = %abs_path.display(), "replay load failed");
                return false;
            }
        };
        let player = bmz_gameplay::replay::ReplayPlayer {
            events: replay_file.events.clone(),
            next_index: 0,
        };
        let options = PlayStartOptions {
            session_mode: SessionMode::Normal,
            autoplay: false,
            practice_mode: false,
            replay_player: Some(player),
            chart_zero_time: TimeUs(0),
            gauge: Some(self.gauge_option),
            gauge_auto_shift: self.gauge_auto_shift_option,
            bottom_shiftable_gauge: self.bottom_shiftable_gauge_option,
            arrange: replay_file.arrange_option(),
            arrange_2p: replay_file.arrange_2p_option(),
            double_option: replay_file.double_option(),
            hs_fix: HsFixOption::Off,
            target: self.target_option,
            arrange_seed: replay_file.arrange_seed,
            arrange_seed_2p: replay_file.arrange_seed_2p,
            random_trainer_seed: None,
            legacy_arrange_seed: replay_file.uses_legacy_seed_scheme(),
            bms_random_seed: None,
            bms_random_choices: replay_file.bms_random_choices.clone(),
            arrange_pattern: replay_file.lane_shuffle_pattern.clone(),
            initial_gauge_value: None,
            initial_gauge_values: None,
            initial_course_combo: None,
            judge_constraint: bmz_core::course::CourseJudgeConstraint::Normal,
            ln_mode_override: None,
            course_gauge_override: None,
            course_gauge_property_override: None,
        };
        self.start_replay_chart_with_options(chart_id, options, show_decide);
        true
    }

    pub(super) fn start_replay_for_selected(&mut self, slot: u8) -> bool {
        // Prefer the chart path when the cursor is on a chart row.
        if let Some(chart_id) = self.currently_selected_chart_id() {
            return self.try_start_replay_for_chart(chart_id, slot, true);
        }
        // Otherwise, if the cursor is on a course row, try to launch the
        // course replay stored in the requested slot.
        if let Some(course_id) = self.currently_selected_course_id() {
            return self.try_start_course_replay_for_slot(course_id, slot);
        }
        false
    }

    pub(super) fn currently_selected_chart_id(&self) -> Option<i64> {
        match self.select_items.get(self.selected_index)? {
            SelectItem::Chart(row) => row.chart.as_ref().map(|chart| chart.chart_id),
            SelectItem::Folder { .. }
            | SelectItem::Course(_)
            | SelectItem::Executable(_)
            | SelectItem::Config(_)
            | SelectItem::KeyBinding(_)
            | SelectItem::SettingsBack
            | SelectItem::SettingsClose
            | SelectItem::AdvancedSettings => None,
        }
    }

    pub(super) fn currently_selected_course_id(&self) -> Option<i64> {
        match self.select_items.get(self.selected_index)? {
            SelectItem::Course(row) => Some(row.course_id),
            SelectItem::Chart(_)
            | SelectItem::Folder { .. }
            | SelectItem::Executable(_)
            | SelectItem::Config(_)
            | SelectItem::KeyBinding(_)
            | SelectItem::SettingsBack
            | SelectItem::SettingsClose
            | SelectItem::AdvancedSettings => None,
        }
    }

    pub(super) fn try_start_course_replay_for_slot(&mut self, course_id: i64, slot: u8) -> bool {
        let Some(identity) = self.ir_course_identity(course_id) else {
            tracing::warn!(course_id, slot, "course identity unavailable for replay slot");
            return false;
        };
        let rule_mode = self.boot.profile_config.play.rule_mode;
        match self.boot.score_db.course_replay_slot(&identity.course_hash, rule_mode, slot) {
            Ok(Some(record)) => {
                tracing::info!(
                    course_id,
                    course_hash = %identity.course_hash,
                    rule_mode = rule_mode.as_str(),
                    course_score_id = record.course_score_id,
                    slot,
                    "starting course replay from select"
                );
                self.start_course_replay(course_id, record.course_score_id);
                true
            }
            Ok(None) => {
                tracing::info!(
                    course_id,
                    course_hash = %identity.course_hash,
                    rule_mode = rule_mode.as_str(),
                    slot,
                    "no saved course attempt in this replay slot"
                );
                false
            }
            Err(error) => {
                tracing::error!(
                    %error,
                    course_id,
                    course_hash = %identity.course_hash,
                    rule_mode = rule_mode.as_str(),
                    slot,
                    "failed to look up course_replay_slot"
                );
                false
            }
        }
    }

    pub(super) fn retry_last_chart_with_mode(&mut self, mode: ResultRetryMode) {
        let Some(chart_id) = self.last_started_chart_id else {
            tracing::warn!("no previous chart is available to retry");
            return;
        };
        let mut options = match mode {
            ResultRetryMode::SameArrange => self.result_retry_same_arrange_options(),
            ResultRetryMode::DifferentArrange => self.result_retry_different_arrange_options(),
        };
        if !self.prepare_session_mode_or_show_error(chart_id, &mut options) {
            return;
        }
        if options.chart_zero_time == TimeUs(0) {
            options.chart_zero_time = self.play_skin_playstart_offset();
        }

        if let Some(cache) = self.play_media_cache.take()
            && cache.chart_id == chart_id
        {
            match retry_preload_kind(mode, cache.chart.is_some()) {
                RetryPreloadKind::CachedChartWithFreshAudio => {
                    self.start_quick_retry_preload_reloading_audio(
                        chart_id,
                        options.clone(),
                        cache,
                    );
                    self.begin_result_retry_play_scene(chart_id, options);
                    return;
                }
                RetryPreloadKind::ReimportedChartWithFreshAudio => {
                    self.start_play_preload_reusing_bga(chart_id, options.clone(), cache);
                    self.begin_result_retry_play_scene(chart_id, options);
                    return;
                }
            }
        }
        self.start_chart_with_options(chart_id, options);
    }

    pub(super) fn begin_result_retry_play_scene(
        &mut self,
        chart_id: i64,
        options: PlayStartOptions,
    ) {
        self.ensure_skin_ready(SkinKind::Decide);
        let play_skin_key_mode = self.play_skin_key_mode_for_chart(chart_id, &options);
        let play_skin_runtime_state = lua_runtime_state_for_play(
            &options,
            self.boot.profile_config.play.auto_play,
            play_skin_key_mode,
            &self.boot.profile_config.display_name,
        );
        self.spawn_play_skin_decode_for(play_skin_key_mode, play_skin_runtime_state);
        self.ensure_skin_ready(SkinKind::Play);
        self.play_ending = None;
        self.result_exit = None;
        self.result_key5_held = false;
        self.result_key7_held = false;
        self.play_ready_sound_started_at = None;
        self.play_ready_last_control_hold_at = None;
        self.decide_sound_stopped_for_chart_start = false;
        self.draining_audio = None;
        self.enter_play_scene(chart_id, options, self.decide_snapshot_for_chart(chart_id));
        self.poll_play_preload();
    }

    /// Replay the whole course from its first chart, reproducing each chart's
    /// recorded arrange.  Reads the just-finished course result for the course
    /// id and per-entry arranges, then re-enters the course in PLAY mode.
    pub(super) fn retry_course_same_arrange(&mut self) {
        self.retry_course_with_mode(ResultRetryMode::SameArrange);
    }

    pub(super) fn retry_course_different_arrange(&mut self) {
        self.retry_course_with_mode(ResultRetryMode::DifferentArrange);
    }

    pub(super) fn retry_course_with_mode(&mut self, mode: ResultRetryMode) {
        let Some(course) = self.finished_course.as_ref() else {
            tracing::warn!("no finished course is available to retry");
            return;
        };
        let course_id = course.course_id;
        let arrange_overrides = match mode {
            ResultRetryMode::SameArrange => course.entry_arranges.clone(),
            ResultRetryMode::DifferentArrange => Vec::new(),
        };
        tracing::info!(course_id, entries = arrange_overrides.len(), ?mode, "retrying course");
        // Drop the finished-course/result state before re-entering the course;
        // start_course_with_arrange installs a fresh active_course session.
        self.clear_finished_course();
        self.finished_play = None;
        self.result_exit = None;
        self.result_key5_held = false;
        self.result_key7_held = false;
        self.start_course_with_arrange(course_id, arrange_overrides, false);
    }

    pub(super) fn result_retry_same_arrange_options(&self) -> PlayStartOptions {
        let mut options = self.play_start_options();
        if let Some(applied) = self.finished_play.as_ref().map(|finished| &finished.applied_arrange)
        {
            options.arrange = applied.arrange;
            options.arrange_seed = applied.seed;
            options.arrange_seed_2p = applied.seed_2p;
            options.legacy_arrange_seed = applied.legacy_seed;
            options.bms_random_choices = Some(applied.bms_random_choices.clone());
            options.arrange_pattern = applied.pattern.clone();
        }
        options
    }

    pub(super) fn result_retry_different_arrange_options(&self) -> PlayStartOptions {
        let mut options = self.play_start_options();
        if let Some(applied) = self.finished_play.as_ref().map(|finished| &finished.applied_arrange)
        {
            options.arrange = applied.arrange;
            options.arrange_pattern = None;
        }
        options
    }

    pub(super) fn active_play_retry_options(&self, mode: ResultRetryMode) -> PlayStartOptions {
        let mut options = self.play_start_options();
        if let Some(active) = &self.active_play {
            let applied = &active.running.applied_arrange;
            options.arrange = applied.arrange;
            options.arrange_2p = applied.arrange_2p;
            options.double_option = applied.double_option;
            match mode {
                ResultRetryMode::SameArrange => {
                    options.arrange_seed = applied.seed;
                    options.arrange_seed_2p = applied.seed_2p;
                    options.legacy_arrange_seed = applied.legacy_seed;
                    options.bms_random_choices = Some(applied.bms_random_choices.clone());
                    options.arrange_pattern = applied.pattern.clone();
                }
                ResultRetryMode::DifferentArrange => {
                    options.arrange_pattern = None;
                }
            }
        }
        options
    }

    pub(super) fn take_play_media_cache_from_active(
        &mut self,
        chart_id: i64,
        mode: ResultRetryMode,
    ) -> Option<PlayMediaCache> {
        let active = self.active_play.as_mut()?;
        let video_bga_decoders = std::mem::take(&mut active.running.video_bga_decoders);
        let (chart, applied_arrange, score_key) = match mode {
            ResultRetryMode::SameArrange => (
                Some(Arc::clone(&active.running.session.chart)),
                Some(active.running.applied_arrange.clone()),
                Some(active.running.score_key),
            ),
            ResultRetryMode::DifferentArrange => (None, None, None),
        };
        Some(PlayMediaCache {
            chart_id,
            chart,
            chart_normalization_gain: active.running.session.audio_mix.chart_normalization_gain,
            applied_arrange,
            score_key,
            bga_frames: active.running.bga_frames.clone(),
            bga_assets: active.running.session.chart.bga_assets.clone(),
            video_bga_decoders,
        })
    }

    pub(super) fn capture_play_media_cache_from_running(
        &mut self,
        chart_id: i64,
        running: &mut crate::audio::RunningPlaySession,
    ) {
        let video_bga_decoders = std::mem::take(&mut running.video_bga_decoders);
        self.play_media_cache = Some(PlayMediaCache {
            chart_id,
            chart: Some(Arc::clone(&running.session.chart)),
            chart_normalization_gain: running.session.audio_mix.chart_normalization_gain,
            applied_arrange: Some(running.applied_arrange.clone()),
            score_key: Some(running.score_key),
            bga_frames: running.bga_frames.clone(),
            bga_assets: running.session.chart.bga_assets.clone(),
            video_bga_decoders,
        });
    }

    pub(super) fn apply_reused_bga_preload(
        &mut self,
        chart_id: i64,
        bga_frames: BgaFrameCatalog,
        bga_assets: Vec<BgaAssetRef>,
    ) {
        let bga_frame_count = bga_frames.len();
        let bga_generation = self.bga_preload.apply_reused(chart_id, bga_frames, bga_assets);
        tracing::info!(
            chart_id,
            bga_generation,
            bga_frames = bga_frame_count,
            "reused static BGA preload"
        );
    }

    pub(super) fn start_play_preload_reusing_bga(
        &mut self,
        chart_id: i64,
        options: PlayStartOptions,
        mut cache: PlayMediaCache,
    ) {
        cache.chart = None;
        cache.applied_arrange = None;
        cache.score_key = None;
        self.play_preload_generation = self.play_preload_generation.wrapping_add(1);
        let generation = self.play_preload_generation;
        self.preloaded_play_session = None;
        self.apply_reused_bga_preload(chart_id, cache.bga_frames.clone(), cache.bga_assets.clone());
        self.play_media_cache = Some(cache);

        let (tx, rx) = mpsc::channel();
        let library_db_path = self.boot.app_paths.library_db.clone();
        let app_config = self.play_session_app_config();
        let ln_policy_setting = self.boot.profile_config.play.ln_mode_policy;
        let rule_mode = self.boot.profile_config.play.rule_mode;
        let input = SharedInputBackend::default();
        let preload_input = input.clone();
        let audio_progress = Arc::new(AtomicU32::new(0));
        let worker_audio_progress = Arc::clone(&audio_progress);
        let applied_arrange = Arc::new(OnceLock::new());
        let worker_applied_arrange = Arc::clone(&applied_arrange);
        thread::Builder::new()
            .name(format!("play-preload-reuse-bga-{chart_id}"))
            .spawn(move || {
                let result = (|| -> Result<PreloadedInputPlaySession> {
                    let library_db =
                        crate::storage::library_db::LibraryDatabase::open(&library_db_path)?;
                    let mut session_options =
                        crate::screens::play_start::play_session_options_from_start(
                            &app_config,
                            options,
                        );
                    session_options.ln_policy_setting = ln_policy_setting;
                    session_options.rule_mode = rule_mode;
                    let preloaded =
                        crate::screens::play_session::preload_play_session_for_chart_with_callbacks(
                            &library_db,
                            chart_id,
                            session_options.clone(),
                            |arrange| {
                                let _ = worker_applied_arrange.set(arrange.clone());
                            },
                            |loaded, total| {
                                worker_audio_progress.store(
                                    resource_load_progress_units(loaded, total),
                                    Ordering::Relaxed,
                                );
                            },
                        )?;
                    Ok(PreloadedInputPlaySession {
                        chart_id,
                        preloaded,
                        input: preload_input,
                        session_options,
                    })
                })()
                .map_err(|error| format!("{error:#}"));
                let _ = tx.send(PlayPreloadResult { generation, chart_id, result });
            })
            .expect("failed to spawn BGA-reusing play preload thread");
        self.pending_play_preload = Some(PendingPlayPreload {
            generation,
            chart_id,
            input,
            audio_progress,
            applied_arrange,
            rx,
        });
        tracing::info!(chart_id, generation, "play preload with reused BGA started");
    }

    /// Keep the expensive BGA resources warm, but rebuild the chart sound bank
    /// for a quick retry.  Reusing decoded samples made the retry depend on the
    /// previous engine's `SoundId` layout; rebuilding from the exact cached
    /// chart restores the keysound/BGM-to-asset correspondence while retaining
    /// static textures and video decoders.
    pub(super) fn start_quick_retry_preload_reloading_audio(
        &mut self,
        chart_id: i64,
        options: PlayStartOptions,
        cache: PlayMediaCache,
    ) {
        let chart = Arc::clone(cache.chart.as_ref().expect("SameArrange cache includes chart"));
        let applied_arrange =
            cache.applied_arrange.clone().expect("SameArrange cache includes applied arrange");
        let score_key = cache.score_key.expect("SameArrange cache includes score key");
        let chart_normalization_gain = cache.chart_normalization_gain;

        self.play_preload_generation = self.play_preload_generation.wrapping_add(1);
        let generation = self.play_preload_generation;
        self.preloaded_play_session = None;
        self.apply_reused_bga_preload(chart_id, cache.bga_frames.clone(), cache.bga_assets.clone());
        self.play_media_cache = Some(cache);

        let mut session_options =
            play_session_options_from_start(&self.play_session_app_config(), options);
        session_options.ln_policy_setting = self.boot.profile_config.play.ln_mode_policy;
        session_options.rule_mode = self.boot.profile_config.play.rule_mode;
        let input = SharedInputBackend::default();
        let preload_input = input.clone();
        let audio_progress = Arc::new(AtomicU32::new(0));
        let worker_audio_progress = Arc::clone(&audio_progress);
        let preview_applied_arrange = Arc::new(OnceLock::new());
        let _ = preview_applied_arrange.set(applied_arrange.clone());
        let sample_rate = session_options.sample_rate;
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name(format!("quick-retry-audio-preload-{chart_id}"))
            .spawn(move || {
                let preloaded = crate::screens::play_session::
                    preload_play_session_reloading_audio_with_progress(
                        chart,
                        sample_rate,
                        chart_normalization_gain,
                        applied_arrange,
                        score_key,
                        |loaded, total| {
                            worker_audio_progress.store(
                                resource_load_progress_units(loaded, total),
                                Ordering::Relaxed,
                            );
                        },
                    );
                let result = Ok(PreloadedInputPlaySession {
                    chart_id,
                    preloaded,
                    input: preload_input,
                    session_options,
                });
                let _ = tx.send(PlayPreloadResult { generation, chart_id, result });
            })
            .expect("failed to spawn quick retry audio preload thread");
        self.pending_play_preload = Some(PendingPlayPreload {
            generation,
            chart_id,
            input,
            audio_progress,
            applied_arrange: preview_applied_arrange,
            rx,
        });
        tracing::info!(chart_id, generation, "quick retry audio preload started");
    }

    pub(super) fn handle_quick_retry_control(&mut self, control: &str) -> bool {
        let Some(ending) = &self.play_ending else {
            return false;
        };
        if !ending.failed
            || self.active_course.is_some()
            || self.practice_session.is_some()
            || self.last_play_was_autoplay
        {
            return false;
        }
        let mode = if self.select_keys.is_start(control) {
            Some(ResultRetryMode::DifferentArrange)
        } else if self.select_keys.is_e2_action(control) || matches!(control, "Select") {
            Some(ResultRetryMode::SameArrange)
        } else {
            None
        };
        let Some(mode) = mode else {
            return false;
        };
        self.quick_retry_active_play(mode);
        true
    }

    pub(super) fn begin_play_fadeout_after_final_notes_control(&mut self, control: &str) -> bool {
        let escape_before_play_ending = control == "Escape" && self.play_ending.is_none();
        if !play_fadeout_after_final_notes_control(control, &self.select_keys)
            && !escape_before_play_ending
        {
            return false;
        }
        if let Some(ending) = &mut self.play_ending {
            if ending.failed {
                return false;
            }
            if ending.fadeout_started_at.is_none() {
                ending.fadeout_started_at = Some(Instant::now());
                self.update_play_ending_snapshot();
                tracing::info!(control, "started pending play fadeout");
            }
            return true;
        }

        let Some(active_play) = &self.active_play else {
            return false;
        };
        let should_begin = should_begin_play_fadeout_after_final_notes(
            control,
            &self.select_keys,
            self.play_ready_sound_started_at.is_some(),
            self.play_ending.is_some(),
            active_play.running.session.state,
            active_play.running.session.judge.is_exhausted(&active_play.running.session.chart),
        );
        if !should_begin {
            return false;
        }

        let finish_mode = if self.active_course.is_some() {
            crate::screens::play_finish::FinishResultMode::CourseStage
        } else {
            crate::screens::play_finish::FinishResultMode::Normal
        };
        let now = Instant::now();
        let full_combo_elapsed_at_finish_ms =
            self.last_play_snapshot.as_ref().and_then(|snapshot| snapshot.full_combo_elapsed_ms);
        let early_finished = {
            let Some(active_play) = &mut self.active_play else {
                return false;
            };
            active_play.running.session.state = bmz_gameplay::session::PlayState::Finished;
            match crate::screens::play_finish::finish_session_result_once(
                &mut active_play.running.finished,
                &mut self.boot.score_db,
                &mut self.boot.network_db,
                crate::screens::play_finish::FinishSessionResultOnceRequest {
                    profile_paths: &self.boot.profile_paths,
                    replay_config: &self.boot.profile_config.replay,
                    ir_config: &self.boot.profile_config.ir,
                    session: &active_play.running.session,
                    played_at: now_unix_seconds(),
                    applied_arrange: &active_play.running.applied_arrange,
                    target_ex_score: active_play.running.target_ex_score,
                    target_name: &active_play.running.target,
                    score_key: active_play.running.score_key,
                    practice_mode: active_play.running.practice_mode,
                    finish_mode,
                },
            ) {
                Ok(mut finished) => {
                    finished.summary.graph = Arc::new(
                        active_play
                            .running
                            .result_graph
                            .snapshot_for_session(&active_play.running.session),
                    );
                    Some(finished)
                }
                Err(error) => {
                    tracing::error!(%error, "failed to finish play session on requested fadeout");
                    None
                }
            }
        };
        self.save_current_play_options(
            self.active_play.as_ref().map(|active| active.running.session.hispeed),
            "play fadeout requested",
        );
        if let Some(finished) = &early_finished {
            self.start_result_ir_for_finished_play(finished);
        }
        self.notify_obs_play_ended();
        self.play_ending = Some(PlayEndingTransition {
            started_at: now,
            fadeout_started_at: Some(now),
            failed: false,
            full_combo_elapsed_at_finish_ms,
            finished: early_finished,
        });
        self.update_play_ending_snapshot();
        tracing::info!(control, "started play fadeout after final notes");
        true
    }

    pub(super) fn quick_retry_active_play(&mut self, mode: ResultRetryMode) {
        let Some(chart_id) = self.last_started_chart_id else {
            tracing::warn!("quick retry ignored without previous chart id");
            return;
        };
        let mut options = self.active_play_retry_options(mode);
        if !self.prepare_session_mode_or_show_error(chart_id, &mut options) {
            return;
        }
        if options.chart_zero_time == TimeUs(0) {
            options.chart_zero_time = self.play_skin_playstart_offset();
        }
        let media = self.take_play_media_cache_from_active(chart_id, mode);
        tracing::info!(chart_id, ?mode, "quick retrying chart");
        self.notify_obs_retry_play();
        self.save_current_play_options(
            self.active_play.as_ref().map(|active| active.running.session.hispeed),
            "quick retry",
        );
        if let Some(active) = &mut self.active_play
            && let Err(error) = active.running.pause_audio()
        {
            tracing::warn!(%error, "failed to stop previous play audio for quick retry");
        }
        self.active_play = None;
        self.play_ending = None;
        self.finished_play = None;
        self.draining_audio = None;
        self.clear_play_control_holds();
        match media {
            Some(cache)
                if retry_preload_kind(mode, cache.chart.is_some())
                    == RetryPreloadKind::CachedChartWithFreshAudio =>
            {
                self.start_quick_retry_preload_reloading_audio(chart_id, options.clone(), cache);
            }
            Some(cache) => {
                self.start_play_preload_reusing_bga(chart_id, options.clone(), cache);
            }
            None => {
                self.play_media_cache = None;
                self.start_play_preload(chart_id, options.clone());
            }
        }
        self.enter_play_scene(chart_id, options, self.decide_snapshot_for_chart(chart_id));
        self.poll_play_preload();
    }
}
