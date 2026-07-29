use std::collections::HashMap;
use std::sync::Arc;

use bmz_chart::hash::compute_chart_identity;
use bmz_chart::model::{ChartMetadata, NoteEvent, NoteKind, PlayableChart};
use bmz_core::ids::NoteId;
use bmz_core::input::{InputDeviceKind, InputEvent, InputKind, InputSource};
use bmz_core::judge::{Judge, TimingSide};
use bmz_core::lane::{KeyMode, Lane};
use bmz_core::time::TimeUs;
use bmz_gameplay::judge::model::JudgementEvent;
use bmz_render::skin::{
    SkinDocument, SkinDocumentRenderExt, SkinDocumentTexture, SkinDrawState, SkinImageSize,
    SkinRenderItem, SkinTextureId,
};

use crate::config::profile_config::ProfileConfig;
use crate::screens::play_session::{PlaySessionOptions, build_game_session};

use super::*;

fn approx_eq(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.0001
}

#[test]
fn rhythm_timer_resets_at_bar_lines_and_integrates_bpm_and_stops() {
    use bmz_chart::timing::{TickTimingEvent, TickTimingEventKind, build_timing_map};

    let constant = build_timing_map(120.0, Vec::new());
    let bars = vec![
        BarLine { measure: 0, tick: ChartTick(0), time: TimeUs(0) },
        BarLine { measure: 1, tick: ChartTick(3_840), time: TimeUs(2_000_000) },
    ];
    assert_eq!(rhythm_timer_elapsed_ms(&constant, &bars, TimeUs(-1)), None);
    assert_eq!(rhythm_timer_elapsed_ms(&constant, &bars, TimeUs(1_000_000)), Some(2_000));
    assert_eq!(rhythm_timer_elapsed_ms(&constant, &bars, TimeUs(2_500_000)), Some(1_000));

    let bpm_change = build_timing_map(
        120.0,
        vec![TickTimingEvent { tick: ChartTick(960), kind: TickTimingEventKind::SetBpm(60.0) }],
    );
    assert_eq!(rhythm_timer_elapsed_ms(&bpm_change, &[], TimeUs(1_500_000)), Some(2_000));

    let with_stop = build_timing_map(
        120.0,
        vec![TickTimingEvent {
            tick: ChartTick(960),
            kind: TickTimingEventKind::StopRaw { value: 96 },
        }],
    );
    assert_eq!(rhythm_timer_elapsed_ms(&with_stop, &[], TimeUs(1_500_000)), Some(3_000));
}

#[test]
fn quarter_note_timer_tracks_real_milliseconds_since_latest_quarter() {
    use bmz_chart::timing::{TickTimingEvent, TickTimingEventKind, build_timing_map};

    let bars = vec![
        BarLine { measure: 0, tick: ChartTick(0), time: TimeUs(0) },
        BarLine { measure: 1, tick: ChartTick(3_840), time: TimeUs(2_000_000) },
    ];
    let constant = build_timing_map(120.0, Vec::new());
    assert_eq!(quarter_note_elapsed_ms(&constant, &bars, TimeUs(-1)), None);
    assert_eq!(quarter_note_elapsed_ms(&constant, &bars, TimeUs(505_000)), Some(5));
    assert_eq!(quarter_note_elapsed_ms(&constant, &bars, TimeUs(1_158_000)), Some(158));

    let bpm_change = build_timing_map(
        120.0,
        vec![TickTimingEvent { tick: ChartTick(960), kind: TickTimingEventKind::SetBpm(60.0) }],
    );
    assert_eq!(quarter_note_elapsed_ms(&bpm_change, &bars, TimeUs(1_505_000)), Some(5));
}

#[test]
fn pms_missed_note_fall_waits_for_late_bad_and_uses_no_speed_rate() {
    use bmz_chart::timing::{TickTimingEvent, TickTimingEventKind, build_timing_map};

    let constant = build_timing_map(120.0, Vec::new());
    assert!(approx_eq(
        pms_missed_note_fall_progress(&constant, ChartTick(0), TimeUs(0), 200_000, TimeUs(200_000),),
        0.0,
    ));
    assert!(approx_eq(
        pms_missed_note_fall_progress(&constant, ChartTick(0), TimeUs(0), 200_000, TimeUs(700_000),),
        0.25,
    ));

    let with_stop = build_timing_map(
        120.0,
        vec![TickTimingEvent {
            tick: ChartTick(0),
            kind: TickTimingEventKind::StopRaw { value: 96 },
        }],
    );
    assert!(approx_eq(
        pms_missed_note_fall_progress(
            &with_stop,
            ChartTick(0),
            TimeUs(0),
            200_000,
            TimeUs(1_200_000),
        ),
        0.0,
    ));
    assert!(approx_eq(
        pms_missed_note_fall_progress(
            &with_stop,
            ChartTick(0),
            TimeUs(0),
            200_000,
            TimeUs(1_700_000),
        ),
        0.25,
    ));
}

#[test]
fn simple_scroll_ranges_skip_past_chart_objects() {
    let bars = vec![
        BarLine { measure: 0, tick: ChartTick(0), time: TimeUs(0) },
        BarLine { measure: 1, tick: ChartTick(960), time: TimeUs(500_000) },
        BarLine { measure: 2, tick: ChartTick(1_920), time: TimeUs(1_000_000) },
        BarLine { measure: 3, tick: ChartTick(2_880), time: TimeUs(1_500_000) },
        BarLine { measure: 4, tick: ChartTick(3_840), time: TimeUs(2_000_000) },
    ];
    let visible = visible_bar_lines(&bars, TimeUs(1_000_000), Some((1_920.0, 2_880.0)));
    assert_eq!(visible.iter().map(|bar| bar.tick.0).collect::<Vec<_>>(), vec![1_920, 2_880]);

    let events = vec![
        bmz_chart::model::TimingEvent {
            tick: ChartTick(0),
            time: TimeUs(0),
            kind: TimingEventKind::BpmChange { bpm: 120.0 },
        },
        bmz_chart::model::TimingEvent {
            tick: ChartTick(1_920),
            time: TimeUs(1_000_000),
            kind: TimingEventKind::Stop { duration_us: 100_000 },
        },
        bmz_chart::model::TimingEvent {
            tick: ChartTick(2_880),
            time: TimeUs(1_500_000),
            kind: TimingEventKind::BpmChange { bpm: 150.0 },
        },
        bmz_chart::model::TimingEvent {
            tick: ChartTick(3_840),
            time: TimeUs(2_000_000),
            kind: TimingEventKind::Stop { duration_us: 100_000 },
        },
    ];
    let visible = visible_timing_events(&events, TimeUs(1_000_000), Some((1_920.0, 2_880.0)));
    assert_eq!(visible.iter().map(|event| event.tick.0).collect::<Vec<_>>(), vec![1_920, 2_880]);
}

#[test]
fn simple_scroll_long_note_range_keeps_crossing_notes_without_past_scan() {
    use bmz_chart::model::{LongNotePair, LongNoteStyle};

    let long = |start_tick: u64, end_tick: u64| LongNotePair {
        lane: Lane::Key1,
        style: LongNoteStyle::ChannelPair,
        mode: None,
        start_note_id: NoteId(start_tick as u32),
        end_note_id: NoteId(end_tick as u32),
        start_tick: ChartTick(start_tick),
        end_tick: ChartTick(end_tick),
        start_time: TimeUs(start_tick as i64),
        end_time: TimeUs(end_tick as i64),
        sound: None,
    };
    let longs = vec![long(0, 960), long(480, 3_000), long(1_920, 2_880), long(4_000, 5_000)];
    let prefix = [960, 3_000, 3_000, 5_000];
    let visible = visible_long_notes(&longs, &prefix, TimeUs(1_920), Some((1_920.0, 3_000.0)))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(visible, vec![1, 2]);
}

#[test]
fn simple_scroll_time_line_range_uses_visible_seconds_only() {
    use bmz_chart::timing::build_timing_map;

    let timing_map = build_timing_map(120.0, Vec::new());
    let upper_tick = timing_map.time_to_tick_f64(TimeUs(184_000_000));
    assert_eq!(
        visible_time_line_seconds(&timing_map, 600, TimeUs(180_100_000), Some(upper_tick)),
        181..185
    );
    // SCROLL/SPEED path keeps all candidates because negative SCROLL can make
    // past lines visible again.
    assert_eq!(visible_time_line_seconds(&timing_map, 600, TimeUs(180_100_000), None), 1..601);
}

#[test]
fn simple_scroll_time_line_range_handles_stop_tick_boundaries() {
    use bmz_chart::timing::{TickTimingEvent, TickTimingEventKind, build_timing_map};

    let timing_map = build_timing_map(
        120.0,
        vec![TickTimingEvent {
            tick: ChartTick(1_920),
            kind: TickTimingEventKind::StopRaw { value: 192 },
        }],
    );
    let during_stop_tick = timing_map.time_to_tick_f64(TimeUs(1_050_000));
    assert!(
        timing_map.time_to_tick_f64(TimeUs(1_000_000))
            <= timing_map.time_to_tick_f64(TimeUs(1_050_000))
    );
    assert!(
        timing_map.time_to_tick_f64(TimeUs(1_050_000))
            <= timing_map.time_to_tick_f64(TimeUs(1_100_000))
    );

    let visible = visible_time_line_seconds(&timing_map, 5, TimeUs(0), Some(during_stop_tick));
    for second in 1..=5 {
        let tick = timing_map.time_to_tick_f64(TimeUs(second * 1_000_000));
        assert_eq!(visible.contains(&second), tick <= during_stop_tick);
    }
}

#[test]
fn fast_slow_filter_suppresses_timing_ms_only_for_threshold_scope() {
    use crate::config::profile_config::FastSlowDisplayScope;
    let judgement = |judge, delta_us| {
        display_judgement(
            &JudgementEvent {
                note_id: Some(NoteId(1)),
                lane: Lane::Key1,
                judge,
                side: TimingSide::Fast,
                delta: TimeUs(delta_us),
                time: TimeUs(1_000),
                affects_score: true,
            },
            1,
        )
    };

    // ThresholdMs: 閾値内 (|delta| < 5ms) は side だけでなく ±ms 表示も隠す。
    let mut snapshot = RenderSnapshot {
        recent_judgements: vec![judgement(Judge::Great, -2_000)],
        ..RenderSnapshot::default()
    };
    apply_fast_slow_display_filter(&mut snapshot, 5, FastSlowDisplayScope::ThresholdMs);
    assert_eq!(snapshot.recent_judgements[0].side, None);
    assert!(snapshot.recent_judgements[0].timing_ms_suppressed);

    // ThresholdMs: 閾値外は両方表示。
    let mut snapshot = RenderSnapshot {
        recent_judgements: vec![judgement(Judge::Great, -8_000)],
        ..RenderSnapshot::default()
    };
    apply_fast_slow_display_filter(&mut snapshot, 5, FastSlowDisplayScope::ThresholdMs);
    assert_eq!(snapshot.recent_judgements[0].side, Some(TimingSide::Fast));
    assert!(!snapshot.recent_judgements[0].timing_ms_suppressed);

    // Auto: 通常プレイの PGREAT は side を隠すが ±ms 表示は beatoraja 準拠で隠さない。
    let mut snapshot = RenderSnapshot {
        recent_judgements: vec![judgement(Judge::PGreat, -2_000)],
        ..RenderSnapshot::default()
    };
    apply_fast_slow_display_filter(&mut snapshot, 5, FastSlowDisplayScope::Auto);
    assert_eq!(snapshot.recent_judgements[0].side, None);
    assert!(!snapshot.recent_judgements[0].timing_ms_suppressed);

    // Replay でも Auto は GREAT 以下表示として扱い、PGREAT side は隠す。
    let mut replay_snapshot = RenderSnapshot {
        replay_playback: true,
        recent_judgements: vec![judgement(Judge::PGreat, -2_000)],
        ..RenderSnapshot::default()
    };
    apply_fast_slow_display_filter(&mut replay_snapshot, 5, FastSlowDisplayScope::Auto);
    assert_eq!(replay_snapshot.recent_judgements[0].side, None);
    assert!(!replay_snapshot.recent_judgements[0].timing_ms_suppressed);

    // ThresholdMs + 0ms は全判定表示なので、リプレイ PGREAT の FAST/SLOW も保持する。
    let mut replay_all_snapshot = RenderSnapshot {
        replay_playback: true,
        recent_judgements: vec![judgement(Judge::PGreat, -2_000)],
        ..RenderSnapshot::default()
    };
    apply_fast_slow_display_filter(&mut replay_all_snapshot, 0, FastSlowDisplayScope::ThresholdMs);
    assert_eq!(replay_all_snapshot.recent_judgements[0].side, Some(TimingSide::Fast));
    assert!(!replay_all_snapshot.recent_judgements[0].timing_ms_suppressed);
}

#[test]
fn bga_texture_ids_do_not_overlap_beatoraja_skin_ranges() {
    // skin_loader::SkinKind::first_texture_id と同じ割当。
    const SELECT_SKIN_BASE: u32 = 20_000;
    const RESULT_SKIN_BASE: u32 = 30_000;
    // result スキンが数千 PNG あっても BGA 帯に届かないこと。
    const MAX_RESULT_SKIN_TEXTURES: u32 = 10_000;

    assert!(CHART_BGA_TEXTURE_BASE >= RESULT_SKIN_BASE + MAX_RESULT_SKIN_TEXTURES);
    assert!(CHART_BGA_TEXTURE_BASE > SELECT_SKIN_BASE);
    assert_eq!(bga_texture_id(BgaAssetId(0)), CHART_BGA_TEXTURE_BASE);
}

#[test]
fn display_duration_uses_current_bpm_and_absolute_lane_range() {
    assert_eq!(display_duration_ms_for_bpm_hispeed(120.0, 1.0, 0.0, 0.0, 1.0).round() as i32, 2000);
    assert_eq!(display_duration_ms_for_bpm_hispeed(240.0, 1.0, 0.0, 0.0, 1.0).round() as i32, 1000);
    assert_eq!(display_duration_ms_for_bpm_hispeed(88.0, 2.75, 0.0, 0.0, 1.0).round() as i32, 992);
    assert_eq!(display_duration_ms_for_bpm_hispeed(88.0, 2.75, 0.59, 0.0, 1.0).round() as i32, 407);
    assert_eq!(
        display_duration_ms_for_bpm_hispeed(120.0, 1.0, 0.25, 0.2, 1.0).round() as i32,
        1100
    );
    assert_eq!(display_duration_ms_for_bpm_hispeed(120.0, 1.0, 0.0, 0.0, 2.0).round() as i32, 1000);
}

#[test]
fn build_render_snapshot_filters_visible_notes_and_formats_judgements() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;
    let judgements = vec![JudgementEvent {
        note_id: Some(NoteId(1)),
        lane: Lane::Key1,
        judge: Judge::EmptyPoor,
        side: TimingSide::Slow,
        delta: TimeUs(5_000),
        time: TimeUs(1_000),
        affects_score: true,
    }];

    let snapshot = build_render_snapshot(&session, TimeUs(0), &judgements, None);

    assert_eq!(snapshot.combo, 0);
    assert_eq!(snapshot.max_combo, 0);
    assert_eq!(snapshot.ex_score, 0);
    assert_eq!(snapshot.total_notes, 1);
    assert_eq!(snapshot.past_notes, 0);
    assert!(snapshot.recent_inputs.is_empty());
    assert_eq!(snapshot.visible_notes[Lane::Key1.index()].len(), 1);
    assert_eq!(snapshot.visible_notes[Lane::Key1.index()][0].y, 0.5);
    assert_eq!(snapshot.recent_judgements[0].lane, Lane::Key1);
    assert_eq!(snapshot.recent_judgements[0].text, "EMPTY POOR SLOW");
    assert_eq!(snapshot.recent_judgements[0].delta_us, 5_000);
}

#[test]
fn visual_offset_moves_lane_objects_without_advancing_effect_timers() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;
    session.offsets.visual_offset_us = 500_000;
    session.full_combo_started_at = Some(TimeUs(0));
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(0));

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(snapshot.time, TimeUs(0));
    assert_eq!(snapshot.play_elapsed_time, TimeUs(0));
    assert_eq!(snapshot.full_combo_elapsed_ms, Some(0));
    assert_eq!(snapshot.keyon_ms[Lane::Key1.index()], Some(0));
    let y = snapshot.visible_notes[Lane::Key1.index()][0].y;
    assert!((y - 0.25).abs() < 1e-3, "expected lane y ~0.25, got {y}");
}

#[test]
fn judged_notes_remain_visible_until_their_scheduled_time() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    let outcome = session.judge.process_input(
        &session.chart,
        InputEvent {
            lane: Lane::Key1,
            kind: InputKind::Press,
            time: TimeUs(990_000),
            source: InputSource::Human,
            device_kind: InputDeviceKind::Keyboard,
            scratch_direction: None,
        },
    );
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(session.judge.lanes[Lane::Key1.index()].next_note_index, 1);

    let before_scheduled_time = build_render_snapshot(&session, TimeUs(990_000), &[], None);
    assert_eq!(before_scheduled_time.visible_notes[Lane::Key1.index()].len(), 1);
    assert_eq!(
        before_scheduled_time.visible_notes[Lane::Key1.index()][0].processed_judge,
        Some(outcome.events[0].judge)
    );

    let after_scheduled_time = build_render_snapshot(&session, TimeUs(1_000_001), &[], None);
    assert!(after_scheduled_time.visible_notes[Lane::Key1.index()].is_empty());
}

#[test]
fn build_render_snapshot_culls_past_and_far_future_notes() {
    let mut chart = chart();
    chart.lane_notes[Lane::Key1.index()] = vec![
        tap_note(1, Lane::Key1, 960, 500_000),
        tap_note(2, Lane::Key1, 1_920, 1_000_000),
        tap_note(3, Lane::Key1, 3_840, 2_000_000),
        tap_note(4, Lane::Key1, 7_680, 4_000_000),
    ];
    chart.total_notes = 4;
    chart.end_time = TimeUs(4_000_000);
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    let snapshot = build_render_snapshot(&session, TimeUs(1_000_000), &[], None);
    let notes = &snapshot.visible_notes[Lane::Key1.index()];

    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].time, TimeUs(1_000_000));
    assert_eq!(notes[0].y, 0.0);
    assert_eq!(notes[1].time, TimeUs(2_000_000));
    assert_eq!(notes[1].y, 0.5);
}

#[test]
fn build_render_snapshot_keeps_k9_poor_note_while_falling() {
    let mut chart = chart();
    chart.metadata.key_mode = KeyMode::K9;
    chart.lane_notes[Lane::Key1.index()] = vec![tap_note(1, Lane::Key1, 0, 0)];
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;
    session.judge.judged_notes.insert(NoteId(1), Judge::Poor);
    let bad_slow_us = session.judge.window_set.note.bad_slow_us.max(0);

    let falling =
        build_render_snapshot(&session, TimeUs(bad_slow_us.saturating_add(500_000)), &[], None);
    let notes = &falling.visible_notes[Lane::Key1.index()];
    assert_eq!(notes.len(), 1);
    assert!(notes[0].y < 0.0);

    let finished =
        build_render_snapshot(&session, TimeUs(bad_slow_us.saturating_add(2_500_000)), &[], None);
    assert!(finished.visible_notes[Lane::Key1.index()].is_empty());
}

#[test]
fn end_of_note_timer_counts_from_last_note_time() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let session = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

    let before = build_render_snapshot(&session, TimeUs(999_999), &[], None);
    let at_end = build_render_snapshot(&session, TimeUs(1_000_000), &[], None);
    let after = build_render_snapshot(&session, TimeUs(1_250_000), &[], None);

    assert_eq!(before.end_of_note_elapsed_ms, None);
    assert_eq!(at_end.end_of_note_elapsed_ms, Some(0));
    assert_eq!(after.end_of_note_elapsed_ms, Some(250));
}

#[test]
fn end_of_note_timer_ignores_invisible_notes_after_last_note() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut chart = chart();
    chart.lane_notes[Lane::Key2.index()].push(NoteEvent {
        id: NoteId(2),
        lane: Lane::Key2,
        kind: NoteKind::Invisible,
        tick: ChartTick(0),
        time: TimeUs(2_000_000),
        sound: None,
        damage: None,
    });
    chart.end_time = TimeUs(2_000_000);
    let session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());

    let before_last_note = build_render_snapshot(&session, TimeUs(999_999), &[], None);
    let at_last_note = build_render_snapshot(&session, TimeUs(1_000_000), &[], None);
    let after_last_note = build_render_snapshot(&session, TimeUs(1_250_000), &[], None);

    assert_eq!(before_last_note.end_of_note_elapsed_ms, None);
    assert_eq!(at_last_note.end_of_note_elapsed_ms, Some(0));
    assert_eq!(after_last_note.end_of_note_elapsed_ms, Some(250));
}

#[test]
fn cached_end_of_note_time_uses_last_renderable_note() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut chart = chart();
    chart.lane_notes[Lane::Key2.index()] = vec![NoteEvent {
        id: NoteId(2),
        lane: Lane::Key2,
        kind: NoteKind::Invisible,
        tick: ChartTick(3_840),
        time: TimeUs(2_000_000),
        sound: None,
        damage: None,
    }];
    chart.lane_notes[Lane::Key3.index()] = vec![NoteEvent {
        id: NoteId(3),
        lane: Lane::Key3,
        kind: NoteKind::Mine,
        tick: ChartTick(5_760),
        time: TimeUs(3_000_000),
        sound: None,
        damage: Some(10),
    }];
    chart.end_time = TimeUs(3_000_000);
    let session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    let cache = PlayRenderSnapshotCache::from_chart(&session.chart);
    let frames = BgaFrameCatalog::new();
    let snapshot_at = |time| {
        build_render_snapshot_with_target_and_bga_frames_cached(
            &session,
            time,
            &[],
            None,
            None,
            None,
            &frames,
            &cache,
        )
    };

    assert_eq!(snapshot_at(TimeUs(2_999_999)).end_of_note_elapsed_ms, None);
    assert_eq!(snapshot_at(TimeUs(3_000_000)).end_of_note_elapsed_ms, Some(0));
    assert_eq!(snapshot_at(TimeUs(3_250_000)).end_of_note_elapsed_ms, Some(250));
}

#[test]
fn build_render_snapshot_hides_lane_objects_during_playstart() {
    use bmz_chart::model::BarLine;

    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut chart = chart();
    chart.bar_lines.push(BarLine { measure: 1, tick: ChartTick(0), time: TimeUs(1_000_000) });
    let session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());

    let playstart = build_render_snapshot(&session, TimeUs(-1_000_000), &[], None);
    let started = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(playstart.time, TimeUs(-1_000_000));
    assert!(playstart.bar_lines.is_empty());
    assert!(playstart.visible_notes[Lane::Key1.index()].is_empty());
    assert_eq!(started.bar_lines.len(), 1);
    assert_eq!(started.visible_notes[Lane::Key1.index()].len(), 1);
    assert!(started.bar_lines[0].y > 0.0);
}

#[test]
fn build_render_snapshot_normalizes_note_y_to_visible_range() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    let early = build_render_snapshot(&session, TimeUs(0), &[], None);
    let later = build_render_snapshot(&session, TimeUs(750_000), &[], None);

    assert_eq!(early.visible_notes[Lane::Key1.index()][0].y, 0.5);
    assert_eq!(later.visible_notes[Lane::Key1.index()][0].y, 0.125);
}

#[test]
fn build_render_snapshot_uses_four_beats_for_note_speed() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut chart = chart();
    chart.metadata.initial_bpm = 240.0;
    chart.lane_notes[Lane::Key1.index()][0].time = TimeUs(500_000);
    let mut session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(snapshot.note_display_duration_ms, 1000);
    assert_eq!(snapshot.visible_notes[Lane::Key1.index()][0].y, 0.5);
}

#[test]
fn build_render_snapshot_moves_bar_lines_with_visual_note_position() {
    use bmz_chart::model::BarLine;

    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut chart = chart();
    chart.bar_lines.push(BarLine { measure: 1, tick: ChartTick(0), time: TimeUs(1_000_000) });
    let mut session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    let early = build_render_snapshot(&session, TimeUs(0), &[], None);
    let later = build_render_snapshot(&session, TimeUs(250_000), &[], None);

    let early_note_y = early.visible_notes[Lane::Key1.index()][0].y;
    let early_bar_y = early.bar_lines[0].y;
    let later_note_y = later.visible_notes[Lane::Key1.index()][0].y;
    let later_bar_y = later.bar_lines[0].y;

    assert_eq!(early_note_y, early_bar_y);
    assert_eq!(later_note_y, later_bar_y);
    assert_eq!(early_note_y - later_note_y, early_bar_y - later_bar_y);
}

#[test]
fn build_render_snapshot_keeps_notes_under_lane_cover() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;
    // Key1 のノートは render_now=0 で progress 0.5 (time 1_000_000 / lookahead 2_000_000)

    session.lane_cover = 0.3;
    let visible = build_render_snapshot(&session, TimeUs(0), &[], None);
    assert_eq!(visible.visible_notes[Lane::Key1.index()].len(), 1);

    // lane cover は描画で隠すだけなので、カバー域に入る progress でも snapshot には残す。
    session.lane_cover = 0.6;
    let covered = build_render_snapshot(&session, TimeUs(0), &[], None);
    assert_eq!(covered.visible_notes[Lane::Key1.index()].len(), 1);
    assert_eq!(covered.visible_notes[Lane::Key1.index()][0].y, 0.5);
}

#[test]
fn build_render_snapshot_lift_shortens_note_travel_range() {
    use bmz_chart::timing::TICKS_PER_BEAT;

    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut c = chart();
    c.lane_notes[Lane::Key1.index()][0].time = TimeUs(1_600_000);
    c.lane_notes[Lane::Key1.index()][0].tick =
        ChartTick((TICKS_PER_BEAT as f64 * 3.2).round() as u64);
    let mut session = build_game_session(Arc::new(c), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;
    session.lift = 0.2;

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(snapshot.note_display_duration_ms, 1600);
    assert!((snapshot.visible_notes[Lane::Key1.index()][0].y - 1.0).abs() < 1e-3);
}

#[test]
fn build_render_snapshot_lifted_lane_cover_duration_matches_note_position() {
    use bmz_chart::timing::TICKS_PER_BEAT;

    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut c = chart();
    c.lane_notes[Lane::Key1.index()][0].time = TimeUs(1_100_000);
    c.lane_notes[Lane::Key1.index()][0].tick =
        ChartTick((TICKS_PER_BEAT as f64 * 2.2).round() as u64);
    let mut session = build_game_session(Arc::new(c), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;
    session.lift = 0.2;
    session.lane_cover = 0.25;

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(snapshot.note_display_duration_ms, 1100);
    let expected_cover_bottom_progress =
        (1.0 - session.lift - session.lane_cover) / (1.0 - session.lift);
    let y = snapshot.visible_notes[Lane::Key1.index()][0].y;
    assert!(
        (y - expected_cover_bottom_progress).abs() < 1e-3,
        "expected {expected_cover_bottom_progress}, got {y}"
    );
}

#[test]
fn build_render_snapshot_scroll_speed_tracks_bpm_change() {
    use bmz_chart::model::{TimingEvent, TimingEventKind};
    use bmz_chart::timing::TICKS_PER_BEAT;

    // 120 BPM の譜面で 4 拍経過時点(500ms)に 240 BPM へ変化。
    // ノートを変化点直後の 1 拍先 (= さらに 250ms) に置く。
    // hispeed=1.0, lookahead=2s, base BPM=120 → lookahead は 4 拍ぶん。
    // 240 BPM 区間では実時間で半分の速さでスクロールに見えるはずで、
    // ノートは「1 / 4 拍 = 0.25」の位置に来る。
    let mut c = chart();
    c.metadata.initial_bpm = 120.0;
    c.timing_events = vec![TimingEvent {
        tick: ChartTick(TICKS_PER_BEAT as u64 * 4),
        time: TimeUs(2_000_000),
        kind: TimingEventKind::BpmChange { bpm: 240.0 },
    }];
    // ノートを 4 拍 + 1 拍 = 5 拍位置に置く。
    // 0..4 拍 = 2s @ 120BPM, 4..5 拍 = 0.25s @ 240BPM → time = 2_250_000us
    c.lane_notes[Lane::Key1.index()][0].tick = ChartTick(TICKS_PER_BEAT as u64 * 5);
    c.lane_notes[Lane::Key1.index()][0].time = TimeUs(2_250_000);

    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(Arc::new(c), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    // render_now=2_000_000 (BPM 変化点ちょうど): ノートは 1 拍先 = 0.25 にいる。
    let snap = build_render_snapshot(&session, TimeUs(2_000_000), &[], None);
    let y = snap.visible_notes[Lane::Key1.index()][0].y;
    assert!((y - 0.25).abs() < 1e-3, "expected ~0.25, got {y}");
}

#[test]
fn visual_offset_advances_lane_bpm_without_advancing_snapshot_time() {
    use bmz_chart::model::{TimingEvent, TimingEventKind};
    use bmz_chart::timing::TICKS_PER_BEAT;

    let mut c = chart();
    c.metadata.initial_bpm = 120.0;
    c.timing_events = vec![TimingEvent {
        tick: ChartTick(TICKS_PER_BEAT as u64 * 4),
        time: TimeUs(2_000_000),
        kind: TimingEventKind::BpmChange { bpm: 240.0 },
    }];
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(Arc::new(c), &profile, PlaySessionOptions::default());
    session.offsets.visual_offset_us = 100_000;

    let snapshot = build_render_snapshot(&session, TimeUs(1_950_000), &[], None);

    assert_eq!(snapshot.time, TimeUs(1_950_000));
    assert_eq!(snapshot.now_bpm, 240.0);
}

#[test]
fn build_render_snapshot_scroll_freezes_during_stop() {
    use bmz_chart::model::{TimingEvent, TimingEventKind};
    use bmz_chart::timing::TICKS_PER_BEAT;

    // 120 BPM で 4 拍経過時点 (2s) に 1 秒の STOP。
    // ノートは 5 拍位置 (実時刻 3.5s — 2s + STOP1s + 0.5s)。
    let mut c = chart();
    c.metadata.initial_bpm = 120.0;
    c.timing_events = vec![TimingEvent {
        tick: ChartTick(TICKS_PER_BEAT as u64 * 4),
        time: TimeUs(0),
        kind: TimingEventKind::Stop { duration_us: 1_000_000 },
    }];
    c.lane_notes[Lane::Key1.index()][0].tick = ChartTick(TICKS_PER_BEAT as u64 * 5);
    c.lane_notes[Lane::Key1.index()][0].time = TimeUs(3_500_000);

    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(Arc::new(c), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    // STOP 直前 (just before tick 4 拍): カーソル tick=4, ノート tick=5 → 1 拍差 = 0.25
    let before = build_render_snapshot(&session, TimeUs(1_999_999), &[], None);
    let y_before = before.visible_notes[Lane::Key1.index()][0].y;
    assert!((y_before - 0.25).abs() < 1e-3, "before: expected ~0.25, got {y_before}");

    // STOP 中: カーソル tick が止まり、ノート位置も動かない。
    let mid = build_render_snapshot(&session, TimeUs(2_500_000), &[], None);
    let y_mid = mid.visible_notes[Lane::Key1.index()][0].y;
    assert!((y_mid - 0.25).abs() < 1e-3, "mid stop: expected ~0.25, got {y_mid}");
}

#[test]
fn build_render_snapshot_hides_same_tick_note_after_stop_starts() {
    use bmz_chart::model::{TimingEvent, TimingEventKind};
    use bmz_chart::timing::TICKS_PER_BEAT;

    // beatoraja は STOP 中のスクロール位置は止めるが、通常ノートの描画可否は
    // TimeLine の microTime >= 現在時刻で切る。同 tick のノートは STOP 終了まで
    // 判定ライン上に残さない。
    let stop_tick = TICKS_PER_BEAT as u64 * 4;
    let mut c = chart();
    c.metadata.initial_bpm = 120.0;
    c.timing_events = vec![TimingEvent {
        tick: ChartTick(stop_tick),
        time: TimeUs(0),
        kind: TimingEventKind::Stop { duration_us: 1_000_000 },
    }];
    c.lane_notes[Lane::Key1.index()][0].tick = ChartTick(stop_tick);
    c.lane_notes[Lane::Key1.index()][0].time = TimeUs(2_000_000);
    c.end_time = TimeUs(2_000_000);

    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(Arc::new(c), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    let at_stop_start = build_render_snapshot(&session, TimeUs(2_000_000), &[], None);
    assert_eq!(at_stop_start.visible_notes[Lane::Key1.index()].len(), 1);
    assert_eq!(at_stop_start.visible_notes[Lane::Key1.index()][0].y, 0.0);

    let during_stop = build_render_snapshot(&session, TimeUs(2_000_001), &[], None);
    assert!(during_stop.visible_notes[Lane::Key1.index()].is_empty());
}

#[test]
fn build_render_snapshot_applies_hispeed_to_note_positions() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.hispeed = 2.0;

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(snapshot.hispeed, 2.0);
    assert_eq!(snapshot.visible_notes[Lane::Key1.index()][0].y, 1.0);
}

#[test]
fn build_render_snapshot_doubles_distance_with_scroll_factor_two() {
    use bmz_chart::model::ScrollEvent;
    let mut chart = chart();
    // tick 0 から factor=2.0 で全区間スクロール倍速。
    chart.scroll_events = vec![ScrollEvent { tick: ChartTick(0), time: TimeUs(0), factor: 2.0 }];
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    // chart() のノートは TimeUs(1_000_000)、lookahead=2_000_000 で 1/2 進捗。
    // SCROLL 2.0 が乗ると見かけ進捗 1.0 (画面上端) になる。
    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);
    let y = snapshot.visible_notes[Lane::Key1.index()][0].y;
    assert!((y - 1.0).abs() < 1e-3, "expected ~1.0 with SCROLL 2.0, got {y}");
    assert_eq!(snapshot.note_display_duration_ms, 1000);
}

#[test]
fn speed_at_interpolates_linearly_between_events() {
    let segments = [(0.0, 1.0), (3840.0, 2.0)];
    // 区間内の中央は中間値 1.5。
    assert!((super::speed_at(&segments, 1920.0) - 1.5).abs() < 1e-6);
    // 1/4 地点。
    assert!((super::speed_at(&segments, 960.0) - 1.25).abs() < 1e-6);
    // 境界の値そのもの。
    assert!((super::speed_at(&segments, 0.0) - 1.0).abs() < 1e-6);
    assert!((super::speed_at(&segments, 3840.0) - 2.0).abs() < 1e-6);
    // 最後のイベント以降はその factor で固定 (補間されない)。
    assert!((super::speed_at(&segments, 5000.0) - 2.0).abs() < 1e-6);
}

#[test]
fn speed_at_returns_one_before_first_event() {
    let segments = [(1000.0, 2.0)];
    assert!((super::speed_at(&segments, 500.0) - 1.0).abs() < 1e-6);
    assert!((super::speed_at(&segments, 1000.0) - 2.0).abs() < 1e-6);
    assert!((super::speed_at(&segments, 2000.0) - 2.0).abs() < 1e-6);
}

#[test]
fn speed_at_keeps_last_factor_at_duplicate_tick() {
    let segments = [(0.0, 1.0), (1000.0, 2.0), (1000.0, 3.0), (2000.0, 5.0)];

    assert!((super::speed_at(&segments, 1000.0) - 3.0).abs() < 1e-6);
    assert!((super::speed_at(&segments, 1500.0) - 4.0).abs() < 1e-6);
}

#[test]
fn accumulate_scroll_preserves_boundaries_reverse_and_negative_factors() {
    let segments = [(0.0, 2.0), (100.0, 0.5), (200.0, -1.0)];
    let integral = ScrollIntegralCache::from_segments(segments);

    assert!((super::accumulate_scroll(&segments, 0.0, 50.0) - 100.0).abs() < 1e-6);
    assert!((super::accumulate_scroll(&segments, 0.0, 150.0) - 225.0).abs() < 1e-6);
    assert!((super::accumulate_scroll(&segments, 0.0, 250.0) - 200.0).abs() < 1e-6);
    assert!((super::accumulate_scroll(&segments, 50.0, 250.0) - 100.0).abs() < 1e-6);
    assert!((super::accumulate_scroll(&segments, 250.0, 50.0) + 100.0).abs() < 1e-6);
    assert_eq!(super::accumulate_scroll(&segments, 50.0, 50.0), 0.0);
    for (from_tick, to_tick) in
        [(0.0, 50.0), (0.0, 150.0), (0.0, 250.0), (50.0, 250.0), (250.0, 50.0), (50.0, 50.0)]
    {
        let expected = super::accumulate_scroll(&segments, from_tick, to_tick);
        assert!((integral.delta(from_tick, to_tick) - expected).abs() < 1e-6);
    }

    let duplicate_tick = [(0.0, 2.0), (0.0, 3.0), (100.0, 1.0)];
    assert!((super::accumulate_scroll(&duplicate_tick, 0.0, 50.0) - 150.0).abs() < 1e-6);
    let duplicate_integral = ScrollIntegralCache::from_segments(duplicate_tick);
    assert!((duplicate_integral.delta(0.0, 50.0) - 150.0).abs() < 1e-6);

    let delayed_event = [(100.0, 2.0), (200.0, 0.5)];
    let delayed_integral = ScrollIntegralCache::from_segments(delayed_event);
    for (from_tick, to_tick) in [(0.0, 50.0), (0.0, 100.0), (50.0, 150.0), (250.0, 50.0)] {
        let expected = super::accumulate_scroll(&delayed_event, from_tick, to_tick);
        assert!((delayed_integral.delta(from_tick, to_tick) - expected).abs() < 1e-6);
    }
    assert_eq!(delayed_integral.delta(0.0, f64::EPSILON / 2.0), 0.0);
}

#[test]
fn build_render_snapshot_applies_speed_factor() {
    use bmz_chart::model::SpeedEvent;
    let mut chart = chart();
    chart.speed_events = vec![SpeedEvent { tick: ChartTick(0), time: TimeUs(0), factor: 2.0 }];
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);
    let y = snapshot.visible_notes[Lane::Key1.index()][0].y;
    assert!((y - 1.0).abs() < 1e-3, "expected ~1.0 with SPEED 2.0, got {y}");
    assert_eq!(snapshot.note_display_duration_ms, 1000);
}

#[test]
fn build_render_snapshot_interpolates_speed_between_events() {
    use bmz_chart::model::SpeedEvent;
    let mut chart = chart();
    // BPM 120 / 4 拍 = 3840 ticks。SPEED を tick=0..3840 で 1.0→3.0 へ補間。
    // chart() のノートは TimeUs(1_000_000) = 1920 ticks (中央) なので、
    // 補間値は 2.0 になる。base 進捗 0.5 × SPEED 2.0 = 1.0 (画面上端)。
    chart.speed_events = vec![
        SpeedEvent { tick: ChartTick(0), time: TimeUs(0), factor: 1.0 },
        SpeedEvent { tick: ChartTick(3840), time: TimeUs(2_000_000), factor: 3.0 },
    ];
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);
    let y = snapshot.visible_notes[Lane::Key1.index()][0].y;
    assert!(
        (y - 1.0).abs() < 1e-3,
        "expected ~1.0 from linear interpolation (0.5 base × 2.0 mid speed), got {y}"
    );
}

#[test]
fn build_render_snapshot_compresses_distance_with_scroll_factor_half() {
    use bmz_chart::model::ScrollEvent;
    let mut chart = chart();
    chart.scroll_events = vec![ScrollEvent { tick: ChartTick(0), time: TimeUs(0), factor: 0.5 }];
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    // 1/2 進捗 × SCROLL 0.5 = 1/4 進捗。
    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);
    let y = snapshot.visible_notes[Lane::Key1.index()][0].y;
    assert!((y - 0.25).abs() < 1e-3, "expected ~0.25 with SCROLL 0.5, got {y}");
}

#[test]
fn build_render_snapshot_hides_note_with_negative_scroll() {
    use bmz_chart::model::ScrollEvent;
    let mut chart = chart();
    // factor < 0 は逆スクロール。delta が負になり描画対象外。
    chart.scroll_events = vec![ScrollEvent { tick: ChartTick(0), time: TimeUs(0), factor: -1.0 }];
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    session.hispeed = 1.0;

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);
    assert!(snapshot.visible_notes[Lane::Key1.index()].is_empty());
}

#[test]
fn build_render_snapshot_reports_lane_cover_changing_and_note_display_duration() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.hispeed = 2.0;
    session.lane_cover = 0.25;
    session.lane_cover_changing = true;

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert!(snapshot.lane_cover_changing);
    assert_eq!(snapshot.note_display_duration_ms, 750);
}

#[test]
fn build_render_snapshot_reports_gauge_skin_timer_elapsed() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.gauge_increase_started_at = Some(TimeUs(100_000));
    session.gauge_max_started_at = Some(TimeUs(250_000));

    let snapshot = build_render_snapshot(&session, TimeUs(400_000), &[], None);

    assert_eq!(snapshot.gauge_increase_elapsed_ms, Some(300));
    assert_eq!(snapshot.gauge_max_elapsed_ms, Some(150));
}

#[test]
fn update_render_snapshot_play_options_refreshes_ready_snapshot_values() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    let mut snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    session.hispeed = 2.0;
    session.lane_cover = 0.25;
    session.lane_cover_changing = true;
    update_render_snapshot_play_options(&mut snapshot, &session, TimeUs(0));

    assert_eq!(snapshot.hispeed, 2.0);
    assert_eq!(snapshot.lane_cover, 0.25);
    assert!(snapshot.lane_cover_changing);
    assert_eq!(snapshot.note_display_duration_ms, 750);
}

#[test]
fn build_render_snapshot_keeps_notes_after_judge_cursor_advances() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.judge.lanes[Lane::Key1.index()].next_note_index = 1;

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(snapshot.visible_notes[Lane::Key1.index()].len(), 1);
    assert_eq!(snapshot.visible_notes[Lane::Key1.index()][0].processed_judge, None);
}

#[test]
fn build_render_snapshot_routes_invisible_and_mine_correctly() {
    let mut chart = chart();
    chart.lane_notes[Lane::Key2.index()].push(NoteEvent {
        id: NoteId(2),
        lane: Lane::Key2,
        kind: NoteKind::Invisible,
        tick: ChartTick(0),
        time: TimeUs(1_000_000),
        sound: None,
        damage: None,
    });
    chart.lane_notes[Lane::Key3.index()].push(NoteEvent {
        id: NoteId(3),
        lane: Lane::Key3,
        kind: NoteKind::Mine,
        tick: ChartTick(0),
        time: TimeUs(1_000_000),
        sound: None,
        damage: Some(8),
    });
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(snapshot.visible_notes[Lane::Key1.index()].len(), 1);
    assert!(snapshot.visible_notes[Lane::Key2.index()].is_empty());
    assert!(snapshot.visible_notes[Lane::Key3.index()].is_empty());
    // Mine は visible_mines 側に振り分けられる。damage も保持。
    assert_eq!(snapshot.visible_mines[Lane::Key3.index()].len(), 1);
    assert_eq!(snapshot.visible_mines[Lane::Key3.index()][0].damage, 8);
    assert!(snapshot.visible_mines[Lane::Key1.index()].is_empty());
    assert!(snapshot.visible_mines[Lane::Key2.index()].is_empty());
}

#[test]
fn build_render_snapshot_copies_recent_inputs() {
    use bmz_core::input::{InputDeviceKind, InputEvent, InputKind, InputSource};

    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.recent_inputs.push(InputEvent {
        lane: Lane::Key3,
        kind: InputKind::Press,
        time: TimeUs(42_000),
        source: InputSource::Human,
        device_kind: InputDeviceKind::Keyboard,
        scratch_direction: None,
    });

    let snapshot = build_render_snapshot(&session, TimeUs(50_000), &[], None);

    assert_eq!(snapshot.recent_inputs.len(), 1);
    assert_eq!(snapshot.recent_inputs[0].lane, Lane::Key3);
    assert_eq!(snapshot.recent_inputs[0].time, TimeUs(42_000));
}

#[test]
fn build_render_snapshot_sums_judge_counts() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.score.apply(&JudgementEvent {
        note_id: Some(NoteId(1)),
        lane: Lane::Key1,
        judge: Judge::PGreat,
        side: TimingSide::Fast,
        delta: TimeUs(-1_000),
        time: TimeUs(1_000),
        affects_score: true,
    });
    session.score.apply(&JudgementEvent {
        note_id: None,
        lane: Lane::Key1,
        judge: Judge::EmptyPoor,
        side: TimingSide::Slow,
        delta: TimeUs(40_000),
        time: TimeUs(2_000),
        affects_score: true,
    });

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(snapshot.judge_counts.pgreat, 1);
    assert_eq!(snapshot.judge_counts.empty_poor, 1);
    assert_eq!(snapshot.fast_slow_counts.fast_pgreat, 1);
    assert_eq!(snapshot.fast_slow_counts.slow_empty_poor, 1);
}

#[test]
fn build_render_snapshot_marks_replay_playback() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let normal = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    let replay = build_game_session(
        Arc::new(chart()),
        &profile,
        PlaySessionOptions {
            replay_player: Some(bmz_gameplay::replay::ReplayPlayer::default()),
            ..PlaySessionOptions::default()
        },
    );

    assert!(!build_render_snapshot(&normal, TimeUs(0), &[], None).replay_playback);
    assert!(build_render_snapshot(&replay, TimeUs(0), &[], None).replay_playback);
}

#[test]
fn build_render_snapshot_carries_chart_total_gauge() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut source = chart();
    source.metadata.total = Some(350.0);
    let session = build_game_session(Arc::new(source), &profile, PlaySessionOptions::default());

    assert_eq!(build_render_snapshot(&session, TimeUs(0), &[], None).chart_total_gauge, 350.0);
}

#[test]
fn build_render_snapshot_treats_replay_as_autoplay_off_for_skin_ops() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.play.auto_play = true;
    let replay = build_game_session(
        Arc::new(chart()),
        &profile,
        PlaySessionOptions {
            replay_player: Some(bmz_gameplay::replay::ReplayPlayer::default()),
            ..PlaySessionOptions::default()
        },
    );

    assert!(replay.autoplay.is_none());
    let snapshot = build_render_snapshot(&replay, TimeUs(0), &[], None);
    assert!(snapshot.replay_playback);
    assert!(!snapshot.autoplay);
}

#[test]
fn build_render_snapshot_passes_judge_rank() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut chart = chart();
    chart.metadata.judge_rank = Some(0);
    let session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(snapshot.judge_rank, Some(0));
}

#[test]
fn build_render_snapshot_passes_best_ex_score() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let session = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

    let with_best = build_render_snapshot(&session, TimeUs(0), &[], Some(42));
    let without_best = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(with_best.best_ex_score, Some(42));
    assert_eq!(without_best.best_ex_score, None);
}

#[test]
fn build_render_snapshot_passes_target_ex_score() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let session = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

    let snapshot = build_render_snapshot_with_target_and_bga_frames(
        &session,
        TimeUs(0),
        &[],
        None,
        None,
        Some(1600),
        &BgaFrameCatalog::new(),
    );

    assert_eq!(snapshot.target_ex_score, Some(1600));
}

#[test]
fn build_render_snapshot_projects_best_score_from_ghost() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.score.past_notes = 3;

    let snapshot = build_render_snapshot_with_target_and_bga_frames(
        &session,
        TimeUs(0),
        &[],
        Some(8),
        Some(&[0, 1, 4, 0]),
        None,
        &BgaFrameCatalog::new(),
    );

    assert_eq!(snapshot.projected_best_ex_score, Some(3));
}

#[test]
fn build_render_snapshot_derives_judge_timing_offset_from_visual_offset() {
    use bmz_gameplay::session::PlayOffsets;

    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.offsets = PlayOffsets { input_offset_us: 3_000, visual_offset_us: 4_000 };

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(snapshot.judge_timing_offset_ms, 4);
}

#[test]
fn build_render_snapshot_copies_skin_offsets() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.skin_offsets.push(bmz_gameplay::session::PlaySkinOffset {
        id: 42,
        x: 1,
        y: 2,
        w: 3,
        h: 4,
        r: 5,
        a: -6,
    });

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(
        snapshot.skin_offsets.get(42),
        Some(SkinOffsetValue { x: 1, y: 2, w: 3, h: 4, r: 5, a: -6 })
    );
}

#[test]
fn build_render_snapshot_sets_scratch_angle_offset() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.skin_offsets.push(bmz_gameplay::session::PlaySkinOffset {
        id: SCRATCH_ANGLE_OFFSET_1P,
        x: 1,
        y: 2,
        w: 3,
        h: 4,
        r: 5,
        a: -6,
    });

    let snapshot = build_render_snapshot(&session, TimeUs(6_000_000), &[], None);

    assert_eq!(
        snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_1P),
        Some(SkinOffsetValue { x: 1, y: 2, w: 3, h: 4, r: 80, a: -6 })
    );
}

#[test]
fn refresh_play_skin_visuals_uses_play_elapsed_during_playstart() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let session = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    let mut snapshot = build_render_snapshot(&session, TimeUs(-1_000_000), &[], None);
    snapshot.play_elapsed_time = TimeUs(6_000_000);

    refresh_play_skin_visuals(&mut snapshot, &session);

    assert_eq!(
        snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_1P),
        Some(SkinOffsetValue { r: 80, ..SkinOffsetValue::default() })
    );
}

#[test]
fn refresh_play_skin_visuals_keeps_turntable_angle_after_chart_start() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let session = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    let mut snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);
    snapshot.play_elapsed_time = TimeUs(6_000_000);

    refresh_play_skin_visuals(&mut snapshot, &session);

    assert_eq!(
        snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_1P),
        Some(SkinOffsetValue { r: 80, ..SkinOffsetValue::default() })
    );
}

#[test]
fn refresh_play_skin_visuals_applies_accumulated_scratch_turntable_phase() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.lane_scratch_angle_delta_ms[Lane::Scratch.index()] = 2_000;
    let mut snapshot = build_render_snapshot(&session, TimeUs(1_000_000), &[], None);
    snapshot.play_elapsed_time = TimeUs(6_000_000);

    refresh_play_skin_visuals(&mut snapshot, &session);

    assert_eq!(
        snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_1P),
        Some(SkinOffsetValue { r: 53, ..SkinOffsetValue::default() })
    );
}

#[test]
fn refresh_play_skin_visuals_keeps_accumulated_scratch_phase_after_release() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.lane_scratch_angle_delta_ms[Lane::Scratch.index()] = -2_000;
    let mut snapshot = build_render_snapshot(&session, TimeUs(1_000_000), &[], None);
    snapshot.play_elapsed_time = TimeUs(6_000_000);

    refresh_play_skin_visuals(&mut snapshot, &session);

    assert_eq!(
        snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_1P),
        Some(SkinOffsetValue { r: 106, ..SkinOffsetValue::default() })
    );
}

#[test]
fn scratch_angle_offsets_match_beatoraja_1p_and_2p_values() {
    let first_frame = TimeUs(6_000_000);
    let next_frame = TimeUs(6_006_000);

    assert_eq!(scratch_angle_degrees(first_frame, 0, 0), 80);
    assert_eq!(scratch_angle_degrees(next_frame, 0, 0), 79);
    assert_eq!(scratch_angle_degrees(first_frame, 1, 0), 280);
    assert_eq!(scratch_angle_degrees(next_frame, 1, 0), 281);
}

#[test]
fn build_render_snapshot_sets_opposite_14k_turntable_offsets() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut chart = chart();
    chart.metadata.key_mode = KeyMode::K14;
    let session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());

    let snapshot = build_render_snapshot(&session, TimeUs(6_000_000), &[], None);

    assert_eq!(snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_1P).unwrap().r, 80);
    assert_eq!(snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_2P).unwrap().r, 280);
}

#[test]
fn scratch_offsets_render_with_beatoraja_rotation_after_skin_conversion() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut chart = chart();
    chart.metadata.key_mode = KeyMode::K14;
    let session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 2,
                "w": 100,
                "h": 100,
                "source": [{ "id": "src", "path": "turntable.png" }],
                "image": [{ "id": "turntable", "src": "src", "w": 10, "h": 10 }],
                "destination": [
                    {
                        "id": "turntable",
                        "offset": 1,
                        "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }]
                    },
                    {
                        "id": "turntable",
                        "offset": 2,
                        "dst": [{ "x": 20, "y": 0, "w": 10, "h": 10 }]
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "src".to_string(),
        SkinDocumentTexture {
            source_id: "src".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        },
    )]);

    for (visual_time, expected_angles) in
        [(TimeUs(6_000_000), [-80, -280]), (TimeUs(6_006_000), [-79, -281])]
    {
        let snapshot = build_render_snapshot(&session, visual_time, &[], None);
        let state = SkinDrawState {
            key_mode: KeyMode::K14,
            skin_offsets: snapshot.skin_offsets,
            ..SkinDrawState::default()
        };
        let angles = document
            .static_image_render_items(&sources, &state)
            .iter()
            .map(|item| match item {
                SkinRenderItem::RotatedImage { angle_deg, .. } => *angle_deg as i32,
                _ => panic!("turntable should be rotated"),
            })
            .collect::<Vec<_>>();

        assert_eq!(angles, expected_angles);
    }
}

#[test]
fn refresh_play_skin_visuals_tracks_pre_ready_keybeam() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(1_000_000));
    let mut snapshot = build_render_snapshot(&session, TimeUs(-1_000_000), &[], None);
    snapshot.play_elapsed_time = TimeUs(1_050_000);

    refresh_play_skin_visuals(&mut snapshot, &session);

    assert_eq!(snapshot.keyon_ms[Lane::Key1.index()], Some(50));
}

#[test]
fn refresh_play_skin_visuals_with_input_elapsed_tracks_short_pre_ready_keybeam() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(100_000));
    let mut snapshot = build_render_snapshot(&session, TimeUs(-100_000), &[], None);

    refresh_play_skin_visuals_with_input_elapsed(&mut snapshot, &session, TimeUs(150_000));

    assert_eq!(snapshot.keyon_ms[Lane::Key1.index()], Some(50));
}

#[test]
fn refresh_play_skin_visuals_hides_stale_pre_ready_keybeam_after_chart_start() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(3_500_000));
    session.lane_keyoff_started_at[Lane::Key2.index()] = Some(TimeUs(3_550_000));
    let mut snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);
    snapshot.play_elapsed_time = TimeUs(4_000_000);

    refresh_play_skin_visuals(&mut snapshot, &session);

    assert_eq!(snapshot.keyon_ms[Lane::Key1.index()], None);
    assert_eq!(snapshot.keyoff_ms[Lane::Key2.index()], None);
}

#[test]
fn refresh_play_skin_visuals_keeps_playstart_keybeam_after_chart_start() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(-500_000));
    let mut snapshot = build_render_snapshot(&session, TimeUs(250_000), &[], None);
    snapshot.play_elapsed_time = TimeUs(4_000_000);

    refresh_play_skin_visuals(&mut snapshot, &session);

    assert_eq!(snapshot.keyon_ms[Lane::Key1.index()], Some(750));
}

#[test]
fn build_render_snapshot_selects_current_bga_frames() {
    use bmz_chart::model::{BgaArgbEvent, BgaAssetKind, BgaAssetRef, BgaEvent, BgaOpacityEvent};

    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut chart = chart();
    chart.metadata.has_bga = true;
    chart.bga_assets = vec![
        BgaAssetRef { id: BgaAssetId(0), path: "base-a.png".into(), kind: BgaAssetKind::Static },
        BgaAssetRef { id: BgaAssetId(1), path: "base-b.png".into(), kind: BgaAssetKind::Static },
        BgaAssetRef { id: BgaAssetId(2), path: "layer.png".into(), kind: BgaAssetKind::Static },
        BgaAssetRef { id: BgaAssetId(3), path: "poor.png".into(), kind: BgaAssetKind::Static },
    ];
    chart.bga_events = vec![
        BgaEvent {
            tick: ChartTick(0),
            time: TimeUs(0),
            asset: Some(BgaAssetId(0)),
            kind: BgaEventKind::Base,
        },
        BgaEvent {
            tick: ChartTick(0),
            time: TimeUs(500_000),
            asset: Some(BgaAssetId(1)),
            kind: BgaEventKind::Base,
        },
        BgaEvent {
            tick: ChartTick(0),
            time: TimeUs(250_000),
            asset: Some(BgaAssetId(2)),
            kind: BgaEventKind::Layer,
        },
        BgaEvent {
            tick: ChartTick(0),
            time: TimeUs(700_000),
            asset: None,
            kind: BgaEventKind::Layer,
        },
        BgaEvent {
            tick: ChartTick(0),
            time: TimeUs(300_000),
            asset: Some(BgaAssetId(3)),
            kind: BgaEventKind::Poor,
        },
    ];
    chart.bga_opacity_events = vec![BgaOpacityEvent {
        tick: ChartTick(0),
        time: TimeUs(200_000),
        layer: BgaEventKind::Layer,
        opacity: 128,
    }];
    chart.bga_argb_events = vec![BgaArgbEvent {
        tick: ChartTick(0),
        time: TimeUs(200_000),
        layer: BgaEventKind::Layer,
        alpha: 255,
        red: 255,
        green: 32,
        blue: 16,
    }];
    let mut session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    session.poor_bga_duration_us = 250_000;
    // レーン表示オフセットが BGA イベント選択へ漏れないことも同時に検証する。
    session.offsets.visual_offset_us = 500_000;
    let bga_frames = BgaFrameCatalog::from([
        (BgaAssetId(0), display_bga_frame(BgaAssetId(0), 256, 256)),
        (BgaAssetId(1), display_bga_frame(BgaAssetId(1), 640, 480)),
        (BgaAssetId(2), display_bga_frame(BgaAssetId(2), 1280, 720)),
        (BgaAssetId(3), display_bga_frame(BgaAssetId(3), 320, 240)),
    ]);
    let poor_judgements = [JudgementEvent {
        note_id: Some(NoteId(1)),
        lane: Lane::Key1,
        judge: Judge::Poor,
        side: TimingSide::Slow,
        delta: TimeUs(0),
        time: TimeUs(400_000),
        affects_score: true,
    }];

    let early =
        build_render_snapshot_with_bga_frames(&session, TimeUs(100_000), &[], None, &bga_frames);
    let late =
        build_render_snapshot_with_bga_frames(&session, TimeUs(600_000), &[], None, &bga_frames);
    let poor_active = build_render_snapshot_with_bga_frames(
        &session,
        TimeUs(600_000),
        &poor_judgements,
        None,
        &bga_frames,
    );
    let poor_expired = build_render_snapshot_with_bga_frames(
        &session,
        TimeUs(651_000),
        &poor_judgements,
        None,
        &bga_frames,
    );
    let layer_cleared =
        build_render_snapshot_with_bga_frames(&session, TimeUs(800_000), &[], None, &bga_frames);

    assert_eq!(early.bga_base.unwrap().texture_id, bga_texture_id(BgaAssetId(0)));
    assert!(early.bga_layer.is_none());
    assert_eq!(late.bga_base.unwrap(), display_bga_frame(BgaAssetId(1), 640, 480));
    let late_layer = late.bga_layer.unwrap();
    assert_eq!(late_layer.texture_id, bga_texture_id(BgaAssetId(2)));
    assert!((late_layer.tint_r - 1.0).abs() < 0.01);
    assert!((late_layer.tint_g - 32.0 / 255.0).abs() < 0.01);
    assert!((late_layer.tint_b - 16.0 / 255.0).abs() < 0.01);
    assert!((late_layer.tint_a - 128.0 / 255.0).abs() < 0.01);
    assert_eq!(poor_active.bga_poor.unwrap(), display_bga_frame(BgaAssetId(3), 320, 240));
    assert!(poor_expired.bga_poor.is_none());
    assert_eq!(layer_cleared.bga_base.unwrap(), display_bga_frame(BgaAssetId(1), 640, 480));
    assert!(layer_cleared.bga_layer.is_none());
}

#[test]
fn current_bpm_returns_initial_bpm_before_first_change() {
    let chart = chart_with_bpm_changes();
    // At time 0, before any BPM change
    assert_eq!(current_bpm(&chart, TimeUs(0)), 120.0);
}

#[test]
fn current_bpm_returns_changed_bpm_after_event() {
    let chart = chart_with_bpm_changes();
    // BPM changes to 180 at t=500_000 µs
    assert_eq!(current_bpm(&chart, TimeUs(500_000)), 180.0);
    // BPM changes to 90 at t=1_000_000 µs
    assert_eq!(current_bpm(&chart, TimeUs(1_000_000)), 90.0);
    // After last change
    assert_eq!(current_bpm(&chart, TimeUs(2_000_000)), 90.0);
}

#[test]
fn chart_min_bpm_returns_minimum_across_all_events() {
    let chart = chart_with_bpm_changes();
    // initial=120, events: 180, 90 → min=90
    assert_eq!(chart_min_bpm(&chart), 90.0);
}

#[test]
fn chart_max_bpm_returns_maximum_across_all_events() {
    let chart = chart_with_bpm_changes();
    // initial=120, events: 180, 90 → max=180
    assert_eq!(chart_max_bpm(&chart), 180.0);
}

#[test]
fn bpm_helpers_use_initial_bpm_when_no_timing_events() {
    let chart = chart(); // no timing_events
    assert_eq!(current_bpm(&chart, TimeUs(0)), 120.0);
    assert_eq!(chart_min_bpm(&chart), 120.0);
    assert_eq!(chart_max_bpm(&chart), 120.0);
}

fn chart_with_bpm_changes() -> PlayableChart {
    use bmz_chart::model::{TimingEvent, TimingEventKind};
    PlayableChart {
        identity: compute_chart_identity(b"bpm-test"),
        metadata: ChartMetadata { initial_bpm: 120.0, ..Default::default() },
        lane_notes: std::array::from_fn(|_| Vec::new()),
        long_notes: Vec::new(),
        bgm_events: Vec::new(),
        bga_events: Vec::new(),
        timing_events: vec![
            TimingEvent {
                tick: ChartTick(0),
                time: TimeUs(500_000),
                kind: TimingEventKind::BpmChange { bpm: 180.0 },
            },
            TimingEvent {
                tick: ChartTick(0),
                time: TimeUs(1_000_000),
                kind: TimingEventKind::BpmChange { bpm: 90.0 },
            },
        ],
        scroll_events: Vec::new(),
        speed_events: Vec::new(),
        judge_rank_events: Vec::new(),
        bgm_volume_events: Vec::new(),
        key_volume_events: Vec::new(),
        text_events: Vec::new(),
        bga_opacity_events: Vec::new(),
        bga_argb_events: Vec::new(),
        swbga_definitions: Vec::new(),
        bga_keybound_events: Vec::new(),
        bga_asset_by_bmp_key: std::collections::HashMap::new(),
        bar_lines: Vec::new(),
        sounds: Vec::new(),
        bga_assets: Vec::new(),
        total_notes: 0,
        end_time: TimeUs(2_000_000),
    }
}

fn chart() -> PlayableChart {
    let note = tap_note(1, Lane::Key1, 0, 1_000_000);
    let mut lane_notes = std::array::from_fn(|_| Vec::new());
    lane_notes[Lane::Key1.index()].push(note);

    PlayableChart {
        identity: compute_chart_identity(b"snapshot"),
        metadata: ChartMetadata {
            title: "snapshot".to_string(),
            initial_bpm: 120.0,
            total: Some(160.0),
            ..Default::default()
        },
        lane_notes,
        long_notes: Vec::new(),
        bgm_events: Vec::new(),
        bga_events: Vec::new(),
        timing_events: Vec::new(),

        scroll_events: Vec::new(),

        speed_events: Vec::new(),
        judge_rank_events: Vec::new(),
        bgm_volume_events: Vec::new(),
        key_volume_events: Vec::new(),
        text_events: Vec::new(),
        bga_opacity_events: Vec::new(),
        bga_argb_events: Vec::new(),
        swbga_definitions: Vec::new(),
        bga_keybound_events: Vec::new(),
        bga_asset_by_bmp_key: std::collections::HashMap::new(),
        bar_lines: Vec::new(),
        sounds: Vec::new(),
        bga_assets: Vec::new(),
        total_notes: 1,
        end_time: TimeUs(1_000_000),
    }
}

fn tap_note(id: u32, lane: Lane, tick: u64, time_us: i64) -> NoteEvent {
    NoteEvent {
        id: NoteId(id),
        lane,
        kind: NoteKind::Tap,
        tick: ChartTick(tick),
        time: TimeUs(time_us),
        sound: None,
        damage: None,
    }
}

/// Key1 に start=500ms, end=1500ms のロングノートを1本持つ譜面。
fn chart_with_long_note() -> PlayableChart {
    use bmz_chart::model::{LongNotePair, LongNoteStyle};

    let start = NoteEvent {
        id: NoteId(1),
        lane: Lane::Key1,
        kind: NoteKind::LongStart,
        tick: ChartTick(0),
        time: TimeUs(500_000),
        sound: None,
        damage: None,
    };
    let end = NoteEvent {
        id: NoteId(2),
        lane: Lane::Key1,
        kind: NoteKind::LongEnd,
        tick: ChartTick(0),
        time: TimeUs(1_500_000),
        sound: None,
        damage: None,
    };
    let mut lane_notes = std::array::from_fn(|_| Vec::new());
    lane_notes[Lane::Key1.index()].push(start);
    lane_notes[Lane::Key1.index()].push(end);

    PlayableChart {
        identity: compute_chart_identity(b"long-note"),
        metadata: ChartMetadata { initial_bpm: 120.0, ..Default::default() },
        lane_notes,
        long_notes: vec![LongNotePair {
            lane: Lane::Key1,
            style: LongNoteStyle::ChannelPair,
            mode: None,
            start_note_id: NoteId(1),
            end_note_id: NoteId(2),
            start_tick: ChartTick(0),
            end_tick: ChartTick(0),
            start_time: TimeUs(500_000),
            end_time: TimeUs(1_500_000),
            sound: None,
        }],
        bgm_events: Vec::new(),
        bga_events: Vec::new(),
        timing_events: Vec::new(),

        scroll_events: Vec::new(),

        speed_events: Vec::new(),
        judge_rank_events: Vec::new(),
        bgm_volume_events: Vec::new(),
        key_volume_events: Vec::new(),
        text_events: Vec::new(),
        bga_opacity_events: Vec::new(),
        bga_argb_events: Vec::new(),
        swbga_definitions: Vec::new(),
        bga_keybound_events: Vec::new(),
        bga_asset_by_bmp_key: std::collections::HashMap::new(),
        bar_lines: Vec::new(),
        sounds: Vec::new(),
        bga_assets: Vec::new(),
        total_notes: 1,
        end_time: TimeUs(1_500_000),
    }
}

#[test]
fn build_render_snapshot_emits_visible_long_note() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(
        Arc::new(chart_with_long_note()),
        &profile,
        PlaySessionOptions::default(),
    );
    session.hispeed = 1.0;

    // render_now=0: start 500ms→0.25, end 1500ms→0.75 (lookahead 2s)
    let upcoming = build_render_snapshot(&session, TimeUs(0), &[], None);
    assert_eq!(upcoming.visible_long_notes.len(), 1);
    assert_eq!(upcoming.visible_long_notes[0].lane, Lane::Key1);
    assert_eq!(upcoming.visible_long_notes[0].head_y, 0.25);
    assert_eq!(upcoming.visible_long_notes[0].tail_y, 0.75);
}

#[test]
fn build_render_snapshot_clamps_held_long_note_head_to_judge_line() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(
        Arc::new(chart_with_long_note()),
        &profile,
        PlaySessionOptions::default(),
    );
    session.hispeed = 1.0;

    // render_now=1_000_000: 始端は判定ライン通過済み(負値→0.0)、終端は 0.25
    let held = build_render_snapshot(&session, TimeUs(1_000_000), &[], None);
    assert_eq!(held.visible_long_notes.len(), 1);
    assert_eq!(held.visible_long_notes[0].head_y, 0.0);
    assert_eq!(held.visible_long_notes[0].tail_y, 0.25);

    // 終端も通過したら非表示
    let passed = build_render_snapshot(&session, TimeUs(2_000_000), &[], None);
    assert!(passed.visible_long_notes.is_empty());
}

#[test]
fn cached_snapshot_remains_correct_after_time_rewind() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session = build_game_session(
        Arc::new(chart_with_long_note()),
        &profile,
        PlaySessionOptions::default(),
    );
    session.hispeed = 1.0;
    let cache = PlayRenderSnapshotCache::from_chart(&session.chart);
    let frames = BgaFrameCatalog::new();

    let later = build_render_snapshot_with_target_and_bga_frames_cached(
        &session,
        TimeUs(1_000_000),
        &[],
        None,
        None,
        None,
        &frames,
        &cache,
    );
    let earlier = build_render_snapshot_with_target_and_bga_frames_cached(
        &session,
        TimeUs(0),
        &[],
        None,
        None,
        None,
        &frames,
        &cache,
    );

    assert_eq!(later.visible_long_notes.len(), 1);
    assert_eq!(later.visible_long_notes[0].head_y, 0.0);
    assert_eq!(later.visible_long_notes[0].tail_y, 0.25);
    assert_eq!(earlier.visible_long_notes.len(), 1);
    assert_eq!(earlier.visible_long_notes[0].head_y, 0.25);
    assert_eq!(earlier.visible_long_notes[0].tail_y, 0.75);
}
