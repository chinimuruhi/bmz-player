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
#[path = "app/play_flow/audio.rs"]
mod play_flow_audio;
#[path = "app/play_flow/launch.rs"]
mod play_flow_launch;
#[path = "app/play_flow/practice.rs"]
mod play_flow_practice;
#[path = "app/play_flow/replay.rs"]
mod play_flow_replay;
#[path = "app/play_flow/retry.rs"]
mod play_flow_retry;
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

#[path = "app/audio_helpers.rs"]
mod audio_helpers;
#[path = "app/constructor.rs"]
mod constructor;
#[path = "app/input_lifecycle.rs"]
mod input_lifecycle;
#[path = "app/lifecycle.rs"]
mod lifecycle;
#[path = "app/platform.rs"]
mod platform;
#[path = "app/runtime_config.rs"]
mod runtime_config;
#[path = "app/runtime_helpers.rs"]
mod runtime_helpers;
#[path = "app/scene_state.rs"]
mod scene_state;

use audio_helpers::*;
use platform::*;
use runtime_config::*;
use runtime_helpers::*;

#[cfg(test)]
#[path = "app/tests.rs"]
mod tests;
