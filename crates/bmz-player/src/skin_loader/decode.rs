use super::*;

pub(super) enum SourceDecodeTask {
    File { index: usize, source_id: String, path: PathBuf },
    Video { index: usize, source_id: String, path: PathBuf },
    Builtin { index: usize, source_id: String, path: PathBuf, asset: RgbaImageAsset },
}

pub(super) struct DecodedSourceResult {
    pub(super) index: usize,
    pub(super) source_id: String,
    pub(super) path: PathBuf,
    pub(super) asset: Option<RgbaImageAsset>,
    pub(super) size: SkinImageSize,
    pub(super) is_video: bool,
    pub(super) cached_texture: Option<SkinTextureId>,
    pub(super) cache_key: Option<SkinSourceAssetCacheKey>,
    pub(super) source_status: Option<SourceCacheStatus>,
    pub(super) texture_status: Option<TextureCacheStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceCacheStatus {
    Hit,
    Miss,
    Uncacheable,
    Disabled,
}

pub(super) const MAX_SKIN_AUDIO_ASSETS: usize = 64;

pub(super) fn decode_skin_audio_assets(
    kind: SkinKind,
    skin_root: &Path,
    document: &SkinDocument,
) -> Vec<DecodedSkinAudio> {
    if kind != SkinKind::Result {
        return Vec::new();
    }
    let mut paths = document
        .scene_audio
        .iter()
        .chain(document.custom_events.iter().flat_map(|event| &event.audio_actions))
        .map(|action| action.path.clone())
        .filter(|path| !path.is_empty())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    paths.sort();
    if paths.len() > MAX_SKIN_AUDIO_ASSETS {
        tracing::warn!(
            count = paths.len(),
            limit = MAX_SKIN_AUDIO_ASSETS,
            "truncating excessive skin audio asset list"
        );
        paths.truncate(MAX_SKIN_AUDIO_ASSETS);
    }
    paths
        .into_par_iter()
        .filter_map(|path| {
            let Some(resolved) = resolve_skin_audio_path(skin_root, &path) else {
                tracing::warn!(path, "skipping invalid or missing skin audio asset");
                return None;
            };
            let mut loader = FfmpegSampleLoader::default();
            match loader.load(&resolved) {
                Ok(sample) => Some(DecodedSkinAudio { path, sample }),
                Err(error) => {
                    tracing::warn!(
                        path = %resolved.display(),
                        %error,
                        "failed to decode skin audio asset"
                    );
                    None
                }
            }
        })
        .collect()
}

pub(super) fn resolve_skin_audio_path(skin_root: &Path, path: &str) -> Option<PathBuf> {
    let normalized = path.replace('\\', "/");
    let relative = Path::new(&normalized);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return None;
    }
    let canonical_root = fs::canonicalize(skin_root).ok()?;
    let candidate = resolve_case_insensitive_path(&skin_root.join(relative));
    let canonical = fs::canonicalize(candidate).ok()?;
    canonical.is_file().then_some(())?;
    canonical.starts_with(canonical_root).then_some(canonical)
}

pub fn default_skin_root() -> PathBuf {
    resolve_app_paths()
        .map(|paths| paths.default_skin_root())
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/default"))
}

pub fn default_skin_root_from_paths(app_paths: &AppPaths) -> PathBuf {
    app_paths.default_skin_root()
}

pub fn apply_default_skin(renderer: &mut Renderer) -> Result<()> {
    let app_paths = resolve_app_paths()?;
    apply_default_skin_from_paths(renderer, &app_paths)
}

pub fn apply_default_skin_from_paths(renderer: &mut Renderer, app_paths: &AppPaths) -> Result<()> {
    let manifest = load_default_skin_into_renderer_from_paths(renderer, app_paths)?;
    let skin_path = default_play_skin_document_path_from_paths(app_paths, KeyMode::K7);
    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play)?;
    install_decoded_skin(renderer, decoded, manifest)
}

/// `profile.toml` の `[skin] play` 設定からスキンをロードする。
/// 空文字列 → デフォルト JSON スキン、`.json`/`.luaskin`/`.lua`/`.lr2skin`
/// 拡張子 → beatoraja スキンとして扱う。BMZ TOML skin directory は非対応。
pub fn apply_skin_from_config(
    renderer: &mut Renderer,
    app_paths: &AppPaths,
    play_skin_path: &str,
) -> Result<()> {
    if play_skin_path.is_empty() {
        return apply_default_skin_from_paths(renderer, app_paths);
    }
    let path = app_paths.resolve_path_ref(play_skin_path)?;
    if is_decodable_skin_path(&path) {
        apply_beatoraja_json_skin(renderer, &path)
    } else {
        anyhow::bail!(
            "unsupported skin path (BMZ TOML skin directories are no longer supported): {}",
            path.display()
        )
    }
}

pub fn apply_beatoraja_json_skin(renderer: &mut Renderer, skin_path: &Path) -> Result<()> {
    apply_beatoraja_json_skin_for_kind(renderer, skin_path, SkinKind::Play)
}

pub fn apply_beatoraja_select_json_skin(renderer: &mut Renderer, skin_path: &Path) -> Result<()> {
    apply_beatoraja_json_skin_for_kind(renderer, skin_path, SkinKind::Select)
}

pub fn apply_beatoraja_result_json_skin(renderer: &mut Renderer, skin_path: &Path) -> Result<()> {
    apply_beatoraja_json_skin_for_kind(renderer, skin_path, SkinKind::Result)
}

pub fn apply_beatoraja_decide_json_skin(renderer: &mut Renderer, skin_path: &Path) -> Result<()> {
    apply_beatoraja_json_skin_for_kind(renderer, skin_path, SkinKind::Decide)
}

pub(super) fn apply_beatoraja_json_skin_for_kind(
    renderer: &mut Renderer,
    skin_path: &Path,
    kind: SkinKind,
) -> Result<()> {
    let manifest = load_default_skin_into_renderer(renderer)?;
    let decoded = decode_beatoraja_skin(skin_path, kind)?;
    install_decoded_skin(renderer, decoded, manifest)
}

/// デフォルトスキンの manifest と PNG テクスチャを renderer に取り込む。
/// 起動時に 1 回だけ呼ばれることを想定 (同じテクスチャを複数回 upsert しても害は無いが無駄)。
pub fn load_default_skin_into_renderer(renderer: &mut Renderer) -> Result<SkinManifest> {
    let default_root = default_skin_root();
    load_default_skin_root_into_renderer(renderer, &default_root)
}

pub fn load_default_skin_into_renderer_from_paths(
    renderer: &mut Renderer,
    app_paths: &AppPaths,
) -> Result<SkinManifest> {
    let default_root = default_skin_root_from_paths(app_paths);
    load_default_skin_root_into_renderer(renderer, &default_root)
}

pub(super) fn load_default_skin_root_into_renderer(
    renderer: &mut Renderer,
    default_root: &Path,
) -> Result<SkinManifest> {
    let manifest = default_skin_manifest_for_root(default_root);

    for texture in manifest.resolve_textures(default_root) {
        renderer.load_png_texture(texture.id, &texture.path).with_context(|| {
            format!(
                "failed to load default skin texture {}: {}",
                texture.id.0,
                texture.path.display()
            )
        })?;
    }
    Ok(manifest)
}

/// beatoraja JSON skin の document/フォント/PNG ソースを並列にデコードする。
/// Renderer には触らないので Send-safe で、別スレッドからも呼べる。
pub fn decode_beatoraja_skin(skin_path: &Path, kind: SkinKind) -> Result<DecodedSkin> {
    decode_beatoraja_skin_with_options(skin_path, kind, &BTreeMap::new(), &BTreeMap::new())
}

/// `decode_beatoraja_skin` のカスタマイズオプション / ファイル選択付き版。
///
/// `options` はオプション名 -> 選択肢名の対応。JSON スキンは選択肢の `op`
/// コード列へ、Lua スキンはそのまま渡して展開する。
///
/// `files` は filepath 定義名 -> 選択ファイルのスキンルート相対パスの対応。
/// Lua スキンは `skin_config.get_path` の解決へ、JSON スキンは `source` /
/// `font` のワイルドカード解決へ反映する。
pub fn decode_beatoraja_skin_with_options(
    skin_path: &Path,
    kind: SkinKind,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
) -> Result<DecodedSkin> {
    decode_beatoraja_skin_with_options_and_runtime_state(
        skin_path,
        kind,
        options,
        files,
        &LuaLoadRuntimeState::default(),
    )
}

pub fn decode_beatoraja_skin_with_options_and_runtime_state(
    skin_path: &Path,
    kind: SkinKind,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
    runtime_state: &LuaLoadRuntimeState,
) -> Result<DecodedSkin> {
    decode_beatoraja_skin_with_options_and_runtime_state_and_source_cache(
        skin_path,
        kind,
        options,
        files,
        runtime_state,
        None,
        None,
    )
}

pub fn decode_beatoraja_skin_with_options_and_runtime_state_and_source_cache(
    skin_path: &Path,
    kind: SkinKind,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
    runtime_state: &LuaLoadRuntimeState,
    source_cache: Option<SharedSkinSourceAssetCache>,
    font_cache: Option<SharedSkinFontCache>,
) -> Result<DecodedSkin> {
    decode_beatoraja_skin_with_options_and_runtime_state_and_caches(
        skin_path,
        kind,
        options,
        files,
        runtime_state,
        None,
        source_cache,
        None,
        font_cache,
        None,
    )
}

pub fn decode_beatoraja_skin_with_options_and_runtime_state_and_caches(
    skin_path: &Path,
    kind: SkinKind,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
    runtime_state: &LuaLoadRuntimeState,
    document_cache: Option<SharedSkinDocumentCache>,
    source_cache: Option<SharedSkinSourceAssetCache>,
    texture_cache: Option<SharedSkinGpuTextureCache>,
    font_cache: Option<SharedSkinFontCache>,
    installed_fonts: Option<HashMap<String, SkinFontCacheKey>>,
) -> Result<DecodedSkin> {
    let document_start = Instant::now();
    let LoadedSkinDocumentForDecode {
        mut document,
        lua_runtime,
        files: resolved_files,
        cache_status,
    } = load_skin_document(skin_path, kind, options, files, runtime_state, document_cache)?;
    let document_us = elapsed_us(document_start);
    // フォント ID は scene 横断的に Renderer のグローバルマップに登録されるので、
    // play / select / result で同じ "0" 等が衝突する。namespace を付与して隔離する。
    // text 定義の font 参照側も同じ namespace を付ける。
    let font_namespace = kind.font_namespace();
    for text in &mut document.text {
        if !text.font.is_empty() {
            text.font = format!("{}:{}", font_namespace, text.font);
        }
    }
    let skin_root = skin_path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();
    let audio_assets = decode_skin_audio_assets(kind, &skin_root, &document);
    let required_sources: HashSet<String> =
        required_skin_source_ids(&document).into_iter().map(str::to_string).collect();
    let warn_missing_required = kind.warn_missing_required_sources();

    // フォントを並列にデコードする。
    let font_tasks: Vec<_> = document
        .font
        .iter()
        .filter_map(|font| {
            if font.id.is_empty() || font.path.is_empty() {
                return None;
            }
            let font_path =
                resolve_json_skin_asset_path(&skin_root, &font.path, &document, &resolved_files)?;
            if !is_supported_font_path(&font_path) {
                tracing::debug!(
                    font_id = %font.id,
                    path = %font_path.display(),
                    "skipping unsupported beatoraja skin font"
                );
                return None;
            }
            let stored_id = format!("{}:{}", font_namespace, font.id);
            Some((stored_id, font_path))
        })
        .collect();

    let font_count = font_tasks.len();
    let font_decode_start = Instant::now();
    let decoded_fonts: Vec<(DecodedFont, FontCacheStatus)> = font_tasks
        .into_par_iter()
        .filter_map(|(stored_id, font_path)| {
            let cache_key = skin_font_cache_key(&font_path);
            if let (Some(installed_fonts), Some(cache_key)) =
                (installed_fonts.as_ref(), cache_key.as_ref())
                && installed_fonts.get(&stored_id) == Some(cache_key)
            {
                return Some((
                    DecodedFont {
                        stored_id,
                        path: font_path,
                        data: None,
                        cache_key: Some(cache_key.clone()),
                    },
                    FontCacheStatus::SkippedInstalled,
                ));
            }
            match decode_font_with_cache_key(&font_path, font_cache.as_ref(), cache_key) {
                Ok((data, status, cache_key)) => Some((
                    DecodedFont { stored_id, path: font_path, data: Some(data), cache_key },
                    status,
                )),
                Err(error) => {
                    tracing::warn!(
                        font_id = %stored_id,
                        path = %font_path.display(),
                        %error,
                        "failed to load beatoraja skin font"
                    );
                    None
                }
            }
        })
        .collect();
    let font_decode_us = elapsed_us(font_decode_start);
    let mut font_payload_skipped = 0;
    let mut font_cache_hits = 0;
    let mut font_cache_misses = 0;
    let mut font_cache_uncacheable = 0;
    let mut font_cache_disabled = 0;
    for (_, status) in &decoded_fonts {
        match status {
            FontCacheStatus::Hit => font_cache_hits += 1,
            FontCacheStatus::Miss => font_cache_misses += 1,
            FontCacheStatus::SkippedInstalled => font_payload_skipped += 1,
            FontCacheStatus::Uncacheable => font_cache_uncacheable += 1,
            FontCacheStatus::Disabled => font_cache_disabled += 1,
        }
    }
    let fonts: Vec<DecodedFont> = decoded_fonts.into_iter().map(|(font, _)| font).collect();

    // ソースは ID 順を保つため、まず resolved path リストを順次組み立て、
    // 静止画/動画先頭フレームのデコード本体だけを並列実行する。
    let source_tasks: Vec<SourceDecodeTask> = document
        .source
        .iter()
        .enumerate()
        .filter_map(|(index, source)| {
            if let Some(asset) = lr2_builtin_source_asset(&source.path) {
                return Some(SourceDecodeTask::Builtin {
                    index,
                    source_id: source.id.clone(),
                    path: PathBuf::from(&source.path),
                    asset,
                });
            }
            let source_path = resolve_json_skin_source_path(
                &skin_root,
                &source.path,
                &document,
                &resolved_files,
            )?;
            let extension = source_path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            if is_skin_static_source_extension(&extension) {
                return Some(SourceDecodeTask::File {
                    index,
                    source_id: source.id.clone(),
                    path: source_path,
                });
            }
            if is_skin_video_source_extension(&extension) {
                return Some(SourceDecodeTask::Video {
                    index,
                    source_id: source.id.clone(),
                    path: source_path,
                });
            }
            {
                tracing::debug!(
                    source_id = %source.id,
                    path = %source_path.display(),
                    "skipping unsupported beatoraja skin source"
                );
                None
            }
        })
        .collect();

    let source_task_count = source_tasks.len();
    let source_decode_start = Instant::now();
    let mut decoded_pairs: Vec<DecodedSourceResult> = source_tasks
        .into_par_iter()
        .filter_map(|task| match task {
            SourceDecodeTask::Builtin { index, source_id, path, asset } => {
                let size = SkinImageSize { width: asset.width as f32, height: asset.height as f32 };
                Some(DecodedSourceResult {
                    index,
                    source_id,
                    path,
                    asset: Some(asset),
                    size,
                    is_video: false,
                    cached_texture: None,
                    cache_key: None,
                    source_status: None,
                    texture_status: None,
                })
            }
            SourceDecodeTask::File { index, source_id, path: source_path } => {
                let (cached_texture, cache_key, texture_status) =
                    lookup_source_texture_cache(texture_cache.as_ref(), &source_path, false);
                if let Some(cached_texture) = cached_texture {
                    return Some(DecodedSourceResult {
                        index,
                        source_id,
                        path: source_path,
                        asset: None,
                        size: cached_texture.size,
                        is_video: false,
                        cached_texture: Some(cached_texture.texture),
                        cache_key,
                        source_status: None,
                        texture_status: Some(texture_status),
                    });
                }
                match load_source_asset_with_cache(
                    &source_path,
                    false,
                    source_cache.as_ref(),
                    || load_static_rgba_image(&source_path),
                ) {
                    Ok((asset, status)) => {
                        let size = SkinImageSize {
                            width: asset.width as f32,
                            height: asset.height as f32,
                        };
                        Some(DecodedSourceResult {
                            index,
                            source_id,
                            path: source_path,
                            asset: Some(asset),
                            size,
                            is_video: false,
                            cached_texture: None,
                            cache_key,
                            source_status: Some(status),
                            texture_status: Some(texture_status),
                        })
                    }
                    Err(error) => {
                        if warn_missing_required && required_sources.contains(&source_id) {
                            tracing::warn!(
                                source_id = %source_id,
                                path = %source_path.display(),
                                %error,
                                "failed to load beatoraja skin source"
                            );
                        } else {
                            tracing::debug!(
                                source_id = %source_id,
                                path = %source_path.display(),
                                %error,
                                "skipping unused missing beatoraja skin source"
                            );
                        }
                        None
                    }
                }
            }
            SourceDecodeTask::Video { index, source_id, path: source_path } => {
                let (cached_texture, cache_key, texture_status) =
                    lookup_source_texture_cache(texture_cache.as_ref(), &source_path, true);
                if let Some(cached_texture) = cached_texture {
                    return Some(DecodedSourceResult {
                        index,
                        source_id,
                        path: source_path,
                        asset: None,
                        size: cached_texture.size,
                        is_video: true,
                        cached_texture: Some(cached_texture.texture),
                        cache_key,
                        source_status: None,
                        texture_status: Some(texture_status),
                    });
                }
                match load_source_asset_with_cache(
                    &source_path,
                    true,
                    source_cache.as_ref(),
                    || load_skin_video_first_frame_rgba(&source_path),
                ) {
                    Ok((asset, status)) => {
                        let size = SkinImageSize {
                            width: asset.width as f32,
                            height: asset.height as f32,
                        };
                        Some(DecodedSourceResult {
                            index,
                            source_id,
                            path: source_path,
                            asset: Some(asset),
                            size,
                            is_video: true,
                            cached_texture: None,
                            cache_key,
                            source_status: Some(status),
                            texture_status: Some(texture_status),
                        })
                    }
                    Err(error) => {
                        tracing::warn!(
                            source_id = %source_id,
                            path = %source_path.display(),
                            %error,
                            "failed to load beatoraja skin video source"
                        );
                        None
                    }
                }
            }
        })
        .collect();
    let source_decode_us = elapsed_us(source_decode_start);
    decoded_pairs.sort_by_key(|decoded| decoded.index);

    let mut stats = SkinDecodeStats {
        document_us,
        document_cache_hits: usize::from(cache_status == DocumentCacheStatus::Hit),
        document_cache_misses: usize::from(cache_status == DocumentCacheStatus::Miss),
        document_cache_uncacheable: usize::from(cache_status == DocumentCacheStatus::Uncacheable),
        document_cache_disabled: usize::from(cache_status == DocumentCacheStatus::Disabled),
        font_count,
        font_decode_us,
        font_payload_skipped,
        font_cache_hits,
        font_cache_misses,
        font_cache_uncacheable,
        font_cache_disabled,
        source_task_count,
        source_decode_us,
        ..Default::default()
    };
    for decoded in &decoded_pairs {
        stats.decoded_source_count += 1;
        if let Some(asset) = &decoded.asset {
            stats.decoded_source_bytes =
                stats.decoded_source_bytes.saturating_add(asset.pixels.len());
        }
        if matches!(decoded.texture_status, Some(TextureCacheStatus::Hit)) {
            stats.source_texture_cache_hits += 1;
            let bytes = (decoded.size.width.max(0.0) as usize)
                .saturating_mul(decoded.size.height.max(0.0) as usize)
                .saturating_mul(4);
            stats.source_texture_cache_hit_bytes =
                stats.source_texture_cache_hit_bytes.saturating_add(bytes);
            if decoded.is_video {
                stats.video_source_texture_cache_hits += 1;
                stats.video_source_texture_cache_hit_bytes =
                    stats.video_source_texture_cache_hit_bytes.saturating_add(bytes);
            }
        }
        match (decoded.is_video, &decoded.source_status, &decoded.texture_status) {
            (_, None, None) => stats.builtin_source_count += 1,
            (true, None, Some(TextureCacheStatus::Hit)) => stats.video_source_count += 1,
            (false, None, Some(TextureCacheStatus::Hit)) => stats.image_source_count += 1,
            (true, Some(_), _) => stats.video_source_count += 1,
            (false, Some(_), _) => stats.image_source_count += 1,
            (_, None, Some(_)) => {}
        }
        match decoded.source_status {
            Some(SourceCacheStatus::Hit) => {
                stats.source_cache_hits += 1;
                if decoded.is_video {
                    stats.video_source_cache_hits += 1;
                }
            }
            Some(SourceCacheStatus::Miss) => {
                stats.source_cache_misses += 1;
                if decoded.is_video {
                    stats.video_source_cache_misses += 1;
                }
            }
            Some(SourceCacheStatus::Uncacheable) => {
                stats.source_cache_uncacheable += 1;
                if decoded.is_video {
                    stats.video_source_cache_uncacheable += 1;
                }
            }
            Some(SourceCacheStatus::Disabled) => {
                stats.source_cache_disabled += 1;
                if decoded.is_video {
                    stats.video_source_cache_disabled += 1;
                }
            }
            None => {}
        }
    }

    let mut next_texture_id = kind.first_texture_id();
    let sources: Vec<DecodedSource> = decoded_pairs
        .into_iter()
        .map(|decoded| {
            let texture = decoded.cached_texture.unwrap_or_else(|| {
                let texture = SkinTextureId(next_texture_id);
                next_texture_id += 1;
                texture
            });
            DecodedSource {
                source_id: decoded.source_id,
                path: decoded.path,
                texture,
                asset: decoded.asset,
                size: decoded.size,
                cache_key: decoded.cache_key,
                is_video: decoded.is_video,
            }
        })
        .collect();

    Ok(DecodedSkin { kind, document, lua_runtime, fonts, sources, audio_assets, stats })
}

pub(super) fn lr2_builtin_source_asset(path: &str) -> Option<RgbaImageAsset> {
    if path == "bmz://lr2/judgedetail" {
        return Some(lr2_judge_detail_asset());
    }
    let pixel = match path {
        "bmz://lr2/black" => [0, 0, 0, 255],
        "bmz://lr2/white" => [255, 255, 255, 255],
        // BACKBMP itself is drawn by the play snapshot path.  Keep a transparent
        // source so LR2 CSV objects using IMAGE_BACKBMP can be decoded without
        // failing texture resolution when the chart has no backbmp.
        "bmz://lr2/backbmp" => [0, 0, 0, 0],
        _ => return None,
    };
    Some(RgbaImageAsset { width: 1, height: 1, pixels: pixel.to_vec() })
}

pub(super) fn lr2_judge_detail_asset() -> RgbaImageAsset {
    const WIDTH: u32 = 120;
    const HEIGHT: u32 = 100;
    let mut pixels = vec![0; (WIDTH * HEIGHT * 4) as usize];
    draw_lr2_bitmap_text(&mut pixels, WIDTH, 5, 5, "EARLY", [255, 255, 255, 255]);
    draw_lr2_bitmap_text(&mut pixels, WIDTH, 59, 5, "LATE", [255, 255, 255, 255]);
    for (pair, color) in [[255, 255, 255, 255], [255, 192, 64, 255]].into_iter().enumerate() {
        for row in 0..2 {
            let y = 20 + pair as u32 * 40 + row * 20;
            for digit in 0..10 {
                draw_lr2_bitmap_glyph(
                    &mut pixels,
                    WIDTH,
                    digit as u32 * 10 + 2,
                    y + 5,
                    char::from(b'0' + digit as u8),
                    color,
                );
            }
            draw_lr2_bitmap_glyph(
                &mut pixels,
                WIDTH,
                112,
                y + 5,
                if row == 0 { '+' } else { '-' },
                color,
            );
        }
    }
    RgbaImageAsset { width: WIDTH, height: HEIGHT, pixels }
}

pub(super) fn draw_lr2_bitmap_text(
    pixels: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    text: &str,
    color: [u8; 4],
) {
    for (index, character) in text.chars().enumerate() {
        draw_lr2_bitmap_glyph(pixels, width, x + index as u32 * 8, y, character, color);
    }
}

pub(super) fn draw_lr2_bitmap_glyph(
    pixels: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    character: char,
    color: [u8; 4],
) {
    let rows = lr2_bitmap_glyph(character);
    for (row, bits) in rows.into_iter().enumerate() {
        for column in 0..3 {
            if bits & (1 << (2 - column)) == 0 {
                continue;
            }
            for dy in 0..2 {
                for dx in 0..2 {
                    let px = x + column * 2 + dx;
                    let py = y + row as u32 * 2 + dy;
                    let offset = ((py * width + px) * 4) as usize;
                    if let Some(target) = pixels.get_mut(offset..offset + 4) {
                        target.copy_from_slice(&color);
                    }
                }
            }
        }
    }
}

pub(super) fn lr2_bitmap_glyph(character: char) -> [u8; 5] {
    match character {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        _ => [0; 5],
    }
}

pub(super) fn is_skin_video_source_extension(extension: &str) -> bool {
    matches!(extension, "mp4" | "wmv" | "m4v" | "webm" | "mpg" | "mpeg" | "m1v" | "m2v" | "avi")
}

pub(super) fn is_skin_static_source_extension(extension: &str) -> bool {
    matches!(extension, "png" | "bmp" | "jpg" | "jpeg" | "gif" | "tga" | "cim")
}

pub(super) fn load_source_asset_with_cache<F>(
    path: &Path,
    is_video: bool,
    source_cache: Option<&SharedSkinSourceAssetCache>,
    load: F,
) -> Result<(RgbaImageAsset, SourceCacheStatus)>
where
    F: FnOnce() -> Result<RgbaImageAsset>,
{
    let Some(source_cache) = source_cache else {
        return load().map(|asset| (asset, SourceCacheStatus::Disabled));
    };
    let Some(key) = skin_source_asset_cache_key(path, is_video) else {
        return load().map(|asset| (asset, SourceCacheStatus::Uncacheable));
    };
    if let Ok(cache) = source_cache.lock()
        && let Some(asset) = cache.get(&key)
    {
        return Ok((asset, SourceCacheStatus::Hit));
    }
    let asset = load()?;
    if let Ok(mut cache) = source_cache.lock() {
        cache.insert(key, asset.clone());
    }
    Ok((asset, SourceCacheStatus::Miss))
}

pub(super) fn lookup_source_texture_cache(
    texture_cache: Option<&SharedSkinGpuTextureCache>,
    path: &Path,
    is_video: bool,
) -> (Option<CachedSkinGpuTexture>, Option<SkinSourceAssetCacheKey>, TextureCacheStatus) {
    let key = skin_source_asset_cache_key(path, is_video);
    match (texture_cache, key.as_ref()) {
        (Some(texture_cache), Some(key)) => {
            if let Ok(cache) = texture_cache.lock()
                && let Some(texture) = cache.get(key)
            {
                return (Some(texture), Some(key.clone()), TextureCacheStatus::Hit);
            }
            (None, Some(key.clone()), TextureCacheStatus::Miss)
        }
        (Some(_), None) => (None, None, TextureCacheStatus::Uncacheable),
        (None, _) => (None, key, TextureCacheStatus::Disabled),
    }
}

pub(super) fn elapsed_us(start: Instant) -> u64 {
    start.elapsed().as_micros().min(u64::MAX as u128) as u64
}

pub(super) fn skin_source_asset_cache_key(
    path: &Path,
    is_video: bool,
) -> Option<SkinSourceAssetCacheKey> {
    let metadata = fs::metadata(path).ok()?;
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Some(SkinSourceAssetCacheKey {
        path,
        modified: metadata.modified().ok(),
        len: metadata.len(),
        is_video,
    })
}

pub(super) fn skin_document_cache_key(path: &Path, kind: SkinKind) -> Option<SkinDocumentCacheKey> {
    let metadata = fs::metadata(path).ok()?;
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Some(SkinDocumentCacheKey {
        path,
        kind,
        modified: metadata.modified().ok(),
        len: metadata.len(),
    })
}

/// beatoraja の設定ファイルを読む Lua スキン向けの、個人情報を含まない読取専用設定。
///
/// ホスト側の beatoraja 設定や BMZ の入力割当は公開せず、入力監視は BMZ のイベント処理を
/// 正とする。各 mode の空設定は WMII の設定探索を安全に完了させるためだけに供給する。
pub(super) fn lua_compat_virtual_io_files() -> BTreeMap<String, String> {
    const PLAYER_CONFIG: &str = concat!(
        "{",
        "\"mode5\":{\"keyboard\":{},\"controller\":[],\"midi\":{}},",
        "\"mode7\":{\"keyboard\":{},\"controller\":[],\"midi\":{}},",
        "\"mode9\":{\"keyboard\":{},\"controller\":[],\"midi\":{}},",
        "\"mode10\":{\"keyboard\":{},\"controller\":[],\"midi\":{}},",
        "\"mode14\":{\"keyboard\":{},\"controller\":[],\"midi\":{}},",
        "\"mode24\":{\"keyboard\":{},\"controller\":[],\"midi\":{}},",
        "\"mode24double\":{\"keyboard\":{},\"controller\":[],\"midi\":{}}",
        "}",
    );
    BTreeMap::from([
        ("config_sys.json".to_string(), "{\"playername\":\"bmz\"}".to_string()),
        ("player/bmz/config_player.json".to_string(), PLAYER_CONFIG.to_string()),
    ])
}

pub(super) fn lua_virtual_io_files(
    runtime_state: &LuaLoadRuntimeState,
) -> BTreeMap<String, String> {
    let mut files = lua_compat_virtual_io_files();
    files.extend(runtime_state.virtual_io_files.clone());
    files
}

pub(super) fn lr2_document_dependency_fingerprint(
    skin_path: &Path,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
    dependencies: &SkinLoadDependencies,
) -> Result<SkinDocumentDependencyFingerprint> {
    let option_values = bmz_skin::load_lr2_csv_skin_dependency_option_values(
        skin_path,
        options,
        dependencies.option_values.keys().copied(),
    )?;
    let file_values = dependencies
        .files
        .iter()
        .map(|name| (name.clone(), files.get(name).cloned().unwrap_or_default()))
        .collect();
    let loaded_files = current_loaded_file_dependencies(&dependencies.loaded_files)
        .context("failed to inspect lr2 skin loaded file dependencies")?;
    Ok(SkinDocumentDependencyFingerprint {
        number_values: BTreeMap::new(),
        text_values: BTreeMap::new(),
        option_values,
        event_index_values: BTreeMap::new(),
        offset_values: BTreeMap::new(),
        offset_id_values: BTreeMap::new(),
        file_values,
        loaded_files,
        virtual_io_files: BTreeMap::new(),
    })
}

pub(super) fn document_dependency_fingerprint(
    document: &SkinDocument,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
    runtime_state: &LuaLoadRuntimeState,
    dependencies: &SkinLoadDependencies,
) -> Option<SkinDocumentDependencyFingerprint> {
    let enabled_options = enabled_options_from_selections(document, options);
    let property_ops = document_property_ops(document);
    let number_values = dependencies
        .number_values
        .keys()
        .map(|ref_id| {
            let value = runtime_state.number_values.get(ref_id).copied().unwrap_or_default();
            (*ref_id, value)
        })
        .collect();
    let text_values = dependencies
        .text_values
        .keys()
        .map(|ref_id| {
            let value = runtime_state.text_values.get(ref_id).cloned().unwrap_or_default();
            (*ref_id, value)
        })
        .collect();
    let option_values = dependencies
        .option_values
        .keys()
        .map(|option_id| {
            let value = if property_ops.contains(option_id) {
                enabled_options.contains(option_id)
            } else {
                runtime_state.option_values.get(option_id).copied().unwrap_or(false)
            };
            (*option_id, value)
        })
        .collect();
    let event_index_values = dependencies
        .event_index_values
        .keys()
        .map(|event_id| {
            let value = runtime_state.event_index_values.get(event_id).copied().unwrap_or_default();
            (*event_id, value)
        })
        .collect();
    let file_values = dependencies
        .files
        .iter()
        .map(|name| (name.clone(), files.get(name).cloned().unwrap_or_default()))
        .collect();
    let loaded_files = current_loaded_file_dependencies(&dependencies.loaded_files).ok()?;
    let virtual_files = lua_virtual_io_files(runtime_state);
    let virtual_io_files = dependencies
        .virtual_io_files
        .keys()
        .map(|path| (path.clone(), virtual_files.get(path).cloned()))
        .collect();
    Some(SkinDocumentDependencyFingerprint {
        number_values,
        text_values,
        option_values,
        event_index_values,
        offset_values: runtime_state.offset_values.clone(),
        offset_id_values: runtime_state.offset_id_values.clone(),
        file_values,
        loaded_files,
        virtual_io_files,
    })
}

pub(super) fn document_property_ops(document: &SkinDocument) -> HashSet<i32> {
    document.property.iter().flat_map(|property| property.item.iter().map(|item| item.op)).collect()
}

pub(super) fn current_loaded_file_dependencies(
    loaded_files: &BTreeMap<PathBuf, SkinLoadedFileDependency>,
) -> Result<BTreeMap<PathBuf, SkinLoadedFileDependency>> {
    let mut result = BTreeMap::new();
    for path in loaded_files.keys() {
        let metadata = fs::metadata(path)
            .with_context(|| format!("failed to read loaded lua skin file: {}", path.display()))?;
        let path = fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        result.insert(
            path,
            SkinLoadedFileDependency { modified: metadata.modified().ok(), len: metadata.len() },
        );
    }
    Ok(result)
}

pub(super) fn load_skin_video_first_frame_rgba(path: &Path) -> Result<RgbaImageAsset> {
    let frame = bmz_video::decode_first_frame(path)
        .with_context(|| format!("failed to decode first video frame: {}", path.display()))?;
    Ok(RgbaImageAsset { width: frame.width, height: frame.height, pixels: frame.rgba })
}

pub(super) struct LoadedSkinDocumentForDecode {
    pub(super) document: SkinDocument,
    pub(super) lua_runtime: Option<LuaSkinRuntime>,
    pub(super) files: BTreeMap<String, String>,
    pub(super) cache_status: DocumentCacheStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DocumentCacheStatus {
    Hit,
    Miss,
    Uncacheable,
    Disabled,
}

pub(super) fn load_skin_document(
    skin_path: &Path,
    kind: SkinKind,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
    runtime_state: &LuaLoadRuntimeState,
    document_cache: Option<SharedSkinDocumentCache>,
) -> Result<LoadedSkinDocumentForDecode> {
    if is_lr2_skin_path(skin_path)
        && let Some(document_cache) = document_cache.as_ref()
        && let Some(key) = skin_document_cache_key(skin_path, kind)
    {
        if let Ok(mut cache) = document_cache.lock()
            && let Some((mut document, mut resolved_files)) =
                cache.get_lr2(&key, skin_path, options, files)
        {
            for (name, selected) in files {
                resolved_files.insert(name.clone(), selected.clone());
            }
            document.user_selected_options =
                Some(enabled_options_from_selections(&document, options));
            return Ok(LoadedSkinDocumentForDecode {
                document,
                lua_runtime: None,
                files: resolved_files,
                cache_status: DocumentCacheStatus::Hit,
            });
        }
        let mut loaded =
            load_skin_document_uncached(skin_path, kind, options, files, runtime_state)?;
        loaded.cache_status = DocumentCacheStatus::Miss;
        if let Ok(mut cache) = document_cache.lock()
            && let Ok(fingerprint) =
                lr2_document_dependency_fingerprint(skin_path, options, files, &loaded.dependencies)
        {
            cache.insert_lr2(
                key,
                fingerprint,
                loaded.document.clone(),
                loaded.files.clone(),
                loaded.dependencies,
            );
        }
        return Ok(LoadedSkinDocumentForDecode {
            document: loaded.document,
            lua_runtime: loaded.lua_runtime,
            files: loaded.files,
            cache_status: loaded.cache_status,
        });
    }
    if is_lua_skin_path(skin_path)
        && let Some(document_cache) = document_cache.as_ref()
        && let Some(key) = skin_document_cache_key(skin_path, kind)
    {
        if let Ok(mut cache) = document_cache.lock()
            && let Some((mut document, mut resolved_files)) =
                cache.get_lua(&key, options, files, runtime_state)
        {
            for (name, selected) in files {
                resolved_files.insert(name.clone(), selected.clone());
            }
            document.user_selected_options =
                Some(enabled_options_from_selections(&document, options));
            return Ok(LoadedSkinDocumentForDecode {
                document,
                lua_runtime: None,
                files: resolved_files,
                cache_status: DocumentCacheStatus::Hit,
            });
        }
        let mut loaded =
            load_skin_document_uncached(skin_path, kind, options, files, runtime_state)?;
        loaded.cache_status = DocumentCacheStatus::Miss;
        if let Ok(mut cache) = document_cache.lock()
            && let Some(fingerprint) = document_dependency_fingerprint(
                &loaded.document,
                options,
                files,
                runtime_state,
                &loaded.dependencies,
            )
        {
            cache.insert_lua(
                key,
                fingerprint,
                loaded.document.clone(),
                loaded.files.clone(),
                loaded.dependencies,
            );
        }
        return Ok(LoadedSkinDocumentForDecode {
            document: loaded.document,
            lua_runtime: loaded.lua_runtime,
            files: loaded.files,
            cache_status: loaded.cache_status,
        });
    }

    let cache_status = if document_cache.is_some() {
        DocumentCacheStatus::Uncacheable
    } else {
        DocumentCacheStatus::Disabled
    };
    let mut loaded = load_skin_document_uncached(skin_path, kind, options, files, runtime_state)?;
    loaded.cache_status = cache_status;
    Ok(LoadedSkinDocumentForDecode {
        document: loaded.document,
        lua_runtime: loaded.lua_runtime,
        files: loaded.files,
        cache_status: loaded.cache_status,
    })
}

pub(super) struct LoadedSkinDocumentWithDependencies {
    pub(super) document: SkinDocument,
    pub(super) lua_runtime: Option<LuaSkinRuntime>,
    pub(super) files: BTreeMap<String, String>,
    pub(super) dependencies: SkinLoadDependencies,
    pub(super) cache_status: DocumentCacheStatus,
}

pub(super) fn load_skin_document_uncached(
    skin_path: &Path,
    kind: SkinKind,
    options: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
    runtime_state: &LuaLoadRuntimeState,
) -> Result<LoadedSkinDocumentWithDependencies> {
    let (mut document, lua_runtime, mut resolved_files, dependencies) =
        if is_lua_skin_path(skin_path) {
            // Lua スキンはオプション選択 (名前 -> 選択肢名) とファイル選択
            // (filepath 定義名 -> 相対パス) をそのまま渡す。
            let virtual_io_files = lua_virtual_io_files(runtime_state);
            let loaded = bmz_skin::load_lua_skin_with_runtime_state_and_virtual_io_files(
                skin_path,
                options,
                files,
                runtime_state,
                &virtual_io_files,
            )
            .with_context(|| format!("failed to load lua skin: {}", skin_path.display()))?;
            for warning in loaded.warnings {
                tracing::warn!(
                    path = %skin_path.display(),
                    kind = ?kind,
                    warning = %warning.message,
                    "lua skin load warning"
                );
            }
            (loaded.document, loaded.lua_runtime, loaded.files, loaded.dependencies)
        } else if is_lr2_skin_path(skin_path) {
            let loaded =
                bmz_skin::load_lr2_csv_skin(skin_path, decode_skin_kind(kind), options, files)
                    .with_context(|| {
                        format!("failed to load lr2 csv skin: {}", skin_path.display())
                    })?;
            for warning in loaded.warnings {
                tracing::warn!(
                    path = %skin_path.display(),
                    kind = ?kind,
                    warning = %warning.message,
                    "lr2 csv skin load warning"
                );
            }
            (loaded.document, None, BTreeMap::new(), loaded.dependencies)
        } else {
            let document = bmz_skin::load_beatoraja_json_skin_with_defaults(skin_path)
                .with_context(|| {
                    format!("failed to load beatoraja json skin: {}", skin_path.display())
                })?;
            if options.is_empty() {
                (document, None, BTreeMap::new(), SkinLoadDependencies::default())
            } else {
                // JSON スキンは property 定義から選択肢の op コード列を組み立て、
                // それを有効オプションとして再デコードする。
                let enabled = enabled_options_from_selections(&document, options);
                let document = bmz_skin::load_beatoraja_json_skin(skin_path, &enabled)
                    .with_context(|| {
                        format!(
                            "failed to load beatoraja json skin with options: {}",
                            skin_path.display()
                        )
                    })?;
                (document, None, BTreeMap::new(), SkinLoadDependencies::default())
            }
        };
    for (name, selected) in files {
        resolved_files.insert(name.clone(), selected.clone());
    }
    // レンダー時の `enabled_options()` がユーザ選択を反映するように、
    // 選択値から算出した op コード列を document に格納する。
    // (選択が空でもデフォルト計算結果と同じになるため、常に設定して問題ない)
    document.user_selected_options = Some(enabled_options_from_selections(&document, options));
    Ok(LoadedSkinDocumentWithDependencies {
        document,
        lua_runtime,
        files: resolved_files,
        dependencies,
        cache_status: DocumentCacheStatus::Disabled,
    })
}

/// property 定義とユーザ選択 (オプション名 -> 選択肢名) から、JSON スキンの
/// 有効オプション (op コード列) を組み立てる。
///
/// 選択が無い property は `def` (空なら先頭 item) の op を使う。
pub(crate) fn enabled_options_from_selections(
    document: &SkinDocument,
    selections: &BTreeMap<String, String>,
) -> Vec<i32> {
    let options = document
        .property
        .iter()
        .filter_map(|property| {
            let selected = selected_property_item(property, selections)
                .or_else(|| default_property_item(property));
            selected.map(|item| item.op)
        })
        .collect();
    document.with_internal_enabled_options(options)
}

pub(super) fn selected_property_item<'a>(
    property: &'a bmz_render::skin::SkinPropertyDef,
    selections: &BTreeMap<String, String>,
) -> Option<&'a bmz_render::skin::SkinPropertyItemDef> {
    let value = selections.get(&property.name)?;
    if let Ok(op) = value.parse::<i32>() {
        return property.item.iter().find(|item| item.op == op);
    }
    property.item.iter().find(|item| &item.name == value)
}

pub(super) fn default_property_item(
    property: &bmz_render::skin::SkinPropertyDef,
) -> Option<&bmz_render::skin::SkinPropertyItemDef> {
    property
        .item
        .iter()
        .find(|item| !property.def.is_empty() && item.name == property.def)
        .or_else(|| property.item.first())
}

pub(super) fn decode_skin_kind(kind: SkinKind) -> DecodeSkinKind {
    match kind {
        SkinKind::Play => DecodeSkinKind::Play,
        SkinKind::Select => DecodeSkinKind::Select,
        SkinKind::Decide => DecodeSkinKind::Decide,
        SkinKind::Result => DecodeSkinKind::Result,
    }
}

pub fn is_decodable_skin_path(path: &Path) -> bool {
    is_json_skin_path(path) || is_lua_skin_path(path) || is_lr2_skin_path(path)
}

pub fn is_json_skin_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
}

pub fn is_lua_skin_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("luaskin") || ext.eq_ignore_ascii_case("lua"))
}

pub fn is_lr2_skin_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("lr2skin"))
}

pub(super) fn decode_font(path: &Path) -> Result<DecodedFontData> {
    if is_bitmap_font_path(path) {
        Ok(DecodedFontData::Bitmap(load_bitmap_font(path)?))
    } else {
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read font: {}", path.display()))?;
        Ok(DecodedFontData::Vector(bytes))
    }
}

#[cfg(test)]
pub(super) fn decode_font_with_cache(
    path: &Path,
    font_cache: Option<&SharedSkinFontCache>,
) -> Result<(DecodedFontData, FontCacheStatus, Option<SkinFontCacheKey>)> {
    decode_font_with_cache_key(path, font_cache, skin_font_cache_key(path))
}

pub(super) fn decode_font_with_cache_key(
    path: &Path,
    font_cache: Option<&SharedSkinFontCache>,
    key: Option<SkinFontCacheKey>,
) -> Result<(DecodedFontData, FontCacheStatus, Option<SkinFontCacheKey>)> {
    let Some(font_cache) = font_cache else {
        return decode_font(path).map(|data| (data, FontCacheStatus::Disabled, None));
    };
    let Some(key) = key else {
        return decode_font(path).map(|data| (data, FontCacheStatus::Uncacheable, None));
    };
    if let Ok(mut cache) = font_cache.lock()
        && let Some(data) = cache.get(&key)
    {
        return Ok((data, FontCacheStatus::Hit, Some(key)));
    }
    let data = decode_font(path)?;
    if let Ok(mut cache) = font_cache.lock() {
        cache.insert(key.clone(), data.clone());
    }
    Ok((data, FontCacheStatus::Miss, Some(key)))
}

pub(super) fn skin_font_cache_key(path: &Path) -> Option<SkinFontCacheKey> {
    let metadata = fs::metadata(path).ok()?;
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    Some(SkinFontCacheKey {
        is_bitmap: is_bitmap_font_path(&path),
        path,
        modified: metadata.modified().ok(),
        len: metadata.len(),
    })
}

pub(super) fn font_data_cache_bytes(data: &DecodedFontData) -> usize {
    match data {
        DecodedFontData::Vector(bytes) => bytes.len(),
        DecodedFontData::Bitmap(font) => font
            .pages
            .values()
            .map(|page| page.image.pixels.len())
            .fold(font.glyphs.len().saturating_mul(64), usize::saturating_add),
    }
}
