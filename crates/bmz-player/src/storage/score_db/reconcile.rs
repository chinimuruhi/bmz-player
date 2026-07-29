use super::*;

pub(super) fn source_score_history_match(
    conn: &Connection,
    record: &ScoreRecord,
) -> Result<Option<SourceScoreHistoryMatch>> {
    let judges = &record.score.judges;
    conn.query_row(
        "SELECT id, device_type
         FROM score_history
         WHERE source_kind = ?1
           AND chart_sha256 = ?2
           AND ln_policy = ?3
           AND double_option = ?4
           AND clear_type = ?5
           AND gauge_type = ?6
           AND (?7 IS NULL OR gauge_value = ?7)
           AND total_notes = ?8
           AND ex_score = ?9
           AND bp = ?10
           AND cb = ?11
           AND max_combo = ?12
           AND fast_pgreat = ?13
           AND slow_pgreat = ?14
           AND fast_great = ?15
           AND slow_great = ?16
           AND fast_good = ?17
           AND slow_good = ?18
           AND fast_bad = ?19
           AND slow_bad = ?20
           AND fast_poor = ?21
           AND slow_poor = ?22
           AND fast_empty_poor = ?23
           AND slow_empty_poor = ?24
           AND random_seed IS ?25
           AND arrange = ?26
           AND arrange_2p = ?27
           AND gauge_option = ?28
           AND rule_mode = ?29
           AND assist_mask = ?30
           AND autoplay = ?31
           AND applied_double_option = ?32
           AND seed_scheme = ?33
         ORDER BY id ASC
         LIMIT 1",
        params![
            record.source_kind.as_str(),
            hash_to_hex(&record.chart_sha256),
            record.ln_policy.as_str(),
            record.double_option.as_str(),
            record.clear_type.as_str(),
            gauge_type_str(record.gauge_type),
            record.gauge_value,
            record.total_notes,
            record.score.ex_score(),
            score_record_bp(record),
            score_record_cb(record),
            record.score.max_combo,
            judges.fast_pgreat,
            judges.slow_pgreat,
            judges.fast_great,
            judges.slow_great,
            judges.fast_good,
            judges.slow_good,
            judges.fast_bad,
            judges.slow_bad,
            judges.fast_poor,
            judges.slow_poor,
            judges.fast_empty_poor,
            judges.slow_empty_poor,
            record.random_seed,
            record.arrange.as_str(),
            record.arrange_2p.as_str(),
            record.gauge_option.as_str(),
            record.rule_mode.as_str(),
            record.assist_mask,
            record.autoplay,
            record.applied_double_option.to_persistent_str(),
            record.seed_scheme.as_str(),
        ],
        |row| {
            Ok(SourceScoreHistoryMatch {
                history_id: row.get(0)?,
                device_type: device_type_from_row(row, 1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

pub(super) fn update_score_best_device_type_from_history(
    conn: &Connection,
    history_id: i64,
    device_type: InputDeviceKind,
) -> Result<()> {
    conn.execute(
        "UPDATE score_best
         SET device_type = ?1
         WHERE best_score_history_id = ?2",
        params![device_type.as_str(), history_id],
    )?;
    Ok(())
}

pub(super) fn legacy_beatoraja_matching_history_ids(
    conn: &Connection,
    selected_alias: &str,
) -> Result<Vec<i64>> {
    let (selected, counterpart) = match selected_alias {
        "legacy" => ("legacy", "imported"),
        "imported" => ("imported", "legacy"),
        _ => unreachable!("invalid legacy beatoraja cleanup alias"),
    };
    let selected_source_kind = if selected == "legacy" { "Local" } else { "Beatoraja" };
    let counterpart_source_kind = if counterpart == "legacy" { "Local" } else { "Beatoraja" };
    let sql = format!(
        "SELECT DISTINCT {selected}.id
         FROM score_history AS {selected}
         WHERE {selected}.source_kind = ?1
           AND {selected}.course_score_id IS NULL
           AND EXISTS (
               SELECT 1
               FROM score_history AS {counterpart}
               WHERE {counterpart}.source_kind = ?2
                 AND {counterpart}.course_score_id IS NULL
                 AND {counterpart}.chart_sha256 = {selected}.chart_sha256
                 AND {counterpart}.played_at = {selected}.played_at
                 AND {counterpart}.ex_score = {selected}.ex_score
                 AND {counterpart}.bp = {selected}.bp
                 AND {counterpart}.cb = {selected}.cb
                 AND {counterpart}.max_combo = {selected}.max_combo
                 AND {counterpart}.fast_pgreat = {selected}.fast_pgreat
                 AND {counterpart}.slow_pgreat = {selected}.slow_pgreat
                 AND {counterpart}.fast_great = {selected}.fast_great
                 AND {counterpart}.slow_great = {selected}.slow_great
                 AND {counterpart}.fast_good = {selected}.fast_good
                 AND {counterpart}.slow_good = {selected}.slow_good
                 AND {counterpart}.fast_bad = {selected}.fast_bad
                 AND {counterpart}.slow_bad = {selected}.slow_bad
                 AND {counterpart}.fast_poor = {selected}.fast_poor
                 AND {counterpart}.slow_poor = {selected}.slow_poor
                 AND {counterpart}.fast_empty_poor = {selected}.fast_empty_poor
                 AND {counterpart}.slow_empty_poor = {selected}.slow_empty_poor
                 AND {counterpart}.random_seed IS {selected}.random_seed
           )
         ORDER BY {selected}.id"
    );
    let mut statement = conn.prepare(&sql)?;
    statement
        .query_map(params![selected_source_kind, counterpart_source_kind], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub(super) fn delete_score_history_ids(conn: &Connection, history_ids: &[i64]) -> Result<u32> {
    const DELETE_CHUNK_SIZE: usize = 500;
    let mut deleted = 0_u32;
    for ids in history_ids.chunks(DELETE_CHUNK_SIZE) {
        let placeholders = std::iter::repeat_n("?", ids.len()).collect::<Vec<_>>().join(", ");
        let sql = format!("DELETE FROM score_history WHERE id IN ({placeholders})");
        deleted = deleted.saturating_add(conn.execute(&sql, params_from_iter(ids.iter()))? as u32);
    }
    Ok(deleted)
}

pub(super) fn rebuild_score_aggregates(conn: &Connection) -> Result<()> {
    // score_history は playtime_seconds を保持していないため、過去の通常プレイの
    // 総プレイ時間は復元できない。候補は外部 import に限定し、既存値を保全する。
    let preserved_playtime_seconds: u64 = conn
        .query_row("SELECT playtime_seconds FROM player_stats WHERE id = 1", [], |row| row.get(0))
        .optional()?
        .unwrap_or(0);
    const SCORE_KEY: &str = "h.chart_sha256 = score_best.chart_sha256
        AND h.ln_policy = score_best.ln_policy
        AND h.double_option = score_best.double_option
        AND CASE h.rule_mode
            WHEN 'Lr2Oraja' THEN 'Lr2Oraja'
            WHEN 'Dx' THEN 'Dx'
            ELSE 'Beatoraja'
        END = score_best.rule_mode
        AND h.course_score_id IS NULL";
    const CLEAR_RANK: &str = "CASE h.clear_type
        WHEN 'NoPlay' THEN 0
        WHEN 'Failed' THEN 1
        WHEN 'AssistEasy' THEN 2
        WHEN 'LightAssistEasy' THEN 3
        WHEN 'Easy' THEN 4
        WHEN 'Normal' THEN 5
        WHEN 'Hard' THEN 6
        WHEN 'ExHard' THEN 7
        WHEN 'FullCombo' THEN 8
        WHEN 'Perfect' THEN 9
        WHEN 'Max' THEN 10
        ELSE 0
    END";
    let score_source = format!(
        "SELECT h.id FROM score_history AS h
         WHERE {SCORE_KEY}
         ORDER BY h.ex_score DESC, h.bp ASC, h.cb ASC, h.max_combo DESC, h.id ASC
         LIMIT 1"
    );
    let clear_source = format!(
        "SELECT h.id FROM score_history AS h
         WHERE {SCORE_KEY}
         ORDER BY {CLEAR_RANK} DESC, h.id ASC
         LIMIT 1"
    );
    let score_value = |column: &str| {
        format!(
            "(SELECT h.{column} FROM score_history AS h
              WHERE h.id = ({score_source}))"
        )
    };
    let clear_value = |column: &str| {
        format!(
            "(SELECT h.{column} FROM score_history AS h
              WHERE h.id = ({clear_source}))"
        )
    };
    let aggregate = |expression: &str| {
        format!("(SELECT {expression} FROM score_history AS h WHERE {SCORE_KEY})")
    };

    conn.execute(
        &format!(
            "DELETE FROM score_best
             WHERE NOT EXISTS (SELECT 1 FROM score_history AS h WHERE {SCORE_KEY})"
        ),
        [],
    )?;
    conn.execute(
        &format!(
            "UPDATE score_best SET
                clear_type = {clear_type},
                gauge_type = {gauge_type},
                gauge_value = {gauge_value},
                ex_score = {ex_score},
                bp = {bp},
                cb = {cb},
                max_combo = {max_combo},
                fast_pgreat = {fast_pgreat},
                slow_pgreat = {slow_pgreat},
                fast_great = {fast_great},
                slow_great = {slow_great},
                fast_good = {fast_good},
                slow_good = {slow_good},
                fast_bad = {fast_bad},
                slow_bad = {slow_bad},
                fast_poor = {fast_poor},
                slow_poor = {slow_poor},
                fast_empty_poor = {fast_empty_poor},
                slow_empty_poor = {slow_empty_poor},
                played_at = {played_at},
                replay_path = {replay_path},
                device_type = {device_type},
                ghost = CASE
                    WHEN best_score_history_id = ({score_source}) THEN ghost
                    ELSE ''
                END,
                best_score_history_id = ({score_source}),
                play_count = {play_count},
                clear_count = {clear_count}",
            clear_type = clear_value("clear_type"),
            gauge_type = clear_value("gauge_type"),
            gauge_value = clear_value("gauge_value"),
            ex_score = score_value("ex_score"),
            bp = aggregate("MIN(h.bp)"),
            cb = aggregate("MIN(h.cb)"),
            max_combo = aggregate("MAX(h.max_combo)"),
            fast_pgreat = score_value("fast_pgreat"),
            slow_pgreat = score_value("slow_pgreat"),
            fast_great = score_value("fast_great"),
            slow_great = score_value("slow_great"),
            fast_good = score_value("fast_good"),
            slow_good = score_value("slow_good"),
            fast_bad = score_value("fast_bad"),
            slow_bad = score_value("slow_bad"),
            fast_poor = score_value("fast_poor"),
            slow_poor = score_value("slow_poor"),
            fast_empty_poor = score_value("fast_empty_poor"),
            slow_empty_poor = score_value("slow_empty_poor"),
            played_at = score_value("played_at"),
            replay_path = score_value("replay_path"),
            device_type = score_value("device_type"),
            play_count = aggregate("COUNT(*)"),
            clear_count = aggregate(
                "SUM(CASE WHEN h.clear_type NOT IN ('NoPlay', 'Failed') THEN 1 ELSE 0 END)"
            ),
        ),
        [],
    )?;
    conn.execute("DELETE FROM player_stats", [])?;
    conn.execute(
        "INSERT INTO player_stats (
            id, play_count, clear_count, playtime_seconds, max_combo,
            fast_pgreat, slow_pgreat, fast_great, slow_great,
            fast_good, slow_good, fast_bad, slow_bad,
            fast_poor, slow_poor, fast_empty_poor, slow_empty_poor, updated_at
         )
         SELECT
            1,
            COUNT(*),
            COALESCE(SUM(CASE WHEN clear_type NOT IN ('NoPlay', 'Failed') THEN 1 ELSE 0 END), 0),
            ?1,
            COALESCE(MAX(max_combo), 0),
            COALESCE(SUM(fast_pgreat), 0),
            COALESCE(SUM(slow_pgreat), 0),
            COALESCE(SUM(fast_great), 0),
            COALESCE(SUM(slow_great), 0),
            COALESCE(SUM(fast_good), 0),
            COALESCE(SUM(slow_good), 0),
            COALESCE(SUM(fast_bad), 0),
            COALESCE(SUM(slow_bad), 0),
            COALESCE(SUM(fast_poor), 0),
            COALESCE(SUM(slow_poor), 0),
            COALESCE(SUM(fast_empty_poor), 0),
            COALESCE(SUM(slow_empty_poor), 0),
            COALESCE(MAX(played_at), 0)
         FROM score_history
         WHERE course_score_id IS NULL",
        params![preserved_playtime_seconds],
    )?;
    Ok(())
}
