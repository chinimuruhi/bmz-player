use super::*;

impl WinitApp {
    /// Key5/Key7 の現在の押下状態を記録する。フェードアウト中も含めて
    /// 常に呼び、終了アニメーション終了時に retry arrange を決める。
    pub(super) fn track_result_lane_hold(&mut self, control: &PhysicalControl, pressed: bool) {
        match self.result_lane_for_control(control) {
            Some(Lane::Key5) => self.result_key5_held = pressed,
            Some(Lane::Key7) => self.result_key7_held = pressed,
            _ => {}
        }
    }

    pub(super) fn handle_result_control(
        &mut self,
        control: &PhysicalControl,
        pressed: bool,
        repeat: bool,
    ) -> bool {
        if pressed
            && !repeat
            && self.result_input_ready()
            && self.result_panel == 1
            && self.result_ir_scope_toggle_is_e1()
            && self.is_result_ir_scope_toggle_control(control)
            && self.toggle_result_ir_scope()
        {
            return true;
        }
        if pressed
            && !repeat
            && self.result_input_ready()
            && self.select_result_panel_for_control(control)
        {
            return true;
        }
        if pressed
            && !repeat
            && self.result_input_ready()
            && self.is_result_panel_toggle_control(control)
            && self.toggle_result_panel()
        {
            return true;
        }
        let Some(lane) = self.result_lane_for_control(control) else {
            return false;
        };
        match lane {
            // ゲージグラフ種別の切り替え。
            Lane::Key6 => {
                if pressed && !repeat && self.result_input_ready() {
                    self.cycle_result_gauge_graph_type();
                }
                true
            }
            // Key1-4 / Key5 / Key7 の押下で終了アニメーションを開始する。
            // フェードアウト終了時の Key5/Key7 押下状態で retry か選曲へ戻るかを決める。
            lane if lane_starts_result_exit(lane) => {
                if pressed && self.result_input_ready() {
                    self.begin_result_exit(ResultExitAction::HeldLanes);
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn handle_course_result_control(
        &mut self,
        control: &PhysicalControl,
        pressed: bool,
        repeat: bool,
    ) -> bool {
        if pressed
            && !repeat
            && self.result_input_ready()
            && self.result_panel == 1
            && self.result_ir_scope_toggle_is_e1()
            && self.is_result_ir_scope_toggle_control(control)
            && self.toggle_result_ir_scope()
        {
            return true;
        }
        if pressed
            && !repeat
            && self.result_input_ready()
            && self.select_result_panel_for_control(control)
        {
            return true;
        }
        if pressed
            && !repeat
            && self.result_input_ready()
            && self.is_result_panel_toggle_control(control)
            && self.toggle_result_panel()
        {
            return true;
        }
        let Some(lane) = self.result_lane_for_control(control) else {
            return false;
        };
        match lane {
            Lane::Key6 => {
                if pressed && !repeat && self.result_input_ready() {
                    self.cycle_result_gauge_graph_type();
                }
                true
            }
            lane if lane_starts_result_exit(lane) => {
                if pressed && self.result_input_ready() {
                    self.begin_result_exit(ResultExitAction::HeldCourseLanes);
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn save_finished_play_replay_slot(&mut self, slot: u8) -> bool {
        let Some(finished) = self.finished_play.as_mut() else {
            return false;
        };
        let saved = match crate::storage::play_result::save_existing_replay_to_slot(
            &mut self.boot.score_db,
            &self.boot.profile_paths,
            &finished.result,
            &finished.stored,
            finished.ln_policy,
            finished.double_option,
            finished.rule_mode,
            slot,
        ) {
            Ok(Some(path)) => {
                finished.stored.slot_paths[slot as usize] = Some(path);
                finished.summary.saved_replay_slots[slot as usize] = true;
                finished.summary.replay_slots[slot as usize] = true;
                true
            }
            Ok(None) => false,
            Err(error) => {
                tracing::warn!(%error, slot, "failed to save replay slot from result");
                false
            }
        };
        if saved {
            self.notify_obs_save_recording(crate::obs::ObsRecordingSaveReason::OnReplay);
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            tracing::info!(slot, "saved result replay slot");
        } else {
            tracing::info!(slot, "result replay slot was not saved");
        }
        saved
    }

    pub(super) fn save_finished_course_replay_slot(&mut self, slot: u8) -> bool {
        let Some(course_id) = self.finished_course.as_ref().map(|course| course.course_id) else {
            return false;
        };
        let Some(course_hash) = self.finished_course_hash.clone() else {
            tracing::warn!(course_id, slot, "course identity unavailable for replay slot save");
            return false;
        };
        let Some(course) = self.finished_course.as_mut() else {
            return false;
        };
        let Some(course_score_id) = course.course_score_id else {
            tracing::info!(slot, "course replay slot unavailable without persisted course score");
            return false;
        };
        if slot > 3 {
            return false;
        }
        let max_combo = course.course_max_combo;
        let clear_rank = if course.course_clear {
            bmz_core::clear::ClearType::Normal as u8
        } else if course.course_failed {
            bmz_core::clear::ClearType::Failed as u8
        } else {
            bmz_core::clear::ClearType::NoPlay as u8
        };
        let played_at = course.course_played_at.unwrap_or(0);
        let rule_mode = course.rule_mode;
        let record = crate::storage::score_db::CourseReplaySlotRecord {
            course_hash: course_hash.clone(),
            rule_mode,
            slot,
            rule: crate::config::profile_config::ReplaySlotRule::Always.as_str().to_string(),
            course_score_id,
            played_at,
            ex_score: course.total_ex_score,
            bp: course.bp,
            max_combo,
            clear_rank,
        };
        match self.boot.score_db.upsert_course_replay_slot(&record) {
            Ok(()) => {
                mark_course_replay_slot_saved(
                    course,
                    self.finished_course_skin_summary.as_mut(),
                    slot as usize,
                );
                self.notify_obs_save_recording(crate::obs::ObsRecordingSaveReason::OnReplay);
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                tracing::info!(
                    course_id,
                    course_hash = %course_hash,
                    rule_mode = rule_mode.as_str(),
                    slot,
                    "saved course replay slot"
                );
                true
            }
            Err(error) => {
                tracing::warn!(
                    %error,
                    course_id,
                    course_hash = %course_hash,
                    slot,
                    "failed to save course replay slot from result"
                );
                false
            }
        }
    }

    pub(super) fn result_lane_for_control(&self, control: &PhysicalControl) -> Option<Lane> {
        if let Some(control_name) = physical_control_name(control)
            && let Some(lane) = self.select_keys.ui_lane_for_control(control_name)
        {
            return Some(lane);
        }
        let key_mode = self.finished_play.as_ref()?.summary.key_mode;
        crate::config::play::lane_binding_for_chart(&self.boot.profile_config.input, key_mode)
            .resolve(DeviceId(0), control)
    }

    pub(super) fn is_result_panel_toggle_control(&self, control: &PhysicalControl) -> bool {
        physical_control_name(control)
            .is_some_and(|control| control == "Select" || self.select_keys.is_e2_action(control))
    }

    pub(super) fn select_result_panel_for_control(&mut self, control: &PhysicalControl) -> bool {
        result_panel_for_control(control).is_some_and(|panel| self.set_result_panel(panel))
    }

    pub(super) fn toggle_result_panel(&mut self) -> bool {
        let Some(document) = self.renderer.result_skin_document() else {
            return false;
        };
        let Some(requested) = toggled_result_panel(
            self.result_panel,
            result_panel_supported(document),
            self.result_ir.is_some(),
        ) else {
            return false;
        };
        self.set_result_panel(requested)
    }

    pub(super) fn set_result_panel(&mut self, requested: i32) -> bool {
        let Some(document) = self.renderer.result_skin_document() else {
            return false;
        };
        let Some(panel) = selected_result_panel(
            self.result_panel,
            requested,
            result_panel_supported(document),
            self.result_ir.is_some(),
        ) else {
            return false;
        };
        self.result_panel = panel;
        tracing::info!(panel = self.result_panel, "result panel changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
        true
    }

    /// `resultIrScopeBinding=active` を宣言したスキンだけが Result IR scope を切り替える。
    /// 既存スキンの standard IR ref は常に global のままにする。
    pub(super) fn select_result_ir_scope(
        &mut self,
        tab: crate::screens::result_ir::ResultRankingTab,
    ) -> bool {
        let Some(document) = self.renderer.result_skin_document() else {
            return false;
        };
        if document.result_ir_scope_binding != bmz_render::skin::ResultIrScopeBinding::Active {
            return false;
        }
        let Some(result_ir) = &mut self.result_ir else {
            return false;
        };
        if !result_ir.supports_tab(tab) || result_ir.active_tab == tab {
            return false;
        }
        result_ir.select_tab(tab);
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
        true
    }

    pub(super) fn toggle_result_ir_scope(&mut self) -> bool {
        let Some(document) = self.renderer.result_skin_document() else {
            return false;
        };
        if document.result_ir_scope_binding != bmz_render::skin::ResultIrScopeBinding::Active {
            return false;
        }
        let Some(result_ir) = &self.result_ir else {
            return false;
        };
        let next = match result_ir.active_tab {
            crate::screens::result_ir::ResultRankingTab::Global => {
                crate::screens::result_ir::ResultRankingTab::SelfAndRivals
            }
            crate::screens::result_ir::ResultRankingTab::SelfAndRivals => {
                crate::screens::result_ir::ResultRankingTab::Global
            }
        };
        self.select_result_ir_scope(next)
    }

    pub(super) fn result_ir_scope_toggle_is_e1(&self) -> bool {
        self.renderer.result_skin_document().is_some_and(|document| {
            document.result_ir_scope_binding == bmz_render::skin::ResultIrScopeBinding::Active
                && document.result_ir_scope_toggle == bmz_render::skin::ResultIrScopeToggle::E1Press
        })
    }

    pub(super) fn is_result_ir_scope_toggle_control(&self, control: &PhysicalControl) -> bool {
        physical_control_name(control).is_some_and(|name| {
            self.select_keys.is_start(name)
                || self.select_keys.e_action_for_control(name) == Some(InputActionConfig::E1)
        })
    }

    /// `selectIrScopeBinding=active` を宣言したスキンだけが Select IR scope を切り替える。
    /// 既存スキンの standard IR ref は常に global のままにする。
    pub(super) fn select_select_ir_scope(
        &mut self,
        scope: crate::screens::select_ir::SelectIrRankingScope,
    ) -> bool {
        let Some(document) = self.renderer.select_skin_document() else {
            return false;
        };
        if document.select_ir_scope_binding != bmz_render::skin::IrScopeBinding::Active {
            return false;
        }
        if !self.select_ir.select_scope(
            &self.boot.profile_config.ir,
            self.selected_chart_sha256(),
            scope,
        ) {
            return false;
        }
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
        true
    }

    pub(super) fn toggle_select_ir_scope(&mut self) -> bool {
        let Some(document) = self.renderer.select_skin_document() else {
            return false;
        };
        if document.select_ir_scope_binding != bmz_render::skin::IrScopeBinding::Active {
            return false;
        }
        if !self.select_ir.toggle_scope(&self.boot.profile_config.ir, self.selected_chart_sha256())
        {
            return false;
        }
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
        true
    }

    pub(super) fn select_ir_scope_toggle_is_e3(&self) -> bool {
        self.renderer.select_skin_document().is_some_and(|document| {
            document.select_ir_scope_binding == bmz_render::skin::IrScopeBinding::Active
                && document.select_ir_scope_toggle == bmz_render::skin::SelectIrScopeToggle::E3Press
        })
    }

    pub(super) fn is_select_ir_scope_toggle_control(&self, control: &str) -> bool {
        self.select_keys.e_action_for_control(control) == Some(InputActionConfig::E3)
    }

    pub(super) fn cycle_result_gauge_graph_type(&mut self) {
        self.result_gauge_graph_type = cycle_result_gauge_graph_type(self.result_gauge_graph_type);
        tracing::info!(
            gauge_type = self.result_gauge_graph_type,
            "result gauge graph type changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    /// リザルト画面の終了アニメーションを開始する。
    /// 通常はスキンが宣言するフェードアウト時間が経過したら、スキップ要求時は
    /// timer=2 の実アニメーションが終わって最終フレームを保持したら、
    /// `advance_result_exit` が実際の遷移 (選曲へ戻る / リトライ) を実行する。
    pub(super) fn begin_result_exit(&mut self, action: ResultExitAction) {
        if self.result_exit.is_some() || self.finished_play.is_none() {
            return;
        }
        tracing::info!(?action, "result screen exit animation started");
        self.result_exit = Some(ResultExit {
            started_at: Instant::now(),
            action,
            skip_requested: false,
            skip_final_frame_held: false,
        });
        let (skin_bgm_volume, skin_se_volume) = self.result_skin_audio_volumes();
        let dispatched = self
            .result_skin_audio
            .as_mut()
            .is_some_and(|audio| audio.trigger_timer(2, skin_bgm_volume, skin_se_volume));
        if dispatched {
            self.start_audio_output_stream();
        }
        // HeldLanes の遷移判定はフェードアウト終了時に Key5/Key7 の
        // 押下状態を読むため、ここでは held フラグをリセットしない。
        // Result SE は毎フレームの master-gain command ではなく callback 側で
        // fade-out させ、ASIO の小さい buffer でも段差が出にくいようにする。
        let fadeout = Duration::from_millis(self.renderer.result_skin_fadeout_ms().max(0) as u64);
        let fade_out_frames = self.result_exit_audio_fade_frames(fadeout);
        self.fade_result_entry_system_sounds(fade_out_frames);
        self.play_result_close_sound_with_fade_out(fade_out_frames);
    }

    pub(super) fn request_result_exit_skip_for_key(
        &mut self,
        physical_key: PhysicalKey,
        state: ElementState,
        repeat: bool,
    ) -> bool {
        if result_exit_skip_key(physical_key, state, repeat) {
            return self.request_result_exit_skip();
        }
        false
    }

    pub(super) fn request_result_exit_skip_for_control(
        &mut self,
        control: &PhysicalControl,
        pressed: bool,
        repeat: bool,
    ) -> bool {
        if pressed && !repeat && self.result_exit_skip_control(control) {
            return self.request_result_exit_skip();
        }
        false
    }

    pub(super) fn result_exit_skip_control(&self, control: &PhysicalControl) -> bool {
        self.result_lane_for_control(control).is_some_and(lane_skips_result_exit)
    }

    pub(super) fn request_result_exit_skip(&mut self) -> bool {
        let Some(exit) = self.result_exit.as_mut() else {
            return false;
        };
        if !exit.skip_requested {
            tracing::info!("result screen exit animation skip requested");
        }
        exit.skip_requested = true;
        true
    }

    pub(super) fn begin_decide_fadeout(&mut self, cancel: bool) {
        if self.pending_decide.is_none() {
            return;
        }
        self.clear_play_control_holds();
        let Some(decide) = &mut self.pending_decide else {
            return;
        };
        if decide.fadeout_started_at.is_some() {
            return;
        }
        decide.cancel = cancel;
        decide.fadeout_started_at = Some(Instant::now());
    }

    pub(super) fn advance_decide_transition(&mut self) {
        let Some(fadeout_started) =
            self.pending_decide.as_ref().map(|decide| decide.fadeout_started_at.is_some())
        else {
            return;
        };
        if !fadeout_started && self.cancel_decide_if_exit_hold_elapsed() {
            return;
        }
        let Some(decide) = &self.pending_decide else {
            return;
        };
        if decide.fadeout_started_at.is_none()
            && decide.started_at.elapsed() >= self.decide_scene_duration()
        {
            self.begin_decide_fadeout(false);
            return;
        }

        let Some(fadeout_started_at) =
            self.pending_decide.as_ref().and_then(|d| d.fadeout_started_at)
        else {
            return;
        };
        if fadeout_started_at.elapsed() < self.decide_fadeout_duration() {
            return;
        }

        if !decide.cancel && !self.decide_play_start_ready() {
            return;
        }

        let Some(decide) = self.pending_decide.take() else {
            return;
        };
        if decide.cancel {
            self.invalidate_play_preload();
            // Decide screen cancel (Escape) returns to select.  If a course
            // was being started, drop the course session — the user opted
            // out before the first chart actually began.
            self.clear_active_course_state();
            self.autoplay_folder = None;
            let now = Instant::now();
            self.select_scene_started_at = now;
            self.restart_select_bar_timer_without_scroll(now);
        } else {
            self.enter_play_scene(decide.chart_id, decide.options, decide.snapshot);
        }
    }

    pub(super) fn decide_play_start_ready(&self) -> bool {
        // preload (WAV ロード等) の完了は待たない。Play 画面へ先に入場し、
        // ロード完了後に poll_play_preload が active_play を install して
        // READY タイマーが始まる。
        !self.skin_pipeline.is_pending(SkinKind::Play)
    }

    pub(super) fn update_decide_cancel_control_state(
        &mut self,
        control: &str,
        pressed: bool,
    ) -> bool {
        let mut handled = false;
        if self.select_keys.is_start(control) {
            self.decide_e1_held = pressed;
            handled = true;
        }
        if self.select_keys.is_e2_action(control) {
            self.play_e2_held = pressed;
            handled = true;
        }
        if self.select_keys.is_e3_action(control) {
            self.play_e3_held = pressed;
            handled = true;
        }
        if !handled {
            return false;
        }
        update_play_exit_hold_started_at(
            &mut self.play_exit_hold_started_at,
            self.decide_e1_held,
            self.play_e2_held,
            Instant::now(),
        );
        if pressed
            && decide_cancel_chord_pressed(
                self.decide_e1_held,
                self.play_e2_held,
                self.play_e3_held,
            )
        {
            self.begin_decide_fadeout(true);
            return true;
        }
        true
    }

    pub(super) fn cancel_decide_if_exit_hold_elapsed(&mut self) -> bool {
        let hold_duration =
            Duration::from_millis(self.boot.profile_config.play.play_exit_hold_ms as u64);
        if play_exit_hold_elapsed(self.play_exit_hold_started_at, Instant::now(), hold_duration) {
            self.begin_decide_fadeout(true);
            return true;
        }
        false
    }

    pub(super) fn advance_play_ending(&mut self) {
        let Some(ending) = &self.play_ending else {
            return;
        };
        if ending.failed {
            if ending.started_at.elapsed() >= self.play_close_duration() {
                self.finish_play_ending();
            }
            return;
        }

        if ending.fadeout_started_at.is_none()
            && ending.started_at.elapsed() >= self.play_pre_fadeout_duration(ending)
        {
            if let Some(ending) = &mut self.play_ending {
                ending.fadeout_started_at = Some(Instant::now());
            }
            return;
        }

        let Some(fadeout_started_at) = self.play_ending.as_ref().and_then(|e| e.fadeout_started_at)
        else {
            return;
        };
        if fadeout_started_at.elapsed() >= self.play_fadeout_duration() {
            self.finish_play_ending();
        }
    }

    pub(super) fn finish_play_ending(&mut self) {
        let Some(mut ending) = self.play_ending.take() else {
            return;
        };
        let Some(mut started) = self.active_play.take() else {
            return;
        };
        let finished = match ending.finished.take() {
            Some(finished) => finished,
            None => {
                match crate::screens::play_finish::finish_session_result_once(
                    &mut started.running.finished,
                    &mut self.boot.score_db,
                    &mut self.boot.network_db,
                    crate::screens::play_finish::FinishSessionResultOnceRequest {
                        profile_paths: &self.boot.profile_paths,
                        replay_config: &self.boot.profile_config.replay,
                        ir_config: &self.boot.profile_config.ir,
                        session: &started.running.session,
                        played_at: now_unix_seconds(),
                        applied_arrange: &started.running.applied_arrange,
                        target_ex_score: started.running.target_ex_score,
                        target_name: &started.running.target,
                        score_key: started.running.score_key,
                        practice_mode: started.running.practice_mode,
                        finish_mode: if self.active_course.is_some() {
                            crate::screens::play_finish::FinishResultMode::CourseStage
                        } else {
                            crate::screens::play_finish::FinishResultMode::Normal
                        },
                    },
                ) {
                    Ok(mut finished) => {
                        finished.summary.graph = Arc::new(
                            started
                                .running
                                .result_graph
                                .snapshot_for_session(&started.running.session),
                        );
                        finished
                    }
                    Err(error) => {
                        tracing::error!(%error, "failed to finish play session");
                        if let Some(chart_id) = self.last_started_chart_id {
                            self.capture_play_media_cache_from_running(
                                chart_id,
                                &mut started.running,
                            );
                        }
                        let mut audio = started.running.audio;
                        audio.mark_draining();
                        self.draining_audio = Some(audio);
                        self.refresh_player_stats_snapshot();
                        self.leave_result();
                        return;
                    }
                }
            }
        };
        if let Some(chart_id) = self.last_started_chart_id {
            self.capture_play_media_cache_from_running(chart_id, &mut started.running);
        }
        let mut audio = started.running.audio;
        audio.mark_draining();
        self.draining_audio = Some(audio);
        self.refresh_player_stats_snapshot();
        if self.active_course.is_some() {
            self.advance_course_after_finish(finished);
            return;
        }
        if finished.stored.slot_paths.iter().any(Option::is_some) {
            self.notify_obs_save_recording(crate::obs::ObsRecordingSaveReason::OnReplay);
        }
        self.finished_play = Some(finished);
        self.result_gauge_graph_type = self
            .finished_play
            .as_ref()
            .map(|finished| finished.summary.gauge_type as i32)
            .unwrap_or(GaugeType::Normal as i32);
        self.result_key5_held = false;
        self.result_key7_held = false;
        self.result_scene_started_at = Instant::now();
        self.ensure_result_skin_ready(ResultSkinSlot::Normal);
    }

    /// 終了フェードアウトの経過を監視し、通常はスキンのフェードアウト時間を、
    /// スキップ時は timer=2 の実アニメーション終端と最終フレーム保持を過ぎたら
    /// 保留していた遷移を実行する。毎フレーム描画前に呼ぶ。
    pub(super) fn advance_result_exit(&mut self) {
        if self.finished_play.is_some()
            && self.result_exit.is_none()
            && let Some(auto_exit_duration) = self.result_auto_exit_duration()
            && self.result_scene_started_at.elapsed() >= auto_exit_duration
        {
            // 中間リザルトは scene 時間経過で次の曲へ、それ以外は選曲へ戻る。
            let action = if self.is_course_intermediate_result() {
                self.course_intermediate_exit_action()
            } else if self.autoplay_folder_has_next() {
                ResultExitAction::AdvanceAutoplayFolder
            } else {
                ResultExitAction::Leave
            };
            self.begin_result_exit(action);
        }
        let Some(exit) = self.result_exit.as_ref() else {
            return;
        };
        // 何らかの理由でリザルトを抜けていたら終了状態を破棄する。
        if self.finished_play.is_none() {
            self.stop_result_exit_system_sounds();
            if let Some(audio) = &self.result_skin_audio {
                audio.stop_all();
            }
            self.result_exit = None;
            return;
        }
        let started_at = exit.started_at;
        let action = exit.action.clone();
        let skip_requested = exit.skip_requested;
        let skip_final_frame_held = exit.skip_final_frame_held;
        let fadeout = Duration::from_millis(self.renderer.result_skin_fadeout_ms().max(0) as u64);
        let animation_duration = Duration::from_millis(
            self.renderer.result_skin_timer_animation_duration_ms(2).max(0) as u64,
        );
        let elapsed = started_at.elapsed();
        // スキンの終了アニメーション時間に合わせて、プレイ残響(draining_audio)を
        // 1.0 → 0.0 へ絞る。リザルトSEは begin_result_exit で callback 側の
        // fade-out を開始済みなので、ここでは毎フレーム command を投げない。
        self.fade_audio_for_result_exit(elapsed, fadeout);
        if skip_requested && elapsed >= animation_duration && !skip_final_frame_held {
            // この呼び出しでは遷移せず、次のフレームで最終状態を1フレーム描画する。
            if let Some(exit) = self.result_exit.as_mut() {
                exit.skip_final_frame_held = true;
            }
            return;
        }
        if !result_exit_transition_ready(
            elapsed,
            fadeout,
            animation_duration,
            skip_requested,
            skip_final_frame_held,
        ) {
            return;
        }
        self.stop_result_exit_system_sounds();
        self.result_exit = None;
        match action {
            ResultExitAction::Leave => self.leave_result(),
            ResultExitAction::Retry(mode) => self.retry_last_chart_with_mode(mode),
            ResultExitAction::HeldLanes => {
                match result_action_for_held_lanes(self.result_key5_held, self.result_key7_held) {
                    Some(mode) => self.retry_last_chart_with_mode(mode),
                    None => self.leave_result(),
                }
            }
            ResultExitAction::RetryCourseSameArrange => self.retry_course_same_arrange(),
            ResultExitAction::HeldCourseLanes => {
                match result_action_for_held_lanes(self.result_key5_held, self.result_key7_held) {
                    Some(ResultRetryMode::SameArrange) => self.retry_course_same_arrange(),
                    Some(ResultRetryMode::DifferentArrange) => {
                        self.retry_course_different_arrange()
                    }
                    None => self.leave_result(),
                }
            }
            ResultExitAction::AdvanceCourse => self.advance_to_next_course_chart(),
            ResultExitAction::FinishCourse => self.finish_course_after_intermediate_result(),
            ResultExitAction::AdvanceAutoplayFolder => self.advance_autoplay_folder(),
        }
    }

    pub(super) fn stop_result_exit_system_sounds(&self) {
        for sound_type in result_exit_system_sounds() {
            self.stop_system_sound(*sound_type);
        }
    }

    /// リザルト終了アニメ中、プレイ残響(draining_audio)のマスターゲインを
    /// 1.0 → 0.0 へランプする。毎フレーム呼ぶ。
    /// フェード時間は `RESULT_EXIT_AUDIO_FADE` を上限とし、スキンの終了アニメ時間
    /// (`fadeout`) がそれより短ければ遷移前に絞り切れるよう短い方を採用する。
    /// 見た目の遷移タイミング自体は `fadeout` のまま変えない。
    pub(super) fn fade_audio_for_result_exit(&mut self, elapsed: Duration, fadeout: Duration) {
        let gain = result_exit_audio_gain(elapsed, fadeout);
        if let Some(audio) = &self.draining_audio {
            audio.engine.set_master_gain(gain);
        }
    }

    pub(super) fn result_exit_audio_fade_frames(&self, fadeout: Duration) -> u32 {
        duration_to_frames(
            result_exit_audio_fade_duration(fadeout),
            self.system_audio_sample_rate(),
        )
    }

    pub(super) fn system_audio_sample_rate(&self) -> u32 {
        self.audio_runtime.as_ref().map(AudioRuntime::sample_rate).unwrap_or(48_000).max(1)
    }

    pub(super) fn fade_result_entry_system_sounds(&self, fade_out_frames: u32) {
        use crate::system_sound::SoundType;
        let Some(manager) = &self.system_sound else {
            return;
        };
        for sound_type in [
            SoundType::ResultClear,
            SoundType::ResultFail,
            SoundType::CourseClear,
            SoundType::CourseFail,
        ] {
            manager.stop_with_fade_out(sound_type, fade_out_frames);
        }
    }

    pub(super) fn play_result_close_sound_with_fade_out(&self, fade_out_frames: u32) {
        let Some(manager) = &self.system_sound else {
            return;
        };
        let sound_type = result_exit_sound_for_context(
            self.active_course.is_some() || self.finished_course.is_some(),
            manager.has_sound(crate::system_sound::SoundType::CourseClose),
        );
        manager.play_with_master_gain_and_fade_out(
            sound_type,
            system_sound_volume_from_mix(&self.boot.profile_config.audio_mix, sound_type),
            1.0,
            fade_out_frames,
        );
        self.start_audio_output_stream();
    }

    pub(super) fn leave_result(&mut self) {
        let score_changed = self
            .finished_play
            .as_ref()
            .is_some_and(|finished| finished.stored.score_history_id > 0);
        if let Some(audio) = &self.result_skin_audio {
            audio.stop_all();
        }
        self.finished_play = None;
        self.autoplay_folder = None;
        self.result_favorite_chart = false;
        self.clear_active_course_state();
        self.result_exit = None;
        self.result_key5_held = false;
        self.result_key7_held = false;
        self.clear_play_meta_image_state();
        // リザルト画面を抜けたら、まだ鳴っていても余韻再生を止める。
        self.draining_audio = None;
        self.play_media_cache = None;
        self.last_play_snapshot = None;
        if score_changed {
            self.invalidate_select_folder_summaries();
        }
        self.reload_select_items();
        self.sync_select_holds_from_pressed_controls();
        let now = Instant::now();
        self.select_scene_started_at = now;
        self.restart_select_bar_timer_without_scroll(now);
    }

    pub(super) fn decide_scene_duration(&self) -> Duration {
        skin_duration_ms(self.renderer.decide_skin_document().map(|d| d.scene).unwrap_or(0))
    }

    pub(super) fn decide_fadeout_duration(&self) -> Duration {
        skin_duration_ms(self.renderer.decide_skin_document().map(|d| d.fadeout).unwrap_or(0))
    }

    pub(super) fn decide_fadeout_scene_timing(&self) -> DecideFadeoutSceneTiming {
        decide_fadeout_scene_timing(self.renderer.decide_skin_document())
    }

    pub(super) fn play_finishmargin_duration(&self) -> Duration {
        skin_duration_ms(self.renderer.play_skin_document().map(|d| d.finishmargin).unwrap_or(0))
    }

    pub(super) fn play_pre_fadeout_duration(&self, ending: &PlayEndingTransition) -> Duration {
        let finishmargin = self.play_finishmargin_duration();
        let Some(elapsed_ms) = ending.full_combo_elapsed_at_finish_ms else {
            return finishmargin;
        };
        let full_combo_ms = self
            .renderer
            .play_skin_timer_animation_duration_ms(48)
            .max(self.renderer.play_skin_timer_animation_duration_ms(49));
        let remaining_ms = full_combo_ms.saturating_sub(elapsed_ms.max(0));
        finishmargin.max(skin_duration_ms(remaining_ms))
    }

    pub(super) fn play_fadeout_duration(&self) -> Duration {
        skin_duration_ms(self.renderer.play_skin_document().map(|d| d.fadeout).unwrap_or(0))
    }

    pub(super) fn play_close_duration(&self) -> Duration {
        skin_duration_ms(self.renderer.play_skin_document().map(|d| d.close).unwrap_or(0))
    }

    pub(super) fn result_input_ready(&self) -> bool {
        self.result_scene_started_at.elapsed() >= self.result_input_duration()
    }

    pub(super) fn result_input_duration(&self) -> Duration {
        result_input_duration_for_document(self.renderer.result_skin_document())
    }

    pub(super) fn result_auto_exit_duration(&self) -> Option<Duration> {
        let duration = result_auto_exit_duration_for_document(
            self.renderer.result_skin_document(),
            self.is_course_intermediate_result(),
            self.course_intermediate_auto_advance_enabled(),
        );
        if duration.is_none() && self.autoplay_folder_has_next() {
            Some(FALLBACK_RESULT_SCENE_DURATION)
        } else {
            duration
        }
    }
}
