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
