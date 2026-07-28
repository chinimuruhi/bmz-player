//! rianIR の legacy HTTP API adapter。
//!
//! BMZ 内部の provider-neutral payload と rianIR wire payload の変換を
//! この module に閉じ込める。rianIR 側の既存 API / DB schema は変更しない。

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use base64::Engine;
use bmz_gameplay::rule::RuleMode;
use reqwest::Url;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::config::profile_config::IrProviderConfig;
use crate::ln_policy::{LnPolicySetting, LnScorePolicy};
use crate::select_options::DoubleOption;

use super::types::{
    IrAuthTokens, IrCourseRankingBody, IrCourseRankingCourseRef, IrCourseRankingEntry,
    IrCourseRankingResult, IrCourseRankingScore, IrJudgePayload, IrJudgeSidePayload, IrPlayerInfo,
    IrRankingBody, IrRankingChartRef, IrRankingEntry, IrRankingPagination, IrRankingPlayer,
    IrRankingResult, IrRankingScope, IrRankingScore, IrRankingSelfRef, IrScoreSubmission,
    IrSubmitResponse,
};

pub const RIAN_IR_PROVIDER: &str = "rian-ir";
pub const RIAN_IR_DEFAULT_BASE_URL: &str = "https://rianir.link/api/";
pub const RIAN_IR_PUBLIC_BASE_URL: &str = "https://rianir.link/";

#[derive(Debug, Clone)]
pub struct RianIrClient {
    base_url: Url,
    http: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct RianSubmitOutcome {
    pub redacted_request_json: String,
    pub response_json: String,
}

#[derive(Debug, serde::Deserialize)]
struct RianLoginResponse {
    data: RianLoginData,
}

#[derive(Debug, serde::Deserialize)]
struct RianLoginData {
    attributes: RianLoginAttributes,
    meta: RianLoginMeta,
}

#[derive(Debug, serde::Deserialize)]
struct RianLoginAttributes {
    player_name: String,
}

#[derive(Debug, serde::Deserialize)]
struct RianLoginMeta {
    api_token: String,
}

#[derive(Debug, serde::Deserialize)]
struct RianRankingResponse {
    #[serde(default)]
    data: Vec<RianRankingResource>,
}

#[derive(Debug, serde::Deserialize)]
struct RianRankingResource {
    #[serde(default)]
    id: String,
    #[serde(default)]
    attributes: Map<String, Value>,
}

impl RianIrClient {
    pub fn new(base_url: &str) -> Result<Self> {
        Ok(Self {
            base_url: parse_base_url(base_url)?,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .context("failed to build rianIR HTTP client")?,
        })
    }

    pub async fn login(&self, login_id: &str, password: &str) -> Result<IrAuthTokens> {
        let response = self
            .http
            .post(self.endpoint("auth/login.php")?)
            .json(&json!({ "id": login_id, "pass": password }))
            .send()
            .await
            .context("failed to send rianIR login request")?;
        let decoded: RianLoginResponse = decode_response(response, "rianIR login").await?;
        if decoded.data.meta.api_token.is_empty() {
            bail!("rianIR login response did not contain api_token");
        }
        Ok(IrAuthTokens {
            provider_key: RIAN_IR_PROVIDER.to_string(),
            access_token: decoded.data.meta.api_token,
            refresh_token: String::new(),
            expires_at: None,
            // rianIR submit API requires the login ID as player_name. The response's
            // attributes.player_name is the display name, not the login ID.
            player: IrPlayerInfo {
                id: login_id.to_string(),
                email: None,
                display_name: Some(decoded.data.attributes.player_name),
            },
        })
    }

    pub async fn submit_score(
        &self,
        payload: &IrScoreSubmission,
        player_id: &str,
        api_token: &str,
    ) -> Result<RianSubmitOutcome> {
        ensure_score_payload_supported(payload)?;
        let request = score_request(payload, player_id, api_token)?;
        let redacted_request_json = redacted_request_json(&request)?;
        let response = self
            .http
            .post(self.endpoint("score/score.php")?)
            .json(&request)
            .send()
            .await
            .context("failed to send rianIR score request")?;
        let response_value: Value = decode_response(response, "rianIR score submission").await?;
        ensure_success_status(&response_value, "rianIR score submission")?;
        Ok(RianSubmitOutcome {
            redacted_request_json,
            response_json: serde_json::to_string(&IrSubmitResponse {
                accepted: true,
                score_id: None,
                best_updated: false,
                previous_best: None,
                rankings: BTreeMap::new(),
            })?,
        })
    }

    pub async fn submit_course_score(
        &self,
        payload: &Value,
        player_id: &str,
        api_token: &str,
    ) -> Result<RianSubmitOutcome> {
        let request = course_request(payload, player_id, api_token)?;
        let redacted_request_json = redacted_request_json(&request)?;
        let response = self
            .http
            .post(self.endpoint("score/course_score.php")?)
            .json(&request)
            .send()
            .await
            .context("failed to send rianIR course score request")?;
        let response_value: Value =
            decode_response(response, "rianIR course score submission").await?;
        ensure_success_status(&response_value, "rianIR course score submission")?;
        Ok(RianSubmitOutcome {
            redacted_request_json,
            response_json: serde_json::to_string(&json!({
                "status": "success",
                "course_score_id": Value::Null,
            }))?,
        })
    }

    pub async fn fetch_ranking(
        &self,
        chart_sha256: &str,
        body: &str,
        scope: IrRankingScope,
        limit: u32,
        self_player_id: Option<&str>,
    ) -> Result<IrRankingResult> {
        if scope != IrRankingScope::Global {
            bail!("rianIR supports global ranking scope only");
        }
        let mut url = self.endpoint("score/get_score.php")?;
        url.query_pairs_mut().append_pair("sha256", chart_sha256).append_pair("body", body);
        let response = self.http.get(url).send().await.context("failed to fetch rianIR ranking")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(empty_ranking(chart_sha256, limit));
        }
        let decoded: RianRankingResponse = decode_response(response, "rianIR ranking").await?;
        Ok(convert_score_ranking(chart_sha256, decoded.data, limit, self_player_id))
    }

    pub async fn fetch_course_ranking(
        &self,
        course_hash: &str,
        body: &str,
        limit: u32,
    ) -> Result<IrCourseRankingResult> {
        let mut url = self.endpoint("score/get_course_score.php")?;
        url.query_pairs_mut().append_pair("course_sha256", course_hash).append_pair("body", body);
        let response =
            self.http.get(url).send().await.context("failed to fetch rianIR course ranking")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(empty_course_ranking(course_hash));
        }
        let decoded: RianRankingResponse =
            decode_response(response, "rianIR course ranking").await?;
        Ok(convert_course_ranking(course_hash, decoded.data, limit))
    }

    fn endpoint(&self, relative: &str) -> Result<Url> {
        self.base_url.join(relative).context("failed to build rianIR endpoint URL")
    }
}

pub fn is_rian_ir_provider(provider: &str) -> bool {
    matches!(provider.trim().to_ascii_lowercase().as_str(), "rian-ir" | "rianir")
}

pub fn is_rian_ir_config(provider: &IrProviderConfig) -> bool {
    is_rian_ir_provider(&provider.provider) || is_rian_ir_provider(&provider.provider_key)
}

pub fn score_submission_supported(ln_policy: LnScorePolicy, double_option: DoubleOption) -> bool {
    matches!(ln_policy, LnScorePolicy::ForceLn | LnScorePolicy::ForceCn | LnScorePolicy::ForceHcn)
        && !matches!(double_option, DoubleOption::Battle | DoubleOption::BattleAutoScratch)
}

pub fn course_submission_supported(
    ln_setting: LnPolicySetting,
    double_option: DoubleOption,
) -> bool {
    ln_setting.is_force()
        && !matches!(double_option, DoubleOption::Battle | DoubleOption::BattleAutoScratch)
}

pub fn body_for_rule_mode(rule_mode: RuleMode) -> &'static str {
    match rule_mode {
        RuleMode::Beatoraja => "beatoraja",
        RuleMode::Lr2Oraja => "LR2oraja",
        RuleMode::Dx => "DX MODE",
    }
}

pub fn body_for_rule_name(rule_mode: &str) -> Result<&'static str> {
    match rule_mode {
        "Beatoraja" | "beatoraja" => Ok("beatoraja"),
        "Lr2Oraja" | "LR2oraja" => Ok("LR2oraja"),
        "Dx" | "DX MODE" => Ok("DX MODE"),
        other => bail!("unsupported rianIR rule mode '{other}'"),
    }
}

fn ensure_score_payload_supported(payload: &IrScoreSubmission) -> Result<()> {
    let double =
        payload.play_options.get("applied_double_option").and_then(Value::as_str).unwrap_or("off");
    if !matches!(
        payload.rule.ln_policy,
        LnScorePolicy::ForceLn | LnScorePolicy::ForceCn | LnScorePolicy::ForceHcn
    ) {
        bail!("rianIR sends only normalized FORCE LN/CN/HCN scores");
    }
    if matches!(double, "battle" | "battle_auto_scratch" | "battle_assist") {
        bail!("rianIR does not accept BATTLE / BATTLE AS scores");
    }
    Ok(())
}

fn score_request(payload: &IrScoreSubmission, player_id: &str, api_token: &str) -> Result<Value> {
    let judges = aggregate_judges(payload.result.judges);
    let body = body_for_rule_name(&payload.rule.rule_mode)?;
    let effective_ln_mode = effective_ln_mode_id(payload.rule.ln_policy);
    let played_at = payload.result.played_at;
    let mut request = json!({
        "player_name": player_id,
        "api_token": api_token,
        "client_hash": super::client_hash::current_client_hash(),
        "client_git_commit": option_env!("GIT_COMMIT_HASH").unwrap_or("unknown"),
        "client": "bmz-player",
        "client_version": env!("CARGO_PKG_VERSION"),
        "sha256": payload.chart.sha256,
        "md5": payload.chart.md5.as_deref().unwrap_or(""),
        // rianIR の既存 Java connector と同じ B64 prefix を使う。
        "song_title": b64(&payload.chart.title),
        "subtitle": b64(&payload.chart.subtitle),
        "genre": b64(&payload.chart.genre),
        "artist": b64(&payload.chart.artist),
        "subartist": b64(&payload.chart.subartists.join(", ")),
        "play_mode": play_mode(&payload.chart.mode),
        "song_ln_mode": effective_ln_mode,
        "ln_mode": client_ln_mode(payload.rule.ln_policy),
        "minbpm": payload.chart.bpm.and_then(|bpm| bpm.min).unwrap_or(0.0),
        "maxbpm": payload.chart.bpm.and_then(|bpm| bpm.max).unwrap_or(0.0),
        "song_level": payload.chart.level.unwrap_or(0),
        "length": 0,
        "clear_type": clear_type_id(&payload.result.clear)?,
        "exscore": payload.result.ex_score,
        "maxcombo": payload.result.max_combo,
        "minbp": payload.result.min_bp,
        "date": played_at,
        "pgreat": judges.pgreat,
        "great": judges.great,
        "good": judges.good,
        "bad": judges.bad,
        "poor": judges.poor,
        "miss": judges.miss,
        "play_option": 0,
        "arrange_1p": arrange_value(payload, "arrange_1p"),
        "arrange_2p": arrange_value(payload, "arrange_2p"),
        "double_option": double_option_value(payload),
        "play_seed": seed_value(payload),
        "play_assist": 0,
        "play_gauge": gauge_type_id(&payload.rule.gauge)?,
        "total_notes": payload.result.notes,
        // length=0 の場合は現行 server の比率検査対象外。実プレイ時間自体は秒で保存する。
        "play_duration": payload.result.duration_ms.map(|ms| ms as f64 / 1000.0).unwrap_or(0.0),
        "body": body,
    });
    let signature = signature(
        api_token,
        &[
            player_id.to_string(),
            payload.chart.sha256.clone(),
            payload.result.ex_score.to_string(),
            payload.result.max_combo.to_string(),
            payload.result.min_bp.to_string(),
            played_at.to_string(),
        ],
    )?;
    request["signature"] = Value::String(signature);
    Ok(request)
}

fn course_request(payload: &Value, player_id: &str, api_token: &str) -> Result<Value> {
    let course = payload.get("course").context("course payload is missing course")?;
    let rule = payload.get("rule").context("course payload is missing rule")?;
    let result = payload.get("result").context("course payload is missing result")?;
    let play_options = payload.get("play_options").unwrap_or(&Value::Null);
    let course_title = course.get("title").and_then(Value::as_str).unwrap_or("Unknown Course");
    let charts: Vec<String> = course
        .get("charts")
        .and_then(Value::as_array)
        .context("course payload is missing charts")?
        .iter()
        .map(|chart| {
            chart
                .as_str()
                .map(str::to_string)
                .context("course payload contains a non-string chart hash")
        })
        .collect::<Result<_>>()?;
    if charts.is_empty() {
        bail!("course payload has no chart hashes");
    }
    let course_hash = super::course_payload::compute_rian_course_hash_v1(course_title, &charts);
    let played_at = required_i64(result, "played_at")?;
    let ex_score = required_u64(result, "ex_score")?;
    let max_combo = required_u64(result, "max_combo")?;
    let bp = required_u64(result, "bp")?;
    let body = body_for_rule_name(required_str(rule, "rule_mode")?)?;
    let judges = result.get("judges").and_then(Value::as_object);
    let total_notes = result
        .get("total_notes")
        .and_then(Value::as_u64)
        .or_else(|| result.get("max_ex_score").and_then(Value::as_u64).map(|v| v / 2))
        .unwrap_or(0);
    let ln_policy = required_str(rule, "ln_policy")?;
    if !matches!(ln_policy, "ForceLn" | "ForceCn" | "ForceHcn") {
        bail!("rianIR sends only FORCE LN/CN/HCN course scores");
    }
    let arrange =
        normalized_arrange(play_options.get("option").and_then(Value::as_str).unwrap_or("normal"));
    let mut request = json!({
        "player_name": player_id,
        "api_token": api_token,
        "client_hash": super::client_hash::current_client_hash(),
        "client_git_commit": option_env!("GIT_COMMIT_HASH").unwrap_or("unknown"),
        "client": "bmz-player",
        "client_version": env!("CARGO_PKG_VERSION"),
        "course_sha256": course_hash,
        "course_md5": "",
        "course_title": b64(course_title),
        "clear_type": clear_type_id(required_str(result, "clear")?)?,
        "exscore": ex_score,
        "maxcombo": max_combo,
        "minbp": bp,
        "date": played_at,
        "pgreat": judge_value(judges, "pgreat"),
        "great": judge_value(judges, "great"),
        "good": judge_value(judges, "good"),
        "bad": judge_value(judges, "bad"),
        "poor": judge_value(judges, "poor"),
        "miss": judge_value(judges, "empty_poor"),
        "play_option": 0,
        "arrange_1p": arrange,
        "arrange_2p": "normal",
        "double_option": "off",
        "play_seed": value_seed(play_options),
        "play_assist": 0,
        "play_gauge": gauge_type_id(required_str(rule, "gauge")?)?,
        "total_notes": total_notes,
        "ln_mode": effective_ln_mode_id_from_name(ln_policy)?,
        "body": body,
        "constraint": constraint_names(course.get("constraints").unwrap_or(&Value::Null)),
        // BMZ の course queue は stage の hash だけを保持しており、曲名などを
        // 正確に再構成できない。rianIR 側へ誤った chart metadata を登録しないため、
        // 初期版では任意フィールドの tracks を空にする。
        "tracks": [],
    });
    request["signature"] = Value::String(signature(
        api_token,
        &[
            player_id.to_string(),
            course_hash.clone(),
            ex_score.to_string(),
            max_combo.to_string(),
            played_at.to_string(),
        ],
    )?);
    Ok(request)
}

#[derive(Debug, Clone, Copy)]
struct AggregatedJudges {
    pgreat: u32,
    great: u32,
    good: u32,
    bad: u32,
    poor: u32,
    miss: u32,
}

fn aggregate_judges(judges: IrJudgePayload) -> AggregatedJudges {
    AggregatedJudges {
        pgreat: judges.fast.pgreat.saturating_add(judges.slow.pgreat),
        great: judges.fast.great.saturating_add(judges.slow.great),
        good: judges.fast.good.saturating_add(judges.slow.good),
        bad: judges.fast.bad.saturating_add(judges.slow.bad),
        poor: judges.fast.poor.saturating_add(judges.slow.poor),
        // BMZ EmptyPoor is rianIR/beatoraja MISS.
        miss: judges.fast.empty_poor.saturating_add(judges.slow.empty_poor),
    }
}

fn play_mode(mode: &str) -> String {
    match mode {
        "9K" => "popn-9k".to_string(),
        other if other.ends_with('K') => format!("beat-{}", other.to_ascii_lowercase()),
        other => other.to_string(),
    }
}

fn effective_ln_mode_id(policy: LnScorePolicy) -> u8 {
    match policy {
        LnScorePolicy::AutoLn | LnScorePolicy::ForceLn => 1,
        LnScorePolicy::AutoCn | LnScorePolicy::ForceCn => 2,
        LnScorePolicy::AutoHcn | LnScorePolicy::ForceHcn => 3,
    }
}

fn effective_ln_mode_id_from_name(policy: &str) -> Result<u8> {
    match policy {
        "ForceLn" => Ok(1),
        "ForceCn" => Ok(2),
        "ForceHcn" => Ok(3),
        other => bail!("unsupported rianIR LN policy '{other}'"),
    }
}

fn client_ln_mode(policy: LnScorePolicy) -> u8 {
    match policy {
        LnScorePolicy::AutoLn | LnScorePolicy::ForceLn => 0,
        LnScorePolicy::AutoCn | LnScorePolicy::ForceCn => 1,
        LnScorePolicy::AutoHcn | LnScorePolicy::ForceHcn => 2,
    }
}

fn clear_type_id(clear: &str) -> Result<u8> {
    Ok(match clear {
        "NoPlay" => 0,
        "Failed" => 1,
        "AssistEasy" => 2,
        "LightAssistEasy" => 3,
        "Easy" => 4,
        "Normal" => 5,
        "Hard" => 6,
        "ExHard" => 7,
        "FullCombo" => 8,
        "Perfect" => 9,
        "Max" => 10,
        other => bail!("unsupported rianIR clear type '{other}'"),
    })
}

fn clear_type_name(clear: i64) -> String {
    match clear {
        0 => "NoPlay",
        1 => "Failed",
        2 => "AssistEasy",
        3 => "LightAssistEasy",
        4 => "Easy",
        5 => "Normal",
        6 => "Hard",
        7 => "ExHard",
        8 => "FullCombo",
        9 => "Perfect",
        10 => "Max",
        _ => "NoPlay",
    }
    .to_string()
}

fn gauge_type_id(gauge: &str) -> Result<u8> {
    Ok(match gauge {
        "AssistEasy" => 0,
        "Easy" => 1,
        "Normal" => 2,
        "Hard" => 3,
        "ExHard" => 4,
        "Hazard" => 5,
        "Class" => 6,
        "ExClass" => 7,
        "ExHardClass" => 8,
        other => bail!("unsupported rianIR gauge '{other}'"),
    })
}

fn arrange_value(payload: &IrScoreSubmission, key: &str) -> String {
    normalized_arrange(payload.play_options.get(key).and_then(Value::as_str).unwrap_or("normal"))
}

fn normalized_arrange(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "all-scr" | "allscratch" => "all-scratch".to_string(),
        other => other.to_string(),
    }
}

fn double_option_value(payload: &IrScoreSubmission) -> String {
    payload
        .play_options
        .get("applied_double_option")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "off" | "flip"))
        .unwrap_or("off")
        .to_string()
}

fn seed_value(payload: &IrScoreSubmission) -> i64 {
    value_seed(&json!(payload.play_options))
}

fn value_seed(play_options: &Value) -> i64 {
    ["random_seed", "seed"]
        .iter()
        .find_map(|key| {
            let value = play_options.get(*key)?;
            value.as_i64().or_else(|| value.as_str()?.parse().ok())
        })
        .unwrap_or(0)
}

fn b64(value: &str) -> String {
    format!("B64:{}", base64::engine::general_purpose::STANDARD.encode(value.as_bytes()))
}

fn signature(api_token: &str, fields: &[String]) -> Result<String> {
    let data = serde_json::to_string(fields)?;
    Ok(hmac_sha256_hex(api_token.as_bytes(), data.as_bytes()))
}

fn hmac_sha256_hex(key: &[u8], data: &[u8]) -> String {
    const BLOCK: usize = 64;
    let mut normalized_key = [0_u8; BLOCK];
    if key.len() > BLOCK {
        normalized_key[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        normalized_key[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36_u8; BLOCK];
    let mut outer_pad = [0x5c_u8; BLOCK];
    for index in 0..BLOCK {
        inner_pad[index] ^= normalized_key[index];
        outer_pad[index] ^= normalized_key[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(data);
    let inner_hash = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_hash);
    outer.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn convert_score_ranking(
    chart_sha256: &str,
    resources: Vec<RianRankingResource>,
    limit: u32,
    self_player_id: Option<&str>,
) -> IrRankingResult {
    let entries: Vec<_> = resources
        .into_iter()
        .take(limit as usize)
        .enumerate()
        .map(|(index, resource)| score_ranking_entry(index, resource))
        .collect();
    let self_summary = self_player_id.and_then(|player_id| {
        entries
            .iter()
            .find(|entry| entry.player.id == player_id)
            .map(|entry| IrRankingSelfRef { rank: entry.rank, score_id: None })
    });
    let total = entries.len() as u32;
    IrRankingResult {
        chart: IrRankingChartRef { sha256: chart_sha256.to_string() },
        ranking: IrRankingBody {
            scope: IrRankingScope::Global,
            entries,
            clear_rate: None,
            self_summary,
            pagination: Some(IrRankingPagination {
                limit,
                offset: 0,
                total: Some(total),
                has_more: false,
            }),
        },
    }
}

fn score_ranking_entry(index: usize, resource: RianRankingResource) -> IrRankingEntry {
    let attributes = resource.attributes;
    let player_id = string_attr(&attributes, "player_name");
    let display_name =
        non_empty_attr(&attributes, "display_name").unwrap_or_else(|| player_id.clone());
    IrRankingEntry {
        rank: index as u32 + 1,
        scope_rank: None,
        player: IrRankingPlayer { id: player_id, display_name },
        score: IrRankingScore {
            clear: clear_type_name(int_attr(&attributes, "clear_type")),
            ex_score: uint_attr(&attributes, "ex_score"),
            max_combo: uint_attr(&attributes, "max_combo"),
            min_bp: uint_attr(&attributes, "min_bp"),
            min_cb: uint_attr(&attributes, "min_bp"),
            judges: Some(ranking_judges(&attributes)),
            device_type: None,
            played_at: non_empty_attr(&attributes, "play_date"),
        },
    }
}

fn convert_course_ranking(
    course_hash: &str,
    resources: Vec<RianRankingResource>,
    limit: u32,
) -> IrCourseRankingResult {
    let entries = resources
        .into_iter()
        .take(limit as usize)
        .enumerate()
        .map(|(index, resource)| {
            let attributes = resource.attributes;
            let player_id = string_attr(&attributes, "player_name");
            let display_name =
                non_empty_attr(&attributes, "display_name").unwrap_or_else(|| player_id.clone());
            IrCourseRankingEntry {
                rank: index as u32 + 1,
                player: IrRankingPlayer { id: player_id, display_name },
                score: IrCourseRankingScore {
                    course_score_id: resource.id,
                    clear: clear_type_name(int_attr(&attributes, "clear_type")),
                    course_clear: int_attr(&attributes, "clear_type") > 1,
                    ex_score: uint_attr(&attributes, "ex_score"),
                    max_combo: uint_attr(&attributes, "max_combo"),
                    bp: uint_attr(&attributes, "min_bp"),
                    device_type: None,
                    played_at: non_empty_attr(&attributes, "play_date"),
                    verification: None,
                },
            }
        })
        .collect();
    IrCourseRankingResult {
        course: IrCourseRankingCourseRef { course_hash: course_hash.to_string() },
        rule: None,
        ranking: IrCourseRankingBody { scope: IrRankingScope::Global, entries },
    }
}

fn empty_ranking(chart_sha256: &str, limit: u32) -> IrRankingResult {
    IrRankingResult {
        chart: IrRankingChartRef { sha256: chart_sha256.to_string() },
        ranking: IrRankingBody {
            scope: IrRankingScope::Global,
            entries: Vec::new(),
            clear_rate: None,
            self_summary: None,
            pagination: Some(IrRankingPagination {
                limit,
                offset: 0,
                total: Some(0),
                has_more: false,
            }),
        },
    }
}

fn empty_course_ranking(course_hash: &str) -> IrCourseRankingResult {
    IrCourseRankingResult {
        course: IrCourseRankingCourseRef { course_hash: course_hash.to_string() },
        rule: None,
        ranking: IrCourseRankingBody { scope: IrRankingScope::Global, entries: Vec::new() },
    }
}

fn ranking_judges(attributes: &Map<String, Value>) -> IrJudgePayload {
    IrJudgePayload {
        fast: IrJudgeSidePayload {
            pgreat: uint_attr(attributes, "pgreat"),
            great: uint_attr(attributes, "great"),
            good: uint_attr(attributes, "good"),
            bad: uint_attr(attributes, "bad"),
            poor: uint_attr(attributes, "poor"),
            empty_poor: uint_attr(attributes, "miss"),
        },
        slow: IrJudgeSidePayload { pgreat: 0, great: 0, good: 0, bad: 0, poor: 0, empty_poor: 0 },
    }
}

fn string_attr(attributes: &Map<String, Value>, key: &str) -> String {
    attributes.get(key).and_then(value_as_string).unwrap_or_default()
}

fn non_empty_attr(attributes: &Map<String, Value>, key: &str) -> Option<String> {
    let value = string_attr(attributes, key);
    (!value.is_empty() && value != "0").then_some(value)
}

fn int_attr(attributes: &Map<String, Value>, key: &str) -> i64 {
    attributes
        .get(key)
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
        .unwrap_or(0)
}

fn uint_attr(attributes: &Map<String, Value>, key: &str) -> u32 {
    int_attr(attributes, key).max(0).min(i64::from(u32::MAX)) as u32
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn required_str<'a>(object: &'a Value, key: &str) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("course payload is missing string '{key}'"))
}

fn required_i64(object: &Value, key: &str) -> Result<i64> {
    object
        .get(key)
        .and_then(Value::as_i64)
        .with_context(|| format!("course payload is missing integer '{key}'"))
}

fn required_u64(object: &Value, key: &str) -> Result<u64> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .with_context(|| format!("course payload is missing unsigned integer '{key}'"))
}

fn judge_value(judges: Option<&Map<String, Value>>, key: &str) -> u64 {
    judges.and_then(|judges| judges.get(key)).and_then(Value::as_u64).unwrap_or(0)
}

fn constraint_names(value: &Value) -> Vec<String> {
    match value {
        Value::Array(values) => values.iter().filter_map(value_as_string).collect(),
        Value::Object(values) => values
            .iter()
            .filter_map(|(key, value)| match value {
                Value::Bool(true) => Some(key.clone()),
                Value::String(value) if !value.is_empty() => Some(value.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn redacted_request_json(request: &Value) -> Result<String> {
    let mut redacted = request.clone();
    if let Some(object) = redacted.as_object_mut() {
        for key in ["api_token", "signature"] {
            if object.contains_key(key) {
                object.insert(key.to_string(), Value::String("[REDACTED]".to_string()));
            }
        }
    }
    Ok(serde_json::to_string(&redacted)?)
}

fn parse_base_url(base_url: &str) -> Result<Url> {
    let mut url = Url::parse(base_url).context("invalid rianIR base URL")?;
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    if !url.path().ends_with("/api/") {
        url = url.join("api/").context("failed to normalize rianIR API base URL")?;
    }
    Ok(url)
}

async fn decode_response<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    context: &str,
) -> Result<T> {
    let status = response.status();
    let body = response.text().await.context("failed to read rianIR response body")?;
    if !status.is_success() {
        let detail = error_detail(&body).unwrap_or_else(|| body.chars().take(500).collect());
        bail!("{context} failed with HTTP {status}: {detail}");
    }
    serde_json::from_str(&body).with_context(|| format!("{context} returned invalid JSON"))
}

fn error_detail(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    value
        .get("errors")
        .and_then(Value::as_array)
        .and_then(|errors| errors.first())
        .and_then(|error| {
            error.get("detail").or_else(|| error.get("title")).and_then(Value::as_str)
        })
        .or_else(|| value.get("message").and_then(Value::as_str))
        .map(str::to_string)
}

fn ensure_success_status(value: &Value, context: &str) -> Result<()> {
    if value.get("status").and_then(Value::as_str) == Some("success") {
        Ok(())
    } else {
        bail!(
            "{context} was not accepted: {}",
            error_detail(&value.to_string()).unwrap_or_else(|| value.to_string())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::types::{
        IrChartFeatures, IrChartLnProfile, IrChartNotes, IrChartPayload, IrClientInfo,
        IrEffectiveLnMode, IrRulePayload,
    };

    fn sample_payload() -> IrScoreSubmission {
        IrScoreSubmission {
            client: IrClientInfo {
                name: "BMZ".to_string(),
                version: "0.1.11".to_string(),
                platform: "test".to_string(),
            },
            chart: IrChartPayload {
                sha256: "ab".repeat(32),
                md5: Some("cd".repeat(16)),
                ln_profile: IrChartLnProfile::default(),
                title: "タイトル".to_string(),
                subtitle: String::new(),
                genre: "genre".to_string(),
                artist: "artist".to_string(),
                subartists: Vec::new(),
                mode: "8K".to_string(),
                level: Some(12),
                difficulty: String::new(),
                total: None,
                judge: None,
                bpm: None,
                notes: IrChartNotes { total: 100, ..Default::default() },
                features: IrChartFeatures::default(),
                urls: None,
                headers: BTreeMap::new(),
            },
            rule: IrRulePayload {
                play_mode: "single".to_string(),
                key_mode: "8K".to_string(),
                gauge: "Hard".to_string(),
                ln_policy: LnScorePolicy::ForceCn,
                effective_ln_mode: IrEffectiveLnMode::Cn,
                judge_algorithm: "bmz_v1".to_string(),
                scoring: "bms_ex_score_v1".to_string(),
                rule_mode: "Beatoraja".to_string(),
            },
            result: super::super::types::IrResultPayload {
                clear: "Hard".to_string(),
                played_at: 1_700_000_000,
                duration_ms: Some(120_000),
                judges: IrJudgePayload {
                    fast: IrJudgeSidePayload {
                        pgreat: 50,
                        great: 10,
                        good: 1,
                        bad: 2,
                        poor: 3,
                        empty_poor: 4,
                    },
                    slow: IrJudgeSidePayload {
                        pgreat: 40,
                        great: 5,
                        good: 0,
                        bad: 1,
                        poor: 2,
                        empty_poor: 1,
                    },
                },
                ex_score: 195,
                max_combo: 90,
                notes: 100,
                pass_notes: None,
                min_bp: 8,
                min_cb: 9,
                ghost: None,
            },
            play_options: BTreeMap::from([
                ("arrange_1p".to_string(), json!("f-random")),
                ("arrange_2p".to_string(), json!("mf-random")),
                ("applied_double_option".to_string(), json!("off")),
                ("random_seed".to_string(), json!("123456")),
            ]),
            replay: None,
            evidence: BTreeMap::new(),
            idempotency_key: "test".to_string(),
        }
    }

    #[test]
    fn provider_aliases_are_recognized() {
        assert!(is_rian_ir_provider("rian-ir"));
        assert!(is_rian_ir_provider("rianIR"));
        assert!(!is_rian_ir_provider("bmz-official"));
    }

    #[test]
    fn score_eligibility_rejects_auto_and_battle() {
        assert!(score_submission_supported(LnScorePolicy::ForceLn, DoubleOption::Off));
        assert!(score_submission_supported(LnScorePolicy::ForceHcn, DoubleOption::Flip));
        assert!(!score_submission_supported(LnScorePolicy::AutoLn, DoubleOption::Off));
        assert!(!score_submission_supported(
            LnScorePolicy::ForceCn,
            DoubleOption::BattleAutoScratch
        ));
        assert!(course_submission_supported(LnPolicySetting::ForceLn, DoubleOption::Off));
        assert!(!course_submission_supported(LnPolicySetting::AutoLn, DoubleOption::Off));
        assert!(!course_submission_supported(LnPolicySetting::ForceHcn, DoubleOption::Battle));
    }

    #[test]
    fn rule_modes_keep_legacy_body_contract() {
        assert_eq!(body_for_rule_mode(RuleMode::Beatoraja), "beatoraja");
        assert_eq!(body_for_rule_mode(RuleMode::Lr2Oraja), "LR2oraja");
        assert_eq!(body_for_rule_mode(RuleMode::Dx), "DX MODE");
    }

    #[test]
    fn score_request_maps_force_ln_modes_and_extended_key_modes() {
        let request = score_request(&sample_payload(), "player", "token").unwrap();
        assert_eq!(request["body"], "beatoraja");
        assert!(request.get("rule_mode").is_none());
        assert_eq!(request["client"], "bmz-player");
        assert_eq!(request["play_mode"], "beat-8k");
        assert_eq!(request["song_ln_mode"], 2);
        assert_eq!(request["ln_mode"], 1);
        assert_eq!(request["arrange_1p"], "f-random");
        assert_eq!(request["arrange_2p"], "mf-random");
        assert_eq!(request["play_seed"], 123456);
        assert_eq!(request["pgreat"], 90);
        assert_eq!(request["poor"], 5);
        assert_eq!(request["miss"], 5);
        assert_eq!(request["play_duration"], 120.0);
    }

    #[test]
    fn all_supported_key_modes_have_canonical_rian_names() {
        assert_eq!(play_mode("4K"), "beat-4k");
        assert_eq!(play_mode("5K"), "beat-5k");
        assert_eq!(play_mode("6K"), "beat-6k");
        assert_eq!(play_mode("7K"), "beat-7k");
        assert_eq!(play_mode("8K"), "beat-8k");
        assert_eq!(play_mode("9K"), "popn-9k");
        assert_eq!(play_mode("10K"), "beat-10k");
        assert_eq!(play_mode("14K"), "beat-14k");
    }

    #[test]
    fn course_request_uses_body_and_rian_course_hash_v1() {
        let local_course_hash = "ef".repeat(32);
        let payload = json!({
            "course": {
                "course_hash": local_course_hash,
                "title": "段位",
                "charts": ["ab".repeat(32), "cd".repeat(32)],
                "constraints": { "grade": true },
            },
            "rule": {
                "gauge": "Class",
                "ln_policy": "ForceHcn",
                "rule_mode": "Dx",
            },
            "result": {
                "clear": "Normal",
                "ex_score": 4000,
                "max_ex_score": 6000,
                "total_notes": 3000,
                "max_combo": 1200,
                "bp": 10,
                "played_at": 1_700_000_001_i64,
                "judges": {
                    "pgreat": 1900,
                    "great": 200,
                    "good": 3,
                    "bad": 4,
                    "poor": 5,
                    "empty_poor": 6,
                },
            },
            "play_options": {
                "option": "AllScratch",
                "random_seed": "281474976710655",
            },
        });
        let request = course_request(&payload, "player", "token").unwrap();
        assert_eq!(
            request["course_sha256"],
            "c3a672ab2881fdd8efb583ff04e94fa88c9ff730941eb72063dadc59101f6d77"
        );
        assert_eq!(
            request["signature"],
            signature(
                "token",
                &[
                    "player".to_string(),
                    "c3a672ab2881fdd8efb583ff04e94fa88c9ff730941eb72063dadc59101f6d77".to_string(),
                    "4000".to_string(),
                    "1200".to_string(),
                    "1700000001".to_string(),
                ],
            )
            .unwrap()
        );
        assert_eq!(request["body"], "DX MODE");
        assert!(request.get("rule_mode").is_none());
        assert_eq!(request["ln_mode"], 3);
        assert_eq!(request["arrange_1p"], "all-scratch");
        assert_eq!(request["play_seed"], 281_474_976_710_655_i64);
        assert_eq!(request["constraint"], json!(["grade"]));
        assert!(request["tracks"].as_array().unwrap().is_empty());
        assert_eq!(request["miss"], 6);
    }

    #[test]
    fn ranking_conversion_accepts_string_database_values() {
        let resource = RianRankingResource {
            id: "score-1".to_string(),
            attributes: serde_json::from_value(json!({
                "player_name": "login-id",
                "display_name": "Player",
                "clear_type": "6",
                "ex_score": "1999",
                "max_combo": "999",
                "min_bp": "4",
                "pgreat": "900",
                "great": "199",
                "miss": "1",
                "play_date": "2026-07-28 12:00:00",
            }))
            .unwrap(),
        };
        let ranking = convert_score_ranking(&"ab".repeat(32), vec![resource], 20, Some("login-id"));
        let entry = &ranking.ranking.entries[0];
        assert_eq!(entry.rank, 1);
        assert_eq!(entry.player.display_name, "Player");
        assert_eq!(entry.score.clear, "Hard");
        assert_eq!(entry.score.ex_score, 1999);
        assert_eq!(entry.score.judges.unwrap().fast.empty_poor, 1);
        assert_eq!(ranking.ranking.self_summary.as_ref().unwrap().rank, 1);
    }

    #[test]
    fn hmac_matches_rfc_4231_vector() {
        assert_eq!(
            hmac_sha256_hex(&[0x0b; 20], b"Hi There"),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn submission_logs_redact_api_token_and_signature() {
        let request = score_request(&sample_payload(), "player", "secret-token").unwrap();
        let logged = redacted_request_json(&request).unwrap();
        assert!(!logged.contains("secret-token"));
        assert!(logged.contains("[REDACTED]"));
    }

    #[test]
    fn base_url_accepts_origin_and_api_prefix() {
        assert_eq!(
            parse_base_url("https://example.test").unwrap().as_str(),
            "https://example.test/api/"
        );
        assert_eq!(
            parse_base_url("https://example.test/api/").unwrap().as_str(),
            "https://example.test/api/"
        );
    }
}
