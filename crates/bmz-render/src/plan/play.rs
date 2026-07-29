use super::*;

pub(super) fn plan_play(
    snapshot: &RenderSnapshot,
    skin: &SkinContext,
    dynamic_timers: &mut crate::skin::DynamicTimerRuntime,
) -> DrawPlan {
    let text = TextRenderer;
    let skin_manifest = skin.manifest();
    let has_document = skin.document().is_some();
    let mut commands = Vec::with_capacity(play_command_capacity(snapshot, has_document));
    if snapshot.backbmp_background {
        push_fullscreen_image(&mut commands, PLAY_BACKBMP_TEXTURE);
    }
    if !has_document {
        push_fallback_bga_background(&mut commands, snapshot);
    }
    let key_mode = snapshot.key_mode;
    let active_lanes = key_mode.active_lanes();
    let active_lane_count = active_lanes.len();
    let board = Rect { x: 0.18, y: 0.05, width: 0.64, height: 0.9 };
    let lane_width = board.width / active_lane_count as f32;

    // 見逃しPOOR（is_miss）はボムエフェクトを出さない
    let mut bomb_ms: [Option<i32>; LANE_COUNT] = [None; LANE_COUNT];
    let mut lane_judge: [Option<usize>; LANE_COUNT] = [None; LANE_COUNT];
    let judge_timer_limit =
        skin.document().map_or(1, |document| document.judgetimer).max(0) as usize;
    for j in &snapshot.recent_judgements {
        if j.is_miss {
            continue;
        }
        let idx = j.lane.index();
        let elapsed =
            ((snapshot.time.0 - j.time.0) / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        let judge_index = judge_image_index(&j.text);
        lane_judge[idx] = judge_index;
        if judge_starts_bomb(judge_index, judge_timer_limit) {
            bomb_ms[idx] = Some(elapsed);
        }
    }

    // keyon/keyoff のタイマー値は session 側で per-lane に追跡された keyon/keyoff
    // 開始時刻から算出済み。snapshot.recent_inputs から再構築しない。
    let keyon_ms = snapshot.keyon_ms;
    let keyoff_ms = snapshot.keyoff_ms;

    let judge_region_count = skin.document().map(|d| d.judge_region_count()).unwrap_or(1);
    let judge_region_state = crate::skin::build_judge_region_state(
        &snapshot.recent_judgements,
        snapshot.time.0,
        judge_region_count,
    );
    let play_elapsed_ms =
        (snapshot.play_elapsed_time.0 / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let ready_timer_ms = snapshot
        .ready_elapsed_time
        .map(|time| (time.0 / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32);
    let skin_canvas_h = skin.document().map_or(720, |d| d.h) as f32;
    let skin_lane_h = skin_lane_height_px(skin, key_mode, skin_canvas_h);

    let mut skin_state = crate::skin::SkinDrawState {
        elapsed_ms: play_elapsed_ms,
        start_input_ms: crate::skin::skin_start_input_elapsed_ms(
            play_elapsed_ms,
            skin.document().map_or(0, |document| document.input),
        ),
        current_fps: snapshot.current_fps,
        operating_time_ms: snapshot.operating_time_ms,
        ready_timer_ms,
        play_timer_ms: (snapshot.time.0 >= 0)
            .then_some((snapshot.time.0 / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32),
        rhythm_timer_ms: snapshot.rhythm_timer_elapsed_ms,
        quarter_note_elapsed_ms: snapshot.quarter_note_elapsed_ms,
        key_mode,
        logical_input_held: snapshot.skin_input.held,
        select_arrange_index: crate::skin::select_arrange_index(&snapshot.arrange),
        select_arrange_2p_index: crate::skin::select_arrange_index(&snapshot.arrange_2p),
        select_target_index: crate::skin::play_target_image_index(&snapshot.target),
        select_extended_arrange_index: crate::skin::extended_arrange_index(&snapshot.arrange),
        select_extended_arrange_2p_index: crate::skin::extended_arrange_index(&snapshot.arrange_2p),
        random_lane_refs: crate::skin::fixed_random_lane_refs(
            &snapshot.lane_shuffle_pattern,
            snapshot.key_mode,
            &snapshot.arrange,
            &snapshot.arrange_2p,
        ),
        combo: snapshot.combo,
        max_combo: snapshot.max_combo,
        ex_score: snapshot.ex_score,
        total_notes: snapshot.total_notes,
        select_chart_total_gauge: snapshot.chart_total_gauge,
        past_notes: snapshot.past_notes,
        judge_counts: snapshot.judge_counts,
        fast_slow_counts: Some(snapshot.fast_slow_counts),
        gauge: snapshot.gauge,
        gauge_type: snapshot.gauge_type,
        gauge_auto_shift: snapshot.gauge_auto_shift,
        gauge_max: snapshot.gauge_max,
        gauge_border: snapshot.gauge_border,
        play_progress: play_progress(snapshot),
        end_of_note: end_of_note(snapshot),
        end_of_note_ms: snapshot.end_of_note_elapsed_ms,
        bomb_ms,
        keyon_ms,
        keyoff_ms,
        hold_ms: snapshot.hold_ms,
        hcn_active_ms: snapshot.hcn_active_ms,
        hcn_damage_ms: snapshot.hcn_damage_ms,
        lane_judge,
        judge_ms: judge_region_state.judge_ms,
        full_combo_ms: snapshot.full_combo_elapsed_ms,
        full_combo_2p_ms: snapshot
            .opponent
            .as_ref()
            .and_then(|opponent| opponent.full_combo_elapsed_ms),
        fadeout_ms: snapshot.fadeout_elapsed_ms,
        failed_ms: snapshot.failed_elapsed_ms,
        music_end_ms: snapshot.music_end_elapsed_ms,
        gauge_increase_ms: snapshot.gauge_increase_elapsed_ms,
        gauge_increase_2p_ms: snapshot
            .opponent
            .as_ref()
            .and_then(|opponent| opponent.gauge_increase_elapsed_ms),
        gauge_max_ms: snapshot.gauge_max_elapsed_ms,
        gauge_max_2p_ms: snapshot
            .opponent
            .as_ref()
            .and_then(|opponent| opponent.gauge_max_elapsed_ms),
        end_of_note_2p_ms: snapshot
            .opponent
            .as_ref()
            .and_then(|opponent| opponent.end_of_note_elapsed_ms),
        judge_index: judge_region_state.judge_index,
        judge_combo: judge_region_state.judge_combo,
        judge_timing_sign: judge_region_state.judge_timing_sign,
        offset_lift_px: skin_lift_offset_px(snapshot.lift, skin_lane_h),
        offset_lanecover_px: skin_lanecover_offset_px(
            snapshot.lane_cover,
            snapshot.lift,
            skin_lane_h,
        ),
        offset_hidden_cover_px: skin_hidden_cover_offset_px(
            snapshot.lift,
            snapshot.hidden_cover,
            skin_lane_h,
        ),
        skin_offsets: snapshot.skin_offsets,
        hispeed: snapshot.hispeed,
        hispeed_mode_index: snapshot.hispeed_mode_index,
        target_green_number: snapshot.target_green_number,
        timeleft_ms: (snapshot.duration.0.saturating_sub(snapshot.time.0) / 1_000)
            .saturating_add(1_000)
            .clamp(0, i32::MAX as i64) as i32,
        total_duration_ms: snapshot.note_display_duration_ms,
        duration_green_ms: Some(crate::skin::duration_to_green_number_ms(
            snapshot.note_display_duration_ms,
        )),
        lane_cover: snapshot.lane_cover,
        lift: snapshot.lift,
        lane_cover_changing: snapshot.lane_cover_changing,
        lanecover_enabled: snapshot.lanecover_enabled,
        lift_enabled: snapshot.lift_enabled,
        hidden_enabled: snapshot.hidden_enabled,
        hidden_cover: snapshot.hidden_cover,
        play_level: skin_level_number(&snapshot.play_level),
        table_song: !snapshot.table_text_primary.is_empty(),
        difficulty: skin_difficulty_code(&snapshot.difficulty_name),
        judge_rank: snapshot.judge_rank,
        now_bpm: snapshot.now_bpm,
        min_bpm: snapshot.min_bpm,
        max_bpm: snapshot.max_bpm,
        has_bga: snapshot.has_bga,
        has_bpm_stop: snapshot.has_bpm_stop,
        bga_enabled: snapshot.bga_enabled,
        has_stagefile: snapshot.stagefile_background,
        stagefile_image_size: snapshot.stagefile_image_size,
        has_backbmp: snapshot.backbmp_background,
        bga_base: snapshot.bga_base.map(skin_bga_frame_from_display),
        bga_layer: snapshot.bga_layer.map(skin_bga_frame_from_display),
        bga_layer2: snapshot.bga_layer2.map(skin_bga_frame_from_display),
        bga_poor: snapshot.bga_poor.map(skin_bga_frame_from_display),
        bga_stretch: snapshot.bga_stretch,
        judge_timing_ms: judge_region_state.judge_timing_ms,
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
        rival_ex_score: snapshot.opponent.as_ref().map(|opponent| i64::from(opponent.ex_score)),
        rival_max_combo: snapshot.opponent.as_ref().map(|opponent| i64::from(opponent.max_combo)),
        rival_bp: snapshot.opponent.as_ref().map(|opponent| {
            i64::from(opponent.judge_counts.bad.saturating_add(opponent.judge_counts.poor))
        }),
        rival_judge_counts: snapshot.opponent.as_ref().map(|opponent| {
            [
                opponent.judge_counts.pgreat,
                opponent.judge_counts.great,
                opponent.judge_counts.good,
                opponent.judge_counts.bad,
                opponent.judge_counts.poor,
            ]
        }),
        course_stage: snapshot.course_stage,
        hit_error_ring: snapshot.hit_error_ring.values,
        hit_error_ring_index: snapshot.hit_error_ring.index,
        // beatoraja の op 80/81 はリソースロード状態ではなく PRELOAD state を表す。
        // TIMER_READY (40) の開始と同じフレームで 80 -> 81 を切り替える。
        skin_loaded: snapshot.ready_elapsed_time.is_some(),
        resource_load_progress: snapshot.resource_load_progress,
        ..crate::skin::SkinDrawState::default()
    };
    dynamic_timers.ingest_skin_events(&snapshot.skin_events, key_mode, snapshot.time.0);
    advance_skin_dynamic_timers(skin, dynamic_timers, &mut skin_state, play_elapsed_ms);
    let skin_text = SkinTextState {
        player_name: &snapshot.player_name,
        title: &snapshot.title,
        subtitle: &snapshot.subtitle,
        artist: &snapshot.artist,
        subartist: &snapshot.subartist,
        genre: &snapshot.genre,
        difficulty_name: &snapshot.difficulty_name,
        play_level: &snapshot.play_level,
        target: &snapshot.target,
        table_level: &snapshot.table_text_secondary,
        table_text_primary: &snapshot.table_text_primary,
        table_text_secondary: &snapshot.table_text_secondary,
        table_text_fallback: &snapshot.table_text_fallback,
        course_stage: snapshot.course_stage,
        course_titles: string_array_refs(&snapshot.course_titles),
        ..SkinTextState::default()
    };
    // `{"id":"notes"}` マーカーと `timer: 3` (FAILED) で3分割。
    // 描画順: 背面skin → ロング/ノーツ → 前面skin → 暗転/閉店オーバーレイ
    let (behind_notes_items, front_notes_items, failed_overlay_items) = skin
        .static_document_play_items_split_for_state_and_text(
            &skin_state,
            &skin_text,
            &snapshot.judge_graph_density,
            &snapshot.bpm_graph_segments,
        );
    let behind_notes_items = skin.apply_play_skin_global_offset(behind_notes_items, &skin_state);
    append_skin_render_items(&mut commands, &behind_notes_items);

    if !has_document {
        // デフォルトスキン: ボード背景・レーン背景を描画
        commands.push(DrawCommand::Rect { rect: board, color: Color::rgb(0.025, 0.025, 0.028) });
        commands.push(DrawCommand::Rect {
            rect: Rect { x: board.x - 0.006, y: board.y, width: 0.006, height: board.height },
            color: Color::rgb(0.18, 0.2, 0.21),
        });
        commands.push(DrawCommand::Rect {
            rect: Rect { x: board.x + board.width, y: board.y, width: 0.006, height: board.height },
            color: Color::rgb(0.18, 0.2, 0.21),
        });

        for (display_index, &lane) in active_lanes.iter().enumerate() {
            let lane_index = lane.index();
            let x = board.x + display_index as f32 * lane_width;
            let color = if display_index % 2 == 0 {
                Color::rgb(0.07, 0.075, 0.08)
            } else {
                Color::rgb(0.045, 0.05, 0.055)
            };
            commands.push(DrawCommand::Rect {
                rect: Rect { x, y: board.y, width: lane_width, height: board.height },
                color,
            });
            if let Some(color) = lane_flash_color(snapshot, lane) {
                commands.push(DrawCommand::Rect {
                    rect: Rect {
                        x: x + lane_width * 0.04,
                        y: board.y + board.height * 0.76,
                        width: lane_width * 0.92,
                        height: board.height * 0.18,
                    },
                    color,
                });
            }

            // ロングノート胴体はタップノートより先に描画する（端のキャップを上に重ねる）
            for body in snapshot.visible_long_notes.iter().filter(|body| body.lane == lane) {
                let top = play_object_y(board, snapshot.lift, body.tail_y);
                let bottom = play_object_y(board, snapshot.lift, body.head_y);
                commands.push(DrawCommand::Rect {
                    rect: Rect {
                        x: x + lane_width * 0.18,
                        y: top,
                        width: lane_width * 0.64,
                        height: (bottom - top).max(0.0),
                    },
                    color: long_note_body_color(body.mode),
                });
                // beatoraja の drawLongNote 同様、キャップは胴体側で描画する。
                // head キャップは押下中も判定ライン (head_y=0) に留まる。
                // LN モードは head キャップのみ、CN/HCN は tail キャップも描画する。
                // show_ln_tail_cap 有効時は LN モードでも tail キャップを描画する。
                let head_rect = Rect {
                    x: x + lane_width * 0.08,
                    y: note_rect_y(board, snapshot.lift, body.head_y),
                    width: lane_width * 0.84,
                    height: NOTE_HEIGHT,
                };
                push_ln_start_skin(skin_manifest, &mut commands, lane, head_rect);
                if (body.mode != LongNoteMode::Ln || snapshot.show_ln_tail_cap) && body.tail_y < 1.0
                {
                    let tail_rect = Rect {
                        x: x + lane_width * 0.08,
                        y: note_rect_y(board, snapshot.lift, body.tail_y),
                        width: lane_width * 0.84,
                        height: NOTE_HEIGHT,
                    };
                    push_ln_end_skin(skin_manifest, &mut commands, lane, tail_rect);
                }
            }

            for note in &snapshot.visible_notes[lane_index] {
                let y = note_rect_y(board, snapshot.lift, note.y);
                let rect = Rect {
                    x: x + lane_width * 0.08,
                    y,
                    width: lane_width * 0.84,
                    height: NOTE_HEIGHT,
                };
                match note.kind {
                    NoteVisualKind::LnStart => {
                        push_ln_start_skin(skin_manifest, &mut commands, lane, rect)
                    }
                    NoteVisualKind::LnEnd => {
                        push_ln_end_skin(skin_manifest, &mut commands, lane, rect)
                    }
                    NoteVisualKind::Tap => {
                        push_default_note_skin(skin_manifest, &mut commands, lane, rect)
                    }
                }
            }

            // Mine: 通常ノーツより前面に「警告ストライプ」テクスチャを重ねる。
            // 全レーン共通の DEFAULT_MINE_NOTE_TEXTURE を使い、レーン色付けは行わない。
            for mine in &snapshot.visible_mines[lane_index] {
                let y = note_rect_y(board, snapshot.lift, mine.y);
                let rect = Rect {
                    x: x + lane_width * 0.08,
                    y,
                    width: lane_width * 0.84,
                    height: NOTE_HEIGHT,
                };
                commands.push(DrawCommand::Image {
                    rect,
                    uv: UvRect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                    source_size: None,
                    texture: DEFAULT_MINE_NOTE_TEXTURE,
                    tint: Color::rgba(1.0, 1.0, 1.0, 1.0),
                    blend: BlendMode::Normal,
                    linear_filter: false,
                });
            }
        }

        push_receptors(
            skin_manifest,
            &mut commands,
            board,
            snapshot.lift,
            lane_width,
            active_lanes,
        );
        for bar in &snapshot.bar_lines {
            push_play_bar_line(
                &mut commands,
                skin,
                &skin_state,
                key_mode,
                board,
                snapshot.lift,
                bar,
                &snapshot.skin_offsets,
            );
        }
        push_play_aux_lines(
            &mut commands,
            skin,
            &skin_state,
            snapshot,
            key_mode,
            &snapshot.skin_offsets,
        );
        push_judge_line(skin_manifest, &mut commands, board, snapshot.lift);

        // SUDDEN+（レーンカバー）: レーン上部を覆う。ノーツは build_render_snapshot で
        // 既に可視域外が除外されているので、ここではカバー帯を描くだけ。
        // レーン背景が暗いグレーなので、カバーは判別しやすいように明確に黒で塗り、
        // 下端に視認用のハイライト帯を付ける。
        if snapshot.lane_cover > 0.0 {
            let cover_bottom = play_object_y(
                board,
                snapshot.lift,
                lane_cover_bottom_progress(snapshot.lane_cover, snapshot.lift),
            );
            let cover_height = (cover_bottom - board.y).max(0.0);
            commands.push(DrawCommand::Rect {
                rect: Rect { x: board.x, y: board.y, width: board.width, height: cover_height },
                color: Color::rgba(0.0, 0.0, 0.0, 1.0),
            });
            // SUDDEN+ の下端ラインを描いて境界を視認できるようにする。
            let line_height = 0.004_f32.min(cover_height);
            if line_height > 0.0 {
                commands.push(DrawCommand::Rect {
                    rect: Rect {
                        x: board.x,
                        y: cover_bottom - line_height,
                        width: board.width,
                        height: line_height,
                    },
                    color: Color::rgb(0.95, 0.65, 0.25),
                });
            }
        }
    } else {
        // beatoraja スキン: ロングノート胴体 → タップノートの順で note.dst のエリアに配置
        for bar in &snapshot.bar_lines {
            push_play_bar_line(
                &mut commands,
                skin,
                &skin_state,
                key_mode,
                board,
                snapshot.lift,
                bar,
                &snapshot.skin_offsets,
            );
        }
        push_play_aux_lines(
            &mut commands,
            skin,
            &skin_state,
            snapshot,
            key_mode,
            &snapshot.skin_offsets,
        );
        for body in &snapshot.visible_long_notes {
            if let Some(rect) =
                skin.note_body_rect(body.lane, key_mode, body.head_y, body.tail_y, &skin_state)
                && let Some(item) = skin.document_long_body_item(
                    body.lane,
                    key_mode,
                    rect,
                    body.mode,
                    body.body_state,
                    &skin_state,
                )
            {
                let item = skin.apply_play_skin_global_offset_to_item(item, &skin_state);
                append_skin_render_item(&mut commands, &item);
            }
            // beatoraja の drawLongNote 同様、キャップは胴体の上に重ねて描画する。
            // head キャップは押下中も判定ライン (head_y=0) に留まり描画され続ける。
            // LN モードは head キャップのみ、CN/HCN は tail キャップも描画する。
            // show_ln_tail_cap 有効時は LN モードでも tail キャップを描画する。
            let note_height = skin.document_note_height(body.lane, key_mode).unwrap_or(NOTE_HEIGHT);
            if let Some(rect) = skin.note_rect_for_progress(
                body.lane,
                key_mode,
                body.head_y,
                note_height,
                &skin_state,
            ) && let Some(item) =
                skin.document_ln_start_item(body.lane, key_mode, rect, body.mode)
            {
                let item = skin.apply_play_skin_global_offset_to_item(item, &skin_state);
                append_skin_render_item(&mut commands, &item);
            }
            if (body.mode != LongNoteMode::Ln || snapshot.show_ln_tail_cap)
                && body.tail_y < 1.0
                && let Some(rect) = skin.note_rect_for_progress(
                    body.lane,
                    key_mode,
                    body.tail_y,
                    note_height,
                    &skin_state,
                )
                && let Some(item) = skin.document_ln_end_item(body.lane, key_mode, rect, body.mode)
            {
                let item = skin.apply_play_skin_global_offset_to_item(item, &skin_state);
                append_skin_render_item(&mut commands, &item);
            }
        }
        for &lane in active_lanes {
            let lane_index = lane.index();
            let note_height = skin.document_note_height(lane, key_mode).unwrap_or(NOTE_HEIGHT);
            for note in &snapshot.visible_notes[lane_index] {
                let rect = if note.y < 0.0 {
                    skin.missed_note_rect_for_fall(
                        lane,
                        key_mode,
                        -note.y,
                        note_height,
                        &skin_state,
                    )
                } else {
                    skin.note_rect_for_progress(lane, key_mode, note.y, note_height, &skin_state)
                };
                if let Some(mut rect) = rect {
                    if key_mode == KeyMode::K9 {
                        let (scale_x, scale_y) = skin.document_note_expansion_scale(&skin_state);
                        let center_x = rect.x + rect.width / 2.0;
                        let center_y = rect.y + rect.height / 2.0;
                        rect.width *= scale_x;
                        rect.height *= scale_y;
                        rect.x = center_x - rect.width / 2.0;
                        rect.y = center_y - rect.height / 2.0;
                    }
                    let item = match note.kind {
                        NoteVisualKind::LnStart => {
                            skin.document_ln_start_item(lane, key_mode, rect, LongNoteMode::Ln)
                        }
                        NoteVisualKind::LnEnd => {
                            skin.document_ln_end_item(lane, key_mode, rect, LongNoteMode::Ln)
                        }
                        NoteVisualKind::Tap => skin.document_note_item(lane, key_mode, rect),
                    };
                    if let Some(item) = item {
                        let item = skin.apply_play_skin_global_offset_to_item(item, &skin_state);
                        append_skin_render_item(&mut commands, &item);
                    }
                }
            }
            // Mine ノーツ: スキン側に `note.mine` が定義されていればそれを使い、
            // 無ければ DEFAULT_MINE_NOTE_TEXTURE をフォールバックとして重ねる。
            for mine in &snapshot.visible_mines[lane_index] {
                if let Some(rect) =
                    skin.note_rect_for_progress(lane, key_mode, mine.y, note_height, &skin_state)
                {
                    if let Some(item) = skin.document_mine_item(lane, key_mode, rect) {
                        let item = skin.apply_play_skin_global_offset_to_item(item, &skin_state);
                        append_skin_render_item(&mut commands, &item);
                    } else {
                        commands.push(DrawCommand::Image {
                            rect,
                            uv: UvRect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                            source_size: None,
                            texture: DEFAULT_MINE_NOTE_TEXTURE,
                            tint: Color::rgba(1.0, 1.0, 1.0, 1.0),
                            blend: BlendMode::Normal,
                            linear_filter: false,
                        });
                    }
                }
            }
        }
    }

    // ノーツより前面の skin 要素（レーンカバー・枠・スコア等）をノーツの上に重ねる
    let front_notes_items = skin.apply_play_skin_global_offset(front_notes_items, &skin_state);
    append_skin_render_items(&mut commands, &front_notes_items);

    // 閉店の暗転 (`black` の a:0→255) 等、timer:3 を最前面に描画
    let failed_overlay_items =
        skin.apply_play_skin_global_offset(failed_overlay_items, &skin_state);
    append_skin_render_items(&mut commands, &failed_overlay_items);

    if !has_document {
        push_combo_panel(skin_manifest, &mut commands, snapshot.combo);
        push_default_play_skin(skin, &mut commands, snapshot);
        push_play_text(&text, &mut commands, snapshot);
        push_lane_text(&text, &mut commands, board, lane_width, active_lanes);
        push_judgement_history(&text, &mut commands, snapshot);
        // READY/GO オーバーレイはデフォルトスキン専用。
        // JSON skin 等は skin 側の演出を使うため描画しない。
        push_start_overlay(&text, &mut commands, snapshot);
        push_default_failed_overlay(&text, &mut commands, snapshot);
    }
    push_chart_text(&text, &mut commands, snapshot);
    push_scene_overlays(&mut commands, &snapshot.overlay);

    DrawPlan { clear: Color::rgb(0.0, 0.0, 0.0), commands }
}

pub(super) fn judge_starts_bomb(judge_index: Option<usize>, judge_timer_limit: usize) -> bool {
    judge_index.is_some_and(|judge| judge <= judge_timer_limit)
}

pub(super) fn play_command_capacity(snapshot: &RenderSnapshot, has_document: bool) -> usize {
    let visible_note_count: usize = snapshot.visible_notes.iter().map(Vec::len).sum();
    let visible_mine_count: usize = snapshot.visible_mines.iter().map(Vec::len).sum();
    let long_note_command_count = snapshot.visible_long_notes.len().saturating_mul(3);
    let skin_command_floor: usize = if has_document { 192 } else { 96 };
    skin_command_floor
        .saturating_add(snapshot.bar_lines.len())
        .saturating_add(visible_note_count)
        .saturating_add(visible_mine_count)
        .saturating_add(long_note_command_count)
}

pub(super) fn push_chart_text(
    text: &TextRenderer,
    commands: &mut Vec<DrawCommand>,
    snapshot: &RenderSnapshot,
) {
    if snapshot.chart_text.is_empty() {
        return;
    }
    commands.push(DrawCommand::Rect {
        rect: Rect { x: 0.18, y: 0.04, width: 0.64, height: 0.06 },
        color: Color::rgba(0.0, 0.0, 0.0, 0.55),
    });
    text.push_text(
        commands,
        &snapshot.chart_text,
        BitmapTextStyle { x: 0.2, y: 0.055, cell: 0.006, color: Color::rgb(0.95, 0.95, 0.9) },
    );
}
