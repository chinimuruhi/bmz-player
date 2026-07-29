use super::*;

/// リザルト遷移時に呼ぶ。IR 未設定なら `None`。
///
/// 起動するタスク:
/// 1. pending スコアジョブの即時送信 (このリザルト分を含む)
/// 2. prefetch 設定が ON の scope のランキング取得
///
/// prefetch が両方 OFF でも、パネル表示時のタブ選択で遅延取得できる。
pub fn spawn_result_ir_task(
    profile_root: PathBuf,
    score_db_path: PathBuf,
    network_db_path: PathBuf,
    logs_dir: PathBuf,
    ir_config: &IrConfig,
    local_score_id: i64,
    chart_sha256_hex: String,
    ln_policy: LnScorePolicy,
    double_option: DoubleOptionScoreBucket,
    rule_mode: RuleMode,
) -> Option<ResultIrState> {
    spawn_result_ir_task_for_target(
        profile_root,
        score_db_path,
        network_db_path,
        logs_dir,
        ir_config,
        ResultIrTarget::Chart {
            local_score_id,
            chart_sha256_hex,
            ln_policy,
            double_option,
            rule_mode,
        },
    )
}

pub fn spawn_course_result_ir_task(
    profile_root: PathBuf,
    score_db_path: PathBuf,
    network_db_path: PathBuf,
    logs_dir: PathBuf,
    ir_config: &IrConfig,
    local_score_id: i64,
    hashes: ResultIrCourseHashes,
    gauge: String,
    ln_policy: String,
    rule_mode: RuleMode,
) -> Option<ResultIrState> {
    spawn_result_ir_task_for_target(
        profile_root,
        score_db_path,
        network_db_path,
        logs_dir,
        ir_config,
        ResultIrTarget::Course {
            local_score_id,
            course_hash: hashes.local,
            rian_course_hash_v1: hashes.rian_v1,
            gauge,
            ln_policy,
            rule_mode,
        },
    )
}

pub(super) fn spawn_result_ir_task_for_target(
    profile_root: PathBuf,
    score_db_path: PathBuf,
    network_db_path: PathBuf,
    logs_dir: PathBuf,
    ir_config: &IrConfig,
    target: ResultIrTarget,
) -> Option<ResultIrState> {
    let provider = crate::ir::provider_key::primary_provider_config(ir_config)?;
    let provider_key = crate::ir::provider_key::configured_provider_key(provider)?;
    let query = ResultIrTaskQuery {
        profile_root,
        provider: provider_key.to_string(),
        account_id: provider.account_id.clone(),
        base_url: provider.base_url.clone(),
        target,
    };
    let (sender, receiver) = channel();

    let mut state = ResultIrState {
        submit: IrSubmitState::Sending,
        global: RankingLoadState::NotRequested,
        self_and_rivals: RankingLoadState::NotRequested,
        active_tab: ResultRankingTab::Global,
        ir_connect_begin_at: Some(Instant::now()),
        ir_connect_success_at: None,
        ir_connect_fail_at: None,
        provider_name: bmz_render::scene::ResultIrRankingName::from_display_name(
            crate::ir::provider_key::configured_provider_display_name(provider)?,
        ),
        user_name: bmz_render::scene::ResultIrRankingName::from_display_name(
            &provider.account_display_name,
        ),
        global_skin_scroll_offset: 0,
        self_and_rivals_skin_scroll_offset: 0,
        query: query.clone(),
        sender: sender.clone(),
        receiver,
    };

    let submit_sender = sender.clone();
    let ir_config = ir_config.clone();
    let submit_query = query.clone();
    // global は Result スキンの NUMBER_IR_RANK / OPTION_IR_* 表示にも使うため、
    // prefetch 設定に関わらず常に取得する。rivals scope のみ設定に従う。
    let prefetch_global = true;
    let prefetch_rivals =
        query.supports_scope(IrRankingScope::SelfAndRivals) && state_prefetch_rivals(&ir_config);
    tokio::spawn(async move {
        let now = now_unix_seconds();
        let outcome = async {
            crate::storage::migration::migrate_network_db(&network_db_path)?;
            let mut network_db =
                crate::storage::network_db::NetworkDatabase::open(&network_db_path)?;
            sync_pending_ir_jobs(
                &mut network_db,
                &score_db_path,
                &submit_query.profile_root,
                &logs_dir,
                &ir_config,
                now,
                IR_SYNC_BATCH_LIMIT,
                false,
                IrSyncThrottle::rate_limited(),
            )
            .await
        }
        .await;
        let mut included_global_ranking = None;
        match outcome {
            Ok(report) => {
                included_global_ranking = included_global_ranking_for_query(&submit_query, &report);
                // 別の同期 task がこの job を先に claim していても、送信完了まで
                // 待ってから ranking を取得する。これで古いサーバ側 ranking を
                // Result に固定しない。
                let event = watch_result_submission(&network_db_path, &submit_query.target).await;
                let _ = submit_sender.send(event);
            }
            Err(error) => {
                let _ = submit_sender.send(ResultIrEvent::Submit {
                    submitted: 0,
                    failed: 0,
                    message: Some(format!("{error:#}")),
                });
            }
        }
        let included_global_loaded = included_global_ranking.is_some();
        if let Some(ranking) = included_global_ranking {
            let _ = submit_sender.send(ResultIrEvent::Ranking {
                scope: IrRankingScope::Global,
                result: Ok(ranking),
            });
        }
        // 送信完了後に prefetch する。best 更新前のランキングを返さないため。
        if prefetch_global && !included_global_loaded {
            fetch_ranking_and_send(&submit_query, IrRankingScope::Global, &submit_sender).await;
        }
        if prefetch_rivals {
            fetch_ranking_and_send(&submit_query, IrRankingScope::SelfAndRivals, &submit_sender)
                .await;
        }
    });

    if prefetch_global {
        state.global = RankingLoadState::Loading;
    }
    if prefetch_rivals {
        state.self_and_rivals = RankingLoadState::Loading;
    }
    Some(state)
}

/// 常駐同期との claim race があっても、今回の attempt の終端状態を待つ。
pub(super) async fn watch_result_submission(
    network_db_path: &std::path::Path,
    target: &ResultIrTarget,
) -> ResultIrEvent {
    const POLL_INTERVAL: Duration = Duration::from_millis(250);
    const MAX_POLLS: usize = 120;
    let (kind, local_score_id) = target.submission_job();

    for _ in 0..MAX_POLLS {
        match crate::storage::network_db::NetworkDatabase::open(network_db_path)
            .and_then(|db| db.ir_score_jobs_for_local_score(kind, local_score_id))
        {
            Ok(jobs) => {
                if let Some((submitted, failed, message)) = submission_result_from_jobs(&jobs) {
                    return ResultIrEvent::Submit { submitted, failed, message };
                }
            }
            Err(error) => {
                return ResultIrEvent::Submit {
                    submitted: 0,
                    failed: 0,
                    message: Some(format!("failed to read IR submission status: {error:#}")),
                };
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }

    ResultIrEvent::Submit {
        submitted: 0,
        failed: 0,
        message: Some("timed out waiting for IR submission".to_string()),
    }
}

pub(super) fn submission_result_from_jobs(
    jobs: &[IrScoreJobRecord],
) -> Option<(u32, u32, Option<String>)> {
    if jobs.is_empty() {
        return Some((0, 0, None));
    }
    let failed: Vec<_> =
        jobs.iter().filter(|job| job.status == IrScoreJobStatus::Failed.as_str()).collect();
    if !failed.is_empty() {
        return Some((
            0,
            failed.len() as u32,
            failed
                .iter()
                .find_map(|job| (!job.last_error.is_empty()).then(|| job.last_error.clone())),
        ));
    }
    if jobs.iter().all(|job| job.status == IrScoreJobStatus::Succeeded.as_str()) {
        return Some((jobs.len() as u32, 0, None));
    }
    None
}

pub(super) fn elapsed_since_ms(started_at: Instant) -> i32 {
    started_at.elapsed().as_millis().min(i32::MAX as u128) as i32
}

pub(super) fn state_prefetch_rivals(ir_config: &IrConfig) -> bool {
    ir_config.prefetch_rival_ranking_on_score_submit
}

pub(super) fn included_global_ranking_for_query(
    query: &ResultIrTaskQuery,
    report: &IrSyncReport,
) -> Option<ResultIrRanking> {
    let ResultIrTarget::Chart { local_score_id, chart_sha256_hex, .. } = &query.target else {
        return None;
    };
    report
        .included_rankings
        .iter()
        .find(|ranking| {
            ranking.provider == query.provider
                && ranking.account_id == query.account_id
                && ranking.kind == IrJobKind::Score
                && ranking.local_score_id == *local_score_id
                && ranking.ranking.chart.sha256 == *chart_sha256_hex
                && ranking.ranking.ranking.scope == IrRankingScope::Global
        })
        .map(|ranking| chart_ranking_to_result_ir_ranking(&ranking.ranking))
}

pub(super) fn spawn_ranking_fetch(
    query: ResultIrTaskQuery,
    scope: IrRankingScope,
    sender: Sender<ResultIrEvent>,
) {
    tokio::spawn(async move {
        fetch_ranking_and_send(&query, scope, &sender).await;
    });
}

pub(super) async fn fetch_ranking_and_send(
    query: &ResultIrTaskQuery,
    scope: IrRankingScope,
    sender: &Sender<ResultIrEvent>,
) {
    let result = fetch_result_ranking(query, scope).await.map_err(|error| format!("{error:#}"));
    let _ = sender.send(ResultIrEvent::Ranking { scope, result });
}

pub(super) async fn fetch_result_ranking(
    query: &ResultIrTaskQuery,
    scope: IrRankingScope,
) -> anyhow::Result<ResultIrRanking> {
    match &query.target {
        ResultIrTarget::Chart { chart_sha256_hex, ln_policy, double_option, rule_mode, .. } => {
            let ranking = fetch_ranking(
                &ResultIrQuery {
                    profile_root: query.profile_root.clone(),
                    provider: query.provider.clone(),
                    base_url: query.base_url.clone(),
                    chart_sha256_hex: chart_sha256_hex.clone(),
                    ln_policy: *ln_policy,
                    double_option: *double_option,
                    rule_mode: *rule_mode,
                },
                scope,
            )
            .await?;
            Ok(chart_ranking_to_result_ir_ranking(&ranking))
        }
        ResultIrTarget::Course {
            course_hash,
            rian_course_hash_v1,
            gauge,
            ln_policy,
            rule_mode,
            ..
        } => {
            if scope != IrRankingScope::Global {
                anyhow::bail!("course IR ranking supports global scope only");
            }
            if crate::ir::rian_ir::is_rian_ir_provider(&query.provider) {
                return crate::ir::rian_ir::RianIrClient::new(&query.base_url)?
                    .fetch_course_ranking(
                        rian_course_hash_v1,
                        crate::ir::rian_ir::body_for_rule_mode(*rule_mode),
                        20,
                    )
                    .await
                    .map(|ranking| course_ranking_to_result_ir_ranking(&ranking));
            }
            let client = BmzOfficialIrClient::anonymous(&query.base_url)?;
            let ranking = client
                .fetch_course_ranking(
                    course_hash,
                    &IrCourseRankingRequest {
                        gauge: gauge.clone(),
                        ln_policy: ln_policy.clone(),
                        limit: 20,
                    },
                )
                .await?;
            Ok(course_ranking_to_result_ir_ranking(&ranking))
        }
    }
}

pub(crate) async fn fetch_ranking(
    query: &ResultIrQuery,
    scope: IrRankingScope,
) -> anyhow::Result<IrRankingResult> {
    let now = now_unix_seconds();
    if crate::ir::rian_ir::is_rian_ir_provider(&query.provider) {
        let credentials =
            ensure_fresh_credentials(&query.profile_root, &query.provider, &query.base_url, now)
                .await
                .ok();
        return crate::ir::rian_ir::RianIrClient::new(&query.base_url)?
            .fetch_ranking(
                &query.chart_sha256_hex,
                crate::ir::rian_ir::body_for_rule_mode(query.rule_mode),
                scope,
                20,
                credentials.as_ref().map(|credentials| credentials.account_id.as_str()),
            )
            .await;
    }
    let mut client = BmzOfficialIrClient::anonymous(&query.base_url)?;
    // self / rivals scope は認証必須。global は匿名でも可。
    match ensure_fresh_credentials(&query.profile_root, &query.provider, &query.base_url, now).await
    {
        Ok(credentials) => client.set_access_token(credentials.access_token),
        Err(error) if scope != IrRankingScope::Global => return Err(error),
        Err(_) => {}
    }
    client
        .fetch_ranking(
            &query.chart_sha256_hex,
            &IrRankingRequest {
                scope,
                ln_policy: query.ln_policy.as_str().to_string(),
                double_option: query.double_option,
                rule_mode: query.rule_mode,
                limit: 20,
                offset: 0,
            },
        )
        .await
}

pub(super) fn now_unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
