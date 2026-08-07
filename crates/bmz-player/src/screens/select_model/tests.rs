use bmz_chart::hash::compute_chart_identity;
use bmz_chart::model::{ChartMetadata, LongNotePair, LongNoteStyle, PlayableChart};
use bmz_core::clear::{ClearType, GaugeType};
use bmz_core::ids::NoteId;
use bmz_core::judge::{Judge, TimingSide};
use bmz_core::lane::Lane;
use bmz_core::time::{ChartTick, TimeUs};
use bmz_gameplay::judge::model::JudgementEvent;
use bmz_gameplay::score::ScoreState;
use rusqlite::Connection;

use super::*;

use crate::storage::common::configure_connection;
use crate::storage::library_db::{ChartImportRecord, LibraryDatabase};
use crate::storage::migration::{
    COLLECTION_MIGRATIONS, LIBRARY_MIGRATIONS, SCORE_MIGRATIONS, run_migrations,
};
use crate::storage::score_db::{ScoreDatabase, ScoreRecord};

fn open_in_memory_dbs() -> (LibraryDatabase, ScoreDatabase) {
    let mut library_conn = Connection::open_in_memory().unwrap();
    configure_connection(&library_conn).unwrap();
    run_migrations(&mut library_conn, LIBRARY_MIGRATIONS).unwrap();
    let mut score_conn = Connection::open_in_memory().unwrap();
    configure_connection(&score_conn).unwrap();
    run_migrations(&mut score_conn, SCORE_MIGRATIONS).unwrap();
    (LibraryDatabase::from_connection(library_conn), ScoreDatabase::from_connection(score_conn))
}

fn open_in_memory_collection_db() -> CollectionDatabase {
    let mut collection_conn = Connection::open_in_memory().unwrap();
    configure_connection(&collection_conn).unwrap();
    run_migrations(&mut collection_conn, COLLECTION_MIGRATIONS).unwrap();
    CollectionDatabase::from_connection(collection_conn)
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

fn record_for_chart<'a>(path: &'a str, chart: &'a PlayableChart) -> ChartImportRecord<'a> {
    ChartImportRecord {
        root_id: None,
        file_path: std::path::Path::new(path),
        file_size: 1,
        modified_at: 1,
        scanned_at: 1,
        chart,
    }
}

fn undefined_ln_pair() -> LongNotePair {
    LongNotePair {
        lane: Lane::Key1,
        style: LongNoteStyle::ChannelPair,
        mode: None,
        start_note_id: NoteId(10),
        end_note_id: NoteId(11),
        start_tick: ChartTick(0),
        end_tick: ChartTick(192),
        start_time: TimeUs(0),
        end_time: TimeUs(1_000_000),
        sound: None,
    }
}

#[test]
fn course_note_preview_uses_auto_fallback_and_force_priority() {
    let (mut library_db, _) = open_in_memory_dbs();
    let mut source = chart("course ln preview");
    let mut pair = undefined_ln_pair();
    pair.mode = Some(bmz_chart::model::LongNoteMode::Ln);
    source.long_notes.push(pair);
    source.total_notes = 1;
    library_db
        .upsert_chart_import(&record_for_chart("/songs/course-ln-preview.bms", &source))
        .unwrap();
    let stored = library_db.list_all_charts().unwrap().pop().unwrap();

    assert_eq!(
        course_chart_total_notes(&stored, LnPolicySetting::AutoLn, CourseLnConstraint::Cn,),
        1,
        "AUTO must preserve the chart's explicitly typed LN",
    );
    assert_eq!(
        course_chart_total_notes(&stored, LnPolicySetting::ForceHcn, CourseLnConstraint::Ln,),
        2,
        "FORCE(HCN) must ignore the course LN constraint",
    );
}

fn difficulty_table_for_md5(
    md5: &[u8; 16],
    symbol: &str,
    level: &str,
) -> crate::difficulty_table::FetchedDifficultyTable {
    use crate::difficulty_table::{FetchedDifficultyTable, FetchedTableEntry};
    FetchedDifficultyTable {
        source_url: format!("https://example.com/{symbol}/"),
        head_url: format!("https://example.com/{symbol}/header.json"),
        name: "Table".to_string(),
        symbol: symbol.to_string(),
        level_order: vec![level.to_string()],
        entries: vec![FetchedTableEntry {
            level: level.to_string(),
            md5: hash_to_hex(md5),
            sha256: String::new(),
            title: String::new(),
            artist: String::new(),
            comment: String::new(),
            ..FetchedTableEntry::default()
        }],
        courses: Vec::new(),
        fetched_at: 0,
    }
}

fn difficulty_table_for_sha256(
    sha256: &[u8; 32],
    symbol: &str,
    level: &str,
) -> crate::difficulty_table::FetchedDifficultyTable {
    use crate::difficulty_table::{FetchedDifficultyTable, FetchedTableEntry};
    FetchedDifficultyTable {
        source_url: format!("https://example.com/{symbol}-sha/"),
        head_url: format!("https://example.com/{symbol}-sha/header.json"),
        name: "Table SHA".to_string(),
        symbol: symbol.to_string(),
        level_order: vec![level.to_string()],
        entries: vec![FetchedTableEntry {
            level: level.to_string(),
            md5: String::new(),
            sha256: hash_to_hex(sha256),
            title: String::new(),
            artist: String::new(),
            comment: String::new(),
            ..FetchedTableEntry::default()
        }],
        courses: Vec::new(),
        fetched_at: 0,
    }
}

fn score_for_chart(chart_sha256: [u8; 32]) -> ScoreRecord {
    let mut score = ScoreState::default();
    score.apply(&JudgementEvent {
        note_id: Some(NoteId(1)),
        lane: bmz_core::lane::Lane::Key1,
        judge: Judge::PGreat,
        side: TimingSide::Slow,
        delta: TimeUs(0),
        time: TimeUs(0),
        affects_score: true,
    });

    ScoreRecord {
        chart_sha256,
        ln_policy: LnScorePolicy::ForceLn,
        double_option: crate::select_options::DoubleOptionScoreBucket::Off,
        applied_double_option: crate::select_options::DoubleOption::Off,
        played_at: 1_700_000_030,
        clear_type: ClearType::Normal,
        gauge_type: Some(GaugeType::Normal),
        gauge_value: Some(80.0),
        total_notes: 1,
        playtime_seconds: 0,
        score,
        count_unprocessed_notes: false,
        random_seed: None,
        seed_scheme: String::new(),
        arrange: "Normal".to_string(),
        arrange_2p: "Normal".to_string(),
        gauge_option: String::new(),
        rule_mode: String::new(),
        assist_mask: 0,
        autoplay: false,
        device_type: bmz_core::input::InputDeviceKind::Keyboard,
        replay_path: String::new(),
        source_kind: crate::storage::score_db::ScoreSourceKind::Local,
    }
}

#[path = "tests/cases_01.rs"]
mod cases_01;
#[path = "tests/cases_02.rs"]
mod cases_02;
