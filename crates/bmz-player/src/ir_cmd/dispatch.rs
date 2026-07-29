pub async fn run_ir_command(cmd: IrCommand) -> Result<()> {
    let (profile_paths, mut profile) = load_active_profile()?;
    match cmd {
        IrCommand::Login { email, password, base_url, provider } => {
            login(&profile_paths, &mut profile, &provider, &email, password, base_url).await
        }
        IrCommand::Logout { provider } => logout(&profile_paths, &mut profile, &provider).await,
        IrCommand::Status => status(&profile_paths, &profile).await,
        IrCommand::Ranking { sha256, ln_policy, scope, limit } => {
            ranking(&profile_paths, &profile, &sha256, &ln_policy, &scope, limit).await
        }
        IrCommand::Sync => sync(&profile_paths, &profile).await,
        IrCommand::UploadLocal {
            provider,
            limit,
            dry_run,
            sync: sync_after_enqueue,
            all,
            resend,
            include_course_stages,
            include_replay,
        } => {
            let options = IrLocalUploadOptions {
                provider,
                limit,
                dry_run,
                resend,
                include_course_stages,
                include_replay,
            };
            upload_local(&profile_paths, &profile, options, sync_after_enqueue, all).await
        }
        IrCommand::DownloadScores { provider, limit, dry_run } => {
            download_scores(
                &profile_paths,
                &profile,
                IrScoreDownloadOptions { provider, limit, dry_run },
            )
            .await
        }
        IrCommand::AttestSubmitted { provider, sync, all } => {
            attest_submitted_scores(&profile_paths, &profile, provider.as_deref(), sync, all).await
        }
        IrCommand::CleanupImported { provider, apply } => {
            cleanup_imported_scores(&profile_paths, &profile, provider.as_deref(), apply).await
        }
        IrCommand::CleanupDuplicate { history_id, provider, apply } => {
            cleanup_duplicate_score_history(
                &profile_paths,
                &profile,
                provider.as_deref(),
                history_id,
                apply,
            )
            .await
        }
        IrCommand::Rivals { action } => rivals(&profile_paths, &mut profile, action).await,
        IrCommand::DeviceKey { rotate } => device_key(&profile_paths, &profile, rotate).await,
        IrCommand::Replay { score_id } => replay(&profile_paths, &profile, &score_id).await,
    }
}
