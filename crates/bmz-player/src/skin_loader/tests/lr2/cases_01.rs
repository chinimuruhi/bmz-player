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
