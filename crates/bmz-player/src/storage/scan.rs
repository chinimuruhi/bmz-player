use std::collections::HashMap;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

use anyhow::Result;
use bmz_chart::import::ImportResult;
use bmz_chart::import::error::ImportError;
use bmz_chart::import::import_bms_chart;
use rayon::prelude::*;

use crate::config::app_config::{PathEntry, ScanConfig};

use super::library_db::{CHART_IMPORT_VERSION, ChartImportRecord, LibraryDatabase};

#[derive(Debug, Clone, Default)]
pub struct ScanSummary {
    pub roots_seen: u32,
    pub roots_unreadable: u32,
    pub files_seen: u32,
    pub discovery_skipped: u32,
    pub imported: u32,
    pub failed: u32,
    pub skipped: u32,
    pub warnings: u32,
}

#[derive(Debug, Clone)]
pub struct ScanFailure {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanDiscoveryOperation {
    OpenDirectory,
    ReadEntry,
    ReadFileType,
    ReadMetadata,
}

impl ScanDiscoveryOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenDirectory => "open_directory",
            Self::ReadEntry => "read_entry",
            Self::ReadFileType => "read_file_type",
            Self::ReadMetadata => "read_metadata",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScanDiscoveryIssue {
    pub root: PathBuf,
    pub path: PathBuf,
    pub operation: ScanDiscoveryOperation,
    pub error_kind: io::ErrorKind,
    pub raw_os_error: Option<i32>,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub summary: ScanSummary,
    pub timing: ScanTiming,
    pub failures: Vec<ScanFailure>,
    pub discovery_issues: Vec<ScanDiscoveryIssue>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanTiming {
    pub total_ms: u128,
    pub discovery_ms: u128,
    pub fingerprint_ms: u128,
    pub skip_check_ms: u128,
    pub parse_ms: u128,
    pub write_ms: u128,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanProgress {
    pub done: u32,
    pub total: u32,
}

/// 1回のバッチで並列パースするファイル数
const IMPORT_BATCH_SIZE: usize = 256;

pub fn scan_song_roots(
    db: &mut LibraryDatabase,
    roots: &[PathEntry],
    scan: &ScanConfig,
    scanned_at: i64,
    force: bool,
) -> Result<ScanReport> {
    scan_song_roots_with_progress(db, roots, scan, scanned_at, force, |_| {})
}

pub fn scan_song_roots_with_progress(
    db: &mut LibraryDatabase,
    roots: &[PathEntry],
    scan: &ScanConfig,
    scanned_at: i64,
    force: bool,
    mut on_progress: impl FnMut(ScanProgress),
) -> Result<ScanReport> {
    struct DiscoveredRoot {
        root_path: PathBuf,
        root_id: i64,
        root_num: usize,
        entries: Vec<ChartFileEntry>,
        discovery_complete: bool,
        root_readable: bool,
    }

    struct FileTodo {
        path: PathBuf,
        file_size: u64,
        modified_at: i64,
    }

    struct ParsedFile {
        path: PathBuf,
        file_size: u64,
        modified_at: i64,
        result: Result<ImportResult, ImportError>,
    }

    let total_start = Instant::now();
    let mut report = ScanReport::default();
    let enabled_roots: Vec<&PathEntry> = roots.iter().filter(|r| r.enabled).collect();
    let root_count = enabled_roots.len();
    let mut discovered_roots = Vec::with_capacity(root_count);
    let mut files_total = 0_u32;

    // Discovery runs across every root before fingerprinting/importing so the
    // progress denominator can grow in real time without resetting per root.
    on_progress(ScanProgress::default());
    for (root_index, root) in enabled_roots.into_iter().enumerate() {
        report.summary.roots_seen = report.summary.roots_seen.saturating_add(1);
        let root_path = Path::new(&root.path);
        let root_id = db.upsert_root(root_path, root.enabled, root.recursive)?;
        let discovery_start = Instant::now();
        let files_before_root = files_total;
        let discovery =
            discover_chart_files_with_progress(root_path, root.recursive, scan, |root_total| {
                on_progress(ScanProgress {
                    done: 0,
                    total: files_before_root.saturating_add(root_total),
                });
            });
        let discovery_ms = discovery_start.elapsed().as_millis();
        report.timing.discovery_ms += discovery_ms;
        let root_files = usize_to_u32(discovery.entries.len());
        files_total = files_total.saturating_add(root_files);
        report.summary.files_seen = files_total;
        report.summary.discovery_skipped =
            report.summary.discovery_skipped.saturating_add(usize_to_u32(discovery.issues.len()));
        if !discovery.root_readable {
            report.summary.roots_unreadable = report.summary.roots_unreadable.saturating_add(1);
        }

        tracing::info!(
            root = %root_path.display(),
            root_num = root_index + 1,
            root_count,
            files = root_files,
            discovery_issues = discovery.issues.len(),
            discovery_ms,
            "song root discovery complete"
        );

        report.discovery_issues.extend(discovery.issues);
        discovered_roots.push(DiscoveredRoot {
            root_path: root_path.to_path_buf(),
            root_id,
            root_num: root_index + 1,
            entries: discovery.entries,
            discovery_complete: discovery.complete,
            root_readable: discovery.root_readable,
        });
    }
    on_progress(ScanProgress { done: 0, total: files_total });

    let mut progress_done = 0_u32;
    for root in discovered_roots {
        if !root.root_readable {
            continue;
        }

        let root_path = root.root_path.as_path();
        let entries = root.entries;
        let root_files_total = usize_to_u32(entries.len());
        let root_skipped_start = report.summary.skipped;
        let root_imported_start = report.summary.imported;
        let root_failed_start = report.summary.failed;
        let folder_document_flags: Vec<(PathBuf, bool)> = entries
            .iter()
            .filter_map(|entry| {
                entry.path.parent().map(|folder| (folder.to_path_buf(), entry.has_document))
            })
            .collect::<HashMap<_, _>>()
            .into_iter()
            .collect();

        // Phase 2: skip判定（1クエリでrootの全fingerprintsをロードしてHashMap lookup）
        let fingerprint_start = Instant::now();
        let fingerprints = db.load_fingerprints_for_root(root.root_id)?;
        let fingerprint_ms = fingerprint_start.elapsed().as_millis();
        report.timing.fingerprint_ms += fingerprint_ms;
        let skip_start = Instant::now();
        let mut to_import: Vec<FileTodo> = Vec::new();
        let mut unchanged_count = 0_u32;
        for entry in &entries {
            let key = entry.path.to_string_lossy();
            let unchanged = !force
                && fingerprints.get(key.as_ref()).is_some_and(|fp| {
                    fp.file_size == entry.file_size
                        && fp.modified_at == entry.modified_at
                        && fp.import_version == CHART_IMPORT_VERSION
                });
            if unchanged {
                report.summary.skipped = report.summary.skipped.saturating_add(1);
                unchanged_count = unchanged_count.saturating_add(1);
            } else {
                to_import.push(FileTodo {
                    path: entry.path.clone(),
                    file_size: entry.file_size,
                    modified_at: entry.modified_at,
                });
            }
        }
        let skip_check_ms = skip_start.elapsed().as_millis();
        report.timing.skip_check_ms += skip_check_ms;
        progress_done = progress_done.saturating_add(unchanged_count).min(files_total);
        on_progress(ScanProgress { done: progress_done, total: files_total });

        let new_total = to_import.len();
        tracing::info!(
            new_files = new_total,
            skipped = unchanged_count,
            fingerprint_ms,
            skip_check_ms,
            root = %root_path.display(),
            "skip check complete"
        );

        // Phase 3+4: バッチごとに並列パース → 1トランザクションでまとめて書き込み
        let mut last_log = std::time::Instant::now();
        let log_interval = std::time::Duration::from_secs(2);

        for (batch_idx, chunk) in to_import.chunks(IMPORT_BATCH_SIZE).enumerate() {
            let batch_done = batch_idx * IMPORT_BATCH_SIZE;
            let now = std::time::Instant::now();
            if now.duration_since(last_log) >= log_interval || batch_idx == 0 {
                last_log = now;
                let pct = batch_done * 100 / new_total.max(1);
                tracing::info!(
                    pct,
                    done = batch_done,
                    total = new_total,
                    root = %root_path.display(),
                    "importing"
                );
            }

            // 並列パース
            let parse_start = std::time::Instant::now();
            let parsed: Vec<ParsedFile> = chunk
                .par_iter()
                .map(|todo| ParsedFile {
                    path: todo.path.clone(),
                    file_size: todo.file_size,
                    modified_at: todo.modified_at,
                    result: import_bms_chart_catching_unwind(&todo.path),
                })
                .collect();
            let parse_ms = parse_start.elapsed().as_millis();
            report.timing.parse_ms += parse_ms;

            // 1トランザクションでバッチ書き込み
            let write_start = std::time::Instant::now();
            {
                let tx = db.conn_mut().transaction()?;
                for p in &parsed {
                    match &p.result {
                        Ok(import_result) => {
                            let record = ChartImportRecord {
                                root_id: Some(root.root_id),
                                file_path: &p.path,
                                file_size: p.file_size,
                                modified_at: p.modified_at,
                                scanned_at,
                                chart: &import_result.chart,
                            };
                            let (_, chart_file_id) =
                                LibraryDatabase::write_chart_import(&tx, &record)?;
                            let warnings_written = LibraryDatabase::write_import_warnings(
                                &tx,
                                chart_file_id,
                                &import_result.warnings,
                                scanned_at,
                            )?;
                            report.summary.imported += 1;
                            report.summary.warnings += warnings_written as u32;
                        }
                        Err(error) => {
                            let message = error.to_string();
                            LibraryDatabase::write_failed_chart(
                                &tx,
                                Some(root.root_id),
                                &p.path,
                                p.file_size,
                                p.modified_at,
                                scanned_at,
                                &message,
                            )?;
                            report.summary.failed += 1;
                            report.failures.push(ScanFailure { path: p.path.clone(), message });
                        }
                    }
                }
                tx.commit()?;
            }
            let write_ms = write_start.elapsed().as_millis();
            report.timing.write_ms += write_ms;

            tracing::info!(
                batch = batch_idx,
                files = chunk.len(),
                parse_ms,
                write_ms,
                root = %root_path.display(),
                "batch timing"
            );
            progress_done =
                progress_done.saturating_add(usize_to_u32(parsed.len())).min(files_total);
            on_progress(ScanProgress { done: progress_done, total: files_total });
        }

        // Discovery already enumerated every song directory. Persist the shared
        // folder flag even when every chart was skipped as unchanged.
        db.update_folder_document_flags(&folder_document_flags)?;

        tracing::info!(
            root_num = root.root_num,
            root_count,
            files = root_files_total,
            imported = report.summary.imported.saturating_sub(root_imported_start),
            skipped = report.summary.skipped.saturating_sub(root_skipped_start),
            failed = report.summary.failed.saturating_sub(root_failed_start),
            root = %root_path.display(),
            "root scan complete"
        );

        if root.discovery_complete {
            db.update_root_scanned_at(root.root_id, scanned_at)?;
        } else {
            tracing::warn!(
                root = %root_path.display(),
                "song root discovery was incomplete; last_scan_at was not updated"
            );
        }
    }

    on_progress(ScanProgress { done: files_total, total: files_total });
    report.timing.total_ms = total_start.elapsed().as_millis();
    Ok(report)
}

fn import_bms_chart_catching_unwind(path: &Path) -> Result<ImportResult, ImportError> {
    match catch_unwind(AssertUnwindSafe(|| import_bms_chart(path, None, false))) {
        Ok(result) => result.map(|mut result| {
            result.chart.metadata.preview_file = crate::chart_asset::normalize_preview_file(
                path,
                &result.chart.metadata.preview_file,
            );
            result
        }),
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&'static str>().copied())
                .unwrap_or("unknown panic");
            Err(ImportError::Parse {
                path: path.to_path_buf(),
                message: format!("chart import panicked: {message}"),
            })
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChartFileEntry {
    pub path: PathBuf,
    pub file_size: u64,
    pub modified_at: i64,
    pub has_document: bool,
}

#[derive(Debug, Default)]
struct ChartDiscovery {
    entries: Vec<ChartFileEntry>,
    issues: Vec<ScanDiscoveryIssue>,
    complete: bool,
    root_readable: bool,
}

pub fn discover_chart_files(
    root: &Path,
    recursive: bool,
    scan: &ScanConfig,
) -> Result<Vec<ChartFileEntry>> {
    Ok(discover_chart_files_with_progress(root, recursive, scan, |_| {}).entries)
}

fn discover_chart_files_with_progress(
    root: &Path,
    recursive: bool,
    scan: &ScanConfig,
    mut on_discovered: impl FnMut(u32),
) -> ChartDiscovery {
    let mut discovery = ChartDiscovery { complete: true, ..Default::default() };
    let mut dirs = vec![root.to_path_buf()];
    let mut discovered_count = 0_u32;

    while let Some(dir) = dirs.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => {
                if dir == root {
                    discovery.root_readable = true;
                }
                entries
            }
            Err(error) => {
                discovery.complete = false;
                record_discovery_issue(
                    &mut discovery.issues,
                    root,
                    &dir,
                    ScanDiscoveryOperation::OpenDirectory,
                    error,
                );
                continue;
            }
        };
        let mut charts_in_dir = Vec::new();
        let mut has_document = false;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    discovery.complete = false;
                    record_discovery_issue(
                        &mut discovery.issues,
                        root,
                        &dir,
                        ScanDiscoveryOperation::ReadEntry,
                        error,
                    );
                    continue;
                }
            };
            let file_name = entry.file_name();
            if scan.skip_hidden && is_hidden_name(&file_name) {
                continue;
            }
            let path = entry.path();

            let (file_type, meta_opt) = if scan.follow_symlinks {
                let meta = match entry.metadata() {
                    Ok(meta) => meta,
                    Err(error) => {
                        discovery.complete = false;
                        record_discovery_issue(
                            &mut discovery.issues,
                            root,
                            &path,
                            ScanDiscoveryOperation::ReadMetadata,
                            error,
                        );
                        continue;
                    }
                };
                let ft = meta.file_type();
                (ft, Some(meta))
            } else {
                let file_type = match entry.file_type() {
                    Ok(file_type) => file_type,
                    Err(error) => {
                        discovery.complete = false;
                        record_discovery_issue(
                            &mut discovery.issues,
                            root,
                            &path,
                            ScanDiscoveryOperation::ReadFileType,
                            error,
                        );
                        continue;
                    }
                };
                (file_type, None)
            };

            if file_type.is_file() && is_document_file_name(&file_name) {
                has_document = true;
            }

            if file_type.is_dir() {
                if recursive {
                    dirs.push(path);
                }
            } else if file_type.is_file() && is_chart_file_name(&file_name) {
                let metadata = match meta_opt {
                    Some(metadata) => metadata,
                    None => match entry.metadata() {
                        Ok(metadata) => metadata,
                        Err(error) => {
                            discovery.complete = false;
                            record_discovery_issue(
                                &mut discovery.issues,
                                root,
                                &path,
                                ScanDiscoveryOperation::ReadMetadata,
                                error,
                            );
                            continue;
                        }
                    },
                };
                let modified_at = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                charts_in_dir.push(ChartFileEntry {
                    path,
                    file_size: metadata.len(),
                    modified_at,
                    has_document: false,
                });
                discovered_count = discovered_count.saturating_add(1);
                on_discovered(discovered_count);
            }
        }
        charts_in_dir.iter_mut().for_each(|entry| entry.has_document = has_document);
        discovery.entries.extend(charts_in_dir);
    }

    discovery
}

fn record_discovery_issue(
    issues: &mut Vec<ScanDiscoveryIssue>,
    root: &Path,
    path: &Path,
    operation: ScanDiscoveryOperation,
    error: io::Error,
) {
    tracing::warn!(
        root = %root.display(),
        path = %path.display(),
        operation = operation.as_str(),
        error_kind = ?error.kind(),
        raw_os_error = ?error.raw_os_error(),
        error = %error,
        "skipping inaccessible song scan path"
    );
    issues.push(ScanDiscoveryIssue {
        root: root.to_path_buf(),
        path: path.to_path_buf(),
        operation,
        error_kind: error.kind(),
        raw_os_error: error.raw_os_error(),
        message: error.to_string(),
    });
}

fn usize_to_u32(value: usize) -> u32 {
    value.min(u32::MAX as usize) as u32
}

fn is_document_file_name(name: &std::ffi::OsStr) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
}

pub(crate) fn folder_has_document(folder: &Path) -> bool {
    std::fs::read_dir(folder).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            entry.file_type().is_ok_and(|file_type| file_type.is_file())
                && is_document_file_name(&entry.file_name())
        })
    })
}

fn is_chart_file_name(name: &std::ffi::OsStr) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "bms" | "bme" | "bml" | "pms" | "bmson"
            )
        })
        .unwrap_or(false)
}

fn is_hidden_name(name: &std::ffi::OsStr) -> bool {
    name.to_str().map(|name| name.starts_with('.')).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::*;
    use crate::storage::common::configure_connection;
    use crate::storage::library_db::LibraryDatabase;
    use crate::storage::migration::{LIBRARY_MIGRATIONS, run_migrations};

    fn scan_config() -> ScanConfig {
        ScanConfig {
            follow_symlinks: false,
            skip_hidden: true,
            auto_rescan_on_startup: false,
            rescan_missing_files: true,
        }
    }

    #[test]
    fn discover_chart_files_respects_recursion_and_hidden_files() {
        let root = make_temp_dir("discover");
        write_file(&root.join("a.bms"), "#TITLE A\n#BPM 120\n");
        write_file(&root.join("ignore.txt"), "");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        write_file(&root.join("sub").join("b.bme"), "#TITLE B\n#BPM 120\n");
        write_file(&root.join(".hidden.bms"), "#TITLE Hidden\n#BPM 120\n");

        let shallow = discover_chart_files(&root, false, &scan_config()).unwrap();
        let deep = discover_chart_files(&root, true, &scan_config()).unwrap();

        assert_eq!(shallow.len(), 1);
        assert_eq!(deep.len(), 2);
        assert!(shallow[0].has_document);
        assert!(deep.iter().any(|entry| entry.has_document));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_song_roots_refreshes_document_flag_without_reimporting_chart() {
        let root = make_temp_dir("scan-document");
        write_file(&root.join("song.bms"), "#TITLE Document\n#BPM 120\n#00011:01\n");

        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut db = LibraryDatabase::from_connection(conn);
        let roots = vec![PathEntry {
            path: root.to_string_lossy().into_owned(),
            enabled: true,
            recursive: true,
        }];

        scan_song_roots(&mut db, &roots, &scan_config(), 1_700_000_030, false).unwrap();
        assert!(!db.list_charts(10, 0).unwrap()[0].has_document);

        write_file(&root.join("README.TXT"), "document");
        let with_document =
            scan_song_roots(&mut db, &roots, &scan_config(), 1_700_000_031, false).unwrap();
        assert_eq!(with_document.summary.skipped, 1);
        assert!(db.list_charts(10, 0).unwrap()[0].has_document);

        std::fs::remove_file(root.join("README.TXT")).unwrap();
        let without_document =
            scan_song_roots(&mut db, &roots, &scan_config(), 1_700_000_032, false).unwrap();
        assert_eq!(without_document.summary.skipped, 1);
        assert!(!db.list_charts(10, 0).unwrap()[0].has_document);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_song_roots_imports_enabled_roots() {
        let root = make_temp_dir("scan");
        write_file(
            &root.join("song.bms"),
            "\
#TITLE Scan Song
#BPM 120
#WAV01 key.wav
#00011:01
",
        );

        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut db = LibraryDatabase::from_connection(conn);
        let roots = vec![PathEntry {
            path: root.to_string_lossy().into_owned(),
            enabled: true,
            recursive: true,
        }];

        let report =
            scan_song_roots(&mut db, &roots, &scan_config(), 1_700_000_020, false).unwrap();

        assert_eq!(report.summary.roots_seen, 1);
        assert_eq!(report.summary.files_seen, 1);
        assert_eq!(report.summary.imported, 1);
        assert_eq!(report.summary.failed, 0);
        assert_eq!(report.summary.skipped, 0);

        let title: String =
            db.conn().query_row("SELECT title FROM charts", [], |row| row.get(0)).unwrap();
        let last_scan_at: i64 =
            db.conn().query_row("SELECT last_scan_at FROM roots", [], |row| row.get(0)).unwrap();
        assert_eq!(title, "Scan Song");
        assert_eq!(last_scan_at, 1_700_000_020);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_song_roots_reports_progress() {
        let root = make_temp_dir("scan-progress");
        write_file(&root.join("a.bms"), "#TITLE A\n#BPM 120\n#00011:01\n");
        write_file(&root.join("b.bms"), "#TITLE B\n#BPM 120\n#00011:01\n");

        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut db = LibraryDatabase::from_connection(conn);
        let roots = vec![PathEntry {
            path: root.to_string_lossy().into_owned(),
            enabled: true,
            recursive: true,
        }];
        let mut progress = Vec::new();

        let report = scan_song_roots_with_progress(
            &mut db,
            &roots,
            &scan_config(),
            1_700_000_020,
            false,
            |p| progress.push(p),
        )
        .unwrap();

        assert_eq!(report.summary.imported, 2);
        assert_eq!(progress.first(), Some(&ScanProgress { done: 0, total: 0 }));
        assert!(progress.contains(&ScanProgress { done: 0, total: 1 }));
        assert!(progress.contains(&ScanProgress { done: 0, total: 2 }));
        assert_eq!(progress.last(), Some(&ScanProgress { done: 2, total: 2 }));
        assert!(progress.windows(2).all(|pair| pair[0].done <= pair[1].done));
        assert!(progress.windows(2).all(|pair| pair[0].total <= pair[1].total));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_song_roots_skips_unreadable_root_and_continues_with_global_progress() {
        let valid_root = make_temp_dir("scan-after-unreadable");
        let missing_root = valid_root.join("missing-external-volume");
        write_file(&valid_root.join("song.bms"), "#TITLE Available\n#BPM 120\n#00011:01\n");

        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut db = LibraryDatabase::from_connection(conn);
        let roots = vec![
            PathEntry {
                path: missing_root.to_string_lossy().into_owned(),
                enabled: true,
                recursive: true,
            },
            PathEntry {
                path: valid_root.to_string_lossy().into_owned(),
                enabled: true,
                recursive: true,
            },
        ];
        let mut progress = Vec::new();

        let report = scan_song_roots_with_progress(
            &mut db,
            &roots,
            &scan_config(),
            1_700_000_040,
            false,
            |value| progress.push(value),
        )
        .unwrap();

        assert_eq!(report.summary.roots_seen, 2);
        assert_eq!(report.summary.roots_unreadable, 1);
        assert_eq!(report.summary.discovery_skipped, 1);
        assert_eq!(report.summary.files_seen, 1);
        assert_eq!(report.summary.imported, 1);
        assert_eq!(report.discovery_issues.len(), 1);
        assert_eq!(report.discovery_issues[0].root, missing_root);
        assert_eq!(report.discovery_issues[0].path, missing_root);
        assert_eq!(report.discovery_issues[0].operation, ScanDiscoveryOperation::OpenDirectory);
        assert_eq!(report.discovery_issues[0].error_kind, io::ErrorKind::NotFound);
        assert!(progress.contains(&ScanProgress { done: 0, total: 1 }));
        assert_eq!(progress.last(), Some(&ScanProgress { done: 1, total: 1 }));
        assert!(progress.windows(2).all(|pair| pair[0].total <= pair[1].total));

        let missing_last_scan: Option<i64> = db
            .conn()
            .query_row(
                "SELECT last_scan_at FROM roots WHERE path = ?1",
                [missing_root.to_string_lossy().as_ref()],
                |row| row.get(0),
            )
            .unwrap();
        let valid_last_scan: Option<i64> = db
            .conn()
            .query_row(
                "SELECT last_scan_at FROM roots WHERE path = ?1",
                [valid_root.to_string_lossy().as_ref()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(missing_last_scan, None);
        assert_eq!(valid_last_scan, Some(1_700_000_040));

        std::fs::remove_dir_all(valid_root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn scan_song_roots_skips_unreadable_subdirectory_and_keeps_accessible_files() {
        use std::os::unix::fs::PermissionsExt;

        let root = make_temp_dir("scan-unreadable-subdirectory");
        let unreadable = root.join("unreadable");
        std::fs::create_dir_all(&unreadable).unwrap();
        write_file(&root.join("available.bms"), "#TITLE Available\n#BPM 120\n#00011:01\n");
        write_file(&unreadable.join("hidden.bms"), "#TITLE Hidden\n#BPM 120\n#00011:01\n");
        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut db = LibraryDatabase::from_connection(conn);
        let roots = vec![PathEntry {
            path: root.to_string_lossy().into_owned(),
            enabled: true,
            recursive: true,
        }];

        let result = scan_song_roots(&mut db, &roots, &scan_config(), 1_700_000_041, false);
        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let report = result.unwrap();

        assert_eq!(report.summary.roots_unreadable, 0);
        assert_eq!(report.summary.discovery_skipped, 1);
        assert_eq!(report.summary.files_seen, 1);
        assert_eq!(report.summary.imported, 1);
        assert_eq!(report.discovery_issues.len(), 1);
        assert_eq!(report.discovery_issues[0].path, unreadable);
        assert_eq!(report.discovery_issues[0].operation, ScanDiscoveryOperation::OpenDirectory);
        assert_eq!(report.discovery_issues[0].error_kind, io::ErrorKind::PermissionDenied);

        let last_scan_at: Option<i64> =
            db.conn().query_row("SELECT last_scan_at FROM roots", [], |row| row.get(0)).unwrap();
        assert_eq!(last_scan_at, None);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_song_roots_records_failed_imports() {
        let root = make_temp_dir("scan-failed");
        // 未定義 WAV id (`#00011:99`) を参照させて warning を発生させる。
        // bms-rs はこのケースでもチャート自体は import するので、`imported_with_warnings`
        // 経路に乗る。
        write_file(&root.join("broken.bms"), "#TITLE Broken\n#BPM 120\n#TOTAL 200\n#00011:0199\n");

        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut db = LibraryDatabase::from_connection(conn);
        let roots = vec![PathEntry {
            path: root.to_string_lossy().into_owned(),
            enabled: true,
            recursive: true,
        }];

        let report =
            scan_song_roots(&mut db, &roots, &scan_config(), 1_700_000_021, false).unwrap();

        assert_eq!(report.summary.files_seen, 1);
        assert_eq!(report.summary.imported, 1);
        assert_eq!(report.summary.failed, 0);

        let (status, warning): (String, String) = db
            .conn()
            .query_row(
                "SELECT chart_files.parse_status, chart_import_warnings.code
                FROM chart_files
                JOIN chart_import_warnings ON chart_import_warnings.chart_file_id = chart_files.id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "Parsed");
        assert_eq!(warning, "MissingWavDefinition");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_song_roots_skips_unchanged_imported_files() {
        let root = make_temp_dir("scan-skip");
        let path = root.join("song.bms");
        write_file(
            &path,
            "\
#TITLE Skip Song
#BPM 120
#00011:01
",
        );

        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut db = LibraryDatabase::from_connection(conn);
        let roots = vec![PathEntry {
            path: root.to_string_lossy().into_owned(),
            enabled: true,
            recursive: true,
        }];

        let first = scan_song_roots(&mut db, &roots, &scan_config(), 1_700_000_022, false).unwrap();
        let second =
            scan_song_roots(&mut db, &roots, &scan_config(), 1_700_000_023, false).unwrap();
        let forced = scan_song_roots(&mut db, &roots, &scan_config(), 1_700_000_024, true).unwrap();

        assert_eq!(first.summary.imported, 1);
        assert_eq!(second.summary.imported, 0);
        assert_eq!(second.summary.skipped, 1);
        assert_eq!(forced.summary.imported, 1);
        assert_eq!(forced.summary.skipped, 0);

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_song_roots_normalizes_preview_extension() {
        let root = make_temp_dir("scan-preview-extension");
        write_file(&root.join("_Preview.ogg"), "ogg");
        write_file(
            &root.join("song.bms"),
            "\
#TITLE Preview Extension
#BPM 120
#PREVIEW _Preview.wav
#00011:01
",
        );

        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut db = LibraryDatabase::from_connection(conn);
        let roots = vec![PathEntry {
            path: root.to_string_lossy().into_owned(),
            enabled: true,
            recursive: true,
        }];

        let report =
            scan_song_roots(&mut db, &roots, &scan_config(), 1_700_000_025, false).unwrap();

        assert_eq!(report.summary.imported, 1);
        let preview_file: String =
            db.conn().query_row("SELECT preview_file FROM charts", [], |row| row.get(0)).unwrap();
        assert_eq!(preview_file, "_Preview.ogg");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_song_roots_fills_preview_prefix_audio_when_header_is_empty() {
        let root = make_temp_dir("scan-preview-prefix");
        write_file(&root.join("preview.ogg"), "ogg");
        write_file(
            &root.join("song.bms"),
            "\
#TITLE Preview Prefix
#BPM 120
#00011:01
",
        );

        let mut conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();
        let mut db = LibraryDatabase::from_connection(conn);
        let roots = vec![PathEntry {
            path: root.to_string_lossy().into_owned(),
            enabled: true,
            recursive: true,
        }];

        let report =
            scan_song_roots(&mut db, &roots, &scan_config(), 1_700_000_026, false).unwrap();

        assert_eq!(report.summary.imported, 1);
        let preview_file: String =
            db.conn().query_row("SELECT preview_file FROM charts", [], |row| row.get(0)).unwrap();
        assert_eq!(preview_file, "preview.ogg");

        std::fs::remove_dir_all(root).unwrap();
    }

    fn make_temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path =
            std::env::temp_dir().join(format!("bmz-player-{label}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_file(path: &Path, text: &str) {
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.sync_all().unwrap();
    }
}
