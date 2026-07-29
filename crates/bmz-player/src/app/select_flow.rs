use super::*;

impl WinitApp {
    pub(super) fn select_snapshot(&self) -> SelectSnapshot {
        let locale = self.boot.profile_config.ui.locale();
        let text = Localizer::new(locale);
        let selected = self.select_items.get(self.selected_index);
        let selected_course_ir = self.selected_course_ir_target();
        let select_ir_scope_binding = self
            .renderer
            .select_skin_document()
            .map(|document| document.select_ir_scope_binding)
            .unwrap_or_default();
        let current_folder = match self.folder_stack.last() {
            None => String::new(),
            Some(path) if path == FAVORITE_ROOT_PATH => "FAVORITE".to_string(),
            Some(path) if path == FAVORITE_CHART_PATH => "FAVORITE CHART".to_string(),
            Some(path) if path == FAVORITE_SONG_PATH => "FAVORITE SONG".to_string(),
            Some(path) if parse_favorite_song_detail_path(path).is_some() => {
                "FAVORITE SONG".to_string()
            }
            Some(path) if let Some(folder) = parse_same_folder_path(path) => {
                std::path::Path::new(folder)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string()
            }
            Some(path) if path.starts_with(TABLE_ROOT_PATH) => match parse_table_path(path) {
                Some(TablePath::Root) | None => text.text("select-difficulty-tables"),
                Some(TablePath::Table { source_url }) => self.table_breadcrumb_name(source_url),
                Some(TablePath::Level { source_url, level }) => {
                    let table = self.table_breadcrumb(source_url);
                    format!("{} > {}{}", table.name, table.symbol, level)
                }
            },
            Some(path) if in_settings_stack(std::slice::from_ref(path)) => {
                settings_breadcrumb_for_locale(path, locale)
            }
            Some(path) => std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(),
        };
        let (search_word, search_word_alpha, search_caret_byte_index) = self.display_search_word();
        self.ensure_visible_select_chart_distributions(25);
        let chart_distributions = self.select_distribution_cache.borrow();
        let note_display_duration_ms =
            Some(Self::select_note_display_duration_ms_for_skin(&self.boot.profile_config));
        SelectSnapshot {
            time: self.select_time(),
            player_name: String::new(),
            current_fps: 0,
            operating_time_ms: 0,
            skin_input: Default::default(),
            skin_offsets: skin_offset_values_from_config(
                &self.boot.profile_config.skin.select_offsets,
            ),
            selection_time: self.select_bar_time(),
            option_panel_time: self.option_panel_time(),
            option_panel_off_times: self
                .option_panel_off_started_at
                .map(|started_at| started_at.map(elapsed_since)),
            option_panel: self.select_option_panel,
            chart_count: self.select_items.len() as u32,
            selected_index: self.selected_index as u32,
            bar_scroll_direction: self.select_bar_scroll_direction,
            bar_scroll_progress: self.select_bar_scroll_progress(),
            selected_chart_id: match selected {
                Some(SelectItem::Chart(row)) => row.chart.as_ref().map(|chart| chart.chart_id),
                _ => None,
            },
            selected_title: selected
                .map(|item| item.display_name_for_locale(locale))
                .unwrap_or_default(),
            hispeed: self.boot.profile_config.lane.hispeed,
            note_display_duration_ms,
            rows: select_snapshot_rows(
                &self.select_items,
                self.selected_index,
                25,
                &self.boot.profile_config,
                self.key_config_edit.as_ref(),
                &chart_distributions,
            ),
            arrange: self.arrange_option.as_str().to_string(),
            arrange_2p: self.arrange_option_2p.as_str().to_string(),
            // 通常のRANDOMはプレイ開始時に抽選する。将来、選曲中に確定した
            // リプレイ／ライバル配置をここへ渡す。
            lane_shuffle_pattern: Vec::new(),
            target: self.target_option.as_string(),
            gauge: gauge_option_as_str(self.gauge_option).to_string(),
            gauge_auto_shift: gauge_auto_shift_as_str(self.gauge_auto_shift_option).to_string(),
            bottom_shiftable_gauge: bottom_shiftable_gauge_as_str(
                self.bottom_shiftable_gauge_option,
            )
            .to_string(),
            double_option: self.double_option.as_str().to_string(),
            hs_fix: self.hs_fix_option.as_str().to_string(),
            assist: self.session_mode.as_str().to_string(),
            select_mode: self.select_mode_filter.as_str().to_string(),
            select_sort: self.select_sort.as_str().to_string(),
            select_ln_mode: self
                .boot
                .profile_config
                .play
                .ln_mode_policy
                .display_label()
                .to_string(),
            judge_algorithm: self
                .boot
                .profile_config
                .judge
                .judge_algorithm
                .beatoraja_name()
                .to_string(),
            bga: bga_mode_as_str(self.boot.profile_config.play.bga).to_string(),
            grade_diff_display: self.boot.profile_config.play.grade_diff_display,
            judge_timing_offset_ms: (self.boot.profile_config.judge.visual_offset_us / 1_000)
                .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            judge_timing_auto_adjust: self.boot.profile_config.judge.visual_offset_auto_adjust,
            lanecover_enabled: matches!(
                self.boot.profile_config.play.lane_effect,
                LaneEffectConfig::Sudden | LaneEffectConfig::HiddenSudden
            ),
            lift_enabled: self.boot.profile_config.lane.lift_enabled,
            hidden_enabled: matches!(
                self.boot.profile_config.play.lane_effect,
                LaneEffectConfig::Hidden | LaneEffectConfig::HiddenSudden
            ),
            hispeed_auto_adjust: self.boot.profile_config.lane.hispeed_auto_adjust,
            master_volume: crate::config::play::volume_unit_to_f32(
                self.boot.profile_config.audio_mix.master_volume,
            ),
            key_volume: crate::config::play::volume_unit_to_f32(
                self.boot.profile_config.audio_mix.key_volume,
            ),
            bgm_volume: crate::config::play::volume_unit_to_f32(
                self.boot.profile_config.audio_mix.bgm_volume,
            ),
            current_folder,
            key_hint: self.select_keys.key_hint().to_string(),
            option_hint: self.select_keys.option_hint().to_string(),
            exit_hold_progress: self.select_exit_hold_progress(),
            overlay: OverlaySnapshot::default(),
            stage_background: self.select_assets.meta_image_loaded(SelectMetaImageSlot::Stage),
            stage_image_size: self.select_assets.meta_image_size(SelectMetaImageSlot::Stage),
            backbmp_image: self.select_assets.meta_image_loaded(SelectMetaImageSlot::Backbmp),
            backbmp_image_size: self.select_assets.meta_image_size(SelectMetaImageSlot::Backbmp),
            banner_image: self.select_assets.meta_image_loaded(SelectMetaImageSlot::Banner),
            banner_image_size: self.select_assets.meta_image_size(SelectMetaImageSlot::Banner),
            in_settings: in_settings_stack(&self.folder_stack),
            settings_editing: self.settings_edit.is_some() || self.key_config_edit.is_some(),
            search_word,
            search_word_alpha,
            search_caret_byte_index,
            mouse_position: self.cursor_position_normalized(),
            ir: selected_course_ir.as_ref().map_or_else(
                || {
                    self.select_ir.snapshot_for_binding(
                        &self.boot.profile_config.ir,
                        self.selected_chart_sha256(),
                        select_ir_scope_binding,
                    )
                },
                |target| {
                    self.select_ir.course_snapshot_for(&self.boot.profile_config.ir, Some(target))
                },
            ),
            rival: self
                .select_ir
                .rival_for(&self.boot.profile_config.ir, self.selected_chart_sha256()),
            replay_slot_rule_indices: replay_slot_rule_indices(
                &self.boot.profile_config.replay.slot_rules,
            ),
            player_stats: self.player_stats.clone(),
        }
    }

    /// 選曲カーソルが曲行のときの chart SHA256。フォルダ / コース行は None。
    pub(super) fn selected_chart_sha256(&self) -> Option<[u8; 32]> {
        match self.select_items.get(self.selected_index)? {
            SelectItem::Chart(row) => row.score_sha256(),
            _ => None,
        }
    }

    pub(super) fn selected_course_ir_target(
        &self,
    ) -> Option<crate::screens::select_ir::SelectCourseIrTarget> {
        let SelectItem::Course(row) = self.select_items.get(self.selected_index)? else {
            return None;
        };
        Some(crate::screens::select_ir::SelectCourseIrTarget {
            course_hash: row.course_hash.clone()?,
            rian_course_hash_v1: row.rian_course_hash_v1.clone()?,
            gauge: crate::screens::play_start::course_gauge_for(self.gauge_option)
                .as_str()
                .to_string(),
            ln_policy: self.boot.profile_config.play.ln_mode_policy.as_ir_str().to_string(),
            rule_mode: self.boot.profile_config.play.rule_mode,
        })
    }

    pub(super) fn select_note_display_duration_ms_for_skin(profile: &ProfileConfig) -> i32 {
        profile.lane.target_green_number.max(1).min(i32::MAX as u32) as i32
    }

    pub(super) fn ensure_visible_select_chart_distributions(&self, visible_limit: usize) {
        let chart_ids: Vec<i64> = select_visible_item_indices(
            self.select_items.len(),
            self.selected_index,
            visible_limit,
        )
        .into_iter()
        .filter_map(|index| match self.select_items.get(index) {
            Some(SelectItem::Chart(row)) => row.chart.as_ref().map(|chart| chart.chart_id),
            _ => None,
        })
        .collect();
        if chart_ids.is_empty() {
            return;
        }

        let missing_ids: Vec<i64> = {
            let cache = self.select_distribution_cache.borrow();
            chart_ids.iter().copied().filter(|chart_id| !cache.contains_key(chart_id)).collect()
        };
        if !missing_ids.is_empty() {
            match self.boot.library_db.chart_distributions_by_chart_ids(&missing_ids) {
                Ok(distributions) => {
                    let mut cache = self.select_distribution_cache.borrow_mut();
                    for (chart_id, distribution) in distributions {
                        cache.insert(chart_id, distribution);
                    }
                    for chart_id in missing_ids {
                        cache.entry(chart_id).or_default();
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "failed to load visible chart distributions");
                }
            }
        }
        self.select_distribution_cache
            .borrow_mut()
            .retain(|chart_id, _| chart_ids.contains(chart_id));
    }

    /// Returns the string to render in the skin's `STRING_SEARCHWORD` (ref=30)
    /// slot along with an alpha multiplier (0.0..=1.0). beatoraja's libgdx
    /// `TextField` uses `messageFontColor=GRAY` for placeholder; we approximate
    /// that by multiplying skin-resolved alpha by `< 1.0` for placeholder /
    /// feedback states.
    pub(super) fn display_search_word(&self) -> (String, f32, Option<usize>) {
        self.search.display_word(
            in_settings_stack(&self.folder_stack),
            Localizer::new(self.boot.profile_config.ui.locale()).text("select-search-placeholder"),
        )
    }

    pub(super) fn poll_select_asset_loads(&mut self) {
        let (image_uploads, previews) = self.select_assets.poll_loads();
        for upload in image_uploads {
            self.upload_select_meta_image(upload.slot, &upload.image);
        }
        for prepared in previews {
            if self.play_select_preview_sample(prepared, 0.0) {
                self.apply_select_preview_audio_mix();
            }
        }
    }

    pub(super) fn selected_chart_needs_generated_preview_distribution(&self) -> bool {
        match self.select_items.get(self.selected_index) {
            Some(SelectItem::Chart(row)) => row.chart.as_ref().is_some_and(|chart| {
                let explicit_key = format!("{}|{}", chart.folder_path, chart.preview_file);
                let explicit_missing = self.select_assets.explicit_preview_missing(&explicit_key);
                should_use_generated_preview(&chart.preview_file, explicit_missing)
            }),
            _ => false,
        }
    }

    pub(super) fn selected_select_preview_cache_key(&self) -> Option<String> {
        match self.select_items.get(self.selected_index) {
            Some(SelectItem::Chart(row)) => {
                let chart = row.chart.as_ref()?;
                let explicit_key = format!("{}|{}", chart.folder_path, chart.preview_file);
                let explicit_missing = self.select_assets.explicit_preview_missing(&explicit_key);
                if !should_use_generated_preview(&chart.preview_file, explicit_missing) {
                    return Some(explicit_key);
                }
                let distributions = self.select_distribution_cache.borrow();
                let distribution = distributions.get(&chart.chart_id)?;
                let start_ms = fallback_preview_start_ms(distribution, chart.length_ms)?;
                Some(generated_preview_cache_key(chart.chart_id, start_ms))
            }
            _ => None,
        }
    }

    pub(super) fn sync_select_preview_audio(&mut self) {
        if self.selected_chart_needs_generated_preview_distribution() {
            self.ensure_visible_select_chart_distributions(25);
        }
        let selected_cache_key = self.selected_select_preview_cache_key();
        let cache_key = select_preview_key_after_delay(
            selected_cache_key,
            self.select_bar_started_at.elapsed(),
            SELECT_PREVIEW_START_DELAY,
        );
        match self.select_assets.sync_preview(cache_key, Instant::now()) {
            SelectPreviewSyncAction::None => {}
            SelectPreviewSyncAction::Play(prepared) => {
                if self.play_select_preview_sample(prepared, 0.0) {
                    self.apply_select_preview_audio_mix();
                }
            }
            SelectPreviewSyncAction::ApplyMix => self.apply_select_preview_audio_mix(),
        }
    }

    pub(super) fn stop_select_preview(&mut self) {
        self.select_assets.stop_preview();
        self.set_select_bgm_volume_factor(1.0);
    }

    pub(super) fn sync_select_banner_texture(&mut self) {
        self.sync_select_meta_image_texture(SelectMetaImageSlot::Banner);
    }

    pub(super) fn sync_select_stage_texture(&mut self) {
        self.sync_select_meta_image_texture(SelectMetaImageSlot::Stage);
    }

    pub(super) fn sync_select_backbmp_texture(&mut self) {
        self.sync_select_meta_image_texture(SelectMetaImageSlot::Backbmp);
    }

    pub(super) fn sync_select_meta_image_texture(&mut self, slot: SelectMetaImageSlot) {
        let cache_key = match self.select_items.get(self.selected_index) {
            Some(SelectItem::Chart(row)) => row.chart.as_ref().and_then(|chart| {
                let file = match slot {
                    SelectMetaImageSlot::Stage => &chart.stage_file,
                    SelectMetaImageSlot::Backbmp => &chart.backbmp_file,
                    SelectMetaImageSlot::Banner => &chart.banner_file,
                };
                (!file.is_empty()).then(|| format!("{}|{}", chart.folder_path, file))
            }),
            _ => None,
        };
        if let Some(image) = self.select_assets.sync_meta_image(slot, cache_key) {
            self.upload_select_meta_image(slot, &image);
        }
    }

    pub(super) fn upload_select_meta_image(
        &mut self,
        slot: SelectMetaImageSlot,
        image: &RgbaImageAsset,
    ) -> bool {
        let texture_id = match slot {
            SelectMetaImageSlot::Stage => SELECT_STAGE_TEXTURE,
            SelectMetaImageSlot::Backbmp => PLAY_BACKBMP_TEXTURE,
            SelectMetaImageSlot::Banner => SELECT_BANNER_TEXTURE,
        };
        if let Err(error) = self.renderer.upsert_image_asset(texture_id, image) {
            tracing::warn!(%error, "failed to upload select meta image");
            self.select_assets.finish_meta_image_upload(slot, None);
            false
        } else {
            self.select_assets.finish_meta_image_upload(
                slot,
                Some(SkinImageSize { width: image.width as f32, height: image.height as f32 }),
            );
            true
        }
    }

    pub(super) fn select_preview_volume(&self) -> f32 {
        self.select_preview_volume_for_gain(self.select_assets.preview_normalization_gain())
    }

    pub(super) fn select_preview_volume_for_gain(&self, analyzed_gain: f32) -> f32 {
        let mix = &self.boot.profile_config.audio_mix;
        let volume = crate::config::play::volume_unit_to_f32(mix.master_volume)
            * crate::config::play::volume_unit_to_f32(mix.preview_volume)
            * select_preview_normalization_gain(mix.normalize_chart_volume, analyzed_gain);
        volume.clamp(0.0, 1.0)
    }

    pub(super) fn play_select_preview_sample(
        &mut self,
        prepared: PreparedSelectPreview,
        volume_factor: f32,
    ) -> bool {
        let volume = self.select_preview_volume_for_gain(prepared.normalization_gain)
            * volume_factor.clamp(0.0, 1.0);
        let loaded = self.select_assets.play_preview(prepared, volume, Instant::now());
        if loaded {
            self.start_audio_output_stream();
        }
        loaded
    }

    pub(super) fn update_select_preview_fade(&mut self) {
        let now = Instant::now();
        self.select_assets.advance_preview_fade(now);
        self.apply_select_preview_audio_mix();
    }

    pub(super) fn apply_select_preview_audio_mix(&self) {
        let preview_factor = self.select_assets.preview_fade_factor(Instant::now());
        self.select_assets.set_preview_volume(self.select_preview_volume() * preview_factor);
        self.set_select_bgm_volume_factor(1.0 - preview_factor);
    }

    pub(super) fn set_select_bgm_volume_factor(&self, factor: f32) {
        let Some(manager) = &self.system_sound else {
            return;
        };
        let volume = system_sound_volume_from_mix(
            &self.boot.profile_config.audio_mix,
            crate::system_sound::SoundType::Select,
        ) * factor.clamp(0.0, 1.0);
        manager.set_volume(crate::system_sound::SoundType::Select, volume);
    }

    pub(super) fn should_exit_via_select_hold(&mut self) -> bool {
        if !matches!(self.view_state(), AppViewState::Select) {
            self.select_exit_hold_started_at = None;
            return false;
        }
        let Some(started) = self.select_exit_hold_started_at else {
            return false;
        };
        started.elapsed() >= SELECT_EXIT_HOLD_DURATION
    }

    pub(super) fn select_exit_hold_progress(&self) -> f32 {
        let Some(started) = self.select_exit_hold_started_at else {
            return 0.0;
        };
        let elapsed = started.elapsed().as_secs_f32();
        let total = SELECT_EXIT_HOLD_DURATION.as_secs_f32();
        (elapsed / total).clamp(0.0, 1.0)
    }

    pub(super) fn select_time(&self) -> TimeUs {
        let micros =
            self.select_scene_started_at.elapsed().as_micros().min(i64::MAX as u128) as i64;
        TimeUs(micros)
    }

    pub(super) fn select_bar_time(&self) -> TimeUs {
        let micros = self.select_bar_started_at.elapsed().as_micros().min(i64::MAX as u128) as i64;
        TimeUs(micros)
    }

    pub(super) fn restart_select_bar_timer_without_scroll(&mut self, now: Instant) {
        self.select_bar_started_at = now;
        self.select_bar_scroll_direction = 0;
        self.select_bar_scroll_duration = Duration::ZERO;
    }

    pub(super) fn select_bar_scroll_progress(&self) -> f32 {
        if self.select_bar_scroll_direction == 0 || self.select_bar_scroll_duration.is_zero() {
            return 0.0;
        }
        let elapsed = self.select_bar_started_at.elapsed();
        if elapsed >= self.select_bar_scroll_duration {
            return 0.0;
        }
        1.0 - elapsed.as_secs_f32() / self.select_bar_scroll_duration.as_secs_f32()
    }

    pub(super) fn select_scroll_duration_low(&self) -> Duration {
        Duration::from_millis(u64::from(select_scroll_duration_low_ms(&self.boot.app_config)))
    }

    pub(super) fn select_scroll_duration_high(&self) -> Duration {
        Duration::from_millis(u64::from(select_scroll_duration_high_ms(&self.boot.app_config)))
    }

    pub(super) fn play_elapsed_time(&self) -> TimeUs {
        let micros = self.play_scene_started_at.elapsed().as_micros().min(i64::MAX as u128) as i64;
        TimeUs(micros)
    }

    pub(super) fn decide_snapshot(&self, decide: &DecideTransition) -> RenderSnapshot {
        let mut snapshot = decide.snapshot_for_render();
        let elapsed = match decide.fadeout_started_at {
            Some(fadeout_started_at) => {
                let fadeout_duration = self.decide_fadeout_duration();
                let fadeout_elapsed = fadeout_started_at.elapsed().min(fadeout_duration);
                let scene_elapsed = decide_fadeout_scene_elapsed(
                    fadeout_started_at.duration_since(decide.started_at),
                    fadeout_elapsed,
                    self.decide_scene_duration(),
                    fadeout_duration,
                    self.decide_fadeout_scene_timing(),
                );
                TimeUs(scene_elapsed.as_micros().min(i64::MAX as u128) as i64)
            }
            None => elapsed_since(decide.started_at),
        };
        snapshot.play_elapsed_time = elapsed;
        snapshot.fadeout_elapsed_ms = decide.fadeout_started_at.map(|started_at| {
            let elapsed_ms = elapsed_since_ms(started_at);
            let fadeout_ms =
                self.decide_fadeout_duration().as_millis().min(i32::MAX as u128) as i32;
            elapsed_ms.min(fadeout_ms)
        });
        snapshot
    }

    pub(super) fn option_panel_time(&self) -> TimeUs {
        let micros =
            self.option_panel_started_at.elapsed().as_micros().min(i64::MAX as u128) as i64;
        TimeUs(micros)
    }

    pub(super) fn set_start_held(&mut self, held: bool) {
        if self.input.start_held != held {
            self.input.start_held = held;
            self.update_select_option_panel();
        }
    }

    pub(super) fn set_select_held(&mut self, held: bool) {
        if self.input.select_held != held {
            self.input.select_held = held;
            self.update_select_option_panel();
        }
    }

    pub(super) fn sync_select_holds_from_pressed_controls(&mut self) {
        let (start_held, select_held, e_action_holds) = select_hold_state_from_pressed_controls(
            &self.input.pressed_controls,
            &self.select_keys,
        );
        self.input.select_e_action_holds = e_action_holds;
        self.set_start_held(start_held);
        self.set_select_held(select_held);
    }

    pub(super) fn update_select_e_action_hold(&mut self, control: &str, held: bool) {
        let Some(action) = self.select_keys.e_action_for_control(control) else {
            return;
        };
        if held {
            self.input.select_e_action_holds.insert(action);
        } else {
            self.input.select_e_action_holds.remove(&action);
        }
    }

    pub(super) fn select_e_action_held(&self) -> bool {
        self.input.select_e_action_held()
    }

    pub(super) fn update_select_option_panel(&mut self) {
        let panel = if in_settings_stack(&self.folder_stack) {
            0
        } else {
            select_option_panel_for_holds(self.input.start_held, self.input.select_held)
        };
        let previous_panel = self.select_option_panel;
        let now = Instant::now();
        if transition_select_option_panel(
            &mut self.select_option_panel,
            &mut self.option_panel_started_at,
            &mut self.option_panel_off_started_at,
            panel,
            now,
        ) {
            self.reset_select_analog_scroll();
            if let Some(sound_type) =
                select_option_panel_sound_for_transition(previous_panel, panel)
            {
                self.play_system_sound(sound_type);
            }
        }
    }

    pub(super) fn begin_settings_edit(&mut self, entry_id: SettingsEntryId) {
        self.settings_edit =
            Some(SettingsEditSession::capture(&self.boot.profile_config, entry_id));
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
        tracing::info!(?entry_id, "settings edit mode started");
    }

    pub(super) fn cancel_settings_edit(&mut self) {
        let Some(session) = self.settings_edit.take() else {
            return;
        };
        let entry_id = session.entry_id;
        let score_context_before = SelectScoreContext::from_profile(&self.boot.profile_config);
        session.restore(&mut self.boot.profile_config);
        self.sync_select_settings_from_profile_if_needed(entry_id);
        self.sync_changed_select_score_context(score_context_before);
        self.play_system_sound(crate::system_sound::SoundType::FolderClose);
        tracing::info!(?entry_id, "settings edit cancelled");
    }

    pub(super) fn commit_settings_edit(&mut self) {
        let Some(session) = self.settings_edit.take() else {
            return;
        };
        let entry_id = session.entry_id;
        self.boot.profile_config.updated_at = now_unix_seconds();
        match save_profile_config(&self.boot.profile_paths.profile_toml, &self.boot.profile_config)
        {
            Ok(()) => {
                self.sync_select_settings_from_profile_if_needed(entry_id);
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                tracing::info!(?entry_id, "settings edit saved");
            }
            Err(error) => {
                tracing::error!(%error, ?entry_id, "failed to save settings");
                let score_context_before =
                    SelectScoreContext::from_profile(&self.boot.profile_config);
                session.restore(&mut self.boot.profile_config);
                self.sync_select_settings_from_profile_if_needed(entry_id);
                self.sync_changed_select_score_context(score_context_before);
            }
        }
    }

    pub(super) fn begin_key_config_edit(
        &mut self,
        key_mode: bmz_core::lane::KeyMode,
        target: KeyBindingTarget,
    ) {
        self.key_config_edit =
            Some(KeyConfigEditSession::begin(key_mode, target, &self.boot.profile_config));
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
        tracing::info!(?key_mode, ?target, "key config listen started");
    }

    pub(super) fn cancel_key_config_edit(&mut self) {
        let Some(session) = self.key_config_edit.take() else {
            return;
        };
        let target = session.target;
        session.cancel(&mut self.boot.profile_config);
        self.suppress_select_analog_until_idle();
        self.play_system_sound(crate::system_sound::SoundType::FolderClose);
        tracing::info!(?target, "key config cancelled");
    }

    pub(super) fn commit_key_config_edit(&mut self) {
        let Some(session) = self.key_config_edit.take() else {
            return;
        };
        let target = session.target;
        self.suppress_select_analog_until_idle();
        self.boot.profile_config.updated_at = now_unix_seconds();
        match save_profile_config(&self.boot.profile_paths.profile_toml, &self.boot.profile_config)
        {
            Ok(()) => {
                self.select_keys = SelectKeyBindings::from_profile(&self.boot.profile_config.input);
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                tracing::info!(?target, "key config saved");
            }
            Err(error) => {
                tracing::error!(%error, ?target, "failed to save key config");
                session.cancel(&mut self.boot.profile_config);
            }
        }
    }

    pub(super) fn apply_key_config_control(&mut self, control: &str) {
        let Some(session) = self.key_config_edit.as_ref() else {
            return;
        };
        if !session.listening {
            return;
        }
        if !matches!(
            session.target.slot(),
            KeyBindingSlot::KeyboardPrimary | KeyBindingSlot::KeyboardSecondary
        ) {
            return;
        }
        let target = session.target;
        let key_mode = session.key_mode;
        if let Err(error) =
            apply_play_binding(&mut self.boot.profile_config.input, key_mode, target, control)
        {
            tracing::warn!(%error, ?key_mode, ?target, control, "failed to apply key binding");
            return;
        }
        self.commit_key_config_edit();
    }

    pub(super) fn apply_key_config_gamepad(&mut self, control: &str) {
        let Some(session) = self.key_config_edit.as_ref() else {
            return;
        };
        if !session.listening || !session.target.slot().is_controller() {
            return;
        }
        let target = session.target;
        let key_mode = session.key_mode;
        if let Err(error) =
            apply_play_binding(&mut self.boot.profile_config.input, key_mode, target, control)
        {
            tracing::warn!(%error, ?key_mode, ?target, control, "failed to apply controller binding");
            return;
        }
        self.commit_key_config_edit();
    }

    pub(super) fn clear_key_config_binding(&mut self) {
        let Some(session) = self.key_config_edit.as_ref() else {
            return;
        };
        if !session.listening {
            return;
        }
        let target = session.target;
        let key_mode = session.key_mode;
        if let Err(error) =
            clear_play_binding(&mut self.boot.profile_config.input, key_mode, target)
        {
            tracing::warn!(%error, ?key_mode, ?target, "failed to clear key binding");
            return;
        }
        self.commit_key_config_edit();
    }

    pub(super) fn adjust_settings_edit(&mut self, direction: i32) {
        if direction == 0 {
            return;
        }
        let Some(session) = self.settings_edit.as_ref() else {
            return;
        };
        let entry_id = session.entry_id;
        let delta = direction * crate::config::settings_registry::settings_adjust_step(entry_id);
        let score_context_before = SelectScoreContext::from_profile(&self.boot.profile_config);
        if adjust_settings_draft(&mut self.boot.profile_config, session, delta) {
            self.sync_select_settings_from_profile_if_needed(entry_id);
            self.sync_changed_select_score_context(score_context_before);
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
        }
    }

    pub(super) fn sync_select_settings_from_profile_if_needed(
        &mut self,
        entry_id: SettingsEntryId,
    ) {
        self.sync_select_play_options_from_profile_if_needed(entry_id);
        if entry_id == SettingsEntryId::SelectInputMode {
            self.select_keys = SelectKeyBindings::from_profile(&self.boot.profile_config.input);
            self.sync_select_holds_from_pressed_controls();
        }
        if matches!(
            entry_id,
            SettingsEntryId::AnalogScratchSensitivity | SettingsEntryId::AnalogScratchThreshold
        ) {
            self.apply_gamepad_analog_config();
        }
        if SettingsEntryId::VOLUME_ENTRIES.contains(&entry_id) {
            self.sync_realtime_profile_settings();
        }
    }

    pub(super) fn sync_changed_gamepad_analog_config_from_profile(
        &mut self,
        before: &ProfileInputConfig,
    ) {
        let after = &self.boot.profile_config.input;
        if before.analog_scratch_sensitivity == after.analog_scratch_sensitivity
            && before.analog_scratch_threshold == after.analog_scratch_threshold
        {
            return;
        }
        self.apply_gamepad_analog_config();
    }

    pub(super) fn apply_gamepad_analog_config(&mut self) {
        let input = &self.boot.profile_config.input;
        if let Some(gamepad) = &mut self.gamepad {
            gamepad.set_analog_config(
                input.analog_scratch_sensitivity,
                input.analog_scratch_threshold,
            );
            tracing::info!(
                sensitivity = input.analog_scratch_sensitivity,
                threshold = input.analog_scratch_threshold,
                "applied analog scratch settings"
            );
        }
    }

    pub(super) fn sync_select_play_options_from_profile_if_needed(
        &mut self,
        entry_id: SettingsEntryId,
    ) {
        if !SettingsEntryId::PLAY_ENTRIES.contains(&entry_id) {
            return;
        }
        self.sync_select_play_options_from_profile();
    }

    pub(super) fn sync_select_play_options_from_profile(&mut self) {
        let options = select_play_options_from_profile(&self.boot.profile_config.play);
        self.set_select_play_options(options);
    }

    pub(super) fn sync_changed_select_play_options_from_profile(
        &mut self,
        before: &PlayDefaultsConfig,
    ) {
        let current = self.current_select_play_options();
        let next = merge_changed_select_play_options_from_profile(
            current,
            before,
            &self.boot.profile_config.play,
        );
        if next != current {
            self.set_select_play_options(next);
            tracing::info!("applied profile play settings to select options");
        }
    }

    pub(super) fn sync_changed_select_score_context(&mut self, before: SelectScoreContext) {
        let after = SelectScoreContext::from_profile(&self.boot.profile_config);
        if before == after {
            return;
        }

        self.select_folder_summaries.sync_score_context(
            &mut self.select_items,
            self.boot.profile_config.play.ln_mode_policy,
            self.boot.profile_config.play.rule_mode,
        );
        self.reload_select_items();
        self.invalidate_play_preload();
        // Result画面からのリトライ用cacheも古いscore key / LN変換済みchartを持つ。
        self.play_media_cache = None;
        tracing::info!(
            rule_mode = after.rule_mode.as_str(),
            ln_mode = after.ln_mode_policy.display_label(),
            "applied profile score context to select"
        );
    }

    pub(super) fn current_select_play_options(&self) -> CurrentPlayOptions {
        CurrentPlayOptions {
            arrange: self.arrange_option,
            arrange_2p: self.arrange_option_2p,
            target: self.target_option,
            gauge: self.gauge_option,
            gauge_auto_shift: self.gauge_auto_shift_option,
            bottom_shiftable_gauge: self.bottom_shiftable_gauge_option,
            double_option: self.double_option,
            hs_fix: self.hs_fix_option,
            session_mode: self.session_mode,
        }
    }

    pub(super) fn set_select_play_options(&mut self, options: CurrentPlayOptions) {
        self.arrange_option = options.arrange;
        self.arrange_option_2p = options.arrange_2p;
        self.target_option = options.target;
        self.gauge_option = options.gauge;
        self.gauge_auto_shift_option = options.gauge_auto_shift;
        self.bottom_shiftable_gauge_option = options.bottom_shiftable_gauge;
        self.double_option = options.double_option;
        self.hs_fix_option = options.hs_fix;
        self.session_mode = options.session_mode;
    }

    pub(super) fn route_settings_control(&mut self, control: &str) -> bool {
        let bindings = SettingsBindings::from_profile(&self.boot.profile_config.input);

        if control.starts_with("Axis")
            && (self.select_keys.is_select_scratch_up(control)
                || self.select_keys.is_select_scratch_down(control))
        {
            return true;
        }

        if self.key_config_edit.is_some() {
            if bindings.is_back(control) {
                self.cancel_key_config_edit();
            }
            return true;
        }

        if self.settings_edit.is_some() {
            if bindings.is_confirm(control) {
                self.commit_settings_edit();
                return true;
            }
            if bindings.is_back(control) {
                self.cancel_settings_edit();
                return true;
            }
            if bindings.is_increase(control) {
                self.adjust_settings_edit(1);
                return true;
            }
            if bindings.is_decrease(control) {
                self.adjust_settings_edit(-1);
                return true;
            }
            return true;
        }

        if bindings.is_back(control) {
            self.exit_folder();
            return true;
        }
        if let Some(select_move) =
            settings_browse_move_control(control, &bindings, &self.select_keys)
        {
            self.move_selection(select_move);
            self.start_select_hold_move(select_move, control.to_string());
            return true;
        }
        if bindings.is_confirm(control) {
            return match self.select_items.get(self.selected_index) {
                Some(SelectItem::Config(row)) => {
                    self.begin_settings_edit(row.entry_id);
                    true
                }
                Some(SelectItem::KeyBinding(row)) => {
                    self.begin_key_config_edit(row.key_mode, row.target);
                    true
                }
                Some(SelectItem::Folder { .. }) => {
                    self.enter_or_play_selected();
                    true
                }
                Some(SelectItem::SettingsBack | SelectItem::SettingsClose) => {
                    self.exit_folder();
                    true
                }
                Some(SelectItem::AdvancedSettings) => {
                    self.open_advanced_settings_from_select();
                    true
                }
                _ => false,
            };
        }
        false
    }

    pub(super) fn cycle_bga_option(&mut self) {
        self.boot.profile_config.play.bga = cycle_bga_option(self.boot.profile_config.play.bga);
        tracing::info!(
            bga = bga_mode_as_str(self.boot.profile_config.play.bga),
            "bga option changed"
        );
    }

    pub(super) fn toggle_gauge_auto_shift(&mut self) {
        self.gauge_auto_shift_option = cycle_gauge_auto_shift_option(self.gauge_auto_shift_option);
        tracing::info!(
            gauge_auto_shift = gauge_auto_shift_as_str(self.gauge_auto_shift_option),
            "gauge auto shift changed"
        );
    }

    pub(super) fn toggle_visual_offset_auto_adjust(&mut self) {
        self.boot.profile_config.judge.visual_offset_auto_adjust =
            !self.boot.profile_config.judge.visual_offset_auto_adjust;
        self.boot.profile_config.updated_at = now_unix_seconds();
        self.sync_realtime_profile_settings();
        tracing::info!(
            visual_offset_auto_adjust = self.boot.profile_config.judge.visual_offset_auto_adjust,
            "visual offset auto adjust changed"
        );
    }

    pub(super) fn apply_play_option_control(&mut self, control: &str) -> bool {
        if self.select_keys.is_key1(control) {
            self.arrange_option = self.arrange_option.cycle();
            tracing::info!(arrange = self.arrange_option.as_str(), "arrange option changed");
            true
        } else if self.select_keys.is_key2(control) {
            self.arrange_option = self.arrange_option.cycle_prev();
            tracing::info!(arrange = self.arrange_option.as_str(), "arrange option changed");
            true
        } else if self.select_keys.is_key8(control) {
            self.arrange_option_2p = self.arrange_option_2p.cycle();
            tracing::info!(arrange_2p = self.arrange_option_2p.as_str(), "2P arrange changed");
            true
        } else if self.select_keys.is_key9(control) {
            self.arrange_option_2p = self.arrange_option_2p.cycle_prev();
            tracing::info!(arrange_2p = self.arrange_option_2p.as_str(), "2P arrange changed");
            true
        } else if self.select_keys.is_ui_key3(control) {
            self.gauge_option = cycle_gauge_option(self.gauge_option);
            tracing::info!(gauge = ?self.gauge_option, "gauge option changed");
            true
        } else if self.select_keys.is_ui_key4(control) {
            self.gauge_option = cycle_gauge_option_prev(self.gauge_option);
            tracing::info!(gauge = ?self.gauge_option, "gauge option changed");
            true
        } else if self.select_keys.is_ui_key5(control) {
            self.hs_fix_option = self.hs_fix_option.cycle();
            tracing::info!(hs_fix = self.hs_fix_option.as_str(), "HS-FIX option changed");
            true
        } else if self.select_keys.is_ui_key6(control) {
            self.double_option = self.double_option.cycle();
            tracing::info!(double_option = self.double_option.as_str(), "double option changed");
            true
        } else if self.select_keys.is_ui_key7(control) {
            self.set_session_mode(self.session_mode.cycle());
            tracing::info!(session_mode = self.session_mode.as_str(), "session mode changed");
            true
        } else {
            false
        }
    }

    pub(super) fn apply_gamepad_play_option_control(
        &mut self,
        device: DeviceId,
        control: &str,
    ) -> bool {
        let app_config = self.play_session_app_config();
        let slots = crate::input::gamepad::GamepadSlotMap::from_runtime_or_legacy(
            app_config.input.gamepad_slot_runtime_device_ids,
            app_config.input.gamepad_slot_gilrs_ids,
        );
        match select_option_lane_for_gamepad(
            &self.boot.profile_config.input,
            slots,
            device,
            control,
        ) {
            Some(Lane::Key1) => {
                self.arrange_option = self.arrange_option.cycle();
                tracing::info!(arrange = self.arrange_option.as_str(), "arrange option changed");
                true
            }
            Some(Lane::Key2) => {
                self.arrange_option = self.arrange_option.cycle_prev();
                tracing::info!(arrange = self.arrange_option.as_str(), "arrange option changed");
                true
            }
            Some(Lane::Key8) => {
                self.arrange_option_2p = self.arrange_option_2p.cycle();
                tracing::info!(arrange_2p = self.arrange_option_2p.as_str(), "2P arrange changed");
                true
            }
            Some(Lane::Key9) => {
                self.arrange_option_2p = self.arrange_option_2p.cycle_prev();
                tracing::info!(arrange_2p = self.arrange_option_2p.as_str(), "2P arrange changed");
                true
            }
            _ => self.apply_play_option_control(control),
        }
    }

    pub(super) fn set_session_mode(&mut self, session_mode: SessionMode) {
        self.session_mode = session_mode;
        self.boot.profile_config.play.session_mode = Some(session_mode);
        self.boot.profile_config.play.auto_play = session_mode.primary_autoplay();
    }

    pub(super) fn apply_target_option_cycle(&mut self, cycle: TargetCycle) {
        self.target_option = match cycle {
            TargetCycle::Previous => self.target_option.cycle_prev(),
            TargetCycle::Next => self.target_option.cycle(),
        };
        tracing::info!(target = self.target_option.as_str(), "target option changed");
    }

    pub(super) fn apply_detail_option_control(&mut self, control: &str) -> bool {
        if self.select_keys.cycle_bga() == Some(control) || self.select_keys.is_ui_key1(control) {
            self.cycle_bga_option();
            true
        } else if let Some(delta) = green_number_delta_control(control, &self.select_keys) {
            self.adjust_select_green_number(delta)
        } else if let Some(delta_ms) = visual_offset_delta_control(control, &self.select_keys) {
            self.adjust_visual_offset_ms(delta_ms)
        } else {
            false
        }
    }

    pub(super) fn adjust_select_green_number(&mut self, delta: i32) -> bool {
        let current = self.boot.profile_config.lane.target_green_number.max(1);
        let next = adjusted_green_number(current, delta);
        if current == next {
            return false;
        }
        self.boot.profile_config.lane.target_green_number = next;
        self.boot.profile_config.updated_at = now_unix_seconds();
        self.sync_realtime_profile_settings();
        tracing::info!(target_green_number = next, "select green number changed");
        true
    }

    pub(super) fn adjust_visual_offset_ms(&mut self, delta_ms: i32) -> bool {
        let changed = crate::config::settings_registry::adjust_settings_value(
            &mut self.boot.profile_config,
            SettingsEntryId::VisualOffsetMs,
            delta_ms,
        );
        if changed {
            self.boot.profile_config.updated_at = now_unix_seconds();
            self.sync_realtime_profile_settings();
            tracing::info!(
                visual_offset_ms = self.boot.profile_config.judge.visual_offset_us / 1_000,
                "visual judge offset changed"
            );
        }
        changed
    }

    pub(super) fn apply_select_action(&mut self, action: SelectAction, hold_control: Option<&str>) {
        match action {
            SelectAction::EnterOrPlay => self.enter_or_play_selected(),
            SelectAction::ExitFolder => self.exit_folder(),
            SelectAction::FavoriteSong => self.toggle_favorite_song_selected(),
            SelectAction::FavoriteChart => self.toggle_favorite_chart_selected(),
            SelectAction::SameFolder => self.open_same_folder_for_selected(),
            SelectAction::Move(select_move) => {
                self.move_selection(select_move);
                if matches!(
                    select_move,
                    SelectMove::Previous
                        | SelectMove::Next
                        | SelectMove::PagePrevious
                        | SelectMove::PageNext
                ) && let Some(control) = hold_control
                {
                    self.start_select_hold_move(select_move, control.to_string());
                }
            }
        }
    }

    pub(super) fn apply_result_action(&mut self, action: ResultAction, course_result: bool) {
        match (course_result, action) {
            (false, ResultAction::Retry) => {
                self.begin_result_exit(ResultExitAction::Retry(ResultRetryMode::SameArrange))
            }
            (true, ResultAction::Retry) => {
                self.begin_result_exit(ResultExitAction::RetryCourseSameArrange)
            }
            (_, ResultAction::Leave) => self.begin_result_exit(ResultExitAction::Leave),
        }
    }

    pub(super) fn route_keyboard_input(&mut self, event: &winit::event::KeyEvent) {
        if !event.repeat
            && let Some(device_event) = key_event_to_device_input(event)
            && self.filter_app_input_bounce(device_event).is_none()
        {
            return;
        }
        let control_event = ControlInputEvent::keyboard(event);
        self.input.track_control(&control_event);
        let play_control = control_event.name.as_deref();
        let play_physical_control = control_event.physical.as_ref();
        let has_play_control_context =
            self.active_play.is_some() || self.pending_play_start.is_some();
        if control_event.pressed
            && !control_event.repeat
            && let Some(control) = play_control
            && self.handle_quick_retry_control(control)
        {
            return;
        }
        if control_event.pressed
            && !control_event.repeat
            && let Some(control) = play_control
            && self.begin_play_fadeout_after_final_notes_control(control)
        {
            return;
        }
        if has_play_control_context && let Some(control) = play_physical_control.as_ref() {
            self.update_play_e1_control_state(
                W_KEYBOARD_DEVICE_ID,
                control,
                event.state == ElementState::Pressed,
            );
        }
        if has_play_control_context
            && let Some(control) = play_physical_control.as_ref()
            && self.update_play_exit_control_state(
                W_KEYBOARD_DEVICE_ID,
                control,
                event.state == ElementState::Pressed,
            )
        {
            return;
        }
        let window_keyboard_gameplay_enabled = self.window_keyboard_gameplay_enabled();
        self.input.track_window_keyboard(
            event.physical_key,
            event.state,
            event.repeat,
            window_keyboard_gameplay_enabled,
            has_play_control_context,
        );
        if has_play_control_context
            && window_keyboard_gameplay_enabled
            && let Some(device_event) = key_event_to_device_input(event)
        {
            self.route_play_device_input(device_event);
        }
        let play_option_lane_action = if event.state == ElementState::Pressed && !event.repeat {
            play_physical_control
                .as_ref()
                .and_then(|control| {
                    play_option_control_for_input(
                        W_KEYBOARD_DEVICE_ID,
                        control,
                        self.play_e1_held,
                        self.play_e2_held,
                        self.play_option_input.as_ref(),
                        &self.boot.profile_config.input,
                    )
                })
                .and_then(|action| lane_action_from_option(action, false))
        } else {
            None
        };
        let fixed_play_lane_action = keyboard_lane_action(&control_event);
        if self.active_play.is_some() {
            let lane_cover_changing = self
                .active_play
                .as_ref()
                .is_some_and(|play| play.running.session.lane_cover_changing);
            if lane_cover_changing && let Some(action) = play_option_lane_action {
                self.apply_play_lane_action(action);
                // E1+lane keys should still reach gameplay input so notes are judged
                // and key beams render while changing play options.
            }
            if let Some(action) = fixed_play_lane_action {
                self.apply_play_lane_action(action);
                return;
            }
            if event.physical_key == PhysicalKey::Code(KeyCode::Escape)
                && event.state == ElementState::Pressed
                && !event.repeat
            {
                self.stop_active_play_like_escape("escape pressed during play");
                return;
            }
            // Start / E1 の2回連続押し → レーンカバー表示切替
            if control_event.pressed
                && !control_event.repeat
                && let Some(control) = play_control
                && self.select_keys.is_start(control)
            {
                self.handle_play_start_double_press();
                // Start キーはゲームプレイ入力としても通すのでフォールスルー
            }
            return;
        }

        if self.pending_decide.is_some() {
            if let Some(control) = control_event.name.as_deref()
                && !event.repeat
                && self.update_decide_cancel_control_state(
                    control,
                    event.state == ElementState::Pressed,
                )
            {
                return;
            }
            if let Some(action) = scene_decide_action(&control_event, &self.select_keys) {
                self.begin_decide_fadeout(matches!(action, DecideAction::Cancel));
            }
            return;
        }

        if self.pending_play_start.is_some() {
            if let Some(action) = fixed_play_lane_action {
                self.apply_play_lane_action(action);
                return;
            }
            if let Some(action) = play_option_lane_action {
                self.apply_play_lane_action(action);
                return;
            }
            if event.state == ElementState::Pressed
                && !event.repeat
                && let Some(control) = play_control
                && self.select_keys.is_start(control)
            {
                self.handle_play_start_double_press();
            }
            return;
        }

        // コース曲間の中間リザルト: リトライ無効、次の曲へ進むだけ。Key6 の
        // ゲージグラフ切替のみ単曲リザルト同様に許可する。retry を持つ単曲
        // リザルト分岐より先に評価し、R/Key5/Key7 等での誤 retry を防ぐ。
        if self.is_course_intermediate_result() {
            let pressed = event.state == ElementState::Pressed;
            if self.request_result_exit_skip_for_key(event.physical_key, event.state, event.repeat)
            {
                return;
            }
            if self.result_exit.is_none()
                && let Some(control) = physical_key_to_control(event.physical_key)
                && self.handle_course_intermediate_control(&control, pressed, event.repeat)
            {
                return;
            }
            if let Some(control) = physical_key_to_control(event.physical_key)
                && self.request_result_exit_skip_for_control(&control, pressed, event.repeat)
            {
                return;
            }
            if self.result_exit.is_none()
                && self.result_input_ready()
                && event.state == ElementState::Pressed
                && !event.repeat
                && let Some(slot) = digit_to_replay_slot(event.physical_key)
            {
                self.save_finished_play_replay_slot(slot);
                return;
            }
            if self.result_exit.is_none()
                && self.result_input_ready()
                && scene_result_action(&control_event).is_some()
            {
                // R / Enter / Escape いずれも次の曲へ進むだけ (retry/leave 区別なし)。
                self.begin_result_exit(self.course_intermediate_exit_action());
            }
            return;
        }

        if self.finished_play.is_some() && self.finished_course.is_none() {
            let pressed = event.state == ElementState::Pressed;
            if let Some(control) = physical_key_to_control(event.physical_key) {
                // フェードアウト中でも Key5/Key7 の押下状態は追跡し、
                // アニメーション終了時の retry arrange 判定に使う。
                self.track_result_lane_hold(&control, pressed);
                if self.request_result_exit_skip_for_key(
                    event.physical_key,
                    event.state,
                    event.repeat,
                ) || self.request_result_exit_skip_for_control(&control, pressed, event.repeat)
                {
                    return;
                }
                // 終了アニメーション中 (result_exit=Some) は held 追跡のみで、
                // 新しいアクションは受け付けない。
                if self.result_exit.is_none()
                    && self.handle_result_control(&control, pressed, event.repeat)
                {
                    return;
                }
            }
            if self.result_exit.is_none()
                && self.result_input_ready()
                && event.state == ElementState::Pressed
                && !event.repeat
                && let Some(slot) = digit_to_replay_slot(event.physical_key)
            {
                self.save_finished_play_replay_slot(slot);
                return;
            }
            if self.result_exit.is_none()
                && self.result_input_ready()
                && let Some(action) = scene_result_action(&control_event)
            {
                self.apply_result_action(action, false);
            }
            return;
        }

        // コース（段位）リザルト: Key5/Key7 はフェードアウト後の hold 状態で
        // retry arrange を決める。Key6 はゲージグラフ切替。
        if self.finished_course.is_some() {
            let pressed = event.state == ElementState::Pressed;
            if let Some(control) = physical_key_to_control(event.physical_key) {
                self.track_result_lane_hold(&control, pressed);
                if self.request_result_exit_skip_for_key(
                    event.physical_key,
                    event.state,
                    event.repeat,
                ) || self.request_result_exit_skip_for_control(&control, pressed, event.repeat)
                {
                    return;
                }
                if self.result_exit.is_none()
                    && self.handle_course_result_control(&control, pressed, event.repeat)
                {
                    return;
                }
            }
            if self.result_exit.is_none()
                && self.result_input_ready()
                && event.state == ElementState::Pressed
                && !event.repeat
                && let Some(slot) = digit_to_replay_slot(event.physical_key)
            {
                self.save_finished_course_replay_slot(slot);
                return;
            }
            if self.result_exit.is_none()
                && self.result_input_ready()
                && let Some(action) = scene_result_action(&control_event)
            {
                self.apply_result_action(action, true);
            }
            return;
        }

        if matches!(self.view_state(), AppViewState::Select)
            && event.physical_key == PhysicalKey::Code(KeyCode::F5)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            self.reload_from_select_context();
            return;
        }

        if matches!(self.view_state(), AppViewState::Select)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            match event.physical_key {
                PhysicalKey::Code(KeyCode::F3) => self.handle_select_f3_action(),
                PhysicalKey::Code(KeyCode::F10) => self.start_autoplay_folder_selected(),
                PhysicalKey::Code(KeyCode::F11) => self.open_primary_ir_for_selected(),
                PhysicalKey::Code(KeyCode::Numpad9) => self.open_selected_chart_documents(),
                _ => {}
            }
            if matches!(
                event.physical_key,
                PhysicalKey::Code(KeyCode::F3 | KeyCode::F10 | KeyCode::F11 | KeyCode::Numpad9)
            ) {
                return;
            }
        }

        if matches!(self.view_state(), AppViewState::Select)
            && event.state == ElementState::Released
            && let Some(control) = physical_key_name(event.physical_key)
        {
            self.update_select_e_action_hold(&control, false);
        }

        // 検索モード中はテキスト入力を最優先で処理し、通常ナビゲーションは抑制する。
        // モード入りトリガ (`/`) も同じ select 画面チェックの直後に処理する。
        if matches!(self.view_state(), AppViewState::Select)
            && !in_settings_stack(&self.folder_stack)
            && self.handle_search_key(event)
        {
            return;
        }

        // Select 画面で ESC 長押し → アプリ終了 (実際の exit は redraw 時にチェック)。
        if event.physical_key == PhysicalKey::Code(KeyCode::Escape) {
            if in_settings_stack(&self.folder_stack)
                && event.state == ElementState::Pressed
                && !event.repeat
            {
                if self.key_config_edit.is_some() {
                    self.cancel_key_config_edit();
                    return;
                }
                if self.settings_edit.is_some() {
                    self.cancel_settings_edit();
                    return;
                }
            }
            match event.state {
                ElementState::Pressed => {
                    if self.select_exit_hold_started_at.is_none() {
                        self.select_exit_hold_started_at = Some(Instant::now());
                    }
                }
                ElementState::Released => {
                    self.select_exit_hold_started_at = None;
                }
            }
            return;
        }

        if in_settings_stack(&self.folder_stack) {
            if event.state == ElementState::Released
                && let Some(control_name) = physical_key_name(event.physical_key)
            {
                self.clear_select_hold_control(&control_name);
                return;
            }
            if self.key_config_edit.is_some()
                && event.state == ElementState::Pressed
                && !event.repeat
            {
                if event.physical_key == PhysicalKey::Code(KeyCode::Delete)
                    || event.physical_key == PhysicalKey::Code(KeyCode::Backspace)
                {
                    self.clear_key_config_binding();
                    return;
                }
                if let Some(control) = physical_key_name(event.physical_key) {
                    if control == "Escape" {
                        self.cancel_key_config_edit();
                    } else if control == "Delete" || control == "Backspace" {
                        self.clear_key_config_binding();
                    } else {
                        self.apply_key_config_control(&control);
                    }
                }
                return;
            }
            if !should_route_settings_key_event(
                event.state,
                event.repeat,
                self.settings_edit.is_some(),
            ) {
                return;
            }
            if let Some(control) = physical_key_name(event.physical_key) {
                self.route_settings_control(&control);
            } else {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::ArrowUp) => {
                        let _ = self.route_settings_control("ArrowUp");
                    }
                    PhysicalKey::Code(KeyCode::ArrowDown) => {
                        let _ = self.route_settings_control("ArrowDown");
                    }
                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        let _ = self.route_settings_control("ArrowLeft");
                    }
                    PhysicalKey::Code(KeyCode::ArrowRight) => {
                        let _ = self.route_settings_control("ArrowRight");
                    }
                    PhysicalKey::Code(KeyCode::Enter) => {
                        let _ = self.route_settings_control("Enter");
                    }
                    PhysicalKey::Code(KeyCode::Space) => {
                        let _ = self.route_settings_control("Space");
                    }
                    PhysicalKey::Code(KeyCode::Escape) => {
                        let _ = self.route_settings_control("Escape");
                    }
                    _ => {}
                }
            }
            return;
        }

        if let Some(control) = physical_key_name(event.physical_key) {
            self.update_select_e_action_hold(&control, event.state == ElementState::Pressed);
        }

        if event.state == ElementState::Pressed
            && !event.repeat
            && self.select_option_panel == 0
            && self.select_ir_scope_toggle_is_e3()
            && let Some(control) = physical_key_name(event.physical_key)
            && self.is_select_ir_scope_toggle_control(&control)
            && self.toggle_select_ir_scope()
        {
            return;
        }

        if is_select_start_key(event.physical_key, &self.select_keys) {
            self.set_start_held(event.state == ElementState::Pressed);
            return;
        }

        if event.state == ElementState::Pressed
            && !event.repeat
            && let Some(control) = physical_key_name(event.physical_key)
            && should_toggle_select_judge_auto_adjust(
                &control,
                self.input.start_held,
                self.input.select_held,
                &self.select_keys,
            )
        {
            self.toggle_visual_offset_auto_adjust();
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            if is_select_modifier_key(event.physical_key, &self.select_keys) {
                self.set_select_held(true);
            }
            return;
        }

        if event.state == ElementState::Pressed
            && !event.repeat
            && let Some(control) = physical_key_name(event.physical_key)
            && should_toggle_select_gauge_auto_shift(
                &control,
                self.input.start_held,
                self.input.select_held,
                &self.select_keys,
            )
        {
            self.toggle_gauge_auto_shift();
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            if is_select_modifier_key(event.physical_key, &self.select_keys) {
                self.set_select_held(true);
            }
            return;
        }

        if is_select_modifier_key(event.physical_key, &self.select_keys) {
            self.set_select_held(event.state == ElementState::Pressed);
            return;
        }

        if self.select_option_panel != 0 {
            if event.state == ElementState::Pressed
                && (!event.repeat
                    || (self.select_option_panel == 3
                        && physical_key_name(event.physical_key).is_some_and(|control| {
                            green_number_delta_control(&control, &self.select_keys).is_some()
                        })))
            {
                match self.select_option_panel {
                    1 => {
                        if let Some(slot) = digit_to_replay_slot(event.physical_key) {
                            if !self.start_replay_for_selected(slot) {
                                tracing::info!(slot, "Start+digit pressed but no replay available");
                            }
                            return;
                        }
                        if let Some(cycle) = target_cycle_from_key(event.physical_key) {
                            self.apply_target_option_cycle(cycle);
                            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                            return;
                        }
                        if let Some(control) = physical_key_name(event.physical_key)
                            && let Some(cycle) =
                                target_cycle_from_control(&control, &self.select_keys)
                        {
                            self.apply_target_option_cycle(cycle);
                            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                            return;
                        }
                        if let Some(control) = physical_key_name(event.physical_key)
                            && self.apply_play_option_control(&control)
                        {
                            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                        }
                    }
                    3 => {
                        if let Some(control) = physical_key_name(event.physical_key)
                            && self.apply_detail_option_control(&control)
                        {
                            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                        }
                    }
                    _ => {}
                }
            }
            return;
        }

        if matches!(self.view_state(), AppViewState::Select) {
            if let Some(action) = scene_select_action(&control_event, &self.select_keys) {
                self.apply_select_action(action, control_event.name.as_deref());
            } else if event.state == ElementState::Released
                && let Some(control_name) = control_event.name.as_deref()
            {
                self.clear_select_hold_control(control_name);
            }
        }
    }

    pub(super) fn poll_gamepad_events(&mut self) {
        let should_log_raw_input = self.should_log_gamepad_key_config_raw_input();
        let Some(gamepad) = &mut self.gamepad else { return };
        let backend_name = gamepad.name();
        let output = gamepad.poll();
        if self.input.should_discard_gamepad_output(self.focused) {
            for event in output
                .buttons
                .iter()
                .filter(|event| should_route_gamepad_event_while_discarding(event.pressed))
            {
                self.route_gamepad_button_event(event);
            }
            self.reset_select_analog_scroll();
            self.reset_play_analog_scroll();
            return;
        }
        if should_log_raw_input {
            for event in &output.raw_events {
                log_gamepad_key_config_raw_event(backend_name, event);
            }
        }
        #[cfg(windows)]
        if let Some(diagnostics) = gamepad.gameinput_diagnostics()
            && diagnostics.reading_count > 0
        {
            tracing::trace!(
                reading_count = diagnostics.reading_count,
                oldest_reading_age_us = diagnostics.oldest_reading_age_us,
                "GameInput main-thread poll"
            );
        }
        for event in &output.buttons {
            self.route_gamepad_button_event(event);
        }
        for tick in &output.axis_ticks {
            // キーコンフィグ待ち受け中は合成 Press を待たず、生 tick から直接捕捉する。
            // 軸が active のままでも (押しっぱなし扱いで Press が出なくても) 確実に拾える。
            if self.key_config_edit.as_ref().is_some_and(|session| session.listening) {
                let control = format!("{}{}", tick.name, if tick.ticks > 0 { "+" } else { "-" });
                self.apply_key_config_gamepad(&control);
                continue;
            }
            self.route_gamepad_axis_ticks(&tick.name, tick.ticks);
        }
    }

    pub(super) fn route_gamepad_button_event(
        &mut self,
        event: &crate::input::gamepad::GamepadButtonEvent,
    ) {
        let mut device_event = crate::input::gamepad::to_device_input_event(event);
        if should_bypass_analog_scratch_bounce(
            event,
            self.play_option_input.as_ref().map(|input| &input.binding),
        ) {
            device_event.bounce_policy = InputBouncePolicy::Bypass;
        }
        let Some(device_event) = self.filter_app_input_bounce(device_event) else {
            return;
        };
        self.route_play_device_input(device_event);
        self.route_gamepad_button(event.device_id, &event.name, event.pressed);
    }

    pub(super) fn should_log_gamepad_key_config_raw_input(&self) -> bool {
        self.key_config_edit
            .as_ref()
            .is_some_and(|session| session.listening && session.target.slot().is_controller())
    }

    pub(super) fn route_gamepad_axis_ticks(&mut self, axis: &str, ticks: i32) {
        if self.apply_play_analog_option_ticks(axis, ticks) {
            return;
        }
        self.accumulate_select_analog_ticks(axis, ticks);
    }

    pub(super) fn apply_play_analog_option_ticks(&mut self, axis: &str, ticks: i32) -> bool {
        let Some(delta) = play_analog_lane_cover_delta(axis, ticks, &self.select_keys) else {
            return false;
        };
        let mode = match (self.play_e1_held, self.play_e2_held) {
            (true, false) => PlayAnalogOptionMode::LaneCover,
            (false, true) => PlayAnalogOptionMode::GreenNumber,
            _ => {
                self.reset_play_analog_scroll();
                return false;
            }
        };
        let lane_value_changing = self
            .active_play
            .as_ref()
            .is_some_and(|active_play| active_play.running.session.lane_cover_changing)
            || self
                .pending_play_start
                .as_ref()
                .is_some_and(|pending| pending.lane.lane_cover_changing);
        if !lane_value_changing {
            self.reset_play_analog_scroll();
            return false;
        }

        let now = Instant::now();
        let idle = self.play_analog_last_tick_at.is_none_or(|t| {
            now.duration_since(t) > Duration::from_millis(SELECT_ANALOG_SCROLL_TOLERANCE_MS)
        });
        self.play_analog_last_tick_at = Some(now);
        if idle {
            self.play_analog_scroll_buffer = 0;
        }
        self.play_analog_scroll_buffer += delta;

        let ticks_per_scroll = self.boot.profile_config.input.analog_ticks_per_scroll.max(1) as i32;
        let steps = take_analog_scroll_steps(&mut self.play_analog_scroll_buffer, ticks_per_scroll);
        if steps == 0 {
            return true;
        }

        let change = if steps > 0 { LaneCoverChange::Down } else { LaneCoverChange::Up };
        let action = match mode {
            PlayAnalogOptionMode::LaneCover => {
                PlayLaneAction::LaneCoverDelta(lane_cover_change_step(change) * steps.abs() as f32)
            }
            PlayAnalogOptionMode::GreenNumber => PlayLaneAction::GreenNumberDelta(
                green_number_change_step(green_number_change_from_analog_steps(steps))
                    * steps.abs(),
            ),
        };
        self.apply_play_lane_action(action);
        true
    }

    /// 選曲画面のアナログスクラッチ tick を蓄積する。回転量比例スクロール用。
    pub(super) fn accumulate_select_analog_ticks(&mut self, axis: &str, ticks: i32) {
        if !matches!(self.view_state(), AppViewState::Select)
            || self.active_play.is_some()
            || self.pending_decide.is_some()
            || self.pending_play_start.is_some()
            || self.key_config_edit.is_some()
            || (self.select_option_panel > 1 && self.settings_edit.is_none())
        {
            return;
        }
        let Some(delta) = select_analog_scroll_delta(axis, ticks, &self.select_keys) else {
            return;
        };
        let now = Instant::now();
        // tick が途切れていたら古い端数を捨てる (beatoraja の 200ms tolerance 相当)
        let idle = self.select_analog_last_tick_at.is_none_or(|t| {
            now.duration_since(t) > Duration::from_millis(SELECT_ANALOG_SCROLL_TOLERANCE_MS)
        });
        self.select_analog_last_tick_at = Some(now);
        update_analog_scroll_buffer(
            &mut self.select_analog_scroll_buffer,
            &mut self.select_analog_suppress_until_idle,
            idle,
            delta,
        );
    }

    /// キーコンフィグ確定/キャンセル後、回転中のスクラッチが止まるまで
    /// アナログスクロールを無効化する。
    pub(super) fn suppress_select_analog_until_idle(&mut self) {
        self.select_analog_suppress_until_idle = true;
        self.select_analog_scroll_buffer = 0;
        self.select_analog_last_tick_at = Some(Instant::now());
    }

    pub(super) fn reset_select_analog_scroll(&mut self) {
        self.select_analog_scroll_buffer = 0;
        self.select_analog_last_tick_at = None;
        self.select_analog_suppress_until_idle = false;
    }

    pub(super) fn reset_play_analog_scroll(&mut self) {
        self.play_analog_scroll_buffer = 0;
        self.play_analog_last_tick_at = None;
    }

    /// 蓄積したアナログ tick を analog_ticks_per_scroll ごとに 1 移動へ変換する。
    /// beatoraja MusicSelectInputProcessor の analogScrollBuffer と同じ仕組み。
    pub(super) fn advance_select_analog_scroll(&mut self) {
        if !self.focused {
            self.reset_select_analog_scroll();
            return;
        }
        if !matches!(self.view_state(), AppViewState::Select) {
            self.reset_select_analog_scroll();
            return;
        }
        if self.key_config_edit.is_some() {
            self.reset_select_analog_scroll();
            return;
        }
        let ticks_per_scroll = self.boot.profile_config.input.analog_ticks_per_scroll.max(1) as i32;
        let mov = take_analog_scroll_steps(&mut self.select_analog_scroll_buffer, ticks_per_scroll);
        if mov == 0 {
            return;
        }
        if self.settings_edit.is_some() {
            let direction = settings_edit_direction_from_analog_scroll(mov);
            for _ in 0..mov.abs() {
                self.adjust_settings_edit(direction);
            }
            return;
        }
        if self.select_option_panel > 1 {
            self.reset_select_analog_scroll();
            return;
        }
        if self.select_option_panel == 1 {
            let cycle = if mov > 0 { TargetCycle::Next } else { TargetCycle::Previous };
            for _ in 0..mov.abs() {
                self.apply_target_option_cycle(cycle);
            }
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
        } else {
            for _ in 0..mov.abs() {
                self.move_selection_with_duration(
                    if mov > 0 { SelectMove::Next } else { SelectMove::Previous },
                    select_analog_scroll_duration(mov),
                );
            }
        }
    }

    pub(super) fn route_gamepad_button(&mut self, device: DeviceId, button: &str, pressed: bool) {
        let control_event = ControlInputEvent::gamepad(device, button, pressed);
        self.input.track_control(&control_event);
        let physical_control =
            control_event.physical.as_ref().expect("gamepad control always has a physical value");
        let has_play_control_context =
            self.active_play.is_some() || self.pending_play_start.is_some();
        if pressed && self.handle_quick_retry_control(button) {
            return;
        }
        if pressed && self.begin_play_fadeout_after_final_notes_control(button) {
            return;
        }
        let play_e1_control = has_play_control_context
            && self.update_play_e1_control_state(device, physical_control, pressed);
        if has_play_control_context
            && self.update_play_exit_control_state(device, physical_control, pressed)
        {
            return;
        }
        let play_option_control = pressed.then(|| {
            play_option_control_for_input(
                device,
                physical_control,
                self.play_e1_held,
                self.play_e2_held,
                self.play_option_input.as_ref(),
                &self.boot.profile_config.input,
            )
        });
        let play_option_control = play_option_control.flatten();
        let play_option_lane_action = play_option_control
            .and_then(|action| lane_action_from_option(action, button.starts_with("Axis")));
        if pressed {
            let lane_cover_changing = self
                .active_play
                .as_ref()
                .is_some_and(|play| play.running.session.lane_cover_changing);
            if lane_cover_changing && play_option_control.is_some() {
                let Some(action) = play_option_lane_action else {
                    return;
                };
                self.apply_play_lane_action(action);
                // Gamepad play input was already queued in poll_gamepad_events.
            }
        }
        if !pressed {
            if in_settings_stack(&self.folder_stack) {
                self.clear_select_hold_control(button);
                return;
            }
            self.update_select_e_action_hold(button, false);
            if self.select_keys.is_start(button) {
                self.set_start_held(false);
            } else if self.select_keys.is_e2_action(button) || matches!(button, "Select") {
                self.set_select_held(false);
            }
            return;
        }

        self.update_select_e_action_hold(button, true);

        // プレイ中: Start / E1 の2回連続押しでレーンカバー表示切替。
        // プレイ入力自体は push_shared_event で処理済み。
        if self.active_play.is_some() {
            if play_e1_control {
                self.handle_play_start_double_press();
            }
            return;
        }

        if self.pending_decide.is_some() {
            if self.update_decide_cancel_control_state(button, pressed) {
                return;
            }
            if let Some(action) = scene_decide_action(&control_event, &self.select_keys) {
                self.begin_decide_fadeout(matches!(action, DecideAction::Cancel));
            }
            return;
        }

        if self.pending_play_start.is_some() {
            if play_option_control.is_some() {
                if let Some(action) = play_option_lane_action {
                    self.apply_play_lane_action(action);
                }
                return;
            }
            if play_e1_control {
                self.handle_play_start_double_press();
            }
            return;
        }

        // コース曲間の中間リザルト: リトライ無効、次の曲へ進むだけ。
        // retry を持つ単曲リザルト分岐より先に評価する。
        if self.is_course_intermediate_result() {
            let control = PhysicalControl::GamepadButton(button.to_string());
            if self.request_result_exit_skip_for_control(&control, pressed, false) {
                return;
            }
            if self.result_exit.is_none() {
                if self.handle_course_intermediate_control(&control, pressed, false) {
                    return;
                }
                if self.result_input_ready() && scene_result_action(&control_event).is_some() {
                    self.begin_result_exit(self.course_intermediate_exit_action());
                }
            }
            return;
        }

        // リザルト画面
        if self.finished_play.is_some() && self.finished_course.is_none() {
            let control = PhysicalControl::GamepadButton(button.to_string());
            // フェードアウト中でも Key5/Key7 の押下状態は追跡する。
            self.track_result_lane_hold(&control, pressed);
            if self.request_result_exit_skip_for_control(&control, pressed, false) {
                return;
            }
            // 終了アニメーション中 (result_exit=Some) は held 追跡のみ行う。
            if self.result_exit.is_none() {
                if self.handle_result_control(&control, pressed, false) {
                    return;
                }
                if self.result_input_ready()
                    && let Some(action) = scene_result_action(&control_event)
                {
                    self.apply_result_action(action, false);
                }
            }
            return;
        }

        // コース（段位）リザルト: Key5/Key7 はフェードアウト後の hold 状態で
        // retry arrange を決める。Button1/Start は同配置リトライ。
        if self.finished_course.is_some() {
            let control = PhysicalControl::GamepadButton(button.to_string());
            self.track_result_lane_hold(&control, pressed);
            if self.request_result_exit_skip_for_control(&control, pressed, false) {
                return;
            }
            if self.result_exit.is_none() {
                if self.handle_course_result_control(&control, pressed, false) {
                    return;
                }
                if self.result_input_ready()
                    && let Some(action) = scene_result_action(&control_event)
                {
                    self.apply_result_action(action, true);
                }
            }
            return;
        }

        if in_settings_stack(&self.folder_stack) {
            if self.key_config_edit.as_ref().is_some_and(|session| session.listening) {
                if pressed {
                    self.apply_key_config_gamepad(button);
                }
                return;
            }
            if pressed {
                let _ = self.route_settings_control(button);
            }
            return;
        }

        if self.select_option_panel == 0
            && self.select_ir_scope_toggle_is_e3()
            && self.is_select_ir_scope_toggle_control(button)
            && self.toggle_select_ir_scope()
        {
            return;
        }

        if should_toggle_select_gauge_auto_shift(
            button,
            self.input.start_held,
            self.input.select_held,
            &self.select_keys,
        ) {
            self.toggle_gauge_auto_shift();
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            if self.select_keys.is_e2_action(button) {
                self.set_select_held(true);
            }
            return;
        }

        if should_toggle_select_judge_auto_adjust(
            button,
            self.input.start_held,
            self.input.select_held,
            &self.select_keys,
        ) {
            self.toggle_visual_offset_auto_adjust();
            self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            if self.select_keys.is_e2_action(button) {
                self.set_select_held(true);
            }
            return;
        }

        if self.select_keys.is_start(button) {
            self.set_start_held(true);
            return;
        }

        if self.select_keys.is_e2_action(button) || matches!(button, "Select") {
            self.set_select_held(true);
            return;
        }

        if self.select_option_panel != 0 {
            if self.select_option_panel == 1
                && let Some(cycle) = target_cycle_from_control(button, &self.select_keys)
            {
                if button.starts_with("Axis") {
                    return;
                }
                self.apply_target_option_cycle(cycle);
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                return;
            }
            let option_changed = match self.select_option_panel {
                1 => self.apply_gamepad_play_option_control(device, button),
                3 => self.apply_detail_option_control(button),
                _ => false,
            };
            if option_changed {
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            return;
        }

        if matches!(self.view_state(), AppViewState::Select) {
            // アナログ軸にバインドされたスクラッチは tick 比例スクロール
            // (advance_select_analog_scroll) で処理する。beatoraja の isNonAnalogPressed 相当。
            if button.starts_with("Axis")
                && (self.select_keys.is_select_scratch_up(button)
                    || self.select_keys.is_select_scratch_down(button))
            {
                return;
            }
            if let Some(action) = scene_select_action(&control_event, &self.select_keys) {
                self.apply_select_action(action, Some(button));
            }
        }
    }

    pub(super) fn route_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        if let Some(change) = lane_cover_wheel_change(delta)
            && (self.active_play.is_some() || self.pending_play_start.is_some())
        {
            self.apply_play_lane_action(PlayLaneAction::LaneCoverDelta(lane_cover_change_step(
                change,
            )));
            return;
        }
        if !matches!(self.view_state(), AppViewState::Select) {
            return;
        }
        if in_settings_stack(&self.folder_stack) && self.settings_edit.is_some() {
            let direction = settings_edit_direction_from_mouse_wheel(delta);
            if direction != 0 {
                self.adjust_settings_edit(direction);
            }
            return;
        }
        if let Some(select_move) = select_wheel_move(delta) {
            self.move_selection(select_move);
        }
    }

    pub(super) fn route_mouse_input(&mut self, state: ElementState, button: MouseButton) {
        if state == ElementState::Released {
            self.select_slider_dragging_type = None;
            return;
        }
        if state != ElementState::Pressed {
            return;
        }
        let Some((x, y)) = self.cursor_position_normalized() else {
            return;
        };
        if matches!(self.view_state(), AppViewState::Result) {
            self.select_slider_dragging_type = None;
            if button == MouseButton::Left && self.result_exit.is_none() {
                let AppSceneSnapshot::Result(snapshot) = self.scene_snapshot() else {
                    return;
                };
                if let Some(hit) = self.renderer.result_skin_slider_hit(&snapshot, x, y) {
                    self.select_slider_dragging_type = Some(hit.slider_type);
                    self.apply_result_slider_hit(hit);
                    return;
                }
                self.handle_result_skin_click(x, y);
            }
            return;
        }
        if !matches!(self.view_state(), AppViewState::Select) {
            self.select_slider_dragging_type = None;
            return;
        }
        if button == MouseButton::Left
            && !in_settings_stack(&self.folder_stack)
            && self.select_search_word_hit(x, y)
        {
            if !self.search.is_active() {
                self.set_search_mode(true);
                tracing::info!("entered song search mode from mouse click");
            } else {
                self.search_cursor_to_end();
            }
            return;
        }
        let snapshot = self.select_snapshot();
        if button == MouseButton::Left
            && let Some(hit) = self.renderer.select_skin_slider_hit(&snapshot, x, y)
        {
            self.select_slider_dragging_type = Some(hit.slider_type);
            self.apply_select_slider_hit(hit);
            return;
        }
        let Some(hit) = self.renderer.select_skin_click_hit(&snapshot, x, y) else {
            return;
        };
        self.handle_select_skin_click(hit, button, x, y);
    }

    pub(super) fn handle_result_skin_click(&mut self, x: f32, y: f32) {
        let AppSceneSnapshot::Result(snapshot) = self.scene_snapshot() else {
            return;
        };
        let Some(hit) = self.renderer.result_skin_click_hit(&snapshot, x, y) else {
            return;
        };
        let SkinClickTarget::Event { event_id, .. } = hit.target else {
            return;
        };
        match result_skin_click_action(event_id) {
            Some(ResultSkinClickAction::SetPanel(panel)) => {
                self.set_result_panel(panel);
            }
            Some(ResultSkinClickAction::SelectIrScope(tab)) => {
                self.select_result_ir_scope(tab);
            }
            Some(ResultSkinClickAction::ToggleIrScope) => {
                self.toggle_result_ir_scope();
            }
            Some(ResultSkinClickAction::ToggleFavoriteChart) => {
                self.toggle_favorite_chart_result();
            }
            Some(ResultSkinClickAction::SaveReplay(slot)) => {
                if self.finished_course.is_some() {
                    self.save_finished_course_replay_slot(slot);
                } else {
                    self.save_finished_play_replay_slot(slot);
                }
            }
            Some(ResultSkinClickAction::ResetDailyStatistics) => {
                self.reset_daily_statistics();
            }
            None => {
                let _ = self.renderer.dispatch_result_skin_runtime_event(event_id);
            }
        }
    }

    pub(super) fn route_select_slider_drag(&mut self) {
        if self.select_slider_dragging_type.is_none() {
            return;
        }
        let Some((x, y)) = self.cursor_position_normalized() else {
            return;
        };
        if matches!(self.view_state(), AppViewState::Result) {
            if self.result_exit.is_some() {
                return;
            }
            let AppSceneSnapshot::Result(snapshot) = self.scene_snapshot() else {
                return;
            };
            if let Some(hit) = self.renderer.result_skin_slider_hit(&snapshot, x, y) {
                self.apply_result_slider_hit(hit);
            }
            return;
        }
        if !matches!(self.view_state(), AppViewState::Select) {
            return;
        }
        let snapshot = self.select_snapshot();
        if let Some(hit) = self.renderer.select_skin_slider_hit(&snapshot, x, y) {
            self.apply_select_slider_hit(hit);
        }
    }

    pub(super) fn cursor_position_normalized(&self) -> Option<(f32, f32)> {
        let window = self.window.as_ref()?;
        let position = self.last_cursor_position?;
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return None;
        }
        Some((
            (position.x as f32 / size.width as f32).clamp(0.0, 1.0),
            (position.y as f32 / size.height as f32).clamp(0.0, 1.0),
        ))
    }

    pub(super) fn select_search_word_hit(&self, x: f32, y: f32) -> bool {
        let Some(document) = self.renderer.select_skin_document() else {
            return false;
        };
        let Some((rect_x, rect_y, rect_w, rect_h)) = document.text_destination_rect_for_ref(30)
        else {
            return false;
        };
        x >= rect_x && x <= rect_x + rect_w && y >= rect_y && y <= rect_y + rect_h
    }

    pub(super) fn search_cursor_to_end(&mut self) {
        self.search.cursor_to_end();
        self.update_search_ime_cursor_area();
    }

    pub(super) fn apply_select_slider_hit(&mut self, hit: SkinSliderHit) {
        match hit.slider_type {
            1 => self.apply_select_scroll_slider(hit.value),
            17..=19 => {
                let value = volume_f32_to_unit(hit.value);
                let mix = &mut self.boot.profile_config.audio_mix;
                match hit.slider_type {
                    17 if mix.master_volume != value => {
                        mix.master_volume = value;
                        self.sync_realtime_profile_settings();
                        tracing::info!(value, "select skin master volume changed");
                    }
                    18 if mix.key_volume != value => {
                        mix.key_volume = value;
                        self.sync_realtime_profile_settings();
                        tracing::info!(value, "select skin key volume changed");
                    }
                    19 if mix.bgm_volume != value => {
                        mix.bgm_volume = value;
                        self.sync_realtime_profile_settings();
                        tracing::info!(value, "select skin bgm volume changed");
                    }
                    _ => {}
                }
            }
            _ => {
                tracing::debug!(slider_type = hit.slider_type, "unsupported select skin slider");
            }
        }
    }

    pub(super) fn apply_result_slider_hit(&mut self, hit: SkinSliderHit) {
        if hit.slider_type == 8 {
            if let Some(result_ir) = &mut self.result_ir {
                result_ir.set_skin_scroll_rate(hit.value);
            }
        } else {
            tracing::debug!(slider_type = hit.slider_type, "unsupported result skin slider");
        }
    }

    pub(super) fn apply_select_scroll_slider(&mut self, value: f32) {
        let Some(next) = select_scroll_slider_index(value, self.select_items.len()) else {
            return;
        };
        if self.selected_index != next {
            self.selected_index = next;
            self.restart_select_bar_timer_without_scroll(Instant::now());
            self.play_system_sound(crate::system_sound::SoundType::Scratch);
        }
    }

    pub(super) fn handle_select_skin_click(
        &mut self,
        hit: SkinClickHit,
        button: MouseButton,
        x: f32,
        y: f32,
    ) {
        match hit.target {
            SkinClickTarget::SelectRow { row_index } => {
                self.handle_select_row_click(row_index, button);
            }
            SkinClickTarget::Event { event_id, click } => {
                let Some(arg) = select_click_event_arg(click, button, hit.rect, x, y) else {
                    return;
                };
                self.execute_select_skin_event(event_id, arg);
            }
        }
    }

    pub(super) fn handle_select_row_click(&mut self, row_index: u32, button: MouseButton) {
        if in_settings_stack(&self.folder_stack) && button == MouseButton::Left {
            if self.settings_edit.is_some() {
                self.commit_settings_edit();
                return;
            }
            if let Some(entry_id) =
                self.select_items.get(row_index as usize).and_then(|item| match item {
                    SelectItem::Config(row) => Some(row.entry_id),
                    _ => None,
                })
            {
                self.selected_index = row_index as usize;
                self.restart_select_bar_timer_without_scroll(Instant::now());
                self.begin_settings_edit(entry_id);
                return;
            }
        }
        match select_row_click_action(
            row_index,
            button,
            self.selected_index,
            self.select_items.len(),
            self.settings_edit.is_some(),
        ) {
            Some(SelectRowClickAction::Select(next)) => {
                self.selected_index = next;
                self.restart_select_bar_timer_without_scroll(Instant::now());
                self.play_system_sound(crate::system_sound::SoundType::Scratch);
            }
            Some(SelectRowClickAction::EnterOrPlay) => self.enter_or_play_selected(),
            Some(SelectRowClickAction::CancelSettingsEdit) => self.cancel_settings_edit(),
            Some(SelectRowClickAction::ExitFolder) => self.exit_folder(),
            None => {}
        }
    }

    pub(super) fn execute_select_skin_event(&mut self, event_id: i32, arg: i32) {
        match event_id {
            SKIN_EVENT_IR_SCOPE_GLOBAL => {
                self.select_select_ir_scope(
                    crate::screens::select_ir::SelectIrRankingScope::Global,
                );
            }
            SKIN_EVENT_IR_SCOPE_RIVAL => {
                self.select_select_ir_scope(
                    crate::screens::select_ir::SelectIrRankingScope::SelfAndRivals,
                );
            }
            SKIN_EVENT_IR_SCOPE_TOGGLE => {
                self.toggle_select_ir_scope();
            }
            SKIN_EVENT_DAILY_STATISTICS_RESET => self.reset_daily_statistics(),
            // beatoraja EventFactory: play / autoplay / practice.
            15 => {
                self.set_session_mode(SessionMode::Normal);
                self.enter_or_play_selected();
            }
            16 => {
                self.set_session_mode(SessionMode::Autoplay);
                self.enter_or_play_selected();
            }
            315 => {
                if let Some(chart_id) = self.currently_selected_chart_id() {
                    self.enter_practice(chart_id, PracticeCliOverrides::default());
                }
            }
            19 | 316 | 317 | 318 => {
                let slot = match event_id {
                    19 => 0,
                    316 => 1,
                    317 => 2,
                    318 => 3,
                    _ => unreachable!(),
                };
                if !self.start_replay_for_selected(slot) {
                    tracing::info!(slot, "select skin replay click ignored; slot is empty");
                }
            }
            11 => self.cycle_select_mode_filter(arg),
            12 => self.cycle_select_sort(arg),
            40 => self.cycle_select_gauge(arg),
            42 => self.cycle_select_arrange(arg),
            43 => self.cycle_select_arrange_2p(arg),
            54 => self.cycle_select_double_option(arg),
            55 => self.cycle_select_hs_fix(arg),
            72 => self.cycle_select_bga(arg),
            73 => self.cycle_select_bga_expand(arg),
            75 => {
                self.toggle_visual_offset_auto_adjust();
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            77 => self.cycle_select_target(arg),
            78 => self.cycle_select_gauge_auto_shift(arg),
            89 => self.toggle_favorite_song_selected(),
            90 => self.toggle_favorite_chart_selected(),
            341 => self.cycle_select_bottom_shiftable_gauge(arg),
            340 => self.cycle_select_judge_algorithm(arg),
            308 => self.cycle_select_ln_mode(arg),
            312 => {
                // BMZ only exposes beatoraja's default sorter set for now.
                self.cycle_select_sort(arg);
            }
            321..=324 => self.cycle_replay_slot_rule(event_id, arg),
            330 => {
                self.boot.profile_config.play.lane_effect =
                    toggled_select_sudden(self.boot.profile_config.play.lane_effect);
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            331 => {
                self.boot.profile_config.lane.lift_enabled =
                    !self.boot.profile_config.lane.lift_enabled;
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            332 => {
                self.boot.profile_config.play.lane_effect =
                    toggled_select_hidden(self.boot.profile_config.play.lane_effect);
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            342 => {
                self.boot.profile_config.lane.hispeed_auto_adjust =
                    !self.boot.profile_config.lane.hispeed_auto_adjust;
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            _ => {
                tracing::debug!(event_id, arg, "unsupported select skin event");
            }
        }
    }

    pub(super) fn reset_daily_statistics(&mut self) {
        match self.boot.score_db.reset_daily_statistics(now_unix_seconds()) {
            Ok(()) => {
                self.refresh_player_stats_snapshot();
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
            }
            Err(error) => tracing::warn!(%error, "failed to reset daily statistics"),
        }
    }

    pub(super) fn cycle_select_mode_filter(&mut self, arg: i32) {
        self.select_mode_filter = if arg >= 0 {
            self.select_mode_filter.next()
        } else {
            self.select_mode_filter.previous()
        };
        // reload_select_items 内で beatoraja 準拠の自動送りと profile config への
        // 永続化（退出 / プレイ後の save_current_play_options 用）を行う。
        let previous_len = self.select_items.len();
        self.reload_select_items();
        tracing::info!(
            mode = self.select_mode_filter.as_str(),
            previous_len,
            current_len = self.select_items.len(),
            "select mode filter changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_gauge(&mut self, arg: i32) {
        self.gauge_option = cycle_gauge_option_with_direction(self.gauge_option, arg);
        tracing::info!(gauge = ?self.gauge_option, "gauge option changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_arrange(&mut self, arg: i32) {
        self.arrange_option = cycle_arrange_option_with_direction(self.arrange_option, arg);
        tracing::info!(arrange = self.arrange_option.as_str(), "arrange option changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_arrange_2p(&mut self, arg: i32) {
        self.arrange_option_2p = cycle_arrange_option_with_direction(self.arrange_option_2p, arg);
        tracing::info!(arrange_2p = self.arrange_option_2p.as_str(), "2P arrange changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_double_option(&mut self, arg: i32) {
        self.double_option = cycle_double_option_with_direction(self.double_option, arg);
        tracing::info!(double_option = self.double_option.as_str(), "double option changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_hs_fix(&mut self, arg: i32) {
        self.hs_fix_option = cycle_hs_fix_option_with_direction(self.hs_fix_option, arg);
        tracing::info!(hs_fix = self.hs_fix_option.as_str(), "HS-FIX option changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_bga(&mut self, arg: i32) {
        self.boot.profile_config.play.bga =
            cycle_bga_option_with_direction(self.boot.profile_config.play.bga, arg);
        tracing::info!(
            bga = bga_mode_as_str(self.boot.profile_config.play.bga),
            "bga option changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_bga_expand(&mut self, arg: i32) {
        self.boot.profile_config.play.bga_expand =
            cycle_bga_expand_with_direction(self.boot.profile_config.play.bga_expand, arg);
        tracing::info!(
            bga_expand = ?self.boot.profile_config.play.bga_expand,
            "bga expand changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_target(&mut self, arg: i32) {
        let cycle = if arg >= 0 { TargetCycle::Next } else { TargetCycle::Previous };
        self.apply_target_option_cycle(cycle);
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_gauge_auto_shift(&mut self, arg: i32) {
        self.gauge_auto_shift_option =
            cycle_gauge_auto_shift_option_with_direction(self.gauge_auto_shift_option, arg);
        tracing::info!(
            gauge_auto_shift = gauge_auto_shift_as_str(self.gauge_auto_shift_option),
            "gauge auto shift changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_bottom_shiftable_gauge(&mut self, arg: i32) {
        self.bottom_shiftable_gauge_option =
            cycle_bottom_shiftable_gauge_with_direction(self.bottom_shiftable_gauge_option, arg);
        tracing::info!(
            bottom_shiftable_gauge =
                bottom_shiftable_gauge_as_str(self.bottom_shiftable_gauge_option),
            "bottom shiftable gauge changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_judge_algorithm(&mut self, arg: i32) {
        self.boot.profile_config.judge.judge_algorithm = cycle_judge_algorithm_with_direction(
            self.boot.profile_config.judge.judge_algorithm,
            arg,
        );
        self.boot.profile_config.updated_at = now_unix_seconds();
        self.sync_realtime_profile_settings();
        self.invalidate_play_preload();
        tracing::info!(
            judge_algorithm = self.boot.profile_config.judge.judge_algorithm.beatoraja_name(),
            "judge algorithm changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_sort(&mut self, arg: i32) {
        self.select_sort =
            if arg >= 0 { self.select_sort.next() } else { self.select_sort.previous() };
        // 退出 / プレイ後の save_current_play_options で永続化されるよう、
        // profile config をメモリ上で先に更新しておく。
        self.boot.profile_config.select.sort = self.select_sort.as_str().to_string();
        self.reload_select_items();
        tracing::info!(sort = self.select_sort.as_str(), "select sort changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_select_ln_mode(&mut self, arg: i32) {
        let score_context_before = SelectScoreContext::from_profile(&self.boot.profile_config);
        self.boot.profile_config.play.ln_mode_policy = if arg >= 0 {
            self.boot.profile_config.play.ln_mode_policy.next()
        } else {
            self.boot.profile_config.play.ln_mode_policy.previous()
        };
        self.sync_changed_select_score_context(score_context_before);
        tracing::info!(
            ln_mode = self.boot.profile_config.play.ln_mode_policy.display_label(),
            "select LN mode policy changed"
        );
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn cycle_replay_slot_rule(&mut self, event_id: i32, arg: i32) {
        let slot = (event_id - 321) as usize;
        if slot >= 4 {
            return;
        }
        let rule = &mut self.boot.profile_config.replay.slot_rules[slot];
        let next = rule.cycle(arg >= 0);
        if next == *rule {
            return;
        }
        *rule = next;
        tracing::info!(slot, ?next, "select replay autosave rule changed");
        self.play_system_sound(crate::system_sound::SoundType::OptionChange);
    }

    pub(super) fn move_selection(&mut self, select_move: SelectMove) {
        self.move_selection_with_duration(select_move, self.select_scroll_duration_low());
    }

    pub(super) fn move_selection_with_duration(
        &mut self,
        select_move: SelectMove,
        duration: Duration,
    ) {
        if self.select_items.is_empty() {
            self.reload_select_items();
        }
        if self.select_items.is_empty() {
            return;
        }
        let previous_index = self.selected_index;
        self.selected_index =
            moved_select_index(self.selected_index, self.select_items.len(), select_move);
        if self.selected_index != previous_index {
            self.select_bar_started_at = Instant::now();
            self.select_bar_scroll_direction = select_move_scroll_direction(select_move);
            self.select_bar_scroll_duration = duration;
            self.play_system_sound(crate::system_sound::SoundType::Scratch);
        }
    }

    pub(super) fn advance_select_hold_move(&mut self) {
        if !self.focused {
            self.clear_select_hold();
            return;
        }
        if !matches!(self.view_state(), AppViewState::Select) {
            self.clear_select_hold();
            return;
        }
        let (Some(select_move), Some(started_at), Some(last_trigger_at)) =
            (self.select_hold_move, self.select_hold_started_at, self.select_hold_last_trigger_at)
        else {
            return;
        };
        let now = Instant::now();
        let elapsed = now.duration_since(started_at);
        if elapsed < self.select_scroll_duration_low() {
            return;
        }
        let since_last = now.duration_since(last_trigger_at);
        if since_last >= self.select_scroll_duration_high() {
            self.select_hold_last_trigger_at = Some(now);
            self.move_selection_with_duration(select_move, self.select_scroll_duration_high());
        }
    }

    pub(super) fn start_select_hold_move(&mut self, select_move: SelectMove, control: String) {
        self.select_hold_move = Some(select_move);
        self.select_hold_started_at = Some(Instant::now());
        self.select_hold_last_trigger_at = Some(Instant::now());
        self.select_hold_control = Some(control);
    }

    pub(super) fn clear_select_hold_control(&mut self, control: &str) {
        if self.select_hold_control.as_deref() == Some(control) {
            self.clear_select_hold();
        }
    }

    pub(super) fn clear_select_hold(&mut self) {
        self.select_hold_move = None;
        self.select_hold_started_at = None;
        self.select_hold_last_trigger_at = None;
        self.select_hold_control = None;
    }

    pub(super) fn open_advanced_settings_from_select(&mut self) {
        if let Some(egui) = self.egui.as_mut() {
            egui.open_advanced_settings();
        }
        self.play_system_sound(crate::system_sound::SoundType::FolderOpen);
        tracing::info!("opened egui advanced settings from select");
    }

    pub(super) fn selected_chart_row(
        &self,
    ) -> Option<&crate::screens::select_model::SelectChartRow> {
        match self.select_items.get(self.selected_index) {
            Some(SelectItem::Chart(row)) => Some(row),
            _ => None,
        }
    }

    pub(super) fn toggle_favorite_chart_selected(&mut self) {
        let Some(row) = self.selected_chart_row().cloned() else {
            return;
        };
        let Some(sha256) = row.score_sha256() else {
            return;
        };
        let hints = favorite_hints_for_row(&row);
        match self.boot.collection_db.toggle_favorite_chart(sha256, &hints, now_unix_seconds()) {
            Ok(enabled) => {
                self.reload_select_items();
                self.restart_select_bar_timer_without_scroll(Instant::now());
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                let text = Localizer::new(self.boot.profile_config.ui.locale());
                self.show_left_overlay_toast(text.text(if enabled {
                    "toast-favorite-chart-added"
                } else {
                    "toast-favorite-chart-removed"
                }));
                tracing::info!(enabled, title = row.display_title(), "favorite chart toggled");
            }
            Err(error) => tracing::error!(%error, "failed to toggle favorite chart"),
        }
    }

    pub(super) fn toggle_favorite_chart_result(&mut self) {
        let Some((sha256, title, artist)) = self.finished_play.as_ref().map(|finished| {
            (
                finished.result.chart_sha256,
                finished.summary.title.clone(),
                finished.summary.artist.clone(),
            )
        }) else {
            return;
        };
        let hints = FavoriteHints::new(title.clone(), artist, "");
        match self.boot.collection_db.toggle_favorite_chart(sha256, &hints, now_unix_seconds()) {
            Ok(enabled) => {
                self.result_favorite_chart = enabled;
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                let text = Localizer::new(self.boot.profile_config.ui.locale());
                self.show_left_overlay_toast(text.text(if enabled {
                    "toast-favorite-chart-added"
                } else {
                    "toast-favorite-chart-removed"
                }));
                tracing::info!(enabled, %title, "favorite chart toggled from result");
            }
            Err(error) => tracing::error!(%error, "failed to toggle favorite chart from result"),
        }
    }

    pub(super) fn handle_select_f3_action(&mut self) {
        let e1_held = self.input.select_e_action_holds.contains(&InputActionConfig::E1);
        let e2_held = self.input.select_e_action_holds.contains(&InputActionConfig::E2);
        let ctrl_held = self.input.pressed_controls.iter().any(|control| {
            matches!(control.as_str(), "LControl" | "RControl" | "ControlLeft" | "ControlRight")
        });
        let shift_held = self.input.pressed_controls.iter().any(|control| {
            matches!(control.as_str(), "LShift" | "RShift" | "ShiftLeft" | "ShiftRight")
        });

        if e1_held {
            self.copy_selected_hash(false);
        } else if e2_held || (ctrl_held && shift_held) {
            self.copy_selected_hash(true);
        } else if ctrl_held {
            self.copy_selected_hash(false);
        } else {
            self.open_selected_chart_folder();
        }
    }

    pub(super) fn copy_selected_hash(&mut self, sha256: bool) {
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let Some(row) = self.selected_chart_row().cloned() else {
            return;
        };
        let Some(value) = (if sha256 {
            row.score_sha256().map(|hash| hash_to_hex(&hash))
        } else {
            row.chart.as_ref().map(|chart| hash_to_hex(&chart.md5))
        }) else {
            self.show_left_overlay_toast(text.text(if sha256 {
                "toast-chart-hash-unavailable-sha256"
            } else {
                "toast-chart-hash-md5-local-only"
            }));
            return;
        };
        match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(value.clone()))
        {
            Ok(()) => {
                self.show_left_overlay_toast(text.text(if sha256 {
                    "toast-chart-hash-copied-sha256"
                } else {
                    "toast-chart-hash-copied-md5"
                }));
                tracing::info!(sha256, hash = %value, "copied chart hash to clipboard");
            }
            Err(error) => {
                tracing::warn!(%error, sha256, "failed to copy chart hash to clipboard");
                self.show_left_overlay_toast(text.text("toast-clipboard-copy-failed"));
            }
        }
    }

    pub(super) fn open_selected_chart_folder(&mut self) {
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let Some(chart) = self.selected_chart_row().and_then(|row| row.chart.clone()) else {
            return;
        };
        let folder = PathBuf::from(&chart.folder_path);
        if let Err(error) = open_file_browser_path(&folder) {
            tracing::warn!(path = %folder.display(), %error, "failed to open selected chart folder");
            self.show_left_overlay_toast(text.text("toast-chart-folder-open-failed"));
        } else {
            tracing::info!(path = %folder.display(), "opened selected chart folder");
        }
    }

    pub(super) fn open_selected_chart_documents(&mut self) {
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let Some(chart) = self.selected_chart_row().and_then(|row| row.chart.clone()) else {
            return;
        };
        let folder = PathBuf::from(&chart.folder_path);
        let mut opened = 0usize;
        match std::fs::read_dir(&folder) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let is_text = path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"));
                    if is_text && open_file_with_default_app(&path).is_ok() {
                        opened += 1;
                    }
                }
            }
            Err(error) => {
                tracing::warn!(path = %folder.display(), %error, "failed to read chart documents");
            }
        }
        if opened == 0 {
            self.show_left_overlay_toast(text.text("toast-chart-text-not-found"));
        } else {
            let mut args = FluentArgs::new();
            args.set("count", opened as i64);
            self.show_left_overlay_toast(text.format("toast-chart-text-opened", &args));
        }
    }

    pub(super) fn open_primary_ir_for_selected(&mut self) {
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let Some(row) = self.selected_chart_row() else {
            return;
        };
        let Some(sha256) = row.score_sha256() else {
            self.show_left_overlay_toast(text.text("toast-ir-chart-hash-missing"));
            return;
        };
        let Some(provider) = primary_ir_provider_for_profile(&self.boot.profile_config) else {
            self.show_left_overlay_toast(text.text("toast-primary-ir-not-configured"));
            return;
        };
        let url =
            format!("{}/charts/{}", provider.base_url.trim_end_matches('/'), hash_to_hex(&sha256));
        match open_external_url(&url) {
            Ok(()) => {
                self.show_left_overlay_toast(text.text("toast-primary-ir-opened"));
                tracing::info!(%url, "opened primary IR chart page");
            }
            Err(error) => {
                tracing::warn!(%error, %url, "failed to open primary IR chart page");
                self.show_left_overlay_toast(text.text("toast-primary-ir-open-failed"));
            }
        }
    }

    pub(super) fn start_autoplay_folder_selected(&mut self) {
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let Some((path, kind)) =
            self.select_items.get(self.selected_index).and_then(|item| match item {
                SelectItem::Folder { path, kind, .. } => Some((path.clone(), *kind)),
                _ => None,
            })
        else {
            return;
        };
        if kind != bmz_render::scene::SelectRowKind::Folder {
            self.show_left_overlay_toast(text.text("toast-folder-autoplay-only-normal-folder"));
            return;
        }
        let mut folder_paths = vec![path.clone()];
        match self.boot.library_db.list_descendant_folder_paths(&path) {
            Ok(descendants) => folder_paths.extend(descendants),
            Err(error) => {
                tracing::warn!(folder = %path, %error, "failed to list autoplay folder descendants");
            }
        }
        let folder_refs: Vec<&str> = folder_paths.iter().map(String::as_str).collect();
        let charts = match self.boot.library_db.list_charts_in_folders(&folder_refs) {
            Ok(charts) => charts,
            Err(error) => {
                tracing::warn!(folder = %path, %error, "failed to list autoplay folder charts");
                self.show_left_overlay_toast(text.text("toast-folder-autoplay-charts-load-failed"));
                return;
            }
        };
        let mut chart_ids = Vec::with_capacity(charts.len());
        let mut seen = HashSet::new();
        for chart in charts {
            if seen.insert(chart.chart_id) {
                chart_ids.push(chart.chart_id);
            }
        }
        let Some(&first_chart_id) = chart_ids.first() else {
            self.show_left_overlay_toast(text.text("toast-folder-autoplay-empty"));
            return;
        };
        self.clear_active_course_state();
        self.autoplay_folder = Some(AutoplayFolderSession { chart_ids, next_index: 1 });
        let mut options = self.play_start_options();
        options.session_mode = SessionMode::Autoplay;
        options.autoplay = true;
        self.begin_decide_for_chart(first_chart_id, options);
        self.show_left_overlay_toast(text.text("toast-folder-autoplay-started"));
        tracing::info!(folder = %path, first_chart_id, "started folder autoplay");
    }

    pub(super) fn toggle_favorite_song_selected(&mut self) {
        let Some(row) = self.selected_chart_row().cloned() else {
            return;
        };
        let Some(sha256) = row.score_sha256() else {
            return;
        };
        let representatives = match row.chart.as_ref() {
            Some(chart) => favorite_song_representatives_for_folder(
                &self.boot.library_db,
                &self.boot.collection_db,
                &chart.folder_path,
            )
            .unwrap_or_else(|error| {
                tracing::error!(%error, "failed to resolve favorite song folders");
                Vec::new()
            }),
            None => Vec::new(),
        };
        let hints = favorite_hints_for_row(&row);
        let result = if representatives.is_empty() {
            self.boot.collection_db.toggle_favorite_song(sha256, &hints, now_unix_seconds())
        } else {
            self.boot.collection_db.remove_favorite_songs(&representatives).map(|_| false)
        };
        match result {
            Ok(enabled) => {
                self.reload_select_items();
                self.restart_select_bar_timer_without_scroll(Instant::now());
                self.play_system_sound(crate::system_sound::SoundType::OptionChange);
                let text = Localizer::new(self.boot.profile_config.ui.locale());
                self.show_left_overlay_toast(text.text(if enabled {
                    "toast-favorite-song-added"
                } else {
                    "toast-favorite-song-removed"
                }));
                tracing::info!(enabled, title = row.display_title(), "favorite song toggled");
            }
            Err(error) => tracing::error!(%error, "failed to toggle favorite song"),
        }
    }

    pub(super) fn open_same_folder_for_selected(&mut self) {
        let Some(row) = self.selected_chart_row() else {
            return;
        };
        let Some(chart) = &row.chart else {
            return;
        };
        let folder_path = chart.folder_path.clone();
        self.selected_index_stack.push(self.selected_index);
        self.folder_stack.push(same_folder_path(&folder_path));
        self.reload_select_items();
        self.selected_index = 0;
        self.restart_select_bar_timer_without_scroll(Instant::now());
        self.play_system_sound(crate::system_sound::SoundType::FolderOpen);
        tracing::info!(folder = %folder_path, "entered same-folder view");
    }

    pub(super) fn start_random_select(&mut self, chart_ids: &[i64]) {
        if chart_ids.is_empty() {
            return;
        }
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let index = (nanos % chart_ids.len() as u128) as usize;
        self.start_chart(chart_ids[index]);
    }

    pub(super) fn enter_or_play_selected(&mut self) {
        if self.select_items.is_empty() {
            self.reload_select_items();
        }
        match self.select_items.get(self.selected_index).cloned() {
            Some(SelectItem::Folder { path, .. }) => {
                // 入る直前のカーソル位置を覚えておき、出た時に復元できるようにする。
                self.selected_index_stack.push(self.selected_index);
                self.folder_stack.push(path);
                self.reload_select_items();
                self.selected_index = 0;
                self.restart_select_bar_timer_without_scroll(Instant::now());
                self.play_system_sound(crate::system_sound::SoundType::FolderOpen);
                tracing::info!(folder = ?self.folder_stack.last(), "entered folder");
            }
            Some(SelectItem::Chart(row)) => {
                if row.in_library() {
                    self.start_chart(
                        row.chart.as_ref().expect("in_library row has chart").chart_id,
                    );
                } else {
                    self.acquire_missing_chart(&row);
                }
            }
            Some(SelectItem::Course(row)) => {
                if row.exists_all_songs() {
                    self.start_course(row.course_id);
                } else {
                    tracing::info!(
                        course_id = row.course_id,
                        title = %row.title,
                        resolved = row.resolved_count,
                        total = row.entry_count,
                        "skipping play for course missing entries"
                    );
                }
            }
            Some(SelectItem::Executable(row)) => match row.kind {
                SelectExecutableKind::RandomSelect => self.start_random_select(&row.chart_ids),
            },
            Some(SelectItem::Config(_)) => {}
            Some(SelectItem::KeyBinding(row)) => {
                self.begin_key_config_edit(row.key_mode, row.target);
            }
            Some(SelectItem::SettingsBack | SelectItem::SettingsClose) => {
                self.exit_folder();
            }
            Some(SelectItem::AdvancedSettings) => {
                self.open_advanced_settings_from_select();
            }
            None => {
                tracing::warn!("no item is available to select");
            }
        }
    }

    pub(super) fn acquire_missing_chart(&mut self, row: &SelectChartRow) {
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let action =
            choose_missing_chart_action(&self.boot.app_config.downloads, &row.download_metadata);
        match action {
            MissingChartAction::Browser(urls) => match open_browser_urls(&urls) {
                Ok(count) => {
                    let mut args = FluentArgs::new();
                    args.set("count", count as i64);
                    self.show_left_overlay_toast(text.format("toast-chart-sources-opened", &args));
                    tracing::info!(title = row.display_title(), count, "opened missing chart URLs");
                }
                Err(error) => {
                    self.show_left_overlay_toast(text.text("toast-chart-sources-open-failed"));
                    tracing::error!(%error, title = row.display_title(), "failed to open chart URLs");
                }
            },
            MissingChartAction::Unavailable => {
                self.show_left_overlay_toast(text.text("toast-chart-source-unavailable"));
                tracing::info!(
                    title = row.display_title(),
                    "missing chart has no available acquisition source"
                );
            }
            action @ (MissingChartAction::Ipfs { .. } | MissingChartAction::Http { .. }) => {
                self.spawn_chart_download(action, row.display_title().to_string());
            }
        }
    }

    pub(super) fn spawn_chart_download(&mut self, action: MissingChartAction, title: String) {
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        if self.pending_chart_download.is_some() {
            self.show_left_overlay_toast(text.text("toast-chart-download-in-progress"));
            return;
        }
        let source_name = match &action {
            MissingChartAction::Ipfs { .. } => "IPFS",
            MissingChartAction::Http { .. } => "HTTP",
            MissingChartAction::Browser(_) | MissingChartAction::Unavailable => return,
        };
        let request = ChartDownloadRequest {
            action,
            title: title.clone(),
            data_dir: self.boot.app_paths.data_dir.clone(),
        };
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("chart-download".to_string())
            .spawn(move || {
                let result = (|| -> Result<ChartDownloadResult> {
                    let runtime =
                        tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
                    runtime.block_on(download_chart(request))
                })();
                let _ = tx.send(result);
            })
            .expect("failed to spawn chart download thread");
        self.pending_chart_download = Some(rx);
        let mut args = FluentArgs::new();
        args.set("source", source_name);
        self.show_left_overlay_toast(text.format("toast-chart-download-started", &args));
        tracing::info!(source = source_name, %title, "started chart download");
    }

    pub(super) fn poll_pending_chart_download(&mut self) {
        let text = Localizer::new(self.boot.profile_config.ui.locale());
        let Some(rx) = &self.pending_chart_download else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(result)) => {
                self.pending_chart_download = None;
                let source_name = result.source.display_name();
                let mut args = FluentArgs::new();
                args.set("source", source_name);
                self.show_left_overlay_toast(
                    text.format("toast-chart-download-complete-registering", &args),
                );
                tracing::info!(
                    source = source_name,
                    path = %result.chart_dir.display(),
                    "chart download complete"
                );
                let label = format!("{source_name} chart download scan");
                if self.pending_song_scan.is_some() {
                    self.queued_download_scan = Some((result.root_dir, label));
                } else {
                    self.spawn_song_scan(
                        vec![PathEntry {
                            path: result.root_dir.to_string_lossy().into_owned(),
                            enabled: true,
                            recursive: true,
                        }],
                        true,
                        label,
                    );
                }
            }
            Ok(Err(error)) => {
                self.pending_chart_download = None;
                self.show_left_overlay_toast(text.text("toast-chart-download-failed"));
                tracing::error!(error = %format_error_chain(&error), "chart download failed");
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending_chart_download = None;
                self.show_left_overlay_toast(text.text("toast-chart-download-worker-ended"));
                tracing::warn!("chart download worker disconnected");
            }
        }
    }

    /// Returns true when the event was consumed by the search input layer
    /// (either because the user is in search mode or pressed the search-toggle
    /// hotkey), which suppresses normal m-select navigation for this event.
    /// Applies a winit IME event (Preedit / Commit / Enabled / Disabled) to the
    /// search query state. Only acts while the user is in search mode on the
    /// select screen — IME events received otherwise are ignored.
    pub(super) fn route_ime_event(&mut self, ime: &winit::event::Ime) {
        if !matches!(self.view_state(), AppViewState::Select) || !self.search.is_active() {
            return;
        }
        self.search.apply_ime(ime);
    }

    /// Toggles search mode and synchronizes IME enablement on the window.
    /// IME is only enabled while search mode is active to avoid macOS / Linux
    /// IMEs swallowing gameplay keypresses.
    pub(super) fn set_search_mode(&mut self, enabled: bool) {
        if enabled && in_settings_stack(&self.folder_stack) {
            return;
        }
        self.search.set_active(enabled);
        if let Some(window) = self.window.as_ref() {
            window.set_ime_allowed(enabled);
        }
        if enabled {
            self.update_search_ime_cursor_area();
        }
    }

    /// Positions the OS IME candidate window over the search input region of
    /// the active select skin (beatoraja `STRING_SEARCHWORD`, ref=30). No-op
    /// when not in search mode or when the skin does not define such a text
    /// element. Pixel coords are derived from the current window size and the
    /// skin canvas; letterboxing is approximated by direct proportional scale,
    /// which is close enough for IME candidate positioning.
    pub(super) fn update_search_ime_cursor_area(&self) {
        if !self.search.is_active() {
            return;
        }
        let Some(window) = self.window.as_ref() else { return };
        let Some(document) = self.renderer.select_skin_document() else { return };
        let Some((x_norm, y_norm, w_norm, h_norm)) = document.text_destination_rect_for_ref(30)
        else {
            return;
        };
        // egui_winit と同じ規約で物理ピクセル top-left を渡す。winit 側で各
        // バックエンドの座標系 (macOS は内部で `to_logical`) に変換される。
        let size = window.inner_size();
        let width = size.width as f32;
        let height = size.height as f32;
        let x = (x_norm * width).round() as i32;
        let y = (y_norm * height).round() as i32;
        let w = (w_norm * width).round().max(1.0) as u32;
        let h = (h_norm * height).round().max(1.0) as u32;
        window.set_ime_cursor_area(
            winit::dpi::PhysicalPosition::new(x, y),
            winit::dpi::PhysicalSize::new(w, h),
        );
    }

    pub(super) fn handle_search_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        match self.search.handle_key(
            event,
            self.select_e_action_held(),
            in_settings_stack(&self.folder_stack),
        ) {
            SearchInputAction::Ignored => false,
            SearchInputAction::Consumed => true,
            SearchInputAction::CursorMoved => {
                self.update_search_ime_cursor_area();
                true
            }
            SearchInputAction::EnterMode => {
                self.set_search_mode(true);
                tracing::info!("entered song search mode");
                true
            }
            SearchInputAction::ExitMode => {
                self.set_search_mode(false);
                tracing::info!("exited song search mode");
                true
            }
            SearchInputAction::Execute => {
                self.execute_song_search();
                true
            }
        }
    }

    /// Runs the current `search_query` against the library DB. On hit: appends
    /// to history (dedupe + bounded), pushes a virtual folder onto the stack,
    /// and exits search mode. On miss: leaves the query intact and updates the
    /// feedback message.
    pub(super) fn execute_song_search(&mut self) {
        let query = self.search.trimmed_query();
        if query.is_empty() {
            return;
        }
        let hit_count = match self.boot.library_db.search_charts(&query) {
            Ok(charts) => charts.len(),
            Err(error) => {
                tracing::error!(%error, %query, "song search failed");
                0
            }
        };
        if hit_count == 0 {
            // クエリをクリアして次入力を待つ。display_search_word はクエリ空 +
            // メッセージ有りの組み合わせで "no song found" を流す。
            self.search.set_no_results(
                Localizer::new(self.boot.profile_config.ui.locale())
                    .text("select-search-no-results"),
            );
            tracing::info!(%query, "song search returned no results");
            return;
        }

        self.search.record_successful_query(query.clone());

        self.set_search_mode(false);
        let mut args = FluentArgs::new();
        args.set("count", hit_count as i64);
        self.search.set_message(
            Localizer::new(self.boot.profile_config.ui.locale())
                .format("select-search-results", &args),
        );

        // 検索結果フォルダへ入る。`enter_or_play_selected` と同じ流儀でカーソル
        // 位置を退避してから push する。
        self.selected_index_stack.push(self.selected_index);
        self.folder_stack.push(format!("{SEARCH_PATH_PREFIX}{query}"));
        self.reload_select_items();
        self.selected_index = 0;
        self.restart_select_bar_timer_without_scroll(Instant::now());
        self.play_system_sound(crate::system_sound::SoundType::FolderOpen);
        tracing::info!(%query, hit_count, "entered search result folder");
    }

    pub(super) fn exit_folder(&mut self) {
        if self.key_config_edit.is_some() {
            self.cancel_key_config_edit();
        }
        if self.settings_edit.is_some() {
            self.cancel_settings_edit();
        }
        if self.folder_stack.pop().is_some() {
            let restored = self.selected_index_stack.pop().unwrap_or(0);
            self.reload_select_items();
            // 復元先がリスト範囲外なら末尾にクランプする。
            self.selected_index = restored.min(self.select_items.len().saturating_sub(1));
            self.restart_select_bar_timer_without_scroll(Instant::now());
            self.play_system_sound(crate::system_sound::SoundType::FolderClose);
            tracing::info!(depth = self.folder_stack.len(), "exited folder");
        }
    }
}
