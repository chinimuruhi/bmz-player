use super::*;

#[test]
fn play_judgegraph_density_uses_canvas_pixel_gap() {
    let mut document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "judgegraph": [{ "id": "density" }],
                "destination": [{ "id": "density", "dst": [{ "x": 10, "y": 10, "w": 40, "h": 10 }] }]
            }
            "#,
        )
        .unwrap();
    document.play_judge_graph_density = vec![1, 2, 3];

    let items = document.static_render_items(
        &HashMap::new(),
        &SkinDrawState::default(),
        &SkinTextState::default(),
    );
    let rects: Vec<Rect> = items
        .iter()
        .filter_map(|item| match item {
            SkinRenderItem::Rect { rect, .. } => Some(*rect),
            _ => None,
        })
        .collect();

    assert_eq!(rects.len(), 3);
    for rect in rects {
        assert!(rect.x >= 0.10);
        assert!(
            rect.x + rect.width <= 0.50 + 0.0001,
            "play judgegraph bar should stay inside the destination: {rect:?}",
        );
    }
}

#[test]
fn skin_document_evaluates_safe_gauge_draw_conditions() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "panel", "draw": "gauge() >= 75", "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "panel", "draw": "gauge() >= 50 and gauge() < 75", "dst": [{ "x": 10, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "panel", "draw": "gauge() < 25", "dst": [{ "x": 20, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "panel", "draw": "unknown() > 0", "dst": [{ "x": 30, "y": 0, "w": 10, "h": 10 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        },
    )]);

    let high = document.static_image_render_items(
        &sources,
        &SkinDrawState { elapsed_ms: 0, gauge: 80.0, ..SkinDrawState::default() },
    );
    let middle = document.static_image_render_items(
        &sources,
        &SkinDrawState { elapsed_ms: 0, gauge: 60.0, ..SkinDrawState::default() },
    );
    let low = document.static_image_render_items(
        &sources,
        &SkinDrawState { elapsed_ms: 0, gauge: 10.0, ..SkinDrawState::default() },
    );

    assert_eq!(high.len(), 1);
    assert_eq!(middle.len(), 1);
    assert_eq!(low.len(), 1);
    assert!(
        matches!(high[0], SkinRenderItem::Image { rect: Rect { x, .. }, .. } if approx_eq(x, 0.0))
    );
    assert!(
        matches!(middle[0], SkinRenderItem::Image { rect: Rect { x, .. }, .. } if approx_eq(x, 0.1))
    );
    assert!(
        matches!(low[0], SkinRenderItem::Image { rect: Rect { x, .. }, .. } if approx_eq(x, 0.2))
    );
}

#[test]
fn skin_document_evaluates_gauge_type_draw_conditions() {
    assert!(eval_skin_draw_condition(
        "gauge_type() == 4 or gauge_type() == 5",
        &SkinDrawState { gauge_type: 4, ..Default::default() }
    ));
    assert!(eval_skin_draw_condition(
        "gauge_type() == 4 or gauge_type() == 5",
        &SkinDrawState { gauge_type: 5, ..Default::default() }
    ));
    assert!(!eval_skin_draw_condition(
        "gauge_type() == 4 or gauge_type() == 5",
        &SkinDrawState { gauge_type: 2, ..Default::default() }
    ));
}

#[test]
fn peaceful_gauge_lead_glow_uses_group_part_border_and_profile() {
    let pms = SkinDrawState { gauge: 60.0, gauge_max: 120.0, gauge_type: 2, ..Default::default() };
    assert!(eval_skin_draw_condition("gauge_lead_glow(groove,12,below)", &pms));
    assert!(!eval_skin_draw_condition("gauge_lead_glow(groove,12,above)", &pms));
    assert!(!eval_skin_draw_condition("gauge_lead_glow(easy,12,below)", &pms));

    let sevenkeys =
        SkinDrawState { gauge: 80.0, gauge_max: 100.0, gauge_type: 2, ..Default::default() };
    assert!(eval_skin_draw_condition("gauge_lead_glow(groove,19,below)", &sevenkeys));
    assert!(!eval_skin_draw_condition("gauge_lead_glow(groove,19,above)", &sevenkeys));

    let class =
        SkinDrawState { gauge: 50.0, gauge_max: 100.0, gauge_type: 6, ..Default::default() };
    assert!(eval_skin_draw_condition("gauge_lead_glow(hard,12,above)", &class));
}

#[test]
fn skin_document_evaluates_gauge_auto_shift_draw_conditions() {
    assert!(eval_skin_draw_condition(
        "gauge_auto_shift() == 1",
        &SkinDrawState { gauge_auto_shift: true, ..Default::default() }
    ));
    assert!(!eval_skin_draw_condition(
        "gauge_auto_shift() == 1",
        &SkinDrawState { gauge_auto_shift: false, ..Default::default() }
    ));
    assert_eq!(select_gauge_auto_shift_index("BEST CLEAR"), 3);
    assert_eq!(select_bottom_shiftable_gauge_index("NORMAL"), 2);
    assert_eq!(
        skin_state_imageset_index(
            78,
            &SkinDrawState { select_gauge_auto_shift_index: 3, ..Default::default() }
        ),
        Some(3)
    );
    assert_eq!(
        skin_state_imageset_index(
            341,
            &SkinDrawState { select_bottom_shiftable_gauge_index: 2, ..Default::default() }
        ),
        Some(2)
    );
}

#[test]
fn static_render_items_resolve_exhard_gauge_additive_overlay() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 1920,
                "h": 1080,
                "source": [{ "id": 1, "path": "gauge.png" }],
                "image": [{ "id": "gauge-node", "src": 1, "x": 0, "y": 0, "w": 5, "h": 10 }],
                "gauge": { "id": "gauge", "nodes": ["gauge-node"], "parts": 2 },
                "destination": [
                    {
                        "id": "gauge",
                        "loop": 1200,
                        "draw": "gauge_type() == 4 or gauge_type() == 5",
                        "blend": 2,
                        "offset": 11,
                        "dst": [
                            { "time": 1200, "x": 54, "y": 151, "w": 450, "h": 28, "a": 0 },
                            { "time": 1700, "a": 80 },
                            { "time": 2000, "a": 0 }
                        ]
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 100.0, height: 100.0 },
        },
    )]);
    let mut skin_offsets = SkinOffsetValues::default();
    skin_offsets
        .set(11, crate::skin_offset::SkinOffsetValue { x: 10, y: 8, w: 4, h: 6, r: 0, a: 0 });
    let (behind, front, _) = document.static_render_items_split(
        &sources,
        &SkinDrawState { gauge_type: 4, elapsed_ms: 1700, skin_offsets, ..Default::default() },
        &SkinTextState::default(),
    );
    let items = behind.into_iter().chain(front).collect::<Vec<_>>();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|item| matches!(
        item,
        SkinRenderItem::Image {
            tint: Color { a, .. },
            blend: BlendMode::Add,
            ..
        } if (*a - 80.0 / 255.0).abs() < 0.01
    )));
    assert!(matches!(
        items[0],
        SkinRenderItem::Image {
            rect: Rect { x, y, width, height },
            ..
        } if approx_eq(x, 62.0 / 1920.0)
            && approx_eq(y, 890.0 / 1080.0)
            && approx_eq(width, 227.0 / 1920.0)
            && approx_eq(height, 34.0 / 1080.0)
    ));
}

#[test]
fn skin_document_resolves_gauge_nodes_into_parts() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "gauge.png" }],
                "image": [{ "id": "gauge-node", "src": 1, "x": 10, "y": 0, "w": 5, "h": 10 }],
                "gauge": { "id": "gauge", "nodes": ["gauge-node"], "parts": 4, "type": 0 },
                "destination": [
                    { "id": "gauge", "dst": [{ "x": 80, "y": 10, "w": -40, "h": 10 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 100.0, height: 100.0 },
        },
    )]);

    let items = document.gauge_render_items(50.0, 0, &sources).unwrap();

    assert_eq!(items.len(), 4);
    assert!(items.iter().all(|item| matches!(item, SkinRenderItem::Image { .. })));
    assert!(matches!(items[0], SkinRenderItem::Image {
                rect: Rect { x, y, width, height },
                ..
            } if approx_eq(x, 0.7)
                && approx_eq(y, 0.8)
                && approx_eq(width, 0.1)
                && approx_eq(height, 0.1)));
}

#[test]
fn skin_gauge_flickering_draws_normal_tip_overlay() {
    let mut document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "gauge.png" }],
                "image": [],
                "gauge": { "id": "gauge", "nodes": [], "parts": 4, "type": 3, "cycle": 33 },
                "destination": [
                    { "id": "gauge", "dst": [{ "x": 0, "y": 0, "w": 40, "h": 10 }] }
                ]
            }
            "#,
    )
    .unwrap();
    document.gauge.as_mut().unwrap().nodes = (0..36).map(|index| format!("node-{index}")).collect();
    document.image = (0..36)
        .map(|index| SkinImageDef {
            id: format!("node-{index}"),
            src: "1".to_string(),
            x: index,
            y: 0,
            w: 1,
            h: 1,
            divx: 1,
            divy: 1,
            timer: None,
            cycle: 0,
            len: 0,
            ref_id: 0,
            click: 0,
            act: None,
            clickable: None,
        })
        .collect();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 36.0, height: 1.0 },
        },
    )]);
    let items = document
        .static_image_render_items(
            &sources,
            &SkinDrawState {
                elapsed_ms: 8,
                gauge: 75.0,
                gauge_max: 100.0,
                gauge_border: 1.0,
                gauge_type: 2,
                ..Default::default()
            },
        )
        .into_iter()
        .filter_map(|item| match item {
            SkinRenderItem::Image { .. } => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(items.len(), 5, "4 parts + flickering tip overlay");
    let flicker = items.iter().find(|item| {
        matches!(
            item,
            SkinRenderItem::Image {
                blend: BlendMode::Normal,
                tint: Color { a, .. },
                ..
            } if *a > 0.2
        )
    });
    assert!(flicker.is_some(), "expected normal-blend tip overlay with alpha fade");
}

#[test]
fn skin_gauge_defaults_to_random_when_type_omitted() {
    let document: SkinDocument =
        serde_json::from_str(r#"{"type":0,"w":100,"h":100,"gauge":{"id":"g","nodes":[]}}"#)
            .unwrap();
    assert_eq!(document.gauge.as_ref().unwrap().gauge_type, 0);
}

#[test]
fn skin_gauge_random_animation_changes_by_cycle() {
    let gauge = SkinGaugeDef {
        id: "g".to_string(),
        nodes: Vec::new(),
        parts: 4,
        gauge_type: 0,
        range: 3,
        cycle: 33,
        starttime: 0,
        endtime: 500,
    };
    let first =
        skin_gauge_animation_index(&gauge, &SkinDrawState { elapsed_ms: 33, ..Default::default() });
    let second =
        skin_gauge_animation_index(&gauge, &SkinDrawState { elapsed_ms: 66, ..Default::default() });

    assert_ne!(first, second, "type=0 RANDOM should not stay fixed at frame 0");
    assert!((0..=3).contains(&first));
    assert!((0..=3).contains(&second));
}

#[test]
fn skin_gauge_decrease_animation_advances_forward() {
    let gauge = SkinGaugeDef {
        id: "g".to_string(),
        nodes: Vec::new(),
        parts: 4,
        gauge_type: 2,
        range: 3,
        cycle: 33,
        starttime: 0,
        endtime: 500,
    };

    assert_eq!(
        skin_gauge_animation_index(&gauge, &SkinDrawState { elapsed_ms: 33, ..Default::default() }),
        1
    );
    assert_eq!(
        skin_gauge_animation_index(&gauge, &SkinDrawState { elapsed_ms: 66, ..Default::default() }),
        2
    );
}

#[test]
fn skin_gauge_notes_count_truncates_toward_zero() {
    assert_eq!(skin_gauge_notes_count(74.9, 4, 100.0), 2);
    assert_eq!(skin_gauge_notes_count(75.0, 4, 100.0), 3);
    assert_eq!(skin_gauge_notes_count(0.0, 4, 100.0), 0);
}

#[test]
fn skin_gauge_omitted_type_has_no_flickering_overlay() {
    let mut document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "gauge.png" }],
                "image": [],
                "gauge": { "id": "gauge", "nodes": [], "parts": 4 },
                "destination": [
                    { "id": "gauge", "dst": [{ "x": 0, "y": 0, "w": 40, "h": 10 }] }
                ]
            }
            "#,
    )
    .unwrap();
    document.gauge.as_mut().unwrap().nodes = (0..36).map(|index| format!("node-{index}")).collect();
    document.image = (0..36)
        .map(|index| SkinImageDef {
            id: format!("node-{index}"),
            src: "1".to_string(),
            x: index,
            y: 0,
            w: 1,
            h: 1,
            divx: 1,
            divy: 1,
            timer: None,
            cycle: 0,
            len: 0,
            ref_id: 0,
            click: 0,
            act: None,
            clickable: None,
        })
        .collect();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 36.0, height: 1.0 },
        },
    )]);
    let items = document
        .static_image_render_items(
            &sources,
            &SkinDrawState {
                elapsed_ms: 8,
                gauge: 75.0,
                gauge_max: 100.0,
                gauge_border: 1.0,
                gauge_type: 2,
                ..Default::default()
            },
        )
        .into_iter()
        .filter_map(|item| match item {
            SkinRenderItem::Image { .. } => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(items.len(), 4, "type=0 should not add flickering tip overlay");
}

#[test]
fn static_render_items_resolve_gauge_in_destination_order() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "gauge.png" }],
                "image": [
                    { "id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "gauge-node", "src": 1, "x": 10, "y": 0, "w": 5, "h": 10 }
                ],
                "gauge": { "id": "gauge", "nodes": ["gauge-node"], "parts": 4, "type": 0 },
                "destination": [
                    { "id": "panel", "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "gauge", "timer": 2, "dst": [{ "x": 80, "y": 10, "w": -40, "h": 10 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 100.0, height: 100.0 },
        },
    )]);

    let inactive = document.static_image_render_items(
        &sources,
        &SkinDrawState {
            elapsed_ms: 500,
            gauge: 50.0,
            gauge_max: 100.0,
            fadeout_ms: None,
            ..Default::default()
        },
    );
    let active = document.static_image_render_items(
        &sources,
        &SkinDrawState {
            elapsed_ms: 500,
            gauge: 50.0,
            gauge_max: 100.0,
            fadeout_ms: Some(250),
            ..Default::default()
        },
    );

    assert_eq!(inactive.len(), 1);
    // beatoraja は全 `parts` 分のセルを描画する (埋まり具合でスプライトだけ変える)。
    assert_eq!(active.len(), 5);
    assert!(active[1..].iter().all(|item| matches!(item, SkinRenderItem::Image { .. })));
}

#[test]
fn best_and_target_scores_follow_note_progress() {
    let state = SkinDrawState {
        ex_score: 450,
        total_notes: 1000,
        past_notes: 250,
        best_ex_score: Some(1800),
        target_ex_score: Some(1600),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(150, &state), Some(450));
    assert_eq!(skin_state_number(170, &state), Some(450));
    assert_eq!(skin_state_number(121, &state), Some(400));
    assert_eq!(skin_state_number(151, &state), Some(400));
    assert_eq!(skin_state_number(152, &state), Some(0));
    assert_eq!(skin_state_number(172, &state), Some(0));
    assert_eq!(skin_state_number(153, &state), Some(50));
}

#[test]
fn target_score_timer_and_ops_follow_current_ex_score() {
    let below = SkinDrawState {
        elapsed_ms: 1234,
        ex_score: 1599,
        total_notes: 900,
        target_ex_score: Some(1600),
        ..SkinDrawState::default()
    };
    let reached = SkinDrawState { ex_score: 1600, ..below.clone() };
    let updated = SkinDrawState { ex_score: 1601, ..below.clone() };

    assert_eq!(skin_timer_elapsed_ms(Some(352), &below), None);
    assert_eq!(skin_timer_elapsed_ms(Some(352), &reached), Some(1234));
    assert!(test_skin_op(1336, &[], &reached));
    assert!(!test_skin_op(336, &[], &reached));
    assert!(test_skin_op(336, &[], &updated));
}

#[test]
fn gauge_timers_use_state_elapsed_values() {
    let inactive = SkinDrawState::default();
    assert_eq!(skin_timer_elapsed_ms(Some(42), &inactive), None);
    assert_eq!(skin_timer_elapsed_ms(Some(43), &inactive), None);
    assert_eq!(skin_timer_elapsed_ms(Some(44), &inactive), None);
    assert_eq!(skin_timer_elapsed_ms(Some(45), &inactive), None);

    let active = SkinDrawState {
        gauge_increase_ms: Some(75),
        gauge_increase_2p_ms: Some(125),
        gauge_max_ms: Some(1_700),
        gauge_max_2p_ms: Some(1_900),
        ..SkinDrawState::default()
    };
    assert_eq!(skin_timer_elapsed_ms(Some(42), &active), Some(75));
    assert_eq!(skin_timer_elapsed_ms(Some(43), &active), Some(125));
    assert_eq!(skin_timer_elapsed_ms(Some(44), &active), Some(1_700));
    assert_eq!(skin_timer_elapsed_ms(Some(45), &active), Some(1_900));
}

#[test]
fn graph_renders_vertical_bar_proportional_to_score() {
    // BARGRAPH_SCORERATE (110): ex_score / (total_notes * 2)
    // total_notes=100, ex_score=100 → value=0.5
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 1280, "h": 720,
                "source": [{ "id": "bar-src", "path": "bar.png" }],
                "graph": [{ "id": "score-bar", "src": "bar-src", "x": 0, "y": 0, "w": 100, "h": 200, "type": 110 }],
                "destination": [
                    { "id": "score-bar", "dst": [{ "time": 0, "x": 0, "y": 0, "w": 100, "h": 480 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let sources = mock_source("bar-src", 100.0, 200.0);
    let state = SkinDrawState { ex_score: 100, total_notes: 100, ..SkinDrawState::default() };
    let items = document.static_image_render_items(&sources, &state);

    assert_eq!(items.len(), 1, "expected one graph bar");
    let SkinRenderItem::Image { rect, uv, .. } = &items[0] else { panic!() };
    // value=0.5 → height = 480/720 * 0.5; destination bottom is y=0 in beatoraja space.
    let dst_h = 480.0 / 720.0;
    assert!(approx_eq(rect.height, dst_h * 0.5), "bar height should be half: got {}", rect.height);
    assert!(
        approx_eq(rect.y, 1.0 - dst_h * 0.5),
        "bar y should start at half-height: got {}",
        rect.y
    );
    // UV should also be clipped to bottom half
    assert!(approx_eq(uv.height, 0.5), "uv height should be 0.5, got {}", uv.height);
    assert!(approx_eq(uv.y, 0.5), "uv y should be 0.5, got {}", uv.y);
}

#[test]
fn graph_renders_current_score_rate_against_past_notes() {
    // BARGRAPH_SCORERATE_FINAL (111): ex_score / (past_notes * 2)
    // total_notes=1000, past_notes=9, ex_score=18 → current rate is 100%.
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 1280, "h": 720,
                "source": [{ "id": "bar-src", "path": "bar.png" }],
                "graph": [{ "id": "score-bar", "src": "bar-src", "x": 0, "y": 0, "w": 100, "h": 200, "type": 111 }],
                "destination": [
                    { "id": "score-bar", "dst": [{ "time": 0, "x": 0, "y": 0, "w": 100, "h": 480 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let sources = mock_source("bar-src", 100.0, 200.0);
    let state = SkinDrawState {
        ex_score: 18,
        total_notes: 1000,
        past_notes: 9,
        ..SkinDrawState::default()
    };
    let items = document.static_image_render_items(&sources, &state);

    assert_eq!(items.len(), 1, "expected one graph bar");
    let SkinRenderItem::Image { rect, uv, .. } = &items[0] else { panic!() };
    let dst_h = 480.0 / 720.0;
    assert!(approx_eq(rect.height, dst_h), "bar height should be full: got {}", rect.height);
    assert!(approx_eq(rect.y, 1.0 - dst_h), "bar y should start at top: got {}", rect.y);
    assert!(approx_eq(uv.height, 1.0), "uv height should be full, got {}", uv.height);
    assert!(approx_eq(uv.y, 0.0), "uv y should start at top, got {}", uv.y);
}

#[test]
fn graph_renders_horizontal_bar_for_load_progress() {
    // BARGRAPH_LOAD_PROGRESS (102): always 1.0
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 1280, "h": 720,
                "source": [{ "id": "bar-src", "path": "bar.png" }],
                "graph": [{ "id": "load-bar", "src": "bar-src", "x": 0, "y": 0, "w": 100, "h": 8, "angle": 0, "type": 102 }],
                "destination": [
                    { "id": "load-bar", "dst": [{ "time": 0, "x": 0, "y": 0, "w": 640, "h": 8 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let sources = mock_source("bar-src", 100.0, 8.0);
    let state = SkinDrawState::default();
    let items = document.static_image_render_items(&sources, &state);

    assert_eq!(items.len(), 1, "expected one load bar");
    let SkinRenderItem::Image { rect, .. } = &items[0] else { panic!() };
    // value=1.0 → full width = 640/1280 = 0.5
    assert!(approx_eq(rect.width, 640.0 / 1280.0), "full load bar width: got {}", rect.width);
}

#[test]
fn lua_graph_with_negative_width_fills_leftwards_from_destination_x() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 1280, "h": 720,
                "source": [{ "id": "bar-src", "path": "bar.png" }],
                "graph": [{
                    "id": "pg_fast", "src": "bar-src", "x": 0, "y": 0, "w": 1, "h": 1, "angle": 0,
                    "value_expr": "(number(410))/(number(410)+number(411))"
                }],
                "destination": [
                    { "id": "pg_fast", "dst": [{ "time": 0, "x": 640, "y": 0, "w": -640, "h": 8 }] }
                ]
            }
            "#,
    )
    .unwrap();

    let sources = mock_source("bar-src", 1.0, 1.0);
    let state = SkinDrawState {
        fast_slow_counts: Some(crate::snapshot::FastSlowJudgeCounts {
            fast_pgreat: 1,
            slow_pgreat: 3,
            ..crate::snapshot::FastSlowJudgeCounts::default()
        }),
        ..SkinDrawState::default()
    };
    assert!(
        approx_eq(graph_raw_value(&document.graph[0], &state), 0.25),
        "WMII graph expression must preserve the FAST ratio"
    );
    let items = document.static_image_render_items(&sources, &state);

    assert_eq!(items.len(), 1);
    let SkinRenderItem::Image { rect, uv, .. } = &items[0] else { panic!() };
    assert!(approx_eq(rect.width, 0.125), "25% of half-canvas width: got rect {rect:?}, uv {uv:?}");
    assert!(
        approx_eq(rect.x, 0.375),
        "negative width must remain anchored at destination x: got {}",
        rect.x
    );
    assert!(approx_eq(uv.width, 0.25), "source UV should be clipped to 25%: got {}", uv.width);
}

#[test]
fn graph_music_progress_uses_play_progress() {
    // BARGRAPH_MUSIC_PROGRESS (101): play_progress=0.75 → bar is 75% full
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 1280, "h": 720,
                "source": [{ "id": "bar-src", "path": "bar.png" }],
                "graph": [{ "id": "music-bar", "src": "bar-src", "x": 0, "y": 0, "w": 100, "h": 8, "angle": 0, "type": 101 }],
                "destination": [
                    { "id": "music-bar", "dst": [{ "time": 0, "x": 0, "y": 0, "w": 1280, "h": 8 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let sources = mock_source("bar-src", 100.0, 8.0);
    let state = SkinDrawState { play_progress: 0.75, ..SkinDrawState::default() };
    let items = document.static_image_render_items(&sources, &state);

    assert_eq!(items.len(), 1, "expected one music bar");
    let SkinRenderItem::Image { rect, uv, .. } = &items[0] else { panic!() };
    // value=0.75 → width = 1280/1280 * 0.75 = 0.75
    assert!(approx_eq(rect.width, 0.75), "music bar width should be 0.75, got {}", rect.width);
    assert!(approx_eq(uv.width, 0.75), "music bar uv.width should be 0.75, got {}", uv.width);
}

#[test]
fn graph_rate_pgreat_uses_judge_count_over_past_notes() {
    // BARGRAPH_RATE_PGREAT (140): pgreat / past_notes
    // pgreat=60, past_notes=100 → 0.6
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 1280, "h": 720,
                "source": [{ "id": "bar-src", "path": "bar.png" }],
                "graph": [{ "id": "pg-bar", "src": "bar-src", "x": 0, "y": 0, "w": 100, "h": 8, "angle": 0, "type": 140 }],
                "destination": [
                    { "id": "pg-bar", "dst": [{ "time": 0, "x": 0, "y": 0, "w": 1000, "h": 8 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let sources = mock_source("bar-src", 100.0, 8.0);
    let state = SkinDrawState {
        judge_counts: DisplayJudgeCounts { pgreat: 60, great: 30, ..Default::default() },
        past_notes: 100,
        total_notes: 200,
        ..SkinDrawState::default()
    };
    let items = document.static_image_render_items(&sources, &state);

    assert_eq!(items.len(), 1);
    let SkinRenderItem::Image { rect, .. } = &items[0] else { panic!() };
    // value=0.6, dst_width = 1000/1280
    assert!(approx_eq(rect.width, 1000.0 / 1280.0 * 0.6), "pg bar width: got {}", rect.width);
}

#[test]
fn skin_value_number_evaluates_peaceful_play_gauge_values() {
    let state = SkinDrawState { gauge: 78.75, gauge_max: 120.0, ..Default::default() };
    let value = |expr: &str| SkinValueDef { value_expr: expr.to_string(), ..Default::default() };

    assert_eq!(skin_value_number(&value(SKIN_EXPR_GAUGE_PERCENT_INTEGER), &state), Some(65));
    assert_eq!(skin_value_number(&value(SKIN_EXPR_GAUGE_PERCENT_FRACTION), &state), Some(62));
    assert_eq!(skin_value_number(&value(SKIN_EXPR_GAUGE_AMOUNT_INTEGER), &state), Some(78));
    assert_eq!(skin_value_number(&value(SKIN_EXPR_GAUGE_AMOUNT_FRACTION), &state), Some(75));
}

#[test]
fn score_rate_parts_matches_beatoraja_score_data_property() {
    let (integer, afterdot) = score_rate_parts(3948, 2006);
    assert_eq!(integer, 98);
    assert_eq!(afterdot, 40);
}

#[test]
fn current_score_rate_refs_use_past_notes() {
    let state = SkinDrawState {
        ex_score: 18,
        total_notes: 1000,
        past_notes: 9,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(102, &state), Some(100));
    assert_eq!(skin_state_number(103, &state), Some(0));
    assert_eq!(skin_state_number(115, &state), Some(0));
    assert_eq!(skin_state_number(116, &state), Some(90));
}

#[test]
fn current_score_rate_starts_at_full_rate_before_first_note() {
    let state = SkinDrawState { total_notes: 1000, ..SkinDrawState::default() };

    assert_eq!(skin_state_number(102, &state), Some(100));
    assert_eq!(skin_state_number(103, &state), Some(0));
    assert!((graph_value(111, &state) - 1.0).abs() < 1e-5);
}

#[test]
fn graph_fill_dimensions_scales_lua_chart_graph_by_dst_multiplier() {
    let graph = SkinGraphDef {
        id: "default_chart_peak".to_string(),
        src: "graph".to_string(),
        value_expr: "4.800000000000001*number(360)".to_string(),
        min: 0,
        max: 320,
        x: 0,
        y: 0,
        w: 1,
        h: 14,
        divx: 1,
        divy: 1,
        timer: None,
        cycle: 0,
        angle: 0,
        graph_type: 0,
        is_ref_num: false,
    };
    let state = SkinDrawState {
        select_screen: true,
        select_chart_peak_density: 12.5,
        ..SkinDrawState::default()
    };
    let (fill, uv) = graph_fill_dimensions(&graph, &state);
    assert!((fill - 57.6).abs() < 0.01);
    assert!((uv - 57.6 / 320.0).abs() < 1e-5);
}

#[test]
fn skin_state_number_best_and_target_score() {
    let state = SkinDrawState {
        best_ex_score: Some(1500),
        target_ex_score: Some(800),
        ..SkinDrawState::default()
    };
    // NUMBER_HIGHSCORE (150)
    assert_eq!(skin_state_number(150, &state), Some(1500));
    // NUMBER_TARGET_SCORE (121)
    assert_eq!(skin_state_number(121, &state), Some(800));
    let ghost_projected = SkinDrawState {
        best_ex_score: Some(1500),
        projected_best_ex_score: Some(321),
        ex_score: 400,
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(150, &ghost_projected), Some(321));
    assert_eq!(skin_state_number(152, &ghost_projected), Some(79));
    // When None → None
    let no_scores = SkinDrawState::default();
    assert_eq!(skin_state_number(150, &no_scores), None);
    assert_eq!(skin_state_number(121, &no_scores), None);
}

#[test]
fn graph_value_bestscorerate_fills_bar_proportionally() {
    // BARGRAPH_BESTSCORERATE (113): best / (total_notes * 2)
    // best=800, total=500 → 800/1000 = 0.8
    let state =
        SkinDrawState { best_ex_score: Some(800), total_notes: 500, ..SkinDrawState::default() };
    let v = graph_value(113, &state);
    assert!((v - 0.8).abs() < 1e-5, "best score rate: expected 0.8, got {v}");
}

#[test]
fn graph_value_targetscorerate_fills_bar_proportionally() {
    // BARGRAPH_TARGETSCORERATE (115): target / (total_notes * 2)
    // target=600, total=600 → 600/1200 = 0.5
    let state =
        SkinDrawState { target_ex_score: Some(600), total_notes: 600, ..SkinDrawState::default() };
    let v = graph_value(115, &state);
    assert!((v - 0.5).abs() < 1e-5, "target score rate: expected 0.5, got {v}");
}

#[test]
fn graph_value_bestscorerate_now_scales_with_past_notes() {
    // BARGRAPH_BESTSCORERATE_NOW (112): best * past / (total^2 * 2)
    // best=160 (80% of max 200), past=50, total=100
    // → 160 * 50 / (100^2 * 2) = 8000 / 20000 = 0.4
    // = best_rate(0.8) × play_fraction(0.5) = 0.4
    let state = SkinDrawState {
        best_ex_score: Some(160),
        past_notes: 50,
        total_notes: 100,
        ..SkinDrawState::default()
    };
    let v = graph_value(112, &state);
    assert!((v - 0.4).abs() < 1e-4, "best now rate: expected 0.4, got {v}");
}

#[test]
fn graph_value_bestscorerate_now_uses_projected_best_score() {
    let state = SkinDrawState {
        best_ex_score: Some(160),
        projected_best_ex_score: Some(100),
        past_notes: 50,
        total_notes: 100,
        ..SkinDrawState::default()
    };

    let v = graph_value(112, &state);

    assert!((v - 0.5).abs() < 1e-4, "best ghost now rate: expected 0.5, got {v}");
}

#[test]
fn graph_value_returns_zero_when_no_best_score() {
    let state = SkinDrawState { total_notes: 100, ..SkinDrawState::default() };
    assert_eq!(graph_value(113, &state), 0.0);
    assert_eq!(graph_value(115, &state), 0.0);
}
