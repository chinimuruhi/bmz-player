use super::*;

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
        layered_sounds: Vec::new(),
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
        layered_sounds: Vec::new(),
        damage: None,
    }];
    chart.lane_notes[Lane::Key3.index()] = vec![NoteEvent {
        id: NoteId(3),
        lane: Lane::Key3,
        kind: NoteKind::Mine,
        tick: ChartTick(5_760),
        time: TimeUs(3_000_000),
        sound: None,
        layered_sounds: Vec::new(),
        damage: Some(10.0),
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
    session.lanecover_enabled = true;
    session.lane_cover_visible = true;

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
