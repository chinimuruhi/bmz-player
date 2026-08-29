use super::download::now_unix_seconds;
use super::jobs::sync_cli_jobs;
use super::*;

pub(super) async fn upload_local(
    app_paths: &AppPaths,
    profile_paths: &ProfilePaths,
    profile: &ProfileConfig,
    options: IrLocalUploadOptions,
    sync_after_enqueue: bool,
    all: bool,
) -> Result<()> {
    if all {
        return upload_all_local(app_paths, profile_paths, profile, options).await;
    }

    crate::storage::migration::migrate_library_db(&app_paths.library_db)?;
    crate::storage::migration::migrate_score_db(&profile_paths.score_db)?;
    crate::storage::migration::migrate_network_db(&profile_paths.network_db)?;

    let mut network_db = NetworkDatabase::open(&profile_paths.network_db)?;
    if sync_after_enqueue && !options.dry_run {
        let (provider_key, account_id) =
            resolve_local_upload_target(&profile.ir, options.provider.as_deref())?;
        let queued = network_db.unfinished_ir_score_job_count_for_kind(
            &provider_key,
            &account_id,
            IrJobKind::Score,
        )?;
        if queued > 0 {
            println!("provider: {provider_key}");
            println!("account: {account_id}");
            println!("existing queued score jobs: {queued}; syncing before enqueueing more");
            let sync_report = sync_cli_jobs(
                &mut network_db,
                &profile_paths.score_db,
                profile_paths.root_dir.as_path(),
                app_paths.logs_dir.as_path(),
                &profile.ir,
                IR_CLI_SYNC_BATCH_LIMIT,
            )
            .await?;
            println!("submitted: {}, failed: {}", sync_report.submitted, sync_report.failed);
            let remaining = network_db.unfinished_ir_score_job_count_for_kind(
                &provider_key,
                &account_id,
                IrJobKind::Score,
            )?;
            println!("remaining queued score jobs for {provider_key}/{account_id}: {remaining}");
            for message in &sync_report.messages {
                println!("  {message}");
            }
            return Ok(());
        }
    }

    let library_db = LibraryDatabase::open(&app_paths.library_db)?;
    let score_db = ScoreDatabase::open(&profile_paths.score_db)?;
    let report = enqueue_local_score_jobs(
        profile_paths.root_dir.as_path(),
        &profile.ir,
        &score_db,
        &library_db,
        &mut network_db,
        &options,
        now_unix_seconds(),
    )?;

    println!("provider: {}", report.provider_key);
    println!("account: {}", report.account_id);
    println!("scanned: {}", report.scanned);
    println!("candidates: {}", report.candidates);
    if options.dry_run {
        println!("would enqueue: {}", report.candidates.min(options.limit.max(1)));
    } else {
        println!("enqueued: {}", report.enqueued);
    }
    println!(
        "skipped: already_submitted={}, already_queued={}, missing_chart={}, course_stage={}, autoplay={}",
        report.skipped_already_submitted,
        report.skipped_already_queued,
        report.skipped_missing_chart,
        report.skipped_course_stage,
        report.skipped_autoplay,
    );
    if report.missing_replays > 0 {
        println!(
            "replay files missing: {} (scores were queued without replay)",
            report.missing_replays
        );
    }
    if report.limit_reached {
        println!("limit reached; rerun `bmz ir upload-local` to enqueue the next batch");
    }

    if options.dry_run || !sync_after_enqueue {
        return Ok(());
    }

    let sync_report = sync_cli_jobs(
        &mut network_db,
        &profile_paths.score_db,
        profile_paths.root_dir.as_path(),
        app_paths.logs_dir.as_path(),
        &profile.ir,
        IR_CLI_SYNC_BATCH_LIMIT,
    )
    .await?;
    println!("submitted: {}, failed: {}", sync_report.submitted, sync_report.failed);
    let remaining = network_db.unfinished_ir_score_job_count_for_kind(
        &report.provider_key,
        &report.account_id,
        IrJobKind::Score,
    )?;
    println!(
        "remaining queued score jobs for {}/{}: {remaining}",
        report.provider_key, report.account_id
    );
    for message in &sync_report.messages {
        println!("  {message}");
    }
    Ok(())
}

async fn upload_all_local(
    app_paths: &AppPaths,
    profile_paths: &ProfilePaths,
    profile: &ProfileConfig,
    options: IrLocalUploadOptions,
) -> Result<()> {
    if options.dry_run {
        bail!("ir upload-local --all cannot be combined with --dry-run");
    }

    crate::storage::migration::migrate_library_db(&app_paths.library_db)?;
    crate::storage::migration::migrate_score_db(&profile_paths.score_db)?;
    crate::storage::migration::migrate_network_db(&profile_paths.network_db)?;

    let (provider_key, account_id) =
        resolve_local_upload_target(&profile.ir, options.provider.as_deref())?;
    let library_db = LibraryDatabase::open(&app_paths.library_db)?;
    let score_db = ScoreDatabase::open(&profile_paths.score_db)?;
    let mut network_db = NetworkDatabase::open(&profile_paths.network_db)?;
    let mut batch = 1_u32;
    let mut total_enqueued = 0_u32;
    let mut total_submitted = 0_u32;

    loop {
        println!("full upload batch: {batch}");
        let queued = network_db.unfinished_ir_score_job_count_for_kind(
            &provider_key,
            &account_id,
            IrJobKind::Score,
        )?;
        if queued > 0 {
            println!("provider: {provider_key}");
            println!("account: {account_id}");
            println!("existing queued score jobs: {queued}; syncing before enqueueing more");
            let sync_report = sync_cli_jobs(
                &mut network_db,
                &profile_paths.score_db,
                profile_paths.root_dir.as_path(),
                app_paths.logs_dir.as_path(),
                &profile.ir,
                IR_CLI_SYNC_BATCH_LIMIT,
            )
            .await?;
            let remaining = network_db.unfinished_ir_score_job_count_for_kind(
                &provider_key,
                &account_id,
                IrJobKind::Score,
            )?;
            print_local_upload_sync_report(&sync_report, &provider_key, &account_id, remaining);
            total_submitted = total_submitted.saturating_add(sync_report.submitted);
            ensure_full_upload_progress(&sync_report, total_submitted)?;
            batch = batch.saturating_add(1);
            continue;
        }

        let report = enqueue_local_score_jobs(
            profile_paths.root_dir.as_path(),
            &profile.ir,
            &score_db,
            &library_db,
            &mut network_db,
            &options,
            now_unix_seconds(),
        )?;
        println!("provider: {}", report.provider_key);
        println!("account: {}", report.account_id);
        println!("scanned: {}", report.scanned);
        println!("candidates: {}", report.candidates);
        println!("enqueued: {}", report.enqueued);
        println!(
            "skipped: already_submitted={}, already_queued={}, missing_chart={}, course_stage={}, autoplay={}",
            report.skipped_already_submitted,
            report.skipped_already_queued,
            report.skipped_missing_chart,
            report.skipped_course_stage,
            report.skipped_autoplay,
        );
        if report.missing_replays > 0 {
            println!(
                "replay files missing: {} (scores were queued without replay)",
                report.missing_replays
            );
        }
        total_enqueued = total_enqueued.saturating_add(report.enqueued);
        if report.enqueued == 0 {
            println!(
                "full local upload complete: enqueued={total_enqueued}, submitted={total_submitted}"
            );
            return Ok(());
        }
        if report.limit_reached {
            println!("limit reached; continuing because --all is enabled");
        }

        let sync_report = sync_cli_jobs(
            &mut network_db,
            &profile_paths.score_db,
            profile_paths.root_dir.as_path(),
            app_paths.logs_dir.as_path(),
            &profile.ir,
            IR_CLI_SYNC_BATCH_LIMIT,
        )
        .await?;
        let remaining = network_db.unfinished_ir_score_job_count_for_kind(
            &provider_key,
            &account_id,
            IrJobKind::Score,
        )?;
        print_local_upload_sync_report(&sync_report, &provider_key, &account_id, remaining);
        total_submitted = total_submitted.saturating_add(sync_report.submitted);
        ensure_full_upload_progress(&sync_report, total_submitted)?;

        if remaining == 0 && !report.limit_reached {
            println!(
                "full local upload complete: enqueued={total_enqueued}, submitted={total_submitted}"
            );
            return Ok(());
        }
        batch = batch.saturating_add(1);
    }
}

fn print_local_upload_sync_report(
    sync_report: &IrSyncReport,
    provider_key: &str,
    account_id: &str,
    remaining: u32,
) {
    println!("submitted: {}, failed: {}", sync_report.submitted, sync_report.failed);
    println!("remaining queued score jobs for {provider_key}/{account_id}: {remaining}");
    for message in &sync_report.messages {
        println!("  {message}");
    }
}

pub(super) fn ensure_full_upload_progress(
    sync_report: &IrSyncReport,
    total_submitted: u32,
) -> Result<()> {
    if sync_report.failed > 0 {
        bail!(
            "full local upload stopped after {total_submitted} submissions because {} jobs failed",
            sync_report.failed
        );
    }
    if sync_report.submitted == 0 {
        bail!(
            "full local upload is waiting for an existing sending job; rerun after its 5-minute lease expires"
        );
    }
    Ok(())
}
