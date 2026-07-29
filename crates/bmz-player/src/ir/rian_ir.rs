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

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RianTableResource {
    pub id: String,
    pub attributes: RianTable,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RianTable {
    pub name: String,
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub folders: Vec<RianTableFolder>,
    #[serde(default)]
    pub courses: Vec<RianTableCourse>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RianTableFolder {
    pub name: String,
    #[serde(default)]
    pub charts: Vec<RianTableChart>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RianTableChart {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub subartist: String,
    #[serde(default)]
    pub md5: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub level: serde_json::Value,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RianTableCourse {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub constraint: Vec<String>,
    #[serde(default)]
    pub charts: Vec<RianTableChart>,
}

#[derive(Debug, serde::Deserialize)]
struct RianTablesResponse {
    #[serde(default)]
    data: Vec<RianTableResource>,
}

mod client;
mod ranking;
mod request;

pub use request::{
    body_for_rule_mode, body_for_rule_name, course_submission_supported, is_rian_ir_config,
    is_rian_ir_provider, score_submission_supported,
};

#[cfg(test)]
use client::parse_base_url;
use ranking::*;
use request::*;
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

    #[test]
    fn tables_response_accepts_dynamic_folders_and_courses() {
        let response: RianTablesResponse = serde_json::from_value(json!({
            "data": [{
                "type": "difficulty-tables",
                "id": "0",
                "attributes": {
                    "name": "rianIR POPULAR",
                    "symbol": "POP",
                    "folders": [{
                        "name": "24H POPULAR SONGS",
                        "charts": [{
                            "title": "Song",
                            "artist": "Artist",
                            "md5": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                            "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                            "level": "Top 20"
                        }]
                    }],
                    "courses": [{
                        "name": "Course",
                        "sha256": "course-hash",
                        "constraint": ["grade", "ln"],
                        "charts": [{
                            "title": "Song",
                            "subtitle": "[Another]",
                            "artist": "Artist",
                            "subartist": "",
                            "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                            "level": 12
                        }]
                    }]
                }
            }]
        }))
        .unwrap();

        assert_eq!(response.data[0].attributes.folders[0].charts[0].level, json!("Top 20"));
        assert_eq!(response.data[0].attributes.courses[0].constraint, vec!["grade", "ln"]);
    }
}
