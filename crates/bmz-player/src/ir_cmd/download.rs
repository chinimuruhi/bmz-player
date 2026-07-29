use super::*;

pub(super) async fn download_scores(
    profile_paths: &ProfilePaths,
    profile: &ProfileConfig,
    options: IrScoreDownloadOptions,
) -> Result<()> {
    crate::storage::migration::migrate_score_db(&profile_paths.score_db)?;
    crate::storage::migration::migrate_network_db(&profile_paths.network_db)?;

    let provider = selected_provider(profile, options.provider.as_deref())?.clone();
    let provider_key = crate::ir::provider_key::configured_provider_key(&provider)
        .context("IR provider key is not set; log in again")?
        .to_string();
    let credentials = ensure_fresh_credentials(
        profile_paths.root_dir.as_path(),
        &provider_key,
        &provider.base_url,
        now_unix_seconds(),
    )
    .await?;
    let client = BmzOfficialIrClient::new(&provider.base_url, credentials.access_token)?;
    let mut score_db = ScoreDatabase::open(&profile_paths.score_db)?;
    let network_db = NetworkDatabase::open(&profile_paths.network_db)?;
    let report = download_ir_scores(
        &client,
        &provider_key,
        &credentials.account_id,
        &mut score_db,
        &network_db,
        &options,
        now_unix_seconds(),
    )
    .await?;

    println!("provider: {}", report.provider_key);
    println!("account: {}", report.account_id);
    println!("pages: {}", report.pages);
    println!("scanned: {}", report.scanned);
    if options.dry_run {
        println!("would import: {}", report.candidates);
        println!("would link existing uploads: {}", report.linked_existing);
    } else {
        println!("imported: {}", report.imported);
        println!("linked existing uploads: {}", report.linked_existing);
    }
    println!("skipped: existing={}, failed={}", report.skipped_existing, report.failed);
    if report.limit_reached {
        println!("limit reached; rerun `bmz ir download-scores` to continue");
    }
    for message in &report.messages {
        println!("  {message}");
    }
    Ok(())
}

pub(super) fn primary_provider(profile: &ProfileConfig) -> Result<&IrProviderConfig> {
    selected_provider(profile, None)
}

pub(super) fn selected_provider<'a>(
    profile: &'a ProfileConfig,
    requested: Option<&str>,
) -> Result<&'a IrProviderConfig> {
    if let Some(requested) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        let provider = profile
            .ir
            .providers
            .iter()
            .find(|entry| {
                entry.provider == requested
                    || crate::ir::provider_key::configured_provider_key(entry)
                        .is_some_and(|provider_key| provider_key == requested)
            })
            .with_context(|| format!("IR provider is not configured: {requested}"))?;
        if !provider.enabled {
            bail!("IR provider '{}' is disabled", provider.provider);
        }
        if provider.base_url.trim().is_empty() {
            bail!("IR provider '{}' has no base_url; run `bmz ir login` first", provider.provider);
        }
        if crate::ir::provider_key::configured_provider_key(provider).is_none() {
            bail!(
                "IR provider '{}' has no provider key; run `bmz ir login` first",
                provider.provider
            );
        }
        return Ok(provider);
    }

    let provider_name = if profile.ir.primary_provider.is_empty() {
        profile
            .ir
            .providers
            .iter()
            .find(|entry| {
                entry.enabled && crate::ir::provider_key::configured_provider_key(entry).is_some()
            })
            .and_then(crate::ir::provider_key::configured_provider_key)
            .map(str::to_string)
            .unwrap_or_default()
    } else {
        profile.ir.primary_provider.clone()
    };
    profile
        .ir
        .providers
        .iter()
        .find(|entry| {
            !entry.base_url.is_empty()
                && crate::ir::provider_key::configured_provider_key(entry)
                    .is_some_and(|provider_key| provider_key == provider_name)
        })
        .context("no IR provider configured; run `bmz ir login` first")
}

pub(super) fn parse_scope(value: &str) -> Result<IrRankingScope> {
    Ok(match value {
        "global" => IrRankingScope::Global,
        "self_and_rivals" => IrRankingScope::SelfAndRivals,
        "rivals" => IrRankingScope::Rivals,
        "self" => IrRankingScope::SelfOnly,
        "around_self" => IrRankingScope::AroundSelf,
        other => bail!("unknown ranking scope: {other}"),
    })
}

pub(super) fn prompt_password() -> Result<String> {
    print!("password: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let password = line.trim_end_matches(['\r', '\n']).to_string();
    if password.is_empty() {
        bail!("password must not be empty");
    }
    Ok(password)
}

pub(super) fn now_unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
