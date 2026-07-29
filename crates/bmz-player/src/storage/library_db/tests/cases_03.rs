use super::*;

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
