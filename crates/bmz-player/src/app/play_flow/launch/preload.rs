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
        // Pre-import placeholder only: resolve the same AUTO fallback / FORCE
        // priority as preload and account for Battle here. The running session
        // replaces this with a count derived from the imported source chart.
        if let Ok(charts) = self.boot.library_db.list_charts_by_ids(&[chart_id])
            && let Some(chart) = charts.first()
        {
            let policy = crate::ln_policy::course_score_ln_policy(
                self.boot.profile_config.play.ln_mode_policy,
                options.ln_mode_override,
                chart.ln_profile,
            );
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
        // 実際の Play 入場で `ensure_skin_ready` が保険として残る。
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

    pub(super) fn start_play_preload(&mut self, chart_id: i64, options: PlayStartOptions) -> u64 {
        // 通常開始・practice・retry は、残っているコース次曲先読みを置き換える。
        // コース側は worker 開始後に同じ generation の launch 情報を設定し直す。
        self.play.pending_course_stage_launch = None;
        self.play.play_preload_generation = self.play.play_preload_generation.wrapping_add(1);
        let generation = self.play.play_preload_generation;
        self.play.preloaded_play_session = None;
        let (tx, rx) = mpsc::channel();
        let library_db_path = self.boot.app_paths.library_db.clone();
        let app_config = self.play_session_app_config();
        let ln_policy_setting = self.boot.profile_config.play.ln_mode_policy;
        let rule_mode = self.boot.profile_config.play.rule_mode;
        let input = SharedInputBackend::default();
        let preload_input = input.clone();
        let audio_progress = Arc::new(AtomicU32::new(0));
        let worker_audio_progress = Arc::clone(&audio_progress);
        let prepared_chart = Arc::new(OnceLock::new());
        let worker_prepared_chart = Arc::clone(&prepared_chart);
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
                            |chart| {
                                let _ = worker_prepared_chart.set(chart.clone());
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
            prepared_chart,
            rx,
        });
        // 譜面変換結果を受け取ってから、その同じ chart manifest で BMP/BGA を開始する。
        // ここでは対象だけを予約し、従来の BGA worker による BMS 二重 parse は行わない。
        self.play.bga_preload.begin_unresolved(chart_id);
        tracing::info!(chart_id, generation, "play preload started");
        generation
    }

    pub(super) fn invalidate_play_preload(&mut self) {
        self.play.play_preload_generation = self.play.play_preload_generation.wrapping_add(1);
        self.play.pending_play_preload = None;
        self.play.pending_course_stage_launch = None;
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
}
