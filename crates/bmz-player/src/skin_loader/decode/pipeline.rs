use crate::skin_loader::*;

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
    decode_beatoraja_skin_request(SkinDecodeRequest {
        skin_path,
        kind,
        options,
        files,
        runtime_state,
        document_cache,
        source_cache,
        texture_cache,
        font_cache,
        installed_fonts,
    })
}

/// Skin decode pipeline に渡す依存を一つにまとめたrequest。
///
/// 公開互換APIは従来の引数列を維持し、内部ではこの型を介してcacheや
/// runtime stateの追加・変更を局所化する。
pub(in crate::skin_loader) struct SkinDecodeRequest<'a> {
    pub(in crate::skin_loader) skin_path: &'a Path,
    pub(in crate::skin_loader) kind: SkinKind,
    pub(in crate::skin_loader) options: &'a BTreeMap<String, String>,
    pub(in crate::skin_loader) files: &'a BTreeMap<String, String>,
    pub(in crate::skin_loader) runtime_state: &'a LuaLoadRuntimeState,
    pub(in crate::skin_loader) document_cache: Option<SharedSkinDocumentCache>,
    pub(in crate::skin_loader) source_cache: Option<SharedSkinSourceAssetCache>,
    pub(in crate::skin_loader) texture_cache: Option<SharedSkinGpuTextureCache>,
    pub(in crate::skin_loader) font_cache: Option<SharedSkinFontCache>,
    pub(in crate::skin_loader) installed_fonts: Option<HashMap<String, SkinFontCacheKey>>,
}

pub(in crate::skin_loader) fn decode_beatoraja_skin_request(
    request: SkinDecodeRequest<'_>,
) -> Result<DecodedSkin> {
    let SkinDecodeRequest {
        skin_path,
        kind,
        options,
        files,
        runtime_state,
        document_cache,
        source_cache,
        texture_cache,
        font_cache,
        installed_fonts,
    } = request;
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
