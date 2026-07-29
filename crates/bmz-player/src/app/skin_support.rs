use super::*;

/// 起動時のスキンロード処理。
///
/// - default skin は必ず一度だけ renderer にアップロードする。
/// - select の JSON skin は同期デコード+install（Select 画面を最短で表示するためクリティカルパス）。
/// - decide / result の JSON skin はバックグラウンドスレッドで Phase A (decode) を実行。
///   完了したものは main thread の `try_recv` で順次 Phase B (install) する。
/// - select/decide/result の各パスが JSON 以外 (空文字または非対応) の場合は警告ログのみ。
/// - プレイスキンは決定画面でチャートの key_mode から個別に decode するためここでは扱わない。
#[allow(clippy::too_many_arguments)]
pub(super) fn load_initial_skin_textures(
    renderer: &mut Renderer,
    app_paths: &crate::paths::AppPaths,
    skin_decode_tx: &mpsc::Sender<PendingSkinResult>,
    skin_source_asset_cache: &SharedSkinSourceAssetCache,
    skin_document_cache: &SharedSkinDocumentCache,
    skin_gpu_texture_cache: &SharedSkinGpuTextureCache,
    skin_font_cache: &SharedSkinFontCache,
    generation: u64,
    player_name: &str,
    select_skin_path: &str,
    decide_skin_path: &str,
    result_skin_path: &str,
    select_options: &BTreeMap<String, String>,
    decide_options: &BTreeMap<String, String>,
    result_options: &BTreeMap<String, String>,
    select_files: &BTreeMap<String, String>,
    decide_files: &BTreeMap<String, String>,
    result_files: &BTreeMap<String, String>,
    select_offsets: &[SkinOffsetConfig],
    decide_offsets: &[SkinOffsetConfig],
    result_offsets: &[SkinOffsetConfig],
) -> (Option<SkinManifest>, HashMap<SkinKind, Vec<ActiveSkinVideoSource>>, bool, bool, bool) {
    // Decide / Result の JSON skin は Select の同期ロードより**前**に decode スレッドを起動して
    // CPU をフル活用する。Select の sync 処理 (PNG GPU upload など) と並列に decode が進む。
    let pending_select = false;
    let mut pending_decide = false;
    let mut pending_result = false;
    let mut skin_video_sources = HashMap::new();

    let decide_trimmed = decide_skin_path.trim().to_string();
    let result_trimmed = result_skin_path.trim().to_string();

    {
        let decide_path = if decide_trimmed.is_empty() {
            default_skin_document_path_from_paths(app_paths, SkinKind::Decide)
        } else {
            match app_paths.resolve_path_ref(&decide_trimmed) {
                Ok(path) => path,
                Err(error) => {
                    tracing::warn!(
                        path = %decide_trimmed,
                        error = %format_error_chain(&error),
                        "failed to resolve decide skin path; ignoring"
                    );
                    PathBuf::new()
                }
            }
        };
        if !decide_path.as_os_str().is_empty() && is_decodable_skin_path(&decide_path) {
            spawn_skin_decode(
                skin_decode_tx.clone(),
                skin_source_asset_cache.clone(),
                skin_document_cache.clone(),
                skin_gpu_texture_cache.clone(),
                skin_font_cache.clone(),
                HashMap::new(),
                generation,
                decide_path,
                SkinKind::Decide,
                if decide_trimmed.is_empty() { BTreeMap::new() } else { decide_options.clone() },
                if decide_trimmed.is_empty() { BTreeMap::new() } else { decide_files.clone() },
                lua_runtime_state_with_skin_offsets(
                    lua_runtime_state_for_player(player_name),
                    decide_offsets,
                ),
            );
            pending_decide = true;
        }
    }
    {
        let result_path = if result_trimmed.is_empty() {
            default_skin_document_path_from_paths(app_paths, SkinKind::Result)
        } else {
            match app_paths.resolve_path_ref(&result_trimmed) {
                Ok(path) => path,
                Err(error) => {
                    tracing::warn!(
                        path = %result_trimmed,
                        error = %format_error_chain(&error),
                        "failed to resolve result skin path; ignoring"
                    );
                    PathBuf::new()
                }
            }
        };
        if !result_path.as_os_str().is_empty() && is_decodable_skin_path(&result_path) {
            spawn_skin_decode(
                skin_decode_tx.clone(),
                skin_source_asset_cache.clone(),
                skin_document_cache.clone(),
                skin_gpu_texture_cache.clone(),
                skin_font_cache.clone(),
                HashMap::new(),
                generation,
                result_path,
                SkinKind::Result,
                if result_trimmed.is_empty() { BTreeMap::new() } else { result_options.clone() },
                if result_trimmed.is_empty() { BTreeMap::new() } else { result_files.clone() },
                lua_runtime_state_with_skin_offsets(
                    lua_runtime_state_for_result(
                        false,
                        None,
                        false,
                        KeyMode::default(),
                        BTreeMap::new(),
                        player_name,
                    ),
                    result_offsets,
                ),
            );
            pending_result = true;
        }
    }

    let default_manifest = match load_default_skin_into_renderer_from_paths(renderer, app_paths) {
        Ok(manifest) => Some(manifest),
        Err(error) => {
            tracing::warn!(
                error = %format_error_chain(&error),
                "failed to load default skin; using fallback drawing"
            );
            None
        }
    };

    // Select skin (クリティカルパス: 起動直後に表示される)
    let select_trimmed = select_skin_path.trim();
    {
        let select_path = if select_trimmed.is_empty() {
            Ok(default_skin_document_path_from_paths(app_paths, SkinKind::Select))
        } else {
            app_paths.resolve_path_ref(select_trimmed)
        };
        let empty_options = BTreeMap::new();
        let empty_files = BTreeMap::new();
        let active_select_options =
            if select_trimmed.is_empty() { &empty_options } else { select_options };
        let active_select_files =
            if select_trimmed.is_empty() { &empty_files } else { select_files };
        match select_path {
            Ok(path) if is_decodable_skin_path(&path) => {
                let video_sources = apply_json_skin_sync(
                    renderer,
                    &path,
                    SkinKind::Select,
                    default_manifest.as_ref(),
                    active_select_options,
                    active_select_files,
                    &lua_runtime_state_with_skin_offsets(
                        lua_runtime_state_for_player(player_name),
                        select_offsets,
                    ),
                );
                if !video_sources.is_empty() {
                    skin_video_sources.insert(SkinKind::Select, video_sources);
                }
            }
            Ok(path) => {
                tracing::warn!(
                    path = %path.display(),
                    "select skin path is not a supported beatoraja skin file; ignoring"
                );
            }
            Err(error) => {
                tracing::warn!(
                    path = %select_trimmed,
                    error = %format_error_chain(&error),
                    "failed to resolve select skin path; ignoring"
                );
            }
        }
    }

    if !result_trimmed.is_empty() {
        match app_paths.resolve_path_ref(&result_trimmed) {
            Ok(path) if !is_decodable_skin_path(&path) => {
                tracing::warn!(
                    path = %path.display(),
                    "result skin path is not a supported beatoraja skin file; ignoring"
                );
            }
            _ => {}
        }
    }

    if !decide_trimmed.is_empty() {
        match app_paths.resolve_path_ref(&decide_trimmed) {
            Ok(path) if !is_decodable_skin_path(&path) => {
                tracing::warn!(
                    path = %path.display(),
                    "decide skin path is not a supported beatoraja skin file; ignoring"
                );
            }
            _ => {}
        }
    }

    (default_manifest, skin_video_sources, pending_select, pending_decide, pending_result)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn reload_skin_textures(
    _renderer: &mut Renderer,
    app_paths: &crate::paths::AppPaths,
    skin_decode_tx: &mpsc::Sender<PendingSkinResult>,
    skin_source_asset_cache: &SharedSkinSourceAssetCache,
    skin_document_cache: &SharedSkinDocumentCache,
    skin_gpu_texture_cache: &SharedSkinGpuTextureCache,
    skin_font_cache: &SharedSkinFontCache,
    generations: &mut SkinReloadGenerations,
    request: SkinReloadRequest,
    player_name: &str,
    select_skin_path: &str,
    decide_skin_path: &str,
    result_skin_path: &str,
    select_options: &BTreeMap<String, String>,
    decide_options: &BTreeMap<String, String>,
    result_options: &BTreeMap<String, String>,
    select_files: &BTreeMap<String, String>,
    decide_files: &BTreeMap<String, String>,
    result_files: &BTreeMap<String, String>,
    select_offsets: &[SkinOffsetConfig],
    decide_offsets: &[SkinOffsetConfig],
    result_offsets: &[SkinOffsetConfig],
) -> (bool, bool, bool) {
    let mut pending_select = false;
    let mut pending_decide = false;
    let mut pending_result = false;

    for (enabled, path_text, kind, options, files, offsets) in [
        (
            request.select,
            select_skin_path,
            SkinKind::Select,
            select_options,
            select_files,
            select_offsets,
        ),
        (
            request.decide,
            decide_skin_path,
            SkinKind::Decide,
            decide_options,
            decide_files,
            decide_offsets,
        ),
        (
            request.result,
            result_skin_path,
            SkinKind::Result,
            result_options,
            result_files,
            result_offsets,
        ),
    ] {
        if !enabled {
            continue;
        }
        let generation = generations.bump(kind);
        let trimmed = path_text.trim();
        let path = if trimmed.is_empty() {
            default_skin_document_path_from_paths(app_paths, kind)
        } else {
            match app_paths.resolve_path_ref(trimmed) {
                Ok(path) => path,
                Err(error) => {
                    tracing::warn!(
                        path = %trimmed,
                        kind = ?kind,
                        error = %format_error_chain(&error),
                        "failed to resolve skin path; ignoring"
                    );
                    continue;
                }
            }
        };
        if is_decodable_skin_path(&path) {
            spawn_skin_decode(
                skin_decode_tx.clone(),
                skin_source_asset_cache.clone(),
                skin_document_cache.clone(),
                skin_gpu_texture_cache.clone(),
                skin_font_cache.clone(),
                HashMap::new(),
                generation,
                path.clone(),
                kind,
                if trimmed.is_empty() { BTreeMap::new() } else { options.clone() },
                if trimmed.is_empty() { BTreeMap::new() } else { files.clone() },
                lua_runtime_state_with_skin_offsets(
                    lua_runtime_state_for_player(player_name),
                    offsets,
                ),
            );
            match kind {
                SkinKind::Select => pending_select = true,
                SkinKind::Decide => pending_decide = true,
                SkinKind::Result => pending_result = true,
                SkinKind::Play => unreachable!("play skin handled via spawn_play_skin_decode_for"),
            }
        } else {
            tracing::warn!(
                path = %path.display(),
                kind = ?kind,
                "skin path is not a supported beatoraja skin file; ignoring"
            );
        }
    }

    (pending_select, pending_decide, pending_result)
}

pub(super) fn apply_json_skin_sync(
    renderer: &mut Renderer,
    path: &Path,
    kind: SkinKind,
    default_manifest: Option<&SkinManifest>,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
    runtime_state: &bmz_skin::LuaLoadRuntimeState,
) -> Vec<ActiveSkinVideoSource> {
    let Some(manifest) = default_manifest else {
        tracing::warn!(
            path = %path.display(),
            kind = ?kind,
            "skipping skin install because default skin manifest is unavailable"
        );
        return Vec::new();
    };
    let decoded = match decode_beatoraja_skin_with_options_and_runtime_state(
        path,
        kind,
        options,
        files,
        runtime_state,
    ) {
        Ok(decoded) => decoded,
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                kind = ?kind,
                error = %format_error_chain(&error),
                "failed to decode beatoraja skin"
            );
            return Vec::new();
        }
    };
    let video_sources = skin_video_sources_from_decoded(&decoded);
    if let Err(error) = install_decoded_skin(renderer, decoded, manifest.clone()) {
        tracing::warn!(
            path = %path.display(),
            kind = ?kind,
            error = %format_error_chain(&error),
            "failed to install beatoraja skin"
        );
        return Vec::new();
    }
    video_sources
}

pub(super) fn skin_video_sources_from_decoded(decoded: &DecodedSkin) -> Vec<ActiveSkinVideoSource> {
    let enabled_options = decoded.document.enabled_options();
    decoded
        .sources
        .iter()
        .filter(|source| source.is_video)
        .map(|source| {
            let gating = skin_video_source_gating(&decoded.document, &source.source_id);
            ActiveSkinVideoSource {
                texture: source.texture,
                path: source.path.clone(),
                decoder: None,
                last_pts: None,
                loop_start_us: 0,
                active: gating.active,
                gating_op_sets: gating.op_sets,
                enabled_options: enabled_options.clone(),
                result_ranktime_ms: decoded.document.ranktime,
                failed: false,
            }
        })
        .collect()
}

pub(super) fn skin_video_sources_need_runtime_state(sources: &[ActiveSkinVideoSource]) -> bool {
    sources
        .iter()
        .any(|source| source.active && !source.failed && !source.gating_op_sets.is_empty())
}

pub(super) fn play_skin_video_draw_state(
    snapshot: &RenderSnapshot,
    skin_height: Option<u32>,
    note_lane_height_px: Option<i32>,
) -> bmz_render::skin::SkinDrawState {
    let play_elapsed_ms = time_us_to_skin_ms(snapshot.play_elapsed_time);
    let skin_height = skin_height.unwrap_or(1080).max(1) as f32;
    let note_lane_height = note_lane_height_px
        .filter(|height| *height > 0)
        .map_or(skin_height, |height| height as f32);
    let lift = snapshot.lift.clamp(0.0, 1.0);
    let lane_cover = crate::config::play::clamp_lane_cover_for_lift(snapshot.lane_cover, lift);
    let offset_lift_px = (lift * note_lane_height).round() as i32;
    let visible_lane_height = note_lane_height * (1.0 - lift);
    let offset_lanecover_px = (-(note_lane_height * lane_cover)).round() as i32;
    let offset_hidden_cover_px =
        (snapshot.hidden_cover.clamp(0.0, 1.0) * visible_lane_height).round() as i32;
    bmz_render::skin::SkinDrawState {
        elapsed_ms: play_elapsed_ms,
        operating_time_ms: snapshot.operating_time_ms,
        ready_timer_ms: snapshot.ready_elapsed_time.map(time_us_to_skin_ms),
        play_timer_ms: (snapshot.time.0 >= 0).then_some(time_us_to_skin_ms(snapshot.time)),
        rhythm_timer_ms: snapshot.rhythm_timer_elapsed_ms,
        quarter_note_elapsed_ms: snapshot.quarter_note_elapsed_ms,
        key_mode: snapshot.key_mode,
        combo: snapshot.combo,
        max_combo: snapshot.max_combo,
        ex_score: snapshot.ex_score,
        total_notes: snapshot.total_notes,
        past_notes: snapshot.past_notes,
        judge_counts: snapshot.judge_counts,
        fast_slow_counts: Some(snapshot.fast_slow_counts),
        gauge: snapshot.gauge,
        gauge_type: snapshot.gauge_type,
        gauge_auto_shift: snapshot.gauge_auto_shift,
        gauge_max: snapshot.gauge_max,
        gauge_border: snapshot.gauge_border,
        play_progress: play_skin_video_progress(snapshot),
        end_of_note: play_skin_video_end_of_note(snapshot),
        end_of_note_ms: snapshot.end_of_note_elapsed_ms,
        fadeout_ms: snapshot.fadeout_elapsed_ms,
        failed_ms: snapshot.failed_elapsed_ms,
        music_end_ms: snapshot.music_end_elapsed_ms,
        skin_offsets: snapshot.skin_offsets,
        hispeed: snapshot.hispeed,
        hispeed_mode_index: snapshot.hispeed_mode_index,
        target_green_number: snapshot.target_green_number,
        timeleft_ms: play_skin_video_timeleft_ms(snapshot),
        total_duration_ms: snapshot.note_display_duration_ms,
        duration_green_ms: Some(bmz_render::skin::duration_to_green_number_ms(
            snapshot.note_display_duration_ms,
        )),
        lane_cover: snapshot.lane_cover,
        lift: snapshot.lift,
        offset_lift_px,
        offset_lanecover_px,
        offset_hidden_cover_px,
        lane_cover_changing: snapshot.lane_cover_changing,
        lanecover_enabled: snapshot.lanecover_enabled,
        lift_enabled: snapshot.lift_enabled,
        hidden_enabled: snapshot.hidden_enabled,
        hispeed_auto_adjust: snapshot.hispeed_auto_adjust,
        hidden_cover: snapshot.hidden_cover,
        play_level: skin_video_play_level_number(&snapshot.play_level),
        table_song: !snapshot.table_text_primary.is_empty(),
        difficulty: skin_video_difficulty_code(&snapshot.difficulty_name),
        judge_rank: snapshot.judge_rank,
        now_bpm: snapshot.now_bpm,
        min_bpm: snapshot.min_bpm,
        max_bpm: snapshot.max_bpm,
        has_bga: snapshot.has_bga,
        has_bpm_stop: snapshot.has_bpm_stop,
        bga_enabled: snapshot.bga_enabled,
        has_backbmp: snapshot.backbmp_background,
        bga_stretch: snapshot.bga_stretch,
        best_ex_score: snapshot.best_ex_score,
        projected_best_ex_score: snapshot.projected_best_ex_score,
        target_ex_score: snapshot.target_ex_score,
        judge_timing_offset_ms: snapshot.judge_timing_offset_ms,
        judge_timing_auto_adjust: snapshot.judge_timing_auto_adjust,
        main_bpm: snapshot.main_bpm,
        hsfix_index: snapshot.hsfix_index,
        fs_threshold_ms: snapshot.fs_threshold_ms,
        adjusted_cover_progress: snapshot.adjusted_cover_progress,
        adjusted_rate: snapshot.adjusted_rate,
        adjusted_rate_adot: snapshot.adjusted_rate_adot,
        autoplay: snapshot.autoplay,
        play_screen: true,
        replay_playback: snapshot.replay_playback,
        practice_mode: snapshot.practice_mode,
        score_save_enabled: Some(snapshot.score_save_enabled),
        course_stage: snapshot.course_stage,
        hit_error_ring: snapshot.hit_error_ring.values,
        hit_error_ring_index: snapshot.hit_error_ring.index,
        skin_loaded: snapshot.ready_elapsed_time.is_some(),
        resource_load_progress: snapshot.resource_load_progress,
        ..bmz_render::skin::SkinDrawState::default()
    }
}

pub(super) fn time_us_to_skin_ms(time: TimeUs) -> i32 {
    (time.0 / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

pub(super) fn play_skin_video_progress(snapshot: &RenderSnapshot) -> f32 {
    if snapshot.duration.0 <= 0 {
        0.0
    } else {
        (snapshot.time.0 as f32 / snapshot.duration.0 as f32).clamp(0.0, 1.0)
    }
}

pub(super) fn play_skin_video_end_of_note(snapshot: &RenderSnapshot) -> bool {
    snapshot.duration.0 > 0 && snapshot.time.0 >= snapshot.duration.0
}

pub(super) fn play_skin_video_timeleft_ms(snapshot: &RenderSnapshot) -> i32 {
    (snapshot.duration.0.saturating_sub(snapshot.time.0) / 1_000)
        .saturating_add(1_000)
        .clamp(0, i32::MAX as i64) as i32
}

pub(super) fn skin_video_play_level_number(label: &str) -> i64 {
    let mut value = 0_i64;
    for digit in label.bytes().filter(u8::is_ascii_digit) {
        value = value.saturating_mul(10).saturating_add((digit - b'0') as i64);
    }
    value
}

pub(super) fn skin_video_difficulty_code(label: &str) -> i64 {
    let label = label.trim();
    match label {
        "1" => 1,
        "2" => 2,
        "3" => 3,
        "4" => 4,
        "5" => 5,
        _ if label.eq_ignore_ascii_case("BEGINNER") => 1,
        _ if label.eq_ignore_ascii_case("NORMAL") => 2,
        _ if label.eq_ignore_ascii_case("HYPER") => 3,
        _ if label.eq_ignore_ascii_case("ANOTHER") => 4,
        _ if label.eq_ignore_ascii_case("INSANE") => 5,
        _ => 0,
    }
}

/// 動画ソースの可視判定に必要なゲーティング情報。
pub(super) struct SkinVideoSourceGating {
    /// スキン config の option による静的な有効判定。
    pub(super) active: bool,
    /// このソースを参照する各 destination の op 条件。conditional destination の
    /// outer `if` 条件も合成済み。空なら参照されていない (= 常時可視)。
    pub(super) op_sets: Vec<Vec<i32>>,
}

pub(super) fn skin_video_source_gating(
    document: &SkinDocument,
    source_id: &str,
) -> SkinVideoSourceGating {
    let image_ids: HashSet<&str> = document
        .image
        .iter()
        .filter(|image| image.src == source_id)
        .map(|image| image.id.as_str())
        .collect();
    if image_ids.is_empty() {
        return SkinVideoSourceGating { active: true, op_sets: Vec::new() };
    }

    let mut render_object_ids = image_ids.clone();
    for imageset in &document.imageset {
        if imageset.images.iter().any(|id| image_ids.contains(id.as_str())) {
            render_object_ids.insert(imageset.id.as_str());
        }
    }

    let property_ops = skin_document_property_ops(document);
    let enabled_options = document.enabled_options();
    let mut referenced = false;
    let mut active = false;
    let mut op_sets = Vec::new();
    for (destination, op_set) in skin_document_destination_op_sets(document) {
        if !render_object_ids.contains(destination.id.as_str()) {
            continue;
        }
        referenced = true;
        if destination_property_ops_allow(&op_set, &enabled_options, &property_ops) {
            active = true;
        }
        op_sets.push(op_set);
    }
    if !referenced {
        return SkinVideoSourceGating { active: true, op_sets: Vec::new() };
    }
    SkinVideoSourceGating { active, op_sets }
}

pub(super) fn skin_document_property_ops(document: &SkinDocument) -> HashSet<i32> {
    document
        .property
        .iter()
        .flat_map(|property| property.item.iter().filter_map(|item| item.op.checked_abs()))
        .collect()
}

pub(super) fn apply_skin_video_source_enabled_options(
    sources: &mut [ActiveSkinVideoSource],
    enabled_options: &[i32],
    property_ops: &HashSet<i32>,
) {
    for source in sources {
        let was_active = source.active;
        source.enabled_options.clear();
        source.enabled_options.extend_from_slice(enabled_options);
        source.active =
            skin_video_source_static_active(&source.gating_op_sets, enabled_options, property_ops);
        if was_active && !source.active {
            source.decoder = None;
            source.last_pts = None;
        }
    }
}

pub(super) fn skin_video_source_static_active(
    op_sets: &[Vec<i32>],
    enabled_options: &[i32],
    property_ops: &HashSet<i32>,
) -> bool {
    op_sets.is_empty()
        || op_sets
            .iter()
            .any(|ops| destination_property_ops_allow(ops, enabled_options, property_ops))
}

/// 実行時 state に対して、動画ソースが現在のシーン状態で表示されるかどうかを判定する。
/// `op_sets` が空 (= destination から参照されていない) 場合は常時可視。
pub(super) fn skin_video_source_runtime_visible(
    source: &ActiveSkinVideoSource,
    state: &bmz_render::skin::SkinDrawState,
) -> bool {
    if source.gating_op_sets.is_empty() {
        return true;
    }
    source
        .gating_op_sets
        .iter()
        .any(|ops| bmz_render::skin::test_skin_ops(ops, &source.enabled_options, state))
}

pub(super) fn skin_document_destination_op_sets(
    document: &SkinDocument,
) -> Vec<(&SkinDestinationDef, Vec<i32>)> {
    document
        .destination
        .iter()
        .flat_map(|entry| match entry {
            DestinationListEntry::Single(destination) => {
                vec![(destination, destination.op.clone())]
            }
            DestinationListEntry::Conditional { if_ops, destinations } => destinations
                .iter()
                .map(|destination| {
                    let mut op_set = if_ops.clone();
                    op_set.extend(destination.op.iter().copied());
                    (destination, op_set)
                })
                .collect::<Vec<_>>(),
        })
        .collect()
}

pub(super) fn destination_property_ops_allow(
    ops: &[i32],
    enabled_options: &[i32],
    property_ops: &HashSet<i32>,
) -> bool {
    ops.iter().all(|op| {
        let Some(abs_op) = op.checked_abs() else {
            return true;
        };
        if !property_ops.contains(&abs_op) {
            return true;
        }
        if *op >= 0 { enabled_options.contains(op) } else { !enabled_options.contains(&abs_op) }
    })
}

pub(super) fn skin_path_options_need_full_reload(path: &Path) -> Result<bool> {
    if is_lua_skin_path(path) || is_lr2_skin_path(path) {
        return Ok(true);
    }
    if !is_json_skin_path(path) {
        return Ok(true);
    }
    json_skin_has_load_time_option_expansion(path)
}

pub(super) fn json_skin_has_load_time_option_expansion(path: &Path) -> Result<bool> {
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize skin root: {}", root.display()))?;
    let mut visited = HashSet::new();
    json_skin_file_has_load_time_option_expansion(path, &root, &mut visited)
}

pub(super) fn json_skin_file_has_load_time_option_expansion(
    path: &Path,
    root: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<bool> {
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize skin json: {}", path.display()))?;
    if !visited.insert(path.clone()) {
        return Ok(false);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read skin json: {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse skin json: {}", path.display()))?;
    let current_dir = path.parent().unwrap_or(root);
    json_skin_value_has_load_time_option_expansion(&value, current_dir, root, visited)
}

pub(super) fn json_skin_value_has_load_time_option_expansion(
    value: &serde_json::Value,
    current_dir: &Path,
    root: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<bool> {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                if json_skin_value_has_load_time_option_expansion(item, current_dir, root, visited)?
                {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        serde_json::Value::Object(object) => {
            if let Some(include) = object.get("include") {
                let include = include.as_str().with_context(|| {
                    format!("skin json include must be a string in {}", current_dir.display())
                })?;
                let included = current_dir
                    .join(include)
                    .canonicalize()
                    .with_context(|| format!("failed to canonicalize skin include: {include}"))?;
                anyhow::ensure!(
                    included.starts_with(root),
                    "skin include escapes skin root: {}",
                    included.display()
                );
                if json_skin_file_has_load_time_option_expansion(&included, root, visited)? {
                    return Ok(true);
                }
            }
            if object.contains_key("if")
                && (object.contains_key("value") || object.contains_key("values"))
            {
                return Ok(true);
            }
            for child in object.values() {
                if json_skin_value_has_load_time_option_expansion(
                    child,
                    current_dir,
                    root,
                    visited,
                )? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_skin_decode(
    tx: mpsc::Sender<PendingSkinResult>,
    source_cache: SharedSkinSourceAssetCache,
    document_cache: SharedSkinDocumentCache,
    texture_cache: SharedSkinGpuTextureCache,
    font_cache: SharedSkinFontCache,
    installed_font_cache: HashMap<String, SkinFontCacheKey>,
    generation: u64,
    path: PathBuf,
    kind: SkinKind,
    options: BTreeMap<String, String>,
    files: BTreeMap<String, String>,
    runtime_state: bmz_skin::LuaLoadRuntimeState,
) {
    let send_path = path.clone();
    let queued_at = Instant::now();
    thread::Builder::new()
        .name(format!("skin-decode-{:?}", kind))
        .spawn(move || {
            let decode_started_at = Instant::now();
            let result = decode_beatoraja_skin_with_options_and_runtime_state_and_caches(
                &path,
                kind,
                &options,
                &files,
                &runtime_state,
                Some(document_cache),
                Some(source_cache),
                Some(texture_cache),
                Some(font_cache),
                Some(installed_font_cache),
            );
            let decode_finished_at = Instant::now();
            let _ = tx.send(PendingSkinResult {
                generation,
                path: send_path,
                kind,
                queued_at,
                decode_started_at,
                decode_finished_at,
                result,
            });
        })
        .expect("failed to spawn skin decode thread");
}

/// upload worker のループ。decode 結果を受け取り、GPU アップロードして main へ返す。
/// decode 側 (`decode_rx`) が全て drop されるとループを抜ける (アプリ終了時)。
pub(super) fn skin_upload_worker(
    decode_rx: Receiver<PendingSkinResult>,
    upload_tx: mpsc::SyncSender<PendingUploadResult>,
    uploader: bmz_render::renderer::GpuUploader,
    texture_cache: SharedSkinGpuTextureCache,
    event_proxy: EventLoopProxy<AppUserEvent>,
) {
    while let Ok(PendingSkinResult {
        generation,
        path,
        kind,
        queued_at,
        decode_started_at,
        decode_finished_at,
        result,
    }) = decode_rx.recv()
    {
        let upload_started_at = Instant::now();
        let uploaded = result.map(|decoded| {
            upload_decoded_skin_with_texture_cache(&uploader, decoded, Some(&texture_cache))
        });
        let upload_finished_at = Instant::now();
        if upload_tx
            .send(PendingUploadResult {
                generation,
                path,
                kind,
                queued_at,
                decode_started_at,
                decode_finished_at,
                upload_started_at,
                upload_finished_at,
                uploaded,
            })
            .is_err()
        {
            // main 側受信端が drop された (アプリ終了)。
            break;
        }
        let _ = event_proxy.send_event(AppUserEvent::SkinUploadReady { sent_at: Instant::now() });
    }
}

pub(super) fn scan_skin_catalog(app_paths: &crate::paths::AppPaths) -> SkinCatalog {
    let mut catalog = SkinCatalog::default();
    let resource_skin_root = app_paths.resource_dir.join("skins");
    let data_skin_root = app_paths.data_dir.join("skins");
    scan_skin_catalog_dir(
        &resource_skin_root,
        &resource_skin_root,
        SkinCandidateOrigin::Bundled,
        &mut catalog,
    );
    if !same_path(&resource_skin_root, &data_skin_root) {
        scan_skin_catalog_dir(
            &data_skin_root,
            &data_skin_root,
            SkinCandidateOrigin::User,
            &mut catalog,
        );
    }
    sort_skin_catalog(&mut catalog);
    catalog
}

pub(super) fn scan_skin_catalog_dir(
    root: &Path,
    dir: &Path,
    origin: SkinCandidateOrigin,
    catalog: &mut SkinCatalog,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_skin_catalog_dir(root, &path, origin, catalog);
            continue;
        }
        if !is_skin_candidate_file(&path) {
            continue;
        }
        match load_skin_candidate(root, &path, origin) {
            Some((skin_type, candidate)) => push_skin_candidate(catalog, skin_type, candidate),
            None => {
                tracing::debug!(path = %path.display(), "skipping skin candidate without readable header")
            }
        }
    }
}

pub(super) fn play_skin_defs_from_path(
    app_paths: &crate::paths::AppPaths,
    path: &str,
) -> SceneSkinDefs {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return SceneSkinDefs::from_play_document(None);
    }
    let document =
        app_paths.resolve_path_ref(trimmed).ok().and_then(|path| load_skin_header_document(&path));
    SceneSkinDefs::from_play_document(document.as_ref())
}

pub(super) fn is_skin_candidate_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "json" | "luaskin" | "lr2skin"))
        .unwrap_or(false)
}

pub(super) fn load_skin_header_document(path: &Path) -> Option<SkinDocument> {
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("luaskin"))
    {
        bmz_skin::load_lua_skin_header_value(path)
            .ok()
            .and_then(|loaded| serde_json::from_value::<SkinDocument>(loaded.value).ok())
    } else if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("lr2skin"))
    {
        bmz_skin::load_lr2_csv_skin(
            path,
            bmz_skin::SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .ok()
        .map(|loaded| loaded.document)
    } else {
        SkinDocument::load_beatoraja_json(path).ok()
    }
}

pub(super) fn load_skin_candidate(
    root: &Path,
    path: &Path,
    origin: SkinCandidateOrigin,
) -> Option<(i32, SkinCandidate)> {
    let document = load_skin_header_document(path)?;
    let relative = path.strip_prefix(root).unwrap_or(path);
    let name = if document.name.trim().is_empty() {
        relative.file_stem().and_then(|name| name.to_str()).unwrap_or("").to_string()
    } else {
        document.name
    };
    let stable_path = match origin {
        SkinCandidateOrigin::Bundled => format!("resource:skins/{}", path_to_slash(relative)),
        SkinCandidateOrigin::User => format!("data:skins/{}", path_to_slash(relative)),
        SkinCandidateOrigin::External => path.to_string_lossy().replace('\\', "/"),
    };
    Some((document.skin_type, SkinCandidate { name, path: stable_path, origin }))
}

pub(super) fn same_path(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

pub(super) fn path_to_slash(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) const BMZ_SKIN_TYPE_PLAY_2KEYS: i32 = 21;
pub(super) const BMZ_SKIN_TYPE_PLAY_4KEYS: i32 = 22;
pub(super) const BMZ_SKIN_TYPE_PLAY_6KEYS: i32 = 23;
pub(super) const BMZ_SKIN_TYPE_PLAY_8KEYS: i32 = 24;

pub(super) fn push_skin_candidate(
    catalog: &mut SkinCatalog,
    skin_type: i32,
    candidate: SkinCandidate,
) {
    match skin_type {
        0 => catalog.play7.push(candidate),
        1 => catalog.play5.push(candidate),
        2 => catalog.play14.push(candidate),
        3 => catalog.play10.push(candidate),
        4 => catalog.play9.push(candidate),
        12 => catalog.battle7.push(candidate),
        13 => catalog.battle5.push(candidate),
        5 => catalog.select.push(candidate),
        6 => catalog.decide.push(candidate),
        7 => catalog.result.push(candidate),
        15 => catalog.course_result.push(candidate),
        BMZ_SKIN_TYPE_PLAY_4KEYS => catalog.play4.push(candidate),
        BMZ_SKIN_TYPE_PLAY_6KEYS => catalog.play6.push(candidate),
        BMZ_SKIN_TYPE_PLAY_8KEYS => catalog.play8.push(candidate),
        BMZ_SKIN_TYPE_PLAY_2KEYS => {}
        _ => {}
    }
}

pub(super) fn sort_skin_catalog(catalog: &mut SkinCatalog) {
    for candidates in [
        &mut catalog.select,
        &mut catalog.decide,
        &mut catalog.play4,
        &mut catalog.play5,
        &mut catalog.play6,
        &mut catalog.play7,
        &mut catalog.play8,
        &mut catalog.play9,
        &mut catalog.play10,
        &mut catalog.play14,
        &mut catalog.battle5,
        &mut catalog.battle7,
        &mut catalog.result,
        &mut catalog.course_result,
    ] {
        candidates.sort_by(|a, b| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
                .then_with(|| a.path.cmp(&b.path))
        });
        candidates.dedup_by(|a, b| a.path == b.path);
    }
}

pub(super) fn chart_asset_folder(chart: &PlayableChart) -> Option<PathBuf> {
    chart
        .sounds
        .iter()
        .find_map(|asset| asset.path.parent())
        .or_else(|| chart.bga_assets.iter().find_map(|asset| asset.path.parent()))
        .map(Path::to_path_buf)
}

pub(super) fn load_chart_meta_texture(
    renderer: &mut Renderer,
    texture_id: TextureId,
    folder_path: &str,
    relative: &str,
) -> Option<SkinImageSize> {
    let path = crate::chart_asset::resolve_chart_asset_path(folder_path, relative)?;
    match load_static_rgba_image(&path) {
        Ok(image) => {
            if let Err(error) = renderer.upsert_image_asset(texture_id, &image) {
                tracing::warn!(path = %path.display(), %error, "failed to upload chart meta image");
                None
            } else {
                Some(SkinImageSize { width: image.width as f32, height: image.height as f32 })
            }
        }
        Err(error) => {
            tracing::debug!(path = %path.display(), %error, "skipping chart meta image");
            None
        }
    }
}

pub(super) fn load_chart_bga_textures(
    renderer: &mut Renderer,
    chart: &PlayableChart,
) -> BgaFrameCatalog {
    use bmz_chart::model::BgaAssetKind;

    let total_start = Instant::now();
    let mut considered_assets = 0u32;
    let mut static_assets = 0u32;
    let mut skipped_non_static = 0u32;
    let mut loaded_assets = 0u32;
    let mut failed_assets = 0u32;
    let mut total_file_bytes = 0u64;
    let mut loaded_file_bytes = 0u64;
    let mut rgba_bytes = 0u64;
    let mut decode_us = 0u128;
    let mut upload_us = 0u128;
    let mut frames = BgaFrameCatalog::new();
    for asset in &chart.bga_assets {
        considered_assets += 1;
        let path = &asset.path;
        let file_bytes = std::fs::metadata(path).map(|metadata| metadata.len()).unwrap_or(0);
        total_file_bytes = total_file_bytes.saturating_add(file_bytes);
        if asset.kind != BgaAssetKind::Static {
            skipped_non_static += 1;
            tracing::debug!(
                asset_id = asset.id.0,
                path = %path.display(),
                "skipping non-static BGA asset (will be decoded at play time)"
            );
            continue;
        }
        static_assets += 1;

        let decode_start = Instant::now();
        match load_chart_bga_image(path) {
            Ok(image) => {
                let image_decode_us = decode_start.elapsed().as_micros();
                decode_us += image_decode_us;
                let texture_id = TextureId(bga_texture_id(asset.id));
                let frame = display_bga_frame(asset.id, image.width, image.height);
                let image_rgba_bytes = image.pixels.len() as u64;
                let upload_start = Instant::now();
                if let Err(error) = renderer.upsert_image_asset(texture_id, &image) {
                    let image_upload_us = upload_start.elapsed().as_micros();
                    upload_us += image_upload_us;
                    failed_assets += 1;
                    tracing::warn!(
                        asset_id = asset.id.0,
                        texture_id = texture_id.0,
                        file_bytes,
                        rgba_bytes = image_rgba_bytes,
                        decode_us = image_decode_us,
                        upload_us = image_upload_us,
                        path = %path.display(),
                        %error,
                        "failed to upload BGA image"
                    );
                } else {
                    let image_upload_us = upload_start.elapsed().as_micros();
                    upload_us += image_upload_us;
                    loaded_assets += 1;
                    loaded_file_bytes = loaded_file_bytes.saturating_add(file_bytes);
                    rgba_bytes = rgba_bytes.saturating_add(image_rgba_bytes);
                    tracing::info!(
                        asset_id = asset.id.0,
                        texture_id = texture_id.0,
                        width = image.width,
                        height = image.height,
                        file_bytes,
                        rgba_bytes = image_rgba_bytes,
                        decode_us = image_decode_us,
                        upload_us = image_upload_us,
                        path = %path.display(),
                        "loaded BGA image"
                    );
                    frames.insert(asset.id, frame);
                }
            }
            Err(error) => {
                let image_decode_us = decode_start.elapsed().as_micros();
                decode_us += image_decode_us;
                failed_assets += 1;
                tracing::warn!(
                    asset_id = asset.id.0,
                    file_bytes,
                    decode_us = image_decode_us,
                    path = %path.display(),
                    %error,
                    "skipping unreadable BGA image"
                );
            }
        }
    }
    tracing::info!(
        chart_bga_assets = considered_assets,
        static_assets,
        skipped_non_static,
        loaded_assets,
        failed_assets,
        total_file_bytes,
        loaded_file_bytes,
        rgba_bytes,
        decode_us,
        upload_us,
        total_us = total_start.elapsed().as_micros(),
        "chart BGA image load timing"
    );
    frames
}
