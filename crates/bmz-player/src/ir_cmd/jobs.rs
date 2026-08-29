use super::download::now_unix_seconds;
use super::*;

pub(super) async fn sync_cli_jobs(
    network_db: &mut NetworkDatabase,
    score_db_path: &Path,
    profile_root: &Path,
    logs_dir: &Path,
    ir_config: &IrConfig,
    limit: u32,
) -> Result<IrSyncReport> {
    sync_cli_jobs_with_filter(
        network_db,
        score_db_path,
        profile_root,
        logs_dir,
        ir_config,
        limit,
        None,
    )
    .await
}

pub(super) async fn sync_cli_jobs_for_kind(
    network_db: &mut NetworkDatabase,
    score_db_path: &Path,
    profile_root: &Path,
    logs_dir: &Path,
    ir_config: &IrConfig,
    provider_key: &str,
    account_id: &str,
    kind: IrJobKind,
    limit: u32,
) -> Result<IrSyncReport> {
    sync_cli_jobs_with_filter(
        network_db,
        score_db_path,
        profile_root,
        logs_dir,
        ir_config,
        limit,
        Some((provider_key, account_id, kind)),
    )
    .await
}

async fn sync_cli_jobs_with_filter(
    network_db: &mut NetworkDatabase,
    score_db_path: &Path,
    profile_root: &Path,
    logs_dir: &Path,
    ir_config: &IrConfig,
    limit: u32,
    filter: Option<(&str, &str, IrJobKind)>,
) -> Result<IrSyncReport> {
    let estimated_seconds = u64::from(limit.saturating_sub(1))
        .saturating_mul(IR_CLI_SYNC_JOB_SPACING_MS)
        .div_ceil(1_000);
    println!("syncing up to {limit} queued jobs (about {estimated_seconds}s)");

    let mut total = IrSyncReport::default();
    for index in 0..limit {
        let ignore_retry_backoff = total.failed == 0;
        if index > 0 {
            let has_next = if ignore_retry_backoff {
                match filter {
                    Some((provider_key, account_id, kind)) => !network_db
                        .pending_ir_score_jobs_for_kind(
                            provider_key,
                            account_id,
                            kind,
                            now_unix_seconds(),
                            1,
                            true,
                        )?
                        .is_empty(),
                    None => !network_db
                        .pending_ir_score_jobs_ignoring_backoff(now_unix_seconds(), 1)?
                        .is_empty(),
                }
            } else {
                match filter {
                    Some((provider_key, account_id, kind)) => !network_db
                        .pending_ir_score_jobs_for_kind(
                            provider_key,
                            account_id,
                            kind,
                            now_unix_seconds(),
                            1,
                            false,
                        )?
                        .is_empty(),
                    None => !network_db.pending_ir_score_jobs(now_unix_seconds(), 1)?.is_empty(),
                }
            };
            if !has_next {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(IR_CLI_SYNC_JOB_SPACING_MS)).await;
        }
        print!("[{}/{}] syncing...", index + 1, limit);
        std::io::stdout().flush()?;
        let sync_result = match filter {
            Some((provider_key, account_id, kind)) => {
                sync_pending_ir_jobs_filtered(
                    network_db,
                    score_db_path,
                    profile_root,
                    logs_dir,
                    ir_config,
                    IrSyncJobFilter { provider_key, account_id, kind, local_score_id: None },
                    now_unix_seconds(),
                    1,
                    ignore_retry_backoff,
                    IrSyncThrottle::none(),
                )
                .await
            }
            None => {
                sync_pending_ir_jobs(
                    network_db,
                    score_db_path,
                    profile_root,
                    logs_dir,
                    ir_config,
                    now_unix_seconds(),
                    1,
                    ignore_retry_backoff,
                    IrSyncThrottle::none(),
                )
                .await
            }
        };
        let report = match sync_result {
            Ok(report) => report,
            Err(error) => {
                println!(" error");
                return Err(error);
            }
        };
        let processed = report.submitted.saturating_add(report.failed);
        if processed == 0 {
            println!(" no queued jobs");
            break;
        }
        println!(" submitted={}, failed={}", report.submitted, report.failed);
        total.submitted = total.submitted.saturating_add(report.submitted);
        total.failed = total.failed.saturating_add(report.failed);
        total.messages.extend(report.messages);
        total.included_rankings.extend(report.included_rankings);
    }
    Ok(total)
}
