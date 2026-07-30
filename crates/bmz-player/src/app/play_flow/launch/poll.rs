use super::*;

impl WinitApp {
    pub(super) fn poll_play_preload(&mut self) {
        // 1) preload worker からの結果を受け取り (Decide 演出中でも受信して退避する)。
        if let Some(pending) = &self.play.pending_play_preload {
            match pending.rx.try_recv() {
                Ok(result) => {
                    self.play.pending_play_preload = None;
                    if result.generation != self.play.play_preload_generation {
                        tracing::debug!(
                            chart_id = result.chart_id,
                            generation = result.generation,
                            current_generation = self.play.play_preload_generation,
                            "discarding stale play preload result"
                        );
                        if self.play.pending_play_start.is_some() {
                            tracing::warn!(
                                chart_id = result.chart_id,
                                generation = result.generation,
                                current_generation = self.play.play_preload_generation,
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
                                self.play.preloaded_play_session = Some(prepared);
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
                                if self.play.pending_play_start.is_some() {
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
                    self.play.pending_play_preload = None;
                    if self.play.pending_play_start.is_some() {
                        self.abort_pending_play_start();
                        return;
                    }
                }
            }
        }

        // 2) Play 入場が確定 (pending_play_start) しており、バッファに preload があれば install。
        if self
            .play
            .practice_session
            .as_ref()
            .is_some_and(|practice| practice.phase == PracticePhase::Config)
        {
            return;
        }
        let Some(play_start) = self.play.pending_play_start.as_ref() else {
            return;
        };
        let Some(prepared) = self.play.preloaded_play_session.take() else {
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
        self.play.pending_play_start = None;
        self.play.active_play = None;
        self.play.decide_sound_stopped_for_chart_start = true;
        self.clear_play_meta_image_state();
        self.play.last_play_snapshot = None;
        // An audio-open / audio-start failure bounces the user back to the
        // select screen.  If they were in a course at the time, the course
        // session is no longer valid — otherwise the next chart they pick
        // would be treated as the next entry of a stale course (route
        // through advance_course_after_finish with mismatched chart_id).
        self.clear_active_course_state();
        self.select.autoplay_folder = None;
        self.play.play_media_cache = None;
        let now = Instant::now();
        self.select.select_scene_started_at = now;
        self.restart_select_bar_timer_without_scroll(now);
    }

    /// Clears any active course session and the cached finished-course
    /// summary.  Call from any path that returns to the select screen
    /// without completing the course naturally.
    pub(super) fn clear_active_course_state(&mut self) {
        if self.play.active_course.is_some() || self.result.finished_course.is_some() {
            tracing::info!(
                had_active = self.play.active_course.is_some(),
                had_finished = self.result.finished_course.is_some(),
                "clearing course session state (abort or cancel)"
            );
        }
        self.play.active_course = None;
        self.clear_finished_course();
    }

    pub(super) fn play_start_options(&self) -> PlayStartOptions {
        // beatoraja assigns a 24-bit seed even to NORMAL/MIRROR. Generate both
        // sides here so preload, retry, replay and IR all observe one stable pair.
        let option_seeds = crate::random_option_seed::RandomOptionSeeds::fresh(true);
        let random_trainer_seed = self.select.random_trainer.arrange_seed(option_seeds.p1);
        PlayStartOptions {
            session_mode: self.select.session_mode,
            autoplay: self.select.session_mode.primary_autoplay(),
            gauge: Some(self.select.gauge_option),
            gauge_auto_shift: self.select.gauge_auto_shift_option,
            bottom_shiftable_gauge: self.select.bottom_shiftable_gauge_option,
            arrange: self.select.arrange_option,
            arrange_2p: self.select.arrange_option_2p,
            double_option: self.select.double_option,
            hs_fix: self.select.hs_fix_option,
            target: self.select.target_option,
            arrange_seed: Some(i64::from(option_seeds.p1.value())),
            arrange_seed_2p: option_seeds.p2.map(|seed| i64::from(seed.value())),
            random_trainer_seed,
            bms_random_seed: Some(crate::random_option_seed::fresh_bms_random_seed()),
            ..Default::default()
        }
    }

    pub(super) fn refresh_play_target_from_source(&mut self) {
        let source = self
            .play
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
                self.play.preloaded_play_session.as_ref().map(|preloaded| {
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
        self.select.select_ir.update(
            &self.boot.profile_config.ir,
            &self.boot.profile_paths.root_dir,
            &context,
            score_key.ln_policy,
            score_key.double_option,
            score_key.rule_mode,
            Some(score_key.chart_sha256),
        );
        let resolved = self.select.select_ir.target_ex_score_for(
            &self.boot.profile_config.ir,
            Some(score_key.chart_sha256),
            target,
            local_best_ex_score,
        );
        if let Some(active) = &mut self.play.active_play
            && active.running.score_key == score_key
            && active.running.target_option == target
        {
            active.running.target_ex_score = resolved;
        }
    }
}
