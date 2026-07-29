async fn login(
    profile_paths: &ProfilePaths,
    profile: &mut ProfileConfig,
    provider: &str,
    email: &str,
    password: Option<String>,
    base_url: Option<String>,
) -> Result<()> {
    let requested_base_url = base_url.clone();
    let existing_base_url = profile
        .ir
        .providers
        .iter()
        .find(|entry| {
            entry.provider == provider
                && requested_base_url.as_ref().is_none_or(|url| entry.base_url == *url)
        })
        .map(|entry| entry.base_url.clone())
        .filter(|url| !url.is_empty());
    let base_url = base_url.or(existing_base_url).or_else(|| {
        crate::ir::rian_ir::is_rian_ir_provider(provider)
            .then(|| crate::ir::rian_ir::RIAN_IR_DEFAULT_BASE_URL.to_string())
    });
    let Some(base_url) = base_url else {
        bail!("IR base URL is not configured. Pass --base-url <URL> on first login.");
    };

    let password = match password {
        Some(password) => password,
        None => prompt_password()?,
    };

    let tokens = if crate::ir::rian_ir::is_rian_ir_provider(provider) {
        crate::ir::rian_ir::RianIrClient::new(&base_url)?.login(email, &password).await?
    } else {
        BmzOfficialIrClient::anonymous(&base_url)?.login(email, &password).await?
    };
    let provider_key = tokens.provider_key.clone();
    let display_name = tokens.player.display_name.clone().unwrap_or_default();
    let now = now_unix_seconds();

    save_credentials(
        profile_paths.root_dir.as_path(),
        &IrStoredCredentials {
            provider: provider_key.clone(),
            account_id: tokens.player.id.clone(),
            display_name: display_name.clone(),
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_at: tokens.expires_at,
        },
    )?;

    let entry_index = profile
        .ir
        .providers
        .iter()
        .position(|entry| entry.provider == provider && entry.base_url == base_url)
        .or_else(|| {
            profile
                .ir
                .providers
                .iter()
                .position(|entry| entry.provider == provider && entry.base_url.is_empty())
        });
    let entry = match entry_index {
        Some(index) => &mut profile.ir.providers[index],
        None => {
            profile.ir.providers.push(IrProviderConfig {
                provider: provider.to_string(),
                provider_key: String::new(),
                base_url: String::new(),
                enabled: false,
                account_display_name: String::new(),
                account_id: String::new(),
                send_policy: IrSendPolicyConfig::default(),
                role: IrProviderRoleConfig::default(),
                last_login_at: None,
                last_success_at: None,
            });
            profile.ir.providers.last_mut().unwrap()
        }
    };
    entry.base_url = base_url;
    entry.provider_key = provider_key.clone();
    entry.enabled = true;
    entry.account_id = tokens.player.id;
    entry.account_display_name = display_name.clone();
    entry.last_login_at = Some(now);
    if profile.ir.primary_provider.is_empty() {
        profile.ir.primary_provider = provider_key;
        entry.role = IrProviderRoleConfig::Primary;
    }
    profile.updated_at = now;
    save_profile_config(&profile_paths.profile_toml, profile)?;

    println!(
        "Signed in to {provider} as {}",
        if display_name.is_empty() { email } else { &display_name }
    );
    Ok(())
}

async fn logout(
    profile_paths: &ProfilePaths,
    profile: &mut ProfileConfig,
    provider: &str,
) -> Result<()> {
    let entry_index = profile.ir.providers.iter().position(|entry| {
        crate::ir::provider_key::configured_provider_key(entry) == Some(provider)
            || entry.provider == provider
    });
    let entry = entry_index.and_then(|index| profile.ir.providers.get(index));
    let credentials = match entry {
        Some(entry) => crate::ir::provider_key::configured_provider_key(entry)
            .map(|provider_key| load_credentials(profile_paths.root_dir.as_path(), provider_key))
            .transpose()?
            .flatten(),
        None => None,
    };
    if let Some(credentials) = &credentials
        && let Some(entry) = entry
        && !crate::ir::rian_ir::is_rian_ir_config(entry)
    {
        let client = BmzOfficialIrClient::new(&entry.base_url, credentials.access_token.clone())?;
        if let Err(error) = client.logout(&credentials.refresh_token).await {
            eprintln!("warning: failed to revoke remote IR session for {provider}: {error:#}");
        }
    }

    let removed = match entry {
        Some(entry) => crate::ir::provider_key::configured_provider_key(entry)
            .map(|provider_key| delete_credentials(profile_paths.root_dir.as_path(), provider_key))
            .transpose()?
            .unwrap_or(false),
        None => false,
    };
    if let Some(index) = entry_index
        && let Some(entry) = profile.ir.providers.get_mut(index)
    {
        entry.enabled = false;
        profile.updated_at = now_unix_seconds();
        save_profile_config(&profile_paths.profile_toml, profile)?;
    }
    if removed {
        println!("Signed out from {provider}.");
    } else {
        println!("No stored credentials for {provider}.");
    }
    Ok(())
}

async fn status(profile_paths: &ProfilePaths, profile: &ProfileConfig) -> Result<()> {
    if profile.ir.providers.is_empty() {
        println!(
            "No IR providers configured. Run `bmz ir login --email <EMAIL> --base-url <URL>`."
        );
        return Ok(());
    }
    println!("primary provider: {}", profile.ir.primary_provider);
    for entry in &profile.ir.providers {
        let provider_key = crate::ir::provider_key::configured_provider_key(entry);
        println!(
            "- {} (key: {}, enabled: {}, base_url: {})",
            entry.provider,
            provider_key.unwrap_or("(not signed in)"),
            entry.enabled,
            entry.base_url
        );
        match provider_key
            .map(|provider_key| load_credentials(profile_paths.root_dir.as_path(), provider_key))
            .transpose()?
            .flatten()
        {
            Some(credentials) => {
                println!("  account: {} ({})", credentials.display_name, credentials.account_id);
                if entry.enabled && !entry.base_url.is_empty() {
                    let now = now_unix_seconds();
                    match ensure_fresh_credentials(
                        profile_paths.root_dir.as_path(),
                        provider_key.unwrap_or(""),
                        &entry.base_url,
                        now,
                    )
                    .await
                    {
                        Ok(fresh) => {
                            if crate::ir::rian_ir::is_rian_ir_config(entry) {
                                println!(
                                    "  connection: credentials stored ({})",
                                    fresh.display_name
                                );
                            } else {
                                let client =
                                    BmzOfficialIrClient::new(&entry.base_url, fresh.access_token)?;
                                match client.me().await {
                                    Ok(me) => println!(
                                        "  connection: OK ({})",
                                        me.player.display_name.unwrap_or(me.player.id)
                                    ),
                                    Err(error) => println!("  connection: NG ({error:#})"),
                                }
                            }
                        }
                        Err(error) => println!("  connection: NG ({error:#})"),
                    }
                }
            }
            None => println!("  account: not signed in"),
        }
    }
    Ok(())
}
