use super::*;

#[test]
fn wmii_result_decodes_with_virtual_io_and_graph_default() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/result/result.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("Expand Panel".to_string(), "ON - GRAPH DEFAULT".to_string())]);
    let runtime_state = LuaLoadRuntimeState {
        number_values: BTreeMap::new(),
        text_values: BTreeMap::new(),
        option_values: BTreeMap::from([(51, true), (160, true)]),
        ..LuaLoadRuntimeState::default()
    };
    let loaded = load_skin_document_uncached(
        &skin_path,
        SkinKind::Result,
        &options,
        &BTreeMap::new(),
        &runtime_state,
    )
    .expect("unmodified WMII result should decode through the BMZ loader");

    assert_eq!(loaded.document.result_panel_default, Some(2));
    assert_eq!(
        loaded
            .document
            .image
            .iter()
            .find(|image| image.id == "BtnGraphData")
            .and_then(|image| image.act),
        Some(bmz_render::skin::SKIN_EVENT_RESULT_PANEL_GRAPH)
    );
    assert_eq!(
        loaded
            .document
            .image
            .iter()
            .find(|image| image.id == "BtnIrData")
            .and_then(|image| image.act),
        Some(bmz_render::skin::SKIN_EVENT_RESULT_PANEL_IR)
    );
    let favorite = loaded
        .document
        .image
        .iter()
        .find(|image| image.id == "favorite")
        .expect("WMII result favorite button should decode");
    assert_eq!(favorite.ref_id, 90);
    assert_eq!(favorite.act, Some(90));
    assert_eq!(favorite.divy, 3);
    assert!(loaded.document.destination.iter().any(|entry| matches!(
        entry,
        DestinationListEntry::Single(destination)
            if destination.draw.contains("result_panel(1)")
    )));
    assert!(loaded.document.destination.iter().any(|entry| matches!(
        entry,
        DestinationListEntry::Single(destination)
            if destination.draw.contains("result_panel(2)")
    )));
    let destinations = loaded
        .document
        .destination
        .iter()
        .filter_map(|entry| match entry {
            DestinationListEntry::Single(destination) => Some(destination),
            DestinationListEntry::Conditional { .. } => None,
        })
        .collect::<Vec<_>>();
    assert!(destinations.iter().any(|destination| destination.id == "randomButton1p"));
    let random_key = destinations
        .iter()
        .find(|destination| destination.id == "randomKeySet1P_1")
        .expect("7K Result should retain the RANDOM lane placement destinations");
    assert!(random_key.draw.contains("event_index(42)"));
    let rank_aaa = destinations
        .iter()
        .find(|destination| destination.id == "rankBig_AAA" && destination.loop_time == Some(100))
        .expect("rankBig_AAA should survive malformed op repair");
    assert_eq!(rank_aaa.op, [300, 920]);
    assert_eq!(rank_aaa.loop_time, Some(100));
    assert_eq!(rank_aaa.filter, 1);
    assert_eq!(rank_aaa.dst.len(), 2);
    for (id, rank) in [("AAA_BG", 300), ("AA_BG", 301), ("A_BG", 302)] {
        let backgrounds = destinations
            .iter()
            .filter(|destination| {
                destination.id == id && matches!(destination.loop_time, Some(500 | 600 | 700))
            })
            .collect::<Vec<_>>();
        assert_eq!(backgrounds.len(), 3, "expected three {id} animations");
        assert!(backgrounds.iter().all(|destination| destination.op == [90, rank]));
    }
    let clear_backgrounds = destinations
        .iter()
        .filter(|destination| {
            destination.id == "clearBG" && matches!(destination.loop_time, Some(500 | 600 | 700))
        })
        .collect::<Vec<_>>();
    assert_eq!(clear_backgrounds.len(), 3);
    assert!(clear_backgrounds.iter().all(|destination| destination.op == [90]));
    let expanded_timing_values = destinations
        .iter()
        .filter(|destination| {
            matches!(
                destination.id.as_str(),
                "timingAvg"
                    | "timingAvgAdot"
                    | "timingDotMS"
                    | "durationAvg"
                    | "durationAvgAdot"
                    | "stddav"
                    | "stddaAdot"
            ) && destination.dst.first().is_some_and(|entry| {
                matches!(
                    entry,
                    bmz_render::skin::SkinDstEntry::Frame(frame)
                        if frame.x.is_some_and(|x| x >= 1_000)
                )
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(expanded_timing_values.len(), 12);
    assert!(
        expanded_timing_values.iter().all(|destination| {
            destination.draw.contains("result_panel(2)")
                && !destination.draw.contains("result_panel(0)")
                && !destination.draw.contains("result_panel(1)")
        }),
        "expanded timing values must stay hidden on the IR panel: {:?}",
        expanded_timing_values
            .iter()
            .map(|destination| (destination.id.as_str(), destination.draw.as_str()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        loaded.dependencies.virtual_io_files.get("config_sys.json"),
        Some(&Some("{\"playername\":\"bmz\"}".to_string()))
    );
    assert!(loaded.dependencies.virtual_io_files.contains_key("player/bmz/config_player.json"));
}

#[test]
fn wmii_course_result_uses_native_stage_titles_and_result_data() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/result/courseResult.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let runtime_state = LuaLoadRuntimeState {
        text_values: BTreeMap::from([
            (150, "Stage One".to_string()),
            (151, "Stage Two".to_string()),
            (152, "Stage Three".to_string()),
            (153, "Stage Four".to_string()),
        ]),
        option_values: BTreeMap::from([(160, true), (290, true)]),
        virtual_io_files: BTreeMap::from([(
            "skin/WMII_FHD/result/courseData.json".to_string(),
            serde_json::json!({
                "songs": [
                    { "stage": 1, "score": 1000, "gauge": 80, "miss": 10, "rate": 0.5 },
                    { "stage": 2, "score": 2000, "gauge": 81, "miss": 11, "rate": 0.6 },
                    { "stage": 3, "score": 3000, "gauge": 82, "miss": 12, "rate": 0.7 },
                    { "stage": 4, "score": 3456, "gauge": 88, "miss": 13, "rate": 0.75 }
                ]
            })
            .to_string(),
        )]),
        ..LuaLoadRuntimeState::default()
    };
    let loaded = load_skin_document_uncached(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &runtime_state,
    )
    .expect("unmodified WMII course result should decode with native stage data");

    for (id, expected) in [("stage_gauge4", "88"), ("stage_score4", "3456"), ("stage_miss4", "13")]
    {
        let value = loaded
            .document
            .value
            .iter()
            .find(|value| value.id == id)
            .unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(value.value_expr, expected, "unexpected {id} expression");
    }
    let graph = loaded
        .document
        .graph
        .iter()
        .find(|graph| graph.id == "stage_scoreGraph4")
        .expect("missing stage 4 score-rate graph");
    assert_eq!(graph.value_expr, "0.75");
    assert!(loaded.document.destination.iter().any(|entry| matches!(
        entry,
        DestinationListEntry::Single(destination) if destination.id == "courseTitle4"
    )));
    assert_eq!(
        loaded
            .document
            .value
            .iter()
            .find(|value| value.id == "courseClearRate")
            .map(|value| value.value_expr.as_str()),
        Some(bmz_render::skin::SKIN_EXPR_COURSE_CLEAR_RATE)
    );
    assert_eq!(
        loaded.dependencies.virtual_io_files.get("skin/WMII_FHD/result/courseData.json"),
        runtime_state
            .virtual_io_files
            .get("skin/WMII_FHD/result/courseData.json")
            .cloned()
            .map(Some)
            .as_ref()
    );
}

#[test]
fn wmii_result_renders_bmz_player_version_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/result/result.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin_with_options(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::from([("Display Version".to_string(), "ON".to_string())]),
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(
        decoded.document.text.iter().any(|text| text.id == "version" && text.ref_id == 1010),
        "WMII version text should retain STRING_VERSION ref 1010"
    );
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let items = decoded.document.static_render_items(
        &sources,
        &SkinDrawState { elapsed_ms: 2_000, ..SkinDrawState::default() },
        &SkinTextState::default(),
    );

    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Text { text, .. }
            if text == &format!("bmz-player {}", env!("CARGO_PKG_VERSION"))
    )));
}

#[test]
fn wmii_result_uses_runtime_combo_break_for_clear_animation() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/result/result.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("Expand Panel".to_string(), "ON - GRAPH DEFAULT".to_string())]);
    let load = |combo_break: i32| {
        load_skin_document_uncached(
            &skin_path,
            SkinKind::Result,
            &options,
            &BTreeMap::new(),
            &LuaLoadRuntimeState {
                number_values: BTreeMap::from([(425, combo_break)]),
                text_values: BTreeMap::new(),
                option_values: BTreeMap::from([(51, true), (160, true)]),
                ..LuaLoadRuntimeState::default()
            },
        )
        .expect("unmodified WMII result should decode")
    };
    let destination_ids = |loaded: &LoadedSkinDocumentWithDependencies| {
        loaded
            .document
            .destination
            .iter()
            .filter_map(|entry| match entry {
                DestinationListEntry::Single(destination) => Some(destination.id.clone()),
                DestinationListEntry::Conditional { .. } => None,
            })
            .collect::<Vec<_>>()
    };

    let full_combo = load(0);
    let full_combo_ids = destination_ids(&full_combo);
    assert!(full_combo_ids.iter().any(|id| id == "result_FULL"));
    assert!(full_combo_ids.iter().any(|id| id == "result_COMBO"));
    assert!(!full_combo_ids.iter().any(|id| id == "result_CLEAR"));

    let normal_clear = load(1);
    let normal_clear_ids = destination_ids(&normal_clear);
    assert!(normal_clear_ids.iter().any(|id| id == "result_CLEAR"));
    assert!(!normal_clear_ids.iter().any(|id| id == "result_FULL"));
    assert!(!normal_clear_ids.iter().any(|id| id == "result_COMBO"));
}

#[test]
fn wmii_fhd_lr2skin_decodes_play_document_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    assert_eq!(decoded.document.name, "WMII FHD play AC");
    assert!(decoded.document.w >= 1920);
    assert!(decoded.document.source.len() >= 10);
    assert!(decoded.document.image.len() >= 100);
    assert!(
        decoded.document.source.iter().any(|source| source.id == "110")
            && decoded.document.source.iter().any(|source| source.id == "111"),
        "expected LR2 black/white reference sources"
    );
    let note = decoded.document.note.as_ref().expect("lr2 play skin should define notes");
    assert!(!note.group.is_empty());
    assert!(decoded.document.gauge.is_some());
    assert!(decoded.document.bga.is_some());
    assert!(
        decoded.sources.len() >= 10,
        "expected WMII sources to decode, got {}; source paths: {:?}; decoded: {:?}",
        decoded.sources.len(),
        decoded.document.source.iter().map(|source| source.path.as_str()).collect::<Vec<_>>(),
        decoded.sources.iter().map(|source| source.path.clone()).collect::<Vec<_>>()
    );
    let black = decoded.sources.iter().find(|source| source.source_id == "110").unwrap();
    let white = decoded.sources.iter().find(|source| source.source_id == "111").unwrap();
    assert_eq!(black.asset.as_ref().unwrap().pixels, vec![0, 0, 0, 255]);
    assert_eq!(white.asset.as_ref().unwrap().pixels, vec![255, 255, 255, 255]);
}

#[test]
fn wmii_fhd_lr2skin_can_be_applied_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }
    let mut renderer = Renderer::default();

    apply_beatoraja_json_skin(&mut renderer, &skin_path).unwrap();
}

#[test]
fn wmii_fhd_lr2skin_produces_static_play_items_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([
        ("GRAPH SIDE".to_string(), "LEFT".to_string()),
        ("Score Graph".to_string(), "On".to_string()),
    ]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        ready_timer_ms: Some(2_000),
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );
    assert!(!items.is_empty());
}

#[test]
fn wmii_fhd_lr2skin_renders_play_fadeout_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let black_texture = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "110")
        .map(|source| source.texture)
        .expect("WMII black reference source should decode");
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let state = bmz_render::skin::SkinDrawState { fadeout_ms: Some(500), ..Default::default() };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );

    assert!(
        items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                if *texture == black_texture
                    && rect.width >= 0.99
                    && rect.height >= 0.99
                    && tint.a > 0.99
        )),
        "expected WMII timer=2 fadeout to draw an opaque fullscreen black image"
    );
}

#[test]
fn wmii_fhd_lr2skin_decodes_auto_judge_button_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("Displayjudge".to_string(), "ON".to_string())]);
    let decoded = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &options,
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        None,
    )
    .unwrap()
    .document;
    let candidates = decoded
        .image
        .iter()
        .filter(|image| image.divx == 1 && image.divy >= 2 && image.h > 0)
        .map(|image| {
            format!(
                "src={} x={} y={} w={} h={} divy={} ref={} act={:?}",
                image.src, image.x, image.y, image.w, image.h, image.divy, image.ref_id, image.act
            )
        })
        .collect::<Vec<_>>();
    let auto_judge = decoded
        .image
        .iter()
        .find(|image| image.act == Some(75) && image.divx == 1 && image.divy >= 2)
        .unwrap_or_else(|| {
            panic!("WMII auto judge button should decode; candidates: {}", candidates.join(" | "))
        });

    assert_eq!(auto_judge.ref_id, 0);
    assert_eq!(auto_judge.click, 2);
    assert_eq!(auto_judge.clickable, Some(false));
    assert!(
        auto_judge.h > 0,
        "WMII auto judge button should keep a positive source height: {auto_judge:?}"
    );
}

#[test]
fn wmii_fhd_lr2skin_renders_ac_bga_frame_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let frame_image = decoded
        .document
        .image
        .iter()
        .find(|image| image.src == "2" && image.x == 1016 && image.y == 1276 && image.w == 389)
        .expect("WMII AC frame image should decode");
    let mut destinations = Vec::new();
    for entry in &decoded.document.destination {
        match entry {
            bmz_render::skin::DestinationListEntry::Single(destination) => {
                destinations.push(destination);
            }
            bmz_render::skin::DestinationListEntry::Conditional {
                destinations: nested, ..
            } => {
                destinations.extend(nested.iter());
            }
        }
    }
    let frame_destination = destinations
        .into_iter()
        .find(|destination| {
            destination.id == frame_image.id
                && destination.op.contains(&33)
                && destination.op.contains(&41)
                && destination.op.contains(&30)
        })
        .expect("WMII AC frame destination should decode");
    assert!(
        frame_destination.dst.len() >= 2,
        "expected WMII AC frame destination keyframes, got {:?}",
        frame_destination.dst
    );
    let frame_texture = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "2")
        .expect("WMII AC frame source should load")
        .texture;
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        ready_timer_ms: Some(2_000),
        has_bga: true,
        bga_enabled: true,
        autoplay: true,
        skin_loaded: true,
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );
    assert!(
        items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                if *texture == frame_texture
                    && (rect.width - 389.0 / 1920.0).abs() < 0.001
                    && tint.a > 0.5
        )),
        "expected WMII AC BGA frame item from source 2; got {items:?}"
    );
}

#[test]
fn wmii_fhd_lr2skin_uses_full_note_lane_region_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let area = decoded
        .document
        .note_lane_area(
            bmz_core::lane::Lane::Scratch,
            bmz_core::lane::KeyMode::K7,
            &decoded.document.enabled_options(),
        )
        .expect("WMII scratch lane area should decode");

    assert!((area.x - 75.0 / 1920.0).abs() < 0.001);
    assert!(
        area.height > 0.65,
        "expected LR2 note.dst to define the full scroll lane height, got {area:?}"
    );
}

#[test]
fn wmii_fhd_lr2skin_maps_note_sources_by_lr2_lane_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let note = decoded.document.note.as_ref().expect("WMII notes should decode");
    let images = decoded.document.image_map();
    let scratch =
        images.get(note.note[7].as_str()).expect("WMII scratch note image should resolve");
    let key1 = images.get(note.note[0].as_str()).expect("WMII key1 note image should resolve");
    let key2 = images.get(note.note[1].as_str()).expect("WMII key2 note image should resolve");

    assert_eq!((scratch.x, scratch.w), (94, 90));
    assert_eq!((key1.x, key1.w), (187, 52));
    assert_eq!((key2.x, key2.w), (241, 40));
}

#[test]
fn wmii_fhd_lr2skin_inserts_notes_marker_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    assert!(
        decoded
            .document
            .all_destinations(&decoded.document.enabled_options())
            .iter()
            .any(|destination| destination.id == "notes"),
        "LR2 play skins should insert the notes marker at the first DST_NOTE command"
    );
}

#[test]
fn wmii_fhd_lr2skin_renders_groove_gauge_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let gauge_texture = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "19")
        .expect("WMII gauge source should load")
        .texture;
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    for gauge_type in [
        bmz_core::clear::GaugeType::AssistEasy,
        bmz_core::clear::GaugeType::Normal,
        bmz_core::clear::GaugeType::Hard,
    ] {
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            gauge: 80.0,
            gauge_max: 100.0,
            gauge_border: 80.0,
            gauge_type: gauge_type as i32,
            ..Default::default()
        };

        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );
        assert!(
            items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                    if *texture == gauge_texture
                        && (rect.x - 54.0 / 1920.0).abs() < 0.001
                        && rect.width > 0.004
                        && tint.a > 0.5
            )),
            "expected WMII groove gauge item from source 19 for {gauge_type:?}; got {items:?}"
        );
    }
}

#[test]
fn wmii_fhd_lr2skin_renders_lift_cover_when_lifted() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    assert!(
        decoded.document.hidden_cover.iter().any(|cover| cover.id.contains("liftcover")
            && cover.disappear_line == 357
            && !cover.is_disappear_line_link_lift),
        "expected LR2 SRC_LIFT to decode as a liftcover hiddenCover"
    );
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let lift_cover = decoded
        .document
        .hidden_cover
        .iter()
        .find(|cover| cover.id.contains("liftcover"))
        .expect("WMII lift cover hiddenCover should decode");
    let lift_texture = decoded
        .sources
        .iter()
        .find(|source| source.source_id == lift_cover.src)
        .map(|source| source.texture)
        .expect("WMII lift source should decode");
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        offset_lift_px: 0,
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );

    assert!(
        !items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { texture, tint, .. }
                if *texture == lift_texture && tint.a > 0.5
        )),
        "expected WMII LIFT cover to stay hidden while lift offset is zero"
    );

    let lifted_items = decoded.document.static_render_items(
        &sources,
        &bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            offset_lift_px: 200,
            lift: 200.0 / 1080.0,
            lift_enabled: true,
            ..Default::default()
        },
        &bmz_render::skin::SkinTextState::default(),
    );
    assert!(
        lifted_items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                if *texture == lift_texture && rect.height < 0.25 && tint.a > 0.5
        )),
        "expected WMII LIFT cover to render clipped once lift offset is active; got {lifted_items:?}"
    );
}

#[test]
fn wmii_fhd_luaskin_renders_lift_cover_when_lifted() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/play7wide.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let lift_cover = decoded
        .document
        .lift_cover
        .iter()
        .find(|cover| cover.id.eq_ignore_ascii_case("lift"))
        .unwrap_or_else(|| {
            panic!(
                "WMII Lua lift cover should decode; got {:?}",
                decoded
                    .document
                    .lift_cover
                    .iter()
                    .map(|cover| (&cover.id, &cover.src))
                    .collect::<Vec<_>>()
            )
        });
    let lift_texture = decoded
        .sources
        .iter()
        .find(|source| source.source_id == lift_cover.src)
        .map(|source| source.texture)
        .expect("WMII Lua lift source should decode");
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();

    let lifted_items = decoded.document.static_render_items(
        &sources,
        &bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            offset_lift_px: 200,
            lift: 200.0 / 1080.0,
            lift_enabled: true,
            ..Default::default()
        },
        &bmz_render::skin::SkinTextState::default(),
    );

    assert!(
        lifted_items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { texture, tint, .. }
                if *texture == lift_texture && tint.a > 0.5
        )),
        "expected WMII Lua LIFT cover to render once lift offset is active; got {lifted_items:?}"
    );
}

#[test]
fn wmii_fhd_lr2skin_moves_judge_line_with_lift_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let judge_line_ids = decoded
        .document
        .image
        .iter()
        .filter(|image| image.src == "1" && image.x == 1231 && image.y == 0)
        .map(|image| image.id.as_str())
        .collect::<Vec<_>>();
    assert!(!judge_line_ids.is_empty(), "expected WMII judge line source image");

    assert!(
        decoded
            .document
            .all_destinations(&decoded.document.enabled_options())
            .iter()
            .any(|destination| judge_line_ids.contains(&destination.id.as_str())
                && destination.offsets.contains(&3)),
        "expected WMII DST_JUDGELINE to include beatoraja default OFFSET_LIFT"
    );
}

#[test]
fn wmii_fhd_lr2skin_renders_score_graph_bars_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin_with_options(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::from([("Score Graph".to_string(), "On".to_string())]),
        &BTreeMap::new(),
    )
    .unwrap();
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        total_notes: 1_000,
        past_notes: 500,
        ex_score: 1_000,
        best_ex_score: Some(1_300),
        projected_best_ex_score: Some(650),
        target_ex_score: Some(1_500),
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );

    assert!(
        items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                if (rect.x - 546.0 / 1920.0).abs() < 0.01
                    && (rect.width - 277.0 / 1920.0).abs() < 0.01
                    && (rect.height - 798.0 / 1080.0).abs() < 0.01
                    && tint.a > 0.5
        )),
        "expected WMII score graph frame/background to render on the left side"
    );
    assert!(
        items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { rect, .. }
                if (rect.x - 670.0 / 1920.0).abs() < 0.01
                    && rect.width > 0.0
                    && rect.height > 0.05
        )),
        "expected WMII score graph bars to render in the graph area"
    );
}

#[test]
fn wmii_fhd_lr2skin_hides_score_graph_and_extends_bga_on_autoplay_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([
        ("BGA Size".to_string(), "Extend".to_string()),
        ("Score Graph".to_string(), "On".to_string()),
    ]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        ready_timer_ms: Some(2_000),
        has_bga: true,
        bga_enabled: true,
        autoplay: true,
        skin_loaded: true,
        total_notes: 1_000,
        past_notes: 500,
        ex_score: 1_000,
        best_ex_score: Some(1_300),
        target_ex_score: Some(1_500),
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );

    assert!(
        items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                if (rect.x - 726.0 / 1920.0).abs() < 0.01
                    && (rect.width - 1027.0 / 1920.0).abs() < 0.01
                    && tint.a > 0.5
        )),
        "expected WMII autoplay extended BGA frame to render; got {items:?}"
    );
    assert!(
        !items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                if (rect.x - 546.0 / 1920.0).abs() < 0.01
                    && (rect.width - 277.0 / 1920.0).abs() < 0.01
                    && (rect.height - 798.0 / 1080.0).abs() < 0.01
                    && tint.a > 0.5
        )),
        "WMII score graph frame must stay hidden during autoplay"
    );
    assert!(
        !items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                if (rect.x - 551.0 / 1920.0).abs() < 0.01
                    && (rect.width - 267.0 / 1920.0).abs() < 0.01
                    && tint.a > 0.5
        )),
        "WMII score graph target labels must stay hidden during autoplay"
    );
}

#[test]
fn wmii_fhd_lr2skin_renders_lane_cover_and_lift_numbers_when_adjusting() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let source1 = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "1")
        .expect("WMII number source should decode");
    let number_uv_y = 883.0 / source1.size.height;
    let number_uv_h = 20.0 / source1.size.height;
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        lane_cover: 0.290,
        lift: 0.222,
        total_duration_ms: 517,
        offset_lift_px: (0.222_f32 * 723.0).round() as i32,
        offset_lanecover_px: -(723.0_f32 * 0.290).round() as i32,
        lane_cover_changing: true,
        lanecover_enabled: true,
        lift_enabled: true,
        now_bpm: 88.0,
        main_bpm: 88.0,
        min_bpm: 38.0,
        max_bpm: 156.0,
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );

    let number_digits = items
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, uv, .. }
                    if *texture == source1.texture
                        && (uv.y - number_uv_y).abs() < 0.001
                        && (uv.height - number_uv_h).abs() < 0.001
            )
        })
        .collect::<Vec<_>>();
    let white_digits = number_digits
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { tint, .. }
                    if tint.r > 0.95 && tint.g > 0.95 && tint.b > 0.95 && tint.a > 0.5
            )
        })
        .count();
    let green_digits = number_digits
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { tint, .. }
                    if tint.r < 0.4 && tint.g > 0.75 && tint.b < 0.5 && tint.a > 0.5
            )
        })
        .count();
    let green_bpm_cover_digits = number_digits
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { tint, rect, .. }
                    if tint.r < 0.4
                        && tint.g > 0.75
                        && tint.b < 0.5
                        && tint.a > 0.5
                        && (rect.y * 1080.0 - 165.0).abs() < 2.0
            )
        })
        .count();
    let green_bpm_no_cover_digits = number_digits
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { tint, rect, .. }
                    if tint.r < 0.4
                        && tint.g > 0.75
                        && tint.b < 0.5
                        && tint.a > 0.5
                        && (rect.y * 1080.0 - 203.0).abs() < 2.0
            )
        })
        .count();
    let green_digit_ys = number_digits
        .iter()
        .filter_map(|item| {
            if let bmz_render::skin::SkinRenderItem::Image { tint, rect, .. } = item
                && tint.r < 0.4
                && tint.g > 0.75
                && tint.b < 0.5
                && tint.a > 0.5
            {
                Some((rect.y * 1080.0).round() as i32)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    assert!(
        white_digits >= 6,
        "expected WMII SUDDEN and LIFT white number digits to render; got {white_digits}"
    );
    assert!(
        green_digits >= 6,
        "expected WMII upper and lower green number digits to render; got {green_digits}"
    );
    assert!(
        green_bpm_cover_digits >= 9,
        "expected WMII BPM green digits to use lanecover-on layout; got {green_bpm_cover_digits}; green ys {green_digit_ys:?}"
    );
    assert_eq!(
        green_bpm_no_cover_digits, 0,
        "expected WMII BPM green digits not to use lanecover-off layout when op271 is active"
    );

    let zero_lift_state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        lane_cover: 0.290,
        lift: 0.0,
        total_duration_ms: 517,
        offset_lift_px: 0,
        offset_lanecover_px: -(723.0_f32 * 0.290).round() as i32,
        lane_cover_changing: true,
        lanecover_enabled: true,
        lift_enabled: true,
        now_bpm: 88.0,
        main_bpm: 88.0,
        min_bpm: 38.0,
        max_bpm: 156.0,
        ..Default::default()
    };
    let zero_lift_items = decoded.document.static_render_items(
        &sources,
        &zero_lift_state,
        &bmz_render::skin::SkinTextState::default(),
    );
    let zero_lift_digits = zero_lift_items
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, uv, rect, .. }
                    if *texture == source1.texture
                        && (uv.y - number_uv_y).abs() < 0.001
                        && (uv.height - number_uv_h).abs() < 0.001
                        && (rect.y * 1080.0 - 724.0).abs() < 2.0
            )
        })
        .collect::<Vec<_>>();
    let zero_lift_white_digits = zero_lift_digits
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { tint, .. }
                    if tint.r > 0.95 && tint.g > 0.95 && tint.b > 0.95 && tint.a > 0.5
            )
        })
        .count();
    let zero_lift_green_digits = zero_lift_digits
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { tint, .. }
                    if tint.r < 0.4 && tint.g > 0.75 && tint.b < 0.5 && tint.a > 0.5
            )
        })
        .count();
    assert!(
        zero_lift_white_digits > 0,
        "expected WMII LIFT white digits to render even when LIFT is zero"
    );
    assert!(
        zero_lift_green_digits > 0,
        "expected WMII LIFT green digits to render even when LIFT is zero"
    );
}

#[test]
fn wmii_fhd_lr2skin_renders_runtime_difficulty_badge_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        difficulty: 4,
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );

    assert!(
        items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                if (rect.x - 617.0 / 1920.0).abs() < 0.01
                    && (rect.width - 187.0 / 1920.0).abs() < 0.01
                    && tint.a > 0.1
        )),
        "expected WMII ANOTHER difficulty badge to render for difficulty op154"
    );
}

#[test]
fn wmii_fhd_lr2skin_renders_judge_and_combo_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("Displayjudge".to_string(), "ON".to_string())]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();
    let judge_texture = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "13")
        .expect("WMII judge source should load")
        .texture;
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let mut judge_ms = [None; bmz_render::skin::MAX_JUDGE_REGIONS];
    judge_ms[0] = Some(100);
    let mut judge_index = [None; bmz_render::skin::MAX_JUDGE_REGIONS];
    judge_index[0] = Some(0);
    let mut judge_combo = [0; bmz_render::skin::MAX_JUDGE_REGIONS];
    judge_combo[0] = 123;
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        judge_ms,
        judge_index,
        judge_combo,
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );
    let judge_items = items
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                    if *texture == judge_texture
                        && rect.height > 0.01
                        && tint.a > 0.5
            )
        })
        .count();

    assert!(
        judge_items >= 2,
        "expected WMII judge text and combo digits from source 13; got {items:?}"
    );
    assert!(
        items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { texture, rect, uv, tint, .. }
                if *texture == judge_texture
                    && rect.height > 0.05
                    && uv.y < 0.001
                    && tint.a > 0.5
        )),
        "expected PGREAT judge image to use the top WMII judge source row; got {items:?}"
    );

    for (judge_index, label) in ["PGREAT", "GREAT", "GOOD", "BAD", "POOR"].iter().enumerate() {
        let mut judge_ms = [None; bmz_render::skin::MAX_JUDGE_REGIONS];
        judge_ms[0] = Some(100);
        let mut judge_indices = [None; bmz_render::skin::MAX_JUDGE_REGIONS];
        judge_indices[0] = Some(judge_index);
        let mut judge_combo = [0; bmz_render::skin::MAX_JUDGE_REGIONS];
        judge_combo[0] = 123;
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            judge_ms,
            judge_index: judge_indices,
            judge_combo,
            ..Default::default()
        };
        let items = decoded.document.static_render_items(
            &sources,
            &state,
            &bmz_render::skin::SkinTextState::default(),
        );
        assert!(
            items.iter().any(|item| matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                    if *texture == judge_texture
                        && rect.height > 0.05
                        && tint.a > 0.5
            )),
            "expected WMII {label} judge image to render; got {items:?}"
        );
    }
}

#[test]
fn wmii_fhd_lr2skin_dp_renders_judge_detail_panel_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC_DP.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([
        ("Displayjudge".to_string(), "ON".to_string()),
        ("GRAPH SIDE".to_string(), "RIGHT".to_string()),
        ("Score Graph".to_string(), "On".to_string()),
    ]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();

    assert!(
        decoded.document.enabled_options().contains(&983),
        "expected WMII DP judge detail panel op983 to stay enabled"
    );

    let frame_texture = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "1")
        .expect("WMII frame source should load")
        .texture;
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        key_mode: bmz_core::lane::KeyMode::K14,
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );

    assert!(
        items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                if *texture == frame_texture
                    && (rect.x - 71.0 / 1920.0).abs() < 0.01
                    && (rect.width - 247.0 / 1920.0).abs() < 0.02
                    && rect.height > 0.1
                    && tint.a > 0.1
        )),
        "expected WMII DP judge detail panel body to render; got {items:?}"
    );
}

#[test]
fn wmii_fhd_lr2skin_renders_fast_slow_during_replay_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("Display FAST/SLOW".to_string(), "ON-A".to_string())]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();
    let sources = decoded.sources.iter().map(|source| SkinDocumentTexture {
        source_id: source.source_id.clone(),
        texture: source.texture,
        source_size: SkinImageSize { width: source.size.width, height: source.size.height },
    });
    let skin = SkinContext::from_manifest_and_document(
        SkinManifest::default(),
        decoded.document.clone(),
        sources,
    );
    let replay_snapshot = bmz_render::snapshot::RenderSnapshot {
        time: TimeUs(100_000),
        play_elapsed_time: TimeUs(100_000),
        replay_playback: true,
        key_mode: bmz_core::lane::KeyMode::K7,
        recent_judgements: vec![bmz_render::snapshot::DisplayJudgement {
            lane: bmz_core::lane::Lane::Key1,
            judge: bmz_core::judge::Judge::PGreat,
            side: Some(bmz_core::judge::TimingSide::Fast),
            text: "PGREAT FAST".to_string(),
            combo: 1,
            delta_us: -2_000,
            time: TimeUs(0),
            is_miss: false,
            timing_ms_suppressed: false,
        }],
        ..Default::default()
    };
    let has_wmii_fast_slow_image = |plan: &DrawPlan| {
        plan.commands.iter().any(|command| {
            matches!(
                command,
                DrawCommand::Image { rect, tint, .. }
                    if ((rect.x - 292.0 / 1920.0).abs() < 0.01
                        || (rect.x - 246.0 / 1920.0).abs() < 0.01)
                        && (rect.y - 502.0 / 1080.0).abs() < 0.01
                        && (rect.width - 82.0 / 1920.0).abs() < 0.01
                        && tint.a > 0.5
            )
        })
    };

    let mut snapshot = replay_snapshot.clone();
    crate::screens::play_snapshot::apply_fast_slow_display_filter(
        &mut snapshot,
        0,
        crate::config::profile_config::FastSlowDisplayScope::ThresholdMs,
    );

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(snapshot),
        &skin,
        &mut DynamicTimerRuntime::default(),
    );

    assert!(
        has_wmii_fast_slow_image(&plan),
        "expected WMII replay PGREAT FAST/SLOW image to render; got {:?}",
        plan.commands
    );

    let mut auto_snapshot = replay_snapshot;
    crate::screens::play_snapshot::apply_fast_slow_display_filter(
        &mut auto_snapshot,
        0,
        crate::config::profile_config::FastSlowDisplayScope::Auto,
    );
    let auto_plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(auto_snapshot),
        &skin,
        &mut DynamicTimerRuntime::default(),
    );

    assert!(
        !has_wmii_fast_slow_image(&auto_plan),
        "expected WMII Auto scope to hide replay PGREAT FAST/SLOW; got {:?}",
        auto_plan.commands
    );
}

#[test]
fn wmii_fhd_lr2skin_applies_play_timing_headers_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

    assert_eq!(decoded.document.loadstart, 0);
    assert_eq!(decoded.document.loadend, 3000);
    assert_eq!(decoded.document.playstart, 1500);
    assert_eq!(decoded.document.fadeout, 500);
    assert_eq!(decoded.document.close, 2500);
}

#[test]
fn wmii_fhd_lr2skin_uses_lr2_bitmap_fonts_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

    assert!(
        decoded.document.font.iter().any(|font| {
            font.id.starts_with("lr2font-")
                && font.path.replace('\\', "/").ends_with("../font/songTitle/font.fnt")
        }),
        "expected LR2FONT font.lr2font to resolve to bundled font.fnt; got {:?}",
        decoded.document.font
    );
    assert!(
        decoded.document.text.iter().any(|text| {
            text.ref_id == 12 && text.font.starts_with("play:lr2font-") && text.size == 0
        }),
        "expected full-title text to keep its LR2 bitmap font id; got {:?}",
        decoded.document.text
    );
    assert!(
        decoded.document.text.iter().any(|text| {
            text.ref_id == 10 && text.font.starts_with("play:lr2font-") && text.size == 0
        }),
        "expected READY title text to use LR2 bitmap font index 0; got {:?}",
        decoded.document.text
    );
    assert!(
        decoded.document.text.iter().any(|text| {
            text.ref_id == 14 && text.font.starts_with("play:lr2font-") && text.size == 0
        }),
        "expected artist text to keep its LR2 bitmap font id; got {:?}",
        decoded.document.text
    );
    assert!(
        decoded.fonts.iter().any(|font| {
            font.stored_id.starts_with("play:lr2font-")
                && matches!(font.data.as_ref(), Some(DecodedFontData::Bitmap(_)))
        }),
        "expected decoded LR2 bitmap font to be loaded"
    );
}

#[test]
fn wmii_fhd_lr2skin_uses_dst_text_size_for_lr2_bitmap_fonts_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let title_id = decoded
        .document
        .text
        .iter()
        .find(|text| text.ref_id == 12)
        .map(|text| text.id.as_str())
        .expect("WMII full-title text should exist");
    let has_frame_height = |id: &str, height: i32| {
        decoded.document.destination.iter().any(|entry| match entry {
            bmz_render::skin::DestinationListEntry::Single(destination) => {
                destination.id == id
                    && destination.dst.iter().any(|frame| match frame {
                        bmz_render::skin::SkinDstEntry::Frame(frame) => frame.h == Some(height),
                        bmz_render::skin::SkinDstEntry::Conditional { frames, .. } => {
                            frames.iter().any(|frame| frame.h == Some(height))
                        }
                    })
            }
            bmz_render::skin::DestinationListEntry::Conditional { destinations, .. } => {
                destinations.iter().any(|destination| {
                    destination.id == id
                        && destination.dst.iter().any(|frame| match frame {
                            bmz_render::skin::SkinDstEntry::Frame(frame) => frame.h == Some(height),
                            bmz_render::skin::SkinDstEntry::Conditional { frames, .. } => {
                                frames.iter().any(|frame| frame.h == Some(height))
                            }
                        })
                })
            }
        })
    };

    assert!(
        has_frame_height(title_id, 41),
        "expected WMII full-title bitmap font size to come from DST_TEXT h=41"
    );
    assert!(
        decoded.document.text.iter().any(|text| {
            text.ref_id == 14
                && text.font.starts_with("play:lr2font-")
                && has_frame_height(&text.id, 29)
        }),
        "expected WMII artist bitmap font size to come from DST_TEXT h=29"
    );
}

#[test]
fn wmii_fhd_lr2skin_uses_lr2_bitmap_font_for_table_level_when_enabled() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("Display Table Level".to_string(), "ON".to_string())]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();

    assert!(
        decoded.document.text.iter().any(|text| {
            text.ref_id == 1002 && text.font.starts_with("play:lr2font-") && text.size == 0
        }),
        "expected difficulty-table text to keep its LR2 bitmap font id; got {:?}",
        decoded.document.text
    );
}

#[test]
fn wmii_fhd_lr2skin_preserves_green_number_digit_width_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let green_numbers = decoded
        .document
        .value
        .iter()
        .filter(|value| matches!(value.ref_id, 313 | 1317 | 1321 | 1325))
        .collect::<Vec<_>>();

    assert!(!green_numbers.is_empty(), "expected WMII green-number value sprites");
    assert!(
        green_numbers.iter().all(|value| value.digit == 3),
        "LR2 keta field should remain 3 digits for WMII green numbers; got {green_numbers:?}"
    );

    assert!(
        decoded.document.value.iter().any(|value| value.ref_id == 310 && value.digit == 1),
        "expected WMII white high-speed integer digit to use LR2 keta=1"
    );
    assert!(
        decoded.document.value.iter().any(|value| value.ref_id == 311 && value.digit == 2),
        "expected WMII white high-speed decimal digits to use LR2 keta=2"
    );
}

#[test]
fn wmii_fhd_lr2skin_keeps_runtime_difficulty_option_destinations_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

    for op in 150..=155 {
        assert!(
            decoded.document.destination.iter().any(|entry| match entry {
                bmz_render::skin::DestinationListEntry::Single(destination) =>
                    destination.op.contains(&op),
                bmz_render::skin::DestinationListEntry::Conditional { destinations, .. } =>
                    destinations.iter().any(|destination| destination.op.contains(&op)),
            }),
            "expected runtime difficulty op {op} to survive LR2 #IF conversion"
        );
    }
}

#[test]
fn wmii_fhd_lr2skin_uses_relative_combo_destination_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("Displayjudge".to_string(), "ON".to_string())]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();

    assert!(
        decoded.document.judge.iter().flat_map(|judge| &judge.numbers).any(|number| {
            number.dst.iter().any(|entry| match entry {
                bmz_render::skin::SkinDstEntry::Frame(frame) => {
                    frame.x == Some(242) && frame.y == Some(0) && frame.h == Some(124)
                }
                bmz_render::skin::SkinDstEntry::Conditional { frames, .. } => {
                    frames.iter().any(|frame| {
                        frame.x == Some(242) && frame.y == Some(0) && frame.h == Some(124)
                    })
                }
            })
        }),
        "expected WMII NOWCOMBO destination to stay relative to judge image"
    );
    assert!(
        decoded
            .document
            .judge
            .iter()
            .flat_map(|judge| &judge.images)
            .any(|image| { image.offsets.contains(&3) && image.offsets.contains(&32) }),
        "expected WMII NOWJUDGE destinations to include beatoraja LR2 judge and lift offsets"
    );
    assert!(
        decoded
            .document
            .judge
            .iter()
            .flat_map(|judge| &judge.numbers)
            .any(|number| { number.offsets.contains(&3) && number.offsets.contains(&32) }),
        "expected WMII NOWCOMBO destinations to include beatoraja LR2 judge and lift offsets"
    );
}

#[test]
fn wmii_fhd_lr2skin_defaults_score_graph_to_off_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

    assert!(decoded.document.graph.iter().all(|graph| !matches!(graph.graph_type, 110..=115)));
    assert!(
        decoded
            .document
            .property
            .iter()
            .any(|property| property.name == "Score Graph" && property.def == "Off"),
        "expected beatoraja's built-in Score Graph option to default to Off"
    );
}

#[test]
fn wmii_fhd_lr2skin_2p_side_maps_single_play_notes_to_active_lanes_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("PLAY SIDE".to_string(), "2P".to_string())]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();
    let note = decoded.document.note.as_ref().expect("WMII note definition should load");

    assert!(
        note.dst.len() <= 8,
        "single-play 2P side should remap LR2 2P lanes into active lanes; got {} dst lanes",
        note.dst.len()
    );
    assert!(
        note.dst.iter().take(8).any(|entry| match entry {
            bmz_render::skin::SkinDstEntry::Frame(frame) =>
                frame.w.unwrap_or_default() > 0 && frame.h.unwrap_or_default() > 0,
            bmz_render::skin::SkinDstEntry::Conditional { frames, .. } =>
                frames.iter().any(|frame| {
                    frame.w.unwrap_or_default() > 0 && frame.h.unwrap_or_default() > 0
                }),
        }),
        "expected remapped 2P note lanes to have visible destinations"
    );
}

#[test]
fn lr2_document_cache_reuses_when_unused_option_changes() {
    let root = unique_test_dir("bmz-lr2-document-cache-option");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("play.lr2skin");
    std::fs::write(
        &skin_path,
        r#"
#INFORMATION,0,Cache Test,Author
#CUSTOMOPTION,Unused,930,Off,On
#CUSTOMOPTION,Branch,910,Off,On
#IF,911
#IMAGE,on.png
#ELSE
#IMAGE,off.png
#ENDIF
"#,
    )
    .unwrap();
    let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

    let first = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(first.document.source[0].path, "off.png");

    let unused_changed = BTreeMap::from([("Unused".to_string(), "On".to_string())]);
    let second = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &unused_changed,
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(second.cache_status, DocumentCacheStatus::Hit);
    assert_eq!(second.document.source[0].path, "off.png");
    assert!(second.document.enabled_options().contains(&931));

    let branch_changed = BTreeMap::from([("Branch".to_string(), "On".to_string())]);
    let third = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &branch_changed,
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(third.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(third.document.source[0].path, "on.png");
}

#[test]
fn lr2_document_cache_misses_when_play_side_remap_changes() {
    let root = unique_test_dir("bmz-lr2-document-cache-play-side");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("play.lr2skin");
    std::fs::write(
        &skin_path,
        r#"
#INFORMATION,0,Cache Test,Author
#CUSTOMOPTION,PLAY SIDE,900,1P,2P
#IMAGE,base.png
"#,
    )
    .unwrap();
    let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

    let first = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(first.document.source[0].path, "base.png");

    let play_side_2p = BTreeMap::from([("PLAY SIDE".to_string(), "2P".to_string())]);
    let second = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &play_side_2p,
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache),
    )
    .unwrap();
    assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(second.document.source[0].path, "base.png");
}

#[test]
fn lr2_document_cache_misses_when_included_file_changes() {
    let root = unique_test_dir("bmz-lr2-document-cache-include");
    std::fs::create_dir_all(&root).unwrap();
    let skin_path = root.join("play.lr2skin");
    let include_path = root.join("parts.csv");
    std::fs::write(
        &skin_path,
        r#"
#INFORMATION,0,Cache Test,Author
#INCLUDE,parts.csv
"#,
    )
    .unwrap();
    std::fs::write(&include_path, "#IMAGE,off.png\n").unwrap();
    let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

    let first = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(first.document.source[0].path, "off.png");

    std::fs::write(&include_path, "#IMAGE,on-longer-name.png\n").unwrap();
    let second = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache),
    )
    .unwrap();
    assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(second.document.source[0].path, "on-longer-name.png");
}

#[test]
fn lr2_document_cache_misses_when_used_file_selection_changes() {
    let root = unique_test_dir("bmz-lr2-document-cache-file");
    std::fs::create_dir_all(root.join("parts")).unwrap();
    std::fs::write(root.join("parts/blue.png"), []).unwrap();
    std::fs::write(root.join("parts/red.png"), []).unwrap();
    let skin_path = root.join("play.lr2skin");
    std::fs::write(
        &skin_path,
        r#"
#INFORMATION,0,Cache Test,Author
#CUSTOMFILE,Parts,parts/*.png,blue
#IMAGE,parts/*.png
"#,
    )
    .unwrap();
    let cache = Arc::new(Mutex::new(SkinDocumentCache::default()));

    let first = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState::default(),
        Some(cache.clone()),
    )
    .unwrap();
    assert_eq!(first.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(first.document.source[0].path, "parts/blue.png");

    let selected = BTreeMap::from([("Parts".to_string(), "red.png".to_string())]);
    let second = load_skin_document(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::new(),
        &selected,
        &LuaLoadRuntimeState::default(),
        Some(cache),
    )
    .unwrap();
    assert_eq!(second.cache_status, DocumentCacheStatus::Miss);
    assert_eq!(second.document.source[0].path, "parts/red.png");
}
