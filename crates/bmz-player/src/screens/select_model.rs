use std::collections::{HashMap, HashSet};

use anyhow::Result;
use bmz_core::course::{CourseKind, CourseLnConstraint};
use bmz_gameplay::rule::RuleMode;
use bmz_render::scene::SelectRowKind;

use crate::i18n::{AppLocale, FluentArgs, Localizer};
use crate::ln_policy::{LnPolicySetting, LnScorePolicy, score_ln_policy};
use crate::screens::settings_model::{ConfigSelectRow, KeyBindingSelectRow};
use crate::song_download::ChartDownloadMetadata;
use crate::storage::collection_db::{CollectionDatabase, FavoriteChartRecord, FavoriteSongRecord};
use crate::storage::common::hash_to_hex;
use crate::storage::library_db::{
    ChartAnalysisSummary, ChartListItem, DifficultyTableEntryRecord, LibraryDatabase,
    TableEntryListItem,
};
use crate::storage::score_db::ScoreKey;
use crate::storage::score_db::{BestScoreSummary, ReplaySlotSummary, ScoreDatabase};
mod enrichment;
mod favorites;
mod folder;
mod paths;
mod root;
mod search;
mod table;

use enrichment::*;
use paths::*;
use table::*;

pub use enrichment::{select_folder_summary, select_folder_summary_for_rule_mode};
pub use favorites::{
    apply_collection_flags, favorite_song_representatives_for_folder,
    load_select_items_for_favorite_charts, load_select_items_for_favorite_song,
    load_select_items_for_favorite_songs,
};
pub use folder::{
    load_select_items_in_folder, load_select_items_in_folder_for_rule_mode,
    load_select_items_in_folder_for_rule_mode_with_filters,
    load_select_items_in_folder_for_rule_mode_with_table_order,
};
pub use paths::{
    COURSE_ROOT_PATH, FAVORITE_CHART_PATH, FAVORITE_ROOT_PATH, FAVORITE_SONG_DETAIL_PREFIX,
    FAVORITE_SONG_PATH, MAX_SEARCH_HISTORY, SAME_FOLDER_PATH_PREFIX, SEARCH_PATH_PREFIX,
    TABLE_LEVEL_SEPARATOR, TABLE_ROOT_PATH, TablePath, favorite_song_detail_path,
    parse_favorite_song_detail_path, parse_same_folder_path, parse_search_query, parse_table_path,
    same_folder_path, search_history_folder_items, search_history_folder_items_for_locale,
    song_scan_path_from_context, table_source_url_from_context,
};
pub use root::{
    favorite_root_item, favorite_root_items, random_select_item_from_items, root_folder_items,
};
pub use search::{
    load_select_items_for_search, load_select_items_for_search_for_rule_mode,
    load_select_items_for_search_for_rule_mode_with_filters,
    load_select_items_for_search_for_rule_mode_with_table_order,
};
pub use table::{
    course_root_item, load_select_items_for_courses, load_select_items_in_table,
    load_select_items_in_table_for_rule_mode, load_select_items_in_table_level,
    load_select_items_in_table_level_for_rule_mode, table_folder_items,
    table_folder_items_for_active_sources, table_level_folder_items,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DifficultyTableText {
    pub table_name: String,
    pub table_level: String,
    pub table_full: String,
}

impl DifficultyTableText {
    pub fn from_parts(table_name: String, table_symbol: &str, level: &str) -> Self {
        let table_level = table_level_label(table_symbol, level);
        let table_full = format!("{table_level}{table_name}");
        Self { table_name, table_level, table_full }
    }

    pub fn from_entry(entry: &DifficultyTableEntryRecord) -> Self {
        Self::from_parts(entry.table_name.clone(), &entry.table_symbol, &entry.level)
    }

    pub fn is_table_song(&self) -> bool {
        !self.table_name.is_empty()
    }

    pub fn as_tuple(&self) -> (String, String, String) {
        (self.table_name.clone(), self.table_level.clone(), self.table_full.clone())
    }
}

/// Resolves beatoraja TEXT_TABLE1/2/3 information for a chart.
///
/// TEXT_TABLE1 is the table name, TEXT_TABLE2 is symbol+level, and TEXT_TABLE3
/// is TEXT_TABLE2 + TEXT_TABLE1, matching PlayerResource#getTableFullname().
/// MD5 has priority; SHA-256 is used only when no MD5 table row is found.
pub fn difficulty_table_text_for_chart(
    library_db: &LibraryDatabase,
    chart: &ChartListItem,
    source_order: &[String],
    source_hint: Option<&str>,
) -> Result<DifficultyTableText> {
    difficulty_table_text_for_chart_with_active_sources(
        library_db,
        chart,
        source_order,
        source_hint,
        None,
    )
}

pub fn difficulty_table_text_for_chart_with_active_sources(
    library_db: &LibraryDatabase,
    chart: &ChartListItem,
    source_order: &[String],
    source_hint: Option<&str>,
    active_source_urls: Option<&[String]>,
) -> Result<DifficultyTableText> {
    let md5_hex = hash_to_hex(&chart.md5);
    let mut md5_entries = library_db.list_difficulty_table_entries_by_md5s(&[md5_hex.as_str()])?;
    retain_active_table_entries(&mut md5_entries, active_source_urls);
    if !md5_entries.is_empty() {
        return Ok(choose_difficulty_table_text(md5_entries, source_order, source_hint));
    }

    let sha256_hex = hash_to_hex(&chart.sha256);
    let mut sha256_entries =
        library_db.list_difficulty_table_entries_by_sha256s(&[sha256_hex.as_str()])?;
    retain_active_table_entries(&mut sha256_entries, active_source_urls);
    Ok(choose_difficulty_table_text(sha256_entries, source_order, source_hint))
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectChartRow {
    pub chart: Option<ChartListItem>,
    pub chart_analysis: Option<ChartAnalysisSummary>,
    /// beatoraja `SongData.hasDocument()` compatible same-folder `.txt` presence.
    pub has_document: bool,
    pub fallback_title: String,
    pub fallback_artist: String,
    pub entry_sha256: Option<[u8; 32]>,
    pub download_metadata: ChartDownloadMetadata,
    pub best_score: Option<BestScoreSummary>,
    pub replay_slots: [bool; 4],
    pub favorite_chart: bool,
    pub favorite_song: bool,
    pub table_level: String,
    pub table_text: DifficultyTableText,
}

impl SelectChartRow {
    pub fn display_title(&self) -> &str {
        self.chart
            .as_ref()
            .map(|chart| chart.title.as_str())
            .filter(|title| !title.is_empty())
            .unwrap_or(self.fallback_title.as_str())
    }

    pub fn display_artist(&self) -> &str {
        self.chart
            .as_ref()
            .map(|chart| chart.artist.as_str())
            .filter(|artist| !artist.is_empty())
            .unwrap_or(self.fallback_artist.as_str())
    }

    pub fn in_library(&self) -> bool {
        self.chart.is_some()
    }

    pub fn score_sha256(&self) -> Option<[u8; 32]> {
        self.chart.as_ref().map(|chart| chart.sha256).or(self.entry_sha256)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectCourseRow {
    pub course_id: i64,
    /// Canonical IR course hash. None while any course entry is unresolved.
    pub course_hash: Option<String>,
    /// rianIR/beatoraja connector互換のremote course hash。
    pub rian_course_hash_v1: Option<String>,
    pub title: String,
    pub kind: CourseKind,
    pub constraints: bmz_core::course::CourseConstraints,
    /// Total number of entries in the course.
    pub entry_count: usize,
    /// Number of entries whose `chart_id` is resolved in the local library.
    pub resolved_count: usize,
    /// Total notes across all resolved entries.
    pub total_notes: u32,
    /// Sum of length in milliseconds across resolved entries.
    pub total_length_ms: i64,
    /// Minimum / maximum BPM among resolved entries.
    pub min_bpm: f32,
    pub max_bpm: f32,
    /// Difficulty band derived from constraints (e.g. "DAN" / "COURSE").
    pub category_label: String,
    /// Trophy names defined for this course (e.g. ["silvermedal", "goldmedal"]).
    pub trophy_names: Vec<String>,
    /// Entries inside the course, used by the preview panel.
    pub entry_previews: Vec<CourseEntryPreview>,
    /// Best persisted course score, if any.  Populated from the
    /// `course_scores` table; `None` when the course has never been played
    /// successfully or when the lookup failed.
    pub best_score: Option<crate::storage::score_db::CourseBestScore>,
    /// Which of the four course replay slots have a saved attempt.  Used by
    /// the select skin to render slot indicators on course rows.
    pub replay_slots: [bool; 4],
    /// Names of trophies that have been earned at least once across all
    /// stored attempts of this course (`course_trophy_achievements`).  A
    /// strict subset of `trophy_names`.
    pub achieved_trophy_names: Vec<String>,
}

impl SelectCourseRow {
    /// beatoraja `GradeBar.existsAllSongs()`: a course is playable only when
    /// every declared entry resolves to a local song.
    pub fn exists_all_songs(&self) -> bool {
        self.entry_count > 0 && self.resolved_count == self.entry_count
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CourseEntryPreview {
    /// Title taken from the resolved library chart when available, otherwise
    /// the title_hint declared in the course JSON.
    pub title: String,
    pub artist: String,
    pub play_level: String,
    pub difficulty_name: String,
    pub total_notes: u32,
    /// True when this entry is resolved to a chart in the local library.
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectFolderSummary {
    pub lamp_counts: [u32; 11],
}

impl SelectFolderSummary {
    pub fn clear_type(&self) -> String {
        let index = self.lamp_counts.iter().position(|count| *count > 0).unwrap_or(0);
        clear_type_name_for_folder_lamp(index).to_string()
    }
}

impl From<&[SelectChartRow]> for SelectFolderSummary {
    fn from(rows: &[SelectChartRow]) -> Self {
        let mut lamp_counts = [0; 11];
        for row in rows {
            let index = row
                .best_score
                .as_ref()
                .map(|score| folder_lamp_index_from_clear_type(&score.clear_type))
                .unwrap_or(0);
            lamp_counts[index] += 1;
        }
        Self { lamp_counts }
    }
}

fn folder_lamp_index_from_clear_type(clear_type: &str) -> usize {
    usize::from(bmz_core::clear::ClearType::rank_from_label(clear_type))
}

fn clear_type_name_for_folder_lamp(index: usize) -> &'static str {
    match index {
        1 => "Failed",
        2 => "AssistEasy",
        3 => "LightAssistEasy",
        4 => "Easy",
        5 => "Normal",
        6 => "Hard",
        7 => "ExHard",
        8 => "FullCombo",
        9 => "Perfect",
        10 => "Max",
        _ => "",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectExecutableKind {
    RandomSelect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectExecutableRow {
    pub title: String,
    pub kind: SelectExecutableKind,
    pub chart_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum SelectItem {
    Folder {
        path: String,
        name: String,
        kind: SelectRowKind,
        summary: Option<SelectFolderSummary>,
    },
    Chart(SelectChartRow),
    Course(SelectCourseRow),
    Executable(SelectExecutableRow),
    Config(ConfigSelectRow),
    KeyBinding(KeyBindingSelectRow),
    /// 設定カテゴリから 1 階層戻るアクション行。
    SettingsBack,
    /// 設定ルートを閉じるアクション行。
    SettingsClose,
    /// ゲーム内設定から egui の詳細設定ウィンドウを開くアクション行。
    AdvancedSettings,
}

impl SelectItem {
    pub fn display_name(&self) -> String {
        self.display_name_for_locale(AppLocale::DEFAULT)
    }

    pub fn display_name_for_locale(&self, locale: AppLocale) -> String {
        match self {
            Self::Folder { name, .. } => name.clone(),
            Self::Chart(row) => row.display_title().to_string(),
            Self::Course(row) => row.title.clone(),
            Self::Executable(row) => row.title.clone(),
            Self::Config(row) => row.label().to_string(),
            Self::KeyBinding(row) => row.label(),
            Self::SettingsBack => Localizer::new(locale).text("select-back"),
            Self::SettingsClose => Localizer::new(locale).text("select-close"),
            Self::AdvancedSettings => Localizer::new(locale).text("select-advanced-settings"),
        }
    }
}
#[cfg(test)]
mod tests {
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

    #[test]
    fn load_select_items_in_folder_attaches_best_scores_by_hash() {
        let (mut library_db, mut score_db) = open_in_memory_dbs();
        let alpha = chart("Alpha");
        let beta = chart("Beta");

        library_db.upsert_chart_import(&record_for_chart("/songs/alpha.bms", &alpha)).unwrap();
        library_db.upsert_chart_import(&record_for_chart("/songs/beta.bms", &beta)).unwrap();
        score_db.insert_score(&score_for_chart(alpha.identity.file_sha256)).unwrap();

        let items =
            load_select_items_in_folder(&library_db, &score_db, "/songs", LnPolicySetting::AutoLn)
                .unwrap();

        let charts: Vec<_> = items
            .iter()
            .filter_map(|i| if let SelectItem::Chart(r) = i { Some(r) } else { None })
            .collect();
        assert_eq!(charts.len(), 2);
        assert_eq!(charts[0].display_title(), "Alpha");
        assert!(charts[0].best_score.is_some());
        assert_eq!(charts[1].display_title(), "Beta");
        assert!(charts[1].best_score.is_none());
    }

    #[test]
    fn load_select_items_in_folder_attaches_replay_slots_from_replay_slots_table() {
        let (mut library_db, mut score_db) = open_in_memory_dbs();
        let alpha = chart("Alpha");

        library_db.upsert_chart_import(&record_for_chart("/songs/alpha.bms", &alpha)).unwrap();
        for slot in 0..4_u8 {
            score_db
                .upsert_replay_slot(&crate::storage::score_db::ReplaySlotRecord {
                    chart_sha256: alpha.identity.file_sha256,
                    ln_policy: LnScorePolicy::ForceLn,
                    double_option: crate::select_options::DoubleOptionScoreBucket::Off,
                    rule_mode: RuleMode::Beatoraja,
                    slot,
                    rule: crate::config::profile_config::ReplaySlotRule::Always,
                    replay_path: format!("replay/{slot}.toml"),
                    played_at: 1_700_000_030 + slot as i64,
                    ex_score: 10 * slot as u32,
                    bp: 0,
                    cb: 0,
                    max_combo: 10,
                    clear_rank: ClearType::Normal as u8,
                })
                .unwrap();
        }

        let items =
            load_select_items_in_folder(&library_db, &score_db, "/songs", LnPolicySetting::AutoLn)
                .unwrap();

        let row = items
            .iter()
            .find_map(|i| if let SelectItem::Chart(r) = i { Some(r) } else { None })
            .unwrap();
        assert_eq!(row.replay_slots, [true, true, true, true]);
    }

    #[test]
    fn load_select_items_uses_profile_ln_policy_for_score_lookup() {
        let (mut library_db, mut score_db) = open_in_memory_dbs();
        let mut alpha = chart("Alpha");
        alpha.long_notes.push(undefined_ln_pair());
        library_db.upsert_chart_import(&record_for_chart("/songs/alpha.bms", &alpha)).unwrap();
        let mut force_ln_score = score_for_chart(alpha.identity.file_sha256);
        force_ln_score.ln_policy = LnScorePolicy::ForceLn;
        force_ln_score.score.judges.slow_pgreat = 50;
        let mut force_cn_score = score_for_chart(alpha.identity.file_sha256);
        force_cn_score.ln_policy = LnScorePolicy::ForceCn;
        force_cn_score.score.judges.slow_pgreat = 100;
        score_db.insert_score(&force_ln_score).unwrap();
        score_db.insert_score(&force_cn_score).unwrap();

        let items =
            load_select_items_in_folder(&library_db, &score_db, "/songs", LnPolicySetting::AutoCn)
                .unwrap();

        let row = items
            .iter()
            .find_map(|i| if let SelectItem::Chart(r) = i { Some(r) } else { None })
            .unwrap();
        assert_eq!(row.best_score.as_ref().map(|s| s.ln_policy), Some(LnScorePolicy::ForceCn));
        assert_eq!(row.best_score.as_ref().map(|s| s.ex_score), Some(200));
    }

    #[test]
    fn load_select_items_in_folder_flattens_leaf_subfolders() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let chart_a = chart("A");
        let chart_b = chart("B");

        // chart_b directly in /bms; chart_a is in a leaf sub-folder (no deeper nesting)
        library_db
            .upsert_chart_import(&record_for_chart("/bms/genre/song_a.bms", &chart_a))
            .unwrap();
        library_db.upsert_chart_import(&record_for_chart("/bms/song_b.bms", &chart_b)).unwrap();

        let items =
            load_select_items_in_folder(&library_db, &score_db, "/bms", LnPolicySetting::AutoLn)
                .unwrap();

        // genre is a leaf folder so its chart appears directly, not as a Folder entry
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| matches!(i, SelectItem::Chart(_))));
        let titles: Vec<_> =
            items
                .iter()
                .filter_map(|i| {
                    if let SelectItem::Chart(r) = i { Some(r.display_title()) } else { None }
                })
                .collect();
        assert!(titles.contains(&"A"));
        assert!(titles.contains(&"B"));
    }

    #[test]
    fn load_select_items_in_folder_shows_non_leaf_subfolder_as_folder() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let chart_a = chart("A");
        let chart_b = chart("B");

        // genre/subgenre/song_a — genre has a subfolder so it is non-leaf
        library_db
            .upsert_chart_import(&record_for_chart("/bms/genre/subgenre/song_a.bms", &chart_a))
            .unwrap();
        library_db.upsert_chart_import(&record_for_chart("/bms/song_b.bms", &chart_b)).unwrap();

        let items =
            load_select_items_in_folder(&library_db, &score_db, "/bms", LnPolicySetting::AutoLn)
                .unwrap();

        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], SelectItem::Folder { name, .. } if name == "genre"));
        assert!(matches!(&items[1], SelectItem::Chart(r) if r.display_title() == "B"));
    }

    #[test]
    fn load_select_items_in_folder_with_filters_hides_charts_outside_active_roots() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let active = chart("Active Song");
        let stale = chart("Stale Song");
        library_db
            .upsert_chart_import(&record_for_chart("/songs/enabled/active.bms", &active))
            .unwrap();
        library_db
            .upsert_chart_import(&record_for_chart("/songs/removed/stale.bms", &stale))
            .unwrap();

        let active_roots = vec!["/songs/enabled".to_string()];
        let items = load_select_items_in_folder_for_rule_mode_with_filters(
            &library_db,
            &score_db,
            "/songs",
            LnPolicySetting::AutoLn,
            RuleMode::Beatoraja,
            &[],
            Some(&active_roots),
            None,
        )
        .unwrap();

        let titles: Vec<_> = items
            .iter()
            .filter_map(|item| {
                if let SelectItem::Chart(row) = item { Some(row.display_title()) } else { None }
            })
            .collect();
        assert_eq!(titles, vec!["Active Song"]);
    }

    #[test]
    fn select_folder_summary_counts_recursive_folder_lamps() {
        let (mut library_db, mut score_db) = open_in_memory_dbs();
        let normal = chart("Normal");
        let hard = chart("Hard");
        let unplayed = chart("Unplayed");
        let outside = chart("Outside");
        library_db
            .upsert_chart_import(&record_for_chart("/songs/folder/normal.bms", &normal))
            .unwrap();
        library_db
            .upsert_chart_import(&record_for_chart("/songs/folder/sub/hard.bms", &hard))
            .unwrap();
        library_db
            .upsert_chart_import(&record_for_chart("/songs/folder/sub/unplayed.bms", &unplayed))
            .unwrap();
        library_db.upsert_chart_import(&record_for_chart("/songs/outside.bms", &outside)).unwrap();
        score_db.insert_score(&score_for_chart(normal.identity.file_sha256)).unwrap();
        let mut hard_score = score_for_chart(hard.identity.file_sha256);
        hard_score.clear_type = ClearType::Hard;
        score_db.insert_score(&hard_score).unwrap();
        score_db.insert_score(&score_for_chart(outside.identity.file_sha256)).unwrap();

        let summary = select_folder_summary(
            &library_db,
            &score_db,
            "/songs/folder",
            SelectRowKind::Folder,
            LnPolicySetting::AutoLn,
        )
        .unwrap()
        .unwrap();

        assert_eq!(summary.lamp_counts[0], 1);
        assert_eq!(summary.lamp_counts[5], 1);
        assert_eq!(summary.lamp_counts[6], 1);
        assert_eq!(summary.lamp_counts.iter().sum::<u32>(), 3);
        assert_eq!(summary.clear_type(), "");
    }

    #[test]
    fn root_folder_items_returns_folder_per_root() {
        let roots = vec!["/bms/a".to_string(), "/bms/b".to_string()];
        let items = root_folder_items(&roots);

        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], SelectItem::Folder { name, .. } if name == "a"));
        assert!(matches!(&items[1], SelectItem::Folder { name, .. } if name == "b"));
    }

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

    #[test]
    fn favorite_song_resolves_all_duplicate_sha256_folders() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let mut collection_db = open_in_memory_collection_db();
        let shared = chart("Shared");
        library_db
            .upsert_chart_import(&record_for_chart("/pack-a/song/shared.bms", &shared))
            .unwrap();
        library_db
            .upsert_chart_import(&record_for_chart("/pack-b/song/shared.bms", &shared))
            .unwrap();
        collection_db
            .upsert_favorite_song(
                shared.identity.file_sha256,
                &crate::storage::collection_db::FavoriteHints::new(
                    "Shared",
                    "artist",
                    "/pack-a/song",
                ),
                10,
            )
            .unwrap();

        assert_eq!(
            favorite_song_representatives_for_folder(&library_db, &collection_db, "/pack-a/song")
                .unwrap(),
            vec![shared.identity.file_sha256]
        );
        assert_eq!(
            favorite_song_representatives_for_folder(&library_db, &collection_db, "/pack-b/song")
                .unwrap(),
            vec![shared.identity.file_sha256]
        );

        let items = load_select_items_for_favorite_song(
            &library_db,
            &score_db,
            &collection_db,
            shared.identity.file_sha256,
            LnPolicySetting::AutoLn,
            RuleMode::Beatoraja,
            &[],
            None,
            None,
        )
        .unwrap();
        let folders: HashSet<String> = items
            .iter()
            .filter_map(|item| match item {
                SelectItem::Chart(row) => row.chart.as_ref().map(|chart| chart.folder_path.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(folders.len(), 2);
        assert!(folders.contains("/pack-a/song"));
        assert!(folders.contains("/pack-b/song"));
        assert!(items.iter().all(|item| match item {
            SelectItem::Chart(row) => row.favorite_song,
            _ => true,
        }));
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
    fn load_select_items_attaches_table_level_via_md5() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let alpha = chart("Alpha");
        library_db.upsert_chart_import(&record_for_chart("/songs/alpha.bms", &alpha)).unwrap();

        let table = difficulty_table_for_md5(&alpha.identity.file_md5, "★", "3");
        library_db.upsert_difficulty_table(&table).unwrap();

        let items =
            load_select_items_in_folder(&library_db, &score_db, "/songs", LnPolicySetting::AutoLn)
                .unwrap();

        let row = items
            .iter()
            .find_map(|i| if let SelectItem::Chart(r) = i { Some(r) } else { None })
            .unwrap();
        assert_eq!(row.table_level, "★3");
        assert_eq!(row.table_text.table_name, "Table");
        assert_eq!(row.table_text.table_level, "★3");
        assert_eq!(row.table_text.table_full, "★3Table");
    }

    #[test]
    fn load_select_items_joins_multiple_table_levels_with_slash() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let alpha = chart("Alpha");
        library_db.upsert_chart_import(&record_for_chart("/songs/alpha.bms", &alpha)).unwrap();

        library_db
            .upsert_difficulty_table(&difficulty_table_for_md5(&alpha.identity.file_md5, "★", "3"))
            .unwrap();
        library_db
            .upsert_difficulty_table(&difficulty_table_for_md5(&alpha.identity.file_md5, "☆", "5"))
            .unwrap();

        let items =
            load_select_items_in_folder(&library_db, &score_db, "/songs", LnPolicySetting::AutoLn)
                .unwrap();

        let row = items
            .iter()
            .find_map(|i| if let SelectItem::Chart(r) = i { Some(r) } else { None })
            .unwrap();
        assert!(row.table_level.contains("★3"), "got: {}", row.table_level);
        assert!(row.table_level.contains("☆5"), "got: {}", row.table_level);
        assert!(row.table_level.contains('/'), "got: {}", row.table_level);
    }

    #[test]
    fn load_select_items_falls_back_to_sha256_when_no_md5_match() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let alpha = chart("Alpha");
        library_db.upsert_chart_import(&record_for_chart("/songs/alpha.bms", &alpha)).unwrap();

        let table = difficulty_table_for_sha256(&alpha.identity.file_sha256, "◆", "7");
        library_db.upsert_difficulty_table(&table).unwrap();

        let items =
            load_select_items_in_folder(&library_db, &score_db, "/songs", LnPolicySetting::AutoLn)
                .unwrap();

        let row = items
            .iter()
            .find_map(|i| if let SelectItem::Chart(r) = i { Some(r) } else { None })
            .unwrap();
        assert_eq!(row.table_level, "◆7");
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

    #[test]
    fn table_folder_items_returns_one_folder_per_table() {
        let (mut library_db, _) = open_in_memory_dbs();
        let alpha = chart("Alpha");
        // Register table using md5 so there's at least one entry (content does not matter here)
        let table = difficulty_table_for_md5(&alpha.identity.file_md5, "★", "1");
        library_db.upsert_difficulty_table(&table).unwrap();

        let items = table_folder_items(&library_db, &[]).unwrap();

        assert_eq!(items.len(), 1);
        assert!(matches!(
            &items[0],
            SelectItem::Folder { path, name, kind, .. }
            if path.starts_with(TABLE_ROOT_PATH) && name == "Table" && *kind == SelectRowKind::TableFolder
        ));
    }

    #[test]
    fn table_folder_items_follow_config_source_order() {
        let (mut library_db, _) = open_in_memory_dbs();
        let chart = chart("Table Song");
        let table_a = difficulty_table_for_md5(&chart.identity.file_md5, "A", "1");
        let table_b = difficulty_table_for_md5(&chart.identity.file_md5, "B", "1");
        library_db.upsert_difficulty_table(&table_a).unwrap();
        library_db.upsert_difficulty_table(&table_b).unwrap();

        let items = table_folder_items(
            &library_db,
            &["https://example.com/B/".to_string(), "https://example.com/A/".to_string()],
        )
        .unwrap();

        let folders: Vec<_> = items
            .iter()
            .filter_map(|item| {
                if let SelectItem::Folder { path, name, .. } = item {
                    Some((path.as_str(), name.as_str()))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            folders,
            vec![
                ("bmz-table:https://example.com/B/", "Table"),
                ("bmz-table:https://example.com/A/", "Table"),
            ]
        );
    }

    #[test]
    fn table_folder_items_with_active_sources_hides_removed_tables() {
        let (mut library_db, _) = open_in_memory_dbs();
        let chart = chart("Table Song");
        let table_a = difficulty_table_for_md5(&chart.identity.file_md5, "A", "1");
        let table_b = difficulty_table_for_md5(&chart.identity.file_md5, "B", "1");
        library_db.upsert_difficulty_table(&table_a).unwrap();
        library_db.upsert_difficulty_table(&table_b).unwrap();

        let active_sources = vec!["https://example.com/B/".to_string()];
        let items = table_folder_items_for_active_sources(
            &library_db,
            &active_sources,
            Some(&active_sources),
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        assert!(matches!(
            &items[0],
            SelectItem::Folder { path, .. } if path == "bmz-table:https://example.com/B/"
        ));
    }

    #[test]
    fn chart_enrichment_with_filters_hides_removed_table_levels() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let chart = chart("Table Song");
        library_db.upsert_chart_import(&record_for_chart("/songs/table.bms", &chart)).unwrap();
        library_db
            .upsert_difficulty_table(&difficulty_table_for_md5(&chart.identity.file_md5, "A", "1"))
            .unwrap();
        library_db
            .upsert_difficulty_table(&difficulty_table_for_md5(&chart.identity.file_md5, "B", "2"))
            .unwrap();

        let active_roots = vec!["/songs".to_string()];
        let active_sources = vec!["https://example.com/B/".to_string()];
        let items = load_select_items_in_folder_for_rule_mode_with_filters(
            &library_db,
            &score_db,
            "/songs",
            LnPolicySetting::AutoLn,
            RuleMode::Beatoraja,
            &active_sources,
            Some(&active_roots),
            Some(&active_sources),
        )
        .unwrap();

        let row = items
            .iter()
            .find_map(|item| if let SelectItem::Chart(row) = item { Some(row) } else { None })
            .unwrap();
        assert_eq!(row.table_level, "B2");
        assert_eq!(row.table_text.table_level, "B2");
    }

    #[test]
    fn load_select_items_in_table_returns_charts_sorted_by_level_order() {
        let (mut library_db, score_db) = open_in_memory_dbs();

        let hard = chart("Hard Song");
        let easy = chart("Easy Song");
        library_db.upsert_chart_import(&record_for_chart("/songs/hard.bms", &hard)).unwrap();
        library_db.upsert_chart_import(&record_for_chart("/songs/easy.bms", &easy)).unwrap();

        // Table has level_order ["5", "10"] — easy(5) before hard(10)
        use crate::difficulty_table::{FetchedDifficultyTable, FetchedTableEntry};
        let table = FetchedDifficultyTable {
            source_url: "https://example.com/table/".to_string(),
            head_url: "https://example.com/table/header.json".to_string(),
            name: "Test Table".to_string(),
            symbol: "★".to_string(),
            level_order: vec!["5".to_string(), "10".to_string()],
            entries: vec![
                FetchedTableEntry {
                    level: "10".to_string(),
                    md5: hash_to_hex(&hard.identity.file_md5),
                    sha256: String::new(),
                    title: String::new(),
                    artist: String::new(),
                    comment: String::new(),
                    ..FetchedTableEntry::default()
                },
                FetchedTableEntry {
                    level: "5".to_string(),
                    md5: hash_to_hex(&easy.identity.file_md5),
                    sha256: String::new(),
                    title: String::new(),
                    artist: String::new(),
                    comment: String::new(),
                    ..FetchedTableEntry::default()
                },
            ],
            courses: Vec::new(),
            fetched_at: 0,
        };
        library_db.upsert_difficulty_table(&table).unwrap();

        let items = load_select_items_in_table(
            &library_db,
            &score_db,
            "https://example.com/table/",
            LnPolicySetting::AutoLn,
        )
        .unwrap();

        assert_eq!(items.len(), 2);
        let titles: Vec<_> =
            items
                .iter()
                .filter_map(|i| {
                    if let SelectItem::Chart(r) = i { Some(r.display_title()) } else { None }
                })
                .collect();
        assert_eq!(titles[0], "Easy Song");
        assert_eq!(titles[1], "Hard Song");

        // table_level should be formatted as symbol+level
        let levels: Vec<_> = items
            .iter()
            .filter_map(|i| {
                if let SelectItem::Chart(r) = i { Some(r.table_level.as_str()) } else { None }
            })
            .collect();
        assert_eq!(levels[0], "★5");
        assert_eq!(levels[1], "★10");
    }

    #[test]
    fn table_source_url_from_context_reads_stack_and_selection() {
        let stack = vec!["bmz-table:https://example.com/t/\n12".to_string()];
        assert_eq!(
            table_source_url_from_context(&stack, None),
            Some("https://example.com/t/".to_string())
        );

        let selected = SelectItem::Folder {
            path: "bmz-table:https://example.com/other/".to_string(),
            name: "[★] Other".to_string(),
            kind: SelectRowKind::TableFolder,
            summary: None,
        };
        assert_eq!(
            table_source_url_from_context(&[], Some(&selected)),
            Some("https://example.com/other/".to_string())
        );

        assert_eq!(table_source_url_from_context(&[], None), None);
    }

    #[test]
    fn song_scan_path_from_context_reads_folder_and_chart() {
        let folder = SelectItem::Folder {
            path: "/music/bms".to_string(),
            name: "bms".to_string(),
            kind: SelectRowKind::Folder,
            summary: None,
        };
        assert_eq!(song_scan_path_from_context(&[], Some(&folder)), Some("/music/bms".to_string()));

        let chart = SelectItem::Chart(SelectChartRow {
            chart: Some(ChartListItem {
                chart_id: 1,
                md5: [0; 16],
                sha256: [0; 32],
                title: "Song".to_string(),
                subtitle: String::new(),
                artist: String::new(),
                subartist: String::new(),
                genre: String::new(),
                difficulty_name: String::new(),
                play_level: String::new(),
                mode: String::new(),
                total_notes: 10,
                initial_bpm: 120.0,
                min_bpm: 120.0,
                max_bpm: 120.0,
                length_ms: 0,
                folder_path: "/music/bms/album".to_string(),
                stage_file: String::new(),
                banner_file: String::new(),
                backbmp_file: String::new(),
                preview_file: String::new(),
                has_document: false,
                has_long_notes: false,
                has_mines: false,
                judge_rank: None,
                bms_total: 0.0,
                ln_profile: Default::default(),
                ln_counts: Default::default(),
            }),
            chart_analysis: None,
            has_document: false,
            fallback_title: String::new(),
            fallback_artist: String::new(),
            entry_sha256: None,
            download_metadata: ChartDownloadMetadata::default(),
            best_score: None,
            replay_slots: [false; 4],
            favorite_chart: false,
            favorite_song: false,
            table_level: String::new(),
            table_text: DifficultyTableText::default(),
        });
        assert_eq!(
            song_scan_path_from_context(&[], Some(&chart)),
            Some("/music/bms/album".to_string())
        );
    }

    #[test]
    fn parse_table_path_distinguishes_root_table_and_level() {
        assert_eq!(parse_table_path("bmz-table:"), Some(TablePath::Root));
        assert_eq!(
            parse_table_path("bmz-table:https://example.com/t/"),
            Some(TablePath::Table { source_url: "https://example.com/t/" })
        );
        assert_eq!(
            parse_table_path("bmz-table:https://example.com/t/\n12"),
            Some(TablePath::Level { source_url: "https://example.com/t/", level: "12" })
        );
        assert_eq!(parse_table_path("/songs/folder"), None);
    }

    #[test]
    fn table_level_folder_items_returns_folder_per_level() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let chart_a = chart("A");
        use crate::difficulty_table::{FetchedDifficultyTable, FetchedTableEntry};
        let table = FetchedDifficultyTable {
            source_url: "https://example.com/insane/".to_string(),
            head_url: "https://example.com/insane/header.json".to_string(),
            name: "Insane".to_string(),
            symbol: "★".to_string(),
            level_order: vec!["1".to_string(), "2".to_string(), "25".to_string()],
            entries: vec![FetchedTableEntry {
                level: "2".to_string(),
                md5: hash_to_hex(&chart_a.identity.file_md5),
                sha256: String::new(),
                title: String::new(),
                artist: String::new(),
                comment: String::new(),
                ..FetchedTableEntry::default()
            }],
            courses: Vec::new(),
            fetched_at: 0,
        };
        library_db.upsert_difficulty_table(&table).unwrap();

        let items = table_level_folder_items(
            &library_db,
            &score_db,
            "https://example.com/insane/",
            LnPolicySetting::AutoLn,
            RuleMode::Beatoraja,
        )
        .unwrap();

        assert_eq!(items.len(), 3);
        assert!(matches!(
            &items[0],
            SelectItem::Folder { path, name, kind, .. }
            if name == "★1" && path == "bmz-table:https://example.com/insane/\n1" && *kind == SelectRowKind::TableFolder
        ));
        assert!(matches!(&items[2], SelectItem::Folder { name, .. } if name == "★25"));
    }

    #[test]
    fn load_select_items_in_table_level_filters_by_level() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let easy = chart("Easy Song");
        let hard = chart("Hard Song");
        library_db.upsert_chart_import(&record_for_chart("/songs/easy.bms", &easy)).unwrap();
        library_db.upsert_chart_import(&record_for_chart("/songs/hard.bms", &hard)).unwrap();

        use crate::difficulty_table::{FetchedDifficultyTable, FetchedTableEntry};
        let table = FetchedDifficultyTable {
            source_url: "https://example.com/insane/".to_string(),
            head_url: "https://example.com/insane/header.json".to_string(),
            name: "Insane".to_string(),
            symbol: "★".to_string(),
            level_order: vec!["5".to_string(), "10".to_string()],
            entries: vec![
                FetchedTableEntry {
                    level: "5".to_string(),
                    md5: hash_to_hex(&easy.identity.file_md5),
                    sha256: String::new(),
                    title: String::new(),
                    artist: String::new(),
                    comment: String::new(),
                    ..FetchedTableEntry::default()
                },
                FetchedTableEntry {
                    level: "10".to_string(),
                    md5: hash_to_hex(&hard.identity.file_md5),
                    sha256: String::new(),
                    title: String::new(),
                    artist: String::new(),
                    comment: String::new(),
                    ..FetchedTableEntry::default()
                },
            ],
            courses: Vec::new(),
            fetched_at: 0,
        };
        library_db.upsert_difficulty_table(&table).unwrap();

        let items = load_select_items_in_table_level(
            &library_db,
            &score_db,
            "https://example.com/insane/",
            "5",
            LnPolicySetting::AutoLn,
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], SelectItem::Chart(r) if r.display_title() == "Easy Song"));
    }

    #[test]
    fn load_select_items_in_table_level_shows_missing_library_entry() {
        let (mut library_db, score_db) = open_in_memory_dbs();

        use crate::difficulty_table::{FetchedDifficultyTable, FetchedTableEntry};
        let table = FetchedDifficultyTable {
            source_url: "https://example.com/missing/".to_string(),
            head_url: "https://example.com/missing/header.json".to_string(),
            name: "Missing".to_string(),
            symbol: "★".to_string(),
            level_order: vec!["12".to_string()],
            entries: vec![FetchedTableEntry {
                level: "12".to_string(),
                md5: "aabbcc".repeat(5) + "aabb",
                sha256: String::new(),
                title: "Missing Song".to_string(),
                artist: "Missing Artist".to_string(),
                comment: String::new(),
                url: "https://example.com/missing".to_string(),
                append_url: "https://example.com/missing-diff".to_string(),
                ipfs: "/ipfs/bafybeigdyrzt5sfp7udm7hu76uh7y26nf3ktekzrxql4i5f3u".to_string(),
                append_ipfs: String::new(),
            }],
            courses: Vec::new(),
            fetched_at: 0,
        };
        library_db.upsert_difficulty_table(&table).unwrap();

        let items = load_select_items_in_table_level(
            &library_db,
            &score_db,
            "https://example.com/missing/",
            "12",
            LnPolicySetting::AutoLn,
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        assert!(matches!(
            &items[0],
            SelectItem::Chart(row)
            if row.display_title() == "Missing Song"
                && row.display_artist() == "Missing Artist"
                && !row.in_library()
                && row.download_metadata.url == "https://example.com/missing"
                && row.download_metadata.ipfs.starts_with("/ipfs/")
        ));
    }

    #[test]
    fn load_select_items_in_table_level_prefers_library_title_when_registered() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let chart = chart("Library Title");
        library_db.upsert_chart_import(&record_for_chart("/songs/registered.bms", &chart)).unwrap();

        use crate::difficulty_table::{FetchedDifficultyTable, FetchedTableEntry};
        let table = FetchedDifficultyTable {
            source_url: "https://example.com/registered/".to_string(),
            head_url: "https://example.com/registered/header.json".to_string(),
            name: "Registered".to_string(),
            symbol: "★".to_string(),
            level_order: vec!["12".to_string()],
            entries: vec![FetchedTableEntry {
                level: "12".to_string(),
                md5: hash_to_hex(&chart.identity.file_md5),
                sha256: String::new(),
                title: "Table Title".to_string(),
                artist: "Table Artist".to_string(),
                comment: String::new(),
                ..FetchedTableEntry::default()
            }],
            courses: Vec::new(),
            fetched_at: 0,
        };
        library_db.upsert_difficulty_table(&table).unwrap();

        let items = load_select_items_in_table_level(
            &library_db,
            &score_db,
            "https://example.com/registered/",
            "12",
            LnPolicySetting::AutoLn,
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        assert!(matches!(
            &items[0],
            SelectItem::Chart(row)
            if row.display_title() == "Library Title" && row.in_library()
        ));
    }

    #[test]
    fn load_select_items_in_table_level_dedupes_matched_chart_and_stale_hash_row() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let chart = chart("Registered Song");
        library_db.upsert_chart_import(&record_for_chart("/songs/registered.bms", &chart)).unwrap();

        use crate::difficulty_table::{FetchedDifficultyTable, FetchedTableEntry};
        let table = FetchedDifficultyTable {
            source_url: "https://example.com/dedupe/".to_string(),
            head_url: "https://example.com/dedupe/header.json".to_string(),
            name: "Dedupe".to_string(),
            symbol: "★".to_string(),
            level_order: vec!["12".to_string()],
            entries: vec![
                FetchedTableEntry {
                    level: "12".to_string(),
                    md5: hash_to_hex(&chart.identity.file_md5),
                    sha256: String::new(),
                    title: "Registered Song".to_string(),
                    artist: String::new(),
                    comment: String::new(),
                    ..FetchedTableEntry::default()
                },
                FetchedTableEntry {
                    level: "12".to_string(),
                    md5: "deadbeef".repeat(4),
                    sha256: String::new(),
                    title: "Registered Song".to_string(),
                    artist: String::new(),
                    comment: String::new(),
                    ..FetchedTableEntry::default()
                },
            ],
            courses: Vec::new(),
            fetched_at: 0,
        };
        library_db.upsert_difficulty_table(&table).unwrap();

        let items = load_select_items_in_table_level(
            &library_db,
            &score_db,
            "https://example.com/dedupe/",
            "12",
            LnPolicySetting::AutoLn,
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        assert!(matches!(
            &items[0],
            SelectItem::Chart(row)
            if row.display_title() == "Registered Song" && row.in_library()
        ));
    }

    #[test]
    fn load_select_items_in_table_level_dedupes_md5_and_sha256_rows_for_same_chart() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let chart = chart("Dual Hash Song");
        library_db.upsert_chart_import(&record_for_chart("/songs/dual.bms", &chart)).unwrap();

        use crate::difficulty_table::{FetchedDifficultyTable, FetchedTableEntry};
        let table = FetchedDifficultyTable {
            source_url: "https://example.com/dual/".to_string(),
            head_url: "https://example.com/dual/header.json".to_string(),
            name: "Dual".to_string(),
            symbol: "★".to_string(),
            level_order: vec!["12".to_string()],
            entries: vec![
                FetchedTableEntry {
                    level: "12".to_string(),
                    md5: hash_to_hex(&chart.identity.file_md5),
                    sha256: String::new(),
                    title: String::new(),
                    artist: String::new(),
                    comment: String::new(),
                    ..FetchedTableEntry::default()
                },
                FetchedTableEntry {
                    level: "12".to_string(),
                    md5: String::new(),
                    sha256: hash_to_hex(&chart.identity.file_sha256),
                    title: String::new(),
                    artist: String::new(),
                    comment: String::new(),
                    ..FetchedTableEntry::default()
                },
            ],
            courses: Vec::new(),
            fetched_at: 0,
        };
        library_db.upsert_difficulty_table(&table).unwrap();

        let items = load_select_items_in_table_level(
            &library_db,
            &score_db,
            "https://example.com/dual/",
            "12",
            LnPolicySetting::AutoLn,
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], SelectItem::Chart(row) if row.in_library()));
    }

    #[test]
    fn load_select_items_in_table_level_dedupes_duplicate_library_chart_ids() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let chart = chart("Duplicate Import Song");
        let chart_id_a = library_db
            .upsert_chart_import(&record_for_chart("/songs/a/track.bms", &chart))
            .unwrap();
        let chart_id_b = library_db
            .upsert_chart_import(&record_for_chart("/songs/b/track.bms", &chart))
            .unwrap();
        assert_ne!(chart_id_a, chart_id_b);

        use crate::difficulty_table::{FetchedDifficultyTable, FetchedTableEntry};
        let table = FetchedDifficultyTable {
            source_url: "https://example.com/dup-import/".to_string(),
            head_url: "https://example.com/dup-import/header.json".to_string(),
            name: "Dup Import".to_string(),
            symbol: "★".to_string(),
            level_order: vec!["12".to_string()],
            entries: vec![
                FetchedTableEntry {
                    level: "12".to_string(),
                    md5: hash_to_hex(&chart.identity.file_md5),
                    sha256: String::new(),
                    title: String::new(),
                    artist: String::new(),
                    comment: String::new(),
                    ..FetchedTableEntry::default()
                },
                FetchedTableEntry {
                    level: "12".to_string(),
                    md5: String::new(),
                    sha256: hash_to_hex(&chart.identity.file_sha256),
                    title: String::new(),
                    artist: String::new(),
                    comment: String::new(),
                    ..FetchedTableEntry::default()
                },
            ],
            courses: Vec::new(),
            fetched_at: 0,
        };
        library_db.upsert_difficulty_table(&table).unwrap();

        let items = load_select_items_in_table_level(
            &library_db,
            &score_db,
            "https://example.com/dup-import/",
            "12",
            LnPolicySetting::AutoLn,
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], SelectItem::Chart(row) if row.in_library()));
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

    #[test]
    fn parse_search_query_round_trips() {
        assert_eq!(parse_search_query("bmz-search:blue"), Some("blue"));
        assert_eq!(parse_search_query("bmz-search:"), None);
        assert_eq!(parse_search_query("/songs/blue"), None);
        assert_eq!(parse_search_query("bmz-table:foo"), None);
    }

    #[test]
    fn search_history_folder_items_formats_each_entry() {
        let history = vec!["alpha".to_string(), "beta".to_string()];
        let items = search_history_folder_items(&history);
        assert_eq!(items.len(), 2);
        match &items[0] {
            SelectItem::Folder { path, name, kind, summary } => {
                assert_eq!(path, "bmz-search:alpha");
                assert_eq!(name, "検索: 'alpha'");
                assert_eq!(*kind, SelectRowKind::SearchFolder);
                assert_eq!(*summary, None);
            }
            other => panic!("expected folder, got {other:?}"),
        }
        match &items[1] {
            SelectItem::Folder { name, .. } => assert_eq!(name, "検索: 'beta'"),
            other => panic!("expected folder, got {other:?}"),
        }

        let english = search_history_folder_items_for_locale(&history, AppLocale::En);
        assert!(matches!(
            &english[0],
            SelectItem::Folder { name, .. } if name == "Search: 'alpha'"
        ));
    }

    #[test]
    fn load_select_items_for_search_returns_chart_rows_with_best_score() {
        let (mut library_db, mut score_db) = open_in_memory_dbs();
        let mut sky = chart("Blue Sky");
        sky.metadata.artist = "Composer A".to_string();
        let mut unrelated = chart("Sunset");
        unrelated.metadata.artist = "Solo".to_string();

        library_db.upsert_chart_import(&record_for_chart("/songs/a.bms", &sky)).unwrap();
        library_db.upsert_chart_import(&record_for_chart("/songs/b.bms", &unrelated)).unwrap();
        score_db.insert_score(&score_for_chart(sky.identity.file_sha256)).unwrap();

        let items =
            load_select_items_for_search(&library_db, &score_db, "blue", LnPolicySetting::AutoLn)
                .unwrap();
        assert_eq!(items.len(), 1);
        let row = match &items[0] {
            SelectItem::Chart(r) => r,
            other => panic!("expected chart row, got {other:?}"),
        };
        assert_eq!(row.display_title(), "Blue Sky");
        assert!(row.best_score.is_some());
    }

    #[test]
    fn load_select_items_for_search_with_filters_hides_removed_song_roots() {
        let (mut library_db, score_db) = open_in_memory_dbs();
        let active = chart("Blue Active");
        let stale = chart("Blue Stale");
        library_db
            .upsert_chart_import(&record_for_chart("/songs/enabled/active.bms", &active))
            .unwrap();
        library_db
            .upsert_chart_import(&record_for_chart("/songs/removed/stale.bms", &stale))
            .unwrap();

        let active_roots = vec!["/songs/enabled".to_string()];
        let items = load_select_items_for_search_for_rule_mode_with_filters(
            &library_db,
            &score_db,
            "blue",
            LnPolicySetting::AutoLn,
            RuleMode::Beatoraja,
            &[],
            Some(&active_roots),
            None,
        )
        .unwrap();

        assert_eq!(items.len(), 1);
        assert!(
            matches!(&items[0], SelectItem::Chart(row) if row.display_title() == "Blue Active")
        );
    }
}
