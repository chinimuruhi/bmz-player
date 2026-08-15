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
    let chart_length_ms =
        library_db.chart_length_ms_by_id(chart_id)?.unwrap_or_default().max(0) as u64;
    let chart_parse_started_at = Instant::now();
    let imported = load_transformed_chart_for_play(library_db, chart_id, &options)?;
    let opponent_chart = if let Some(opponent) = &options.battle_opponent {
        let mut opponent_options = options.clone();
        opponent_options.session_mode = SessionMode::Normal;
        opponent_options.autoplay = false;
        opponent_options.practice_mode = false;
        opponent_options.seven_to_six = false;
        opponent_options.score_save_disabled = true;
        opponent_options.assist = AssistOptionConfig::default();
        opponent_options.assist_runtime = Default::default();
        opponent_options.replay_player = None;
        opponent_options.battle_opponent = None;
        opponent_options.opponent_chart = None;
        opponent_options.arrange = opponent.arrange;
        opponent_options.arrange_2p = opponent.arrange_2p;
        opponent_options.double_option = opponent.double_option;
        opponent_options.arrange_seed = opponent.arrange_seed;
        opponent_options.arrange_seed_2p = opponent.arrange_seed_2p;
        if let Some(packed) = opponent.packed_seed.and_then(|seed| u64::try_from(seed).ok())
            && let Some(seeds) = RandomOptionSeeds::unpack(
                packed,
                matches!(imported.source_key_mode, KeyMode::K10 | KeyMode::K14),
            )
        {
            opponent_options.arrange_seed = Some(i64::from(seeds.p1.value()));
            opponent_options.arrange_seed_2p = seeds.p2.map(|seed| i64::from(seed.value()));
        }
        opponent_options.bms_random_choices = opponent.bms_random_choices.clone();
        opponent_options.arrange_pattern = opponent.arrange_pattern.clone();
        opponent_options.s_random_scheme = opponent.s_random_scheme;
        opponent_options.s_random_scheme_2p = opponent.s_random_scheme_2p;
        Some(Arc::new(
            load_transformed_chart_for_play(library_db, chart_id, &opponent_options)?.chart,
        ))
    } else {
        None
    };
    let chart_parse_elapsed = chart_parse_started_at.elapsed();
    let skin_attempt = bmz_render::snapshot::SkinAttemptState {
        source_key_mode: Some(imported.source_key_mode),
        effective_key_mode: Some(imported.chart.metadata.key_mode),
        seven_to_six: imported.applied_arrange.seven_to_six,
        source_ln_profile_bits: Some(crate::skin_extension::source_ln_profile_bits(
            imported.source_ln_profile,
        )),
        session_mode_index: Some(crate::skin_extension::session_mode_index(
            options.session_mode,
            options.battle_opponent.is_some(),
        )),
        double_option_index: Some(crate::skin_extension::double_option_index(
            imported.applied_arrange.double_option,
        )),
        hsfix_index: Some(crate::skin_extension::hsfix_index(options.hs_fix)),
        ln_mode_index: Some(crate::skin_extension::long_note_mode_index(
            imported.chart.metadata.long_note_mode,
        )),
        has_bga: Some(imported.chart.metadata.has_bga),
        has_random_sequence: Some(imported.chart.metadata.has_bms_random),
        ..Default::default()
    };
    let mut primary_chart = imported.chart;
    if options.session_mode.is_battle()
        && let Some(opponent) = opponent_chart.as_deref()
    {
        apply_battle_opponent_chart(&mut primary_chart, opponent);
    }
    let chart = Arc::new(primary_chart);
    let prepared_chart = PreparedPlayChart {
        render_snapshot_cache: crate::screens::play_snapshot::PlayRenderSnapshotCache::from_chart(
            &chart,
        ),
        chart,
        skin_attempt,
        source_ln_profile: imported.source_ln_profile,
        chart_length_ms,
        applied_arrange: imported.applied_arrange,
        score_key: imported.score_key,
        assist_runtime: imported.assist_runtime,
        score_save_disabled: imported.score_save_disabled,
        opponent_chart,
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
        skin_attempt: prepared_chart.skin_attempt,
        source_ln_profile: prepared_chart.source_ln_profile,
        chart_length_ms: prepared_chart.chart_length_ms,
        audio,
        sample_report,
        chart_normalization_gain,
        render_snapshot_cache: prepared_chart.render_snapshot_cache,
        applied_arrange: prepared_chart.applied_arrange,
        score_key: prepared_chart.score_key,
        assist_runtime: prepared_chart.assist_runtime,
        score_save_disabled: prepared_chart.score_save_disabled,
        opponent_chart: prepared_chart.opponent_chart,
    })
}

/// Rebuild only the audio side of an already transformed chart.
///
/// Same-arrange quick retry can keep the exact chart/BGA resources, but the
/// sound bank is rebuilt from that chart so every `SoundId` is paired with the
/// asset declared by the retried session.  This intentionally does not carry
/// mixer voices, scheduled sounds, or any other playback state across retries.
pub fn preload_play_session_reloading_audio_with_progress(
    prepared_chart: PreparedPlayChart,
    sample_rate: u32,
    chart_normalization_gain: f32,
    on_progress: impl FnMut(usize, usize),
) -> PreloadedPlaySession {
    let PreparedPlayChart {
        chart,
        skin_attempt,
        source_ln_profile,
        chart_length_ms,
        render_snapshot_cache,
        applied_arrange,
        score_key,
        assist_runtime,
        score_save_disabled,
        opponent_chart,
    } = prepared_chart;
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
        skin_attempt,
        source_ln_profile,
        chart_length_ms,
        audio,
        sample_report,
        chart_normalization_gain,
        render_snapshot_cache,
        applied_arrange,
        score_key,
        assist_runtime,
        score_save_disabled,
        opponent_chart,
    }
}

pub(super) struct TransformedPlayChart {
    pub(super) chart: PlayableChart,
    pub(super) source_ln_profile: ChartLnProfile,
    pub(super) applied_arrange: AppliedArrange,
    pub(super) score_key: ScoreKey,
    pub(super) assist_runtime: bmz_gameplay::session::AssistRuntime,
    pub(super) score_save_disabled: bool,
    pub(super) source_key_mode: KeyMode,
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
    let seven_to_six = options.seven_to_six && source_key_mode == KeyMode::K7;
    let battle_presentation =
        options.session_mode.is_battle() && matches!(source_key_mode, KeyMode::K5 | KeyMode::K7);
    // SessionMode の battle は表示用に2P側を作るが、通常の BATTLE 譜面オプションとは
    // 異なり、1P側のスコアキーと保存可否を維持する。
    let applied_double_option = if seven_to_six || battle_presentation {
        DoubleOption::Off
    } else {
        options.double_option.normalize_for_key_mode(source_key_mode)
    };
    let ln_policy = course_score_ln_policy(
        options.ln_policy_setting,
        options.ln_mode_override,
        source_ln_profile,
    );
    let score_key = ScoreKey::with_options(
        chart.identity.file_sha256,
        ln_policy,
        applied_double_option.score_bucket(),
        options.rule_mode,
    );
    apply_score_ln_policy_to_chart(ln_policy, &mut chart);
    let assist_runtime = crate::assist::apply_chart_assists(
        &mut chart,
        options.assist,
        options.arrange_seed.unwrap_or(0),
    );
    let arrange = if seven_to_six {
        normalize_arrange_for_seven_to_six(options.arrange)
    } else {
        options.arrange
    };
    let arrange_seed = effective_arrange_seed(
        source_key_mode,
        arrange,
        options.arrange_seed,
        options.random_trainer_seed,
        options.arrange_pattern.as_deref(),
    )
    .or_else(|| seven_to_six.then(generate_arrange_seed));
    if seven_to_six {
        apply_seven_to_six(
            &mut chart,
            arrange_seed.expect("7K to 6K seed"),
            options.legacy_arrange_seed,
        );
    }
    apply_double_option(&mut chart, applied_double_option);
    if !seven_to_six && battle_presentation && options.battle_opponent.is_none() {
        apply_battle_double_option(&mut chart);
    }
    let mut applied_arrange = apply_arrange_pair(
        &mut chart,
        arrange,
        if seven_to_six { ArrangeOption::Normal } else { options.arrange_2p },
        arrange_seed,
        options.arrange_seed_2p,
        options.legacy_arrange_seed,
        options.s_random_scheme,
        options.s_random_scheme_2p,
        options.arrange_pattern.as_deref(),
    );
    applied_arrange.double_option = applied_double_option;
    applied_arrange.bms_random_choices = import.bms_random_choices;
    applied_arrange.seven_to_six = seven_to_six;

    Ok(TransformedPlayChart {
        chart,
        source_ln_profile,
        applied_arrange,
        score_key,
        assist_runtime,
        score_save_disabled: options.score_save_disabled || seven_to_six,
        source_key_mode,
    })
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
    Ok(scored_chart_metrics_for_chart(library_db, chart_id, options)?.total_notes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoredChartMetrics {
    pub total_notes: u32,
    pub ln_mode: Option<LongNoteMode>,
    pub source_ln_profile: ChartLnProfile,
}

pub fn scored_chart_metrics_from_prepared(prepared: &PreparedPlayChart) -> ScoredChartMetrics {
    ScoredChartMetrics {
        total_notes: scored_note_count(&prepared.chart),
        ln_mode: played_ln_mode(prepared.source_ln_profile, prepared.score_key.ln_policy),
        source_ln_profile: prepared.source_ln_profile,
    }
}

pub fn scored_chart_metrics_for_chart(
    library_db: &LibraryDatabase,
    chart_id: i64,
    options: &PlaySessionOptions,
) -> Result<ScoredChartMetrics> {
    let imported = load_transformed_chart_for_play(library_db, chart_id, options)?;
    Ok(ScoredChartMetrics {
        total_notes: scored_note_count(&imported.chart),
        ln_mode: played_ln_mode(imported.source_ln_profile, imported.score_key.ln_policy),
        source_ln_profile: imported.source_ln_profile,
    })
}

pub fn build_practice_prepared_from_preloaded(
    preloaded: PreloadedPlaySession,
    profile: &ProfileConfig,
    property: &PracticeProperty,
    mut options: PlaySessionOptions,
    input_backend: Box<dyn InputBackend>,
) -> PreparedPlaySession {
    let mut chart = (*preloaded.chart).clone();
    apply_practice_property(&mut chart, property);
    let double_option = if property.dp_flip { DoubleOption::Flip } else { DoubleOption::Off };
    apply_double_option(&mut chart, double_option);
    let mut applied_arrange = apply_arrange_pair(
        &mut chart,
        property.arrange,
        property.arrange_2p,
        None,
        None,
        false,
        options.s_random_scheme,
        options.s_random_scheme_2p,
        None,
    );
    applied_arrange.double_option = double_option;
    applied_arrange.seven_to_six = preloaded.applied_arrange.seven_to_six;
    options.practice_mode = true;
    options.assist_runtime = preloaded.assist_runtime;
    options.autoplay = false;
    options.replay_player = None;
    options.gauge_override = Some(property.gauge.gauge_type());
    options.gauge_property = property.gauge_category;
    options.gauge_auto_shift = if property.gauge == PracticeGaugeType::AutoShift {
        GaugeAutoShiftMode::BestClear
    } else {
        GaugeAutoShiftMode::Off
    };
    options.arrange = property.arrange;
    options.arrange_2p = property.arrange_2p;
    options.double_option = double_option;
    options.playback_rate_percent = property.playback_rate_percent;
    let target = TargetOption::None.as_string();
    let practice_mode = options.practice_mode;
    options.score_save_disabled |= preloaded.score_save_disabled;
    let score_save_disabled = options.score_save_disabled;
    let playback_rate_percent = options.playback_rate_percent;
    let mut session =
        build_game_session_with_input_backend(Arc::new(chart), profile, options, input_backend);
    session.audio_mix.chart_normalization_gain = preloaded.chart_normalization_gain;
    apply_practice_start_gauge(&mut session.gauge, property.start_gauge);
    let render_snapshot_cache =
        crate::screens::play_snapshot::PlayRenderSnapshotCache::from_chart(&session.chart);
    let mut skin_attempt = preloaded.skin_attempt;
    skin_attempt.effective_key_mode = Some(session.chart.metadata.key_mode);
    skin_attempt.double_option_index =
        Some(crate::skin_extension::double_option_index(applied_arrange.double_option));
    skin_attempt.hsfix_index = usize::try_from(session.hsfix_index).ok();
    skin_attempt.gauge_auto_shift_index =
        Some(crate::skin_extension::gauge_auto_shift_index(session.gauge.auto_shift_mode));
    skin_attempt.bottom_shiftable_gauge_index = Some(
        crate::skin_extension::bottom_shiftable_gauge_index(session.gauge.bottom_shiftable_gauge),
    );
    skin_attempt.judge_algorithm_index =
        Some(crate::skin_extension::judge_algorithm_index(session.judge.algorithm));
    skin_attempt.ln_mode_index =
        Some(crate::skin_extension::long_note_mode_index(session.chart.metadata.long_note_mode));
    PreparedPlaySession {
        session,
        skin_attempt,
        source_ln_profile: preloaded.source_ln_profile,
        chart_length_ms: preloaded.chart_length_ms,
        audio: preloaded.audio,
        sample_report: preloaded.sample_report,
        render_snapshot_cache,
        applied_arrange,
        score_key: preloaded.score_key,
        target_option: TargetOption::None,
        target,
        resolved_target: None,
        practice_mode,
        score_save_disabled,
        playback_rate_percent,
    }
}

pub fn build_prepared_play_session_from_preloaded(
    preloaded: PreloadedPlaySession,
    profile: &ProfileConfig,
    mut options: PlaySessionOptions,
    input_backend: Box<dyn InputBackend>,
) -> PreparedPlaySession {
    options.score_save_disabled |= preloaded.score_save_disabled;
    options.double_option = preloaded.applied_arrange.double_option;
    options.assist_runtime = preloaded.assist_runtime;
    let target_option = options.target;
    let resolved_target = options.resolved_target.clone();
    let target = resolved_target
        .as_ref()
        .map(|target| target.name.clone())
        .unwrap_or_else(|| options.target.as_string());
    let practice_mode = options.practice_mode;
    let score_save_disabled = options.score_save_disabled;
    let playback_rate_percent = options.playback_rate_percent;
    options.opponent_chart = preloaded.opponent_chart;
    let session =
        build_game_session_with_input_backend(preloaded.chart, profile, options, input_backend);
    let mut session = session;
    session.audio_mix.chart_normalization_gain = preloaded.chart_normalization_gain;
    let mut skin_attempt = preloaded.skin_attempt;
    skin_attempt.effective_key_mode = Some(session.chart.metadata.key_mode);
    skin_attempt.hsfix_index = usize::try_from(session.hsfix_index).ok();
    skin_attempt.gauge_auto_shift_index =
        Some(crate::skin_extension::gauge_auto_shift_index(session.gauge.auto_shift_mode));
    skin_attempt.bottom_shiftable_gauge_index = Some(
        crate::skin_extension::bottom_shiftable_gauge_index(session.gauge.bottom_shiftable_gauge),
    );
    skin_attempt.judge_algorithm_index =
        Some(crate::skin_extension::judge_algorithm_index(session.judge.algorithm));
    skin_attempt.ln_mode_index =
        Some(crate::skin_extension::long_note_mode_index(session.chart.metadata.long_note_mode));
    PreparedPlaySession {
        session,
        skin_attempt,
        source_ln_profile: preloaded.source_ln_profile,
        chart_length_ms: preloaded.chart_length_ms,
        audio: preloaded.audio,
        sample_report: preloaded.sample_report,
        render_snapshot_cache: preloaded.render_snapshot_cache,
        applied_arrange: preloaded.applied_arrange,
        score_key: preloaded.score_key,
        target_option,
        target,
        resolved_target,
        practice_mode,
        score_save_disabled,
        playback_rate_percent,
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
