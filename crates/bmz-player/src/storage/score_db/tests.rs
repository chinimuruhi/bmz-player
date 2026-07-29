use bmz_core::clear::{ClearType, GaugeType};
use bmz_core::ids::NoteId;
use bmz_core::input::InputDeviceKind;
use bmz_core::judge::{Judge, TimingSide};
use bmz_core::lane::Lane;
use bmz_core::time::TimeUs;
use bmz_gameplay::judge::model::JudgementEvent;
use bmz_gameplay::result::PlayResult;
use bmz_gameplay::score::ScoreState;

use super::*;
use crate::storage::migration::{SCORE_MIGRATIONS, run_migrations};

fn score_with_ex_score(ex_score: u32) -> ScoreState {
    let mut score = ScoreState::default();
    for index in 0..(ex_score / 2) {
        score.apply(&JudgementEvent {
            note_id: Some(NoteId(index)),
            lane: Lane::Key1,
            judge: Judge::PGreat,
            side: TimingSide::Slow,
            delta: TimeUs(0),
            time: TimeUs(index as i64),
            affects_score: true,
        });
    }
    score
}

fn record(ex_score: u32, clear_type: ClearType) -> ScoreRecord {
    ScoreRecord {
        chart_sha256: [7; 32],
        ln_policy: LnScorePolicy::ForceLn,
        double_option: DoubleOptionScoreBucket::Off,
        applied_double_option: DoubleOption::Off,
        played_at: 1_700_000_000,
        clear_type,
        gauge_type: Some(GaugeType::Normal),
        gauge_value: Some(82.0),
        total_notes: ex_score / 2,
        playtime_seconds: 0,
        score: score_with_ex_score(ex_score),
        count_unprocessed_notes: clear_type == ClearType::Failed,
        random_seed: None,
        seed_scheme: String::new(),
        arrange: "Normal".to_string(),
        arrange_2p: "Normal".to_string(),
        gauge_option: String::new(),
        rule_mode: String::new(),
        assist_mask: 0,
        autoplay: false,
        device_type: InputDeviceKind::Keyboard,
        replay_path: String::new(),
        source_kind: ScoreSourceKind::Local,
    }
}

fn key(sha: [u8; 32]) -> ScoreKey {
    ScoreKey::new(sha, LnScorePolicy::ForceLn)
}

fn insert_test_course_score(db: &mut ScoreDatabase, course_hash: &str) -> i64 {
    db.conn_mut()
        .execute(
            "INSERT INTO course_scores (
                    course_hash, source, course_key, title, kind, constraints_json,
                    chart_sha256s_json, ex_score, max_ex_score, clear_type, gauge_type,
                    gauge_value, max_combo, bp, course_failed, course_clear, arrange,
                    trophies_json, played_at, rule_mode
                 ) VALUES (
                    ?1, '', '', '', '', '{}',
                    '[]', 0, 0, 'NoPlay', '',
                    0.0, 0, 0, 0, 0, 'Normal',
                    '[]', 0, 'Beatoraja'
                 )",
            params![course_hash],
        )
        .unwrap();
    db.conn().last_insert_rowid()
}

#[test]
fn insert_score_persists_enum_strings_and_empty_values() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut record = record(20, ClearType::Normal);
    record.gauge_type = None;
    record.rule_mode = "Dx".to_string();
    record.arrange = "Random".to_string();
    record.arrange_2p = "Mirror".to_string();
    record.applied_double_option = DoubleOption::Flip;
    record.source_kind = ScoreSourceKind::Beatoraja;
    record.seed_scheme = "beatoraja_24bit_v1".to_string();
    record.device_type = InputDeviceKind::Controller;
    db.insert_score(&record).unwrap();

    let (
            clear_type,
            gauge_type,
            gauge_option,
            rule_mode,
            arrange,
            arrange_2p,
            device_type,
            replay_path,
            source_kind,
            applied_double_option,
        ): (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        ) = db
            .conn()
            .query_row(
                "SELECT clear_type, gauge_type, gauge_option, rule_mode, arrange, arrange_2p, device_type, replay_path, source_kind, applied_double_option FROM score_history",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                    ))
                },
            )
            .unwrap();

    assert_eq!(clear_type, "Normal");
    assert_eq!(gauge_type, "");
    assert_eq!(gauge_option, "");
    assert_eq!(rule_mode, "Dx");
    assert_eq!(arrange, "Random");
    assert_eq!(arrange_2p, "Mirror");
    assert_eq!(device_type, "controller");
    assert_eq!(replay_path, "");
    assert_eq!(source_kind, "Beatoraja");
    assert_eq!(applied_double_option, "Flip");
    let seed_scheme: String =
        db.conn().query_row("SELECT seed_scheme FROM score_history", [], |row| row.get(0)).unwrap();
    assert_eq!(seed_scheme, "beatoraja_24bit_v1");
    assert_eq!(db.recent_history(1, 0).unwrap()[0].source_kind, ScoreSourceKind::Beatoraja);
    assert_eq!(db.recent_history(1, 0).unwrap()[0].applied_double_option, DoubleOption::Flip);
}

#[test]
fn same_score_from_source_ignores_time_but_keeps_score_context_distinct() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut imported = record(20, ClearType::Normal);
    imported.source_kind = ScoreSourceKind::Beatoraja;
    imported.random_seed = Some(1234);
    imported.arrange = "Random".to_string();
    imported.arrange_2p = "Mirror".to_string();
    imported.applied_double_option = DoubleOption::Flip;
    imported.rule_mode = "Beatoraja".to_string();
    db.insert_score(&imported).unwrap();

    let mut same = imported.clone();
    same.played_at += 60;
    assert!(db.has_same_score_from_source(&same).unwrap());

    let mut different_source = same.clone();
    different_source.source_kind = ScoreSourceKind::Lr2Oraja;
    assert!(!db.has_same_score_from_source(&different_source).unwrap());

    let mut different_seed = same.clone();
    different_seed.random_seed = Some(1235);
    assert!(!db.has_same_score_from_source(&different_seed).unwrap());

    let mut different_arrange_2p = same.clone();
    different_arrange_2p.arrange_2p = "Random".to_string();
    assert!(!db.has_same_score_from_source(&different_arrange_2p).unwrap());

    let mut different_applied_double_option = same.clone();
    different_applied_double_option.applied_double_option = DoubleOption::Off;
    assert!(!db.has_same_score_from_source(&different_applied_double_option).unwrap());

    let mut different_judges = same;
    different_judges.score.judges.fast_empty_poor = 1;
    assert!(!db.has_same_score_from_source(&different_judges).unwrap());
}

#[test]
fn imported_score_reconciliation_updates_history_and_its_best_device() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut imported = record(20, ClearType::Normal);
    imported.source_kind = ScoreSourceKind::Beatoraja;
    let history_id = db.insert_score(&imported).unwrap();

    let mut corrected = imported.clone();
    corrected.played_at += 60;
    corrected.device_type = InputDeviceKind::Controller;
    assert_eq!(
        db.reconcile_imported_score_device_type(&corrected).unwrap(),
        ImportedScoreReconciliation::Corrected
    );
    assert!(db.has_same_score_from_source(&corrected).unwrap());
    assert_eq!(
        db.reconcile_imported_score_device_type(&corrected).unwrap(),
        ImportedScoreReconciliation::Unchanged
    );

    let history_device: String = db
        .conn()
        .query_row(
            "SELECT device_type FROM score_history WHERE id = ?1",
            params![history_id],
            |row| row.get(0),
        )
        .unwrap();
    let (best_history_id, best_device): (i64, String) = db
        .conn()
        .query_row("SELECT best_score_history_id, device_type FROM score_best", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(history_device, "controller");
    assert_eq!(best_history_id, history_id);
    assert_eq!(best_device, "controller");
}

#[test]
fn imported_score_reconciliation_does_not_change_local_best_device() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut imported = record(20, ClearType::Normal);
    imported.source_kind = ScoreSourceKind::Beatoraja;
    db.insert_score(&imported).unwrap();

    let local = record(40, ClearType::Normal);
    let local_history_id = db.insert_score(&local).unwrap();

    let mut corrected = imported.clone();
    corrected.device_type = InputDeviceKind::Controller;
    assert_eq!(
        db.reconcile_imported_score_device_type(&corrected).unwrap(),
        ImportedScoreReconciliation::Corrected
    );

    let (best_history_id, best_device): (i64, String) = db
        .conn()
        .query_row("SELECT best_score_history_id, device_type FROM score_best", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(best_history_id, local_history_id);
    assert_eq!(best_device, "keyboard");
}

#[test]
fn legacy_beatoraja_cleanup_removes_matching_local_history_and_rebuilds_aggregates() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut legacy = record(20, ClearType::Normal);
    legacy.playtime_seconds = 10;
    legacy.random_seed = Some(1234);
    let legacy_first_id = db.insert_score(&legacy).unwrap();
    let legacy_second_id = db.insert_score(&legacy).unwrap();

    let mut imported = legacy.clone();
    imported.playtime_seconds = 20;
    imported.arrange = "Random".to_string();
    imported.source_kind = ScoreSourceKind::Beatoraja;
    imported.device_type = InputDeviceKind::Controller;
    let imported_id = db.insert_score(&imported).unwrap();

    let mut ordinary = record(30, ClearType::Hard);
    ordinary.chart_sha256 = [8; 32];
    ordinary.playtime_seconds = 30;
    ordinary.played_at += 1;
    db.insert_score(&ordinary).unwrap();

    let plan = db.legacy_beatoraja_cleanup_plan().unwrap();
    assert_eq!(plan.legacy_history_ids, vec![legacy_first_id, legacy_second_id]);
    assert_eq!(plan.retained_beatoraja_history_ids, vec![imported_id]);

    let report = db.purge_legacy_beatoraja_imports(&plan).unwrap();
    assert_eq!(report.removed_legacy_history, 2);
    assert_eq!(report.retained_beatoraja_history, 1);
    assert!(db.legacy_beatoraja_cleanup_plan().unwrap().legacy_history_ids.is_empty());

    let history_ids: Vec<i64> = db
        .conn()
        .prepare("SELECT id FROM score_history ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    assert_eq!(history_ids, vec![imported_id, imported_id + 1]);

    let imported_best = db.best_scores_for_charts(&[key([7; 32])]).unwrap().pop().unwrap();
    assert_eq!(imported_best.play_count, 1);
    assert_eq!(imported_best.device_type, InputDeviceKind::Controller);
    let best_history_id: i64 = db
        .conn()
        .query_row(
            "SELECT best_score_history_id FROM score_best WHERE chart_sha256 = ?1",
            params![hash_to_hex(&[7; 32])],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(best_history_id, imported_id);

    let stats = db.player_stats().unwrap();
    assert_eq!(stats.play_count, 2);
    assert_eq!(stats.clear_count, 2);
    assert_eq!(stats.playtime_seconds, 70);
}

#[test]
fn same_source_duplicate_history_ids_match_the_cleanup_fingerprint() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut first = record(20, ClearType::Normal);
    first.random_seed = Some(1234);
    let first_id = db.insert_score(&first).unwrap();
    let duplicate_id = db.insert_score(&first).unwrap();

    let mut different = first;
    different.played_at += 1;
    let different_id = db.insert_score(&different).unwrap();

    assert_eq!(db.same_source_duplicate_history_ids(duplicate_id).unwrap(), vec![first_id]);
    assert!(db.same_source_duplicate_history_ids(different_id).unwrap().is_empty());
}

#[test]
fn best_score_keeps_higher_ex_score() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    db.insert_score(&record(20, ClearType::Normal)).unwrap();
    db.insert_score(&record(10, ClearType::Hard)).unwrap();
    db.insert_score(&record(30, ClearType::Easy)).unwrap();

    assert_eq!(db.best_ex_score(key([7; 32])).unwrap(), Some(30));
}

#[test]
fn score_best_updates_clear_lamp_independently_from_ex_score() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    db.insert_score(&record(40, ClearType::Normal)).unwrap();
    let mut hard = record(20, ClearType::Hard);
    hard.gauge_type = Some(GaugeType::Hard);
    hard.gauge_value = Some(12.0);
    db.insert_score(&hard).unwrap();

    let best = db.best_scores_for_charts(&[key([7; 32])]).unwrap().pop().unwrap();
    assert_eq!(best.ex_score, 40);
    assert_eq!(best.clear_type, "Hard");
    assert_eq!(best.gauge_type, "Hard");
    assert_eq!(best.gauge_value, Some(12.0));
}

#[test]
fn score_best_does_not_downgrade_clear_lamp_on_higher_ex_score() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut hard = record(20, ClearType::Hard);
    hard.gauge_type = Some(GaugeType::Hard);
    hard.gauge_value = Some(12.0);
    db.insert_score(&hard).unwrap();
    let mut normal = record(40, ClearType::Normal);
    normal.gauge_type = Some(GaugeType::Normal);
    normal.gauge_value = Some(82.0);
    db.insert_score(&normal).unwrap();

    let best = db.best_scores_for_charts(&[key([7; 32])]).unwrap().pop().unwrap();
    assert_eq!(best.ex_score, 40);
    assert_eq!(best.clear_type, "Hard");
    assert_eq!(best.gauge_type, "Hard");
    assert_eq!(best.gauge_value, Some(12.0));
}

#[test]
fn score_best_updates_only_max_combo_when_lower_ex_score_improves_combo() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut initial = record(40, ClearType::Normal);
    initial.score.judges.fast_bad = 3;
    initial.score.judges.fast_empty_poor = 2;
    db.insert_score(&initial).unwrap();

    let mut combo = record(20, ClearType::Easy);
    combo.score.max_combo = 50;
    combo.score.judges.fast_bad = 4;
    combo.score.judges.fast_empty_poor = 2;
    db.insert_score(&combo).unwrap();

    let best = db.best_scores_for_charts(&[key([7; 32])]).unwrap().pop().unwrap();
    assert_eq!(best.ex_score, 40);
    assert_eq!(best.clear_type, "Normal");
    assert_eq!(best.bp, 5);
    assert_eq!(best.cb, 3);
    assert_eq!(best.max_combo, 50);
    assert_eq!(best.judge_counts.pgreat, 20);
    assert_eq!(best.judge_counts.bad, 3);
    assert_eq!(best.judge_counts.empty_poor, 2);
}

#[test]
fn score_best_updates_only_bp_when_lower_ex_score_improves_bp() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut initial = record(40, ClearType::Normal);
    initial.score.judges.fast_bad = 3;
    initial.score.judges.fast_empty_poor = 3;
    db.insert_score(&initial).unwrap();

    let mut lower_bp = record(20, ClearType::Easy);
    lower_bp.score.judges.fast_bad = 3;
    lower_bp.score.judges.fast_empty_poor = 1;
    db.insert_score(&lower_bp).unwrap();

    let best = db.best_scores_for_charts(&[key([7; 32])]).unwrap().pop().unwrap();
    assert_eq!(best.ex_score, 40);
    assert_eq!(best.clear_type, "Normal");
    assert_eq!(best.bp, 4);
    assert_eq!(best.cb, 3);
    assert_eq!(best.max_combo, 20);
    assert_eq!(best.judge_counts.pgreat, 20);
    assert_eq!(best.judge_counts.bad, 3);
    assert_eq!(best.judge_counts.empty_poor, 3);
}

#[test]
fn score_best_updates_only_cb_when_lower_ex_score_improves_cb() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut initial = record(40, ClearType::Normal);
    initial.score.judges.fast_bad = 4;
    initial.score.judges.fast_empty_poor = 1;
    db.insert_score(&initial).unwrap();

    let mut lower_cb = record(20, ClearType::Easy);
    lower_cb.score.judges.fast_bad = 2;
    lower_cb.score.judges.fast_empty_poor = 3;
    db.insert_score(&lower_cb).unwrap();

    let best = db.best_scores_for_charts(&[key([7; 32])]).unwrap().pop().unwrap();
    assert_eq!(best.ex_score, 40);
    assert_eq!(best.clear_type, "Normal");
    assert_eq!(best.bp, 5);
    assert_eq!(best.cb, 2);
    assert_eq!(best.max_combo, 20);
    assert_eq!(best.judge_counts.pgreat, 20);
    assert_eq!(best.judge_counts.bad, 4);
    assert_eq!(best.judge_counts.empty_poor, 1);
}

#[test]
fn score_best_is_separate_per_double_option() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    db.insert_score(&record(20, ClearType::Normal)).unwrap();
    let mut battle = record(60, ClearType::Hard);
    battle.double_option = DoubleOptionScoreBucket::Battle;
    db.insert_score(&battle).unwrap();

    let off_key = key([7; 32]);
    let battle_key = ScoreKey::with_double_option(
        [7; 32],
        LnScorePolicy::ForceLn,
        DoubleOptionScoreBucket::Battle,
    );

    assert_eq!(db.best_ex_score(off_key).unwrap(), Some(20));
    assert_eq!(db.best_ex_score(battle_key).unwrap(), Some(60));

    let summaries = db.best_scores_for_charts(&[off_key, battle_key]).unwrap();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].double_option, DoubleOptionScoreBucket::Off);
    assert_eq!(summaries[0].ex_score, 20);
    assert_eq!(summaries[1].double_option, DoubleOptionScoreBucket::Battle);
    assert_eq!(summaries[1].ex_score, 60);
}

#[test]
fn score_best_is_separate_per_rule_mode() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut beatoraja = record(20, ClearType::Normal);
    beatoraja.rule_mode = RuleMode::Beatoraja.as_str().to_string();
    let mut dx = record(80, ClearType::Hard);
    dx.rule_mode = RuleMode::Dx.as_str().to_string();

    db.insert_score(&beatoraja).unwrap();
    db.insert_score(&dx).unwrap();

    let beatoraja_key = key([7; 32]).with_rule_mode(RuleMode::Beatoraja);
    let dx_key = key([7; 32]).with_rule_mode(RuleMode::Dx);

    assert_eq!(db.best_ex_score(beatoraja_key).unwrap(), Some(20));
    assert_eq!(db.best_ex_score(dx_key).unwrap(), Some(80));

    let summaries = db.best_scores_for_charts(&[beatoraja_key, dx_key]).unwrap();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].rule_mode, RuleMode::Beatoraja);
    assert_eq!(summaries[0].ex_score, 20);
    assert_eq!(summaries[1].rule_mode, RuleMode::Dx);
    assert_eq!(summaries[1].ex_score, 80);
}

#[test]
fn best_score_tiebreaks_by_lower_bp_then_lower_cb() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut high_bp = record(20, ClearType::Normal);
    high_bp.score.judges.fast_bad = 3;
    high_bp.score.judges.fast_empty_poor = 2;
    db.insert_score(&high_bp).unwrap();

    let mut lower_bp = record(20, ClearType::Normal);
    lower_bp.score.judges.fast_bad = 2;
    lower_bp.score.judges.fast_empty_poor = 2;
    db.insert_score(&lower_bp).unwrap();

    let best = db.best_scores_for_charts(&[key([7; 32])]).unwrap().pop().unwrap();
    assert_eq!(best.bp, 4);
    assert_eq!(best.cb, 2);

    let mut higher_cb = record(20, ClearType::Normal);
    higher_cb.score.judges.fast_bad = 4;
    higher_cb.score.judges.fast_empty_poor = 1;
    db.insert_score(&higher_cb).unwrap();

    let best = db.best_scores_for_charts(&[key([7; 32])]).unwrap().pop().unwrap();
    assert_eq!(best.bp, 4);
    assert_eq!(best.cb, 2);

    let mut lower_cb = record(20, ClearType::Normal);
    lower_cb.score.judges.fast_great = 6;
    lower_cb.score.judges.slow_good = 5;
    lower_cb.score.judges.fast_bad = 1;
    lower_cb.score.judges.fast_empty_poor = 3;
    db.insert_score(&lower_cb).unwrap();

    let best = db.best_scores_for_charts(&[key([7; 32])]).unwrap().pop().unwrap();
    assert_eq!(best.bp, 4);
    assert_eq!(best.cb, 1);
    assert_eq!(best.judge_counts.pgreat, 10);
    assert_eq!(best.judge_counts.great, 6);
    assert_eq!(best.judge_counts.good, 5);
    assert_eq!(best.judge_counts.bad, 1);
    assert_eq!(best.judge_counts.empty_poor, 3);
    assert_eq!(best.fast_slow_counts.fast_great, 6);
    assert_eq!(best.fast_slow_counts.slow_good, 5);
    assert_eq!(best.fast_slow_counts.fast_empty_poor, 3);
    assert_eq!(best.play_count, 4);
    assert_eq!(best.clear_count, 4);
}

#[test]
fn score_best_counts_every_play_but_only_clear_results() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    db.insert_score(&record(20, ClearType::Normal)).unwrap();
    let mut failed = record(10, ClearType::Failed);
    failed.played_at = 2;
    db.insert_score(&failed).unwrap();
    let mut clear = record(10, ClearType::Easy);
    clear.played_at = 3;
    db.insert_score(&clear).unwrap();

    let best = db.best_scores_for_charts(&[key([7; 32])]).unwrap().pop().unwrap();
    assert_eq!(best.ex_score, 20);
    assert_eq!(best.clear_type, "Normal");
    assert_eq!(best.play_count, 3);
    assert_eq!(best.clear_count, 2);
}

#[test]
fn score_best_keeps_independent_bp_cb_and_max_combo_records() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut best_score = record(20, ClearType::Normal);
    best_score.score.max_combo = 30;
    best_score.score.judges.fast_bad = 4;
    db.insert_score(&best_score).unwrap();

    let mut lower_score = record(10, ClearType::Failed);
    lower_score.played_at = 2;
    lower_score.score.max_combo = 80;
    lower_score.score.judges.fast_bad = 2;
    db.insert_score(&lower_score).unwrap();

    let best = db.best_scores_for_charts(&[key([7; 32])]).unwrap().pop().unwrap();
    assert_eq!(best.ex_score, 20);
    assert_eq!(best.clear_type, "Normal");
    assert_eq!(best.max_combo, 80);
    assert_eq!(best.bp, 2);
    assert_eq!(best.cb, 2);
    assert_eq!(best.play_count, 2);
    assert_eq!(best.clear_count, 1);
}

#[test]
fn failed_score_counts_unprocessed_notes_for_bp_and_cb_records() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut failed = record(0, ClearType::Failed);
    failed.total_notes = 100;
    db.insert_score(&failed).unwrap();

    let history = db.recent_history(10, 0).unwrap();
    assert_eq!(history[0].bp, 100);
    assert_eq!(history[0].cb, 100);

    let best = db.best_scores_for_charts(&[key([7; 32])]).unwrap().pop().unwrap();
    assert_eq!(best.clear_type, "Failed");
    assert_eq!(best.bp, 100);
    assert_eq!(best.cb, 100);
}

#[test]
fn score_best_is_separate_per_ln_policy() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut ln = record(20, ClearType::Normal);
    ln.ln_policy = LnScorePolicy::ForceLn;
    let mut cn = record(40, ClearType::Hard);
    cn.ln_policy = LnScorePolicy::ForceCn;

    db.insert_score(&ln).unwrap();
    db.insert_score(&cn).unwrap();

    assert_eq!(db.best_ex_score(key([7; 32])).unwrap(), Some(20));
    assert_eq!(db.best_ex_score(ScoreKey::new([7; 32], LnScorePolicy::ForceCn)).unwrap(), Some(40));
}

#[test]
fn player_info_is_created_and_display_name_updates() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let info = db.player_info().unwrap();
    assert_eq!(info.player_uuid.len(), 32);
    assert!(info.player_uuid.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(info.display_name, "");

    db.set_player_display_name("hyrorre", 1_700_000_099).unwrap();

    let info = db.player_info().unwrap();
    assert_eq!(info.display_name, "hyrorre");
    assert_eq!(info.updated_at, 1_700_000_099);
}

#[test]
fn player_stats_accumulates_profile_wide_scores() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut first = record(20, ClearType::Normal);
    first.played_at = 10;
    first.playtime_seconds = 120;
    first.score.judges.fast_great = 3;
    first.score.judges.slow_bad = 2;
    let mut failed = record(10, ClearType::Failed);
    failed.played_at = 20;
    failed.playtime_seconds = 30;
    failed.score.max_combo = 99;
    failed.score.judges.fast_empty_poor = 4;

    db.insert_score(&first).unwrap();
    db.insert_score(&failed).unwrap();

    let stats = db.player_stats().unwrap();
    assert_eq!(stats.play_count, 2);
    assert_eq!(stats.clear_count, 1);
    assert_eq!(stats.playtime_seconds, 150);
    assert_eq!(stats.max_combo, 99);
    assert_eq!(stats.fast_pgreat, 0);
    assert_eq!(stats.slow_pgreat, 15);
    assert_eq!(stats.fast_great, 3);
    assert_eq!(stats.slow_bad, 2);
    assert_eq!(stats.fast_empty_poor, 4);
    assert_eq!(stats.updated_at, 20);
}

#[test]
fn daily_player_stats_aggregates_only_local_history_inside_range() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut played = record(0, ClearType::Normal);
    played.played_at = 110;
    played.score = ScoreState::default();
    played.score.judges.fast_pgreat = 2;
    played.score.judges.slow_great = 3;
    played.score.judges.fast_good = 4;
    played.score.judges.slow_bad = 5;
    played.score.judges.fast_poor = 6;
    played.score.judges.slow_empty_poor = 7;
    db.insert_score(&played).unwrap();

    let mut failed = record(0, ClearType::Failed);
    failed.chart_sha256 = [8; 32];
    failed.played_at = 120;
    failed.score = ScoreState::default();
    failed.score.judges.slow_pgreat = 11;
    db.insert_score(&failed).unwrap();

    let mut outside = record(0, ClearType::Normal);
    outside.chart_sha256 = [9; 32];
    outside.played_at = 99;
    outside.score = ScoreState::default();
    outside.score.judges.fast_pgreat = 100;
    db.insert_score(&outside).unwrap();

    let mut imported = record(0, ClearType::Normal);
    imported.chart_sha256 = [10; 32];
    imported.played_at = 130;
    imported.source_kind = ScoreSourceKind::Beatoraja;
    imported.score = ScoreState::default();
    imported.score.judges.fast_pgreat = 200;
    db.insert_score(&imported).unwrap();

    let stats = db.daily_player_stats_between(100, 200).unwrap();
    assert_eq!(
        stats,
        DailyPlayerStats {
            play_count: 2,
            clear_count: 1,
            pgreat: 13,
            great: 3,
            good: 4,
            bad: 5,
            poor: 6,
            empty_poor: 7,
            score_update_count: 2,
            clear_update_count: 2,
            miss_count_update_count: 2,
        }
    );
    assert_eq!(
        db.daily_recent_chart_sha256s_between(100, 200, 10).unwrap(),
        vec![[8; 32], [7; 32]]
    );
    db.reset_daily_statistics(i64::MAX).unwrap();
    let (reset_start, reset_end) = db.current_daily_statistics_range(0).unwrap();
    assert_eq!(reset_start, reset_end);
    assert_eq!(db.current_local_day_player_stats().unwrap(), DailyPlayerStats::default());
}

#[test]
fn player_stats_migration_backfills_existing_history() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, &SCORE_MIGRATIONS[..7]).unwrap();
    conn.execute(
        "INSERT INTO score_history (
                chart_sha256, ln_policy, played_at, clear_type, gauge_type, gauge_value,
                total_notes, ex_score, bp, cb, max_combo,
                fast_pgreat, slow_pgreat, fast_great, slow_great,
                fast_good, slow_good, fast_bad, slow_bad,
                fast_poor, slow_poor, fast_empty_poor, slow_empty_poor,
                random_seed, gauge_option, rule_mode, assist_mask, autoplay,
                replay_path, ghost
            ) VALUES (
                ?1, 'ForceLn', 10, 'Normal', 'Normal', 80.0,
                10, 20, 1, 1, 8,
                1, 2, 3, 4,
                5, 6, 7, 8,
                9, 10, 11, 12,
                NULL, '', 'Beatoraja', 0, 0,
                '', ''
            )",
        params![hash_to_hex(&[1; 32])],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO score_history (
                chart_sha256, ln_policy, played_at, clear_type, gauge_type, gauge_value,
                total_notes, ex_score, bp, cb, max_combo,
                fast_pgreat, slow_pgreat, fast_great, slow_great,
                fast_good, slow_good, fast_bad, slow_bad,
                fast_poor, slow_poor, fast_empty_poor, slow_empty_poor,
                random_seed, gauge_option, rule_mode, assist_mask, autoplay,
                replay_path, ghost
            ) VALUES (
                ?1, 'ForceLn', 20, 'Failed', 'Normal', 20.0,
                10, 10, 5, 5, 12,
                2, 3, 4, 5,
                6, 7, 8, 9,
                10, 11, 12, 13,
                NULL, '', 'Beatoraja', 0, 0,
                '', ''
            )",
        params![hash_to_hex(&[2; 32])],
    )
    .unwrap();

    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let db = ScoreDatabase { conn };

    let stats = db.player_stats().unwrap();
    assert_eq!(stats.play_count, 2);
    assert_eq!(stats.clear_count, 1);
    assert_eq!(stats.playtime_seconds, 0);
    assert_eq!(stats.max_combo, 12);
    assert_eq!(stats.fast_pgreat, 3);
    assert_eq!(stats.slow_empty_poor, 25);
    assert_eq!(stats.updated_at, 20);
}

#[test]
fn score_history_migration_drops_history_ghost_and_sanitizes_course_links() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, &SCORE_MIGRATIONS[..18]).unwrap();
    conn.execute(
        "INSERT INTO score_history (
                chart_sha256, played_at, clear_type, gauge_type, gauge_value,
                total_notes, ex_score, bp, cb, max_combo,
                fast_pgreat, slow_pgreat, fast_great, slow_great,
                fast_good, slow_good, fast_bad, slow_bad,
                fast_poor, slow_poor, fast_empty_poor, slow_empty_poor,
                random_seed, gauge_option, assist_mask, autoplay,
                replay_path, ghost, course_score_id
            ) VALUES (
                ?1, 10, 'Normal', 'Normal', 80.0,
                10, 20, 1, 1, 8,
                1, 2, 3, 4,
                5, 6, 7, 8,
                9, 10, 11, 12,
                NULL, '', 0, 0,
                '', 'legacy-ghost', 9999
            )",
        params![hash_to_hex(&[1; 32])],
    )
    .unwrap();

    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();

    let columns: Vec<String> = conn
        .prepare("PRAGMA table_info(score_history)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(!columns.iter().any(|column| column == "ghost"));

    let course_score_id: Option<i64> =
        conn.query_row("SELECT course_score_id FROM score_history", [], |row| row.get(0)).unwrap();
    assert_eq!(course_score_id, None);

    let (source_kind, arrange_2p, applied_double_option): (String, String, String) = conn
        .query_row(
            "SELECT source_kind, arrange_2p, applied_double_option FROM score_history",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(source_kind, ScoreSourceKind::Local.as_str());
    assert_eq!(arrange_2p, "Normal");
    assert_eq!(applied_double_option, DoubleOption::Off.to_persistent_str());
}

#[test]
fn score_history_records_previous_best_snapshot() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut first = record(20, ClearType::Normal);
    first.played_at = 10;
    first.score.max_combo = 15;
    first.score.judges.fast_bad = 2;
    db.insert_score(&first).unwrap();

    let mut second = record(30, ClearType::Hard);
    second.played_at = 20;
    db.insert_score(&second).unwrap();

    let history = db.recent_history(10, 0).unwrap();
    assert_eq!(history[1].previous_best, None);
    assert_eq!(
        history[0].previous_best,
        Some(PreviousBestSnapshot {
            clear_type: "Normal".to_string(),
            ex_score: 20,
            max_combo: 15,
            bp: 2,
            cb: 2,
        })
    );
}

#[test]
fn score_history_previous_best_is_separate_per_ln_policy() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut ln = record(20, ClearType::Normal);
    ln.ln_policy = LnScorePolicy::ForceLn;
    ln.played_at = 10;
    let mut cn_first = record(40, ClearType::Hard);
    cn_first.ln_policy = LnScorePolicy::ForceCn;
    cn_first.played_at = 20;
    let mut cn_second = record(10, ClearType::Easy);
    cn_second.ln_policy = LnScorePolicy::ForceCn;
    cn_second.played_at = 30;

    db.insert_score(&ln).unwrap();
    db.insert_score(&cn_first).unwrap();
    db.insert_score(&cn_second).unwrap();

    let history = db.recent_history(10, 0).unwrap();
    assert_eq!(
        history[0].previous_best.as_ref().map(|best| (best.clear_type.as_str(), best.ex_score)),
        Some(("Hard", 40))
    );
    assert_eq!(history[1].previous_best, None);
    assert_eq!(history[2].previous_best, None);
}

#[test]
fn beatoraja_ghost_round_trips_as_gzip_urlsafe_base64() {
    let ghost = vec![0, 1, 2, 3, 4];

    let encoded = encode_beatoraja_ghost(&ghost).unwrap();
    let decoded = decode_beatoraja_ghost(&encoded, ghost.len() as u32).unwrap();

    assert_eq!(decoded, ghost);
}

#[test]
fn insert_score_persists_best_ghost_for_current_best() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    db.insert_score(&record(20, ClearType::Normal)).unwrap();
    db.insert_score(&record(10, ClearType::Hard)).unwrap();

    assert_eq!(db.best_ghost(key([7; 32]), 10).unwrap(), Some(vec![0; 10]));
}

#[test]
fn class_gauge_types_round_trip_via_score_history_and_best() {
    // 段位ゲージで終わったプレイが score_history / score_best 経由で
    // `"Class" / "ExClass" / "ExHardClass"` の文字列として正しく永続化・
    // 復元されることを担保する。
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let cases = [
        ([10u8; 32], GaugeType::Class, "Class"),
        ([11u8; 32], GaugeType::ExClass, "ExClass"),
        ([12u8; 32], GaugeType::ExHardClass, "ExHardClass"),
    ];

    // ex_score は (sha[0], 段位ごと) で順に上げ、score_best が上書きされて
    // 残ることを保証する。
    for (i, (sha, gauge, _)) in cases.iter().enumerate() {
        let mut rec = record(20 + i as u32 * 10, ClearType::Hard);
        rec.chart_sha256 = *sha;
        rec.gauge_type = Some(*gauge);
        rec.gauge_value = Some(42.0 + i as f32);
        db.insert_score(&rec).unwrap();
    }

    // score_history: GaugeType::as_str() の文字列で素直に入る。
    let history = db.recent_history(10, 0).unwrap();
    let mut history_map: std::collections::HashMap<[u8; 32], String> =
        history.into_iter().map(|entry| (entry.chart_sha256, entry.gauge_type)).collect();
    for (sha, _, expected) in &cases {
        assert_eq!(history_map.remove(sha).as_deref(), Some(*expected), "history {sha:?}");
    }

    // score_best: 同じく文字列でラウンドトリップ、gauge_value も保持される。
    let keys: Vec<_> = cases.iter().map(|(sha, _, _)| key(*sha)).collect();
    let best = db.best_scores_for_charts(&keys).unwrap();
    assert_eq!(best.len(), 3);
    let mut by_sha: std::collections::HashMap<_, _> =
        best.into_iter().map(|s| (s.chart_sha256, s)).collect();
    for (i, (sha, _, expected_label)) in cases.iter().enumerate() {
        let summary = by_sha.remove(sha).expect("best entry exists");
        assert_eq!(summary.gauge_type, *expected_label);
        assert_eq!(summary.gauge_value, Some(42.0 + i as f32));
    }
}

#[test]
fn gauge_type_str_matches_enum_display_for_class_gauges() {
    assert_eq!(gauge_type_str(Some(GaugeType::Class)), "Class");
    assert_eq!(gauge_type_str(Some(GaugeType::ExClass)), "ExClass");
    assert_eq!(gauge_type_str(Some(GaugeType::ExHardClass)), "ExHardClass");
    // sanity: 非段位ゲージも従来通り。
    assert_eq!(gauge_type_str(Some(GaugeType::Normal)), "Normal");
    assert_eq!(gauge_type_str(None), "");
}

#[test]
fn best_scores_for_charts_returns_existing_scores() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };
    let mut first = record(20, ClearType::Normal);
    first.chart_sha256 = [1; 32];
    first.replay_path = "replay/one.bzr".to_string();
    let mut second = record(10, ClearType::Easy);
    second.chart_sha256 = [2; 32];
    second.gauge_type = None;

    db.insert_score(&first).unwrap();
    db.insert_score(&second).unwrap();

    let scores = db.best_scores_for_charts(&[key([2; 32]), key([3; 32]), key([1; 32])]).unwrap();

    assert_eq!(scores.len(), 2);
    assert_eq!(scores[0].chart_sha256, [2; 32]);
    assert_eq!(scores[0].gauge_type, "");
    assert_eq!(scores[1].chart_sha256, [1; 32]);
    assert_eq!(scores[1].replay_path, "replay/one.bzr");
}

fn sample_slot(slot: u8, ex_score: u32) -> ReplaySlotRecord {
    ReplaySlotRecord {
        chart_sha256: [1; 32],
        ln_policy: LnScorePolicy::ForceLn,
        double_option: DoubleOptionScoreBucket::Off,
        rule_mode: RuleMode::Beatoraja,
        slot,
        rule: ReplaySlotRule::Always,
        replay_path: format!("replay/{slot}.toml"),
        played_at: 1_700_000_000 + slot as i64,
        ex_score,
        bp: 0,
        cb: 0,
        max_combo: ex_score,
        clear_rank: ClearType::Normal as u8,
    }
}

#[test]
fn replay_slots_for_charts_reports_slot_presence_from_new_table() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };
    db.upsert_replay_slot(&sample_slot(0, 10)).unwrap();
    db.upsert_replay_slot(&sample_slot(2, 30)).unwrap();

    let slots = db.replay_slots_for_charts(&[key([2; 32]), key([1; 32])]).unwrap();

    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].chart_sha256, [1; 32]);
    assert_eq!(slots[0].replay_slots, [true, false, true, false]);
}

#[test]
fn select_score_lookups_batch_more_keys_than_one_sqlite_variable_chunk() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };

    let mut first = record(20, ClearType::Normal);
    first.chart_sha256 = [1; 32];
    let mut second = record(10, ClearType::Easy);
    second.chart_sha256 = [2; 32];
    db.insert_score(&first).unwrap();
    db.insert_score(&second).unwrap();
    db.upsert_replay_slot(&sample_slot(0, 20)).unwrap();
    db.upsert_replay_slot(&sample_slot(2, 20)).unwrap();
    let mut second_slot = sample_slot(1, 10);
    second_slot.chart_sha256 = [2; 32];
    db.upsert_replay_slot(&second_slot).unwrap();

    let mut keys = (0..SCORE_KEY_LOOKUP_BATCH_SIZE * 2 + 1)
        .map(|index| {
            let mut sha = [0; 32];
            sha[..8].copy_from_slice(&(index as u64 + 100).to_le_bytes());
            key(sha)
        })
        .collect::<Vec<_>>();
    keys.extend([key([2; 32]), key([1; 32]), key([1; 32])]);

    let scores = db.best_scores_for_charts(&keys).unwrap();
    let slots = db.replay_slots_for_charts(&keys).unwrap();

    assert_eq!(
        scores.iter().map(|score| score.chart_sha256).collect::<Vec<_>>(),
        [[2; 32], [1; 32], [1; 32]]
    );
    assert_eq!(slots.len(), 3);
    assert_eq!(slots[0].chart_sha256, [2; 32]);
    assert_eq!(slots[0].replay_slots, [false, true, false, false]);
    assert_eq!(slots[1].chart_sha256, [1; 32]);
    assert_eq!(slots[1].replay_slots, [true, false, true, false]);
    assert_eq!(slots[2], slots[1]);
}

#[test]
fn upsert_replay_slot_overwrites_same_slot() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };
    db.upsert_replay_slot(&sample_slot(0, 10)).unwrap();
    let mut updated = sample_slot(0, 99);
    updated.replay_path = "replay/updated.toml".to_string();
    db.upsert_replay_slot(&updated).unwrap();

    let record = db.replay_slot(key([1; 32]), 0).unwrap().unwrap();
    assert_eq!(record.ex_score, 99);
    assert_eq!(record.replay_path, "replay/updated.toml");
}

#[test]
fn replay_slots_for_chart_returns_all_four_slots() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };
    db.upsert_replay_slot(&sample_slot(0, 10)).unwrap();
    db.upsert_replay_slot(&sample_slot(3, 30)).unwrap();

    let slots = db.replay_slots_for_chart(key([1; 32])).unwrap();

    assert!(slots[0].is_some());
    assert!(slots[1].is_none());
    assert!(slots[2].is_none());
    assert_eq!(slots[3].as_ref().unwrap().ex_score, 30);
}

#[test]
fn replay_slots_are_separate_per_ln_policy() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };
    let mut ln = sample_slot(0, 10);
    ln.ln_policy = LnScorePolicy::ForceLn;
    let mut cn = sample_slot(0, 99);
    cn.ln_policy = LnScorePolicy::ForceCn;

    db.upsert_replay_slot(&ln).unwrap();
    db.upsert_replay_slot(&cn).unwrap();

    let ln_slot = db.replay_slot(key([1; 32]), 0).unwrap().unwrap();
    let cn_slot =
        db.replay_slot(ScoreKey::new([1; 32], LnScorePolicy::ForceCn), 0).unwrap().unwrap();
    assert_eq!(ln_slot.ex_score, 10);
    assert_eq!(cn_slot.ex_score, 99);
}

#[test]
fn replay_slots_are_separate_per_rule_mode() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase { conn };
    let mut beatoraja = sample_slot(0, 10);
    beatoraja.rule_mode = RuleMode::Beatoraja;
    let mut dx = sample_slot(0, 99);
    dx.rule_mode = RuleMode::Dx;

    db.upsert_replay_slot(&beatoraja).unwrap();
    db.upsert_replay_slot(&dx).unwrap();

    let beatoraja_slot =
        db.replay_slot(key([1; 32]).with_rule_mode(RuleMode::Beatoraja), 0).unwrap().unwrap();
    let dx_slot = db.replay_slot(key([1; 32]).with_rule_mode(RuleMode::Dx), 0).unwrap().unwrap();
    assert_eq!(beatoraja_slot.ex_score, 10);
    assert_eq!(dx_slot.ex_score, 99);
}

#[test]
fn score_record_can_be_built_from_play_result() {
    let result = PlayResult {
        chart_sha256: [9; 32],
        clear_type: ClearType::Normal,
        gauge_type: GaugeType::Hard,
        gauge_value: 76.5,
        total_notes: 1,
        score: score_with_ex_score(2),
        autoplay: true,
    };

    let record = ScoreRecord::from_play_result(
        &result,
        ScoreRecordMetadata::new(
            LnScorePolicy::ForceCn,
            DoubleOptionScoreBucket::Battle,
            1_700_000_040,
            Some(123),
            "Normal",
            "Hard",
            "Lr2Oraja",
            0,
            InputDeviceKind::Controller,
            "",
        )
        .with_arrange_2p("Mirror")
        .with_source_kind(ScoreSourceKind::Lr2Oraja),
    );

    assert_eq!(record.chart_sha256, [9; 32]);
    assert_eq!(record.ln_policy, LnScorePolicy::ForceCn);
    assert_eq!(record.double_option, DoubleOptionScoreBucket::Battle);
    assert_eq!(record.played_at, 1_700_000_040);
    assert_eq!(record.clear_type, ClearType::Normal);
    assert_eq!(record.gauge_type, Some(GaugeType::Hard));
    assert_eq!(record.gauge_value, Some(76.5));
    assert_eq!(record.device_type, InputDeviceKind::Controller);
    assert_eq!(record.arrange_2p, "Mirror");
    assert_eq!(record.source_kind, ScoreSourceKind::Lr2Oraja);
    assert_eq!(record.score.ex_score(), 2);
    assert!(record.autoplay);
    assert_eq!(record.gauge_option, "Hard");
    assert_eq!(record.rule_mode, "Lr2Oraja");
    assert_eq!(record.replay_path, "");
}

#[test]
fn tag_score_history_with_course_updates_only_given_rows() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase::from_connection(conn);

    let mut r1 = record(20, ClearType::Normal);
    r1.chart_sha256 = [1; 32];
    let mut r2 = record(30, ClearType::Easy);
    r2.chart_sha256 = [2; 32];
    let mut r3 = record(10, ClearType::Failed);
    r3.chart_sha256 = [3; 32];
    let id1 = db.insert_score(&r1).unwrap();
    let id2 = db.insert_score(&r2).unwrap();
    let id3 = db.insert_score(&r3).unwrap();

    let course_score_id = insert_test_course_score(&mut db, "course-a");

    // Tag the first two with a real course score, leave r3 untouched.
    let updated = db.tag_score_history_with_course(&[id1, id2], course_score_id).unwrap();
    assert_eq!(updated, 2);

    let rows: Vec<(i64, Option<i64>)> = db
        .conn()
        .prepare("SELECT id, course_score_id FROM score_history ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(rows, vec![(id1, Some(course_score_id)), (id2, Some(course_score_id)), (id3, None)]);
}

#[test]
fn tag_score_history_with_course_no_op_on_empty_list() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase::from_connection(conn);
    assert_eq!(db.tag_score_history_with_course(&[], 1).unwrap(), 0);
}

#[test]
fn recent_history_returns_newest_scores_first() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase::from_connection(conn);
    let mut older = record(20, ClearType::Normal);
    older.played_at = 1;
    older.chart_sha256 = [1; 32];
    let mut newer = record(10, ClearType::Easy);
    newer.played_at = 2;
    newer.chart_sha256 = [2; 32];
    newer.autoplay = true;

    db.insert_score(&older).unwrap();
    db.insert_score(&newer).unwrap();

    let history = db.recent_history(10, 0).unwrap();

    assert_eq!(history.len(), 2);
    assert_eq!(history[0].chart_sha256, [2; 32]);
    assert_eq!(history[0].played_at, 2);
    assert!(history[0].autoplay);
    assert_eq!(history[1].chart_sha256, [1; 32]);
}

#[test]
fn recent_history_exposes_course_score_id_when_tagged() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase::from_connection(conn);

    let mut solo = record(20, ClearType::Normal);
    solo.chart_sha256 = [1; 32];
    let solo_id = db.insert_score(&solo).unwrap();

    let mut course_play = record(30, ClearType::Easy);
    course_play.chart_sha256 = [2; 32];
    let course_play_id = db.insert_score(&course_play).unwrap();

    let course_score_id = insert_test_course_score(&mut db, "course-a");

    // Tag the course-attempt row only.
    db.tag_score_history_with_course(&[course_play_id], course_score_id).unwrap();

    let history = db.recent_history(10, 0).unwrap();
    let by_id: std::collections::HashMap<i64, &ScoreHistoryEntry> =
        history.iter().map(|h| (h.id, h)).collect();
    assert_eq!(by_id.get(&solo_id).unwrap().course_score_id, None);
    assert_eq!(by_id.get(&course_play_id).unwrap().course_score_id, Some(course_score_id));
}

#[test]
fn deleting_course_score_nulls_history_course_link() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase::from_connection(conn);

    let mut course_play = record(30, ClearType::Easy);
    course_play.chart_sha256 = [2; 32];
    let course_play_id = db.insert_score(&course_play).unwrap();
    let course_score_id = insert_test_course_score(&mut db, "course-a");
    db.tag_score_history_with_course(&[course_play_id], course_score_id).unwrap();

    db.conn_mut()
        .execute("DELETE FROM course_scores WHERE id = ?1", params![course_score_id])
        .unwrap();

    let course_score_id_after_delete: Option<i64> = db
        .conn()
        .query_row(
            "SELECT course_score_id FROM score_history WHERE id = ?1",
            params![course_play_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(course_score_id_after_delete, None);
}

#[test]
fn score_db_migrations_do_not_leave_ir_tables() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
                 FROM sqlite_master
                 WHERE type = 'table' AND name LIKE 'ir_%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn score_db_migrations_drop_redundant_prefix_indexes() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();

    for index in
        ["idx_score_best_chart", "idx_replay_slots_chart", "idx_score_course_replay_slots_hash"]
    {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                params![index],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "{index} should be covered by a PRIMARY KEY prefix");
    }
}

#[test]
fn chart_sha256_columns_are_lowercase_hex_text() {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut db = ScoreDatabase::from_connection(conn);
    db.insert_score(&record(20, ClearType::Normal)).unwrap();

    let (hist_typeof, best_typeof, best_hex): (String, String, String) = db
        .conn()
        .query_row(
            "SELECT
                    (SELECT typeof(chart_sha256) FROM score_history LIMIT 1),
                    (SELECT typeof(chart_sha256) FROM score_best LIMIT 1),
                    (SELECT chart_sha256 FROM score_best LIMIT 1)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(hist_typeof, "text");
    assert_eq!(best_typeof, "text");
    assert_eq!(best_hex.len(), 64);
    assert!(best_hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}
