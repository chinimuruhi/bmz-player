use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use bmz_core::input::InputDeviceKind;
use bmz_core::lane::KeyMode;
use bmz_gameplay::rule::RuleMode;

use crate::config::profile_config::ReplaySlotRule;
use crate::ln_policy::{LnPolicySetting, score_ln_policy};
use crate::paths::ProfilePaths;

use super::beatoraja_replay::{
    BeatorajaReplay, BeatorajaReplayConversion, load_beatoraja_replay_with_fingerprint,
};
use super::library_db::{ChartListItem, LibraryDatabase};
use super::replay::{replay_slot_file_name, save_replay_for_import};
use super::score_db::{ReplaySlotRecord, ScoreDatabase, ScoreKey, ScoreSourceKind};

const DEFAULT_BEATORAJA_H_RANDOM_THRESHOLD_MS: u32 = 125;
const REPLAY_IMPORT_DB_BATCH_SIZE: usize = 256;

#[derive(Debug, Clone)]
pub struct ImportBeatorajaReplaysRequest {
    pub source: PathBuf,
    pub overwrite_protected_slots: bool,
    pub device_kind: InputDeviceKind,
}

impl ImportBeatorajaReplaysRequest {
    pub fn new(source: impl Into<PathBuf>) -> Self {
        Self {
            source: source.into(),
            overwrite_protected_slots: false,
            device_kind: InputDeviceKind::Keyboard,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayImportIssueKind {
    MissingChart,
    ProtectedSlot,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayImportIssue {
    pub path: PathBuf,
    pub kind: ReplayImportIssueKind,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplayImportReport {
    pub scanned: usize,
    pub imported: usize,
    pub replaced: usize,
    pub unchanged: usize,
    pub missing_chart: usize,
    pub protected_slot: usize,
    pub unsupported: usize,
    pub cancelled: bool,
    pub issues: Vec<ReplayImportIssue>,
    pub threshold_warning: Option<String>,
}

impl ReplayImportReport {
    pub fn summary(&self) -> String {
        format!(
            "scanned={}, imported={}, replaced={}, unchanged={}, missing_chart={}, protected_slot={}, unsupported={}, cancelled={}",
            self.scanned,
            self.imported,
            self.replaced,
            self.unchanged,
            self.missing_chart,
            self.protected_slot,
            self.unsupported,
            self.cancelled
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplayImportProgress {
    pub done: usize,
    pub total: usize,
}

enum ImportOneOutcome {
    Imported { replaced: bool, record: ReplaySlotRecord },
    Unchanged { fingerprint_backfill: Option<ReplaySlotRecord> },
    MissingChart(String),
    ProtectedSlot(String),
}

pub fn import_beatoraja_replays(
    library_db: &LibraryDatabase,
    score_db: &mut ScoreDatabase,
    profile_paths: &ProfilePaths,
    request: &ImportBeatorajaReplaysRequest,
) -> Result<ReplayImportReport> {
    import_beatoraja_replays_with_progress(
        library_db,
        score_db,
        profile_paths,
        request,
        |_| {},
        || false,
    )
}

pub fn import_beatoraja_replays_with_progress(
    library_db: &LibraryDatabase,
    score_db: &mut ScoreDatabase,
    profile_paths: &ProfilePaths,
    request: &ImportBeatorajaReplaysRequest,
    mut on_progress: impl FnMut(ReplayImportProgress),
    is_cancelled: impl Fn() -> bool,
) -> Result<ReplayImportReport> {
    let (replay_paths, player_root) = discover_replay_paths(&request.source)?;
    let (h_random_threshold_ms, threshold_warning) =
        load_h_random_threshold_ms(player_root.as_deref());
    profile_paths.ensure_dirs()?;

    let total = replay_paths.len();
    on_progress(ReplayImportProgress { done: 0, total });
    let mut report = ReplayImportReport { threshold_warning, ..Default::default() };
    let mut pending_records = Vec::with_capacity(REPLAY_IMPORT_DB_BATCH_SIZE);
    for path in replay_paths {
        if is_cancelled() {
            report.cancelled = true;
            break;
        }
        report.scanned += 1;
        match import_one(library_db, score_db, profile_paths, request, &path, h_random_threshold_ms)
        {
            Ok(ImportOneOutcome::Imported { replaced, record }) => {
                report.imported += 1;
                report.replaced += usize::from(replaced);
                pending_records.push(record);
            }
            Ok(ImportOneOutcome::Unchanged { fingerprint_backfill }) => {
                report.unchanged += 1;
                if let Some(record) = fingerprint_backfill {
                    pending_records.push(record);
                }
            }
            Ok(ImportOneOutcome::MissingChart(message)) => {
                report.missing_chart += 1;
                report.issues.push(ReplayImportIssue {
                    path,
                    kind: ReplayImportIssueKind::MissingChart,
                    message,
                });
            }
            Ok(ImportOneOutcome::ProtectedSlot(message)) => {
                report.protected_slot += 1;
                report.issues.push(ReplayImportIssue {
                    path,
                    kind: ReplayImportIssueKind::ProtectedSlot,
                    message,
                });
            }
            Err(error) => {
                report.unsupported += 1;
                report.issues.push(ReplayImportIssue {
                    path,
                    kind: ReplayImportIssueKind::Unsupported,
                    message: format!("{error:#}"),
                });
            }
        }
        if pending_records.len() >= REPLAY_IMPORT_DB_BATCH_SIZE {
            score_db.upsert_replay_slots(&pending_records)?;
            pending_records.clear();
        }
        on_progress(ReplayImportProgress { done: report.scanned, total });
    }
    if !pending_records.is_empty() {
        score_db.upsert_replay_slots(&pending_records)?;
    }
    Ok(report)
}

fn import_one(
    library_db: &LibraryDatabase,
    score_db: &mut ScoreDatabase,
    profile_paths: &ProfilePaths,
    request: &ImportBeatorajaReplaysRequest,
    source_path: &Path,
    h_random_threshold_ms: u32,
) -> Result<ImportOneOutcome> {
    let (replay, source_fingerprint) = load_beatoraja_replay_with_fingerprint(source_path)?;
    let chart_sha256 = replay.chart_sha256()?;
    let charts = library_db.list_charts_by_sha256(chart_sha256)?;
    let Some(chart) = charts.first() else {
        return Ok(ImportOneOutcome::MissingChart(format!(
            "chart {} is not registered in the BMZ library",
            super::common::hash_to_hex(&chart_sha256)
        )));
    };
    ensure_consistent_chart_rows(&charts)?;

    let key_mode = KeyMode::from_str_opt(&chart.mode)
        .with_context(|| format!("unsupported library key mode: {}", chart.mode))?;
    let ln_setting = replay_ln_setting(&replay)?;
    let ln_policy = score_ln_policy(ln_setting, chart.ln_profile);
    let converted = replay.to_replay_file(BeatorajaReplayConversion {
        key_mode,
        ln_policy,
        device_kind: request.device_kind,
        h_random_threshold_ms: Some(h_random_threshold_ms),
    })?;
    let slot = replay_slot_from_path(source_path)?;
    let rule_mode = RuleMode::Beatoraja;
    let key = ScoreKey::with_options(
        chart_sha256,
        ln_policy,
        converted.double_option_bucket(),
        rule_mode,
    );
    let previous = score_db.replay_slot(key, slot)?;
    if let Some(previous) = previous.as_ref()
        && previous.source_kind != ScoreSourceKind::Beatoraja
        && !request.overwrite_protected_slots
    {
        return Ok(ImportOneOutcome::ProtectedSlot(format!(
            "slot {} is owned by {} and was not overwritten",
            slot + 1,
            previous.source_kind.as_str()
        )));
    }

    let file_name = replay_slot_file_name(
        chart_sha256,
        ln_policy,
        converted.double_option_bucket(),
        rule_mode,
        slot,
    );
    let destination = profile_paths.replay_dir.join(&file_name);
    let replay_path = format!("replay/{file_name}");
    let source_path_text = source_path.to_string_lossy().into_owned();
    if let Some(previous) = previous.as_ref()
        && previous.source_kind == ScoreSourceKind::Beatoraja
        && profile_paths.root_dir.join(&previous.replay_path).is_file()
    {
        let fingerprint_matches = !previous.source_fingerprint.is_empty()
            && previous.source_fingerprint == source_fingerprint;
        let legacy_record_matches = previous.source_fingerprint.is_empty()
            && previous.source_path == source_path_text
            && previous.played_at == converted.played_at;
        if fingerprint_matches || legacy_record_matches {
            let fingerprint_backfill = legacy_record_matches.then(|| {
                let mut record = previous.clone();
                record.source_fingerprint = source_fingerprint.clone();
                record
            });
            return Ok(ImportOneOutcome::Unchanged { fingerprint_backfill });
        }
    }

    save_replay_for_import(&destination, &converted)?;
    let record = ReplaySlotRecord {
        chart_sha256,
        ln_policy,
        double_option: converted.double_option_bucket(),
        rule_mode,
        slot,
        rule: ReplaySlotRule::Always,
        replay_path,
        played_at: converted.played_at,
        ex_score: None,
        bp: None,
        cb: None,
        max_combo: None,
        clear_rank: None,
        source_kind: ScoreSourceKind::Beatoraja,
        source_path: source_path_text,
        source_fingerprint,
    };
    Ok(ImportOneOutcome::Imported { replaced: previous.is_some(), record })
}

fn ensure_consistent_chart_rows(charts: &[ChartListItem]) -> Result<()> {
    let Some(first) = charts.first() else {
        return Ok(());
    };
    if charts
        .iter()
        .skip(1)
        .any(|chart| chart.mode != first.mode || chart.ln_profile != first.ln_profile)
    {
        bail!("duplicate library charts disagree on key mode or long-note profile");
    }
    Ok(())
}

fn replay_ln_setting(replay: &BeatorajaReplay) -> Result<LnPolicySetting> {
    match replay.ln_mode() {
        0 => Ok(LnPolicySetting::AutoLn),
        1 => Ok(LnPolicySetting::AutoCn),
        2 => Ok(LnPolicySetting::AutoHcn),
        mode => bail!("unknown beatoraja long-note mode: {mode}"),
    }
}

fn discover_replay_paths(source: &Path) -> Result<(Vec<PathBuf>, Option<PathBuf>)> {
    if source.is_file() {
        if !has_brd_extension(source) {
            bail!("beatoraja replay file must have a .brd extension");
        }
        let player_root = source
            .parent()
            .and_then(|parent| {
                parent.file_name().is_some_and(|name| name == "replay").then_some(parent)
            })
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        return Ok((vec![source.to_path_buf()], player_root));
    }
    if !source.is_dir() {
        bail!("beatoraja replay source does not exist: {}", source.display());
    }

    let nested_replay = source.join("replay");
    let (replay_dir, player_root) = if nested_replay.is_dir() {
        (nested_replay, Some(source.to_path_buf()))
    } else {
        let root = source
            .file_name()
            .is_some_and(|name| name == "replay")
            .then(|| source.parent().map(Path::to_path_buf))
            .flatten();
        (source.to_path_buf(), root)
    };
    let mut paths = fs::read_dir(&replay_dir)
        .with_context(|| format!("failed to read replay directory: {}", replay_dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && has_brd_extension(path))
        .collect::<Vec<_>>();
    paths.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    Ok((paths, player_root))
}

fn has_brd_extension(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("brd"))
}

fn replay_slot_from_path(path: &Path) -> Result<u8> {
    let stem =
        path.file_stem().and_then(|stem| stem.to_str()).context("invalid replay filename")?;
    let slot = stem.rsplit_once('_').and_then(|(_, suffix)| suffix.parse::<u8>().ok()).unwrap_or(0);
    if slot > 3 {
        bail!("beatoraja replay slot index is outside 0..=3: {slot}");
    }
    Ok(slot)
}

fn load_h_random_threshold_ms(player_root: Option<&Path>) -> (u32, Option<String>) {
    let Some(player_root) = player_root else {
        return (
            DEFAULT_BEATORAJA_H_RANDOM_THRESHOLD_MS,
            Some(
                "config_player.json was not located; using beatoraja's 125 ms default".to_string(),
            ),
        );
    };
    let path = player_root.join("config_player.json");
    let threshold_bpm = fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value.get("hranThresholdBPM").and_then(serde_json::Value::as_i64));
    match threshold_bpm {
        Some(value) if value > 0 => {
            let value = u32::try_from(value).unwrap_or(u32::MAX);
            (15_000_u32.div_ceil(value), None)
        }
        Some(0) => (0, None),
        _ => (
            DEFAULT_BEATORAJA_H_RANDOM_THRESHOLD_MS,
            Some(format!(
                "{} did not provide hranThresholdBPM; using beatoraja's 125 ms default",
                path.display()
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE;
    use bmz_chart::hash::compute_chart_identity;
    use bmz_chart::model::{ChartMetadata, PlayableChart};
    use bmz_core::time::TimeUs;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use rusqlite::Connection;
    use serde_json::json;

    use super::*;
    use crate::storage::common::configure_connection;
    use crate::storage::library_db::ChartImportRecord;
    use crate::storage::migration::{LIBRARY_MIGRATIONS, SCORE_MIGRATIONS, run_migrations};
    use crate::storage::replay::load_replay;

    fn temp_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir()
            .join(format!("bmz-replay-import-{label}-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn replay_bytes() -> Vec<u8> {
        let mut packed = Vec::new();
        packed.push(1);
        packed.extend_from_slice(&10_000_i64.to_le_bytes());
        packed.push((-1_i8) as u8);
        packed.extend_from_slice(&20_000_i64.to_le_bytes());
        let value = json!({
            "sha256": "0202020202020202020202020202020202020202020202020202020202020202",
            "mode": 1,
            "keyinput": URL_SAFE.encode(gzip(&packed)),
            "gauge": 3,
            "rand": [1],
            "date": 1_700_000_123_i64,
            "randomoption": 0,
            "randomoptionseed": 42,
            "randomoption2": 0,
            "randomoption2seed": 84,
            "doubleoption": 0
        });
        gzip(value.to_string().as_bytes())
    }

    fn chart() -> PlayableChart {
        let mut chart = PlayableChart {
            identity: compute_chart_identity(b"beatoraja replay import test"),
            metadata: ChartMetadata {
                title: "Import Target".to_string(),
                key_mode: KeyMode::K7,
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
            bga_asset_by_bmp_key: HashMap::new(),
            bar_lines: Vec::new(),
            sounds: Vec::new(),
            bga_assets: Vec::new(),
            total_notes: 1,
            end_time: TimeUs(1_000_000),
        };
        chart.identity.file_md5 = [1; 16];
        chart.identity.file_sha256 = [2; 32];
        chart
    }

    fn databases() -> (LibraryDatabase, ScoreDatabase) {
        let mut library_conn = Connection::open_in_memory().unwrap();
        configure_connection(&library_conn).unwrap();
        run_migrations(&mut library_conn, LIBRARY_MIGRATIONS).unwrap();
        let mut library_db = LibraryDatabase::from_connection(library_conn);
        let chart = chart();
        library_db
            .upsert_chart_import(&ChartImportRecord {
                root_id: None,
                file_path: Path::new("/songs/import.bms"),
                file_size: 1,
                modified_at: 1,
                scanned_at: 1,
                chart: &chart,
            })
            .unwrap();

        let mut score_conn = Connection::open_in_memory().unwrap();
        configure_connection(&score_conn).unwrap();
        run_migrations(&mut score_conn, SCORE_MIGRATIONS).unwrap();
        (library_db, ScoreDatabase::from_connection(score_conn))
    }

    fn profile_paths(root: &Path) -> ProfilePaths {
        ProfilePaths {
            root_dir: root.join("profile"),
            profile_toml: root.join("profile/profile.toml"),
            collection_db: root.join("profile/collection.db"),
            score_db: root.join("profile/score.db"),
            network_db: root.join("profile/network.db"),
            replay_dir: root.join("profile/replay"),
        }
    }

    #[test]
    fn imports_replay_with_nullable_metrics_and_provenance() {
        let root = temp_root("success");
        let player = root.join("player");
        let replay_dir = player.join("replay");
        fs::create_dir_all(&replay_dir).unwrap();
        fs::write(player.join("config_player.json"), br#"{"hranThresholdBPM":100}"#).unwrap();
        let source = replay_dir.join(format!("{}.brd", "02".repeat(32)));
        fs::write(&source, replay_bytes()).unwrap();
        let (library_db, mut score_db) = databases();
        let paths = profile_paths(&root);

        let report = import_beatoraja_replays(
            &library_db,
            &mut score_db,
            &paths,
            &ImportBeatorajaReplaysRequest::new(&player),
        )
        .unwrap();

        assert_eq!(report.imported, 1);
        assert_eq!(report.unsupported, 0);
        let key = ScoreKey::new([2; 32], crate::ln_policy::LnScorePolicy::ForceLn);
        let slot = score_db.replay_slot(key, 0).unwrap().unwrap();
        assert_eq!(slot.source_kind, ScoreSourceKind::Beatoraja);
        assert_eq!(slot.ex_score, None);
        assert_eq!(slot.source_path, source.to_string_lossy());
        assert!(!slot.source_fingerprint.is_empty());
        let converted = load_replay(&paths.root_dir.join(slot.replay_path)).unwrap();
        assert_eq!(converted.h_random_threshold_ms, Some(150));

        let repeated = import_beatoraja_replays(
            &library_db,
            &mut score_db,
            &paths,
            &ImportBeatorajaReplaysRequest::new(&player),
        )
        .unwrap();
        assert_eq!(repeated.imported, 0);
        assert_eq!(repeated.unchanged, 1);

        let mut legacy_slot = score_db.replay_slot(key, 0).unwrap().unwrap();
        legacy_slot.source_fingerprint.clear();
        score_db.upsert_replay_slot(&legacy_slot).unwrap();
        let backfilled = import_beatoraja_replays(
            &library_db,
            &mut score_db,
            &paths,
            &ImportBeatorajaReplaysRequest::new(&player),
        )
        .unwrap();
        assert_eq!(backfilled.unchanged, 1);
        assert!(!score_db.replay_slot(key, 0).unwrap().unwrap().source_fingerprint.is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn protects_local_slot_unless_overwrite_is_requested() {
        let root = temp_root("conflict");
        let player = root.join("player");
        let replay_dir = player.join("replay");
        fs::create_dir_all(&replay_dir).unwrap();
        fs::write(replay_dir.join(format!("{}.brd", "02".repeat(32))), replay_bytes()).unwrap();
        let (library_db, mut score_db) = databases();
        let paths = profile_paths(&root);
        let key = ScoreKey::new([2; 32], crate::ln_policy::LnScorePolicy::ForceLn);
        score_db
            .upsert_replay_slot(&ReplaySlotRecord {
                chart_sha256: [2; 32],
                ln_policy: crate::ln_policy::LnScorePolicy::ForceLn,
                double_option: crate::select_options::DoubleOptionScoreBucket::Off,
                rule_mode: RuleMode::Beatoraja,
                slot: 0,
                rule: ReplaySlotRule::Always,
                replay_path: "replay/local.toml".to_string(),
                played_at: 1,
                ex_score: Some(100),
                bp: Some(0),
                cb: Some(0),
                max_combo: Some(50),
                clear_rank: Some(5),
                source_kind: ScoreSourceKind::Local,
                source_path: String::new(),
                source_fingerprint: String::new(),
            })
            .unwrap();

        let report = import_beatoraja_replays(
            &library_db,
            &mut score_db,
            &paths,
            &ImportBeatorajaReplaysRequest::new(&player),
        )
        .unwrap();

        assert_eq!(report.protected_slot, 1);
        assert_eq!(report.imported, 0);
        assert_eq!(
            score_db.replay_slot(key, 0).unwrap().unwrap().source_kind,
            ScoreSourceKind::Local
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_progress_and_honors_cancellation() {
        let root = temp_root("cancel");
        let player = root.join("player");
        let replay_dir = player.join("replay");
        fs::create_dir_all(&replay_dir).unwrap();
        let hash = "02".repeat(32);
        fs::write(replay_dir.join(format!("{hash}.brd")), replay_bytes()).unwrap();
        fs::write(replay_dir.join(format!("{hash}_1.brd")), replay_bytes()).unwrap();
        let (library_db, mut score_db) = databases();
        let paths = profile_paths(&root);
        let progress = RefCell::new(Vec::new());
        let cancel = Cell::new(false);

        let report = import_beatoraja_replays_with_progress(
            &library_db,
            &mut score_db,
            &paths,
            &ImportBeatorajaReplaysRequest::new(&player),
            |value| {
                progress.borrow_mut().push(value);
                if value.done == 1 {
                    cancel.set(true);
                }
            },
            || cancel.get(),
        )
        .unwrap();

        assert_eq!(report.scanned, 1);
        assert_eq!(report.imported, 1);
        assert!(report.cancelled);
        assert_eq!(
            progress.into_inner(),
            vec![
                ReplayImportProgress { done: 0, total: 2 },
                ReplayImportProgress { done: 1, total: 2 },
            ]
        );

        fs::remove_dir_all(root).unwrap();
    }
}
