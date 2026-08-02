use std::path::Path;

use anyhow::Result;
use rusqlite::{Connection, params};

use super::common::configure_connection;

pub struct Migration {
    pub version: i32,
    pub statements: &'static [&'static str],
}

pub fn migrate_library_db(path: &Path) -> Result<()> {
    let mut conn = Connection::open(path)?;
    configure_connection(&conn)?;
    run_migrations(&mut conn, LIBRARY_MIGRATIONS)?;
    backfill_unknown_chart_document_flags(&mut conn)
}

fn backfill_unknown_chart_document_flags(conn: &mut Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT folder_path
         FROM charts
         WHERE has_document IS NULL",
    )?;
    let folders =
        stmt.query_map([], |row| row.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);

    if folders.is_empty() {
        return Ok(());
    }

    let tx = conn.transaction()?;
    {
        let mut update =
            tx.prepare_cached("UPDATE charts SET has_document = ?1 WHERE folder_path = ?2")?;
        for folder in folders {
            let has_document = super::scan::folder_has_document(Path::new(&folder));
            update.execute(params![has_document, folder])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn migrate_score_db(path: &Path) -> Result<()> {
    let mut conn = Connection::open(path)?;
    configure_connection(&conn)?;
    run_score_migrations(&mut conn)
}

pub fn migrate_network_db(path: &Path) -> Result<()> {
    let mut conn = Connection::open(path)?;
    configure_connection(&conn)?;
    run_migrations(&mut conn, NETWORK_MIGRATIONS)
}

pub fn migrate_collection_db(path: &Path) -> Result<()> {
    let mut conn = Connection::open(path)?;
    configure_connection(&conn)?;
    run_migrations(&mut conn, COLLECTION_MIGRATIONS)
}

pub fn run_migrations(conn: &mut Connection, migrations: &[Migration]) -> Result<()> {
    let current_version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

    for migration in migrations {
        if migration.version > current_version {
            let tx = conn.transaction()?;
            for stmt in migration.statements {
                tx.execute_batch(stmt)?;
            }
            tx.pragma_update(None, "user_version", migration.version)?;
            tx.commit()?;
        }
    }

    Ok(())
}

fn run_score_migrations(conn: &mut Connection) -> Result<()> {
    repair_ir_score_migration_20_collision(conn)?;
    run_migrations(conn, SCORE_MIGRATIONS)
}

/// `codex/ir-score-import` originally used score DB migration version 20 for
/// `score_history_sources`, while main independently used the same version for
/// score provenance columns. Repair databases created by that branch before
/// applying main's later migrations, without changing `user_version`.
fn repair_ir_score_migration_20_collision(conn: &mut Connection) -> Result<()> {
    let current_version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current_version != 20 || !table_exists(conn, "score_history_sources")? {
        return Ok(());
    }

    let needs_source_kind = !column_exists(conn, "score_history", "source_kind")?;
    let needs_arrange_2p = !column_exists(conn, "score_history", "arrange_2p")?;
    if !needs_source_kind && !needs_arrange_2p {
        return Ok(());
    }

    let tx = conn.transaction()?;
    if needs_source_kind {
        tx.execute_batch(
            "ALTER TABLE score_history
                ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'Local';
             CREATE INDEX idx_score_history_source_kind_chart_sha256
                ON score_history(source_kind, chart_sha256);",
        )?;
    }
    if needs_arrange_2p {
        tx.execute_batch(
            "ALTER TABLE score_history
                ADD COLUMN arrange_2p TEXT NOT NULL DEFAULT 'Normal';",
        )?;
    }
    tx.commit()?;
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )?)
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1)");
    Ok(conn.query_row(&sql, [column], |row| row.get(0))?)
}

mod collection;
mod library;
mod network;
mod score;

pub use collection::COLLECTION_MIGRATIONS;
pub use library::LIBRARY_MIGRATIONS;
pub use network::NETWORK_MIGRATIONS;
pub use score::SCORE_MIGRATIONS;

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;

    #[test]
    fn library_migration_adds_long_note_pair_counts() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();

        let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
        assert_eq!(version, 30);

        let mut stmt = conn.prepare("PRAGMA table_info(charts)").unwrap();
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        for column in
            ["undefined_ln_pairs", "defined_ln_pairs", "defined_cn_pairs", "defined_hcn_pairs"]
        {
            assert!(columns.iter().any(|candidate| candidate == column));
        }
        assert!(columns.iter().any(|candidate| candidate == "has_document"));

        let chart_file_columns = conn
            .prepare("PRAGMA table_info(chart_files)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(chart_file_columns.iter().any(|column| column == "first_seen_at"));

        let analysis_columns = conn
            .prepare("PRAGMA table_info(chart_analysis)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(analysis_columns.iter().any(|column| column == "loudness_lufs"));
        assert!(!analysis_columns.iter().any(|column| column == "normalization_gain"));
    }

    #[test]
    fn library_migration_keeps_loudness_while_removing_derived_gain() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE chart_analysis (
                loudness_lufs REAL,
                normalization_gain REAL,
                loudness_analysis_version INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE roots (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                last_scan_at INTEGER
            );
            CREATE TABLE chart_files (
                id INTEGER PRIMARY KEY,
                root_id INTEGER,
                path TEXT NOT NULL UNIQUE,
                scanned_at INTEGER NOT NULL
            );
            CREATE TABLE charts (id INTEGER PRIMARY KEY);
            CREATE TABLE chart_file_links (chart_id INTEGER, chart_file_id INTEGER);
            CREATE TABLE chart_import_warnings (chart_file_id INTEGER);
            CREATE TABLE course_entries (chart_id INTEGER);
            INSERT INTO chart_analysis (
                loudness_lufs, normalization_gain, loudness_analysis_version
            ) VALUES (-10.5, 0.75, 1);
            PRAGMA user_version = 27;",
        )
        .unwrap();

        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();

        let loudness: f32 = conn
            .query_row("SELECT loudness_lufs FROM chart_analysis", [], |row| row.get(0))
            .unwrap();
        let columns = conn
            .prepare("PRAGMA table_info(chart_analysis)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(loudness, -10.5);
        assert!(!columns.iter().any(|column| column == "normalization_gain"));
    }

    #[test]
    fn library_document_backfill_reads_existing_song_folders_once() {
        let folder = std::env::temp_dir().join(format!(
            "bmz-library-document-backfill-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("README.TXT"), b"document").unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE charts (
                folder_path TEXT NOT NULL,
                has_document INTEGER
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO charts (folder_path, has_document) VALUES (?1, NULL)",
            params![folder.to_string_lossy()],
        )
        .unwrap();

        backfill_unknown_chart_document_flags(&mut conn).unwrap();

        let has_document: bool =
            conn.query_row("SELECT has_document FROM charts", [], |row| row.get(0)).unwrap();
        assert!(has_document);

        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn library_migration_indexes_difficulty_table_levels() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();

        let columns = conn
            .prepare("PRAGMA index_info(idx_dte_table_id_level)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(2))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert_eq!(columns, ["table_id", "level"]);
    }

    #[test]
    fn library_migration_adds_difficulty_table_download_metadata() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();

        let columns = conn
            .prepare("PRAGMA table_info(difficulty_table_entries)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        for column in ["url", "append_url", "ipfs", "append_ipfs"] {
            assert!(columns.iter().any(|candidate| candidate == column));
        }

        let table_columns = conn
            .prepare("PRAGMA table_info(difficulty_tables)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(table_columns.iter().any(|column| column == "download_metadata_version"));

        conn.execute(
            "INSERT INTO difficulty_tables
             (source_url, head_url, name, symbol, level_order, fetched_at)
             VALUES ('https://example.com/', 'https://example.com/header.json',
                     'Example', '★', '[]', 0)",
            [],
        )
        .unwrap();
        let version: i64 = conn
            .query_row(
                "SELECT download_metadata_version FROM difficulty_tables
                 WHERE source_url = 'https://example.com/'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 0);
    }

    #[test]
    fn library_migration_clears_persisted_raw_headers() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE charts (
                id INTEGER PRIMARY KEY,
                headers_json TEXT NOT NULL,
                sha256 TEXT NOT NULL DEFAULT '',
                md5 TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE course_entries (
                chart_id INTEGER,
                sha256 TEXT NOT NULL DEFAULT '',
                md5 TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE difficulty_table_entries (
                table_id INTEGER NOT NULL,
                level TEXT NOT NULL
             );
             CREATE TABLE difficulty_tables (
                source_url TEXT NOT NULL
             );
             CREATE TABLE chart_analysis (
                loudness_lufs REAL,
                normalization_gain REAL,
                loudness_analysis_version INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE chart_files (
                id INTEGER PRIMARY KEY,
                root_id INTEGER,
                path TEXT NOT NULL UNIQUE,
                scanned_at INTEGER NOT NULL
             );
             CREATE TABLE roots (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                last_scan_at INTEGER
             );
             CREATE TABLE chart_file_links (chart_id INTEGER, chart_file_id INTEGER);
             CREATE TABLE chart_import_warnings (chart_file_id INTEGER);
             INSERT INTO charts (headers_json) VALUES ('{\"002D9\":\"note data\"}');
             PRAGMA user_version = 21;",
        )
        .unwrap();

        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();

        let headers_json: String =
            conn.query_row("SELECT headers_json FROM charts", [], |row| row.get(0)).unwrap();
        let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
        assert_eq!(headers_json, "{}");
        assert_eq!(version, 30);
    }

    #[test]
    fn library_migration_backfills_unresolved_course_entries() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE charts (
                id INTEGER PRIMARY KEY,
                sha256 TEXT NOT NULL,
                md5 TEXT NOT NULL
             );
             CREATE TABLE course_entries (
                position INTEGER PRIMARY KEY,
                chart_id INTEGER,
                sha256 TEXT NOT NULL,
                md5 TEXT NOT NULL
             );
             CREATE TABLE difficulty_table_entries (
                table_id INTEGER NOT NULL,
                level TEXT NOT NULL
             );
             CREATE TABLE difficulty_tables (
                source_url TEXT NOT NULL
             );
             CREATE TABLE chart_analysis (
                loudness_lufs REAL,
                normalization_gain REAL,
                loudness_analysis_version INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE chart_files (
                id INTEGER PRIMARY KEY,
                root_id INTEGER,
                path TEXT NOT NULL UNIQUE,
                scanned_at INTEGER NOT NULL
             );
             CREATE TABLE roots (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                last_scan_at INTEGER
             );
             CREATE TABLE chart_file_links (chart_id INTEGER, chart_file_id INTEGER);
             CREATE TABLE chart_import_warnings (chart_file_id INTEGER);
             INSERT INTO charts (id, sha256, md5) VALUES
                (10, 'preferred-sha', 'other-md5'),
                (20, 'other-sha', 'fallback-md5');
             INSERT INTO course_entries (position, chart_id, sha256, md5) VALUES
                (0, NULL, 'preferred-sha', 'fallback-md5'),
                (1, NULL, 'missing-sha', 'fallback-md5'),
                (2, NULL, 'missing-sha', 'missing-md5'),
                (3, 99, 'preferred-sha', 'fallback-md5');
             PRAGMA user_version = 22;",
        )
        .unwrap();

        run_migrations(&mut conn, LIBRARY_MIGRATIONS).unwrap();

        let chart_ids = conn
            .prepare("SELECT chart_id FROM course_entries ORDER BY position")
            .unwrap()
            .query_map([], |row| row.get::<_, Option<i64>>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
        assert_eq!(chart_ids, vec![Some(10), Some(20), None, Some(99)]);
        assert_eq!(version, 30);
    }

    #[test]
    fn score_migration_backfills_best_score_history_reference() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn, &SCORE_MIGRATIONS[..21]).unwrap();
        conn.execute_batch(
            "INSERT INTO score_history (
                chart_sha256, played_at, clear_type, gauge_type, gauge_value,
                total_notes, ex_score, bp, cb, max_combo,
                fast_pgreat, slow_pgreat, fast_great, slow_great,
                fast_good, slow_good, fast_bad, slow_bad,
                fast_poor, slow_poor, fast_empty_poor, slow_empty_poor,
                gauge_option, replay_path
            ) VALUES (
                'chart', 1, 'NoPlay', '', 0.0,
                0, 0, 0, 0, 0,
                0, 0, 0, 0,
                0, 0, 0, 0,
                0, 0, 0, 0,
                '', ''
            );",
        )
        .unwrap();
        let history_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO score_best (
                chart_sha256, ln_policy, double_option, rule_mode,
                clear_type, gauge_type, gauge_value,
                ex_score, bp, cb, max_combo,
                fast_pgreat, slow_pgreat, fast_great, slow_great,
                fast_good, slow_good, fast_bad, slow_bad,
                fast_poor, slow_poor, fast_empty_poor, slow_empty_poor,
                played_at, replay_path, ghost, play_count, clear_count, device_type
            )
            SELECT
                chart_sha256, ln_policy, double_option, rule_mode,
                clear_type, gauge_type, gauge_value,
                ex_score, bp, cb, max_combo,
                fast_pgreat, slow_pgreat, fast_great, slow_great,
                fast_good, slow_good, fast_bad, slow_bad,
                fast_poor, slow_poor, fast_empty_poor, slow_empty_poor,
                played_at, replay_path, '', 1, 0, device_type
            FROM score_history
            WHERE id = ?1",
            params![history_id],
        )
        .unwrap();
        let version_26 =
            SCORE_MIGRATIONS.iter().position(|migration| migration.version == 26).unwrap();
        run_migrations(&mut conn, &SCORE_MIGRATIONS[..version_26]).unwrap();
        conn.execute(
            "INSERT INTO score_history_sources (
                score_history_id, source, provider, account_id, remote_score_id,
                verification, server_received_at, imported_at
            ) VALUES (?1, 'bmz_ir', 'provider', 'account', 'remote', '', 2, 3)",
            params![history_id],
        )
        .unwrap();
        run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();

        let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
        assert_eq!(version, 26);

        let mut stmt = conn.prepare("PRAGMA table_info(score_best)").unwrap();
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(columns.iter().any(|column| column == "best_score_history_id"));

        let linked_history_id: i64 = conn
            .query_row("SELECT best_score_history_id FROM score_best", [], |row| row.get(0))
            .unwrap();
        assert_eq!(linked_history_id, history_id);

        let source_history_id: i64 = conn
            .query_row("SELECT score_history_id FROM score_history_sources", [], |row| row.get(0))
            .unwrap();
        assert_eq!(source_history_id, history_id);

        for table in ["score_history", "score_best"] {
            let sql = format!(
                "SELECT \"notnull\" FROM pragma_table_info('{table}') WHERE name = 'gauge_value'"
            );
            let not_null: i32 = conn.query_row(&sql, [], |row| row.get(0)).unwrap();
            assert_eq!(not_null, 0, "{table}.gauge_value should be nullable");
            conn.execute(&format!("UPDATE {table} SET gauge_value = NULL"), []).unwrap();
            let gauge_value: Option<f32> = conn
                .query_row(&format!("SELECT gauge_value FROM {table}"), [], |row| row.get(0))
                .unwrap();
            assert_eq!(gauge_value, None);
        }
    }

    #[test]
    fn score_migration_repairs_ir_branch_version_20_collision() {
        let mut conn = Connection::open_in_memory().unwrap();
        let main_version_20 =
            SCORE_MIGRATIONS.iter().position(|migration| migration.version == 20).unwrap();
        run_migrations(&mut conn, &SCORE_MIGRATIONS[..main_version_20]).unwrap();
        conn.execute_batch(
            "CREATE TABLE score_history_sources (
                id INTEGER PRIMARY KEY,
                score_history_id INTEGER NOT NULL
                    REFERENCES score_history(id) ON DELETE CASCADE,
                source TEXT NOT NULL,
                provider TEXT NOT NULL,
                account_id TEXT NOT NULL,
                remote_score_id TEXT NOT NULL,
                verification TEXT NOT NULL DEFAULT '',
                server_received_at INTEGER NOT NULL DEFAULT 0,
                imported_at INTEGER NOT NULL,
                UNIQUE(source, provider, account_id, remote_score_id)
            );
            CREATE INDEX idx_score_history_sources_history
                ON score_history_sources(score_history_id);
            PRAGMA user_version = 20;",
        )
        .unwrap();

        run_score_migrations(&mut conn).unwrap();

        let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
        assert_eq!(version, 26);
        assert!(column_exists(&conn, "score_history", "source_kind").unwrap());
        assert!(column_exists(&conn, "score_history", "arrange_2p").unwrap());
        assert!(column_exists(&conn, "score_history", "applied_double_option").unwrap());
        assert!(column_exists(&conn, "score_history", "seed_scheme").unwrap());
        assert!(table_exists(&conn, "score_history_sources").unwrap());
    }
}
