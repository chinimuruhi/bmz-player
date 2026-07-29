use super::*;

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
