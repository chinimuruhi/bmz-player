use std::sync::Arc;

use bmz_chart::hash::compute_chart_identity;
use bmz_chart::model::{ChartMetadata, NoteEvent, NoteKind, PlayableChart};
use bmz_core::clear::ClearType;
use bmz_core::ids::NoteId;
use bmz_core::input::{InputDeviceKind, InputKind, InputSource};
use bmz_core::lane::Lane;
use bmz_core::time::{ChartTick, TimeUs};
use bmz_gameplay::input::backend::NullInputBackend;
use bmz_gameplay::input::binding::LaneBinding;
use bmz_gameplay::input::system::InputSystem;
use bmz_gameplay::input::translator::DefaultInputTranslator;
use bmz_gameplay::judge::engine::JudgeEngine;
use bmz_gameplay::replay::ReplayRecorder;
use bmz_gameplay::session::{BgmScheduler, GameSession, PlayAudioMix, PlayOffsets, PlayState};
use rusqlite::Connection;

use super::*;
use crate::config::play::DEFAULT_JUDGE_WINDOW;
use crate::config::profile_config::{IrConfig, IrProviderConfig, ReplayConfig};
use crate::storage::common::configure_connection;
use crate::storage::migration::{NETWORK_MIGRATIONS, SCORE_MIGRATIONS, run_migrations};

#[test]
fn should_send_ir_score_follows_policy() {
    use crate::config::profile_config::IrSendPolicyConfig;
    use crate::storage::score_db::BestScoreSummary;

    let mut result = play_result_from_session(&session());
    result.gauge_value = 0.0;
    // Failed (ゲージ 0) は CompleteSong では送らない。
    assert!(should_send_ir_score(IrSendPolicyConfig::Always, &result, None));
    assert!(!should_send_ir_score(IrSendPolicyConfig::CompleteSong, &result, None));
    result.gauge_value = 12.0;
    assert!(should_send_ir_score(IrSendPolicyConfig::CompleteSong, &result, None));

    // UpdateScore: ベストが無ければ送る。
    assert!(should_send_ir_score(IrSendPolicyConfig::UpdateScore, &result, None));

    let best = BestScoreSummary {
        chart_sha256: [0; 32],
        ln_policy: crate::ln_policy::LnScorePolicy::ForceLn,
        double_option: crate::select_options::DoubleOptionScoreBucket::Off,
        rule_mode: bmz_gameplay::rule::RuleMode::Beatoraja,
        clear_type: "Hard".to_string(),
        gauge_type: "Normal".to_string(),
        gauge_value: Some(100.0),
        ex_score: 100,
        bp: 0,
        cb: 0,
        max_combo: 100,
        judge_counts: Default::default(),
        fast_slow_counts: Default::default(),
        play_count: 1,
        clear_count: 1,
        device_type: InputDeviceKind::Keyboard,
        played_at: 0,
        replay_path: String::new(),
    };
    // session() の結果は EX 0 / Failed / combo 0 / BP 1 なので全項目で劣る。
    assert!(!should_send_ir_score(IrSendPolicyConfig::UpdateScore, &result, Some(&best)));
    // EX が改善すれば送る。
    let mut improved = result.clone();
    improved.score.judges.fast_pgreat = 100;
    assert!(should_send_ir_score(IrSendPolicyConfig::UpdateScore, &improved, Some(&best)));
}

fn open_network_db() -> NetworkDatabase {
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, NETWORK_MIGRATIONS).unwrap();
    NetworkDatabase::from_connection(conn)
}

#[test]
fn play_result_from_session_uses_session_state() {
    let session = session();

    let result = play_result_from_session(&session);

    assert_eq!(result.chart_sha256, session.chart.identity.file_sha256);
    assert_eq!(result.clear_type, ClearType::Failed);
    assert!(!result.autoplay);
}

#[test]
fn store_session_result_writes_replay_events() {
    let root = make_temp_dir("finish-session");
    let paths = ProfilePaths {
        root_dir: root.clone(),
        profile_toml: root.join("profile.toml"),
        collection_db: root.join("collection.db"),
        score_db: root.join("score.db"),
        network_db: root.join("network.db"),
        replay_dir: root.join("replay"),
    };
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut score_db = ScoreDatabase::from_connection(conn);
    let mut network_db = open_network_db();
    let replay_config = ReplayConfig {
        auto_save: true,
        compress: false,
        slot_rules: crate::config::profile_config::default_slot_rules(),
    };
    let mut session = session();
    session.replay_recorder.record(bmz_core::input::InputEvent {
        lane: Lane::Key1,
        kind: InputKind::Press,
        time: TimeUs(10),
        source: InputSource::Human,
        device_kind: InputDeviceKind::Keyboard,
        scratch_direction: None,
    });

    let stored = store_session_result(
        &mut score_db,
        &mut network_db,
        &paths,
        &replay_config,
        &crate::config::profile_config::IrConfig::default(),
        &session,
        1_700_000_100,
        &AppliedArrange::default(),
        score_key(&session),
        false,
    )
    .unwrap();

    assert!(stored.score_history_id > 0);
    assert!(!stored.replay_path.is_empty());
    assert!(root.join(&stored.replay_path).exists());

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn finish_session_result_returns_summary() {
    let root = make_temp_dir("finish-summary");
    let paths = ProfilePaths {
        root_dir: root.clone(),
        profile_toml: root.join("profile.toml"),
        collection_db: root.join("collection.db"),
        score_db: root.join("score.db"),
        network_db: root.join("network.db"),
        replay_dir: root.join("replay"),
    };
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut score_db = ScoreDatabase::from_connection(conn);
    let mut network_db = open_network_db();
    let replay_config = ReplayConfig {
        auto_save: true,
        compress: false,
        slot_rules: crate::config::profile_config::default_slot_rules(),
    };
    let session = session();
    let lane_shuffle_pattern = (0..bmz_core::lane::LANE_COUNT as u8).rev().collect::<Vec<_>>();
    let applied_arrange = AppliedArrange {
        arrange: crate::select_options::ArrangeOption::Random,
        arrange_2p: crate::select_options::ArrangeOption::Mirror,
        double_option: crate::select_options::DoubleOption::Off,
        seed: Some(42),
        seed_2p: None,
        legacy_seed: false,
        bms_random_choices: vec![1, 2],
        pattern: Some(lane_shuffle_pattern.clone()),
    };

    let finished = finish_session_result(
        &mut score_db,
        &mut network_db,
        FinishSessionResultRequest {
            profile_paths: &paths,
            replay_config: &replay_config,
            ir_config: &crate::config::profile_config::IrConfig::default(),
            session: &session,
            source_ln_profile: ChartLnProfile::from_chart(&session.chart),
            chart_length_ms: None,
            play_duration_ms: None,
            played_at: 1_700_000_102,
            applied_arrange: &applied_arrange,
            target_ex_score: Some(1600),
            score_key: score_key(&session),
            practice_mode: false,
            finish_mode: FinishResultMode::Normal,
        },
    )
    .unwrap();

    assert_eq!(finished.summary.score_history_id, finished.stored.score_history_id);
    assert_eq!(finished.summary.clear_type, finished.result.clear_type);
    assert_eq!(finished.summary.arrange, "RANDOM");
    assert_eq!(finished.summary.arrange_2p, "MIRROR");
    assert_eq!(finished.summary.lane_shuffle_pattern, lane_shuffle_pattern);
    assert_eq!(finished.summary.target_ex_score, Some(1600));
    assert_eq!(finished.summary.saved_replay_slots, [true, true, true, false]);
    assert_eq!(finished.summary.replay_slots, [true, true, true, false]);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn finish_session_result_course_stage_rounds_summary_clear_type() {
    let root = make_temp_dir("finish-course-stage");
    let paths = ProfilePaths {
        root_dir: root.clone(),
        profile_toml: root.join("profile.toml"),
        collection_db: root.join("collection.db"),
        score_db: root.join("score.db"),
        network_db: root.join("network.db"),
        replay_dir: root.join("replay"),
    };
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut score_db = ScoreDatabase::from_connection(conn);
    let mut network_db = open_network_db();
    let replay_config = ReplayConfig {
        auto_save: true,
        compress: false,
        slot_rules: crate::config::profile_config::default_slot_rules(),
    };
    let session = session();

    let finished = finish_session_result(
        &mut score_db,
        &mut network_db,
        FinishSessionResultRequest {
            profile_paths: &paths,
            replay_config: &replay_config,
            ir_config: &crate::config::profile_config::IrConfig::default(),
            session: &session,
            source_ln_profile: ChartLnProfile::from_chart(&session.chart),
            chart_length_ms: None,
            play_duration_ms: None,
            played_at: 1_700_000_109,
            applied_arrange: &AppliedArrange::default(),
            target_ex_score: None,
            score_key: score_key(&session),
            practice_mode: false,
            finish_mode: FinishResultMode::CourseStage,
        },
    )
    .unwrap();

    assert_eq!(finished.result.clear_type, ClearType::Failed);
    assert_eq!(finished.summary.clear_type, ClearType::NoPlay);
    assert_eq!(finished.summary.saved_replay_slots, [false; 4]);
    let bests = score_db.best_scores_for_charts(&[score_key(&session)]).unwrap();
    assert_eq!(bests.len(), 1);
    assert_eq!(bests[0].clear_type, "NoPlay");
    assert_eq!(finished.summary.best_ex_score, Some(bests[0].ex_score));
    assert_eq!(finished.summary.best_clear_type, Some(ClearType::NoPlay));
    let history = score_db.recent_history(10, 0).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].clear_type, "NoPlay");
    assert_eq!(history[0].bp, session.chart.total_notes);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn finish_session_result_course_stage_enqueues_ir_with_rounded_clear_type() {
    let root = make_temp_dir("finish-course-stage-ir");
    let paths = ProfilePaths {
        root_dir: root.clone(),
        profile_toml: root.join("profile.toml"),
        collection_db: root.join("collection.db"),
        score_db: root.join("score.db"),
        network_db: root.join("network.db"),
        replay_dir: root.join("replay"),
    };
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut score_db = ScoreDatabase::from_connection(conn);
    let mut network_db = open_network_db();
    let replay_config = ReplayConfig {
        auto_save: false,
        compress: false,
        slot_rules: crate::config::profile_config::default_slot_rules(),
    };
    let ir_config = IrConfig {
        primary_provider: "bmz-official".to_string(),
        providers: vec![IrProviderConfig {
            provider: "bmz-official".to_string(),
            provider_key: "bmz-official".to_string(),
            base_url: String::new(),
            enabled: true,
            account_display_name: "Player".to_string(),
            account_id: "account-1".to_string(),
            send_policy: Default::default(),
            role: Default::default(),
            last_login_at: None,
            last_success_at: None,
        }],
        ..IrConfig::default()
    };
    let session = session();

    let finished = finish_session_result(
        &mut score_db,
        &mut network_db,
        FinishSessionResultRequest {
            profile_paths: &paths,
            replay_config: &replay_config,
            ir_config: &ir_config,
            session: &session,
            source_ln_profile: ChartLnProfile::from_chart(&session.chart),
            chart_length_ms: None,
            play_duration_ms: None,
            played_at: 1_700_000_110,
            applied_arrange: &AppliedArrange::default(),
            target_ex_score: None,
            score_key: score_key(&session),
            practice_mode: false,
            finish_mode: FinishResultMode::CourseStage,
        },
    )
    .unwrap();

    assert_eq!(finished.summary.ir_queued_jobs, 1);
    let jobs = network_db.pending_ir_score_jobs(1_700_000_110, 10).unwrap();
    assert_eq!(jobs.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&jobs[0].payload_json).unwrap();
    assert_eq!(payload["result"]["clear"], "NoPlay");

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn finish_session_result_enqueues_ir_jobs_for_enabled_providers() {
    let root = make_temp_dir("finish-ir");
    let paths = ProfilePaths {
        root_dir: root.clone(),
        profile_toml: root.join("profile.toml"),
        collection_db: root.join("collection.db"),
        score_db: root.join("score.db"),
        network_db: root.join("network.db"),
        replay_dir: root.join("replay"),
    };
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut score_db = ScoreDatabase::from_connection(conn);
    let mut network_db = open_network_db();
    let replay_config = ReplayConfig {
        auto_save: false,
        compress: false,
        slot_rules: crate::config::profile_config::default_slot_rules(),
    };
    let ir_config = IrConfig {
        primary_provider: "bmz-official".to_string(),
        providers: vec![IrProviderConfig {
            provider: "bmz-official".to_string(),
            provider_key: "bmz-official".to_string(),
            base_url: String::new(),
            enabled: true,
            account_display_name: "Player".to_string(),
            account_id: "account-1".to_string(),
            send_policy: Default::default(),
            role: Default::default(),
            last_login_at: None,
            last_success_at: None,
        }],
        ..IrConfig::default()
    };
    let mut session = session();
    Arc::get_mut(&mut session.chart).unwrap().end_time = TimeUs(123_456_789);

    let finished = finish_session_result(
        &mut score_db,
        &mut network_db,
        FinishSessionResultRequest {
            profile_paths: &paths,
            replay_config: &replay_config,
            ir_config: &ir_config,
            session: &session,
            source_ln_profile: ChartLnProfile::from_chart(&session.chart),
            chart_length_ms: Some(123_456),
            play_duration_ms: Some(120_000),
            played_at: 1_700_000_108,
            applied_arrange: &AppliedArrange::default(),
            target_ex_score: None,
            score_key: score_key(&session),
            practice_mode: false,
            finish_mode: FinishResultMode::Normal,
        },
    )
    .unwrap();

    assert_eq!(finished.summary.ir_queued_jobs, 1);
    let jobs = network_db.pending_ir_score_jobs(1_700_000_108, 10).unwrap();
    assert_eq!(jobs.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&jobs[0].payload_json).unwrap();
    assert_eq!(payload["chart"]["length_ms"], 123_456);
    assert_eq!(payload["result"]["duration_ms"], 120_000);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn finish_session_result_once_reuses_cached_result() {
    let root = make_temp_dir("finish-once");
    let paths = ProfilePaths {
        root_dir: root.clone(),
        profile_toml: root.join("profile.toml"),
        collection_db: root.join("collection.db"),
        score_db: root.join("score.db"),
        network_db: root.join("network.db"),
        replay_dir: root.join("replay"),
    };
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut score_db = ScoreDatabase::from_connection(conn);
    let mut network_db = open_network_db();
    let replay_config = ReplayConfig {
        auto_save: true,
        compress: false,
        slot_rules: crate::config::profile_config::default_slot_rules(),
    };
    let session = session();
    let mut cached = None;

    let first = finish_session_result_once(
        &mut cached,
        &mut score_db,
        &mut network_db,
        FinishSessionResultOnceRequest {
            profile_paths: &paths,
            replay_config: &replay_config,
            ir_config: &crate::config::profile_config::IrConfig::default(),
            session: &session,
            source_ln_profile: ChartLnProfile::from_chart(&session.chart),
            chart_length_ms: None,
            play_duration_ms: None,
            played_at: 1_700_000_103,
            applied_arrange: &AppliedArrange::default(),
            target_ex_score: None,
            target_name: "RANK_AAA",
            score_key: score_key(&session),
            practice_mode: false,
            finish_mode: FinishResultMode::Normal,
        },
    )
    .unwrap();
    let second = finish_session_result_once(
        &mut cached,
        &mut score_db,
        &mut network_db,
        FinishSessionResultOnceRequest {
            profile_paths: &paths,
            replay_config: &replay_config,
            ir_config: &crate::config::profile_config::IrConfig::default(),
            session: &session,
            source_ln_profile: ChartLnProfile::from_chart(&session.chart),
            chart_length_ms: None,
            play_duration_ms: None,
            played_at: 1_700_000_104,
            applied_arrange: &AppliedArrange::default(),
            target_ex_score: None,
            target_name: "RANK_AAA",
            score_key: score_key(&session),
            practice_mode: false,
            finish_mode: FinishResultMode::Normal,
        },
    )
    .unwrap();

    assert_eq!(first.stored.score_history_id, second.stored.score_history_id);
    assert_eq!(first.summary.target_name, "RANK AAA");
    assert_eq!(score_db.recent_history(10, 0).unwrap().len(), 1);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn finish_session_result_skips_storage_for_autoplay() {
    let root = make_temp_dir("finish-autoplay");
    let paths = ProfilePaths {
        root_dir: root.clone(),
        profile_toml: root.join("profile.toml"),
        collection_db: root.join("collection.db"),
        score_db: root.join("score.db"),
        network_db: root.join("network.db"),
        replay_dir: root.join("replay"),
    };
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut score_db = ScoreDatabase::from_connection(conn);
    let mut network_db = open_network_db();
    let replay_config = ReplayConfig {
        auto_save: true,
        compress: false,
        slot_rules: crate::config::profile_config::default_slot_rules(),
    };
    let mut session = session();
    session.autoplay = Some(bmz_gameplay::autoplay::AutoplayController::default());

    let finished = finish_session_result(
        &mut score_db,
        &mut network_db,
        FinishSessionResultRequest {
            profile_paths: &paths,
            replay_config: &replay_config,
            ir_config: &crate::config::profile_config::IrConfig::default(),
            session: &session,
            source_ln_profile: ChartLnProfile::from_chart(&session.chart),
            chart_length_ms: None,
            play_duration_ms: None,
            played_at: 1_700_000_105,
            applied_arrange: &AppliedArrange::default(),
            target_ex_score: None,
            score_key: score_key(&session),
            practice_mode: false,
            finish_mode: FinishResultMode::Normal,
        },
    )
    .unwrap();

    // オートプレイ時はDB保存・リプレイ保存をしない。
    assert_eq!(finished.stored.score_history_id, 0);
    assert!(finished.stored.replay_path.is_empty());
    assert!(finished.stored.slot_paths.iter().all(Option::is_none));
    assert_eq!(score_db.recent_history(10, 0).unwrap().len(), 0);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn finish_session_result_skips_storage_for_replay_playback() {
    let root = make_temp_dir("finish-replay");
    let paths = ProfilePaths {
        root_dir: root.clone(),
        profile_toml: root.join("profile.toml"),
        collection_db: root.join("collection.db"),
        score_db: root.join("score.db"),
        network_db: root.join("network.db"),
        replay_dir: root.join("replay"),
    };
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut score_db = ScoreDatabase::from_connection(conn);
    let mut network_db = open_network_db();
    let replay_config = ReplayConfig {
        auto_save: true,
        compress: false,
        slot_rules: crate::config::profile_config::default_slot_rules(),
    };
    let mut session = session();
    session.replay_player = Some(bmz_gameplay::replay::ReplayPlayer::default());

    let finished = finish_session_result(
        &mut score_db,
        &mut network_db,
        FinishSessionResultRequest {
            profile_paths: &paths,
            replay_config: &replay_config,
            ir_config: &crate::config::profile_config::IrConfig::default(),
            session: &session,
            source_ln_profile: ChartLnProfile::from_chart(&session.chart),
            chart_length_ms: None,
            play_duration_ms: None,
            played_at: 1_700_000_106,
            applied_arrange: &AppliedArrange::default(),
            target_ex_score: None,
            score_key: score_key(&session),
            practice_mode: false,
            finish_mode: FinishResultMode::Normal,
        },
    )
    .unwrap();

    assert!(finished.replay_playback);
    assert_eq!(finished.stored.score_history_id, 0);
    assert!(finished.stored.replay_path.is_empty());
    assert!(finished.stored.slot_paths.iter().all(Option::is_none));
    assert_eq!(score_db.recent_history(10, 0).unwrap().len(), 0);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn store_session_result_rejects_unfinished_session() {
    let root = make_temp_dir("unfinished-session");
    let paths = ProfilePaths {
        root_dir: root.clone(),
        profile_toml: root.join("profile.toml"),
        collection_db: root.join("collection.db"),
        score_db: root.join("score.db"),
        network_db: root.join("network.db"),
        replay_dir: root.join("replay"),
    };
    let mut conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    run_migrations(&mut conn, SCORE_MIGRATIONS).unwrap();
    let mut score_db = ScoreDatabase::from_connection(conn);
    let mut network_db = open_network_db();
    let replay_config = ReplayConfig {
        auto_save: true,
        compress: false,
        slot_rules: crate::config::profile_config::default_slot_rules(),
    };
    let mut session = session();
    session.state = PlayState::Playing;

    let result = store_session_result(
        &mut score_db,
        &mut network_db,
        &paths,
        &replay_config,
        &crate::config::profile_config::IrConfig::default(),
        &session,
        1_700_000_101,
        &AppliedArrange::default(),
        score_key(&session),
        false,
    );

    assert!(result.is_err());

    std::fs::remove_dir_all(root).unwrap();
}

fn session() -> GameSession {
    let chart = Arc::new(chart());
    let timing_map = bmz_chart::timing::TimingMap::from_chart_timing_events(
        chart.metadata.initial_bpm,
        &chart.timing_events,
    );
    GameSession {
        chart: Arc::clone(&chart),
        primary_key_mode: chart.metadata.key_mode,
        scored_total_notes: bmz_gameplay::score::scored_note_count(&chart),
        timing_map,
        audio_clock: bmz_audio::clock::AudioClock::stopped(48_000),
        input_system: InputSystem {
            backend: Box::new(NullInputBackend),
            translator: Box::new(DefaultInputTranslator {
                binding: LaneBinding { entries: Vec::new() },
            }),
            bounce_filter: Default::default(),
        },
        judge: JudgeEngine::new(DEFAULT_JUDGE_WINDOW),
        base_judge_window: DEFAULT_JUDGE_WINDOW,
        base_judge_windows: JudgeWindows::uniform(DEFAULT_JUDGE_WINDOW),
        rule_mode: bmz_gameplay::rule::RuleMode::Beatoraja,
        score: Default::default(),
        opponent_score: None,
        course_combo_carry: 0,
        course_combo_carry_active: false,
        course_max_combo: 0,
        gauge: bmz_gameplay::gauge::GaugeState::new(
            bmz_core::clear::GaugeType::Normal,
            160.0,
            chart.total_notes,
        ),
        opponent_gauge: None,
        replay_recorder: ReplayRecorder::default(),
        replay_player: None,
        replay_lane_mask: None,
        display_only_lane_mask: [false; bmz_core::lane::LANE_COUNT],
        autoplay: None,
        recent_inputs: Vec::new(),
        lane_keyon_started_at: Default::default(),
        lane_keyoff_started_at: Default::default(),
        lane_scratch_direction: Default::default(),
        lane_scratch_angle_delta_ms: Default::default(),
        scratch_angle_last_render_at: None,
        lane_auto_release_at: Default::default(),
        recent_judgements: Vec::new(),
        pending_skin_events: Vec::new(),
        next_skin_event_sequence: 0,
        result_judgements: Default::default(),
        hit_error_ring: bmz_gameplay::hit_error::HitErrorRing::default(),
        gauge_increase_started_at: None,
        opponent_gauge_increase_started_at: None,
        gauge_max_started_at: None,
        opponent_gauge_max_started_at: None,
        full_combo_started_at: None,
        opponent_full_combo_started_at: None,
        bgm_scheduler: BgmScheduler::default(),
        offsets: PlayOffsets { input_offset_us: 0, visual_offset_us: 0 },
        input_offset_auto_adjust_enabled: false,
        input_offset_auto_adjust: None,
        audio_mix: PlayAudioMix {
            master_volume: 1.0,
            chart_normalization_gain: 1.0,
            normalize_chart_volume: true,
            key_volume: 1.0,
            bgm_volume: 1.0,
        },
        hispeed: 2.0,
        hispeed_mode: bmz_gameplay::session::HispeedMode::Normal,
        target_green_number: 300,
        hsfix_base_bpm: 120.0,
        lift: 0.0,
        lane_cover: 0.0,
        lane_cover_visible: true,
        lane_cover_changing: false,
        lanecover_enabled: false,
        lift_enabled: true,
        hidden_enabled: false,
        hispeed_auto_adjust: false,
        hidden_cover: 0.0,
        skin_offsets: Vec::new(),
        bga_enabled: true,
        poor_bga_duration_us: 500_000,
        bga_stretch: 1,
        show_ln_tail_cap: false,
        lane_hcn_timer: [None; bmz_core::lane::LANE_COUNT],
        lane_hcn_keysound_muted: [None; bmz_core::lane::LANE_COUNT],
        pending_keysounds: Vec::new(),
        pending_keysound_volumes: Vec::new(),
        hsfix_index: 0,
        input_timestamp_anchor: None,
        pending_mine_hits: Vec::new(),
        state: PlayState::Finished,
        last_hcn_gauge_at: None,
    }
}

fn chart() -> PlayableChart {
    let note = NoteEvent {
        id: NoteId(1),
        lane: Lane::Key1,
        kind: NoteKind::Tap,
        tick: ChartTick(0),
        time: TimeUs(0),
        sound: None,
        layered_sounds: Vec::new(),
        damage: None,
    };
    let mut lane_notes = std::array::from_fn(|_| Vec::new());
    lane_notes[Lane::Key1.index()].push(note);

    PlayableChart {
        identity: compute_chart_identity(b"finish-session"),
        metadata: ChartMetadata::default(),
        lane_notes,
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
        total_notes: 1,
        end_time: TimeUs(0),
    }
}

fn score_key(session: &GameSession) -> ScoreKey {
    ScoreKey::new(session.chart.identity.file_sha256, crate::ln_policy::LnScorePolicy::ForceLn)
        .with_rule_mode(session.rule_mode)
}

fn make_temp_dir(label: &str) -> std::path::PathBuf {
    let stamp =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let path =
        std::env::temp_dir().join(format!("bmz-player-{label}-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    path
}
