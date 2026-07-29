//! 本体設定 / スキン設定 / デバッグ表示のための egui レイヤ。
//!
//! `egui::Context` と winit 連携状態 (`egui_winit::State`) を所有し、毎フレーム
//! UI を構築して描画プリミティブ (`EguiFrame`) を生成する。bmz-render はその
//! プリミティブをゲーム / スキン描画の上にペイントするだけにする。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use bmz_core::input::InputDeviceKind;
use bmz_gameplay::rule::RuleMode;
use bmz_render::scene::ResultGradeDiffDisplay;
use bmz_render::skin::{SkinDocument, SkinFilepathDef, SkinOffsetDef, SkinPropertyDef};
use bmz_render::skin_offset::SKIN_OFFSET_BAR_LINE;
use bmz_render::ui::EguiFrame;
use egui::{NumExt, ViewportId};
use winit::event::WindowEvent;
use winit::window::Window;

use crate::config::app_config::{
    AppConfig, AudioBackend, AudioBufferSizeMode, AudioOutputMode, AudioSampleRateMode,
    DifficultyTableSource, GamepadBackendKind, InputBackendKind, InternalResolutionModeConfig,
    LogLevel, ObsActionConfig, ObsRecordingMode, PathEntry, RendererBackend, UpdateChannelConfig,
    VsyncModeConfig, WindowMode,
};
use crate::config::play::{TARGET_GREEN_NUMBER_MAX, TARGET_GREEN_NUMBER_MIN};
use crate::config::profile_config::{
    BgaExpandConfig, BgaModeConfig, BottomShiftableGaugeConfig, DoubleOptionConfig,
    FastSlowDisplayScope, GaugeAutoShiftConfig, GaugeTypeConfig, HISPEED_STEP_MAX,
    HISPEED_STEP_MIN, HispeedModeConfig, HsFixConfig, IrConfig, IrCredentialStoreConfig,
    IrProviderConfig, IrProviderRoleConfig, IrSendPolicyConfig, JudgeAlgorithmConfig,
    LaneEffectConfig, ProfileConfig, RELEASE_BOUNCE_MS_MAX, RandomOptionConfig, ReplaySlotRule,
    SkinConfig, SkinHistoryEntryConfig, SkinOffsetConfig, TargetOptionConfig,
    default_hispeed_step_fhs, default_hispeed_step_nhs, normalize_hispeed_step,
};
use crate::i18n::{AppLocale, FluentArgs, Localizer};
use crate::ln_policy::LnPolicySetting;
use crate::logging::{LogBuffer, LogEntry, LogLevel as TracingLogLevel};
use crate::paths::{AppPaths, resolve_app_paths};
use crate::practice_ui::{PracticePanelContext, build_practice_panel};
use crate::profile_cmd;
use crate::random_trainer::RandomTrainerState;
use crate::screens::course_session::CourseResultSummary;
use crate::screens::select_model::SelectCourseRow;
use crate::select_options::SessionMode;
use crate::skin_loader::RANDOM_FILE_SELECTION;
use crate::songs_cmd::add_song_root_entry;
use crate::storage::difficulty_table_db::DifficultyTableRecord;
use crate::storage::score_import::{ScoreImportKind, ScoreImportRequest};
use crate::update::{UpdateAssetKind, UpdateCandidate, current_version};
use crate::window_config::monitor_config_name;

const BUNDLED_THIRD_PARTY_NOTICES: &str = include_str!("../../../THIRD-PARTY-NOTICES.txt");
const THIRD_PARTY_NOTICE_PATH: &str = "licenses/third-party-notices.txt";
const RUST_DEPENDENCY_LICENSE_PATH: &str = "licenses/rust-dependency-licenses.txt";
const LOCAL_RUST_DEPENDENCY_LICENSE_FILE: &str = "rust-dependency-licenses.txt";

macro_rules! tr {
    ($text:expr, $key:literal) => {
        $text.text($key)
    };
    ($text:expr, $key:literal, $($name:literal => $value:expr),+ $(,)?) => {{
        let mut args = FluentArgs::new();
        $(args.set($name, $value);)+
        $text.format($key, &args)
    }};
}

mod auxiliary_panels;
mod profile_panel;
mod settings_panel;
mod skin_panel;

use auxiliary_panels::*;
use profile_panel::*;
use settings_panel::*;
use skin_panel::*;

/// スキンが宣言する設定可能項目の定義 (1 シーン分)。
///
/// renderer が保持する `SkinDocument` から複製して egui パネルへ渡す。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SkinReloadRequest {
    pub select: bool,
    pub decide: bool,
    pub result: bool,
    pub course_result: bool,
    pub play4: bool,
    pub play5: bool,
    pub play6: bool,
    pub play7: bool,
    pub play8: bool,
    pub play9: bool,
    pub play10: bool,
    pub play14: bool,
    pub offsets: bool,
}

impl SkinReloadRequest {
    pub fn any_reload(self) -> bool {
        self.select
            || self.decide
            || self.result
            || self.course_result
            || self.play4
            || self.play5
            || self.play6
            || self.play7
            || self.play8
            || self.play9
            || self.play10
            || self.play14
    }

    pub fn any(self) -> bool {
        self.any_reload() || self.offsets
    }

    pub fn union(&mut self, other: Self) {
        self.select |= other.select;
        self.decide |= other.decide;
        self.result |= other.result;
        self.course_result |= other.course_result;
        self.play4 |= other.play4;
        self.play5 |= other.play5;
        self.play6 |= other.play6;
        self.play7 |= other.play7;
        self.play8 |= other.play8;
        self.play9 |= other.play9;
        self.play10 |= other.play10;
        self.play14 |= other.play14;
        self.offsets |= other.offsets;
    }
}

#[derive(Clone, Default)]
pub struct SceneSkinDefs {
    pub property: Vec<SkinPropertyDef>,
    pub filepath: Vec<SkinFilepathDef>,
    pub offset: Vec<SkinOffsetDef>,
}

impl SceneSkinDefs {
    /// renderer の `SkinDocument` から設定可能項目の定義を複製する。
    pub fn from_document(document: Option<&SkinDocument>) -> Self {
        match document {
            Some(doc) => Self {
                property: doc.property.clone(),
                filepath: doc.filepath.clone(),
                offset: doc.offset.clone(),
            },
            None => Self::default(),
        }
    }

    /// beatoraja はすべてのプレイ用スキンに共通 offset を追加するため、
    /// BMZ のスキン設定 UI でも play skin だけ同じ項目を常時出す。
    pub fn from_play_document(document: Option<&SkinDocument>) -> Self {
        let mut defs = Self::from_document(document);
        defs.append_play_common_offsets();
        defs
    }

    fn is_empty(&self) -> bool {
        self.property.is_empty() && self.filepath.is_empty() && self.offset.is_empty()
    }

    fn append_play_common_offsets(&mut self) {
        // beatoraja はスキン定義との ID 重複を除外せず、共通 offset を定義列の
        // 末尾へ追加する。runtime の ID map では後勝ちになる一方、設定値は名前で
        // 独立して保持される。
        for offset in beatoraja_play_common_offsets() {
            self.offset.push(offset);
        }

        // Bar Line offset は BMZ 独自拡張で、beatoraja の共通 offset とは分けて
        // 従来どおり ID 34 の定義を補完する。
        let bar_line = bmz_play_bar_line_offset();
        if let Some(existing) =
            self.offset.iter_mut().find(|existing| existing.id == SKIN_OFFSET_BAR_LINE)
        {
            existing.h = true;
            existing.a = true;
        } else {
            self.offset.push(bar_line);
        }
    }
}

fn beatoraja_play_common_offsets() -> [SkinOffsetDef; 4] {
    [
        SkinOffsetDef {
            category: "beatoraja".to_string(),
            name: "All offset(%)".to_string(),
            id: 10,
            x: true,
            y: true,
            w: true,
            h: true,
            r: false,
            a: false,
        },
        SkinOffsetDef {
            category: "beatoraja".to_string(),
            name: "Notes offset".to_string(),
            id: 30,
            x: false,
            y: false,
            w: false,
            h: true,
            r: false,
            a: false,
        },
        SkinOffsetDef {
            category: "beatoraja".to_string(),
            name: "Judge offset".to_string(),
            id: 32,
            x: true,
            y: true,
            w: true,
            h: true,
            r: false,
            a: true,
        },
        SkinOffsetDef {
            category: "beatoraja".to_string(),
            name: "Judge Detail offset".to_string(),
            id: 33,
            x: true,
            y: true,
            w: true,
            h: true,
            r: false,
            a: true,
        },
    ]
}

fn bmz_play_bar_line_offset() -> SkinOffsetDef {
    SkinOffsetDef {
        category: "bmz".to_string(),
        name: "Bar Line offset".to_string(),
        id: SKIN_OFFSET_BAR_LINE,
        x: false,
        y: false,
        w: false,
        h: true,
        r: false,
        a: true,
    }
}

/// 選曲 / プレイ / リザルト各スキンの設定可能項目。
#[derive(Default)]
pub struct SkinConfigMeta {
    pub select: SceneSkinDefs,
    pub decide: SceneSkinDefs,
    pub play4: SceneSkinDefs,
    pub play5: SceneSkinDefs,
    pub play6: SceneSkinDefs,
    pub play7: SceneSkinDefs,
    pub play8: SceneSkinDefs,
    pub play9: SceneSkinDefs,
    pub play10: SceneSkinDefs,
    pub play14: SceneSkinDefs,
    pub battle5: SceneSkinDefs,
    pub battle7: SceneSkinDefs,
    pub result: SceneSkinDefs,
    pub course_result: SceneSkinDefs,
}

#[derive(Debug, Clone, Default)]
pub struct SkinCatalog {
    pub select: Vec<SkinCandidate>,
    pub decide: Vec<SkinCandidate>,
    pub play4: Vec<SkinCandidate>,
    pub play5: Vec<SkinCandidate>,
    pub play6: Vec<SkinCandidate>,
    pub play7: Vec<SkinCandidate>,
    pub play8: Vec<SkinCandidate>,
    pub play9: Vec<SkinCandidate>,
    pub play10: Vec<SkinCandidate>,
    pub play14: Vec<SkinCandidate>,
    pub battle5: Vec<SkinCandidate>,
    pub battle7: Vec<SkinCandidate>,
    pub result: Vec<SkinCandidate>,
    pub course_result: Vec<SkinCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinCandidate {
    pub name: String,
    pub path: String,
    pub origin: SkinCandidateOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinCandidateOrigin {
    Bundled,
    User,
    External,
}

/// デバッグ表示パネルへ毎フレーム渡すアプリ側の情報。
pub struct DebugInfo {
    /// 現在のシーン種別 ("Select" / "Play" / "Result")。
    pub scene: &'static str,
    /// 右上 FPS オーバーレイと同じ、1 秒間の実測 FPS。
    pub current_fps: u32,
    /// 描画サーフェスの幅 (px)。
    pub width: u32,
    /// 描画サーフェスの高さ (px)。
    pub height: u32,
    /// GPU/OS capabilityのfallbackを反映した実効present mode。
    pub effective_present_mode: Option<&'static str>,
    /// swapchainに許可している最大in-flight frame数。
    pub maximum_frame_latency: Option<u32>,
}

/// `EguiLayer::run` の 1 フレーム入力。
pub struct EguiRunContext<'a, 'practice> {
    pub info: &'a DebugInfo,
    pub log_buffer: &'a LogBuffer,
    pub app_config: &'a mut AppConfig,
    pub profile_config: &'a mut ProfileConfig,
    pub random_trainer: &'a mut RandomTrainerState,
    pub skin_meta: &'a SkinConfigMeta,
    pub skin_catalog: &'a SkinCatalog,
    pub course_result: Option<&'a CourseResultSummary>,
    pub course_preview: Option<&'a SelectCourseRow>,
    pub practice: Option<&'a mut PracticePanelContext<'practice>>,
    pub result_ir: Option<&'a mut crate::screens::result_ir::ResultIrState>,
    pub profile_root: &'a Path,
    pub app_paths: &'a AppPaths,
    /// 取得済み難易度表のメタデータ。設定済み URL の表示名解決に使う。
    pub difficulty_tables: &'a [DifficultyTableRecord],
    pub update_dialog: Option<UpdateDialog<'a>>,
    pub obs_connection_status: &'a crate::obs::ObsConnectionStatus,
    /// 接続中ゲームパッド一覧 (gilrs)。未初期化時は空。
    pub connected_gamepads: &'a [crate::input::gamepad::ConnectedGamepad],
}

/// `EguiLayer::run` の 1 フレーム出力。
pub struct EguiOutput {
    /// renderer へ渡す描画データ。
    pub frame: EguiFrame,
    /// OBS WebSocket の有効/無効変更を実行中のコントローラへ即時反映する要求。
    pub obs_enabled_changed: bool,
    /// 本体設定 (`AppConfig`) の保存が要求されたか。
    pub save_app_config: bool,
    /// プロファイル設定 (`ProfileConfig`) の保存が要求されたか。
    pub save_profile_config: bool,
    /// profile.toml からスキン設定を再読込して未保存変更を戻す要求。
    pub reset_skin_config: bool,
    /// スキン設定値のうち、再読込や即時反映が必要な対象。
    pub skin_reload_request: SkinReloadRequest,
    /// 有効な曲ルートをライブラリ DB へ再スキャンする要求。
    pub trigger_song_rescan: bool,
    /// 曲フォルダのスキャン要求。
    pub song_scan_requests: Vec<SongScanRequest>,
    /// 難易度表の取得要求。空なら取得しない。
    pub table_fetch_urls: Vec<String>,
    pub score_import_request: Option<ScoreImportRequest>,
    /// 現在の設定で音声出力(cpal ストリーム)を開き直す要求。
    pub apply_audio_output: bool,
    pub check_for_update: bool,
    pub update_dialog_action: Option<UpdateDialogAction>,
    pub practice_start: bool,
    pub practice_leave: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum UpdateDialog<'a> {
    Available(&'a UpdateCandidate),
    Downloading(&'a UpdateCandidate),
    Error { message: &'a str, candidate: Option<&'a UpdateCandidate> },
    UpToDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateDialogAction {
    Update,
    NotNow,
    SkipRelease,
    OpenReleasePage,
}

#[derive(Clone, Debug)]
pub struct SongScanRequest {
    pub roots: Vec<PathEntry>,
    pub force: bool,
    pub label: String,
}

/// egui の状態管理とフレーム構築を担うレイヤ。
pub struct EguiLayer {
    ctx: egui::Context,
    state: egui_winit::State,
    /// egui の未指定テキストで最優先する地域別 CJK coverage。
    font_coverage: bmz_render::FontCoverage,
    /// OS フォントに依存しない CJK fallback の検索先。
    font_search_paths: Vec<PathBuf>,
    /// メニュー全体の表示状態。F1 でトグルする。
    visible: bool,
    /// デバッグ表示パネルの開閉状態。
    show_debug: bool,
    /// 7K RANDOM 固定配置パネルの開閉状態。
    show_random_trainer: bool,
    /// デバッグ表示内のログ最低表示レベル。
    debug_log_filter: DebugLogFilter,
    /// デバッグ表示内のログを末尾へ追従するか。
    debug_log_autoscroll: bool,
    /// 右上 FPS オーバーレイの表示状態。
    show_fps: bool,
    /// 本体設定パネルの開閉状態。
    show_settings: bool,
    /// プロファイル設定パネルの開閉状態。
    show_profile_settings: bool,
    /// スキン設定パネルの開閉状態。
    show_skin: bool,
    /// ライセンス / third-party notice 表示パネルの開閉状態。
    show_license_notice: bool,
    /// ライセンス表示パネルに出す結合済み notice text。
    license_notice_text: Option<String>,
    update_dialog_active: bool,
    /// 本体設定パネル: 曲フォルダ追加用の入力欄。
    settings_new_root_path: String,
    /// 本体設定パネル: 曲フォルダ追加の直近エラー。
    settings_add_root_error: String,
    settings_new_table_url: String,
    settings_add_table_error: String,
    score_import_path: String,
    score_import_kind: ScoreImportKind,
    score_import_device_type: InputDeviceKind,
    score_import_status: String,
    score_import_error: String,
    /// 本体設定パネル: 出力デバイス選択用の列挙キャッシュ。
    audio_device_picker: AudioDevicePickerState,
    /// 本体設定パネル: OBS scene list 取得状態。
    obs_scene_picker: ObsScenePickerState,
    /// プロファイル設定パネル: IR ログインフォームの状態。
    ir_login: IrLoginUiState,
    /// プロファイル設定パネル: IR device key 操作用の状態。
    ir_device_key: IrDeviceKeyUiState,
    /// プロファイル設定パネル: profile 作成 / 複製フォームの状態。
    profile_manager: ProfileManagerUiState,
    /// BMZ メニュー: OS のファイルマネージャでディレクトリを開いた直近結果。
    directory_open_status: Option<DirectoryOpenStatus>,
}

#[derive(Debug, Clone)]
struct DirectoryOpenStatus {
    label: &'static str,
    path: PathBuf,
    error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct DirectoryOpenTarget<'a> {
    label: &'static str,
    path: &'a Path,
}

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

/// 設定パネルの出力デバイス選択 ComboBox 用キャッシュ。
#[derive(Default)]
struct AudioDevicePickerState {
    /// 列挙済み出力デバイス名(ASIO ならドライバ名)。
    names: Vec<String>,
    /// `names` を列挙したときのバックエンド。変化したら再列挙する。
    backend: Option<AudioBackend>,
}

impl EguiLayer {
    /// `show_fps` は右上 FPS オーバーレイの初期表示状態。
    pub fn new(window: &Window, show_fps: bool, font_search_paths: Vec<PathBuf>) -> Self {
        let ctx = egui::Context::default();
        let font_coverage = bmz_render::FontCoverage::Japanese;
        install_cjk_fonts(&ctx, font_coverage, &font_search_paths);
        let state = egui_winit::State::new(
            ctx.clone(),
            ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        Self {
            ctx,
            state,
            font_coverage,
            font_search_paths,
            visible: false,
            show_debug: false,
            show_random_trainer: false,
            debug_log_filter: DebugLogFilter::default(),
            debug_log_autoscroll: true,
            show_fps,
            show_settings: false,
            show_profile_settings: false,
            show_skin: false,
            show_license_notice: false,
            license_notice_text: None,
            update_dialog_active: false,
            settings_new_root_path: String::new(),
            settings_add_root_error: String::new(),
            settings_new_table_url: String::new(),
            settings_add_table_error: String::new(),
            score_import_path: String::new(),
            score_import_kind: ScoreImportKind::default(),
            score_import_device_type: InputDeviceKind::Keyboard,
            score_import_status: String::new(),
            score_import_error: String::new(),
            audio_device_picker: AudioDevicePickerState::default(),
            obs_scene_picker: ObsScenePickerState::default(),
            ir_login: IrLoginUiState::default(),
            ir_device_key: IrDeviceKeyUiState::default(),
            profile_manager: ProfileManagerUiState::default(),
            directory_open_status: None,
        }
    }

    /// メニュー表示状態を反転する (F1)。
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        tracing::info!(visible = self.visible, "egui menu toggled");
    }

    /// 選曲画面の「詳細設定」から egui メニューと本体設定パネルを開く。
    pub fn open_advanced_settings(&mut self) {
        self.visible = true;
        self.show_settings = true;
        tracing::info!("egui advanced settings opened from select");
    }

    pub fn set_score_import_status(&mut self, status: String, error: bool) {
        if error {
            self.score_import_error = status;
            self.score_import_status.clear();
        } else {
            self.score_import_status = status;
            self.score_import_error.clear();
        }
    }

    /// winit イベントを egui へ供給する。
    ///
    /// 戻り値が true のとき、その入力は egui が消費したのでゲーム側へ伝播させない。
    /// メニュー非表示中は egui に状態は渡すが消費とは扱わず、ゲーム操作を妨げない。
    pub fn on_window_event(
        &mut self,
        window: &Window,
        event: &WindowEvent,
        practice_overlay: bool,
    ) -> bool {
        let response = self.state.on_window_event(window, event);
        self.blocks_game_input(practice_overlay) && response.consumed
    }

    pub fn blocks_game_input(&self, practice_overlay: bool) -> bool {
        self.visible || practice_overlay || self.update_dialog_active
    }

    /// 設定 metadata や profile 差分検出を含む完全な egui frame が必要かを返す。
    ///
    /// Play 中に F1 menu 等が閉じている場合は、winit/egui の入力状態と texture
    /// delta だけを進める idle frame へ切り替えられる。
    pub fn needs_full_frame(
        &self,
        scene: &str,
        practice_overlay: bool,
        has_update_dialog: bool,
    ) -> bool {
        egui_frame_needs_full_state(
            self.visible,
            practice_overlay,
            has_update_dialog,
            scene,
            self.show_settings,
        )
    }

    /// UI が非表示のフレームを最小構成で進める。
    ///
    /// `take_egui_input` と `textures_delta` の消費は継続し、F1 で再表示したときに
    /// 入力状態や managed texture が不整合にならないようにする。
    pub fn run_idle_frame(
        &mut self,
        window: &Window,
        font_coverage: bmz_render::FontCoverage,
    ) -> EguiFrame {
        if font_coverage != self.font_coverage {
            install_cjk_fonts(&self.ctx, font_coverage, &self.font_search_paths);
            self.font_coverage = font_coverage;
        }
        self.update_dialog_active = false;
        let raw_input = self.state.take_egui_input(window);
        let full_output = self.ctx.run_ui(raw_input, |_| {});
        self.state.handle_platform_output(window, full_output.platform_output);
        let primitives = self.ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        EguiFrame {
            primitives,
            textures_delta: full_output.textures_delta,
            pixels_per_point: full_output.pixels_per_point,
        }
    }

    /// 1 フレーム分の UI を構築し、描画データと要求されたアクションを返す。
    pub fn run(&mut self, window: &Window, context: EguiRunContext<'_, '_>) -> EguiOutput {
        let EguiRunContext {
            info,
            log_buffer,
            app_config,
            profile_config,
            random_trainer,
            skin_meta,
            skin_catalog,
            course_result,
            course_preview,
            mut practice,
            mut result_ir,
            profile_root,
            app_paths,
            difficulty_tables,
            update_dialog,
            obs_connection_status,
            connected_gamepads,
        } = context;
        let font_coverage = profile_config.ui.locale().font_coverage();
        if font_coverage != self.font_coverage {
            install_cjk_fonts(&self.ctx, font_coverage, &self.font_search_paths);
            self.font_coverage = font_coverage;
        }
        let text = Localizer::new(profile_config.ui.locale());
        let raw_input = self.state.take_egui_input(window);
        let ctx = self.ctx.clone();
        let show_debug = &mut self.show_debug;
        let show_random_trainer = &mut self.show_random_trainer;
        let show_settings = &mut self.show_settings;
        let show_profile_settings = &mut self.show_profile_settings;
        let show_skin = &mut self.show_skin;
        let show_fps = &mut self.show_fps;
        let show_license_notice = &mut self.show_license_notice;
        let license_notice_text = &mut self.license_notice_text;
        let mut obs_enabled_changed = false;
        let mut save_app_config = false;
        let mut save_profile_config = false;
        let mut reset_skin_config = false;
        let mut skin_reload_request = SkinReloadRequest::default();
        let mut trigger_song_rescan = false;
        let mut song_scan_requests = Vec::new();
        let mut table_fetch_urls = Vec::new();
        let mut score_import_request = None;
        let mut apply_audio_output = false;
        let mut check_for_update = false;
        let mut update_dialog_action = None;
        let mut practice_start = false;
        let mut practice_leave = false;
        let settings_editable = !scene_restricts_settings(info.scene);
        let mut readonly_app_config = (!settings_editable).then(|| app_config.clone());
        let visible_flag = &mut self.visible;
        let ir_login = &mut self.ir_login;
        let directory_open_status = &mut self.directory_open_status;
        let update_dialog_allowed =
            update_dialog.is_some() && (info.scene == "Select" || *show_settings);
        self.update_dialog_active = update_dialog_allowed;
        let full_output = ctx.run_ui(raw_input, |ui| {
            if update_dialog_allowed && let Some(dialog) = update_dialog {
                update_dialog_action = build_update_dialog(ui.ctx(), dialog, text);
            }
            if let Some(practice_ctx) = practice.as_mut() {
                let panel = build_practice_panel(ui.ctx(), practice_ctx, text);
                practice_start |= panel.start_play;
                practice_leave |= panel.leave;
            }
            if *visible_flag {
                let ctx = ui.ctx();
                let result_ir_visible = result_ir.is_some();
                // IR ランキングも egui 補助ウィンドウなので、他の egui
                // ウィンドウと同じ F1 メニュー表示中だけ出す。
                if let Some(state) = result_ir.as_mut() {
                    build_result_ir_panel(ctx, state, text);
                }
                // Course info panels are developer/debug egui overlays, so keep
                // them behind the same F1 menu visibility gate as the other
                // egui windows.
                if let Some(summary) = course_result {
                    build_course_result_panel(ctx, summary, result_ir_visible, text);
                }
                if let Some(preview) = course_preview {
                    build_course_preview_panel(ctx, preview, text);
                }
                build_menu(
                    ctx,
                    visible_flag,
                    MenuPanelVisibility {
                        debug: show_debug,
                        random_trainer: show_random_trainer,
                        settings: show_settings,
                        profile_settings: show_profile_settings,
                        skin: show_skin,
                        license_notice: show_license_notice,
                    },
                    app_paths,
                    directory_open_status,
                    text,
                );
                build_third_party_notice_panel(
                    ctx,
                    show_license_notice,
                    app_paths,
                    license_notice_text,
                    text,
                );
                build_debug_panel(
                    ctx,
                    show_debug,
                    info,
                    log_buffer,
                    &mut self.debug_log_filter,
                    &mut self.debug_log_autoscroll,
                    text,
                );
                build_random_trainer_panel(ctx, show_random_trainer, random_trainer, text);
                let settings_actions = build_settings_panel(
                    ctx,
                    window,
                    show_settings,
                    if settings_editable {
                        app_config
                    } else {
                        readonly_app_config.as_mut().expect("read-only config must exist")
                    },
                    profile_config,
                    show_fps,
                    settings_editable,
                    difficulty_tables,
                    text,
                    SettingsPanelState {
                        new_root_path: &mut self.settings_new_root_path,
                        add_root_error: &mut self.settings_add_root_error,
                        new_table_url: &mut self.settings_new_table_url,
                        add_table_error: &mut self.settings_add_table_error,
                        score_import_path: &mut self.score_import_path,
                        score_import_kind: &mut self.score_import_kind,
                        score_import_device_type: &mut self.score_import_device_type,
                        score_import_status: &self.score_import_status,
                        score_import_error: &self.score_import_error,
                        audio_device_picker: &mut self.audio_device_picker,
                        obs_scene_picker: &mut self.obs_scene_picker,
                        obs_connection_status,
                        connected_gamepads,
                    },
                );
                obs_enabled_changed |= settings_actions.obs_enabled_changed;
                save_app_config |= settings_actions.save;
                save_profile_config |= settings_actions.save_profile;
                check_for_update |= settings_actions.check_update;
                trigger_song_rescan |= settings_actions.rescan;
                song_scan_requests.extend(settings_actions.song_scan_requests);
                table_fetch_urls.extend(settings_actions.table_fetch_urls);
                apply_audio_output |= settings_actions.apply_audio;
                score_import_request = settings_actions.score_import_request;
                let profile_settings_actions = build_profile_settings_panel(
                    ctx,
                    show_profile_settings,
                    profile_config,
                    app_config,
                    show_fps,
                    ir_login,
                    &mut self.ir_device_key,
                    &mut self.profile_manager,
                    profile_root,
                    settings_editable,
                    text,
                );
                save_profile_config |= profile_settings_actions.save;
                save_app_config |= profile_settings_actions.save_app_config;
                let skin_actions = build_skin_panel(
                    ctx,
                    show_skin,
                    &mut profile_config.skin,
                    skin_meta,
                    skin_catalog,
                    app_paths,
                    text,
                );
                save_profile_config |= skin_actions.save;
                reset_skin_config |= skin_actions.reset;
                skin_reload_request.union(skin_actions.reload);
            }
        });
        self.state.handle_platform_output(window, full_output.platform_output);
        let primitives = self.ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        EguiOutput {
            frame: EguiFrame {
                primitives,
                textures_delta: full_output.textures_delta,
                pixels_per_point: full_output.pixels_per_point,
            },
            obs_enabled_changed,
            save_app_config,
            save_profile_config,
            reset_skin_config,
            skin_reload_request,
            trigger_song_rescan,
            song_scan_requests,
            table_fetch_urls,
            score_import_request,
            apply_audio_output,
            check_for_update,
            update_dialog_action,
            practice_start,
            practice_leave,
        }
    }
}

fn egui_frame_needs_full_state(
    visible: bool,
    practice_overlay: bool,
    has_update_dialog: bool,
    scene: &str,
    show_settings: bool,
) -> bool {
    visible || practice_overlay || (has_update_dialog && (scene == "Select" || show_settings))
}

/// egui のデフォルトフォントは CJK グリフを含まないため、locale の地域別字形を
/// 優先した全 CJK face を各フォントファミリの末尾 fallback として登録する。
fn install_cjk_fonts(
    ctx: &egui::Context,
    preferred: bmz_render::FontCoverage,
    font_search_paths: &[PathBuf],
) {
    let fallbacks = bmz_render::renderer::load_cjk_font_fallback_data(preferred, font_search_paths);
    ctx.set_fonts(cjk_font_definitions(fallbacks));
}

fn cjk_font_definitions(
    fallbacks: Vec<(bmz_render::FontCoverage, bmz_render::renderer::SystemFontData)>,
) -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    for (coverage, data) in fallbacks {
        let name = cjk_font_name(coverage).to_owned();
        let mut font_data = egui::FontData::from_owned(data.bytes).tweak(egui::FontTweak {
            scale: 1.0,
            y_offset_factor: 0.26,
            y_offset: 0.0,
            ..Default::default()
        });
        font_data.index = data.font_index;
        fonts.font_data.insert(name.clone(), std::sync::Arc::new(font_data));
        // Latin は egui 既定フォントの先頭順を維持し、欠落グリフだけ CJK face へ
        // preferred 順で fallback させる。
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            if let Some(chain) = fonts.families.get_mut(&family) {
                chain.push(name.clone());
            }
        }
    }
    fonts
}

const fn cjk_font_name(coverage: bmz_render::FontCoverage) -> &'static str {
    match coverage {
        bmz_render::FontCoverage::Japanese => "bmz_cjk_japanese",
        bmz_render::FontCoverage::Korean => "bmz_cjk_korean",
        bmz_render::FontCoverage::SimplifiedChinese => "bmz_cjk_simplified_chinese",
        bmz_render::FontCoverage::TraditionalChinese => "bmz_cjk_traditional_chinese",
        bmz_render::FontCoverage::HongKong => "bmz_cjk_hong_kong",
    }
}

/// 各サブパネルの開閉を切り替えるメインメニューハブ。
struct MenuPanelVisibility<'a> {
    debug: &'a mut bool,
    random_trainer: &'a mut bool,
    settings: &'a mut bool,
    profile_settings: &'a mut bool,
    skin: &'a mut bool,
    license_notice: &'a mut bool,
}

fn build_menu(
    ctx: &egui::Context,
    visible: &mut bool,
    panels: MenuPanelVisibility<'_>,
    app_paths: &AppPaths,
    directory_open_status: &mut Option<DirectoryOpenStatus>,
    text: Localizer,
) {
    egui::Window::new(tr!(text, "menu-title"))
        .id(egui::Id::new("bmz_menu"))
        .open(visible)
        .constrain_to(ctx.content_rect().shrink(PANEL_VIEWPORT_MARGIN))
        .default_pos(egui::pos2(16.0, 16.0))
        .show(ctx, |ui| {
            ui.label(tr!(text, "menu-toggle-help"));
            ui.separator();
            ui.checkbox(panels.debug, tr!(text, "menu-debug"));
            ui.checkbox(panels.random_trainer, tr!(text, "menu-random-trainer"));
            ui.checkbox(panels.settings, tr!(text, "menu-app-settings"));
            ui.checkbox(panels.profile_settings, tr!(text, "menu-profile-settings"));
            ui.checkbox(panels.skin, tr!(text, "menu-skin-settings"));
            ui.checkbox(panels.license_notice, tr!(text, "menu-licenses"));
            ui.separator();
            ui.label(tr!(text, "menu-open-directory"));
            ui.horizontal_wrapped(|ui| {
                for target in directory_open_targets(app_paths) {
                    if ui
                        .button(target.label)
                        .on_hover_text(target.path.display().to_string())
                        .clicked()
                    {
                        *directory_open_status = Some(open_directory_target(target, text));
                    }
                }
            });
            if let Some(status) = directory_open_status.as_ref() {
                match status.error.as_deref() {
                    Some(error) => {
                        ui.colored_label(
                            egui::Color32::LIGHT_RED,
                            tr!(
                                text,
                                "menu-directory-open-failed",
                                "label" => status.label,
                                "error" => error
                            ),
                        )
                        .on_hover_text(status.path.display().to_string());
                    }
                    None => {
                        ui.small(tr!(
                            text,
                            "menu-directory-opened",
                            "label" => status.label
                        ))
                        .on_hover_text(status.path.display().to_string());
                    }
                }
            }
        });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RandomTrainerLaneDrag {
    index: usize,
}

fn build_random_trainer_panel(
    ctx: &egui::Context,
    visible: &mut bool,
    trainer: &mut RandomTrainerState,
    text: Localizer,
) {
    if !*visible {
        return;
    }

    egui::Window::new(tr!(text, "random-trainer-title"))
        .id(egui::Id::new("bmz_random_trainer"))
        .open(visible)
        .resizable(false)
        .constrain_to(ctx.content_rect().shrink(PANEL_VIEWPORT_MARGIN))
        .default_pos(egui::pos2(360.0, 32.0))
        .show(ctx, |ui| {
            let mut enabled = trainer.is_enabled();
            if ui.checkbox(&mut enabled, tr!(text, "random-trainer-enabled")).changed() {
                trainer.set_enabled(enabled);
            }
            ui.label(tr!(text, "random-trainer-description"));
            ui.label(tr!(text, "random-trainer-next-play"));
            let mut black_white_random = trainer.black_white_random();
            if ui
                .checkbox(&mut black_white_random, tr!(text, "random-trainer-black-white"))
                .changed()
            {
                trainer.set_black_white_random(black_white_random);
            }
            ui.label(tr!(text, "random-trainer-black-white-help"));
            ui.label(tr!(text, "random-trainer-partial-help"));
            ui.separator();
            ui.label(format!(
                "{} {}",
                tr!(text, "random-trainer-order"),
                trainer.lane_order_string()
            ));

            let lane_order = *trainer.lane_order();
            let mut swap = None;
            let mut toggle_partial = None;
            ui.horizontal(|ui| {
                for (index, lane) in lane_order.into_iter().enumerate() {
                    ui.push_id(("random_trainer_lane", index), |ui| {
                        let is_blue = lane % 2 == 0;
                        let is_partial_random = trainer.is_lane_partial_random(lane);
                        let fill = if is_blue {
                            egui::Color32::from_rgb(0, 60, 150)
                        } else {
                            egui::Color32::from_rgb(235, 238, 244)
                        };
                        let text_color = if is_blue {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_gray(35)
                        };
                        let label =
                            if is_partial_random { format!("{lane}\n?") } else { lane.to_string() };
                        let mut button = egui::Button::new(
                            egui::RichText::new(label).size(20.0).color(text_color),
                        )
                        .fill(fill)
                        .sense(egui::Sense::click_and_drag());
                        if is_partial_random {
                            button = button.stroke(egui::Stroke::new(
                                3.0_f32,
                                egui::Color32::from_rgb(220, 80, 150),
                            ));
                        }
                        let (_, dropped) =
                            ui.dnd_drop_zone::<RandomTrainerLaneDrag, _>(egui::Frame::NONE, |ui| {
                                let response = ui.add_sized([42.0, 64.0], button);
                                response.dnd_set_drag_payload(RandomTrainerLaneDrag { index });
                                let response = response
                                    .on_hover_cursor(egui::CursorIcon::Grab)
                                    .on_hover_text(tr!(text, "random-trainer-drag"));
                                if response.secondary_clicked() {
                                    toggle_partial = Some(lane);
                                }
                            });
                        if let Some(payload) = dropped {
                            swap = Some((payload.index, index));
                        }
                    });
                }
            });
            if let Some((from, to)) = swap {
                trainer.swap_positions(from, to);
            }
            if let Some(lane) = toggle_partial {
                trainer.toggle_lane_partial_random(lane);
            }

            ui.horizontal_wrapped(|ui| {
                if ui.button(tr!(text, "random-trainer-reset")).clicked() {
                    trainer.reset();
                }
                if ui.button(tr!(text, "random-trainer-mirror")).clicked() {
                    trainer.mirror();
                }
                if ui.button(tr!(text, "random-trainer-shift-left")).clicked() {
                    trainer.shift_left();
                }
                if ui.button(tr!(text, "random-trainer-shift-right")).clicked() {
                    trainer.shift_right();
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ir_provider(provider: &str, base_url: &str) -> IrProviderConfig {
        IrProviderConfig {
            provider: provider.to_string(),
            provider_key: String::new(),
            base_url: base_url.to_string(),
            enabled: false,
            account_display_name: String::new(),
            account_id: String::new(),
            send_policy: IrSendPolicyConfig::default(),
            role: IrProviderRoleConfig::default(),
            last_login_at: None,
            last_success_at: None,
        }
    }

    #[test]
    fn ir_provider_presets_recognize_official_and_legacy_urls() {
        assert_eq!(
            classify_ir_provider_preset(&test_ir_provider(
                "bmz-official",
                "https://bmz-player.hyrorre.workers.dev"
            )),
            IrProviderPreset::BmzIr
        );
        assert_eq!(
            classify_ir_provider_preset(&test_ir_provider("rianIR", "https://rianir.link/api/")),
            IrProviderPreset::RianIr
        );
        assert_eq!(
            classify_ir_provider_preset(&test_ir_provider("rian-ir", "http://localhost:8888/api/")),
            IrProviderPreset::Other
        );
    }

    #[test]
    fn applying_ir_provider_presets_writes_canonical_values() {
        let mut provider = test_ir_provider("custom", "http://localhost:8888/");
        apply_ir_provider_preset(&mut provider, IrProviderPreset::BmzIr);
        assert_eq!(provider.provider, "bmz");
        assert_eq!(provider.base_url, "https://bmz-player.hyrorre.workers.dev/");

        apply_ir_provider_preset(&mut provider, IrProviderPreset::RianIr);
        assert_eq!(provider.provider, "rian-ir");
        assert_eq!(provider.base_url, "https://rianir.link/");

        apply_ir_provider_preset(&mut provider, IrProviderPreset::Other);
        assert_eq!(provider.provider, "rian-ir");
        assert_eq!(provider.base_url, "https://rianir.link/");
    }

    #[test]
    fn cjk_font_definitions_keep_latin_first_and_preserve_face_indices() {
        use bmz_render::FontCoverage;
        use bmz_render::renderer::SystemFontData;

        let defaults = egui::FontDefinitions::default();
        let fonts = cjk_font_definitions(vec![
            (FontCoverage::Korean, SystemFontData { bytes: vec![1], font_index: 3 }),
            (FontCoverage::Japanese, SystemFontData { bytes: vec![2], font_index: 7 }),
        ]);

        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let default_chain = defaults.families.get(&family).expect("default family");
            let chain = fonts.families.get(&family).expect("CJK family");
            assert_eq!(&chain[..default_chain.len()], default_chain);
            assert_eq!(
                &chain[default_chain.len()..],
                &["bmz_cjk_korean".to_string(), "bmz_cjk_japanese".to_string()]
            );
        }
        assert_eq!(fonts.font_data["bmz_cjk_korean"].index, 3);
        assert_eq!(fonts.font_data["bmz_cjk_japanese"].index, 7);
    }

    #[test]
    fn decide_and_play_restrict_settings_panels() {
        assert!(!scene_restricts_settings("Select"));
        assert!(scene_restricts_settings("Decide"));
        assert!(scene_restricts_settings("Play"));
        assert!(!scene_restricts_settings("Result"));
    }

    #[test]
    fn hidden_play_egui_uses_idle_frame_until_an_overlay_needs_full_state() {
        assert!(!egui_frame_needs_full_state(false, false, false, "Play", false));
        assert!(egui_frame_needs_full_state(true, false, false, "Play", false));
        assert!(egui_frame_needs_full_state(false, true, false, "Play", false));
        assert!(egui_frame_needs_full_state(false, false, true, "Select", false));
        assert!(egui_frame_needs_full_state(false, false, true, "Play", true));
        assert!(!egui_frame_needs_full_state(false, false, true, "Play", false));
    }

    #[test]
    fn difficulty_table_source_label_shows_fetched_table_name() {
        let tables = vec![DifficultyTableRecord {
            id: 1,
            source_url: "https://example.com/header.json".to_string(),
            name: "発狂BMS難易度表".to_string(),
            symbol: "★".to_string(),
            level_order: vec!["1".to_string()],
            fetched_at: 1_700_000_000,
        }];

        assert_eq!(
            difficulty_table_source_label("https://example.com/header.json", &tables),
            "発狂BMS難易度表 (https://example.com/header.json)"
        );
    }

    #[test]
    fn difficulty_table_source_label_keeps_url_before_first_fetch() {
        assert_eq!(
            difficulty_table_source_label("https://example.com/header.json", &[]),
            "https://example.com/header.json"
        );
    }

    #[test]
    fn debug_log_filter_keeps_selected_level_and_more_severe_entries() {
        assert!(!DebugLogFilter::Info.allows(TracingLogLevel::Debug));
        assert!(DebugLogFilter::Info.allows(TracingLogLevel::Info));
        assert!(DebugLogFilter::Info.allows(TracingLogLevel::Error));
        assert!(DebugLogFilter::All.allows(TracingLogLevel::Trace));
    }

    #[test]
    fn debug_log_copy_text_includes_level_target_and_message() {
        let entry = LogEntry {
            level: TracingLogLevel::Warn,
            target: "bmz_player::test".to_string(),
            message: "slow frame".to_string(),
        };

        let text = Localizer::new(AppLocale::En);
        assert_eq!(format_log_entry(&entry, text), "[WARN] bmz_player::test slow frame");

        let empty = LogEntry { message: String::new(), ..entry };
        assert_eq!(format_log_entry(&empty, text), "[WARN] bmz_player::test (no message)");
    }

    #[test]
    fn restricted_profile_settings_keep_only_realtime_categories() {
        let baseline = ProfileConfig::new_default("default", "Default", 1);
        let mut edited = baseline.clone();
        edited.display_name = "Changed".to_string();
        edited.play.rule_mode = RuleMode::Dx;
        edited.audio_mix.master_volume = 23;
        edited.judge.input_offset_us = 4_000;
        edited.lane.hispeed = 3.25;
        edited.input.analog_scratch_threshold = 321;
        edited.input.keyboard_release_bounce_ms = 4;
        edited.input.controller_release_bounce_ms = 7;

        restore_restricted_profile_settings(&mut edited, baseline.clone());

        assert_eq!(edited.display_name, baseline.display_name);
        assert_eq!(edited.play.rule_mode, baseline.play.rule_mode);
        assert_eq!(edited.audio_mix.master_volume, 23);
        assert_eq!(edited.judge.input_offset_us, 4_000);
        assert_eq!(edited.lane.hispeed, 3.25);
        assert_eq!(edited.input.analog_scratch_threshold, 321);
        assert_eq!(edited.input.keyboard_release_bounce_ms, 4);
        assert_eq!(edited.input.controller_release_bounce_ms, 7);
    }
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_test_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("{name}-{nanos}-{counter}"))
    }

    fn test_offset_def(name: &str, id: i32) -> SkinOffsetDef {
        SkinOffsetDef {
            category: "test".to_string(),
            name: name.to_string(),
            id,
            x: true,
            y: true,
            w: true,
            h: true,
            r: true,
            a: true,
        }
    }

    #[test]
    fn sanitize_profile_id_input_keeps_portable_path_chars_only() {
        let mut value = "abc_日本語-_.012/\\: xyz".to_string();

        sanitize_profile_id_input(&mut value);

        assert_eq!(value, "abc_-_012xyz");
    }

    #[test]
    fn sanitize_profile_id_input_truncates_to_profile_id_limit() {
        let mut value = "a".repeat(80);

        sanitize_profile_id_input(&mut value);

        assert_eq!(value.len(), 64);
    }

    #[test]
    fn skin_candidate_display_hides_bundled_origin_label_when_requested() {
        let candidate = SkinCandidate {
            name: "Default".to_string(),
            path: "resource:skins/default/select.json".to_string(),
            origin: SkinCandidateOrigin::Bundled,
        };

        assert_eq!(
            skin_candidate_display(&candidate, true, Localizer::new(crate::i18n::AppLocale::Ja),),
            "[同梱] Default (resource:skins/default/select.json)"
        );
        assert_eq!(
            skin_candidate_display(&candidate, false, Localizer::new(crate::i18n::AppLocale::Ja),),
            "Default (resource:skins/default/select.json)"
        );
    }

    #[test]
    fn skin_candidate_display_keeps_user_origin_label() {
        let candidate = SkinCandidate {
            name: "Custom".to_string(),
            path: "data:skins/custom/play7.luaskin".to_string(),
            origin: SkinCandidateOrigin::User,
        };

        assert_eq!(
            skin_candidate_display(&candidate, false, Localizer::new(crate::i18n::AppLocale::Ja),),
            "[ユーザー] Custom (data:skins/custom/play7.luaskin)"
        );
    }

    #[test]
    fn bundled_skin_origin_is_hidden_for_development_or_portable_layout() {
        let app_paths = AppPaths::from_dirs(
            PathBuf::from("data"),
            PathBuf::from("data"),
            PathBuf::from("data/cache"),
            PathBuf::from("data/logs"),
        );
        let mut catalog = SkinCatalog::default();
        catalog.select.push(SkinCandidate {
            name: "Default".to_string(),
            path: "resource:skins/default/select.json".to_string(),
            origin: SkinCandidateOrigin::Bundled,
        });
        catalog.select.push(SkinCandidate {
            name: "Custom".to_string(),
            path: "data:skins/custom/select.luaskin".to_string(),
            origin: SkinCandidateOrigin::User,
        });

        assert!(!show_bundled_skin_origin(&app_paths, &catalog));
    }

    #[test]
    fn bundled_skin_origin_is_shown_when_user_candidates_share_a_regular_layout() {
        let app_paths = AppPaths::from_dirs(
            PathBuf::from("resources"),
            PathBuf::from("profile-data"),
            PathBuf::from("profile-data/cache"),
            PathBuf::from("profile-data/logs"),
        );
        let mut catalog = SkinCatalog::default();
        catalog.select.push(SkinCandidate {
            name: "Default".to_string(),
            path: "resource:skins/default/select.json".to_string(),
            origin: SkinCandidateOrigin::Bundled,
        });
        catalog.select.push(SkinCandidate {
            name: "Custom".to_string(),
            path: "data:skins/custom/select.luaskin".to_string(),
            origin: SkinCandidateOrigin::User,
        });

        assert!(show_bundled_skin_origin(&app_paths, &catalog));
    }

    #[test]
    fn bundled_skin_origin_is_hidden_when_catalog_has_no_user_candidates() {
        let app_paths = AppPaths::from_dirs(
            PathBuf::from("resources"),
            PathBuf::from("profile-data"),
            PathBuf::from("profile-data/cache"),
            PathBuf::from("profile-data/logs"),
        );
        let mut catalog = SkinCatalog::default();
        catalog.select.push(SkinCandidate {
            name: "Default".to_string(),
            path: "resource:skins/default/select.json".to_string(),
            origin: SkinCandidateOrigin::Bundled,
        });

        assert!(!show_bundled_skin_origin(&app_paths, &catalog));
    }

    #[test]
    fn sync_ir_provider_roles_keeps_only_primary_role() {
        let mut ir_config = IrConfig {
            primary_provider: "bmz-dev".to_string(),
            providers: vec![
                IrProviderConfig {
                    provider: "bmz".to_string(),
                    provider_key: "bmz".to_string(),
                    base_url: "https://bmz-player.hyrorre.workers.dev".to_string(),
                    enabled: true,
                    account_display_name: String::new(),
                    account_id: String::new(),
                    send_policy: IrSendPolicyConfig::default(),
                    role: IrProviderRoleConfig::Primary,
                    last_login_at: None,
                    last_success_at: None,
                },
                IrProviderConfig {
                    provider: "bmz".to_string(),
                    provider_key: "bmz-dev".to_string(),
                    base_url: "http://localhost:3000".to_string(),
                    enabled: true,
                    account_display_name: String::new(),
                    account_id: String::new(),
                    send_policy: IrSendPolicyConfig::default(),
                    role: IrProviderRoleConfig::SubmitOnly,
                    last_login_at: None,
                    last_success_at: None,
                },
            ],
            ..IrConfig::default()
        };

        assert!(sync_ir_provider_roles(&mut ir_config));
        assert_eq!(ir_config.providers[0].role, IrProviderRoleConfig::SubmitOnly);
        assert_eq!(ir_config.providers[1].role, IrProviderRoleConfig::Primary);

        ir_config.primary_provider.clear();
        assert!(sync_ir_provider_roles(&mut ir_config));
        assert_eq!(ir_config.providers[0].role, IrProviderRoleConfig::SubmitOnly);
        assert_eq!(ir_config.providers[1].role, IrProviderRoleConfig::SubmitOnly);
    }

    #[test]
    fn clamp_panel_layout_fits_high_dpi_1920x1080_logical_viewport() {
        // 1920x1080 物理ウィンドウ @ 2x → egui 論理 960x540 相当。
        let constrain = egui::Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(928.0, 508.0));
        // egui 0.34 既定 style 付近の chrome 高さ (frame + title bar)。
        let chrome = egui::vec2(12.0, 58.0);
        let (default_inner, max_inner, pos) =
            clamp_panel_layout(constrain, chrome, 440.0, 560.0, egui::pos2(16.0, 480.0));

        let outer = default_inner + chrome;
        assert!(outer.x <= constrain.width() + 0.01);
        assert!(outer.y <= constrain.height() + 0.01);
        assert!(pos.x + outer.x <= constrain.max.x + 0.01);
        assert!(pos.y + outer.y <= constrain.max.y + 0.01);
        assert_eq!(pos, egui::pos2(16.0, 16.0));
        assert!(default_inner.y < 560.0);
        assert_eq!(max_inner, egui::vec2(916.0, 450.0));
    }

    #[test]
    fn clamp_panel_layout_keeps_preferred_size_on_large_viewport() {
        let constrain =
            egui::Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(1888.0, 1048.0));
        let chrome = egui::vec2(12.0, 58.0);
        let (default_inner, max_inner, pos) =
            clamp_panel_layout(constrain, chrome, 440.0, 560.0, egui::pos2(16.0, 480.0));

        assert_eq!(default_inner, egui::vec2(440.0, 560.0));
        assert_eq!(max_inner, egui::vec2(1876.0, 990.0));
        // outer 高さ 618 のため y=480 では下端がはみ出す → 446 へクランプ。
        assert_eq!(pos, egui::pos2(16.0, 446.0));
    }

    #[test]
    fn apply_settings_list_action_moves_and_removes_entries() {
        let mut items = vec!["a", "b", "c"];

        apply_settings_list_action(&mut items, SettingsListAction::MoveDown(0));
        assert_eq!(items, vec!["b", "a", "c"]);

        apply_settings_list_action(&mut items, SettingsListAction::MoveUp(2));
        assert_eq!(items, vec!["b", "c", "a"]);

        apply_settings_list_action(&mut items, SettingsListAction::Remove(1));
        assert_eq!(items, vec!["b", "a"]);
    }

    #[test]
    fn apply_settings_list_action_moves_entry_to_index() {
        let mut items = vec!["a", "b", "c", "d"];

        apply_settings_list_action(&mut items, SettingsListAction::MoveTo { from: 0, to: 2 });
        assert_eq!(items, vec!["b", "c", "a", "d"]);

        apply_settings_list_action(&mut items, SettingsListAction::MoveTo { from: 3, to: 1 });
        assert_eq!(items, vec!["b", "d", "c", "a"]);
    }

    #[test]
    fn apply_settings_list_action_ignores_invalid_moves() {
        let mut items = vec!["a", "b"];

        apply_settings_list_action(&mut items, SettingsListAction::MoveUp(0));
        apply_settings_list_action(&mut items, SettingsListAction::MoveDown(1));
        apply_settings_list_action(&mut items, SettingsListAction::MoveTo { from: 0, to: 2 });
        apply_settings_list_action(&mut items, SettingsListAction::MoveTo { from: 2, to: 0 });
        apply_settings_list_action(&mut items, SettingsListAction::Remove(2));

        assert_eq!(items, vec!["a", "b"]);
    }

    #[test]
    fn directory_open_targets_expose_only_app_path_roots() {
        let root = unique_test_dir("bmz-ui-directory-targets");
        let app_paths = AppPaths::from_dirs(
            root.join("resources"),
            root.join("data"),
            root.join("cache"),
            root.join("logs"),
        );

        let targets = directory_open_targets(&app_paths);
        let labels = targets.iter().map(|target| target.label).collect::<Vec<_>>();
        let paths = targets.iter().map(|target| target.path).collect::<Vec<_>>();

        assert_eq!(labels, vec!["resource_dir", "data_dir", "cache_dir", "logs_dir"]);
        assert_eq!(
            paths,
            vec![
                app_paths.resource_dir.as_path(),
                app_paths.data_dir.as_path(),
                app_paths.cache_dir.as_path(),
                app_paths.logs_dir.as_path(),
            ]
        );
    }

    #[test]
    fn combined_license_notice_uses_packaged_notice_files() {
        let root = unique_test_dir("bmz-ui-license-packaged");
        let resource_dir = root.join("resources");
        let license_dir = resource_dir.join("licenses");
        fs::create_dir_all(&license_dir).unwrap();
        fs::write(license_dir.join("third-party-notices.txt"), "packaged third party").unwrap();
        fs::write(license_dir.join("rust-dependency-licenses.txt"), "packaged rust report")
            .unwrap();
        let app_paths = AppPaths::from_dirs(
            resource_dir,
            root.join("data"),
            root.join("cache"),
            root.join("logs"),
        );

        let notice = combined_license_notice_text_with_repo_root(&app_paths, &root);

        assert!(notice.contains("packaged third party"));
        assert!(notice.contains("packaged rust report"));
        assert!(!notice.contains("The generated Rust dependency license report was not found."));
    }

    #[test]
    fn combined_license_notice_uses_local_rust_report_for_development() {
        let root = unique_test_dir("bmz-ui-license-local");
        let resource_dir = root.join("resources");
        let license_dir = resource_dir.join("licenses");
        fs::create_dir_all(&license_dir).unwrap();
        fs::write(license_dir.join("third-party-notices.txt"), "packaged third party").unwrap();
        fs::write(root.join("rust-dependency-licenses.txt"), "local rust report").unwrap();
        let app_paths = AppPaths::from_dirs(
            resource_dir,
            root.join("data"),
            root.join("cache"),
            root.join("logs"),
        );

        let notice = combined_license_notice_text_with_repo_root(&app_paths, &root);

        assert!(notice.contains("packaged third party"));
        assert!(notice.contains("local rust report"));
        assert!(!notice.contains("The generated Rust dependency license report was not found."));
    }

    #[test]
    fn combined_license_notice_explains_missing_rust_report() {
        let root = unique_test_dir("bmz-ui-license-missing");
        let app_paths = AppPaths::from_dirs(
            root.join("resources"),
            root.join("data"),
            root.join("cache"),
            root.join("logs"),
        );

        let notice = combined_license_notice_text_with_repo_root(&app_paths, &root);

        assert!(notice.contains("BMZ Player Third-Party Notices"));
        assert!(notice.contains("The generated Rust dependency license report was not found."));
        assert!(notice.contains("cargo-about generate --workspace --locked --fail"));
    }

    #[test]
    fn glob_candidates_lists_files_matching_simple_pattern() {
        let root = unique_test_dir("bmz-ui-glob");
        fs::create_dir_all(root.join("parts")).unwrap();
        fs::write(root.join("parts/a.png"), []).unwrap();
        fs::write(root.join("parts/b.png"), []).unwrap();
        fs::write(root.join("parts/c.txt"), []).unwrap();

        let candidates = glob_candidates(&root, "parts/*.png");

        assert_eq!(candidates.len(), 2);
        assert!(candidates.contains(&"parts/a.png".to_string()));
        assert!(candidates.contains(&"parts/b.png".to_string()));
    }

    #[test]
    fn glob_candidates_strips_beatoraja_filter_suffix() {
        let root = unique_test_dir("bmz-ui-glob");
        fs::create_dir_all(root.join("parts/lanecover_lift")).unwrap();
        fs::write(root.join("parts/lanecover_lift/default.png"), []).unwrap();
        fs::write(root.join("parts/lanecover_lift/TYPE-M.png"), []).unwrap();

        let candidates = glob_candidates(&root, "parts/lanecover_lift/*.png|lanecover|");

        assert_eq!(candidates.len(), 2);
        assert!(candidates.contains(&"parts/lanecover_lift/TYPE-M.png".to_string()));
        assert!(candidates.contains(&"parts/lanecover_lift/default.png".to_string()));
    }

    #[test]
    fn normalize_filepath_selection_maps_legacy_basename_to_relative_candidate() {
        let candidates =
            vec!["parts/gauge/default.png".to_string(), "parts/gauge/blue.png".to_string()];

        assert_eq!(
            normalize_filepath_selection("blue.png", &candidates).as_deref(),
            Some("parts/gauge/blue.png")
        );
        assert_eq!(normalize_filepath_selection("old/blue.png", &candidates), None);
    }

    #[test]
    fn property_default_uses_matching_def_name_or_first_item() {
        let prop = SkinPropertyDef {
            category: String::new(),
            name: "Notes".to_string(),
            item: vec![
                bmz_render::skin::SkinPropertyItemDef { name: "Light".to_string(), op: 1 },
                bmz_render::skin::SkinPropertyItemDef { name: "Dark".to_string(), op: 2 },
            ],
            def: "Dark".to_string(),
        };
        assert_eq!(property_default(&prop), "Dark");

        let prop = SkinPropertyDef { def: "Missing".to_string(), ..prop };
        assert_eq!(property_default(&prop), "Light");
    }

    #[test]
    fn filepath_default_matches_def_with_or_without_extension_case_insensitive() {
        let filepath = SkinFilepathDef {
            category: String::new(),
            name: "Notes".to_string(),
            path: "notes/*.png".to_string(),
            def: "default".to_string(),
        };
        let candidates = vec!["aaa.png".to_string(), "Default.PNG".to_string()];

        assert_eq!(filepath_default(&filepath, &candidates).as_deref(), Some("Default.PNG"));

        let filepath = SkinFilepathDef { def: "missing".to_string(), ..filepath };
        assert_eq!(filepath_default(&filepath, &candidates).as_deref(), Some("aaa.png"));
    }

    #[test]
    fn filepath_default_uses_random_sentinel_for_random_def() {
        // def="Random" は具体ファイルへ固定せず、ランダム番兵を既定にする。
        let filepath = SkinFilepathDef {
            category: String::new(),
            name: "BG".to_string(),
            path: "bg/*.mp4".to_string(),
            def: "Random".to_string(),
        };
        let candidates = vec!["bg/one.mp4".to_string(), "bg/two.mp4".to_string()];
        assert_eq!(
            filepath_default(&filepath, &candidates).as_deref(),
            Some(RANDOM_FILE_SELECTION)
        );
    }

    #[test]
    fn filepath_default_prefers_default_stem_when_def_missing() {
        let filepath = SkinFilepathDef {
            category: String::new(),
            name: "Note".to_string(),
            path: "notes/*.png".to_string(),
            def: String::new(),
        };
        let candidates = vec!["pastel.png".to_string(), "default.png".to_string()];

        assert_eq!(filepath_default(&filepath, &candidates).as_deref(), Some("default.png"));
    }

    #[test]
    fn fill_missing_skin_defaults_keeps_saved_values_and_fills_new_items() {
        let root = unique_test_dir("bmz-ui-defaults");
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(root.join("notes/aaa.png"), []).unwrap();
        fs::write(root.join("notes/default.png"), []).unwrap();
        let defs = SceneSkinDefs {
            property: vec![
                SkinPropertyDef {
                    category: String::new(),
                    name: "Lane".to_string(),
                    item: vec![
                        bmz_render::skin::SkinPropertyItemDef { name: "Off".to_string(), op: 0 },
                        bmz_render::skin::SkinPropertyItemDef { name: "On".to_string(), op: 1 },
                    ],
                    def: "On".to_string(),
                },
                SkinPropertyDef {
                    category: String::new(),
                    name: "Saved".to_string(),
                    item: vec![
                        bmz_render::skin::SkinPropertyItemDef { name: "A".to_string(), op: 0 },
                        bmz_render::skin::SkinPropertyItemDef { name: "B".to_string(), op: 1 },
                    ],
                    def: "A".to_string(),
                },
            ],
            filepath: vec![SkinFilepathDef {
                category: String::new(),
                name: "Notes".to_string(),
                path: "notes/*.png".to_string(),
                def: "default".to_string(),
            }],
            offset: Vec::new(),
        };
        let mut options = BTreeMap::from([("Saved".to_string(), "B".to_string())]);
        let mut files = BTreeMap::new();

        assert!(fill_missing_skin_defaults(&defs, Some(&root), &mut options, &mut files));

        assert_eq!(options.get("Lane").map(String::as_str), Some("On"));
        assert_eq!(options.get("Saved").map(String::as_str), Some("B"));
        assert_eq!(files.get("Notes").map(String::as_str), Some("notes/default.png"));
    }

    #[test]
    fn fill_missing_skin_defaults_replaces_stale_option_selection() {
        let defs = SceneSkinDefs {
            property: vec![SkinPropertyDef {
                category: String::new(),
                name: "Graph".to_string(),
                item: vec![
                    bmz_render::skin::SkinPropertyItemDef { name: "AC".to_string(), op: 922 },
                    bmz_render::skin::SkinPropertyItemDef { name: "TYPE-M".to_string(), op: 923 },
                ],
                def: "AC".to_string(),
            }],
            filepath: Vec::new(),
            offset: Vec::new(),
        };
        let mut options = BTreeMap::from([("Graph".to_string(), "999".to_string())]);
        let mut files = BTreeMap::new();

        assert!(fill_missing_skin_defaults(&defs, None, &mut options, &mut files));

        assert_eq!(options.get("Graph").map(String::as_str), Some("AC"));
    }

    #[test]
    fn fill_missing_skin_defaults_keeps_stale_file_selection_like_beatoraja() {
        let root = unique_test_dir("bmz-ui-defaults-stale");
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(root.join("notes/aaa.png"), []).unwrap();
        fs::write(root.join("notes/default.png"), []).unwrap();
        let defs = SceneSkinDefs {
            property: Vec::new(),
            filepath: vec![SkinFilepathDef {
                category: String::new(),
                name: "Notes".to_string(),
                path: "notes/*.png".to_string(),
                def: "default".to_string(),
            }],
            offset: Vec::new(),
        };
        let mut options = BTreeMap::new();
        let mut files = BTreeMap::from([("Notes".to_string(), "../old/default.png".to_string())]);

        assert!(!fill_missing_skin_defaults(&defs, Some(&root), &mut options, &mut files));

        assert_eq!(files.get("Notes").map(String::as_str), Some("../old/default.png"));
    }

    #[test]
    fn play_skin_defs_include_beatoraja_common_offsets() {
        let defs = SceneSkinDefs::from_play_document(None);

        let offsets: Vec<_> =
            defs.offset.iter().map(|offset| (offset.id, offset.name.as_str())).collect();
        assert!(offsets.contains(&(10, "All offset(%)")));
        assert!(offsets.contains(&(30, "Notes offset")));
        assert!(offsets.contains(&(32, "Judge offset")));
        assert!(offsets.contains(&(33, "Judge Detail offset")));
        assert!(offsets.contains(&(SKIN_OFFSET_BAR_LINE, "Bar Line offset")));
    }

    #[test]
    fn play_skin_defs_append_beatoraja_common_offsets_after_same_id_custom_defs() {
        let mut defs = SceneSkinDefs::default();
        defs.offset.push(SkinOffsetDef {
            category: "custom".to_string(),
            name: "Custom all".to_string(),
            id: 10,
            x: true,
            y: true,
            w: false,
            h: false,
            r: false,
            a: false,
        });

        defs.append_play_common_offsets();

        assert_eq!(defs.offset.iter().filter(|offset| offset.id == 10).count(), 2);
        assert_eq!(defs.offset.len(), 6);
        assert_eq!(
            defs.offset.iter().rfind(|offset| offset.id == 10).map(|offset| offset.name.as_str()),
            Some("All offset(%)")
        );
    }

    #[test]
    fn play_skin_defs_enable_bar_line_alpha_when_skin_def_disables_it() {
        let mut defs = SceneSkinDefs::default();
        defs.offset.push(SkinOffsetDef {
            category: "custom".to_string(),
            name: "Custom bar".to_string(),
            id: SKIN_OFFSET_BAR_LINE,
            x: false,
            y: false,
            w: false,
            h: true,
            r: false,
            a: false,
        });

        defs.append_play_common_offsets();

        let bar_line = defs
            .offset
            .iter()
            .find(|offset| offset.id == SKIN_OFFSET_BAR_LINE)
            .expect("bar line offset def");
        assert!(bar_line.a);
    }

    #[test]
    fn skin_offset_sync_prefers_name_and_updates_changed_definition_id() {
        let defs = vec![test_offset_def("Antique lane", 80)];
        let mut offsets = vec![
            SkinOffsetConfig {
                name: Some("Antique lane".to_string()),
                id: 70,
                x: 12,
                ..Default::default()
            },
            SkinOffsetConfig { id: 80, x: 99, ..Default::default() },
        ];

        assert!(sync_skin_offsets_with_defs(&defs, &mut offsets));
        assert_eq!(
            offsets,
            vec![SkinOffsetConfig {
                name: Some("Antique lane".to_string()),
                id: 80,
                x: 12,
                ..Default::default()
            }]
        );
    }

    #[test]
    fn skin_offset_sync_expands_legacy_duplicate_id_into_independent_names() {
        let defs = vec![test_offset_def("Lane A", 42), test_offset_def("Lane B", 42)];
        let mut offsets = vec![SkinOffsetConfig { id: 42, y: -8, ..Default::default() }];

        assert!(sync_skin_offsets_with_defs(&defs, &mut offsets));
        assert_eq!(offsets.len(), 2);
        assert_eq!(offsets[0].name.as_deref(), Some("Lane A"));
        assert_eq!(offsets[1].name.as_deref(), Some("Lane B"));
        assert_eq!(offsets[0].y, -8);
        assert_eq!(offsets[1].y, -8);

        let mut edited = offsets[0].clone();
        edited.y = 24;
        assert!(update_skin_offset_value(&mut offsets, &defs[0], edited));
        assert_eq!(offsets[0].y, 24);
        assert_eq!(offsets[1].y, -8);
    }

    #[test]
    fn skin_offset_sync_shares_first_named_value_across_duplicate_name_ids() {
        let defs = vec![test_offset_def("Shared", 51), test_offset_def("Shared", 52)];
        let mut offsets = vec![
            SkinOffsetConfig {
                name: Some("Shared".to_string()),
                id: 51,
                a: 120,
                ..Default::default()
            },
            SkinOffsetConfig {
                name: Some("Shared".to_string()),
                id: 52,
                a: 240,
                ..Default::default()
            },
        ];

        assert!(sync_skin_offsets_with_defs(&defs, &mut offsets));
        assert_eq!(offsets.iter().map(|offset| offset.id).collect::<Vec<_>>(), vec![51, 52]);
        assert!(offsets.iter().all(|offset| offset.a == 120));

        let mut edited = offsets[1].clone();
        edited.a = 64;
        assert!(update_skin_offset_value(&mut offsets, &defs[1], edited));
        assert!(offsets.iter().all(|offset| offset.a == 64));
    }

    #[test]
    fn reset_scene_skin_to_defaults_clears_saved_values_and_restores_factory_defaults() {
        let root = unique_test_dir("bmz-ui-reset-scene");
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::write(root.join("notes/aaa.png"), []).unwrap();
        fs::write(root.join("notes/default.png"), []).unwrap();
        let defs = SceneSkinDefs {
            property: vec![SkinPropertyDef {
                category: String::new(),
                name: "Lane".to_string(),
                item: vec![
                    bmz_render::skin::SkinPropertyItemDef { name: "Off".to_string(), op: 0 },
                    bmz_render::skin::SkinPropertyItemDef { name: "On".to_string(), op: 1 },
                ],
                def: "On".to_string(),
            }],
            filepath: vec![SkinFilepathDef {
                category: String::new(),
                name: "Notes".to_string(),
                path: "notes/*.png".to_string(),
                def: "default".to_string(),
            }],
            offset: vec![SkinOffsetDef {
                category: "test".to_string(),
                name: "Judge".to_string(),
                id: 32,
                x: true,
                y: true,
                w: false,
                h: false,
                r: false,
                a: false,
            }],
        };
        let mut options = BTreeMap::from([("Lane".to_string(), "Off".to_string())]);
        let mut files = BTreeMap::from([("Notes".to_string(), "aaa.png".to_string())]);
        let mut offsets = vec![SkinOffsetConfig { id: 32, x: 99, ..Default::default() }];

        assert!(reset_scene_skin_to_defaults(
            &defs,
            Some(&root),
            &mut options,
            &mut files,
            &mut offsets
        ));

        assert_eq!(options.get("Lane").map(String::as_str), Some("On"));
        assert_eq!(files.get("Notes").map(String::as_str), Some("notes/default.png"));
        assert!(offsets.is_empty());
    }

    #[test]
    fn reset_scene_skin_to_defaults_removes_named_defs_without_same_id_name_collision() {
        let defs =
            SceneSkinDefs { offset: vec![test_offset_def("Current", 32)], ..Default::default() };
        let mut options = BTreeMap::new();
        let mut files = BTreeMap::new();
        let mut offsets = vec![
            SkinOffsetConfig {
                name: Some("Current".to_string()),
                id: 32,
                x: 10,
                ..Default::default()
            },
            SkinOffsetConfig {
                name: Some("Other".to_string()),
                id: 32,
                x: 20,
                ..Default::default()
            },
        ];

        assert!(reset_scene_skin_to_defaults(&defs, None, &mut options, &mut files, &mut offsets));
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0].name.as_deref(), Some("Other"));
        assert_eq!(offsets[0].x, 20);
    }

    #[test]
    fn skin_slot_history_restores_options_files_and_offsets_by_path() {
        let mut skin = SkinConfig {
            play7: "data/skins/ECFN/play/play7.luaskin".to_string(),
            play7_offsets: vec![SkinOffsetConfig {
                name: Some("Judge offset".to_string()),
                id: 32,
                x: 12,
                ..Default::default()
            }],
            ..SkinConfig::default()
        };
        skin.play7_options.insert("Judge".to_string(), "On".to_string());
        skin.play7_files.insert("Notes".to_string(), "default.png".to_string());

        save_skin_slot_history(&mut skin, SkinSlot::Play7);
        skin.play7 = "data/skins/Starseeker/play/play7.luaskin".to_string();
        skin.play7_options.insert("Judge".to_string(), "Off".to_string());
        skin.play7_files.insert("Notes".to_string(), "other.png".to_string());
        skin.play7_offsets = vec![SkinOffsetConfig {
            name: Some("Judge offset".to_string()),
            id: 32,
            x: -4,
            ..Default::default()
        }];
        save_skin_slot_history(&mut skin, SkinSlot::Play7);

        skin.play7 = "data/skins/ECFN/play/play7.luaskin".to_string();
        restore_skin_slot_history(&mut skin, SkinSlot::Play7);

        assert_eq!(skin.play7_options.get("Judge").map(String::as_str), Some("On"));
        assert_eq!(skin.play7_files.get("Notes").map(String::as_str), Some("default.png"));
        assert_eq!(
            skin.play7_offsets,
            vec![SkinOffsetConfig {
                name: Some("Judge offset".to_string()),
                id: 32,
                x: 12,
                ..Default::default()
            }]
        );
    }

    #[test]
    fn skin_slot_history_isolates_same_path_by_slot() {
        let shared_path = "data/skins/shared/play.luaskin".to_string();
        let mut skin = SkinConfig {
            play7: shared_path.clone(),
            play14: shared_path,
            play7_offsets: vec![SkinOffsetConfig { id: 30, h: 7, ..Default::default() }],
            play14_offsets: vec![SkinOffsetConfig { id: 30, h: 14, ..Default::default() }],
            ..SkinConfig::default()
        };

        save_skin_slot_history(&mut skin, SkinSlot::Play7);
        save_skin_slot_history(&mut skin, SkinSlot::Play14);
        skin.play7_offsets.clear();
        skin.play14_offsets.clear();
        restore_skin_slot_history(&mut skin, SkinSlot::Play7);
        restore_skin_slot_history(&mut skin, SkinSlot::Play14);

        assert_eq!(skin.play7_offsets[0].h, 7);
        assert_eq!(skin.play14_offsets[0].h, 14);
    }

    #[test]
    fn skin_slot_history_restores_legacy_path_only_entry() {
        let path = "data/skins/legacy/play7.luaskin".to_string();
        let mut skin = SkinConfig { play7: path.clone(), ..SkinConfig::default() };
        skin.history.insert(
            path.clone(),
            SkinHistoryEntryConfig {
                offsets: vec![SkinOffsetConfig { id: 30, h: 12, ..Default::default() }],
                ..Default::default()
            },
        );

        restore_skin_slot_history(&mut skin, SkinSlot::Play7);

        assert_eq!(skin.play7_offsets[0].h, 12);
        assert!(skin.history.contains_key(&skin_slot_history_key(SkinSlot::Play7, &path)));
    }

    #[test]
    fn skin_reload_diff_scopes_play_slot_without_select_reload() {
        let before = SkinConfig::default();
        let mut after = before.clone();
        after.play7_files.insert("Notes".to_string(), "blue.png".to_string());

        let request = skin_reload_request_from_diff(&before, &after);

        assert!(request.play7);
        assert!(!request.select);
        assert!(!request.play5);
        assert!(!request.result);
        assert!(request.any_reload());
    }

    #[test]
    fn skin_reload_diff_separates_result_and_course_result_slots() {
        let before = SkinConfig::default();
        let mut after = before.clone();
        after.course_result = "data/skins/course/result.luaskin".to_string();
        after.course_result_options.insert("Layout".to_string(), "Course".to_string());

        let request = skin_reload_request_from_diff(&before, &after);

        assert!(request.course_result);
        assert!(!request.result);

        let mut after = before.clone();
        after.result_files.insert("Background".to_string(), "normal.png".to_string());

        let request = skin_reload_request_from_diff(&before, &after);

        assert!(request.result);
        assert!(!request.course_result);
    }

    #[test]
    fn skin_reload_diff_marks_each_offset_slot_for_redecode() {
        let cases: &[(&str, fn(&mut SkinConfig), fn(SkinReloadRequest) -> bool)] = &[
            (
                "select",
                |skin| skin.select_offsets.push(Default::default()),
                |request| request.select,
            ),
            (
                "decide",
                |skin| skin.decide_offsets.push(Default::default()),
                |request| request.decide,
            ),
            ("play4", |skin| skin.play4_offsets.push(Default::default()), |request| request.play4),
            ("play5", |skin| skin.play5_offsets.push(Default::default()), |request| request.play5),
            ("play6", |skin| skin.play6_offsets.push(Default::default()), |request| request.play6),
            ("play7", |skin| skin.play7_offsets.push(Default::default()), |request| request.play7),
            ("play8", |skin| skin.play8_offsets.push(Default::default()), |request| request.play8),
            ("play9", |skin| skin.play9_offsets.push(Default::default()), |request| request.play9),
            (
                "play10",
                |skin| skin.play10_offsets.push(Default::default()),
                |request| request.play10,
            ),
            (
                "play14",
                |skin| skin.play14_offsets.push(Default::default()),
                |request| request.play14,
            ),
            (
                "result",
                |skin| skin.result_offsets.push(Default::default()),
                |request| request.result,
            ),
            (
                "course_result",
                |skin| skin.course_result_offsets.push(Default::default()),
                |request| request.course_result,
            ),
        ];

        for &(slot, change, slot_requested) in cases {
            let before = SkinConfig::default();
            let mut after = before.clone();
            change(&mut after);

            let request = skin_reload_request_from_diff(&before, &after);

            assert!(request.offsets, "{slot} offset did not mark runtime offset update");
            assert!(slot_requested(request), "{slot} offset did not mark scene re-decode");
            assert!(request.any_reload(), "{slot} offset did not request reload");
            assert!(request.any());
        }
    }
}
