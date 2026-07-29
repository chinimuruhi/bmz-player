/// プロファイル設定パネルの IR ログインフォーム状態。
///
/// ログインはネットワーク I/O なので tokio タスクで実行し、
/// 結果は channel 経由で次フレーム以降に反映する。
#[derive(Default)]
struct IrLoginUiState {
    email: String,
    password: String,
    busy: bool,
    busy_target: Option<IrProviderUiTarget>,
    message: Option<IrProviderUiMessage>,
    receiver: Option<std::sync::mpsc::Receiver<Result<IrLoginOutcome, String>>>,
}

#[derive(Default)]
struct ProfileManagerUiState {
    create_id: String,
    create_display_name: String,
    create_activate: bool,
    copy_source_id: String,
    copy_target_id: String,
    copy_display_name: String,
    copy_activate: bool,
    message: String,
    error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IrProviderUiTarget {
    provider: String,
    base_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IrProviderPreset {
    BmzIr,
    RianIr,
    Other,
}

impl IrProviderUiTarget {
    fn new(provider: String, base_url: String) -> Self {
        Self { provider, base_url }
    }

    fn matches(&self, provider: &str, base_url: &str) -> bool {
        self.provider == provider && self.base_url == base_url
    }
}

#[derive(Debug, Clone)]
struct IrProviderUiMessage {
    target: IrProviderUiTarget,
    ok: bool,
    text: String,
}

/// ログインタスクから UI スレッドへ返す結果。
struct IrLoginOutcome {
    provider: String,
    provider_key: String,
    base_url: String,
    account_id: String,
    display_name: String,
}

/// プロファイル設定パネルの IR device key 操作状態。
#[derive(Default)]
struct IrDeviceKeyUiState {
    busy_provider: Option<String>,
    busy_target: Option<IrProviderUiTarget>,
    message: Option<IrProviderUiMessage>,
    receiver: Option<std::sync::mpsc::Receiver<Result<IrDeviceKeyOutcome, String>>>,
}

struct IrDeviceKeyOutcome {
    provider: String,
    base_url: String,
    public_key: String,
    key_id: String,
}

impl IrDeviceKeyUiState {
    fn poll(&mut self, text: Localizer) {
        let Some(receiver) = &self.receiver else {
            return;
        };
        let Ok(result) = receiver.try_recv() else {
            return;
        };
        self.receiver = None;
        let target = self.busy_target.take();
        self.busy_provider = None;
        self.message = match result {
            Ok(outcome) => Some(IrProviderUiMessage {
                target: IrProviderUiTarget::new(outcome.provider.clone(), outcome.base_url),
                ok: true,
                text: tr!(
                    text,
                    "profile-ir-device-key-rotated",
                    "provider" => outcome.provider,
                    "public_key" => short_public_key(&outcome.public_key),
                    "key_id" => outcome.key_id,
                ),
            }),
            Err(error) => {
                target.map(|target| IrProviderUiMessage { target, ok: false, text: error })
            }
        };
    }

    fn start_rotate(
        &mut self,
        profile_root: std::path::PathBuf,
        provider: String,
        provider_key: String,
        base_url: String,
    ) {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.receiver = Some(receiver);
        self.busy_provider = Some(provider_key.clone());
        self.busy_target = Some(IrProviderUiTarget::new(provider.clone(), base_url.clone()));
        self.message = None;
        tokio::spawn(async move {
            let outcome = async {
                let credentials = crate::ir::sync::ensure_fresh_credentials(
                    &profile_root,
                    &provider_key,
                    &base_url,
                    now_unix_seconds(),
                )
                .await?;
                let client = crate::ir::bmz_official::BmzOfficialIrClient::new(
                    &base_url,
                    credentials.access_token,
                )?;
                let key = crate::ir::device_key::rotate_registered_device_key(
                    &profile_root,
                    &provider_key,
                    &client,
                )
                .await?;
                anyhow::Ok(IrDeviceKeyOutcome {
                    provider,
                    base_url,
                    public_key: key.public_key,
                    key_id: key.key_id.unwrap_or_default(),
                })
            }
            .await
            .map_err(|error| format!("{error:#}"));
            let _ = sender.send(outcome);
        });
    }
}

impl IrLoginUiState {
    /// ログインタスクの完了を取り込み、成功時は provider 設定を更新する。
    /// profile 設定が更新された (保存が必要な) 場合に true を返す。
    fn poll(&mut self, profile: &mut ProfileConfig, text: Localizer) -> bool {
        let Some(receiver) = &self.receiver else {
            return false;
        };
        let Ok(result) = receiver.try_recv() else {
            return false;
        };
        self.receiver = None;
        self.busy = false;
        let target = self.busy_target.take();
        match result {
            Ok(outcome) => {
                self.password.clear();
                self.message = Some(IrProviderUiMessage {
                    target: IrProviderUiTarget::new(
                        outcome.provider.clone(),
                        outcome.base_url.clone(),
                    ),
                    ok: true,
                    text: tr!(
                        text,
                        "profile-ir-login-success",
                        "display_name" => outcome.display_name.clone(),
                    ),
                });
                if let Some(entry) = profile.ir.providers.iter_mut().find(|entry| {
                    entry.provider == outcome.provider && entry.base_url == outcome.base_url
                }) {
                    entry.enabled = true;
                    entry.provider_key = outcome.provider_key.clone();
                    entry.account_id = outcome.account_id;
                    entry.account_display_name = outcome.display_name;
                    entry.last_login_at = Some(now_unix_seconds());
                    if profile.ir.primary_provider.is_empty() {
                        profile.ir.primary_provider = outcome.provider_key;
                        entry.role = IrProviderRoleConfig::Primary;
                    }
                    sync_ir_provider_roles(&mut profile.ir);
                    return true;
                }
                false
            }
            Err(error) => {
                self.message =
                    target.map(|target| IrProviderUiMessage { target, ok: false, text: error });
                false
            }
        }
    }

    /// ログインタスクを起動する。
    fn start_login(
        &mut self,
        profile_root: std::path::PathBuf,
        provider: String,
        base_url: String,
    ) {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.receiver = Some(receiver);
        self.busy = true;
        self.busy_target = Some(IrProviderUiTarget::new(provider.clone(), base_url.clone()));
        self.message = None;
        let email = self.email.clone();
        let password = self.password.clone();
        tokio::spawn(async move {
            let outcome = async {
                let tokens = if crate::ir::rian_ir::is_rian_ir_provider(&provider) {
                    crate::ir::rian_ir::RianIrClient::new(&base_url)?
                        .login(&email, &password)
                        .await?
                } else {
                    crate::ir::bmz_official::BmzOfficialIrClient::anonymous(&base_url)?
                        .login(&email, &password)
                        .await?
                };
                let provider_key = tokens.provider_key.clone();
                let display_name =
                    tokens.player.display_name.clone().unwrap_or_else(|| email.clone());
                crate::ir::credentials::save_credentials(
                    &profile_root,
                    &crate::ir::credentials::IrStoredCredentials {
                        provider: provider_key.clone(),
                        account_id: tokens.player.id.clone(),
                        display_name: display_name.clone(),
                        access_token: tokens.access_token,
                        refresh_token: tokens.refresh_token,
                        expires_at: tokens.expires_at,
                    },
                )?;
                anyhow::Ok(IrLoginOutcome {
                    provider,
                    provider_key,
                    base_url,
                    account_id: tokens.player.id,
                    display_name,
                })
            }
            .await
            .map_err(|error| format!("{error:#}"));
            let _ = sender.send(outcome);
        });
    }
}

fn now_unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn short_public_key(public_key: &str) -> String {
    if public_key.len() <= 16 {
        return public_key.to_string();
    }
    format!("{}…{}", &public_key[..8], &public_key[public_key.len() - 8..])
}
