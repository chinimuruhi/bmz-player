use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
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
    TARGET_GREEN_NUMBER_MAX, TARGET_GREEN_NUMBER_MIN, clamp_hispeed,
    input_bounce_config_from_profile,
};
use crate::config::profile_config::{
    BgaExpandConfig, BgaModeConfig, BottomShiftableGaugeConfig, DoubleOptionConfig,
    GaugeAutoShiftConfig, GaugeTypeConfig, HispeedDirectionConfig, HispeedModeConfig, HsFixConfig,
    InputActionConfig, JudgeAlgorithmConfig, LaneEffectConfig, LaneViewConfig, PlayDefaultsConfig,
    ProfileConfig, ProfileInputConfig, RandomOptionConfig, RivalSourceConfig, SkinConfig,
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
use crate::screens::play_session::build_practice_prepared_from_preloaded;
use crate::screens::play_session::{AppliedArrange, PreparedPlayChart};
use crate::screens::play_snapshot::{
    BgaFrameCatalog, apply_fast_slow_display_filter, apply_prepared_chart_to_render_snapshot,
    bga_texture_id, build_render_snapshot_with_target_and_bga_frames_cached, display_bga_frame,
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
    TABLE_ROOT_PATH, TablePath, VIRTUAL_FOLDER_PATH_PREFIX, apply_collection_flags,
    course_root_item, difficulty_table_text_for_chart_with_active_sources, favorite_root_item,
    favorite_root_items, favorite_song_representatives_for_folder, load_select_items_for_courses,
    load_select_items_for_favorite_charts, load_select_items_for_favorite_song,
    load_select_items_for_favorite_songs, load_select_items_for_search_for_rule_mode_with_filters,
    load_select_items_in_folder_for_rule_mode_with_filters,
    load_select_items_in_table_level_for_rule_mode, load_select_items_in_virtual_folder,
    new_course_item_for_locale, parse_favorite_song_detail_path, parse_same_folder_path,
    parse_search_query, parse_table_path, random_select_item_from_items, root_folder_items,
    same_folder_path, search_history_folder_items_for_locale, song_scan_path_from_context,
    table_folder_items_for_active_sources, table_level_folder_items, table_source_url_from_context,
    virtual_folder_breadcrumb, virtual_folder_root_items,
};
use crate::screens::settings_edit::{SettingsBindings, SettingsEditSession, adjust_settings_draft};
use crate::screens::settings_model::{
    in_settings_stack, load_settings_items_for_locale, settings_breadcrumb_for_locale,
    settings_root_item_for_locale,
};
use crate::select_options::{
    ArrangeOption, DoubleOption, HsFixOption, ResolvedTarget, SessionMode, TargetOption,
};
use crate::skin_loader::{
    BeatorajaSkinDecodeRequest, DecodedSkin, PreparedSource, SharedSkinGpuTextureCache,
    SkinFontCacheKey, SkinKind, UploadedSkin, decode_beatoraja_skin_request,
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
    CourseEditorAction, CourseEditorChart, CourseEditorData, DebugInfo, EguiLayer, EguiRunContext,
    SceneSkinDefs, SelectCourseBuilderAction, SelectCourseBuilderData, SkinCandidate,
    SkinCandidateOrigin, SkinCatalog, SkinConfigMeta, SkinReloadRequest, SongScanRequest,
    UpdateDialog, UpdateDialogAction,
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

mod app_support;
mod background_jobs;
mod bga_runtime;
mod chart_assets;
#[path = "app/course_editor.rs"]
mod course_editor;
#[path = "app/course_flow/advance.rs"]
mod course_flow_advance;
#[path = "app/course_flow/finish.rs"]
mod course_flow_finish;
#[path = "app/course_flow/ir.rs"]
mod course_flow_ir;
#[path = "app/course_flow/metrics.rs"]
mod course_flow_metrics;
#[path = "app/course_flow/start.rs"]
mod course_flow_start;
#[path = "app/course_metrics_state.rs"]
mod course_metrics_state;
mod frame_flow;
mod frame_runtime;
mod input_runtime;
mod integration_support;
mod integrations;
mod maintenance;
mod pending_state;
mod play_control;
#[path = "app/play_flow/audio.rs"]
mod play_flow_audio;
#[path = "app/play_flow/launch/bga.rs"]
mod play_flow_launch_bga;
#[path = "app/play_flow/launch/poll.rs"]
mod play_flow_launch_poll;
#[path = "app/play_flow/launch/preload.rs"]
mod play_flow_launch_preload;
#[path = "app/play_flow/launch/start.rs"]
mod play_flow_launch_start;
#[path = "app/play_flow/practice.rs"]
mod play_flow_practice;
#[path = "app/play_flow/replay.rs"]
mod play_flow_replay;
#[path = "app/play_flow/retry.rs"]
mod play_flow_retry;
mod play_loop_flow;
mod play_preload_state;
mod play_support;
mod play_transition_state;
#[path = "app/result_flow/ending.rs"]
mod result_flow_ending;
#[path = "app/result_flow/interaction.rs"]
mod result_flow_interaction;
#[path = "app/result_flow/timing.rs"]
mod result_flow_timing;
#[path = "app/result_flow/transition.rs"]
mod result_flow_transition;
mod result_runtime;
mod result_support;
#[path = "app/result_support/timing.rs"]
mod result_timing_support;
mod rival_sync;
mod runtime_state;
mod scene_input;
mod select_assets;
#[path = "app/select_course_builder.rs"]
mod select_course_builder;
#[path = "app/select_flow/controls.rs"]
mod select_flow_controls;
#[path = "app/select_flow/gamepad.rs"]
mod select_flow_gamepad;
#[path = "app/select_flow/keyboard.rs"]
mod select_flow_keyboard;
#[path = "app/select_flow/mode_config.rs"]
mod select_flow_mode_config;
#[path = "app/select_flow/navigation.rs"]
mod select_flow_navigation;
#[path = "app/select_flow/pointer.rs"]
mod select_flow_pointer;
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
mod skin_catalog;
#[path = "app/skin_flow/profile.rs"]
mod skin_flow_profile;
#[path = "app/skin_flow/reload.rs"]
mod skin_flow_reload;
#[path = "app/skin_flow/upload.rs"]
mod skin_flow_upload;
#[path = "app/skin_flow/video.rs"]
mod skin_flow_video;
mod skin_loading;
mod skin_options;
mod skin_pipeline;
mod skin_runtime_types;
mod skin_video;
mod skin_workers;
mod table_fetch_runtime;
mod update_prompt;

use app_support::*;
use course_metrics_state::*;
use pending_state::*;
use play_preload_state::*;
use play_transition_state::*;
use runtime_state::*;
use skin_runtime_types::*;
use update_prompt::*;

use select_key_bindings::{
    SelectKeyBindings, play_analog_lane_cover_delta, select_analog_scroll_delta,
    take_analog_scroll_steps, update_analog_scroll_buffer,
};

#[cfg(test)]
use crate::config::profile_config::{LaneConfig, SelectInputModeConfig};

use bga_runtime::{
    BgaImageLoadStatus, BgaPreloadRuntime, PendingBgaImageResult, RESOURCE_LOAD_PROGRESS_SCALE,
    combined_resource_load_progress, load_worker as chart_bga_texture_load_worker,
    resource_load_progress_units,
};
use chart_assets::*;
use frame_runtime::{
    AppLoopFrameTimings, FramePacingState, FrameProfileKind, FrameRuntime, FrameSchedule,
    FrameWindowMode, SceneFrameProfileSample, SkinVideoFrameProfile,
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
use result_timing_support::*;
use rival_sync::*;
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
use skin_catalog::*;
use skin_loading::*;
use skin_options::*;
use skin_pipeline::SkinPipelineRuntime;
use skin_video::*;
use skin_workers::*;
use table_fetch_runtime::{
    RianTableFetchOutcome, RianTableFetchWorkerResult, TableFetchProgress, TableFetchRuntime,
    TableFetchWorkerEvent, startup_difficulty_table_fetch_urls_for_boot,
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
    SkinUpload { sent_at: Instant },
    TableFetch,
    RivalSync,
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

    // Raw Input へ実行中に切り替えられるよう、Windows message hook は起動時から
    // 常設する。デバイス usage の登録は RawInputBackend の attach 時まで行わない。
    let raw_input_bridge = cfg!(windows).then(crate::input::rawinput::RawInputBridge::new);
    let mut event_loop_builder = EventLoop::<AppUserEvent>::with_user_event();
    #[cfg(windows)]
    if let Some(bridge) = raw_input_bridge.clone() {
        use winit::platform::windows::EventLoopBuilderExtWindows;

        event_loop_builder.with_msg_hook(move |message| {
            bridge.handle_message(message);
            false
        });
    }
    let event_loop = event_loop_builder.build().context("failed to create event loop")?;
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

    let (maintenance_select_tx, maintenance_select_rx) = tokio::sync::watch::channel(false);
    spawn_ir_sync_worker(&boot, maintenance_select_rx);

    let mut app = Box::new(WinitApp::new(
        boot,
        options,
        None,
        None,
        shutdown_requested,
        event_proxy,
        log_buffer,
        maintenance_select_tx,
        raw_input_bridge,
    )?);
    tracing::info!("starting winit event loop");
    event_loop.run_app(app.as_mut()).context("winit event loop failed")
}

/// IR スコアジョブをバックグラウンドで定期送信する。
///
/// メインスレッドの DB connection とは別 connection を開く (DB は WAL)。
/// IR が未設定なら何もしない。
fn spawn_ir_sync_worker(
    boot: &bootstrap::BootstrappedApp,
    mut select_rx: tokio::sync::watch::Receiver<bool>,
) {
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
        let interval = std::time::Duration::from_secs(crate::ir::sync::IR_SYNC_LOOP_INTERVAL_SECS);
        let mut next_run_at = tokio::time::Instant::now();
        loop {
            while !*select_rx.borrow() {
                if select_rx.changed().await.is_err() {
                    return;
                }
            }
            if tokio::time::Instant::now() < next_run_at {
                tokio::select! {
                    _ = tokio::time::sleep_until(next_run_at) => {}
                    changed = select_rx.changed() => {
                        if changed.is_err() {
                            return;
                        }
                        continue;
                    }
                }
            }
            if !*select_rx.borrow() {
                continue;
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            if let Err(error) = migrate_network_db(&network_db_path) {
                tracing::warn!(%error, "failed to migrate network db for IR sync");
                next_run_at = tokio::time::Instant::now() + interval;
                continue;
            }
            match crate::storage::network_db::NetworkDatabase::open(&network_db_path) {
                Ok(mut network_db) => {
                    let mut submitted = 0_u32;
                    let mut failed = 0_u32;
                    for index in 0..crate::ir::sync::IR_SYNC_BATCH_LIMIT {
                        if !*select_rx.borrow() {
                            break;
                        }
                        let job_now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(now);
                        match crate::ir::sync::sync_pending_ir_jobs(
                            &mut network_db,
                            &score_db_path,
                            &profile_root,
                            &logs_dir,
                            &ir_config,
                            job_now,
                            1,
                            false,
                            crate::ir::sync::IrSyncThrottle::none(),
                        )
                        .await
                        {
                            Ok(report) => {
                                submitted = submitted.saturating_add(report.submitted);
                                failed = failed.saturating_add(report.failed);
                                if report.submitted == 0 && report.failed == 0 {
                                    break;
                                }
                            }
                            Err(error) => {
                                tracing::warn!(%error, "IR score sync failed");
                                break;
                            }
                        }
                        if index + 1 < crate::ir::sync::IR_SYNC_BATCH_LIMIT {
                            tokio::select! {
                                _ = tokio::time::sleep(std::time::Duration::from_millis(
                                    crate::ir::sync::IR_SYNC_JOB_SPACING_MS,
                                )) => {}
                                changed = select_rx.changed() => {
                                    if changed.is_err() {
                                        return;
                                    }
                                    if !*select_rx.borrow() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if submitted > 0 || failed > 0 {
                        tracing::info!(submitted, failed, "IR score sync finished");
                    }
                }
                Err(error) => tracing::warn!(%error, "failed to open network db for IR sync"),
            }
            next_run_at = tokio::time::Instant::now() + interval;
        }
    });
}

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
    /// 実行中に Raw Input backend を生成し直すための常設 message bridge。
    raw_input_bridge: Option<crate::input::rawinput::RawInputBridge>,
    gamepad: Option<crate::input::gamepad::GamepadBackend>,
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
