use super::download::now_unix_seconds;
/// 再import済み Beatoraja 行と照合できる、source_kind 導入前の Local 履歴を整理する。
///
/// IR 本体を先に削除し、成功後にネットワーク台帳と score.db を更新する。中断時は
/// リモート削除 API の missing 応答を成功として扱うため、同じコマンドを再実行できる。
use super::*;

pub(super) async fn cleanup_imported_scores(
    profile_paths: &ProfilePaths,
    profile: &ProfileConfig,
    requested_provider: Option<&str>,
    apply: bool,
) -> Result<()> {
    const REMOTE_DELETE_BATCH_SIZE: usize = 19;
    const REMOTE_DELETE_BATCH_SPACING_MS: u64 = 200;

    crate::storage::migration::migrate_score_db(&profile_paths.score_db)?;
    crate::storage::migration::migrate_network_db(&profile_paths.network_db)?;

    let (provider_key, account_id) = resolve_local_upload_target(&profile.ir, requested_provider)?;
    let mut score_db = ScoreDatabase::open(&profile_paths.score_db)?;
    let plan = score_db.legacy_beatoraja_cleanup_plan()?;
    let mut network_db = NetworkDatabase::open(&profile_paths.network_db)?;
    let submitted_links =
        network_db.successful_ir_score_submissions_for_local_scores(&plan.legacy_history_ids)?;
    let selected_remote_score_ids = submitted_links
        .iter()
        .filter(|link| link.provider == provider_key && link.account_id == account_id)
        .map(|link| link.remote_score_id.clone())
        .collect::<BTreeSet<_>>();
    let unselected_targets = submitted_links
        .iter()
        .filter(|link| link.provider != provider_key || link.account_id != account_id)
        .map(|link| format!("{}/{}", link.provider, link.account_id))
        .collect::<BTreeSet<_>>();

    println!("provider: {provider_key}");
    println!("account: {account_id}");
    println!("old Local candidates: {}", plan.legacy_history_ids.len());
    println!("retained Beatoraja rows: {}", plan.retained_beatoraja_history_ids.len());
    println!(
        "submitted IR scores linked to old Local candidates for selected provider: {}",
        selected_remote_score_ids.len()
    );
    if !unselected_targets.is_empty() {
        println!(
            "submitted scores for other IR accounts: {} ({})",
            unselected_targets.len(),
            unselected_targets.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
        );
    }
    if plan.legacy_history_ids.is_empty() {
        println!("no old Local candidates match a Beatoraja row");
        return Ok(());
    }
    if !apply {
        println!("dry-run only; rerun with `bmz ir cleanup-imported --apply` to delete them");
        return Ok(());
    }
    if !unselected_targets.is_empty() {
        bail!(
            "cleanup stopped: matching Local rows were submitted to other IR accounts; rerun once per account with --provider before deleting local history"
        );
    }

    if !selected_remote_score_ids.is_empty() {
        let provider = crate::ir::provider_key::provider_config_for_key(&profile.ir, &provider_key)
            .context("configured cleanup provider is not available; run `bmz ir login` first")?;
        let credentials = ensure_fresh_credentials(
            profile_paths.root_dir.as_path(),
            &provider_key,
            &provider.base_url,
            now_unix_seconds(),
        )
        .await?;
        let client = BmzOfficialIrClient::new(&provider.base_url, credentials.access_token)?;
        let batches = selected_remote_score_ids.iter().cloned().collect::<Vec<_>>();
        let batch_count = batches.len().div_ceil(REMOTE_DELETE_BATCH_SIZE);
        let mut retained_remote_score_ids = BTreeSet::new();
        for (index, score_ids) in batches.chunks(REMOTE_DELETE_BATCH_SIZE).enumerate() {
            if index > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(
                    REMOTE_DELETE_BATCH_SPACING_MS,
                ))
                .await;
            }
            println!(
                "[{}/{}] deleting {} IR local backfill scores",
                index + 1,
                batch_count,
                score_ids.len()
            );
            let response =
                client.delete_local_backfill_scores(score_ids).await.with_context(|| {
                    format!("failed to delete IR local backfill batch {}", index + 1)
                })?;
            let requested = score_ids.iter().cloned().collect::<BTreeSet<_>>();
            let returned = response
                .deleted_score_ids
                .iter()
                .chain(&response.missing_score_ids)
                .chain(&response.retained_score_ids)
                .cloned()
                .collect::<BTreeSet<_>>();
            if requested != returned {
                bail!(
                    "IR local backfill cleanup batch {} returned an unexpected score set; local history was not changed",
                    index + 1
                );
            }
            retained_remote_score_ids.extend(response.retained_score_ids);
        }
        if !retained_remote_score_ids.is_empty() {
            println!(
                "retained IR scores not marked local_backfill: {}",
                retained_remote_score_ids.len()
            );
        }
    }

    let network_report = network_db.purge_ir_records_for_local_scores(
        &provider_key,
        &account_id,
        &plan.legacy_history_ids,
    )?;
    let score_report = score_db.purge_legacy_beatoraja_imports(&plan)?;
    println!("removed old Local history: {}", score_report.removed_legacy_history);
    println!("retained Beatoraja rows: {}", score_report.retained_beatoraja_history);
    println!(
        "removed local IR records: jobs={}, submissions={}",
        network_report.removed_jobs, network_report.removed_submissions
    );
    println!(
        "run `bmz ir upload-local --all --provider {provider_key}` to submit retained Beatoraja rows with current metadata"
    );
    Ok(())
}

/// 同じ source_kind の完全重複履歴を1件だけ削除する。
///
/// 対応する local_backfill IR score は先に server API で削除し、成功後に
/// network.db と score.db を同じ history id で整理する。
pub(super) async fn cleanup_duplicate_score_history(
    profile_paths: &ProfilePaths,
    profile: &ProfileConfig,
    requested_provider: Option<&str>,
    history_id: i64,
    apply: bool,
) -> Result<()> {
    const REMOTE_DELETE_BATCH_SIZE: usize = 19;
    const REMOTE_DELETE_BATCH_SPACING_MS: u64 = 200;

    if !apply {
        bail!("duplicate cleanup requires --apply");
    }

    crate::storage::migration::migrate_score_db(&profile_paths.score_db)?;
    crate::storage::migration::migrate_network_db(&profile_paths.network_db)?;

    let mut score_db = ScoreDatabase::open(&profile_paths.score_db)?;
    let duplicate_ids = score_db.same_source_duplicate_history_ids(history_id)?;
    if duplicate_ids.len() != 1 {
        bail!(
            "score_history {history_id} must have exactly one same-source duplicate, found {}",
            duplicate_ids.len()
        );
    }
    let retained_history_id = duplicate_ids[0];
    let (provider_key, account_id) = resolve_local_upload_target(&profile.ir, requested_provider)?;
    let mut network_db = NetworkDatabase::open(&profile_paths.network_db)?;
    let submitted_links =
        network_db.successful_ir_score_submissions_for_local_scores(&[history_id])?;
    let selected_remote_score_ids = submitted_links
        .iter()
        .filter(|link| link.provider == provider_key && link.account_id == account_id)
        .map(|link| link.remote_score_id.clone())
        .collect::<BTreeSet<_>>();
    let unselected_targets = submitted_links
        .iter()
        .filter(|link| link.provider != provider_key || link.account_id != account_id)
        .map(|link| format!("{}/{}", link.provider, link.account_id))
        .collect::<BTreeSet<_>>();

    if !unselected_targets.is_empty() {
        bail!(
            "cleanup stopped: score_history {history_id} was submitted to other IR accounts ({})",
            unselected_targets.into_iter().collect::<Vec<_>>().join(", ")
        );
    }

    println!("provider: {provider_key}");
    println!("account: {account_id}");
    println!("removing duplicate score_history: {history_id}");
    println!("retained duplicate score_history: {retained_history_id}");
    println!("submitted IR scores for the removed history: {}", selected_remote_score_ids.len());

    if !selected_remote_score_ids.is_empty() {
        let provider = crate::ir::provider_key::provider_config_for_key(&profile.ir, &provider_key)
            .context("configured cleanup provider is not available; run `bmz ir login` first")?;
        let credentials = ensure_fresh_credentials(
            profile_paths.root_dir.as_path(),
            &provider_key,
            &provider.base_url,
            now_unix_seconds(),
        )
        .await?;
        let client = BmzOfficialIrClient::new(&provider.base_url, credentials.access_token)?;
        let batches = selected_remote_score_ids.iter().cloned().collect::<Vec<_>>();
        let batch_count = batches.len().div_ceil(REMOTE_DELETE_BATCH_SIZE);
        let mut retained_remote_score_ids = BTreeSet::new();
        for (index, score_ids) in batches.chunks(REMOTE_DELETE_BATCH_SIZE).enumerate() {
            if index > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(
                    REMOTE_DELETE_BATCH_SPACING_MS,
                ))
                .await;
            }
            println!(
                "[{}/{}] deleting {} IR local backfill scores",
                index + 1,
                batch_count,
                score_ids.len()
            );
            let response =
                client.delete_local_backfill_scores(score_ids).await.with_context(|| {
                    format!("failed to delete IR local backfill batch {}", index + 1)
                })?;
            let requested = score_ids.iter().cloned().collect::<BTreeSet<_>>();
            let returned = response
                .deleted_score_ids
                .iter()
                .chain(&response.missing_score_ids)
                .chain(&response.retained_score_ids)
                .cloned()
                .collect::<BTreeSet<_>>();
            if requested != returned {
                bail!(
                    "IR local backfill cleanup batch {} returned an unexpected score set; local history was not changed",
                    index + 1
                );
            }
            retained_remote_score_ids.extend(response.retained_score_ids);
        }
        if !retained_remote_score_ids.is_empty() {
            println!(
                "retained IR scores not marked local_backfill: {}",
                retained_remote_score_ids.len()
            );
        }
    }

    let network_report =
        network_db.purge_ir_records_for_local_scores(&provider_key, &account_id, &[history_id])?;
    let removed_history = score_db.purge_score_history_ids_and_rebuild(&[history_id])?;
    if removed_history != 1 {
        bail!(
            "expected to remove score_history {history_id}, removed {removed_history}; network records were not restored"
        );
    }
    println!("removed duplicate score_history: {history_id}");
    println!(
        "removed local IR records: jobs={}, submissions={}",
        network_report.removed_jobs, network_report.removed_submissions
    );
    Ok(())
}
