use super::*;

pub(super) fn enabled_provider(ir_config: &IrConfig) -> Option<(String, String)> {
    let provider = crate::ir::provider_key::primary_provider_config(ir_config)?;
    Some((
        crate::ir::provider_key::configured_provider_key(provider)?.to_string(),
        provider.base_url.clone(),
    ))
}

pub(super) fn spawn_fetch(
    query: ResultIrQuery,
    context: String,
    sha256: [u8; 32],
    requested_at: Instant,
    sender: Sender<FetchResult>,
) {
    tracing::debug!(chart = %query.chart_sha256_hex, "fetching select IR ranking");
    tokio::spawn(async move {
        let result = async {
            let global =
                crate::screens::result_ir::fetch_ranking(&query, IrRankingScope::Global).await?;
            // Self-and-Rivals / Rivals scope は要認証。未ログイン等で失敗しても
            // グローバルランキング表示は維持する。
            let (self_and_rivals, rivals) =
                if crate::ir::rian_ir::is_rian_ir_provider(&query.provider) {
                    (None, None)
                } else {
                    let self_and_rivals = crate::screens::result_ir::fetch_ranking(
                        &query,
                        IrRankingScope::SelfAndRivals,
                    )
                    .await
                    .ok();
                    let rivals =
                        crate::screens::result_ir::fetch_ranking(&query, IrRankingScope::Rivals)
                            .await
                            .ok();
                    (self_and_rivals, rivals)
                };
            anyhow::Ok((global, self_and_rivals, rivals))
        }
        .await
        .map_err(|error| format!("{error:#}"));
        let _ = sender.send((context, sha256, requested_at, result));
    });
}

pub(super) fn spawn_course_fetch(
    provider: String,
    base_url: String,
    context: String,
    target: SelectCourseIrTarget,
    requested_at: Instant,
    sender: Sender<CourseFetchResult>,
) {
    tokio::spawn(async move {
        let result = async {
            if crate::ir::rian_ir::is_rian_ir_provider(&provider) {
                return crate::ir::rian_ir::RianIrClient::new(&base_url)?
                    .fetch_course_ranking(
                        &target.rian_course_hash_v1,
                        crate::ir::rian_ir::body_for_rule_mode(target.rule_mode),
                        20,
                    )
                    .await;
            }
            let client = BmzOfficialIrClient::anonymous(&base_url)?;
            client
                .fetch_course_ranking(
                    &target.course_hash,
                    &IrCourseRankingRequest {
                        gauge: target.gauge.clone(),
                        ln_policy: target.ln_policy.clone(),
                        limit: 20,
                    },
                )
                .await
        }
        .await
        .map_err(|error| format!("{error:#}"));
        let _ = sender.send((context, target, requested_at, result));
    });
}

pub(super) fn spawn_rival_fetch(
    target: SelectRivalFetchTarget,
    profile_root: &Path,
    requested_at: Instant,
    sender: Sender<RivalFetchResult>,
) {
    let network_db_path = profile_root.join("network.db");
    tokio::spawn(async move {
        let result = async {
            crate::storage::migration::migrate_network_db(&network_db_path)?;
            let mut database = crate::storage::network_db::NetworkDatabase::open(&network_db_path)?;
            let cache = database.rival_score_cache_state(
                &target.provider,
                &target.rival_id,
                &target.body,
            )?;
            let response = crate::ir::rian_ir::RianIrClient::new(&target.base_url)?
                .fetch_rival_scores(
                    &target.rival_id,
                    &target.body,
                    (!cache.etag.is_empty()).then_some(cache.etag.as_str()),
                )
                .await?;
            if response.not_modified {
                return database.rival_scores(
                    &target.provider,
                    &target.rival_id,
                    &target.body,
                );
            }
            let scores = response
                .scores
                .into_iter()
                .filter_map(|score| {
                    let chart_sha256 = match hex_to_hash::<32>(&score.sha256) {
                        Ok(hash) => hash,
                        Err(error) => {
                            tracing::warn!(hash = %score.sha256, %error, "discarding invalid rival score hash");
                            return None;
                        }
                    };
                    Some(IrRivalScoreRecord {
                        chart_sha256,
                        ln_mode: score.ln_mode,
                        ex_score: score.ex_score,
                        clear_type: score.clear_type,
                        max_combo: score.max_combo,
                        min_bp: score.min_bp,
                        play_option: score.play_option,
                        arrange_1p: score.arrange_1p,
                        arrange_2p: score.arrange_2p,
                        double_option: score.double_option,
                        play_seed: score.play_seed,
                    })
                })
                .collect::<Vec<_>>();
            let fetched_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
                .unwrap_or(0);
            database.replace_rival_scores(
                &target.provider,
                &target.rival_id,
                &target.body,
                &scores,
                &response.etag,
                fetched_at,
            )?;
            anyhow::Ok(scores)
        }
        .await
        .map_err(|error| format!("{error:#}"));
        let _ = sender.send((target, requested_at, result));
    });
}

/// rivals scope ランキングの先頭 (ライバル中ベスト) をスキン用に変換する。
pub(super) fn top_rival_snapshot(rivals: &IrRankingResult) -> Option<SelectRivalSnapshot> {
    let entry = rivals.ranking.entries.first()?;
    Some(SelectRivalSnapshot {
        display_name: entry.player.display_name.clone(),
        ex_score: entry.score.ex_score,
        max_combo: entry.score.max_combo,
        bp: entry.score.min_bp,
        judge_counts: entry.score.judges.map(|judges| SelectRivalJudgeCounts {
            pgreat: judges.fast.pgreat.saturating_add(judges.slow.pgreat),
            great: judges.fast.great.saturating_add(judges.slow.great),
            good: judges.fast.good.saturating_add(judges.slow.good),
            bad: judges.fast.bad.saturating_add(judges.slow.bad),
            poor: judges.fast.poor.saturating_add(judges.slow.poor),
        }),
    })
}

pub(super) fn ranking_ex_scores(ranking: &IrRankingResult) -> Vec<u32> {
    ranking.ranking.entries.iter().map(|entry| entry.score.ex_score).collect()
}

pub(super) fn result_ranking_ex_scores(ranking: &ResultIrRanking) -> Vec<u32> {
    ranking.entries.iter().map(|entry| entry.ex_score).collect()
}

pub(super) fn elapsed_since_ms(started_at: Instant) -> i32 {
    started_at.elapsed().as_millis().min(i32::MAX as u128) as i32
}

pub(super) fn next_ex_score_above(scores_desc: &[u32], current_ex_score: u32) -> Option<u32> {
    if scores_desc.is_empty() {
        return None;
    }
    for (index, &score) in scores_desc.iter().enumerate() {
        if score <= current_ex_score {
            return Some(scores_desc[index.saturating_sub(1)]);
        }
    }
    scores_desc.first().copied()
}
