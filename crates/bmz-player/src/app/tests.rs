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
        SkinOffsetConfig { name: Some("First".to_string()), id: 42, x: 10, ..Default::default() },
        SkinOffsetConfig { name: Some("Second".to_string()), id: 42, x: 20, ..Default::default() },
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
    assert_eq!(crate::config::play::play_offsets_from_profile(&profile).visual_offset_us, 1_000);

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

    assert_eq!(result_input_duration_for_document(Some(&document)), Duration::from_millis(1500));
    assert_eq!(result_scene_duration_for_document(Some(&document)), Duration::from_millis(2345));
}

#[test]
fn normal_result_scene_zero_disables_auto_leave() {
    let document: SkinDocument =
        serde_json::from_str(r#"{ "type": 7, "input": 1500, "scene": 0 }"#).unwrap();

    assert_eq!(result_auto_exit_duration_for_document(Some(&document), false, false), None);
}

#[test]
fn result_auto_exit_uses_scene_when_positive() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 7, "scene": 2345 }"#).unwrap();

    assert_eq!(
        result_auto_exit_duration_for_document(Some(&document), false, false),
        Some(Duration::from_millis(2345))
    );
}

#[test]
fn course_intermediate_result_waits_for_input_without_auto_advance() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 7, "scene": 2345 }"#).unwrap();

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
    assert_eq!(config_internal_resolution_mode(&config), bmz_render::InternalResolutionMode::Skin);
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

    let selected = play_input_backend_for_context(None, true, None, Some(&preload_input)).unwrap();
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
    let mut snapshot = RenderSnapshot { play_elapsed_time: TimeUs(150_000), ..Default::default() };
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

    assert!(manifest.textures.iter().any(|texture| texture.id == 1 && texture.path == "note.png"));
    assert!(
        manifest.textures.iter().any(|texture| texture.id == 2 && texture.path == "note-blue.png")
    );
    assert!(
        manifest.textures.iter().any(|texture| texture.id == 3 && texture.path == "note-red.png")
    );
    assert!(
        manifest.textures.iter().any(|texture| texture.id == 4 && texture.path == "receptor.png")
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
        manifest.textures.iter().any(|texture| texture.id == 7 && texture.path == "judge-line.png")
    );
    assert!(
        manifest
            .textures
            .iter()
            .any(|texture| texture.id == 8 && texture.path == "gauge-frame.png")
    );
    assert!(
        manifest.textures.iter().any(|texture| texture.id == 9 && texture.path == "gauge-fill.png")
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
        manifest.textures.iter().any(|texture| texture.id == 12 && texture.path == "note-mine.png")
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
    fn entry_summary(ex_score: u32, notes: u32, max_combo: u32, duration_ms: i32) -> ResultSummary {
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
    apply_result_summary_lua_load_state(&mut runtime_state, &summary, "Table", "★12", "Table ★12");
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
        runtime_state.number_values.get(&(bmz_render::skin::SKIN_REF_BMZ_COURSE_STAGE_EX_BASE + 1)),
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

    let offline =
        lua_runtime_state_for_result(false, None, false, KeyMode::K5, BTreeMap::new(), "Player");
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
        select_action(PhysicalKey::Code(KeyCode::ArrowDown), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::Next))
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::ShiftLeft), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::Previous))
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::ControlLeft), ElementState::Pressed, false, &keys),
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
        select_action(PhysicalKey::Code(KeyCode::ShiftRight), ElementState::Pressed, false, &keys),
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
        select_action(PhysicalKey::Code(KeyCode::PageDown), ElementState::Pressed, false, &keys),
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
        select_action(PhysicalKey::Code(KeyCode::ArrowDown), ElementState::Released, false, &keys),
        None
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::ArrowDown), ElementState::Pressed, true, &keys),
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
    assert_eq!(select_wheel_move(MouseScrollDelta::LineDelta(0.0, -1.0)), Some(SelectMove::Next));
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
    assert!(keys.option_hint().contains("Q+K5:HS-FIX"), "HS-FIX in hint: {}", keys.option_hint());
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
    let keyboard =
        |control: &str| (W_KEYBOARD_DEVICE_ID, PhysicalControl::KeyboardKey(control.to_string()));
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

    assert!(should_play_retire_sound_for_failed_transition(PlayState::Playing, PlayState::Failed));
    assert!(!should_play_retire_sound_for_failed_transition(PlayState::Failed, PlayState::Failed));
    assert!(!should_play_retire_sound_for_failed_transition(PlayState::Ready, PlayState::Failed));
    assert!(!should_play_retire_sound_for_failed_transition(
        PlayState::Playing,
        PlayState::Finished
    ));
}

#[test]
fn select_analog_scroll_delta_maps_scratch_bindings() {
    let gamepad_keys =
        SelectKeyBindings::from_profile(&ProfileConfig::new_default("default", "Default", 1).input);
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
    assert_eq!(settings_edit_direction_from_mouse_wheel(MouseScrollDelta::LineDelta(0.0, 1.0)), 1);
    assert_eq!(
        settings_edit_direction_from_mouse_wheel(MouseScrollDelta::PixelDelta(
            winit::dpi::PhysicalPosition::new(0.0, -12.0)
        )),
        -1
    );
}

#[test]
fn play_analog_lane_cover_delta_maps_scratch_bindings() {
    let gamepad_keys =
        SelectKeyBindings::from_profile(&ProfileConfig::new_default("default", "Default", 1).input);

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
    let gamepad_keys =
        SelectKeyBindings::from_profile(&ProfileConfig::new_default("default", "Default", 1).input);

    assert_eq!(target_cycle_from_key(PhysicalKey::Code(KeyCode::ArrowUp)), Some(TargetCycle::Next));
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
        select_action(PhysicalKey::Code(KeyCode::ArrowLeft), ElementState::Pressed, false, &keys),
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
    assert_eq!(decide_fadeout_scene_timing(Some(&document)), DecideFadeoutSceneTiming::DirectOnly);
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
    assert!(result_exit_skip_key(PhysicalKey::Code(KeyCode::Enter), ElementState::Pressed, false));
    assert!(result_exit_skip_key(PhysicalKey::Code(KeyCode::Escape), ElementState::Pressed, false));
    assert!(!result_exit_skip_key(
        PhysicalKey::Code(KeyCode::Enter),
        ElementState::Released,
        false
    ));
    assert!(!result_exit_skip_key(PhysicalKey::Code(KeyCode::Escape), ElementState::Pressed, true));
    assert!(!result_exit_skip_key(PhysicalKey::Code(KeyCode::Space), ElementState::Pressed, false));
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
    assert_eq!(cycle_result_gauge_graph_type(GaugeType::Class as i32), GaugeType::ExClass as i32);
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
    assert_eq!(result_action_for_held_lanes(true, false), Some(ResultRetryMode::DifferentArrange));
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
    session.audio_clock = bmz_audio::clock::AudioClock::with_position(48_000, 0, 0, frame, true);

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
    session.audio_clock = bmz_audio::clock::AudioClock::with_position(48_000, 0, 0, frame, true);

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
    session.audio_clock = bmz_audio::clock::AudioClock::with_position(48_000, 0, 0, frame, true);
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
    let slots = crate::input::gamepad::GamepadSlotMap::from_device_ids([Some(DeviceId(11)), None]);
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
    let synced = merge_changed_select_play_options_from_profile(current, &before, &profile.play);

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
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::RRandom), ArrangeOption::RRandom);
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::SRandom), ArrangeOption::SRandom);
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::Spiral), ArrangeOption::Spiral);
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::HRandom), ArrangeOption::HRandom);
    assert_eq!(
        arrange_option_from_profile(RandomOptionConfig::AllScratch),
        ArrangeOption::AllScratch
    );
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::RandomEx), ArrangeOption::RandomEx);
    assert_eq!(
        arrange_option_from_profile(RandomOptionConfig::SRandomEx),
        ArrangeOption::SRandomEx
    );
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::FRandom), ArrangeOption::FRandom);
    assert_eq!(arrange_option_from_profile(RandomOptionConfig::MFRandom), ArrangeOption::MFRandom);
    assert!(matches!(random_config_from_arrange(ArrangeOption::Normal), RandomOptionConfig::Off));
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
        (select_preview_fade_factor(SelectPreviewFade::FadingIn { started_at }, half) - 0.5).abs()
            < 0.001
    );
    assert_eq!(select_preview_fade_factor(SelectPreviewFade::FadingIn { started_at }, done), 1.0);
    assert!(
        (select_preview_fade_factor(SelectPreviewFade::FadingOut { started_at }, half) - 0.5).abs()
            < 0.001
    );
    assert_eq!(select_preview_fade_factor(SelectPreviewFade::FadingOut { started_at }, done), 0.0);
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
                        crate::storage::library_db::ChartSpeedChange { speed: 100.0, time_ms: 0 },
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
    chart.ln_counts = crate::ln_policy::ChartLnCounts { defined_cn_pairs: 2, ..Default::default() };
    let rows = vec![SelectItem::Chart(row)];
    let profile = ProfileConfig::new_default("default", "Default", 0);

    let snapshot = select_snapshot_rows(&rows, 0, 1, &profile, None, &HashMap::new());

    assert_eq!(snapshot[0].total_notes, 102);
    assert_eq!(snapshot[0].chart_total_gauge, bmz_gameplay::gauge::default_gauge_total(102) as f32);
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
    let rows: Vec<SelectItem> = (0..4).map(|i| SelectItem::Chart(select_chart_row(i))).collect();

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
    let rows: Vec<SelectItem> = (0..30).map(|i| SelectItem::Chart(select_chart_row(i))).collect();

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
    assert_eq!(cycle_hs_fix_option_with_direction(HsFixOption::StartBpm, 1), HsFixOption::MaxBpm);
    assert_eq!(cycle_hs_fix_option_with_direction(HsFixOption::MaxBpm, 1), HsFixOption::MainBpm);
    assert_eq!(cycle_hs_fix_option_with_direction(HsFixOption::MainBpm, 1), HsFixOption::MinBpm);
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
        play_skin_key_mode_for_options(KeyMode::K7, DoubleOption::Off, SessionMode::GhostBattle,),
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
    assert_eq!(resolve_non_empty_mode_filter(&items, SelectModeFilter::K7), SelectModeFilter::K7);
}

#[test]
fn resolve_mode_filter_advances_when_all_charts_mismatch() {
    // 5K しか無いフォルダで 7K フィルターを掛けると全消えになるため、
    // beatoraja 同様に前方向 (K7 -> K14 -> K9 -> K5) へ送って K5 で止まる。
    let items = vec![chart_row_with_mode(1, "5K"), chart_row_with_mode(2, "5K")];
    assert_eq!(resolve_non_empty_mode_filter(&items, SelectModeFilter::K7), SelectModeFilter::K5);
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
    assert_eq!(resolve_non_empty_mode_filter(&items, SelectModeFilter::K7), SelectModeFilter::K7);
}

#[test]
fn resolve_mode_filter_keeps_all_filter() {
    let items = vec![chart_row_with_mode(1, "5K")];
    assert_eq!(resolve_non_empty_mode_filter(&items, SelectModeFilter::All), SelectModeFilter::All);
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
