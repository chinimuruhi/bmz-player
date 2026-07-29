use super::*;

impl WinitApp {
    pub(super) fn restart_select_scene_timers(&mut self) {
        let now = Instant::now();
        self.select_scene_started_at = now;
        self.restart_select_bar_timer_without_scroll(now);
        self.option_panel_started_at = now;
        self.option_panel_off_started_at = [None; 6];
    }

    pub(super) fn current_frame_limit(&self) -> u32 {
        if self.focused {
            self.boot.app_config.video.target_fps
        } else {
            self.boot.app_config.video.frame_limit_in_background
        }
    }

    /// `RedrawRequested` が現在の deadline に到達していればフレームを開始する。
    ///
    /// deadline より早い redraw は描画せず `WaitUntil` へ戻す。event loop thread を
    /// sleep させないため、待機中も keyboard/device event を遅延なく受け取れる。
    pub(super) fn begin_scheduled_frame(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let fps = self.current_frame_limit();
        let now = Instant::now();
        match self.frame.begin_scheduled_frame(now, fps) {
            FrameSchedule::Start => true,
            FrameSchedule::WaitUntil(deadline) => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
                false
            }
        }
    }

    /// 次の frame deadline まで winit に待機させ、到達時に redraw を要求する。
    /// FPS が 0、設定変更直後、明示的な skip 時は即座に次フレームを要求する。
    pub(super) fn schedule_next_frame(&self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let fps = self.current_frame_limit();
        if let Some(deadline) = self.frame.next_deadline(now, fps) {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            return;
        }

        if fps == 0 {
            event_loop.set_control_flow(ControlFlow::Poll);
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
        self.request_redraw();
    }

    /// egui の 1 フレームを構築し、renderer へ描画データを渡す。
    /// `render_current_scene` の前に呼ぶこと。
    pub(super) fn run_egui_frame(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let scene_kind = self.current_scene_kind();
        let scene = match scene_kind {
            AppSceneKind::Select => "Select",
            AppSceneKind::Decide => "Decide",
            AppSceneKind::Play => "Play",
            AppSceneKind::Result => "Result",
        };
        let practice_overlay = self
            .practice_session
            .as_ref()
            .is_some_and(|practice| practice.phase == PracticePhase::Config);
        let use_idle_egui_frame = scene_kind == AppSceneKind::Play
            && self.play_ending.is_none()
            && self.egui.as_ref().is_some_and(|egui| {
                !egui.needs_full_frame(scene, practice_overlay, self.update_prompt.is_some())
            });
        if use_idle_egui_frame {
            let Some(mut egui) = self.egui.take() else {
                return;
            };
            let frame =
                egui.run_idle_frame(&window, self.boot.profile_config.ui.locale().font_coverage());
            self.egui = Some(egui);
            self.renderer.set_egui_frame(frame);
            return;
        }
        let size = window.inner_size();
        let presentation = self.renderer.surface_presentation_status();
        let info = DebugInfo {
            scene,
            current_fps: self.frame.current_fps(),
            width: size.width,
            height: size.height,
            effective_present_mode: presentation.map(|status| status.effective_mode),
            maximum_frame_latency: presentation.map(|status| status.maximum_frame_latency),
        };
        let play4_path = self.boot.profile_config.skin.play4.clone();
        let play5_path = self.boot.profile_config.skin.play5.clone();
        let play6_path = self.boot.profile_config.skin.play6.clone();
        let play7_path = self.boot.profile_config.skin.play7.clone();
        let play8_path = self.boot.profile_config.skin.play8.clone();
        let play9_path = self.boot.profile_config.skin.play9.clone();
        let play10_path = self.boot.profile_config.skin.play10.clone();
        let play14_path = self.boot.profile_config.skin.play14.clone();
        let battle5_path = self.boot.profile_config.skin.battle5.clone();
        let battle7_path = self.boot.profile_config.skin.battle7.clone();
        let course_result_path = self.boot.profile_config.skin.course_result.clone();
        let play4_defs = self.play_skin_defs_for_path(&play4_path);
        let play5_defs = self.play_skin_defs_for_path(&play5_path);
        let play6_defs = self.play_skin_defs_for_path(&play6_path);
        let play7_defs = self.play_skin_defs_for_path(&play7_path);
        let play8_defs = self.play_skin_defs_for_path(&play8_path);
        let play9_defs = self.play_skin_defs_for_path(&play9_path);
        let play10_defs = self.play_skin_defs_for_path(&play10_path);
        let play14_defs = self.play_skin_defs_for_path(&play14_path);
        let battle5_defs = self.play_skin_defs_for_path(&battle5_path);
        let battle7_defs = self.play_skin_defs_for_path(&battle7_path);
        let course_result_defs = self.play_skin_defs_for_path(&course_result_path);
        let skin_meta = SkinConfigMeta {
            select: SceneSkinDefs::from_document(self.renderer.select_skin_document()),
            decide: SceneSkinDefs::from_document(self.renderer.decide_skin_document()),
            play4: play4_defs,
            play5: play5_defs,
            play6: play6_defs,
            play7: play7_defs,
            play8: play8_defs,
            play9: play9_defs,
            play10: play10_defs,
            play14: play14_defs,
            battle5: battle5_defs,
            battle7: battle7_defs,
            result: SceneSkinDefs::from_document(self.renderer.result_skin_document()),
            course_result: course_result_defs,
        };
        let course_result_active =
            matches!(scene_kind, AppSceneKind::Result) && self.finished_course.is_some();
        if matches!(scene_kind, AppSceneKind::Result)
            && self
                .result_ir
                .as_ref()
                .is_some_and(|state| state.is_course() != course_result_active)
        {
            self.result_ir = None;
        }
        if course_result_active && self.result_ir.is_none() && !self.finished_course_ir_attempted {
            // IR が無効、または identity が解決できない場合も、この Result 滞在中の
            // 起動判定は一度で完了させる。
            self.finished_course_ir_attempted = true;
            if let Some((course_hash, rian_course_hash_v1, gauge, ln_policy, rule_mode)) =
                self.course_result_ir_target()
            {
                self.result_ir = crate::screens::result_ir::spawn_course_result_ir_task(
                    self.boot.profile_paths.root_dir.clone(),
                    self.boot.profile_paths.score_db.clone(),
                    self.boot.profile_paths.network_db.clone(),
                    self.boot.app_paths.logs_dir.clone(),
                    &self.boot.profile_config.ir,
                    self.finished_course
                        .as_ref()
                        .and_then(|course| course.course_score_id)
                        .unwrap_or_default(),
                    crate::screens::result_ir::ResultIrCourseHashes {
                        local: course_hash,
                        rian_v1: rian_course_hash_v1,
                    },
                    gauge,
                    ln_policy,
                    rule_mode,
                );
            }
        }
        // `egui` は run の直前に Option から一時的に取り出すため、コース全ステージの
        // graph を含む CourseResultSummary をフレームごとに clone せず参照で渡せる。
        let course_result = self.finished_course.as_ref();
        // Only show the course preview when the user is on the select screen
        // and the cursor is over a course row.
        let course_preview = matches!(scene_kind, AppSceneKind::Select)
            .then(|| {
                self.select_items.get(self.selected_index).and_then(|item| match item {
                    SelectItem::Course(row) => Some(row.clone()),
                    _ => None,
                })
            })
            .flatten();
        let selected_course_ir_target = matches!(scene_kind, AppSceneKind::Select)
            .then(|| self.selected_course_ir_target())
            .flatten();
        let practice_media_ready = self.practice_media_ready();
        let mut practice_panel_ctx = None;
        if let Some(practice) = &mut self.practice_session
            && practice.phase == PracticePhase::Config
        {
            practice_panel_ctx = Some(PracticePanelContext {
                property: &mut practice.property,
                chart_title: &practice.chart_title,
                media_ready: practice_media_ready,
                max_end_time_ms: practice.max_end_time_ms,
            });
        }
        // 通常プレイは play ending に入った時点で IR 送信を早期起動し、Result
        // 画面まで状態を保持する。コース最終リザルトでは course_hash ベースの
        // course ranking を取得するため、単曲用 state は Result 突入時に差し替える。
        if matches!(scene_kind, AppSceneKind::Result) {
            if self.result_ir.is_none()
                && !course_result_active
                && let Some(finished) = &self.finished_play
            {
                self.result_ir = crate::screens::result_ir::spawn_result_ir_task(
                    self.boot.profile_paths.root_dir.clone(),
                    self.boot.profile_paths.score_db.clone(),
                    self.boot.profile_paths.network_db.clone(),
                    self.boot.app_paths.logs_dir.clone(),
                    &self.boot.profile_config.ir,
                    finished.stored.score_history_id,
                    crate::storage::common::hash_to_hex(&finished.result.chart_sha256),
                    finished.ln_policy,
                    finished.double_option,
                    finished.rule_mode,
                );
            }
            let loaded_rankings =
                self.result_ir.as_mut().map(|state| state.poll()).unwrap_or_default();
            for ranking in loaded_rankings {
                self.select_ir
                    .cache_result_global_ranking(&ranking.chart_sha256_hex, &ranking.ranking);
            }
        } else if self.play_ending.is_some() {
            let loaded_rankings =
                self.result_ir.as_mut().map(|state| state.poll()).unwrap_or_default();
            for ranking in loaded_rankings {
                self.select_ir
                    .cache_result_global_ranking(&ranking.chart_sha256_hex, &ranking.ranking);
            }
        } else {
            self.result_ir = None;
        }
        // 選曲画面ではカーソル譜面の IR ランキングをデバウンスつきで取得する
        // (NUMBER_IR_RANK / NUMBER_IR_TOTALPLAYER / OPTION_IR_* 用)。
        if matches!(scene_kind, AppSceneKind::Select) {
            // `selected_chart_sha256()` は &self 全体を借りるため、practice ctx の
            // &mut 借用と衝突しないようフィールド単位で参照する。
            let (selected, ln_profile, key_mode) = match self.select_items.get(self.selected_index)
            {
                Some(SelectItem::Chart(row)) => (
                    row.score_sha256(),
                    // library 登録済みなら譜面の LN プロファイルから実プレイと
                    // 同じスコア分離キーを解決する。未登録は default 近似。
                    row.chart.as_ref().map(|chart| chart.ln_profile).unwrap_or_default(),
                    row.chart
                        .as_ref()
                        .and_then(|chart| KeyMode::from_str_opt(&chart.mode))
                        .unwrap_or_default(),
                ),
                _ => (None, crate::ln_policy::ChartLnProfile::default(), KeyMode::default()),
            };
            let ln_policy = crate::ln_policy::score_ln_policy(
                self.boot.profile_config.play.ln_mode_policy,
                ln_profile,
            );
            let double_option = self.double_option.normalize_for_key_mode(key_mode).score_bucket();
            let ir_config = self.boot.profile_config.ir.clone();
            if let Some(course) = selected_course_ir_target {
                let context = format!(
                    "course:{}:{}:{}:{}:{}",
                    course.course_hash,
                    course.rian_course_hash_v1,
                    course.gauge,
                    course.ln_policy,
                    course.rule_mode.as_str()
                );
                self.select_ir.update_course(&ir_config, &context, Some(course));
            } else {
                let context = select_ir_cache_context(
                    self.boot.profile_config.play.ln_mode_policy,
                    ln_policy,
                    double_option,
                    self.boot.profile_config.play.rule_mode,
                );
                self.select_ir.update(
                    &ir_config,
                    &self.boot.profile_paths.root_dir,
                    &context,
                    ln_policy,
                    double_option,
                    self.boot.profile_config.play.rule_mode,
                    selected,
                );
            }
        }
        let result_ir_panel = self.result_ir.as_mut();
        let update_dialog = self.update_prompt.as_ref().map(UpdatePrompt::as_dialog);
        let obs_connection_status = self
            .obs_controller
            .as_ref()
            .map(crate::obs::ObsController::status)
            .unwrap_or_else(crate::obs::ObsConnectionStatus::disabled);
        let connected_gamepads =
            self.gamepad.as_ref().map(|gamepad| gamepad.connected_gamepads()).unwrap_or_default();
        let locale_before_ui = self.boot.profile_config.ui.locale();
        let Some(mut egui) = self.egui.take() else {
            return;
        };
        let play_profile_before_egui = self.boot.profile_config.play.clone();
        let lane_profile_before_egui = self.boot.profile_config.lane.clone();
        let input_profile_before_egui = self.boot.profile_config.input.clone();
        let output = egui.run(
            &window,
            EguiRunContext {
                info: &info,
                app_config: &mut self.boot.app_config,
                profile_config: &mut self.boot.profile_config,
                random_trainer: &mut self.random_trainer,
                skin_meta: &skin_meta,
                skin_catalog: &self.skin_catalog,
                course_result,
                course_preview: course_preview.as_ref(),
                practice: practice_panel_ctx.as_mut(),
                result_ir: result_ir_panel,
                profile_root: &self.boot.profile_paths.root_dir,
                app_paths: &self.boot.app_paths,
                difficulty_tables: &self.difficulty_tables,
                log_buffer: &self.log_buffer,
                update_dialog,
                obs_connection_status: &obs_connection_status,
                connected_gamepads: &connected_gamepads,
            },
        );
        self.egui = Some(egui);
        self.reconcile_rian_table_identity();
        let locale_after_ui = self.boot.profile_config.ui.locale();
        self.renderer.set_default_font_coverage(locale_after_ui.font_coverage());
        if locale_after_ui != locale_before_ui {
            // 設定・検索履歴などアプリが生成した行名を新しい locale で作り直す。
            // 選択復元は表示名ではなく typed/path ID を使う。
            self.search.clear_message();
            self.reload_select_items();
        }
        self.renderer.set_egui_frame(output.frame);
        self.sync_changed_select_play_options_from_profile(&play_profile_before_egui);
        self.sync_changed_select_score_context(SelectScoreContext::from_play(
            &play_profile_before_egui,
        ));
        self.sync_changed_gamepad_analog_config_from_profile(&input_profile_before_egui);
        if profile_lane_settings_changed(&lane_profile_before_egui, &self.boot.profile_config.lane)
        {
            self.sync_active_play_lane_settings_from_profile(&lane_profile_before_egui);
        }
        self.sync_realtime_profile_settings();
        self.sync_discord_presence_config();
        if output.practice_leave {
            self.leave_practice();
            return;
        }
        if output.practice_start {
            self.start_practice_round();
        }
        // 本体設定パネルでの present mode 変更を即座に反映する。
        self.renderer.set_present_mode(config_present_mode(&self.boot.app_config.video));
        self.renderer.set_internal_resolution_mode(config_internal_resolution_mode(
            &self.boot.app_config.video,
        ));
        // ウィンドウモード変更をライブ反映する (差分があるときのみ適用)。
        let desired_mode = self.boot.app_config.video.mode.clone();
        if desired_mode != self.applied_window_mode {
            let monitor = select_monitor(
                &self.boot.app_config.video.monitor_name,
                window.available_monitors(),
                window.primary_monitor(),
            );
            window.set_fullscreen(fullscreen_from_config(&desired_mode, monitor));
            tracing::info!(mode = ?desired_mode, "window mode updated");
            self.applied_window_mode = desired_mode;
        }
        let mut apply_obs_config = output.obs_enabled_changed;
        if output.save_app_config {
            match save_app_config(&self.boot.app_paths.config_toml, &self.boot.app_config) {
                Ok(()) => {
                    tracing::info!("app config saved from egui settings panel");
                    apply_obs_config = true;
                }
                Err(error) => tracing::error!(%error, "failed to save app config"),
            }
        }
        if apply_obs_config {
            self.sync_obs_controller();
        }
        if output.check_for_update {
            self.spawn_update_check("manual update check", true);
        }
        if let Some(action) = output.update_dialog_action {
            self.handle_update_dialog_action(action);
        }
        if output.apply_audio_output {
            self.reopen_audio_output();
        }
        if !output.table_fetch_urls.is_empty() {
            self.spawn_table_fetches(output.table_fetch_urls, "egui table fetch".to_string());
        }
        for request in output.song_scan_requests {
            self.spawn_song_scan_request(request);
        }
        if output.trigger_song_rescan {
            self.load_songs_and_reload();
        }
        if let Some(request) = output.score_import_request {
            self.import_external_scores(request);
        }
        if output.save_profile_config {
            match save_profile_config(
                &self.boot.profile_paths.profile_toml,
                &self.boot.profile_config,
            ) {
                Ok(()) => tracing::info!("profile config saved from egui skin panel"),
                Err(error) => tracing::error!(%error, "failed to save profile config"),
            }
        }
        if output.reset_skin_config {
            self.reset_skin_config_from_disk();
        } else if output.skin_reload_request.any() {
            if output.skin_reload_request.offsets {
                self.apply_profile_skin_offsets_to_active_play();
            }
            if output.skin_reload_request.any_reload() {
                self.reload_skins(output.skin_reload_request);
            }
        }
    }

    /// リザルト遷移後も鳴らし続けている音声出力を監視し、スケジュール済みの
    /// BGM/キー音がすべて鳴り切ったら出力を解放する。
    pub(super) fn advance_draining_audio(&mut self) {
        let Some(audio) = &self.draining_audio else {
            return;
        };
        if audio.engine.is_idle() {
            tracing::info!("play audio drained after result; releasing output");
            self.draining_audio = None;
        }
    }

    pub(super) fn render_current_scene(&mut self) -> Option<SceneFrameProfileSample> {
        let select_view = matches!(self.view_state(), AppViewState::Select);
        let play_view = matches!(self.view_state(), AppViewState::Play);
        let result_view = matches!(self.view_state(), AppViewState::Result);
        let profiling_select = select_view
            && tracing::enabled!(target: "bmz_player::select_profile", tracing::Level::DEBUG);
        let profiling_play = play_view
            && tracing::enabled!(target: "bmz_player::play_profile", tracing::Level::DEBUG);
        let profiling_result = result_view
            && tracing::enabled!(target: "bmz_player::result_profile", tracing::Level::DEBUG);
        if select_view {
            self.refresh_visible_select_folder_summaries();
            self.poll_select_asset_loads();
            self.sync_select_stage_texture();
            self.sync_select_backbmp_texture();
            self.sync_select_banner_texture();
            self.sync_select_preview_audio();
            self.update_select_preview_fade();
        }
        self.start_scene_timers_before_snapshot(select_view, result_view);
        let snapshot_start = Instant::now();
        let scene = self.scene_snapshot();
        let snapshot_us = snapshot_start.elapsed().as_micros();
        let video_start = Instant::now();
        let video_profile = self.update_current_skin_video_sources(
            &scene,
            profiling_select || profiling_play || profiling_result,
        );
        let video_us = video_start.elapsed().as_micros();
        let scene_kind = scene_kind(&scene);
        self.update_window_title_for_scene(scene_kind);
        if let (Some(path), Some(exit_after_frames)) =
            (&self.smoke_screenshot_path, self.smoke_exit_after_frames)
            && self.rendered_frames.saturating_add(1) >= exit_after_frames
        {
            self.renderer.request_screenshot(path.clone());
        }
        let render_start = Instant::now();
        let render_status = self.renderer.render_scene_status(scene);
        let render_us = render_start.elapsed().as_micros();
        let frame_timings = self.renderer.last_frame_timings();
        if let Some(probe) = self.pending_skin_render_probe.take() {
            let expected_scene = match probe.kind {
                SkinKind::Select => AppSceneKind::Select,
                SkinKind::Decide => AppSceneKind::Decide,
                SkinKind::Play => AppSceneKind::Play,
                SkinKind::Result => AppSceneKind::Result,
            };
            if expected_scene == scene_kind {
                let timings = frame_timings.unwrap_or_default();
                tracing::debug!(
                    kind = ?probe.kind,
                    generation = probe.generation,
                    scene = ?scene_kind,
                    status = ?render_status.as_ref().ok().copied(),
                    since_apply_us = instant_elapsed_us_u64(probe.applied_at),
                    snapshot_us,
                    video_us,
                    render_us,
                    plan_us = timings.plan_us,
                    draw_us = timings.draw_us,
                    text_us = timings.text_us,
                    geometry_us = timings.geometry_us,
                    upload_us = timings.upload_us,
                    submit_us = timings.submit_us,
                    surface_us = timings.surface_us,
                    bind_us = timings.bind_us,
                    encode_us = timings.encode_us,
                    queue_us = timings.queue_us,
                    present_us = timings.present_us,
                    commands = timings.commands,
                    steps = timings.steps,
                    rect_steps = timings.rect_steps,
                    image_steps = timings.image_steps,
                    text_steps = timings.text_steps,
                    rect_instances = timings.rect_instances,
                    image_instances = timings.image_instances,
                    text_instances = timings.text_instances,
                    "skin reload first render timings"
                );
            } else {
                self.pending_skin_render_probe = Some(probe);
            }
        }
        match render_status {
            Ok(RenderSurfaceStatus::Rendered)
            | Ok(RenderSurfaceStatus::SkippedNoSurface)
            | Ok(RenderSurfaceStatus::SkippedZeroSize) => {}
            Ok(RenderSurfaceStatus::Reconfigured) => {
                tracing::debug!("renderer surface reconfigured");
            }
            Ok(RenderSurfaceStatus::TimedOut) => {
                tracing::debug!("renderer surface acquisition timed out");
            }
            Err(error) => {
                tracing::error!(%error, "failed to present render scene");
            }
        }
        if profiling_select {
            Some(SceneFrameProfileSample {
                kind: FrameProfileKind::Select,
                video_us,
                video_profile,
                snapshot_us,
                render_us,
                render_timings: frame_timings,
            })
        } else if profiling_play {
            Some(SceneFrameProfileSample {
                kind: FrameProfileKind::Play,
                video_us,
                video_profile,
                snapshot_us,
                render_us,
                render_timings: frame_timings,
            })
        } else if profiling_result {
            Some(SceneFrameProfileSample {
                kind: FrameProfileKind::Result,
                video_us,
                video_profile,
                snapshot_us,
                render_us,
                render_timings: frame_timings,
            })
        } else {
            None
        }
    }

    pub(super) fn request_manual_screenshot(&mut self) {
        let path = next_screenshot_path(
            &self.boot.app_config.screenshot.dir,
            &self.boot.app_paths.data_dir,
        );
        let toast_message = if self.boot.app_config.screenshot.copy_to_clipboard {
            self.renderer.request_screenshot_with_clipboard(path.clone());
            tracing::info!(
                path = %path.display(),
                "manual screenshot requested with clipboard copy"
            );
            Localizer::new(self.boot.profile_config.ui.locale()).text("screenshot-saved-clipboard")
        } else {
            self.renderer.request_screenshot(path.clone());
            tracing::info!(path = %path.display(), "manual screenshot requested");
            Localizer::new(self.boot.profile_config.ui.locale()).text("screenshot-saved")
        };
        // トーストは次フレーム以降に出す。撮影フレームでは has_pending_screenshot で隠す。
        self.show_left_overlay_toast(toast_message);
        self.notify_obs_save_recording(crate::obs::ObsRecordingSaveReason::OnScreenshot);
    }

    pub(super) fn show_left_overlay_toast(&mut self, message: impl Into<String>) {
        self.left_overlay_toast =
            Some(LeftOverlayToast { message: message.into(), shown_at: Instant::now() });
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    pub(super) fn flush_pending_screenshots(&mut self, reason: &'static str) {
        if let Err(error) = self.renderer.flush_pending_screenshots() {
            tracing::warn!(%error, reason, "failed to flush pending screenshots");
        }
    }

    pub(super) fn handle_smoke_exit_after_redraw(&mut self, event_loop: &ActiveEventLoop) {
        if self.smoke_exit_on_result && self.finished_play.is_some() {
            self.smoke_exit_on_result = false;
            tracing::info!("smoke result reached; leaving event loop");
            self.save_configs_for_exit(None, "game exit");
            self.flush_pending_screenshots("smoke result exit");
            event_loop.exit();
            return;
        }

        if let Some(exit_after_result_frames) = self.smoke_exit_after_result_frames
            && self.finished_play.is_some()
        {
            self.rendered_result_frames = self.rendered_result_frames.saturating_add(1);
            if self.rendered_result_frames >= exit_after_result_frames {
                self.smoke_exit_after_result_frames = None;
                tracing::info!(
                    frames = self.rendered_result_frames,
                    "smoke result frame count reached; leaving event loop"
                );
                self.save_configs_for_exit(None, "game exit");
                self.flush_pending_screenshots("smoke result frame exit");
                event_loop.exit();
                return;
            }
        }

        if let Some(exit_after_play_frames) = self.smoke_exit_after_play_frames
            && self.current_scene_kind() == AppSceneKind::Play
        {
            let (frames, should_exit) =
                count_smoke_play_frame(self.rendered_play_frames, exit_after_play_frames);
            self.rendered_play_frames = frames;
            if should_exit {
                self.smoke_exit_after_play_frames = None;
                tracing::info!(
                    frames = self.rendered_play_frames,
                    "smoke play frame count reached; leaving event loop"
                );
                self.save_configs_for_exit(self.active_hispeed(), "smoke play frame exit");
                self.flush_pending_screenshots("smoke play frame exit");
                event_loop.exit();
                return;
            }
        }

        let Some(exit_after_frames) = self.smoke_exit_after_frames else {
            return;
        };

        self.rendered_frames = self.rendered_frames.saturating_add(1);
        if self.rendered_frames >= exit_after_frames {
            self.smoke_exit_after_frames = None;
            tracing::info!(
                frames = self.rendered_frames,
                "smoke exit frame count reached; leaving event loop"
            );
            self.save_configs_for_exit(self.active_hispeed(), "game exit");
            self.flush_pending_screenshots("smoke frame exit");
            event_loop.exit();
        }
    }

    pub(super) fn active_hispeed(&self) -> Option<f32> {
        self.active_play
            .as_ref()
            .map(|active| active.running.session.hispeed)
            .or_else(|| self.pending_play_start.as_ref().map(|pending| pending.lane.hispeed))
    }

    pub(super) fn start_scene_timers_before_snapshot(
        &mut self,
        select_view: bool,
        result_view: bool,
    ) {
        match self.last_scene_kind {
            Some(AppSceneKind::Select) if select_view => {}
            _ if select_view => self.restart_select_scene_timers(),
            Some(AppSceneKind::Result) if result_view => {}
            _ if result_view => {
                self.result_scene_started_at = Instant::now();
            }
            _ => {}
        }
    }

    pub(super) fn active_lane_state(&self) -> Option<ActiveLaneState> {
        self.active_play
            .as_ref()
            .map(|active| active_lane_state_for_session(&active.running.session))
            .or_else(|| {
                self.pending_play_start.as_ref().map(|pending| ActiveLaneState {
                    lane_cover: pending.lane.lane_cover,
                    lift: pending.lane.lift,
                    hispeed_mode: pending.lane.hispeed_mode,
                    target_green_number: pending.lane.target_green_number,
                })
            })
    }

    pub(super) fn commit_pending_play_lane_state_to_profile(&mut self) {
        let Some(pending) = &self.pending_play_start else {
            return;
        };
        if pending.lane_actions.is_empty() {
            return;
        }
        apply_lane_state_to_profile(
            &mut self.boot.profile_config,
            Some(pending.lane.hispeed),
            Some(ActiveLaneState {
                lane_cover: pending.lane.lane_cover,
                lift: pending.lane.lift,
                hispeed_mode: pending.lane.hispeed_mode,
                target_green_number: pending.lane.target_green_number,
            }),
        );
        self.boot.profile_config.updated_at = now_unix_seconds();
    }

    pub(super) fn commit_active_play_lane_state_to_profile(&mut self) -> bool {
        let Some(active_play) = &self.active_play else {
            return false;
        };
        let session = &active_play.running.session;
        apply_lane_state_to_profile(
            &mut self.boot.profile_config,
            Some(session.hispeed),
            Some(active_lane_state_for_session(session)),
        );
        self.boot.profile_config.updated_at = now_unix_seconds();
        true
    }

    pub(super) fn save_current_play_options(&mut self, hispeed: Option<f32>, reason: &'static str) {
        let lane_state = self.active_lane_state();
        let options = self.current_select_play_options();
        self.sync_profile_visual_offset_from_active_play();
        apply_current_play_options_to_profile(
            &mut self.boot.profile_config,
            hispeed,
            lane_state,
            options,
            now_unix_seconds(),
        );
        if let Err(error) =
            save_profile_config(&self.boot.profile_paths.profile_toml, &self.boot.profile_config)
        {
            tracing::error!(%error, reason, "failed to save profile play options");
        } else {
            tracing::info!(reason, "saved profile play options");
        }
    }

    pub(super) fn save_configs_for_exit(&mut self, hispeed: Option<f32>, reason: &'static str) {
        if self.exit_configs_saved {
            return;
        }
        self.save_current_play_options(hispeed, reason);
        if let Err(error) = save_app_config(&self.boot.app_paths.config_toml, &self.boot.app_config)
        {
            tracing::error!(%error, reason, "failed to save app config on exit");
        } else {
            tracing::info!(reason, "saved app config on exit");
        }
        self.exit_configs_saved = true;
    }
}
