use super::*;

pub fn load_game_session_for_chart(
    library_db: &LibraryDatabase,
    chart_id: i64,
    profile: &ProfileConfig,
    options: PlaySessionOptions,
) -> Result<GameSession> {
    load_game_session_for_chart_with_input_backend(
        library_db,
        chart_id,
        profile,
        options,
        Box::new(NullInputBackend),
    )
}

pub fn load_game_session_for_chart_with_input_backend(
    library_db: &LibraryDatabase,
    chart_id: i64,
    profile: &ProfileConfig,
    options: PlaySessionOptions,
    input_backend: Box<dyn InputBackend>,
) -> Result<GameSession> {
    let Some(path) = library_db.primary_chart_file_path(chart_id)? else {
        bail!("chart file not found for chart id {chart_id}");
    };
    let import = import_bms_chart_with_random_source(
        std::path::Path::new(&path),
        bms_random_source_for_chart(&options),
        true,
    )
    .with_context(|| format!("failed to import chart file: {path}"))?;
    Ok(build_game_session_with_input_backend(
        Arc::new(import.chart),
        profile,
        options,
        input_backend,
    ))
}

pub(super) fn bms_random_source_for_chart(options: &PlaySessionOptions) -> BmsRandomSource {
    if let Some(choices) = &options.bms_random_choices {
        BmsRandomSource::Choices(choices.clone())
    } else if options.legacy_arrange_seed {
        // Replay v3 and older derived `#RANDOM` from the shared arrange seed.
        BmsRandomSource::Seed(options.arrange_seed.map(|seed| seed as u64))
    } else {
        BmsRandomSource::Seed(options.bms_random_seed)
    }
}

pub fn build_audio_engine_for_chart(
    chart: &PlayableChart,
    sample_rate: u32,
    loader: &mut dyn SampleLoader,
) -> (AudioEngine, Vec<LoadedSampleReport>) {
    let mut audio = AudioEngine::new(sample_rate);
    let sample_report = load_chart_samples(&mut audio, chart, loader);
    (audio, sample_report)
}

pub(super) fn build_audio_engine_for_chart_with_progress(
    chart: &PlayableChart,
    sample_rate: u32,
    loader: &mut dyn SampleLoader,
    on_progress: impl FnMut(usize, usize),
) -> (AudioEngine, Vec<LoadedSampleReport>) {
    let mut audio = AudioEngine::new(sample_rate);
    let sample_report = load_chart_samples_with_progress(&mut audio, chart, loader, on_progress);
    (audio, sample_report)
}

pub fn load_prepared_play_session_for_chart(
    library_db: &LibraryDatabase,
    chart_id: i64,
    profile: &ProfileConfig,
    options: PlaySessionOptions,
) -> Result<PreparedPlaySession> {
    load_prepared_play_session_for_chart_with_input_backend(
        library_db,
        chart_id,
        profile,
        options,
        Box::new(NullInputBackend),
    )
}

pub fn load_prepared_play_session_for_chart_with_input_backend(
    library_db: &LibraryDatabase,
    chart_id: i64,
    profile: &ProfileConfig,
    options: PlaySessionOptions,
    input_backend: Box<dyn InputBackend>,
) -> Result<PreparedPlaySession> {
    let preloaded = preload_play_session_for_chart(
        library_db,
        chart_id,
        PlaySessionOptions { rule_mode: profile.play.rule_mode, ..options.clone() },
    )?;
    Ok(build_prepared_play_session_from_preloaded(preloaded, profile, options, input_backend))
}

pub fn preload_play_session_for_chart(
    library_db: &LibraryDatabase,
    chart_id: i64,
    options: PlaySessionOptions,
) -> Result<PreloadedPlaySession> {
    preload_play_session_for_chart_with_progress(library_db, chart_id, options, |_, _| {})
}

pub fn preload_play_session_for_chart_with_progress(
    library_db: &LibraryDatabase,
    chart_id: i64,
    options: PlaySessionOptions,
    on_progress: impl FnMut(usize, usize),
) -> Result<PreloadedPlaySession> {
    preload_play_session_for_chart_with_callbacks(
        library_db,
        chart_id,
        options,
        |_| {},
        on_progress,
    )
}

/// 譜面変換と配置確定が終わった時点で `on_arrange` を呼び、その後にWAVをロードする。
///
/// Play画面はこの通知を使って、重い音源ロードの完了を待たずに実配置を表示できる。
pub fn preload_play_session_for_chart_with_callbacks(
    library_db: &LibraryDatabase,
    chart_id: i64,
    options: PlaySessionOptions,
    on_arrange: impl FnOnce(&AppliedArrange),
    on_progress: impl FnMut(usize, usize),
) -> Result<PreloadedPlaySession> {
    let imported = load_transformed_chart_for_play(library_db, chart_id, &options)?;
    on_arrange(&imported.applied_arrange);
    let chart = Arc::new(imported.chart);
    let mut loader = FfmpegSampleLoader::default();
    let (audio, sample_report) = build_audio_engine_for_chart_with_progress(
        &chart,
        options.sample_rate,
        &mut loader,
        on_progress,
    );
    let chart_normalization_gain =
        load_or_compute_chart_normalization_gain(library_db, chart_id, &chart, &audio)?;

    Ok(PreloadedPlaySession {
        chart,
        audio,
        sample_report,
        chart_normalization_gain,
        applied_arrange: imported.applied_arrange,
        score_key: imported.score_key,
    })
}

/// Rebuild only the audio side of an already transformed chart.
///
/// Same-arrange quick retry can keep the exact chart/BGA resources, but the
/// sound bank is rebuilt from that chart so every `SoundId` is paired with the
/// asset declared by the retried session.  This intentionally does not carry
/// mixer voices, scheduled sounds, or any other playback state across retries.
pub fn preload_play_session_reloading_audio_with_progress(
    chart: Arc<PlayableChart>,
    sample_rate: u32,
    chart_normalization_gain: f32,
    applied_arrange: AppliedArrange,
    score_key: ScoreKey,
    on_progress: impl FnMut(usize, usize),
) -> PreloadedPlaySession {
    let mut loader = FfmpegSampleLoader::default();
    let (audio, sample_report) =
        build_audio_engine_for_chart_with_progress(&chart, sample_rate, &mut loader, on_progress);
    PreloadedPlaySession {
        chart,
        audio,
        sample_report,
        chart_normalization_gain,
        applied_arrange,
        score_key,
    }
}

pub(super) struct TransformedPlayChart {
    pub(super) chart: PlayableChart,
    pub(super) applied_arrange: AppliedArrange,
    pub(super) score_key: ScoreKey,
}

pub fn load_source_chart_for_chart(
    library_db: &LibraryDatabase,
    chart_id: i64,
    random_seed: Option<u64>,
) -> Result<PlayableChart> {
    let Some(path) = library_db.primary_chart_file_path(chart_id)? else {
        bail!("chart file not found for chart id {chart_id}");
    };
    Ok(import_bms_chart(std::path::Path::new(&path), random_seed, true)
        .with_context(|| format!("failed to import chart file: {path}"))?
        .chart)
}

pub(super) fn load_source_chart_import_for_play(
    library_db: &LibraryDatabase,
    chart_id: i64,
    options: &PlaySessionOptions,
) -> Result<ImportResult> {
    let Some(path) = library_db.primary_chart_file_path(chart_id)? else {
        bail!("chart file not found for chart id {chart_id}");
    };
    import_bms_chart_with_random_source(
        std::path::Path::new(&path),
        bms_random_source_for_chart(options),
        true,
    )
    .with_context(|| format!("failed to import chart file: {path}"))
}

pub(super) fn load_transformed_chart_for_play(
    library_db: &LibraryDatabase,
    chart_id: i64,
    options: &PlaySessionOptions,
) -> Result<TransformedPlayChart> {
    let import = load_source_chart_import_for_play(library_db, chart_id, options)?;
    let mut chart = import.chart;
    // beatoraja BMSModelUtils.setStartNoteTime(model, 1000) 相当。
    // LN / arrange より前に適用し、practice 切出しもシフト後時刻を使う。
    apply_start_note_margin(&mut chart);
    let source_key_mode = chart.metadata.key_mode;
    // SessionMode の battle は表示用に2P側を作るが、通常の BATTLE 譜面オプションとは
    // 異なり、1P側のスコアキーと保存可否を維持する。
    let applied_double_option = if options.session_mode.is_battle() {
        DoubleOption::Off
    } else {
        options.double_option.normalize_for_key_mode(source_key_mode)
    };
    let score_key = ScoreKey::with_options(
        chart.identity.file_sha256,
        score_ln_policy_for_chart(options.ln_policy_setting, &chart),
        applied_double_option.score_bucket(),
        options.rule_mode,
    );
    apply_ln_policy_to_chart(options.ln_policy_setting, &mut chart);
    // Course constraint may force a specific LN mode (Ln/Cn/Hcn) regardless of
    // what the chart declared. Mirrors beatoraja PlayerConfig.setLnmode().
    if let Some(ln_mode) = options.ln_mode_override {
        force_ln_mode_for_chart(ln_mode, &mut chart);
    }
    apply_double_option(&mut chart, applied_double_option);
    if options.session_mode.is_battle() && matches!(source_key_mode, KeyMode::K5 | KeyMode::K7) {
        apply_battle_double_option(&mut chart);
    }
    let arrange_seed = effective_arrange_seed(
        chart.metadata.key_mode,
        options.arrange,
        options.arrange_seed,
        options.random_trainer_seed,
        options.arrange_pattern.as_deref(),
    );
    let mut applied_arrange = apply_arrange_pair(
        &mut chart,
        options.arrange,
        options.arrange_2p,
        arrange_seed,
        options.arrange_seed_2p,
        options.legacy_arrange_seed,
        options.arrange_pattern.as_deref(),
    );
    applied_arrange.double_option = applied_double_option;
    applied_arrange.bms_random_choices = import.bms_random_choices;

    Ok(TransformedPlayChart { chart, applied_arrange, score_key })
}

pub(super) fn effective_arrange_seed(
    key_mode: KeyMode,
    arrange: ArrangeOption,
    arrange_seed: Option<i64>,
    random_trainer_seed: Option<i64>,
    recorded_pattern: Option<&[u8]>,
) -> Option<i64> {
    if key_mode == KeyMode::K7 && arrange == ArrangeOption::Random && recorded_pattern.is_none() {
        random_trainer_seed.or(arrange_seed)
    } else {
        arrange_seed
    }
}

pub fn scored_note_count_for_chart(
    library_db: &LibraryDatabase,
    chart_id: i64,
    options: &PlaySessionOptions,
) -> Result<u32> {
    let imported = load_transformed_chart_for_play(library_db, chart_id, options)?;
    Ok(scored_note_count(&imported.chart))
}

pub fn load_chart_bga_assets_for_chart(
    library_db: &LibraryDatabase,
    chart_id: i64,
    options: &PlaySessionOptions,
) -> Result<Vec<BgaAssetRef>> {
    Ok(load_source_chart_import_for_play(library_db, chart_id, options)?.chart.bga_assets)
}

pub fn build_practice_prepared_from_preloaded(
    preloaded: PreloadedPlaySession,
    profile: &ProfileConfig,
    property: &PracticeProperty,
    mut options: PlaySessionOptions,
    input_backend: Box<dyn InputBackend>,
) -> PreparedPlaySession {
    let mut chart = (*preloaded.chart).clone();
    let applied_arrange = apply_practice_property(&mut chart, property);
    options.practice_mode = true;
    options.autoplay = false;
    options.replay_player = None;
    options.gauge_override = Some(gauge_type_from_config(property.gauge));
    options.gauge_auto_shift = GaugeAutoShiftMode::Off;
    options.arrange = property.arrange;
    let target = TargetOption::None.as_string();
    let practice_mode = options.practice_mode;
    let mut session =
        build_game_session_with_input_backend(Arc::new(chart), profile, options, input_backend);
    session.audio_mix.chart_normalization_gain = preloaded.chart_normalization_gain;
    apply_practice_start_gauge(&mut session.gauge, property.start_gauge);
    PreparedPlaySession {
        session,
        audio: preloaded.audio,
        sample_report: preloaded.sample_report,
        applied_arrange,
        score_key: preloaded.score_key,
        target_option: TargetOption::None,
        target,
        practice_mode,
    }
}

pub fn build_prepared_play_session_from_preloaded(
    preloaded: PreloadedPlaySession,
    profile: &ProfileConfig,
    mut options: PlaySessionOptions,
    input_backend: Box<dyn InputBackend>,
) -> PreparedPlaySession {
    options.double_option = preloaded.applied_arrange.double_option;
    let target_option = options.target;
    let target = options.target.as_string();
    let practice_mode = options.practice_mode;
    let session =
        build_game_session_with_input_backend(preloaded.chart, profile, options, input_backend);
    let mut session = session;
    session.audio_mix.chart_normalization_gain = preloaded.chart_normalization_gain;
    PreparedPlaySession {
        session,
        audio: preloaded.audio,
        sample_report: preloaded.sample_report,
        applied_arrange: preloaded.applied_arrange,
        score_key: preloaded.score_key,
        target_option,
        target,
        practice_mode,
    }
}

pub(super) fn load_or_compute_chart_normalization_gain(
    library_db: &LibraryDatabase,
    chart_id: i64,
    chart: &PlayableChart,
    audio: &AudioEngine,
) -> Result<f32> {
    if let Some(analysis) = library_db.chart_normalization_analysis_by_chart_id(chart_id)? {
        return Ok(play_normalization_gain_for_loudness(analysis.loudness_lufs));
    }

    let Some(analysis) = analyze_chart_loudness(chart, &audio.samples, audio.output_sample_rate())
    else {
        tracing::warn!(chart_id, "failed to analyze chart loudness; using unity gain");
        return Ok(1.0);
    };
    let stored = ChartNormalizationAnalysis { loudness_lufs: analysis.loudness_lufs };
    library_db.write_chart_normalization_analysis(chart_id, stored)?;
    let play_gain = play_normalization_gain_for_loudness(stored.loudness_lufs);
    tracing::info!(
        chart_id,
        loudness_lufs = stored.loudness_lufs,
        chart_normalization_gain = play_gain,
        "stored chart volume normalization analysis"
    );
    Ok(play_gain)
}
