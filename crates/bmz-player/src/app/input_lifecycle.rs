use super::*;

impl WinitApp {
    pub(super) fn refresh_player_stats_snapshot(&mut self) {
        self.select.player_stats = player_stats_snapshot(
            &self.boot.score_db,
            &self.boot.library_db,
            self.boot.profile_config.statistics.day_start_hour,
        );
    }

    pub(super) fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    pub(super) fn keyboard_input_backend(&self) -> Option<KeyboardInputBackend> {
        keyboard_input_backend_for_config(&self.boot.app_config)
    }

    pub(super) fn raw_input_keyboard_enabled(&self) -> bool {
        self.keyboard_input_backend() == Some(KeyboardInputBackend::RawInput)
    }

    pub(super) fn window_keyboard_gameplay_enabled(&self) -> bool {
        self.keyboard_input_backend() == Some(KeyboardInputBackend::Window)
    }

    pub(super) fn configure_device_events(&self, event_loop: &ActiveEventLoop) {
        let device_events = if self.raw_input_keyboard_enabled() {
            DeviceEvents::WhenFocused
        } else {
            DeviceEvents::Never
        };
        event_loop.listen_device_events(device_events);
    }

    pub(super) fn raw_input_gameplay_blocked(&self) -> bool {
        let practice_overlay = self
            .play
            .practice_session
            .as_ref()
            .is_some_and(|practice| practice.phase == PracticePhase::Config);
        self.ui.egui.as_ref().is_some_and(|egui| egui.blocks_game_input(practice_overlay))
    }

    pub(super) fn play_input_backend(&self) -> Option<SharedInputBackend> {
        play_input_backend_for_context(
            self.play.active_play.as_ref().map(|active_play| &active_play.input),
            self.play.pending_play_start.is_some(),
            self.play.preloaded_play_session.as_ref().map(|preloaded| &preloaded.input),
            self.play.pending_play_preload.as_ref().map(|pending| &pending.input),
        )
    }

    pub(super) fn filter_app_input_bounce(
        &mut self,
        event: DeviceInputEvent,
    ) -> Option<DeviceInputEvent> {
        let config = input_bounce_config_from_profile(&self.boot.profile_config.input);
        self.input.accept_app_event(config, event)
    }

    pub(super) fn route_play_device_input(&mut self, event: DeviceInputEvent) {
        let Some(input) = self.play_input_backend() else {
            return;
        };
        input.push_shared_event(event.clone());
        if self.play.active_play.is_some() {
            return;
        }
        let visual_now = self.play_elapsed_time();
        if let Some(pending) = &mut self.play.pending_play_start {
            pending.visual_input.apply_event(&event, visual_now);
        }
        self.refresh_pending_play_visual_snapshot(visual_now);
    }

    pub(super) fn refresh_pending_play_visual_snapshot(&mut self, visual_now: TimeUs) {
        if self.play.active_play.is_some() {
            return;
        }
        let Some(pending) = &mut self.play.pending_play_start else {
            return;
        };
        pending.visual_input.advance(visual_now);
        let Some(snapshot) = &mut self.play.last_play_snapshot else {
            return;
        };
        crate::screens::play_snapshot::refresh_pending_play_input_visuals(
            snapshot,
            pending.visual_input.key_mode,
            pending.visual_input.lane_keyon_started_at,
            pending.visual_input.lane_keyoff_started_at,
            pending.visual_input.lane_scratch_angle_delta_ms,
            visual_now,
        );
    }

    pub(super) fn route_raw_keyboard_gameplay_input(
        &mut self,
        physical_key: PhysicalKey,
        state: ElementState,
    ) {
        if !self.raw_input_keyboard_enabled() {
            return;
        }
        if self.play_input_backend().is_none() {
            self.input.discard_raw_keyboard_transition(physical_key, state);
            return;
        }
        let config = input_bounce_config_from_profile(&self.boot.profile_config.input);
        let gameplay_blocked = self.raw_input_gameplay_blocked();
        if let Some(event) =
            self.input.raw_keyboard_transition(config, physical_key, state, gameplay_blocked)
        {
            self.route_play_device_input(event);
        }
    }

    pub(super) fn ensure_window(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let video = &self.boot.app_config.video;
        let attributes =
            window_attributes_from_config(video).with_fullscreen(fullscreen_from_config(
                &video.mode,
                select_monitor(
                    &video.monitor_name,
                    event_loop.available_monitors(),
                    event_loop.primary_monitor(),
                ),
            ));
        match event_loop.create_window(attributes) {
            Ok(window) => {
                let window = Arc::new(window);
                window.set_visible(true);
                let size = surface_size_for_window(&window);
                // サーフェス生成前に present mode とバックエンド設定を反映させておく。
                self.renderer.set_present_mode(config_present_mode(&self.boot.app_config.video));
                let backend = config_renderer_backend(self.boot.app_config.video.renderer.clone());
                self.renderer.set_backend(backend);
                if let Err(error) = self.renderer.attach_surface(Arc::clone(&window), size) {
                    tracing::error!(%error, "failed to initialize renderer surface");
                    event_loop.exit();
                    return;
                }
                tracing::info!(
                    width = size.width,
                    height = size.height,
                    "window and renderer surface ready"
                );
                // surface 接続後 (= GPU device/queue 利用可能) に upload worker を起動する。
                // decode 結果はそれまで skin_decode_rx にバッファされ、起動後にドレインされる。
                self.start_skin_upload_worker();
                self.configure_device_events(event_loop);
                window.request_redraw();
                self.ui.egui = Some(EguiLayer::new(
                    &window,
                    self.boot.profile_config.ui.show_fps,
                    vec![self.boot.app_paths.bundled_noto_cjk_font_root()],
                ));
                self.window = Some(window);
            }
            Err(error) => {
                tracing::error!(%error, "failed to create window");
                event_loop.exit();
            }
        }
    }

    pub(super) fn start_deferred_boot(&mut self) {
        let Some(boot) = self.deferred_boot.take() else {
            return;
        };
        match boot {
            DeferredBoot::Chart { chart_id, replay_slot } => {
                tracing::info!(chart_id, "booting directly into chart");
                if let Some(slot) = replay_slot {
                    if !self.try_start_replay_for_chart(chart_id, slot, false) {
                        tracing::warn!(slot, "boot replay slot empty; falling back to normal play");
                        self.start_chart(chart_id);
                    }
                } else {
                    self.start_chart(chart_id);
                }
            }
            DeferredBoot::Practice { chart_id, start_time_ms, end_time_ms } => {
                tracing::info!(chart_id, "booting into practice mode");
                self.enter_practice(chart_id, PracticeCliOverrides { start_time_ms, end_time_ms });
            }
            DeferredBoot::ReplayFile { path } => {
                tracing::info!(%path, "booting replay from file");
                if !self.try_start_replay_from_file(std::path::Path::new(&path)) {
                    tracing::warn!(%path, "replay file boot failed; staying on select");
                }
            }
            DeferredBoot::CourseReplay { course_id } => {
                let Some(identity) = self.ir_course_identity(course_id) else {
                    tracing::warn!(
                        course_id,
                        "course identity unavailable; --boot-course-replay has nothing to replay"
                    );
                    return;
                };
                let rule_mode = self.boot.profile_config.play.rule_mode;
                match self.boot.score_db.latest_course_score_id(&identity.course_hash, rule_mode) {
                    Ok(Some(course_score_id)) => {
                        tracing::info!(course_id, course_score_id, "booting into course replay");
                        self.start_course_replay_with_auto_advance(
                            course_id,
                            course_score_id,
                            true,
                        );
                    }
                    Ok(None) => {
                        tracing::warn!(
                            course_id,
                            "no saved course attempt; --boot-course-replay has nothing to replay"
                        );
                    }
                    Err(error) => {
                        tracing::error!(
                            %error,
                            course_id,
                            "failed to look up latest course score for replay boot"
                        );
                    }
                }
            }
            DeferredBoot::Course { course_id } => {
                tracing::info!(course_id, "booting into fresh course");
                self.start_course_with_arrange(course_id, Vec::new(), true);
            }
        }
    }
}
