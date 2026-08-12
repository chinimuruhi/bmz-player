use super::*;

impl WinitApp {
    pub(super) fn begin_decide_for_chart(&mut self, chart_id: i64, mut options: PlayStartOptions) {
        self.normalize_seven_to_six_options(chart_id, &mut options);
        self.apply_active_rival_play_overrides(chart_id, &mut options);
        let snapshot = self.decide_snapshot_for_chart(chart_id);
        self.begin_decide_for_chart_with_snapshot(chart_id, options, snapshot, None, None);
    }

    pub(super) fn apply_active_rival_play_overrides(
        &self,
        chart_id: i64,
        options: &mut PlayStartOptions,
    ) {
        if options.session_mode != SessionMode::Normal
            || options.autoplay
            || options.practice_mode
            || options.seven_to_six
            || options.replay_player.is_some()
            || self.play.active_course.is_some()
        {
            return;
        }
        let Some(chart) = self
            .boot
            .library_db
            .list_charts_by_ids(&[chart_id])
            .ok()
            .and_then(|mut charts| charts.pop())
        else {
            return;
        };
        let policy = crate::ln_policy::score_ln_policy(
            self.boot.profile_config.play.ln_mode_policy,
            chart.ln_profile,
        );
        let ln_mode = crate::screens::select_ir::rian_ln_mode_for_chart(chart.ln_profile, policy);
        let Some(score) = self.select.select_ir.active_rival_score(chart.sha256, ln_mode).cloned()
        else {
            // beatoraja: 選択ライバルが未プレイなら通常ターゲットへ戻す。
            return;
        };
        if let Some(name) = self.select.select_ir.active_rival_display_name() {
            options.resolved_target =
                Some(ResolvedTarget { name: name.to_string(), ex_score: score.ex_score });
        }

        use crate::config::profile_config::ChartReplicationModeConfig;
        let replication = self.boot.profile_config.rival.chart_replication_mode;
        if replication == ChartReplicationModeConfig::None {
            return;
        }
        let key_mode = KeyMode::from_str_opt(&chart.mode).unwrap_or_default();
        let (arrange, arrange_2p, double_option) = rival_arrange_options(&score);
        options.arrange = arrange;
        options.arrange_2p = arrange_2p;
        options.double_option = double_option.normalize_for_key_mode(key_mode);
        options.arrange_pattern = None;
        options.random_trainer_seed = None;

        if replication == ChartReplicationModeConfig::RivalChart
            && let Some(packed) = score.play_seed.and_then(|seed| u64::try_from(seed).ok())
        {
            let is_double = matches!(key_mode, KeyMode::K10 | KeyMode::K14);
            if let Some(seeds) =
                crate::random_option_seed::RandomOptionSeeds::unpack(packed, is_double)
            {
                options.arrange_seed = Some(i64::from(seeds.p1.value()));
                options.arrange_seed_2p = seeds.p2.map(|seed| i64::from(seed.value()));
                options.legacy_arrange_seed = false;
            } else {
                tracing::warn!(packed, ?key_mode, "ignoring invalid rival play seed");
            }
        }
    }

    pub(super) fn begin_course_decide_for_chart(
        &mut self,
        chart_id: i64,
        mut options: PlayStartOptions,
        course_title: &str,
        chart_metadata: ChartListItem,
    ) {
        self.normalize_seven_to_six_options(chart_id, &mut options);
        let mut snapshot = self.decide_snapshot_for_chart_with_metadata(chart_id, &chart_metadata);
        self.apply_course_skin_context(&mut snapshot);
        let title_override =
            DecideTitleOverride { title: course_title.to_string(), subtitle: String::new() };
        self.begin_decide_for_chart_with_snapshot(
            chart_id,
            options,
            snapshot,
            Some(title_override),
            Some(chart_metadata),
        );
    }

    pub(super) fn begin_decide_for_chart_with_snapshot(
        &mut self,
        chart_id: i64,
        options: PlayStartOptions,
        mut snapshot: RenderSnapshot,
        title_override: Option<DecideTitleOverride>,
        chart_metadata: Option<ChartListItem>,
    ) {
        // Pre-import placeholder only: resolve the same AUTO fallback / FORCE
        // priority as preload and account for Battle here. The running session
        // replaces this with a count derived from the imported source chart.
        let chart_metadata = chart_metadata.or_else(|| {
            self.boot
                .library_db
                .list_charts_by_ids(&[chart_id])
                .ok()
                .and_then(|mut charts| charts.pop())
        });
        if let Some(chart) = &chart_metadata {
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
        if let Some(chart) = &chart_metadata {
            self.prepare_play_meta_image_textures_from_chart(chart);
        } else {
            self.prepare_play_meta_image_textures(chart_id);
        }
        // Play スキンは裏で decode+upload を進めるが、Decide 入場では待たない。
        // 実際の Play 入場で `ensure_skin_ready` が保険として残る。
        let play_skin_key_mode = chart_metadata
            .as_ref()
            .and_then(|chart| KeyMode::from_str_opt(&chart.mode))
            .map(|key_mode| {
                play_skin_key_mode_for_options(
                    key_mode,
                    options.double_option,
                    options.session_mode,
                    options.seven_to_six,
                )
            })
            .unwrap_or_else(|| self.play_skin_key_mode_for_chart(chart_id, &options));
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

    pub(super) fn start_play_preload(
        &mut self,
        chart_id: i64,
        mut options: PlayStartOptions,
    ) -> u64 {
        self.normalize_seven_to_six_options(chart_id, &mut options);
        // 通常開始・practice・retry は、残っているコース次曲先読みを置き換える。
        // コース側は worker 開始後に同じ generation の launch 情報を設定し直す。
        self.play.pending_course_stage_launch = None;
        self.play.play_preload_generation = self.play.play_preload_generation.wrapping_add(1);
        let generation = self.play.play_preload_generation;
        self.play.preloaded_play_session = None;
        let (tx, rx) = mpsc::channel();
        let library_db_path = self.boot.app_paths.library_db.clone();
        let app_config = self.play_session_app_config();
        let play_config_key_mode =
            effective_play_key_mode(self.key_mode_for_chart(chart_id), options.seven_to_six);
        options.hs_fix = hs_fix_option_from_profile(
            self.boot.profile_config.play_mode_config(play_config_key_mode).hs_fix,
        );
        let (ln_policy_setting, rule_mode) = self
            .play
            .active_course
            .as_ref()
            .map(|course| (course.ln_policy_setting, course.rule_mode))
            .unwrap_or((
                self.boot.profile_config.play.ln_mode_policy,
                self.boot.profile_config.play.rule_mode,
            ));
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
                    session_options.play_config_key_mode = Some(play_config_key_mode);
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
            options.seven_to_six,
        )
    }

    pub(super) fn normalize_seven_to_six_options(
        &self,
        chart_id: i64,
        options: &mut PlayStartOptions,
    ) {
        if !options.seven_to_six || self.key_mode_for_chart(chart_id) != KeyMode::K7 {
            return;
        }
        options.score_save_disabled = true;
        options.arrange =
            crate::screens::play_session::normalize_arrange_for_seven_to_six(options.arrange);
        options.arrange_2p = ArrangeOption::Normal;
        options.double_option = DoubleOption::Off;
        options.session_mode = match options.session_mode {
            SessionMode::AutoplayBattle => SessionMode::Autoplay,
            SessionMode::GhostBattle => SessionMode::Normal,
            other => other,
        };
        options.autoplay = options.session_mode.primary_autoplay();
        options.target = TargetOption::None;
        options.resolved_target = None;
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
            resolve_gamepad_runtime_slots(&app_config.input, self.gamepad.as_ref())
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
        self.decide_snapshot_for_chart_metadata(chart_id, metadata)
    }

    pub(super) fn decide_snapshot_for_chart_with_metadata(
        &self,
        chart_id: i64,
        chart: &ChartListItem,
    ) -> RenderSnapshot {
        self.decide_snapshot_for_chart_metadata(chart_id, Some((chart.clone(), None)))
    }

    fn decide_snapshot_for_chart_metadata(
        &self,
        chart_id: i64,
        metadata: Option<(ChartListItem, Option<u32>)>,
    ) -> RenderSnapshot {
        let mut snapshot = RenderSnapshot::default();
        let chart_hint = metadata.as_ref().map(|(chart, _)| chart);
        if let Some((chart, best_ex_score)) = &metadata {
            let total_notes =
                chart.scored_total_notes_for_setting(self.boot.profile_config.play.ln_mode_policy);
            apply_chart_metadata_to_snapshot(&mut snapshot, chart, total_notes, *best_ex_score);
        }
        let (primary, secondary, fallback) =
            self.table_text_context_for_chart_with_metadata(chart_id, chart_hint).as_tuple();
        snapshot.table_text_primary = primary;
        snapshot.table_text_secondary = secondary;
        snapshot.table_text_fallback = fallback;
        snapshot
    }
}

fn rival_arrange_options(
    score: &crate::storage::network_db::IrRivalScoreRecord,
) -> (ArrangeOption, ArrangeOption, DoubleOption) {
    if !score.arrange_1p.trim().is_empty() {
        return (
            arrange_option_from_rian(&score.arrange_1p),
            arrange_option_from_rian(&score.arrange_2p),
            double_option_from_rian(&score.double_option),
        );
    }
    let packed = score.play_option.max(0);
    (
        arrange_option_from_beatoraja_id(packed % 10),
        arrange_option_from_beatoraja_id((packed / 10) % 10),
        if (packed / 100) % 10 == 1 { DoubleOption::Flip } else { DoubleOption::Off },
    )
}

fn arrange_option_from_rian(value: &str) -> ArrangeOption {
    match value.trim().to_ascii_lowercase().as_str() {
        "mirror" => ArrangeOption::Mirror,
        "random" => ArrangeOption::Random,
        "r-random" => ArrangeOption::RRandom,
        "s-random" => ArrangeOption::SRandom,
        "spiral" => ArrangeOption::Spiral,
        "h-random" => ArrangeOption::HRandom,
        "all-scratch" => ArrangeOption::AllScratch,
        "random-ex" => ArrangeOption::RandomEx,
        "s-random-ex" => ArrangeOption::SRandomEx,
        "f-random" => ArrangeOption::FRandom,
        "mf-random" => ArrangeOption::MFRandom,
        _ => ArrangeOption::Normal,
    }
}

fn arrange_option_from_beatoraja_id(value: i32) -> ArrangeOption {
    match value {
        1 => ArrangeOption::Mirror,
        2 => ArrangeOption::Random,
        3 => ArrangeOption::RRandom,
        4 => ArrangeOption::SRandom,
        5 => ArrangeOption::Spiral,
        6 => ArrangeOption::HRandom,
        7 => ArrangeOption::AllScratch,
        8 => ArrangeOption::RandomEx,
        9 => ArrangeOption::SRandomEx,
        _ => ArrangeOption::Normal,
    }
}

fn double_option_from_rian(value: &str) -> DoubleOption {
    match value.trim().to_ascii_lowercase().as_str() {
        "flip" => DoubleOption::Flip,
        _ => DoubleOption::Off,
    }
}

#[cfg(test)]
mod rival_replication_tests {
    use super::*;
    use crate::storage::network_db::IrRivalScoreRecord;

    fn rival_score(play_option: i32, arrange_1p: &str) -> IrRivalScoreRecord {
        IrRivalScoreRecord {
            chart_sha256: [7; 32],
            ln_mode: 1,
            ex_score: 1234,
            clear_type: 5,
            max_combo: 600,
            min_bp: 10,
            play_option,
            arrange_1p: arrange_1p.to_string(),
            arrange_2p: String::new(),
            double_option: "off".to_string(),
            play_seed: Some(42),
        }
    }

    #[test]
    fn structured_f_random_wins_over_legacy_play_option() {
        let score = rival_score(0, "f-random");

        assert_eq!(
            rival_arrange_options(&score),
            (ArrangeOption::FRandom, ArrangeOption::Normal, DoubleOption::Off)
        );
    }

    #[test]
    fn legacy_play_option_is_used_when_structured_option_is_missing() {
        let mut score = rival_score(121, "");
        score.arrange_2p.clear();
        score.double_option.clear();

        assert_eq!(
            rival_arrange_options(&score),
            (ArrangeOption::Mirror, ArrangeOption::Random, DoubleOption::Flip)
        );
    }
}
