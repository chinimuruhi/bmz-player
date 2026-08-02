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
        layered_sounds: Vec::new(),
        damage: None,
    }
}

fn timing_event(
    time_us: i64,
    kind: bmz_chart::model::TimingEventKind,
) -> bmz_chart::model::TimingEvent {
    bmz_chart::model::TimingEvent { tick: ChartTick(0), time: TimeUs(time_us), kind }
}

#[path = "tests/cases_01.rs"]
mod cases_01;
#[path = "tests/cases_02.rs"]
mod cases_02;
#[path = "tests/cases_03.rs"]
mod cases_03;
