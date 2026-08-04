use anyhow::{Result, bail};
use bmz_core::clear::ClearType;
use bmz_core::input::InputDeviceKind;
use bmz_gameplay::gauge::GaugeCarryValue;
#[cfg(test)]
use bmz_gameplay::judge::model::JudgeWindows;
use bmz_gameplay::result::PlayResult;
use bmz_gameplay::session::{GameSession, PlayState};

use crate::config::profile_config::{IrConfig, ReplayConfig};
use crate::ir::payload::{IrSubmissionContext, build_score_submission};
use crate::ln_policy::ChartLnProfile;
use crate::paths::ProfilePaths;
use crate::screens::play_session::AppliedArrange;
use crate::screens::result_model::ResultSummary;
use crate::storage::network_db::{IrJobKind, NetworkDatabase, NewIrScoreJob};
use crate::storage::play_result::{
    StorePlayResultMode, StorePlayResultRequest, StoredPlayResult, course_stage_clear_type,
    store_play_result,
};
use crate::storage::score_db::{ScoreDatabase, ScoreKey};

#[derive(Debug, Clone)]
pub struct FinishedPlaySession {
    pub result: PlayResult,
    pub stored: StoredPlayResult,
    pub summary: ResultSummary,
    pub gauge_carry: Vec<GaugeCarryValue>,
    pub course_combo: u32,
    pub course_max_combo: u32,
    pub replay_playback: bool,
    pub arrange: crate::select_options::ArrangeOption,
    pub applied_arrange: AppliedArrange,
    /// IR ランキング照会に使うスコア分離キー。
    pub ln_policy: crate::ln_policy::LnScorePolicy,
    pub double_option: crate::select_options::DoubleOptionScoreBucket,
    pub rule_mode: bmz_gameplay::rule::RuleMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishResultMode {
    Normal,
    CourseStage,
}

impl FinishResultMode {
    fn store_mode(self) -> StorePlayResultMode {
        match self {
            Self::Normal => StorePlayResultMode::Normal,
            Self::CourseStage => StorePlayResultMode::CourseStage,
        }
    }

    fn summary_clear_type(self, clear_type: ClearType) -> ClearType {
        match self {
            Self::Normal => clear_type,
            Self::CourseStage => course_stage_clear_type(clear_type),
        }
    }

    fn enqueue_score_ir(self) -> bool {
        match self {
            Self::Normal | Self::CourseStage => true,
        }
    }
}

pub fn play_result_from_session(session: &GameSession) -> PlayResult {
    PlayResult::from_states_with_total_notes(
        &session.chart,
        &session.score,
        &session.gauge,
        session.scored_total_notes,
        session.state,
        session.autoplay.as_ref().is_some_and(|autoplay| autoplay.is_full()),
    )
}

pub fn store_session_result(
    score_db: &mut ScoreDatabase,
    network_db: &mut NetworkDatabase,
    profile_paths: &ProfilePaths,
    replay_config: &ReplayConfig,
    ir_config: &IrConfig,
    session: &GameSession,
    played_at: i64,
    applied_arrange: &AppliedArrange,
    score_key: ScoreKey,
    practice_mode: bool,
) -> Result<StoredPlayResult> {
    Ok(finish_session_result(
        score_db,
        network_db,
        FinishSessionResultRequest {
            profile_paths,
            replay_config,
            ir_config,
            session,
            played_at,
            applied_arrange,
            source_ln_profile: ChartLnProfile::from_chart(&session.chart),
            target_ex_score: None,
            score_key,
            practice_mode,
            finish_mode: FinishResultMode::Normal,
        },
    )?
    .stored)
}

pub struct FinishSessionResultRequest<'a> {
    pub profile_paths: &'a ProfilePaths,
    pub replay_config: &'a ReplayConfig,
    pub ir_config: &'a IrConfig,
    pub session: &'a GameSession,
    pub played_at: i64,
    pub applied_arrange: &'a AppliedArrange,
    pub source_ln_profile: ChartLnProfile,
    pub target_ex_score: Option<u32>,
    pub score_key: ScoreKey,
    pub practice_mode: bool,
    pub finish_mode: FinishResultMode,
}

pub fn finish_session_result(
    score_db: &mut ScoreDatabase,
    network_db: &mut NetworkDatabase,
    request: FinishSessionResultRequest<'_>,
) -> Result<FinishedPlaySession> {
    let FinishSessionResultRequest {
        profile_paths,
        replay_config,
        ir_config,
        session,
        played_at,
        applied_arrange,
        source_ln_profile,
        target_ex_score,
        score_key,
        practice_mode,
        finish_mode,
    } = request;
    ensure_storable_state(session.state)?;
    let result = play_result_from_session(session);
    let summary_clear_type = finish_mode.summary_clear_type(result.clear_type);
    let replay_playback = session.replay_player.is_some() && session.replay_lane_mask.is_none();
    let previous_best =
        score_db.best_scores_for_charts(&[score_key]).ok().and_then(|mut bests| bests.pop());
    // オートプレイ / リプレイ再生 / プラクティス時はスコア・リプレイをDBに保存しない
    // （リザルト画面の表示のみ行う）。
    let full_autoplay = session.autoplay.as_ref().is_some_and(|autoplay| autoplay.is_full());
    let stored = if full_autoplay || replay_playback || practice_mode {
        StoredPlayResult {
            score_history_id: 0,
            played_at,
            replay_path: String::new(),
            replay_sha256: None,
            slot_paths: [None, None, None, None],
            device_type: InputDeviceKind::Keyboard,
        }
    } else {
        let arrange = applied_arrange.arrange;
        let arrange_seed = applied_arrange.seed;
        let random_seed = applied_arrange.packed_beatoraja_seed(session.primary_key_mode);
        let arrange_pattern = applied_arrange.pattern.clone();
        store_play_result(
            score_db,
            profile_paths,
            replay_config,
            &result,
            StorePlayResultRequest {
                played_at,
                playtime_seconds: chart_playtime_seconds(&session.chart),
                ln_policy: score_key.ln_policy,
                double_option: score_key.double_option,
                applied_double_option: applied_arrange.double_option,
                random_seed,
                gauge_option: String::new(),
                rule_mode: session.rule_mode.as_str().to_string(),
                assist_mask: 0,
                replay_events: session.replay_recorder.events.clone(),
                arrange,
                arrange_2p: applied_arrange.arrange_2p,
                arrange_seed,
                arrange_seed_2p: applied_arrange.seed_2p,
                bms_random_choices: applied_arrange.bms_random_choices.clone(),
                seed_scheme: if applied_arrange.legacy_seed {
                    crate::storage::replay::SEED_SCHEME_LEGACY_SHARED_V3.to_string()
                } else {
                    crate::storage::replay::SEED_SCHEME_BEATORAJA_24BIT_V1.to_string()
                },
                arrange_pattern,
                mode: finish_mode.store_mode(),
            },
        )?
    };
    let mut summary = ResultSummary::from_play_result(&result, &stored, &session.chart);
    summary.key_mode = session.primary_key_mode;
    summary.clear_type = summary_clear_type;
    summary.arrange = applied_arrange.arrange.as_str().to_string();
    summary.arrange_2p = applied_arrange.arrange_2p.as_str().to_string();
    summary.lane_shuffle_pattern = applied_arrange.pattern.clone().unwrap_or_default();
    summary.target_ex_score = target_ex_score;
    summary.saved_replay_slots = stored.slot_paths.each_ref().map(Option::is_some);
    if let Some(best) = &previous_best {
        summary.previous_best_ex_score = Some(best.ex_score);
        summary.previous_best_clear_type = clear_type_from_name(&best.clear_type);
        summary.previous_best_max_combo = Some(best.max_combo);
        summary.previous_best_bp = Some(best.bp);
    }
    // 過去ベストスコア・ベストコンボを ResultSummary にフィルする。
    // 今回のスコアが直前に upsert_score_best されているので、`best_*` は
    // 「現在の最高記録」を返す。差分表示は `current - best` として 0 になり得る。
    if let Ok(bests) = score_db.best_scores_for_charts(&[score_key])
        && let Some(best) = bests.into_iter().next()
    {
        summary.best_ex_score = Some(best.ex_score);
        summary.best_clear_type = clear_type_from_name(&best.clear_type);
        summary.best_max_combo = Some(best.max_combo);
        summary.best_bp = Some(best.bp);
    }
    if let Ok(slots) = score_db.replay_slots_for_chart(score_key) {
        summary.replay_slots = slots.each_ref().map(Option::is_some);
        for (index, saved) in summary.saved_replay_slots.iter().enumerate() {
            if *saved {
                summary.replay_slots[index] = true;
            }
        }
    }
    if finish_mode.enqueue_score_ir() {
        let mut ir_result = result.clone();
        ir_result.clear_type = summary_clear_type;
        enqueue_ir_jobs(
            network_db,
            ir_config,
            EnqueueIrJobsRequest {
                session,
                result: &ir_result,
                stored: &stored,
                played_at,
                score_key,
                applied_arrange,
                source_ln_profile,
                summary: &mut summary,
                previous_best: previous_best.as_ref(),
            },
        );
    }

    Ok(FinishedPlaySession {
        result,
        stored,
        summary,
        gauge_carry: session.gauge.carry_values(),
        course_combo: session.display_combo(),
        course_max_combo: session.display_max_combo(),
        replay_playback,
        arrange: applied_arrange.arrange,
        applied_arrange: applied_arrange.clone(),
        ln_policy: score_key.ln_policy,
        double_option: score_key.double_option,
        rule_mode: score_key.rule_mode,
    })
}

fn chart_playtime_seconds(chart: &bmz_chart::model::PlayableChart) -> u32 {
    (chart.end_time.0.max(0) / 1_000_000).min(i64::from(u32::MAX)) as u32
}

fn chart_duration_ms(chart: &bmz_chart::model::PlayableChart) -> u64 {
    (chart.end_time.0.max(0) / 1_000) as u64
}

fn clear_type_from_name(name: &str) -> Option<ClearType> {
    match name {
        "NoPlay" => Some(ClearType::NoPlay),
        "Failed" => Some(ClearType::Failed),
        "AssistEasy" => Some(ClearType::AssistEasy),
        "LightAssistEasy" => Some(ClearType::LightAssistEasy),
        "Easy" => Some(ClearType::Easy),
        "Normal" => Some(ClearType::Normal),
        "Hard" => Some(ClearType::Hard),
        "ExHard" => Some(ClearType::ExHard),
        "FullCombo" => Some(ClearType::FullCombo),
        "Perfect" => Some(ClearType::Perfect),
        "Max" => Some(ClearType::Max),
        _ => None,
    }
}

struct EnqueueIrJobsRequest<'a> {
    session: &'a GameSession,
    result: &'a PlayResult,
    stored: &'a StoredPlayResult,
    played_at: i64,
    score_key: ScoreKey,
    applied_arrange: &'a AppliedArrange,
    source_ln_profile: ChartLnProfile,
    summary: &'a mut ResultSummary,
    previous_best: Option<&'a crate::storage::score_db::BestScoreSummary>,
}

fn enqueue_ir_jobs(
    network_db: &mut NetworkDatabase,
    ir_config: &IrConfig,
    request: EnqueueIrJobsRequest<'_>,
) {
    let EnqueueIrJobsRequest {
        session,
        result,
        stored,
        played_at,
        score_key,
        applied_arrange,
        source_ln_profile,
        summary,
        previous_best,
    } = request;
    // Ghost Battle はローカルの1Pスコアとして保存するが、表示用に複製した
    // K10/K14 chart を外部IRへ通常譜面として送信しない。
    if stored.score_history_id <= 0 || session.replay_lane_mask.is_some() {
        return;
    }
    let enabled: Vec<_> = ir_config
        .providers
        .iter()
        .filter(|provider| {
            provider.enabled
                && should_send_ir_score(provider.send_policy, result, previous_best)
                && (!crate::ir::rian_ir::is_rian_ir_config(provider)
                    || crate::ir::rian_ir::score_submission_supported(
                        score_key.ln_policy,
                        applied_arrange.double_option,
                    ))
        })
        .collect();
    if enabled.is_empty() {
        return;
    }
    let payload = build_score_submission(
        &session.chart,
        result,
        IrSubmissionContext {
            played_at,
            duration_ms: Some(chart_duration_ms(&session.chart)),
            ln_policy: score_key.ln_policy,
            source_ln_profile,
            gauge_option: result.gauge_type.as_str().to_string(),
            device_type: stored.device_type,
            idempotency_key: format!("bmz-score-{}", stored.score_history_id),
            arrange: applied_arrange.arrange,
            arrange_2p: applied_arrange.arrange_2p,
            double_option: score_key.double_option,
            applied_double_option: applied_arrange.double_option,
            arrange_seed: applied_arrange.packed_beatoraja_seed(session.primary_key_mode),
            random_seed: applied_arrange.packed_beatoraja_seed(session.primary_key_mode),
            seed_scheme: if applied_arrange.legacy_seed {
                crate::storage::replay::SEED_SCHEME_LEGACY_SHARED_V3.to_string()
            } else {
                crate::storage::replay::SEED_SCHEME_BEATORAJA_24BIT_V1.to_string()
            },
            bms_random_choices: applied_arrange.bms_random_choices.clone(),
            rule_mode: session.rule_mode.as_str().to_string(),
            // 保存時に serialize 済みバイト列から計算した hash。プレイ終了
            // 直後のフレームでリプレイファイルを読み直さない。
            replay_hash: stored.replay_sha256.clone(),
        },
    );
    let Ok(payload_json) = serde_json::to_string(&payload) else {
        summary.ir_last_error = Some("failed to serialize IR payload".to_string());
        return;
    };
    for provider in enabled {
        let Some(provider_key) = crate::ir::provider_key::configured_provider_key(provider) else {
            summary.ir_last_error = Some(format!(
                "IR provider '{}' is missing provider_key; log in again",
                provider.provider
            ));
            continue;
        };
        match network_db.enqueue_ir_score_job(&NewIrScoreJob {
            provider: provider_key.to_string(),
            account_id: provider.account_id.clone(),
            kind: IrJobKind::Score,
            local_score_id: stored.score_history_id,
            chart_sha256: result.chart_sha256,
            ln_policy: score_key.ln_policy,
            payload_json: payload_json.clone(),
            now: played_at,
        }) {
            Ok(_) => summary.ir_queued_jobs += 1,
            Err(error) => {
                summary.ir_last_error = Some(error.to_string());
                tracing::warn!(provider = provider.provider, provider_key, %error, "failed to enqueue IR score job");
            }
        }
    }
}

/// 送信ポリシーによる IR ジョブ作成可否。
///
/// - `Always`: 常に送る
/// - `CompleteSong`: 最終ゲージが 0 より大きい場合だけ送る
/// - `UpdateScore`: EX / clear / max combo / BP / CB のいずれかが
///   ローカルベストから改善した場合 (または初プレイ) だけ送る
///
/// サーバー側でも best 更新判定は別途行われるため、これはクライアント側の
/// 送信量制御にすぎない。
fn should_send_ir_score(
    policy: crate::config::profile_config::IrSendPolicyConfig,
    result: &PlayResult,
    previous_best: Option<&crate::storage::score_db::BestScoreSummary>,
) -> bool {
    use crate::config::profile_config::IrSendPolicyConfig;
    match policy {
        IrSendPolicyConfig::Always => true,
        IrSendPolicyConfig::CompleteSong => result.gauge_value > 0.0,
        IrSendPolicyConfig::UpdateScore => {
            let Some(best) = previous_best else {
                return true;
            };
            let best_clear_rank =
                clear_type_from_name(&best.clear_type).map(|clear| clear as i32).unwrap_or(0);
            result.score.ex_score() > best.ex_score
                || (result.clear_type as i32) > best_clear_rank
                || result.score.max_combo > best.max_combo
                || result.record_bp() < best.bp
                || result.record_cb() < best.cb
        }
    }
}

pub fn finish_session_result_once(
    cached: &mut Option<FinishedPlaySession>,
    score_db: &mut ScoreDatabase,
    network_db: &mut NetworkDatabase,
    request: FinishSessionResultOnceRequest<'_>,
) -> Result<FinishedPlaySession> {
    if let Some(finished) = cached.clone() {
        return Ok(finished);
    }

    let mut finished = finish_session_result(
        score_db,
        network_db,
        FinishSessionResultRequest {
            profile_paths: request.profile_paths,
            replay_config: request.replay_config,
            ir_config: request.ir_config,
            session: request.session,
            played_at: request.played_at,
            applied_arrange: request.applied_arrange,
            source_ln_profile: request.source_ln_profile,
            target_ex_score: request.target_ex_score,
            score_key: request.score_key,
            practice_mode: request.practice_mode,
            finish_mode: request.finish_mode,
        },
    )?;
    finished.summary.target_name = request.target_name.replace('_', " ");
    *cached = Some(finished.clone());
    Ok(finished)
}

pub struct FinishSessionResultOnceRequest<'a> {
    pub profile_paths: &'a ProfilePaths,
    pub replay_config: &'a ReplayConfig,
    pub ir_config: &'a IrConfig,
    pub session: &'a GameSession,
    pub played_at: i64,
    pub applied_arrange: &'a AppliedArrange,
    pub source_ln_profile: ChartLnProfile,
    pub target_ex_score: Option<u32>,
    pub target_name: &'a str,
    pub score_key: ScoreKey,
    pub practice_mode: bool,
    pub finish_mode: FinishResultMode,
}

fn ensure_storable_state(state: PlayState) -> Result<()> {
    match state {
        PlayState::Finished | PlayState::Failed => Ok(()),
        PlayState::Ready | PlayState::Playing => bail!("play session is not finished yet"),
    }
}

#[cfg(test)]
#[path = "play_finish/tests.rs"]
mod tests;
