use super::*;

pub fn build_render_snapshot(
    session: &GameSession,
    chart_now: TimeUs,
    recent_judgements: &[JudgementEvent],
    best_ex_score: Option<u32>,
) -> RenderSnapshot {
    build_render_snapshot_with_bga_frames(
        session,
        chart_now,
        recent_judgements,
        best_ex_score,
        &BgaFrameCatalog::new(),
    )
}

pub fn build_render_snapshot_with_bga_frames(
    session: &GameSession,
    chart_now: TimeUs,
    recent_judgements: &[JudgementEvent],
    best_ex_score: Option<u32>,
    bga_frames: &BgaFrameCatalog,
) -> RenderSnapshot {
    build_render_snapshot_with_target_and_bga_frames(
        session,
        chart_now,
        recent_judgements,
        best_ex_score,
        None,
        None,
        bga_frames,
    )
}

pub fn build_render_snapshot_with_target_and_bga_frames(
    session: &GameSession,
    chart_now: TimeUs,
    recent_judgements: &[JudgementEvent],
    best_ex_score: Option<u32>,
    best_ghost: Option<&[u8]>,
    target_ex_score: Option<u32>,
    bga_frames: &BgaFrameCatalog,
) -> RenderSnapshot {
    let cache = PlayRenderSnapshotCache::from_chart(&session.chart);
    build_render_snapshot_with_target_and_bga_frames_cached(
        session,
        chart_now,
        recent_judgements,
        best_ex_score,
        best_ghost,
        target_ex_score,
        bga_frames,
        &cache,
    )
}

pub fn build_render_snapshot_with_target_and_bga_frames_cached(
    session: &GameSession,
    chart_now: TimeUs,
    recent_judgements: &[JudgementEvent],
    best_ex_score: Option<u32>,
    best_ghost: Option<&[u8]>,
    target_ex_score: Option<u32>,
    bga_frames: &BgaFrameCatalog,
    cache: &PlayRenderSnapshotCache,
) -> RenderSnapshot {
    let projected_best_ex_score =
        best_ghost.map(|ghost| ghost_ex_score_at_progress(ghost, session.score.past_notes));
    let lane_render_now = lane_render_time(session, chart_now);
    let play_elapsed_time = if chart_now.0 < 0 { TimeUs(0) } else { chart_now };
    let gauge_graph_time_ms = (chart_now.0.max(0) / 1_000).clamp(0, i32::MAX as i64) as i32;
    // beatoraja の LaneRenderer と同じく、現在 BPM と緑数字は表示オフセットを
    // 適用したレーン時刻から求める。判定演出や BGA の時計には適用しない。
    let now_bpm = session.timing_map.bpm_at_time(lane_render_now) as f32;
    let cursor_tick = session.timing_map.time_to_tick_f64(scroll_render_time(lane_render_now));
    let scroll_multiplier = current_scroll_multiplier_from_segments(
        &cache.scroll_integral,
        &cache.speed_segments,
        cursor_tick,
    );
    let note_display_duration_ms = note_display_duration_ms(session, now_bpm, scroll_multiplier);
    let lane_cover = if session.lane_cover_visible {
        crate::config::play::clamp_lane_cover_for_lift(session.lane_cover, session.lift)
    } else {
        0.0
    };
    let adjusted_cover_progress = compute_adjusted_cover_progress(
        session.hidden_enabled,
        lane_cover,
        session.lift,
        session.hsfix_index,
        now_bpm,
        cache.max_bpm,
        session.chart.metadata.initial_bpm as f32,
    );
    let adjusted_rate = compute_adjusted_rate(
        session.hidden_enabled,
        session.lanecover_enabled,
        session.hsfix_index,
        now_bpm,
        cache.max_bpm,
        session.chart.metadata.initial_bpm as f32,
    );
    let gauge_graph_points = session
        .gauge
        .gauges
        .iter()
        .map(|gauge| ResultGaugeGraphPoint {
            time_ms: gauge_graph_time_ms,
            value: gauge.value,
            max: gauge.definition.max,
            border: gauge.definition.border,
            gauge_type: gauge.definition.gauge_type as i32,
        })
        .collect();
    let opponent = session.opponent_score.as_ref().zip(session.opponent_gauge.as_ref()).map(
        |(score, gauge)| OpponentRenderSnapshot {
            combo: score.combo,
            max_combo: score.max_combo,
            ex_score: score.ex_score(),
            total_notes: session.scored_total_notes,
            past_notes: score.past_notes,
            judge_counts: display_judge_counts_for_score(score),
            gauge: gauge.current().value,
            gauge_type: gauge.current().definition.gauge_type as i32,
            gauge_max: gauge.current().definition.max,
            gauge_border: gauge.current().definition.border,
            full_combo_elapsed_ms: session.opponent_full_combo_started_at.and_then(|started_at| {
                (chart_now.0 >= started_at.0).then_some(
                    ((chart_now.0 - started_at.0) / 1_000).clamp(0, i32::MAX as i64) as i32,
                )
            }),
            end_of_note_elapsed_ms: end_of_note_elapsed_ms(chart_now, cache.end_of_note_time),
            gauge_increase_elapsed_ms: optional_skin_timer_elapsed_ms(
                chart_now,
                session.opponent_gauge_increase_started_at,
            ),
            gauge_max_elapsed_ms: optional_skin_timer_elapsed_ms(
                chart_now,
                session.opponent_gauge_max_started_at,
            ),
        },
    );
    let mut snapshot = RenderSnapshot {
        time: chart_now,
        player_name: String::new(),
        current_fps: 0,
        play_elapsed_time,
        operating_time_ms: 0,
        skin_input: Default::default(),
        ready_elapsed_time: None,
        rhythm_timer_elapsed_ms: rhythm_timer_elapsed_ms(
            &session.timing_map,
            &session.chart.bar_lines,
            chart_now,
        ),
        quarter_note_elapsed_ms: quarter_note_elapsed_ms(
            &session.timing_map,
            &session.chart.bar_lines,
            chart_now,
        ),
        // session が構築できている時点で WAV 等のロードは完了している。
        resources_loaded: true,
        resource_load_progress: 1.0,
        duration: session.chart.end_time,
        title: session.chart.metadata.title.clone(),
        subtitle: session.chart.metadata.subtitle.clone(),
        artist: session.chart.metadata.artist.clone(),
        subartist: session.chart.metadata.subartist.clone(),
        genre: session.chart.metadata.genre.clone(),
        difficulty_name: session.chart.metadata.difficulty_name.clone(),
        judge_rank: session.chart.metadata.judge_rank,
        play_level: session.chart.metadata.play_level.clone(),
        arrange: "NORMAL".to_string(),
        arrange_2p: "NORMAL".to_string(),
        lane_shuffle_pattern: Vec::new(),
        target: String::new(),
        combo: session.display_combo(),
        max_combo: session.display_max_combo(),
        ex_score: session.score.ex_score(),
        total_notes: session.scored_total_notes,
        chart_total_gauge: gauge_total_for_chart(
            session.chart.metadata.total,
            session.scored_total_notes,
        ) as f32,
        past_notes: session.score.past_notes,
        judge_counts: display_judge_counts(session),
        fast_slow_counts: display_fast_slow_counts(session),
        gauge: session.gauge.current().value,
        gauge_type: session.gauge.current().definition.gauge_type as i32,
        gauge_graph_points,
        gauge_auto_shift: session.gauge.auto_shift,
        gauge_max: session.gauge.current().definition.max,
        gauge_border: session.gauge.current().definition.border,
        opponent,
        hispeed: session.hispeed,
        hispeed_mode_index: hispeed_mode_index(session.hispeed_mode),
        target_green_number: session.target_green_number,
        lift: session.lift,
        lane_cover,
        lane_cover_changing: session.lane_cover_changing,
        lanecover_enabled: session.lanecover_enabled,
        lift_enabled: session.lift_enabled,
        hidden_enabled: session.hidden_enabled,
        hispeed_auto_adjust: session.hispeed_auto_adjust,
        note_display_duration_ms,
        hidden_cover: session.hidden_cover,
        skin_offsets: skin_offsets_from_session(session, chart_now, play_elapsed_time),
        now_bpm,
        min_bpm: cache.min_bpm,
        max_bpm: cache.max_bpm,
        has_bga: session.chart.metadata.has_bga,
        has_bpm_stop: cache.has_bpm_stop,
        bga_enabled: session.bga_enabled,
        bga_base: session
            .bga_enabled
            .then(|| current_bga_frame(cache, chart_now, BgaEventKind::Base, bga_frames))
            .flatten(),
        bga_layer: session
            .bga_enabled
            .then(|| {
                current_keybound_bga_frame(session, cache, chart_now, bga_frames).or_else(|| {
                    current_bga_frame(cache, chart_now, BgaEventKind::Layer, bga_frames)
                })
            })
            .flatten(),
        bga_layer2: session
            .bga_enabled
            .then(|| current_bga_frame(cache, chart_now, BgaEventKind::Layer2, bga_frames))
            .flatten(),
        bga_poor: session
            .bga_enabled
            .then(|| {
                current_poor_bga_frame(
                    cache,
                    chart_now,
                    recent_judgements,
                    bga_frames,
                    session.poor_bga_duration_us,
                )
            })
            .flatten(),
        bga_stretch: session.bga_stretch,
        best_ex_score,
        projected_best_ex_score,
        target_ex_score,
        judge_timing_offset_ms: (session.offsets.visual_offset_us / 1_000) as i32,
        judge_timing_auto_adjust: session.input_offset_auto_adjust_enabled,
        main_bpm: session.chart.metadata.initial_bpm as f32,
        hsfix_index: session.hsfix_index,
        fs_threshold_ms: rm_skin_fs_threshold_ms(
            session.chart.metadata.judge_rank,
            session.primary_key_mode,
        ),
        adjusted_cover_progress,
        adjusted_rate,
        adjusted_rate_adot: adjusted_rate.map(|rate| (rate * 100.0).floor() as i32),
        judge_graph_density: Arc::clone(&cache.judge_graph_density),
        bpm_graph_segments: Arc::clone(&cache.bpm_graph_segments),
        autoplay: session.autoplay.as_ref().is_some_and(|autoplay| autoplay.is_full()),
        replay_playback: session.replay_player.is_some() && session.replay_lane_mask.is_none(),
        practice_mode: false,
        score_save_enabled: !session.autoplay.as_ref().is_some_and(|autoplay| autoplay.is_full())
            && !(session.replay_player.is_some() && session.replay_lane_mask.is_none()),
        course_stage: None,
        course_titles: Default::default(),
        table_text_primary: String::new(),
        table_text_secondary: String::new(),
        table_text_fallback: String::new(),
        key_mode: session.chart.metadata.key_mode,
        visible_notes: std::array::from_fn(|_| Vec::new()),
        visible_mines: std::array::from_fn(|_| Vec::new()),
        recent_inputs: session
            .recent_inputs
            .iter()
            .map(|input| DisplayInput { lane: input.lane, time: input.time })
            .collect(),
        recent_judgements: recent_judgements
            .iter()
            .map(|event| {
                let combo = if session.display_only_lane_mask[event.lane.index()] {
                    session.opponent_score.as_ref().map_or(0, |score| score.combo)
                } else {
                    session.display_combo()
                };
                display_judgement(event, combo)
            })
            .collect(),
        skin_events: Vec::new(),
        hit_error_ring: bmz_render::snapshot::HitErrorRingSnapshot {
            values: session.hit_error_ring.values,
            index: session.hit_error_ring.index,
        },
        full_combo_elapsed_ms: session.full_combo_started_at.and_then(|started_at| {
            (chart_now.0 >= started_at.0)
                .then_some(((chart_now.0 - started_at.0) / 1_000).clamp(0, i32::MAX as i64) as i32)
        }),
        end_of_note_elapsed_ms: end_of_note_elapsed_ms(chart_now, cache.end_of_note_time),
        fadeout_elapsed_ms: None,
        failed_elapsed_ms: None,
        music_end_elapsed_ms: None,
        gauge_increase_elapsed_ms: optional_skin_timer_elapsed_ms(
            chart_now,
            session.gauge_increase_started_at,
        ),
        gauge_max_elapsed_ms: optional_skin_timer_elapsed_ms(
            chart_now,
            session.gauge_max_started_at,
        ),
        bar_lines: Vec::new(),
        bpm_lines: Vec::new(),
        stop_lines: Vec::new(),
        time_lines: Vec::new(),
        visible_long_notes: Vec::new(),
        keyon_ms: lane_keyon_ms(session, chart_now, play_elapsed_time),
        keyoff_ms: lane_keyoff_ms(session, chart_now, play_elapsed_time),
        show_ln_tail_cap: session.show_ln_tail_cap,
        // beatoraja の TIMER_HCN_ACTIVE / TIMER_HCN_DAMAGE: HCN passing 中のみアクティブ。
        hcn_active_ms: std::array::from_fn(|lane_index| {
            session.lane_hcn_timer[lane_index].filter(|t| t.inclease).map(|t| {
                ((chart_now.0 - t.since.0) / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32
            })
        }),
        hcn_damage_ms: std::array::from_fn(|lane_index| {
            session.lane_hcn_timer[lane_index].filter(|t| !t.inclease).map(|t| {
                ((chart_now.0 - t.since.0) / 1_000).clamp(i32::MIN as i64, i32::MAX as i64) as i32
            })
        }),
        // beatoraja の TIMER_HOLD: LN ホールド中 (processing != null) のみアクティブ。
        hold_ms: std::array::from_fn(|lane_index| {
            session.judge.lanes[lane_index].active_long.map(|active| {
                ((chart_now.0 - active.started_at.0) / 1_000)
                    .clamp(i32::MIN as i64, i32::MAX as i64) as i32
            })
        }),
        overlay: OverlaySnapshot::default(),
        stagefile_background: false,
        stagefile_image_size: None,
        backbmp_background: false,
        chart_text: bmz_chart::text::chart_text_at_time(&session.chart.text_events, chart_now)
            .to_string(),
    };

    // beatoraja の LaneRenderer と同様、playstart 中 (lane_render_now < 0) は
    // レーンスクロールの基準時刻を 0 に固定する。音声の chart_zero_time は
    // マイナスのまま維持し、見た目だけ clamp する。
    // レーン描画時刻が 0 未満の間は譜面オブジェクトを出さない (beatoraja の
    // TIMER_PLAY 開始前と同じ)。
    let scroll_time = scroll_render_time(lane_render_now);
    if lane_render_now.0 >= 0 {
        let scroll = ScrollContext::new(session, cache);
        let cursor_tick = scroll.cursor_tick(scroll_time);
        let simple_tick_upper_bound = scroll.simple_tick_upper_bound(cursor_tick);
        let note_lower_time = (snapshot.key_mode != KeyMode::K9).then_some(lane_render_now);

        for lane in Lane::ALL {
            for note in visible_lane_notes(
                session.chart.notes_for_lane(lane),
                note_lower_time,
                simple_tick_upper_bound,
            ) {
                let processed_judge = session.judge.judged_notes.get(&note.id).copied();
                let falling_pms_poor = snapshot.key_mode == KeyMode::K9
                    && note.kind == NoteKind::Tap
                    && processed_judge == Some(Judge::Poor)
                    && note.time < lane_render_now;
                if note.time < lane_render_now && !falling_pms_poor {
                    continue;
                }
                match note.kind {
                    NoteKind::Invisible => continue,
                    NoteKind::Mine => {
                        if let Some(y) = scroll.note_y(note.time, cursor_tick) {
                            snapshot.visible_mines[lane.index()].push(VisibleMine {
                                lane,
                                time: note.time,
                                y,
                                damage: note.damage.unwrap_or(0),
                            });
                        }
                    }
                    // LN START/END のキャップは beatoraja の drawLongNote 同様、
                    // visible_long_notes 側でロングノート本体と一緒に描画する。
                    NoteKind::LongStart | NoteKind::LongEnd => continue,
                    NoteKind::Tap => {
                        let y = if falling_pms_poor {
                            Some(-pms_missed_note_fall_progress(
                                &session.timing_map,
                                note.tick,
                                note.time,
                                session.judge.window_set.note.bad_slow_us.max(0),
                                lane_render_now,
                            ))
                            .filter(|fall| *fall >= -1.0)
                        } else {
                            scroll.note_y(note.time, cursor_tick)
                        };
                        if let Some(y) = y {
                            snapshot.visible_notes[lane.index()].push(VisibleNote {
                                lane,
                                time: note.time,
                                y,
                                kind: NoteVisualKind::Tap,
                                processed_judge,
                            });
                        }
                    }
                }
            }
        }

        for bar in visible_bar_lines(
            &session.chart.bar_lines,
            scroll_time,
            simple_tick_upper_bound.map(|upper| (cursor_tick, upper)),
        ) {
            if let Some(y) = scroll.note_y(bar.time, cursor_tick) {
                snapshot.bar_lines.push(VisibleBarLine { time: bar.time, y });
            }
        }

        for event in visible_timing_events(
            &session.chart.timing_events,
            scroll_time,
            simple_tick_upper_bound.map(|upper| (cursor_tick, upper)),
        ) {
            let Some(y) = scroll.note_y(event.time, cursor_tick) else {
                continue;
            };
            let line = VisibleBarLine { time: event.time, y };
            match event.kind {
                TimingEventKind::BpmChange { .. } => snapshot.bpm_lines.push(line),
                TimingEventKind::Stop { .. } => snapshot.stop_lines.push(line),
            }
        }

        let end_second = (session.chart.end_time.0.max(0) / 1_000_000).min(21_600);
        let seconds = visible_time_line_seconds(
            &session.timing_map,
            end_second,
            scroll_time,
            simple_tick_upper_bound,
        );
        for second in seconds {
            let time = TimeUs(second.saturating_mul(1_000_000));
            if let Some(y) = scroll.note_y(time, cursor_tick) {
                snapshot.time_lines.push(VisibleBarLine { time, y });
            }
        }

        for (pair_index, long) in visible_long_notes(
            &session.chart.long_notes,
            &cache.long_note_prefix_max_end_times,
            scroll_time,
            simple_tick_upper_bound.map(|upper| (cursor_tick, upper)),
        ) {
            let head = scroll.note_progress(long.start_time, cursor_tick);
            let tail = scroll.note_progress(long.end_time, cursor_tick);
            // 終端が判定ラインを過ぎた、または始端が画面上端より奥なら非表示。
            // lane cover は前面描画で隠すだけで、ノーツのカリング範囲は変えない。
            if tail < 0.0 || head > 1.0 {
                continue;
            }
            let mode = long.mode.unwrap_or(session.chart.metadata.long_note_mode);
            // beatoraja drawLongNote の longImage 選択に対応する状態判定:
            // processing == pair → Processing。HCN は passing 中 (区間内かつ始端判定
            // 済み) なら押下状態で HcnActive / HcnDamage。それ以外は Inactive。
            // 物理キー状態は processing 判定には使わない。
            let lane_index = long.lane.index();
            let is_processing = session.judge.lanes[lane_index]
                .active_long
                .is_some_and(|active| active.pair_index == pair_index);
            let body_state = if is_processing {
                LongBodyState::Processing
            } else if mode == LongNoteMode::Hcn
                && chart_now.0 >= long.start_time.0
                && chart_now.0 < long.end_time.0
                && let Some(timer) = session.lane_hcn_timer[lane_index]
            {
                if timer.inclease { LongBodyState::HcnActive } else { LongBodyState::HcnDamage }
            } else {
                LongBodyState::Inactive
            };
            snapshot.visible_long_notes.push(VisibleLongNote {
                lane: long.lane,
                mode,
                head_y: head.clamp(0.0, 1.0),
                tail_y: tail.clamp(0.0, 1.0),
                body_state,
            });
        }
    }

    snapshot
}

pub fn update_render_snapshot_play_options(
    snapshot: &mut RenderSnapshot,
    session: &GameSession,
    chart_now: TimeUs,
) {
    snapshot.hispeed = session.hispeed;
    snapshot.hispeed_mode_index = hispeed_mode_index(session.hispeed_mode);
    snapshot.target_green_number = session.target_green_number;
    snapshot.lift = session.lift;
    snapshot.lane_cover = if session.lane_cover_visible {
        crate::config::play::clamp_lane_cover_for_lift(session.lane_cover, session.lift)
    } else {
        0.0
    };
    snapshot.lane_cover_changing = session.lane_cover_changing;
    snapshot.lanecover_enabled = session.lanecover_enabled;
    snapshot.lift_enabled = session.lift_enabled;
    snapshot.hidden_enabled = session.hidden_enabled;
    snapshot.hispeed_auto_adjust = session.hispeed_auto_adjust;
    let lane_render_now = lane_render_time(session, chart_now);
    snapshot.note_display_duration_ms = note_display_duration_ms(
        session,
        session.timing_map.bpm_at_time(lane_render_now) as f32,
        current_scroll_multiplier(&session.chart, &session.timing_map, lane_render_now),
    );
    snapshot.hidden_cover = session.hidden_cover;
}

pub(super) fn hispeed_mode_index(mode: bmz_gameplay::session::HispeedMode) -> i32 {
    match mode {
        bmz_gameplay::session::HispeedMode::Normal => 0,
        bmz_gameplay::session::HispeedMode::Floating => 1,
    }
}
