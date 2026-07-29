use super::*;

impl WinitApp {
    /// upload worker を起動する。surface 接続後に一度だけ呼ぶ。
    /// decode worker からの receiver (`skin_decode_rx`) と GPU uploader を worker へ
    /// move し、worker は decode 結果を受けて GPU アップロードし `skin_upload_tx` で
    /// main へ返す。
    pub(super) fn start_skin_upload_worker(&mut self) {
        if self.skin_pipeline.upload_worker_started {
            return;
        }
        let Some(decode_rx) = self.skin_pipeline.decode_rx.take() else {
            return;
        };
        let Some(uploader) = self.renderer.gpu_uploader() else {
            // surface 未接続。次回接続時に再試行できるよう receiver を戻す。
            self.skin_pipeline.decode_rx = Some(decode_rx);
            return;
        };
        let upload_tx = self.skin_pipeline.upload_tx.clone();
        let texture_cache = self.skin_pipeline.gpu_texture_cache.clone();
        let event_proxy = self.event_proxy.clone();
        thread::Builder::new()
            .name("skin-upload".to_string())
            .spawn(move || {
                skin_upload_worker(decode_rx, upload_tx, uploader, texture_cache, event_proxy)
            })
            .expect("failed to spawn skin upload thread");
        self.skin_pipeline.upload_worker_started = true;
    }

    /// upload worker が GPU アップロードまで終えたスキンを非ブロッキングで取り込む。
    /// 毎フレーム呼ぶ。テクスチャ挿入 + フォント登録 + SkinContext 構築のみで軽量。
    pub(super) fn drain_pending_skins(&mut self) -> SkinDrainStats {
        let mut stats = SkinDrainStats::default();
        for _ in 0..MAX_SKIN_UPLOADS_PER_REDRAW {
            match self.skin_pipeline.upload_rx.try_recv() {
                Ok(result) => {
                    stats.received_count += 1;
                    stats.max_upload_wait_us = stats
                        .max_upload_wait_us
                        .max(instant_elapsed_us_u64(result.upload_finished_at));
                    if self.apply_uploaded_skin(result) {
                        stats.applied_count += 1;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
        stats
    }

    /// 指定された kind のスキンがアップロードされ取り込まれるまでブロックして待つ。
    /// scene 遷移直前 (特にプレイ開始) に呼ぶ。GPU アップロードは upload worker 上で
    /// 進むため、main は worker からの受信を待つだけで重い同期処理は無い。
    /// 先読みが間に合っていれば待ちはゼロ。
    pub(super) fn ensure_skin_ready(&mut self, kind: SkinKind) {
        while self.is_kind_pending_decode(kind) {
            match self.skin_pipeline.upload_rx.recv() {
                Ok(result) => {
                    let _ = self.apply_uploaded_skin(result);
                }
                Err(_) => break,
            }
        }
    }

    pub(super) fn ensure_result_skin_ready(&mut self, slot: ResultSkinSlot) {
        self.refresh_result_favorite_chart();
        self.spawn_result_skin_decode_for(slot);
        self.ensure_skin_ready(SkinKind::Result);
        self.renderer.reset_result_skin_runtime();
        let (skin_bgm_volume, skin_se_volume) = self.result_skin_audio_volumes();
        let started = self.result_skin_audio.as_mut().is_some_and(|audio| {
            audio.reset();
            audio.start_scene(skin_bgm_volume, skin_se_volume)
        });
        if started {
            self.start_audio_output_stream();
        }
        self.result_panel = self
            .renderer
            .result_skin_document()
            .and_then(|document| document.result_panel_default)
            .filter(|panel| (0..=2).contains(panel))
            .unwrap_or(0);
    }

    pub(super) fn refresh_result_favorite_chart(&mut self) {
        let Some(sha256) = self.finished_play.as_ref().map(|finished| finished.result.chart_sha256)
        else {
            self.result_favorite_chart = false;
            return;
        };
        self.result_favorite_chart = match self.boot.collection_db.is_favorite_chart(sha256) {
            Ok(favorite) => favorite,
            Err(error) => {
                tracing::warn!(%error, "failed to load favorite chart state for result");
                false
            }
        };
    }

    pub(super) fn current_result_skin_slot(&self) -> ResultSkinSlot {
        if self.finished_course.is_some() { ResultSkinSlot::Course } else { ResultSkinSlot::Normal }
    }

    pub(super) fn spawn_result_skin_decode_for(&mut self, slot: ResultSkinSlot) {
        let skin = &self.boot.profile_config.skin;
        let table_song = !self.play_table_text_primary.is_empty();
        let ir_name = result_ir_skin_name(&self.boot.profile_config.ir);
        let runtime_state = self.result_lua_runtime_state(slot, table_song, ir_name);
        let signature = result_skin_signature_for_config(skin, slot, runtime_state);
        if !self.skin_pipeline.is_pending(SkinKind::Result)
            && self.last_result_skin_signature.as_ref() == Some(&signature)
        {
            tracing::debug!(?slot, "result skin reuse (signature unchanged)");
            return;
        }

        let (_, trimmed, options, files, runtime_state) = signature.clone();
        self.last_result_skin_signature = Some(signature);
        self.skin_pipeline.set_pending(SkinKind::Result, false);
        let generation = self.skin_pipeline.bump_generation(SkinKind::Result);

        let (path, path_label, options, files) = if trimmed.is_empty() {
            (
                default_skin_document_path_from_paths(&self.boot.app_paths, SkinKind::Result),
                "default result skin".to_string(),
                BTreeMap::new(),
                BTreeMap::new(),
            )
        } else {
            let path = match self.boot.app_paths.resolve_path_ref(&trimmed) {
                Ok(path) => path,
                Err(error) => {
                    self.set_empty_result_skin_context();
                    tracing::warn!(
                        ?slot,
                        path = %trimmed,
                        error = %format_error_chain(&error),
                        "failed to resolve result skin path; using fallback result drawing"
                    );
                    return;
                }
            };
            (path, trimmed.clone(), options, files)
        };
        if !is_decodable_skin_path(&path) {
            self.set_empty_result_skin_context();
            tracing::warn!(
                ?slot,
                path = %path.display(),
                "result skin path is not a supported beatoraja skin file; using fallback result drawing"
            );
            return;
        }

        spawn_skin_decode(
            self.skin_pipeline.decode_tx.clone(),
            self.skin_pipeline.source_asset_cache.clone(),
            self.skin_pipeline.document_cache.clone(),
            self.skin_pipeline.gpu_texture_cache.clone(),
            self.skin_pipeline.font_cache.clone(),
            self.skin_pipeline.installed_font_cache.clone(),
            generation,
            path,
            SkinKind::Result,
            options,
            files,
            runtime_state,
        );
        self.skin_pipeline.set_pending(SkinKind::Result, true);
        tracing::info!(?slot, path = %path_label, generation, "result skin decode queued");
    }

    pub(super) fn result_lua_runtime_state(
        &self,
        slot: ResultSkinSlot,
        table_song: bool,
        ir_name: Option<&str>,
    ) -> bmz_skin::LuaLoadRuntimeState {
        let summary = match slot {
            ResultSkinSlot::Course => self.finished_course_skin_summary.as_ref(),
            ResultSkinSlot::Normal => self.finished_play.as_ref().map(|finished| &finished.summary),
        };
        let key_mode = summary.map(|summary| summary.key_mode).unwrap_or_default();
        let number_values =
            summary.map(result_lua_runtime_number_values_for_summary).unwrap_or_default();
        let mut runtime_state = lua_runtime_state_for_result(
            table_song,
            ir_name,
            self.result_score_save_enabled_for_slot(slot),
            key_mode,
            number_values,
            &self.boot.profile_config.display_name,
        );
        if let Some(summary) = summary {
            apply_result_summary_lua_load_state(
                &mut runtime_state,
                summary,
                &self.play_table_text_primary,
                &self.play_table_text_secondary,
                &self.play_table_text_fallback,
            );
        }
        runtime_state.event_index_values.insert(
            54,
            i32::try_from(bmz_render::skin::select_double_option_index(
                self.result_double_option_for_slot(slot).as_str(),
            ))
            .unwrap_or_default(),
        );
        if let Some(stage) = self.current_course_stage_marker() {
            apply_course_mode_lua_options(&mut runtime_state, Some(stage));
        }
        if let ResultSkinSlot::Course = slot
            && let Some(course) = &self.finished_course
        {
            apply_course_mode_lua_options(&mut runtime_state, None);
            apply_course_result_lua_load_state(&mut runtime_state, course);
        }
        runtime_state
    }

    pub(super) fn result_double_option_for_slot(&self, slot: ResultSkinSlot) -> DoubleOption {
        match slot {
            ResultSkinSlot::Normal => self
                .finished_play
                .as_ref()
                .map(|finished| finished.applied_arrange.double_option)
                .unwrap_or(DoubleOption::Off),
            ResultSkinSlot::Course => DoubleOption::Off,
        }
    }

    pub(super) fn current_result_score_save_enabled(&self) -> bool {
        self.result_score_save_enabled_for_slot(self.current_result_skin_slot())
    }

    pub(super) fn result_score_save_enabled_for_slot(&self, slot: ResultSkinSlot) -> bool {
        match slot {
            ResultSkinSlot::Course => {
                self.finished_course.as_ref().is_some_and(|course| course.course_score_id.is_some())
            }
            ResultSkinSlot::Normal => self
                .finished_play
                .as_ref()
                .is_some_and(|finished| finished.stored.score_history_id > 0),
        }
    }

    pub(super) fn set_empty_result_skin_context(&mut self) {
        let context =
            self.default_skin_manifest.clone().map(SkinContext::from_manifest).unwrap_or_default();
        self.renderer.set_result_skin_context(context);
        self.skin_video_sources.remove(&SkinKind::Result);
    }

    pub(super) fn is_kind_pending_decode(&self, kind: SkinKind) -> bool {
        self.skin_pipeline.is_pending(kind)
    }

    pub(super) fn has_pending_skin_reload(&self) -> bool {
        self.skin_pipeline.has_pending()
    }

    /// upload worker から届いた `UploadedSkin` を Renderer へ取り込む。
    /// stale generation は破棄。GPU アップロードは worker で完了済みなので、
    /// ここではハンドル挿入・フォント登録・SkinContext 構築のみ (軽量)。
    pub(super) fn apply_uploaded_skin(&mut self, pending: PendingUploadResult) -> bool {
        let PendingUploadResult {
            generation,
            path,
            kind,
            queued_at,
            decode_started_at,
            decode_finished_at,
            upload_started_at,
            upload_finished_at,
            uploaded,
        } = pending;
        let apply_started_at = Instant::now();
        let current_generation = self.skin_pipeline.generation(kind);
        if generation != current_generation {
            tracing::debug!(
                path = %path.display(),
                kind = ?kind,
                generation,
                current = current_generation,
                total_us = instant_elapsed_us_u64(queued_at),
                decode_us = instant_duration_us_u64(decode_started_at, decode_finished_at),
                upload_us = instant_duration_us_u64(upload_started_at, upload_finished_at),
                "discarding stale uploaded skin"
            );
            return false;
        }
        self.skin_pipeline.set_pending(kind, false);
        let uploaded = match uploaded {
            Ok(uploaded) => uploaded,
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    kind = ?kind,
                    total_us = instant_elapsed_us_u64(queued_at),
                    decode_us = instant_duration_us_u64(decode_started_at, decode_finished_at),
                    upload_us = instant_duration_us_u64(upload_started_at, upload_finished_at),
                    error = %format_error_chain(&error),
                    "failed to decode/upload beatoraja skin in background"
                );
                return false;
            }
        };
        let Some(manifest) = self.default_skin_manifest.clone() else {
            tracing::warn!(
                path = %path.display(),
                kind = ?kind,
                "skipping uploaded skin because default skin manifest is unavailable"
            );
            return false;
        };
        let UploadedSkin {
            kind,
            document,
            lua_runtime,
            fonts,
            prepared,
            audio_assets,
            decode_stats,
            upload_stats,
        } = uploaded;
        if kind == SkinKind::Result {
            self.result_skin_audio = self.system_audio.as_ref().map(|audio| {
                crate::skin_audio::SkinAudioRuntime::install(
                    audio.engine(),
                    &document,
                    audio_assets,
                )
            });
        }
        let font_count = fonts.len();
        let font_install_start = Instant::now();
        let mut font_install_count = 0usize;
        let mut font_install_skip_count = 0usize;
        let mut font_install_failed_count = 0usize;
        // フォント登録。reload 間で同一 font key の場合は text atlas reset ごと避ける。
        for font in fonts {
            let stored_id = font.stored_id.clone();
            let cache_key = font.cache_key.clone();
            if let Some(cache_key) = cache_key.as_ref()
                && self.skin_pipeline.installed_font_cache.get(&stored_id) == Some(cache_key)
            {
                font_install_skip_count += 1;
                continue;
            }
            if install_decoded_font(&mut self.renderer, font) {
                font_install_count += 1;
                if let Some(cache_key) = cache_key {
                    self.skin_pipeline.installed_font_cache.insert(stored_id, cache_key);
                } else {
                    self.skin_pipeline.installed_font_cache.remove(&stored_id);
                }
            } else {
                font_install_failed_count += 1;
                self.skin_pipeline.installed_font_cache.remove(&stored_id);
            }
        }
        let font_install_us = instant_elapsed_us_u64(font_install_start);
        // アップロード済みテクスチャを差し込み、SkinDocumentTexture を組む。
        let mut document_textures = Vec::with_capacity(prepared.len());
        let mut video_sources = Vec::new();
        for source in prepared {
            let PreparedSource { source_id, path, texture, prepared, size, is_video, cache_key } =
                source;
            if let Some(prepared) = prepared {
                self.renderer.insert_prepared_texture(TextureId(texture.0), prepared);
                if let Some(cache_key) = cache_key
                    && let Ok(mut cache) = self.skin_pipeline.gpu_texture_cache.lock()
                {
                    cache.insert(cache_key, texture, size);
                }
            }
            if is_video {
                let gating = skin_video_source_gating(&document, &source_id);
                video_sources.push(ActiveSkinVideoSource {
                    texture,
                    path,
                    decoder: None,
                    last_pts: None,
                    loop_start_us: 0,
                    active: gating.active,
                    gating_op_sets: gating.op_sets,
                    enabled_options: document.enabled_options(),
                    result_ranktime_ms: document.ranktime,
                    failed: false,
                });
            }
            document_textures.push(SkinDocumentTexture { source_id, texture, source_size: size });
        }
        if video_sources.is_empty() {
            self.skin_video_sources.remove(&kind);
        } else {
            self.skin_video_sources.insert(kind, video_sources);
        }
        let preserve_play_dynamic_timers = kind == SkinKind::Play && self.active_play.is_some();
        let installed_sources = document_textures.len();
        set_decoded_skin_context(
            &mut self.renderer,
            kind,
            manifest,
            document,
            lua_runtime,
            document_textures,
            preserve_play_dynamic_timers,
        );
        self.pending_skin_render_probe =
            Some(PendingSkinRenderProbe { kind, generation, applied_at: Instant::now() });
        self.frame.request_immediate_frame();
        tracing::debug!(
            path = %path.display(),
            kind = ?kind,
            generation,
            total_us = instant_elapsed_us_u64(queued_at),
            decode_queue_us = instant_duration_us_u64(queued_at, decode_started_at),
            decode_thread_us = instant_duration_us_u64(decode_started_at, decode_finished_at),
            upload_queue_us = instant_duration_us_u64(decode_finished_at, upload_started_at),
            upload_thread_us = instant_duration_us_u64(upload_started_at, upload_finished_at),
            apply_queue_us = instant_duration_us_u64(upload_finished_at, apply_started_at),
            apply_us = instant_elapsed_us_u64(apply_started_at),
            document_us = decode_stats.document_us,
            document_cache_hits = decode_stats.document_cache_hits,
            document_cache_misses = decode_stats.document_cache_misses,
            document_cache_uncacheable = decode_stats.document_cache_uncacheable,
            document_cache_disabled = decode_stats.document_cache_disabled,
            font_count,
            font_decode_us = decode_stats.font_decode_us,
            font_payload_skipped = decode_stats.font_payload_skipped,
            font_cache_hits = decode_stats.font_cache_hits,
            font_cache_misses = decode_stats.font_cache_misses,
            font_cache_uncacheable = decode_stats.font_cache_uncacheable,
            font_cache_disabled = decode_stats.font_cache_disabled,
            font_install_us,
            font_installed = font_install_count,
            font_install_skipped = font_install_skip_count,
            font_install_failed = font_install_failed_count,
            source_task_count = decode_stats.source_task_count,
            source_decode_us = decode_stats.source_decode_us,
            decoded_sources = decode_stats.decoded_source_count,
            decoded_source_bytes = decode_stats.decoded_source_bytes,
            builtin_sources = decode_stats.builtin_source_count,
            image_sources = decode_stats.image_source_count,
            video_sources = decode_stats.video_source_count,
            source_cache_hits = decode_stats.source_cache_hits,
            source_cache_misses = decode_stats.source_cache_misses,
            source_cache_uncacheable = decode_stats.source_cache_uncacheable,
            source_cache_disabled = decode_stats.source_cache_disabled,
            video_source_cache_hits = decode_stats.video_source_cache_hits,
            video_source_cache_misses = decode_stats.video_source_cache_misses,
            video_source_cache_uncacheable = decode_stats.video_source_cache_uncacheable,
            video_source_cache_disabled = decode_stats.video_source_cache_disabled,
            source_texture_cache_hits = decode_stats.source_texture_cache_hits,
            source_texture_cache_hit_bytes = decode_stats.source_texture_cache_hit_bytes,
            video_source_texture_cache_hits = decode_stats.video_source_texture_cache_hits,
            video_source_texture_cache_hit_bytes =
                decode_stats.video_source_texture_cache_hit_bytes,
            uploaded_sources = upload_stats.uploaded_source_count,
            uploaded_source_bytes = upload_stats.uploaded_source_bytes,
            uploaded_video_sources = upload_stats.uploaded_video_source_count,
            uploaded_video_source_bytes = upload_stats.uploaded_video_source_bytes,
            texture_cache_hits = upload_stats.texture_cache_hits,
            texture_cache_misses = upload_stats.texture_cache_misses,
            texture_cache_uncacheable = upload_stats.texture_cache_uncacheable,
            texture_cache_disabled = upload_stats.texture_cache_disabled,
            video_texture_cache_hits = upload_stats.video_texture_cache_hits,
            video_texture_cache_misses = upload_stats.video_texture_cache_misses,
            video_texture_cache_uncacheable = upload_stats.video_texture_cache_uncacheable,
            video_texture_cache_disabled = upload_stats.video_texture_cache_disabled,
            installed_sources,
            "beatoraja skin reload timings"
        );
        if kind == SkinKind::Select && matches!(self.view_state(), AppViewState::Select) {
            self.restart_select_scene_timers();
        }
        true
    }

    pub(super) fn sync_realtime_profile_settings(&mut self) {
        self.sync_active_play_realtime_profile_settings();
        if let Some(manager) = &self.system_sound {
            let mix = self.boot.profile_config.audio_mix.clone();
            let preview_factor =
                select_preview_fade_factor(self.select_assets.preview_fade(), Instant::now());
            manager.refresh_volumes(|sound_type| {
                let volume = system_sound_volume_from_mix(&mix, sound_type);
                if sound_type == crate::system_sound::SoundType::Select {
                    volume * (1.0 - preview_factor).clamp(0.0, 1.0)
                } else {
                    volume
                }
            });
        }
        self.apply_select_preview_audio_mix();
    }

    pub(super) fn sync_active_play_lane_settings_from_profile(&mut self, before: &LaneViewConfig) {
        let speed_locked = self.active_course.as_ref().is_some_and(|course| {
            course.definition.constraints.speed == bmz_core::course::CourseSpeedConstraint::NoSpeed
        });
        let profile_lane = self.boot.profile_config.lane.clone();
        let Some(active_play) = &mut self.active_play else {
            return;
        };
        if apply_profile_lane_settings_to_session(
            &mut active_play.running.session,
            before,
            &profile_lane,
            speed_locked,
        ) {
            update_pre_ready_play_snapshot_options_for_session(
                self.play_ready_sound_started_at,
                &mut self.last_play_snapshot,
                &active_play.running.session,
                &active_play.running.applied_arrange,
            );
            tracing::info!(
                hispeed = active_play.running.session.hispeed,
                hispeed_mode = ?active_play.running.session.hispeed_mode,
                target_green_number = active_play.running.session.target_green_number,
                lane_cover = active_play.running.session.lane_cover,
                lift = active_play.running.session.lift,
                "applied egui lane settings to active play"
            );
        }
    }

    pub(super) fn sync_active_play_realtime_profile_settings(&mut self) {
        if let Some(active_play) = &mut self.active_play {
            let session = &mut active_play.running.session;
            let chart_normalization_gain = session.audio_mix.chart_normalization_gain;
            session.audio_mix = crate::config::play::audio_mix_from_profile_with_chart_gain(
                &self.boot.profile_config,
                chart_normalization_gain,
            );
            session.offsets =
                crate::config::play::play_offsets_from_profile(&self.boot.profile_config);
            session.input_offset_auto_adjust_enabled =
                self.boot.profile_config.judge.visual_offset_auto_adjust;
            let auto_adjust_available = session.replay_player.is_none()
                && !session.autoplay.as_ref().is_some_and(|autoplay| autoplay.is_full());
            if session.input_offset_auto_adjust_enabled && auto_adjust_available {
                session.input_offset_auto_adjust.get_or_insert_with(Default::default);
            } else {
                session.input_offset_auto_adjust = None;
            }
        }
    }

    pub(super) fn sync_profile_visual_offset_from_active_play(&mut self) {
        let Some((visual_offset_us, auto_adjust_active)) =
            self.active_play.as_ref().map(|active| {
                (
                    active.running.session.offsets.visual_offset_us,
                    active.running.session.input_offset_auto_adjust.is_some(),
                )
            })
        else {
            return;
        };
        sync_active_play_visual_offset_to_profile(
            &mut self.boot.profile_config,
            visual_offset_us,
            auto_adjust_active,
        );
    }

    pub(super) fn play_skin_defs_for_path(&mut self, path: &str) -> SceneSkinDefs {
        let key = path.trim().to_string();
        if let Some(defs) = self.skin_defs_cache.get(&key) {
            return defs.clone();
        }
        let defs = play_skin_defs_from_path(&self.boot.app_paths, &key);
        self.skin_defs_cache.insert(key, defs.clone());
        defs
    }

    pub(super) fn reset_skin_config_from_disk(&mut self) {
        match load_profile_config(&self.boot.profile_paths.profile_toml) {
            Ok(profile) => {
                replace_skin_config_from_loaded_profile(&mut self.boot.profile_config, profile);
                self.apply_profile_skin_offsets_to_active_play();
                self.reload_skins(SkinReloadRequest {
                    select: true,
                    decide: true,
                    result: true,
                    course_result: true,
                    play4: true,
                    play5: true,
                    play6: true,
                    play7: true,
                    play8: true,
                    play9: true,
                    play10: true,
                    play14: true,
                    offsets: true,
                });
                tracing::info!("skin config reset from profile.toml");
            }
            Err(error) => {
                tracing::error!(
                    path = %self.boot.profile_paths.profile_toml.display(),
                    %error,
                    "failed to reset skin config from profile.toml"
                );
            }
        }
    }

    pub(super) fn apply_profile_skin_offsets_to_active_play(&mut self) {
        let Some(key_mode) = self
            .active_play
            .as_ref()
            .map(|active_play| active_play.running.session.chart.metadata.key_mode)
        else {
            return;
        };
        let offsets = play_skin_selection_for_session(
            &self.boot.profile_config.skin,
            key_mode,
            self.session_mode,
        )
        .offsets
        .iter()
        .map(|offset| PlaySkinOffset {
            id: offset.id,
            x: offset.x,
            y: offset.y,
            w: offset.w,
            h: offset.h,
            r: offset.r,
            a: offset.a,
        })
        .collect();
        if let Some(active_play) = &mut self.active_play {
            active_play.running.session.skin_offsets = offsets;
        }
    }

    /// 現在の profile config のスキンパスを renderer へ再適用する。
    ///
    /// 起動時と同じ `load_skin_textures` 経路を使い、JSON スキンは
    /// バックグラウンド decode + 段階 install パイプラインへ流す。
    pub(super) fn reload_skins(&mut self, request: SkinReloadRequest) {
        let skin = self.boot.profile_config.skin.clone();
        let texture_request = SkinReloadRequest { result: false, course_result: false, ..request };
        let (pending_select, pending_decide, _pending_result) = reload_skin_textures(
            &mut self.renderer,
            &self.boot.app_paths,
            &self.skin_pipeline.decode_tx,
            &self.skin_pipeline.source_asset_cache,
            &self.skin_pipeline.document_cache,
            &self.skin_pipeline.gpu_texture_cache,
            &self.skin_pipeline.font_cache,
            &mut self.skin_pipeline.generations,
            texture_request,
            &self.boot.profile_config.display_name,
            &skin.select,
            &skin.decide,
            &skin.result,
            &skin.select_options,
            &skin.decide_options,
            &skin.result_options,
            &skin.select_files,
            &skin.decide_files,
            &skin.result_files,
            &skin.select_offsets,
            &skin.decide_offsets,
            &skin.result_offsets,
        );
        if request.select {
            self.skin_pipeline.set_pending(SkinKind::Select, pending_select);
        }
        if request.decide {
            self.skin_pipeline.set_pending(SkinKind::Decide, pending_decide);
        }
        if request.result || request.course_result {
            self.last_result_skin_signature = None;
            if matches!(self.current_scene_kind(), AppSceneKind::Result) {
                let slot = self.current_result_skin_slot();
                if matches!(
                    (slot, request.result, request.course_result),
                    (ResultSkinSlot::Normal, true, _) | (ResultSkinSlot::Course, _, true)
                ) {
                    self.spawn_result_skin_decode_for(slot);
                }
            }
        }
        // 旧 generation 分の upload 結果は apply_uploaded_skin の generation
        // チェックで破棄されるため、ここでの明示的なキュー破棄は不要。
        if let Some((key_mode, old_path, old_options, old_files, runtime_state)) =
            self.last_play_skin_signature.clone()
            && skin_reload_request_includes_key_mode(request, key_mode)
        {
            let selection = play_skin_selection_for_session(&skin, key_mode, self.session_mode);
            let play_options_only = self.active_play.is_some()
                && old_path == selection.path.trim()
                && old_files == *selection.files
                && old_options != *selection.options;
            let play_options_need_full_reload = play_options_only
                && self.play_skin_options_need_full_reload(key_mode, selection.path.trim());
            if play_options_only
                && !play_options_need_full_reload
                && self.apply_active_play_skin_options_fast_path(key_mode, selection.options)
            {
                self.last_play_skin_signature = Some((
                    key_mode,
                    selection.path.trim().to_string(),
                    selection.options.clone(),
                    selection.files.clone(),
                    runtime_state,
                ));
                tracing::debug!(
                    ?key_mode,
                    "play skin option change applied without background reload"
                );
                tracing::info!(?request, "skin reload queued from egui skin panel");
                return;
            }
            self.last_play_skin_signature = None;
            self.spawn_play_skin_decode_for(key_mode, runtime_state);
        }
        let pending_after_reload = self.has_pending_skin_reload();
        tracing::info!(?request, "skin reload queued from egui skin panel");
        if pending_after_reload {
            self.frame.request_immediate_frame();
            let _ = self.drain_pending_skins();
            self.request_redraw();
        }
    }

    pub(super) fn apply_active_play_skin_options_fast_path(
        &mut self,
        key_mode: KeyMode,
        options: &BTreeMap<String, String>,
    ) -> bool {
        let Some((enabled_options, property_ops)) =
            self.renderer.play_skin_document().map(|document| {
                (
                    enabled_options_from_selections(document, options),
                    skin_document_property_ops(document),
                )
            })
        else {
            return false;
        };
        let applied_options = enabled_options.clone();
        if self.renderer.set_play_skin_user_selected_options(enabled_options) {
            if let Some(sources) = self.skin_video_sources.get_mut(&SkinKind::Play) {
                apply_skin_video_source_enabled_options(sources, &applied_options, &property_ops);
            }
            tracing::debug!(?key_mode, "applied play skin option change before background reload");
            return true;
        }
        false
    }

    pub(super) fn play_skin_options_need_full_reload(
        &self,
        key_mode: KeyMode,
        trimmed_path: &str,
    ) -> bool {
        let path = if trimmed_path.is_empty() {
            default_play_skin_document_path_from_paths(&self.boot.app_paths, key_mode)
        } else {
            match self.boot.app_paths.resolve_path_ref(trimmed_path) {
                Ok(path) => path,
                Err(error) => {
                    tracing::warn!(
                        ?key_mode,
                        path = %trimmed_path,
                        error = %format_error_chain(&error),
                        "keeping play skin background reload because skin path could not be resolved"
                    );
                    return true;
                }
            }
        };
        match skin_path_options_need_full_reload(&path) {
            Ok(needed) => needed,
            Err(error) => {
                tracing::warn!(
                    ?key_mode,
                    path = %path.display(),
                    error = %format_error_chain(&error),
                    "keeping play skin background reload because skin option dependencies could not be inspected"
                );
                true
            }
        }
    }

    /// 決定対象チャートの key_mode に対応するプレイスキンを background decode に投入する。
    /// 直前と同じ mode かつ path/options/files が同じなら何もしない。
    pub(super) fn spawn_play_skin_decode_for(
        &mut self,
        key_mode: KeyMode,
        mut runtime_state: bmz_skin::LuaLoadRuntimeState,
    ) {
        let selection = play_skin_selection_for_session(
            &self.boot.profile_config.skin,
            key_mode,
            self.session_mode,
        );
        runtime_state.offset_values.clear();
        runtime_state.offset_id_values.clear();
        apply_skin_offsets_to_lua_runtime_state(&mut runtime_state, selection.offsets);
        let trimmed = selection.path.trim();
        let signature = (
            key_mode,
            trimmed.to_string(),
            selection.options.clone(),
            selection.files.clone(),
            runtime_state.clone(),
        );

        if !self.skin_pipeline.is_pending(SkinKind::Play)
            && self.last_play_skin_signature.as_ref() == Some(&signature)
        {
            tracing::debug!(?key_mode, "play skin reuse (signature unchanged)");
            return;
        }
        self.last_play_skin_signature = Some(signature);
        self.skin_pipeline.set_pending(SkinKind::Play, false);
        let generation = self.skin_pipeline.bump_generation(SkinKind::Play);

        let (path, path_label, options, files) = if trimmed.is_empty() {
            (
                default_play_skin_document_path_from_paths(&self.boot.app_paths, key_mode),
                format!("default play skin for {key_mode:?}"),
                BTreeMap::new(),
                BTreeMap::new(),
            )
        } else {
            let path = match self.boot.app_paths.resolve_path_ref(trimmed) {
                Ok(path) => path,
                Err(error) => {
                    tracing::warn!(
                        ?key_mode,
                        path = trimmed,
                        error = %format_error_chain(&error),
                        "failed to resolve play skin path; using existing textures"
                    );
                    return;
                }
            };
            (path, trimmed.to_string(), selection.options.clone(), selection.files.clone())
        };
        if !is_decodable_skin_path(&path) {
            tracing::warn!(
                ?key_mode,
                path = %path.display(),
                "play skin path is not a supported beatoraja skin file; using existing textures"
            );
            return;
        }

        spawn_skin_decode(
            self.skin_pipeline.decode_tx.clone(),
            self.skin_pipeline.source_asset_cache.clone(),
            self.skin_pipeline.document_cache.clone(),
            self.skin_pipeline.gpu_texture_cache.clone(),
            self.skin_pipeline.font_cache.clone(),
            self.skin_pipeline.installed_font_cache.clone(),
            generation,
            path,
            SkinKind::Play,
            options,
            files,
            runtime_state,
        );
        self.skin_pipeline.set_pending(SkinKind::Play, true);
        tracing::info!(?key_mode, path = %path_label, generation, "play skin decode queued");
    }

    pub(super) fn update_current_skin_video_sources(
        &mut self,
        scene: &AppSceneSnapshot,
        profiling: bool,
    ) -> SkinVideoFrameProfile {
        let mut profile = SkinVideoFrameProfile::default();
        let Some((kind, elapsed_us)) = self.current_skin_video_context() else {
            return profile;
        };
        let needs_runtime_state = self
            .skin_video_sources
            .get(&kind)
            .is_some_and(|sources| skin_video_sources_need_runtime_state(sources));
        // 実行時 op 条件 (例: リザルトのランク別 BG) で実際に表示されるソースだけを
        // デコードする。実行時 op を持つソースが無い場合は state 構築自体を避ける。
        let runtime_state = needs_runtime_state
            .then(|| self.current_skin_video_draw_state_for_scene(kind, scene))
            .flatten();
        let Some(sources) = self.skin_video_sources.get_mut(&kind) else {
            return profile;
        };
        for source in sources {
            if source.failed || !source.active {
                continue;
            }
            profile.active_sources += 1;
            if let Some(state) = runtime_state.as_ref()
                && !skin_video_source_runtime_visible(source, state)
            {
                // 現在のシーン状態では非表示。デコード中なら止めて開放する。
                if source.decoder.is_some() {
                    source.decoder = None;
                    source.last_pts = None;
                }
                continue;
            }
            profile.visible_sources += 1;
            if source.decoder.is_none() {
                match VideoBgaDecoder::open_following_playback_time(&source.path) {
                    Ok(decoder) => {
                        // 非同期 skin decode 完了後など、リザルト開始から時間が経ってから
                        // decoder を開くと video_offset が大きくなり、clocked decode が
                        // 1 周デコードして loop_base を追いつかせるまでフレームを出せない。
                        // 開いた時点の elapsed を loop 原点に合わせ、常に offset ≈ 0 から始める。
                        source.loop_start_us = elapsed_us;
                        source.last_pts = None;
                        tracing::info!(
                            kind = ?kind,
                            texture_id = source.texture.0,
                            path = %source.path.display(),
                            "opened skin video source decoder"
                        );
                        source.decoder = Some(decoder);
                        profile.opened += 1;
                    }
                    Err(error) => {
                        tracing::warn!(
                            kind = ?kind,
                            texture_id = source.texture.0,
                            path = %source.path.display(),
                            %error,
                            "failed to open skin video source"
                        );
                        source.failed = true;
                        continue;
                    }
                }
            }

            let Some(decoder) = source.decoder.as_mut() else {
                continue;
            };
            let video_offset_us = elapsed_us.saturating_sub(source.loop_start_us);
            let poll_start = profiling.then(Instant::now);
            let frame = decoder.poll_frame(video_offset_us);
            if let Some(start) = poll_start {
                profile.poll_us += start.elapsed().as_micros();
            }
            if let Some(frame) = frame
                && source.last_pts != Some(frame.pts_us)
            {
                let pts = frame.pts_us;
                let upload_start = profiling.then(Instant::now);
                match self.renderer.upsert_rgba_texture_ref(
                    TextureId(source.texture.0),
                    frame.width,
                    frame.height,
                    &frame.rgba,
                ) {
                    Ok(()) => {
                        source.last_pts = Some(pts);
                        profile.uploaded_frames += 1;
                    }
                    Err(error) => {
                        tracing::warn!(
                            kind = ?kind,
                            texture_id = source.texture.0,
                            path = %source.path.display(),
                            %error,
                            "failed to upload skin video source frame"
                        );
                    }
                }
                if let Some(start) = upload_start {
                    profile.upload_us += start.elapsed().as_micros();
                }
            }
            if source.decoder.as_ref().is_some_and(VideoBgaDecoder::is_finished) {
                source.decoder = None;
                source.last_pts = None;
                source.loop_start_us = elapsed_us;
            }
        }
        profile
    }

    pub(super) fn current_skin_video_context(&self) -> Option<(SkinKind, i64)> {
        match self.view_state() {
            AppViewState::Select => Some((SkinKind::Select, self.select_time().0)),
            AppViewState::Decide => self
                .pending_decide
                .as_ref()
                .map(|decide| (SkinKind::Decide, elapsed_since(decide.started_at).0)),
            AppViewState::Play => Some((SkinKind::Play, self.play_elapsed_time().0)),
            AppViewState::Result => {
                Some((SkinKind::Result, elapsed_since(self.result_scene_started_at).0))
            }
        }
    }

    /// 動画ソースの実行時可視判定に使う `SkinDrawState` を、現在のシーン用に構築する。
    pub(super) fn current_skin_video_draw_state_for_scene(
        &self,
        kind: SkinKind,
        scene: &AppSceneSnapshot,
    ) -> Option<bmz_render::skin::SkinDrawState> {
        match kind {
            SkinKind::Play => {
                let AppSceneSnapshot::Play(snapshot) = scene else {
                    return None;
                };
                let play_skin_document = self.renderer.play_skin_document();
                Some(play_skin_video_draw_state(
                    snapshot,
                    play_skin_document.map(|document| document.h),
                    play_skin_document.and_then(|document| document.primary_note_lane_height_px()),
                ))
            }
            SkinKind::Result => {
                let AppSceneSnapshot::Result(snapshot) = scene else {
                    return None;
                };
                let ranktime = self
                    .skin_video_sources
                    .get(&SkinKind::Result)
                    .and_then(|sources| sources.first())
                    .map_or(0, |source| source.result_ranktime_ms);
                Some(bmz_render::plan::result_skin_draw_state(snapshot, ranktime))
            }
            _ => None,
        }
    }
}
