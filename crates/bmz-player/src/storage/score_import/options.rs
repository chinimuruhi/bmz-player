use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct ResolvedImportLnPolicy {
    pub(super) ln_policy: LnScorePolicy,
    pub(super) expected_notes: u32,
}

pub(super) fn beatoraja_mode_to_ln_setting(mode: i64) -> LnPolicySetting {
    match mode {
        0 => LnPolicySetting::AutoLn,
        1 => LnPolicySetting::AutoCn,
        2 => LnPolicySetting::AutoHcn,
        other => {
            tracing::debug!(mode = other, "unknown beatoraja score.mode; treating as AutoLn");
            LnPolicySetting::AutoLn
        }
    }
}

/// Maps beatoraja's decimal-packed score option to the two arrangement slots
/// which BMZ records for an attempt.  The low digit is 1P, the tens digit is
/// 2P; the hundreds digit is handled separately by
/// [`beatoraja_double_option`].
pub(super) fn beatoraja_arrange_options(
    option: i64,
    chart_mode: &str,
) -> (ArrangeOption, ArrangeOption) {
    if option < 0 {
        tracing::debug!(option, "negative beatoraja score.option; using Normal arrange");
        return (ArrangeOption::Normal, ArrangeOption::Normal);
    }
    (
        beatoraja_arrange_option(option % 10, chart_mode),
        beatoraja_arrange_option((option / 10) % 10, chart_mode),
    )
}

pub(super) fn beatoraja_arrange_option(random_option: i64, chart_mode: &str) -> ArrangeOption {
    let general = match random_option {
        0 => ArrangeOption::Normal,
        1 => ArrangeOption::Mirror,
        2 => ArrangeOption::Random,
        3 => ArrangeOption::RRandom,
        4 => ArrangeOption::SRandom,
        5 => ArrangeOption::Spiral,
        6 => ArrangeOption::HRandom,
        7 => ArrangeOption::AllScratch,
        8 => ArrangeOption::RandomEx,
        9 => ArrangeOption::SRandomEx,
        _ => {
            tracing::debug!(random_option, "unknown beatoraja random option; using Normal");
            ArrangeOption::Normal
        }
    };

    // beatoraja has a distinct POP'N option table.  BMZ does not implement
    // CONVERGE or the playable-only variants, so retain an equivalent normal/
    // random class without claiming an unsupported arrangement was reproduced.
    if chart_mode != "9K" {
        return general;
    }
    match random_option {
        7 => {
            tracing::debug!("beatoraja PMS CONVERGE has no BMZ equivalent; using Normal");
            ArrangeOption::Normal
        }
        8 => {
            tracing::debug!("approximating beatoraja PMS RANDOM PLAYABLE as Random");
            ArrangeOption::Random
        }
        9 => {
            tracing::debug!("approximating beatoraja PMS S-RANDOM PLAYABLE as SRandom");
            ArrangeOption::SRandom
        }
        _ => general,
    }
}

/// Reads beatoraja's hundreds digit as the actual DP option selected for the
/// attempt.  This is intentionally separate from
/// [`DoubleOptionScoreBucket`](crate::select_options::DoubleOptionScoreBucket):
/// BMZ groups OFF and FLIP scores together, but history must retain which of
/// those two layouts the player used.
pub(super) fn beatoraja_double_option(option: i64) -> DoubleOption {
    if option < 0 {
        return DoubleOption::Off;
    }
    match option / 100 {
        0 => DoubleOption::Off,
        1 => DoubleOption::Flip,
        2 => DoubleOption::Battle,
        3 => DoubleOption::BattleAutoScratch,
        double_option => {
            tracing::debug!(double_option, "unknown beatoraja double option; using Off bucket");
            DoubleOption::Off
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Lr2ImportOptions {
    pub(super) arrange: ArrangeOption,
    pub(super) arrange_2p: ArrangeOption,
    pub(super) applied_double_option: DoubleOption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lr2ImportOptionError {
    Negative,
    Scatter { player: u8 },
    Converge { player: u8 },
    UnknownArrange { player: u8, option: i64 },
    UnknownDouble { option: i64 },
}

/// Decodes LR2's decimal-packed `score.op_best` setting.
///
/// The units digit is the gauge and intentionally ignored: LR2 keeps aggregate
/// best values, so it need not identify the play that supplied every stored
/// field.  Tens/hundreds are the 1P/2P arrangements and the thousands digit
/// records DP FLIP.
pub(super) fn lr2_import_options(op_best: i64) -> Result<Lr2ImportOptions, Lr2ImportOptionError> {
    if op_best < 0 {
        return Err(Lr2ImportOptionError::Negative);
    }
    let arrange = lr2_arrange_option((op_best / 10) % 10, 1)?;
    let arrange_2p = lr2_arrange_option((op_best / 100) % 10, 2)?;
    let applied_double_option = match (op_best / 1000) % 10 {
        0 => DoubleOption::Off,
        1 => DoubleOption::Flip,
        option => return Err(Lr2ImportOptionError::UnknownDouble { option }),
    };
    Ok(Lr2ImportOptions { arrange, arrange_2p, applied_double_option })
}

pub(super) fn lr2_arrange_option(
    option: i64,
    player: u8,
) -> Result<ArrangeOption, Lr2ImportOptionError> {
    match option {
        0 => Ok(ArrangeOption::Normal),
        1 => Ok(ArrangeOption::Mirror),
        2 => Ok(ArrangeOption::Random),
        3 => Ok(ArrangeOption::SRandom),
        4 => Err(Lr2ImportOptionError::Scatter { player }),
        5 => Err(Lr2ImportOptionError::Converge { player }),
        option => Err(Lr2ImportOptionError::UnknownArrange { player, option }),
    }
}

pub(super) fn lr2_ex_score(row: &Lr2ScoreRow) -> u64 {
    u64::from(row.perfect) * 2 + u64::from(row.great)
}

pub(super) fn beatoraja_ex_score(row: &BeatorajaScoreRow) -> u64 {
    (u64::from(row.epg) + u64::from(row.lpg)) * 2 + u64::from(row.egr) + u64::from(row.lgr)
}

pub(super) fn score_summary_is_sane(total_notes: u32, max_combo: u32, ex_score: u64) -> bool {
    total_notes > 0
        && max_combo <= total_notes
        && ex_score <= u64::from(total_notes).saturating_mul(2)
}

pub(super) fn resolve_import_ln_policy(
    library_db: &LibraryDatabase,
    chart_sha256: [u8; 32],
    initial_policy: LnScorePolicy,
    source_notes: u32,
    chart_cache: &mut HashMap<[u8; 32], Arc<PlayableChart>>,
) -> Result<Option<ResolvedImportLnPolicy>> {
    let expected =
        expected_notes_for_policy(library_db, chart_sha256, initial_policy, chart_cache)?;
    if source_notes == expected {
        return Ok(Some(ResolvedImportLnPolicy {
            ln_policy: initial_policy,
            expected_notes: expected,
        }));
    }
    if initial_policy != LnScorePolicy::ForceLn {
        let force_expected = expected_notes_for_policy(
            library_db,
            chart_sha256,
            LnScorePolicy::ForceLn,
            chart_cache,
        )?;
        if source_notes == force_expected {
            return Ok(Some(ResolvedImportLnPolicy {
                ln_policy: LnScorePolicy::ForceLn,
                expected_notes: force_expected,
            }));
        }
    }
    Ok(None)
}

pub(super) fn expected_notes_for_policy(
    library_db: &LibraryDatabase,
    chart_sha256: [u8; 32],
    policy: LnScorePolicy,
    chart_cache: &mut HashMap<[u8; 32], Arc<PlayableChart>>,
) -> Result<u32> {
    let charts = library_db.list_charts_by_sha256(chart_sha256)?;
    let Some(item) = charts.first() else {
        bail!("chart missing from library while resolving import note count");
    };
    // No long notes: every policy collapses to ForceLn / base total_notes.
    if !item.ln_profile.has_any_ln() {
        return Ok(item.total_notes);
    }
    // ForceLn never scores long ends separately, so base total_notes is enough.
    if policy == LnScorePolicy::ForceLn {
        return Ok(item.total_notes);
    }
    let chart = load_import_chart(library_db, chart_sha256, item.chart_id, chart_cache)?;
    Ok(expected_scored_note_count_for_policy(&chart, policy))
}

pub(super) fn load_import_chart(
    library_db: &LibraryDatabase,
    chart_sha256: [u8; 32],
    chart_id: i64,
    chart_cache: &mut HashMap<[u8; 32], Arc<PlayableChart>>,
) -> Result<Arc<PlayableChart>> {
    if let Some(chart) = chart_cache.get(&chart_sha256) {
        return Ok(Arc::clone(chart));
    }
    #[cfg(test)]
    if let Some(chart) = take_test_import_chart(chart_sha256) {
        let chart = Arc::new(chart);
        chart_cache.insert(chart_sha256, Arc::clone(&chart));
        return Ok(chart);
    }
    let Some(path) = library_db.primary_chart_file_path(chart_id)? else {
        bail!("chart file path missing for chart id {chart_id}");
    };
    let imported = import_bms_chart(Path::new(&path), None, false)
        .with_context(|| format!("failed to import chart for score note-count check: {path}"))?;
    let chart = Arc::new(imported.chart);
    chart_cache.insert(chart_sha256, Arc::clone(&chart));
    Ok(chart)
}

#[cfg(test)]
thread_local! {
    static TEST_IMPORT_CHARTS: std::cell::RefCell<HashMap<[u8; 32], PlayableChart>> =
        std::cell::RefCell::new(HashMap::new());
}

#[cfg(test)]
pub(super) fn set_test_import_chart(sha256: [u8; 32], chart: PlayableChart) {
    TEST_IMPORT_CHARTS.with(|maps| {
        maps.borrow_mut().insert(sha256, chart);
    });
}

#[cfg(test)]
pub(super) fn take_test_import_chart(sha256: [u8; 32]) -> Option<PlayableChart> {
    TEST_IMPORT_CHARTS.with(|maps| maps.borrow().get(&sha256).cloned())
}

#[cfg(test)]
pub(super) fn clear_test_import_charts() {
    TEST_IMPORT_CHARTS.with(|maps| maps.borrow_mut().clear());
}

pub(super) fn imported_score_record(
    chart_sha256: [u8; 32],
    played_at: i64,
    clear_type: ClearType,
    total_notes: u32,
    score: ScoreState,
    random_seed: Option<i64>,
    rule_mode: &str,
    ln_policy: LnScorePolicy,
) -> ScoreRecord {
    ScoreRecord {
        chart_sha256,
        ln_policy,
        double_option: crate::select_options::DoubleOptionScoreBucket::Off,
        applied_double_option: DoubleOption::Off,
        played_at,
        clear_type,
        gauge_type: gauge_type_for_clear(clear_type),
        gauge_value: Some(gauge_value_for_clear(clear_type)),
        total_notes,
        playtime_seconds: 0,
        score,
        count_unprocessed_notes: clear_type == ClearType::Failed,
        random_seed,
        seed_scheme: crate::storage::replay::SEED_SCHEME_LEGACY_SHARED_V3.to_string(),
        arrange: "Normal".to_string(),
        arrange_2p: "Normal".to_string(),
        gauge_option: String::new(),
        rule_mode: rule_mode.to_string(),
        assist_mask: 0,
        autoplay: false,
        device_type: InputDeviceKind::Keyboard,
        replay_path: String::new(),
        source_kind: ScoreSourceKind::Local,
    }
}
