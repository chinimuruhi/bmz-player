use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

use bmz_audio::loader::LoadedSampleStatus;
use bmz_chart::hash::compute_chart_identity;
use bmz_chart::model::{ChartMetadata, PlayableChart, SoundAssetRef};
use bmz_core::clear::GaugeType;
use bmz_core::ids::{NoteId, SoundId};
use bmz_core::input::InputKind;
use bmz_core::lane::{KeyMode, Lane};
use bmz_core::time::TimeUs;
use bmz_gameplay::input::backend::{
    BufferedInputBackend, DeviceId, DeviceInputEvent, DeviceTimestamp, PhysicalControl,
};
use bmz_gameplay::input::translator::InputTimingContext;
use bmz_gameplay::rule::RuleMode;
use rusqlite::Connection;

use super::*;
use crate::config::profile_config::HispeedModeConfig;
use crate::storage::common::configure_connection;
use crate::storage::library_db::{ChartImportRecord, LibraryDatabase};
use crate::storage::migration::{LIBRARY_MIGRATIONS, run_migrations};

fn class_gauge_values(session: &GameSession) -> [f32; 6] {
    session
        .gauge
        .gauges
        .iter()
        .find(|g| g.definition.gauge_type == GaugeType::Class)
        .map(|g| g.definition.values)
        .expect("Class gauge present")
}

fn chart() -> PlayableChart {
    PlayableChart {
        identity: compute_chart_identity(b"session"),
        metadata: ChartMetadata {
            title: "session".to_string(),
            initial_bpm: 120.0,
            total: Some(160.0),
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
        sounds: Vec::<SoundAssetRef>::new(),
        bga_assets: Vec::new(),
        total_notes: 1,
        end_time: TimeUs(0),
    }
}

fn note(id: u32, lane: Lane, time_us: i64) -> NoteEvent {
    use bmz_core::time::ChartTick;

    NoteEvent {
        id: NoteId(id),
        lane,
        kind: NoteKind::Tap,
        tick: ChartTick((time_us / 1_000) as u64),
        time: TimeUs(time_us),
        sound: None,
        damage: None,
    }
}

fn chart_with_two_notes_same_lane() -> PlayableChart {
    let mut chart = chart();
    chart.metadata.key_mode = KeyMode::K7;
    chart.lane_notes[Lane::Key1.index()].push(note(1, Lane::Key1, 1_000_000));
    chart.lane_notes[Lane::Key1.index()].push(note(2, Lane::Key1, 1_020_000));
    chart
}

fn lanes_for_notes(chart: &PlayableChart) -> Vec<(NoteId, Lane)> {
    let mut lanes: Vec<_> =
        chart.lane_notes.iter().flatten().map(|note| (note.id, note.lane)).collect();
    lanes.sort_by_key(|(id, _)| *id);
    lanes
}

fn write_temp_bms(text: &str) -> std::path::PathBuf {
    let stamp =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let path =
        std::env::temp_dir().join(format!("bmz-play-session-{}-{stamp}.bms", std::process::id()));
    std::fs::write(&path, text).unwrap();
    path
}

fn write_temp_bms_with_wav(text: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let stamp =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir();
    let bms_path = dir.join(format!("bmz-prepared-session-{}-{stamp}.bms", std::process::id()));
    let wav_name = format!("bmz-prepared-session-{}-{stamp}.wav", std::process::id());
    let wav_path = dir.join(&wav_name);
    std::fs::write(&bms_path, text.replace("test.wav", &wav_name)).unwrap();
    std::fs::write(&wav_path, [wav_header(1, 1, 48_000, 16, 2).as_slice(), &[0x00, 0x40]].concat())
        .unwrap();
    (bms_path, wav_path)
}

fn write_temp_bms_with_two_wavs(
    text: &str,
) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let stamp =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir();
    let prefix = format!("bmz-retry-audio-{}-{stamp}", std::process::id());
    let bms_path = dir.join(format!("{prefix}.bms"));
    let bgm_path = dir.join(format!("{prefix}-bgm.wav"));
    let key_path = dir.join(format!("{prefix}-key.wav"));
    let bms = text
        .replace("bgm.wav", bgm_path.file_name().unwrap().to_str().unwrap())
        .replace("key.wav", key_path.file_name().unwrap().to_str().unwrap());
    std::fs::write(&bms_path, bms).unwrap();
    std::fs::write(
        &bgm_path,
        [wav_header(1, 1, 48_000, 16, 2).as_slice(), &16_384_i16.to_le_bytes()].concat(),
    )
    .unwrap();
    std::fs::write(
        &key_path,
        [wav_header(1, 1, 48_000, 16, 2).as_slice(), &(-16_384_i16).to_le_bytes()].concat(),
    )
    .unwrap();
    (bms_path, bgm_path, key_path)
}

fn wav_header(format: u16, channels: u16, sample_rate: u32, bits: u16, data_len: u32) -> Vec<u8> {
    let byte_rate = sample_rate * channels as u32 * bits as u32 / 8;
    let block_align = channels * bits / 8;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&format.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out
}

#[path = "tests/cases_01.rs"]
mod cases_01;
#[path = "tests/cases_02.rs"]
mod cases_02;
#[path = "tests/cases_03.rs"]
mod cases_03;
#[path = "tests/cases_04.rs"]
mod cases_04;
#[path = "tests/cases_05.rs"]
mod cases_05;
