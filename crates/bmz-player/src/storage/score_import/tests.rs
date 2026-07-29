use std::collections::HashMap;
use std::path::Path;

use bmz_chart::hash::compute_chart_identity;
use bmz_chart::model::{ChartMetadata, LongNotePair, LongNoteStyle, PlayableChart};
use bmz_core::ids::NoteId;
use bmz_core::lane::Lane;
use bmz_core::time::{ChartTick, TimeUs};
use rusqlite::params;

use super::*;
use crate::select_options::DoubleOptionScoreBucket;
use crate::storage::common::hash_to_hex;
use crate::storage::library_db::{ChartImportRecord, LibraryDatabase};
use crate::storage::migration::{LIBRARY_MIGRATIONS, SCORE_MIGRATIONS, run_migrations};
use crate::storage::score_db::ScoreKey;
use bmz_gameplay::rule::RuleMode;

#[test]
fn lr2_import_maps_md5_and_clear_type() {
    let (mut library_db, mut score_db, sha256, md5) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    create_lr2_source(&source, &md5);

    let report = import_lr2_scores_with_device_type(
        &source,
        ScoreImportKind::Lr2,
        &mut library_db,
        &mut score_db,
        1_700_000_000,
        InputDeviceKind::Controller,
    )
    .unwrap();

    assert_eq!(report.imported, 1);
    let best = score_db
        .best_scores_for_charts(&[
            ScoreKey::new(sha256, LnScorePolicy::ForceLn).with_rule_mode(RuleMode::Lr2Oraja)
        ])
        .unwrap();
    assert_eq!(best[0].clear_type, "Hard");
    assert_eq!(best[0].ex_score, 222);
    assert_eq!(best[0].ln_policy, LnScorePolicy::ForceLn);
    assert_eq!(best[0].device_type, InputDeviceKind::Controller);
}

#[test]
fn lr2_import_preserves_supported_op_best_arrangements_and_flip() {
    let (mut library_db, mut score_db, _, md5) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    create_lr2_source(&source, &md5);
    // Gauge=1 (ignored), 1P=RANDOM, 2P=S-RANDOM, DP=FLIP.
    source.execute("UPDATE score SET op_best = 1321", []).unwrap();

    let report = import_lr2_scores(
        &source,
        ScoreImportKind::Lr2,
        &mut library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();

    assert_eq!(report.imported, 1);
    let options: (String, String, String, String) = score_db
        .conn()
        .query_row(
            "SELECT arrange, arrange_2p, double_option, applied_double_option
                 FROM score_history",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        options,
        ("Random".to_string(), "SRandom".to_string(), "Off".to_string(), "Flip".to_string(),)
    );
}

#[test]
fn lr2_import_skips_unsupported_op_best_arrangements() {
    for op_best in [40, 500, 60, 2_000] {
        let (mut library_db, mut score_db, _, md5) = open_test_databases();
        let source = Connection::open_in_memory().unwrap();
        create_lr2_source(&source, &md5);
        source.execute("UPDATE score SET op_best = ?1", [op_best]).unwrap();

        let report = import_lr2_scores(
            &source,
            ScoreImportKind::Lr2,
            &mut library_db,
            &mut score_db,
            1_700_000_000,
        )
        .unwrap();

        assert_eq!(report.scanned, 1, "op_best={op_best}");
        assert_eq!(report.skipped, 1, "op_best={op_best}");
        assert_eq!(report.imported, 0, "op_best={op_best}");
        assert_eq!(report.failed, 0, "op_best={op_best}");
    }
}

#[test]
fn beatoraja_import_preserves_fast_slow_counts_and_current_schema_fields() {
    let (library_db, mut score_db, sha256, _) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    create_beatoraja_source(&source, &sha256, 1_700_000_001_000, 0);
    // 1P=ROTATE, 2P=MIRROR, double=FLIP.  FLIP shares the Off score
    // bucket, but is retained as the applied option in history.
    source.execute("UPDATE score SET option = 113", []).unwrap();

    let report = import_beatoraja_scores_with_device_type(
        &source,
        ScoreImportKind::Beatoraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
        InputDeviceKind::Controller,
    )
    .unwrap();

    assert_eq!(report.imported, 1);
    type ScoreFields = (String, u32, u32, u32, String, String, String, String);
    type ContextFields = (String, String, String, String, i64);
    let row: (ScoreFields, ContextFields) = score_db
        .conn()
        .query_row(
            "SELECT clear_type, fast_pgreat, slow_pgreat, slow_empty_poor,
                    rule_mode, ln_policy, double_option, applied_double_option, arrange, arrange_2p,
                    device_type, source_kind, played_at
                 FROM score_history",
            [],
            |row| {
                Ok((
                    (
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ),
                    (row.get(8)?, row.get(9)?, row.get(10)?, row.get(11)?, row.get(12)?),
                ))
            },
        )
        .unwrap();
    assert_eq!(
        row,
        (
            (
                "ExHard".to_string(),
                100,
                10,
                1,
                "Beatoraja".to_string(),
                "ForceLn".to_string(),
                "Off".to_string(),
                "Flip".to_string(),
            ),
            (
                "RRandom".to_string(),
                "Mirror".to_string(),
                "controller".to_string(),
                "Beatoraja".to_string(),
                1_700_000_001,
            ),
        )
    );
}

#[test]
fn beatoraja_option_maps_both_arrange_slots_and_double_bucket() {
    assert_eq!(
        beatoraja_arrange_options(213, "7K"),
        (ArrangeOption::RRandom, ArrangeOption::Mirror)
    );
    assert_eq!(beatoraja_double_option(213).score_bucket(), DoubleOptionScoreBucket::Battle);
    assert_eq!(beatoraja_double_option(100), DoubleOption::Flip);
    // FLIP is intentionally in the existing Off score bucket, while its
    // actual option is retained by `ScoreRecord::applied_double_option`.
    assert_eq!(beatoraja_double_option(100).score_bucket(), DoubleOptionScoreBucket::Off);
    assert_eq!(beatoraja_double_option(200), DoubleOption::Battle);
    assert_eq!(
        beatoraja_double_option(300).score_bucket(),
        DoubleOptionScoreBucket::BattleAutoScratch
    );
    assert_eq!(beatoraja_arrange_options(7, "9K"), (ArrangeOption::Normal, ArrangeOption::Normal));
    assert_eq!(beatoraja_arrange_options(8, "9K"), (ArrangeOption::Random, ArrangeOption::Normal));
    assert_eq!(beatoraja_arrange_options(9, "9K"), (ArrangeOption::SRandom, ArrangeOption::Normal));
}

#[test]
fn beatoraja_import_skips_identical_scores_from_same_source_kind() {
    let (library_db, mut score_db, sha256, _) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    create_beatoraja_source(&source, &sha256, 1_700_000_001_000, 0);

    let first = import_beatoraja_scores(
        &source,
        ScoreImportKind::Beatoraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();
    assert_eq!(first.imported, 1);

    // A timestamp change alone does not make an external score a new play.
    source.execute("UPDATE score SET date = ?1", params![1_700_000_002_000_i64]).unwrap();
    let duplicate = import_beatoraja_scores(
        &source,
        ScoreImportKind::Beatoraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();
    assert_eq!(duplicate.imported, 0);
    assert_eq!(duplicate.skipped, 1);

    // Provenance is part of the duplicate key: the same score imported from
    // LR2oraja remains a separate history entry.
    let distinct_source = import_beatoraja_scores(
        &source,
        ScoreImportKind::Lr2Oraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();
    assert_eq!(distinct_source.imported, 1);
    let source_kinds: Vec<String> = score_db
        .conn()
        .prepare("SELECT source_kind FROM score_history ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    assert_eq!(source_kinds, vec!["Beatoraja", "Lr2Oraja"]);
}

#[test]
fn beatoraja_reimport_corrects_device_type_without_adding_history() {
    let (library_db, mut score_db, sha256, _) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    create_beatoraja_source(&source, &sha256, 1_700_000_001_000, 0);

    let first = import_beatoraja_scores_with_device_type(
        &source,
        ScoreImportKind::Beatoraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
        InputDeviceKind::Keyboard,
    )
    .unwrap();
    assert_eq!(first.imported, 1);

    let corrected = import_beatoraja_scores_with_device_type(
        &source,
        ScoreImportKind::Beatoraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
        InputDeviceKind::Controller,
    )
    .unwrap();
    assert_eq!(corrected.imported, 0);
    assert_eq!(corrected.corrected, 1);
    assert_eq!(corrected.skipped, 0);

    let history_count: u32 = score_db
        .conn()
        .query_row("SELECT COUNT(*) FROM score_history", [], |row| row.get(0))
        .unwrap();
    let history_device: String = score_db
        .conn()
        .query_row("SELECT device_type FROM score_history", [], |row| row.get(0))
        .unwrap();
    let best = score_db
        .best_scores_for_charts(&[ScoreKey::new(sha256, LnScorePolicy::ForceLn)])
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(history_count, 1);
    assert_eq!(history_device, "controller");
    assert_eq!(best.device_type, InputDeviceKind::Controller);
}

#[test]
fn lr2oraja_dx_import_sets_dx_rule_mode() {
    let (library_db, mut score_db, sha256, _) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    create_beatoraja_source(&source, &sha256, 1_700_000_001_000, 0);

    import_beatoraja_scores(
        &source,
        ScoreImportKind::Lr2OrajaDx,
        &library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();

    let rule_mode: String = score_db
        .conn()
        .query_row("SELECT rule_mode FROM score_history", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rule_mode, "Dx");
}

#[test]
fn lr2oraja_import_uses_beatoraja_schema_and_rule_mode() {
    let (library_db, mut score_db, sha256, _) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    create_beatoraja_source(&source, &sha256, 1_700_000_001_000, 0);

    let report = import_beatoraja_scores(
        &source,
        ScoreImportKind::Lr2Oraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();
    assert_eq!(report.imported, 1);
    let rule_mode: String = score_db
        .conn()
        .query_row("SELECT rule_mode FROM score_history", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rule_mode, "Lr2Oraja");
}

#[test]
fn beatoraja_import_mode_cn_on_undefined_ln_sets_force_cn() {
    clear_test_import_charts();
    let (mut library_db, mut score_db, sha256, _) =
        open_test_databases_with_chart(undefined_ln_chart(2, 2));
    set_test_import_chart(sha256, undefined_ln_chart(2, 2));
    let source = Connection::open_in_memory().unwrap();
    // ForceCn expected = 2 base + 2 CN ends = 4
    create_beatoraja_source_with_score(
        &source,
        &hash_to_hex(&sha256),
        1_700_000_001_000,
        1,
        7,
        4,
        4,
        4,
    );

    let report = import_beatoraja_scores(
        &source,
        ScoreImportKind::Beatoraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();
    assert_eq!(report.imported, 1);
    let ln_policy: String = score_db
        .conn()
        .query_row("SELECT ln_policy FROM score_history", [], |row| row.get(0))
        .unwrap();
    assert_eq!(ln_policy, "ForceCn");
    clear_test_import_charts();
    let _ = &mut library_db;
}

#[test]
fn beatoraja_import_falls_back_to_force_ln_when_only_ln_expected_matches() {
    clear_test_import_charts();
    let (library_db, mut score_db, sha256, _) =
        open_test_databases_with_chart(undefined_ln_chart(2, 2));
    set_test_import_chart(sha256, undefined_ln_chart(2, 2));
    let source = Connection::open_in_memory().unwrap();
    // mode=1 -> ForceCn expects 4, but source notes=2 match ForceLn only.
    create_beatoraja_source_with_score(
        &source,
        &hash_to_hex(&sha256),
        1_700_000_001_000,
        1,
        7,
        2,
        2,
        2,
    );

    let report = import_beatoraja_scores(
        &source,
        ScoreImportKind::Beatoraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();
    assert_eq!(report.imported, 1);
    let ln_policy: String = score_db
        .conn()
        .query_row("SELECT ln_policy FROM score_history", [], |row| row.get(0))
        .unwrap();
    assert_eq!(ln_policy, "ForceLn");
    clear_test_import_charts();
}

#[test]
fn beatoraja_import_fails_when_source_note_count_mismatches_all_policies() {
    clear_test_import_charts();
    let (library_db, mut score_db, sha256, _) =
        open_test_databases_with_chart(undefined_ln_chart(2, 2));
    set_test_import_chart(sha256, undefined_ln_chart(2, 2));
    let source = Connection::open_in_memory().unwrap();
    create_beatoraja_source_with_score(
        &source,
        &hash_to_hex(&sha256),
        1_700_000_001_000,
        1,
        7,
        3,
        3,
        3,
    );

    let report = import_beatoraja_scores(
        &source,
        ScoreImportKind::Beatoraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();
    assert_eq!(report.failed, 1);
    assert_eq!(report.imported, 0);
    clear_test_import_charts();
}

#[test]
fn beatoraja_import_accepts_failed_row_with_fewer_judgements() {
    clear_test_import_charts();
    let (library_db, mut score_db, sha256, _) =
        open_test_databases_with_chart(undefined_ln_chart(2, 2));
    set_test_import_chart(sha256, undefined_ln_chart(2, 2));
    let source = Connection::open_in_memory().unwrap();
    create_beatoraja_source_with_score(
        &source,
        &hash_to_hex(&sha256),
        1_700_000_001_000,
        1,
        1,
        4,
        3,
        3,
    );

    let report = import_beatoraja_scores(
        &source,
        ScoreImportKind::Beatoraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();
    assert_eq!(report.imported, 1);
    assert_eq!(report.failed, 0);
    let clear_type: String = score_db
        .conn()
        .query_row("SELECT clear_type FROM score_history", [], |row| row.get(0))
        .unwrap();
    assert_eq!(clear_type, "Failed");
    clear_test_import_charts();
}

#[test]
fn beatoraja_import_accepts_more_judgements_than_source_notes() {
    let (library_db, mut score_db, sha256, _) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    create_beatoraja_source_with_score(
        &source,
        &hash_to_hex(&sha256),
        1_700_000_001_000,
        0,
        7,
        128,
        129,
        80,
    );

    let report = import_beatoraja_scores(
        &source,
        ScoreImportKind::Beatoraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();
    assert_eq!(report.imported, 1);
    assert_eq!(report.failed, 0);
}

#[test]
fn lr2_import_accepts_empty_poor_in_judge_total() {
    let (mut library_db, mut score_db, _, md5) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    create_lr2_source_with_score(&source, &hash_to_hex(&md5), 128, 64, 100, 22, 3, 2, 20);

    let report = import_lr2_scores(
        &source,
        ScoreImportKind::Lr2,
        &mut library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();
    assert_eq!(report.failed, 0);
    assert_eq!(report.imported, 1);
}

#[test]
fn lr2_import_fails_when_source_note_count_mismatches() {
    let (mut library_db, mut score_db, _, md5) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    create_lr2_source_with_score(&source, &hash_to_hex(&md5), 127, 64, 100, 20, 3, 2, 1);

    let report = import_lr2_scores(
        &source,
        ScoreImportKind::Lr2,
        &mut library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();
    assert_eq!(report.failed, 1);
    assert_eq!(report.imported, 0);
}

#[test]
fn import_score_summary_sanity_checks_ex_score_and_combo() {
    assert!(score_summary_is_sane(128, 128, 256));
    assert!(!score_summary_is_sane(0, 0, 0));
    assert!(!score_summary_is_sane(128, 129, 256));
    assert!(!score_summary_is_sane(128, 128, 257));
}

#[test]
fn lr2_import_skips_unregistered_md5() {
    let (mut library_db, mut score_db, _, _) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    create_lr2_source(&source, &[9; 16]);

    let report = import_lr2_scores(
        &source,
        ScoreImportKind::Lr2,
        &mut library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();

    assert_eq!(report.scanned, 1);
    assert_eq!(report.skipped, 1);
    assert_eq!(report.imported, 0);
}

#[test]
fn beatoraja_import_skips_course_scores_without_failing() {
    let (library_db, mut score_db, _, _) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    // A 4-song course key: four 64-char hashes concatenated (256 chars).
    let course_key = "a".repeat(256);
    create_beatoraja_source_with_sha256(&source, &course_key, 1_700_000_001_000, 0);

    let report = import_beatoraja_scores(
        &source,
        ScoreImportKind::Beatoraja,
        &library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();

    assert_eq!(report.scanned, 1);
    assert_eq!(report.skipped, 1);
    assert_eq!(report.failed, 0);
    assert_eq!(report.imported, 0);
}

#[test]
fn lr2_import_skips_course_scores_without_failing() {
    let (mut library_db, mut score_db, _, _) = open_test_databases();
    let source = Connection::open_in_memory().unwrap();
    // An LR2 course key: a 32-char marker plus four 32-char md5s (160 chars).
    let course_key = "0".repeat(32) + &"a".repeat(128);
    create_lr2_source_with_hash(&source, &course_key);

    let report = import_lr2_scores(
        &source,
        ScoreImportKind::Lr2,
        &mut library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();

    assert_eq!(report.scanned, 1);
    assert_eq!(report.skipped, 1);
    assert_eq!(report.failed, 0);
    assert_eq!(report.imported, 0);
}

#[test]
fn lr2_course_import_resolves_canonical_and_fans_out_ln_variants() {
    use bmz_core::course::{
        CourseConstraints, CourseDefinition, CourseEntry, CourseKind, CourseLnConstraint,
    };

    let (mut library_db, mut score_db, _, _) = open_test_databases();
    let stage_md5s = [
        "11111111111111111111111111111111",
        "22222222222222222222222222222222",
        "33333333333333333333333333333333",
        "44444444444444444444444444444444",
    ];
    let stage_sha256s = ["11".repeat(32), "22".repeat(32), "33".repeat(32), "44".repeat(32)];
    let entries: Vec<CourseEntry> = stage_md5s
        .iter()
        .enumerate()
        .map(|(i, m)| CourseEntry {
            title_hint: format!("stage{i}"),
            md5: Some(m.to_string()),
            sha256: Some(stage_sha256s[i].clone()),
            chart_id: None,
        })
        .collect();
    let course =
        |key: &str, judge: CourseJudgeConstraint, ln: CourseLnConstraint| CourseDefinition {
            key: key.to_string(),
            title: key.to_string(),
            kind: CourseKind::Dan,
            entries: entries.clone(),
            constraints: CourseConstraints {
                class: CourseClassConstraint::GradeMirrorAllowed,
                speed: CourseSpeedConstraint::Free,
                judge,
                gauge: CourseGaugeConstraint::Lr2,
                ln,
                source_constraints: Vec::new(),
            },
            trophies: Vec::new(),
            release: true,
        };
    // Two canonical variants differing only by LN -> both receive the score.
    library_db
        .upsert_course(
            "table:x",
            &course("dan_default", CourseJudgeConstraint::Normal, CourseLnConstraint::Default),
            0,
            1,
        )
        .unwrap();
    library_db
        .upsert_course(
            "table:x",
            &course("dan_ln", CourseJudgeConstraint::Normal, CourseLnConstraint::Ln),
            1,
            1,
        )
        .unwrap();
    // Non-canonical (no_good judge) sharing the same songs -> must be ignored.
    library_db
        .upsert_course(
            "table:x",
            &course("dan_nogood", CourseJudgeConstraint::NoGood, CourseLnConstraint::Default),
            2,
            1,
        )
        .unwrap();

    // LR2 course record: 32-char marker + the four stage md5s (160 chars).
    let hash = "0".repeat(32) + &stage_md5s.concat();
    let source = Connection::open_in_memory().unwrap();
    create_lr2_source_with_hash(&source, &hash);

    let report = import_lr2_scores(
        &source,
        ScoreImportKind::Lr2,
        &mut library_db,
        &mut score_db,
        1_700_000_000,
    )
    .unwrap();

    assert_eq!(report.scanned, 1);
    assert_eq!(report.matched, 1);
    // Fanned out into the two canonical LN variants, not the no_good course.
    assert_eq!(report.imported, 2);
    let count: i64 =
        score_db.conn().query_row("SELECT COUNT(*) FROM course_scores", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 2);
    let distinct_hashes: i64 = score_db
        .conn()
        .query_row("SELECT COUNT(DISTINCT course_hash) FROM course_scores", [], |r| r.get(0))
        .unwrap();
    assert_eq!(distinct_hashes, 2);
    // The imported course score reflects the LR2 aggregate row (clear=4 -> Hard,
    // ex = perfect*2 + great = 222).
    let (clear, ex): (String, u32) = score_db
        .conn()
        .query_row("SELECT clear_type, ex_score FROM course_scores LIMIT 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(clear, "Hard");
    assert_eq!(ex, 222);

    // Course rows share LR2's `score` table: do not create another course
    // history entry when its best score used SCATTER.
    source.execute("UPDATE score SET op_best = 40", []).unwrap();
    let skipped = import_lr2_scores(
        &source,
        ScoreImportKind::Lr2,
        &mut library_db,
        &mut score_db,
        1_700_000_001,
    )
    .unwrap();
    assert_eq!(skipped.skipped, 1);
    assert_eq!(skipped.imported, 0);
    let count_after_skip: i64 =
        score_db.conn().query_row("SELECT COUNT(*) FROM course_scores", [], |r| r.get(0)).unwrap();
    assert_eq!(count_after_skip, 2);
}

#[test]
fn decode_lr2_ghost_handles_plain_symbols() {
    // No dictionary tokens, no run counts: B A E D -> Bad, Poor, PGreat, Great.
    assert_eq!(decode_lr2_ghost("BAED", 4), vec![3, 4, 0, 1]);
    // Single PGreat.
    assert_eq!(decode_lr2_ghost("E", 1), vec![0]);
}

#[test]
fn decode_lr2_ghost_expands_dictionary_and_runs() {
    // Real LR2 ghost captured from a player DB.  Exercises both dictionary
    // layers (m,c,k,S,c,b,Z tokens), a run count (`@2`, `8`) and the leading
    // empty-poor (`@`) that must be dropped.  Validated against the LR2 score
    // row's judge counts.
    let ghost = decode_lr2_ghost("@2mBckScb8Z", 20);
    assert_eq!(ghost, vec![4, 1, 3, 2, 2, 4, 3, 1, 1, 2, 2, 1, 4, 4, 4, 4, 4, 4, 4, 4]);
}

#[test]
fn decode_lr2_ghost_pads_and_truncates_to_total_notes() {
    // Aborted play: decoded ghost shorter than the chart -> pad with Poor (4).
    let padded = decode_lr2_ghost("E", 4);
    assert_eq!(padded, vec![0, 4, 4, 4]);
    // Over-long ghost is truncated to the note count.
    let truncated = decode_lr2_ghost("E", 0);
    assert_eq!(truncated, vec![0]); // total_notes 0 leaves the decode untouched
    let truncated = decode_lr2_ghost("BAED", 2);
    assert_eq!(truncated, vec![3, 4]);
}

#[test]
fn lr2_score_state_decodes_ghost() {
    let row = Lr2ScoreRow {
        md5: "0".repeat(32),
        clear: 4,
        perfect: 100,
        great: 21,
        good: 3,
        bad: 2,
        poor: 1,
        total_notes: 4,
        max_combo: 64,
        min_bp: 3,
        play_count: 2,
        clear_count: 1,
        ghost: "BAED".to_string(),
        random_seed: Some(123),
        op_best: 0,
    };
    let state = score_state_from_lr2(&row, 4);
    assert_eq!(state.ghost, vec![3, 4, 0, 1]);
    assert_eq!(state.judges.fast_pgreat, 100);
}

#[test]
fn is_course_hash_classifies_by_length() {
    // beatoraja sha256 width.
    assert!(!is_course_hash(&"a".repeat(64), 64));
    assert!(is_course_hash(&"a".repeat(128), 64));
    assert!(is_course_hash(&"a".repeat(256), 64));
    // LR2 md5 width.
    assert!(!is_course_hash(&"a".repeat(32), 32));
    assert!(is_course_hash(&"a".repeat(160), 32));
    // Genuinely malformed (not a multiple of the width) stays a hard failure.
    assert!(!is_course_hash(&"a".repeat(100), 64));
    assert!(!is_course_hash("", 64));
}

fn open_test_databases() -> (LibraryDatabase, ScoreDatabase, [u8; 32], [u8; 16]) {
    open_test_databases_with_chart(chart())
}

fn open_test_databases_with_chart(
    chart: PlayableChart,
) -> (LibraryDatabase, ScoreDatabase, [u8; 32], [u8; 16]) {
    let mut library_conn = Connection::open_in_memory().unwrap();
    super::super::common::configure_connection(&library_conn).unwrap();
    run_migrations(&mut library_conn, LIBRARY_MIGRATIONS).unwrap();
    let mut library_db = LibraryDatabase::from_connection(library_conn);
    let sha256 = chart.identity.file_sha256;
    let md5 = chart.identity.file_md5;
    library_db
        .upsert_chart_import(&ChartImportRecord {
            root_id: None,
            file_path: Path::new("/songs/import.bms"),
            file_size: 10,
            modified_at: 1,
            scanned_at: 1,
            chart: &chart,
        })
        .unwrap();

    let mut score_conn = Connection::open_in_memory().unwrap();
    super::super::common::configure_connection(&score_conn).unwrap();
    run_migrations(&mut score_conn, SCORE_MIGRATIONS).unwrap();
    (library_db, ScoreDatabase::from_connection(score_conn), sha256, md5)
}

fn chart() -> PlayableChart {
    let mut chart = PlayableChart {
        identity: compute_chart_identity(b"score import test"),
        metadata: ChartMetadata {
            title: "Import Target".to_string(),
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
        bga_asset_by_bmp_key: HashMap::new(),
        bar_lines: Vec::new(),
        sounds: Vec::new(),
        bga_assets: Vec::new(),
        total_notes: 128,
        end_time: TimeUs(10_000_000),
    };
    chart.identity.file_md5 = [1; 16];
    chart.identity.file_sha256 = [2; 32];
    chart
}

fn undefined_ln_chart(total_notes: u32, long_pairs: u32) -> PlayableChart {
    let mut chart = chart();
    chart.total_notes = total_notes;
    chart.long_notes = (0..long_pairs)
        .map(|index| LongNotePair {
            lane: Lane::Key1,
            style: LongNoteStyle::ChannelPair,
            mode: None,
            start_note_id: NoteId(index * 2 + 1),
            end_note_id: NoteId(index * 2 + 2),
            start_tick: ChartTick(0),
            end_tick: ChartTick(192),
            start_time: TimeUs(0),
            end_time: TimeUs(1_000_000),
            sound: None,
        })
        .collect();
    chart
}

fn create_lr2_source(conn: &Connection, md5: &[u8; 16]) {
    create_lr2_source_with_hash(conn, &hash_to_hex(md5));
}

fn create_lr2_source_with_hash(conn: &Connection, hash: &str) {
    // `poor` includes Empty Poor in LR2 and may make the judge sum exceed totalnotes.
    create_lr2_source_with_score(conn, hash, 128, 64, 100, 22, 3, 2, 10);
}

#[allow(clippy::too_many_arguments)]
fn create_lr2_source_with_score(
    conn: &Connection,
    hash: &str,
    total_notes: u32,
    max_combo: u32,
    perfect: u32,
    great: u32,
    good: u32,
    bad: u32,
    poor: u32,
) {
    conn.execute_batch(
        "CREATE TABLE score (
                hash TEXT, clear INTEGER, perfect INTEGER, great INTEGER,
                good INTEGER, bad INTEGER, poor INTEGER, totalnotes INTEGER,
                maxcombo INTEGER, minbp INTEGER, playcount INTEGER, clearcount INTEGER,
                ghost TEXT, rseed INTEGER, op_best INTEGER
            );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO score VALUES (?1, 4, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 3, 2, 1, '', 123, 0)",
        params![hash, perfect, great, good, bad, poor, total_notes, max_combo],
    )
    .unwrap();
}

fn create_beatoraja_source(conn: &Connection, sha256: &[u8; 32], date: i64, mode: i64) {
    create_beatoraja_source_with_sha256(conn, &hash_to_hex(sha256), date, mode);
}

fn create_beatoraja_source_with_sha256(conn: &Connection, sha256: &str, date: i64, mode: i64) {
    // Default no-LN chart expects 128 scored notes.
    create_beatoraja_source_with_score(conn, sha256, date, mode, 7, 128, 128, 80);
}

#[allow(clippy::too_many_arguments)]
fn create_beatoraja_source_with_score(
    conn: &Connection,
    sha256: &str,
    date: i64,
    mode: i64,
    clear: i64,
    total_notes: u32,
    judged: u32,
    max_combo: u32,
) {
    // Split judged across fast/slow buckets for schema coverage; empty poor
    // (ems/lms) is excluded from the import note-count check.
    let epg = judged.saturating_sub(28).min(judged);
    let rem = judged.saturating_sub(epg);
    let lpg = rem.min(10);
    let rem = rem.saturating_sub(lpg);
    let egr = rem.min(5);
    let rem = rem.saturating_sub(egr);
    let lgr = rem.min(3);
    let rem = rem.saturating_sub(lgr);
    let egd = rem.min(2);
    let rem = rem.saturating_sub(egd);
    let lgd = rem.min(1);
    let rem = rem.saturating_sub(lgd);
    let ebd = rem.min(2);
    let rem = rem.saturating_sub(ebd);
    let lbd = rem.min(1);
    let rem = rem.saturating_sub(lbd);
    let epr = rem.min(3);
    let lpr = rem.saturating_sub(epr);

    if conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='score'",
            [],
            |_| Ok(()),
        )
        .is_err()
    {
        conn.execute_batch(
            "CREATE TABLE score (
                    sha256 TEXT, mode INTEGER, clear INTEGER, epg INTEGER, lpg INTEGER,
                    egr INTEGER, lgr INTEGER, egd INTEGER, lgd INTEGER,
                    ebd INTEGER, lbd INTEGER, epr INTEGER, lpr INTEGER,
                    ems INTEGER, lms INTEGER, notes INTEGER, combo INTEGER,
                    minbp INTEGER, ghost TEXT, seed INTEGER, date INTEGER, option INTEGER
                );",
        )
        .unwrap();
    }
    conn.execute(
            "INSERT INTO score VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 3, 1, ?14, ?15, 2, '', 456, ?16, 0
            )",
            params![
                sha256,
                mode,
                clear,
                epg,
                lpg,
                egr,
                lgr,
                egd,
                lgd,
                ebd,
                lbd,
                epr,
                lpr,
                total_notes,
                max_combo,
                date
            ],
        )
        .unwrap();
}
