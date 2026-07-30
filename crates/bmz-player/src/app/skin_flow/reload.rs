use super::*;

impl WinitApp {
    /// 現在の profile config のスキンパスを renderer へ再適用する。
    ///
    /// 起動時と同じ `load_skin_textures` 経路を使い、JSON スキンは
    /// バックグラウンド decode + 段階 install パイプラインへ流す。
    pub(super) fn reload_skins(&mut self, request: SkinReloadRequest) {
        let skin = self.boot.profile_config.skin.clone();
        let texture_request = SkinReloadRequest { result: false, course_result: false, ..request };
        let (pending_select, pending_decide, _pending_result) = reload_skin_textures(
            &self.boot.app_paths,
            &mut self.skin.skin_pipeline,
            texture_request,
            &self.boot.profile_config.display_name,
            &skin,
        );
        if request.select {
            self.skin.skin_pipeline.set_pending(SkinKind::Select, pending_select);
        }
        if request.decide {
            self.skin.skin_pipeline.set_pending(SkinKind::Decide, pending_decide);
        }
        if request.result || request.course_result {
            self.skin.last_result_skin_signature = None;
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
            self.skin.last_play_skin_signature.clone()
            && skin_reload_request_includes_key_mode(request, key_mode)
        {
            let selection =
                play_skin_selection_for_session(&skin, key_mode, self.select.session_mode);
            let play_options_only = self.play.active_play.is_some()
                && old_path == selection.path.trim()
                && old_files == *selection.files
                && old_options != *selection.options;
            let play_options_need_full_reload = play_options_only
                && self.play_skin_options_need_full_reload(key_mode, selection.path.trim());
            if play_options_only
                && !play_options_need_full_reload
                && self.apply_active_play_skin_options_fast_path(key_mode, selection.options)
            {
                self.skin.last_play_skin_signature = Some((
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
            self.skin.last_play_skin_signature = None;
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
            if let Some(sources) = self.skin.skin_video_sources.get_mut(&SkinKind::Play) {
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
            self.select.session_mode,
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

        if !self.skin.skin_pipeline.is_pending(SkinKind::Play)
            && self.skin.last_play_skin_signature.as_ref() == Some(&signature)
        {
            tracing::debug!(?key_mode, "play skin reuse (signature unchanged)");
            return;
        }
        self.skin.last_play_skin_signature = Some(signature);
        self.skin.skin_pipeline.set_pending(SkinKind::Play, false);
        let generation = self.skin.skin_pipeline.bump_generation(SkinKind::Play);

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

        let request =
            SkinDecodeRequest::new(generation, path, SkinKind::Play, options, files, runtime_state)
                .reuse_installed_fonts(&self.skin.skin_pipeline);
        spawn_skin_decode(&self.skin.skin_pipeline, request);
        self.skin.skin_pipeline.set_pending(SkinKind::Play, true);
        tracing::info!(?key_mode, path = %path_label, generation, "play skin decode queued");
    }
}
