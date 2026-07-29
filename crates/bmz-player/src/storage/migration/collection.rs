use super::*;

pub const COLLECTION_MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    statements: &[
        "CREATE TABLE favorite_charts (
            chart_sha256 TEXT PRIMARY KEY,
            title_hint TEXT NOT NULL DEFAULT '',
            artist_hint TEXT NOT NULL DEFAULT '',
            folder_hint TEXT NOT NULL DEFAULT '',
            chart_path_hint TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );",
        "CREATE TABLE favorite_songs (
            representative_sha256 TEXT PRIMARY KEY,
            title_hint TEXT NOT NULL DEFAULT '',
            artist_hint TEXT NOT NULL DEFAULT '',
            origin_folder_hint TEXT NOT NULL DEFAULT '',
            origin_chart_path_hint TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );",
        "CREATE INDEX idx_favorite_songs_origin_folder
            ON favorite_songs(origin_folder_hint);",
    ],
}];
