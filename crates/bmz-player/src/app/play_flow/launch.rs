use super::*;

impl WinitApp {
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
        self.play.pending_decide = Some(DecideTransition {
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
        self.play.play_preload_generation = self.play.play_preload_generation.wrapping_add(1);
        let generation = self.play.play_preload_generation;
        self.play.preloaded_play_session = None;
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
        self.play.pending_play_preload = Some(PendingPlayPreload {
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
        self.play.play_preload_generation = self.play.play_preload_generation.wrapping_add(1);
        self.play.pending_play_preload = None;
        // 裏で完成して退避していた結果も無効化する (decide キャンセル / 譜面差し替え)。
        self.play.preloaded_play_session = None;
        self.invalidate_chart_bga_texture_preload();
    }

    /// select_items に持っている `ChartListItem.mode` から KeyMode を引く。
    /// コース行から開始した譜面など select_items に Chart 行が無い場合は DB を参照し、
    /// 未知 / 見つからない場合だけデフォルトの 7K を返す。
    pub(super) fn key_mode_for_chart(&self, chart_id: i64) -> KeyMode {
        if let Some(key_mode) = self
            .select
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
        let runtime = self.audio.audio_runtime.as_ref().context("audio output is not available")?;
        open_prepared_winit_play_session(&self.boot.score_db, runtime, prepared)
    }

    pub(super) fn play_output_sample_rate(&self) -> u32 {
        self.audio
            .audio_runtime
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
    pub(super) fn decide_snapshot_for_chart(&self, chart_id: i64) -> RenderSnapshot {
        let mut snapshot = RenderSnapshot::default();
        let metadata = chart_snapshot_metadata_for_chart(
            &self.select.select_items,
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

    pub(super) fn start_chart_bga_texture_preload(
        &mut self,
        chart_id: i64,
        options: PlayStartOptions,
    ) {
        let generation = self.play.bga_preload.begin_unresolved(chart_id);
        let Some(uploader) = self.renderer.gpu_uploader() else {
            tracing::warn!(chart_id, "skipping BGA preload because GPU uploader is unavailable");
            self.play.bga_preload.status = BgaImageLoadStatus::skipped(generation, chart_id);
            return;
        };

        let library_db_path = self.boot.app_paths.library_db.clone();
        let app_config = self.play_session_app_config();
        thread::Builder::new()
            .name(format!("bga-image-load-{chart_id}"))
            .spawn({
                let (tx, rx) = bounded_gpu_upload_channel(MAX_PENDING_BGA_TEXTURE_UPLOADS);
                self.play.bga_preload.rx = Some(rx);
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
        self.play.bga_preload.invalidate();
    }

    pub(super) fn start_chart_bga_texture_load_for_chart(
        &mut self,
        chart_id: i64,
        chart: &PlayableChart,
    ) -> BgaFrameCatalog {
        let generation = self.play.bga_preload.begin_chart(chart_id, chart.bga_assets.clone());
        let static_asset_count = chart
            .bga_assets
            .iter()
            .filter(|asset| asset.kind == bmz_chart::model::BgaAssetKind::Static)
            .count();
        if static_asset_count == 0 {
            self.play.bga_preload.status = BgaImageLoadStatus::ready(generation, chart_id);
            return BgaFrameCatalog::new();
        }
        let Some(uploader) = self.renderer.gpu_uploader() else {
            tracing::warn!("loading BGA images synchronously because GPU uploader is unavailable");
            let frames = load_chart_bga_textures(&mut self.renderer, chart);
            self.play.bga_preload.completed_assets = self.play.bga_preload.total_assets;
            self.play.bga_preload.status = BgaImageLoadStatus::ready(generation, chart_id);
            return frames;
        };

        let assets = chart.bga_assets.clone();
        let (tx, rx) = bounded_gpu_upload_channel(MAX_PENDING_BGA_TEXTURE_UPLOADS);
        thread::Builder::new()
            .name("bga-image-load".to_string())
            .spawn(move || chart_bga_texture_load_worker(generation, assets, tx, uploader))
            .expect("failed to spawn BGA image load thread");
        self.play.bga_preload.rx = Some(rx);
        tracing::info!(chart_id, generation, "BGA image preload started");
        BgaFrameCatalog::new()
    }

    pub(super) fn poll_chart_bga_texture_load(&mut self) {
        let Some(rx) = self.play.bga_preload.rx.take() else {
            return;
        };
        let mut keep_rx = true;
        for _ in 0..MAX_BGA_TEXTURE_RESULTS_PER_REDRAW {
            match rx.try_recv() {
                Ok(PendingBgaImageResult::Manifest { generation, assets }) => {
                    if generation != self.play.bga_preload.generation {
                        continue;
                    }
                    self.play.bga_preload.total_assets = assets
                        .iter()
                        .filter(|asset| asset.kind == bmz_chart::model::BgaAssetKind::Static)
                        .count()
                        .min(u32::MAX as usize)
                        as u32;
                    self.play.bga_preload.completed_assets = 0;
                    self.play.bga_preload.assets = Some(assets);
                }
                Ok(PendingBgaImageResult::Loaded(image)) => {
                    if image.generation != self.play.bga_preload.generation {
                        continue;
                    }
                    self.play.bga_preload.completed_assets =
                        self.play.bga_preload.completed_assets.saturating_add(1);
                    self.renderer.insert_prepared_texture(image.texture_id, image.prepared);
                    self.play.bga_preload.frames.insert(
                        image.asset_id,
                        display_bga_frame(image.asset_id, image.width, image.height),
                    );
                    if let Some(active_play) = &mut self.play.active_play {
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
                    if generation != self.play.bga_preload.generation {
                        continue;
                    }
                    self.play.bga_preload.completed_assets =
                        self.play.bga_preload.completed_assets.saturating_add(1);
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
                    if generation != self.play.bga_preload.generation {
                        continue;
                    }
                    self.play.bga_preload.status = BgaImageLoadStatus::failed(generation, chart_id);
                    tracing::warn!(chart_id, error, "BGA image preload failed");
                    keep_rx = false;
                    break;
                }
                Ok(PendingBgaImageResult::Finished { generation, stats }) => {
                    if generation == self.play.bga_preload.generation {
                        self.play.bga_preload.completed_assets = self.play.bga_preload.total_assets;
                        if let Some(chart_id) = self.play.bga_preload.chart_id {
                            self.play.bga_preload.status =
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
                    if let Some(chart_id) = self.play.bga_preload.chart_id {
                        self.play.bga_preload.status =
                            BgaImageLoadStatus::failed(self.play.bga_preload.generation, chart_id);
                    }
                    keep_rx = false;
                    break;
                }
            }
        }
        if keep_rx {
            self.play.bga_preload.rx = Some(rx);
        }
    }

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
