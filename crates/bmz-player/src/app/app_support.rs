use super::*;

pub(super) fn select_ir_cache_context(
    ln_policy_setting: crate::ln_policy::LnPolicySetting,
    ln_policy: crate::ln_policy::LnScorePolicy,
    double_option: crate::select_options::DoubleOptionScoreBucket,
    rule_mode: bmz_gameplay::rule::RuleMode,
) -> String {
    format!(
        "{}:{}:{}:{}",
        ln_policy_setting.as_ir_str(),
        ln_policy.as_str(),
        double_option.as_str(),
        rule_mode.as_str()
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CoursePlayMetrics {
    pub(super) total_notes: u32,
    pub(super) ln_mode: Option<bmz_chart::model::LongNoteMode>,
}

pub(super) fn course_play_metrics_for_definition(
    library_db: &LibraryDatabase,
    definition: &bmz_core::course::CourseDefinition,
    app_config: &AppConfig,
    ln_policy_setting: crate::ln_policy::LnPolicySetting,
    rule_mode: bmz_gameplay::rule::RuleMode,
    entry_start_options: &[PlayStartOptions],
) -> Result<CoursePlayMetrics> {
    anyhow::ensure!(
        definition.entries.len() == entry_start_options.len(),
        "course entry option count mismatch: entries={}, options={}",
        definition.entries.len(),
        entry_start_options.len()
    );
    let mut total_notes = 0u32;
    let mut ln_mode = None;
    for (index, (entry, start_options)) in
        definition.entries.iter().zip(entry_start_options).enumerate()
    {
        let chart_id = entry
            .chart_id
            .with_context(|| format!("course entry {} is not resolved", index + 1))?;
        let mut session_options =
            play_session_options_from_start(app_config, start_options.clone());
        session_options.ln_policy_setting = ln_policy_setting;
        session_options.rule_mode = rule_mode;
        let metrics = crate::screens::play_session::scored_chart_metrics_for_chart(
            library_db,
            chart_id,
            &session_options,
        )
        .with_context(|| format!("failed to count course entry {} from source", index + 1))?;
        total_notes = total_notes.saturating_add(metrics.total_notes);
        ln_mode = crate::ln_policy::max_long_note_mode(ln_mode, metrics.ln_mode);
    }
    Ok(CoursePlayMetrics { total_notes, ln_mode })
}

pub(super) fn hydrate_course_entry_title_hints(
    library_db: &LibraryDatabase,
    definition: &mut bmz_core::course::CourseDefinition,
) -> Result<()> {
    let chart_ids =
        definition.entries.iter().filter_map(|entry| entry.chart_id).collect::<Vec<_>>();
    let titles = library_db
        .list_charts_by_ids(&chart_ids)?
        .into_iter()
        .map(|chart| (chart.chart_id, chart.title))
        .collect::<HashMap<_, _>>();
    apply_course_entry_title_hints(definition, &titles);
    Ok(())
}

pub(super) fn apply_course_entry_title_hints(
    definition: &mut bmz_core::course::CourseDefinition,
    titles: &HashMap<i64, String>,
) {
    for entry in &mut definition.entries {
        let Some(chart_id) = entry.chart_id else {
            continue;
        };
        let Some(title) = titles.get(&chart_id).filter(|title| !title.trim().is_empty()) else {
            continue;
        };
        entry.title_hint.clone_from(title);
    }
}

pub(super) fn player_stats_snapshot(
    score_db: &ScoreDatabase,
    library_db: &LibraryDatabase,
    day_start_hour: u8,
) -> PlayerStatsSnapshot {
    let mut snapshot = match score_db.player_stats() {
        Ok(stats) => player_stats_snapshot_from_stats(&stats),
        Err(error) => {
            tracing::warn!(%error, "failed to load player statistics");
            PlayerStatsSnapshot::default()
        }
    };
    match score_db.current_daily_statistics_range(day_start_hour) {
        Ok((start_at, end_at)) => {
            match score_db.daily_player_stats_between(start_at, end_at) {
                Ok(stats) => snapshot.daily = daily_player_stats_snapshot_from_stats(&stats),
                Err(error) => tracing::warn!(%error, "failed to load daily player statistics"),
            }
            match score_db.daily_recent_chart_sha256s_between(start_at, end_at, 10) {
                Ok(hashes) => {
                    for (index, hash) in hashes.into_iter().enumerate() {
                        snapshot.daily.recent_titles[index] = library_db
                            .list_charts_by_sha256(hash)
                            .ok()
                            .and_then(|charts| charts.into_iter().next())
                            .map(|chart| chart.title)
                            .unwrap_or_default();
                    }
                }
                Err(error) => tracing::warn!(%error, "failed to load recent daily chart titles"),
            }
        }
        Err(error) => tracing::warn!(%error, "failed to resolve daily statistics range"),
    }
    snapshot
}

pub(super) fn player_stats_snapshot_from_stats(stats: &PlayerStats) -> PlayerStatsSnapshot {
    PlayerStatsSnapshot {
        play_count: stats.play_count,
        clear_count: stats.clear_count,
        playtime_seconds: stats.playtime_seconds,
        max_combo: stats.max_combo,
        fast_pgreat: stats.fast_pgreat,
        slow_pgreat: stats.slow_pgreat,
        fast_great: stats.fast_great,
        slow_great: stats.slow_great,
        fast_good: stats.fast_good,
        slow_good: stats.slow_good,
        fast_bad: stats.fast_bad,
        slow_bad: stats.slow_bad,
        fast_poor: stats.fast_poor,
        slow_poor: stats.slow_poor,
        fast_empty_poor: stats.fast_empty_poor,
        slow_empty_poor: stats.slow_empty_poor,
        daily: DailyPlayerStatsSnapshot::default(),
    }
}

pub(super) fn daily_player_stats_snapshot_from_stats(
    stats: &DailyPlayerStats,
) -> DailyPlayerStatsSnapshot {
    DailyPlayerStatsSnapshot {
        play_count: stats.play_count,
        clear_count: stats.clear_count,
        pgreat: stats.pgreat,
        great: stats.great,
        good: stats.good,
        bad: stats.bad,
        poor: stats.poor,
        empty_poor: stats.empty_poor,
        score_update_count: stats.score_update_count,
        clear_update_count: stats.clear_update_count,
        miss_count_update_count: stats.miss_count_update_count,
        recent_titles: Default::default(),
    }
}

pub(super) fn initialize_gamepad_backend(
    kind: GamepadBackendKind,
    sensitivity: f32,
    scratch_threshold: u32,
    raw_input_bridge: Option<crate::input::rawinput::RawInputBridge>,
) -> Option<crate::input::gamepad::GamepadBackend> {
    match kind {
        GamepadBackendKind::Auto => initialize_gilrs_backend(sensitivity, scratch_threshold),
        GamepadBackendKind::Gilrs => initialize_gilrs_backend(sensitivity, scratch_threshold),
        GamepadBackendKind::RawInput => {
            #[cfg(windows)]
            if let Some(bridge) = raw_input_bridge {
                tracing::info!("Raw Input gamepad backend initialized; awaiting window attachment");
                return Some(crate::input::gamepad::GamepadBackend::RawInput(Box::new(
                    crate::input::rawinput::RawInputBackend::new(
                        bridge,
                        sensitivity,
                        scratch_threshold,
                    ),
                )));
            }
            #[cfg(windows)]
            tracing::warn!("Raw Input message bridge is unavailable; falling back to gilrs");
            #[cfg(not(windows))]
            tracing::warn!(
                "Raw Input gamepad backend is only available on Windows; falling back to gilrs"
            );
            initialize_gilrs_backend(sensitivity, scratch_threshold)
        }
        GamepadBackendKind::GameInput => {
            #[cfg(all(windows, feature = "experimental-gameinput"))]
            {
                if let Some(backend) = initialize_gameinput_backend(sensitivity, scratch_threshold)
                {
                    return Some(backend);
                }
                tracing::warn!("GameInput initialization failed; falling back to gilrs");
            }
            #[cfg(not(all(windows, feature = "experimental-gameinput")))]
            tracing::warn!("GameInput backend is disabled; falling back to gilrs");
            initialize_gilrs_backend(sensitivity, scratch_threshold)
        }
    }
}

#[cfg(all(windows, feature = "experimental-gameinput"))]
pub(super) fn initialize_gameinput_backend(
    sensitivity: f32,
    scratch_threshold: u32,
) -> Option<crate::input::gamepad::GamepadBackend> {
    match crate::input::gameinput::GameInputBackend::new(sensitivity, scratch_threshold) {
        Ok(backend) => {
            tracing::info!("GameInput initialized on main thread");
            Some(crate::input::gamepad::GamepadBackend::GameInput(Box::new(backend)))
        }
        Err(error) => {
            tracing::warn!(%error, "GameInput init failed");
            None
        }
    }
}

pub(super) fn initialize_gilrs_backend(
    sensitivity: f32,
    scratch_threshold: u32,
) -> Option<crate::input::gamepad::GamepadBackend> {
    match crate::input::gilrs::GilrsBackend::new(sensitivity, scratch_threshold) {
        Ok(backend) => {
            tracing::info!("gilrs initialized");
            Some(crate::input::gamepad::GamepadBackend::Gilrs(Box::new(backend)))
        }
        Err(error) => {
            tracing::warn!(%error, "gilrs init failed");
            None
        }
    }
}

pub(super) fn resolve_gamepad_runtime_slots(
    config: &GlobalInputConfig,
    backend: Option<&crate::input::gamepad::GamepadBackend>,
) -> [Option<DeviceId>; 2] {
    let connected = backend
        .into_iter()
        .flat_map(crate::input::gamepad::GamepadBackend::connected_gamepads)
        .collect::<Vec<_>>();
    let using_gilrs = backend.is_some_and(crate::input::gamepad::GamepadBackend::is_gilrs);
    crate::input::gamepad::resolve_gamepad_slot_assignments(
        config.gamepad_slot_device_ids.each_ref().map(Option::as_deref),
        config.gamepad_slot_gilrs_ids,
        using_gilrs,
        !using_gilrs,
        &connected,
    )
}
