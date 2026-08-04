use super::*;
use std::collections::HashSet;
use std::time::Instant;

/// 音源 preload の入力規模。source は宣言パス単位、region は `SoundId` 単位で数える。
///
/// source candidate の拡張子フォールバックや decode cache の実装には依存しないため、
/// 音源 region API の移行前後で比較できる計測値として使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SoundPreloadCounts {
    pub(super) source_count: usize,
    pub(super) region_count: usize,
}

pub(super) fn sound_preload_counts(chart: &PlayableChart) -> SoundPreloadCounts {
    SoundPreloadCounts {
        source_count: chart.sounds.iter().map(|sound| &sound.path).collect::<HashSet<_>>().len(),
        region_count: chart.sounds.len(),
    }
}

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

/// 譜面変換と配置確定が終わった時点で `on_chart` を呼び、その後にWAVをロードする。
///
/// Play画面とBMP loaderはこの通知を使って、重い音源ロードの完了を待たずに
/// 完成済み譜面と実配置を参照できる。
pub fn preload_play_session_for_chart_with_callbacks(
    library_db: &LibraryDatabase,
    chart_id: i64,
    options: PlaySessionOptions,
    on_chart: impl FnOnce(&PreparedPlayChart),
    on_progress: impl FnMut(usize, usize),
) -> Result<PreloadedPlaySession> {
    let preload_started_at = Instant::now();
    let chart_parse_started_at = Instant::now();
    let imported = load_transformed_chart_for_play(library_db, chart_id, &options)?;
    let chart_parse_elapsed = chart_parse_started_at.elapsed();
    let chart = Arc::new(imported.chart);
    let prepared_chart = PreparedPlayChart {
        render_snapshot_cache: crate::screens::play_snapshot::PlayRenderSnapshotCache::from_chart(
            &chart,
        ),
        chart,
        source_ln_profile: imported.source_ln_profile,
        applied_arrange: imported.applied_arrange,
        score_key: imported.score_key,
    };
    let sound_counts = sound_preload_counts(&prepared_chart.chart);
    tracing::info!(
        chart_id,
        chart_parse_elapsed_ms = chart_parse_elapsed.as_millis(),
        sound_sources = sound_counts.source_count,
        sound_regions = sound_counts.region_count,
        "play preload parsed and prepared chart"
    );
    on_chart(&prepared_chart);
    let mut loader = FfmpegSampleLoader::default();
    let audio_load_started_at = Instant::now();
    let (audio, sample_report) = build_audio_engine_for_chart_with_progress(
        &prepared_chart.chart,
        options.sample_rate,
        &mut loader,
        on_progress,
    );
    let audio_load_elapsed = audio_load_started_at.elapsed();
    tracing::info!(
        chart_id,
        audio_load_elapsed_ms = audio_load_elapsed.as_millis(),
        sound_sources = sound_counts.source_count,
        sound_regions = sound_counts.region_count,
        decoded_sources = audio.samples.source_count(),
        loaded_regions = audio.samples.region_count(),
        "play preload loaded audio"
    );
    let normalization_started_at = Instant::now();
    let chart_normalization_gain = load_or_compute_chart_normalization_gain(
        library_db,
        chart_id,
        &prepared_chart.chart,
        &audio,
    )?;
    tracing::info!(
        chart_id,
        normalization_elapsed_ms = normalization_started_at.elapsed().as_millis(),
        preload_elapsed_ms = preload_started_at.elapsed().as_millis(),
        sound_sources = sound_counts.source_count,
        sound_regions = sound_counts.region_count,
        chart_normalization_gain,
        "play preload complete"
    );

    Ok(PreloadedPlaySession {
        chart: prepared_chart.chart,
        source_ln_profile: prepared_chart.source_ln_profile,
        audio,
        sample_report,
        chart_normalization_gain,
        render_snapshot_cache: prepared_chart.render_snapshot_cache,
        applied_arrange: prepared_chart.applied_arrange,
        score_key: prepared_chart.score_key,
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
    source_ln_profile: ChartLnProfile,
    sample_rate: u32,
    chart_normalization_gain: f32,
    render_snapshot_cache: crate::screens::play_snapshot::PlayRenderSnapshotCache,
    applied_arrange: AppliedArrange,
    score_key: ScoreKey,
    on_progress: impl FnMut(usize, usize),
) -> PreloadedPlaySession {
    let sound_counts = sound_preload_counts(&chart);
    let mut loader = FfmpegSampleLoader::default();
    let audio_load_started_at = Instant::now();
    let (audio, sample_report) =
        build_audio_engine_for_chart_with_progress(&chart, sample_rate, &mut loader, on_progress);
    tracing::info!(
        audio_load_elapsed_ms = audio_load_started_at.elapsed().as_millis(),
        sound_sources = sound_counts.source_count,
        sound_regions = sound_counts.region_count,
        decoded_sources = audio.samples.source_count(),
        loaded_regions = audio.samples.region_count(),
        "play quick retry reloaded audio"
    );
    PreloadedPlaySession {
        chart,
        source_ln_profile,
        audio,
        sample_report,
        chart_normalization_gain,
        render_snapshot_cache,
        applied_arrange,
        score_key,
    }
}

pub(super) struct TransformedPlayChart {
    pub(super) chart: PlayableChart,
    pub(super) source_ln_profile: ChartLnProfile,
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
    let source_ln_profile = ChartLnProfile::from_chart(&chart);
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

    Ok(TransformedPlayChart { chart, source_ln_profile, applied_arrange, score_key })
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
    let render_snapshot_cache =
        crate::screens::play_snapshot::PlayRenderSnapshotCache::from_chart(&session.chart);
    PreparedPlaySession {
        session,
        source_ln_profile: preloaded.source_ln_profile,
        audio: preloaded.audio,
        sample_report: preloaded.sample_report,
        render_snapshot_cache,
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
        source_ln_profile: preloaded.source_ln_profile,
        audio: preloaded.audio,
        sample_report: preloaded.sample_report,
        render_snapshot_cache: preloaded.render_snapshot_cache,
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
