use super::*;

#[test]
fn render_lua_main_state_reads_current_frame_skin_offset() {
    let mut state = SkinDrawState::default();
    state
        .skin_offsets
        .set(45, bmz_render::skin_offset::SkinOffsetValue { x: 1, y: 2, w: 3, h: 4, r: 5, a: -6 });
    let text_values = BTreeMap::new();
    let provider =
        RenderLuaMainState { state: &state, enabled_options: &[], text_values: &text_values };

    assert_eq!(
        provider.offset(45),
        bmz_skin::LuaSkinOffsetValue { x: 1, y: 2, w: 3, h: 4, r: 5, a: -6 }
    );
    assert_eq!(provider.offset(46), bmz_skin::LuaSkinOffsetValue::default());
}

#[test]
fn lua_compat_virtual_io_contains_only_sanitized_beatoraja_config() {
    let files = lua_compat_virtual_io_files();
    assert_eq!(files.len(), 2);

    let system: serde_json::Value =
        serde_json::from_str(&files["config_sys.json"]).expect("system config should be JSON");
    assert_eq!(system, serde_json::json!({ "playername": "bmz" }));

    let player: serde_json::Value = serde_json::from_str(&files["player/bmz/config_player.json"])
        .expect("player config should be JSON");
    let player = player.as_object().expect("player config should be an object");
    assert_eq!(
        player.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from(["mode5", "mode7", "mode9", "mode10", "mode14", "mode24", "mode24double"])
    );
    for mode in player.values() {
        assert_eq!(mode["keyboard"], serde_json::json!({}));
        assert_eq!(mode["controller"], serde_json::json!([]));
        assert_eq!(mode["midi"], serde_json::json!({}));
    }
}

#[test]
fn modern_chic_result_bakes_runtime_song_label_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ModernChic/result.luaskin");
    if !skin_path.is_file() {
        return;
    }
    let runtime_state = LuaLoadRuntimeState {
        text_values: BTreeMap::from([
            (10, "Song".to_string()),
            (11, "Subtitle".to_string()),
            (12, "Song Subtitle".to_string()),
            (13, "Genre".to_string()),
            (14, "Artist".to_string()),
            (1003, "Table ★12".to_string()),
        ]),
        ..LuaLoadRuntimeState::default()
    };
    let loaded = load_skin_document_uncached(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &runtime_state,
    )
    .expect("unmodified ModernChic result should decode with runtime song text");
    let bottom = loaded
        .document
        .text
        .iter()
        .find(|text| text.id == "bottomResult")
        .expect("ModernChic bottomResult text");
    assert_eq!(bottom.constant_text, "Song Subtitle / Artist / Genre / Table ★12");
}

#[test]
fn luxe_flat_result_decodes_local_panel_state_and_tab_actions() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/Luxez-Flat/result/result.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let runtime_state = LuaLoadRuntimeState {
        option_values: BTreeMap::from([(50, false), (51, true)]),
        ..LuaLoadRuntimeState::default()
    };
    let loaded = load_skin_document_uncached(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &runtime_state,
    )
    .expect("unmodified Luxe Flat result should decode through the BMZ loader");

    assert_eq!(loaded.document.result_panel_default, Some(2));
    assert_eq!(
        loaded
            .document
            .image
            .iter()
            .find(|image| image.id == "result_modeselect_graph_data_off")
            .and_then(|image| image.act),
        Some(bmz_render::skin::SKIN_EVENT_RESULT_PANEL_GRAPH)
    );
    assert_eq!(
        loaded
            .document
            .image
            .iter()
            .find(|image| image.id == "result_modeselect_ir_ranking_off")
            .and_then(|image| image.act),
        Some(bmz_render::skin::SKIN_EVENT_RESULT_PANEL_IR)
    );
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
    assert_eq!(
        loaded
            .document
            .value
            .iter()
            .find(|value| value.id == "rank_diff_count")
            .map(|value| value.value_expr.as_str()),
        Some("bmz:nearest_rank_diff_abs")
    );
    assert_eq!(
        loaded
            .document
            .value
            .iter()
            .find(|value| value.id == "ir_scorerate1")
            .map(|value| value.value_expr.as_str()),
        Some("bmz:ir_score_rate_integer:1")
    );
    assert_eq!(
        loaded
            .document
            .value
            .iter()
            .find(|value| value.id == "ir_scorerate_dot1")
            .map(|value| value.value_expr.as_str()),
        Some("bmz:ir_score_rate_fraction:1")
    );
    assert!(loaded.document.destination.iter().any(|entry| matches!(
        entry,
        DestinationListEntry::Single(destination)
            if destination.id == "rank_diff_aaa_plus"
                && destination.draw.contains("nearest_rank(AAA,plus)")
    )));
}

#[test]
fn luxe_flat_result_displays_extended_arrange_labels_and_lane_pattern() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/Luxez-Flat/result/result.luaskin");
    if !skin_path.is_file() {
        return;
    }
    let runtime_state = LuaLoadRuntimeState {
        event_index_values: BTreeMap::from([(42, 2), (43, 2), (344, 10), (345, 11)]),
        option_values: BTreeMap::from([(163, true)]),
        ..LuaLoadRuntimeState::default()
    };
    let loaded = load_skin_document_uncached(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &runtime_state,
    )
    .expect("Luxe Flat result should decode extended arrange labels");

    assert_eq!(
        loaded
            .document
            .text
            .iter()
            .find(|text| text.id == "lane_option")
            .map(|text| text.constant_text.as_str()),
        Some("F-RANDOM / MF-RANDOM")
    );
    assert!(loaded.document.destination.iter().any(|entry| matches!(
        entry,
        DestinationListEntry::Single(destination)
            if destination.id == "1key"
                && destination.draw.contains("event_index(450) == 1")
    )));

    let course_skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/Luxez-Flat/result/courseresult.luaskin");
    let course_loaded = load_skin_document_uncached(
        &course_skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &runtime_state,
    )
    .expect("Luxe Flat course result should decode extended arrange labels");
    assert_eq!(
        course_loaded
            .document
            .text
            .iter()
            .find(|text| text.id == "lane_option")
            .map(|text| text.constant_text.as_str()),
        Some("F-RANDOM / MF-RANDOM")
    );
}

#[test]
fn ecfn_play7_1p_json_skin_can_be_applied_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN/play/play7-1p.json");
    if !skin_path.is_file() {
        return;
    }
    let mut renderer = Renderer::default();

    apply_beatoraja_json_skin(&mut renderer, &skin_path).unwrap();
}

#[test]
fn ecfn_result_json_skin_can_be_applied_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/ECFN/RESULT/result-converted.json");
    if !skin_path.is_file() {
        return;
    }
    let mut renderer = Renderer::default();

    apply_beatoraja_result_json_skin(&mut renderer, &skin_path).unwrap();
}

#[test]
fn ecfn_select_json_skin_can_be_applied_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/ECFN/select/select-converted.json");
    if !skin_path.is_file() {
        return;
    }
    let mut renderer = Renderer::default();

    apply_beatoraja_select_json_skin(&mut renderer, &skin_path).unwrap();
}

#[test]
#[ignore = "manual select skin profiling helper"]
fn profile_ecfn_select_plan_generation() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN/select/select.luaskin");
    if !skin_path.is_file() {
        eprintln!("skip: {} is missing", skin_path.display());
        return;
    }

    let decoded = decode_beatoraja_skin_with_options(
        &skin_path,
        SkinKind::Select,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();
    let document_textures = decoded.sources.iter().map(|source| SkinDocumentTexture {
        source_id: source.source_id.clone(),
        texture: source.texture,
        source_size: SkinImageSize { width: source.size.width, height: source.size.height },
    });
    let skin = SkinContext::from_manifest_and_document(
        bmz_render::skin::default_skin_manifest(),
        decoded.document,
        document_textures,
    );
    let rows = (0..25)
        .map(|index| SelectRowSnapshot {
            index,
            title: format!("War in the Mirrorworld[{index:02}]"),
            artist: "Aoi".to_string(),
            difficulty_name: "ANOTHER".to_string(),
            play_level: "12".to_string(),
            total_notes: 2253,
            chart_normal_notes: 2167,
            chart_scratch_notes: 86,
            chart_density: 19.0,
            chart_peak_density: 38.0,
            chart_end_density: 25.0,
            min_bpm: 171.0,
            max_bpm: 171.0,
            chart_main_bpm: 171.0,
            initial_bpm: 171.0,
            length_ms: 115_000,
            ..SelectRowSnapshot::default()
        })
        .collect();
    let mut runtime = DynamicTimerRuntime::default();
    let mut snapshot = SelectSnapshot {
        time: TimeUs(0),
        selection_time: TimeUs(0),
        chart_count: 1_000,
        selected_index: 12,
        rows,
        stage_background: true,
        banner_image: true,
        ..SelectSnapshot::default()
    };

    for frame in 0..30 {
        snapshot.time = TimeUs(frame * 16_666);
        black_box(DrawPlan::from_scene_with_skin(
            &AppSceneSnapshot::Select(snapshot.clone()),
            &skin,
            &mut runtime,
        ));
    }

    let frames = 300;
    let start = Instant::now();
    let mut commands = 0_usize;
    for frame in 0..frames {
        snapshot.time = TimeUs((frame + 30) * 16_666);
        let plan = DrawPlan::from_scene_with_skin(
            &AppSceneSnapshot::Select(snapshot.clone()),
            &skin,
            &mut runtime,
        );
        commands += plan.commands.len();
        black_box(plan);
    }
    let elapsed = start.elapsed();
    println!(
        "profile_ecfn_select_plan_generation frames={frames} avg_plan_ms={:.3} avg_commands={}",
        elapsed.as_secs_f64() * 1000.0 / frames as f64,
        commands / frames as usize
    );
}

#[test]
fn m_select_lua_select_skin_renders_items_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/mz-select/music_select.luaskin");
    if !skin_path.is_file() {
        return;
    }
    let decoded = decode_beatoraja_skin_with_options(
        &skin_path,
        SkinKind::Select,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(decoded.document.text.iter().any(|text| {
        text.id == "defaultNotesProcessingCounter_notes"
            || text.id == "defaultNotesProcessingCounter_stroke"
    }));
    let document_textures =
        decoded.sources.iter().map(|source| bmz_render::skin::SkinDocumentTexture {
            source_id: source.source_id.clone(),
            texture: source.texture,
            source_size: bmz_render::skin::SkinImageSize {
                width: source.size.width,
                height: source.size.height,
            },
        });
    let context = bmz_render::skin::SkinContext::from_manifest_and_document(
        bmz_render::skin::default_skin_manifest(),
        decoded.document,
        document_textures,
    );
    assert!(context.document().is_some_and(|document| document.skin_type == 5));
    let snapshot = bmz_render::scene::SelectSnapshot {
        arrange: "F-RANDOM".to_string(),
        arrange_2p: "MF-RANDOM".to_string(),
        gauge: "EX-HARD".to_string(),
        double_option: "BATTLE".to_string(),
        hs_fix: "CONSTANT".to_string(),
        rows: vec![bmz_render::scene::SelectRowSnapshot {
            title: "Song".to_string(),
            ..Default::default()
        }],
        chart_count: 1,
        ..Default::default()
    };
    let items = context.select_document_items_with_dynamic_timers(&snapshot, None);
    assert!(!items.is_empty(), "m_select select skin should produce render items");
    assert!(
            items
                .iter()
                .any(|item| matches!(item, bmz_render::skin::SkinRenderItem::Text { text, .. } if text == "Song")),
            "m_select select skin should render the song title text"
        );
    for label in ["F-RANDOM", "MF-RANDOM", "EX-HARD", "BATTLE", "CONSTANT"] {
        assert!(
                items.iter().any(
                    |item| matches!(item, bmz_render::skin::SkinRenderItem::Text { text, .. } if text == label)
                ),
                "m_select should render the dynamic option label {label}"
            );
    }
    for x in [503.0, 586.0] {
        let hit = context
            .select_click_hit(&snapshot, x / 1920.0, 0.98)
            .expect("m_select arrange cell should remain clickable across its full width");
        assert_eq!(hit.target, bmz_render::skin::SkinClickTarget::Event { event_id: 42, click: 2 });
        assert!((hit.rect.x - 462.0 / 1920.0).abs() < f32::EPSILON);
        assert!((hit.rect.width - 166.0 / 1920.0).abs() < f32::EPSILON);
    }
}

#[test]
fn antique_play_skin_shows_random_lane_pattern_before_ready_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/mz-select/play/antique/system/play7main.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let load_context = |options: &BTreeMap<String, String>| {
        let decoded = decode_beatoraja_skin_with_options(
            &skin_path,
            SkinKind::Play,
            options,
            &BTreeMap::new(),
        )
        .unwrap();
        let number_texture = decoded
            .sources
            .iter()
            .find(|source| source.source_id == "src_number_lane")
            .expect("antique number lane source")
            .texture;
        let document_textures = decoded.sources.iter().map(|source| SkinDocumentTexture {
            source_id: source.source_id.clone(),
            texture: source.texture,
            source_size: SkinImageSize { width: source.size.width, height: source.size.height },
        });
        (
            SkinContext::from_manifest_and_document(
                SkinManifest::default(),
                decoded.document,
                document_textures,
            ),
            number_texture,
        )
    };
    let (default_context, default_number_texture) = load_context(&BTreeMap::new());
    let default_document = default_context.document().expect("antique play document");
    assert!(default_document.enabled_options().contains(&916));
    assert!(!default_document.enabled_options().contains(&917));
    assert!(
        default_document
            .property
            .iter()
            .any(|property| { property.name == "RANDOM配置表示" && property.def == "OFF" })
    );
    let options = BTreeMap::from([("RANDOM配置表示".to_string(), "ON".to_string())]);
    let (context, number_texture) = load_context(&options);
    let document = context.document().expect("antique play document");
    assert!(document.enabled_options().contains(&917));
    assert!(!document.enabled_options().contains(&916));
    assert!(
        document
            .all_destinations(&document.enabled_options())
            .iter()
            .filter(|destination| destination.id.starts_with("num_random_"))
            .all(|destination| !destination.draw.starts_with("bmz:lua_draw_callback:")),
        "RANDOM digit color predicates should compile without per-frame Lua callbacks"
    );
    let displayed_values = [2_u8, 3, 4, 5, 6, 7, 1];
    let mut pattern = (0..bmz_core::lane::LANE_COUNT as u8).collect::<Vec<_>>();
    for (destination, source) in (1..=7).zip(displayed_values) {
        pattern[destination] = source;
    }
    let applied_arrange = crate::screens::play_session::AppliedArrange {
        arrange: crate::select_options::ArrangeOption::Random,
        pattern: Some(pattern.clone()),
        ..crate::screens::play_session::AppliedArrange::default()
    };
    let mut pre_ready = bmz_render::snapshot::RenderSnapshot {
        key_mode: KeyMode::K7,
        ready_elapsed_time: None,
        ..Default::default()
    };
    crate::screens::play_loop::apply_play_arrange_to_snapshot(&mut pre_ready, &applied_arrange);
    assert_eq!(pre_ready.lane_shuffle_pattern, pattern);

    let render = |context: &SkinContext, snapshot| {
        bmz_render::plan::DrawPlan::from_scene_with_skin(
            &bmz_render::scene::AppSceneSnapshot::Play(snapshot),
            context,
            &mut bmz_render::skin::DynamicTimerRuntime::default(),
        )
    };
    let random_digits = |plan: &bmz_render::plan::DrawPlan, number_texture: SkinTextureId| {
        let mut digits = plan
            .commands
            .iter()
            .filter_map(|command| match command {
                bmz_render::plan::DrawCommand::Image { texture, rect, tint, .. }
                    if texture.0 == number_texture.0 && (0.69..0.72).contains(&rect.y) =>
                {
                    Some((rect.x, *tint))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        digits.sort_by(|left, right| left.0.total_cmp(&right.0));
        digits
    };

    let digits = random_digits(&render(&context, pre_ready.clone()), number_texture);
    assert_eq!(digits.len(), 7, "expected seven pre-READY RANDOM digits");
    for (index, (_, tint)) in digits.into_iter().enumerate() {
        let value = displayed_values[index];
        let expected = if matches!(value, 1 | 3 | 5 | 7) {
            (1.0, 1.0, 1.0)
        } else {
            (64.0 / 255.0, 160.0 / 255.0, 1.0)
        };
        assert!((tint.r - expected.0).abs() < 0.01);
        assert!((tint.g - expected.1).abs() < 0.01);
        assert!((tint.b - expected.2).abs() < 0.01);
    }

    let mut ready = pre_ready.clone();
    ready.ready_elapsed_time = Some(TimeUs(0));
    let ready_digits = random_digits(&render(&context, ready.clone()), number_texture);
    assert_eq!(ready_digits.len(), 7, "READY should start at full opacity");
    assert!(ready_digits.iter().all(|(_, tint)| (tint.a - 1.0).abs() < 0.01));

    ready.ready_elapsed_time = Some(TimeUs(250_000));
    let fading_digits = random_digits(&render(&context, ready.clone()), number_texture);
    assert_eq!(fading_digits.len(), 7, "RANDOM digits should fade for 500 ms");
    assert!(fading_digits.iter().all(|(_, tint)| (tint.a - 0.5).abs() < 0.02));

    ready.ready_elapsed_time = Some(TimeUs(501_000));
    assert!(
        random_digits(&render(&context, ready), number_texture).is_empty(),
        "RANDOM digits should disappear after the BACKBMP 500 ms fade"
    );

    let mut no_pattern = pre_ready.clone();
    crate::screens::play_loop::apply_play_arrange_to_snapshot(
        &mut no_pattern,
        &crate::screens::play_session::AppliedArrange {
            arrange: crate::select_options::ArrangeOption::Random,
            ..crate::screens::play_session::AppliedArrange::default()
        },
    );
    assert!(random_digits(&render(&context, no_pattern), number_texture).is_empty());

    assert!(
        random_digits(&render(&default_context, pre_ready), default_number_texture).is_empty(),
        "RANDOM display should default to OFF"
    );
}

#[test]
fn luxe_flat_lua_select_skin_keeps_operating_time_refs_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/Luxez-Flat/music_select.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Select).unwrap();
    for ref_id in 27..=29 {
        assert!(
            decoded.document.value.iter().any(|value| value.ref_id == ref_id),
            "Luxe Flat should retain operating-time ref {ref_id}"
        );
    }
    for (id, center_x) in [
        ("bmz_select_arrange", 138),
        ("bmz_select_gauge", 302),
        ("bmz_select_double_option", 446),
        ("bmz_select_hs_fix", 613),
        ("bmz_select_arrange_2p", 790),
    ] {
        assert!(
            decoded.document.text.iter().any(|text| text.id == id),
            "Luxe Flat should decode dynamic {id} text"
        );
        assert!(decoded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination)
                if destination.id == id
                    && destination.act.is_none()
                    && matches!(
                        destination.dst.first(),
                        Some(bmz_render::skin::SkinDstEntry::Frame(frame))
                            if frame.x == Some(center_x)
            )
        )));
    }
    assert!(
        decoded
            .document
            .panel
            .iter()
            .any(|panel| panel.id == "bmz_select_option_hit" && panel.color == "00000000")
    );
    for (act, left_x, width) in
        [(42, 69, 138), (40, 254, 96), (54, 381, 129), (55, 550, 126), (43, 721, 138)]
    {
        assert!(decoded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination)
                if destination.id == "bmz_select_option_hit"
                    && destination.act == Some(act)
                    && matches!(
                        destination.dst.first(),
                        Some(bmz_render::skin::SkinDstEntry::Frame(frame))
                            if frame.x == Some(left_x) && frame.w == Some(width)
            )
        )));
    }
    let document_textures =
        decoded.sources.iter().map(|source| bmz_render::skin::SkinDocumentTexture {
            source_id: source.source_id.clone(),
            texture: source.texture,
            source_size: bmz_render::skin::SkinImageSize {
                width: source.size.width,
                height: source.size.height,
            },
        });
    let context = bmz_render::skin::SkinContext::from_manifest_and_document(
        bmz_render::skin::default_skin_manifest(),
        decoded.document,
        document_textures,
    );
    let hit = context
        .select_click_hit(&bmz_render::scene::SelectSnapshot::default(), 100.0 / 1920.0, 0.98)
        .expect("Luxe Flat arrange cell should be clickable from its left half");
    assert_eq!(hit.target, bmz_render::skin::SkinClickTarget::Event { event_id: 42, click: 0 });
    assert!((hit.rect.x - 69.0 / 1920.0).abs() < f32::EPSILON);
    assert!((hit.rect.width - 138.0 / 1920.0).abs() < f32::EPSILON);
}

#[test]
fn rm_skin_play_lua_skins_can_be_decoded_when_available() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/Rm-skin");
    let cases = [
        (root.join("play5main.luaskin"), SkinKind::Play),
        (root.join("play7main.luaskin"), SkinKind::Play),
        (root.join("play9main.luaskin"), SkinKind::Play),
    ];
    for (skin_path, kind) in cases {
        if !skin_path.is_file() {
            continue;
        }
        let decoded = decode_beatoraja_skin(&skin_path, kind).unwrap();
        assert!(!decoded.document.destination.is_empty(), "{}", skin_path.display());
    }
}

#[test]
fn ecfn_lua_skins_can_be_decoded_when_available() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN");
    let cases = [
        (root.join("select/select.luaskin"), SkinKind::Select),
        (root.join("play/play7.luaskin"), SkinKind::Play),
        (root.join("RESULT/result.luaskin"), SkinKind::Result),
    ];
    for (skin_path, kind) in cases {
        if !skin_path.is_file() {
            continue;
        }
        let decoded = decode_beatoraja_skin(&skin_path, kind).unwrap();
        assert!(!decoded.document.destination.is_empty());
    }
}

#[test]
fn luxe_flat_lua_select_skin_keeps_score_availability_guards_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/Luxez-Flat/music_select.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Select).unwrap();
    let clear_state = decoded
        .document
        .destination
        .iter()
        .find_map(|entry| match entry {
            DestinationListEntry::Single(destination)
                if destination.id == "default_playerdata_state_clear" =>
            {
                Some(destination)
            }
            DestinationListEntry::Single(_) | DestinationListEntry::Conditional { .. } => None,
        })
        .expect("Luxe Flat should retain the player clear-state destination");
    assert_eq!(clear_state.draw, "select_score_available()");
}

#[test]
fn mz_select_lua_select_skin_keeps_local_score_availability_guards() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/mz-select/music_select.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Select).unwrap();
    let guarded = decoded
        .document
        .destination
        .iter()
        .filter_map(|entry| match entry {
            DestinationListEntry::Single(destination)
                if destination.id.starts_with("default_playerdata_")
                    && destination.draw == "select_score_available()" =>
            {
                Some(destination.id.as_str())
            }
            DestinationListEntry::Single(_) | DestinationListEntry::Conditional { .. } => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(guarded.len(), 21, "mz-select player-data score guards: {guarded:?}");
    assert!(guarded.contains(&"default_playerdata_state_clear"));
    assert!(guarded.contains(&"default_playerdata_score_count"));
    assert!(guarded.contains(&"default_playerdata_scorerate_dot_count"));
}

#[test]
fn mz_select_result_uses_runtime_decisions_and_draws_note_graphs() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/mz-select/result/result.luaskin");
    if !skin_path.is_file() {
        return;
    }
    let runtime_state = LuaLoadRuntimeState {
        number_values: BTreeMap::from([
            (74, 100),
            (153, 354),
            (370, 7),
            (371, 5),
            (374, -12),
            (375, -50),
            (410, 20),
            (411, 10),
            (412, 8),
            (413, 4),
            (414, 3),
            (415, 2),
            (416, 1),
            (417, 1),
            (418, 1),
            (419, 1),
            (421, 1),
            (422, 1),
        ]),
        ..LuaLoadRuntimeState::default()
    };
    let decoded = decode_beatoraja_skin_with_options_and_runtime_state(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &runtime_state,
    )
    .expect("decode mz-select result skin with runtime result values");

    let timing = decoded
        .document
        .text
        .iter()
        .find(|text| text.id == "timing")
        .expect("mz-select timing text");
    assert_eq!(timing.constant_text, "平均12.5ms遅い");
    for (id, label, draw) in [
        ("arrange_f_random", "F-RANDOM", "event_index(344) == 10"),
        ("arrange_mf_random", "MF-RANDOM", "event_index(344) == 11"),
        ("arrange_f_random_2p", "2P F-RANDOM", "event_index(345) == 10"),
        ("arrange_mf_random_2p", "2P MF-RANDOM", "event_index(345) == 11"),
    ] {
        assert!(
            decoded.document.text.iter().any(|text| text.id == id && text.constant_text == label),
            "mz-select result should decode {id} text"
        );
        assert!(decoded.document.destination.iter().any(|entry| matches!(
            entry,
            DestinationListEntry::Single(destination)
                if destination.id == id && destination.draw == draw
        )));
    }
    let clear_state = decoded
        .document
        .image
        .iter()
        .find(|image| image.id == "clear_state")
        .expect("mz-select clear update image");
    assert_eq!(clear_state.x, 0, "current clear above previous should use UP image");
    assert!(decoded.document.destination.iter().any(|entry| matches!(
        entry,
        DestinationListEntry::Single(destination) if destination.id == "win"
    )));
    assert!(!decoded.document.destination.iter().any(|entry| matches!(
        entry,
        DestinationListEntry::Single(destination) if destination.id == "draw"
    )));

    let document_textures = decoded.sources.iter().map(|source| SkinDocumentTexture {
        source_id: source.source_id.clone(),
        texture: source.texture,
        source_size: source.size,
    });
    let context = SkinContext::from_manifest_and_document(
        bmz_render::skin::default_skin_manifest(),
        decoded.document,
        document_textures,
    );
    let graph = std::sync::Arc::new(bmz_render::snapshot::ResultGraphSnapshot {
        judge_graph_buckets: vec![
            bmz_render::snapshot::ResultJudgeGraphBucket { values: [0, 10, 5, 2, 1, 1] },
            bmz_render::snapshot::ResultJudgeGraphBucket { values: [0, 8, 4, 2, 1, 0] },
        ],
        early_late_graph_buckets: vec![
            bmz_render::snapshot::ResultEarlyLateGraphBucket {
                values: [0, 10, 4, 2, 1, 0, 3, 2, 1, 0],
            },
            bmz_render::snapshot::ResultEarlyLateGraphBucket {
                values: [0, 8, 3, 2, 1, 0, 4, 2, 1, 0],
            },
        ],
        judge_graph_density: vec![12, 18],
        ..bmz_render::snapshot::ResultGraphSnapshot::default()
    });
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 500,
        result_failed: Some(false),
        total_notes: 100,
        key_mode: KeyMode::K7,
        ..bmz_render::skin::SkinDrawState::default()
    };
    let items = context.static_document_items_for_result_state_and_text(
        &graph,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );
    let populated_batches = items
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::RectBatch { rects, .. } if !rects.is_empty()
            )
        })
        .count();
    assert_eq!(populated_batches, 2, "JUDGE and FAST/SLOW graph batches should render");
    assert!(
        !items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Rect {
                color,
                blend: bmz_render::skin::BlendMode::Add,
                ..
            } if color.r == 0.0 && color.g == 0.0 && color.b == 0.0
        )),
        "additive black gauge backgrounds must not cover the two note graphs"
    );
}

#[test]
fn ecfn_play7_judge_combo_x_matches_beatoraja_layout_when_available() {
    use std::collections::HashMap;

    use bmz_render::skin::{SkinDocumentTexture, SkinImageSize, SkinRenderItem, SkinTextureId};

    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN/play/play7.luaskin");
    if !skin_path.is_file() {
        return;
    }
    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let mock_texture = SkinDocumentTexture {
        source_id: "mock".to_string(),
        texture: SkinTextureId(1),
        source_size: SkinImageSize { width: 1920.0, height: 1080.0 },
    };
    let sources: HashMap<String, SkinDocumentTexture> = decoded
        .document
        .source
        .iter()
        .map(|source| (source.id.clone(), mock_texture.clone()))
        .chain(decoded.document.value.iter().map(|value| (value.src.clone(), mock_texture.clone())))
        .chain(decoded.document.image.iter().map(|image| (image.src.clone(), mock_texture.clone())))
        .collect();
    let items =
        decoded.document.judge_render_items("PGREAT", 42, 100, &sources).expect("judge items");
    let digit_xs: Vec<f32> = items
        .iter()
        .skip(1)
        .filter_map(|item| match item {
            SkinRenderItem::Image { rect, .. } => Some(rect.x),
            _ => None,
        })
        .collect();
    assert_eq!(digit_xs.len(), 2);
    let expected_first = 334.0 / 1920.0;
    let expected_second = 392.0 / 1920.0;
    assert!(
        (digit_xs[0] - expected_first).abs() < 0.001,
        "first digit x={} expected {expected_first}",
        digit_xs[0]
    );
    assert!(
        (digit_xs[1] - expected_second).abs() < 0.001,
        "second digit x={} expected {expected_second}",
        digit_xs[1]
    );
}

#[test]
fn ecfn_play7_pre_notes_judge_line_renders_in_front_when_available() {
    use std::collections::HashMap;

    use bmz_render::skin::{
        SkinDocumentTexture, SkinDrawState, SkinImageSize, SkinRenderItem, SkinTextState,
    };

    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN/play/play7.luaskin");
    if !skin_path.is_file() {
        return;
    }
    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let image_15 = decoded
        .document
        .image
        .iter()
        .find(|image| image.id == "15")
        .expect("ECFN id=15 image should decode");
    assert_eq!((image_15.src.as_str(), image_15.x, image_15.y), ("0", 16, 0));
    let image_15_map = decoded.document.image_map();
    let mapped_15 = image_15_map.get("15").expect("ECFN id=15 image should map");
    assert_eq!((mapped_15.src.as_str(), mapped_15.x, mapped_15.y), ("0", 16, 0));
    let system_texture = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "0")
        .map(|source| source.texture)
        .expect("ECFN source 0 should decode");
    let system_size = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "0")
        .map(|source| SkinImageSize { width: source.size.width, height: source.size.height })
        .expect("ECFN source 0 should decode");
    let sources: HashMap<String, SkinDocumentTexture> = decoded
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
        .collect();

    let (behind, front, _) = decoded.document.static_render_items_split(
        &sources,
        &SkinDrawState::default(),
        &SkinTextState::default(),
    );

    assert!(
        behind.iter().all(|item| !matches!(
            item,
            SkinRenderItem::Image {
                texture,
                rect,
                ..
            } if *texture == system_texture
                && (rect.y - 715.0 / 1080.0).abs() < 0.001
                && (rect.height - 8.0 / 1080.0).abs() < 0.001
        )),
        "ECFN judge line should not remain behind notes"
    );
    assert!(
        front.iter().any(|item| matches!(
            item,
            SkinRenderItem::Image {
                texture,
                rect,
                uv,
                ..
            } if *texture == system_texture
                && (rect.y - 715.0 / 1080.0).abs() < 0.001
                && (rect.height - 8.0 / 1080.0).abs() < 0.001
                && (uv.x - 16.0 / system_size.width).abs() < 0.001
                && uv.y.abs() < 0.001
        )),
        "expected ECFN id=15 judge line in front items; got {front:?}"
    );
}

#[test]
fn ecfn_play14_judge1_combo_is_right_of_judge_when_available() {
    use std::collections::HashMap;

    use bmz_core::lane::Lane;
    use bmz_render::skin::{
        MAX_JUDGE_REGIONS, SkinDocumentTexture, SkinDrawState, SkinImageSize, SkinRenderItem,
        SkinTextureId,
    };

    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN/play/play14.luaskin");
    if !skin_path.is_file() {
        return;
    }
    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play)
        .expect("ECFN play14 should decode with default options");
    let judge0 = decoded.document.judge.iter().find(|judge| judge.id == "judge").expect("judge");
    let judge1 = decoded.document.judge.iter().find(|judge| judge.id == "judge1").expect("judge1");
    assert_eq!(judge0.index, 0);
    assert_eq!(judge1.index, 1);

    let mock_texture = SkinDocumentTexture {
        source_id: "mock".to_string(),
        texture: SkinTextureId(1),
        source_size: SkinImageSize { width: 1920.0, height: 1080.0 },
    };
    let sources: HashMap<String, SkinDocumentTexture> = decoded
        .document
        .source
        .iter()
        .map(|source| (source.id.clone(), mock_texture.clone()))
        .chain(decoded.document.value.iter().map(|value| (value.src.clone(), mock_texture.clone())))
        .chain(decoded.document.image.iter().map(|image| (image.src.clone(), mock_texture.clone())))
        .collect();

    let mut judge_ms = [None; MAX_JUDGE_REGIONS];
    let mut judge_index = [None; MAX_JUDGE_REGIONS];
    judge_ms[0] = Some(100);
    judge_ms[1] = Some(100);
    judge_index[0] = Some(0);
    judge_index[1] = Some(0);
    let state = SkinDrawState { judge_ms, judge_index, combo: 42, ..SkinDrawState::default() };

    let left_items = decoded
        .document
        .judge_render_items_for_def(judge0, 0, 42, 100, &sources, &state)
        .expect("left judge");
    let right_items = decoded
        .document
        .judge_render_items_for_def(judge1, 0, 42, 100, &sources, &state)
        .expect("right judge");
    let left_digit = left_items
        .iter()
        .skip(1)
        .find_map(|item| match item {
            SkinRenderItem::Image { rect, .. } => Some(rect.x),
            _ => None,
        })
        .expect("left combo digit");
    let right_digit = right_items
        .iter()
        .skip(1)
        .find_map(|item| match item {
            SkinRenderItem::Image { rect, .. } => Some(rect.x),
            _ => None,
        })
        .expect("right combo digit");
    assert!(
        right_digit > left_digit,
        "judge1 digit x={right_digit} should be right of judge x={left_digit}"
    );

    let region = bmz_render::skin::lane_judge_region(
        Lane::Key8.index(),
        bmz_core::lane::LANE_COUNT,
        decoded.document.judge_region_count(),
    );
    assert_eq!(region, 1);
}

#[test]
fn starseeker_play_lua_skin_can_be_decoded_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/Starseeker/play/play7.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

    assert!(!decoded.document.destination.is_empty());
}

#[test]
fn starseeker_result_lua_skin_renders_stat_details_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/Starseeker/result/result.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([
        ("F/Sリスト".to_string(), "Default".to_string()),
        ("逆サイド詳細フレーム".to_string(), "ON".to_string()),
        ("プレーサイド".to_string(), "1P".to_string()),
    ]);
    let files = BTreeMap::from([
        ("使用テーマ".to_string(), "Theme/starseeker".to_string()),
        ("フォント".to_string(), "_font/TYPE-M".to_string()),
        ("シャッター".to_string(), "Shutter/TYPE-M".to_string()),
    ]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Result, &options, &files)
            .expect("decode starseeker result skin");
    let destinations = decoded.document.all_destinations(&[]);
    let slow_judgement_timing = destinations
        .iter()
        .find(|destination| destination.id == "judge_adv_s")
        .expect("starseeker result should keep SLOW timing label destination");
    let fast_judgement_timing = destinations
        .iter()
        .find(|destination| destination.id == "judge_adv_f")
        .expect("starseeker result should keep FAST timing label destination");
    assert_eq!(slow_judgement_timing.draw, "number(374) < 0 or number(375) < 0");
    assert_eq!(fast_judgement_timing.draw, "number(374) > 0 or number(375) > 0");
    assert!(
        decoded.document.all_destinations(&[]).iter().any(|destination| {
            matches!(
                destination.id.as_str(),
                "judge_detail" | "judgegraph" | "fsgraph" | "timingGraph"
            )
        }),
        "starseeker result stat destinations should survive lua conversion"
    );
    assert!(
        decoded.document.source.iter().any(|source| source.id == "jud_detail_main"),
        "starseeker result document should keep jud_detail_main source; sources: {:?}",
        decoded.document.source.iter().map(|source| source.id.as_str()).collect::<Vec<_>>()
    );
    let stat_texture = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "jud_detail_main")
        .map(|source| source.texture)
        .expect("starseeker result should load jud_detail_main source");
    let document_textures =
        decoded.sources.iter().map(|source| bmz_render::skin::SkinDocumentTexture {
            source_id: source.source_id.clone(),
            texture: source.texture,
            source_size: bmz_render::skin::SkinImageSize {
                width: source.size.width,
                height: source.size.height,
            },
        });
    let context = bmz_render::skin::SkinContext::from_manifest_and_document(
        bmz_render::skin::default_skin_manifest(),
        decoded.document,
        document_textures,
    );
    let bmz_render::scene::AppSceneSnapshot::Result(mut snapshot) =
        bmz_render::sample::sample_result_scene()
    else {
        panic!("sample result scene");
    };
    snapshot.elapsed_time = bmz_core::time::TimeUs(1_000_000);
    snapshot.judge_counts = bmz_render::snapshot::DisplayJudgeCounts {
        pgreat: 120,
        great: 40,
        good: 12,
        bad: 4,
        poor: 3,
        empty_poor: 2,
    };
    snapshot.fast_slow_counts = bmz_render::snapshot::FastSlowJudgeCounts {
        fast_pgreat: 80,
        slow_pgreat: 40,
        fast_great: 12,
        slow_great: 28,
        fast_good: 4,
        slow_good: 8,
        fast_bad: 1,
        slow_bad: 3,
        fast_poor: 1,
        slow_poor: 2,
        fast_empty_poor: 1,
        slow_empty_poor: 1,
    };
    let graph = std::sync::Arc::make_mut(&mut snapshot.graph);
    graph.judge_graph_density = vec![1, 3, 2, 4];
    graph.timing_points = vec![
        bmz_render::snapshot::ResultTimingPoint {
            time_ms: 100,
            delta_us: -12_000,
            judge: bmz_core::judge::Judge::Great,
        },
        bmz_render::snapshot::ResultTimingPoint {
            time_ms: 200,
            delta_us: 8_000,
            judge: bmz_core::judge::Judge::PGreat,
        },
    ];

    let plan = bmz_render::plan::DrawPlan::from_scene_with_skin(
        &bmz_render::scene::AppSceneSnapshot::Result(snapshot),
        &context,
        &mut bmz_render::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        bmz_render::plan::DrawCommand::Image { texture, .. }
            if *texture == bmz_render::plan::TextureId(stat_texture.0)
    )));
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        bmz_render::plan::DrawCommand::Rect { rect, .. }
            if rect.x > 0.70 && rect.y > 0.20 && rect.y < 0.55
    )));
}

#[test]
fn milliondollar_result_runtime_events_toggle_observe_timers_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/MILLIONDOLLAR/result.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin_with_options(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .expect("decode MILLIONDOLLAR result skin");
    let cim_sources = decoded
        .sources
        .iter()
        .filter(|source| {
            source
                .path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("cim"))
        })
        .collect::<Vec<_>>();
    assert_eq!(cim_sources.len(), 11, "all MILLIONDOLLAR CIM atlases must decode");
    assert!(
        cim_sources.iter().all(|source| source.asset.is_some()),
        "MILLIONDOLLAR CIM atlases must provide RGBA assets before GPU upload"
    );
    let document = &decoded.document;
    let circle_destinations = document
        .destination
        .iter()
        .filter_map(|entry| match entry {
            DestinationListEntry::Single(destination)
                if destination.id == "Graph_Circle_Meter"
                    || destination.id == "Graph_Circle_Frame" =>
            {
                Some(destination)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(circle_destinations.len(), 724);
    let circle_timers = circle_destinations
        .iter()
        .filter_map(|destination| destination.timer)
        .collect::<BTreeSet<_>>();
    assert_eq!(circle_timers.len(), 1, "the shared circle visibility edge needs one timer");
    assert!(circle_timers.iter().all(|timer| {
        ((*timer - bmz_render::skin::SKIN_DYNAMIC_TIMER_BASE) as usize)
            < bmz_render::skin::SKIN_DYNAMIC_TIMER_COUNT
    }));

    let source_12 = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "12")
        .expect("MILLIONDOLLAR parts atlas");
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: source.size,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let mut circle_runtime = DynamicTimerRuntime::default();
    circle_runtime.reset_for_document(Some(document));
    let mut circle_state = SkinDrawState::default();
    circle_runtime.advance(document, &mut circle_state, 100);
    let circle_items =
        document.static_render_items(&sources, &circle_state, &SkinTextState::default());
    let rendered_segments = circle_items
        .iter()
        .filter(|item| {
            matches!(
                item,
                SkinRenderItem::RotatedImage { texture, .. }
                    if *texture == source_12.texture
            )
        })
        .count();
    assert!(
        rendered_segments >= 700,
        "MILLIONDOLLAR circle graph segments must render, got {rendered_segments}"
    );
    let circle_angles = circle_items
        .iter()
        .filter_map(|item| match item {
            SkinRenderItem::RotatedImage { texture, angle_deg, center, .. }
                if *texture == source_12.texture =>
            {
                assert!((center.x - 0.5).abs() < f32::EPSILON);
                assert!((center.y - 0.5).abs() < f32::EPSILON);
                Some(*angle_deg)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(circle_angles.iter().all(|angle| *angle >= 0.0));
    assert!(circle_angles.iter().any(|angle| *angle >= 359.0));

    let event = document.runtime_events.first().expect("runtime toggle event");
    let initial_true = event
        .toggle_flags
        .iter()
        .find(|flag_id| {
            document.runtime_flags.iter().any(|flag| flag.id == **flag_id && flag.initial)
        })
        .copied()
        .expect("initially visible flag");
    let initial_false = event
        .toggle_flags
        .iter()
        .find(|flag_id| {
            document.runtime_flags.iter().any(|flag| flag.id == **flag_id && !flag.initial)
        })
        .copied()
        .expect("initially hidden flag");
    let timer_index = |flag_id: i32| {
        let observe = format!("runtime_flag({flag_id})");
        let timer = document
            .dynamic_timers
            .iter()
            .find(|timer| timer.observe == observe)
            .expect("timer observing runtime flag");
        usize::try_from(timer.id - bmz_render::skin::SKIN_DYNAMIC_TIMER_BASE).unwrap()
    };
    let true_timer = timer_index(initial_true);
    let false_timer = timer_index(initial_false);
    let mut runtime = DynamicTimerRuntime::default();
    runtime.reset_for_document(Some(document));
    let mut state = SkinDrawState::default();

    runtime.advance(document, &mut state, 100);
    assert_eq!(state.dynamic_timer_ms[true_timer], Some(0));
    assert_eq!(state.dynamic_timer_ms[false_timer], None);

    assert!(runtime.dispatch_runtime_event(document, event.id));
    runtime.advance(document, &mut state, 150);
    assert_eq!(state.dynamic_timer_ms[true_timer], None);
    assert_eq!(state.dynamic_timer_ms[false_timer], Some(0));

    assert!(runtime.dispatch_runtime_event(document, event.id));
    runtime.advance(document, &mut state, 200);
    assert_eq!(state.dynamic_timer_ms[true_timer], Some(0));
    assert_eq!(state.dynamic_timer_ms[false_timer], None);
}

#[test]
fn milliondollar_result_song_info_uses_runtime_judge_rank_and_ln_mode_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/MILLIONDOLLAR/result.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin_with_options_and_runtime_state(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState {
            text_values: BTreeMap::from([(1, "RANK AAA".to_string())]),
            option_values: BTreeMap::from([
                (180, false),
                (181, true),
                (182, false),
                (183, false),
                (184, false),
            ]),
            event_index_values: BTreeMap::from([(42, 2), (43, 0), (54, 0), (308, 2)]),
            ..LuaLoadRuntimeState::default()
        },
    )
    .expect("decode MILLIONDOLLAR result skin with chart metadata");

    let judge_rank = decoded
        .document
        .image
        .iter()
        .find(|image| image.id == "Parts_Text_Info_Judgerank")
        .expect("MILLIONDOLLAR judge-rank label");
    let ln_type = decoded
        .document
        .image
        .iter()
        .find(|image| image.id == "Parts_Text_Info_Lntype")
        .expect("MILLIONDOLLAR LN-type label");
    let arrange = decoded
        .document
        .image
        .iter()
        .find(|image| image.id == "Parts_Texts_Useoption_SP")
        .expect("MILLIONDOLLAR SP arrange label");
    let target_rank = decoded
        .document
        .image
        .iter()
        .find(|image| image.id == "Parts_Texts_Target_Rank")
        .expect("MILLIONDOLLAR fixed target rank label");
    assert_eq!(judge_rank.y, 310, "HARD must select atlas row 3");
    assert_eq!(ln_type.y, 291, "HCN must select atlas row 2");
    assert_eq!(arrange.y, 48, "RANDOM must select atlas row 2");
    assert_eq!(target_rank.y, 16, "RANK AAA must select the AAA target row");
    assert!(decoded.document.destination.iter().any(|entry| matches!(
        entry,
        DestinationListEntry::Single(destination)
            if destination.id == "Parts_Texts_Useoption_SP"
    )));
}

#[test]
fn milliondollar_result_uses_integer_only_gauge_layout_at_one_hundred_percent_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/MILLIONDOLLAR/result.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin_with_options_and_runtime_state(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState {
            number_values: BTreeMap::from([(107, 100)]),
            ..LuaLoadRuntimeState::default()
        },
    )
    .expect("decode MILLIONDOLLAR result skin with full gauge");

    let draw_for = |id: &str| {
        decoded.document.destination.iter().find_map(|entry| match entry {
            DestinationListEntry::Single(destination) if destination.id == id => {
                Some(destination.draw.as_str())
            }
            _ => None,
        })
    };
    assert_eq!(draw_for("Number_Remaingauge_Max_1"), Some("number(107) == 100"));
    assert_eq!(draw_for("Number_Remaingauge_Max_00"), Some("number(107) == 100"));
    assert_eq!(draw_for("Number_Remaingauge_Normal"), Some("number(107) < 100"));
    assert_eq!(draw_for("Parts_Text_Remaingauge_Dot"), Some("number(107) < 100"));
    assert_eq!(draw_for("Number_Remaingauge_Afterdot"), Some("number(107) < 100"));
}

#[test]
fn milliondollar_result_rank_diff_uses_load_time_result_scores_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/MILLIONDOLLAR/result.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin_with_options_and_runtime_state(
        &skin_path,
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &LuaLoadRuntimeState {
            number_values: BTreeMap::from([
                (71, 2_877),
                (74, 1_550),
                (151, 2_756),
                (170, 3_021),
                (171, 2_877),
            ]),
            ..LuaLoadRuntimeState::default()
        },
    )
    .expect("decode MILLIONDOLLAR result skin with result scores");

    let best_rank = decoded
        .document
        .image
        .iter()
        .find(|image| image.id == "Parts_Rank_Middle_Best")
        .expect("MILLIONDOLLAR best DJ level");
    let next_rank = decoded
        .document
        .image
        .iter()
        .find(|image| image.id == "Parts_Rank_Nextrank")
        .expect("MILLIONDOLLAR next-rank label");
    let next_rank_diff = decoded
        .document
        .value
        .iter()
        .find(|value| value.id == "Number_Nextrank_Diff")
        .expect("MILLIONDOLLAR next-rank difference");

    assert_eq!(best_rank.y, 0, "3021/3100 must select the AAA row");
    assert_eq!(next_rank.x, 951, "positive rank difference must select the plus label");
    assert_eq!(next_rank.y, 18, "AAA+ must select the plus row");
    assert_eq!(next_rank_diff.value_expr, "121");
}

#[test]
fn starseeker_result_misscount_diff_uses_runtime_number_color_block() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/Starseeker/result/result.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([
        ("F/Sリスト".to_string(), "Default".to_string()),
        ("逆サイド詳細フレーム".to_string(), "ON".to_string()),
        ("プレーサイド".to_string(), "1P".to_string()),
    ]);
    let files = BTreeMap::from([
        ("使用テーマ".to_string(), "Theme/starseeker".to_string()),
        ("フォント".to_string(), "_font/TYPE-M".to_string()),
        ("シャッター".to_string(), "Shutter/TYPE-M".to_string()),
    ]);
    let runtime_state = LuaLoadRuntimeState {
        number_values: BTreeMap::from([(178, -1)]),
        text_values: BTreeMap::new(),
        option_values: BTreeMap::new(),
        ..LuaLoadRuntimeState::default()
    };
    let decoded = decode_beatoraja_skin_with_options_and_runtime_state(
        &skin_path,
        SkinKind::Result,
        &options,
        &files,
        &runtime_state,
    )
    .expect("decode starseeker result skin with misscount diff");

    let diff_misscount = decoded
        .document
        .value
        .iter()
        .find(|value| value.id == "Diff_Misscount")
        .expect("starseeker result should define Diff_Misscount");

    assert_eq!(diff_misscount.ref_id, 178);
    assert_eq!(diff_misscount.y, 345);
}

#[test]
fn rmz_play8_lua_skin_decodes_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/Rmz-skin/play8main.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

    assert_eq!(decoded.document.skin_type, 24);
    let note = decoded.document.note.as_ref().expect("play8 skin should define notes");
    assert_eq!(note.note.len(), 8);
    assert_eq!(note.dst.len(), 8);
}

#[test]
fn antique_play_lua_bakes_configured_keybeam_height_offset_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/mz-select/play/antique/system/play7main.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let load = |height| {
        load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState {
                offset_values: BTreeMap::from([(
                    "キービームの長さ".to_string(),
                    bmz_skin::LuaSkinOffsetValue { h: height, ..Default::default() },
                )]),
                offset_id_values: BTreeMap::from([(
                    53,
                    bmz_skin::LuaSkinOffsetValue { h: height, ..Default::default() },
                )]),
                ..Default::default()
            },
            None,
        )
        .expect("decode Antique play skin")
        .document
    };
    let keybeam_height = |document: &SkinDocument| {
        document.destination.iter().find_map(|entry| match entry {
            DestinationListEntry::Single(destination)
                if destination.id == "imgset_keybeam1" && destination.timer == Some(101) =>
            {
                destination.dst.first().and_then(|entry| match entry {
                    bmz_render::skin::SkinDstEntry::Frame(frame) => frame.h,
                    bmz_render::skin::SkinDstEntry::Conditional { .. } => None,
                })
            }
            _ => None,
        })
    };

    assert_eq!(keybeam_height(&load(0)), Some(564));
    assert_eq!(keybeam_height(&load(37)), Some(601));
}

#[test]
fn simple_play_lua_bakes_configured_note_height_offset_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/simple-play/play7.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let load_note_sizes = |height| {
        load_skin_document(
            &skin_path,
            SkinKind::Play,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &LuaLoadRuntimeState {
                offset_values: BTreeMap::from([(
                    "ノーツオフセット Notes Offset".to_string(),
                    bmz_skin::LuaSkinOffsetValue { h: height, ..Default::default() },
                )]),
                ..Default::default()
            },
            None,
        )
        .expect("decode simple-play skin")
        .document
        .note
        .expect("simple-play note definition")
        .size
    };
    let baseline = load_note_sizes(0);
    let configured = load_note_sizes(7);

    assert_eq!(baseline.len(), configured.len());
    assert!(
        baseline.iter().zip(configured).all(|(before, after)| after == before + 7),
        "simple-play note heights did not receive the configured offset"
    );
}

#[test]
fn rmz_play7_keeps_runtime_stagefile_loading_destinations_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/Rmz-skin/play7main.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let destinations = decoded.document.all_destinations(&decoded.document.enabled_options());
    let stagefile_destinations =
        destinations.iter().filter(|destination| destination.id == "-100").collect::<Vec<_>>();

    assert!(stagefile_destinations.iter().any(|destination| {
        destination.timer.is_none() && destination.op.contains(&80) && destination.op.contains(&191)
    }));
    assert!(stagefile_destinations.iter().any(|destination| {
        destination.timer == Some(40)
            && destination.op.contains(&81)
            && destination.op.contains(&191)
    }));
}

#[test]
fn rmz_play7_lanecover_green_renders_green_number_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/Rmz-skin/play7main.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let lanecover_green_value = decoded
        .document
        .value
        .iter()
        .find(|value| value.id == "lanecover-green")
        .expect("Rmz lanecover green value should decode");
    assert_eq!(
        lanecover_green_value.value_expr, "0.6*number(312)",
        "decoded value: {lanecover_green_value:?}"
    );
    let source = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "play_system_src")
        .expect("Rmz play system source should decode");
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
        total_duration_ms: 500,
        duration_green_ms: Some(300),
        lane_cover_changing: true,
        lanecover_enabled: true,
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );
    assert!(
            !items.iter().any(
                |item| matches!(item, bmz_render::skin::SkinRenderItem::Text { text, .. } if text == "FHS")
            ),
            "FHS mark should stay hidden while NHS is active"
        );
    let digit_width = 20.0;
    let source_candidates = items
        .iter()
        .filter_map(|item| {
            if let bmz_render::skin::SkinRenderItem::Image { texture, rect, uv, .. } = item
                && *texture == source.texture
            {
                Some((
                    (rect.x * 1920.0).round() as i32,
                    (rect.y * 1080.0).round() as i32,
                    (uv.x * source.size.width / digit_width).round() as i32,
                    (uv.y * source.size.height).round() as i32,
                ))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let mut digits = items
        .iter()
        .filter_map(|item| {
            if let bmz_render::skin::SkinRenderItem::Image { texture, rect, uv, .. } = item
                && *texture == source.texture
                && (rect.y * 1080.0 - 10.0).abs() < 2.0
                && (rect.x * 1920.0 - 849.0).abs() < 80.0
            {
                let digit = (uv.x * source.size.width / digit_width).round() as i32;
                Some(((rect.x * 1920.0).round() as i32, digit))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    digits.sort_by_key(|(x, _)| *x);
    let digits = digits.into_iter().map(|(_, digit)| digit).collect::<Vec<_>>();

    assert_eq!(digits, vec![3, 0, 0], "source candidates: {source_candidates:?}");

    let fhs_state = bmz_render::skin::SkinDrawState { hispeed_mode_index: 1, ..state.clone() };
    let fhs_items = decoded.document.static_render_items(
        &sources,
        &fhs_state,
        &bmz_render::skin::SkinTextState::default(),
    );
    assert!(
            fhs_items.iter().any(
                |item| matches!(item, bmz_render::skin::SkinRenderItem::Text { text, .. } if text == "FHS")
            ),
            "FHS mark should render while FHS is active"
        );
}
