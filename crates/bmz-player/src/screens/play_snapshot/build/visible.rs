use super::*;

pub(super) fn populate_visible_playfield(
    snapshot: &mut RenderSnapshot,
    session: &GameSession,
    chart_now: TimeUs,
    cache: &PlayRenderSnapshotCache,
    lane_render_now: TimeUs,
) {
    // playstart 中は見た目の基準だけ 0 に固定し、音声時刻は負値のまま維持する。
    if lane_render_now.0 < 0 {
        return;
    }
    let scroll_time = scroll_render_time(lane_render_now);
    let scroll = ScrollContext::new(session, cache);
    let cursor_tick = scroll.cursor_tick(scroll_time);
    let tick_upper_bound = scroll.simple_tick_upper_bound(cursor_tick);

    populate_visible_notes(
        snapshot,
        session,
        lane_render_now,
        &scroll,
        cursor_tick,
        tick_upper_bound,
    );
    populate_visible_guide_lines(
        snapshot,
        session,
        scroll_time,
        &scroll,
        cursor_tick,
        tick_upper_bound,
    );
    populate_visible_long_notes(
        snapshot,
        session,
        chart_now,
        cache,
        scroll_time,
        &scroll,
        cursor_tick,
        tick_upper_bound,
    );
}

fn populate_visible_notes(
    snapshot: &mut RenderSnapshot,
    session: &GameSession,
    lane_render_now: TimeUs,
    scroll: &ScrollContext<'_>,
    cursor_tick: f64,
    tick_upper_bound: Option<f64>,
) {
    let note_lower_time = (snapshot.key_mode != KeyMode::K9).then_some(lane_render_now);
    for lane in Lane::ALL {
        for note in visible_lane_notes(
            session.chart.notes_for_lane(lane),
            note_lower_time,
            tick_upper_bound,
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
                NoteKind::Invisible | NoteKind::LongStart | NoteKind::LongEnd => {}
                NoteKind::Mine => {
                    if let Some(y) = scroll.note_y(note.time, cursor_tick) {
                        snapshot.visible_mines[lane.index()].push(VisibleMine {
                            lane,
                            time: note.time,
                            y,
                            damage: note.damage.unwrap_or(0.0),
                        });
                    }
                }
                NoteKind::Tap => {
                    let y = visible_tap_y(
                        session,
                        scroll,
                        cursor_tick,
                        note,
                        lane_render_now,
                        falling_pms_poor,
                    );
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
}

fn visible_tap_y(
    session: &GameSession,
    scroll: &ScrollContext<'_>,
    cursor_tick: f64,
    note: &NoteEvent,
    lane_render_now: TimeUs,
    falling_pms_poor: bool,
) -> Option<f32> {
    if falling_pms_poor {
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
    }
}

fn populate_visible_guide_lines(
    snapshot: &mut RenderSnapshot,
    session: &GameSession,
    scroll_time: TimeUs,
    scroll: &ScrollContext<'_>,
    cursor_tick: f64,
    tick_upper_bound: Option<f64>,
) {
    let tick_range = tick_upper_bound.map(|upper| (cursor_tick, upper));
    for bar in visible_bar_lines(&session.chart.bar_lines, scroll_time, tick_range) {
        if let Some(y) = scroll.note_y(bar.time, cursor_tick) {
            snapshot.bar_lines.push(VisibleBarLine { time: bar.time, y });
        }
    }
    for event in visible_timing_events(&session.chart.timing_events, scroll_time, tick_range) {
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
    for second in
        visible_time_line_seconds(&session.timing_map, end_second, scroll_time, tick_upper_bound)
    {
        let time = TimeUs(second.saturating_mul(1_000_000));
        if let Some(y) = scroll.note_y(time, cursor_tick) {
            snapshot.time_lines.push(VisibleBarLine { time, y });
        }
    }
}

fn populate_visible_long_notes(
    snapshot: &mut RenderSnapshot,
    session: &GameSession,
    chart_now: TimeUs,
    cache: &PlayRenderSnapshotCache,
    scroll_time: TimeUs,
    scroll: &ScrollContext<'_>,
    cursor_tick: f64,
    tick_upper_bound: Option<f64>,
) {
    let tick_range = tick_upper_bound.map(|upper| (cursor_tick, upper));
    for (pair_index, long) in visible_long_notes(
        &session.chart.long_notes,
        &cache.long_note_prefix_max_end_times,
        scroll_time,
        tick_range,
    ) {
        let head = scroll.note_progress(long.start_time, cursor_tick);
        let tail = scroll.note_progress(long.end_time, cursor_tick);
        if tail < 0.0 || head > 1.0 {
            continue;
        }
        let mode = long.mode.unwrap_or(session.chart.metadata.long_note_mode);
        snapshot.visible_long_notes.push(VisibleLongNote {
            lane: long.lane,
            mode,
            head_y: head.clamp(0.0, 1.0),
            tail_y: tail.clamp(0.0, 1.0),
            body_state: long_body_state(session, chart_now, pair_index, long, mode),
        });
    }
}

fn long_body_state(
    session: &GameSession,
    chart_now: TimeUs,
    pair_index: usize,
    long: &bmz_chart::model::LongNotePair,
    mode: LongNoteMode,
) -> LongBodyState {
    let lane_index = long.lane.index();
    if session.judge.lanes[lane_index]
        .active_long
        .is_some_and(|active| active.pair_index == pair_index)
    {
        return LongBodyState::Processing;
    }
    if mode == LongNoteMode::Hcn
        && chart_now.0 >= long.start_time.0
        && chart_now.0 < long.end_time.0
        && let Some(timer) = session.lane_hcn_timer[lane_index]
    {
        if timer.inclease { LongBodyState::HcnActive } else { LongBodyState::HcnDamage }
    } else {
        LongBodyState::Inactive
    }
}
