use super::*;

#[test]
fn select_destination_judgegraph_renders_selected_chart_distribution() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "judgegraph": [{ "id": "density", "delay": 0, "backTexOff": 1, "noGap": 1, "noGapX": 1 }],
                "destination": [{ "id": "density", "dst": [{ "x": 0, "y": 0, "w": 40, "h": 10 }] }]
            }
            "#,
        )
        .unwrap();
    let snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            kind: SelectRowKind::Song,
            in_library: true,
            chart_distribution: vec![
                crate::scene::SelectChartDistributionSecond { key_taps: 4, ..Default::default() },
                crate::scene::SelectChartDistributionSecond {
                    scratch_taps: 2,
                    ..Default::default()
                },
            ],
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&HashMap::new(), &snapshot);

    assert_eq!(items.iter().filter(|item| matches!(item, SkinRenderItem::Rect { .. })).count(), 2);
}

#[test]
fn select_destination_bpmgraph_renders_selected_chart_segments() {
    let document: SkinDocument = serde_json::from_str(
            r##"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "bpmgraph": [{ "id": "bpm", "lineWidth": 2, "mainBPMColor": "#ff0000", "otherBPMColor": "#00ff00" }],
                "destination": [{ "id": "bpm", "dst": [{ "x": 0, "y": 0, "w": 40, "h": 20 }] }]
            }
            "##,
        )
        .unwrap();
    let snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            kind: SelectRowKind::Song,
            in_library: true,
            min_bpm: 100.0,
            max_bpm: 200.0,
            chart_main_bpm: 100.0,
            chart_bpm_graph_segments: vec![
                crate::chart_graph::BpmGraphSegment {
                    start_ratio: 0.0,
                    end_ratio: 0.5,
                    bpm: 100.0,
                    is_stop: false,
                },
                crate::chart_graph::BpmGraphSegment {
                    start_ratio: 0.5,
                    end_ratio: 1.0,
                    bpm: 200.0,
                    is_stop: false,
                },
            ],
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&HashMap::new(), &snapshot);

    // 横線2本 + BPM変化縦線1本 = 3
    assert_eq!(items.iter().filter(|item| matches!(item, SkinRenderItem::Rect { .. })).count(), 3);
}

#[test]
fn select_songlist_bpmgraph_renders_row_segments() {
    let document: SkinDocument = serde_json::from_str(
            r##"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "bpmgraph": [{ "id": "bpm", "lineWidth": 2, "mainBPMColor": "#ff0000", "otherBPMColor": "#00ff00" }],
                "songlist": {
                    "id": "list",
                    "center": 0,
                    "liston": [{ "id": "row", "dst": [{ "x": 0, "y": 0, "w": 100, "h": 100 }] }],
                    "listoff": [{ "id": "row", "dst": [{ "x": 0, "y": 0, "w": 100, "h": 100 }] }],
                    "bpmgraph": [{ "id": "bpm", "dst": [{ "x": 0, "y": 0, "w": 40, "h": 20 }] }]
                },
                "destination": [{ "id": "list" }]
            }
            "##,
        )
        .unwrap();
    let snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            kind: SelectRowKind::Song,
            in_library: true,
            min_bpm: 100.0,
            max_bpm: 200.0,
            chart_main_bpm: 100.0,
            chart_bpm_graph_segments: vec![
                crate::chart_graph::BpmGraphSegment {
                    start_ratio: 0.0,
                    end_ratio: 0.5,
                    bpm: 100.0,
                    is_stop: false,
                },
                crate::chart_graph::BpmGraphSegment {
                    start_ratio: 0.5,
                    end_ratio: 1.0,
                    bpm: 200.0,
                    is_stop: false,
                },
            ],
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&HashMap::new(), &snapshot);

    // 横線2本 + BPM変化縦線1本 = 3
    assert_eq!(items.iter().filter(|item| matches!(item, SkinRenderItem::Rect { .. })).count(), 3);
}

#[test]
fn select_option_panel_three_renders_judge_timing_value() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "value": [{ "id": "judgetiming", "src": 1, "x": 0, "y": 0, "w": 120, "h": 20, "divx": 12, "divy": 2, "digit": 3, "ref": 12 }],
                "destination": [{ "id": "judgetiming", "timer": 23, "op": [23], "dst": [{ "x": 40, "y": 0, "w": 10, "h": 10 }] }]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 120.0, 40.0);
    let snapshot = SelectSnapshot {
        option_panel: 3,
        option_panel_time: TimeUs(100_000),
        judge_timing_offset_ms: -12,
        note_display_duration_ms: Some(300),
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);

    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.4)
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, uv, .. }
            if approx_eq(rect.x, 0.4) && approx_eq(uv.x, 11.0 / 12.0) && uv.y > 0.0
    )));
}

#[test]
fn select_option_panel_text_uses_snapshot_option_strings() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "text": [
                    { "id": "bmz_select_gauge", "size": 10 },
                    { "id": "bmz_select_target", "size": 10 },
                    { "id": "bmz_select_judge_timing_auto_adjust", "size": 10 }
                ],
                "destination": [
                    { "id": "bmz_select_gauge", "op": [23], "dst": [{ "x": 0, "y": 0, "w": 50, "h": 10 }] },
                    { "id": "bmz_select_target", "op": [23], "dst": [{ "x": 0, "y": 10, "w": 50, "h": 10 }] },
                    { "id": "bmz_select_judge_timing_auto_adjust", "op": [23], "dst": [{ "x": 0, "y": 20, "w": 50, "h": 10 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let snapshot = SelectSnapshot {
        option_panel: 3,
        gauge: "HARD".to_string(),
        target: "AAA".to_string(),
        judge_timing_auto_adjust: true,
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&HashMap::new(), &snapshot);

    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Text {
            text, ..
        } if text == "HARD")));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Text {
            text, ..
        } if text == "RANK AAA")));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Text {
            text, ..
        } if text == "ON")));
}

#[test]
fn select_draw_state_uses_select_judge_timing_offset() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 5 }"#).unwrap();
    let snapshot = SelectSnapshot {
        option_panel: 3,
        judge_timing_offset_ms: -12,
        note_display_duration_ms: Some(300),
        ..SelectSnapshot::default()
    };

    let (state, _) = document.select_draw_state(&snapshot, None);

    assert_eq!(skin_state_number(12, &state), Some(-12));
}

#[test]
fn select_snapshot_custom_offset_adjusts_destination_geometry_and_alpha() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100, "h": 100,
                "source": [{ "id": "src", "path": "a.png" }],
                "image": [{ "id": "img", "src": "src", "w": 10, "h": 10 }],
                "destination": [
                    { "id": "img", "offset": 42, "dst": [
                        { "time": 0, "x": 10, "y": 20, "w": 30, "h": 40, "a": 200 }
                    ]}
                ]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("src", 10.0, 10.0);
    let mut skin_offsets = SkinOffsetValues::default();
    skin_offsets
        .set(42, crate::skin_offset::SkinOffsetValue { x: 6, y: 8, w: 10, h: 12, r: 0, a: -50 });

    let items = document.select_render_items(
        &sources,
        &SelectSnapshot { skin_offsets, ..SelectSnapshot::default() },
    );

    assert_eq!(items.len(), 1);
    let SkinRenderItem::Image { rect, tint, .. } = &items[0] else { panic!() };
    assert!(approx_eq(rect.x, 0.11));
    assert!(approx_eq(rect.y, 0.26));
    assert!(approx_eq(rect.width, 0.4));
    assert!(approx_eq(rect.height, 0.52));
    assert!(approx_eq(tint.a, 150.0 / 255.0));
}

#[test]
fn select_draw_state_uses_application_operating_time() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 5 }"#).unwrap();
    let snapshot = SelectSnapshot { operating_time_ms: 90_061_234, ..SelectSnapshot::default() };

    let (state, _) = document.select_draw_state(&snapshot, None);

    assert_eq!(skin_state_number(27, &state), Some(25));
    assert_eq!(skin_state_number(28, &state), Some(1));
    assert_eq!(skin_state_number(29, &state), Some(1));
}

#[test]
fn select_draw_state_maps_hispeed_and_green_number() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 5 }"#).unwrap();
    let snapshot = SelectSnapshot {
        hispeed: 3.25,
        note_display_duration_ms: Some(280),
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            kind: SelectRowKind::Song,
            in_library: true,
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let (state, _) = document.select_draw_state(&snapshot, None);

    assert_eq!(skin_state_number(310, &state), Some(3));
    assert_eq!(skin_state_number(311, &state), Some(25));
    assert_eq!(skin_state_number(312, &state), Some(467));
    assert_eq!(skin_state_number(313, &state), Some(280));
}

#[test]
fn select_draw_state_maps_extended_option_refs() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 5 }"#).unwrap();
    let snapshot = SelectSnapshot {
        arrange: "RANDOM".to_string(),
        arrange_2p: "SPIRAL".to_string(),
        double_option: "BATTLE AS".to_string(),
        hs_fix: "MAIN BPM".to_string(),
        hispeed_auto_adjust: true,
        note_display_duration_ms: Some(300),
        ..SelectSnapshot::default()
    };

    let (state, _) = document.select_draw_state(&snapshot, None);

    assert_eq!(skin_state_number(42, &state), Some(2));
    assert_eq!(skin_state_number(43, &state), Some(5));
    assert_eq!(skin_state_number(54, &state), Some(3));
    assert_eq!(skin_state_number(55, &state), Some(3));
    assert_eq!(skin_state_number(342, &state), Some(1));
}

#[test]
fn select_draw_state_exposes_planned_random_lane_pattern() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 5 }"#).unwrap();
    let mut pattern = (0..LANE_COUNT as u8).collect::<Vec<_>>();
    pattern[Lane::Key1.index()] = Lane::Key7.index() as u8;
    let snapshot = SelectSnapshot {
        arrange: "NORMAL".to_string(),
        lane_shuffle_pattern: pattern,
        rows: vec![SelectRowSnapshot {
            index: 0,
            kind: SelectRowKind::Song,
            chart_key_mode: Some(KeyMode::K7),
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let (state, _) = document.select_draw_state(&snapshot, None);

    assert_eq!(skin_state_number(42, &state), Some(0));
    assert_eq!(skin_state_number(450, &state), Some(7));
}

#[test]
fn select_songlist_judgegraph_honors_delay_backtexoff_and_type() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "judgegraph": [
                    { "id": "density", "type": 0, "delay": 1000, "backTexOff": 1, "noGap": 1, "noGapX": 1 },
                    { "id": "judge", "type": 1, "delay": 0 }
                ],
                "songlist": {
                    "id": "songlist",
                    "center": 0,
                    "liston": [{ "id": "row", "dst": [{ "x": 0, "y": 0, "w": 100, "h": 20 }] }],
                    "listoff": [{ "id": "row", "dst": [{ "x": 0, "y": 0, "w": 100, "h": 20 }] }],
                    "judgegraph": [
                        { "id": "density", "dst": [{ "x": 0, "y": 0, "w": 100, "h": 20 }] },
                        { "id": "judge", "dst": [{ "x": 0, "y": 20, "w": 100, "h": 20 }] }
                    ]
                },
                "destination": [{ "id": "songlist" }]
            }
            "#,
        )
        .unwrap();
    let row = SelectRowSnapshot {
        index: 0,
        kind: SelectRowKind::Song,
        in_library: true,
        chart_distribution: vec![
            crate::scene::SelectChartDistributionSecond { key_taps: 4, ..Default::default() },
            crate::scene::SelectChartDistributionSecond { key_taps: 4, ..Default::default() },
        ],
        ..SelectRowSnapshot::default()
    };
    let snapshot = SelectSnapshot {
        time: TimeUs(500_000),
        selected_index: 0,
        rows: vec![row],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&HashMap::new(), &snapshot);

    assert_eq!(items.iter().filter(|item| matches!(item, SkinRenderItem::Rect { .. })).count(), 1);
    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Rect { rect, .. } if approx_eq(rect.x, 0.0) && approx_eq(rect.width, 0.5)
    )));
}

#[test]
fn select_context_exposes_chart_image_sources_to_skin_document() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "image": [
                    { "id": "stage", "src": 100, "x": 0, "y": 0, "w": 40, "h": 20 },
                    { "id": "back", "src": 101, "x": 0, "y": 0, "w": 20, "h": 10 },
                    { "id": "banner", "src": 102, "x": 0, "y": 0, "w": 30, "h": 12 }
                ],
                "destination": [
                    { "id": "stage", "op": [191], "dst": [{ "x": 0, "y": 0, "w": 40, "h": 20 }] },
                    { "id": "back", "op": [195], "dst": [{ "x": 40, "y": 0, "w": 20, "h": 10 }] },
                    { "id": "banner", "op": [193], "dst": [{ "x": 60, "y": 0, "w": 30, "h": 12 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let context = SkinContext::from_manifest_and_document(default_skin_manifest(), document, []);
    let snapshot = SelectSnapshot {
        stage_background: true,
        stage_image_size: Some(SkinImageSize { width: 400.0, height: 200.0 }),
        backbmp_image: true,
        backbmp_image_size: Some(SkinImageSize { width: 200.0, height: 100.0 }),
        banner_image: true,
        banner_image_size: Some(SkinImageSize { width: 300.0, height: 120.0 }),
        ..SelectSnapshot::default()
    };

    let items = context.select_document_items(&snapshot);

    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { texture, .. } if *texture == SkinTextureId(SELECT_STAGE_TEXTURE.0)
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { texture, .. } if *texture == SkinTextureId(PLAY_BACKBMP_TEXTURE.0)
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { texture, .. } if *texture == SkinTextureId(SELECT_BANNER_TEXTURE.0)
    )));
}

#[test]
fn select_destination_negative_image_id_renders_runtime_stagefile_source() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "destination": [
                    { "id": "-100", "op": [191], "dst": [{ "x": 0, "y": 0, "w": 40, "h": 20 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let context = SkinContext::from_manifest_and_document(default_skin_manifest(), document, []);
    let snapshot = SelectSnapshot {
        stage_background: true,
        stage_image_size: Some(SkinImageSize { width: 400.0, height: 200.0 }),
        ..SelectSnapshot::default()
    };

    let items = context.select_document_items(&snapshot);

    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image {
            texture,
            source_size: Some(SkinImageSize { width: 400.0, height: 200.0 }),
            ..
        } if *texture == SkinTextureId(SELECT_STAGE_TEXTURE.0)
    )));

    let items_without_stage = context.select_document_items(&SelectSnapshot::default());
    assert!(!items_without_stage.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { texture, .. }
            if *texture == SkinTextureId(SELECT_STAGE_TEXTURE.0)
    )));
}

#[test]
fn select_chart_image_ops_follow_loaded_runtime_images() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "image": [
                    { "id": "no_stage", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "stage", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "no_back", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "back", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }
                ],
                "destination": [
                    { "id": "no_stage", "op": [190], "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "stage", "op": [191], "dst": [{ "x": 10, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "no_back", "op": [194], "dst": [{ "x": 20, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "back", "op": [195], "dst": [{ "x": 30, "y": 0, "w": 10, "h": 10 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let context = SkinContext::from_manifest_and_document(
        default_skin_manifest(),
        document,
        [SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(1),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        }],
    );

    let missing = context.select_document_items(&SelectSnapshot::default());
    assert!(missing.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.0)
    )));
    assert!(missing.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.2)
    )));
    assert!(!missing.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.1) || approx_eq(rect.x, 0.3)
    )));

    let loaded = context.select_document_items(&SelectSnapshot {
        stage_background: true,
        backbmp_image: true,
        ..SelectSnapshot::default()
    });
    assert!(loaded.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.1)
    )));
    assert!(loaded.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.3)
    )));
    assert!(!loaded.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.0) || approx_eq(rect.x, 0.2)
    )));
}

#[test]
fn select_click_hit_resolves_destination_act_for_dynamic_text() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "text": [
                    { "id": "bmz_select_arrange", "font": "default", "size": 18 },
                    { "id": "disabled", "font": "default", "size": 18, "constantText": "OFF" }
                ],
                "destination": [
                    {
                        "id": "bmz_select_arrange",
                        "act": 42,
                        "click": 2,
                        "dst": [{ "x": 10, "y": 20, "w": 30, "h": 10 }]
                    },
                    {
                        "id": "disabled",
                        "act": 43,
                        "clickable": false,
                        "dst": [{ "x": 50, "y": 20, "w": 30, "h": 10 }]
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let snapshot = SelectSnapshot { arrange: "MF-RANDOM".to_string(), ..SelectSnapshot::default() };

    assert!(document.select_render_items(&HashMap::new(), &snapshot).iter().any(|item| matches!(
        item,
        SkinRenderItem::Text { text, .. } if text == "MF-RANDOM"
    )));
    let hit = document
        .select_click_hit(
            &HashMap::new(),
            &snapshot,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            0.2,
            0.75,
        )
        .unwrap();

    assert_eq!(hit.target, SkinClickTarget::Event { event_id: 42, click: 2 });
    assert_eq!(hit.rect, Rect { x: 0.1, y: 0.7, width: 0.3, height: 0.1 });
    assert!(
        document
            .select_click_hit(
                &HashMap::new(),
                &snapshot,
                &crate::select_settings_dest::SelectSettingsDestIndex::default(),
                0.6,
                0.75,
            )
            .is_none()
    );
}
