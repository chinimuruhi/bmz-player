//! Native submit-only adapter for BMS-IR.
//!
//! The durable queue keeps bmz-player's provider-neutral score payload. This
//! module owns only BMS-IR authentication, eligibility, request wrapping, and
//! response decoding.

use anyhow::{Context, Result, bail};
use bmz_chart::model::{ChartSourceFormat, LongNoteMode};
use bmz_gameplay::rule::RuleMode;
use reqwest::Url;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::config::profile_config::IrProviderConfig;
use crate::ln_policy::{ChartLnProfile, LnScorePolicy, played_ln_mode};
use crate::select_options::DoubleOption;

use super::types::{
    IrAuthTokens, IrEffectiveLnMode, IrPlayerInfo, IrScoreSubmission, IrSubmitResponse,
};

pub const BMS_IR_PROVIDER: &str = "bms-ir";
pub const BMS_IR_PRODUCTION_BASE_URL: &str = "https://www.bms-ir.org";

/// Compile-time override for local integration builds. Normal release builds
/// keep the production endpoint and profile files cannot redirect the fixed
/// BMS-IR credential target.
pub const BMS_IR_DEFAULT_BASE_URL: &str = match option_env!("BMZ_BMS_IR_BASE_URL") {
    Some(value) => value,
    None => BMS_IR_PRODUCTION_BASE_URL,
};

const BMS_IR_KEY_MODES: &[&str] = &["4K", "5K", "6K", "7K", "8K", "9K", "10K", "14K", "24K", "48K"];

fn is_supported_key_mode(mode: &str) -> bool {
    BMS_IR_KEY_MODES.contains(&mode)
}

#[derive(Debug, Clone)]
pub struct BmsIrClient {
    base_url: Url,
    http: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct BmsIrSubmitOutcome {
    pub redacted_request_json: String,
    pub response_json: String,
}

#[derive(Debug, serde::Deserialize)]
struct BmsIrLoginResponse {
    ok: bool,
    player_id: u64,
}

pub fn is_bms_ir_provider(provider: &str) -> bool {
    matches!(provider.trim().to_ascii_lowercase().as_str(), "bms-ir" | "bmsir" | "bms_ir")
}

pub fn is_bms_ir_config(provider: &IrProviderConfig) -> bool {
    is_bms_ir_provider(&provider.provider) || is_bms_ir_provider(&provider.provider_key)
}

pub fn score_submission_supported(
    rule_mode: RuleMode,
    source_format: ChartSourceFormat,
    source_ln_profile: ChartLnProfile,
    ln_policy: LnScorePolicy,
    double_option: DoubleOption,
    is_course_stage: bool,
) -> bool {
    !is_course_stage
        && rule_mode == RuleMode::Beatoraja
        && matches!(
            source_format,
            ChartSourceFormat::Bms | ChartSourceFormat::Bmson | ChartSourceFormat::Pms
        )
        && played_ln_mode(source_ln_profile, ln_policy).is_none_or(|mode| mode == LongNoteMode::Ln)
        && matches!(double_option, DoubleOption::Off | DoubleOption::Flip)
}

pub fn ensure_score_payload_supported(payload: &IrScoreSubmission) -> Result<()> {
    if crate::ir::backfill::is_local_backfill_submission(payload) {
        bail!("BMS-IR local score backfill is disabled");
    }
    if payload.rule.rule_mode != "Beatoraja" {
        bail!("BMS-IR accepts Beatoraja rule mode only");
    }
    if payload.rule.effective_ln_mode != IrEffectiveLnMode::Ln {
        bail!("BMS-IR accepts effective LN scoring only");
    }
    if payload.rule.judge_algorithm != "bmz_v1" || payload.rule.scoring != "bms_ex_score_v1" {
        bail!("BMS-IR score algorithm is not supported");
    }
    if !matches!(payload.chart.source_format.as_str(), "bms" | "bmson" | "pms") {
        bail!("BMS-IR chart source format is not supported");
    }
    if !is_supported_key_mode(&payload.chart.mode) {
        bail!("BMS-IR key mode is not supported");
    }
    if !matches!(
        payload.result.clear.as_str(),
        "Failed" | "Easy" | "Normal" | "Hard" | "ExHard" | "FullCombo" | "Perfect" | "Max"
    ) {
        bail!("BMS-IR clear type is not supported");
    }
    let double_option = payload
        .play_options
        .get("applied_double_option")
        .or_else(|| payload.play_options.get("double_option"))
        .and_then(Value::as_str)
        .unwrap_or("off");
    if !matches!(double_option, "off" | "flip") {
        bail!("BMS-IR accepts OFF or FLIP double option only");
    }
    Ok(())
}

impl BmsIrClient {
    pub fn new(base_url: &str) -> Result<Self> {
        let mut base_url = Url::parse(base_url.trim()).context("invalid BMS-IR base URL")?;
        if !matches!(base_url.scheme(), "http" | "https") {
            bail!("BMS-IR base URL must use HTTP or HTTPS");
        }
        base_url.set_query(None);
        base_url.set_fragment(None);
        Ok(Self {
            base_url,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .context("failed to build BMS-IR HTTP client")?,
        })
    }

    pub async fn login(&self, player_id: &str, game_token: &str) -> Result<IrAuthTokens> {
        let player_id = parse_player_id(player_id)?;
        if game_token.trim().is_empty() {
            bail!("BMS-IR game token is empty");
        }
        let response = self
            .http
            .post(self.endpoint("/api/bmz-player/v1/login")?)
            .json(&serde_json::json!({
                "player_id": player_id,
                "game_token": game_token,
            }))
            .send()
            .await
            .context("failed to send BMS-IR login request")?;
        let decoded: BmsIrLoginResponse = decode_response(response, "BMS-IR login").await?;
        if !decoded.ok || decoded.player_id != player_id {
            bail!("BMS-IR login response did not confirm the requested player ID");
        }
        Ok(IrAuthTokens {
            provider_key: BMS_IR_PROVIDER.to_string(),
            access_token: game_token.to_string(),
            refresh_token: String::new(),
            expires_at: None,
            player: IrPlayerInfo {
                id: player_id.to_string(),
                email: None,
                display_name: Some(player_id.to_string()),
            },
        })
    }

    pub async fn submit_score(
        &self,
        payload: &IrScoreSubmission,
        player_id: &str,
        game_token: &str,
    ) -> Result<BmsIrSubmitOutcome> {
        ensure_score_payload_supported(payload)?;
        let player_id = parse_player_id(player_id)?;
        if game_token.trim().is_empty() {
            bail!("BMS-IR game token is empty");
        }
        let request = score_request_value(player_id, game_token, payload)?;
        let redacted_request_json = redacted_score_request_json(&request)?;
        let response = self
            .http
            .post(self.endpoint("/api/bmz-player/v1/score")?)
            .json(&request)
            .send()
            .await
            .context("failed to send BMS-IR score request")?;
        let decoded: IrSubmitResponse =
            decode_response(response, "BMS-IR score submission").await?;
        if !decoded.accepted {
            bail!("BMS-IR did not accept the score");
        }
        Ok(BmsIrSubmitOutcome {
            redacted_request_json,
            response_json: serde_json::to_string(&decoded)?,
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url> {
        self.base_url.join(path).context("failed to build BMS-IR endpoint URL")
    }
}

fn parse_player_id(value: &str) -> Result<u64> {
    let player_id = value.trim().parse::<u64>().context("BMS-IR ID must be a positive integer")?;
    if player_id == 0 {
        bail!("BMS-IR ID must be a positive integer");
    }
    Ok(player_id)
}

fn score_request_value<T: Serialize>(player_id: u64, game_token: &str, score: &T) -> Result<Value> {
    Ok(serde_json::json!({
        "player_id": player_id,
        "game_token": game_token,
        "score": score,
    }))
}

fn redacted_score_request_json(request: &Value) -> Result<String> {
    let mut redacted = request.clone();
    if let Some(object) = redacted.as_object_mut() {
        object.insert("game_token".to_string(), Value::String("<redacted>".to_string()));
    }
    Ok(serde_json::to_string(&redacted)?)
}

async fn decode_response<T: DeserializeOwned>(
    response: reqwest::Response,
    context: &str,
) -> Result<T> {
    let status = response.status();
    let body =
        response.bytes().await.with_context(|| format!("failed to read {context} response"))?;
    if !status.is_success() {
        let error = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|value| value.get("error")?.as_str().map(str::to_string))
            .unwrap_or_else(|| "request rejected".to_string());
        bail!("{context} failed with HTTP {}: {error}", status.as_u16());
    }
    serde_json::from_slice(&body).with_context(|| format!("failed to decode {context} response"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_aliases_and_queue_eligibility_are_narrow() {
        assert!(is_bms_ir_provider("bms-ir"));
        assert!(is_bms_ir_provider("BMSIR"));
        assert!(!is_bms_ir_provider("bmz"));

        let profile = ChartLnProfile { has_undefined_ln: true, ..Default::default() };
        assert!(score_submission_supported(
            RuleMode::Beatoraja,
            ChartSourceFormat::Bms,
            profile,
            LnScorePolicy::AutoLn,
            DoubleOption::Off,
            false,
        ));
        assert!(!score_submission_supported(
            RuleMode::Lr2Oraja,
            ChartSourceFormat::Bms,
            profile,
            LnScorePolicy::AutoLn,
            DoubleOption::Off,
            false,
        ));
        assert!(!score_submission_supported(
            RuleMode::Beatoraja,
            ChartSourceFormat::Bmson,
            profile,
            LnScorePolicy::ForceCn,
            DoubleOption::Off,
            false,
        ));
        assert!(!score_submission_supported(
            RuleMode::Beatoraja,
            ChartSourceFormat::Pms,
            profile,
            LnScorePolicy::AutoLn,
            DoubleOption::Battle,
            false,
        ));
        assert!(!score_submission_supported(
            RuleMode::Beatoraja,
            ChartSourceFormat::Bms,
            profile,
            LnScorePolicy::AutoLn,
            DoubleOption::Off,
            true,
        ));
    }

    #[test]
    fn canonical_bms_ir_key_modes_include_4k_6k_and_8k() {
        for mode in BMS_IR_KEY_MODES {
            assert!(is_supported_key_mode(mode), "{mode}");
        }
        assert!(!is_supported_key_mode("3K"));
    }

    #[test]
    fn request_log_redacts_game_token_without_changing_score() {
        let request = score_request_value(
            123,
            "secret-game-token",
            &serde_json::json!({"idempotency_key": "score-1"}),
        )
        .unwrap();
        let redacted = redacted_score_request_json(&request).unwrap();
        assert!(!redacted.contains("secret-game-token"));
        let decoded: Value = serde_json::from_str(&redacted).unwrap();
        assert_eq!(decoded["game_token"], "<redacted>");
        assert_eq!(decoded["score"]["idempotency_key"], "score-1");
    }
}
