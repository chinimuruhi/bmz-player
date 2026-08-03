use super::*;

impl WinitApp {
    pub(super) fn start_chart_with_options(
        &mut self,
        chart_id: i64,
        mut options: PlayStartOptions,
    ) {
        self.result.last_play_was_autoplay = options.autoplay;
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
        if self.play.play_media_cache.as_ref().is_some_and(|cache| cache.chart_id != chart_id) {
            self.play.play_media_cache = None;
        }
        self.play.play_ending = None;
        self.result.result_exit = None;
        self.result.result_key5_held = false;
        self.result.result_key7_held = false;
        self.play.play_ready_sound_started_at = None;
        self.play.play_ready_last_control_hold_at = None;
        self.play.decide_sound_stopped_for_chart_start = false;
        if options.chart_zero_time == TimeUs(0) {
            options.chart_zero_time = self.play_skin_playstart_offset();
        }
        // 新しいプレイの音声出力を開く前に、前曲の余韻再生を止めて出力を解放する。
        self.audio.draining_audio = None;

        // Decide 演出中に preload worker が完成させていればそれを使う。
        // 譜面/音源は別スレッドでロード済みなので、ここでは音声出力 open 等の軽量処理だけ。
        // バッファが無ければ (course モード / preload 不発時) 従来通り main で同期ロードする。
        let opened = match self.play.preloaded_play_session.take() {
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
        self.play.play_stagefile_source = None;
        self.play.play_stagefile_loaded = false;
        self.play.play_stagefile_size = None;
    }

    pub(super) fn clear_play_backbmp_state(&mut self) {
        self.play.play_backbmp_source = None;
        self.play.play_backbmp_loaded = false;
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
        if self.play.play_stagefile_source.as_deref() == Some(stagefile_key.as_str()) {
            return;
        }
        self.play.play_stagefile_source = Some(stagefile_key);
        self.play.play_stagefile_size =
            load_chart_meta_texture(&mut self.renderer, SELECT_STAGE_TEXTURE, folder, relative);
        self.play.play_stagefile_loaded = self.play.play_stagefile_size.is_some();
    }

    pub(super) fn sync_play_backbmp_texture(&mut self, folder: &str, relative: &str) {
        let backbmp_key = format!("{folder}|{relative}");
        if self.play.play_backbmp_source.as_deref() == Some(backbmp_key.as_str()) {
            return;
        }
        self.play.play_backbmp_source = Some(backbmp_key);
        self.play.play_backbmp_loaded =
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
        self.result.result_ir = None;
        self.clear_result_ir_scroll_input();
        self.play.play_ending = None;
        self.result.result_exit = None;
        self.play.play_ready_sound_started_at = None;
        self.play.play_ready_last_control_hold_at = None;
        self.play.decide_sound_stopped_for_chart_start = false;
        self.play.active_play = None;
        self.clear_play_control_holds();
        // begin_decide_for_chart_with_snapshot で先行ロードした stagefile / backbmp は保持する。
        // boot / retry など Decide を通らない経路でも、この呼び出しで補完する。
        self.prepare_play_meta_image_textures(chart_id);
        self.result.finished_play = None;
        self.audio.draining_audio = None;
        self.play.play_scene_started_at = Instant::now();
        snapshot.arrange = options.arrange.as_str().to_string();
        snapshot.arrange_2p = options.arrange_2p.as_str().to_string();
        snapshot.play_elapsed_time = TimeUs(0);
        snapshot.ready_elapsed_time = None;
        snapshot.time = self.play_skin_playstart_offset();
        snapshot.stagefile_background = self.play.play_stagefile_loaded;
        snapshot.stagefile_image_size = self.play.play_stagefile_size;
        snapshot.backbmp_background = self.play.play_backbmp_loaded;
        let prepared_chart = self.play_preload_prepared_chart(chart_id);
        if let Some(prepared) = &prepared_chart {
            apply_prepared_chart_to_render_snapshot(
                &mut snapshot,
                &prepared.chart,
                &prepared.render_snapshot_cache,
                options.session_mode.is_battle(),
            );
        }
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
        // placeholder 初期値の算出には上で反映した正確な TOTAL / BPM を使い、
        // その後に chart 依存の派生値を実セッションと同じ値へ揃える。
        if let Some(prepared) = &prepared_chart {
            apply_prepared_chart_to_render_snapshot(
                &mut snapshot,
                &prepared.chart,
                &prepared.render_snapshot_cache,
                session_options.session_mode.is_battle(),
            );
        }
        // 譜面変換はWAVロードより先に完了する。preload workerが先行公開した
        // 実配置を使い、Play入場直後のロード画面からRANDOM refを表示する。
        if let Some(prepared) = &prepared_chart {
            apply_play_arrange_to_snapshot(&mut snapshot, &prepared.applied_arrange);
        }
        let mut pending_play_start = PendingPlayStart::from_snapshot(
            chart_id,
            options,
            &snapshot,
            &self.boot.profile_config,
            key_mode,
            session_options.gamepad_slots,
        );
        pending_play_start.prepared_chart_applied = prepared_chart.is_some();
        pending_play_start.lane.apply_to_snapshot(&mut snapshot);
        self.play.play_option_input = Some(PlayOptionInput::new(
            key_mode,
            pending_play_start.visual_input.binding.clone(),
            &self.boot.profile_config.input,
            session_options.gamepad_slots,
        ));
        self.capture_play_table_text_for_chart(chart_id);
        self.apply_course_skin_context(&mut snapshot);
        self.apply_play_table_text(&mut snapshot);
        self.play.last_play_snapshot = Some(snapshot.clone());
        self.play.pending_play_start = Some(pending_play_start);
        self.sync_play_control_holds_from_pressed_controls();
        self.play.last_started_chart_id = Some(chart_id);
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
        self.result.last_play_was_autoplay = active_play
            .running
            .session
            .autoplay
            .as_ref()
            .is_some_and(|autoplay| autoplay.is_full());
        if let Some(pending) =
            self.play.pending_play_start.as_ref().filter(|pending| pending.chart_id == chart_id)
        {
            let speed_locked = self.play.active_course.as_ref().is_some_and(|course| {
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
            self.play.bga_preload.matches_chart(chart_id, active_bga_assets);
        if self.play.bga_preload.chart_id == Some(chart_id) && !preload_matches_active_chart {
            tracing::warn!(
                chart_id,
                preloaded_assets = self.play.bga_preload.assets.as_ref().map_or(0, Vec::len),
                active_assets = active_bga_assets.len(),
                "discarding BGA preload because its asset manifest does not match the active chart"
            );
        }
        active_play.running.bga_frames = if preload_matches_active_chart {
            self.play.bga_preload.frames.clone()
        } else {
            self.start_chart_bga_texture_load_for_chart(
                chart_id,
                &active_play.running.session.chart,
            )
        };
        if let Some(cache) = self.play.play_media_cache.as_mut()
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
        snapshot.stagefile_background = self.play.play_stagefile_loaded;
        snapshot.stagefile_image_size = self.play.play_stagefile_size;
        snapshot.backbmp_background = self.play.play_backbmp_loaded;
        let play_elapsed_time = self.play_elapsed_time();
        snapshot.play_elapsed_time = play_elapsed_time;
        snapshot.ready_elapsed_time = self.play.play_ready_sound_started_at.map(elapsed_since);
        self.apply_course_skin_context(&mut snapshot);
        self.apply_play_table_text(&mut snapshot);
        crate::screens::play_snapshot::refresh_play_skin_visuals_with_input_elapsed(
            &mut snapshot,
            &active_play.running.session,
            play_elapsed_time,
        );
        self.play.last_play_snapshot = Some(snapshot);
        self.play.active_play = Some(active_play);
        // preload 経路では Play シーンへの遷移後にここで曲メタデータが確定する。
        // 曲情報なしで送った Presence を実際の譜面情報で置き換える。
        self.publish_discord_presence_for_scene(AppSceneKind::Play);
        self.update_play_exit_hold_timer();
    }
}
