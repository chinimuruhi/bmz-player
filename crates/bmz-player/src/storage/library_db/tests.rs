use bmz_chart::hash::compute_chart_identity;
use bmz_chart::model::{ChartMetadata, LongNotePair, LongNoteStyle, NoteEvent, PlayableChart};
use bmz_core::course::{CourseConstraints, CourseDefinition, CourseEntry, CourseKind};
use bmz_core::ids::NoteId;
use bmz_core::lane::Lane;
use bmz_core::time::{ChartTick, TimeUs};

use super::*;
use crate::storage::migration::{LIBRARY_MIGRATIONS, run_migrations};

fn record_for_chart<'a>(path: &'a str, c: &'a PlayableChart) -> ChartImportRecord<'a> {
    ChartImportRecord {
        root_id: None,
        file_path: Path::new(path),
        file_size: 1,
        modified_at: 1,
        scanned_at: 1,
        chart: c,
    }
}

fn chart(title: &str) -> PlayableChart {
    PlayableChart {
        identity: compute_chart_identity(title.as_bytes()),
        metadata: ChartMetadata {
            title: title.to_string(),
            artist: "artist".to_string(),
            initial_bpm: 128.0,
            ..Default::default()
        },
        lane_notes: std::array::from_fn(|_| Vec::new()),
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
        total_notes: 0,
        end_time: TimeUs(10_000_000),
    }
}

fn course_with_entries(entries: Vec<CourseEntry>) -> CourseDefinition {
    CourseDefinition {
        key: "table:test#0".to_string(),
        title: "Test Course".to_string(),
        kind: CourseKind::Dan,
        entries,
        constraints: CourseConstraints::default(),
        trophies: Vec::new(),
        release: true,
    }
}

fn note(id: u32, lane: Lane, kind: NoteKind, time_us: i64) -> NoteEvent {
    NoteEvent {
        id: NoteId(id),
        lane,
        kind,
        tick: ChartTick(0),
        time: TimeUs(time_us),
        sound: None,
        damage: None,
    }
}

fn timing_event(
    time_us: i64,
    kind: bmz_chart::model::TimingEventKind,
) -> bmz_chart::model::TimingEvent {
    bmz_chart::model::TimingEvent { tick: ChartTick(0), time: TimeUs(time_us), kind }
}

/// STOP 後の resume エントリが正しく追加されるか確認。
/// 修正前は STOP 後も speed=0 のまま曲末まで続いていた。
#[test]
fn chart_speed_changes_emits_resume_after_stop() {
    use bmz_chart::model::TimingEventKind;
    let mut c = chart("stop_test");
    // BPM=128 で始まり、2 秒目に 0.5 秒の STOP
    c.timing_events.push(timing_event(2_000_000, TimingEventKind::Stop { duration_us: 500_000 }));
    let changes = chart_speed_changes(&c);
    // 期待: [{128, 0}, {0, 2000}, {128, 2500}, {128, 10000}]
    // STOP 直後の resume (speed=128 at 2500ms) が必須
    let resume = changes.iter().find(|c| c.time_ms == 2_500 && c.speed == 128.0);
    assert!(resume.is_some(), "resume entry after stop must exist: {changes:?}");
    // 末尾エントリが speed=0 になってはいけない
    assert_ne!(
        changes.last().unwrap().speed,
        0.0,
        "last entry must not be stop speed: {changes:?}"
    );
}

/// STOP 区間内に BPM 変化がある場合、STOP 終了後に新 BPM で再開すること。
#[test]
fn chart_speed_changes_resume_bpm_reflects_change_during_stop() {
    use bmz_chart::model::TimingEventKind;
    let mut c = chart("stop_bpm_change");
    // 1 秒目: STOP 2 秒間 (終了 3 秒)
    c.timing_events.push(timing_event(1_000_000, TimingEventKind::Stop { duration_us: 2_000_000 }));
    // 2 秒目 (STOP 中): BPM 200 に変化
    c.timing_events.push(timing_event(2_000_000, TimingEventKind::BpmChange { bpm: 200.0 }));
    let changes = chart_speed_changes(&c);
    // resume は STOP 終了 (3 秒) に BPM=200 で出るはず
    let resume = changes.iter().find(|c| c.time_ms == 3_000);
    assert!(resume.is_some_and(|r| r.speed == 200.0), "resume must use post-stop BPM: {changes:?}");
}

#[test]
fn upsert_chart_import_persists_file_chart_and_link() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase { conn };
    let mut chart = chart("song");
    chart.metadata.has_bga = true;
    let record = ChartImportRecord {
        root_id: None,
        file_path: Path::new("/songs/song.bms"),
        file_size: 123,
        modified_at: 1_700_000_001,
        scanned_at: 1_700_000_002,
        chart: &chart,
    };

    let chart_id = db.upsert_chart_import(&record).unwrap();

    assert_eq!(db.chart_id_by_sha256(chart.identity.file_sha256).unwrap(), Some(chart_id));
    let (path, parse_status, title, mode, ln_type, has_bga): (
            String,
            String,
            String,
            String,
            String,
            bool,
        ) =
            db.conn()
                .query_row(
                    "SELECT chart_files.path, chart_files.parse_status, charts.title, charts.mode, charts.ln_type, charts.has_bga
                    FROM chart_file_links
                    JOIN chart_files ON chart_files.id = chart_file_links.chart_file_id
                    JOIN charts ON charts.id = chart_file_links.chart_id",
                    [],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .unwrap();

    assert_eq!(path, "/songs/song.bms");
    assert_eq!(parse_status, "Parsed");
    assert_eq!(title, "song");
    assert_eq!(mode, "7K");
    assert_eq!(ln_type, "");
    assert!(has_bga);

    let analysis = db.chart_analysis_by_chart_id(chart_id).unwrap().unwrap();
    assert_eq!(analysis.total_gauge, 260.0);
    assert_eq!(analysis.main_bpm, 128.0);
}

#[test]
fn upsert_chart_import_backfills_unresolved_course_entries() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase { conn };
    let chart = chart("course song");
    let md5 = hash_to_hex(&chart.identity.file_md5);
    let sha256 = hash_to_hex(&chart.identity.file_sha256);
    let course = course_with_entries(vec![
        CourseEntry {
            title_hint: "SHA-256 match".to_string(),
            md5: Some(md5.clone()),
            sha256: Some(sha256),
            chart_id: None,
        },
        CourseEntry {
            title_hint: "MD5 fallback".to_string(),
            md5: Some(md5),
            sha256: Some("f".repeat(64)),
            chart_id: None,
        },
    ]);
    let course_id = db.upsert_course("table:test", &course, 0, 1).unwrap();
    assert!(
        db.list_course_entries(course_id)
            .unwrap()
            .iter()
            .all(|entry| entry.entry.chart_id.is_none())
    );

    let chart_id = db.upsert_chart_import(&record_for_chart("/songs/course.bms", &chart)).unwrap();

    let entries = db.list_course_entries(course_id).unwrap();
    assert_eq!(entries[0].entry.chart_id, Some(chart_id));
    assert_eq!(entries[1].entry.chart_id, Some(chart_id));
}

#[test]
fn successful_reimport_restores_course_link_after_import_failure() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase { conn };
    let chart = chart("recovered course song");
    let path = Path::new("/songs/recovered.bms");
    let record = record_for_chart("/songs/recovered.bms", &chart);
    let original_chart_id = db.upsert_chart_import(&record).unwrap();
    let course = course_with_entries(vec![CourseEntry {
        title_hint: "Recovered song".to_string(),
        md5: Some(hash_to_hex(&chart.identity.file_md5)),
        sha256: Some(hash_to_hex(&chart.identity.file_sha256)),
        chart_id: None,
    }]);
    let course_id = db.upsert_course("table:test", &course, 0, 1).unwrap();
    assert_eq!(
        db.list_course_entries(course_id).unwrap()[0].entry.chart_id,
        Some(original_chart_id)
    );

    db.upsert_failed_chart_file(None, path, 1, 2, 2, "temporary failure").unwrap();
    assert_eq!(db.list_course_entries(course_id).unwrap()[0].entry.chart_id, None);

    let recovered_chart_id = db.upsert_chart_import(&record).unwrap();
    assert_eq!(
        db.list_course_entries(course_id).unwrap()[0].entry.chart_id,
        Some(recovered_chart_id)
    );
}

#[test]
fn course_backfill_uses_oldest_duplicate_chart_id() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase { conn };
    let chart = chart("duplicate course song");
    let oldest_chart_id =
        db.upsert_chart_import(&record_for_chart("/songs/first.bms", &chart)).unwrap();
    let course = course_with_entries(vec![CourseEntry {
        title_hint: "Duplicate song".to_string(),
        md5: Some(hash_to_hex(&chart.identity.file_md5)),
        sha256: Some(hash_to_hex(&chart.identity.file_sha256)),
        chart_id: None,
    }]);
    let course_id = db.upsert_course("table:test", &course, 0, 1).unwrap();
    db.conn()
        .execute(
            "UPDATE course_entries SET chart_id = NULL WHERE course_id = ?1",
            params![course_id],
        )
        .unwrap();

    let duplicate_chart_id =
        db.upsert_chart_import(&record_for_chart("/songs/second.bms", &chart)).unwrap();

    assert!(duplicate_chart_id > oldest_chart_id);
    assert_eq!(db.list_course_entries(course_id).unwrap()[0].entry.chart_id, Some(oldest_chart_id));
}

#[test]
fn upsert_chart_import_persists_bms_total_separately_from_gauge_total() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase { conn };
    let mut chart = chart("bms total");
    chart.metadata.total = Some(320.0);
    chart.total_notes = 500;
    let record = ChartImportRecord {
        root_id: None,
        file_path: Path::new("/songs/total.bms"),
        file_size: 123,
        modified_at: 1_700_000_001,
        scanned_at: 1_700_000_002,
        chart: &chart,
    };
    let chart_id = db.upsert_chart_import(&record).unwrap();

    let stored: f64 = db
        .conn
        .query_row("SELECT bms_total FROM charts WHERE id = ?1", params![chart_id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(stored, 320.0);

    let listed = db.list_charts_by_ids(&[chart_id]).unwrap().pop().unwrap();
    assert_eq!(listed.bms_total, 320.0);
}

#[test]
fn upsert_chart_import_persists_ln_profile_and_pair_counts() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase { conn };
    let mut chart = chart("defined cn");
    chart.metadata.long_note_mode = LongNoteMode::Cn;
    chart.metadata.long_note_mode_defined = true;
    for (index, mode) in
        [None, Some(LongNoteMode::Ln), Some(LongNoteMode::Cn), Some(LongNoteMode::Hcn)]
            .into_iter()
            .enumerate()
    {
        chart.long_notes.push(LongNotePair {
            lane: Lane::Key1,
            style: LongNoteStyle::ChannelPair,
            mode,
            start_note_id: NoteId((index * 2 + 1) as u32),
            end_note_id: NoteId((index * 2 + 2) as u32),
            start_tick: ChartTick(0),
            end_tick: ChartTick(192),
            start_time: TimeUs(1_000_000),
            end_time: TimeUs(2_000_000),
            sound: None,
        });
    }

    let chart_id =
        db.upsert_chart_import(&record_for_chart("/songs/defined-cn.bms", &chart)).unwrap();
    let row = db.list_charts_by_ids(&[chart_id]).unwrap().pop().unwrap();

    assert!(row.ln_profile.has_undefined_ln);
    assert!(row.ln_profile.has_defined_ln);
    assert!(row.ln_profile.has_defined_cn);
    assert!(row.ln_profile.has_defined_hcn);
    assert_eq!(
        row.ln_counts,
        ChartLnCounts {
            undefined_ln_pairs: 1,
            defined_ln_pairs: 1,
            defined_cn_pairs: 1,
            defined_hcn_pairs: 1,
        }
    );
    assert_eq!(row.scored_total_notes(LnScorePolicy::ForceCn), 4);

    chart.long_notes.truncate(1);
    let updated_id =
        db.upsert_chart_import(&record_for_chart("/songs/defined-cn.bms", &chart)).unwrap();
    assert_eq!(updated_id, chart_id);
    let updated = db.list_charts_by_ids(&[chart_id]).unwrap().pop().unwrap();
    assert_eq!(updated.ln_counts.undefined_ln_pairs, 1);
    assert_eq!(updated.ln_counts.defined_ln_pairs, 0);
    assert_eq!(updated.ln_counts.defined_cn_pairs, 0);
    assert_eq!(updated.ln_counts.defined_hcn_pairs, 0);
}

#[test]
fn upsert_chart_import_persists_source_url_without_raw_headers() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase { conn };
    let mut chart = chart("url song");
    chart.metadata.source_url = "http://example.com/bms".to_string();
    chart.metadata.append_url = "http://example.com/append".to_string();
    chart.metadata.bms_headers.insert("TITLE".to_string(), "url song".to_string());
    chart.metadata.bms_headers.insert("URL".to_string(), "http://example.com/bms".to_string());
    let record = ChartImportRecord {
        root_id: None,
        file_path: Path::new("/songs/url.bms"),
        file_size: 123,
        modified_at: 1_700_000_001,
        scanned_at: 1_700_000_002,
        chart: &chart,
    };

    let chart_id = db.upsert_chart_import(&record).unwrap();
    let (source_url, append_url, headers_json): (String, String, String) = db
        .conn()
        .query_row(
            "SELECT source_url, append_url, headers_json FROM charts WHERE id = ?1",
            params![chart_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert_eq!(source_url, "http://example.com/bms");
    assert_eq!(append_url, "http://example.com/append");
    assert_eq!(headers_json, "{}");
}

#[test]
fn upsert_chart_import_persists_chart_analysis_distribution() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase { conn };
    let mut chart = chart("analysis");
    chart.total_notes = 3;
    chart.end_time = TimeUs(3_000_000);
    chart.lane_notes[Lane::Key1.index()].push(note(1, Lane::Key1, NoteKind::Tap, 100_000));
    chart.lane_notes[Lane::Scratch.index()].push(note(
        2,
        Lane::Scratch,
        NoteKind::LongStart,
        1_000_000,
    ));
    chart.lane_notes[Lane::Scratch.index()].push(note(
        3,
        Lane::Scratch,
        NoteKind::LongEnd,
        2_000_000,
    ));
    chart.lane_notes[Lane::Key2.index()].push(note(4, Lane::Key2, NoteKind::Mine, 1_500_000));
    chart.long_notes.push(LongNotePair {
        lane: Lane::Scratch,
        style: LongNoteStyle::ChannelPair,
        mode: None,
        start_note_id: NoteId(2),
        end_note_id: NoteId(3),
        start_tick: ChartTick(0),
        end_tick: ChartTick(192),
        start_time: TimeUs(1_000_000),
        end_time: TimeUs(2_000_000),
        sound: None,
    });

    let chart_id =
        db.upsert_chart_import(&record_for_chart("/songs/analysis.bms", &chart)).unwrap();
    let analysis = db.chart_analysis_by_chart_id(chart_id).unwrap().unwrap();

    assert_eq!(analysis.normal_notes, 1);
    assert_eq!(analysis.long_notes, 1);
    assert_eq!(analysis.scratch_notes, 0);
    assert_eq!(analysis.long_scratch_notes, 1);
    assert_eq!(analysis.distribution[0].key_taps, 1);
    assert_eq!(analysis.distribution[1].scratch_long_heads, 1);
    assert_eq!(analysis.distribution[1].scratch_long_bodies, 0);
    assert_eq!(analysis.distribution[1].mines, 1);
    assert_eq!(analysis.distribution[2].scratch_long_bodies, 1);
    assert_eq!(analysis.lane_notes[Lane::Scratch.index()].long_notes, 1);
    assert_eq!(analysis.lane_notes[Lane::Key2.index()].mines, 1);
    assert_eq!(analysis.peak_density, 1.0);

    let stored_distribution: String = db
        .conn()
        .query_row(
            "SELECT distribution_json FROM chart_analysis WHERE chart_id = ?1",
            params![chart_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(stored_distribution.starts_with('#'));
    assert_eq!(stored_distribution.len(), 1 + analysis.distribution.len() * 14);
}

#[test]
fn chart_normalization_analysis_roundtrips_and_rescan_clears_it() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase { conn };
    let chart = chart("normalization");

    let chart_id =
        db.upsert_chart_import(&record_for_chart("/songs/normalization.bms", &chart)).unwrap();
    assert!(db.chart_normalization_analysis_by_chart_id(chart_id).unwrap().is_none());

    db.write_chart_normalization_analysis(
        chart_id,
        ChartNormalizationAnalysis { loudness_lufs: -10.5 },
    )
    .unwrap();
    let stored = db.chart_normalization_analysis_by_chart_id(chart_id).unwrap().unwrap();
    assert_eq!(stored.loudness_lufs, -10.5);

    db.upsert_chart_import(&record_for_chart("/songs/normalization.bms", &chart)).unwrap();
    assert!(db.chart_normalization_analysis_by_chart_id(chart_id).unwrap().is_none());
}

#[test]
fn chart_analysis_counts_defined_cn_long_end_independently_of_chart_default() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase { conn };
    let mut chart = chart("mixed analysis");
    chart.metadata.long_note_mode = LongNoteMode::Ln;
    chart.total_notes = 1;
    chart.end_time = TimeUs(2_000_000);
    chart.lane_notes[Lane::Key1.index()].push(note(1, Lane::Key1, NoteKind::LongStart, 1_000_000));
    chart.lane_notes[Lane::Key1.index()].push(note(2, Lane::Key1, NoteKind::LongEnd, 2_000_000));
    chart.long_notes.push(LongNotePair {
        lane: Lane::Key1,
        style: LongNoteStyle::ChannelPair,
        mode: Some(LongNoteMode::Cn),
        start_note_id: NoteId(1),
        end_note_id: NoteId(2),
        start_tick: ChartTick(0),
        end_tick: ChartTick(192),
        start_time: TimeUs(1_000_000),
        end_time: TimeUs(2_000_000),
        sound: None,
    });

    let chart_id =
        db.upsert_chart_import(&record_for_chart("/songs/mixed-analysis.bms", &chart)).unwrap();
    let analysis = db.chart_analysis_by_chart_id(chart_id).unwrap().unwrap();

    assert_eq!(analysis.long_notes, 2);
    assert_eq!(analysis.lane_notes[Lane::Key1.index()].long_notes, 2);
    assert_eq!(analysis.distribution[2].key_long_heads, 1);
    assert_eq!(analysis.total_gauge, gauge_total_for_chart(None, 2));
}

#[test]
fn chart_analysis_caps_extreme_distribution_length() {
    let mut chart = chart("long analysis");
    chart.end_time = TimeUs(i64::MAX);
    chart.long_notes.push(LongNotePair {
        lane: Lane::Key1,
        style: LongNoteStyle::ChannelPair,
        mode: None,
        start_note_id: NoteId(1),
        end_note_id: NoteId(2),
        start_tick: ChartTick(0),
        end_tick: ChartTick(0),
        start_time: TimeUs(0),
        end_time: TimeUs(i64::MAX),
        sound: None,
    });

    let analysis = ChartAnalysis::from_chart(&chart);

    assert_eq!(analysis.distribution.len(), MAX_ANALYSIS_DISTRIBUTION_SECONDS);
}

#[test]
fn chart_analysis_trims_distribution_to_last_note_second() {
    let mut chart = chart("trim analysis");
    chart.end_time = TimeUs(i64::MAX);
    chart.lane_notes[Lane::Key1.index()].push(note(1, Lane::Key1, NoteKind::Tap, 2_000_000));

    let analysis = ChartAnalysis::from_chart(&chart);

    assert_eq!(analysis.distribution.len(), 3);
    assert_eq!(analysis.distribution[2].key_taps, 1);
}

#[test]
fn chart_analysis_excludes_invisible_notes_from_density() {
    let mut chart = chart("invisible analysis");
    chart.lane_notes[Lane::Key1.index()].push(note(1, Lane::Key1, NoteKind::Tap, 0));
    chart.lane_notes[Lane::Key1.index()].push(note(2, Lane::Key1, NoteKind::Invisible, 0));
    chart.total_notes = 1;

    let analysis = ChartAnalysis::from_chart(&chart);

    assert_eq!(analysis.normal_notes, 1);
    assert_eq!(analysis.distribution[0].key_taps, 1);
}

#[test]
fn compact_distribution_round_trips_and_accepts_legacy_json() {
    let distribution = vec![
        ChartDistributionSecond {
            scratch_long_heads: 1,
            scratch_long_bodies: 2,
            scratch_taps: 3,
            key_long_heads: 4,
            key_long_bodies: 5,
            key_taps: 6,
            mines: 7,
        },
        ChartDistributionSecond { key_taps: 36 * 36, ..Default::default() },
    ];

    let compact = encode_distribution_compact(&distribution);
    assert_eq!(compact.len(), 1 + distribution.len() * 14);
    let decoded = decode_distribution(&compact);
    assert_eq!(decoded[0], distribution[0]);
    assert_eq!(decoded[1].key_taps, 36 * 36 - 1);

    let legacy_json = serde_json::to_string(&distribution).unwrap();
    assert_eq!(decode_distribution(&legacy_json), distribution);
}

#[test]
fn replace_import_warnings_replaces_previous_rows() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase { conn };
    let chart = chart("song");
    let record = ChartImportRecord {
        root_id: None,
        file_path: Path::new("/songs/song.bms"),
        file_size: 123,
        modified_at: 1,
        scanned_at: 2,
        chart: &chart,
    };
    db.upsert_chart_import(&record).unwrap();
    let chart_file_id = db.chart_file_id_by_path(record.file_path).unwrap().unwrap();

    db.replace_import_warnings(
        chart_file_id,
        &[ImportWarning::UnsupportedChannel { channel: 99 }],
        3,
    )
    .unwrap();
    db.replace_import_warnings(
        chart_file_id,
        &[ImportWarning::MissingWavDefinition { key: 10 }],
        4,
    )
    .unwrap();

    let (count, code): (u32, String) = db
        .conn()
        .query_row("SELECT COUNT(*), MAX(code) FROM chart_import_warnings", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();

    assert_eq!(count, 1);
    assert_eq!(code, "MissingWavDefinition");
}

#[test]
fn upsert_root_updates_existing_row() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);

    let first = db.upsert_root(Path::new("/songs"), true, true).unwrap();
    let second = db.upsert_root(Path::new("/songs"), false, false).unwrap();
    db.update_root_scanned_at(first, 42).unwrap();

    let (count, enabled, recursive, last_scan_at): (u32, bool, bool, i64) = db
        .conn()
        .query_row("SELECT COUNT(*), enabled, recursive, last_scan_at FROM roots", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap();

    assert_eq!(first, second);
    assert_eq!(count, 1);
    assert!(!enabled);
    assert!(!recursive);
    assert_eq!(last_scan_at, 42);
}

#[test]
fn list_charts_orders_by_title() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);
    let alpha = chart("Alpha");
    let beta = chart("beta");

    db.upsert_chart_import(&ChartImportRecord {
        root_id: None,
        file_path: Path::new("/songs/beta.bms"),
        file_size: 1,
        modified_at: 1,
        scanned_at: 1,
        chart: &beta,
    })
    .unwrap();
    db.upsert_chart_import(&ChartImportRecord {
        root_id: None,
        file_path: Path::new("/songs/alpha.bms"),
        file_size: 1,
        modified_at: 1,
        scanned_at: 1,
        chart: &alpha,
    })
    .unwrap();

    let charts = db.list_charts(10, 0).unwrap();

    assert_eq!(charts.len(), 2);
    assert_eq!(charts[0].title, "Alpha");
    assert_eq!(charts[1].title, "beta");
    assert_eq!(charts[0].mode, "7K");
    assert_eq!(charts[0].length_ms, 10_000);
}

#[test]
fn search_charts_matches_substring_across_metadata_fields_case_insensitively() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);

    let mut by_title = chart("Blue Sky");
    by_title.metadata.artist = "Composer A".to_string();
    by_title.metadata.genre = "Trance".to_string();
    let mut by_artist = chart("untitled");
    by_artist.metadata.artist = "DJ Blueprint".to_string();
    let mut by_genre = chart("other");
    by_genre.metadata.artist = "Nobody".to_string();
    by_genre.metadata.genre = "Drum & Bass (BLUE mix)".to_string();
    let mut unrelated = chart("Sunset");
    unrelated.metadata.artist = "Solo".to_string();
    unrelated.metadata.genre = "Ambient".to_string();

    for (path, c) in [
        ("/songs/a.bms", &by_title),
        ("/songs/b.bms", &by_artist),
        ("/songs/c.bms", &by_genre),
        ("/songs/d.bms", &unrelated),
    ] {
        db.upsert_chart_import(&ChartImportRecord {
            root_id: None,
            file_path: Path::new(path),
            file_size: 1,
            modified_at: 1,
            scanned_at: 1,
            chart: c,
        })
        .unwrap();
    }

    let hits = db.search_charts("blue").unwrap();
    let titles: Vec<&str> = hits.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles.len(), 3, "expected three matches, got {titles:?}");
    assert!(titles.contains(&"Blue Sky"));
    assert!(titles.contains(&"untitled"));
    assert!(titles.contains(&"other"));

    assert!(db.search_charts("nonexistent_query_xyz").unwrap().is_empty());
}

#[test]
fn search_charts_treats_like_wildcards_as_literal() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);

    let mut with_percent = chart("100% pure");
    with_percent.metadata.artist = "p".to_string();
    let mut without = chart("zero");
    without.metadata.artist = "z".to_string();

    for (path, c) in [("/songs/a.bms", &with_percent), ("/songs/b.bms", &without)] {
        db.upsert_chart_import(&ChartImportRecord {
            root_id: None,
            file_path: Path::new(path),
            file_size: 1,
            modified_at: 1,
            scanned_at: 1,
            chart: c,
        })
        .unwrap();
    }

    let hits = db.search_charts("%").unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "100% pure");
}

#[test]
fn primary_chart_file_path_returns_linked_file() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);
    let chart = chart("song");
    let chart_id = db
        .upsert_chart_import(&ChartImportRecord {
            root_id: None,
            file_path: Path::new("/songs/song.bms"),
            file_size: 1,
            modified_at: 1,
            scanned_at: 1,
            chart: &chart,
        })
        .unwrap();

    assert_eq!(db.primary_chart_file_path(chart_id).unwrap(), Some("/songs/song.bms".to_string()));
    assert_eq!(db.primary_chart_file_path(chart_id + 1).unwrap(), None);
}

#[test]
fn chart_id_by_chart_file_path_resolves_linked_chart() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);
    let chart = chart("boot");
    let chart_id = db
        .upsert_chart_import(&ChartImportRecord {
            root_id: None,
            file_path: Path::new("/songs/boot.bms"),
            file_size: 1,
            modified_at: 1,
            scanned_at: 1,
            chart: &chart,
        })
        .unwrap();

    assert_eq!(
        db.chart_id_by_chart_file_path(Path::new("/songs/boot.bms")).unwrap(),
        Some(chart_id)
    );
    assert_eq!(db.chart_id_by_chart_file_path(Path::new("/missing.bms")).unwrap(), None);
}

#[test]
fn upsert_failed_chart_file_records_failure_status_and_warning() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);

    let chart_file_id = db
        .upsert_failed_chart_file(None, Path::new("/songs/broken.bms"), 10, 1, 2, "broken")
        .unwrap();

    let (status, code): (String, String) = db
        .conn()
        .query_row(
            "SELECT chart_files.parse_status, chart_import_warnings.code
                FROM chart_files
                JOIN chart_import_warnings ON chart_import_warnings.chart_file_id = chart_files.id
                WHERE chart_files.id = ?1",
            params![chart_file_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(status, "Failed");
    assert_eq!(code, "ImportFailed");
}

#[test]
fn failed_reimport_removes_previous_chart_link_and_orphan() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);
    let path = Path::new("/songs/unsupported.bmson");
    db.upsert_chart_import(&record_for_chart(path.to_str().unwrap(), &chart("old"))).unwrap();

    db.upsert_failed_chart_file(None, path, 10, 2, 3, "unsupported chart mode").unwrap();

    assert_eq!(db.chart_id_by_chart_file_path(path).unwrap(), None);
    let chart_count: i64 =
        db.conn().query_row("SELECT COUNT(*) FROM charts", [], |row| row.get(0)).unwrap();
    assert_eq!(chart_count, 0);
}

#[test]
fn list_failed_chart_files_returns_failures() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);
    db.upsert_failed_chart_file(None, Path::new("/songs/broken.bms"), 10, 1, 2, "broken").unwrap();

    let failed = db.list_failed_chart_files(10, 0).unwrap();

    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].path, "/songs/broken.bms");
    assert_eq!(failed[0].message, "broken");
    assert_eq!(failed[0].scanned_at, 2);
}

#[test]
fn upsert_chart_import_updates_chart_in_place_when_content_changes() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);

    let v1 = chart("content-v1");
    let id1 = db.upsert_chart_import(&record_for_chart("/songs/track.bms", &v1)).unwrap();

    let v2 = chart("content-v2");
    let id2 = db.upsert_chart_import(&record_for_chart("/songs/track.bms", &v2)).unwrap();

    assert_eq!(id1, id2, "same path must return the same chart id");

    let count: i64 = db.conn().query_row("SELECT COUNT(*) FROM charts", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 1, "re-import of same path must not create an extra chart row");

    let title: String = db
        .conn()
        .query_row("SELECT title FROM charts WHERE id = ?1", params![id2], |r| r.get(0))
        .unwrap();
    assert_eq!(title, "content-v2");

    let link_count: i64 =
        db.conn().query_row("SELECT COUNT(*) FROM chart_file_links", [], |r| r.get(0)).unwrap();
    assert_eq!(link_count, 1);
}

#[test]
fn upsert_chart_import_creates_separate_charts_for_different_paths_with_same_sha256() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);

    let same_chart = chart("duplicate");
    let id_a =
        db.upsert_chart_import(&record_for_chart("/songs/a/track.bms", &same_chart)).unwrap();
    let id_b =
        db.upsert_chart_import(&record_for_chart("/songs/b/track.bms", &same_chart)).unwrap();

    assert_ne!(id_a, id_b, "different paths must produce separate chart records");

    let count: i64 = db.conn().query_row("SELECT COUNT(*) FROM charts", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 2);
}

#[test]
fn charts_by_md5s_prefers_newest_chart_id() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);

    let same_chart = chart("duplicate");
    let stale_id =
        db.upsert_chart_import(&record_for_chart("/songs/a/track.bms", &same_chart)).unwrap();
    let fresh_id =
        db.upsert_chart_import(&record_for_chart("/songs/b/track.bms", &same_chart)).unwrap();
    assert!(stale_id < fresh_id);

    let md5 = hash_to_hex(&same_chart.identity.file_md5);
    let resolved = db.charts_by_md5s(&[md5.as_str()]).unwrap();

    assert_eq!(resolved.get(&md5).map(|chart| chart.chart_id), Some(fresh_id));
}

#[test]
fn charts_by_md5s_batches_more_hashes_than_one_sqlite_variable_chunk() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);

    let first = chart("batch-first");
    let second = chart("batch-second");
    let first_id = db.upsert_chart_import(&record_for_chart("/songs/first.bms", &first)).unwrap();
    let second_id =
        db.upsert_chart_import(&record_for_chart("/songs/second.bms", &second)).unwrap();
    let first_md5 = hash_to_hex(&first.identity.file_md5);
    let second_md5 = hash_to_hex(&second.identity.file_md5);

    let mut hashes = (0..CHART_HASH_LOOKUP_BATCH_SIZE * 2 + 1)
        .map(|index| format!("{index:032x}"))
        .collect::<Vec<_>>();
    hashes.push(first_md5.clone());
    hashes.push(second_md5.clone());
    hashes.push(first_md5.clone());
    let hash_refs = hashes.iter().map(String::as_str).collect::<Vec<_>>();

    let resolved = db.charts_by_md5s(&hash_refs).unwrap();

    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved.get(&first_md5).map(|chart| chart.chart_id), Some(first_id));
    assert_eq!(resolved.get(&second_md5).map(|chart| chart.chart_id), Some(second_id));
}

#[test]
fn chart_analysis_summaries_batch_more_ids_than_one_sqlite_variable_chunk() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);

    let first = chart("analysis-batch-first");
    let second = chart("analysis-batch-second");
    let first_id = db.upsert_chart_import(&record_for_chart("/songs/first.bms", &first)).unwrap();
    let second_id =
        db.upsert_chart_import(&record_for_chart("/songs/second.bms", &second)).unwrap();
    let mut ids =
        (10_000..10_000 + CHART_ANALYSIS_LOOKUP_BATCH_SIZE as i64 * 2 + 1).collect::<Vec<_>>();
    ids.push(first_id);
    ids.push(second_id);
    ids.push(first_id);

    let summaries = db.chart_analysis_summaries_by_chart_ids(&ids).unwrap();

    assert_eq!(summaries.len(), 2);
    assert!(summaries.contains_key(&first_id));
    assert!(summaries.contains_key(&second_id));
}

#[test]
fn chart_file_fingerprint_reads_imported_file_metadata() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);
    let chart = chart("song");
    db.upsert_chart_import(&ChartImportRecord {
        root_id: None,
        file_path: Path::new("/songs/song.bms"),
        file_size: 123,
        modified_at: 456,
        scanned_at: 789,
        chart: &chart,
    })
    .unwrap();

    assert_eq!(
        db.chart_file_fingerprint(Path::new("/songs/song.bms")).unwrap(),
        Some(ChartFileFingerprint {
            file_size: 123,
            modified_at: 456,
            import_version: CHART_IMPORT_VERSION,
        })
    );
}

#[test]
fn folder_navigation_normalizes_backslash_separators() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);
    let chart = chart("song");
    // file_path はスラッシュ区切りで与える（Path::parent() の OS 依存を避ける）。
    // folder_path は "G:/BMS/INSANE/sub" として保存される。
    db.upsert_chart_import(&ChartImportRecord {
        root_id: None,
        file_path: Path::new("G:/BMS/INSANE/sub/song.bms"),
        file_size: 1,
        modified_at: 1,
        scanned_at: 1,
        chart: &chart,
    })
    .unwrap();

    // バックスラッシュ区切りの引数でも、スラッシュ保存された行が見つかること。
    assert_eq!(db.list_child_folder_names("G:\\BMS\\INSANE").unwrap(), vec!["sub".to_string()]);
    assert_eq!(db.list_charts_in_folder("G:\\BMS\\INSANE\\sub").unwrap().len(), 1);
}

#[test]
fn list_descendant_folder_paths_returns_only_strict_descendants() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);
    for (i, path) in [
        "G:/BMS/INSANE/a/song.bms",
        "G:/BMS/INSANE/b/c/song.bms",
        "G:/BMS/INSANE/song.bms", // 親そのもの直下: 子孫扱いしない
        "G:/BMS/OTHER/song.bms",  // 別ルート: 含まれない
    ]
    .iter()
    .enumerate()
    {
        let c = chart(&format!("s{i}"));
        db.upsert_chart_import(&ChartImportRecord {
            root_id: None,
            file_path: Path::new(path),
            file_size: 1,
            modified_at: 1,
            scanned_at: 1,
            chart: &c,
        })
        .unwrap();
    }

    let mut got = db.list_descendant_folder_paths("G:/BMS/INSANE").unwrap();
    got.sort();
    assert_eq!(got, vec!["G:/BMS/INSANE/a", "G:/BMS/INSANE/b/c"]);
}

#[test]
fn list_charts_in_folders_collects_charts_across_paths() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);
    db.upsert_chart_import(&record_for_chart("/songs/a/song.bms", &chart("A"))).unwrap();
    db.upsert_chart_import(&record_for_chart("/songs/b/song.bms", &chart("B"))).unwrap();
    db.upsert_chart_import(&record_for_chart("/songs/c/song.bms", &chart("C"))).unwrap();

    let got = db.list_charts_in_folders(&["/songs/a", "/songs/c"]).unwrap();
    let titles: Vec<_> = got.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, vec!["A", "C"]);

    assert!(db.list_charts_in_folders(&[]).unwrap().is_empty());
}

#[test]
fn charts_hash_columns_are_lowercase_hex_text() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
    let mut db = LibraryDatabase::from_connection(conn);
    let chart = chart("song");
    db.upsert_chart_import(&record_for_chart("/songs/song.bms", &chart)).unwrap();

    let (md5_typeof, sha256_typeof, md5_hex, sha256_hex): (String, String, String, String) = db
        .conn()
        .query_row("SELECT typeof(md5), typeof(sha256), md5, sha256 FROM charts", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap();
    assert_eq!(md5_typeof, "text");
    assert_eq!(sha256_typeof, "text");
    assert_eq!(md5_hex.len(), 32);
    assert_eq!(sha256_hex.len(), 64);
    assert!(md5_hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert!(sha256_hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));

    // chart_files も同様に小文字 hex TEXT。
    let (cf_md5_typeof, cf_sha256_typeof): (String, String) = db
        .conn()
        .query_row("SELECT typeof(md5), typeof(sha256) FROM chart_files", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(cf_md5_typeof, "text");
    assert_eq!(cf_sha256_typeof, "text");
}

#[test]
fn list_charts_with_level_in_table_uses_hash_indexes() {
    // 難易度表結合が `c.md5 = dte.md5` の通常 JOIN になり、`idx_charts_md5` /
    // `idx_charts_sha256` でルックアップされることを EXPLAIN QUERY PLAN で確認する。
    // 関数結合（`lower(hex(c.md5)) = dte.md5`）に戻ると SCAN charts になる。
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();

    let plan: Vec<String> = conn
        .prepare(
            "EXPLAIN QUERY PLAN
                SELECT c.id FROM difficulty_table_entries dte
                JOIN difficulty_tables dt ON dt.id = dte.table_id
                JOIN charts c ON c.md5 = dte.md5
                WHERE dt.source_url = ?1 AND length(dte.md5) >= 24",
        )
        .unwrap()
        .query_map(params!["http://example.com/"], |row| row.get::<_, String>(3))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    let combined = plan.join("\n");
    assert!(
        combined.contains("idx_charts_md5"),
        "expected idx_charts_md5 to be used, got:\n{combined}"
    );
    assert!(
        !combined.contains("SCAN c "),
        "expected charts to be searched via index, not full scanned:\n{combined}"
    );
}
