use super::*;

/// Returns one folder item per registered difficulty table.
pub fn table_folder_items(
    library_db: &LibraryDatabase,
    source_order: &[String],
) -> Result<Vec<SelectItem>> {
    table_folder_items_for_active_sources(library_db, source_order, None)
}

pub fn table_folder_items_for_active_sources(
    library_db: &LibraryDatabase,
    source_order: &[String],
    active_source_urls: Option<&[String]>,
) -> Result<Vec<SelectItem>> {
    let mut tables = library_db.list_difficulty_tables()?;
    if let Some(active_source_urls) = active_source_urls {
        let active: HashSet<&str> = active_source_urls.iter().map(String::as_str).collect();
        tables.retain(|table| active.contains(table.source_url.as_str()));
    }
    if !source_order.is_empty() {
        let order: HashMap<&str, usize> = source_order
            .iter()
            .enumerate()
            .map(|(index, source_url)| (source_url.as_str(), index))
            .collect();
        tables.sort_by_key(|table| {
            order.get(table.source_url.as_str()).copied().unwrap_or(usize::MAX)
        });
    }
    Ok(tables
        .into_iter()
        .map(|t| SelectItem::Folder {
            path: format!("{TABLE_ROOT_PATH}{}", t.source_url),
            name: t.name,
            kind: SelectRowKind::TableFolder,
            summary: None,
        })
        .collect())
}

/// Returns a folder item for the course list root.
pub fn course_root_item() -> SelectItem {
    SelectItem::Folder {
        path: COURSE_ROOT_PATH.to_string(),
        name: "COURSE".to_string(),
        kind: SelectRowKind::TableFolder,
        summary: None,
    }
}

/// Loads manually-imported courses (not from a difficulty table) as `SelectItem::Course` entries.
/// Table-sourced courses appear inside each table's folder via `table_level_folder_items`.
pub fn load_select_items_for_courses(
    library_db: &LibraryDatabase,
    score_db: &ScoreDatabase,
    ln_policy_setting: LnPolicySetting,
    rule_mode: RuleMode,
) -> Result<Vec<SelectItem>> {
    let courses = library_db.list_courses()?;
    Ok(courses
        .into_iter()
        .filter(|stored| !stored.source.starts_with("table:"))
        .map(|stored| {
            build_select_course_row(library_db, score_db, ln_policy_setting, rule_mode, stored)
        })
        .collect())
}

/// Aggregates per-entry chart stats into a `SelectCourseRow`.
pub(super) fn build_select_course_row(
    library_db: &LibraryDatabase,
    score_db: &ScoreDatabase,
    ln_policy_setting: LnPolicySetting,
    rule_mode: RuleMode,
    stored: crate::storage::library_db::StoredCourse,
) -> SelectItem {
    let entry_count = stored.definition.entries.len();
    let resolved_count = stored.definition.entries.iter().filter(|e| e.chart_id.is_some()).count();

    let chart_ids: Vec<i64> = stored.definition.entries.iter().filter_map(|e| e.chart_id).collect();
    let charts = library_db.list_charts_by_ids(&chart_ids).unwrap_or_default();
    let chart_by_id: std::collections::HashMap<i64, &ChartListItem> =
        charts.iter().map(|c| (c.chart_id, c)).collect();

    let entry_previews: Vec<CourseEntryPreview> = stored
        .definition
        .entries
        .iter()
        .map(|entry| match entry.chart_id.and_then(|id| chart_by_id.get(&id).copied()) {
            Some(chart) => CourseEntryPreview {
                title: chart.title.clone(),
                artist: chart.artist.clone(),
                play_level: chart.play_level.clone(),
                difficulty_name: chart.difficulty_name.clone(),
                total_notes: course_chart_total_notes(
                    chart,
                    ln_policy_setting,
                    stored.definition.constraints.ln,
                ),
                resolved: true,
            },
            None => CourseEntryPreview {
                title: entry.title_hint.clone(),
                artist: String::new(),
                play_level: String::new(),
                difficulty_name: String::new(),
                total_notes: 0,
                resolved: false,
            },
        })
        .collect();

    // Sum entries rather than the de-duplicated SQL result so a course that
    // intentionally contains the same chart more than once counts every stage.
    let total_notes: u32 = entry_previews.iter().map(|entry| entry.total_notes).sum();
    let total_length_ms: i64 = charts.iter().map(|c| c.length_ms).sum();
    let min_bpm = charts.iter().map(|c| c.min_bpm as f32).fold(f32::INFINITY, f32::min);
    let max_bpm = charts.iter().map(|c| c.max_bpm as f32).fold(f32::NEG_INFINITY, f32::max);
    let (min_bpm, max_bpm) =
        if min_bpm.is_finite() && max_bpm.is_finite() { (min_bpm, max_bpm) } else { (0.0, 0.0) };

    let category_label = match stored.definition.kind {
        bmz_core::course::CourseKind::Dan => "DAN".to_string(),
        bmz_core::course::CourseKind::Course => "COURSE".to_string(),
    };
    let trophy_names: Vec<String> =
        stored.definition.trophies.iter().map(|t| t.name.clone()).collect();

    let identity = crate::ir::course_payload::course_identity_from_stored(library_db, &stored);
    let best_score = identity.as_ref().and_then(|identity| {
        score_db
            .best_course_score(&identity.course_hash, ln_policy_setting, rule_mode)
            .unwrap_or_else(|error| {
                tracing::warn!(
                    %error,
                    course_id = stored.id,
                    course_hash = %identity.course_hash,
                    rule_mode = rule_mode.as_str(),
                    "failed to load best course score"
                );
                None
            })
    });
    let replay_slots = identity
        .as_ref()
        .map(|identity| {
            score_db
                .course_replay_slot_presence(&identity.course_hash, ln_policy_setting, rule_mode)
                .unwrap_or_else(|error| {
                    tracing::warn!(
                        %error,
                        course_id = stored.id,
                        course_hash = %identity.course_hash,
                        rule_mode = rule_mode.as_str(),
                        "failed to load course_replay_slot_presence"
                    );
                    [false; 4]
                })
        })
        .unwrap_or([false; 4]);
    let achieved_trophy_names = identity
        .as_ref()
        .map(|identity| {
            score_db
                .achieved_trophy_names_for_course(
                    &identity.course_hash,
                    ln_policy_setting,
                    rule_mode,
                )
                .unwrap_or_else(|error| {
                    tracing::warn!(
                        %error,
                        course_id = stored.id,
                        course_hash = %identity.course_hash,
                        rule_mode = rule_mode.as_str(),
                        "failed to load achieved_trophy_names_for_course"
                    );
                    Vec::new()
                })
        })
        .unwrap_or_default();

    SelectItem::Course(SelectCourseRow {
        course_id: stored.id,
        course_hash: identity.as_ref().map(|identity| identity.course_hash.clone()),
        rian_course_hash_v1: identity.as_ref().map(|identity| identity.rian_course_hash_v1.clone()),
        title: stored.definition.title,
        kind: stored.definition.kind,
        constraints: stored.definition.constraints,
        entry_count,
        resolved_count,
        total_notes,
        total_length_ms,
        min_bpm,
        max_bpm,
        category_label,
        trophy_names,
        entry_previews,
        best_score,
        replay_slots,
        achieved_trophy_names,
    })
}

pub(super) fn course_chart_total_notes(
    chart: &ChartListItem,
    setting: LnPolicySetting,
    constraint: CourseLnConstraint,
) -> u32 {
    let course_fallback = match constraint {
        CourseLnConstraint::Default => None,
        CourseLnConstraint::Ln => Some(bmz_chart::model::LongNoteMode::Ln),
        CourseLnConstraint::Cn => Some(bmz_chart::model::LongNoteMode::Cn),
        CourseLnConstraint::Hcn => Some(bmz_chart::model::LongNoteMode::Hcn),
    };
    let policy = course_score_ln_policy(setting, course_fallback, chart.ln_profile);
    chart.scored_total_notes(policy)
}

/// Returns one folder item per level of the difficulty table, ordered by the
/// table's `level_order`, followed by any courses imported from that table.
pub fn table_level_folder_items(
    library_db: &LibraryDatabase,
    score_db: &ScoreDatabase,
    source_url: &str,
    ln_policy_setting: LnPolicySetting,
    rule_mode: RuleMode,
) -> Result<Vec<SelectItem>> {
    let Some(table) =
        library_db.list_difficulty_tables()?.into_iter().find(|t| t.source_url == source_url)
    else {
        return Ok(Vec::new());
    };

    let mut items: Vec<SelectItem> = table
        .level_order
        .iter()
        .map(|level| SelectItem::Folder {
            path: format!("{TABLE_ROOT_PATH}{source_url}{TABLE_LEVEL_SEPARATOR}{level}"),
            name: format!("{}{}", table.symbol, level),
            kind: SelectRowKind::TableFolder,
            summary: None,
        })
        .collect();

    // Append courses that were imported from this table.
    let table_source = format!("table:{source_url}");
    if let Ok(courses) = library_db.list_courses_by_source(&table_source) {
        tracing::info!(source = %table_source, count = courses.len(), "courses found for table");
        for stored in courses {
            items.push(build_select_course_row(
                library_db,
                score_db,
                ln_policy_setting,
                rule_mode,
                stored,
            ));
        }
    }

    Ok(items)
}

/// Loads charts that are stored in the local library and belong to the given
/// difficulty table (identified by `source_url`).  Charts are sorted by the
/// table's `level_order`, then by title within each level.
pub fn load_select_items_in_table(
    library_db: &LibraryDatabase,
    score_db: &ScoreDatabase,
    source_url: &str,
    ln_policy_setting: LnPolicySetting,
) -> Result<Vec<SelectItem>> {
    load_select_items_in_table_for_rule_mode(
        library_db,
        score_db,
        source_url,
        ln_policy_setting,
        RuleMode::Beatoraja,
    )
}

pub fn load_select_items_in_table_for_rule_mode(
    library_db: &LibraryDatabase,
    score_db: &ScoreDatabase,
    source_url: &str,
    ln_policy_setting: LnPolicySetting,
    rule_mode: RuleMode,
) -> Result<Vec<SelectItem>> {
    load_select_items_in_table_filtered(
        library_db,
        score_db,
        source_url,
        None,
        ln_policy_setting,
        rule_mode,
    )
}

/// Loads the charts of a single level of the difficulty table.
pub fn load_select_items_in_table_level(
    library_db: &LibraryDatabase,
    score_db: &ScoreDatabase,
    source_url: &str,
    level: &str,
    ln_policy_setting: LnPolicySetting,
) -> Result<Vec<SelectItem>> {
    load_select_items_in_table_level_for_rule_mode(
        library_db,
        score_db,
        source_url,
        level,
        ln_policy_setting,
        RuleMode::Beatoraja,
    )
}

pub fn load_select_items_in_table_level_for_rule_mode(
    library_db: &LibraryDatabase,
    score_db: &ScoreDatabase,
    source_url: &str,
    level: &str,
    ln_policy_setting: LnPolicySetting,
    rule_mode: RuleMode,
) -> Result<Vec<SelectItem>> {
    load_select_items_in_table_filtered(
        library_db,
        score_db,
        source_url,
        Some(level),
        ln_policy_setting,
        rule_mode,
    )
}

pub(super) fn load_select_items_in_table_filtered(
    library_db: &LibraryDatabase,
    score_db: &ScoreDatabase,
    source_url: &str,
    level_filter: Option<&str>,
    ln_policy_setting: LnPolicySetting,
    rule_mode: RuleMode,
) -> Result<Vec<SelectItem>> {
    // Fetch table metadata for symbol and level ordering.
    let (table_name, symbol, level_order) = library_db
        .list_difficulty_tables()?
        .into_iter()
        .find(|t| t.source_url == source_url)
        .map(|t| (t.name, t.symbol, t.level_order))
        .unwrap_or_default();

    let mut entries =
        library_db.list_table_entries_with_chart_at_level(source_url, level_filter)?;
    entries = dedupe_table_entries(entries);

    // Sort by the table's level_order, then alphabetically by display title.
    let level_rank = |level: &str| -> usize {
        level_order.iter().position(|l| l == level).unwrap_or(usize::MAX)
    };
    entries.sort_by(|a, b| {
        level_rank(&a.level).cmp(&level_rank(&b.level)).then_with(|| {
            entry_display_title(a).to_lowercase().cmp(&entry_display_title(b).to_lowercase())
        })
    });

    // Batch score lookup.
    let keys: Vec<ScoreKey> = entries
        .iter()
        .filter_map(|entry| entry_score_key(entry, ln_policy_setting, rule_mode))
        .collect();
    let mut score_map: HashMap<ScoreKey, BestScoreSummary> = score_db
        .best_scores_for_charts(&keys)?
        .into_iter()
        .map(|s| {
            (ScoreKey::with_options(s.chart_sha256, s.ln_policy, s.double_option, s.rule_mode), s)
        })
        .collect();
    let mut replay_slot_map = replay_slot_map(score_db, &keys)?;
    let chart_ids: Vec<i64> = entries
        .iter()
        .filter_map(|entry| entry.chart.as_ref().map(|chart| chart.chart_id))
        .collect();
    let mut analysis_map = library_db.chart_analysis_summaries_by_chart_ids(&chart_ids)?;

    Ok(entries
        .into_iter()
        .map(|entry| {
            let table_text =
                DifficultyTableText::from_parts(table_name.clone(), &symbol, &entry.level);
            let score_key = entry_score_key(&entry, ln_policy_setting, rule_mode);
            let best_score = score_key.and_then(|key| score_map.remove(&key));
            let replay_slots =
                score_key.and_then(|key| replay_slot_map.remove(&key)).unwrap_or([false; 4]);
            let chart_analysis =
                entry.chart.as_ref().and_then(|chart| analysis_map.remove(&chart.chart_id));
            let has_document = entry.chart.as_ref().is_some_and(|chart| chart.has_document);
            SelectItem::Chart(select_chart_row_from_table_entry(
                entry,
                chart_analysis,
                has_document,
                best_score,
                replay_slots,
                table_text,
            ))
        })
        .collect())
}

pub(super) fn entry_display_title(entry: &TableEntryListItem) -> &str {
    entry
        .chart
        .as_ref()
        .map(|chart| chart.title.as_str())
        .filter(|title| !title.is_empty())
        .unwrap_or(entry.title.as_str())
}

/// Collapses duplicate difficulty-table rows that refer to the same local chart.
///
/// Tables often contain redundant hash rows for the same song.  When also showing
/// unmatched entries we drop duplicate matched charts and stale rows that no longer
/// resolve to a unique missing song.
pub(super) fn dedupe_table_entries(entries: Vec<TableEntryListItem>) -> Vec<TableEntryListItem> {
    let mut claimed_md5_by_level: HashMap<String, HashSet<String>> = HashMap::new();
    let mut claimed_sha256_by_level: HashMap<String, HashSet<String>> = HashMap::new();
    let mut claimed_titles_by_level: HashMap<String, HashSet<String>> = HashMap::new();

    for entry in &entries {
        let Some(chart) = &entry.chart else {
            continue;
        };
        let md5s = claimed_md5_by_level.entry(entry.level.clone()).or_default();
        let sha256s = claimed_sha256_by_level.entry(entry.level.clone()).or_default();
        let titles = claimed_titles_by_level.entry(entry.level.clone()).or_default();

        if entry.md5.len() >= 24 {
            md5s.insert(entry.md5.clone());
        }
        if entry.sha256.len() >= 24 {
            sha256s.insert(entry.sha256.clone());
        }
        md5s.insert(hash_to_hex(&chart.md5));
        sha256s.insert(hash_to_hex(&chart.sha256));
        if !entry.title.is_empty() {
            titles.insert(entry.title.to_lowercase());
        }
        if !chart.title.is_empty() {
            titles.insert(chart.title.to_lowercase());
        }
    }

    let mut seen_chart_sha256_by_level: HashSet<(String, [u8; 32])> = HashSet::new();
    let mut seen_unmatched_keys: HashSet<(String, String, String)> = HashSet::new();
    let mut result = Vec::with_capacity(entries.len());

    for entry in entries {
        if let Some(chart) = &entry.chart {
            let identity = (entry.level.clone(), chart.sha256);
            if !seen_chart_sha256_by_level.insert(identity) {
                continue;
            }
            result.push(entry);
            continue;
        }

        if entry_claimed_by_matched_entry(&entry, &claimed_md5_by_level, &claimed_sha256_by_level) {
            continue;
        }
        if !entry.title.is_empty()
            && claimed_titles_by_level
                .get(&entry.level)
                .is_some_and(|titles| titles.contains(&entry.title.to_lowercase()))
        {
            continue;
        }

        let unmatched_key = (entry.level.clone(), entry.md5.clone(), entry.sha256.clone());
        if !seen_unmatched_keys.insert(unmatched_key) {
            continue;
        }

        result.push(entry);
    }

    result
}

pub(super) fn entry_claimed_by_matched_entry(
    entry: &TableEntryListItem,
    claimed_md5_by_level: &HashMap<String, HashSet<String>>,
    claimed_sha256_by_level: &HashMap<String, HashSet<String>>,
) -> bool {
    if entry.md5.len() >= 24
        && claimed_md5_by_level.get(&entry.level).is_some_and(|hashes| hashes.contains(&entry.md5))
    {
        return true;
    }
    entry.sha256.len() >= 24
        && claimed_sha256_by_level
            .get(&entry.level)
            .is_some_and(|hashes| hashes.contains(&entry.sha256))
}

pub(super) fn entry_score_sha256(entry: &TableEntryListItem) -> Option<[u8; 32]> {
    if let Some(chart) = &entry.chart {
        return Some(chart.sha256);
    }
    if entry.sha256.len() >= 48 {
        return hex_to_hash::<32>(&entry.sha256).ok();
    }
    None
}

pub(super) fn entry_score_key(
    entry: &TableEntryListItem,
    ln_policy_setting: LnPolicySetting,
    rule_mode: RuleMode,
) -> Option<ScoreKey> {
    if let Some(chart) = &entry.chart {
        return Some(score_key_for_chart(chart, ln_policy_setting, rule_mode));
    }
    entry_score_sha256(entry)
        .map(|sha256| ScoreKey::new(sha256, LnScorePolicy::ForceLn).with_rule_mode(rule_mode))
}

pub(super) fn select_chart_row_from_table_entry(
    entry: TableEntryListItem,
    chart_analysis: Option<ChartAnalysisSummary>,
    has_document: bool,
    best_score: Option<BestScoreSummary>,
    replay_slots: [bool; 4],
    table_text: DifficultyTableText,
) -> SelectChartRow {
    let entry_sha256 = entry_score_sha256(&entry);
    let table_level = table_text.table_level.clone();
    SelectChartRow {
        chart: entry.chart,
        chart_analysis,
        has_document,
        fallback_title: entry.title,
        fallback_artist: entry.artist,
        entry_sha256,
        download_metadata: ChartDownloadMetadata {
            md5: entry.md5,
            sha256: entry.sha256,
            url: entry.url,
            append_url: entry.append_url,
            ipfs: entry.ipfs,
            append_ipfs: entry.append_ipfs,
        },
        best_score,
        replay_slots,
        favorite_chart: false,
        favorite_song: false,
        table_level,
        table_text,
    }
}

pub(super) fn hex_to_hash<const N: usize>(hex: &str) -> Result<[u8; N]> {
    crate::storage::common::hex_to_hash(hex).map_err(Into::into)
}
