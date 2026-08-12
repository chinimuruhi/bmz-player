use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, bail};
use tokio::sync::Mutex;

use crate::config::profile_config::{IrConfig, IrProviderConfig};
use crate::storage::network_db::{
    IrJobKind, IrScoreJobRecord, IrScoreJobStatus, NetworkDatabase, NewIrScoreJob,
    NewIrScoreSubmission,
};
use crate::storage::score_db::ScoreDatabase;

use super::bmz_official::{BmzOfficialIrClient, retry_after_seconds_from_error};
use super::credentials::{IrStoredCredentials, load_credentials, save_credentials};
use crate::ir::types::{IrRankingResult, IrRankingScope, IrScoreSubmission, IrSubmitOptions};

static CREDENTIAL_REFRESH_LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Debug, Default, Clone)]
pub struct IrSyncReport {
    pub submitted: u32,
    pub failed: u32,
    pub messages: Vec<String>,
    /// 送信レスポンスに同梱されたランキングと、そのレスポンスを返したローカル job。
    ///
    /// 同じ譜面を複数回送信するバッチでは chart hash だけで ranking を選ぶと、
    /// 古い試行の応答を今回のリザルトへ表示してしまう。Result 側が今回の
    /// score_history_id と照合できるよう、job の識別子を一緒に保持する。
    pub included_rankings: Vec<IrIncludedRanking>,
}

#[derive(Debug, Clone)]
pub struct IrIncludedRanking {
    pub provider: String,
    pub account_id: String,
    pub kind: IrJobKind,
    pub local_score_id: i64,
    pub previous_rank: Option<u32>,
    pub ranking: IrRankingResult,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct IrReplayJobPayload {
    remote_score_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct IrScoreAttestationJobPayload {
    remote_score_id: String,
}

pub const IR_SYNC_BATCH_LIMIT: u32 = 20;
pub const IR_SYNC_JOB_SPACING_MS: u64 = 3_100;
/// 手動の `ir sync` / local backfill 用。結果画面・常駐同期の待機時間とは分ける。
pub const IR_CLI_SYNC_BATCH_LIMIT: u32 = 100;
pub const IR_CLI_SYNC_JOB_SPACING_MS: u64 = 200;
pub const IR_SYNC_LOOP_INTERVAL_SECS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IrSyncThrottle {
    pub job_spacing_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct IrSyncJobFilter<'a> {
    pub provider_key: &'a str,
    pub account_id: &'a str,
    pub kind: IrJobKind,
}

impl IrSyncThrottle {
    pub const fn none() -> Self {
        Self { job_spacing_ms: 0 }
    }

    pub const fn rate_limited() -> Self {
        Self { job_spacing_ms: IR_SYNC_JOB_SPACING_MS }
    }

    fn job_delay(self) -> Option<std::time::Duration> {
        if self.job_spacing_ms == 0 {
            None
        } else {
            Some(std::time::Duration::from_millis(self.job_spacing_ms))
        }
    }
}

/// 保存済み credentials を読み、失効が近ければ refresh して保存し直す。
pub async fn ensure_fresh_credentials(
    profile_root: &Path,
    provider_key: &str,
    base_url: &str,
    now: i64,
) -> Result<IrStoredCredentials> {
    let _guard = CREDENTIAL_REFRESH_LOCK.lock().await;
    let Some(credentials) = load_credentials(profile_root, provider_key)? else {
        bail!("not signed in to IR provider '{provider_key}'; run `bmz ir login` first");
    };
    if !credentials.needs_refresh(now) {
        return Ok(credentials);
    }
    let client = BmzOfficialIrClient::anonymous(base_url)?;
    let tokens = client
        .refresh(&credentials.refresh_token)
        .await
        .with_context(|| format!("failed to refresh IR token for '{provider_key}'"))?;
    let refreshed = IrStoredCredentials {
        provider: tokens.provider_key,
        account_id: tokens.player.id,
        display_name: tokens.player.display_name.unwrap_or(credentials.display_name),
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_at: tokens.expires_at,
    };
    save_credentials(profile_root, &refreshed)?;
    Ok(refreshed)
}

/// pending / failed (retry時刻到達済み) の IR スコアジョブを送信する。
pub async fn sync_pending_ir_jobs(
    network_db: &mut NetworkDatabase,
    score_db_path: &Path,
    profile_root: &Path,
    logs_dir: &Path,
    ir_config: &IrConfig,
    now: i64,
    limit: u32,
    ignore_retry_backoff: bool,
    throttle: IrSyncThrottle,
) -> Result<IrSyncReport> {
    sync_pending_ir_jobs_with_filter(
        network_db,
        score_db_path,
        profile_root,
        logs_dir,
        ir_config,
        now,
        limit,
        ignore_retry_backoff,
        throttle,
        None,
    )
    .await
}

pub async fn sync_pending_ir_jobs_filtered(
    network_db: &mut NetworkDatabase,
    score_db_path: &Path,
    profile_root: &Path,
    logs_dir: &Path,
    ir_config: &IrConfig,
    filter: IrSyncJobFilter<'_>,
    now: i64,
    limit: u32,
    ignore_retry_backoff: bool,
    throttle: IrSyncThrottle,
) -> Result<IrSyncReport> {
    sync_pending_ir_jobs_with_filter(
        network_db,
        score_db_path,
        profile_root,
        logs_dir,
        ir_config,
        now,
        limit,
        ignore_retry_backoff,
        throttle,
        Some(filter),
    )
    .await
}

mod evidence;
mod payload;
mod process;
mod replay;

use evidence::*;
use payload::*;
use process::*;
use replay::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ln_policy::LnScorePolicy;

    #[test]
    fn ir_sync_throttles_keep_background_and_cli_budgets_separate() {
        assert_eq!(IR_SYNC_BATCH_LIMIT, 20);
        assert_eq!(IR_SYNC_JOB_SPACING_MS, 3_100);
        assert_eq!(IR_CLI_SYNC_BATCH_LIMIT, 100);
        assert_eq!(IR_CLI_SYNC_JOB_SPACING_MS, 200);
        assert_eq!(IR_SYNC_LOOP_INTERVAL_SECS, 30);
        assert_eq!(
            IrSyncThrottle::rate_limited().job_delay(),
            Some(std::time::Duration::from_millis(3_100))
        );
        assert_eq!(IrSyncThrottle::none().job_delay(), None);
    }

    #[test]
    fn replay_verification_rejects_non_verified_status() {
        assert!(ensure_replay_verified("rejected").is_err());
        assert!(ensure_replay_verified("verified").is_ok());
    }

    #[test]
    fn queued_local_backfill_is_blocked_only_for_rian_ir() {
        let payload: IrScoreSubmission = serde_json::from_value(serde_json::json!({
            "client": { "name": "BMZ", "version": "test", "platform": "test" },
            "chart": {
                "sha256": "00",
                "ln_profile": {
                    "has_undefined_ln": false,
                    "has_defined_ln": false,
                    "has_defined_cn": true,
                    "has_defined_hcn": false
                },
                "mode": "7K",
                "notes": { "total": 0, "ln": 0, "cn": 0, "hcn": 0, "mine": 0 },
                "features": {
                    "random": false,
                    "stop": false,
                    "ln": false,
                    "cn": true,
                    "hcn": false,
                    "mine": false
                }
            },
            "rule": {
                "play_mode": "single",
                "key_mode": "7K",
                "gauge": "Hard",
                "ln_policy": "ForceCn",
                "effective_ln_mode": "cn",
                "judge_algorithm": "bmz_v1",
                "scoring": "bms_ex_score_v1"
            },
            "result": {
                "clear": "Hard",
                "played_at": 0,
                "judges": {
                    "fast": { "pgreat": 0, "great": 0, "good": 0, "bad": 0, "poor": 0, "empty_poor": 0 },
                    "slow": { "pgreat": 0, "great": 0, "good": 0, "bad": 0, "poor": 0, "empty_poor": 0 }
                },
                "ex_score": 0,
                "max_combo": 0,
                "notes": 0,
                "min_bp": 0,
                "min_cb": 0
            },
            "play_options": { "submission_source": "local_backfill" },
            "idempotency_key": "test"
        }))
        .unwrap();
        let rian = IrProviderConfig::rian_ir();
        let bmz = IrProviderConfig::bmz_ir();

        let error = ensure_score_payload_allowed(&rian, &payload).unwrap_err();
        assert_eq!(error.to_string(), "rianIR local score backfill is disabled");
        assert!(ensure_score_payload_allowed(&bmz, &payload).is_ok());
    }

    #[test]
    fn ir_submission_log_is_jsonl_under_logs_dir() {
        let stamp =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let logs_dir = std::env::temp_dir()
            .join(format!("bmz-player-ir-submission-log-{}-{stamp}", std::process::id()));
        let job = IrScoreJobRecord {
            id: 7,
            provider: "bmz-official".to_string(),
            account_id: "account-1".to_string(),
            kind: IrJobKind::Score,
            local_score_id: 42,
            chart_sha256: [1; 32],
            ln_policy: LnScorePolicy::ForceLn,
            payload_json: String::new(),
            status: "sending".to_string(),
            attempt_count: 0,
            next_attempt_at: 0,
            last_error: String::new(),
            created_at: 100,
            updated_at: 100,
        };

        let log_path = write_ir_submission_log(
            &logs_dir,
            &job,
            "succeeded",
            "remote-1",
            123,
            "{\"score\":1}",
            "{\"accepted\":true}",
            "",
        );

        assert_eq!(log_path, "ir-submissions.jsonl");
        let line = std::fs::read_to_string(logs_dir.join(&log_path)).unwrap();
        let value: serde_json::Value = serde_json::from_str(line.trim_end()).unwrap();
        assert_eq!(value["provider"], "bmz-official");
        assert_eq!(value["kind"], "score");
        assert_eq!(value["payload"]["score"], 1);
        assert_eq!(value["response"]["accepted"], true);

        let _ = std::fs::remove_dir_all(logs_dir);
    }

    #[test]
    fn legacy_course_payload_defaults_missing_rule_mode() {
        let mut payload = serde_json::json!({
            "play_options": {
                "seed": 1783820891178268800_i64,
                "random_seed": 42
            },
            "rule": {
                "gauge": "Class",
                "ln_policy": "AutoLn",
                "scoring": "bms_ex_score_v1"
            }
        });

        normalize_legacy_course_payload(&mut payload);

        assert_eq!(payload["rule"]["rule_mode"], "Beatoraja");
        assert_eq!(payload["play_options"]["seed"], "1783820891178268800");
        assert_eq!(payload["play_options"]["random_seed"], "42");
    }

    #[test]
    fn legacy_integer_seed_value_becomes_decimal_string() {
        let mut seed = serde_json::json!(1783820891178268800_i64);

        normalize_integer_value_to_string(&mut seed);

        assert_eq!(seed, "1783820891178268800");
    }

    #[test]
    fn legacy_course_payload_keeps_existing_rule_mode() {
        let mut payload = serde_json::json!({
            "rule": {
                "rule_mode": "Dx"
            }
        });

        normalize_legacy_course_payload(&mut payload);

        assert_eq!(payload["rule"]["rule_mode"], "Dx");
    }
}
