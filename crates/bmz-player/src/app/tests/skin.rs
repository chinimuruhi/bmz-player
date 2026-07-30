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
