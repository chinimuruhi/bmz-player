use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use bmz_chart::model::{BgaAssetRef, PlayableChart};
use bmz_core::clear::{ClearType, GaugeType};
use bmz_core::input::{InputKind, ScratchDirection};
use bmz_core::lane::{KeyMode, LANE_COUNT, Lane};
use bmz_core::time::TimeUs;
use bmz_gameplay::input::backend::{
    DeviceId, DeviceInputEvent, InputBackend, InputBouncePolicy, PhysicalControl,
};
use bmz_gameplay::input::binding::LaneBinding;
use bmz_gameplay::input::system::last_input_collection_diagnostics;
use bmz_gameplay::rule::RuleMode;
use bmz_gameplay::session::compute_frame_times;
use bmz_gameplay::session::{HispeedMode, PlaySkinOffset};
use bmz_render::assets::{RgbaImageAsset, load_chart_bga_image, load_static_rgba_image};
use bmz_render::plan::{
    PLAY_BACKBMP_TEXTURE, Rect, SELECT_BANNER_TEXTURE, SELECT_STAGE_TEXTURE, TextureId,
};
use bmz_render::renderer::{RenderSurfaceStatus, Renderer, SurfaceSize};
use bmz_render::scene::{
    AppSceneSnapshot, DailyPlayerStatsSnapshot, PlayerStatsSnapshot, ResultSnapshot,
    SelectChartDistributionSecond, SelectRowSnapshot, SelectSnapshot,
};
use bmz_render::skin::{SkinImageSize, SkinTextureId};
use bmz_render::skin_offset::{SkinOffsetValue, SkinOffsetValues};
use bmz_render::snapshot::{
    CourseStageMarker, DisplayJudgeCounts, FastSlowJudgeCounts, OverlaySnapshot, RenderSnapshot,
    SkinLogicalInputSnapshot,
};
use bmz_video::VideoBgaDecoder;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{
    DeviceEvent, ElementState, MouseButton, MouseScrollDelta, StartCause, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::monitor::{MonitorHandle, VideoModeHandle};
use winit::window::{Fullscreen, Icon, Window, WindowAttributes, WindowId};

use crate::audio::{AppAudioOutput, AudioOutputDiagnostics, AudioRuntime};
use crate::bootstrap::{self, BootstrappedApp};
use crate::chart_preview::SelectChartPreview;
use crate::cli::{
    AUTOPLAY_ON_START_ARG, AppOptions, BOOT_RESULT_SAMPLE_ARG, SMOKE_EXIT_AFTER_FRAMES_ARG,
    SMOKE_EXIT_AFTER_PLAY_FRAMES_ARG, SMOKE_EXIT_AFTER_RESULT_FRAMES_ARG, SMOKE_EXIT_ON_RESULT_ARG,
    SMOKE_SCREENSHOT_ARG,
};
use crate::config::app_config::{
    AppConfig, GamepadBackendKind, GlobalInputConfig, InputBackendKind,
    InternalResolutionModeConfig, ObsConfig, PathEntry, WindowMode,
};
use crate::config::key_config::{
    KeyBindingSlot, KeyBindingTarget, apply_play_binding, clear_play_binding,
    is_scratch_down_control, is_scratch_up_control,
};
use crate::config::load::load_profile_config;
use crate::config::play::{
    TARGET_GREEN_NUMBER_MAX, TARGET_GREEN_NUMBER_MIN, input_bounce_config_from_profile,
};
use crate::config::profile_config::{
    AssistOptionConfig, BgaExpandConfig, BgaModeConfig, BottomShiftableGaugeConfig,
    DoubleOptionConfig, GaugeAutoShiftConfig, GaugeTypeConfig, HispeedDirectionConfig,
    HispeedModeConfig, HsFixConfig, InputActionConfig, JudgeAlgorithmConfig, LaneEffectConfig,
    LaneViewConfig, PlayDefaultsConfig, ProfileConfig, ProfileInputConfig, RandomOptionConfig,
    SkinOffsetConfig, TargetOptionConfig, default_hispeed_step_fhs, default_hispeed_step_nhs,
    normalize_hispeed_step, replay_slot_rule_indices,
};
use crate::config::save::{save_app_config, save_profile_config};
use crate::config::settings_registry::SettingsEntryId;
use crate::discord_presence::{DiscordPresence, DiscordPresenceConfig, DiscordPresenceHandle};
use crate::generated_preview::{fallback_preview_start_ms, generated_preview_cache_key};
use crate::i18n::{FluentArgs, Localizer};
use crate::input::shared::SharedInputBackend;
use crate::input::winit::{
    W_KEYBOARD_DEVICE_ID, key_event_to_device_input, physical_key_to_control,
};
use crate::ir::table::{
    RIAN_TABLE_MANUAL_REFRESH_COOLDOWN, RIAN_TABLE_REFRESH_INTERVAL, RianTableIdentity,
    active_source_urls as active_rian_table_source_urls, is_rian_table_source,
};
use crate::ln_policy::LnPolicySetting;
use crate::logging::LogBuffer;
use crate::practice_ui::PracticePanelContext;
use crate::random_trainer::RandomTrainerState;
use crate::screens::course_session::{ActiveCourseSession, CourseEntryResult, CourseResultSummary};
use crate::screens::key_config_edit::KeyConfigEditSession;
use crate::screens::play_finish::FinishedPlaySession;
use crate::screens::play_loop::{
    PlayEndingSkinTimers, advance_running_play_session, apply_play_arrange_to_snapshot,
    refresh_play_ending_snapshot,
};
use crate::screens::play_session::AppliedArrange;
use crate::screens::play_session::build_practice_prepared_from_preloaded;
use crate::screens::play_snapshot::{
    BgaFrameCatalog, apply_fast_slow_display_filter, bga_texture_id,
    build_render_snapshot_with_target_and_bga_frames_cached, display_bga_frame,
};
use crate::screens::play_start::{
    PlayStartOptions, PreloadedInputPlaySession, PreparedInputPlaySession, StartedInputPlaySession,
    apply_arrange_override, apply_course_constraints, apply_queued_replay,
    open_prepared_winit_play_session, play_session_options_from_start,
    prepare_play_session_for_chart_with_winit_input, prepare_winit_play_session_from_preloaded,
};
use crate::screens::practice::{
    PracticeCliOverrides, PracticePhase, PracticeSession, clamp_practice_property,
    load_practice_property, practice_chart_zero_time, save_practice_property,
};
use crate::screens::result_model::ResultSummary;
use crate::screens::select_model::{
    COURSE_ROOT_PATH, DifficultyTableText, FAVORITE_CHART_PATH, FAVORITE_ROOT_PATH,
    FAVORITE_SONG_PATH, SEARCH_PATH_PREFIX, SelectChartRow, SelectExecutableKind, SelectItem,
    TABLE_ROOT_PATH, TablePath, apply_collection_flags, course_root_item,
    difficulty_table_text_for_chart_with_active_sources, favorite_root_item, favorite_root_items,
    favorite_song_representatives_for_folder, load_select_items_for_courses,
    load_select_items_for_favorite_charts, load_select_items_for_favorite_song,
    load_select_items_for_favorite_songs, load_select_items_for_search_for_rule_mode_with_filters,
    load_select_items_in_folder_for_rule_mode_with_filters,
    load_select_items_in_table_level_for_rule_mode, parse_favorite_song_detail_path,
    parse_same_folder_path, parse_search_query, parse_table_path, random_select_item_from_items,
    root_folder_items, same_folder_path, search_history_folder_items_for_locale,
    song_scan_path_from_context, table_folder_items_for_active_sources, table_level_folder_items,
    table_source_url_from_context,
};
use crate::screens::settings_edit::{SettingsBindings, SettingsEditSession, adjust_settings_draft};
use crate::screens::settings_model::{
    in_settings_stack, load_settings_items_for_locale, settings_breadcrumb_for_locale,
    settings_root_item_for_locale,
};
use crate::select_options::{ArrangeOption, DoubleOption, HsFixOption, SessionMode, TargetOption};
use crate::skin_loader::{
    DecodedSkin, PreparedSource, SharedSkinDocumentCache, SharedSkinFontCache,
    SharedSkinGpuTextureCache, SharedSkinSourceAssetCache, SkinFontCacheKey, SkinKind,
    UploadedSkin, decode_beatoraja_skin_with_options_and_runtime_state,
    decode_beatoraja_skin_with_options_and_runtime_state_and_caches,
    default_play_skin_document_path_from_paths, default_skin_document_path_from_paths,
    enabled_options_from_selections, install_decoded_font, install_decoded_skin,
    is_decodable_skin_path, is_json_skin_path, is_lr2_skin_path, is_lua_skin_path,
    load_default_skin_into_renderer_from_paths, play_skin_selection_for_session,
    set_decoded_skin_context, upload_decoded_skin_with_texture_cache,
};
use crate::song_download::{
    ChartDownloadRequest, ChartDownloadResult, MissingChartAction, choose_missing_chart_action,
    download_chart, open_browser_urls,
};
use crate::songs_cmd::scan_songs_with_progress;
use crate::storage::collection_db::FavoriteHints;
use crate::storage::common::hash_to_hex;
use crate::storage::difficulty_table_db::DifficultyTableRecord;
use crate::storage::library_db::{ChartDistributionSecond, ChartListItem, LibraryDatabase};
use crate::storage::migration::{migrate_library_db, migrate_network_db};
use crate::storage::replay::load_replay_for_chart_policy_and_double_option;
use crate::storage::scan::{ScanProgress, ScanReport};
use crate::storage::score_db::{DailyPlayerStats, PlayerStats, ScoreDatabase};
use crate::storage::score_import::{ScoreImportRequest, import_scores};
use crate::table_cmd::{TableFetchOutcome, TableFetchReport};
use crate::ui::{
    DebugInfo, EguiLayer, EguiRunContext, SceneSkinDefs, SkinCandidate, SkinCandidateOrigin,
    SkinCatalog, SkinConfigMeta, SkinReloadRequest, SongScanRequest, UpdateDialog,
    UpdateDialogAction,
};
use crate::update::{DownloadedUpdate, UpdateAssetKind, UpdateCandidate};
use crate::window_config::select_monitor;
use bmz_render::skin::{
    DestinationListEntry, SKIN_EVENT_DAILY_STATISTICS_RESET, SKIN_EVENT_IR_SCOPE_GLOBAL,
    SKIN_EVENT_IR_SCOPE_RIVAL, SKIN_EVENT_IR_SCOPE_TOGGLE, SKIN_EVENT_RESULT_PANEL_GRAPH,
    SKIN_EVENT_RESULT_PANEL_IR, SKIN_OPTION_BMZ_DOUBLE_PLAY, SKIN_OPTION_BMZ_KEY_MODE_BASE,
    SKIN_OPTION_BMZ_KEY_MODE_COUNT, SKIN_OPTION_BMZ_NO_SCRATCH, SKIN_OPTION_BMZ_SINGLE_PLAY,
    SKIN_REF_BMZ_ACTIVE_LANE_COUNT, SKIN_REF_BMZ_KEY_MODE, SkinAnimationDef, SkinClickHit,
    SkinClickTarget, SkinContext, SkinDestinationDef, SkinDocument, SkinDocumentRenderExt,
    SkinDocumentTexture, SkinDstEntry, SkinManifest, SkinSliderHit,
};

mod background_jobs;
mod bga_runtime;
mod course_flow;
mod frame_flow;
mod frame_runtime;
mod input_runtime;
mod integration_support;
mod integrations;
mod play_control;
mod play_flow;
mod play_loop_flow;
mod play_support;
mod result_flow;
mod result_runtime;
mod result_support;
mod runtime_state;
mod scene_input;
mod select_assets;
#[path = "app/select_flow/controls.rs"]
mod select_flow_controls;
#[path = "app/select_flow/input.rs"]
mod select_flow_input;
#[path = "app/select_flow/navigation.rs"]
mod select_flow_navigation;
#[path = "app/select_flow/preview.rs"]
mod select_flow_preview;
#[path = "app/select_flow/skin_events.rs"]
mod select_flow_skin_events;
#[path = "app/select_flow/snapshot.rs"]
mod select_flow_snapshot;
mod select_folder_summary;
mod select_key_bindings;
mod select_search;
mod select_support;
mod skin_flow;
mod skin_pipeline;
mod skin_support;
mod table_fetch_runtime;

use runtime_state::*;

use select_key_bindings::{
    SelectKeyBindings, play_analog_lane_cover_delta, select_analog_scroll_delta,
    take_analog_scroll_steps, update_analog_scroll_buffer,
};

#[cfg(test)]
use crate::config::profile_config::{LaneConfig, SelectInputModeConfig};

use bga_runtime::{
    BgaImageLoadStatus, BgaPreloadRuntime, PendingBgaImageResult, RESOURCE_LOAD_PROGRESS_SCALE,
    combined_resource_load_progress, load_worker as chart_bga_texture_load_worker,
    preload_worker as chart_bga_texture_preload_worker, resource_load_progress_units,
};
use frame_runtime::{
    FrameProfileKind, FrameRuntime, FrameSchedule, PlayLoopFrameTimings, SceneFrameProfileSample,
    SkinVideoFrameProfile,
};
use input_runtime::{
    AppInputRuntime, ControlInputEvent, should_route_gamepad_event_while_discarding,
};
use integration_support::*;
use play_control::{
    GreenNumberChange, HispeedChange, LaneCoverChange, PlayAnalogOptionMode, PlayLaneAction,
    PlayOptionControl, keyboard_lane_action, lane_action_from_option,
};
use play_support::*;
use result_runtime::{
    course_result_skin_snapshot, course_result_summary_for_skin, debug_boot_finished_play_session,
    mark_course_replay_slot_saved, result_main_bpm, result_max_bpm, result_min_bpm,
};
use result_support::*;
use scene_input::{
    DecideAction, ResultAction, SelectAction, SelectMove, decide_action as scene_decide_action,
    result_action as scene_result_action, select_action as scene_select_action,
};
use select_assets::{
    PreparedSelectPreview, SelectAssetRuntime, SelectMetaImageSlot, SelectPreviewFade,
    SelectPreviewSyncAction, select_preview_fade_factor,
};
use select_folder_summary::SelectFolderSummaryRuntime;
use select_search::{SearchInputAction, SelectSearchRuntime};
use select_support::*;
use skin_pipeline::{SkinPipelineRuntime, SkinReloadGenerations};
use skin_support::*;
use table_fetch_runtime::{
    RianTableFetchWorkerResult, TableFetchProgress, TableFetchRuntime, TableFetchWorkerEvent,
    startup_difficulty_table_fetch_urls_for_boot,
};

#[cfg(test)]
use crate::input::winit::physical_key_to_device_input;
#[cfg(test)]
use crate::screens::result_model::ResultFastSlowJudgeCounts;
#[cfg(test)]
use bmz_audio::sample::DecodedSample;
#[cfg(test)]
use result_runtime::debug_boot_result_summary;
#[cfg(test)]
use select_assets::{SELECT_PREVIEW_FADE_DURATION, SelectPreviewLoadQueue, prepare_select_preview};
#[cfg(test)]
use skin_pipeline::MAX_PENDING_SKIN_UPLOADS;

const SAMPLE_PLAYABLE_TITLE: &str = "BMZ Sample Playable";

#[derive(Debug, Clone, Copy)]
enum AppUserEvent {
    SkinUploadReady { sent_at: Instant },
    TableFetchReady,
}

pub async fn run() -> Result<()> {
    run_with_options(AppOptions::default()).await
}

pub async fn run_with_options(options: AppOptions) -> Result<()> {
    run_with_options_and_log_buffer(options, LogBuffer::default()).await
}

pub async fn run_with_options_and_log_buffer(
    options: AppOptions,
    log_buffer: LogBuffer,
) -> Result<()> {
    let boot = bootstrap::bootstrap()?;

    let event_loop = EventLoop::<AppUserEvent>::with_user_event()
        .build()
        .context("failed to create event loop")?;
    // 描画間隔は `FramePacer` の deadline を `WaitUntil` へ渡して制御する。
    // event loop thread 自体を sleep させず、フレーム待機中も入力イベントを処理する。
    event_loop.set_control_flow(ControlFlow::Wait);
    let event_proxy = event_loop.create_proxy();

    // Ctrl-C(SIGINT)で event loop を正常終了させ、cpal/ASIO ストリームの Drop を
    // 走らせる。捕捉しないと既定ハンドラがプロセスを即殺し、ASIO の停止処理が走らず
    // ドライバがノイズを流し続ける。
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    {
        let shutdown_requested = Arc::clone(&shutdown_requested);
        if let Err(error) =
            ctrlc::set_handler(move || shutdown_requested.store(true, Ordering::SeqCst))
        {
            tracing::warn!(%error, "failed to install Ctrl-C handler");
        }
    }

    spawn_ir_sync_worker(&boot);

    let mut app = Box::new(WinitApp::new(
        boot,
        options,
        None,
        None,
        shutdown_requested,
        event_proxy,
        log_buffer,
    )?);
    tracing::info!("starting winit event loop");
    event_loop.run_app(app.as_mut()).context("winit event loop failed")
}

/// IR スコアジョブをバックグラウンドで定期送信する。
///
/// メインスレッドの DB connection とは別 connection を開く (DB は WAL)。
/// IR が未設定なら何もしない。
fn spawn_ir_sync_worker(boot: &bootstrap::BootstrappedApp) {
    let ir_config = boot.profile_config.ir.clone();
    if !ir_config.providers.iter().any(|provider| provider.enabled && !provider.base_url.is_empty())
    {
        return;
    }
    let profile_root = boot.profile_paths.root_dir.clone();
    let logs_dir = boot.app_paths.logs_dir.clone();
    let score_db_path = boot.profile_paths.score_db.clone();
    let network_db_path = boot.profile_paths.network_db.clone();
    tokio::spawn(async move {
        loop {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            if let Err(error) = migrate_network_db(&network_db_path) {
                tracing::warn!(%error, "failed to migrate network db for IR sync");
                tokio::time::sleep(std::time::Duration::from_secs(
                    crate::ir::sync::IR_SYNC_LOOP_INTERVAL_SECS,
                ))
                .await;
                continue;
            }
            match crate::storage::network_db::NetworkDatabase::open(&network_db_path) {
                Ok(mut network_db) => {
                    match crate::ir::sync::sync_pending_ir_jobs(
                        &mut network_db,
                        &score_db_path,
                        &profile_root,
                        &logs_dir,
                        &ir_config,
                        now,
                        crate::ir::sync::IR_SYNC_BATCH_LIMIT,
                        false,
                        crate::ir::sync::IrSyncThrottle::rate_limited(),
                    )
                    .await
                    {
                        Ok(report) if report.submitted > 0 || report.failed > 0 => {
                            tracing::info!(
                                submitted = report.submitted,
                                failed = report.failed,
                                "IR score sync finished"
                            );
                        }
                        Ok(_) => {}
                        Err(error) => tracing::warn!(%error, "IR score sync failed"),
                    }
                }
                Err(error) => tracing::warn!(%error, "failed to open network db for IR sync"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(
                crate::ir::sync::IR_SYNC_LOOP_INTERVAL_SECS,
            ))
            .await;
        }
    });
}

#[derive(Debug, Clone)]
enum UpdatePrompt {
    Available(UpdateCandidate),
    Downloading(UpdateCandidate),
    Error { message: String, candidate: Option<UpdateCandidate> },
    UpToDate,
}

impl UpdatePrompt {
    fn candidate(&self) -> Option<&UpdateCandidate> {
        match self {
            Self::Available(candidate) | Self::Downloading(candidate) => Some(candidate),
            Self::Error { candidate, .. } => candidate.as_ref(),
            Self::UpToDate => None,
        }
    }

    fn candidate_version(&self) -> Option<&str> {
        self.candidate().map(|candidate| candidate.version.as_str())
    }

    fn as_dialog(&self) -> UpdateDialog<'_> {
        match self {
            Self::Available(candidate) => UpdateDialog::Available(candidate),
            Self::Downloading(candidate) => UpdateDialog::Downloading(candidate),
            Self::Error { message, candidate } => {
                UpdateDialog::Error { message, candidate: candidate.as_ref() }
            }
            Self::UpToDate => UpdateDialog::UpToDate,
        }
    }
}

/// 左上へ短時間表示するトースト。
struct LeftOverlayToast {
    message: String,
    shown_at: Instant,
}

const LEFT_OVERLAY_TOAST_DURATION: Duration = Duration::from_secs(2);

struct WinitApp {
    boot: BootstrappedApp,
    window: Option<Arc<Window>>,
    first_frame_startup_completed: bool,
    /// Ctrl-C(SIGINT)受信フラグ。セットされたら `about_to_wait` で event loop を
    /// 正常終了させ、cpal/ASIO ストリームの Drop(停止・後処理)を確実に走らせる。
    shutdown_requested: Arc<AtomicBool>,
    renderer: Box<Renderer>,
    /// device共通の押下集合とkeyboard bounce状態。
    input: AppInputRuntime,
    gamepad: Option<Box<crate::input::gamepad::GamepadBackend>>,
    /// worker 完了時に main thread の redraw を起こすための winit user event proxy。
    event_proxy: EventLoopProxy<AppUserEvent>,
    /// frame pacing、確定FPS、scene別profile集計をまとめた描画runtime。
    frame: FrameRuntime,
    deferred_boot: Option<DeferredBoot>,
    select: SelectRuntimeState,
    play: PlayRuntimeState,
    result: ResultRuntimeState,
    jobs: AppJobs,
    integrations: IntegrationRuntimeState,
    smoke: SmokeRuntime,
    skin: SkinRuntimeState,
    audio: AppAudioRuntimeState,
    ui: UiRuntimeState,
}

struct ActiveSkinVideoSource {
    texture: SkinTextureId,
    path: PathBuf,
    decoder: Option<VideoBgaDecoder>,
    last_pts: Option<i64>,
    loop_start_us: i64,
    /// スキン config の option による静的な有効判定。
    active: bool,
    /// このソースを参照する各 destination の op 条件。実行時 state に対して
    /// 評価し、現在のシーン状態 (例: リザルトのランク) で実際に表示されるソース
    /// だけをデコードするために使う。空なら参照されておらず常時可視扱い。
    gating_op_sets: Vec<Vec<i32>>,
    /// `gating_op_sets` 評価に必要な document の有効 option 一覧。
    enabled_options: Vec<i32>,
    /// リザルト draw state 構築に使う document の ranktime。
    result_ranktime_ms: i32,
    failed: bool,
}

#[derive(Debug, Clone, Copy)]
struct PendingSkinRenderProbe {
    kind: SkinKind,
    generation: u64,
    applied_at: Instant,
}

type PlaySkinSignature = (
    KeyMode,
    String,
    BTreeMap<String, String>,
    BTreeMap<String, String>,
    bmz_skin::LuaLoadRuntimeState,
);
type ResultSkinSignature = (
    ResultSkinSlot,
    String,
    BTreeMap<String, String>,
    BTreeMap<String, String>,
    bmz_skin::LuaLoadRuntimeState,
);

fn skin_offset_values_from_config(offsets: &[SkinOffsetConfig]) -> SkinOffsetValues {
    let mut values = SkinOffsetValues::default();
    for offset in offsets {
        values.set(
            offset.id,
            SkinOffsetValue {
                x: offset.x,
                y: offset.y,
                w: offset.w,
                h: offset.h,
                r: offset.r,
                a: offset.a,
            },
        );
    }
    values
}

fn apply_skin_offsets_to_lua_runtime_state(
    runtime_state: &mut bmz_skin::LuaLoadRuntimeState,
    offsets: &[SkinOffsetConfig],
) {
    for offset in offsets {
        let value = bmz_skin::LuaSkinOffsetValue {
            x: offset.x,
            y: offset.y,
            w: offset.w,
            h: offset.h,
            r: offset.r,
            a: offset.a,
        };
        if let Some(name) = offset.name.as_deref().filter(|name| !name.is_empty()) {
            runtime_state.offset_values.entry(name.to_string()).or_insert(value);
        }
        runtime_state.offset_id_values.insert(offset.id, value);
    }
}

fn lua_runtime_state_with_skin_offsets(
    mut runtime_state: bmz_skin::LuaLoadRuntimeState,
    offsets: &[SkinOffsetConfig],
) -> bmz_skin::LuaLoadRuntimeState {
    apply_skin_offsets_to_lua_runtime_state(&mut runtime_state, offsets);
    runtime_state
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultSkinSlot {
    Normal,
    Course,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultSkinClickAction {
    SetPanel(i32),
    SelectIrScope(crate::screens::result_ir::ResultRankingTab),
    ToggleIrScope,
    ToggleFavoriteChart,
    SaveReplay(u8),
    ResetDailyStatistics,
}

#[derive(Debug, Clone)]
struct TableBreadcrumb {
    name: String,
    symbol: String,
}

fn table_breadcrumb_from_record(table: &DifficultyTableRecord) -> TableBreadcrumb {
    TableBreadcrumb { name: table.name.clone(), symbol: table.symbol.clone() }
}

struct DecideTransition {
    chart_id: i64,
    options: PlayStartOptions,
    started_at: Instant,
    fadeout_started_at: Option<Instant>,
    cancel: bool,
    snapshot: RenderSnapshot,
    title_override: Option<DecideTitleOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecideTitleOverride {
    title: String,
    subtitle: String,
}

impl DecideTransition {
    fn snapshot_for_render(&self) -> RenderSnapshot {
        let mut snapshot = self.snapshot.clone();
        if let Some(title_override) = &self.title_override {
            snapshot.title.clone_from(&title_override.title);
            snapshot.subtitle.clone_from(&title_override.subtitle);
        }
        snapshot
    }
}

struct PendingPlayStart {
    chart_id: i64,
    options: PlayStartOptions,
    lane: PendingPlayLaneState,
    lane_actions: Vec<PlayLaneAction>,
    visual_input: PendingPlayVisualInput,
}

impl PendingPlayStart {
    fn from_snapshot(
        chart_id: i64,
        options: PlayStartOptions,
        snapshot: &RenderSnapshot,
        profile: &ProfileConfig,
        key_mode: KeyMode,
        gamepad_slots: crate::input::gamepad::GamepadSlotMap,
    ) -> Self {
        let binding = crate::config::play::lane_binding_for_chart_with_slots(
            &profile.input,
            key_mode,
            gamepad_slots,
        );
        let hs_fix = options.hs_fix;
        Self {
            chart_id,
            options,
            lane: PendingPlayLaneState::from_snapshot(
                snapshot,
                profile.lane.target_green_number,
                hs_fix,
                profile.lane.hispeed_auto_adjust,
            ),
            lane_actions: Vec::new(),
            visual_input: PendingPlayVisualInput::new(key_mode, binding, snapshot.autoplay),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PendingPlayLaneState {
    hispeed: f32,
    hispeed_mode: HispeedMode,
    target_green_number: u32,
    lane_cover: f32,
    lift: f32,
    lane_cover_visible: bool,
    lane_cover_changing: bool,
    hsfix_base_bpm: f32,
    hispeed_auto_adjust: bool,
}

impl PendingPlayLaneState {
    fn from_snapshot(
        snapshot: &RenderSnapshot,
        target_green_number: u32,
        hs_fix: HsFixOption,
        hispeed_auto_adjust: bool,
    ) -> Self {
        Self {
            hispeed: snapshot.hispeed,
            hispeed_mode: if snapshot.hispeed_mode_index == 0 {
                HispeedMode::Normal
            } else {
                HispeedMode::Floating
            },
            target_green_number: target_green_number.max(1),
            lane_cover: snapshot.lane_cover,
            lift: snapshot.lift,
            lane_cover_visible: true,
            lane_cover_changing: snapshot.lane_cover_changing,
            hsfix_base_bpm: match hs_fix {
                HsFixOption::Off | HsFixOption::StartBpm => snapshot.now_bpm,
                HsFixOption::MaxBpm => snapshot.max_bpm,
                HsFixOption::MainBpm => snapshot.main_bpm,
                HsFixOption::MinBpm => snapshot.min_bpm,
            }
            .max(1.0),
            hispeed_auto_adjust,
        }
    }

    fn active_lane_cover(self) -> f32 {
        if self.lane_cover_visible {
            crate::config::play::clamp_lane_cover_for_lift(self.lane_cover, self.lift)
        } else {
            0.0
        }
    }

    fn refresh_floating_hispeed(&mut self, now_bpm: f32, speed_locked: bool) {
        if self.hispeed_mode != HispeedMode::Floating || speed_locked {
            return;
        }
        let visible =
            crate::config::play::visible_lane_fraction(self.active_lane_cover(), self.lift);
        self.hispeed = crate::screens::play_snapshot::hispeed_for_green_number_values(
            self.target_green_number.max(1) as f32,
            visible,
            now_bpm.max(1.0) as f64,
            1.0,
        )
        .clamp(0.5, 10.0);
    }

    fn refresh_cover_hispeed(&mut self, now_bpm: f32, speed_locked: bool) {
        let target_bpm = if self.hispeed_auto_adjust { now_bpm } else { self.hsfix_base_bpm };
        self.refresh_floating_hispeed(target_bpm, speed_locked);
    }

    fn current_green_number(self, now_bpm: f32) -> u32 {
        let duration = crate::screens::play_snapshot::display_duration_ms_for_bpm_hispeed(
            now_bpm,
            self.hispeed,
            self.active_lane_cover(),
            self.lift,
            1.0,
        );
        green_number_from_display_duration(duration)
    }

    fn apply_to_snapshot(self, snapshot: &mut RenderSnapshot) {
        snapshot.hispeed = self.hispeed;
        snapshot.hispeed_mode_index = match self.hispeed_mode {
            HispeedMode::Normal => 0,
            HispeedMode::Floating => 1,
        };
        snapshot.target_green_number = self.target_green_number;
        snapshot.lift = self.lift;
        snapshot.lane_cover = self.active_lane_cover();
        snapshot.lane_cover_changing = self.lane_cover_changing;
        snapshot.note_display_duration_ms =
            crate::screens::play_snapshot::display_duration_ms_for_bpm_hispeed(
                snapshot.now_bpm,
                self.hispeed,
                snapshot.lane_cover,
                self.lift,
                1.0,
            )
            .round()
            .clamp(0.0, i32::MAX as f32) as i32;
    }
}

#[derive(Debug, Clone)]
struct PlayOptionInput {
    key_mode: KeyMode,
    binding: LaneBinding,
    scratch_binding: LaneBinding,
    action_bindings: Vec<PlayActionBinding>,
}

#[derive(Debug, Clone)]
struct PlayActionBinding {
    device: Option<DeviceId>,
    control: PhysicalControl,
    action: InputActionConfig,
}

impl PlayOptionInput {
    fn new(
        key_mode: KeyMode,
        binding: LaneBinding,
        profile_input: &ProfileInputConfig,
        gamepad_slots: crate::input::gamepad::GamepadSlotMap,
    ) -> Self {
        let scratch_binding =
            crate::config::play_input::lane_binding_for_play_option_scratch_with_slots(
                profile_input,
                key_mode,
                gamepad_slots,
            )
            .unwrap_or_else(|_| LaneBinding { entries: Vec::new() });
        let mut action_bindings: Vec<_> = profile_input
            .ui
            .bindings
            .iter()
            .filter_map(|entry| {
                let action = entry.action?;
                let (device, control) = match entry.device.trim().to_ascii_lowercase().as_str() {
                    "keyboard" => (
                        Some(W_KEYBOARD_DEVICE_ID),
                        PhysicalControl::KeyboardKey(entry.control.clone()),
                    ),
                    "hid" => (None, PhysicalControl::HidButton(entry.control.parse::<u32>().ok()?)),
                    "gamepad" => (None, PhysicalControl::GamepadButton(entry.control.clone())),
                    device => {
                        let player = crate::config::play_input::gamepad_player_index(device)?;
                        (
                            gamepad_slots.device_id_for_player(player),
                            PhysicalControl::GamepadButton(entry.control.clone()),
                        )
                    }
                };
                Some(PlayActionBinding { device, control, action })
            })
            .collect();
        if let Some(legacy_start) = profile_input.start_key.as_ref()
            && !action_bindings.iter().any(|entry| {
                entry.device == Some(W_KEYBOARD_DEVICE_ID)
                    && entry.control == PhysicalControl::KeyboardKey(legacy_start.clone())
                    && entry.action == InputActionConfig::E1
            })
        {
            action_bindings.push(PlayActionBinding {
                device: Some(W_KEYBOARD_DEVICE_ID),
                control: PhysicalControl::KeyboardKey(legacy_start.clone()),
                action: InputActionConfig::E1,
            });
        }
        if !action_bindings.iter().any(|entry| entry.action == InputActionConfig::E1) {
            action_bindings.push(PlayActionBinding {
                device: Some(W_KEYBOARD_DEVICE_ID),
                control: PhysicalControl::KeyboardKey("Q".to_string()),
                action: InputActionConfig::E1,
            });
        }
        Self { key_mode, binding, scratch_binding, action_bindings }
    }

    fn resolve_entry(
        &self,
        device: DeviceId,
        control: &PhysicalControl,
    ) -> Option<bmz_gameplay::input::binding::BindingResolution> {
        self.binding
            .resolve_entry(device, control)
            .or_else(|| self.scratch_binding.resolve_entry(device, control))
    }

    fn resolves_lane(&self, device: DeviceId, control: &PhysicalControl) -> bool {
        self.resolve_entry(device, control).is_some()
    }

    fn is_action(
        &self,
        device: DeviceId,
        control: &PhysicalControl,
        action: InputActionConfig,
    ) -> bool {
        let has_device_specific_binding = self
            .action_bindings
            .iter()
            .any(|entry| entry.device == Some(device) && entry.control == *control);
        self.action_bindings.iter().any(|entry| {
            entry.control == *control
                && entry.action == action
                && if has_device_specific_binding {
                    entry.device == Some(device)
                } else {
                    entry.device.is_none()
                }
        })
    }
}

#[derive(Debug, Clone)]
struct PendingPlayVisualInput {
    key_mode: KeyMode,
    binding: LaneBinding,
    suppress_human_input: bool,
    lane_keyon_started_at: [Option<TimeUs>; LANE_COUNT],
    lane_keyoff_started_at: [Option<TimeUs>; LANE_COUNT],
    lane_scratch_direction: [Option<ScratchDirection>; LANE_COUNT],
    lane_scratch_angle_delta_ms: [i64; LANE_COUNT],
    scratch_angle_last_render_at: Option<TimeUs>,
}

impl PendingPlayVisualInput {
    fn new(key_mode: KeyMode, binding: LaneBinding, suppress_human_input: bool) -> Self {
        Self {
            key_mode,
            binding,
            suppress_human_input,
            lane_keyon_started_at: [None; LANE_COUNT],
            lane_keyoff_started_at: [None; LANE_COUNT],
            lane_scratch_direction: [None; LANE_COUNT],
            lane_scratch_angle_delta_ms: [0; LANE_COUNT],
            scratch_angle_last_render_at: None,
        }
    }

    fn apply_event(&mut self, event: &DeviceInputEvent, visual_now: TimeUs) {
        if self.suppress_human_input {
            return;
        }
        let Some(binding) = self.binding.resolve_entry(event.device, &event.control) else {
            return;
        };
        let lane = binding.lane.index();
        match event.kind {
            InputKind::Press => {
                self.lane_keyon_started_at[lane] = Some(visual_now);
                self.lane_keyoff_started_at[lane] = None;
                self.lane_scratch_direction[lane] = binding.scratch_direction;
            }
            InputKind::Release => {
                if self.lane_keyon_started_at[lane].is_some() {
                    self.lane_keyon_started_at[lane] = None;
                    self.lane_keyoff_started_at[lane] = Some(visual_now);
                }
                self.lane_scratch_direction[lane] = None;
            }
        }
    }

    fn advance(&mut self, visual_now: TimeUs) {
        let Some(last_render_at) = self.scratch_angle_last_render_at.replace(visual_now) else {
            return;
        };
        let delta_ms = ((visual_now.0 - last_render_at.0) / 1_000).max(0);
        if delta_ms == 0 {
            return;
        }
        for lane in [Lane::Scratch, Lane::Scratch2] {
            let lane_index = lane.index();
            if self.lane_keyon_started_at[lane_index].is_none() {
                continue;
            }
            let sign =
                match self.lane_scratch_direction[lane_index].unwrap_or(ScratchDirection::Down) {
                    ScratchDirection::Up => 1,
                    ScratchDirection::Down => -1,
                };
            self.lane_scratch_angle_delta_ms[lane_index] =
                (self.lane_scratch_angle_delta_ms[lane_index] + sign * delta_ms.saturating_mul(2))
                    .rem_euclid(2_160);
        }
    }

    fn apply_to_session(self, session: &mut bmz_gameplay::session::GameSession) {
        session.lane_keyon_started_at = self.lane_keyon_started_at;
        session.lane_keyoff_started_at = self.lane_keyoff_started_at;
        session.lane_scratch_direction = self.lane_scratch_direction;
        session.lane_scratch_angle_delta_ms = self.lane_scratch_angle_delta_ms;
        session.scratch_angle_last_render_at = self.scratch_angle_last_render_at;
    }
}

struct PendingPlayPreload {
    generation: u64,
    chart_id: i64,
    input: SharedInputBackend,
    audio_progress: Arc<AtomicU32>,
    applied_arrange: Arc<OnceLock<AppliedArrange>>,
    rx: Receiver<PlayPreloadResult>,
}

struct PlayPreloadResult {
    generation: u64,
    chart_id: i64,
    result: std::result::Result<PreloadedInputPlaySession, String>,
}

/// Media kept across same-song retry (beatoraja `BMSResource` style).
/// Cleared when leaving result back to select, or when starting an unrelated chart.
struct PlayMediaCache {
    chart_id: i64,
    /// Present for SameArrange reuse of the exact chart Arc.
    chart: Option<std::sync::Arc<PlayableChart>>,
    chart_normalization_gain: f32,
    applied_arrange: Option<crate::screens::play_session::AppliedArrange>,
    score_key: Option<crate::storage::score_db::ScoreKey>,
    bga_frames: BgaFrameCatalog,
    bga_assets: Vec<BgaAssetRef>,
    video_bga_decoders: crate::video_bga::VideoBgaDecoderMap,
}

struct PendingSongScan {
    finished: Receiver<Result<ScanReport>>,
    progress: Arc<AtomicU64>,
}

struct PracticeChartDefaults {
    property: crate::screens::practice::PracticeProperty,
    title: String,
    sha256: [u8; 32],
}

struct PlayEndingTransition {
    started_at: Instant,
    fadeout_started_at: Option<Instant>,
    finished: Option<FinishedPlaySession>,
    failed: bool,
    full_combo_elapsed_at_finish_ms: Option<i32>,
}

fn failed_play_ending(started_at: Instant) -> PlayEndingTransition {
    PlayEndingTransition {
        started_at,
        fadeout_started_at: None,
        finished: None,
        failed: true,
        full_combo_elapsed_at_finish_ms: None,
    }
}

/// リザルト画面終了フェードアウトの進行状態。
/// 通常はフェードアウト時間が経過したら、スキップ要求時は実アニメーションの
/// 最終フレームを1フレーム保持してから `action` を実行して画面を切り替える。
struct ResultExit {
    started_at: Instant,
    action: ResultExitAction,
    skip_requested: bool,
    skip_final_frame_held: bool,
}

/// F10 で開始したフォルダ内 Autoplay の進行状態。
#[derive(Debug, Clone, PartialEq, Eq)]
struct AutoplayFolderSession {
    chart_ids: Vec<i64>,
    next_index: usize,
}

/// リザルト画面を抜けたあとに実行する遷移。
#[derive(Debug, Clone, PartialEq, Eq)]
enum ResultExitAction {
    /// 選曲画面へ戻る。
    Leave,
    /// 直前と同じ譜面を、指定した arrange でもう一度プレイする。
    Retry(ResultRetryMode),
    /// レーンキー (Key1-4 / Key5 / Key7) 押下で開始した遷移。
    /// フェードアウト終了時の Key5/Key7 押下状態で、retry(arrange) か
    /// 選曲へ戻るかを決める (beatoraja の REPLAY_SAME / REPLAY_DIFFERENT / OK 相当)。
    HeldLanes,
    /// コース（段位）リザルトから、コース全体を同配置で再プレイする。
    RetryCourseSameArrange,
    /// コース（段位）リザルトから、Key5/Key7 の押下状態で arrange を決める。
    HeldCourseLanes,
    /// コース曲間の中間リザルトを閉じて、コースの次の曲を開始する。
    /// リトライは発生させず次譜面へ進むだけ (beatoraja の MusicResult コース分岐相当)。
    AdvanceCourse,
    /// コース途中落ちの単曲リザルトを閉じて、コース最終リザルトへ進む。
    FinishCourse,
    /// フォルダ内 Autoplay の次の譜面を開始する。
    AdvanceAutoplayFolder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultRetryMode {
    SameArrange,
    DifferentArrange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetryPreloadKind {
    CachedChartWithFreshAudio,
    ReimportedChartWithFreshAudio,
}

const SELECT_EXIT_HOLD_DURATION: Duration = Duration::from_millis(1_200);
const FALLBACK_RESULT_SCENE_DURATION: Duration = Duration::from_secs(10);
/// プレイ中の Start ボタンを「2回連続押し」と判定する間隔上限。
const PLAY_START_DOUBLE_PRESS_WINDOW: Duration = Duration::from_millis(400);
/// リザルト退出時にプレイ残響(draining_audio)を絞り切るまでの上限時間。
/// スキンの終了アニメーション (`fadeout`) が長くても (例: Starseeker は 3000ms)、
/// 音声はこの時間内でフェードし切る。スキンの fadeout がこれより短ければそちらを優先。
const RESULT_EXIT_AUDIO_FADE: Duration = Duration::from_millis(1_500);
const AUDIO_DIAGNOSTICS_LOG_INTERVAL: Duration = Duration::from_secs(1);
/// beatoraja PreviewMusicProcessor fades select BGM over 10 * 15ms steps.
/// beatoraja MusicSelector waits this long after a song-bar change before preview starts.
const SELECT_PREVIEW_START_DELAY: Duration = Duration::from_millis(400);
/// レーンカバー / LIFT を上下キーで動かす際のステップ幅。
const LANE_COVER_STEP: f32 = 0.001;
const LANE_COVER_REPEAT_STEP: f32 = 0.01;
/// アナログスクラッチの tick が途切れたとみなし、端数バッファを捨てるまでの時間 (ms)。
/// beatoraja の `getAnalogDiffAndReset(i, 200)` の tolerance に相当。
const SELECT_ANALOG_SCROLL_TOLERANCE_MS: u64 = 200;
const SKIN_RELOAD_REDRAW_PROFILE_THRESHOLD: Duration = Duration::from_millis(8);
/// GPU texture の登録を伴う完了結果は、通常描画を止めないよう少量ずつ処理する。
/// BGA worker 側も同じ数で backpressure を掛け、先行した `Queue::write_texture` が
/// GPU queue を埋め続けないようにする。
const MAX_PENDING_BGA_TEXTURE_UPLOADS: usize = 2;
const MAX_BGA_TEXTURE_RESULTS_PER_REDRAW: usize = 2;
const MAX_SKIN_UPLOADS_PER_REDRAW: usize = 1;

fn bounded_gpu_upload_channel<T>(capacity: usize) -> (mpsc::SyncSender<T>, Receiver<T>) {
    debug_assert!(capacity > 0);
    mpsc::sync_channel(capacity)
}

struct PendingSkinResult {
    generation: u64,
    path: PathBuf,
    kind: SkinKind,
    queued_at: Instant,
    decode_started_at: Instant,
    decode_finished_at: Instant,
    result: Result<DecodedSkin>,
}

/// upload worker が GPU アップロードまで終えた結果を main へ返すメッセージ。
/// `UploadedSkin` 内の `PreparedTexture` は `Send` なのでスレッド間で渡せる。
/// main は受信後、テクスチャを差し込んで `SkinContext` を組むだけ (軽量)。
struct PendingUploadResult {
    generation: u64,
    path: PathBuf,
    kind: SkinKind,
    queued_at: Instant,
    decode_started_at: Instant,
    decode_finished_at: Instant,
    upload_started_at: Instant,
    upload_finished_at: Instant,
    uploaded: Result<UploadedSkin>,
}

#[derive(Debug, Default, Clone, Copy)]
struct SkinDrainStats {
    received_count: usize,
    applied_count: usize,
    max_upload_wait_us: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DeferredBoot {
    Chart {
        chart_id: i64,
        replay_slot: Option<u8>,
    },
    Practice {
        chart_id: i64,
        start_time_ms: Option<u32>,
        end_time_ms: Option<u32>,
    },
    /// `--boot-replay-file <PATH>`: リプレイファイル直接指定の再生。
    ReplayFile {
        path: String,
    },
    CourseReplay {
        course_id: i64,
    },
    Course {
        course_id: i64,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum AppViewState {
    Select,
    Decide,
    Play,
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppSceneKind {
    Select,
    Decide,
    Play,
    Result,
}

fn select_ir_cache_context(
    ln_policy_setting: crate::ln_policy::LnPolicySetting,
    ln_policy: crate::ln_policy::LnScorePolicy,
    double_option: crate::select_options::DoubleOptionScoreBucket,
    rule_mode: bmz_gameplay::rule::RuleMode,
) -> String {
    format!(
        "{}:{}:{}:{}",
        ln_policy_setting.as_ir_str(),
        ln_policy.as_str(),
        double_option.as_str(),
        rule_mode.as_str()
    )
}

fn course_total_notes_for_definition(
    library_db: &LibraryDatabase,
    definition: &bmz_core::course::CourseDefinition,
    app_config: &AppConfig,
    ln_policy_setting: crate::ln_policy::LnPolicySetting,
    rule_mode: bmz_gameplay::rule::RuleMode,
    entry_start_options: &[PlayStartOptions],
) -> Result<u32> {
    anyhow::ensure!(
        definition.entries.len() == entry_start_options.len(),
        "course entry option count mismatch: entries={}, options={}",
        definition.entries.len(),
        entry_start_options.len()
    );
    let mut total_notes = 0u32;
    for (index, (entry, start_options)) in
        definition.entries.iter().zip(entry_start_options).enumerate()
    {
        let chart_id = entry
            .chart_id
            .with_context(|| format!("course entry {} is not resolved", index + 1))?;
        let mut session_options =
            play_session_options_from_start(app_config, start_options.clone());
        session_options.ln_policy_setting = ln_policy_setting;
        session_options.rule_mode = rule_mode;
        let notes = crate::screens::play_session::scored_note_count_for_chart(
            library_db,
            chart_id,
            &session_options,
        )
        .with_context(|| format!("failed to count course entry {} from source", index + 1))?;
        total_notes = total_notes.saturating_add(notes);
    }
    Ok(total_notes)
}

fn hydrate_course_entry_title_hints(
    library_db: &LibraryDatabase,
    definition: &mut bmz_core::course::CourseDefinition,
) -> Result<()> {
    let chart_ids =
        definition.entries.iter().filter_map(|entry| entry.chart_id).collect::<Vec<_>>();
    let titles = library_db
        .list_charts_by_ids(&chart_ids)?
        .into_iter()
        .map(|chart| (chart.chart_id, chart.title))
        .collect::<HashMap<_, _>>();
    apply_course_entry_title_hints(definition, &titles);
    Ok(())
}

fn apply_course_entry_title_hints(
    definition: &mut bmz_core::course::CourseDefinition,
    titles: &HashMap<i64, String>,
) {
    for entry in &mut definition.entries {
        let Some(chart_id) = entry.chart_id else {
            continue;
        };
        let Some(title) = titles.get(&chart_id).filter(|title| !title.trim().is_empty()) else {
            continue;
        };
        entry.title_hint.clone_from(title);
    }
}

fn player_stats_snapshot(
    score_db: &ScoreDatabase,
    library_db: &LibraryDatabase,
    day_start_hour: u8,
) -> PlayerStatsSnapshot {
    let mut snapshot = match score_db.player_stats() {
        Ok(stats) => player_stats_snapshot_from_stats(&stats),
        Err(error) => {
            tracing::warn!(%error, "failed to load player statistics");
            PlayerStatsSnapshot::default()
        }
    };
    match score_db.current_daily_statistics_range(day_start_hour) {
        Ok((start_at, end_at)) => {
            match score_db.daily_player_stats_between(start_at, end_at) {
                Ok(stats) => snapshot.daily = daily_player_stats_snapshot_from_stats(&stats),
                Err(error) => tracing::warn!(%error, "failed to load daily player statistics"),
            }
            match score_db.daily_recent_chart_sha256s_between(start_at, end_at, 10) {
                Ok(hashes) => {
                    for (index, hash) in hashes.into_iter().enumerate() {
                        snapshot.daily.recent_titles[index] = library_db
                            .list_charts_by_sha256(hash)
                            .ok()
                            .and_then(|charts| charts.into_iter().next())
                            .map(|chart| chart.title)
                            .unwrap_or_default();
                    }
                }
                Err(error) => tracing::warn!(%error, "failed to load recent daily chart titles"),
            }
        }
        Err(error) => tracing::warn!(%error, "failed to resolve daily statistics range"),
    }
    snapshot
}

fn player_stats_snapshot_from_stats(stats: &PlayerStats) -> PlayerStatsSnapshot {
    PlayerStatsSnapshot {
        play_count: stats.play_count,
        clear_count: stats.clear_count,
        playtime_seconds: stats.playtime_seconds,
        max_combo: stats.max_combo,
        fast_pgreat: stats.fast_pgreat,
        slow_pgreat: stats.slow_pgreat,
        fast_great: stats.fast_great,
        slow_great: stats.slow_great,
        fast_good: stats.fast_good,
        slow_good: stats.slow_good,
        fast_bad: stats.fast_bad,
        slow_bad: stats.slow_bad,
        fast_poor: stats.fast_poor,
        slow_poor: stats.slow_poor,
        fast_empty_poor: stats.fast_empty_poor,
        slow_empty_poor: stats.slow_empty_poor,
        daily: DailyPlayerStatsSnapshot::default(),
    }
}

fn daily_player_stats_snapshot_from_stats(stats: &DailyPlayerStats) -> DailyPlayerStatsSnapshot {
    DailyPlayerStatsSnapshot {
        play_count: stats.play_count,
        clear_count: stats.clear_count,
        pgreat: stats.pgreat,
        great: stats.great,
        good: stats.good,
        bad: stats.bad,
        poor: stats.poor,
        empty_poor: stats.empty_poor,
        score_update_count: stats.score_update_count,
        clear_update_count: stats.clear_update_count,
        miss_count_update_count: stats.miss_count_update_count,
        recent_titles: Default::default(),
    }
}

fn initialize_gamepad_backend(
    kind: GamepadBackendKind,
    sensitivity: f32,
    scratch_threshold: u32,
) -> Option<Box<crate::input::gamepad::GamepadBackend>> {
    match kind {
        GamepadBackendKind::Auto => {
            if let Some(backend) = initialize_gilrs_backend(sensitivity, scratch_threshold) {
                return Some(backend);
            }
            #[cfg(windows)]
            return initialize_gameinput_backend(sensitivity, scratch_threshold);
            #[cfg(not(windows))]
            None
        }
        GamepadBackendKind::Gilrs => initialize_gilrs_backend(sensitivity, scratch_threshold),
        GamepadBackendKind::GameInput => {
            #[cfg(windows)]
            {
                if let Some(backend) = initialize_gameinput_backend(sensitivity, scratch_threshold)
                {
                    return Some(backend);
                }
            }
            #[cfg(not(windows))]
            tracing::warn!("GameInput is only available on Windows, falling back to gilrs");
            initialize_gilrs_backend(sensitivity, scratch_threshold)
        }
    }
}

#[cfg(windows)]
fn initialize_gameinput_backend(
    sensitivity: f32,
    scratch_threshold: u32,
) -> Option<Box<crate::input::gamepad::GamepadBackend>> {
    match crate::input::gameinput::GameInputBackend::new(sensitivity, scratch_threshold) {
        Ok(backend) => {
            tracing::info!("GameInput initialized on main thread");
            Some(Box::new(crate::input::gamepad::GamepadBackend::GameInput(backend)))
        }
        Err(error) => {
            tracing::warn!(%error, "GameInput init failed");
            None
        }
    }
}

fn initialize_gilrs_backend(
    sensitivity: f32,
    scratch_threshold: u32,
) -> Option<Box<crate::input::gamepad::GamepadBackend>> {
    match crate::input::gilrs::GilrsBackend::new(sensitivity, scratch_threshold) {
        Ok(backend) => {
            tracing::info!("gilrs initialized");
            Some(Box::new(crate::input::gamepad::GamepadBackend::Gilrs(backend)))
        }
        Err(error) => {
            tracing::warn!(%error, "gilrs init failed");
            None
        }
    }
}

fn resolve_gamepad_runtime_slots(
    config: &GlobalInputConfig,
    backend: Option<&crate::input::gamepad::GamepadBackend>,
) -> [Option<DeviceId>; 2] {
    let connected = backend
        .into_iter()
        .flat_map(crate::input::gamepad::GamepadBackend::connected_gamepads)
        .collect::<Vec<_>>();
    let using_gilrs = backend.is_some_and(crate::input::gamepad::GamepadBackend::is_gilrs);
    crate::input::gamepad::resolve_gamepad_slot_assignments(
        config.gamepad_slot_device_ids.each_ref().map(Option::as_deref),
        config.gamepad_slot_gilrs_ids,
        using_gilrs,
        !using_gilrs,
        &connected,
    )
}

impl WinitApp {
    fn new(
        boot: BootstrappedApp,
        options: AppOptions,
        audio_runtime: Option<AudioRuntime>,
        system_audio: Option<crate::audio::SystemAudio>,
        shutdown_requested: Arc<AtomicBool>,
        event_proxy: EventLoopProxy<AppUserEvent>,
        log_buffer: LogBuffer,
    ) -> Result<Self> {
        let mut boot = boot;
        if let Some(cli_renderer) = options.renderer.clone() {
            tracing::info!(?cli_renderer, "overriding renderer backend via CLI option");
            boot.app_config.video.renderer = cli_renderer;
        }

        // ネットワークへ出る前に、DBから必要なURLだけ選定しておく。実際の取得は
        // 最初の描画後に開始するため、初回起動でもウィンドウ表示を待たせない。
        let startup_table_fetch_urls = startup_difficulty_table_fetch_urls_for_boot(&boot);

        let folder_stack = initial_folder_stack(&boot.app_config);
        let initial_mode_filter =
            SelectModeFilter::from_str_or_default(&boot.profile_config.select.mode_filter);
        let select_sort = SelectSort::from_str_or_default(&boot.profile_config.select.sort);
        let (select_items, select_mode_filter) =
            load_items_for_stack(&boot, &folder_stack, &[], initial_mode_filter, select_sort);
        boot.profile_config.select.mode_filter = select_mode_filter.as_str().to_string();
        let boot_chart_id = resolve_boot_chart_id(&boot.library_db, &options);
        log_startup_options(&options);

        let session_mode = if options.autoplay_on_start {
            SessionMode::Autoplay
        } else {
            session_mode_from_profile(&boot.profile_config.play)
        };
        let gauge_option = if boot.profile_config.play.gauge == GaugeTypeConfig::AutoShift {
            GaugeTypeConfig::ExHard
        } else {
            boot.profile_config.play.gauge
        };
        let gauge_auto_shift_option =
            if boot.profile_config.play.gauge == GaugeTypeConfig::AutoShift {
                GaugeAutoShiftConfig::BestClear
            } else {
                boot.profile_config.play.gauge_auto_shift
            };
        let bottom_shiftable_gauge_option = boot.profile_config.play.bottom_shiftable_gauge;
        let arrange_option = arrange_option_from_profile(boot.profile_config.play.random);
        let arrange_option_2p = arrange_option_from_profile(boot.profile_config.play.random2);
        let double_option = double_option_from_profile(boot.profile_config.play.double_option);
        let hs_fix_option = hs_fix_option_from_profile(boot.profile_config.play.hs_fix);
        let target_option = target_option_from_profile(boot.profile_config.play.target);
        let select_keys = SelectKeyBindings::from_profile(&boot.profile_config.input);
        let mut renderer = Box::new(Renderer::default());
        renderer.set_default_font_search_paths(vec![boot.app_paths.bundled_noto_cjk_font_root()]);
        renderer.set_default_font_coverage(boot.profile_config.ui.locale().font_coverage());
        renderer
            .set_internal_resolution_mode(config_internal_resolution_mode(&boot.app_config.video));
        let skin_catalog = scan_skin_catalog(&boot.app_paths);
        let mut skin_pipeline = SkinPipelineRuntime::new();
        let (
            default_skin_manifest,
            initial_skin_video_sources,
            pending_select_skin,
            pending_decide_skin,
            pending_result_skin,
        ) = load_initial_skin_textures(
            renderer.as_mut(),
            &boot.app_paths,
            &skin_pipeline.decode_tx,
            &skin_pipeline.source_asset_cache,
            &skin_pipeline.document_cache,
            &skin_pipeline.gpu_texture_cache,
            &skin_pipeline.font_cache,
            0,
            &boot.profile_config.display_name,
            &boot.profile_config.skin.select,
            &boot.profile_config.skin.decide,
            &boot.profile_config.skin.result,
            &boot.profile_config.skin.select_options,
            &boot.profile_config.skin.decide_options,
            &boot.profile_config.skin.result_options,
            &boot.profile_config.skin.select_files,
            &boot.profile_config.skin.decide_files,
            &boot.profile_config.skin.result_files,
            &boot.profile_config.skin.select_offsets,
            &boot.profile_config.skin.decide_offsets,
            &boot.profile_config.skin.result_offsets,
        );
        skin_pipeline.set_pending(SkinKind::Select, pending_select_skin);
        skin_pipeline.set_pending(SkinKind::Decide, pending_decide_skin);
        skin_pipeline.set_pending(SkinKind::Result, pending_result_skin);
        let now = Instant::now();

        let gamepad = if boot.app_config.input.gamepad_enabled {
            let sensitivity = boot.profile_config.input.analog_scratch_sensitivity;
            let threshold = boot.profile_config.input.analog_scratch_threshold;
            initialize_gamepad_backend(
                boot.app_config.input.gamepad_backend,
                sensitivity,
                threshold,
            )
        } else {
            None
        };

        let initial_window_mode = boot.app_config.video.mode.clone();
        let applied_obs_config = boot.app_config.obs.clone();
        let obs_controller = crate::obs::ObsController::spawn(applied_obs_config.clone());

        // システム SE / BGM facade を構築する。
        // - `profile.[system_sound].bgm_dir` / `se_dir` が指定されていれば再帰スキャンして
        //   セットを集め、その中からランダム選択する(beatoraja 互換)。
        // - 空なら scan を省略し、`default_sound_dir` だけにフォールバックする。
        let system_sound =
            system_audio.as_ref().map(|audio| system_sound_manager_from_boot(&boot, audio));
        let select_preview =
            system_audio.as_ref().map(|audio| SelectChartPreview::new(audio.engine()));
        let select_assets =
            SelectAssetRuntime::new(select_preview, boot.app_paths.library_db.clone());
        let audio_output_open_attempted = audio_runtime.is_some();
        let player_stats = player_stats_snapshot(
            &boot.score_db,
            &boot.library_db,
            boot.profile_config.statistics.day_start_hour,
        );
        let initial_result_skin_signature = result_skin_signature_for_config(
            &boot.profile_config.skin,
            ResultSkinSlot::Normal,
            lua_runtime_state_for_result(
                false,
                None,
                false,
                KeyMode::default(),
                BTreeMap::new(),
                &boot.profile_config.display_name,
            ),
        );
        let difficulty_tables = match boot.library_db.list_difficulty_tables() {
            Ok(tables) => tables,
            Err(error) => {
                tracing::warn!(%error, "failed to list difficulty tables for egui");
                Vec::new()
            }
        };
        let select_folder_summary_ln_policy = boot.profile_config.play.ln_mode_policy;
        let select_folder_summary_rule_mode = boot.profile_config.play.rule_mode;
        let select_folder_summaries = SelectFolderSummaryRuntime::new(
            boot.app_paths.library_db.clone(),
            boot.profile_paths.score_db.clone(),
            &folder_stack,
            select_folder_summary_ln_policy,
            select_folder_summary_rule_mode,
        )?;
        let rian_table_identity = RianTableIdentity::from_ir_config(&boot.profile_config.ir);
        let table_fetch = TableFetchRuntime::new(startup_table_fetch_urls, rian_table_identity);

        let mut app = Self {
            boot,
            window: None,
            first_frame_startup_completed: false,
            shutdown_requested,
            renderer,
            input: AppInputRuntime::default(),
            gamepad,
            event_proxy,
            frame: FrameRuntime::new(now),
            deferred_boot: deferred_boot_action(boot_chart_id, &options),
            select: SelectRuntimeState {
                autoplay_folder: None,
                select_ir: crate::screens::select_ir::SelectIrRanking::default(),
                player_stats,
                select_items,
                select_distribution_cache: RefCell::new(HashMap::new()),
                difficulty_tables,
                table_breadcrumb_cache: RefCell::new(HashMap::new()),
                select_folder_summaries,
                selected_index_stack: vec![0; folder_stack.len()],
                folder_stack,
                selected_index: 0,
                arrange_option,
                arrange_option_2p,
                random_trainer: RandomTrainerState::default(),
                target_option,
                gauge_option,
                gauge_auto_shift_option,
                bottom_shiftable_gauge_option,
                double_option,
                hs_fix_option,
                session_mode,
                select_mode_filter,
                select_sort,
                select_keys,
                select_bar_scroll_direction: 0,
                select_bar_scroll_duration: Duration::ZERO,
                select_hold_move: None,
                select_hold_started_at: None,
                select_hold_last_trigger_at: None,
                select_hold_control: None,
                select_analog_scroll_buffer: 0,
                select_analog_last_tick_at: None,
                select_analog_suppress_until_idle: false,
                select_scene_started_at: now,
                select_bar_started_at: now,
                option_panel_started_at: now,
                option_panel_off_started_at: [None; 6],
                select_option_panel: 0,
                select_exit_hold_started_at: None,
                select_assets,
                settings_edit: None,
                key_config_edit: None,
                search: SelectSearchRuntime::new(now),
                last_cursor_position: None,
                select_slider_dragging_type: None,
            },
            play: PlayRuntimeState {
                active_play: None,
                active_course: None,
                last_play_snapshot: None,
                pending_decide: None,
                pending_play_start: None,
                pending_play_preload: None,
                preloaded_play_session: None,
                play_preload_generation: 0,
                play_media_cache: None,
                play_ending: None,
                last_started_chart_id: None,
                play_table_text_primary: String::new(),
                play_table_text_secondary: String::new(),
                play_table_text_fallback: String::new(),
                play_option_input: None,
                play_analog_scroll_buffer: 0,
                play_analog_last_tick_at: None,
                play_scene_started_at: now,
                play_ready_sound_started_at: None,
                play_ready_last_control_hold_at: None,
                decide_sound_stopped_for_chart_start: false,
                bga_preload: BgaPreloadRuntime::default(),
                play_stagefile_source: None,
                play_stagefile_loaded: false,
                play_stagefile_size: None,
                play_backbmp_source: None,
                play_backbmp_loaded: false,
                last_play_start_press_at: None,
                decide_e1_held: false,
                play_e1_held: false,
                play_e2_held: false,
                play_e3_held: false,
                play_exit_hold_started_at: None,
                practice_session: None,
                practice_chart_zero_time: None,
            },
            result: ResultRuntimeState {
                finished_course: None,
                finished_course_skin_summary: None,
                finished_course_hash: None,
                finished_course_rian_hash_v1: None,
                finished_course_ir_attempted: false,
                finished_play: None,
                result_favorite_chart: false,
                result_ir: None,
                last_play_was_autoplay: false,
                result_scene_started_at: now,
                result_skin_audio: None,
                result_exit: None,
                result_key5_held: false,
                result_key7_held: false,
                result_gauge_graph_type: GaugeType::Normal as i32,
                result_panel: 0,
            },
            jobs: AppJobs {
                table_fetch,
                pending_song_scan: None,
                pending_chart_download: None,
                queued_download_scan: None,
                song_scan_progress: None,
                pending_update_check: None,
                pending_update_check_reports_up_to_date: false,
                pending_update_download: None,
                update_prompt: None,
                update_dismissed_session_version: None,
            },
            integrations: IntegrationRuntimeState {
                obs_controller,
                applied_obs_config,
                exit_configs_saved: false,
                last_scene_kind: None,
                discord_presence: None,
                discord_presence_config: None,
                last_obs_event_key: None,
            },
            smoke: SmokeRuntime {
                smoke_exit_after_frames: options.smoke_exit_after_frames,
                smoke_exit_after_play_frames: options.smoke_exit_after_play_frames,
                smoke_exit_after_result_frames: options.smoke_exit_after_result_frames,
                smoke_exit_on_result: options.smoke_exit_on_result,
                smoke_screenshot_path: options.smoke_screenshot_path.as_ref().map(PathBuf::from),
                left_overlay_toast: None,
                rendered_frames: 0,
                rendered_play_frames: 0,
                rendered_result_frames: 0,
                app_started_at: now,
            },
            skin: SkinRuntimeState {
                skin_catalog,
                skin_defs_cache: BTreeMap::new(),
                default_skin_manifest,
                skin_pipeline,
                skin_video_sources: initial_skin_video_sources,
                pending_skin_render_probe: None,
                last_play_skin_signature: None,
                last_result_skin_signature: Some(initial_result_skin_signature),
            },
            audio: AppAudioRuntimeState {
                draining_audio: None,
                audio_runtime,
                audio_output_open_attempted,
                audio_diagnostics_last_log_at: now,
                audio_diagnostics_last: None,
                input_diagnostics_last_sequence: 0,
                system_audio,
                system_sound,
            },
            ui: UiRuntimeState {
                egui: None,
                log_buffer,
                applied_window_mode: initial_window_mode,
                focused: true,
                last_cursor_action_at: now,
                cursor_visible: true,
            },
        };
        if options.boot_result_sample {
            tracing::info!("booting directly into synthetic result screen");
            app.result.finished_play = Some(debug_boot_finished_play_session());
            app.result.result_gauge_graph_type = app
                .result
                .finished_play
                .as_ref()
                .map(|finished| finished.summary.gauge_type as i32)
                .unwrap_or(GaugeType::Normal as i32);
            app.result.result_key5_held = false;
            app.result.result_key7_held = false;
            app.result.result_scene_started_at = Instant::now();
        }
        app.sync_discord_presence_config();
        if app.boot.app_config.updates.enabled && app.boot.app_config.updates.check_on_startup {
            app.spawn_update_check("startup update check", false);
        }

        Ok(app)
    }

    fn refresh_player_stats_snapshot(&mut self) {
        self.select.player_stats = player_stats_snapshot(
            &self.boot.score_db,
            &self.boot.library_db,
            self.boot.profile_config.statistics.day_start_hour,
        );
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn keyboard_input_backend(&self) -> Option<KeyboardInputBackend> {
        keyboard_input_backend_for_config(&self.boot.app_config)
    }

    fn raw_input_keyboard_enabled(&self) -> bool {
        self.keyboard_input_backend() == Some(KeyboardInputBackend::RawInput)
    }

    fn window_keyboard_gameplay_enabled(&self) -> bool {
        self.keyboard_input_backend() == Some(KeyboardInputBackend::Window)
    }

    fn configure_device_events(&self, event_loop: &ActiveEventLoop) {
        let device_events = if self.raw_input_keyboard_enabled() {
            DeviceEvents::WhenFocused
        } else {
            DeviceEvents::Never
        };
        event_loop.listen_device_events(device_events);
    }

    fn raw_input_gameplay_blocked(&self) -> bool {
        let practice_overlay = self
            .play
            .practice_session
            .as_ref()
            .is_some_and(|practice| practice.phase == PracticePhase::Config);
        self.ui.egui.as_ref().is_some_and(|egui| egui.blocks_game_input(practice_overlay))
    }

    fn play_input_backend(&self) -> Option<SharedInputBackend> {
        play_input_backend_for_context(
            self.play.active_play.as_ref().map(|active_play| &active_play.input),
            self.play.pending_play_start.is_some(),
            self.play.preloaded_play_session.as_ref().map(|preloaded| &preloaded.input),
            self.play.pending_play_preload.as_ref().map(|pending| &pending.input),
        )
    }

    fn filter_app_input_bounce(&mut self, event: DeviceInputEvent) -> Option<DeviceInputEvent> {
        let config = input_bounce_config_from_profile(&self.boot.profile_config.input);
        self.input.accept_app_event(config, event)
    }

    fn route_play_device_input(&mut self, event: DeviceInputEvent) {
        let Some(input) = self.play_input_backend() else {
            return;
        };
        input.push_shared_event(event.clone());
        if self.play.active_play.is_some() {
            return;
        }
        let visual_now = self.play_elapsed_time();
        if let Some(pending) = &mut self.play.pending_play_start {
            pending.visual_input.apply_event(&event, visual_now);
        }
        self.refresh_pending_play_visual_snapshot(visual_now);
    }

    fn refresh_pending_play_visual_snapshot(&mut self, visual_now: TimeUs) {
        if self.play.active_play.is_some() {
            return;
        }
        let Some(pending) = &mut self.play.pending_play_start else {
            return;
        };
        pending.visual_input.advance(visual_now);
        let Some(snapshot) = &mut self.play.last_play_snapshot else {
            return;
        };
        crate::screens::play_snapshot::refresh_pending_play_input_visuals(
            snapshot,
            pending.visual_input.key_mode,
            pending.visual_input.lane_keyon_started_at,
            pending.visual_input.lane_keyoff_started_at,
            pending.visual_input.lane_scratch_angle_delta_ms,
            visual_now,
        );
    }

    fn route_raw_keyboard_gameplay_input(
        &mut self,
        physical_key: PhysicalKey,
        state: ElementState,
    ) {
        if !self.raw_input_keyboard_enabled() {
            return;
        }
        if self.play_input_backend().is_none() {
            self.input.discard_raw_keyboard_transition(physical_key, state);
            return;
        }
        let config = input_bounce_config_from_profile(&self.boot.profile_config.input);
        let gameplay_blocked = self.raw_input_gameplay_blocked();
        if let Some(event) =
            self.input.raw_keyboard_transition(config, physical_key, state, gameplay_blocked)
        {
            self.route_play_device_input(event);
        }
    }

    fn ensure_window(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let video = &self.boot.app_config.video;
        let attributes =
            window_attributes_from_config(video).with_fullscreen(fullscreen_from_config(
                &video.mode,
                select_monitor(
                    &video.monitor_name,
                    event_loop.available_monitors(),
                    event_loop.primary_monitor(),
                ),
            ));
        match event_loop.create_window(attributes) {
            Ok(window) => {
                let window = Arc::new(window);
                window.set_visible(true);
                let size = surface_size_for_window(&window);
                // サーフェス生成前に present mode とバックエンド設定を反映させておく。
                self.renderer.set_present_mode(config_present_mode(&self.boot.app_config.video));
                let backend = config_renderer_backend(self.boot.app_config.video.renderer.clone());
                self.renderer.set_backend(backend);
                if let Err(error) = self.renderer.attach_surface(Arc::clone(&window), size) {
                    tracing::error!(%error, "failed to initialize renderer surface");
                    event_loop.exit();
                    return;
                }
                tracing::info!(
                    width = size.width,
                    height = size.height,
                    "window and renderer surface ready"
                );
                // surface 接続後 (= GPU device/queue 利用可能) に upload worker を起動する。
                // decode 結果はそれまで skin_decode_rx にバッファされ、起動後にドレインされる。
                self.start_skin_upload_worker();
                self.configure_device_events(event_loop);
                window.request_redraw();
                self.ui.egui = Some(EguiLayer::new(
                    &window,
                    self.boot.profile_config.ui.show_fps,
                    vec![self.boot.app_paths.bundled_noto_cjk_font_root()],
                ));
                self.window = Some(window);
            }
            Err(error) => {
                tracing::error!(%error, "failed to create window");
                event_loop.exit();
            }
        }
    }

    fn start_deferred_boot(&mut self) {
        let Some(boot) = self.deferred_boot.take() else {
            return;
        };
        match boot {
            DeferredBoot::Chart { chart_id, replay_slot } => {
                tracing::info!(chart_id, "booting directly into chart");
                if let Some(slot) = replay_slot {
                    if !self.try_start_replay_for_chart(chart_id, slot, false) {
                        tracing::warn!(slot, "boot replay slot empty; falling back to normal play");
                        self.start_chart(chart_id);
                    }
                } else {
                    self.start_chart(chart_id);
                }
            }
            DeferredBoot::Practice { chart_id, start_time_ms, end_time_ms } => {
                tracing::info!(chart_id, "booting into practice mode");
                self.enter_practice(chart_id, PracticeCliOverrides { start_time_ms, end_time_ms });
            }
            DeferredBoot::ReplayFile { path } => {
                tracing::info!(%path, "booting replay from file");
                if !self.try_start_replay_from_file(std::path::Path::new(&path)) {
                    tracing::warn!(%path, "replay file boot failed; staying on select");
                }
            }
            DeferredBoot::CourseReplay { course_id } => {
                let Some(identity) = self.ir_course_identity(course_id) else {
                    tracing::warn!(
                        course_id,
                        "course identity unavailable; --boot-course-replay has nothing to replay"
                    );
                    return;
                };
                let rule_mode = self.boot.profile_config.play.rule_mode;
                match self.boot.score_db.latest_course_score_id(&identity.course_hash, rule_mode) {
                    Ok(Some(course_score_id)) => {
                        tracing::info!(course_id, course_score_id, "booting into course replay");
                        self.start_course_replay_with_auto_advance(
                            course_id,
                            course_score_id,
                            true,
                        );
                    }
                    Ok(None) => {
                        tracing::warn!(
                            course_id,
                            "no saved course attempt; --boot-course-replay has nothing to replay"
                        );
                    }
                    Err(error) => {
                        tracing::error!(
                            %error,
                            course_id,
                            "failed to look up latest course score for replay boot"
                        );
                    }
                }
            }
            DeferredBoot::Course { course_id } => {
                tracing::info!(course_id, "booting into fresh course");
                self.start_course_with_arrange(course_id, Vec::new(), true);
            }
        }
    }

    fn view_state(&self) -> AppViewState {
        if self.play.pending_decide.is_some() {
            return AppViewState::Decide;
        }
        if self.play.active_play.is_some() || self.play.pending_play_start.is_some() {
            return AppViewState::Play;
        }

        if self.result.finished_course.is_some() || self.result.finished_play.is_some() {
            return AppViewState::Result;
        }

        AppViewState::Select
    }

    fn current_scene_kind(&self) -> AppSceneKind {
        if self.play.pending_decide.is_some() {
            return AppSceneKind::Decide;
        }
        if self.play.active_play.is_some() || self.play.pending_play_start.is_some() {
            return AppSceneKind::Play;
        }
        if self.result.finished_course.is_some() || self.result.finished_play.is_some() {
            return AppSceneKind::Result;
        }
        AppSceneKind::Select
    }

    fn current_result_summary(&self) -> Option<&ResultSummary> {
        self.result
            .finished_course_skin_summary
            .as_ref()
            .or_else(|| self.result.finished_play.as_ref().map(|finished| &finished.summary))
    }

    fn install_finished_course(
        &mut self,
        course: CourseResultSummary,
        course_hash: Option<String>,
        rian_course_hash_v1: Option<String>,
    ) {
        self.result.finished_course_skin_summary = Some(course_result_summary_for_skin(&course));
        self.result.finished_course = Some(course);
        self.result.finished_course_hash = course_hash;
        self.result.finished_course_rian_hash_v1 = rian_course_hash_v1;
        self.result.finished_course_ir_attempted = false;
    }

    fn clear_finished_course(&mut self) {
        self.result.finished_course = None;
        self.result.finished_course_skin_summary = None;
        self.result.finished_course_hash = None;
        self.result.finished_course_rian_hash_v1 = None;
        self.result.finished_course_ir_attempted = false;
    }

    fn scene_snapshot(&self) -> AppSceneSnapshot {
        let mut scene = match self.view_state() {
            AppViewState::Select => AppSceneSnapshot::Select(self.select_snapshot()),
            AppViewState::Decide => {
                let mut snapshot = self
                    .play
                    .pending_decide
                    .as_ref()
                    .map(|decide| self.decide_snapshot(decide))
                    .or_else(|| self.play.last_play_snapshot.clone())
                    .unwrap_or_default();
                snapshot.skin_offsets =
                    skin_offset_values_from_config(&self.boot.profile_config.skin.decide_offsets);
                AppSceneSnapshot::Decide(snapshot)
            }
            AppViewState::Play => {
                AppSceneSnapshot::Play(self.play.last_play_snapshot.clone().unwrap_or_default())
            }
            AppViewState::Result => {
                // `view_state` only returns Result when one of the result sources exists.
                // A finished course is always installed together with its skin summary.
                let summary =
                    self.current_result_summary().expect("result scene is missing its summary");
                let raw_clear_type = self
                    .is_course_intermediate_result()
                    .then(|| {
                        self.result
                            .finished_play
                            .as_ref()
                            .map(|finished| finished.result.clear_type)
                    })
                    .flatten();
                let result_failed = result_failed_for_skin_ops(summary.clear_type, raw_clear_type);
                let score_save_enabled = self.current_result_score_save_enabled();
                let result_ir_scope_binding = self
                    .renderer
                    .result_skin_document()
                    .map(|document| document.result_ir_scope_binding)
                    .unwrap_or_default();
                AppSceneSnapshot::Result(ResultSnapshot {
                    player_name: String::new(),
                    target_name: summary.target_name.clone(),
                    current_fps: 0,
                    skin_input: Default::default(),
                    skin_offsets: skin_offset_values_from_config(
                        match self.current_result_skin_slot() {
                            ResultSkinSlot::Normal => &self.boot.profile_config.skin.result_offsets,
                            ResultSkinSlot::Course => {
                                &self.boot.profile_config.skin.course_result_offsets
                            }
                        },
                    ),
                    hispeed_auto_adjust: self.boot.profile_config.lane.hispeed_auto_adjust,
                    clear_type: summary.clear_type,
                    result_failed,
                    arrange: summary.arrange.as_str().to_string(),
                    arrange_2p: summary.arrange_2p.as_str().to_string(),
                    double_option: self
                        .result_double_option_for_slot(self.current_result_skin_slot())
                        .as_str()
                        .to_string(),
                    lane_shuffle_pattern: summary.lane_shuffle_pattern.clone(),
                    ex_score: summary.ex_score,
                    ex_score_rate: summary.ex_score_rate(),
                    max_combo: summary.max_combo,
                    bp: summary.bp,
                    cb: summary.cb,
                    gauge_value: summary.gauge_value,
                    gauge_type: summary.gauge_type as i32,
                    total_notes: summary.total_notes,
                    grade_diff_display: self.boot.profile_config.play.grade_diff_display,
                    duration_ms: summary.duration_ms,
                    note_display_duration_ms: Some(Self::select_note_display_duration_ms_for_skin(
                        &self.boot.profile_config,
                    )),
                    initial_bpm: summary.initial_bpm,
                    min_bpm: result_min_bpm(summary),
                    max_bpm: result_max_bpm(summary),
                    main_bpm: result_main_bpm(summary),
                    total_gauge: summary.total_gauge,
                    judge_rank: summary.judge_rank,
                    key_mode: summary.key_mode,
                    has_long_notes: summary.has_long_notes,
                    ln_mode_index: result_long_note_mode_index(summary.long_note_mode),
                    result_gauge_graph_type: self.result.result_gauge_graph_type,
                    result_panel: self.result.result_panel,
                    favorite_chart: self.result.result_favorite_chart,
                    judge_counts: DisplayJudgeCounts {
                        pgreat: summary.judge_counts.pgreat,
                        great: summary.judge_counts.great,
                        good: summary.judge_counts.good,
                        bad: summary.judge_counts.bad,
                        poor: summary.judge_counts.poor,
                        empty_poor: summary.judge_counts.empty_poor,
                    },
                    fast_slow_counts: FastSlowJudgeCounts {
                        fast_pgreat: summary.fast_slow_counts.fast_pgreat,
                        slow_pgreat: summary.fast_slow_counts.slow_pgreat,
                        fast_great: summary.fast_slow_counts.fast_great,
                        slow_great: summary.fast_slow_counts.slow_great,
                        fast_good: summary.fast_slow_counts.fast_good,
                        slow_good: summary.fast_slow_counts.slow_good,
                        fast_bad: summary.fast_slow_counts.fast_bad,
                        slow_bad: summary.fast_slow_counts.slow_bad,
                        fast_poor: summary.fast_slow_counts.fast_poor,
                        slow_poor: summary.fast_slow_counts.slow_poor,
                        fast_empty_poor: summary.fast_slow_counts.fast_empty_poor,
                        slow_empty_poor: summary.fast_slow_counts.slow_empty_poor,
                    },
                    score_save_enabled,
                    score_history_id: summary.score_history_id,
                    replay_saved: !summary.replay_path.is_empty(),
                    replay_slots: summary.replay_slots,
                    saved_replay_slots: summary.saved_replay_slots,
                    best_ex_score: summary.best_ex_score,
                    best_clear_type: summary.best_clear_type,
                    target_ex_score: summary.target_ex_score,
                    best_max_combo: summary.best_max_combo,
                    target_max_combo: summary.target_max_combo,
                    best_bp: summary.best_bp,
                    target_bp: summary.target_bp,
                    previous_best_ex_score: summary.previous_best_ex_score,
                    previous_best_clear_type: summary.previous_best_clear_type,
                    previous_best_max_combo: summary.previous_best_max_combo,
                    previous_best_bp: summary.previous_best_bp,
                    target_clear_type: summary.target_clear_type,
                    elapsed_time: bmz_core::time::TimeUs(
                        self.result
                            .result_scene_started_at
                            .elapsed()
                            .as_micros()
                            .min(i64::MAX as u128) as i64,
                    ),
                    fadeout_elapsed: self.result.result_exit.as_ref().map(|exit| {
                        bmz_core::time::TimeUs(
                            exit.started_at.elapsed().as_micros().min(i64::MAX as u128) as i64,
                        )
                    }),
                    title: summary.title.clone(),
                    subtitle: summary.subtitle.clone(),
                    artist: summary.artist.clone(),
                    subartist: summary.subartist.clone(),
                    genre: summary.genre.clone(),
                    difficulty_name: summary.difficulty_name.clone(),
                    play_level: summary.play_level.clone(),
                    table_text_primary: self.play.play_table_text_primary.clone(),
                    table_text_secondary: self.play.play_table_text_secondary.clone(),
                    table_text_fallback: self.play.play_table_text_fallback.clone(),
                    stagefile_background: self.play.play_stagefile_loaded,
                    stagefile_image_size: self.play.play_stagefile_size,
                    course_titles: self
                        .result
                        .finished_course
                        .as_ref()
                        .map(|course| course.course_titles.clone())
                        .unwrap_or_default(),
                    course_result: self
                        .result
                        .finished_course
                        .as_ref()
                        .map(course_result_skin_snapshot)
                        .unwrap_or_default(),
                    graph: summary.graph.clone(),
                    overlay: OverlaySnapshot::default(),
                    ir: self
                        .result
                        .result_ir
                        .as_ref()
                        .map(|state| state.skin_snapshot_for_binding(result_ir_scope_binding))
                        .unwrap_or_default(),
                    player_stats: self.select.player_stats.clone(),
                })
            }
        };
        apply_skin_logical_input_to_scene(
            &mut scene,
            skin_logical_input_snapshot_from_pressed_controls(
                &self.input.pressed_controls,
                &self.select.select_keys,
            ),
        );
        self.apply_operating_time_to_scene(&mut scene);
        self.apply_skin_runtime_info_to_scene(&mut scene);
        let overlay = self.build_overlay_snapshot();
        self.apply_overlay_to_scene(&mut scene, overlay);
        scene
    }

    fn operating_time_ms(&self) -> i32 {
        elapsed_since_ms(self.smoke.app_started_at)
    }

    fn apply_operating_time_to_scene(&self, scene: &mut AppSceneSnapshot) {
        apply_operating_time_ms_to_scene(scene, self.operating_time_ms());
    }

    fn apply_skin_runtime_info_to_scene(&self, scene: &mut AppSceneSnapshot) {
        apply_skin_runtime_info_to_scene(
            scene,
            &self.boot.profile_config.display_name,
            self.frame.current_fps(),
        );
    }

    fn build_overlay_snapshot(&self) -> OverlaySnapshot {
        OverlaySnapshot {
            left_text: self.left_overlay_text(),
            text: self.always_overlay_text(),
            fps_text: self.skin_fps_overlay_text(),
        }
    }

    fn left_overlay_text(&self) -> String {
        resolve_left_overlay_text(
            self.renderer.has_pending_screenshot(),
            self.smoke
                .left_overlay_toast
                .as_ref()
                .map(|toast| (toast.message.as_str(), toast.shown_at.elapsed())),
            &self.background_task_overlay_text(),
        )
    }

    fn background_task_overlay_text(&self) -> String {
        let mut tasks = Vec::new();
        if let Some(progress) = self.jobs.song_scan_progress {
            tasks.push(format!("SCAN {} / {}", progress.done, progress.total));
        }
        if let Some(progress) = &self.jobs.table_fetch.progress {
            tasks.push(format!("TABLE {} / {}", progress.completed, progress.total));
        }
        tasks.join(" | ")
    }

    fn always_overlay_text(&self) -> String {
        let player_name = env!("CARGO_PKG_NAME");
        let player_version = env!("CARGO_PKG_VERSION");
        if self.is_autoplay_for_overlay() {
            format!("{player_name} {player_version} autoplay")
        } else {
            format!("{player_name} {player_version}")
        }
    }

    fn skin_fps_overlay_text(&self) -> String {
        self.frame.overlay_text(
            self.boot.profile_config.ui.show_fps,
            Localizer::new(self.boot.profile_config.ui.locale()),
        )
    }

    fn is_autoplay_for_overlay(&self) -> bool {
        match self.view_state() {
            AppViewState::Result => self.result.last_play_was_autoplay,
            AppViewState::Play => self
                .play
                .active_play
                .as_ref()
                .map(|active| {
                    active
                        .running
                        .session
                        .autoplay
                        .as_ref()
                        .is_some_and(|autoplay| autoplay.is_full())
                })
                .or_else(|| {
                    self.play
                        .pending_play_start
                        .as_ref()
                        .map(|_| self.select.session_mode.primary_autoplay())
                })
                .unwrap_or(self.result.last_play_was_autoplay),
            AppViewState::Select | AppViewState::Decide => {
                self.select.session_mode.primary_autoplay()
            }
        }
    }

    fn apply_overlay_to_scene(&self, scene: &mut AppSceneSnapshot, overlay: OverlaySnapshot) {
        match scene {
            AppSceneSnapshot::Select(snapshot) => snapshot.overlay = overlay,
            AppSceneSnapshot::Decide(snapshot) | AppSceneSnapshot::Play(snapshot) => {
                snapshot.overlay = overlay
            }
            AppSceneSnapshot::Result(snapshot) => snapshot.overlay = overlay,
        }
    }

    fn fallback_table_breadcrumb(source_url: &str) -> TableBreadcrumb {
        TableBreadcrumb {
            name: std::path::Path::new(source_url)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(source_url)
                .to_string(),
            symbol: String::new(),
        }
    }

    fn table_breadcrumb(&self, source_url: &str) -> TableBreadcrumb {
        if let Some(cached) = self.select.table_breadcrumb_cache.borrow().get(source_url) {
            return cached.clone();
        }

        let breadcrumb = self
            .select
            .difficulty_tables
            .iter()
            .find(|table| table.source_url == source_url)
            .map(table_breadcrumb_from_record)
            .unwrap_or_else(|| Self::fallback_table_breadcrumb(source_url));
        self.select
            .table_breadcrumb_cache
            .borrow_mut()
            .insert(source_url.to_string(), breadcrumb.clone());
        breadcrumb
    }

    /// 難易度表のパンくず表示名。テーブルが既知なら表名、
    /// 不明なら URL のファイル名部分にフォールバックする。
    fn table_breadcrumb_name(&self, source_url: &str) -> String {
        self.table_breadcrumb(source_url).name
    }

    fn table_text_context_for_chart(&self, chart_id: i64) -> DifficultyTableText {
        if let Some(table_text) = self.select.select_items.iter().find_map(|item| match item {
            SelectItem::Chart(row)
                if row.chart.as_ref().is_some_and(|chart| chart.chart_id == chart_id) =>
            {
                row.table_text.is_table_song().then(|| row.table_text.clone())
            }
            _ => None,
        }) {
            return table_text;
        }
        let selected = self.select.select_items.get(self.select.selected_index);
        let source_hint = table_source_url_from_context(&self.select.folder_stack, selected);
        let source_order = table_source_order(&self.boot.app_config);

        let chart = self
            .select.select_items
            .iter()
            .find_map(|item| match item {
                SelectItem::Chart(row)
                    if row.chart.as_ref().is_some_and(|chart| chart.chart_id == chart_id) =>
                {
                    row.chart.clone()
                }
                _ => None,
            })
            .or_else(|| {
                self.boot
                    .library_db
                    .list_charts_by_ids(&[chart_id])
                    .map_err(|error| {
                        tracing::warn!(%error, chart_id, "failed to load chart for table skin text");
                        error
                    })
                    .ok()
                    .and_then(|mut charts| charts.pop())
            });

        let Some(chart) = chart else {
            return DifficultyTableText::default();
        };

        difficulty_table_text_for_chart_with_active_sources(
            &self.boot.library_db,
            &chart,
            &source_order,
            source_hint.as_deref(),
            Some(&source_order),
        )
        .map_err(|error| {
            tracing::warn!(%error, chart_id, "failed to resolve difficulty table skin text");
            error
        })
        .unwrap_or_default()
    }

    fn capture_play_table_text_for_chart(&mut self, chart_id: i64) {
        let (primary, secondary, fallback) = self.table_text_context_for_chart(chart_id).as_tuple();
        self.play.play_table_text_primary = primary;
        self.play.play_table_text_secondary = secondary;
        self.play.play_table_text_fallback = fallback;
    }

    fn apply_play_table_text(&self, snapshot: &mut RenderSnapshot) {
        snapshot.table_text_primary = self.play.play_table_text_primary.clone();
        snapshot.table_text_secondary = self.play.play_table_text_secondary.clone();
        snapshot.table_text_fallback = self.play.play_table_text_fallback.clone();
    }
}

fn should_bypass_analog_scratch_bounce(
    event: &crate::input::gamepad::GamepadButtonEvent,
    play_binding: Option<&LaneBinding>,
) -> bool {
    if !event.synthesized_analog_axis {
        return false;
    }
    let Some(play_binding) = play_binding else { return false };
    let control = PhysicalControl::GamepadButton(event.name.clone());
    play_binding
        .resolve_entry(event.device_id, &control)
        .is_some_and(|binding| matches!(binding.lane, Lane::Scratch | Lane::Scratch2))
}

fn should_play_select_bgm_on_enter(select_preview_playing: bool) -> bool {
    !select_preview_playing
}

fn system_bgm_stop_targets_on_scene_enter(
    scene_kind: AppSceneKind,
) -> &'static [crate::system_sound::SoundType] {
    use crate::system_sound::SoundType;
    match scene_kind {
        AppSceneKind::Play => &[SoundType::Select],
        AppSceneKind::Select | AppSceneKind::Decide | AppSceneKind::Result => {
            &[SoundType::Select, SoundType::Decide]
        }
    }
}

fn select_preview_fade_name(fade: SelectPreviewFade) -> &'static str {
    match fade {
        SelectPreviewFade::Silent => "silent",
        SelectPreviewFade::FadingIn { .. } => "fading_in",
        SelectPreviewFade::Playing => "playing",
        SelectPreviewFade::FadingOut { .. } => "fading_out",
    }
}

fn select_preview_key_after_delay(
    key: Option<String>,
    elapsed: Duration,
    delay: Duration,
) -> Option<String> {
    if elapsed >= delay { key } else { None }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioOutputIssueCause {
    StreamError,
    CallbackLockContention,
    CommandContention,
    GeneratedPreviewCpuPressure,
    CallbackDeadlineExceeded,
    MixClipping,
    Unknown,
}

impl AudioOutputIssueCause {
    fn as_str(self) -> &'static str {
        match self {
            Self::StreamError => "stream_error",
            Self::CallbackLockContention => "callback_lock_contention",
            Self::CommandContention => "command_contention",
            Self::GeneratedPreviewCpuPressure => "generated_preview_cpu_pressure",
            Self::CallbackDeadlineExceeded => "callback_deadline_exceeded",
            Self::MixClipping => "mix_clipping",
            Self::Unknown => "unknown",
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_audio_output_issue(
    stream_errors: u64,
    source_lock_misses: u64,
    engine_lock_misses: u64,
    command_drops: u64,
    _command_drain_lock_misses: u64,
    command_engine_lock_misses: u64,
    callback_over_budget: bool,
    clipped_samples: u64,
    generated_preview_loading: bool,
) -> AudioOutputIssueCause {
    if stream_errors != 0 {
        AudioOutputIssueCause::StreamError
    } else if source_lock_misses != 0 || engine_lock_misses != 0 {
        AudioOutputIssueCause::CallbackLockContention
    } else if command_drops != 0 || command_engine_lock_misses != 0 {
        AudioOutputIssueCause::CommandContention
    } else if callback_over_budget && generated_preview_loading {
        AudioOutputIssueCause::GeneratedPreviewCpuPressure
    } else if callback_over_budget {
        AudioOutputIssueCause::CallbackDeadlineExceeded
    } else if clipped_samples != 0 {
        AudioOutputIssueCause::MixClipping
    } else {
        AudioOutputIssueCause::Unknown
    }
}

fn select_preview_normalization_gain(enabled: bool, analyzed_gain: f32) -> f32 {
    if enabled && analyzed_gain.is_finite() { analyzed_gain.clamp(0.0, 1.0) } else { 1.0 }
}

fn should_use_generated_preview(preview_file: &str, explicit_preview_missing: bool) -> bool {
    preview_file.is_empty() || explicit_preview_missing
}

fn result_exit_audio_gain(elapsed: Duration, fadeout: Duration) -> f32 {
    let audio_fade = result_exit_audio_fade_duration(fadeout);
    if audio_fade.is_zero() {
        0.0
    } else {
        (1.0 - elapsed.as_secs_f32() / audio_fade.as_secs_f32()).clamp(0.0, 1.0)
    }
}

fn result_exit_audio_fade_duration(fadeout: Duration) -> Duration {
    fadeout.min(RESULT_EXIT_AUDIO_FADE)
}

fn duration_to_frames(duration: Duration, sample_rate: u32) -> u32 {
    if duration.is_zero() || sample_rate == 0 {
        return 0;
    }
    let frames = duration.as_secs_f64() * f64::from(sample_rate);
    frames.round().clamp(1.0, f64::from(u32::MAX)) as u32
}

fn result_exit_system_sounds() -> &'static [crate::system_sound::SoundType] {
    use crate::system_sound::SoundType;
    &[
        SoundType::ResultClear,
        SoundType::ResultFail,
        SoundType::ResultClose,
        SoundType::CourseClear,
        SoundType::CourseFail,
        SoundType::CourseClose,
    ]
}

fn result_entry_sound_for_clear(
    clear: bmz_core::clear::ClearType,
) -> crate::system_sound::SoundType {
    use crate::system_sound::SoundType;
    if matches!(clear, bmz_core::clear::ClearType::Failed) {
        SoundType::ResultFail
    } else {
        SoundType::ResultClear
    }
}

fn result_entry_clear_type_for_sound(finished: &FinishedPlaySession) -> bmz_core::clear::ClearType {
    finished.result.clear_type
}

fn course_result_entry_sound_for_clear(
    clear: bmz_core::clear::ClearType,
) -> crate::system_sound::SoundType {
    use crate::system_sound::SoundType;
    if matches!(clear, bmz_core::clear::ClearType::Failed) {
        SoundType::CourseFail
    } else {
        SoundType::CourseClear
    }
}

fn result_exit_sound_for_context(
    is_course_result: bool,
    course_close_available: bool,
) -> crate::system_sound::SoundType {
    use crate::system_sound::SoundType;

    if is_course_result && course_close_available {
        SoundType::CourseClose
    } else {
        SoundType::ResultClose
    }
}

fn should_route_settings_key_event(
    state: ElementState,
    repeat: bool,
    settings_editing: bool,
) -> bool {
    state == ElementState::Pressed && (settings_editing || !repeat)
}

fn settings_browse_move_control(
    control: &str,
    bindings: &SettingsBindings,
    select_bindings: &SelectKeyBindings,
) -> Option<SelectMove> {
    match control {
        "ArrowUp" | "DPadUp" | "ScratchUp" => Some(SelectMove::Previous),
        "ArrowDown" | "DPadDown" | "ScratchDown" => Some(SelectMove::Next),
        _ if select_bindings.is_select_scratch_up(control) => Some(SelectMove::Previous),
        _ if select_bindings.is_select_scratch_down(control) => Some(SelectMove::Next),
        _ if bindings.is_increase(control) => Some(SelectMove::Next),
        _ if bindings.is_decrease(control) => Some(SelectMove::Previous),
        _ => None,
    }
}

fn settings_edit_direction_from_analog_scroll(mov: i32) -> i32 {
    mov.signum()
}

fn settings_edit_direction_from_mouse_wheel(delta: MouseScrollDelta) -> i32 {
    mouse_wheel_y(delta).signum() as i32
}

fn system_sound_manager_from_boot(
    boot: &BootstrappedApp,
    audio: &crate::audio::SystemAudio,
) -> crate::system_sound_manager::SystemSoundManager {
    let cfg = &boot.profile_config.system_sound;
    let bgm_candidates = if cfg.bgm_dir.is_empty() {
        Vec::new()
    } else {
        crate::system_sound::scan_sound_sets(
            Path::new(&cfg.bgm_dir),
            crate::system_sound::SoundType::Select.file_name(),
        )
    };
    let se_candidates = if cfg.se_dir.is_empty() {
        Vec::new()
    } else {
        crate::system_sound::scan_sound_sets(
            Path::new(&cfg.se_dir),
            crate::system_sound::SoundType::ResultClear.file_name(),
        )
    };
    let default_dir = if cfg.default_sound_dir.is_empty() {
        None
    } else {
        Some(PathBuf::from(&cfg.default_sound_dir))
    };
    let selection =
        crate::system_sound::select_random_sound_set(&bgm_candidates, &se_candidates, default_dir);
    crate::system_sound_manager::SystemSoundManager::new(audio.engine(), &selection)
}

fn system_sound_volume_from_mix(
    mix: &crate::config::profile_config::AudioMixConfig,
    sound_type: crate::system_sound::SoundType,
) -> f32 {
    let unit = if sound_type.is_bgm() { mix.system_bgm_volume } else { mix.system_se_volume };
    let volume = crate::config::play::volume_unit_to_f32(mix.master_volume)
        * crate::config::play::volume_unit_to_f32(unit);
    volume.clamp(0.0, 1.0)
}

fn window_attributes_from_config(
    video: &crate::config::app_config::VideoConfig,
) -> WindowAttributes {
    WindowAttributes::default()
        .with_title("bmz-player")
        .with_window_icon(app_window_icon())
        .with_inner_size(PhysicalSize::new(video.width.max(1), video.height.max(1)))
}

fn app_window_icon() -> Option<Icon> {
    let image = image::load_from_memory(app_window_icon_png()).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).ok()
}

#[cfg(target_os = "windows")]
fn app_window_icon_png() -> &'static [u8] {
    include_bytes!("../../../assets/app-icon/bmz-player-window-windows.png")
}

#[cfg(not(target_os = "windows"))]
fn app_window_icon_png() -> &'static [u8] {
    include_bytes!("../../../assets/app-icon/bmz-player-window.png")
}

/// 設定のウィンドウモードに対応する winit の `Fullscreen` を返す。
///
/// 排他フルスクリーンはモニタの video mode が必要で、取得できない場合は
/// ボーダレスへフォールバックする。
fn fullscreen_from_config(mode: &WindowMode, monitor: Option<MonitorHandle>) -> Option<Fullscreen> {
    match mode {
        WindowMode::Windowed => None,
        WindowMode::BorderlessFullscreen => Some(Fullscreen::Borderless(monitor)),
        WindowMode::ExclusiveFullscreen => {
            let monitor = monitor?;
            match pick_exclusive_video_mode(&monitor) {
                Some(video_mode) => Some(Fullscreen::Exclusive(video_mode)),
                None => {
                    tracing::warn!("no exclusive video mode available; using borderless");
                    Some(Fullscreen::Borderless(Some(monitor)))
                }
            }
        }
    }
}

/// 排他フルスクリーン用に、解像度とリフレッシュレートが最大の video mode を選ぶ。
fn pick_exclusive_video_mode(monitor: &MonitorHandle) -> Option<VideoModeHandle> {
    monitor.video_modes().max_by_key(|mode| {
        let size = mode.size();
        (u64::from(size.width) * u64::from(size.height), mode.refresh_rate_millihertz())
    })
}

fn format_error_chain(error: &anyhow::Error) -> String {
    error.chain().map(ToString::to_string).collect::<Vec<_>>().join(": ")
}

fn open_external_url(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .context("failed to open URL with cmd /C start")?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn().context("failed to open URL with open")?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(url).spawn().context("failed to open URL with xdg-open")?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        anyhow::bail!("opening URLs is not supported on this platform: {url}");
    }
    Ok(())
}

fn open_file_browser_path(path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let target = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent().unwrap_or(path).to_path_buf()
        };
        Command::new("explorer")
            .arg(target)
            .spawn()
            .context("failed to open path with explorer")?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn().context("failed to open path with open")?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let target = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent().unwrap_or(path).to_path_buf()
        };
        Command::new("xdg-open")
            .arg(target)
            .spawn()
            .context("failed to open path with xdg-open")?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        anyhow::bail!("opening paths is not supported on this platform: {}", path.display());
    }
    Ok(())
}

fn open_file_with_default_app(path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn()
            .context("failed to open file with cmd /C start")?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn().context("failed to open file with open")?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn().context("failed to open file with xdg-open")?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        anyhow::bail!("opening files is not supported on this platform: {}", path.display());
    }
    Ok(())
}

fn primary_ir_provider_for_profile(
    profile: &ProfileConfig,
) -> Option<&crate::config::profile_config::IrProviderConfig> {
    let key = if profile.ir.primary_provider.trim().is_empty() {
        profile
            .ir
            .providers
            .iter()
            .find(|provider| {
                provider.enabled
                    && !provider.base_url.trim().is_empty()
                    && crate::ir::provider_key::configured_provider_key(provider).is_some()
            })
            .and_then(crate::ir::provider_key::configured_provider_key)
    } else {
        Some(profile.ir.primary_provider.trim())
    }?;
    crate::ir::provider_key::provider_config_for_key(&profile.ir, key)
}

fn launch_update_installer(path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new(path)
            .arg("/SP-")
            .spawn()
            .with_context(|| format!("failed to launch update installer: {}", path.display()))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!(
            "automatic installer launch is only supported on Windows: {}",
            path.display()
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyboardInputBackend {
    Window,
    RawInput,
}

fn play_input_backend_for_context(
    active: Option<&SharedInputBackend>,
    pending_start: bool,
    preloaded: Option<&SharedInputBackend>,
    pending_preload: Option<&SharedInputBackend>,
) -> Option<SharedInputBackend> {
    if let Some(active) = active {
        return Some(active.clone());
    }
    if !pending_start {
        return None;
    }
    preloaded.or(pending_preload).cloned()
}

fn keyboard_input_backend_for_config(config: &AppConfig) -> Option<KeyboardInputBackend> {
    if !config.input.keyboard_enabled {
        return None;
    }
    match config.input.backend {
        InputBackendKind::Auto if cfg!(target_os = "windows") => {
            Some(KeyboardInputBackend::RawInput)
        }
        InputBackendKind::RawInput if cfg!(target_os = "windows") => {
            Some(KeyboardInputBackend::RawInput)
        }
        _ => Some(KeyboardInputBackend::Window),
    }
}

fn config_renderer_backend(
    backend: crate::config::app_config::RendererBackend,
) -> bmz_render::WgpuBackend {
    match backend {
        crate::config::app_config::RendererBackend::Auto => bmz_render::WgpuBackend::Auto,
        crate::config::app_config::RendererBackend::Vulkan => bmz_render::WgpuBackend::Vulkan,
        crate::config::app_config::RendererBackend::Metal => bmz_render::WgpuBackend::Metal,
        crate::config::app_config::RendererBackend::Dx12 => bmz_render::WgpuBackend::Dx12,
        crate::config::app_config::RendererBackend::Gl => bmz_render::WgpuBackend::Gl,
    }
}

fn config_present_mode(
    video: &crate::config::app_config::VideoConfig,
) -> bmz_render::WgpuPresentMode {
    match video.vsync_mode {
        crate::config::app_config::VsyncModeConfig::Vsync => bmz_render::WgpuPresentMode::Fifo,
        crate::config::app_config::VsyncModeConfig::AdaptiveVsync => {
            bmz_render::WgpuPresentMode::FifoRelaxed
        }
        crate::config::app_config::VsyncModeConfig::VsyncOff => {
            bmz_render::WgpuPresentMode::Immediate
        }
        crate::config::app_config::VsyncModeConfig::FastVsync => {
            bmz_render::WgpuPresentMode::Mailbox
        }
    }
}

fn config_internal_resolution_mode(
    video: &crate::config::app_config::VideoConfig,
) -> bmz_render::InternalResolutionMode {
    match video.internal_resolution {
        InternalResolutionModeConfig::Native => bmz_render::InternalResolutionMode::Native,
        InternalResolutionModeConfig::Skin => bmz_render::InternalResolutionMode::Skin,
    }
}

impl ApplicationHandler<AppUserEvent> for WinitApp {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Init {
            tracing::info!("winit app init");
            self.ensure_window(event_loop);
        } else if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            // `WaitUntil` の deadline 到達時だけ描画を要求する。待機中に届いた
            // keyboard/device/user event は redraw を発生させず、その場で処理できる。
            self.request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        tracing::info!("winit app resumed");
        self.ensure_window(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window.as_ref().map(|window| window.id()) != Some(window_id) {
            return;
        }

        // すべてのウィンドウイベントを egui へ供給する。RedrawRequested など
        // egui が関知しないイベントは egui_winit 側で無視される。
        let practice_overlay = self
            .play
            .practice_session
            .as_ref()
            .is_some_and(|practice| practice.phase == PracticePhase::Config);
        let egui_consumed = match (self.window.clone(), self.ui.egui.as_mut()) {
            (Some(window), Some(egui)) => egui.on_window_event(&window, &event, practice_overlay),
            _ => false,
        };

        match event {
            WindowEvent::CloseRequested => {
                self.save_configs_for_exit(self.active_hispeed(), "game exit");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // F1 で egui メニューを開閉する。
                if event.physical_key == PhysicalKey::Code(KeyCode::F1)
                    && event.state == ElementState::Pressed
                    && !event.repeat
                {
                    if let Some(egui) = self.ui.egui.as_mut() {
                        egui.toggle();
                    }
                    return;
                }
                if event.physical_key == PhysicalKey::Code(KeyCode::F12)
                    && event.state == ElementState::Pressed
                    && !event.repeat
                {
                    self.request_manual_screenshot();
                    return;
                }
                // egui がフォーカスを持つ間はゲーム入力へ伝播させない。
                if egui_consumed {
                    return;
                }
                self.route_keyboard_input(&event);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.ui.last_cursor_action_at = Instant::now();
                if !self.ui.cursor_visible {
                    if let Some(window) = &self.window {
                        window.set_cursor_visible(true);
                    }
                    self.ui.cursor_visible = true;
                }
                if egui_consumed {
                    return;
                }
                self.route_mouse_wheel(delta);
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.select.last_cursor_position = Some(position);
                self.ui.last_cursor_action_at = Instant::now();
                if !self.ui.cursor_visible {
                    if let Some(window) = &self.window {
                        window.set_cursor_visible(true);
                    }
                    self.ui.cursor_visible = true;
                }
                if !egui_consumed {
                    self.route_select_slider_drag();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.ui.last_cursor_action_at = Instant::now();
                if !self.ui.cursor_visible {
                    if let Some(window) = &self.window {
                        window.set_cursor_visible(true);
                    }
                    self.ui.cursor_visible = true;
                }
                if egui_consumed {
                    return;
                }
                self.route_mouse_input(state, button);
            }
            WindowEvent::Ime(ime) => {
                if egui_consumed {
                    return;
                }
                self.route_ime_event(&ime);
            }
            WindowEvent::Resized(size) => {
                self.renderer
                    .resize_surface(SurfaceSize { width: size.width, height: size.height });
                // 検索モード中はリサイズに合わせて IME 候補ウィンドウ位置を再計算する。
                self.update_search_ime_cursor_area();
            }
            WindowEvent::Focused(focused) => {
                self.ui.focused = focused;
                if !focused {
                    let releases = self.input.handle_focus_lost();
                    for event in releases.raw_keyboard {
                        self.route_play_device_input(event);
                    }
                    for event in releases.window_keyboard {
                        self.route_play_device_input(event);
                    }
                    self.sync_select_holds_from_pressed_controls();
                    self.clear_select_hold();
                    self.reset_select_analog_scroll();
                    self.reset_play_analog_scroll();
                    self.clear_play_control_holds();
                }
            }
            WindowEvent::RedrawRequested => {
                let limit_start = Instant::now();
                if !self.begin_scheduled_frame(event_loop) {
                    return;
                }
                let limit_us = instant_elapsed_us_u64(limit_start);
                let redraw_started_at = Instant::now();
                let scene_before = self.current_scene_kind();
                let pending_skin_before = self.has_pending_skin_reload();
                let render_probe_before = self.skin.pending_skin_render_probe.is_some();
                let cursor_start = Instant::now();
                if self.ui.cursor_visible
                    && self.ui.last_cursor_action_at.elapsed() >= Duration::from_secs(2)
                {
                    if let Some(window) = &self.window {
                        window.set_cursor_visible(false);
                    }
                    self.ui.cursor_visible = false;
                }
                let cursor_us = instant_elapsed_us_u64(cursor_start);
                // Worker completion should be applied before intentional frame pacing sleep;
                // otherwise reload latency includes the frame limiter wait.
                let drain_start = Instant::now();
                let skin_drain_stats = self.drain_pending_skins();
                let drain_us = instant_elapsed_us_u64(drain_start);
                let input_start = Instant::now();
                self.poll_gamepad_events();
                self.advance_select_hold_move();
                self.advance_select_analog_scroll();
                let input_us = instant_elapsed_us_u64(input_start);
                let background_start = Instant::now();
                self.poll_chart_bga_texture_load();
                self.poll_play_preload();
                self.refresh_play_target_from_source();
                self.poll_pending_table_fetch();
                self.poll_pending_rian_table_fetch();
                self.maybe_start_periodic_rian_table_fetch();
                self.poll_pending_chart_download();
                self.poll_pending_song_scan();
                self.poll_pending_update_check();
                self.poll_pending_update_download();
                let background_us = instant_elapsed_us_u64(background_start);
                let transition_start = Instant::now();
                self.advance_decide_transition();
                self.advance_play_ending();
                self.advance_result_exit();
                let transition_us = instant_elapsed_us_u64(transition_start);
                let egui_start = Instant::now();
                self.run_egui_frame();
                let egui_us = instant_elapsed_us_u64(egui_start);
                if !self.first_frame_startup_completed {
                    self.ensure_audio_output();
                }
                let advance_active_play_start = Instant::now();
                self.advance_active_play();
                let advance_active_play_us = instant_elapsed_us_u64(advance_active_play_start);
                self.log_input_diagnostics();
                let scene_start = Instant::now();
                let scene_profile = self.render_current_scene();
                let scene_us = instant_elapsed_us_u64(scene_start);
                let post_scene_start = Instant::now();
                if !self.first_frame_startup_completed {
                    self.first_frame_startup_completed = true;
                    self.start_startup_table_fetch_after_first_frame();
                    self.start_deferred_boot();
                    if self.current_scene_kind() == AppSceneKind::Result {
                        self.ensure_result_skin_ready(self.current_result_skin_slot());
                    }
                    // render_current_scene() が既に last_scene_kind を更新済み。
                    // None に戻すと次フレームの start_scene_timers_before_snapshot が
                    // result_scene_started_at を再初期化し、動画 decode 時計が巻き戻って
                    // clocked decode thread が古い loop_base で待ち続けることがある。
                }
                self.advance_draining_audio();
                if let Some(runtime) = &self.audio.audio_runtime {
                    // chart sample bank を保持する source の破棄は、CPAL callback
                    // ではなく app thread 側で回収する。
                    runtime.reap_retired_sources();
                }
                self.log_audio_diagnostics();
                let post_scene_us = instant_elapsed_us_u64(post_scene_start);
                let total_us = instant_elapsed_us_u64(redraw_started_at);
                if let Some(sample) = scene_profile {
                    let play_loop =
                        (sample.kind == FrameProfileKind::Play).then_some(PlayLoopFrameTimings {
                            total_redraw_us: total_us,
                            input_us,
                            background_us,
                            transition_us,
                            egui_us,
                            advance_active_play_us,
                            post_scene_us,
                        });
                    self.frame.record_profile(sample, play_loop);
                }
                let pending_skin_after = self.has_pending_skin_reload();
                if skin_drain_stats.received_count > 0
                    || render_probe_before
                    || (pending_skin_before
                        && total_us >= duration_us_u64(SKIN_RELOAD_REDRAW_PROFILE_THRESHOLD))
                {
                    tracing::debug!(
                        scene_before = ?scene_before,
                        scene_after = ?self.current_scene_kind(),
                        pending_before = pending_skin_before,
                        pending_after = pending_skin_after,
                        render_probe_before,
                        received_uploads = skin_drain_stats.received_count,
                        applied_uploads = skin_drain_stats.applied_count,
                        max_upload_wait_us = skin_drain_stats.max_upload_wait_us,
                        total_us,
                        cursor_us,
                        drain_us,
                        limit_us,
                        input_us,
                        background_us,
                        transition_us,
                        egui_us,
                        scene_us,
                        post_scene_us,
                        "skin reload redraw timings"
                    );
                }
                if self.should_exit_via_select_hold() {
                    tracing::info!("escape held for 2s on select screen; exiting app");
                    self.save_configs_for_exit(self.active_hispeed(), "select exit hold");
                    event_loop.exit();
                    return;
                }
                self.handle_smoke_exit_after_redraw(event_loop);
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::Key(raw) = event {
            self.route_raw_keyboard_gameplay_input(raw.physical_key, raw.state);
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppUserEvent) {
        match event {
            AppUserEvent::SkinUploadReady { sent_at } => {
                let event_received_at = Instant::now();
                let pending_before = self.has_pending_skin_reload();
                let drain_start = Instant::now();
                let skin_drain_stats = self.drain_pending_skins();
                let drain_us = instant_elapsed_us_u64(drain_start);
                self.request_redraw();
                tracing::debug!(
                    event_delay_us = instant_duration_us_u64(sent_at, event_received_at),
                    pending_before,
                    pending_after = self.has_pending_skin_reload(),
                    received_uploads = skin_drain_stats.received_count,
                    applied_uploads = skin_drain_stats.applied_count,
                    max_upload_wait_us = skin_drain_stats.max_upload_wait_us,
                    drain_us,
                    "skin upload ready event timings"
                );
            }
            AppUserEvent::TableFetchReady => {
                self.poll_pending_table_fetch();
                self.poll_pending_rian_table_fetch();
                self.request_redraw();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let pending_before = self.has_pending_skin_reload();
        if pending_before {
            let drain_start = Instant::now();
            let skin_drain_stats = self.drain_pending_skins();
            let drain_us = instant_elapsed_us_u64(drain_start);
            if skin_drain_stats.received_count > 0 {
                self.request_redraw();
            }
            if skin_drain_stats.received_count > 0
                || drain_us >= duration_us_u64(SKIN_RELOAD_REDRAW_PROFILE_THRESHOLD)
            {
                tracing::debug!(
                    pending_before,
                    pending_after = self.has_pending_skin_reload(),
                    received_uploads = skin_drain_stats.received_count,
                    applied_uploads = skin_drain_stats.applied_count,
                    max_upload_wait_us = skin_drain_stats.max_upload_wait_us,
                    drain_us,
                    "skin reload about_to_wait timings"
                );
            }
        }
        if self.shutdown_requested.load(Ordering::SeqCst) {
            tracing::info!("Ctrl-C received; exiting cleanly");
            event_loop.exit();
            return;
        }
        self.schedule_next_frame(event_loop);
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(handle) = self.integrations.discord_presence.take() {
            handle.shutdown();
        }
        self.flush_pending_screenshots("app exit");
        self.save_configs_for_exit(self.active_hispeed(), "game exit");
        if self.release_audio_for_process_exit() {
            std::process::exit(0);
        }
        // Linux の winit/wgpu backend では Window より後に Surface を drop すると
        // native 側で落ちることがあるため、Window を保持したまま GPU 資源を解放する。
        self.ui.egui = None;
        if let Ok(mut cache) = self.skin.skin_pipeline.gpu_texture_cache.lock() {
            cache.clear();
        }
        self.renderer.detach_surface();
    }
}

impl WinitApp {
    fn release_audio_for_process_exit(&mut self) -> bool {
        if self.audio.audio_runtime.as_ref().is_some_and(AudioRuntime::uses_pulseaudio_host) {
            // cpal 0.18 の PulseAudio backend は stream Drop 時に pulseaudio crate の
            // reactor 切断と stream delete が重なり、終了時に native 側で abort する
            // 環境がある。プロセス終了直前だけ handle を残し、通常の drop cascade
            // に戻らずプロセスを終了する。
            if let Some(audio) = self.audio.draining_audio.take() {
                std::mem::forget(audio);
            }
            if let Some(active_play) = self.play.active_play.take() {
                std::mem::forget(active_play);
            }
            if let Some(system_audio) = self.audio.system_audio.take() {
                std::mem::forget(system_audio);
            }
            if let Some(runtime) = self.audio.audio_runtime.take() {
                std::mem::forget(runtime);
            }
            tracing::debug!("exiting process directly after PulseAudio output workaround");
            return true;
        }

        // プロセス終了前に音声出力を確実に Drop し、ASIO の停止・後処理を走らせる。
        self.audio.draining_audio = None;
        self.play.active_play = None;
        self.audio.system_audio = None;
        self.audio.audio_runtime = None;
        false
    }
}

fn surface_size_for_window(window: &Window) -> SurfaceSize {
    let size = window.inner_size();
    SurfaceSize { width: size.width, height: size.height }
}

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn now_unix_millis() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|duration| duration.as_millis()).unwrap_or(0)
}

fn next_screenshot_path(config_dir: &str, data_dir: &Path) -> PathBuf {
    let dir = screenshot_dir(config_dir, data_dir);
    let stamp = now_unix_millis();
    for index in 0..1000 {
        let file_name = if index == 0 {
            format!("bmz-screenshot-{stamp}.png")
        } else {
            format!("bmz-screenshot-{stamp}-{index}.png")
        };
        let path = dir.join(file_name);
        if !path.exists() {
            return path;
        }
    }
    dir.join(format!("bmz-screenshot-{stamp}-overflow.png"))
}

/// 左上オーバーレイ文字列を決める。
///
/// 撮影フレーム (`hide_toast`) ではトーストを隠し、連続撮影時の写り込みを防ぐ。
fn resolve_left_overlay_text(
    hide_toast: bool,
    toast: Option<(&str, Duration)>,
    fallback: &str,
) -> String {
    if !hide_toast
        && let Some((message, age)) = toast
        && age < LEFT_OVERLAY_TOAST_DURATION
        && !message.is_empty()
    {
        return message.to_string();
    }
    fallback.to_string()
}

fn pack_scan_progress(progress: ScanProgress) -> u64 {
    (u64::from(progress.done) << 32) | u64::from(progress.total)
}

fn unpack_scan_progress(packed: u64) -> ScanProgress {
    ScanProgress { done: (packed >> 32) as u32, total: packed as u32 }
}

fn screenshot_dir(config_dir: &str, data_dir: &Path) -> PathBuf {
    let trimmed = config_dir.trim();
    let path = if trimmed.is_empty() {
        PathBuf::from(crate::config::app_config::default_screenshot_dir())
    } else {
        PathBuf::from(trimmed)
    };
    if path.is_absolute() {
        return path;
    }
    if let Some(relative) = screenshot_dir_legacy_data_relative(&path) {
        data_dir.join(relative)
    } else {
        data_dir.join(path)
    }
}

fn screenshot_dir_legacy_data_relative(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    match components.next()? {
        std::path::Component::Normal(part) if part == std::ffi::OsStr::new("data") => {
            Some(components.as_path().to_path_buf())
        }
        _ => None,
    }
}

fn deferred_boot_action(boot_chart_id: Option<i64>, options: &AppOptions) -> Option<DeferredBoot> {
    if let Some(chart_id) = boot_chart_id {
        if options.boot_practice {
            return Some(DeferredBoot::Practice {
                chart_id,
                start_time_ms: options.practice_start_ms,
                end_time_ms: options.practice_end_ms,
            });
        }
        return Some(DeferredBoot::Chart { chart_id, replay_slot: options.boot_replay_slot });
    }
    if let Some(path) = options.boot_replay_file.clone() {
        return Some(DeferredBoot::ReplayFile { path });
    }
    if let Some(course_id) = options.boot_course_replay_id {
        return Some(DeferredBoot::CourseReplay { course_id });
    }
    options.boot_course_id.map(|course_id| DeferredBoot::Course { course_id })
}

fn resolve_boot_chart_id(
    library_db: &crate::storage::library_db::LibraryDatabase,
    options: &AppOptions,
) -> Option<i64> {
    if let Some(path) = options.boot_play_path.as_deref() {
        return lookup_boot_chart_id(library_db, path);
    }
    if options.boot_play_sample {
        return library_db.chart_id_by_title(SAMPLE_PLAYABLE_TITLE).ok().flatten();
    }
    None
}

fn lookup_boot_chart_id(
    library_db: &crate::storage::library_db::LibraryDatabase,
    path: &str,
) -> Option<i64> {
    let path_obj = Path::new(path);
    if !path_obj.is_file() {
        tracing::warn!(path, "boot chart path not found; starting normally");
        return None;
    }
    match library_db.chart_id_by_chart_file_path(path_obj) {
        Ok(Some(chart_id)) => Some(chart_id),
        Ok(None) => {
            tracing::warn!(path, "boot chart path is not in library; starting normally");
            None
        }
        Err(error) => {
            tracing::error!(%error, path, "failed to resolve boot chart path; starting normally");
            None
        }
    }
}

fn log_startup_options(options: &AppOptions) {
    if let Some(path) = &options.boot_play_path {
        tracing::info!(boot_play_path = %path, "boot chart path specified");
    }
    if options.boot_result_sample {
        tracing::info!(arg = BOOT_RESULT_SAMPLE_ARG, "debug result boot enabled");
    }
    if options.autoplay_on_start {
        tracing::info!(arg = AUTOPLAY_ON_START_ARG, "autoplay enabled for started charts");
    }
    if let Some(frames) = options.smoke_exit_after_frames {
        tracing::info!(arg = SMOKE_EXIT_AFTER_FRAMES_ARG, frames, "smoke auto-exit enabled");
    }
    if let Some(frames) = options.smoke_exit_after_play_frames {
        tracing::info!(
            arg = SMOKE_EXIT_AFTER_PLAY_FRAMES_ARG,
            frames,
            "smoke play-frame auto-exit enabled"
        );
    }
    if let Some(frames) = options.smoke_exit_after_result_frames {
        tracing::info!(
            arg = SMOKE_EXIT_AFTER_RESULT_FRAMES_ARG,
            frames,
            "smoke result-frame auto-exit enabled"
        );
    }
    if options.smoke_exit_on_result {
        tracing::info!(arg = SMOKE_EXIT_ON_RESULT_ARG, "smoke auto-exit on result enabled");
    }
    if options.boot_practice {
        tracing::info!("practice mode enabled for boot chart");
    }
    if let Some(path) = &options.smoke_screenshot_path {
        tracing::info!(arg = SMOKE_SCREENSHOT_ARG, path, "smoke screenshot enabled");
    }
}

#[cfg(test)]
mod tests {
    use bmz_render::scene::SelectRowKind;
    use bmz_render::skin::default_skin_manifest;

    use crate::config::app_config::{AppConfig, PathEntry, VsyncModeConfig};
    use crate::config::profile_config::ProfileConfig;
    use crate::screens::select_model::{SelectChartRow, SelectCourseRow};
    use crate::skin_loader::default_skin_root;
    use crate::storage::score_db::BestScoreSummary;

    use super::*;

    #[test]
    fn winit_app_stack_size_stays_bounded() {
        let size = std::mem::size_of::<WinitApp>();
        assert!(size < 64 * 1024, "WinitApp is {size} bytes");
    }

    #[test]
    fn lua_runtime_offsets_keep_names_distinct_and_runtime_ids_last_wins() {
        let offsets = vec![
            SkinOffsetConfig {
                name: Some("First".to_string()),
                id: 42,
                x: 10,
                ..Default::default()
            },
            SkinOffsetConfig {
                name: Some("Second".to_string()),
                id: 42,
                x: 20,
                ..Default::default()
            },
        ];
        let state =
            lua_runtime_state_with_skin_offsets(bmz_skin::LuaLoadRuntimeState::default(), &offsets);

        assert_eq!(state.offset_values["First"].x, 10);
        assert_eq!(state.offset_values["Second"].x, 20);
        assert_eq!(state.offset_id_values[&42].x, 20);
    }

    #[test]
    fn result_skin_signature_changes_when_only_offset_changes() {
        let mut skin = crate::config::profile_config::SkinConfig::default();
        let before = result_skin_signature_for_config(
            &skin,
            ResultSkinSlot::Normal,
            bmz_skin::LuaLoadRuntimeState::default(),
        );
        skin.result_offsets.push(SkinOffsetConfig {
            name: Some("Mascot".to_string()),
            id: 90,
            x: 12,
            ..Default::default()
        });
        let after = result_skin_signature_for_config(
            &skin,
            ResultSkinSlot::Normal,
            bmz_skin::LuaLoadRuntimeState::default(),
        );

        assert_ne!(before, after);
        assert_eq!(after.4.offset_values["Mascot"].x, 12);
        assert_eq!(after.4.offset_id_values[&90].x, 12);
    }

    #[test]
    fn gpu_upload_channels_apply_backpressure_at_the_configured_capacity() {
        let (bga_tx, _bga_rx) = bounded_gpu_upload_channel::<u8>(MAX_PENDING_BGA_TEXTURE_UPLOADS);
        for value in 0..MAX_PENDING_BGA_TEXTURE_UPLOADS {
            bga_tx.try_send(value as u8).expect("BGA queue should accept its capacity");
        }
        assert!(matches!(bga_tx.try_send(255), Err(mpsc::TrySendError::Full(255))));

        let (skin_tx, _skin_rx) = bounded_gpu_upload_channel::<u8>(MAX_PENDING_SKIN_UPLOADS);
        for value in 0..MAX_PENDING_SKIN_UPLOADS {
            skin_tx.try_send(value as u8).expect("skin queue should accept its capacity");
        }
        assert!(matches!(skin_tx.try_send(255), Err(mpsc::TrySendError::Full(255))));
    }

    #[test]
    fn operating_time_is_applied_to_select_snapshot() {
        let mut scene = AppSceneSnapshot::Select(SelectSnapshot::default());

        apply_operating_time_ms_to_scene(&mut scene, 90_061_234);

        let AppSceneSnapshot::Select(snapshot) = scene else {
            panic!("expected select snapshot");
        };
        assert_eq!(snapshot.operating_time_ms, 90_061_234);
    }

    #[test]
    fn smoke_play_frame_counter_only_exits_at_the_requested_count() {
        assert_eq!(count_smoke_play_frame(0, 3), (1, false));
        assert_eq!(count_smoke_play_frame(2, 3), (3, true));
        assert_eq!(count_smoke_play_frame(u32::MAX, 1), (u32::MAX, true));
    }

    #[test]
    fn player_name_and_fps_are_applied_to_every_scene() {
        let mut scenes = [
            AppSceneSnapshot::Select(SelectSnapshot::default()),
            AppSceneSnapshot::Play(RenderSnapshot::default()),
            bmz_render::sample::sample_result_scene(),
        ];

        for scene in &mut scenes {
            apply_skin_runtime_info_to_scene(scene, "Test Player", 237);
            match scene {
                AppSceneSnapshot::Select(snapshot) => {
                    assert_eq!(snapshot.player_name, "Test Player");
                    assert_eq!(snapshot.current_fps, 237);
                }
                AppSceneSnapshot::Decide(snapshot) | AppSceneSnapshot::Play(snapshot) => {
                    assert_eq!(snapshot.player_name, "Test Player");
                    assert_eq!(snapshot.current_fps, 237);
                }
                AppSceneSnapshot::Result(snapshot) => {
                    assert_eq!(snapshot.player_name, "Test Player");
                    assert_eq!(snapshot.current_fps, 237);
                }
            }
        }
    }

    #[test]
    fn course_decide_title_override_does_not_replace_play_snapshot_title() {
        let transition = DecideTransition {
            chart_id: 1,
            options: PlayStartOptions::default(),
            started_at: Instant::now(),
            fadeout_started_at: None,
            cancel: false,
            snapshot: RenderSnapshot {
                title: "Song Title".to_string(),
                subtitle: "Song Subtitle".to_string(),
                ..RenderSnapshot::default()
            },
            title_override: Some(DecideTitleOverride {
                title: "Course Title".to_string(),
                subtitle: String::new(),
            }),
        };

        let decide_snapshot = transition.snapshot_for_render();

        assert_eq!(decide_snapshot.title, "Course Title");
        assert_eq!(decide_snapshot.subtitle, "");
        assert_eq!(transition.snapshot.title, "Song Title");
        assert_eq!(transition.snapshot.subtitle, "Song Subtitle");
    }

    #[test]
    fn course_play_snapshot_uses_fallback_metadata_when_chart_row_is_absent() {
        let mut chart = select_chart_row(7).chart.unwrap();
        chart.title = "Resolved Song".to_string();
        chart.subtitle = "Resolved Subtitle".to_string();
        let items = vec![SelectItem::Course(select_course_row(1, 1))];
        let (chart, best_ex_score) = chart_snapshot_metadata_for_chart(&items, 7, |chart_id| {
            assert_eq!(chart_id, 7);
            Some(chart)
        })
        .expect("library chart metadata");
        let mut snapshot = RenderSnapshot::default();

        apply_chart_metadata_to_snapshot(&mut snapshot, &chart, 123, best_ex_score);

        assert_eq!(snapshot.title, "Resolved Song");
        assert_eq!(snapshot.subtitle, "Resolved Subtitle");
        assert_eq!(snapshot.total_notes, 123);
        assert_eq!(snapshot.best_ex_score, None);
    }

    #[test]
    fn chart_snapshot_metadata_preserves_selected_chart_best_score() {
        let mut row = select_chart_row(7);
        row.best_score = Some(best_score_with_replay(456, "best.json"));
        let items = vec![SelectItem::Chart(row)];

        let (chart, best_ex_score) = chart_snapshot_metadata_for_chart(&items, 7, |_| {
            panic!("selected chart metadata should take priority")
        })
        .expect("selected chart metadata");

        assert_eq!(chart.title, "Title 7");
        assert_eq!(best_ex_score, Some(456));
    }

    #[test]
    fn active_play_visual_offset_sync_preserves_auto_adjusted_value() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);

        sync_active_play_visual_offset_to_profile(&mut profile, 1_000, true);

        assert_eq!(profile.judge.visual_offset_us, 1_000);
        assert_eq!(
            crate::config::play::play_offsets_from_profile(&profile).visual_offset_us,
            1_000
        );

        sync_active_play_visual_offset_to_profile(&mut profile, 2_000, false);
        assert_eq!(profile.judge.visual_offset_us, 1_000);
    }

    fn app_test_chart() -> bmz_chart::model::PlayableChart {
        bmz_chart::model::PlayableChart {
            identity: bmz_core::chart::ChartIdentity { file_md5: [0; 16], file_sha256: [0; 32] },
            metadata: bmz_chart::model::ChartMetadata {
                title: "app test".to_string(),
                initial_bpm: 120.0,
                total: Some(160.0),
                ..Default::default()
            },
            lane_notes: std::array::from_fn(|_| Vec::new()),
            long_notes: Vec::new(),
            bgm_events: Vec::new(),
            bga_events: Vec::new(),
            timing_events: Vec::new(),
            scroll_events: Vec::new(),
            speed_events: Vec::new(),
            judge_rank_events: Vec::new(),
            bgm_volume_events: Vec::new(),
            key_volume_events: Vec::new(),
            text_events: Vec::new(),
            bga_opacity_events: Vec::new(),
            bga_argb_events: Vec::new(),
            swbga_definitions: Vec::new(),
            bga_keybound_events: Vec::new(),
            bga_asset_by_bmp_key: std::collections::HashMap::new(),
            bar_lines: Vec::new(),
            sounds: Vec::new(),
            bga_assets: Vec::new(),
            total_notes: 0,
            end_time: TimeUs(0),
        }
    }

    #[test]
    fn skin_video_play_level_number_extracts_digits_without_allocating_label_shapes() {
        assert_eq!(skin_video_play_level_number("12"), 12);
        assert_eq!(skin_video_play_level_number("LV 10+"), 10);
        assert_eq!(skin_video_play_level_number("no level"), 0);
    }

    #[test]
    fn skin_video_difficulty_code_matches_numeric_and_case_insensitive_names() {
        assert_eq!(skin_video_difficulty_code("1"), 1);
        assert_eq!(skin_video_difficulty_code(" normal "), 2);
        assert_eq!(skin_video_difficulty_code("INSANE"), 5);
        assert_eq!(skin_video_difficulty_code("unknown"), 0);
    }

    #[test]
    fn table_breadcrumb_uses_table_name_without_symbol_prefix() {
        let breadcrumb = table_breadcrumb_from_record(&DifficultyTableRecord {
            id: 1,
            source_url: "https://example.com/insane/".to_string(),
            name: "通常難易度表".to_string(),
            symbol: "★".to_string(),
            level_order: vec!["1".to_string()],
            fetched_at: 0,
        });

        assert_eq!(breadcrumb.name, "通常難易度表");
        assert_eq!(breadcrumb.symbol, "★");
    }

    #[test]
    fn fallback_result_scene_uses_nonzero_duration() {
        assert_eq!(result_input_duration_for_document(None), Duration::ZERO);
        assert_eq!(result_scene_duration_for_document(None), FALLBACK_RESULT_SCENE_DURATION);
    }

    #[test]
    fn result_scene_duration_respects_skin_document() {
        let document: SkinDocument =
            serde_json::from_str(r#"{ "type": 7, "input": 1500, "scene": 2345 }"#).unwrap();

        assert_eq!(
            result_input_duration_for_document(Some(&document)),
            Duration::from_millis(1500)
        );
        assert_eq!(
            result_scene_duration_for_document(Some(&document)),
            Duration::from_millis(2345)
        );
    }

    #[test]
    fn normal_result_scene_zero_disables_auto_leave() {
        let document: SkinDocument =
            serde_json::from_str(r#"{ "type": 7, "input": 1500, "scene": 0 }"#).unwrap();

        assert_eq!(result_auto_exit_duration_for_document(Some(&document), false, false), None);
    }

    #[test]
    fn result_auto_exit_uses_scene_when_positive() {
        let document: SkinDocument =
            serde_json::from_str(r#"{ "type": 7, "scene": 2345 }"#).unwrap();

        assert_eq!(
            result_auto_exit_duration_for_document(Some(&document), false, false),
            Some(Duration::from_millis(2345))
        );
    }

    #[test]
    fn course_intermediate_result_waits_for_input_without_auto_advance() {
        let document: SkinDocument =
            serde_json::from_str(r#"{ "type": 7, "scene": 2345 }"#).unwrap();

        assert_eq!(result_auto_exit_duration_for_document(Some(&document), true, false), None);
    }

    #[test]
    fn boot_course_intermediate_result_falls_back_when_scene_is_zero() {
        let document: SkinDocument = serde_json::from_str(r#"{ "type": 7, "scene": 0 }"#).unwrap();

        assert_eq!(
            result_auto_exit_duration_for_document(Some(&document), true, true),
            Some(FALLBACK_RESULT_SCENE_DURATION)
        );
    }

    #[test]
    fn failed_play_ending_starts_failed_timer_without_finish_result() {
        let started_at = Instant::now();
        let ending = failed_play_ending(started_at);

        assert_eq!(ending.started_at, started_at);
        assert!(ending.failed);
        assert!(ending.finished.is_none());
        assert!(ending.fadeout_started_at.is_none());
        assert!(ending.full_combo_elapsed_at_finish_ms.is_none());
    }

    #[test]
    fn initial_folder_stack_starts_at_select_root_even_with_single_enabled_root() {
        let mut config = AppConfig::default();
        config.songs.roots =
            vec![PathEntry { path: "/music/bms".to_string(), enabled: true, recursive: true }];
        assert!(initial_folder_stack(&config).is_empty());
    }

    #[test]
    fn config_present_mode_maps_vsync_modes() {
        let mut config = AppConfig::default().video;

        config.vsync_mode = VsyncModeConfig::Vsync;
        assert_eq!(config_present_mode(&config), bmz_render::WgpuPresentMode::Fifo);

        config.vsync_mode = VsyncModeConfig::AdaptiveVsync;
        assert_eq!(config_present_mode(&config), bmz_render::WgpuPresentMode::FifoRelaxed);

        config.vsync_mode = VsyncModeConfig::VsyncOff;
        assert_eq!(config_present_mode(&config), bmz_render::WgpuPresentMode::Immediate);

        config.vsync_mode = VsyncModeConfig::FastVsync;
        assert_eq!(config_present_mode(&config), bmz_render::WgpuPresentMode::Mailbox);
    }

    #[test]
    fn config_internal_resolution_mode_maps_video_setting() {
        let mut config = AppConfig::default().video;

        config.internal_resolution = InternalResolutionModeConfig::Native;
        assert_eq!(
            config_internal_resolution_mode(&config),
            bmz_render::InternalResolutionMode::Native
        );

        config.internal_resolution = InternalResolutionModeConfig::Skin;
        assert_eq!(
            config_internal_resolution_mode(&config),
            bmz_render::InternalResolutionMode::Skin
        );
    }

    #[test]
    fn keyboard_input_backend_uses_raw_input_on_windows_auto() {
        let mut config = AppConfig::default();
        config.input.backend = InputBackendKind::Auto;
        let expected_auto = if cfg!(target_os = "windows") {
            KeyboardInputBackend::RawInput
        } else {
            KeyboardInputBackend::Window
        };
        assert_eq!(keyboard_input_backend_for_config(&config), Some(expected_auto));

        config.input.backend = InputBackendKind::Winit;
        assert_eq!(keyboard_input_backend_for_config(&config), Some(KeyboardInputBackend::Window));

        config.input.keyboard_enabled = false;
        assert_eq!(keyboard_input_backend_for_config(&config), None);
    }

    #[test]
    fn pending_play_uses_preload_input_before_session_install() {
        use bmz_core::input::InputKind;
        use bmz_gameplay::input::backend::InputBackend;

        let preload_input = SharedInputBackend::default();
        assert!(play_input_backend_for_context(None, false, None, Some(&preload_input)).is_none());

        let selected =
            play_input_backend_for_context(None, true, None, Some(&preload_input)).unwrap();
        crate::input::winit::handle_key_parts(
            &selected,
            PhysicalKey::Code(KeyCode::KeyZ),
            ElementState::Pressed,
            false,
        );

        let events = preload_input.clone().drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, InputKind::Press);
    }

    #[test]
    fn pending_play_input_updates_keybeam_before_session_install() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let binding = crate::config::play::lane_binding_for_chart_with_slots(
            &profile.input,
            KeyMode::K7,
            Default::default(),
        );
        let mut visual = PendingPlayVisualInput::new(KeyMode::K7, binding, false);
        let press = physical_key_to_device_input(
            PhysicalKey::Code(KeyCode::KeyZ),
            ElementState::Pressed,
            false,
        )
        .unwrap();

        visual.apply_event(&press, TimeUs(100_000));
        let mut snapshot = RenderSnapshot::default();
        crate::screens::play_snapshot::refresh_pending_play_input_visuals(
            &mut snapshot,
            visual.key_mode,
            visual.lane_keyon_started_at,
            visual.lane_keyoff_started_at,
            visual.lane_scratch_angle_delta_ms,
            TimeUs(150_000),
        );

        assert_eq!(snapshot.keyon_ms[Lane::Key1.index()], Some(50));
        assert_eq!(snapshot.keyoff_ms[Lane::Key1.index()], None);

        let release = physical_key_to_device_input(
            PhysicalKey::Code(KeyCode::KeyZ),
            ElementState::Released,
            false,
        )
        .unwrap();
        visual.apply_event(&release, TimeUs(160_000));
        crate::screens::play_snapshot::refresh_pending_play_input_visuals(
            &mut snapshot,
            visual.key_mode,
            visual.lane_keyon_started_at,
            visual.lane_keyoff_started_at,
            visual.lane_scratch_angle_delta_ms,
            TimeUs(175_000),
        );
        assert_eq!(snapshot.keyon_ms[Lane::Key1.index()], None);
        assert_eq!(snapshot.keyoff_ms[Lane::Key1.index()], Some(15));
    }

    #[test]
    fn pending_play_input_state_hands_off_without_resetting_keybeam_timer() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let binding = crate::config::play::lane_binding_for_chart_with_slots(
            &profile.input,
            KeyMode::K7,
            Default::default(),
        );
        let mut visual = PendingPlayVisualInput::new(KeyMode::K7, binding, false);
        let press = physical_key_to_device_input(
            PhysicalKey::Code(KeyCode::KeyZ),
            ElementState::Pressed,
            false,
        )
        .unwrap();
        visual.apply_event(&press, TimeUs(100_000));
        let input = SharedInputBackend::default();
        input.push_shared_event(press);
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(app_test_chart()),
            &profile,
            crate::screens::play_session::PlaySessionOptions::default(),
        );

        handoff_pending_play_visual_input(&mut session, &input, &visual);
        let mut snapshot =
            RenderSnapshot { play_elapsed_time: TimeUs(150_000), ..Default::default() };
        crate::screens::play_snapshot::refresh_play_skin_visuals_with_input_elapsed(
            &mut snapshot,
            &session,
            TimeUs(150_000),
        );

        assert_eq!(session.lane_keyon_started_at[Lane::Key1.index()], Some(TimeUs(100_000)));
        assert_eq!(snapshot.keyon_ms[Lane::Key1.index()], Some(50));
        assert!(input.clone().drain_events().is_empty());
    }

    #[test]
    fn pending_play_input_suppresses_human_keybeam_for_full_autoplay() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let binding = crate::config::play::lane_binding_for_chart_with_slots(
            &profile.input,
            KeyMode::K7,
            Default::default(),
        );
        let mut visual = PendingPlayVisualInput::new(KeyMode::K7, binding, true);
        let press = physical_key_to_device_input(
            PhysicalKey::Code(KeyCode::KeyZ),
            ElementState::Pressed,
            false,
        )
        .unwrap();

        visual.apply_event(&press, TimeUs(100_000));

        assert_eq!(visual.lane_keyon_started_at[Lane::Key1.index()], None);
    }

    #[test]
    fn default_skin_note_texture_exists() {
        assert!(default_skin_root().join("note.png").is_file());
        assert!(default_skin_root().join("note-blue.png").is_file());
        assert!(default_skin_root().join("note-red.png").is_file());
        assert!(default_skin_root().join("receptor.png").is_file());
        assert!(default_skin_root().join("receptor-blue.png").is_file());
        assert!(default_skin_root().join("receptor-red.png").is_file());
        assert!(default_skin_root().join("judge-line.png").is_file());
        assert!(default_skin_root().join("gauge-frame.png").is_file());
        assert!(default_skin_root().join("gauge-fill.png").is_file());
        assert!(default_skin_root().join("combo-panel.png").is_file());
        assert!(default_skin_root().join("combo-panel-inactive.png").is_file());
    }

    #[test]
    fn debug_boot_result_summary_has_stat_graph_data() {
        let finished = debug_boot_finished_play_session();
        let summary = &finished.summary;

        assert_eq!(summary.title, "Debug Result Boot [ANOTHER]");
        assert_eq!(summary.key_mode, KeyMode::K7);
        assert!(summary.ex_score > 0);
        assert!(!summary.graph.gauge_points.is_empty());
        assert!(!summary.graph.judge_graph_buckets.is_empty());
        assert!(!summary.graph.early_late_graph_buckets.is_empty());
        assert!(!summary.graph.timing_points.is_empty());
        assert!(summary.graph.timing_distribution.total() > 0);
    }

    #[test]
    fn result_lua_runtime_values_cover_load_time_result_decisions() {
        let mut summary = debug_boot_result_summary();
        let graph = Arc::make_mut(&mut summary.graph);
        graph.timing_distribution = bmz_render::snapshot::ResultTimingDistribution::new(150);
        graph.timing_distribution.add(-13);
        graph.timing_distribution.add(-12);

        let values = result_lua_runtime_number_values_for_summary(&summary);

        assert_eq!(values.get(&150), Some(&760));
        assert_eq!(values.get(&170), Some(&760));
        assert_eq!(values.get(&171), Some(&(summary.ex_score as i32)));
        assert_eq!(values.get(&121), Some(&1_056));
        assert_eq!(values.get(&151), Some(&1_056));
        assert_eq!(values.get(&152), Some(&((summary.ex_score as i32).saturating_sub(760))));
        assert_eq!(values.get(&153), Some(&((summary.ex_score as i32).saturating_sub(1_056))));
        assert_eq!(values.get(&370), Some(&(ClearType::Failed as i32)));
        assert_eq!(values.get(&371), Some(&(ClearType::Normal as i32)));
        assert_eq!(values.get(&374), Some(&-12));
        assert_eq!(values.get(&375), Some(&-50));
        assert_eq!(values.get(&410), Some(&128));
        assert_eq!(values.get(&422), Some(&2));
        assert_eq!(values.get(&423), Some(&46));
        assert_eq!(values.get(&424), Some(&104));
    }

    #[test]
    fn default_skin_texture_catalog_defines_expected_assets() {
        let manifest = default_skin_manifest();

        assert!(
            manifest.textures.iter().any(|texture| texture.id == 1 && texture.path == "note.png")
        );
        assert!(
            manifest
                .textures
                .iter()
                .any(|texture| texture.id == 2 && texture.path == "note-blue.png")
        );
        assert!(
            manifest
                .textures
                .iter()
                .any(|texture| texture.id == 3 && texture.path == "note-red.png")
        );
        assert!(
            manifest
                .textures
                .iter()
                .any(|texture| texture.id == 4 && texture.path == "receptor.png")
        );
        assert!(
            manifest
                .textures
                .iter()
                .any(|texture| texture.id == 5 && texture.path == "receptor-blue.png")
        );
        assert!(
            manifest
                .textures
                .iter()
                .any(|texture| texture.id == 6 && texture.path == "receptor-red.png")
        );
        assert!(
            manifest
                .textures
                .iter()
                .any(|texture| texture.id == 7 && texture.path == "judge-line.png")
        );
        assert!(
            manifest
                .textures
                .iter()
                .any(|texture| texture.id == 8 && texture.path == "gauge-frame.png")
        );
        assert!(
            manifest
                .textures
                .iter()
                .any(|texture| texture.id == 9 && texture.path == "gauge-fill.png")
        );
        assert!(
            manifest
                .textures
                .iter()
                .any(|texture| texture.id == 10 && texture.path == "combo-panel.png")
        );
        assert!(
            manifest
                .textures
                .iter()
                .any(|texture| texture.id == 11 && texture.path == "combo-panel-inactive.png")
        );
        assert!(
            manifest
                .textures
                .iter()
                .any(|texture| texture.id == 12 && texture.path == "note-mine.png")
        );
    }

    #[test]
    fn skin_catalog_scan_ignores_lua_parts_files() {
        assert!(is_skin_candidate_file(Path::new("data/skins/ECFN/play/play7.luaskin")));
        assert!(is_skin_candidate_file(Path::new("data/skins/ECFN/play/play7-1p.json")));
        assert!(is_skin_candidate_file(Path::new("data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin")));
        assert!(!is_skin_candidate_file(Path::new("data/skins/ECFN/play/play_parts.lua")));
    }

    #[test]
    fn lr2skin_header_document_exposes_skin_config_defs_when_available() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
        if !path.is_file() {
            return;
        }

        let document = load_skin_header_document(&path).expect("load lr2 skin header");

        assert!(document.property.iter().any(|property| property.name == "Displayjudge"));
        assert!(document.filepath.iter().any(|filepath| filepath.name == "GAUGE COLOR"));
        assert!(document.offset.iter().any(|offset| offset.id == 1));
    }

    #[test]
    fn skin_catalog_loads_rm_skin_lua_headers_when_available() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let skin_root = repo_root.join("data/skins");
        let root = skin_root.join("Rmz-skin");
        let cases = [
            ("play4main.luaskin", BMZ_SKIN_TYPE_PLAY_4KEYS),
            ("play5main.luaskin", 1),
            ("play6main.luaskin", BMZ_SKIN_TYPE_PLAY_6KEYS),
            ("play7main.luaskin", 0),
            ("play8main.luaskin", BMZ_SKIN_TYPE_PLAY_8KEYS),
            ("play9main.luaskin", 4),
        ];

        for (file_name, expected_type) in cases {
            let path = root.join(file_name);
            if !path.is_file() {
                continue;
            }

            let (skin_type, candidate) =
                load_skin_candidate(&skin_root, &path, SkinCandidateOrigin::Bundled)
                    .expect("load Rm-skin catalog candidate");

            assert_eq!(skin_type, expected_type, "{}", path.display());
            assert_eq!(candidate.path, format!("resource:skins/Rmz-skin/{file_name}"));
            assert_eq!(candidate.origin, SkinCandidateOrigin::Bundled);
            assert!(candidate.name.contains("Rm-skin"), "candidate name: {}", candidate.name);
        }
    }

    #[test]
    fn skin_catalog_loads_mz_select_lua_header_when_available() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let skin_root = repo_root.join("data/skins");
        let path = skin_root.join("mz-select/music_select.luaskin");
        if !path.is_file() {
            return;
        }

        let (skin_type, candidate) =
            load_skin_candidate(&skin_root, &path, SkinCandidateOrigin::Bundled)
                .expect("load mz-select catalog candidate");

        assert_eq!(skin_type, 5);
        assert_eq!(candidate.path, "resource:skins/mz-select/music_select.luaskin");
        assert_eq!(candidate.origin, SkinCandidateOrigin::Bundled);
        assert!(candidate.name.contains("m-select"), "candidate name: {}", candidate.name);
    }

    #[test]
    fn skin_catalog_loads_luxez_flat_select_lua_header_when_available() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let skin_root = repo_root.join("data/skins");
        let path = skin_root.join("Luxez-Flat/music_select.luaskin");
        if !path.is_file() {
            return;
        }

        let (skin_type, candidate) =
            load_skin_candidate(&skin_root, &path, SkinCandidateOrigin::Bundled)
                .expect("load Luxez-Flat catalog candidate");

        assert_eq!(skin_type, 5);
        assert_eq!(candidate.path, "resource:skins/Luxez-Flat/music_select.luaskin");
        assert_eq!(candidate.origin, SkinCandidateOrigin::Bundled);
        assert!(!candidate.name.trim().is_empty(), "candidate name should not be empty");
    }

    #[test]
    fn skin_catalog_maps_play_key_modes_by_exact_skin_type() {
        let mut catalog = SkinCatalog::default();
        push_skin_candidate(
            &mut catalog,
            0,
            SkinCandidate {
                name: "Seven".to_string(),
                path: "data/skins/example/play7.luaskin".to_string(),
                origin: SkinCandidateOrigin::User,
            },
        );
        push_skin_candidate(
            &mut catalog,
            1,
            SkinCandidate {
                name: "Five".to_string(),
                path: "data/skins/example/play5.luaskin".to_string(),
                origin: SkinCandidateOrigin::User,
            },
        );
        push_skin_candidate(
            &mut catalog,
            BMZ_SKIN_TYPE_PLAY_4KEYS,
            SkinCandidate {
                name: "Four".to_string(),
                path: "data/skins/example/play4.luaskin".to_string(),
                origin: SkinCandidateOrigin::User,
            },
        );
        push_skin_candidate(
            &mut catalog,
            BMZ_SKIN_TYPE_PLAY_6KEYS,
            SkinCandidate {
                name: "Six".to_string(),
                path: "data/skins/example/play6.luaskin".to_string(),
                origin: SkinCandidateOrigin::User,
            },
        );
        push_skin_candidate(
            &mut catalog,
            BMZ_SKIN_TYPE_PLAY_8KEYS,
            SkinCandidate {
                name: "Eight".to_string(),
                path: "data/skins/example/play8.luaskin".to_string(),
                origin: SkinCandidateOrigin::User,
            },
        );
        push_skin_candidate(
            &mut catalog,
            2,
            SkinCandidate {
                name: "Fourteen".to_string(),
                path: "data/skins/example/play14.luaskin".to_string(),
                origin: SkinCandidateOrigin::User,
            },
        );
        push_skin_candidate(
            &mut catalog,
            3,
            SkinCandidate {
                name: "Ten".to_string(),
                path: "data/skins/example/play10.luaskin".to_string(),
                origin: SkinCandidateOrigin::User,
            },
        );
        push_skin_candidate(
            &mut catalog,
            4,
            SkinCandidate {
                name: "Nine".to_string(),
                path: "data/skins/example/play9.luaskin".to_string(),
                origin: SkinCandidateOrigin::User,
            },
        );
        push_skin_candidate(
            &mut catalog,
            12,
            SkinCandidate {
                name: "Battle Seven".to_string(),
                path: "data/skins/example/battle7.lr2skin".to_string(),
                origin: SkinCandidateOrigin::User,
            },
        );
        push_skin_candidate(
            &mut catalog,
            13,
            SkinCandidate {
                name: "Battle Five".to_string(),
                path: "data/skins/example/battle5.lr2skin".to_string(),
                origin: SkinCandidateOrigin::User,
            },
        );
        push_skin_candidate(
            &mut catalog,
            15,
            SkinCandidate {
                name: "Course Result".to_string(),
                path: "data/skins/example/course-result.luaskin".to_string(),
                origin: SkinCandidateOrigin::User,
            },
        );

        assert_eq!(catalog.play4.len(), 1);
        assert_eq!(catalog.play5.len(), 1);
        assert_eq!(catalog.play6.len(), 1);
        assert_eq!(catalog.play7.len(), 1);
        assert_eq!(catalog.play8.len(), 1);
        assert_eq!(catalog.play9.len(), 1);
        assert_eq!(catalog.play10.len(), 1);
        assert_eq!(catalog.play14.len(), 1);
        assert_eq!(catalog.battle5.len(), 1);
        assert_eq!(catalog.battle7.len(), 1);
        assert_eq!(catalog.result.len(), 0);
        assert_eq!(catalog.course_result.len(), 1);
        assert_eq!(catalog.play4[0].path, "data/skins/example/play4.luaskin");
        assert_eq!(catalog.play5[0].path, "data/skins/example/play5.luaskin");
        assert_eq!(catalog.play6[0].path, "data/skins/example/play6.luaskin");
        assert_eq!(catalog.play7[0].path, "data/skins/example/play7.luaskin");
        assert_eq!(catalog.play8[0].path, "data/skins/example/play8.luaskin");
        assert_eq!(catalog.play9[0].path, "data/skins/example/play9.luaskin");
        assert_eq!(catalog.play10[0].path, "data/skins/example/play10.luaskin");
        assert_eq!(catalog.play14[0].path, "data/skins/example/play14.luaskin");
        assert_eq!(catalog.battle5[0].path, "data/skins/example/battle5.lr2skin");
        assert_eq!(catalog.battle7[0].path, "data/skins/example/battle7.lr2skin");
        assert_eq!(catalog.course_result[0].path, "data/skins/example/course-result.luaskin");
    }

    #[test]
    fn skin_catalog_loads_modern_chic_headers_when_available() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let skin_root = repo_root.join("data/skins");
        let root = skin_root.join("ModernChic");
        if !root.is_dir() {
            return;
        }
        let cases = [
            ("musicselect.luaskin", 5),
            ("decide.luaskin", 6),
            ("play5_hw.luaskin", 1),
            ("play7_hw.luaskin", 0),
            ("play10_hw.luaskin", 3),
            ("play14_hw.luaskin", 2),
            ("result.luaskin", 7),
            ("course.luaskin", 15),
        ];

        for (file_name, expected_type) in cases {
            let path = root.join(file_name);
            let loaded = bmz_skin::load_lua_skin_header_value(&path)
                .unwrap_or_else(|error| panic!("load {} header: {error:#}", path.display()));
            let document: SkinDocument = serde_json::from_value(loaded.value)
                .unwrap_or_else(|error| panic!("decode {} header: {error:#}", path.display()));
            assert_eq!(document.skin_type, expected_type, "{}", path.display());

            let (skin_type, candidate) =
                load_skin_candidate(&skin_root, &path, SkinCandidateOrigin::Bundled)
                    .unwrap_or_else(|| panic!("load {} catalog candidate", path.display()));
            assert_eq!(skin_type, expected_type, "{}", path.display());
            assert!(candidate.name.contains("ModernChic"), "candidate name: {}", candidate.name);
        }
    }

    #[test]
    fn course_result_summary_for_skin_uses_aggregate_course_values() {
        fn entry_summary(
            ex_score: u32,
            notes: u32,
            max_combo: u32,
            duration_ms: i32,
        ) -> ResultSummary {
            ResultSummary {
                clear_type: ClearType::NoPlay,
                target_name: "RANK AAA".to_string(),
                arrange: "NORMAL".to_string(),
                arrange_2p: "NORMAL".to_string(),
                lane_shuffle_pattern: Vec::new(),
                ex_score,
                max_combo,
                bp: 0,
                cb: 0,
                gauge_value: 80.0,
                gauge_type: GaugeType::Normal,
                total_notes: notes,
                duration_ms,
                initial_bpm: 128.0,
                min_bpm: 128.0,
                max_bpm: 128.0,
                main_bpm: 128.0,
                total_gauge: 260.0,
                judge_rank: Some(2),
                key_mode: KeyMode::K7,
                has_long_notes: false,
                long_note_mode: bmz_chart::model::LongNoteMode::Ln,
                judge_counts: crate::screens::result_model::ResultJudgeCounts {
                    pgreat: ex_score / 2,
                    ..Default::default()
                },
                fast_slow_counts: ResultFastSlowJudgeCounts {
                    fast_pgreat: ex_score / 2,
                    ..Default::default()
                },
                replay_path: String::new(),
                replay_slots: [false; 4],
                saved_replay_slots: [false; 4],
                score_history_id: 0,
                best_ex_score: None,
                best_clear_type: None,
                best_max_combo: None,
                best_bp: None,
                previous_best_ex_score: None,
                previous_best_clear_type: None,
                previous_best_max_combo: None,
                previous_best_bp: None,
                target_ex_score: Some(ex_score + 40),
                target_max_combo: None,
                target_bp: None,
                target_clear_type: None,
                ir_queued_jobs: 0,
                ir_last_error: None,
                title: String::new(),
                subtitle: String::new(),
                artist: String::new(),
                subartist: String::new(),
                genre: String::new(),
                difficulty_name: String::new(),
                play_level: String::new(),
                graph: Arc::new(bmz_render::snapshot::ResultGraphSnapshot {
                    gauge_points: vec![bmz_render::snapshot::ResultGaugeGraphPoint {
                        time_ms: duration_ms,
                        value: 80.0,
                        max: 100.0,
                        border: 20.0,
                        gauge_type: GaugeType::Normal as i32,
                    }],
                    timing_points: vec![bmz_render::snapshot::ResultTimingPoint {
                        time_ms: duration_ms,
                        delta_us: i64::from(duration_ms),
                        judge: bmz_core::judge::Judge::PGreat,
                    }],
                    judge_graph_density: vec![notes as u8],
                    bpm_graph_segments: vec![bmz_render::snapshot::BpmGraphSegment {
                        start_ratio: 0.0,
                        end_ratio: 1.0,
                        bpm: 120.0 + duration_ms as f32,
                        is_stop: false,
                    }],
                    ..Default::default()
                }),
            }
        }

        let mut course = CourseResultSummary {
            course_id: 1,
            course_score_id: None,
            course_played_at: None,
            rule_mode: bmz_gameplay::rule::RuleMode::Beatoraja,
            title: "Course Title".to_string(),
            kind: bmz_core::course::CourseKind::Dan,
            course_titles: [
                "Stage 1".to_string(),
                "Stage 2".to_string(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ],
            entry_summaries: vec![
                entry_summary(120, 100, 80, 1_000),
                entry_summary(200, 120, 90, 2_000),
            ],
            entry_arranges: Vec::new(),
            total_ex_score: 320,
            max_ex_score: 800,
            // Failed course results keep the full course notes as the rank/rate
            // denominator even when only a subset of entries produced summaries.
            total_notes: 400,
            bp: 37,
            final_clear_type: ClearType::Hard,
            final_gauge_type: GaugeType::ExClass,
            final_gauge_value: 42.5,
            course_max_combo: 170,
            judge_counts: crate::screens::result_model::ResultJudgeCounts {
                pgreat: 160,
                bad: 2,
                ..Default::default()
            },
            trophy_results: Vec::new(),
            course_clear: true,
            course_failed: false,
            total_entries: 2,
            played_entries: 2,
            replay_slots: [true, false, true, false],
            saved_replay_slots: [false, false, true, false],
            best_score: Some(crate::storage::score_db::CourseBestScore {
                course_score_id: 22,
                course_hash: "course-hash".to_string(),
                rule_mode: bmz_gameplay::rule::RuleMode::Beatoraja,
                ex_score: 340,
                max_ex_score: 800,
                clear_type: "ExHard".to_string(),
                gauge_type: "ExHardClass".to_string(),
                gauge_value: 64.0,
                max_combo: 180,
                bp: 4,
                cb: 2,
                judge_counts: Default::default(),
                fast_slow_counts: Default::default(),
                course_failed: false,
                course_clear: true,
                play_count: 3,
                clear_count: 2,
                played_at: 2,
            }),
            previous_best_score: Some(crate::storage::score_db::CourseBestScore {
                course_score_id: 21,
                course_hash: "course-hash".to_string(),
                rule_mode: bmz_gameplay::rule::RuleMode::Beatoraja,
                ex_score: 300,
                max_ex_score: 800,
                clear_type: "Normal".to_string(),
                gauge_type: "Class".to_string(),
                gauge_value: 60.0,
                max_combo: 150,
                bp: 12,
                cb: 8,
                judge_counts: Default::default(),
                fast_slow_counts: Default::default(),
                course_failed: false,
                course_clear: true,
                play_count: 2,
                clear_count: 1,
                played_at: 1,
            }),
        };

        let mut summary = course_result_summary_for_skin(&course);
        assert_eq!(summary.title, "Course Title");
        assert_eq!(summary.genre, "DAN");
        assert_eq!(summary.clear_type, ClearType::Hard);
        assert_eq!(summary.gauge_type, GaugeType::ExClass);
        assert_eq!(summary.gauge_value, 42.5);
        assert_eq!(summary.ex_score, 320);
        assert_eq!(summary.total_notes, 400);
        assert_eq!(summary.bp, 37);
        assert!((summary.ex_score_rate() - 0.4).abs() < 0.001);
        assert_eq!(summary.max_combo, 170);
        assert_eq!(summary.score_history_id, 22);
        assert_eq!(summary.best_ex_score, Some(300));
        assert_eq!(summary.best_clear_type, Some(ClearType::Normal));
        assert_eq!(summary.previous_best_ex_score, Some(300));
        assert_eq!(summary.previous_best_clear_type, Some(ClearType::Normal));
        assert_eq!(summary.previous_best_bp, Some(12));
        assert_eq!(summary.target_ex_score, Some(400));
        let number_values = result_lua_runtime_number_values_for_summary(&summary);
        assert_eq!(number_values.get(&74), Some(&400));
        assert_eq!(number_values.get(&110), Some(&160));
        assert_eq!(number_values.get(&113), Some(&2));
        assert_eq!(number_values.get(&426), Some(&0));
        assert_eq!(number_values.get(&178), Some(&25));
        assert_eq!(number_values.get(&425).copied(), Some(i32::try_from(summary.cb).unwrap()));
        assert_eq!(summary.replay_slots, [true, false, true, false]);
        assert_eq!(summary.saved_replay_slots, [false, false, true, false]);
        assert_eq!(summary.judge_counts.pgreat, 160);
        assert_eq!(summary.fast_slow_counts.fast_pgreat, 160);
        assert_eq!(
            summary.graph.gauge_points.iter().map(|point| point.time_ms).collect::<Vec<_>>(),
            vec![1_000, 3_000]
        );
        assert_eq!(
            summary.graph.timing_points.iter().map(|point| point.time_ms).collect::<Vec<_>>(),
            vec![1_000, 3_000]
        );
        assert_eq!(summary.graph.judge_graph_density, vec![100, 120]);
        assert_eq!(summary.graph.bpm_graph_segments[0].start_ratio, 0.0);
        assert!((summary.graph.bpm_graph_segments[0].end_ratio - 1.0 / 3.0).abs() < 0.001);
        assert!((summary.graph.bpm_graph_segments[1].start_ratio - 1.0 / 3.0).abs() < 0.001);
        assert_eq!(summary.graph.bpm_graph_segments[1].end_ratio, 1.0);

        summary.judge_rank = Some(3);
        summary.long_note_mode = bmz_chart::model::LongNoteMode::Hcn;
        summary.arrange = "RANDOM".to_string();
        summary.arrange_2p = "MIRROR".to_string();
        summary.target_name = "RANK AAA".to_string();
        let mut runtime_state =
            lua_runtime_state_for_result(false, None, true, KeyMode::K7, number_values, "Player");
        apply_result_summary_lua_load_state(
            &mut runtime_state,
            &summary,
            "Table",
            "★12",
            "Table ★12",
        );
        assert_eq!(runtime_state.text_values.get(&1).map(String::as_str), Some("RANK AAA"));
        assert_eq!(runtime_state.text_values.get(&3).map(String::as_str), Some("RANK AAA"));
        apply_course_result_lua_load_state(&mut runtime_state, &course);
        assert_eq!(runtime_state.text_values.get(&10).map(String::as_str), Some("Course Title"));
        assert_eq!(runtime_state.text_values.get(&12).map(String::as_str), Some("Course Title"));
        assert_eq!(runtime_state.text_values.get(&1003).map(String::as_str), Some("Table ★12"));
        assert_eq!(runtime_state.text_values.get(&150).map(String::as_str), Some("Stage 1"));
        assert_eq!(runtime_state.option_values.get(&180), Some(&false));
        assert_eq!(runtime_state.option_values.get(&183), Some(&true));
        assert_eq!(runtime_state.option_values.get(&184), Some(&false));
        assert_eq!(runtime_state.event_index_values.get(&308), Some(&2));
        assert_eq!(runtime_state.event_index_values.get(&42), Some(&2));
        assert_eq!(runtime_state.event_index_values.get(&43), Some(&1));
        assert_eq!(runtime_state.event_index_values.get(&344), Some(&2));
        assert_eq!(runtime_state.event_index_values.get(&345), Some(&1));
        summary.arrange = "F-RANDOM".to_string();
        summary.arrange_2p = "MF-RANDOM".to_string();
        let mut extended_runtime_state = bmz_skin::LuaLoadRuntimeState::default();
        apply_result_summary_lua_load_state(
            &mut extended_runtime_state,
            &summary,
            "Table",
            "★12",
            "Table ★12",
        );
        assert_eq!(extended_runtime_state.event_index_values.get(&42), Some(&2));
        assert_eq!(extended_runtime_state.event_index_values.get(&43), Some(&2));
        assert_eq!(extended_runtime_state.event_index_values.get(&344), Some(&10));
        assert_eq!(extended_runtime_state.event_index_values.get(&345), Some(&11));
        assert_eq!(
            runtime_state.number_values.get(&bmz_render::skin::SKIN_REF_BMZ_COURSE_STAGE_COUNT),
            Some(&2)
        );
        assert_eq!(
            runtime_state
                .number_values
                .get(&(bmz_render::skin::SKIN_REF_BMZ_COURSE_STAGE_EX_BASE + 1)),
            Some(&200)
        );
        assert_eq!(
            runtime_state
                .number_values
                .get(&(bmz_render::skin::SKIN_REF_BMZ_COURSE_STAGE_GAUGE_BASE + 1)),
            Some(&80)
        );
        let data: serde_json::Value = serde_json::from_str(
            &runtime_state.virtual_io_files["skin/WMII_FHD/result/courseData.json"],
        )
        .unwrap();
        assert_eq!(data["songs"].as_array().map(Vec::len), Some(2));
        assert_eq!(data["songs"][1]["score"], serde_json::json!(200));

        mark_course_replay_slot_saved(&mut course, Some(&mut summary), 1);
        assert_eq!(course.replay_slots, [true, true, true, false]);
        assert_eq!(course.saved_replay_slots, [false, true, true, false]);
        assert_eq!(summary.replay_slots, course.replay_slots);
        assert_eq!(summary.saved_replay_slots, course.saved_replay_slots);
    }

    #[test]
    fn course_entry_title_hints_are_hydrated_for_unplayed_stages() {
        let mut definition = bmz_core::course::CourseDefinition {
            key: "course".to_string(),
            title: "Course".to_string(),
            kind: bmz_core::course::CourseKind::Course,
            entries: vec![
                bmz_core::course::CourseEntry {
                    title_hint: String::new(),
                    md5: None,
                    sha256: None,
                    chart_id: Some(10),
                },
                bmz_core::course::CourseEntry {
                    title_hint: "stale".to_string(),
                    md5: None,
                    sha256: None,
                    chart_id: Some(20),
                },
                bmz_core::course::CourseEntry {
                    title_hint: "Missing".to_string(),
                    md5: None,
                    sha256: None,
                    chart_id: None,
                },
            ],
            constraints: bmz_core::course::CourseConstraints::default(),
            trophies: Vec::new(),
            release: true,
        };
        apply_course_entry_title_hints(
            &mut definition,
            &HashMap::from([(10, "Resolved One".to_string()), (20, "Resolved Two".to_string())]),
        );

        assert_eq!(definition.entries[0].title_hint, "Resolved One");
        assert_eq!(definition.entries[1].title_hint, "Resolved Two");
        assert_eq!(definition.entries[2].title_hint, "Missing");
    }

    #[test]
    fn result_lua_runtime_state_exposes_ir_connection_options() {
        let online = lua_runtime_state_for_result(
            false,
            Some("BMZ IR"),
            true,
            KeyMode::K7,
            BTreeMap::new(),
            "Player",
        );
        assert_eq!(online.option_values.get(&50), Some(&false));
        assert_eq!(online.option_values.get(&51), Some(&true));
        assert_eq!(online.option_values.get(&60), Some(&false));
        assert_eq!(online.option_values.get(&61), Some(&true));
        assert_eq!(online.option_values.get(&160), Some(&true));
        assert_eq!(online.option_values.get(&161), Some(&false));
        assert_eq!(online.text_values.get(&1020).map(String::as_str), Some("BMZ IR"));

        let offline = lua_runtime_state_for_result(
            false,
            None,
            false,
            KeyMode::K5,
            BTreeMap::new(),
            "Player",
        );
        assert_eq!(offline.option_values.get(&50), Some(&true));
        assert_eq!(offline.option_values.get(&51), Some(&false));
        assert_eq!(offline.option_values.get(&60), Some(&true));
        assert_eq!(offline.option_values.get(&61), Some(&false));
        assert_eq!(offline.option_values.get(&160), Some(&false));
        assert_eq!(offline.option_values.get(&161), Some(&true));
        assert_eq!(offline.text_values.get(&1020).map(String::as_str), Some(""));
    }

    #[test]
    fn result_ir_skin_name_uses_primary_provider_instead_of_registration_order() {
        use crate::config::profile_config::{
            IrConfig, IrProviderConfig, IrProviderRoleConfig, IrSendPolicyConfig,
        };

        let provider = |provider: &str, provider_key: &str, role| IrProviderConfig {
            provider: provider.to_string(),
            provider_key: provider_key.to_string(),
            base_url: "https://example.test/".to_string(),
            enabled: true,
            account_display_name: String::new(),
            account_id: String::new(),
            send_policy: IrSendPolicyConfig::default(),
            role,
            last_login_at: None,
            last_success_at: None,
        };
        let ir = IrConfig {
            primary_provider: "rian-ir".to_string(),
            providers: vec![
                provider("bmz", "bmz", IrProviderRoleConfig::SubmitOnly),
                provider("rian-ir", "rian-ir", IrProviderRoleConfig::Primary),
            ],
            ..IrConfig::default()
        };

        assert_eq!(result_ir_skin_name(&ir), Some("rianIR"));
    }

    #[test]
    fn result_judge_rank_options_match_beatoraja_ranges() {
        for (rank, expected) in [
            (Some(0), Some(180)),
            (Some(34), Some(180)),
            (Some(1), Some(181)),
            (Some(59), Some(181)),
            (Some(2), Some(182)),
            (Some(84), Some(182)),
            (Some(3), Some(183)),
            (Some(109), Some(183)),
            (Some(4), Some(184)),
            (Some(110), Some(184)),
            (None, Some(182)),
        ] {
            assert_eq!(result_judge_rank_option_id(rank), expected, "rank {rank:?}");
        }
        assert_eq!(result_judge_rank_option_id(Some(9)), None);
    }

    #[test]
    fn play_lua_runtime_state_exposes_play_mode_and_score_save_options() {
        let normal =
            lua_runtime_state_for_play(&PlayStartOptions::default(), false, KeyMode::K7, "Player");
        assert_eq!(normal.text_values.get(&2).map(String::as_str), Some("Player"));
        assert_eq!(normal.option_values.get(&61), Some(&true));
        assert_eq!(normal.option_values.get(&82), Some(&true));
        assert_eq!(normal.option_values.get(&84), Some(&false));
        assert_eq!(normal.number_values.get(&SKIN_REF_BMZ_KEY_MODE), Some(&7));
        assert_eq!(normal.number_values.get(&SKIN_REF_BMZ_ACTIVE_LANE_COUNT), Some(&8));
        assert_eq!(normal.option_values.get(&(SKIN_OPTION_BMZ_KEY_MODE_BASE + 3)), Some(&true));
        assert_eq!(normal.option_values.get(&SKIN_OPTION_BMZ_SINGLE_PLAY), Some(&true));

        let autoplay = lua_runtime_state_for_play(
            &PlayStartOptions { autoplay: true, ..PlayStartOptions::default() },
            false,
            KeyMode::K7,
            "Player",
        );
        assert_eq!(autoplay.option_values.get(&33), Some(&true));
        assert_eq!(autoplay.option_values.get(&60), Some(&true));
        assert_eq!(autoplay.option_values.get(&82), Some(&false));

        let replay = lua_runtime_state_for_play(
            &PlayStartOptions {
                replay_player: Some(bmz_gameplay::replay::ReplayPlayer::default()),
                ..PlayStartOptions::default()
            },
            false,
            KeyMode::K7,
            "Player",
        );
        assert_eq!(replay.option_values.get(&33), Some(&false));
        assert_eq!(replay.option_values.get(&84), Some(&true));

        let practice = lua_runtime_state_for_play(
            &PlayStartOptions { practice_mode: true, ..PlayStartOptions::default() },
            false,
            KeyMode::K7,
            "Player",
        );
        assert_eq!(practice.option_values.get(&60), Some(&true));
        assert_eq!(practice.option_values.get(&82), Some(&true));
        assert_eq!(practice.option_values.get(&1080), Some(&true));
    }

    #[test]
    fn play_skin_defs_load_from_configured_path_without_renderer_install() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = repo.join("data/skins/ECFN/play/play7.luaskin");
        if !path.is_file() {
            return;
        }

        let app_paths = crate::paths::AppPaths::from_dirs(
            repo.join("data"),
            repo.join("data"),
            repo.join("data/cache"),
            repo.join("data/logs"),
        );
        let defs = play_skin_defs_from_path(&app_paths, &path.to_string_lossy());

        assert!(!defs.property.is_empty());
        assert!(!defs.filepath.is_empty());
        assert!(defs.offset.iter().any(|offset| offset.id == 10));
    }

    fn default_select_keys() -> SelectKeyBindings {
        SelectKeyBindings::from_profile(&crate::config::play_input::default_profile_input())
    }

    fn select_keys_9k() -> SelectKeyBindings {
        let mut input = crate::config::play_input::default_profile_input();
        input.select_input_mode = SelectInputModeConfig::Key9;
        SelectKeyBindings::from_profile(&input)
    }

    fn play_option_input_for(input: &ProfileInputConfig, key_mode: KeyMode) -> PlayOptionInput {
        PlayOptionInput::new(
            key_mode,
            crate::config::play::lane_binding_for_chart(input, key_mode),
            input,
            crate::input::gamepad::GamepadSlotMap::default(),
        )
    }

    fn keyboard_play_option(
        control: &str,
        e1_held: bool,
        e2_held: bool,
        _keys: &SelectKeyBindings,
        play_input: &PlayOptionInput,
        input: &ProfileInputConfig,
    ) -> Option<PlayOptionControl> {
        play_option_control_for_input(
            W_KEYBOARD_DEVICE_ID,
            &PhysicalControl::KeyboardKey(control.to_string()),
            e1_held,
            e2_held,
            Some(play_input),
            input,
        )
    }

    fn select_keys_with_full_2p_bindings() -> SelectKeyBindings {
        let mut input = crate::config::play_input::default_profile_input();
        let key = KeyMode::K14.play_map_key().to_string();
        input.play.insert(
            key.clone(),
            crate::config::profile_config::PlayModeInputConfig {
                inherit: None,
                bindings: crate::config::play_input::default_play_14k_bindings(),
                ..Default::default()
            },
        );
        let play14 = input.play.get_mut(&key).expect("14K bindings");
        play14.bindings.push(crate::config::play_input::play_binding("P2K6", LaneConfig::Key13));
        play14.bindings.push(crate::config::play_input::play_binding("P2K7", LaneConfig::Key14));
        SelectKeyBindings::from_profile(&input)
    }

    #[test]
    fn select_action_maps_start_and_vertical_movement() {
        let keys = default_select_keys();
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::Enter), ElementState::Pressed, false, &keys),
            Some(SelectAction::EnterOrPlay)
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::ArrowUp), ElementState::Pressed, false, &keys),
            Some(SelectAction::Move(SelectMove::Previous))
        );
        assert_eq!(
            select_action(
                PhysicalKey::Code(KeyCode::ArrowDown),
                ElementState::Pressed,
                false,
                &keys
            ),
            Some(SelectAction::Move(SelectMove::Next))
        );
        assert_eq!(
            select_action(
                PhysicalKey::Code(KeyCode::ShiftLeft),
                ElementState::Pressed,
                false,
                &keys
            ),
            Some(SelectAction::Move(SelectMove::Previous))
        );
        assert_eq!(
            select_action(
                PhysicalKey::Code(KeyCode::ControlLeft),
                ElementState::Pressed,
                false,
                &keys
            ),
            Some(SelectAction::Move(SelectMove::Next))
        );
        assert_eq!(
            select_action(
                PhysicalKey::Code(KeyCode::ControlRight),
                ElementState::Pressed,
                false,
                &keys
            ),
            Some(SelectAction::Move(SelectMove::Next))
        );
        assert_eq!(
            select_action(
                PhysicalKey::Code(KeyCode::ShiftRight),
                ElementState::Pressed,
                false,
                &keys
            ),
            Some(SelectAction::Move(SelectMove::Previous))
        );
    }

    #[test]
    fn select_option_gamepad_lane_distinguishes_same_buttons_by_device() {
        let profile = ProfileConfig::new_default("default", "Default", 0);
        let control = "Button1";

        assert_eq!(
            select_option_lane_for_gamepad(
                &profile.input,
                crate::input::gamepad::GamepadSlotMap::from_slot_ids([Some(0), Some(1)]),
                DeviceId(16),
                control,
            ),
            Some(Lane::Key1)
        );
        assert_eq!(
            select_option_lane_for_gamepad(
                &profile.input,
                crate::input::gamepad::GamepadSlotMap::from_slot_ids([Some(0), Some(1)]),
                DeviceId(17),
                control,
            ),
            Some(Lane::Key8)
        );
        assert_eq!(
            select_option_lane_for_gamepad(
                &profile.input,
                crate::input::gamepad::GamepadSlotMap::from_slot_ids([Some(1), Some(0)]),
                DeviceId(16),
                control,
            ),
            Some(Lane::Key8)
        );
    }

    #[test]
    fn select_row_click_enters_only_when_row_is_already_selected() {
        assert_eq!(
            select_row_click_action(2, MouseButton::Left, 0, 4, false),
            Some(SelectRowClickAction::Select(2))
        );
        assert_eq!(
            select_row_click_action(2, MouseButton::Left, 2, 4, false),
            Some(SelectRowClickAction::EnterOrPlay)
        );
        assert_eq!(select_row_click_action(4, MouseButton::Left, 2, 4, false), None);
        assert_eq!(
            select_row_click_action(2, MouseButton::Right, 2, 4, false),
            Some(SelectRowClickAction::ExitFolder)
        );
        assert_eq!(
            select_row_click_action(2, MouseButton::Right, 2, 4, true),
            Some(SelectRowClickAction::CancelSettingsEdit)
        );
        assert_eq!(select_row_click_action(2, MouseButton::Middle, 2, 4, false), None);
    }

    #[test]
    fn select_key_bindings_identify_e_action_controls() {
        let keys = default_select_keys();

        assert_eq!(keys.e_action_for_control("Q"), Some(InputActionConfig::E1));
        assert_eq!(keys.e_action_for_control("W"), Some(InputActionConfig::E2));
        assert_eq!(keys.e_action_for_control("E"), Some(InputActionConfig::E3));
        assert_eq!(keys.e_action_for_control("R"), Some(InputActionConfig::E4));
        assert_eq!(keys.e_action_for_control("Slash"), None);
    }

    #[test]
    fn select_scroll_slider_value_maps_to_nearest_row() {
        assert_eq!(select_scroll_slider_index(0.0, 0), None);
        assert_eq!(select_scroll_slider_index(0.5, 1), Some(0));
        assert_eq!(select_scroll_slider_index(-1.0, 10), Some(0));
        assert_eq!(select_scroll_slider_index(0.0, 10), Some(0));
        assert_eq!(select_scroll_slider_index(0.49, 10), Some(4));
        assert_eq!(select_scroll_slider_index(0.50, 10), Some(5));
        assert_eq!(select_scroll_slider_index(1.0, 10), Some(9));
        assert_eq!(select_scroll_slider_index(2.0, 10), Some(9));
    }

    #[test]
    fn skin_video_source_respects_static_property_ops() {
        let mut document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "property": [
                    {
                        "name": "動画を使用する",
                        "def": "ON",
                        "item": [
                            { "name": "ON", "op": 920 },
                            { "name": "OFF", "op": 921 }
                        ]
                    }
                ],
                "source": [{ "id": "mv", "path": "mv/default.mp4" }],
                "image": [{ "id": "mv", "src": "mv", "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [{ "id": "mv", "op": [920], "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] }]
            }
            "#,
        )
        .unwrap();

        assert!(skin_video_source_gating(&document, "mv").active);

        document.user_selected_options = Some(vec![921]);
        assert!(!skin_video_source_gating(&document, "mv").active);
        assert!(skin_video_source_gating(&document, "unknown-source").active);
    }

    #[test]
    fn skin_video_source_fast_path_updates_selected_options() {
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "property": [
                    {
                        "name": "動画を使用する",
                        "def": "ON",
                        "item": [
                            { "name": "ON", "op": 920 },
                            { "name": "OFF", "op": 921 }
                        ]
                    }
                ],
                "source": [{ "id": "mv", "path": "mv/default.mp4" }],
                "image": [{ "id": "mv", "src": "mv", "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [{ "id": "mv", "op": [920], "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] }]
            }
            "#,
        )
        .unwrap();
        let gating = skin_video_source_gating(&document, "mv");
        let mut sources = vec![ActiveSkinVideoSource {
            texture: SkinTextureId(0),
            path: PathBuf::new(),
            decoder: None,
            last_pts: None,
            loop_start_us: 0,
            active: gating.active,
            gating_op_sets: gating.op_sets,
            enabled_options: document.enabled_options(),
            result_ranktime_ms: document.ranktime,
            failed: false,
        }];

        apply_skin_video_source_enabled_options(
            &mut sources,
            &[921],
            &skin_document_property_ops(&document),
        );

        assert_eq!(sources[0].enabled_options, vec![921]);
        assert!(!sources[0].active);
    }

    #[test]
    fn json_skin_option_reload_detection_allows_op_only_skins() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir()
            .join(format!("bmz-player-json-skin-reload-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let op_only = root.join("op-only.json");
        std::fs::write(
            &op_only,
            r#"
            {
                "type": 5,
                "property": [
                    {
                        "name": "Option",
                        "def": "ON",
                        "item": [
                            { "name": "ON", "op": 920 },
                            { "name": "OFF", "op": 921 }
                        ]
                    }
                ],
                "destination": [
                    { "id": "panel", "op": [920], "dst": [{ "x": 0, "y": 0, "w": 1, "h": 1 }] }
                ]
            }
            "#,
        )
        .unwrap();
        let load_time = root.join("load-time.json");
        std::fs::write(
            &load_time,
            r#"
            {
                "type": 5,
                "destination": [
                    { "if": 920, "values": [
                        { "id": "panel", "dst": [{ "x": 0, "y": 0, "w": 1, "h": 1 }] }
                    ] }
                ]
            }
            "#,
        )
        .unwrap();
        let include = root.join("include.json");
        std::fs::write(
            &include,
            r#"
            [
                { "if": 920, "value": { "id": "included", "src": "1", "x": 0, "y": 0, "w": 1, "h": 1 } }
            ]
            "#,
        )
        .unwrap();
        let includes_load_time = root.join("includes-load-time.json");
        std::fs::write(
            &includes_load_time,
            r#"
            {
                "type": 5,
                "image": [{ "include": "include.json" }]
            }
            "#,
        )
        .unwrap();
        let lua_skin = root.join("load-time.luaskin");
        std::fs::write(&lua_skin, "return { type = 5 }").unwrap();
        let lr2_skin = root.join("load-time.lr2skin");
        std::fs::write(&lr2_skin, "#LR2SKIN").unwrap();

        assert!(!skin_path_options_need_full_reload(&op_only).unwrap());
        assert!(skin_path_options_need_full_reload(&load_time).unwrap());
        assert!(skin_path_options_need_full_reload(&includes_load_time).unwrap());
        assert!(skin_path_options_need_full_reload(&lua_skin).unwrap());
        assert!(skin_path_options_need_full_reload(&lr2_skin).unwrap());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn skin_video_source_runtime_visibility_follows_result_rank_op() {
        use bmz_render::skin::SkinDrawState;

        // ランク別 BG を op で出し分けるリザルトスキン構成 (Starseeker 相当)。
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 7,
                "source": [
                    { "id": "BG_A", "path": "BG/A/a.mp4" },
                    { "id": "BG_AAA", "path": "BG/AAA/aaa.mp4" }
                ],
                "image": [
                    { "id": "BG_A", "src": "BG_A", "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "BG_AAA", "src": "BG_AAA", "x": 0, "y": 0, "w": 10, "h": 10 }
                ],
                "destination": [
                    { "id": "BG_A", "op": [90, 302], "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "BG_AAA", "op": [90, 300], "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] }
                ]
            }
            "#,
        )
        .unwrap();

        let make_source = |source_id: &str| {
            let gating = skin_video_source_gating(&document, source_id);
            ActiveSkinVideoSource {
                texture: SkinTextureId(0),
                path: PathBuf::new(),
                decoder: None,
                last_pts: None,
                loop_start_us: 0,
                active: gating.active,
                gating_op_sets: gating.op_sets,
                enabled_options: document.enabled_options(),
                result_ranktime_ms: document.ranktime,
                failed: false,
            }
        };
        let bg_a = make_source("BG_A");
        let bg_aaa = make_source("BG_AAA");

        // ex_score / total_notes でランクが決まる。9/9 = AAA, 6/9 = A 付近。
        let aaa_state = SkinDrawState {
            result_failed: Some(false),
            ex_score: 18,
            total_notes: 9,
            ..SkinDrawState::default()
        };
        assert!(skin_video_source_runtime_visible(&bg_aaa, &aaa_state));
        assert!(!skin_video_source_runtime_visible(&bg_a, &aaa_state));

        // 13/18 = 72.2% は rank index 2 (= A), op 302 に対応する。
        let a_state = SkinDrawState {
            result_failed: Some(false),
            ex_score: 13,
            total_notes: 9,
            ..SkinDrawState::default()
        };
        assert!(skin_video_source_runtime_visible(&bg_a, &a_state));
        assert!(!skin_video_source_runtime_visible(&bg_aaa, &a_state));
    }

    #[test]
    fn skin_video_sources_need_runtime_state_only_for_active_gated_sources() {
        let make_source =
            |active: bool, failed: bool, gating_op_sets: Vec<Vec<i32>>| ActiveSkinVideoSource {
                texture: SkinTextureId(0),
                path: PathBuf::new(),
                decoder: None,
                last_pts: None,
                loop_start_us: 0,
                active,
                gating_op_sets,
                enabled_options: Vec::new(),
                result_ranktime_ms: 0,
                failed,
            };

        assert!(!skin_video_sources_need_runtime_state(&[
            make_source(true, false, Vec::new()),
            make_source(false, false, vec![vec![90]]),
            make_source(true, true, vec![vec![90]]),
        ]));
        let gated_source = make_source(true, false, vec![vec![90]]);
        assert!(skin_video_sources_need_runtime_state(&[gated_source]));
    }

    #[test]
    fn play_skin_video_source_runtime_visibility_follows_bga_ops() {
        // ECFN の generic BGA 相当。beatoraja では BGA ON かつ曲BGAなしの時だけ
        // destination が有効になり、動画フレーム取得も走る。
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "property": [
                    {
                        "name": "Generic BGA",
                        "def": "P1",
                        "item": [
                            { "name": "P1", "op": 924 },
                            { "name": "P2", "op": 925 }
                        ]
                    }
                ],
                "source": [{ "id": "mv", "path": "generic.mp4" }],
                "image": [{ "id": "generic-BGA", "src": "mv", "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "generic-BGA", "op": [41, 170, 924], "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] }
                ]
            }
            "#,
        )
        .unwrap();

        let gating = skin_video_source_gating(&document, "mv");
        assert!(gating.active);
        assert_eq!(gating.op_sets, vec![vec![41, 170, 924]]);
        let source = ActiveSkinVideoSource {
            texture: SkinTextureId(0),
            path: PathBuf::new(),
            decoder: None,
            last_pts: None,
            loop_start_us: 0,
            active: gating.active,
            gating_op_sets: gating.op_sets,
            enabled_options: document.enabled_options(),
            result_ranktime_ms: document.ranktime,
            failed: false,
        };

        let visible_state = play_skin_video_draw_state(
            &RenderSnapshot {
                has_bga: false,
                bga_enabled: true,
                resources_loaded: true,
                ..RenderSnapshot::default()
            },
            None,
            None,
        );
        assert!(skin_video_source_runtime_visible(&source, &visible_state));

        let song_bga_state = play_skin_video_draw_state(
            &RenderSnapshot {
                has_bga: true,
                bga_enabled: true,
                resources_loaded: true,
                ..RenderSnapshot::default()
            },
            None,
            None,
        );
        assert!(!skin_video_source_runtime_visible(&source, &song_bga_state));

        let bga_off_state = play_skin_video_draw_state(
            &RenderSnapshot {
                has_bga: false,
                bga_enabled: false,
                resources_loaded: true,
                ..RenderSnapshot::default()
            },
            None,
            None,
        );
        assert!(!skin_video_source_runtime_visible(&source, &bga_off_state));

        let song_bga_off_state = play_skin_video_draw_state(
            &RenderSnapshot {
                has_bga: true,
                bga_enabled: false,
                resources_loaded: true,
                ..RenderSnapshot::default()
            },
            None,
            None,
        );
        assert!(!skin_video_source_runtime_visible(&source, &song_bga_off_state));
    }

    #[test]
    fn play_skin_draw_state_maps_lane_cover_and_lift_offsets_to_skin_pixels() {
        let state = play_skin_video_draw_state(
            &RenderSnapshot {
                lane_cover: 0.5,
                lift: 0.25,
                hidden_cover: 0.1,
                ..RenderSnapshot::default()
            },
            Some(1080),
            Some(720),
        );

        assert_eq!(state.offset_lift_px, 180);
        assert_eq!(state.offset_lanecover_px, -360);
        assert_eq!(state.offset_hidden_cover_px, 54);
    }

    #[test]
    fn play_skin_video_loaded_state_starts_with_ready_timer() {
        let preload_state = play_skin_video_draw_state(
            &RenderSnapshot {
                resources_loaded: true,
                ready_elapsed_time: None,
                ..RenderSnapshot::default()
            },
            None,
            None,
        );
        assert!(!preload_state.skin_loaded);

        let ready_state = play_skin_video_draw_state(
            &RenderSnapshot {
                resources_loaded: true,
                ready_elapsed_time: Some(TimeUs(0)),
                ..RenderSnapshot::default()
            },
            None,
            None,
        );
        assert!(ready_state.skin_loaded);
    }

    #[test]
    fn skin_video_source_gating_respects_conditional_destination_if_ops() {
        use bmz_render::skin::SkinDrawState;

        let mut document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 7,
                "property": [
                    {
                        "name": "動画を使用する",
                        "def": "ON",
                        "item": [
                            { "name": "ON", "op": 920 },
                            { "name": "OFF", "op": 921 }
                        ]
                    }
                ],
                "source": [{ "id": "BG_AAA", "path": "BG/AAA/aaa.mp4" }],
                "image": [{ "id": "BG_AAA", "src": "BG_AAA", "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    {
                        "if": [920],
                        "values": [
                            { "id": "BG_AAA", "op": [90, 300], "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] }
                        ]
                    }
                ]
            }
            "#,
        )
        .unwrap();

        let gating = skin_video_source_gating(&document, "BG_AAA");
        assert!(gating.active);
        assert_eq!(gating.op_sets, vec![vec![920, 90, 300]]);
        let aaa_state = SkinDrawState {
            result_failed: Some(false),
            ex_score: 18,
            total_notes: 9,
            ..SkinDrawState::default()
        };
        let source = ActiveSkinVideoSource {
            texture: SkinTextureId(0),
            path: PathBuf::new(),
            decoder: None,
            last_pts: None,
            loop_start_us: 0,
            active: gating.active,
            gating_op_sets: gating.op_sets,
            enabled_options: document.enabled_options(),
            result_ranktime_ms: document.ranktime,
            failed: false,
        };
        assert!(skin_video_source_runtime_visible(&source, &aaa_state));

        document.user_selected_options = Some(vec![921]);
        let gating = skin_video_source_gating(&document, "BG_AAA");
        assert!(!gating.active);
        let disabled_source = ActiveSkinVideoSource {
            texture: SkinTextureId(0),
            path: PathBuf::new(),
            decoder: None,
            last_pts: None,
            loop_start_us: 0,
            active: gating.active,
            gating_op_sets: gating.op_sets,
            enabled_options: document.enabled_options(),
            result_ranktime_ms: document.ranktime,
            failed: false,
        };
        assert!(!skin_video_source_runtime_visible(&disabled_source, &aaa_state));
    }

    #[test]
    fn select_action_maps_page_and_edge_movement() {
        let keys = default_select_keys();
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::PageUp), ElementState::Pressed, false, &keys),
            Some(SelectAction::Move(SelectMove::PagePrevious))
        );
        assert_eq!(
            select_action(
                PhysicalKey::Code(KeyCode::PageDown),
                ElementState::Pressed,
                false,
                &keys
            ),
            Some(SelectAction::Move(SelectMove::PageNext))
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::Home), ElementState::Pressed, false, &keys),
            Some(SelectAction::Move(SelectMove::First))
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::End), ElementState::Pressed, false, &keys),
            Some(SelectAction::Move(SelectMove::Last))
        );
    }

    #[test]
    fn select_action_maps_configured_lane_keys() {
        let keys = default_select_keys();
        // Key1(Z), Key3(X), Key5(C), Key7(V) → EnterOrPlay
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyZ), ElementState::Pressed, false, &keys),
            Some(SelectAction::EnterOrPlay)
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyV), ElementState::Pressed, false, &keys),
            Some(SelectAction::EnterOrPlay)
        );
        // Key2(S), Key4(D), Key6(F) → ExitFolder
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyS), ElementState::Pressed, false, &keys),
            Some(SelectAction::ExitFolder)
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyD), ElementState::Pressed, false, &keys),
            Some(SelectAction::ExitFolder)
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyF), ElementState::Pressed, false, &keys),
            Some(SelectAction::ExitFolder)
        );
        // E2(W) is also mapped to ExitFolder for direct lookup paths.
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyW), ElementState::Pressed, false, &keys),
            Some(SelectAction::ExitFolder)
        );
    }

    #[test]
    fn select_action_maps_collection_keys() {
        let keys = default_select_keys();
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::F8), ElementState::Pressed, false, &keys),
            Some(SelectAction::FavoriteSong)
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::F9), ElementState::Pressed, false, &keys),
            Some(SelectAction::FavoriteChart)
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::Numpad8), ElementState::Pressed, false, &keys),
            Some(SelectAction::SameFolder)
        );
    }

    #[test]
    fn select_control_action_uses_key2_binding_for_controller_back() {
        let input = crate::config::play_input::default_profile_input();
        let keys = SelectKeyBindings::from_profile(&input);

        assert!(keys.is_back("Button2"));
        assert_eq!(select_control_action("Button2", &keys), Some(SelectAction::ExitFolder));
        assert_eq!(select_control_action("Button1", &keys), Some(SelectAction::EnterOrPlay));
    }

    #[test]
    fn select_control_action_does_not_hardcode_button2_as_back() {
        let mut input = crate::config::play_input::default_profile_input();
        let play7 = input.play.get_mut(KeyMode::K7.play_map_key()).expect("7K bindings");
        for entry in &mut play7.bindings {
            if entry.device == "gamepad" && entry.control == "Button2" {
                entry.lane = Some(LaneConfig::Key3);
            }
        }
        let keys = SelectKeyBindings::from_profile(&input);

        assert!(keys.is_enter("Button2"));
        assert_eq!(select_control_action("Button2", &keys), Some(SelectAction::EnterOrPlay));
        assert_eq!(select_control_action("Button1", &keys), Some(SelectAction::EnterOrPlay));
    }

    #[test]
    fn key9_select_input_maps_configured_lane_keys() {
        let keys = select_keys_9k();

        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyF), ElementState::Pressed, false, &keys),
            Some(SelectAction::Move(SelectMove::Next))
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyD), ElementState::Pressed, false, &keys),
            Some(SelectAction::Move(SelectMove::Previous))
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyC), ElementState::Pressed, false, &keys),
            Some(SelectAction::EnterOrPlay)
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyV), ElementState::Pressed, false, &keys),
            Some(SelectAction::EnterOrPlay)
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyX), ElementState::Pressed, false, &keys),
            Some(SelectAction::ExitFolder)
        );
        assert_eq!(target_cycle_from_control("G", &keys), Some(TargetCycle::Next));
        assert_eq!(target_cycle_from_control("B", &keys), Some(TargetCycle::Previous));
    }

    #[test]
    fn select_action_rejects_releases_repeats_and_other_keys() {
        let keys = default_select_keys();
        assert_eq!(
            select_action(
                PhysicalKey::Code(KeyCode::ArrowDown),
                ElementState::Released,
                false,
                &keys
            ),
            None
        );
        assert_eq!(
            select_action(
                PhysicalKey::Code(KeyCode::ArrowDown),
                ElementState::Pressed,
                true,
                &keys
            ),
            None
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyA), ElementState::Pressed, false, &keys),
            None
        );
    }

    #[test]
    fn settings_key_repeat_is_accepted_only_while_editing_value() {
        assert!(should_route_settings_key_event(ElementState::Pressed, false, false));
        assert!(!should_route_settings_key_event(ElementState::Pressed, true, false));
        assert!(should_route_settings_key_event(ElementState::Pressed, true, true));
        assert!(!should_route_settings_key_event(ElementState::Released, true, true));
    }

    #[test]
    fn settings_browse_keeps_cursor_navigation_direction() {
        let profile = ProfileConfig::new_default("default", "Default", 0);
        let bindings = SettingsBindings::from_profile(&profile.input);
        let select_bindings = SelectKeyBindings::from_profile(&profile.input);

        assert_eq!(
            settings_browse_move_control("ArrowUp", &bindings, &select_bindings),
            Some(SelectMove::Previous)
        );
        assert_eq!(
            settings_browse_move_control("ArrowDown", &bindings, &select_bindings),
            Some(SelectMove::Next)
        );
        assert_eq!(
            settings_browse_move_control("DPadUp", &bindings, &select_bindings),
            Some(SelectMove::Previous)
        );
        assert_eq!(
            settings_browse_move_control("DPadDown", &bindings, &select_bindings),
            Some(SelectMove::Next)
        );
        assert_eq!(
            settings_browse_move_control("LShift", &bindings, &select_bindings),
            Some(SelectMove::Previous)
        );
        assert_eq!(
            settings_browse_move_control("LControl", &bindings, &select_bindings),
            Some(SelectMove::Next)
        );
    }

    #[test]
    fn select_wheel_move_maps_vertical_scroll_to_selection_movement() {
        assert_eq!(
            select_wheel_move(MouseScrollDelta::LineDelta(0.0, 1.0)),
            Some(SelectMove::Previous)
        );
        assert_eq!(
            select_wheel_move(MouseScrollDelta::LineDelta(0.0, -1.0)),
            Some(SelectMove::Next)
        );
        assert_eq!(select_wheel_move(MouseScrollDelta::LineDelta(3.0, 0.0)), None);
    }

    #[test]
    fn select_wheel_move_supports_pixel_delta() {
        assert_eq!(
            select_wheel_move(MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(
                0.0, 12.0
            ))),
            Some(SelectMove::Previous)
        );
        assert_eq!(
            select_wheel_move(MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(
                0.0, -12.0
            ))),
            Some(SelectMove::Next)
        );
    }

    #[test]
    fn lane_cover_wheel_change_maps_vertical_scroll() {
        assert_eq!(
            lane_cover_wheel_change(MouseScrollDelta::LineDelta(0.0, 1.0)),
            Some(LaneCoverChange::Up)
        );
        assert_eq!(
            lane_cover_wheel_change(MouseScrollDelta::LineDelta(0.0, -1.0)),
            Some(LaneCoverChange::Down)
        );
        assert_eq!(lane_cover_wheel_change(MouseScrollDelta::LineDelta(1.0, 0.0)), None);
    }

    #[test]
    fn select_click_event_arg_matches_beatoraja_click_types() {
        let rect = Rect { x: 0.2, y: 0.3, width: 0.4, height: 0.2 };
        assert_eq!(select_click_event_arg(0, MouseButton::Left, rect, 0.3, 0.4), Some(1));
        assert_eq!(select_click_event_arg(0, MouseButton::Right, rect, 0.3, 0.4), Some(-1));
        assert_eq!(select_click_event_arg(1, MouseButton::Right, rect, 0.3, 0.4), Some(1));
        assert_eq!(select_click_event_arg(2, MouseButton::Left, rect, 0.39, 0.4), Some(-1));
        assert_eq!(select_click_event_arg(2, MouseButton::Left, rect, 0.41, 0.4), Some(1));
        assert_eq!(select_click_event_arg(3, MouseButton::Left, rect, 0.3, 0.39), Some(1));
        assert_eq!(select_click_event_arg(3, MouseButton::Left, rect, 0.3, 0.41), Some(-1));
        assert_eq!(select_click_event_arg(4, MouseButton::Left, rect, 0.3, 0.4), None);
    }

    #[test]
    fn select_key_bindings_builds_correct_hints() {
        let keys = default_select_keys();
        assert!(keys.key_hint().contains("Z/X/C/V"), "enter keys in hint: {}", keys.key_hint());
        assert!(keys.key_hint().contains("/S/D/F:BACK"), "back keys in hint: {}", keys.key_hint());
        assert!(keys.key_hint().contains(" Q"), "start key in hint: {}", keys.key_hint());
        assert!(keys.option_hint().contains("F1 MENU"), "menu in hint: {}", keys.option_hint());
        assert!(keys.option_hint().contains("F5 RELOAD"), "reload in hint: {}", keys.option_hint());
        assert!(
            keys.option_hint().contains("Q+K1/K2:1P ARR"),
            "1P arrange in hint: {}",
            keys.option_hint()
        );
        assert!(
            keys.option_hint().contains("Q+2P K1/K2:2P ARR"),
            "2P arrange in hint: {}",
            keys.option_hint()
        );
        assert!(
            keys.option_hint().contains("Q+K5:HS-FIX"),
            "HS-FIX in hint: {}",
            keys.option_hint()
        );
        assert!(
            keys.option_hint().contains("Q+K6:DP OPT"),
            "DP option in hint: {}",
            keys.option_hint()
        );
        assert!(
            keys.option_hint().contains("Q+UP/DOWN:TARGET"),
            "target in hint: {}",
            keys.option_hint()
        );
    }

    #[test]
    fn select_option_panel_maps_start_and_select_holds() {
        assert_eq!(select_option_panel_for_holds(false, false), 0);
        assert_eq!(select_option_panel_for_holds(true, false), 1);
        assert_eq!(select_option_panel_for_holds(false, true), 2);
        assert_eq!(select_option_panel_for_holds(true, true), 3);
    }

    #[test]
    fn select_option_panel_transition_plays_open_and_close_sounds() {
        use crate::system_sound::SoundType;

        assert_eq!(select_option_panel_sound_for_transition(0, 1), Some(SoundType::OptionOpen));
        assert_eq!(select_option_panel_sound_for_transition(3, 0), Some(SoundType::OptionClose));
        assert_eq!(select_option_panel_sound_for_transition(1, 2), None);
        assert_eq!(select_option_panel_sound_for_transition(2, 3), None);
        assert_eq!(select_option_panel_sound_for_transition(0, 0), None);
    }

    #[test]
    fn select_option_panel_transition_tracks_independent_off_timers() {
        let base = Instant::now();
        let mut current = 1;
        let mut on_started_at = base;
        let mut off_started_at = [None; 6];

        assert!(transition_select_option_panel(
            &mut current,
            &mut on_started_at,
            &mut off_started_at,
            2,
            base + Duration::from_millis(100),
        ));
        assert_eq!(current, 2);
        assert_eq!(off_started_at[0], Some(base + Duration::from_millis(100)));
        assert_eq!(off_started_at[1], None);

        assert!(transition_select_option_panel(
            &mut current,
            &mut on_started_at,
            &mut off_started_at,
            0,
            base + Duration::from_millis(200),
        ));
        assert_eq!(off_started_at[0], Some(base + Duration::from_millis(100)));
        assert_eq!(off_started_at[1], Some(base + Duration::from_millis(200)));

        assert!(transition_select_option_panel(
            &mut current,
            &mut on_started_at,
            &mut off_started_at,
            1,
            base + Duration::from_millis(300),
        ));
        assert_eq!(off_started_at[0], None);
        assert_eq!(off_started_at[1], Some(base + Duration::from_millis(200)));
        assert!(!transition_select_option_panel(
            &mut current,
            &mut on_started_at,
            &mut off_started_at,
            1,
            base + Duration::from_millis(400),
        ));
    }

    #[test]
    fn select_hold_state_rebuilds_from_pressed_controls() {
        let keys = default_select_keys();
        let pressed = HashSet::from(["Q".to_string(), "W".to_string()]);

        let (start_held, select_held, e_action_holds) =
            select_hold_state_from_pressed_controls(&pressed, &keys);

        assert!(start_held);
        assert!(select_held);
        assert!(e_action_holds.contains(&InputActionConfig::E1));
        assert!(e_action_holds.contains(&InputActionConfig::E2));

        let pressed = HashSet::from(["W".to_string()]);
        let (start_held, select_held, e_action_holds) =
            select_hold_state_from_pressed_controls(&pressed, &keys);

        assert!(!start_held);
        assert!(select_held);
        assert!(!e_action_holds.contains(&InputActionConfig::E1));
        assert!(e_action_holds.contains(&InputActionConfig::E2));
    }

    #[test]
    fn skin_logical_inputs_include_all_e_actions_and_ui_directions() {
        let keys = default_select_keys();
        let pressed = HashSet::from([
            "Q".to_string(),
            "W".to_string(),
            "E".to_string(),
            "R".to_string(),
            "ArrowLeft".to_string(),
            "DPadRight".to_string(),
            "ArrowUp".to_string(),
            "DPadDown".to_string(),
        ]);

        assert_eq!(
            skin_logical_input_snapshot_from_pressed_controls(&pressed, &keys).held,
            [true; bmz_render::skin::SKIN_BMZ_INPUT_COUNT]
        );
    }

    #[test]
    fn play_control_hold_state_rebuilds_from_pressed_controls() {
        let input = crate::config::play_input::default_profile_input();
        let play_input = play_option_input_for(&input, KeyMode::K7);
        let keyboard = |control: &str| {
            (W_KEYBOARD_DEVICE_ID, PhysicalControl::KeyboardKey(control.to_string()))
        };
        let pressed = HashSet::from([keyboard("Q"), keyboard("W"), keyboard("E")]);

        assert_eq!(
            play_control_hold_state_from_pressed_inputs(&pressed, &play_input),
            (true, true, true)
        );

        let pressed = HashSet::from([keyboard("Q")]);
        assert_eq!(
            play_control_hold_state_from_pressed_inputs(&pressed, &play_input),
            (true, false, false)
        );

        let pressed = HashSet::from([keyboard("W")]);
        assert_eq!(
            play_control_hold_state_from_pressed_inputs(&pressed, &play_input),
            (false, true, false)
        );
    }

    #[test]
    fn play_control_hold_state_keeps_legacy_and_default_e1_fallbacks() {
        let mut legacy_input = crate::config::play_input::default_profile_input();
        legacy_input.ui.bindings.retain(|entry| entry.action != Some(InputActionConfig::E1));
        legacy_input.start_key = Some("E".to_string());
        let legacy_play_input = play_option_input_for(&legacy_input, KeyMode::K7);
        let legacy_pressed =
            HashSet::from([(W_KEYBOARD_DEVICE_ID, PhysicalControl::KeyboardKey("E".to_string()))]);
        assert_eq!(
            play_control_hold_state_from_pressed_inputs(&legacy_pressed, &legacy_play_input),
            (true, false, true)
        );

        legacy_input.start_key = None;
        let fallback_play_input = play_option_input_for(&legacy_input, KeyMode::K7);
        let fallback_pressed =
            HashSet::from([(W_KEYBOARD_DEVICE_ID, PhysicalControl::KeyboardKey("Q".to_string()))]);
        assert_eq!(
            play_control_hold_state_from_pressed_inputs(&fallback_pressed, &fallback_play_input),
            (true, false, false)
        );
    }

    #[test]
    fn play_ready_is_blocked_while_e1_or_e2_is_held() {
        assert!(!play_ready_blocked_by_control_holds(false, false));
        assert!(play_ready_blocked_by_control_holds(true, false));
        assert!(play_ready_blocked_by_control_holds(false, true));
        assert!(play_ready_blocked_by_control_holds(true, true));
    }

    #[test]
    fn play_ready_waits_one_second_after_last_e1_or_e2_hold() {
        let last_control_hold_at = Instant::now();

        assert!(play_ready_blocked_by_recent_control_hold(
            Some(last_control_hold_at),
            last_control_hold_at + Duration::from_millis(999)
        ));
        assert!(play_ready_blocked_by_recent_control_hold(
            Some(last_control_hold_at),
            last_control_hold_at + Duration::from_secs(1)
        ));
        assert!(!play_ready_blocked_by_recent_control_hold(
            Some(last_control_hold_at),
            last_control_hold_at + Duration::from_millis(1_001)
        ));
    }

    #[test]
    fn play_ready_has_no_release_delay_without_prior_control_hold() {
        assert!(!play_ready_blocked_by_recent_control_hold(None, Instant::now()));
    }

    #[test]
    fn final_notes_fadeout_accepts_e1_and_e2_controls() {
        let keys = default_select_keys();

        assert!(play_fadeout_after_final_notes_control("Q", &keys));
        assert!(play_fadeout_after_final_notes_control("W", &keys));
        assert!(!play_fadeout_after_final_notes_control("Escape", &keys));
        assert!(!play_fadeout_after_final_notes_control("Z", &keys));
    }

    #[test]
    fn final_notes_fadeout_requires_active_finished_note_state() {
        let keys = default_select_keys();

        assert!(should_begin_play_fadeout_after_final_notes(
            "Q",
            &keys,
            true,
            false,
            bmz_gameplay::session::PlayState::Playing,
            true,
        ));
        assert!(should_begin_play_fadeout_after_final_notes(
            "Escape",
            &keys,
            true,
            false,
            bmz_gameplay::session::PlayState::Playing,
            true,
        ));
        assert!(!should_begin_play_fadeout_after_final_notes(
            "Q",
            &keys,
            false,
            false,
            bmz_gameplay::session::PlayState::Playing,
            true,
        ));
        assert!(!should_begin_play_fadeout_after_final_notes(
            "Escape",
            &keys,
            true,
            true,
            bmz_gameplay::session::PlayState::Playing,
            true,
        ));
        assert!(!should_begin_play_fadeout_after_final_notes(
            "Escape",
            &keys,
            true,
            false,
            bmz_gameplay::session::PlayState::Playing,
            false,
        ));
        assert!(!should_begin_play_fadeout_after_final_notes(
            "Q",
            &keys,
            true,
            false,
            bmz_gameplay::session::PlayState::Playing,
            false,
        ));
        assert!(!should_begin_play_fadeout_after_final_notes(
            "Q",
            &keys,
            true,
            true,
            bmz_gameplay::session::PlayState::Playing,
            true,
        ));
        assert!(!should_begin_play_fadeout_after_final_notes(
            "Q",
            &keys,
            true,
            false,
            bmz_gameplay::session::PlayState::Failed,
            true,
        ));
    }

    #[test]
    fn failed_transition_retire_sound_only_starts_on_new_failure() {
        use bmz_gameplay::session::PlayState;

        assert!(should_play_retire_sound_for_failed_transition(
            PlayState::Playing,
            PlayState::Failed
        ));
        assert!(!should_play_retire_sound_for_failed_transition(
            PlayState::Failed,
            PlayState::Failed
        ));
        assert!(!should_play_retire_sound_for_failed_transition(
            PlayState::Ready,
            PlayState::Failed
        ));
        assert!(!should_play_retire_sound_for_failed_transition(
            PlayState::Playing,
            PlayState::Finished
        ));
    }

    #[test]
    fn select_analog_scroll_delta_maps_scratch_bindings() {
        let gamepad_keys = SelectKeyBindings::from_profile(
            &ProfileConfig::new_default("default", "Default", 1).input,
        );
        // Axis1+ = scratch up (Previous = 負), Axis1- = scratch down (Next = 正)
        assert_eq!(select_analog_scroll_delta("Axis1", 4, &gamepad_keys), Some(-4));
        assert_eq!(select_analog_scroll_delta("Axis1", -4, &gamepad_keys), Some(4));
        assert_eq!(select_analog_scroll_delta("Axis2", -4, &gamepad_keys), None);
        assert_eq!(select_analog_scroll_delta("Axis1", 0, &gamepad_keys), None);
        assert_eq!(select_analog_scroll_delta("Axis3", 4, &gamepad_keys), None);
    }

    #[test]
    fn settings_edit_analog_scroll_uses_scratch_direction() {
        assert_eq!(settings_edit_direction_from_analog_scroll(3), 1);
        assert_eq!(settings_edit_direction_from_analog_scroll(-2), -1);
        assert_eq!(settings_edit_direction_from_analog_scroll(0), 0);
    }

    #[test]
    fn settings_edit_mouse_wheel_uses_scroll_direction() {
        assert_eq!(
            settings_edit_direction_from_mouse_wheel(MouseScrollDelta::LineDelta(0.0, 1.0)),
            1
        );
        assert_eq!(
            settings_edit_direction_from_mouse_wheel(MouseScrollDelta::PixelDelta(
                winit::dpi::PhysicalPosition::new(0.0, -12.0)
            )),
            -1
        );
    }

    #[test]
    fn play_analog_lane_cover_delta_maps_scratch_bindings() {
        let gamepad_keys = SelectKeyBindings::from_profile(
            &ProfileConfig::new_default("default", "Default", 1).input,
        );

        assert_eq!(play_analog_lane_cover_delta("Axis1", 4, &gamepad_keys), Some(-4));
        assert_eq!(play_analog_lane_cover_delta("Axis1", -4, &gamepad_keys), Some(4));
        assert_eq!(play_analog_lane_cover_delta("Axis2", -4, &gamepad_keys), None);
        assert_eq!(play_analog_lane_cover_delta("Axis1", 0, &gamepad_keys), None);
    }

    #[test]
    fn play_analog_green_number_uses_opposite_direction_from_lane_cover() {
        assert_eq!(green_number_change_from_analog_steps(1), GreenNumberChange::Up);
        assert_eq!(green_number_change_from_analog_steps(-1), GreenNumberChange::Down);
    }

    #[test]
    fn update_analog_scroll_buffer_suppresses_until_idle() {
        let mut buffer = 0;
        let mut suppress = true;
        // 回転継続中 (idle=false) は捨て続ける
        update_analog_scroll_buffer(&mut buffer, &mut suppress, false, 5);
        assert_eq!(buffer, 0);
        assert!(suppress);
        // 一度止まった後の tick から蓄積再開
        update_analog_scroll_buffer(&mut buffer, &mut suppress, true, 2);
        assert_eq!(buffer, 2);
        assert!(!suppress);
        update_analog_scroll_buffer(&mut buffer, &mut suppress, false, 3);
        assert_eq!(buffer, 5);
        // 通常時も idle で端数を破棄
        update_analog_scroll_buffer(&mut buffer, &mut suppress, true, 1);
        assert_eq!(buffer, 1);
    }

    #[test]
    fn take_analog_scroll_steps_keeps_remainder() {
        let mut buffer = 7;
        assert_eq!(take_analog_scroll_steps(&mut buffer, 3), 2);
        assert_eq!(buffer, 1);

        let mut buffer = -7;
        assert_eq!(take_analog_scroll_steps(&mut buffer, 3), -2);
        assert_eq!(buffer, -1);

        let mut buffer = 2;
        assert_eq!(take_analog_scroll_steps(&mut buffer, 3), 0);
        assert_eq!(buffer, 2);
    }

    #[test]
    fn target_cycle_maps_start_arrow_and_scratch_controls() {
        let keys = default_select_keys();
        let gamepad_keys = SelectKeyBindings::from_profile(
            &ProfileConfig::new_default("default", "Default", 1).input,
        );

        assert_eq!(
            target_cycle_from_key(PhysicalKey::Code(KeyCode::ArrowUp)),
            Some(TargetCycle::Next)
        );
        assert_eq!(
            target_cycle_from_key(PhysicalKey::Code(KeyCode::ArrowDown)),
            Some(TargetCycle::Previous)
        );
        assert_eq!(target_cycle_from_control("ScratchUp", &keys), Some(TargetCycle::Next));
        assert_eq!(target_cycle_from_control("ScratchDown", &keys), Some(TargetCycle::Previous));
        assert_eq!(target_cycle_from_control("Axis1+", &gamepad_keys), Some(TargetCycle::Next));
        assert_eq!(target_cycle_from_control("Axis1-", &gamepad_keys), Some(TargetCycle::Previous));
        assert_eq!(target_cycle_from_control("Axis2-", &gamepad_keys), None);
        assert_eq!(target_cycle_from_control("Axis2+", &gamepad_keys), None);
    }

    #[test]
    fn select_modifier_keys_are_handled_before_folder_back() {
        let keys = default_select_keys();
        assert!(!is_select_modifier_key(PhysicalKey::Code(KeyCode::ArrowLeft), &keys));
        assert!(is_select_modifier_key(PhysicalKey::Code(KeyCode::KeyW), &keys));
        assert!(!is_select_modifier_key(PhysicalKey::Code(KeyCode::KeyS), &keys));
        assert_eq!(
            select_action(
                PhysicalKey::Code(KeyCode::ArrowLeft),
                ElementState::Pressed,
                false,
                &keys
            ),
            Some(SelectAction::ExitFolder)
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyW), ElementState::Pressed, false, &keys),
            Some(SelectAction::ExitFolder)
        );
        assert_eq!(
            select_action(PhysicalKey::Code(KeyCode::KeyS), ElementState::Pressed, false, &keys),
            Some(SelectAction::ExitFolder)
        );
    }

    #[test]
    fn select_start_key_uses_profile_start_binding() {
        let keys = default_select_keys();
        assert!(is_select_start_key(PhysicalKey::Code(KeyCode::KeyQ), &keys));
        assert!(!is_select_start_key(PhysicalKey::Code(KeyCode::KeyW), &keys));
        assert!(!is_select_start_key(PhysicalKey::Code(KeyCode::KeyS), &keys));
    }

    #[test]
    fn select_key_bindings_map_e1_plus_key7_to_autoplay_option() {
        let keys = default_select_keys();

        assert!(keys.is_start("Q"));
        assert!(keys.is_ui_key7("V"));
        assert!(keys.is_enter("V"));
    }

    #[test]
    fn select_key_bindings_include_e3_action() {
        let keys = default_select_keys();

        assert!(keys.is_e3_action("E"));
    }

    #[test]
    fn select_key_bindings_expose_key2_for_gas_toggle() {
        let keys = default_select_keys();

        assert!(keys.is_start("Q"));
        assert!(keys.is_back("W"));
        assert!(keys.is_back("S"));
        assert!(keys.is_back("D"));
        assert!(keys.is_back("F"));
        assert!(keys.is_key2("S"));
    }

    #[test]
    fn select_key_bindings_expose_2p_keys_for_random2() {
        let keys = default_select_keys();

        assert!(keys.is_key8("M"));
        assert!(keys.is_key9("K"));
        assert!(keys.is_key10("Comma"));
        assert!(keys.is_key11("L"));
        assert!(keys.is_key12("Period"));
        assert!(keys.is_key13("Semicolon"));
        assert!(keys.is_key14("Slash"));
    }

    #[test]
    fn select_key_bindings_treat_2p_keys_as_ui_equivalents() {
        let keys = select_keys_with_full_2p_bindings();

        for control in ["M", "Comma", "Period", "Slash", "P2K7"] {
            assert!(keys.is_enter(control), "{control} should decide like odd 1P keys");
        }
        for control in ["K", "L", "Semicolon", "P2K6"] {
            assert!(keys.is_back(control), "{control} should go back like even 1P keys");
        }
        assert_eq!(keys.ui_lane_for_control("M"), Some(Lane::Key1));
        assert_eq!(keys.ui_lane_for_control("K"), Some(Lane::Key2));
        assert_eq!(keys.ui_lane_for_control("Comma"), Some(Lane::Key3));
        assert_eq!(keys.ui_lane_for_control("L"), Some(Lane::Key4));
        assert_eq!(keys.ui_lane_for_control("Period"), Some(Lane::Key5));
        assert_eq!(keys.ui_lane_for_control("Semicolon"), Some(Lane::Key6));
        assert_eq!(keys.ui_lane_for_control("Slash"), Some(Lane::Key7));
        assert_eq!(keys.ui_lane_for_control("P2K6"), Some(Lane::Key6));
        assert_eq!(keys.ui_lane_for_control("P2K7"), Some(Lane::Key7));
    }

    #[test]
    fn select_gauge_auto_shift_toggle_requires_start_then_key2() {
        let keys = default_select_keys();

        assert!(should_toggle_select_gauge_auto_shift("S", true, true, &keys));
        assert!(should_toggle_select_gauge_auto_shift("K", true, true, &keys));
        assert!(!should_toggle_select_gauge_auto_shift("Q", false, true, &keys));
        assert!(!should_toggle_select_gauge_auto_shift("Q", true, true, &keys));
        assert!(!should_toggle_select_gauge_auto_shift("W", true, false, &keys));
    }

    #[test]
    fn select_judge_auto_adjust_toggle_requires_start_then_key3() {
        let keys = default_select_keys();

        assert!(should_toggle_select_judge_auto_adjust("X", true, true, &keys));
        assert!(should_toggle_select_judge_auto_adjust("Comma", true, true, &keys));
        assert!(!should_toggle_select_judge_auto_adjust("X", false, true, &keys));
        assert!(!should_toggle_select_judge_auto_adjust("S", true, true, &keys));
        assert!(!should_toggle_select_judge_auto_adjust("W", true, false, &keys));
    }

    #[test]
    fn play_exit_hold_timer_uses_beatoraja_default_duration() {
        let default_hold = Duration::from_millis(1_000);
        let start = Instant::now();
        let mut held_since = None;

        update_play_exit_hold_started_at(&mut held_since, true, false, start);
        assert!(held_since.is_none());

        update_play_exit_hold_started_at(&mut held_since, true, true, start);
        assert_eq!(held_since, Some(start));
        assert!(!play_exit_hold_elapsed(held_since, start + default_hold / 2, default_hold));
        assert!(play_exit_hold_elapsed(held_since, start + default_hold, default_hold));

        update_play_exit_hold_started_at(&mut held_since, false, true, start + default_hold);
        assert!(held_since.is_none());
    }

    #[test]
    fn decide_control_action_skips_with_1p_and_2p_decide_keys() {
        let keys = select_keys_with_full_2p_bindings();

        assert_eq!(decide_control_action("Z", &keys), Some(DecideAction::Confirm));
        assert_eq!(decide_control_action("M", &keys), Some(DecideAction::Confirm));
        assert_eq!(decide_control_action("P2K7", &keys), Some(DecideAction::Confirm));
        assert_eq!(decide_control_action("S", &keys), None);
        assert_eq!(decide_control_action("P2K6", &keys), None);
    }

    #[test]
    fn decide_cancel_chord_accepts_e1_e2_and_e2_e3() {
        assert!(decide_cancel_chord_pressed(true, true, false));
        assert!(decide_cancel_chord_pressed(false, true, true));
        assert!(decide_cancel_chord_pressed(true, true, true));
        assert!(!decide_cancel_chord_pressed(true, false, true));
        assert!(!decide_cancel_chord_pressed(false, true, false));
    }

    #[test]
    fn decide_fadeout_scene_elapsed_enters_scene_tail_on_early_skip() {
        let elapsed = decide_fadeout_scene_elapsed(
            Duration::from_millis(100),
            Duration::from_millis(250),
            Duration::from_millis(2500),
            Duration::from_millis(1000),
            DecideFadeoutSceneTiming::DefaultTail,
        );

        assert_eq!(elapsed, Duration::from_millis(1750));
    }

    #[test]
    fn decide_fadeout_scene_elapsed_stretches_detected_tail_fadeout() {
        let elapsed = decide_fadeout_scene_elapsed(
            Duration::from_millis(100),
            Duration::from_millis(500),
            Duration::from_millis(2500),
            Duration::from_millis(1000),
            DecideFadeoutSceneTiming::TailStart(Duration::from_millis(2300)),
        );

        assert_eq!(elapsed, Duration::from_millis(2400));
    }

    #[test]
    fn decide_fadeout_scene_elapsed_stays_direct_when_timer_fadeout_exists() {
        let elapsed = decide_fadeout_scene_elapsed(
            Duration::from_millis(100),
            Duration::from_millis(0),
            Duration::from_millis(2500),
            Duration::from_millis(500),
            DecideFadeoutSceneTiming::DirectOnly,
        );

        assert_eq!(elapsed, Duration::from_millis(100));
    }

    #[test]
    fn decide_fadeout_scene_elapsed_does_not_rewind_auto_fadeout() {
        let elapsed = decide_fadeout_scene_elapsed(
            Duration::from_millis(2500),
            Duration::from_millis(250),
            Duration::from_millis(2500),
            Duration::from_millis(1000),
            DecideFadeoutSceneTiming::DefaultTail,
        );

        assert_eq!(elapsed, Duration::from_millis(2750));
    }

    #[test]
    fn decide_scene_fadeout_tail_start_detects_scene_end_black_fade() {
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 6,
                "w": 1920,
                "h": 1080,
                "scene": 2500,
                "fadeout": 1000,
                "destination": [
                    { "id": -110, "loop": 800, "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 1920, "h": 1080, "a": 255 },
                        { "time": 800, "a": 0 }
                    ] },
                    { "id": -110, "loop": 2500, "dst": [
                        { "time": 2300, "x": 0, "y": 0, "w": 1920, "h": 1080, "a": 0 },
                        { "time": 2500, "a": 255 }
                    ] }
                ]
            }
            "#,
        )
        .unwrap();

        assert_eq!(decide_scene_fadeout_tail_start(Some(&document)), Some(2300));
    }

    #[test]
    fn decide_scene_fadeout_tail_start_ignores_scene_tail_when_timer_fadeout_exists() {
        let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 6,
                "w": 1920,
                "h": 1080,
                "scene": 2500,
                "fadeout": 500,
                "destination": [
                    { "id": -110, "loop": 2000, "dst": [
                        { "time": 1500, "x": 0, "y": 0, "w": 1920, "h": 1080, "a": 0 },
                        { "time": 2000, "a": 255 }
                    ] },
                    { "id": -110, "loop": 500, "timer": 2, "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 1920, "h": 1080, "a": 0 },
                        { "time": 500, "a": 255 }
                    ] }
                ]
            }
            "#,
        )
        .unwrap();

        assert!(document_has_fadeout_timer_black(&document));
        assert_eq!(
            decide_fadeout_scene_timing(Some(&document)),
            DecideFadeoutSceneTiming::DirectOnly
        );
        assert_eq!(decide_scene_fadeout_tail_start(Some(&document)), None);
    }

    #[test]
    fn bga_option_cycles_on_auto_off() {
        assert!(matches!(cycle_bga_option(BgaModeConfig::On), BgaModeConfig::Auto));
        assert!(matches!(cycle_bga_option(BgaModeConfig::Auto), BgaModeConfig::Off));
        assert!(matches!(cycle_bga_option(BgaModeConfig::Off), BgaModeConfig::On));
    }

    #[test]
    fn volume_f32_to_unit_clamps_and_rounds() {
        assert_eq!(volume_f32_to_unit(-0.5), 0);
        assert_eq!(volume_f32_to_unit(0.345), 35);
        assert_eq!(volume_f32_to_unit(1.5), 100);
    }

    #[test]
    fn result_action_accepts_retry_and_leave_keys() {
        assert_eq!(
            result_action(PhysicalKey::Code(KeyCode::KeyR), ElementState::Pressed, false),
            Some(ResultAction::Retry)
        );
        assert_eq!(
            result_action(PhysicalKey::Code(KeyCode::Enter), ElementState::Pressed, false),
            Some(ResultAction::Leave)
        );
        assert_eq!(
            result_action(PhysicalKey::Code(KeyCode::Escape), ElementState::Pressed, false),
            Some(ResultAction::Leave)
        );
    }

    #[test]
    fn result_action_rejects_releases_repeats_and_other_keys() {
        assert_eq!(
            result_action(PhysicalKey::Code(KeyCode::KeyR), ElementState::Released, false),
            None
        );
        assert_eq!(
            result_action(PhysicalKey::Code(KeyCode::Escape), ElementState::Pressed, true),
            None
        );
        assert_eq!(
            result_action(PhysicalKey::Code(KeyCode::Space), ElementState::Pressed, false),
            None
        );
    }

    #[test]
    fn result_exit_skip_key_accepts_enter_and_escape_only_on_pressed() {
        assert!(result_exit_skip_key(
            PhysicalKey::Code(KeyCode::Enter),
            ElementState::Pressed,
            false
        ));
        assert!(result_exit_skip_key(
            PhysicalKey::Code(KeyCode::Escape),
            ElementState::Pressed,
            false
        ));
        assert!(!result_exit_skip_key(
            PhysicalKey::Code(KeyCode::Enter),
            ElementState::Released,
            false
        ));
        assert!(!result_exit_skip_key(
            PhysicalKey::Code(KeyCode::Escape),
            ElementState::Pressed,
            true
        ));
        assert!(!result_exit_skip_key(
            PhysicalKey::Code(KeyCode::Space),
            ElementState::Pressed,
            false
        ));
    }

    #[test]
    fn result_exit_skip_waits_for_animation_and_holds_final_frame_once() {
        let animation_duration = Duration::from_millis(1_000);
        let fadeout = Duration::from_millis(3_000);

        assert!(!result_exit_transition_ready(
            Duration::from_millis(999),
            fadeout,
            animation_duration,
            true,
            false,
        ));
        assert!(!result_exit_transition_ready(
            animation_duration,
            fadeout,
            animation_duration,
            true,
            false,
        ));
        assert!(result_exit_transition_ready(
            animation_duration,
            fadeout,
            animation_duration,
            true,
            true,
        ));
    }

    #[test]
    fn result_exit_without_skip_still_waits_for_skin_fadeout() {
        let fadeout = Duration::from_millis(3_000);
        let animation_duration = Duration::from_millis(1_000);

        assert!(!result_exit_transition_ready(
            animation_duration,
            fadeout,
            animation_duration,
            false,
            false,
        ));
        assert!(result_exit_transition_ready(fadeout, fadeout, animation_duration, false, false,));
    }

    #[test]
    fn lane_skips_result_exit_matches_1p_and_2p_requested_keys() {
        for lane in [Lane::Key1, Lane::Key3, Lane::Key8, Lane::Key10, Lane::Key12, Lane::Key14] {
            assert!(lane_skips_result_exit(lane), "{lane:?} should skip");
        }
        for lane in [
            Lane::Scratch,
            Lane::Key2,
            Lane::Key4,
            Lane::Key5,
            Lane::Key6,
            Lane::Key7,
            Lane::Key9,
            Lane::Key11,
            Lane::Key13,
            Lane::Scratch2,
        ] {
            assert!(!lane_skips_result_exit(lane), "{lane:?} should not skip");
        }
    }

    #[test]
    fn result_exit_lanes_match_requested_mapping() {
        // BMZ では Key2 を「戻る」系に寄せるため、終了開始から外す。
        for lane in [Lane::Key1, Lane::Key3, Lane::Key4, Lane::Key5, Lane::Key7] {
            assert!(lane_starts_result_exit(lane), "{lane:?} should start result exit");
        }
        // Key6 は CHANGE_GRAPH、scratch は無割り当て。
        for lane in [Lane::Scratch, Lane::Key2, Lane::Key6] {
            assert!(!lane_starts_result_exit(lane), "{lane:?} should not start result exit");
        }
    }

    #[test]
    fn result_gauge_graph_cycle_matches_beatoraja_order() {
        assert_eq!(cycle_result_gauge_graph_type(GaugeType::Normal as i32), GaugeType::Hard as i32);
        assert_eq!(cycle_result_gauge_graph_type(GaugeType::Hard as i32), GaugeType::ExHard as i32);
        assert_eq!(
            cycle_result_gauge_graph_type(GaugeType::Hazard as i32),
            GaugeType::AssistEasy as i32
        );
        assert_eq!(
            cycle_result_gauge_graph_type(GaugeType::Class as i32),
            GaugeType::ExClass as i32
        );
        assert_eq!(
            cycle_result_gauge_graph_type(GaugeType::ExHardClass as i32),
            GaugeType::Class as i32
        );
    }

    #[test]
    fn result_skin_event_90_toggles_favorite_without_invisible_state() {
        assert_eq!(result_skin_click_action(90), Some(ResultSkinClickAction::ToggleFavoriteChart));
        assert_eq!(
            result_skin_click_action(SKIN_EVENT_DAILY_STATISTICS_RESET),
            Some(ResultSkinClickAction::ResetDailyStatistics)
        );
        assert_eq!(
            result_skin_click_action(SKIN_EVENT_RESULT_PANEL_IR),
            Some(ResultSkinClickAction::SetPanel(1))
        );
        assert_eq!(
            result_skin_click_action(SKIN_EVENT_IR_SCOPE_GLOBAL),
            Some(ResultSkinClickAction::SelectIrScope(
                crate::screens::result_ir::ResultRankingTab::Global
            ))
        );
        assert_eq!(
            result_skin_click_action(SKIN_EVENT_IR_SCOPE_RIVAL),
            Some(ResultSkinClickAction::SelectIrScope(
                crate::screens::result_ir::ResultRankingTab::SelfAndRivals
            ))
        );
        assert_eq!(
            result_skin_click_action(SKIN_EVENT_IR_SCOPE_TOGGLE),
            Some(ResultSkinClickAction::ToggleIrScope)
        );
        assert_eq!(result_skin_click_action(91), None);
    }

    #[test]
    fn result_skin_replay_events_map_all_four_slots() {
        assert_eq!(result_skin_click_action(19), Some(ResultSkinClickAction::SaveReplay(0)));
        assert_eq!(result_skin_click_action(316), Some(ResultSkinClickAction::SaveReplay(1)));
        assert_eq!(result_skin_click_action(317), Some(ResultSkinClickAction::SaveReplay(2)));
        assert_eq!(result_skin_click_action(318), Some(ResultSkinClickAction::SaveReplay(3)));
        assert_eq!(result_skin_click_action(319), None);
    }

    #[test]
    fn select_skin_cover_events_toggle_sudden_and_hidden_independently() {
        assert_eq!(toggled_select_sudden(LaneEffectConfig::Off), LaneEffectConfig::Sudden);
        assert_eq!(toggled_select_sudden(LaneEffectConfig::Hidden), LaneEffectConfig::HiddenSudden);
        assert_eq!(toggled_select_sudden(LaneEffectConfig::HiddenSudden), LaneEffectConfig::Hidden);

        assert_eq!(toggled_select_hidden(LaneEffectConfig::Off), LaneEffectConfig::Hidden);
        assert_eq!(toggled_select_hidden(LaneEffectConfig::Sudden), LaneEffectConfig::HiddenSudden);
        assert_eq!(toggled_select_hidden(LaneEffectConfig::HiddenSudden), LaneEffectConfig::Sudden);
    }

    #[test]
    fn result_panel_toggle_requires_supported_skin_and_ir() {
        assert_eq!(toggled_result_panel(1, true, true), Some(2));
        assert_eq!(toggled_result_panel(2, true, true), Some(1));
        assert_eq!(toggled_result_panel(0, true, true), None);
        assert_eq!(toggled_result_panel(1, false, true), None);
        assert_eq!(toggled_result_panel(1, true, false), None);
    }

    #[test]
    fn result_panel_arrow_keys_match_luxe_flat_direction() {
        assert_eq!(
            result_panel_for_control(&PhysicalControl::KeyboardKey("ArrowLeft".to_string())),
            Some(2)
        );
        assert_eq!(
            result_panel_for_control(&PhysicalControl::KeyboardKey("ArrowRight".to_string())),
            Some(1)
        );
        assert_eq!(
            result_panel_for_control(&PhysicalControl::KeyboardKey("ArrowUp".to_string())),
            None
        );
    }

    #[test]
    fn result_panel_direct_selection_matches_tab_availability() {
        assert_eq!(selected_result_panel(1, 2, true, true), Some(2));
        assert_eq!(selected_result_panel(2, 1, true, true), Some(1));
        assert_eq!(selected_result_panel(2, 1, true, false), None);
        assert_eq!(selected_result_panel(1, 2, true, false), Some(2));
        assert_eq!(selected_result_panel(2, 2, true, true), None);
        assert_eq!(selected_result_panel(1, 2, false, true), None);
    }

    #[test]
    fn result_panel_support_requires_default_and_runtime_draw_gate() {
        let document: SkinDocument = serde_json::from_value(serde_json::json!({
            "type": 7,
            "resultPanelDefault": 2,
            "destination": [{
                "id": "panel",
                "draw": "result_panel(2)",
                "dst": [{"x": 0, "y": 0, "w": 1, "h": 1}]
            }]
        }))
        .unwrap();
        assert!(result_panel_supported(&document));

        let without_gate: SkinDocument = serde_json::from_value(serde_json::json!({
            "type": 7,
            "resultPanelDefault": 2,
            "destination": []
        }))
        .unwrap();
        assert!(!result_panel_supported(&without_gate));
    }

    #[test]
    fn course_intermediate_result_only_with_active_course_and_no_course_result() {
        // active_course 保持 + finished_play あり + finished_course 無し → 中間リザルト。
        assert!(is_course_intermediate_result(true, false, true));
        // コース最終結果 (finished_course あり) は中間リザルトではない。
        assert!(!is_course_intermediate_result(true, true, true));
        // 単曲 (非コース) リザルトは中間リザルトではない。
        assert!(!is_course_intermediate_result(false, false, true));
        // 結果未表示なら中間リザルトではない。
        assert!(!is_course_intermediate_result(true, false, false));
    }

    #[test]
    fn course_intermediate_result_keeps_rounded_clear_type_for_skin_display() {
        let mut finished = debug_boot_finished_play_session();
        finished.result.clear_type = ClearType::Normal;
        finished.summary.clear_type = ClearType::NoPlay;

        assert_eq!(finished.summary.clear_type, ClearType::NoPlay);
    }

    #[test]
    fn course_intermediate_result_skin_ops_use_raw_clear_result() {
        assert!(!result_failed_for_skin_ops(ClearType::NoPlay, Some(ClearType::Normal)));
        assert!(result_failed_for_skin_ops(ClearType::NoPlay, Some(ClearType::Failed)));
        assert!(result_failed_for_skin_ops(ClearType::NoPlay, None));
    }

    #[test]
    fn course_intermediate_exit_action_finishes_failed_or_final_stage() {
        assert_eq!(
            course_intermediate_exit_action_for_state(false, true),
            ResultExitAction::AdvanceCourse
        );
        assert_eq!(
            course_intermediate_exit_action_for_state(true, true),
            ResultExitAction::FinishCourse
        );
        assert_eq!(
            course_intermediate_exit_action_for_state(false, false),
            ResultExitAction::FinishCourse
        );
    }

    #[test]
    fn course_stage_result_is_shown_for_next_failed_or_final_stage() {
        assert!(should_show_course_stage_result(false, true, true));
        assert!(should_show_course_stage_result(true, true, false));
        assert!(should_show_course_stage_result(false, false, false));
        assert!(!should_show_course_stage_result(false, true, false));
    }

    #[test]
    fn retry_preload_always_builds_fresh_audio_for_the_retried_chart() {
        assert_eq!(
            retry_preload_kind(ResultRetryMode::SameArrange, true),
            RetryPreloadKind::CachedChartWithFreshAudio
        );
        assert_eq!(
            retry_preload_kind(ResultRetryMode::SameArrange, false),
            RetryPreloadKind::ReimportedChartWithFreshAudio
        );
        assert_eq!(
            retry_preload_kind(ResultRetryMode::DifferentArrange, true),
            RetryPreloadKind::ReimportedChartWithFreshAudio
        );
        assert_eq!(
            retry_preload_kind(ResultRetryMode::DifferentArrange, false),
            RetryPreloadKind::ReimportedChartWithFreshAudio
        );
    }

    #[test]
    fn result_action_resolves_from_held_lanes() {
        // beatoraja 準拠: Key5 のみ → 別配置 (REPLAY_DIFFERENT)。
        assert_eq!(
            result_action_for_held_lanes(true, false),
            Some(ResultRetryMode::DifferentArrange)
        );
        // Key7 のみ → 同配置 (REPLAY_SAME)。
        assert_eq!(result_action_for_held_lanes(false, true), Some(ResultRetryMode::SameArrange));
        // 両押し → 同配置 (ユーザー仕様)。
        assert_eq!(result_action_for_held_lanes(true, true), Some(ResultRetryMode::SameArrange));
        // どちらも非押下 → 選曲へ戻る。
        assert_eq!(result_action_for_held_lanes(false, false), None);
    }

    #[test]
    fn hispeed_action_maps_left_and_right_presses() {
        assert_eq!(
            hispeed_action(PhysicalKey::Code(KeyCode::ArrowLeft), ElementState::Pressed, false),
            Some(HispeedChange::Down)
        );
        assert_eq!(
            hispeed_action(PhysicalKey::Code(KeyCode::ArrowRight), ElementState::Pressed, false),
            Some(HispeedChange::Up)
        );
    }

    #[test]
    fn hispeed_action_rejects_releases_and_other_keys() {
        assert_eq!(
            hispeed_action(PhysicalKey::Code(KeyCode::ArrowLeft), ElementState::Released, false),
            None
        );
        assert_eq!(
            hispeed_action(PhysicalKey::Code(KeyCode::ArrowUp), ElementState::Pressed, false),
            None
        );
    }

    #[test]
    fn adjusted_hispeed_uses_configured_step_and_clamps_range() {
        assert_eq!(adjusted_hispeed(2.0, HispeedChange::Up, 0.25), 2.25);
        assert_eq!(adjusted_hispeed(2.0, HispeedChange::Down, 0.25), 1.75);
        assert_eq!(adjusted_hispeed(2.0, HispeedChange::Up, 0.5), 2.5);
        assert_eq!(adjusted_hispeed(10.0, HispeedChange::Up, 0.5), 10.0);
        assert_eq!(adjusted_hispeed(0.5, HispeedChange::Down, 0.5), 0.5);
    }

    #[test]
    fn pending_hispeed_changes_use_displayed_mode_without_mutating_profile() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let profile_hispeed = profile.lane.hispeed;
        let mut lane = PendingPlayLaneState {
            hispeed: 2.0,
            hispeed_mode: HispeedMode::Floating,
            target_green_number: 300,
            lane_cover: 0.0,
            lift: 0.0,
            lane_cover_visible: true,
            lane_cover_changing: false,
            hsfix_base_bpm: 120.0,
            hispeed_auto_adjust: false,
        };

        assert!(apply_pending_play_lane_action_to_state(
            &mut lane,
            PlayLaneAction::Hispeed(HispeedChange::Up),
            &profile,
            120.0,
            false,
        ));

        assert_eq!(lane.hispeed, 2.5);
        assert_eq!(lane.target_green_number, 300);
        assert_eq!(profile.lane.hispeed, profile_hispeed);
    }

    #[test]
    fn pending_green_number_change_switches_displayed_state_to_floating() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let mut lane = PendingPlayLaneState {
            hispeed: 2.0,
            hispeed_mode: HispeedMode::Normal,
            target_green_number: 300,
            lane_cover: 0.0,
            lift: 0.0,
            lane_cover_visible: true,
            lane_cover_changing: true,
            hsfix_base_bpm: 120.0,
            hispeed_auto_adjust: false,
        };

        assert!(apply_pending_play_lane_action_to_state(
            &mut lane,
            PlayLaneAction::GreenNumberDelta(1),
            &profile,
            120.0,
            false,
        ));

        assert_eq!(lane.hispeed_mode, HispeedMode::Floating);
        assert_eq!(lane.target_green_number, 601);
        let expected =
            crate::screens::play_snapshot::hispeed_for_green_number_values(601.0, 1.0, 120.0, 1.0);
        assert!((lane.hispeed - expected).abs() < 0.000_1, "hispeed={}", lane.hispeed);
    }

    #[test]
    fn pending_lane_state_matches_no_speed_control_rules() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let mut lane = PendingPlayLaneState {
            hispeed: 2.0,
            hispeed_mode: HispeedMode::Floating,
            target_green_number: 300,
            lane_cover: 0.0,
            lift: 0.0,
            lane_cover_visible: true,
            lane_cover_changing: true,
            hsfix_base_bpm: 120.0,
            hispeed_auto_adjust: false,
        };

        assert!(!apply_pending_play_lane_action_to_state(
            &mut lane,
            PlayLaneAction::Hispeed(HispeedChange::Up),
            &profile,
            120.0,
            true,
        ));
        assert!(apply_pending_play_lane_action_to_state(
            &mut lane,
            PlayLaneAction::LaneCoverDelta(-LANE_COVER_STEP),
            &profile,
            120.0,
            true,
        ));
        assert_eq!(lane.hispeed, 2.0);
        assert!((lane.lane_cover - LANE_COVER_STEP).abs() < f32::EPSILON);
    }

    #[test]
    fn pending_lane_actions_replay_once_on_loaded_session() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.lane.target_green_number = 300;
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(app_test_chart()),
            &profile,
            crate::screens::play_session::PlaySessionOptions::default(),
        );
        let initial_hispeed = session.hispeed;
        let hispeed_step = hispeed_step_for_profile(&profile, session.hispeed_mode);

        replay_pending_play_lane_actions(
            &mut session,
            &[PlayLaneAction::Hispeed(HispeedChange::Up)],
            &profile,
            false,
        );

        assert_eq!(session.hispeed, initial_hispeed + hispeed_step);
        replay_pending_play_lane_actions(
            &mut session,
            &[PlayLaneAction::LaneCoverDelta(-LANE_COVER_STEP)],
            &profile,
            false,
        );
        assert!((session.lane_cover - LANE_COVER_STEP).abs() < f32::EPSILON);
    }

    #[test]
    fn floating_hispeed_recalculation_uses_hsfix_base_before_chart_start() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.lane.target_green_number = 300;
        let mut chart = app_test_chart();
        chart.metadata.initial_bpm = 120.0;
        chart.timing_events.push(bmz_chart::model::TimingEvent {
            tick: bmz_core::time::ChartTick(48),
            time: TimeUs(1_000_000),
            kind: bmz_chart::model::TimingEventKind::BpmChange { bpm: 240.0 },
        });
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(chart),
            &profile,
            crate::screens::play_session::PlaySessionOptions {
                hs_fix: HsFixOption::MaxBpm,
                ..Default::default()
            },
        );
        session.lane_cover = 0.25;

        reset_floating_hispeed_if_enabled(&mut session, false);

        assert_eq!(session.hsfix_base_bpm, 240.0);
        assert!((session.hispeed - 1.5).abs() < 0.000_1, "hispeed={}", session.hispeed);
    }

    #[test]
    fn floating_hispeed_recalculation_uses_current_bpm_after_chart_start() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.lane.hispeed_auto_adjust = true;
        profile.lane.target_green_number = 300;
        let mut chart = app_test_chart();
        chart.metadata.initial_bpm = 120.0;
        chart.timing_events.push(bmz_chart::model::TimingEvent {
            tick: bmz_core::time::ChartTick(48),
            time: TimeUs(1_000_000),
            kind: bmz_chart::model::TimingEventKind::BpmChange { bpm: 240.0 },
        });
        let frame = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(chart),
            &profile,
            crate::screens::play_session::PlaySessionOptions {
                hs_fix: HsFixOption::MaxBpm,
                ..Default::default()
            },
        );
        session.audio_clock =
            bmz_audio::clock::AudioClock::with_position(48_000, 0, 0, frame, true);

        apply_lane_cover_step_to_session(&mut session, -0.25, false);

        assert_eq!(session.hsfix_base_bpm, 240.0);
        assert!((session.hispeed - 3.0).abs() < 0.000_1, "hispeed={}", session.hispeed);
    }

    #[test]
    fn lane_cover_change_uses_hsfix_base_when_hispeed_auto_adjust_is_off() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.lane.hispeed_auto_adjust = false;
        profile.lane.target_green_number = 300;
        let mut chart = app_test_chart();
        chart.metadata.initial_bpm = 120.0;
        chart.timing_events.push(bmz_chart::model::TimingEvent {
            tick: bmz_core::time::ChartTick(48),
            time: TimeUs(1_000_000),
            kind: bmz_chart::model::TimingEventKind::BpmChange { bpm: 240.0 },
        });
        let frame = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(chart),
            &profile,
            crate::screens::play_session::PlaySessionOptions {
                hs_fix: HsFixOption::MaxBpm,
                ..Default::default()
            },
        );
        session.audio_clock =
            bmz_audio::clock::AudioClock::with_position(48_000, 0, 0, frame, true);

        apply_lane_cover_step_to_session(&mut session, -0.25, false);

        assert!(!session.hispeed_auto_adjust);
        assert!((session.hispeed - 1.5).abs() < 0.000_1, "hispeed={}", session.hispeed);
    }

    #[test]
    fn egui_lane_profile_cover_change_keeps_runtime_nhs_hispeed() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let before = profile.lane.clone();
        let mut edited = profile.lane.clone();
        edited.sudden = 250;
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(app_test_chart()),
            &profile,
            crate::screens::play_session::PlaySessionOptions::default(),
        );
        session.hispeed = 3.5;

        assert!(apply_profile_lane_settings_to_session(&mut session, &before, &edited, false));
        assert!((session.hispeed - 3.5).abs() < f32::EPSILON);
        assert!((session.lane_cover - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn egui_lane_profile_target_change_recalculates_fhs_hispeed() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        let before = profile.lane.clone();
        let mut edited = profile.lane.clone();
        edited.target_green_number = 320;
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(app_test_chart()),
            &profile,
            crate::screens::play_session::PlaySessionOptions {
                hs_fix: HsFixOption::StartBpm,
                ..Default::default()
            },
        );

        assert!(apply_profile_lane_settings_to_session(&mut session, &before, &edited, false));
        assert_eq!(session.hispeed_mode, HispeedMode::Floating);
        assert_eq!(session.target_green_number, 320);
        assert!((session.hispeed - 3.75).abs() < 0.000_1, "hispeed={}", session.hispeed);
    }

    #[test]
    fn chart_started_for_system_sound_waits_until_running_clock_reaches_zero() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let frame = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(app_test_chart()),
            &profile,
            crate::screens::play_session::PlaySessionOptions::default(),
        );

        assert!(!chart_started_for_system_sound(&session));

        session.audio_clock =
            bmz_audio::clock::AudioClock::with_position(48_000, 0, -1_000_000, frame.clone(), true);
        assert!(!chart_started_for_system_sound(&session));

        frame.store(48_000, std::sync::atomic::Ordering::Relaxed);
        assert!(chart_started_for_system_sound(&session));
    }

    #[test]
    fn lane_cover_step_moves_one_profile_unit() {
        assert!((LANE_COVER_STEP - 0.001).abs() < f32::EPSILON);
    }

    #[test]
    fn lane_cover_step_accelerates_on_key_repeat() {
        assert_eq!(
            lane_cover_step(PhysicalKey::Code(KeyCode::ArrowUp), ElementState::Pressed, false),
            Some(0.001)
        );
        assert_eq!(
            lane_cover_step(PhysicalKey::Code(KeyCode::ArrowUp), ElementState::Pressed, true),
            Some(0.01)
        );
        assert_eq!(
            lane_cover_step(PhysicalKey::Code(KeyCode::ArrowDown), ElementState::Pressed, true),
            Some(-0.01)
        );
    }

    #[test]
    fn lane_cover_step_clamps_sudden_and_lift_to_combined_range() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(app_test_chart()),
            &profile,
            crate::screens::play_session::PlaySessionOptions::default(),
        );

        session.lift = 0.2;
        session.lane_cover = 0.79;
        session.lane_cover_visible = true;
        assert!(apply_lane_cover_step_to_session(&mut session, -0.02, false));
        assert!((session.lane_cover - 0.8).abs() < 0.000_01);

        session.lane_cover = 0.3;
        session.lift = 0.69;
        session.lane_cover_visible = false;
        assert!(apply_lane_cover_step_to_session(&mut session, 0.02, false));
        assert!((session.lift - 0.7).abs() < 0.000_01);
    }

    #[test]
    fn play_start_double_press_registers_within_window() {
        let mut last = None;
        let t0 = Instant::now();
        assert!(!register_play_start_double_press(&mut last, t0));
        assert_eq!(last, Some(t0));

        let t1 = t0 + Duration::from_millis(200);
        assert!(register_play_start_double_press(&mut last, t1));
        assert_eq!(last, None);
    }

    #[test]
    fn play_start_double_press_expires_outside_window() {
        let mut last = None;
        let t0 = Instant::now();
        assert!(!register_play_start_double_press(&mut last, t0));

        let t1 = t0 + PLAY_START_DOUBLE_PRESS_WINDOW + Duration::from_millis(1);
        assert!(!register_play_start_double_press(&mut last, t1));
        assert_eq!(last, Some(t1));
    }

    #[test]
    fn toggle_lane_cover_visibility_flips_sudden_display() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(app_test_chart()),
            &profile,
            crate::screens::play_session::PlaySessionOptions::default(),
        );
        session.lane_cover_visible = true;

        toggle_lane_cover_visibility(&mut session, false);
        assert!(!session.lane_cover_visible);

        toggle_lane_cover_visibility(&mut session, false);
        assert!(session.lane_cover_visible);
    }

    #[test]
    fn green_number_step_switches_normal_hispeed_to_floating() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(app_test_chart()),
            &profile,
            crate::screens::play_session::PlaySessionOptions::default(),
        );

        assert!(apply_green_number_step_to_session(&mut session, 1, false));

        assert_eq!(session.hispeed_mode, HispeedMode::Floating);
        assert_eq!(session.target_green_number, 601);
        assert!(session.hispeed < 2.0);
    }

    #[test]
    fn green_number_step_respects_no_speed_constraint() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(app_test_chart()),
            &profile,
            crate::screens::play_session::PlaySessionOptions::default(),
        );

        assert!(!apply_green_number_step_to_session(&mut session, 1, true));

        assert_eq!(session.hispeed_mode, HispeedMode::Normal);
        assert_eq!(session.target_green_number, 300);
        assert_eq!(session.hispeed, 2.0);
    }

    #[test]
    fn floating_hispeed_change_keeps_target_green_during_play() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.lane.target_green_number = 300;
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(app_test_chart()),
            &profile,
            crate::screens::play_session::PlaySessionOptions {
                hs_fix: HsFixOption::StartBpm,
                ..Default::default()
            },
        );

        let hispeed = session.hispeed;
        apply_hispeed_change_to_session(&mut session, HispeedChange::Up, 0.5);

        assert_eq!(session.hispeed, hispeed + 0.5);
        assert_eq!(session.target_green_number, 300);
    }

    #[test]
    fn e1_hispeed_change_keeps_target_green_during_play() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.lane.target_green_number = 300;
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(app_test_chart()),
            &profile,
            crate::screens::play_session::PlaySessionOptions {
                hs_fix: HsFixOption::StartBpm,
                ..Default::default()
            },
        );

        assert!(apply_play_option_control_to_session(
            &mut session,
            PlayOptionControl::Hispeed(HispeedChange::Up),
            false,
            0.5,
        ));

        assert_eq!(session.target_green_number, 300);
    }

    #[test]
    fn active_lane_state_keeps_green_number_captured_when_switching_to_fhs() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let mut session = crate::screens::play_session::build_game_session(
            std::sync::Arc::new(app_test_chart()),
            &profile,
            crate::screens::play_session::PlaySessionOptions::default(),
        );
        let frame = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        session.audio_clock =
            bmz_audio::clock::AudioClock::with_position(48_000, 0, 0, frame, true);
        let expected_target = current_green_number(&session, session.audio_clock.now());
        assert_ne!(expected_target, session.target_green_number);

        assert!(apply_play_option_control_to_session(
            &mut session,
            PlayOptionControl::ToggleHispeedMode,
            false,
            0.25,
        ));
        assert_eq!(session.hispeed_mode, HispeedMode::Floating);
        assert_eq!(session.target_green_number, expected_target);

        // NHSへ戻ってHSを変更しても、終了時の現在緑数字でtargetを上書きしない。
        session.hispeed = 1.0;
        assert!(apply_play_option_control_to_session(
            &mut session,
            PlayOptionControl::ToggleHispeedMode,
            false,
            0.25,
        ));
        let state = active_lane_state_for_session(&session);

        assert_eq!(state.hispeed_mode, HispeedMode::Normal);
        assert_eq!(state.target_green_number, expected_target);
    }

    #[test]
    fn play_option_control_maps_seven_key_lane_and_scratch_targets() {
        let input = crate::config::play_input::default_profile_input();
        let keys = SelectKeyBindings::from_profile(&input);
        let play_input = play_option_input_for(&input, KeyMode::K7);

        assert_eq!(
            keyboard_play_option("W", true, true, &keys, &play_input, &input),
            Some(PlayOptionControl::ToggleHispeedMode)
        );
        assert_eq!(
            keyboard_play_option("Z", true, false, &keys, &play_input, &input),
            Some(PlayOptionControl::Hispeed(HispeedChange::Down))
        );
        assert_eq!(
            keyboard_play_option("V", true, false, &keys, &play_input, &input),
            Some(PlayOptionControl::Hispeed(HispeedChange::Down))
        );
        assert_eq!(
            keyboard_play_option("S", true, false, &keys, &play_input, &input),
            Some(PlayOptionControl::Hispeed(HispeedChange::Up))
        );
        assert_eq!(
            keyboard_play_option("F", true, false, &keys, &play_input, &input),
            Some(PlayOptionControl::Hispeed(HispeedChange::Up))
        );
        assert_eq!(
            keyboard_play_option("LShift", true, false, &keys, &play_input, &input),
            Some(PlayOptionControl::LaneCover(LaneCoverChange::Up))
        );
        assert_eq!(
            keyboard_play_option("LControl", true, false, &keys, &play_input, &input),
            Some(PlayOptionControl::LaneCover(LaneCoverChange::Down))
        );
    }

    #[test]
    fn play_option_control_maps_scratch_for_scratchless_key_modes() {
        let input = crate::config::play_input::default_profile_input();
        let keys = SelectKeyBindings::from_profile(&input);

        for key_mode in [KeyMode::K4, KeyMode::K6, KeyMode::K8, KeyMode::K9] {
            let play_input = play_option_input_for(&input, key_mode);
            assert_eq!(
                keyboard_play_option("LShift", true, false, &keys, &play_input, &input),
                Some(PlayOptionControl::LaneCover(LaneCoverChange::Up)),
                "{} Scratch Up",
                key_mode.as_str(),
            );
            assert_eq!(
                keyboard_play_option("LControl", true, false, &keys, &play_input, &input),
                Some(PlayOptionControl::LaneCover(LaneCoverChange::Down)),
                "{} Scratch Down",
                key_mode.as_str(),
            );
            assert_eq!(
                keyboard_play_option("LShift", false, true, &keys, &play_input, &input),
                Some(PlayOptionControl::GreenNumber(GreenNumberChange::Up)),
                "{} Scratch Up green number",
                key_mode.as_str(),
            );
            assert_eq!(
                keyboard_play_option("LControl", false, true, &keys, &play_input, &input),
                Some(PlayOptionControl::GreenNumber(GreenNumberChange::Down)),
                "{} Scratch Down green number",
                key_mode.as_str(),
            );
        }
    }

    #[test]
    fn play_option_control_maps_e2_to_mode_specific_green_number_direction() {
        let input = crate::config::play_input::default_profile_input();
        let keys = SelectKeyBindings::from_profile(&input);
        let play_input = play_option_input_for(&input, KeyMode::K7);

        assert_eq!(
            keyboard_play_option("Z", false, true, &keys, &play_input, &input),
            Some(PlayOptionControl::GreenNumber(GreenNumberChange::Down))
        );
        assert_eq!(
            keyboard_play_option("S", false, true, &keys, &play_input, &input),
            Some(PlayOptionControl::GreenNumber(GreenNumberChange::Up))
        );
        assert_eq!(
            keyboard_play_option("LShift", false, true, &keys, &play_input, &input),
            Some(PlayOptionControl::GreenNumber(GreenNumberChange::Up))
        );
        assert_eq!(
            keyboard_play_option("LControl", false, true, &keys, &play_input, &input),
            Some(PlayOptionControl::GreenNumber(GreenNumberChange::Down))
        );
        assert_eq!(keyboard_play_option("Z", true, true, &keys, &play_input, &input), None);
    }

    #[test]
    fn play_option_control_uses_chart_mode_instead_of_select_input_mode() {
        let input = crate::config::play_input::default_profile_input();
        assert_eq!(input.select_input_mode, SelectInputModeConfig::Key7Key14);
        let keys = SelectKeyBindings::from_profile(&input);
        let play_input = play_option_input_for(&input, KeyMode::K9);

        assert_eq!(
            keyboard_play_option("B", true, false, &keys, &play_input, &input),
            Some(PlayOptionControl::Hispeed(HispeedChange::Down))
        );
        assert_eq!(
            keyboard_play_option("G", true, false, &keys, &play_input, &input),
            Some(PlayOptionControl::Hispeed(HispeedChange::Up))
        );
    }

    #[test]
    fn play_option_control_applies_eight_key_default_and_override() {
        let mut input = crate::config::play_input::default_profile_input();
        let keys = SelectKeyBindings::from_profile(&input);
        let play_input = play_option_input_for(&input, KeyMode::K8);

        assert_eq!(
            keyboard_play_option("Z", true, false, &keys, &play_input, &input),
            Some(PlayOptionControl::Hispeed(HispeedChange::Up))
        );
        assert!(crate::config::play_input::set_eight_key_hispeed_direction(
            &mut input,
            LaneConfig::Key1,
            HispeedDirectionConfig::Down,
        ));
        assert_eq!(
            keyboard_play_option("Z", true, false, &keys, &play_input, &input),
            Some(PlayOptionControl::Hispeed(HispeedChange::Down))
        );
    }

    #[test]
    fn play_option_control_distinguishes_two_player_gamepads() {
        let mut input = crate::config::play_input::default_profile_input();
        input.play.insert(
            KeyMode::K14.play_map_key().to_string(),
            crate::config::profile_config::PlayModeInputConfig {
                inherit: None,
                bindings: vec![
                    crate::config::play_input::gamepad_play_binding_for_device(
                        "gamepad1",
                        "Button1",
                        LaneConfig::Key1,
                    ),
                    crate::config::play_input::gamepad_play_binding_for_device(
                        "gamepad2",
                        "Button1",
                        LaneConfig::Key9,
                    ),
                ],
                ..Default::default()
            },
        );
        let slots = crate::input::gamepad::GamepadSlotMap::from_device_ids([
            Some(DeviceId(11)),
            Some(DeviceId(22)),
        ]);
        let play_input = PlayOptionInput::new(
            KeyMode::K14,
            crate::config::play::lane_binding_for_chart_with_slots(&input, KeyMode::K14, slots),
            &input,
            slots,
        );
        let control = PhysicalControl::GamepadButton("Button1".to_string());

        assert_eq!(
            play_option_control_for_input(
                DeviceId(11),
                &control,
                true,
                false,
                Some(&play_input),
                &input,
            ),
            Some(PlayOptionControl::Hispeed(HispeedChange::Down))
        );
        assert_eq!(
            play_option_control_for_input(
                DeviceId(22),
                &control,
                true,
                false,
                Some(&play_input),
                &input,
            ),
            Some(PlayOptionControl::Hispeed(HispeedChange::Up))
        );
    }

    #[test]
    fn bounce_bypass_requires_synthesized_axis_bound_to_profile_scratch_lane() {
        let mut input = crate::config::play_input::default_profile_input();
        input.play.insert(
            KeyMode::K14.play_map_key().to_string(),
            crate::config::profile_config::PlayModeInputConfig {
                inherit: None,
                bindings: vec![
                    crate::config::play_input::gamepad_play_binding_for_device(
                        "gamepad1",
                        "Axis1+",
                        LaneConfig::Scratch,
                    ),
                    crate::config::play_input::gamepad_play_binding_for_device(
                        "gamepad1",
                        "Axis2+",
                        LaneConfig::Key1,
                    ),
                    crate::config::play_input::gamepad_play_binding_for_device(
                        "gamepad1",
                        "Axis3+",
                        LaneConfig::Scratch2,
                    ),
                ],
                ..Default::default()
            },
        );
        let slots =
            crate::input::gamepad::GamepadSlotMap::from_device_ids([Some(DeviceId(11)), None]);
        let binding =
            crate::config::play::lane_binding_for_chart_with_slots(&input, KeyMode::K14, slots);
        let event = |name: &str, device_id, synthesized_analog_axis| {
            crate::input::gamepad::GamepadButtonEvent {
                name: name.to_string(),
                device_id,
                pressed: true,
                timestamp: bmz_gameplay::input::backend::DeviceTimestamp::MonotonicNs(1),
                synthesized_analog_axis,
            }
        };

        assert!(should_bypass_analog_scratch_bounce(
            &event("Axis1+", DeviceId(11), true),
            Some(&binding),
        ));
        assert!(!should_bypass_analog_scratch_bounce(
            &event("Axis2+", DeviceId(11), true),
            Some(&binding),
        ));
        assert!(should_bypass_analog_scratch_bounce(
            &event("Axis3+", DeviceId(11), true),
            Some(&binding),
        ));
        assert!(!should_bypass_analog_scratch_bounce(
            &event("Axis1+", DeviceId(11), false),
            Some(&binding),
        ));
        assert!(!should_bypass_analog_scratch_bounce(
            &event("Axis1+", DeviceId(22), true),
            Some(&binding),
        ));
        assert!(!should_bypass_analog_scratch_bounce(&event("Axis1+", DeviceId(11), true), None,));
    }

    #[test]
    fn play_option_control_prioritizes_two_player_lane_over_other_devices_e2_button() {
        let mut input = crate::config::play_input::default_profile_input();
        input.ui.bindings.retain(|entry| {
            entry.action != Some(InputActionConfig::E2)
                || !crate::config::play_input::is_gamepad_device(&entry.device)
        });
        input.ui.bindings.push(crate::config::profile_config::BindingConfigEntry {
            device: "gamepad1".to_string(),
            control: "Button10".to_string(),
            keyboard_slot: None,
            lane: None,
            action: Some(InputActionConfig::E2),
            scratch: None,
        });
        input.play.insert(
            KeyMode::K14.play_map_key().to_string(),
            crate::config::profile_config::PlayModeInputConfig {
                inherit: None,
                bindings: vec![
                    crate::config::play_input::gamepad_play_binding_for_device(
                        "gamepad1",
                        "Button1",
                        LaneConfig::Key1,
                    ),
                    crate::config::play_input::gamepad_play_binding_for_device(
                        "gamepad2",
                        "Button10",
                        LaneConfig::Key9,
                    ),
                ],
                ..Default::default()
            },
        );
        let slots = crate::input::gamepad::GamepadSlotMap::from_device_ids([
            Some(DeviceId(11)),
            Some(DeviceId(22)),
        ]);
        let play_input = PlayOptionInput::new(
            KeyMode::K14,
            crate::config::play::lane_binding_for_chart_with_slots(&input, KeyMode::K14, slots),
            &input,
            slots,
        );
        let control = PhysicalControl::GamepadButton("Button10".to_string());

        assert_eq!(
            play_option_control_for_input(
                DeviceId(11),
                &control,
                true,
                true,
                Some(&play_input),
                &input,
            ),
            Some(PlayOptionControl::ToggleHispeedMode)
        );
        assert_eq!(
            play_option_control_for_input(
                DeviceId(22),
                &control,
                true,
                false,
                Some(&play_input),
                &input,
            ),
            Some(PlayOptionControl::Hispeed(HispeedChange::Up))
        );
        assert_eq!(
            play_option_control_for_input(
                DeviceId(22),
                &control,
                false,
                true,
                Some(&play_input),
                &input,
            ),
            Some(PlayOptionControl::GreenNumber(GreenNumberChange::Up))
        );

        let p2_lane_pressed = HashSet::from([(DeviceId(22), control.clone())]);
        assert_eq!(
            play_control_hold_state_from_pressed_inputs(&p2_lane_pressed, &play_input),
            (false, false, false)
        );
        let p1_e2_pressed = HashSet::from([(DeviceId(11), control)]);
        assert_eq!(
            play_control_hold_state_from_pressed_inputs(&p1_e2_pressed, &play_input),
            (false, true, false)
        );
    }

    #[test]
    fn detail_option_control_maps_key5_and_key7_to_visual_offset() {
        let keys = select_keys_with_full_2p_bindings();

        assert_eq!(visual_offset_delta_control("C", &keys), Some(-1));
        assert_eq!(visual_offset_delta_control("V", &keys), Some(1));
        assert_eq!(visual_offset_delta_control("Period", &keys), Some(-1));
        assert_eq!(visual_offset_delta_control("P2K7", &keys), Some(1));
        assert_eq!(visual_offset_delta_control("Z", &keys), None);
        assert_eq!(green_number_delta_control("D", &keys), Some(-1));
        assert_eq!(green_number_delta_control("F", &keys), Some(1));
        assert_eq!(green_number_delta_control("C", &keys), None);
    }

    #[test]
    fn floating_hispeed_formula_uses_green_number_and_lane_cover() {
        assert_eq!(hispeed_for_green_number_values(300.0, 1.0, 120.0, 1.0), 4.0);
        assert_eq!(hispeed_for_green_number_values(300.0, 0.5, 120.0, 1.0), 2.0);
        assert_eq!(hispeed_for_green_number_values(300.0, 1.0, 240.0, 1.0), 2.0);
        assert_eq!(hispeed_for_green_number_values(300.0, 1.0, 120.0, 2.0), 2.0);
        assert!(
            (hispeed_for_green_number_values(295.0, 0.93, 120.0, 1.0) - 3.783_051).abs() < 0.000_01
        );
    }

    #[test]
    fn green_number_change_uses_the_displayed_integer_duration() {
        assert_eq!(green_number_from_display_duration(500.0), 300);
        assert_eq!(green_number_from_display_duration(500.6), 301);
    }

    #[test]
    fn select_skin_green_number_uses_profile_target_green_for_nhs() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed = 2.0;
        profile.lane.hispeed_mode = HispeedModeConfig::Normal;
        profile.lane.target_green_number = 300;

        assert_eq!(WinitApp::select_note_display_duration_ms_for_skin(&profile), 300);
    }

    #[test]
    fn select_skin_green_number_uses_target_green_for_fhs() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.hispeed_mode = HispeedModeConfig::Floating;
        profile.lane.target_green_number = 280;

        assert_eq!(WinitApp::select_note_display_duration_ms_for_skin(&profile), 280);
    }

    #[test]
    fn active_lane_state_saves_current_green_number_for_nhs() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);

        apply_current_play_options_to_profile(
            &mut profile,
            Some(2.0),
            Some(ActiveLaneState {
                lane_cover: 0.0,
                lift: 0.0,
                hispeed_mode: HispeedMode::Normal,
                target_green_number: 600,
            }),
            CurrentPlayOptions {
                arrange: ArrangeOption::Normal,
                arrange_2p: ArrangeOption::Normal,
                target: TargetOption::None,
                gauge: GaugeTypeConfig::Normal,
                gauge_auto_shift: GaugeAutoShiftConfig::Off,
                bottom_shiftable_gauge: BottomShiftableGaugeConfig::Easy,
                double_option: DoubleOption::Off,
                hs_fix: HsFixOption::Off,
                session_mode: SessionMode::Normal,
            },
            42,
        );

        assert_eq!(profile.lane.hispeed_mode, HispeedModeConfig::Normal);
        assert_eq!(profile.lane.target_green_number, 600);
    }

    #[test]
    fn normal_hispeed_rounding_restores_quarter_steps() {
        assert_eq!(clamp_hispeed_for_profile(3.783_051, HispeedModeConfig::Normal, 0.25), 3.75);
    }

    #[test]
    fn custom_hispeed_step_preserves_non_quarter_profile_values() {
        assert_eq!(clamp_hispeed_for_profile(2.3, HispeedModeConfig::Normal, 0.3), 2.3);
        assert_eq!(clamp_hispeed_for_profile(2.37, HispeedModeConfig::Floating, 0.5), 2.37);
    }

    #[test]
    fn gauge_option_cycle_includes_auto_shift() {
        assert_eq!(cycle_gauge_option(GaugeTypeConfig::ExHard), GaugeTypeConfig::Hazard);
        assert_eq!(
            cycle_gauge_auto_shift_option(GaugeAutoShiftConfig::Off),
            GaugeAutoShiftConfig::Continue
        );
        assert_eq!(gauge_auto_shift_as_str(GaugeAutoShiftConfig::BestClear), "BEST CLEAR");
        assert_eq!(
            cycle_bottom_shiftable_gauge_with_direction(BottomShiftableGaugeConfig::Normal, 1),
            BottomShiftableGaugeConfig::AssistEasy
        );
        assert_eq!(bottom_shiftable_gauge_as_str(BottomShiftableGaugeConfig::Easy), "EASY");
        assert_eq!(cycle_gauge_option(GaugeTypeConfig::AutoShift), GaugeTypeConfig::Hazard);
    }

    #[test]
    fn apply_current_play_options_updates_profile_defaults() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);

        apply_current_play_options_to_profile(
            &mut profile,
            Some(3.37),
            Some(ActiveLaneState {
                lane_cover: 0.42,
                lift: 0.1,
                hispeed_mode: HispeedMode::Floating,
                target_green_number: 280,
            }),
            CurrentPlayOptions {
                arrange: ArrangeOption::Mirror,
                arrange_2p: ArrangeOption::Random,
                target: TargetOption::RankAaa,
                gauge: GaugeTypeConfig::Hard,
                gauge_auto_shift: GaugeAutoShiftConfig::BestClear,
                bottom_shiftable_gauge: BottomShiftableGaugeConfig::Normal,
                double_option: DoubleOption::Flip,
                hs_fix: HsFixOption::MainBpm,
                session_mode: SessionMode::Autoplay,
            },
            42,
        );

        assert_eq!(profile.lane.hispeed, 3.37);
        assert_eq!(profile.lane.sudden, 420);
        assert_eq!(profile.lane.lift, 100);
        assert_eq!(profile.lane.hispeed_mode, HispeedModeConfig::Floating);
        assert_eq!(profile.lane.target_green_number, 280);
        assert!(matches!(profile.play.random, RandomOptionConfig::Mirror));
        assert!(matches!(profile.play.random2, RandomOptionConfig::Random));
        assert!(matches!(profile.play.target, TargetOptionConfig::RankAaa));
        assert!(matches!(profile.play.gauge, GaugeTypeConfig::Hard));
        assert!(matches!(profile.play.gauge_auto_shift, GaugeAutoShiftConfig::BestClear));
        assert!(matches!(profile.play.bottom_shiftable_gauge, BottomShiftableGaugeConfig::Normal));
        assert!(matches!(profile.play.double_option, DoubleOptionConfig::Flip));
        assert!(matches!(profile.play.hs_fix, HsFixConfig::MainBpm));
        assert!(profile.play.auto_play);
        assert!(matches!(profile.play.assist, AssistOptionConfig::None));
        assert_eq!(profile.updated_at, 42);
    }

    #[test]
    fn profile_play_option_changes_disable_random_and_autoplay_without_rollback() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.play.random = RandomOptionConfig::Random;
        profile.play.random2 = RandomOptionConfig::Mirror;
        profile.play.session_mode = None;
        profile.play.auto_play = true;
        let before = profile.play.clone();
        let current = select_play_options_from_profile(&before);

        profile.play.random = RandomOptionConfig::Off;
        profile.play.random2 = RandomOptionConfig::Off;
        profile.play.auto_play = false;
        let synced =
            merge_changed_select_play_options_from_profile(current, &before, &profile.play);

        assert_eq!(synced.arrange, ArrangeOption::Normal);
        assert_eq!(synced.arrange_2p, ArrangeOption::Normal);
        assert_eq!(synced.session_mode, SessionMode::Normal);

        apply_current_play_options_to_profile(&mut profile, None, None, synced, 42);
        assert_eq!(profile.play.random, RandomOptionConfig::Off);
        assert_eq!(profile.play.random2, RandomOptionConfig::Off);
        assert!(!profile.play.auto_play);
    }

    #[test]
    fn session_mode_profile_migrates_legacy_autoplay_and_persists_battle() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.play.session_mode = None;
        profile.play.auto_play = true;
        assert_eq!(session_mode_from_profile(&profile.play), SessionMode::Autoplay);

        let mut options = select_play_options_from_profile(&profile.play);
        options.session_mode = SessionMode::GhostBattle;
        apply_current_play_options_to_profile(&mut profile, None, None, options, 2);

        assert_eq!(profile.play.session_mode, Some(SessionMode::GhostBattle));
        assert!(!profile.play.auto_play);
        let serialized = toml::to_string(&profile).unwrap();
        assert!(serialized.contains(r#"session_mode = "GhostBattle""#));
    }

    #[test]
    fn course_normalizes_battle_session_modes() {
        let mut autoplay_battle = PlayStartOptions {
            session_mode: SessionMode::AutoplayBattle,
            autoplay: true,
            replay_player: Some(bmz_gameplay::replay::ReplayPlayer::default()),
            ..PlayStartOptions::default()
        };
        normalize_session_mode_for_course(&mut autoplay_battle);
        assert_eq!(autoplay_battle.session_mode, SessionMode::Autoplay);
        assert!(autoplay_battle.autoplay);
        assert!(autoplay_battle.replay_player.is_none());

        let mut ghost_battle = PlayStartOptions {
            session_mode: SessionMode::GhostBattle,
            replay_player: Some(bmz_gameplay::replay::ReplayPlayer::default()),
            ..PlayStartOptions::default()
        };
        normalize_session_mode_for_course(&mut ghost_battle);
        assert_eq!(ghost_battle.session_mode, SessionMode::Normal);
        assert!(!ghost_battle.autoplay);
        assert!(ghost_battle.replay_player.is_none());
    }

    #[test]
    fn profile_random_change_preserves_cli_autoplay_runtime_option() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let before = profile.play.clone();
        let mut current = select_play_options_from_profile(&before);
        current.session_mode = SessionMode::Autoplay;

        let mut after = before.clone();
        after.random = RandomOptionConfig::Mirror;
        let synced = merge_changed_select_play_options_from_profile(current, &before, &after);

        assert_eq!(synced.arrange, ArrangeOption::Mirror);
        assert_eq!(synced.session_mode, SessionMode::Autoplay);
    }

    #[test]
    fn profile_play_option_changes_sync_all_select_runtime_options() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let before = profile.play.clone();
        let current = select_play_options_from_profile(&before);
        let mut after = before.clone();
        after.gauge = GaugeTypeConfig::AutoShift;
        after.gauge_auto_shift = GaugeAutoShiftConfig::Continue;
        after.bottom_shiftable_gauge = BottomShiftableGaugeConfig::Normal;
        after.random = RandomOptionConfig::SRandom;
        after.random2 = RandomOptionConfig::RRandom;
        after.double_option = DoubleOptionConfig::Flip;
        after.hs_fix = HsFixConfig::MainBpm;
        after.target = TargetOptionConfig::RankAaa;
        after.auto_play = true;

        let synced = merge_changed_select_play_options_from_profile(current, &before, &after);

        assert_eq!(synced.gauge, GaugeTypeConfig::ExHard);
        assert_eq!(synced.gauge_auto_shift, GaugeAutoShiftConfig::BestClear);
        assert_eq!(synced.bottom_shiftable_gauge, BottomShiftableGaugeConfig::Normal);
        assert_eq!(synced.arrange, ArrangeOption::SRandom);
        assert_eq!(synced.arrange_2p, ArrangeOption::RRandom);
        assert_eq!(synced.double_option, DoubleOption::Flip);
        assert_eq!(synced.hs_fix, HsFixOption::MainBpm);
        assert_eq!(synced.target, TargetOption::RankAaa);
        assert_eq!(synced.session_mode, SessionMode::Autoplay);
    }

    #[test]
    fn select_score_context_changes_only_for_rule_or_ln_mode() {
        let profile = ProfileConfig::new_default("default", "Default", 1);
        let before = SelectScoreContext::from_profile(&profile);

        let mut random_changed = profile.clone();
        random_changed.play.random = RandomOptionConfig::Mirror;
        assert_eq!(before, SelectScoreContext::from_profile(&random_changed));

        let mut rule_changed = profile.clone();
        rule_changed.play.rule_mode = RuleMode::Dx;
        assert_ne!(before, SelectScoreContext::from_profile(&rule_changed));

        let mut ln_changed = profile;
        ln_changed.play.ln_mode_policy = LnPolicySetting::ForceCn;
        assert_ne!(before, SelectScoreContext::from_profile(&ln_changed));
    }

    #[test]
    fn loaded_skin_reset_preserves_non_skin_profile_settings() {
        let mut current = ProfileConfig::new_default("default", "Current", 1);
        current.play.random = RandomOptionConfig::SRandom;
        current.input.analog_scratch_sensitivity = 2.5;
        current.ui.show_fps = true;
        current.skin.select = "current/select.json".to_string();

        let mut loaded = ProfileConfig::new_default("default", "Disk", 2);
        loaded.play.random = RandomOptionConfig::Mirror;
        loaded.input.analog_scratch_sensitivity = 0.5;
        loaded.ui.show_fps = false;
        loaded.skin.select = "disk/select.json".to_string();

        replace_skin_config_from_loaded_profile(&mut current, loaded);

        assert_eq!(current.display_name, "Current");
        assert_eq!(current.updated_at, 1);
        assert_eq!(current.play.random, RandomOptionConfig::SRandom);
        assert_eq!(current.input.analog_scratch_sensitivity, 2.5);
        assert!(current.ui.show_fps);
        assert_eq!(current.skin.select, "disk/select.json");
    }

    #[test]
    fn apply_lane_state_preserves_lift_amount_while_lift_is_disabled() {
        let mut profile = ProfileConfig::new_default("default", "Default", 1);
        profile.lane.lift = 240;
        profile.lane.lift_enabled = false;

        apply_lane_state_to_profile(
            &mut profile,
            None,
            Some(ActiveLaneState {
                lane_cover: 0.3,
                lift: 0.0,
                hispeed_mode: HispeedMode::Normal,
                target_green_number: 300,
            }),
        );

        assert_eq!(profile.lane.lift, 240);
        assert!(!profile.lane.lift_enabled);
    }

    #[test]
    fn arrange_option_maps_profile_random_defaults() {
        assert_eq!(arrange_option_from_profile(RandomOptionConfig::Off), ArrangeOption::Normal);
        assert_eq!(arrange_option_from_profile(RandomOptionConfig::Mirror), ArrangeOption::Mirror);
        assert_eq!(arrange_option_from_profile(RandomOptionConfig::Random), ArrangeOption::Random);
        assert_eq!(
            arrange_option_from_profile(RandomOptionConfig::RRandom),
            ArrangeOption::RRandom
        );
        assert_eq!(
            arrange_option_from_profile(RandomOptionConfig::SRandom),
            ArrangeOption::SRandom
        );
        assert_eq!(arrange_option_from_profile(RandomOptionConfig::Spiral), ArrangeOption::Spiral);
        assert_eq!(
            arrange_option_from_profile(RandomOptionConfig::HRandom),
            ArrangeOption::HRandom
        );
        assert_eq!(
            arrange_option_from_profile(RandomOptionConfig::AllScratch),
            ArrangeOption::AllScratch
        );
        assert_eq!(
            arrange_option_from_profile(RandomOptionConfig::RandomEx),
            ArrangeOption::RandomEx
        );
        assert_eq!(
            arrange_option_from_profile(RandomOptionConfig::SRandomEx),
            ArrangeOption::SRandomEx
        );
        assert_eq!(
            arrange_option_from_profile(RandomOptionConfig::FRandom),
            ArrangeOption::FRandom
        );
        assert_eq!(
            arrange_option_from_profile(RandomOptionConfig::MFRandom),
            ArrangeOption::MFRandom
        );
        assert!(matches!(
            random_config_from_arrange(ArrangeOption::Normal),
            RandomOptionConfig::Off
        ));
        assert!(matches!(
            random_config_from_arrange(ArrangeOption::Mirror),
            RandomOptionConfig::Mirror
        ));
        assert!(matches!(
            random_config_from_arrange(ArrangeOption::Random),
            RandomOptionConfig::Random
        ));
        assert!(matches!(
            random_config_from_arrange(ArrangeOption::RRandom),
            RandomOptionConfig::RRandom
        ));
        assert!(matches!(
            random_config_from_arrange(ArrangeOption::SRandom),
            RandomOptionConfig::SRandom
        ));
        assert!(matches!(
            random_config_from_arrange(ArrangeOption::Spiral),
            RandomOptionConfig::Spiral
        ));
        assert!(matches!(
            random_config_from_arrange(ArrangeOption::HRandom),
            RandomOptionConfig::HRandom
        ));
        assert!(matches!(
            random_config_from_arrange(ArrangeOption::AllScratch),
            RandomOptionConfig::AllScratch
        ));
        assert!(matches!(
            random_config_from_arrange(ArrangeOption::RandomEx),
            RandomOptionConfig::RandomEx
        ));
        assert!(matches!(
            random_config_from_arrange(ArrangeOption::SRandomEx),
            RandomOptionConfig::SRandomEx
        ));
        assert!(matches!(
            random_config_from_arrange(ArrangeOption::FRandom),
            RandomOptionConfig::FRandom
        ));
        assert!(matches!(
            random_config_from_arrange(ArrangeOption::MFRandom),
            RandomOptionConfig::MFRandom
        ));
    }

    #[test]
    fn window_title_uses_scene_name() {
        assert_eq!(window_title_for_scene(AppSceneKind::Select), "bmz-player - Select");
        assert_eq!(window_title_for_scene(AppSceneKind::Play), "bmz-player - Play");
        assert_eq!(window_title_for_scene(AppSceneKind::Result), "bmz-player - Result");
    }

    #[test]
    fn deferred_boot_action_keeps_practice_boot_after_window_init() {
        let mut options = AppOptions {
            boot_practice: true,
            practice_start_ms: Some(5_000),
            practice_end_ms: Some(120_000),
            ..AppOptions::default()
        };

        assert_eq!(
            deferred_boot_action(Some(42), &options),
            Some(DeferredBoot::Practice {
                chart_id: 42,
                start_time_ms: Some(5_000),
                end_time_ms: Some(120_000),
            })
        );

        options.boot_practice = false;
        assert_eq!(
            deferred_boot_action(Some(42), &options),
            Some(DeferredBoot::Chart { chart_id: 42, replay_slot: None })
        );
    }

    #[test]
    fn select_bgm_is_skipped_when_preview_is_already_playing() {
        assert!(should_play_select_bgm_on_enter(false));
        assert!(!should_play_select_bgm_on_enter(true));
    }

    #[test]
    fn play_scene_keeps_decide_bgm_until_chart_start() {
        use crate::system_sound::SoundType;

        let sounds = system_bgm_stop_targets_on_scene_enter(AppSceneKind::Play);

        assert!(sounds.contains(&SoundType::Select));
        assert!(!sounds.contains(&SoundType::Decide));
    }

    #[test]
    fn non_play_scene_stops_all_transition_bgms() {
        use crate::system_sound::SoundType;

        for scene in [AppSceneKind::Select, AppSceneKind::Decide, AppSceneKind::Result] {
            let sounds = system_bgm_stop_targets_on_scene_enter(scene);
            assert!(sounds.contains(&SoundType::Select), "scene={scene:?}");
            assert!(sounds.contains(&SoundType::Decide), "scene={scene:?}");
        }
    }

    #[test]
    fn select_preview_fade_factor_ramps_in_and_out() {
        let started_at = Instant::now();
        let half = started_at + SELECT_PREVIEW_FADE_DURATION / 2;
        let done = started_at + SELECT_PREVIEW_FADE_DURATION;

        assert_eq!(
            select_preview_fade_factor(SelectPreviewFade::FadingIn { started_at }, started_at),
            0.0
        );
        assert!(
            (select_preview_fade_factor(SelectPreviewFade::FadingIn { started_at }, half) - 0.5)
                .abs()
                < 0.001
        );
        assert_eq!(
            select_preview_fade_factor(SelectPreviewFade::FadingIn { started_at }, done),
            1.0
        );
        assert!(
            (select_preview_fade_factor(SelectPreviewFade::FadingOut { started_at }, half) - 0.5)
                .abs()
                < 0.001
        );
        assert_eq!(
            select_preview_fade_factor(SelectPreviewFade::FadingOut { started_at }, done),
            0.0
        );
    }

    #[test]
    fn select_preview_normalization_gain_follows_chart_normalization_setting() {
        assert_eq!(select_preview_normalization_gain(true, 0.25), 0.25);
        assert_eq!(select_preview_normalization_gain(false, 0.25), 1.0);
        assert_eq!(select_preview_normalization_gain(true, f32::NAN), 1.0);
        assert_eq!(select_preview_normalization_gain(true, 1.5), 1.0);
    }

    #[test]
    fn prepare_select_preview_keeps_sample_with_analyzed_gain() {
        let sample = DecodedSample { channels: 2, sample_rate: 48_000, frames: vec![1.0; 480] };

        let prepared = prepare_select_preview(sample.clone());

        assert_eq!(prepared.sample.frames, sample.frames);
        assert!(prepared.normalization_gain > 0.0);
        assert!(prepared.normalization_gain < 1.0);
    }

    #[test]
    fn result_exit_audio_gain_uses_shorter_skin_fadeout() {
        let fadeout = Duration::from_millis(600);

        assert_eq!(result_exit_audio_gain(Duration::ZERO, fadeout), 1.0);
        assert!((result_exit_audio_gain(Duration::from_millis(300), fadeout) - 0.5).abs() < 0.001);
        assert_eq!(result_exit_audio_gain(fadeout, fadeout), 0.0);
    }

    #[test]
    fn result_exit_audio_gain_caps_long_skin_fadeout() {
        let fadeout = Duration::from_millis(3_000);

        assert!((result_exit_audio_gain(Duration::from_millis(750), fadeout) - 0.5).abs() < 0.001);
        assert_eq!(result_exit_audio_gain(RESULT_EXIT_AUDIO_FADE, fadeout), 0.0);
    }

    #[test]
    fn result_exit_audio_gain_is_zero_for_zero_fadeout() {
        assert_eq!(result_exit_audio_gain(Duration::ZERO, Duration::ZERO), 0.0);
    }

    #[test]
    fn result_exit_cleanup_only_targets_result_sounds() {
        use crate::system_sound::SoundType;

        let sounds = result_exit_system_sounds();

        assert!(sounds.contains(&SoundType::ResultClear));
        assert!(sounds.contains(&SoundType::ResultFail));
        assert!(sounds.contains(&SoundType::ResultClose));
        assert!(sounds.contains(&SoundType::CourseClear));
        assert!(sounds.contains(&SoundType::CourseFail));
        assert!(sounds.contains(&SoundType::CourseClose));
        assert!(!sounds.contains(&SoundType::Select));
        assert!(!sounds.contains(&SoundType::Decide));
        assert!(!sounds.contains(&SoundType::OptionChange));
        assert!(!sounds.contains(&SoundType::Landmine));
    }

    #[test]
    fn result_entry_sound_uses_fail_for_failed_play() {
        use crate::system_sound::SoundType;

        assert_eq!(result_entry_sound_for_clear(ClearType::Failed), SoundType::ResultFail);
        assert_eq!(result_entry_sound_for_clear(ClearType::Normal), SoundType::ResultClear);
        assert_eq!(course_result_entry_sound_for_clear(ClearType::Failed), SoundType::CourseFail);
        assert_eq!(course_result_entry_sound_for_clear(ClearType::Normal), SoundType::CourseClear);
    }

    #[test]
    fn result_exit_sound_prefers_course_close_for_course_results() {
        use crate::system_sound::SoundType;

        assert_eq!(result_exit_sound_for_context(false, false), SoundType::ResultClose);
        assert_eq!(result_exit_sound_for_context(true, true), SoundType::CourseClose);
        assert_eq!(result_exit_sound_for_context(true, false), SoundType::ResultClose);
    }

    #[test]
    fn result_entry_sound_clear_type_uses_raw_result_for_course_stage() {
        let mut finished = debug_boot_finished_play_session();
        finished.summary.clear_type = ClearType::NoPlay;

        finished.result.clear_type = ClearType::Normal;
        assert_eq!(result_entry_clear_type_for_sound(&finished), ClearType::Normal);

        finished.result.clear_type = ClearType::Failed;
        assert_eq!(result_entry_clear_type_for_sound(&finished), ClearType::Failed);
    }

    #[test]
    fn select_preview_key_waits_for_beatoraja_start_delay() {
        let key = Some("folder|preview.ogg".to_string());

        assert_eq!(
            select_preview_key_after_delay(
                key.clone(),
                SELECT_PREVIEW_START_DELAY - Duration::from_millis(1),
                SELECT_PREVIEW_START_DELAY,
            ),
            None
        );
        assert_eq!(
            select_preview_key_after_delay(
                key.clone(),
                SELECT_PREVIEW_START_DELAY,
                SELECT_PREVIEW_START_DELAY,
            ),
            key
        );
    }

    #[test]
    fn select_preview_load_queue_keeps_only_latest_pending_request() {
        let mut queue = SelectPreviewLoadQueue::default();

        assert_eq!(queue.request("first".to_string()), Some("first".to_string()));
        assert_eq!(queue.request("second".to_string()), None);
        assert_eq!(queue.request("latest".to_string()), None);
        assert_eq!(queue.finish(), Some("latest".to_string()));
        assert_eq!(queue.finish(), None);
        assert_eq!(queue.request("after-idle".to_string()), Some("after-idle".to_string()));
    }

    #[test]
    fn select_preview_uses_generated_fallback_after_explicit_preview_fails() {
        assert!(should_use_generated_preview("", false));
        assert!(should_use_generated_preview("missing-preview.ogg", true));
        assert!(!should_use_generated_preview("preview.ogg", false));
    }

    #[test]
    fn audio_diagnostic_marks_generated_preview_callback_pressure() {
        assert_eq!(
            classify_audio_output_issue(0, 0, 0, 0, 0, 0, true, 0, true),
            AudioOutputIssueCause::GeneratedPreviewCpuPressure
        );
        assert_eq!(
            classify_audio_output_issue(0, 0, 1, 0, 0, 0, true, 0, true),
            AudioOutputIssueCause::CallbackLockContention
        );
        assert_eq!(
            classify_audio_output_issue(0, 0, 0, 0, 0, 0, false, 1, true),
            AudioOutputIssueCause::MixClipping
        );
        assert_eq!(
            classify_audio_output_issue(0, 0, 0, 0, 1, 0, false, 0, false),
            AudioOutputIssueCause::Unknown
        );
    }

    #[test]
    fn window_attributes_use_configured_video_size() {
        let mut config = crate::config::app_config::AppConfig::default().video;
        config.width = 1920;
        config.height = 1080;

        let attributes = window_attributes_from_config(&config);

        assert_eq!(attributes.inner_size, Some(PhysicalSize::new(1920, 1080).into()));
        assert!(attributes.window_icon.is_some());
    }

    #[test]
    fn left_overlay_hides_toast_while_screenshot_pending() {
        let toast = Some(("スクリーンショットを保存しました", Duration::from_millis(100)));
        assert_eq!(resolve_left_overlay_text(true, toast, "SCAN 1 / 2"), "SCAN 1 / 2");
        assert_eq!(
            resolve_left_overlay_text(false, toast, "SCAN 1 / 2"),
            "スクリーンショットを保存しました"
        );
    }

    #[test]
    fn song_scan_progress_atomic_value_roundtrips() {
        let progress = ScanProgress { done: 123, total: 456 };

        assert_eq!(unpack_scan_progress(pack_scan_progress(progress)), progress);
    }

    #[test]
    fn left_overlay_expires_toast() {
        let toast = Some(("スクリーンショットを保存しました", LEFT_OVERLAY_TOAST_DURATION));
        assert_eq!(resolve_left_overlay_text(false, toast, ""), "");
    }

    #[test]
    fn screenshot_dir_defaults_when_empty() {
        let data_dir = Path::new("user-data");

        assert_eq!(screenshot_dir("", data_dir), PathBuf::from("user-data/screenshots"));
        assert_eq!(screenshot_dir("   ", data_dir), PathBuf::from("user-data/screenshots"));
    }

    #[test]
    fn screenshot_dir_uses_configured_path() {
        let data_dir = Path::new("user-data");

        assert_eq!(screenshot_dir("captures", data_dir), PathBuf::from("user-data/captures"));
    }

    #[test]
    fn screenshot_dir_maps_legacy_data_default_to_data_dir() {
        let data_dir = Path::new("user-data");

        assert_eq!(
            screenshot_dir("data/screenshots", data_dir),
            PathBuf::from("user-data/screenshots")
        );
    }

    #[test]
    fn screenshot_dir_keeps_absolute_configured_path() {
        let data_dir = Path::new("user-data");
        let absolute_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("captures");

        assert_eq!(screenshot_dir(&absolute_dir.to_string_lossy(), data_dir), absolute_dir);
    }

    #[test]
    fn select_snapshot_rows_centers_selection_and_copies_score_summary() {
        let rows: Vec<SelectItem> = (0..10)
            .map(|index| {
                let mut row = select_chart_row(index);
                if index == 5 {
                    if let Some(analysis) = &mut row.chart_analysis {
                        analysis.speed_changes = vec![
                            crate::storage::library_db::ChartSpeedChange {
                                speed: 100.0,
                                time_ms: 0,
                            },
                            crate::storage::library_db::ChartSpeedChange {
                                speed: 200.0,
                                time_ms: 45_000,
                            },
                        ];
                    }
                    let mut best_score = best_score_with_replay(1234, "replay/test.toml");
                    best_score.bp = 12;
                    best_score.cb = 8;
                    best_score.max_combo = 345;
                    row.best_score = Some(best_score);
                    row.replay_slots = [true, false, false, false];
                    row.table_text =
                        DifficultyTableText::from_parts("Test Table".to_string(), "T", "5");
                    row.table_level = row.table_text.table_level.clone();
                }
                SelectItem::Chart(row)
            })
            .collect();

        let profile = ProfileConfig::new_default("default", "Default", 0);
        let mut chart_distributions = HashMap::new();
        chart_distributions.insert(
            5,
            vec![crate::storage::library_db::ChartDistributionSecond {
                key_taps: 2,
                key_long_heads: 1,
                ..Default::default()
            }],
        );
        let snapshot_rows = select_snapshot_rows(&rows, 5, 7, &profile, None, &chart_distributions);

        assert_eq!(snapshot_rows.len(), 7);
        assert_eq!(snapshot_rows[0].index, 2);
        assert_eq!(snapshot_rows[3].index, 5);
        assert_eq!(snapshot_rows[3].title, "Title 5");
        assert_eq!(snapshot_rows[3].clear_type, "Normal");
        assert_eq!(snapshot_rows[3].ex_score, Some(1234));
        assert_eq!(snapshot_rows[3].bp, Some(12));
        assert_eq!(snapshot_rows[3].cb, Some(8));
        assert_eq!(snapshot_rows[3].max_combo, Some(345));
        assert_eq!(snapshot_rows[3].judge_rank, Some(1));
        assert_eq!(snapshot_rows[3].play_count, 42);
        assert_eq!(snapshot_rows[3].clear_count, 31);
        assert_eq!(snapshot_rows[3].replay_slots, [true, false, false, false]);
        assert_eq!(snapshot_rows[3].chart_normal_notes, 45);
        assert_eq!(snapshot_rows[3].chart_long_notes, 6);
        assert_eq!(snapshot_rows[3].chart_peak_density, 12.5);
        assert_eq!(snapshot_rows[3].chart_distribution.len(), 1);
        assert_eq!(snapshot_rows[3].chart_distribution[0].key_taps, 2);
        assert_eq!(snapshot_rows[3].chart_bpm_graph_segments.len(), 2);
        assert_eq!(snapshot_rows[3].chart_bpm_graph_segments[0].start_ratio, 0.0);
        assert_eq!(snapshot_rows[3].chart_bpm_graph_segments[0].end_ratio, 0.5);
        assert_eq!(snapshot_rows[3].chart_bpm_graph_segments[1].start_ratio, 0.5);
        assert_eq!(snapshot_rows[3].chart_bpm_graph_segments[1].end_ratio, 1.0);
        assert_eq!(snapshot_rows[3].table_text_primary, "Test Table");
        assert_eq!(snapshot_rows[3].table_text_secondary, "T5");
        assert_eq!(snapshot_rows[3].table_text_fallback, "T5Test Table");
    }

    #[test]
    fn select_snapshot_rows_preserves_settings_action_kinds() {
        let rows = vec![SelectItem::SettingsBack, SelectItem::SettingsClose];
        let profile = ProfileConfig::new_default("default", "Default", 0);

        let snapshot_rows = select_snapshot_rows(&rows, 0, 2, &profile, None, &HashMap::new());

        let back = snapshot_rows
            .iter()
            .find(|row| row.kind == bmz_render::scene::SelectRowKind::SettingsBack)
            .unwrap();
        let close = snapshot_rows
            .iter()
            .find(|row| row.kind == bmz_render::scene::SelectRowKind::SettingsClose)
            .unwrap();
        assert_eq!(back.title, "戻る");
        assert_eq!(close.title, "閉じる");
        assert!(back.is_folder);
        assert!(close.is_folder);
    }

    #[test]
    fn select_snapshot_rows_uses_policy_scored_note_count() {
        let mut row = select_chart_row(0);
        let chart = row.chart.as_mut().unwrap();
        chart.total_notes = 100;
        chart.bms_total = 0.0;
        chart.ln_profile =
            crate::ln_policy::ChartLnProfile { has_defined_cn: true, ..Default::default() };
        chart.ln_counts =
            crate::ln_policy::ChartLnCounts { defined_cn_pairs: 2, ..Default::default() };
        let rows = vec![SelectItem::Chart(row)];
        let profile = ProfileConfig::new_default("default", "Default", 0);

        let snapshot = select_snapshot_rows(&rows, 0, 1, &profile, None, &HashMap::new());

        assert_eq!(snapshot[0].total_notes, 102);
        assert_eq!(
            snapshot[0].chart_total_gauge,
            bmz_gameplay::gauge::default_gauge_total(102) as f32
        );
    }

    #[test]
    fn select_snapshot_rows_copies_course_best_score_summary() {
        let mut row = select_course_row(2, 2);
        row.best_score = Some(crate::storage::score_db::CourseBestScore {
            course_score_id: 99,
            course_hash: "course-hash".to_string(),
            rule_mode: bmz_gameplay::rule::RuleMode::Beatoraja,
            ex_score: 1234,
            max_ex_score: 2000,
            clear_type: "Hard".to_string(),
            gauge_type: "Class".to_string(),
            gauge_value: 80.0,
            max_combo: 345,
            bp: 12,
            cb: 8,
            judge_counts: DisplayJudgeCounts {
                pgreat: 500,
                great: 100,
                good: 20,
                bad: 10,
                poor: 5,
                empty_poor: 3,
            },
            fast_slow_counts: bmz_render::snapshot::FastSlowJudgeCounts {
                fast_pgreat: 300,
                slow_pgreat: 200,
                ..Default::default()
            },
            course_failed: false,
            course_clear: true,
            play_count: 42,
            clear_count: 31,
            played_at: 1,
        });
        row.replay_slots = [true, false, true, false];
        let rows = vec![SelectItem::Course(row)];

        let profile = ProfileConfig::new_default("default", "Default", 0);
        let snapshot_rows = select_snapshot_rows(&rows, 0, 1, &profile, None, &HashMap::new());

        assert_eq!(snapshot_rows.len(), 1);
        assert_eq!(snapshot_rows[0].kind, bmz_render::scene::SelectRowKind::Course);
        assert!(snapshot_rows[0].play_level.is_empty());
        assert_eq!(snapshot_rows[0].clear_type, "Hard");
        assert_eq!(snapshot_rows[0].ex_score, Some(1234));
        assert_eq!(snapshot_rows[0].bp, Some(12));
        assert_eq!(snapshot_rows[0].cb, Some(8));
        assert_eq!(snapshot_rows[0].max_combo, Some(345));
        assert_eq!(snapshot_rows[0].judge_counts.pgreat, 500);
        assert_eq!(snapshot_rows[0].judge_counts.empty_poor, 3);
        assert_eq!(snapshot_rows[0].fast_slow_counts.unwrap().fast_pgreat, 300);
        assert_eq!(snapshot_rows[0].play_count, 42);
        assert_eq!(snapshot_rows[0].clear_count, 31);
        assert_eq!(snapshot_rows[0].replay_slots, [true, false, true, false]);
    }

    #[test]
    fn select_snapshot_rows_wraps_near_edges() {
        let rows: Vec<SelectItem> =
            (0..4).map(|i| SelectItem::Chart(select_chart_row(i))).collect();

        let profile = ProfileConfig::new_default("default", "Default", 0);
        let snapshot_rows = select_snapshot_rows(&rows, 0, 7, &profile, None, &HashMap::new());

        assert_eq!(snapshot_rows.len(), 7);
        assert_eq!(
            snapshot_rows.iter().map(|row| row.index).collect::<Vec<_>>(),
            vec![1, 2, 3, 0, 1, 2, 3]
        );
    }

    #[test]
    fn select_snapshot_rows_keeps_twelve_rows_around_selection() {
        let rows: Vec<SelectItem> =
            (0..30).map(|i| SelectItem::Chart(select_chart_row(i))).collect();

        let profile = ProfileConfig::new_default("default", "Default", 0);
        let snapshot_rows = select_snapshot_rows(&rows, 2, 25, &profile, None, &HashMap::new());

        assert_eq!(snapshot_rows.len(), 25);
        assert_eq!(snapshot_rows[0].index, 20);
        assert_eq!(snapshot_rows[12].index, 2);
        assert_eq!(snapshot_rows[24].index, 14);
    }

    #[test]
    fn course_rows_are_playable_only_when_all_entries_resolve() {
        let rows = vec![
            SelectItem::Course(select_course_row(4, 4)),
            SelectItem::Course(select_course_row(3, 4)),
        ];

        let profile = ProfileConfig::new_default("default", "Default", 0);
        let snapshot_rows = select_snapshot_rows(&rows, 0, 2, &profile, None, &HashMap::new());

        assert!(snapshot_rows.iter().any(|row| row.title == "Course 4/4" && row.in_library));
        assert!(snapshot_rows.iter().any(|row| row.title == "Course 3/4" && !row.in_library));
        let partial = snapshot_rows.iter().find(|row| row.title == "Course 3/4").unwrap();
        assert_eq!(partial.course_titles[0], "Stage 1");
        assert_eq!(partial.course_titles[3], "(no song) Stage 4");
    }

    #[test]
    fn course_constraint_flags_match_beatoraja_gradebar_ops() {
        let constraints = bmz_core::course::CourseConstraints {
            class: bmz_core::course::CourseClassConstraint::GradeRandomAllowed,
            speed: bmz_core::course::CourseSpeedConstraint::NoSpeed,
            judge: bmz_core::course::CourseJudgeConstraint::NoGood,
            gauge: bmz_core::course::CourseGaugeConstraint::Keys24,
            ln: bmz_core::course::CourseLnConstraint::Cn,
            source_constraints: Vec::new(),
        };

        let flags = course_constraint_flags(&constraints);

        assert!(!flags.class);
        assert!(!flags.mirror);
        assert!(flags.random);
        assert!(flags.no_speed);
        assert!(flags.no_good);
        assert!(!flags.no_great);
        assert!(flags.gauge_24k);
        assert!(!flags.gauge_7k);
        assert!(flags.cn);
        assert!(!flags.hcn);
    }

    #[test]
    fn moved_select_index_moves_by_single_page_and_wraps_edges() {
        assert_eq!(moved_select_index(4, 10, SelectMove::Previous), 3);
        assert_eq!(moved_select_index(4, 10, SelectMove::Next), 5);
        assert_eq!(moved_select_index(9, 10, SelectMove::Next), 0);
        assert_eq!(moved_select_index(0, 10, SelectMove::Previous), 9);
        assert_eq!(moved_select_index(8, 10, SelectMove::PagePrevious), 1);
        assert_eq!(moved_select_index(4, 10, SelectMove::PagePrevious), 7);
        assert_eq!(moved_select_index(7, 10, SelectMove::PageNext), 4);
        assert_eq!(moved_select_index(0, 10, SelectMove::Last), 9);
        assert_eq!(moved_select_index(9, 10, SelectMove::First), 0);
    }

    #[test]
    fn moved_select_index_handles_empty_rows() {
        assert_eq!(moved_select_index(9, 0, SelectMove::Last), 0);
    }

    #[test]
    fn select_scroll_duration_config_uses_beatoraja_bounds() {
        let mut config = AppConfig::default();
        config.select.scroll_duration_low_ms = 0;
        config.select.scroll_duration_high_ms = 0;
        assert_eq!(select_scroll_duration_low_ms(&config), 2);
        assert_eq!(select_scroll_duration_high_ms(&config), 1);

        config.select.scroll_duration_low_ms = 5_000;
        config.select.scroll_duration_high_ms = 5_000;
        assert_eq!(select_scroll_duration_low_ms(&config), 1000);
        assert_eq!(select_scroll_duration_high_ms(&config), 1000);
    }

    #[test]
    fn select_move_scroll_direction_matches_row_movement() {
        assert_eq!(select_move_scroll_direction(SelectMove::Previous), -1);
        assert_eq!(select_move_scroll_direction(SelectMove::Next), 1);
        assert_eq!(select_move_scroll_direction(SelectMove::PagePrevious), -1);
        assert_eq!(select_move_scroll_direction(SelectMove::PageNext), 1);
        assert_eq!(select_move_scroll_direction(SelectMove::First), 0);
        assert_eq!(select_move_scroll_direction(SelectMove::Last), 0);
    }

    #[test]
    fn select_skin_event_state_cycles_supported_mode_filters() {
        assert_eq!(SelectModeFilter::All.next(), SelectModeFilter::K7);
        assert_eq!(SelectModeFilter::All.previous(), SelectModeFilter::K10);
        assert_eq!(SelectSort::Title.next(), SelectSort::Artist);
        assert_eq!(SelectSort::Title.previous(), SelectSort::Bp);
        assert_eq!(
            crate::ln_policy::LnPolicySetting::AutoLn.next(),
            crate::ln_policy::LnPolicySetting::AutoCn
        );
        assert_eq!(
            crate::ln_policy::LnPolicySetting::AutoLn.previous(),
            crate::ln_policy::LnPolicySetting::ForceHcn
        );
        assert_eq!(crate::ln_policy::LnPolicySetting::ForceHcn.display_label(), "FORCE(HCN)");
        assert_eq!(
            cycle_gauge_option_with_direction(GaugeTypeConfig::Normal, 1),
            GaugeTypeConfig::Hard
        );
        assert_eq!(
            cycle_gauge_option_with_direction(GaugeTypeConfig::Normal, -1),
            GaugeTypeConfig::Easy
        );
        assert_eq!(
            cycle_arrange_option_with_direction(ArrangeOption::Normal, -1),
            ArrangeOption::MFRandom
        );
        assert_eq!(
            cycle_double_option_with_direction(DoubleOption::Off, -1),
            DoubleOption::BattleAutoScratch
        );
        assert_eq!(cycle_hs_fix_option_with_direction(HsFixOption::Off, 1), HsFixOption::StartBpm);
        assert_eq!(
            cycle_hs_fix_option_with_direction(HsFixOption::StartBpm, 1),
            HsFixOption::MaxBpm
        );
        assert_eq!(
            cycle_hs_fix_option_with_direction(HsFixOption::MaxBpm, 1),
            HsFixOption::MainBpm
        );
        assert_eq!(
            cycle_hs_fix_option_with_direction(HsFixOption::MainBpm, 1),
            HsFixOption::MinBpm
        );
        assert_eq!(cycle_hs_fix_option_with_direction(HsFixOption::Off, -1), HsFixOption::MinBpm);
        assert_eq!(cycle_bga_option_with_direction(BgaModeConfig::On, -1), BgaModeConfig::Off);
        assert_eq!(
            cycle_bga_expand_with_direction(BgaExpandConfig::KeepAspect, 1),
            BgaExpandConfig::Full
        );
        assert_eq!(
            cycle_gauge_auto_shift_option_with_direction(GaugeAutoShiftConfig::Off, -1),
            GaugeAutoShiftConfig::SelectToUnder
        );
        assert_eq!(
            cycle_judge_algorithm_with_direction(JudgeAlgorithmConfig::Combo, 1),
            JudgeAlgorithmConfig::Duration
        );
        assert_eq!(
            cycle_judge_algorithm_with_direction(JudgeAlgorithmConfig::Combo, -1),
            JudgeAlgorithmConfig::Lowest
        );
    }

    #[test]
    fn play_skin_key_mode_uses_battle_double_mode() {
        assert_eq!(
            play_skin_key_mode_for_options(KeyMode::K7, DoubleOption::Battle, SessionMode::Normal,),
            KeyMode::K14
        );
        assert_eq!(
            play_skin_key_mode_for_options(
                KeyMode::K7,
                DoubleOption::BattleAutoScratch,
                SessionMode::Normal,
            ),
            KeyMode::K14
        );
        assert_eq!(
            play_skin_key_mode_for_options(KeyMode::K5, DoubleOption::Battle, SessionMode::Normal,),
            KeyMode::K10
        );
        assert_eq!(
            play_skin_key_mode_for_options(KeyMode::K7, DoubleOption::Flip, SessionMode::Normal,),
            KeyMode::K7
        );
        assert_eq!(
            play_skin_key_mode_for_options(KeyMode::K14, DoubleOption::Battle, SessionMode::Normal,),
            KeyMode::K14
        );
        assert_eq!(
            play_skin_key_mode_for_options(
                KeyMode::K7,
                DoubleOption::Off,
                SessionMode::GhostBattle,
            ),
            KeyMode::K14
        );
    }

    #[test]
    fn select_ir_context_separates_source_resolved_score_keys() {
        let auto_ln = select_ir_cache_context(
            crate::ln_policy::LnPolicySetting::AutoLn,
            crate::ln_policy::LnScorePolicy::AutoLn,
            crate::select_options::DoubleOptionScoreBucket::Off,
            bmz_gameplay::rule::RuleMode::Beatoraja,
        );
        let auto_cn = select_ir_cache_context(
            crate::ln_policy::LnPolicySetting::AutoLn,
            crate::ln_policy::LnScorePolicy::AutoCn,
            crate::select_options::DoubleOptionScoreBucket::Off,
            bmz_gameplay::rule::RuleMode::Beatoraja,
        );

        assert_ne!(auto_ln, auto_cn);
    }

    #[test]
    fn select_mode_filter_keeps_matching_chart_rows() {
        let mut k7 = select_chart_row(1);
        k7.chart.as_mut().unwrap().mode = "7K".to_string();
        let mut k14 = select_chart_row(2);
        k14.chart.as_mut().unwrap().mode = "14K".to_string();
        let mut items = vec![
            SelectItem::Folder {
                path: "folder".to_string(),
                name: "folder".to_string(),
                kind: SelectRowKind::Folder,
                summary: None,
            },
            SelectItem::Chart(k7),
            SelectItem::Chart(k14),
        ];

        apply_select_mode_filter(&mut items, SelectModeFilter::K14);

        assert_eq!(items.len(), 2);
        assert!(matches!(items[0], SelectItem::Folder { .. }));
        assert_eq!(items[1].display_name(), "Title 2");
    }

    fn chart_row_with_mode(index: usize, mode: &str) -> SelectItem {
        let mut row = select_chart_row(index);
        row.chart.as_mut().unwrap().mode = mode.to_string();
        SelectItem::Chart(row)
    }

    #[test]
    fn clear_rank_separates_unowned_from_noplay() {
        // 所持済み・スコア無し → NoPlay = 0。
        let noplay = select_chart_row(1);
        assert!(noplay.in_library());
        assert_eq!(clear_rank(&noplay), 0);

        // 難易度表エントリだがローカル未所持 → NoPlay より下位の -1。
        let mut unowned = select_chart_row(2);
        unowned.chart = None;
        unowned.entry_sha256 = Some([2u8; 32]);
        assert!(!unowned.in_library());
        assert_eq!(clear_rank(&unowned), -1);

        assert!(clear_rank(&unowned) < clear_rank(&noplay));
    }

    #[test]
    fn resolve_mode_filter_keeps_mode_with_matching_charts() {
        let items = vec![chart_row_with_mode(1, "7K"), chart_row_with_mode(2, "5K")];
        // 7K のチャートがあるので据え置く。
        assert_eq!(
            resolve_non_empty_mode_filter(&items, SelectModeFilter::K7),
            SelectModeFilter::K7
        );
    }

    #[test]
    fn resolve_mode_filter_advances_when_all_charts_mismatch() {
        // 5K しか無いフォルダで 7K フィルターを掛けると全消えになるため、
        // beatoraja 同様に前方向 (K7 -> K14 -> K9 -> K5) へ送って K5 で止まる。
        let items = vec![chart_row_with_mode(1, "5K"), chart_row_with_mode(2, "5K")];
        assert_eq!(
            resolve_non_empty_mode_filter(&items, SelectModeFilter::K7),
            SelectModeFilter::K5
        );
    }

    #[test]
    fn resolve_mode_filter_does_not_advance_when_folder_remains() {
        // フォルダ行が残るなら全消えにはならないので据え置く（beatoraja 準拠）。
        let items = vec![
            SelectItem::Folder {
                path: "folder".to_string(),
                name: "folder".to_string(),
                kind: SelectRowKind::Folder,
                summary: None,
            },
            chart_row_with_mode(1, "5K"),
        ];
        assert_eq!(
            resolve_non_empty_mode_filter(&items, SelectModeFilter::K7),
            SelectModeFilter::K7
        );
    }

    #[test]
    fn resolve_mode_filter_keeps_all_filter() {
        let items = vec![chart_row_with_mode(1, "5K")];
        assert_eq!(
            resolve_non_empty_mode_filter(&items, SelectModeFilter::All),
            SelectModeFilter::All
        );
    }

    #[test]
    fn select_mode_filter_roundtrips_through_str() {
        for mode in SelectModeFilter::ORDER {
            assert_eq!(SelectModeFilter::from_str_or_default(mode.as_str()), mode);
        }
        assert_eq!(SelectModeFilter::from_str_or_default("24K"), SelectModeFilter::All);
        assert_eq!(SelectModeFilter::from_str_or_default("24K_DOUBLE"), SelectModeFilter::All);
        assert_eq!(SelectModeFilter::from_str_or_default("unknown"), SelectModeFilter::All);
    }

    #[test]
    fn select_sort_roundtrips_through_str() {
        for sort in SelectSort::ORDER {
            assert_eq!(SelectSort::from_str_or_default(sort.as_str()), sort);
        }
        assert_eq!(SelectSort::from_str_or_default("unknown"), SelectSort::Title);
    }

    #[test]
    fn select_sort_orders_chart_rows_without_moving_folders() {
        let mut slow = select_chart_row(1);
        slow.chart.as_mut().unwrap().title = "Slow".to_string();
        slow.chart.as_mut().unwrap().initial_bpm = 100.0;
        let mut fast = select_chart_row(2);
        fast.chart.as_mut().unwrap().title = "Fast".to_string();
        fast.chart.as_mut().unwrap().initial_bpm = 200.0;
        let mut items = vec![
            SelectItem::Folder {
                path: "folder".to_string(),
                name: "folder".to_string(),
                kind: SelectRowKind::Folder,
                summary: None,
            },
            SelectItem::Chart(fast),
            SelectItem::Chart(slow),
        ];

        apply_select_sort(&mut items, SelectSort::Bpm);

        assert!(matches!(items[0], SelectItem::Folder { .. }));
        assert_eq!(items[1].display_name(), "Slow");
        assert_eq!(items[2].display_name(), "Fast");
    }

    #[test]
    fn restored_select_index_keeps_chart_when_clear_sort_moves_after_score_update() {
        let mut played = select_chart_row(1);
        played.chart.as_mut().unwrap().title = "Played".to_string();
        let mut other = select_chart_row(2);
        other.chart.as_mut().unwrap().title = "Other".to_string();
        let old_items = vec![SelectItem::Chart(played.clone()), SelectItem::Chart(other.clone())];
        let selected_key = select_item_key(&old_items[0]);

        played.best_score = Some(BestScoreSummary {
            clear_type: "Hard".to_string(),
            ..best_score_with_replay(100, "played.json")
        });
        let mut new_items = vec![SelectItem::Chart(played), SelectItem::Chart(other)];
        apply_select_sort(&mut new_items, SelectSort::Clear);

        assert_eq!(restored_select_index(&new_items, Some(&selected_key), 0), 1);
        assert_eq!(new_items[1].display_name(), "Played");
    }

    #[test]
    fn select_item_key_uses_typed_settings_identity() {
        let config = SelectItem::Config(crate::screens::settings_model::ConfigSelectRow {
            entry_id: SettingsEntryId::MasterVolume,
        });
        assert_eq!(select_item_key(&config), SelectItemKey::Config(SettingsEntryId::MasterVolume));

        let binding = SelectItem::KeyBinding(crate::screens::settings_model::KeyBindingSelectRow {
            key_mode: KeyMode::K7,
            target: KeyBindingTarget::Action {
                action: InputActionConfig::E1,
                slot: KeyBindingSlot::KeyboardPrimary,
            },
        });
        assert_eq!(
            select_item_key(&binding),
            SelectItemKey::KeyBinding {
                key_mode: KeyMode::K7,
                target: KeyBindingTarget::Action {
                    action: InputActionConfig::E1,
                    slot: KeyBindingSlot::KeyboardPrimary,
                },
            }
        );
    }

    fn select_chart_row(index: usize) -> SelectChartRow {
        SelectChartRow {
            chart: Some(ChartListItem {
                chart_id: index as i64,
                md5: [0u8; 16],
                sha256: [index as u8; 32],
                title: format!("Title {index}"),
                subtitle: String::new(),
                artist: format!("Artist {index}"),
                subartist: String::new(),
                genre: String::new(),
                difficulty_name: String::new(),
                play_level: index.to_string(),
                mode: "7K".to_string(),
                total_notes: 100,
                initial_bpm: 128.0,
                min_bpm: 128.0,
                max_bpm: 128.0,
                length_ms: 90_000,
                folder_path: String::new(),
                stage_file: String::new(),
                banner_file: String::new(),
                backbmp_file: String::new(),
                preview_file: String::new(),
                has_document: false,
                has_long_notes: false,
                has_mines: false,
                judge_rank: Some(1),
                bms_total: 200.0,
                ln_profile: Default::default(),
                ln_counts: Default::default(),
            }),
            chart_analysis: Some(crate::storage::library_db::ChartAnalysisSummary {
                normal_notes: 40 + index as u32,
                long_notes: 1 + index as u32,
                scratch_notes: 3,
                long_scratch_notes: 1,
                density: 4.5,
                peak_density: 12.5,
                end_density: 8.25,
                total_gauge: 260.0,
                main_bpm: 128.0,
                speed_changes: Vec::new(),
            }),
            has_document: false,
            fallback_title: String::new(),
            fallback_artist: String::new(),
            entry_sha256: None,
            download_metadata: crate::song_download::ChartDownloadMetadata::default(),
            best_score: None,
            replay_slots: [false; 4],
            favorite_chart: false,
            favorite_song: false,
            table_level: String::new(),
            table_text: DifficultyTableText::default(),
        }
    }

    fn select_course_row(resolved_count: usize, entry_count: usize) -> SelectCourseRow {
        let entry_previews = (0..entry_count)
            .map(|index| crate::screens::select_model::CourseEntryPreview {
                title: format!("Stage {}", index + 1),
                artist: String::new(),
                play_level: String::new(),
                difficulty_name: String::new(),
                total_notes: 0,
                resolved: index < resolved_count,
            })
            .collect();
        SelectCourseRow {
            course_id: resolved_count as i64,
            course_hash: None,
            rian_course_hash_v1: None,
            title: format!("Course {resolved_count}/{entry_count}"),
            kind: bmz_core::course::CourseKind::Dan,
            constraints: bmz_core::course::CourseConstraints::default(),
            entry_count,
            resolved_count,
            total_notes: 100,
            total_length_ms: 90_000,
            min_bpm: 128.0,
            max_bpm: 128.0,
            category_label: "DAN".to_string(),
            trophy_names: Vec::new(),
            entry_previews,
            best_score: None,
            replay_slots: [false; 4],
            achieved_trophy_names: Vec::new(),
        }
    }

    fn best_score_with_replay(ex_score: u32, replay_path: &str) -> BestScoreSummary {
        BestScoreSummary {
            chart_sha256: [0; 32],
            ln_policy: crate::ln_policy::LnScorePolicy::ForceLn,
            double_option: crate::select_options::DoubleOptionScoreBucket::Off,
            rule_mode: bmz_gameplay::rule::RuleMode::Beatoraja,
            clear_type: "Normal".to_string(),
            gauge_type: "Normal".to_string(),
            gauge_value: Some(80.0),
            ex_score,
            bp: 0,
            cb: 0,
            max_combo: 100,
            judge_counts: DisplayJudgeCounts::default(),
            fast_slow_counts: FastSlowJudgeCounts::default(),
            play_count: 42,
            clear_count: 31,
            device_type: bmz_core::input::InputDeviceKind::Keyboard,
            played_at: 1,
            replay_path: replay_path.to_string(),
        }
    }
}
