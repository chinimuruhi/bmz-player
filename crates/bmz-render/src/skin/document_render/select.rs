macro_rules! skin_document_render_select_methods {
    () => {
        fn select_render_items(
            &self,
            sources: &HashMap<String, SkinDocumentTexture>,
            snapshot: &SelectSnapshot,
        ) -> Vec<SkinRenderItem> {
            self.select_render_items_with_dynamic_timers(
                sources,
                snapshot,
                None,
                &crate::select_settings_dest::SelectSettingsDestIndex::default(),
                None,
            )
        }

        fn select_render_items_with_dynamic_timers(
            &self,
            sources: &HashMap<String, SkinDocumentTexture>,
            snapshot: &SelectSnapshot,
            dynamic_timers: Option<&mut DynamicTimerRuntime>,
            settings_dest_index: &crate::select_settings_dest::SelectSettingsDestIndex,
            lua_draw_runtime: Option<Arc<dyn SkinLuaDrawRuntime>>,
        ) -> Vec<SkinRenderItem> {
            let (mut state, selected_row) = self.select_draw_state(snapshot, dynamic_timers);
            let text = SkinTextState {
                player_name: &snapshot.player_name,
                title: select_detail_title(snapshot, selected_row),
                subtitle: select_detail_subtitle(snapshot, selected_row),
                artist: select_detail_artist(snapshot, selected_row),
                genre: select_detail_genre(snapshot, selected_row),
                difficulty_name: if snapshot.in_settings {
                    ""
                } else {
                    selected_row.map(|row| row.difficulty_name.as_str()).unwrap_or_default()
                },
                play_level: selected_row.map(|row| row.play_level.as_str()).unwrap_or_default(),
                target: if snapshot.in_settings { "" } else { &snapshot.target },
                select_arrange: &snapshot.arrange,
                select_arrange_2p: &snapshot.arrange_2p,
                select_gauge: &snapshot.gauge,
                select_gauge_auto_shift: &snapshot.gauge_auto_shift,
                select_bottom_shiftable_gauge: &snapshot.bottom_shiftable_gauge,
                select_double_option: &snapshot.double_option,
                select_hs_fix: &snapshot.hs_fix,
                select_assist: &snapshot.assist,
                select_mode: &snapshot.select_mode,
                select_sort: &snapshot.select_sort,
                select_ln_mode: &snapshot.select_ln_mode,
                select_bga: &snapshot.bga,
                select_judge_timing_auto_adjust: if snapshot.judge_timing_auto_adjust {
                    "ON"
                } else {
                    "OFF"
                },
                current_folder: &snapshot.current_folder,
                table_level: selected_row
                    .map(|row| {
                        if row.table_text_secondary.is_empty() {
                            row.table_level.as_str()
                        } else {
                            row.table_text_secondary.as_str()
                        }
                    })
                    .unwrap_or_default(),
                table_text_primary: selected_row
                    .map(|row| row.table_text_primary.as_str())
                    .unwrap_or_default(),
                table_text_secondary: selected_row
                    .map(|row| row.table_text_secondary.as_str())
                    .unwrap_or_default(),
                table_text_fallback: selected_row
                    .map(|row| row.table_text_fallback.as_str())
                    .unwrap_or_default(),
                course_titles: selected_row
                    .map(|row| string_array_refs(&row.course_titles))
                    .unwrap_or_default(),
                search_word: &snapshot.search_word,
                search_word_alpha: snapshot.search_word_alpha,
                search_caret_byte_index: snapshot.search_caret_byte_index,
                rival: snapshot
                    .rival
                    .as_ref()
                    .map(|rival| rival.display_name.as_str())
                    .unwrap_or(""),
                ir_ranking: &snapshot.ir,
                ..SkinTextState::default()
            };

            let images = self.image_map();
            let values: HashMap<&str, &SkinValueDef> =
                self.value.iter().map(|value| (value.id.as_str(), value)).collect();
            let enabled_options = self.enabled_options();
            if let Some(runtime) = lua_draw_runtime {
                state.lua_runtime = Some(SkinLuaRuntimeContext {
                    runtime,
                    enabled_options: Arc::from(enabled_options.clone()),
                    text_values: Arc::new(lua_main_state_text_values(&state, &text)),
                });
            }
            let destinations = self.all_destinations(&enabled_options);
            let has_nearest_f_diff_rank_destination =
                nearest_f_diff_rank_destination_available(&destinations);
            let mut items = Vec::new();
            for (destination_index, destination) in destinations.into_iter().enumerate() {
                if destination.id
                    == self.songlist.as_ref().map(|list| list.id.as_str()).unwrap_or("")
                {
                    items.extend(self.select_songlist_items(
                        sources,
                        snapshot,
                        &images,
                        &enabled_options,
                        &state,
                    ));
                    continue;
                }
                if !crate::select_settings_dest::test_select_destination_visible(
                    settings_dest_index,
                    destination,
                    &enabled_options,
                    &state,
                    snapshot,
                    selected_row,
                    eval_skin_draw_condition,
                    |ops, enabled_options, state| {
                        if ops.len() == destination.op.len() && ops.iter().eq(destination.op.iter())
                        {
                            destination_ops_match(
                                destination,
                                enabled_options,
                                state,
                                has_nearest_f_diff_rank_destination,
                            )
                        } else {
                            test_skin_ops(ops, enabled_options, state)
                        }
                    },
                ) {
                    continue;
                }
                if let (Some(row), Some(judge_graph)) = (
                    selected_row.filter(|row| select_row_shows_score_decorations(row)),
                    self.judgegraph.iter().find(|graph| graph.id == destination.id),
                ) {
                    items.extend(self.select_note_distribution_graph_render_items(
                        row,
                        judge_graph,
                        destination,
                        (0, 0),
                        &enabled_options,
                        &state,
                    ));
                    continue;
                }
                if let (Some(row), Some(bpm_graph)) = (
                    selected_row.filter(|row| select_row_shows_score_decorations(row)),
                    self.bpmgraph.iter().find(|graph| graph.id == destination.id),
                ) {
                    let Some(elapsed) = skin_timer_elapsed_ms(destination.timer, &state) else {
                        continue;
                    };
                    let Some(mut frame) =
                        resolve_destination_frame(destination, elapsed, &enabled_options, &state)
                    else {
                        continue;
                    };
                    apply_skin_offset_to_frame(destination, &mut frame, &state, false);
                    if !destination_mouse_rect_contains(destination, frame, &state) {
                        continue;
                    }
                    items.extend(self.bpmgraph_render_items_with_segments(
                        bpm_graph,
                        destination,
                        frame,
                        &state,
                        &row.chart_bpm_graph_segments,
                    ));
                    continue;
                }
                if let Some(resolved) = self.resolve_destination_items(
                    destination_index,
                    destination,
                    DestinationResolveContext {
                        images: &images,
                        values: &values,
                        enabled_options: &enabled_options,
                        state: &state,
                        text_state: &text,
                        sources,
                        runtime_graphs: SkinRuntimeGraphs::from_document(self),
                        has_nearest_f_diff_rank_destination,
                        cache: None,
                    },
                ) {
                    items.extend(resolved);
                }
            }
            items
        }

        fn select_draw_state<'a>(
            &self,
            snapshot: &'a SelectSnapshot,
            dynamic_timers: Option<&mut DynamicTimerRuntime>,
        ) -> (SkinDrawState, Option<&'a SelectRowSnapshot>) {
            let selected_row =
                snapshot.rows.iter().find(|row| row.index == snapshot.selected_index);
            let mouse_position = snapshot.mouse_position.map(|(x, y)| {
                (x.clamp(0.0, 1.0) * self.w as f32, (1.0 - y.clamp(0.0, 1.0)) * self.h as f32)
            });
            let duration_green_ms = snapshot.note_display_duration_ms;
            let elapsed_ms =
                (snapshot.time.0 / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            let mut state = SkinDrawState {
                elapsed_ms,
                start_input_ms: skin_start_input_elapsed_ms(elapsed_ms, self.input),
                current_fps: snapshot.current_fps,
                operating_time_ms: snapshot.operating_time_ms,
                logical_input_held: snapshot.skin_input.held,
                skin_offsets: snapshot.skin_offsets,
                select_bar_elapsed_ms: (snapshot.selection_time.0 / 1_000)
                    .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                select_option_panel_elapsed_ms: (snapshot.option_panel_time.0 / 1_000)
                    .clamp(i32::MIN as i64, i32::MAX as i64)
                    as i32,
                select_option_panel_off_elapsed_ms: snapshot.option_panel_off_times.map(
                    |elapsed| {
                        elapsed.map(|elapsed| {
                            (elapsed.0 / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32
                        })
                    },
                ),
                select_option_panel: snapshot.option_panel,
                select_arrange_index: select_arrange_index(&snapshot.arrange),
                select_arrange_2p_index: select_arrange_index(&snapshot.arrange_2p),
                select_extended_arrange_index: extended_arrange_index(&snapshot.arrange),
                select_extended_arrange_2p_index: extended_arrange_index(&snapshot.arrange_2p),
                select_double_option_index: select_double_option_index(&snapshot.double_option),
                select_hs_fix_index: select_hs_fix_index(&snapshot.hs_fix),
                select_gauge_index: select_gauge_index(&snapshot.gauge),
                select_gauge_auto_shift_index: select_gauge_auto_shift_index(
                    &snapshot.gauge_auto_shift,
                ),
                select_bottom_shiftable_gauge_index: select_bottom_shiftable_gauge_index(
                    &snapshot.bottom_shiftable_gauge,
                ),
                select_target_index: play_target_image_index(&snapshot.target),
                select_bga_index: select_bga_index(&snapshot.bga),
                judge_timing_offset_ms: snapshot.judge_timing_offset_ms,
                judge_timing_auto_adjust: snapshot.judge_timing_auto_adjust,
                lanecover_enabled: snapshot.lanecover_enabled,
                lift_enabled: snapshot.lift_enabled,
                hidden_enabled: snapshot.hidden_enabled,
                hispeed_auto_adjust: snapshot.hispeed_auto_adjust,
                player_stats: snapshot.player_stats.clone(),
                select_assist_index: select_assist_index(&snapshot.assist),
                select_mode_index: select_mode_index(&snapshot.select_mode),
                select_sort_index: select_sort_index(&snapshot.select_sort),
                select_ln_mode_index: select_ln_mode_index(&snapshot.select_ln_mode),
                select_judge_algorithm_index: select_judge_algorithm_index(
                    &snapshot.judge_algorithm,
                ),
                hispeed: snapshot.hispeed,
                total_duration_ms: duration_green_ms
                    .map(green_duration_to_duration)
                    .unwrap_or(0)
                    .min(i32::MAX as i64) as i32,
                duration_green_ms,
                result_grade_diff_display: snapshot.grade_diff_display,
                select_scroll_progress: select_scroll_progress(snapshot),
                select_master_volume: snapshot.master_volume,
                select_key_volume: snapshot.key_volume,
                select_bgm_volume: snapshot.bgm_volume,
                select_has_banner: snapshot.banner_image,
                select_has_document: selected_row.is_some_and(|row| row.has_document),
                has_stagefile: snapshot.stage_background,
                has_backbmp: snapshot.backbmp_image,
                select_folder_song_count: selected_row.and_then(select_row_folder_song_count),
                select_screen: true,
                select_play_level: selected_row.map(select_row_level_number).unwrap_or(0),
                play_level: selected_row.map(select_row_level_number).unwrap_or(0),
                table_song: selected_row.is_some_and(|row| !row.table_text_primary.is_empty()),
                min_bpm: selected_row.map(|row| row.min_bpm).unwrap_or(0.0),
                max_bpm: selected_row.map(|row| row.max_bpm).unwrap_or(0.0),
                has_bpm_stop: selected_row
                    .map(|row| row.chart_bpm_graph_segments.iter().any(|s| s.is_stop))
                    .unwrap_or(false),
                main_bpm: selected_row.map(|row| row.chart_main_bpm).unwrap_or(0.0),
                difficulty: selected_row.map(select_row_difficulty_code).unwrap_or(0),
                judge_rank: selected_row.and_then(|row| row.judge_rank),
                select_ex_score: selected_row.and_then(|row| row.ex_score),
                select_replay_slots: selected_row.map(|row| row.replay_slots).unwrap_or([false; 4]),
                select_replay_index: selected_row.and_then(select_row_replay_index),
                select_clear_index: selected_row.map(select_row_clear_index).unwrap_or(0) as i64,
                select_favorite_song: selected_row.is_some_and(|row| row.favorite_song),
                select_favorite_chart: selected_row.is_some_and(|row| row.favorite_chart),
                select_replay_slot_rule_indices: snapshot.replay_slot_rule_indices,
                select_folder_lamp_counts: selected_row
                    .map(|row| row.folder_lamp_counts)
                    .unwrap_or([0; 11]),
                select_row_kind: selected_row.map(|row| row.kind).unwrap_or(SelectRowKind::Song),
                select_course_constraints: selected_row
                    .map(|row| row.course_constraints)
                    .unwrap_or_default(),
                select_is_folder: selected_row.is_some_and(|row| row.is_folder),
                select_in_library: selected_row.is_none_or(|row| row.in_library),
                select_total_notes: selected_row.map(|row| row.total_notes).unwrap_or(0),
                select_chart_normal_notes: selected_row
                    .map(|row| row.chart_normal_notes)
                    .unwrap_or(0),
                select_chart_long_notes: selected_row.map(|row| row.chart_long_notes).unwrap_or(0),
                select_chart_scratch_notes: selected_row
                    .map(|row| row.chart_scratch_notes)
                    .unwrap_or(0),
                select_chart_long_scratch_notes: selected_row
                    .map(|row| row.chart_long_scratch_notes)
                    .unwrap_or(0),
                select_chart_mine_notes: selected_row.map(|row| row.chart_mine_notes).unwrap_or(0),
                select_chart_density: selected_row.map(|row| row.chart_density).unwrap_or(0.0),
                select_chart_peak_density: selected_row
                    .map(|row| row.chart_peak_density)
                    .unwrap_or(0.0),
                select_chart_end_density: selected_row
                    .map(|row| row.chart_end_density)
                    .unwrap_or(0.0),
                select_chart_total_gauge: selected_row
                    .map(|row| row.chart_total_gauge)
                    .unwrap_or(0.0),
                select_chart_main_bpm: selected_row.map(|row| row.chart_main_bpm).unwrap_or(0.0),
                select_bpm: selected_row.map(|row| row.initial_bpm).unwrap_or(0.0),
                select_min_bpm: selected_row.map(|row| row.min_bpm).unwrap_or(0.0),
                select_max_bpm: selected_row.map(|row| row.max_bpm).unwrap_or(0.0),
                select_length_ms: selected_row.map(|row| row.length_ms).unwrap_or(0),
                select_play_count: selected_row.map(|row| row.play_count).unwrap_or(0),
                select_clear_count: selected_row.map(|row| row.clear_count).unwrap_or(0),
                select_bp: selected_row.and_then(|row| row.bp),
                select_cb: selected_row.and_then(|row| row.cb),
                judge_counts: selected_row.map(|row| row.judge_counts).unwrap_or_default(),
                fast_slow_counts: selected_row.and_then(|row| row.fast_slow_counts),
                max_combo: selected_row.and_then(|row| row.max_combo).unwrap_or(0),
                total_notes: selected_row.map(|row| row.total_notes).unwrap_or(0),
                past_notes: selected_row.map(|row| row.total_notes).unwrap_or(0),
                gauge: selected_row.and_then(|row| row.gauge_value).unwrap_or(0.0),
                gauge_auto_shift: snapshot.gauge_auto_shift != "OFF",
                ex_score: selected_row.and_then(|row| row.ex_score).unwrap_or(0),
                in_settings: snapshot.in_settings,
                settings_editing: snapshot.settings_editing,
                select_chart_key_mode: selected_row.and_then(|row| row.chart_key_mode),
                random_lane_refs: selected_row
                    .and_then(|row| row.chart_key_mode)
                    .map_or([0; SKIN_RANDOM_LANE_REF_COUNT], |key_mode| {
                        random_lane_refs(&snapshot.lane_shuffle_pattern, key_mode)
                    }),
                mouse_x: mouse_position.map(|position| position.0),
                mouse_y: mouse_position.map(|position| position.1),
                ir_ranking: snapshot.ir.clone(),
                rival_ex_score: snapshot.rival.as_ref().map(|rival| i64::from(rival.ex_score)),
                rival_max_combo: snapshot.rival.as_ref().map(|rival| i64::from(rival.max_combo)),
                rival_bp: snapshot.rival.as_ref().map(|rival| i64::from(rival.bp)),
                rival_judge_counts: snapshot.rival.as_ref().and_then(|rival| {
                    rival.judge_counts.map(|counts| {
                        [counts.pgreat, counts.great, counts.good, counts.bad, counts.poor]
                    })
                }),
                ..SkinDrawState::default()
            };
            if let Some(runtime) = dynamic_timers {
                let now_ms = state.elapsed_ms;
                runtime.advance(self, &mut state, now_ms);
            }
            (state, selected_row)
        }

        fn select_click_hit(
            &self,
            sources: &HashMap<String, SkinDocumentTexture>,
            snapshot: &SelectSnapshot,
            settings_dest_index: &crate::select_settings_dest::SelectSettingsDestIndex,
            x: f32,
            y: f32,
        ) -> Option<SkinClickHit> {
            self.select_click_hits(sources, snapshot, settings_dest_index)
                .into_iter()
                .rev()
                .find(|hit| rect_contains(hit.rect, x, y))
        }

        fn result_click_hit(&self, state: &SkinDrawState, x: f32, y: f32) -> Option<SkinClickHit> {
            let enabled_options = self.enabled_options();
            let images = self.image_map();
            let destinations = self.all_destinations(&enabled_options);
            let has_nearest_f_diff_rank_destination =
                nearest_f_diff_rank_destination_available(&destinations);
            destinations
                .into_iter()
                .filter(|destination| {
                    destination_ops_match(
                        destination,
                        &enabled_options,
                        state,
                        has_nearest_f_diff_rank_destination,
                    ) && eval_skin_draw_condition(&destination.draw, state)
                })
                .filter_map(|destination| {
                    Some(SkinClickHit {
                        target: self.click_target_for_destination(destination, &images)?,
                        rect: self.destination_click_rect(destination, &enabled_options, state)?,
                    })
                })
                .rev()
                .find(|hit| rect_contains(hit.rect, x, y))
        }

        fn result_slider_hit(
            &self,
            state: &SkinDrawState,
            x: f32,
            y: f32,
        ) -> Option<SkinSliderHit> {
            let enabled_options = self.enabled_options();
            let destinations = self.all_destinations(&enabled_options);
            let has_nearest_f_diff_rank_destination =
                nearest_f_diff_rank_destination_available(&destinations);
            destinations
                .into_iter()
                .filter(|destination| {
                    destination_ops_match(
                        destination,
                        &enabled_options,
                        state,
                        has_nearest_f_diff_rank_destination,
                    ) && eval_skin_draw_condition(&destination.draw, state)
                })
                .filter_map(|destination| {
                    let slider = self.slider.iter().find(|slider| slider.id == destination.id)?;
                    (slider.slider_type == 8).then_some(())?;
                    self.destination_slider_hit(slider, destination, &enabled_options, state, x, y)
                })
                .next_back()
        }

        fn select_slider_hit(
            &self,
            snapshot: &SelectSnapshot,
            settings_dest_index: &crate::select_settings_dest::SelectSettingsDestIndex,
            x: f32,
            y: f32,
        ) -> Option<SkinSliderHit> {
            let (state, selected_row) = self.select_draw_state(snapshot, None);
            let enabled_options = self.enabled_options();
            self.all_destinations(&enabled_options)
                .into_iter()
                .filter_map(|destination| {
                    if !crate::select_settings_dest::test_select_destination_visible(
                        settings_dest_index,
                        destination,
                        &enabled_options,
                        &state,
                        snapshot,
                        selected_row,
                        eval_skin_draw_condition,
                        test_skin_ops,
                    ) {
                        return None;
                    }
                    let slider = self.slider.iter().find(|slider| slider.id == destination.id)?;
                    self.destination_slider_hit(slider, destination, &enabled_options, &state, x, y)
                })
                .next_back()
        }

        fn select_click_hits(
            &self,
            _sources: &HashMap<String, SkinDocumentTexture>,
            snapshot: &SelectSnapshot,
            settings_dest_index: &crate::select_settings_dest::SelectSettingsDestIndex,
        ) -> Vec<SkinClickHit> {
            let (state, selected_row) = self.select_draw_state(snapshot, None);
            let enabled_options = self.enabled_options();
            let images = self.image_map();
            let mut hits = Vec::new();
            for destination in self.all_destinations(&enabled_options) {
                if destination.id
                    == self.songlist.as_ref().map(|list| list.id.as_str()).unwrap_or("")
                {
                    hits.extend(self.select_songlist_click_hits(
                        snapshot,
                        &enabled_options,
                        &state,
                    ));
                    continue;
                }
                if !crate::select_settings_dest::test_select_destination_visible(
                    settings_dest_index,
                    destination,
                    &enabled_options,
                    &state,
                    snapshot,
                    selected_row,
                    eval_skin_draw_condition,
                    test_skin_ops,
                ) {
                    continue;
                }
                let Some(target) = self.click_target_for_destination(destination, &images) else {
                    continue;
                };
                let Some(rect) = self.destination_click_rect(destination, &enabled_options, &state)
                else {
                    continue;
                };
                hits.push(SkinClickHit { target, rect });
            }
            hits
        }

        fn select_songlist_click_hits(
            &self,
            snapshot: &SelectSnapshot,
            enabled_options: &[i32],
            state: &SkinDrawState,
        ) -> Vec<SkinClickHit> {
            let Some(songlist) = &self.songlist else {
                return Vec::new();
            };
            let selected_row_position =
                select_snapshot_selected_row_position(&snapshot.rows, snapshot.selected_index)
                    as i32;
            let mut hits = Vec::new();
            let mut row_state = state.clone();
            for (row_position, row) in snapshot.rows.iter().enumerate() {
                let offset = row_position as i32 - selected_row_position;
                let slot = songlist.center + offset;
                if !songlist.clickable.contains(&slot) || slot < 0 {
                    continue;
                }
                let selected = row_position as i32 == selected_row_position;
                let row_destinations = if selected { &songlist.liston } else { &songlist.listoff };
                let Some(row_destination) =
                    destination_entry_at(row_destinations, slot as usize, enabled_options)
                else {
                    continue;
                };
                Self::apply_select_songlist_click_row_state(&mut row_state, row);
                let elapsed = skin_timer_elapsed_ms(row_destination.timer, state).unwrap_or(0);
                let Some(mut frame) = resolve_destination_frame(
                    row_destination,
                    elapsed,
                    enabled_options,
                    &row_state,
                ) else {
                    continue;
                };
                self.apply_select_songlist_scroll_to_frame(
                    &mut frame,
                    songlist,
                    slot,
                    enabled_options,
                    &row_state,
                    snapshot.bar_scroll_direction,
                    snapshot.bar_scroll_progress,
                );
                apply_skin_offset_to_frame(row_destination, &mut frame, &row_state, false);
                if !destination_mouse_rect_contains(row_destination, frame, &row_state) {
                    continue;
                }
                let rect = normalize_skin_frame_rect(frame, self.w, self.h);
                if rect.width <= 0.0 || rect.height <= 0.0 {
                    continue;
                }
                hits.push(SkinClickHit {
                    target: SkinClickTarget::SelectRow { row_index: row.index },
                    rect,
                });
            }
            hits
        }

        fn apply_select_songlist_render_row_state(
            state: &mut SkinDrawState,
            row: &SelectRowSnapshot,
        ) {
            state.select_play_level = select_row_level_number(row);
            state.play_level = select_row_level_number(row);
            state.table_song = !row.table_text_primary.is_empty();
            state.difficulty = select_row_difficulty_code(row);
            state.judge_rank = row.judge_rank;
            state.select_ex_score = row.ex_score;
            state.select_replay_slots = row.replay_slots;
            state.select_replay_index = select_row_replay_index(row);
            state.select_clear_index = select_row_clear_index(row) as i64;
            state.select_favorite_song = row.favorite_song;
            state.select_favorite_chart = row.favorite_chart;
            state.select_folder_lamp_counts = row.folder_lamp_counts;
            state.select_row_kind = row.kind;
            state.select_course_constraints = row.course_constraints;
            state.select_is_folder = row.is_folder;
            state.select_in_library = row.in_library;
            state.select_total_notes = row.total_notes;
            state.select_chart_normal_notes = row.chart_normal_notes;
            state.select_chart_long_notes = row.chart_long_notes;
            state.select_chart_scratch_notes = row.chart_scratch_notes;
            state.select_chart_long_scratch_notes = row.chart_long_scratch_notes;
            state.select_chart_mine_notes = row.chart_mine_notes;
            state.select_chart_density = row.chart_density;
            state.select_chart_peak_density = row.chart_peak_density;
            state.select_chart_end_density = row.chart_end_density;
            state.select_chart_total_gauge = row.chart_total_gauge;
            state.select_chart_main_bpm = row.chart_main_bpm;
            state.select_bpm = row.initial_bpm;
            state.select_min_bpm = row.min_bpm;
            state.select_max_bpm = row.max_bpm;
            state.min_bpm = row.min_bpm;
            state.max_bpm = row.max_bpm;
            state.main_bpm = row.chart_main_bpm;
            state.select_length_ms = row.length_ms;
            state.select_play_count = row.play_count;
            state.select_clear_count = row.clear_count;
            state.select_bp = row.bp;
            state.select_cb = row.cb;
            state.max_combo = row.max_combo.unwrap_or(0);
            state.total_notes = row.total_notes;
            state.gauge = row.gauge_value.unwrap_or(0.0);
            state.ex_score = row.ex_score.unwrap_or(0);
            state.select_chart_key_mode = row.chart_key_mode;
        }

        fn apply_select_songlist_click_row_state(
            state: &mut SkinDrawState,
            row: &SelectRowSnapshot,
        ) {
            Self::apply_select_songlist_render_row_state(state, row);
        }

        fn click_target_for_destination(
            &self,
            destination: &SkinDestinationDef,
            images: &HashMap<&str, &SkinImageDef>,
        ) -> Option<SkinClickTarget> {
            if destination.clickable == Some(false) {
                return None;
            }
            if let Some(event_id) = destination.act {
                return Some(SkinClickTarget::Event { event_id, click: destination.click });
            }
            if let Some(image) = images.get(destination.id.as_str())
                && destination.clickable.or(image.clickable).unwrap_or(image.act.is_some())
                && let Some(event_id) = image.act
            {
                return Some(SkinClickTarget::Event { event_id, click: image.click });
            }
            let imageset = self.imageset.iter().find(|set| set.id == destination.id)?;
            destination
                .clickable
                .or(imageset.clickable)
                .unwrap_or(imageset.act.is_some())
                .then_some(imageset.act)
                .flatten()
                .map(|event_id| SkinClickTarget::Event { event_id, click: imageset.click })
        }

        fn destination_click_rect(
            &self,
            destination: &SkinDestinationDef,
            enabled_options: &[i32],
            state: &SkinDrawState,
        ) -> Option<Rect> {
            let elapsed = skin_timer_elapsed_ms(destination.timer, state)?;
            let mut frame =
                resolve_destination_frame(destination, elapsed, enabled_options, state)?;
            apply_skin_offset_to_frame(destination, &mut frame, state, false);
            if !destination_mouse_rect_contains(destination, frame, state) {
                return None;
            }
            let rect = normalize_skin_frame_rect(frame, self.w, self.h);
            if rect.width <= 0.0 || rect.height <= 0.0 { None } else { Some(rect) }
        }

        fn destination_slider_hit(
            &self,
            slider: &SkinSliderDef,
            destination: &SkinDestinationDef,
            enabled_options: &[i32],
            state: &SkinDrawState,
            x: f32,
            y: f32,
        ) -> Option<SkinSliderHit> {
            if !slider.changeable || !matches!(slider.slider_type, 1 | 8 | 17..=19) {
                return None;
            }
            let elapsed = skin_timer_elapsed_ms(destination.timer, state)?;
            let mut frame =
                resolve_destination_frame(destination, elapsed, enabled_options, state)?;
            apply_skin_offset_to_frame(destination, &mut frame, state, false);
            if !destination_mouse_rect_contains(destination, frame, state) {
                return None;
            }
            let mouse_x = x.clamp(0.0, 1.0) * self.w as f32;
            let mouse_y = (1.0 - y.clamp(0.0, 1.0)) * self.h as f32;
            let value = if slider.slider_type == 1 {
                scroll_slider_value_at(slider, frame, mouse_x, mouse_y)?
            } else {
                slider_value_at(slider, frame, mouse_x, mouse_y)?
            };
            Some(SkinSliderHit { slider_type: slider.slider_type, value })
        }

        fn select_songlist_items(
            &self,
            sources: &HashMap<String, SkinDocumentTexture>,
            snapshot: &SelectSnapshot,
            images: &HashMap<&str, &SkinImageDef>,
            enabled_options: &[i32],
            state: &SkinDrawState,
        ) -> Vec<SkinRenderItem> {
            let Some(songlist) = &self.songlist else {
                return Vec::new();
            };
            let mut items = Vec::new();
            let selected_row_position =
                select_snapshot_selected_row_position(&snapshot.rows, snapshot.selected_index)
                    as i32;
            let mut row_state = state.clone();
            for (row_position, row) in snapshot.rows.iter().enumerate() {
                let offset = row_position as i32 - selected_row_position;
                let slot = songlist.center + offset;
                if slot < 0 {
                    continue;
                }
                let selected = row_position as i32 == selected_row_position;
                let row_destinations = if selected { &songlist.liston } else { &songlist.listoff };
                let Some(row_destination) =
                    destination_entry_at(row_destinations, slot as usize, enabled_options)
                else {
                    continue;
                };
                Self::apply_select_songlist_render_row_state(&mut row_state, row);
                let elapsed = skin_timer_elapsed_ms(row_destination.timer, state).unwrap_or(0);
                let Some(mut row_frame) = resolve_destination_frame(
                    row_destination,
                    elapsed,
                    enabled_options,
                    &row_state,
                ) else {
                    continue;
                };
                self.apply_select_songlist_scroll_to_frame(
                    &mut row_frame,
                    songlist,
                    slot,
                    enabled_options,
                    &row_state,
                    snapshot.bar_scroll_direction,
                    snapshot.bar_scroll_progress,
                );
                let row_origin = (row_frame.x, row_frame.y);
                apply_skin_offset_to_frame(row_destination, &mut row_frame, state, false);
                if let Some(item) = self.select_bar_item(row, row_destination, row_frame, sources) {
                    items.push(item);
                }
                if select_row_shows_lamp(row) {
                    let clear_index = select_row_clear_index(row);
                    items.extend(self.select_songlist_child_items_by_index(
                        &songlist.lamp,
                        clear_index,
                        row_origin,
                        images,
                        enabled_options,
                        &row_state,
                        sources,
                    ));
                }
                if select_row_shows_score_decorations(row) {
                    if select_row_shows_level(row) {
                        items.extend(self.select_songlist_level_items(
                            &songlist.level,
                            row,
                            row_origin,
                            images,
                            enabled_options,
                            &row_state,
                            sources,
                        ));
                    }
                    for label_index in select_row_label_indices(row) {
                        items.extend(self.select_songlist_child_items_by_index(
                            &songlist.label,
                            label_index,
                            row_origin,
                            images,
                            enabled_options,
                            &row_state,
                            sources,
                        ));
                    }
                    if select_row_shows_course_trophy(row)
                        && let Some(trophy_index) = select_row_trophy_index(row)
                    {
                        items.extend(self.select_songlist_child_items_by_index(
                            &songlist.trophy,
                            trophy_index,
                            row_origin,
                            images,
                            enabled_options,
                            &row_state,
                            sources,
                        ));
                    }
                    items.extend(self.select_songlist_all_child_items(
                        &songlist.judgegraph,
                        row,
                        row_origin,
                        images,
                        enabled_options,
                        &row_state,
                        sources,
                    ));
                    items.extend(self.select_songlist_all_child_items(
                        &songlist.bpmgraph,
                        row,
                        row_origin,
                        images,
                        enabled_options,
                        &row_state,
                        sources,
                    ));
                }
                if select_row_shows_folder_distribution(row) {
                    items.extend(self.select_songlist_all_child_items(
                        &songlist.graph,
                        row,
                        row_origin,
                        images,
                        enabled_options,
                        &row_state,
                        sources,
                    ));
                }
                items.extend(self.select_songlist_text_items(
                    row,
                    row_origin,
                    images,
                    enabled_options,
                    &row_state,
                    sources,
                ));
            }
            items
        }

        fn apply_select_songlist_scroll_to_frame(
            &self,
            frame: &mut ResolvedSkinFrame,
            songlist: &SkinSongListDef,
            slot: i32,
            enabled_options: &[i32],
            state: &SkinDrawState,
            direction: i32,
            progress: f32,
        ) {
            let direction = direction.signum();
            let progress = progress.clamp(0.0, 1.0);
            if direction == 0 || progress <= 0.0 {
                return;
            }
            let next_slot = slot + direction;
            if next_slot < 0 {
                return;
            }
            let next_selected = next_slot == songlist.center;
            let next_destinations =
                if next_selected { &songlist.liston } else { &songlist.listoff };
            let Some(next_destination) =
                destination_entry_at(next_destinations, next_slot as usize, enabled_options)
            else {
                return;
            };
            let elapsed = skin_timer_elapsed_ms(next_destination.timer, state).unwrap_or(0);
            let Some(next_frame) =
                resolve_destination_frame(next_destination, elapsed, enabled_options, state)
            else {
                return;
            };
            frame.x += ((next_frame.x - frame.x) as f32 * progress).round() as i32;
            frame.y += ((next_frame.y - frame.y) as f32 * progress).round() as i32;
        }

        fn select_songlist_all_child_items(
            &self,
            entries: &[DestinationListEntry],
            row: &SelectRowSnapshot,
            row_origin: (i32, i32),
            images: &HashMap<&str, &SkinImageDef>,
            enabled_options: &[i32],
            state: &SkinDrawState,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Vec<SkinRenderItem> {
            let mut items = Vec::new();
            for destination in destination_entries(entries, enabled_options) {
                if let Some(judge_graph) =
                    self.judgegraph.iter().find(|graph| graph.id == destination.id)
                {
                    items.extend(self.select_note_distribution_graph_render_items(
                        row,
                        judge_graph,
                        destination,
                        row_origin,
                        enabled_options,
                        state,
                    ));
                    continue;
                }
                if let Some(bpm_graph) =
                    self.bpmgraph.iter().find(|graph| graph.id == destination.id)
                {
                    items.extend(self.select_bpmgraph_row_render_items(
                        row,
                        bpm_graph,
                        destination,
                        row_origin,
                        enabled_options,
                        state,
                    ));
                    continue;
                }
                if select_row_shows_folder_distribution(row)
                    && let Some(graph) = self.graph.iter().find(|graph| graph.id == destination.id)
                {
                    items.extend(self.select_folder_distribution_graph_render_items(
                        row,
                        graph,
                        destination,
                        row_origin,
                        enabled_options,
                        state,
                        sources,
                    ));
                    continue;
                }
                if let Some(mut resolved) = self.resolve_offset_destination_items(
                    destination,
                    row_origin,
                    images,
                    enabled_options,
                    state,
                    &SkinTextState::default(),
                    sources,
                ) {
                    items.append(&mut resolved);
                }
            }
            items
        }

        fn select_folder_distribution_graph_render_items(
            &self,
            row: &SelectRowSnapshot,
            graph: &SkinGraphDef,
            destination: &SkinDestinationDef,
            row_origin: (i32, i32),
            enabled_options: &[i32],
            state: &SkinDrawState,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Vec<SkinRenderItem> {
            let Some(source) = sources.get(&graph.src) else {
                return Vec::new();
            };
            if !test_skin_ops(&destination.op, enabled_options, state)
                || !eval_skin_draw_condition(&destination.draw, state)
            {
                return Vec::new();
            }
            let Some(elapsed) = skin_timer_elapsed_ms(destination.timer, state) else {
                return Vec::new();
            };
            let Some(mut frame) =
                resolve_destination_frame(destination, elapsed, enabled_options, state)
            else {
                return Vec::new();
            };
            frame.x += row_origin.0;
            frame.y += row_origin.1;
            apply_skin_offset_to_frame(destination, &mut frame, state, false);

            let total: u32 = row.folder_lamp_counts.iter().sum();
            if total == 0 {
                return Vec::new();
            }

            let dst = normalize_skin_frame_rect(frame, self.w, self.h);
            let source_w = source.source_size.width.max(1.0);
            let source_h = source.source_size.height.max(1.0);
            let cell_w = skin_grid_cell_size(graph.w, graph.divx.max(11));
            let cell_h = skin_grid_cell_size(graph.h, graph.divy);
            if cell_w <= 0 || cell_h <= 0 {
                return Vec::new();
            }
            let animation_rows = graph.divy.max(1);
            let animation_row = if graph.cycle > 0 && animation_rows > 1 {
                (elapsed.rem_euclid(graph.cycle) * animation_rows / graph.cycle)
                    .min(animation_rows - 1)
            } else {
                0
            };

            let mut items = Vec::new();
            let mut filled = 0.0;
            for lamp_index in (0..row.folder_lamp_counts.len()).rev() {
                let count = row.folder_lamp_counts[lamp_index];
                if count == 0 {
                    continue;
                }
                let width = dst.width * (count as f32 / total as f32);
                if width <= 0.0 {
                    continue;
                }
                let rect = Rect { x: dst.x + filled, width, ..dst };
                let source_x = graph.x + cell_w * lamp_index as i32;
                let source_y = graph.y + cell_h * animation_row;
                let uv = TextureRegion {
                    x: source_x as f32 / source_w,
                    y: source_y as f32 / source_h,
                    width: cell_w as f32 / source_w,
                    height: cell_h as f32 / source_h,
                };
                items.push(SkinRenderItem::Image {
                    texture: source.texture,
                    rect,
                    uv,
                    tint: Color::rgba(
                        frame.r as f32 / 255.0,
                        frame.g as f32 / 255.0,
                        frame.b as f32 / 255.0,
                        frame.a as f32 / 255.0,
                    ),
                    blend: BlendMode::Normal,
                    scale: SkinImageScale::Stretch,
                    border: None,
                    source_size: Some(source.source_size),
                    linear_filter: false,
                });
                filled += width;
            }
            items
        }

        fn select_songlist_level_items(
            &self,
            entries: &[DestinationListEntry],
            row: &SelectRowSnapshot,
            row_origin: (i32, i32),
            images: &HashMap<&str, &SkinImageDef>,
            enabled_options: &[i32],
            state: &SkinDrawState,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Vec<SkinRenderItem> {
            let level_index = select_row_difficulty_code(row).clamp(0, i64::MAX) as usize;
            self.select_songlist_child_items_by_index(
                entries,
                level_index,
                row_origin,
                images,
                enabled_options,
                state,
                sources,
            )
        }

        fn select_songlist_child_items_by_index(
            &self,
            entries: &[DestinationListEntry],
            index: usize,
            row_origin: (i32, i32),
            images: &HashMap<&str, &SkinImageDef>,
            enabled_options: &[i32],
            state: &SkinDrawState,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Vec<SkinRenderItem> {
            let mut items = Vec::new();
            let Some(destination) = destination_entry_at(entries, index, enabled_options) else {
                return items;
            };
            if let Some(mut resolved) = self.resolve_offset_destination_items(
                destination,
                row_origin,
                images,
                enabled_options,
                state,
                &SkinTextState::default(),
                sources,
            ) {
                items.append(&mut resolved);
            }
            items
        }

        fn select_songlist_text_items(
            &self,
            row: &SelectRowSnapshot,
            row_origin: (i32, i32),
            images: &HashMap<&str, &SkinImageDef>,
            enabled_options: &[i32],
            state: &SkinDrawState,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Vec<SkinRenderItem> {
            let Some(songlist) = &self.songlist else {
                return Vec::new();
            };
            let mut items = Vec::new();
            let text_state = SkinTextState {
                bar_text: &row.title,
                table_level: if row.table_text_secondary.is_empty() {
                    &row.table_level
                } else {
                    &row.table_text_secondary
                },
                table_text_primary: &row.table_text_primary,
                table_text_secondary: &row.table_text_secondary,
                table_text_fallback: &row.table_text_fallback,
                ..SkinTextState::default()
            };
            let destinations = destination_entries(&songlist.text, enabled_options);
            let Some(destination) = select_row_slot_with_fallbacks(
                &destinations,
                select_row_bar_text_index(row),
                select_row_bar_text_fallback_indices(row),
            )
            .copied() else {
                return items;
            };
            {
                if let Some(mut resolved) = self.resolve_offset_destination_items(
                    destination,
                    row_origin,
                    images,
                    enabled_options,
                    state,
                    &text_state,
                    sources,
                ) {
                    items.append(&mut resolved);
                }
            }
            items
        }

        fn select_bar_item(
            &self,
            row: &SelectRowSnapshot,
            destination: &SkinDestinationDef,
            frame: ResolvedSkinFrame,
            sources: &HashMap<String, SkinDocumentTexture>,
        ) -> Option<SkinRenderItem> {
            let imageset = self.imageset.iter().find(|set| set.id == destination.id)?;
            let image_index = select_row_bar_image_index(row);
            let image_id = select_row_slot_with_fallbacks(
                &imageset.images,
                image_index,
                select_row_bar_image_fallback_indices(row),
            )?;
            let image = self.image.iter().find(|image| image.id == *image_id)?;
            let source = resolve_document_source(sources, &image.src)?;
            let elapsed =
                skin_timer_elapsed_ms(destination.timer, &SkinDrawState::default()).unwrap_or(0);
            let (rect, uv) = stretch_skin_image_geometry(
                destination.stretch,
                normalize_skin_frame_rect(frame, self.w, self.h),
                skin_image_texture_region(image, source.source_size, elapsed),
                source.source_size,
                self.w,
                self.h,
            );
            Some(skin_image_item_for_frame(
                source.texture,
                rect,
                uv,
                frame,
                destination.center,
                if destination.blend == 2 { BlendMode::Add } else { BlendMode::Normal },
                Some(source.source_size),
                destination.filter != 0,
            ))
        }
    };
}

pub(super) use skin_document_render_select_methods;
