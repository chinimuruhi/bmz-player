use super::*;

pub(super) fn initial_folder_stack(
    _app_config: &crate::config::app_config::AppConfig,
) -> Vec<String> {
    // 有効な曲フォルダが 1 つだけでも、設定フォルダ等を含む選曲ルートから始める。
    Vec::new()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectModeFilter {
    All,
    K7,
    K14,
    K9,
    K5,
    K10,
}

impl SelectModeFilter {
    pub(super) const ORDER: [Self; 6] =
        [Self::All, Self::K7, Self::K14, Self::K9, Self::K5, Self::K10];

    pub(super) fn next(self) -> Self {
        cycle_enum(Self::ORDER, self, 1)
    }

    pub(super) fn previous(self) -> Self {
        cycle_enum(Self::ORDER, self, -1)
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::K7 => "7K",
            Self::K14 => "14K",
            Self::K9 => "9K",
            Self::K5 => "5K",
            Self::K10 => "10K",
        }
    }

    pub(super) fn key_mode(self) -> Option<KeyMode> {
        match self {
            Self::All => None,
            Self::K7 => Some(KeyMode::K7),
            Self::K14 => Some(KeyMode::K14),
            Self::K9 => Some(KeyMode::K9),
            Self::K5 => Some(KeyMode::K5),
            Self::K10 => Some(KeyMode::K10),
        }
    }

    /// `as_str()` の逆変換。未知の値は `ALL` へフォールバックする。
    pub(super) fn from_str_or_default(value: &str) -> Self {
        Self::ORDER.into_iter().find(|mode| mode.as_str() == value).unwrap_or(Self::All)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectSort {
    Title,
    Artist,
    Bpm,
    Length,
    Level,
    Clear,
    Score,
    Bp,
}

impl SelectSort {
    pub(super) const ORDER: [Self; 8] = [
        Self::Title,
        Self::Artist,
        Self::Bpm,
        Self::Length,
        Self::Level,
        Self::Clear,
        Self::Score,
        Self::Bp,
    ];

    pub(super) fn next(self) -> Self {
        cycle_enum(Self::ORDER, self, 1)
    }

    pub(super) fn previous(self) -> Self {
        cycle_enum(Self::ORDER, self, -1)
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Title => "TITLE",
            Self::Artist => "ARTIST",
            Self::Bpm => "BPM",
            Self::Length => "LENGTH",
            Self::Level => "LEVEL",
            Self::Clear => "CLEAR",
            Self::Score => "SCORE",
            Self::Bp => "BPCOUNT",
        }
    }

    /// `as_str()` の逆変換。未知の値は `TITLE` へフォールバックする。
    pub(super) fn from_str_or_default(value: &str) -> Self {
        Self::ORDER.into_iter().find(|sort| sort.as_str() == value).unwrap_or(Self::Title)
    }
}

pub(super) fn cycle_enum<T: Copy + PartialEq, const N: usize>(
    values: [T; N],
    current: T,
    direction: i32,
) -> T {
    let index = values.iter().position(|value| *value == current).unwrap_or(0);
    let len = values.len();
    if direction >= 0 { values[(index + 1) % len] } else { values[(index + len - 1) % len] }
}

pub(super) fn enabled_root_paths(app_config: &crate::config::app_config::AppConfig) -> Vec<String> {
    app_config.songs.roots.iter().filter(|p| p.enabled).map(|p| p.path.clone()).collect()
}

pub(super) fn table_source_order(app_config: &crate::config::app_config::AppConfig) -> Vec<String> {
    app_config
        .tables
        .sources
        .iter()
        .filter(|source| source.enabled)
        .map(|source| source.url.clone())
        .collect()
}

/// 選曲リストを構築し、mode filter / sort を適用して返す。
///
/// mode filter は beatoraja `BarManager` 準拠で、指定モードがこの一覧の
/// チャートを「全て」消してしまう場合のみ、チャートが残るモードへ前方向に
/// 自動送りする。実際に適用したモードを items と共に返すので、呼び出し側で
/// 永続化 / 表示状態を更新できる。
pub(super) fn load_items_for_stack(
    boot: &crate::bootstrap::BootstrappedApp,
    stack: &[String],
    search_history: &[String],
    mode_filter: SelectModeFilter,
    sort: SelectSort,
) -> (Vec<SelectItem>, SelectModeFilter) {
    let mut items = build_select_items_for_stack(boot, stack, search_history);
    let resolved = resolve_non_empty_mode_filter(&items, mode_filter);
    apply_select_mode_filter(&mut items, resolved);
    apply_select_sort(&mut items, sort);
    if let Err(error) = apply_collection_flags(&boot.library_db, &boot.collection_db, &mut items) {
        tracing::error!(%error, "failed to apply collection flags to select items");
    }
    if boot.profile_config.select.random_select
        && let Some(random_item) = random_select_item_from_items(&items)
    {
        items.insert(0, random_item);
    }
    (items, resolved)
}

pub(super) fn build_select_items_for_stack(
    boot: &crate::bootstrap::BootstrappedApp,
    stack: &[String],
    search_history: &[String],
) -> Vec<SelectItem> {
    let active_song_roots = enabled_root_paths(&boot.app_config);
    let mut active_table_sources = table_source_order(&boot.app_config);
    if let Some(identity) = RianTableIdentity::from_ir_config(&boot.profile_config.ir) {
        match active_rian_table_source_urls(&boot.library_db, &identity) {
            Ok(sources) => active_table_sources.extend(sources),
            Err(error) => tracing::warn!(%error, "failed to load cached rianIR table sources"),
        }
    }
    match stack.last() {
        Some(path) if path.starts_with(crate::screens::settings_model::CONFIG_ROOT_PATH) => {
            load_settings_items_for_locale(path, boot.profile_config.ui.locale())
        }
        Some(path) if path == COURSE_ROOT_PATH => {
            match load_select_items_for_courses(
                &boot.library_db,
                &boot.score_db,
                boot.profile_config.play.ln_mode_policy,
                boot.profile_config.play.rule_mode,
            ) {
                Ok(items) => items,
                Err(error) => {
                    tracing::error!(%error, "failed to load course list");
                    Vec::new()
                }
            }
        }
        Some(path) if path == FAVORITE_ROOT_PATH => {
            match favorite_root_items(&boot.collection_db) {
                Ok(items) => items,
                Err(error) => {
                    tracing::error!(%error, "failed to load favorite root items");
                    Vec::new()
                }
            }
        }
        Some(path) if path == FAVORITE_CHART_PATH => {
            match load_select_items_for_favorite_charts(
                &boot.library_db,
                &boot.score_db,
                &boot.collection_db,
                boot.profile_config.play.ln_mode_policy,
                boot.profile_config.play.rule_mode,
                &active_table_sources,
                Some(&active_song_roots),
                Some(&active_table_sources),
            ) {
                Ok(items) => items,
                Err(error) => {
                    tracing::error!(%error, "failed to load favorite chart items");
                    Vec::new()
                }
            }
        }
        Some(path) if path == FAVORITE_SONG_PATH => {
            match load_select_items_for_favorite_songs(&boot.collection_db) {
                Ok(items) => items,
                Err(error) => {
                    tracing::error!(%error, "failed to load favorite song folders");
                    Vec::new()
                }
            }
        }
        Some(path) if parse_favorite_song_detail_path(path).is_some() => {
            let representative_sha256 = parse_favorite_song_detail_path(path).unwrap();
            match load_select_items_for_favorite_song(
                &boot.library_db,
                &boot.score_db,
                &boot.collection_db,
                representative_sha256,
                boot.profile_config.play.ln_mode_policy,
                boot.profile_config.play.rule_mode,
                &active_table_sources,
                Some(&active_song_roots),
                Some(&active_table_sources),
            ) {
                Ok(items) => items,
                Err(error) => {
                    tracing::error!(%error, "failed to load favorite song items");
                    Vec::new()
                }
            }
        }
        Some(path) if path.starts_with(SEARCH_PATH_PREFIX) => match parse_search_query(path) {
            Some(query) => {
                match load_select_items_for_search_for_rule_mode_with_filters(
                    &boot.library_db,
                    &boot.score_db,
                    query,
                    boot.profile_config.play.ln_mode_policy,
                    boot.profile_config.play.rule_mode,
                    &active_table_sources,
                    Some(&active_song_roots),
                    Some(&active_table_sources),
                ) {
                    Ok(items) => items,
                    Err(error) => {
                        tracing::error!(%error, query, "failed to load search results");
                        Vec::new()
                    }
                }
            }
            None => Vec::new(),
        },
        Some(path) if parse_same_folder_path(path).is_some() => {
            let folder = parse_same_folder_path(path).unwrap();
            match load_select_items_in_folder_for_rule_mode_with_filters(
                &boot.library_db,
                &boot.score_db,
                folder,
                boot.profile_config.play.ln_mode_policy,
                boot.profile_config.play.rule_mode,
                &active_table_sources,
                Some(&active_song_roots),
                Some(&active_table_sources),
            ) {
                Ok(items) => items,
                Err(error) => {
                    tracing::error!(%error, "failed to load same-folder items");
                    Vec::new()
                }
            }
        }
        Some(path) if path.starts_with(TABLE_ROOT_PATH) => match parse_table_path(path) {
            Some(TablePath::Root) => {
                match table_folder_items_for_active_sources(
                    &boot.library_db,
                    &active_table_sources,
                    Some(&active_table_sources),
                ) {
                    Ok(items) => items,
                    Err(error) => {
                        tracing::error!(%error, "failed to load difficulty table list");
                        Vec::new()
                    }
                }
            }
            Some(TablePath::Table { source_url }) => {
                if !active_table_sources.iter().any(|url| url == source_url) {
                    return Vec::new();
                }
                match table_level_folder_items(
                    &boot.library_db,
                    &boot.score_db,
                    source_url,
                    boot.profile_config.play.ln_mode_policy,
                    boot.profile_config.play.rule_mode,
                ) {
                    Ok(items) => items,
                    Err(error) => {
                        tracing::error!(%error, "failed to load difficulty table levels");
                        Vec::new()
                    }
                }
            }
            Some(TablePath::Level { source_url, level }) => {
                if !active_table_sources.iter().any(|url| url == source_url) {
                    return Vec::new();
                }
                match load_select_items_in_table_level_for_rule_mode(
                    &boot.library_db,
                    &boot.score_db,
                    source_url,
                    level,
                    boot.profile_config.play.ln_mode_policy,
                    boot.profile_config.play.rule_mode,
                ) {
                    Ok(items) => items,
                    Err(error) => {
                        tracing::error!(%error, "failed to load difficulty table charts");
                        Vec::new()
                    }
                }
            }
            None => Vec::new(),
        },
        Some(folder) => {
            match load_select_items_in_folder_for_rule_mode_with_filters(
                &boot.library_db,
                &boot.score_db,
                folder,
                boot.profile_config.play.ln_mode_policy,
                boot.profile_config.play.rule_mode,
                &active_table_sources,
                Some(&active_song_roots),
                Some(&active_table_sources),
            ) {
                Ok(items) => items,
                Err(error) => {
                    tracing::error!(%error, "failed to load select items");
                    Vec::new()
                }
            }
        }
        None => {
            // ルートには曲フォルダに続けて、コースフォルダ・各難易度表フォルダを並べる。
            // 難易度表由来のコースは各テーブルフォルダ内に表示されるため、
            // 手動インポート分（source が "table:..." でないもの）がある場合のみ COURSE フォルダを表示する。
            let mut items = root_folder_items(&active_song_roots);
            match favorite_root_items(&boot.collection_db) {
                Ok(favorites) if !favorites.is_empty() => items.push(favorite_root_item()),
                Ok(_) => {}
                Err(error) => tracing::error!(%error, "failed to check favorite root"),
            }
            match boot.library_db.list_courses() {
                Ok(courses) if courses.iter().any(|c| !c.source.starts_with("table:")) => {
                    items.push(course_root_item());
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::error!(%error, "failed to check course list for root");
                }
            }
            match table_folder_items_for_active_sources(
                &boot.library_db,
                &active_table_sources,
                Some(&active_table_sources),
            ) {
                Ok(tables) => items.extend(tables),
                Err(error) => {
                    tracing::error!(%error, "failed to load difficulty table folders");
                }
            }
            items.push(settings_root_item_for_locale(boot.profile_config.ui.locale()));
            if !search_history.is_empty() {
                items.extend(search_history_folder_items_for_locale(
                    search_history,
                    boot.profile_config.ui.locale(),
                ));
            }
            items
        }
    }
}

/// beatoraja `BarManager` 準拠の mode filter 自動送り。
///
/// 指定モードがこの一覧の全 bar を消す（= 残るのが mismatch のチャート行だけ）
/// 場合のみ、チャートが残るモードへ前方向に送る。フォルダ等チャート以外の行が
/// 1 つでも残る、または ALL のように絞り込まないモードの場合は据え置く。
pub(super) fn resolve_non_empty_mode_filter(
    items: &[SelectItem],
    start: SelectModeFilter,
) -> SelectModeFilter {
    let mut candidate = start;
    for _ in 0..SelectModeFilter::ORDER.len() {
        if !mode_filter_removes_everything(items, candidate) {
            return candidate;
        }
        candidate = candidate.next();
    }
    start
}

/// `apply_select_mode_filter` を適用すると一覧が空になるか。
pub(super) fn mode_filter_removes_everything(
    items: &[SelectItem],
    filter: SelectModeFilter,
) -> bool {
    if items.is_empty() {
        return false;
    }
    let Some(key_mode) = filter.key_mode() else {
        // ALL は絞り込まないので空にはならない。
        return false;
    };
    items.iter().all(|item| match item {
        SelectItem::Chart(row) => !row
            .chart
            .as_ref()
            .and_then(|chart| KeyMode::from_str_opt(&chart.mode))
            .is_some_and(|mode| mode == key_mode),
        // フォルダ・コース等は除去対象外なので、残れば「全除去」ではない。
        _ => false,
    })
}

pub(super) fn apply_select_mode_filter(items: &mut Vec<SelectItem>, filter: SelectModeFilter) {
    let Some(key_mode) = filter.key_mode() else {
        return;
    };
    items.retain(|item| match item {
        SelectItem::Chart(row) => row
            .chart
            .as_ref()
            .and_then(|chart| KeyMode::from_str_opt(&chart.mode))
            .is_some_and(|mode| mode == key_mode),
        _ => true,
    });
}

pub(super) fn apply_select_sort(items: &mut [SelectItem], sort: SelectSort) {
    items.sort_by(|a, b| match (a, b) {
        (SelectItem::Chart(a), SelectItem::Chart(b)) => compare_select_chart_rows(a, b, sort),
        _ => std::cmp::Ordering::Equal,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SelectItemKey {
    Folder(String),
    ChartId(i64),
    ChartSha256([u8; 32]),
    ChartFallback { title: String, artist: String, table_level: String, table_full: String },
    Course(i64),
    Executable(String),
    Config(SettingsEntryId),
    KeyBinding { key_mode: KeyMode, target: KeyBindingTarget },
    SettingsBack,
    SettingsClose,
    AdvancedSettings,
}

pub(super) fn select_item_key(item: &SelectItem) -> SelectItemKey {
    match item {
        SelectItem::Folder { path, .. } => SelectItemKey::Folder(path.clone()),
        SelectItem::Chart(row) => row
            .chart
            .as_ref()
            .map(|chart| SelectItemKey::ChartId(chart.chart_id))
            .or_else(|| row.score_sha256().map(SelectItemKey::ChartSha256))
            .unwrap_or_else(|| SelectItemKey::ChartFallback {
                title: row.display_title().to_string(),
                artist: row.display_artist().to_string(),
                table_level: row.table_level.clone(),
                table_full: row.table_text.table_full.clone(),
            }),
        SelectItem::Course(row) => SelectItemKey::Course(row.course_id),
        SelectItem::Executable(row) => SelectItemKey::Executable(row.title.clone()),
        SelectItem::Config(row) => SelectItemKey::Config(row.entry_id),
        SelectItem::KeyBinding(row) => {
            SelectItemKey::KeyBinding { key_mode: row.key_mode, target: row.target }
        }
        SelectItem::SettingsBack => SelectItemKey::SettingsBack,
        SelectItem::SettingsClose => SelectItemKey::SettingsClose,
        SelectItem::AdvancedSettings => SelectItemKey::AdvancedSettings,
    }
}

pub(super) fn favorite_hints_for_row(
    row: &crate::screens::select_model::SelectChartRow,
) -> FavoriteHints {
    let folder = row.chart.as_ref().map(|chart| chart.folder_path.clone()).unwrap_or_default();
    FavoriteHints {
        title: row.display_title().to_string(),
        artist: row.display_artist().to_string(),
        folder,
        chart_path: String::new(),
    }
}

pub(super) fn restored_select_index(
    items: &[SelectItem],
    previous_selected_key: Option<&SelectItemKey>,
    previous_index: usize,
) -> usize {
    previous_selected_key
        .and_then(|key| items.iter().position(|item| select_item_key(item) == *key))
        .unwrap_or_else(|| previous_index.min(items.len().saturating_sub(1)))
}

pub(super) fn compare_select_chart_rows(
    a: &crate::screens::select_model::SelectChartRow,
    b: &crate::screens::select_model::SelectChartRow,
    sort: SelectSort,
) -> std::cmp::Ordering {
    let ordering = match sort {
        SelectSort::Title => compare_case_insensitive(a.display_title(), b.display_title()),
        SelectSort::Artist => compare_case_insensitive(a.display_artist(), b.display_artist()),
        SelectSort::Bpm => chart_initial_bpm(a).total_cmp(&chart_initial_bpm(b)),
        SelectSort::Length => chart_length_ms(a).cmp(&chart_length_ms(b)),
        SelectSort::Level => compare_play_level(a, b),
        SelectSort::Clear => clear_rank(a).cmp(&clear_rank(b)),
        SelectSort::Score => ex_score(a).cmp(&ex_score(b)),
        SelectSort::Bp => bp(a).cmp(&bp(b)),
    };
    ordering.then_with(|| compare_case_insensitive(a.display_title(), b.display_title()))
}

pub(super) fn compare_case_insensitive(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

pub(super) fn chart_initial_bpm(row: &crate::screens::select_model::SelectChartRow) -> f64 {
    row.chart.as_ref().map(|chart| chart.initial_bpm).unwrap_or(0.0)
}

pub(super) fn chart_length_ms(row: &crate::screens::select_model::SelectChartRow) -> i64 {
    row.chart.as_ref().map(|chart| chart.length_ms).unwrap_or(0)
}

pub(super) fn compare_play_level(
    a: &crate::screens::select_model::SelectChartRow,
    b: &crate::screens::select_model::SelectChartRow,
) -> std::cmp::Ordering {
    play_level_number(a)
        .total_cmp(&play_level_number(b))
        .then_with(|| compare_case_insensitive(a.display_title(), b.display_title()))
}

pub(super) fn play_level_number(row: &crate::screens::select_model::SelectChartRow) -> f64 {
    row.chart.as_ref().and_then(|chart| chart.play_level.parse::<f64>().ok()).unwrap_or(0.0)
}

pub(super) fn clear_rank(row: &crate::screens::select_model::SelectChartRow) -> i8 {
    if !row.in_library() {
        // 難易度表にあるがローカル未所持。NoPlay よりさらに下位へ並べる。
        return -1;
    }
    // 所持済み: NoPlay / 未記録 = 0、Failed=1 .. Max=10。
    ClearType::rank_from_label(
        row.best_score.as_ref().map(|score| score.clear_type.as_str()).unwrap_or_default(),
    ) as i8
}

pub(super) fn ex_score(row: &crate::screens::select_model::SelectChartRow) -> u32 {
    row.best_score.as_ref().map(|score| score.ex_score).unwrap_or(0)
}

pub(super) fn bp(row: &crate::screens::select_model::SelectChartRow) -> u32 {
    row.best_score.as_ref().map(|score| score.bp).unwrap_or(u32::MAX)
}

pub(super) fn cycle_gauge_option(current: GaugeTypeConfig) -> GaugeTypeConfig {
    match current {
        GaugeTypeConfig::AssistEasy => GaugeTypeConfig::Easy,
        GaugeTypeConfig::Easy => GaugeTypeConfig::Normal,
        GaugeTypeConfig::Normal => GaugeTypeConfig::Hard,
        GaugeTypeConfig::Hard => GaugeTypeConfig::ExHard,
        GaugeTypeConfig::ExHard | GaugeTypeConfig::AutoShift => GaugeTypeConfig::Hazard,
        GaugeTypeConfig::Hazard => GaugeTypeConfig::AssistEasy,
    }
}

pub(super) fn cycle_gauge_option_prev(current: GaugeTypeConfig) -> GaugeTypeConfig {
    cycle_gauge_option_with_direction(current, -1)
}

pub(super) fn cycle_gauge_option_with_direction(
    current: GaugeTypeConfig,
    direction: i32,
) -> GaugeTypeConfig {
    const VALUES: [GaugeTypeConfig; 6] = [
        GaugeTypeConfig::AssistEasy,
        GaugeTypeConfig::Easy,
        GaugeTypeConfig::Normal,
        GaugeTypeConfig::Hard,
        GaugeTypeConfig::ExHard,
        GaugeTypeConfig::Hazard,
    ];
    cycle_enum(VALUES, normalize_gauge_option(current), direction)
}

pub(super) fn normalize_gauge_option(current: GaugeTypeConfig) -> GaugeTypeConfig {
    match current {
        GaugeTypeConfig::AutoShift => GaugeTypeConfig::ExHard,
        _ => current,
    }
}

pub(super) fn gauge_option_as_str(gauge: GaugeTypeConfig) -> &'static str {
    match gauge {
        GaugeTypeConfig::AssistEasy => "A-EASY",
        GaugeTypeConfig::Easy => "EASY",
        GaugeTypeConfig::Normal => "NORMAL",
        GaugeTypeConfig::Hard => "HARD",
        GaugeTypeConfig::ExHard => "EX-HARD",
        GaugeTypeConfig::AutoShift => "EX-HARD",
        GaugeTypeConfig::Hazard => "HAZARD",
    }
}

pub(super) fn cycle_gauge_auto_shift_option(current: GaugeAutoShiftConfig) -> GaugeAutoShiftConfig {
    match current {
        GaugeAutoShiftConfig::Off => GaugeAutoShiftConfig::Continue,
        GaugeAutoShiftConfig::Continue => GaugeAutoShiftConfig::HardToGroove,
        GaugeAutoShiftConfig::HardToGroove => GaugeAutoShiftConfig::BestClear,
        GaugeAutoShiftConfig::BestClear => GaugeAutoShiftConfig::SelectToUnder,
        GaugeAutoShiftConfig::SelectToUnder => GaugeAutoShiftConfig::Off,
    }
}

pub(super) fn cycle_gauge_auto_shift_option_with_direction(
    current: GaugeAutoShiftConfig,
    direction: i32,
) -> GaugeAutoShiftConfig {
    const VALUES: [GaugeAutoShiftConfig; 5] = [
        GaugeAutoShiftConfig::Off,
        GaugeAutoShiftConfig::Continue,
        GaugeAutoShiftConfig::HardToGroove,
        GaugeAutoShiftConfig::BestClear,
        GaugeAutoShiftConfig::SelectToUnder,
    ];
    cycle_enum(VALUES, current, direction)
}

pub(super) fn gauge_auto_shift_as_str(mode: GaugeAutoShiftConfig) -> &'static str {
    match mode {
        GaugeAutoShiftConfig::Off => "OFF",
        GaugeAutoShiftConfig::Continue => "CONTINUE",
        GaugeAutoShiftConfig::HardToGroove => "HARD TO GROOVE",
        GaugeAutoShiftConfig::BestClear => "BEST CLEAR",
        GaugeAutoShiftConfig::SelectToUnder => "SELECT TO UNDER",
    }
}

pub(super) fn cycle_bottom_shiftable_gauge_with_direction(
    current: BottomShiftableGaugeConfig,
    direction: i32,
) -> BottomShiftableGaugeConfig {
    const VALUES: [BottomShiftableGaugeConfig; 3] = [
        BottomShiftableGaugeConfig::AssistEasy,
        BottomShiftableGaugeConfig::Easy,
        BottomShiftableGaugeConfig::Normal,
    ];
    cycle_enum(VALUES, current, direction)
}

pub(super) fn cycle_judge_algorithm_with_direction(
    current: JudgeAlgorithmConfig,
    direction: i32,
) -> JudgeAlgorithmConfig {
    cycle_enum(JudgeAlgorithmConfig::ORDER, current, direction)
}

pub(super) fn bottom_shiftable_gauge_as_str(gauge: BottomShiftableGaugeConfig) -> &'static str {
    match gauge {
        BottomShiftableGaugeConfig::AssistEasy => "A-EASY",
        BottomShiftableGaugeConfig::Easy => "EASY",
        BottomShiftableGaugeConfig::Normal => "NORMAL",
    }
}

pub(super) fn bga_mode_as_str(bga: BgaModeConfig) -> &'static str {
    match bga {
        BgaModeConfig::On => "ON",
        BgaModeConfig::Auto => "AUTO",
        BgaModeConfig::Off => "OFF",
    }
}

pub(super) fn volume_f32_to_unit(value: f32) -> u32 {
    (value.clamp(0.0, 1.0) * 100.0).round() as u32
}

pub(super) fn cycle_arrange_option_with_direction(
    current: ArrangeOption,
    direction: i32,
) -> ArrangeOption {
    cycle_enum(ArrangeOption::VALUES, current, direction)
}

pub(super) fn cycle_double_option_with_direction(
    current: DoubleOption,
    direction: i32,
) -> DoubleOption {
    const VALUES: [DoubleOption; 4] = [
        DoubleOption::Off,
        DoubleOption::Flip,
        DoubleOption::Battle,
        DoubleOption::BattleAutoScratch,
    ];
    cycle_enum(VALUES, current, direction)
}

pub(super) fn cycle_hs_fix_option_with_direction(
    current: HsFixOption,
    direction: i32,
) -> HsFixOption {
    const VALUES: [HsFixOption; 5] = [
        HsFixOption::Off,
        HsFixOption::StartBpm,
        HsFixOption::MaxBpm,
        HsFixOption::MainBpm,
        HsFixOption::MinBpm,
    ];
    cycle_enum(VALUES, current, direction)
}

pub(super) fn cycle_bga_option(current: BgaModeConfig) -> BgaModeConfig {
    match current {
        BgaModeConfig::On => BgaModeConfig::Auto,
        BgaModeConfig::Auto => BgaModeConfig::Off,
        BgaModeConfig::Off => BgaModeConfig::On,
    }
}

pub(super) fn cycle_result_gauge_graph_type(current: i32) -> i32 {
    if (GaugeType::AssistEasy as i32..=GaugeType::Hazard as i32).contains(&current) {
        (current + 1).rem_euclid(6)
    } else {
        (current - 5).rem_euclid(3) + 6
    }
}

pub(super) fn toggled_select_sudden(current: LaneEffectConfig) -> LaneEffectConfig {
    match current {
        LaneEffectConfig::Off => LaneEffectConfig::Sudden,
        LaneEffectConfig::Hidden => LaneEffectConfig::HiddenSudden,
        LaneEffectConfig::Sudden => LaneEffectConfig::Off,
        LaneEffectConfig::HiddenSudden => LaneEffectConfig::Hidden,
    }
}

pub(super) fn toggled_select_hidden(current: LaneEffectConfig) -> LaneEffectConfig {
    match current {
        LaneEffectConfig::Off => LaneEffectConfig::Hidden,
        LaneEffectConfig::Hidden => LaneEffectConfig::Off,
        LaneEffectConfig::Sudden => LaneEffectConfig::HiddenSudden,
        LaneEffectConfig::HiddenSudden => LaneEffectConfig::Sudden,
    }
}

pub(super) fn select_chart_distribution(
    distribution: &[ChartDistributionSecond],
) -> Vec<SelectChartDistributionSecond> {
    distribution
        .iter()
        .map(|second| SelectChartDistributionSecond {
            scratch_long_heads: second.scratch_long_heads,
            scratch_long_bodies: second.scratch_long_bodies,
            scratch_taps: second.scratch_taps,
            key_long_heads: second.key_long_heads,
            key_long_bodies: second.key_long_bodies,
            key_taps: second.key_taps,
            mines: second.mines,
        })
        .collect()
}

pub(super) fn select_bpm_graph_segments(
    speed_changes: &[crate::storage::library_db::ChartSpeedChange],
    length_ms: i64,
) -> Vec<bmz_render::chart_graph::BpmGraphSegment> {
    let duration_ms = length_ms.max(1) as f32;
    let mut segments = Vec::new();
    for (index, change) in speed_changes.iter().enumerate() {
        let start_ms = change.time_ms.max(0) as f32;
        let end_ms = speed_changes
            .get(index + 1)
            .map(|next| next.time_ms.max(change.time_ms) as f32)
            .unwrap_or(duration_ms)
            .min(duration_ms);
        if end_ms <= start_ms {
            continue;
        }
        segments.push(bmz_render::chart_graph::BpmGraphSegment {
            start_ratio: (start_ms / duration_ms).clamp(0.0, 1.0),
            end_ratio: (end_ms / duration_ms).clamp(0.0, 1.0),
            bpm: change.speed.max(0.0) as f32,
            is_stop: change.speed == 0.0,
        });
    }
    segments
}

pub(super) fn select_visible_item_indices(
    item_len: usize,
    selected_index: usize,
    visible_limit: usize,
) -> Vec<usize> {
    if item_len == 0 || visible_limit == 0 {
        return Vec::new();
    }

    let row_count = visible_limit;
    let selected_index = selected_index.min(item_len - 1);
    let half_window = row_count / 2;
    let start = (selected_index + item_len - (half_window % item_len)) % item_len;

    (0..row_count).map(|offset| (start + offset) % item_len).collect()
}

pub(super) fn select_snapshot_rows(
    items: &[SelectItem],
    selected_index: usize,
    visible_limit: usize,
    profile: &ProfileConfig,
    key_config_edit: Option<&KeyConfigEditSession>,
    chart_distributions: &HashMap<i64, Vec<ChartDistributionSecond>>,
) -> Vec<SelectRowSnapshot> {
    let visible_indices = select_visible_item_indices(items.len(), selected_index, visible_limit);
    if visible_indices.is_empty() {
        return Vec::new();
    }

    visible_indices
        .into_iter()
        .map(|index| {
            let item = &items[index];
            match item {
                SelectItem::Folder { name, kind, summary, .. } => SelectRowSnapshot {
                    index: index as u32,
                    title: name.clone(),
                    subtitle: String::new(),
                    artist: String::new(),
                    genre: String::new(),
                    difficulty_name: String::new(),
                    play_level: String::new(),
                    table_level: String::new(),
                    table_text_primary: String::new(),
                    table_text_secondary: String::new(),
                    table_text_fallback: String::new(),
                    judge_rank: None,
                    total_notes: 0,
                    initial_bpm: 0.0,
                    min_bpm: 0.0,
                    max_bpm: 0.0,
                    length_ms: 0,
                    clear_type: summary
                        .as_ref()
                        .map(|summary| summary.clear_type())
                        .unwrap_or_default(),
                    ex_score: None,
                    max_combo: None,
                    gauge_value: None,
                    bp: None,
                    cb: None,
                    judge_counts: DisplayJudgeCounts::default(),
                    fast_slow_counts: None,
                    play_count: 0,
                    clear_count: 0,
                    replay_slots: [false; 4],
                    favorite_chart: false,
                    favorite_song: false,
                    has_document: false,
                    has_long_notes: false,
                    has_mines: false,
                    has_random: false,
                    chart_normal_notes: 0,
                    chart_long_notes: 0,
                    chart_scratch_notes: 0,
                    chart_long_scratch_notes: 0,
                    chart_mine_notes: 0,
                    chart_density: 0.0,
                    chart_peak_density: 0.0,
                    chart_end_density: 0.0,
                    chart_total_gauge: 0.0,
                    chart_main_bpm: 0.0,
                    chart_distribution: Vec::new(),
                    chart_bpm_graph_segments: Vec::new(),
                    folder_lamp_counts: summary
                        .as_ref()
                        .map(|summary| summary.lamp_counts)
                        .unwrap_or([0; 11]),
                    is_folder: true,
                    kind: *kind,
                    in_library: true,
                    achieved_trophy_names: Vec::new(),
                    course_titles: Default::default(),
                    course_constraints: Default::default(),
                    chart_key_mode: None,
                },
                SelectItem::Chart(row) => {
                    let play_count =
                        row.best_score.as_ref().map(|score| score.play_count).unwrap_or(0);
                    let clear_count =
                        row.best_score.as_ref().map(|score| score.clear_count).unwrap_or(0);
                    let scored_total_notes = row
                        .chart
                        .as_ref()
                        .map(|chart| {
                            chart.scored_total_notes_for_setting(profile.play.ln_mode_policy)
                        })
                        .unwrap_or(0);
                    SelectRowSnapshot {
                        index: index as u32,
                        title: row.display_title().to_string(),
                        subtitle: row
                            .chart
                            .as_ref()
                            .map(|chart| chart.subtitle.clone())
                            .unwrap_or_default(),
                        artist: row.display_artist().to_string(),
                        genre: row
                            .chart
                            .as_ref()
                            .map(|chart| chart.genre.clone())
                            .unwrap_or_default(),
                        difficulty_name: row
                            .chart
                            .as_ref()
                            .map(|chart| chart.difficulty_name.clone())
                            .unwrap_or_default(),
                        play_level: row
                            .chart
                            .as_ref()
                            .map(|chart| chart.play_level.clone())
                            .unwrap_or_default(),
                        table_level: row.table_level.clone(),
                        table_text_primary: row.table_text.table_name.clone(),
                        table_text_secondary: row.table_text.table_level.clone(),
                        table_text_fallback: row.table_text.table_full.clone(),
                        judge_rank: row.chart.as_ref().and_then(|chart| chart.judge_rank),
                        total_notes: scored_total_notes,
                        initial_bpm: row
                            .chart
                            .as_ref()
                            .map(|chart| chart.initial_bpm as f32)
                            .unwrap_or(0.0),
                        min_bpm: row
                            .chart
                            .as_ref()
                            .map(|chart| chart.min_bpm as f32)
                            .unwrap_or(0.0),
                        max_bpm: row
                            .chart
                            .as_ref()
                            .map(|chart| chart.max_bpm as f32)
                            .unwrap_or(0.0),
                        length_ms: row.chart.as_ref().map(|chart| chart.length_ms).unwrap_or(0),
                        clear_type: row
                            .best_score
                            .as_ref()
                            .map(|score| score.clear_type.clone())
                            .unwrap_or_default(),
                        ex_score: row.best_score.as_ref().map(|score| score.ex_score),
                        max_combo: row.best_score.as_ref().map(|score| score.max_combo),
                        gauge_value: row.best_score.as_ref().and_then(|score| score.gauge_value),
                        bp: row.best_score.as_ref().map(|score| score.bp),
                        cb: row.best_score.as_ref().map(|score| score.cb),
                        judge_counts: row
                            .best_score
                            .as_ref()
                            .map(|score| score.judge_counts)
                            .unwrap_or_default(),
                        fast_slow_counts: row
                            .best_score
                            .as_ref()
                            .map(|score| score.fast_slow_counts),
                        play_count,
                        clear_count,
                        replay_slots: row.replay_slots,
                        favorite_chart: row.favorite_chart,
                        favorite_song: row.favorite_song,
                        has_document: row.has_document,
                        has_long_notes: row
                            .chart
                            .as_ref()
                            .is_some_and(|chart| chart.has_long_notes),
                        has_mines: row.chart.as_ref().is_some_and(|chart| chart.has_mines),
                        has_random: false,
                        chart_normal_notes: row
                            .chart_analysis
                            .as_ref()
                            .map(|analysis| analysis.normal_notes)
                            .unwrap_or_else(|| {
                                row.chart.as_ref().map(|chart| chart.total_notes).unwrap_or(0)
                            }),
                        chart_long_notes: row
                            .chart_analysis
                            .as_ref()
                            .map(|analysis| analysis.long_notes)
                            .unwrap_or(0),
                        chart_scratch_notes: row
                            .chart_analysis
                            .as_ref()
                            .map(|analysis| analysis.scratch_notes)
                            .unwrap_or(0),
                        chart_long_scratch_notes: row
                            .chart_analysis
                            .as_ref()
                            .map(|analysis| analysis.long_scratch_notes)
                            .unwrap_or(0),
                        chart_density: row
                            .chart_analysis
                            .as_ref()
                            .map(|analysis| analysis.density as f32)
                            .unwrap_or(0.0),
                        chart_peak_density: row
                            .chart_analysis
                            .as_ref()
                            .map(|analysis| analysis.peak_density as f32)
                            .unwrap_or(0.0),
                        chart_end_density: row
                            .chart_analysis
                            .as_ref()
                            .map(|analysis| analysis.end_density as f32)
                            .unwrap_or(0.0),
                        chart_total_gauge: row
                            .chart
                            .as_ref()
                            .map(|chart| {
                                bmz_gameplay::gauge::gauge_total_for_chart(
                                    (chart.bms_total > 0.0).then_some(chart.bms_total),
                                    scored_total_notes,
                                ) as f32
                            })
                            .unwrap_or(0.0),
                        chart_main_bpm: row
                            .chart_analysis
                            .as_ref()
                            .map(|analysis| analysis.main_bpm as f32)
                            .unwrap_or_else(|| {
                                row.chart
                                    .as_ref()
                                    .map(|chart| chart.initial_bpm as f32)
                                    .unwrap_or(0.0)
                            }),
                        chart_distribution: row
                            .chart
                            .as_ref()
                            .and_then(|chart| chart_distributions.get(&chart.chart_id))
                            .map(|distribution| select_chart_distribution(distribution))
                            .unwrap_or_default(),
                        chart_mine_notes: row
                            .chart
                            .as_ref()
                            .and_then(|chart| chart_distributions.get(&chart.chart_id))
                            .map(|distribution| {
                                distribution.iter().map(|second| u32::from(second.mines)).sum()
                            })
                            .unwrap_or(0),
                        chart_bpm_graph_segments: row
                            .chart_analysis
                            .as_ref()
                            .map(|analysis| {
                                select_bpm_graph_segments(
                                    &analysis.speed_changes,
                                    row.chart.as_ref().map(|chart| chart.length_ms).unwrap_or(0),
                                )
                            })
                            .unwrap_or_default(),
                        folder_lamp_counts: [0; 11],
                        is_folder: false,
                        kind: bmz_render::scene::SelectRowKind::Song,
                        in_library: row.in_library(),
                        // Song rows have no course trophies.
                        achieved_trophy_names: Vec::new(),
                        course_titles: Default::default(),
                        course_constraints: Default::default(),
                        chart_key_mode: row
                            .chart
                            .as_ref()
                            .and_then(|chart| KeyMode::from_str_opt(&chart.mode)),
                    }
                }
                SelectItem::Course(row) => SelectRowSnapshot {
                    index: index as u32,
                    title: row.title.clone(),
                    subtitle: String::new(),
                    // Use the trophy names joined as "subtitle" so the artist
                    // slot shows e.g. "silvermedal / goldmedal".
                    artist: row.trophy_names.join(" / "),
                    genre: String::new(),
                    // Beatoraja-style category tag (DAN / COURSE).
                    difficulty_name: row.category_label.clone(),
                    // Beatoraja GradeBar rows do not expose a play level.
                    play_level: String::new(),
                    table_level: String::new(),
                    table_text_primary: String::new(),
                    table_text_secondary: String::new(),
                    table_text_fallback: String::new(),
                    judge_rank: None,
                    total_notes: row.total_notes,
                    initial_bpm: row.min_bpm,
                    min_bpm: row.min_bpm,
                    max_bpm: row.max_bpm,
                    length_ms: row.total_length_ms,
                    clear_type: row
                        .best_score
                        .as_ref()
                        .map(|best| best.clear_type.clone())
                        .unwrap_or_default(),
                    ex_score: row.best_score.as_ref().map(|best| best.ex_score),
                    max_combo: row.best_score.as_ref().map(|best| best.max_combo),
                    gauge_value: row.best_score.as_ref().map(|best| best.gauge_value),
                    bp: row.best_score.as_ref().map(|best| best.bp),
                    cb: row.best_score.as_ref().map(|best| best.cb),
                    judge_counts: row
                        .best_score
                        .as_ref()
                        .map(|best| best.judge_counts)
                        .unwrap_or_default(),
                    fast_slow_counts: row.best_score.as_ref().map(|best| best.fast_slow_counts),
                    play_count: row.best_score.as_ref().map(|best| best.play_count).unwrap_or(0),
                    clear_count: row.best_score.as_ref().map(|best| best.clear_count).unwrap_or(0),
                    replay_slots: row.replay_slots,
                    favorite_chart: false,
                    favorite_song: false,
                    has_document: false,
                    has_long_notes: false,
                    has_mines: false,
                    has_random: false,
                    chart_normal_notes: 0,
                    chart_long_notes: 0,
                    chart_scratch_notes: 0,
                    chart_long_scratch_notes: 0,
                    chart_mine_notes: 0,
                    chart_density: 0.0,
                    chart_peak_density: 0.0,
                    chart_end_density: 0.0,
                    chart_total_gauge: 0.0,
                    chart_main_bpm: 0.0,
                    chart_distribution: Vec::new(),
                    chart_bpm_graph_segments: Vec::new(),
                    folder_lamp_counts: [0; 11],
                    is_folder: false,
                    kind: bmz_render::scene::SelectRowKind::Course,
                    in_library: row.exists_all_songs(),
                    achieved_trophy_names: row.achieved_trophy_names.clone(),
                    course_titles: course_titles_from_entries(
                        row.entry_previews
                            .iter()
                            .map(|entry| (entry.title.as_str(), entry.resolved)),
                    ),
                    course_constraints: course_constraint_flags(&row.constraints),
                    chart_key_mode: None,
                },
                SelectItem::Executable(row) => SelectRowSnapshot {
                    index: index as u32,
                    title: row.title.clone(),
                    kind: bmz_render::scene::SelectRowKind::Executable,
                    in_library: !row.chart_ids.is_empty(),
                    ..SelectRowSnapshot::default()
                },
                SelectItem::Config(row) => {
                    let value = row.value_text(profile);
                    SelectRowSnapshot {
                        index: index as u32,
                        title: row.label().to_string(),
                        subtitle: String::new(),
                        artist: value.clone(),
                        genre: String::new(),
                        difficulty_name: String::new(),
                        play_level: value,
                        table_level: String::new(),
                        table_text_primary: String::new(),
                        table_text_secondary: String::new(),
                        table_text_fallback: String::new(),
                        judge_rank: None,
                        total_notes: 0,
                        initial_bpm: 0.0,
                        min_bpm: 0.0,
                        max_bpm: 0.0,
                        length_ms: 0,
                        clear_type: String::new(),
                        ex_score: None,
                        max_combo: None,
                        gauge_value: None,
                        bp: None,
                        cb: None,
                        judge_counts: DisplayJudgeCounts::default(),
                        fast_slow_counts: None,
                        play_count: 0,
                        clear_count: 0,
                        replay_slots: [false; 4],
                        favorite_chart: false,
                        favorite_song: false,
                        has_document: false,
                        has_long_notes: false,
                        has_mines: false,
                        has_random: false,
                        chart_normal_notes: 0,
                        chart_long_notes: 0,
                        chart_scratch_notes: 0,
                        chart_long_scratch_notes: 0,
                        chart_mine_notes: 0,
                        chart_density: 0.0,
                        chart_peak_density: 0.0,
                        chart_end_density: 0.0,
                        chart_total_gauge: 0.0,
                        chart_main_bpm: 0.0,
                        chart_distribution: Vec::new(),
                        chart_bpm_graph_segments: Vec::new(),
                        folder_lamp_counts: [0; 11],
                        is_folder: false,
                        kind: bmz_render::scene::SelectRowKind::Config,
                        in_library: true,
                        achieved_trophy_names: Vec::new(),
                        course_titles: Default::default(),
                        course_constraints: Default::default(),
                        chart_key_mode: None,
                    }
                }
                SelectItem::KeyBinding(row) => {
                    let value = key_config_edit
                        .filter(|session| {
                            session.key_mode == row.key_mode && session.target == row.target
                        })
                        .map(|session| session.preview_value(profile))
                        .unwrap_or_else(|| row.value_text(profile));
                    SelectRowSnapshot {
                        index: index as u32,
                        title: row.label(),
                        subtitle: String::new(),
                        artist: value.clone(),
                        genre: String::new(),
                        difficulty_name: String::new(),
                        play_level: value,
                        table_level: String::new(),
                        table_text_primary: String::new(),
                        table_text_secondary: String::new(),
                        table_text_fallback: String::new(),
                        judge_rank: None,
                        total_notes: 0,
                        initial_bpm: 0.0,
                        min_bpm: 0.0,
                        max_bpm: 0.0,
                        length_ms: 0,
                        clear_type: String::new(),
                        ex_score: None,
                        max_combo: None,
                        gauge_value: None,
                        bp: None,
                        cb: None,
                        judge_counts: DisplayJudgeCounts::default(),
                        fast_slow_counts: None,
                        play_count: 0,
                        clear_count: 0,
                        replay_slots: [false; 4],
                        favorite_chart: false,
                        favorite_song: false,
                        has_document: false,
                        has_long_notes: false,
                        has_mines: false,
                        has_random: false,
                        chart_normal_notes: 0,
                        chart_long_notes: 0,
                        chart_scratch_notes: 0,
                        chart_long_scratch_notes: 0,
                        chart_mine_notes: 0,
                        chart_density: 0.0,
                        chart_peak_density: 0.0,
                        chart_end_density: 0.0,
                        chart_total_gauge: 0.0,
                        chart_main_bpm: 0.0,
                        chart_distribution: Vec::new(),
                        chart_bpm_graph_segments: Vec::new(),
                        folder_lamp_counts: [0; 11],
                        is_folder: false,
                        kind: bmz_render::scene::SelectRowKind::Config,
                        in_library: true,
                        achieved_trophy_names: Vec::new(),
                        course_titles: Default::default(),
                        course_constraints: Default::default(),
                        chart_key_mode: None,
                    }
                }
                SelectItem::SettingsBack | SelectItem::SettingsClose => {
                    let (title_id, kind) = match item {
                        SelectItem::SettingsBack => {
                            ("select-back", bmz_render::scene::SelectRowKind::SettingsBack)
                        }
                        SelectItem::SettingsClose => {
                            ("select-close", bmz_render::scene::SelectRowKind::SettingsClose)
                        }
                        _ => unreachable!(),
                    };
                    SelectRowSnapshot {
                        index: index as u32,
                        title: Localizer::new(profile.ui.locale()).text(title_id),
                        subtitle: String::new(),
                        artist: String::new(),
                        genre: String::new(),
                        difficulty_name: String::new(),
                        play_level: String::new(),
                        table_level: String::new(),
                        table_text_primary: String::new(),
                        table_text_secondary: String::new(),
                        table_text_fallback: String::new(),
                        judge_rank: None,
                        total_notes: 0,
                        initial_bpm: 0.0,
                        min_bpm: 0.0,
                        max_bpm: 0.0,
                        length_ms: 0,
                        clear_type: String::new(),
                        ex_score: None,
                        max_combo: None,
                        gauge_value: None,
                        bp: None,
                        cb: None,
                        judge_counts: DisplayJudgeCounts::default(),
                        fast_slow_counts: None,
                        play_count: 0,
                        clear_count: 0,
                        replay_slots: [false; 4],
                        favorite_chart: false,
                        favorite_song: false,
                        has_document: false,
                        has_long_notes: false,
                        has_mines: false,
                        has_random: false,
                        chart_normal_notes: 0,
                        chart_long_notes: 0,
                        chart_scratch_notes: 0,
                        chart_long_scratch_notes: 0,
                        chart_mine_notes: 0,
                        chart_density: 0.0,
                        chart_peak_density: 0.0,
                        chart_end_density: 0.0,
                        chart_total_gauge: 0.0,
                        chart_main_bpm: 0.0,
                        chart_distribution: Vec::new(),
                        chart_bpm_graph_segments: Vec::new(),
                        folder_lamp_counts: [0; 11],
                        is_folder: true,
                        kind,
                        in_library: true,
                        achieved_trophy_names: Vec::new(),
                        course_titles: Default::default(),
                        course_constraints: Default::default(),
                        chart_key_mode: None,
                    }
                }
                SelectItem::AdvancedSettings => SelectRowSnapshot {
                    index: index as u32,
                    title: Localizer::new(profile.ui.locale()).text("select-advanced-settings"),
                    subtitle: String::new(),
                    artist: String::new(),
                    genre: String::new(),
                    difficulty_name: String::new(),
                    play_level: String::new(),
                    table_level: String::new(),
                    table_text_primary: String::new(),
                    table_text_secondary: String::new(),
                    table_text_fallback: String::new(),
                    judge_rank: None,
                    total_notes: 0,
                    initial_bpm: 0.0,
                    min_bpm: 0.0,
                    max_bpm: 0.0,
                    length_ms: 0,
                    clear_type: String::new(),
                    ex_score: None,
                    max_combo: None,
                    gauge_value: None,
                    bp: None,
                    cb: None,
                    judge_counts: DisplayJudgeCounts::default(),
                    fast_slow_counts: None,
                    play_count: 0,
                    clear_count: 0,
                    replay_slots: [false; 4],
                    favorite_chart: false,
                    favorite_song: false,
                    has_document: false,
                    has_long_notes: false,
                    has_mines: false,
                    has_random: false,
                    chart_normal_notes: 0,
                    chart_long_notes: 0,
                    chart_scratch_notes: 0,
                    chart_long_scratch_notes: 0,
                    chart_mine_notes: 0,
                    chart_density: 0.0,
                    chart_peak_density: 0.0,
                    chart_end_density: 0.0,
                    chart_total_gauge: 0.0,
                    chart_main_bpm: 0.0,
                    chart_distribution: Vec::new(),
                    chart_bpm_graph_segments: Vec::new(),
                    folder_lamp_counts: [0; 11],
                    is_folder: true,
                    kind: bmz_render::scene::SelectRowKind::SettingsFolder,
                    in_library: true,
                    achieved_trophy_names: Vec::new(),
                    course_titles: Default::default(),
                    course_constraints: Default::default(),
                    chart_key_mode: None,
                },
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectRowClickAction {
    Select(usize),
    EnterOrPlay,
    CancelSettingsEdit,
    ExitFolder,
}

pub(super) fn select_row_click_action(
    row_index: u32,
    button: MouseButton,
    selected_index: usize,
    item_len: usize,
    settings_editing: bool,
) -> Option<SelectRowClickAction> {
    match button {
        MouseButton::Left => {
            let next = row_index as usize;
            if next >= item_len {
                None
            } else if next == selected_index {
                Some(SelectRowClickAction::EnterOrPlay)
            } else {
                Some(SelectRowClickAction::Select(next))
            }
        }
        MouseButton::Right => Some(if settings_editing {
            SelectRowClickAction::CancelSettingsEdit
        } else {
            SelectRowClickAction::ExitFolder
        }),
        _ => None,
    }
}

pub(super) fn select_scroll_slider_index(value: f32, item_len: usize) -> Option<usize> {
    if item_len == 0 {
        return None;
    }
    if item_len == 1 {
        return Some(0);
    }
    let max_index = item_len - 1;
    Some((value.clamp(0.0, 1.0) * max_index as f32).round() as usize)
}

pub(super) fn select_scroll_duration_low_ms(config: &crate::config::app_config::AppConfig) -> u32 {
    config.select.scroll_duration_low_ms.clamp(2, 1000)
}

pub(super) fn select_scroll_duration_high_ms(config: &crate::config::app_config::AppConfig) -> u32 {
    config.select.scroll_duration_high_ms.clamp(1, 1000)
}

pub(super) fn select_analog_scroll_duration(mov: i32) -> Duration {
    let remaining = mov.abs().clamp(1, 2);
    Duration::from_millis((120 / remaining / remaining) as u64)
}

pub(super) fn log_gamepad_key_config_raw_event(
    backend: &str,
    event: &crate::input::gamepad::RawInputEvent,
) {
    let mapped_control = event.mapped_control.as_deref().unwrap_or("<unmapped>");
    tracing::info!(
        device_id = event.device_id.0,
        kind = event.kind.as_str(),
        logical = %event.logical,
        raw_code = event.raw_code.value,
        raw_code_label = %event.raw_code.label,
        mapped_control = %mapped_control,
        pressed = ?event.pressed,
        value = ?event.value,
        ticks = ?event.ticks,
        backend,
        "gamepad key config input"
    );
}

#[cfg(test)]
pub(super) fn select_control_action(
    control: &str,
    bindings: &SelectKeyBindings,
) -> Option<SelectAction> {
    scene_select_action(&ControlInputEvent::gamepad(DeviceId(1), control, true), bindings)
}

#[cfg(test)]
pub(super) fn select_action(
    physical_key: PhysicalKey,
    state: ElementState,
    repeat: bool,
    bindings: &SelectKeyBindings,
) -> Option<SelectAction> {
    scene_select_action(&ControlInputEvent::keyboard_parts(physical_key, state, repeat), bindings)
}

pub(super) fn select_wheel_move(delta: MouseScrollDelta) -> Option<SelectMove> {
    let y = mouse_wheel_y(delta);

    if y > 0.0 {
        Some(SelectMove::Previous)
    } else if y < 0.0 {
        Some(SelectMove::Next)
    } else {
        None
    }
}

pub(super) fn lane_cover_wheel_change(delta: MouseScrollDelta) -> Option<LaneCoverChange> {
    let y = mouse_wheel_y(delta);
    if y > 0.0 {
        Some(LaneCoverChange::Up)
    } else if y < 0.0 {
        Some(LaneCoverChange::Down)
    } else {
        None
    }
}

pub(super) fn mouse_wheel_y(delta: MouseScrollDelta) -> f32 {
    match delta {
        MouseScrollDelta::LineDelta(_, y) => y,
        MouseScrollDelta::PixelDelta(position) => position.y as f32,
    }
}
