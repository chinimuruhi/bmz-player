use super::*;

#[test]
fn static_image_cache_does_not_freeze_destination_animation() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 7,
                "w": 100,
                "h": 100,
                "source": [{ "id": "src", "path": "parts.png" }],
                "image": [
                    { "id": "background", "src": "src", "w": 10, "h": 10 },
                    { "id": "animated", "src": "src", "w": 10, "h": 10 },
                    { "id": "delayed", "src": "src", "w": 10, "h": 10 }
                ],
                "destination": [
                    { "id": "background", "dst": [
                        { "time": 0, "x": 10, "y": 0, "w": 10, "h": 10 }
                    ]},
                    { "id": "animated", "loop": 100, "dst": [
                        { "time": 0, "x": 20, "y": 0, "w": 10, "h": 10 },
                        { "time": 100, "x": 60, "y": 0, "w": 10, "h": 10 }
                    ]},
                    { "id": "delayed", "loop": 50, "dst": [
                        { "time": 50, "x": 80, "y": 0, "w": 10, "h": 10 }
                    ]}
                ]
            }
            "#,
    )
    .unwrap();
    let skin = SkinContext::from_manifest_and_document(
        default_skin_manifest(),
        document,
        [SkinDocumentTexture {
            source_id: "src".to_string(),
            texture: SkinTextureId(1),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        }],
    );

    let initial = skin.static_document_items_for_state(&SkinDrawState::default());
    let final_frame = skin.static_document_items_for_state(&SkinDrawState {
        elapsed_ms: 100,
        ..SkinDrawState::default()
    });
    let image_xs = |items: &[SkinRenderItem]| {
        items
            .iter()
            .filter_map(|item| match item {
                SkinRenderItem::Image { rect, .. } => Some(rect.x),
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(image_xs(&initial), [0.1, 0.2]);
    assert_eq!(image_xs(&final_frame), [0.1, 0.6, 0.8]);
}

#[test]
fn note_group_lift_offset_matches_note_lift_once() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 100, "h": 100,
                "source": [{ "id": 1, "path": "line.png" }],
                "image": [
                    { "id": "n1", "src": 1, "x": 0, "y": 0, "w": 10, "h": 1 },
                    { "id": "section-line", "src": 1, "x": 0, "y": 0, "w": 10, "h": 1 }
                ],
                "note": {
                    "id": "notes",
                    "note": ["n1"],
                    "dst": [{ "time": 0, "x": 10, "y": 20, "w": 40, "h": 60 }],
                    "group": [{
                        "id": "section-line",
                        "offset": 3,
                        "dst": [{ "time": 0, "x": 10, "y": 20, "w": 40, "h": 2 }]
                    }]
                }
            }
            "#,
    )
    .unwrap();
    let source_texture = SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: SkinTextureId(1),
        source_size: SkinImageSize { width: 10.0, height: 1.0 },
    };
    let skin = SkinContext::from_manifest_and_document(
        default_skin_manifest(),
        document,
        [source_texture],
    );
    let note_height = skin.document_note_height(Lane::Key1, KeyMode::K7).unwrap();
    let state_no_lift = SkinDrawState { offset_lift_px: 0, ..SkinDrawState::default() };
    let state_lifted = SkinDrawState { offset_lift_px: 10, ..SkinDrawState::default() };

    let note_no_lift = skin
        .note_rect_for_progress(Lane::Key1, KeyMode::K7, 0.0, note_height, &state_no_lift)
        .unwrap();
    let note_lifted = skin
        .note_rect_for_progress(Lane::Key1, KeyMode::K7, 0.0, note_height, &state_lifted)
        .unwrap();

    let bar_bottom_y = |state: &SkinDrawState| {
        let items = skin.document_bar_line_items(0.0, KeyMode::K7, state);
        let Some(SkinRenderItem::Image { rect, .. }) = items.first() else { panic!() };
        rect.y + rect.height
    };
    let note_shift = (note_lifted.y + note_lifted.height) - (note_no_lift.y + note_no_lift.height);
    let bar_shift = bar_bottom_y(&state_lifted) - bar_bottom_y(&state_no_lift);

    assert!(approx_eq(note_shift, -0.1), "expected note to lift once, got {note_shift}");
    assert!(
        approx_eq(bar_shift, note_shift),
        "bar line shift {bar_shift} should match note shift {note_shift}"
    );
}

#[test]
fn judge_offset_height_keeps_image_and_combo_y_aligned() {
    // beatoraja は SkinNumber を `setRelative(true)` で扱うため、
    // OFFSET_JUDGE_1P.h を変えても 判定文字 (image) とコンボ数 (number)
    // の Y 位置は同じ量だけシフトする (中心アンカー伸縮)。
    // 過去には number_frame にも x/y シフトが二重適用され、
    // 判定文字とコンボ数の Y がずれていた。
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 100, "h": 100,
                "source": [{ "id": "src", "path": "judge.png" }],
                "image": [{ "id": "judgef-pg", "src": "src", "x": 0, "y": 0, "w": 10, "h": 10 }],
                "value": [{
                    "id": "combo-num", "src": "src",
                    "x": 0, "y": 10, "w": 10, "h": 20,
                    "divx": 10, "divy": 1, "digit": 4, "ref": 102
                }],
                "judge": [{
                    "id": "judge",
                    "images": [
                        { "id": "judgef-pg", "offsets": [32], "dst": [
                            { "time": 0, "x": 10, "y": 20, "w": 30, "h": 10 },
                            { "time": 500 }
                        ]}
                    ],
                    "numbers": [
                        { "id": "combo-num", "offsets": [32], "dst": [
                            { "time": 0, "x": 0, "y": 30, "w": 10, "h": 20 },
                            { "time": 500 }
                        ]}
                    ]
                }]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("src", 10.0, 10.0);

    fn render_y_positions(
        document: &SkinDocument,
        sources: &HashMap<String, SkinDocumentTexture>,
        offset_h: i32,
    ) -> (f32, f32) {
        let mut offsets = SkinOffsetValues::default();
        offsets.set(
            OFFSET_JUDGE_1P,
            crate::skin_offset::SkinOffsetValue { x: 0, y: 0, w: 0, h: offset_h, r: 0, a: 0 },
        );
        let items =
            document.judge_render_items_with_offsets("PGREAT", 42, 0, &offsets, sources).unwrap();
        // [0] = 判定文字 image, [1..] = combo digit images
        let SkinRenderItem::Image { rect: image_rect, .. } = &items[0] else {
            panic!("first item should be image")
        };
        let SkinRenderItem::Image { rect: combo_rect, .. } = &items[1] else {
            panic!("second item should be first combo digit")
        };
        (image_rect.y + image_rect.height / 2.0, combo_rect.y + combo_rect.height / 2.0)
    }

    let (image_center_y_0, combo_center_y_0) = render_y_positions(&document, &sources, 0);
    let (image_center_y_h, combo_center_y_h) = render_y_positions(&document, &sources, 20);

    let image_shift = image_center_y_h - image_center_y_0;
    let combo_shift = combo_center_y_h - combo_center_y_0;
    assert!(
        approx_eq(image_shift, combo_shift),
        "image Y shift {image_shift} should match combo Y shift {combo_shift}"
    );
}

#[test]
fn judge_offset_width_matches_beatoraja_combo_layout() {
    // beatoraja は judge number の destination x 補正を元の w で行い、
    // その後 relative offset の w だけを加算する。Judge offset.w を
    // X 補正後の幅で再計算すると、コンボ数字が余計に左へずれる。
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 100, "h": 100,
                "source": [{ "id": "src", "path": "judge.png" }],
                "image": [{ "id": "judgef-pg", "src": "src", "x": 0, "y": 0, "w": 10, "h": 10 }],
                "value": [{
                    "id": "combo-num", "src": "src",
                    "x": 0, "y": 10, "w": 10, "h": 20,
                    "divx": 10, "divy": 1, "digit": 4, "ref": 102
                }],
                "judge": [{
                    "id": "judge", "shift": true,
                    "images": [
                        { "id": "judgef-pg", "offsets": [32], "dst": [
                            { "time": 0, "x": 10, "y": 20, "w": 30, "h": 10 },
                            { "time": 500 }
                        ]}
                    ],
                    "numbers": [
                        { "id": "combo-num", "offsets": [32], "dst": [
                            { "time": 0, "x": 0, "y": 30, "w": 10, "h": 20 },
                            { "time": 500 }
                        ]}
                    ]
                }]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("src", 10.0, 10.0);
    let mut offsets = SkinOffsetValues::default();
    offsets
        .set(OFFSET_JUDGE_1P, crate::skin_offset::SkinOffsetValue { w: 20, ..Default::default() });

    let items =
        document.judge_render_items_with_offsets("PGREAT", 42, 0, &offsets, &sources).unwrap();
    let SkinRenderItem::Image { rect: judge_rect, .. } = &items[0] else {
        panic!("first item should be judge image")
    };
    let SkinRenderItem::Image { rect: combo_rect, .. } = &items[1] else {
        panic!("second item should be first combo digit")
    };

    assert!(approx_eq(judge_rect.x, -0.3), "judge x {}", judge_rect.x);
    assert!(approx_eq(judge_rect.width, 0.5), "judge width {}", judge_rect.width);
    assert!(approx_eq(combo_rect.x, 0.1), "combo x {}", combo_rect.x);
}

#[test]
fn judge_lift_offset_keeps_image_and_combo_y_aligned() {
    // SkinNumber は relative offset のため、判定文字の destination と同じ
    // LIFT offset を持っていても combo 数字側で y を二重に動かさない。
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 100, "h": 100,
                "source": [{ "id": "src", "path": "judge.png" }],
                "image": [{ "id": "judgef-pg", "src": "src", "x": 0, "y": 0, "w": 10, "h": 10 }],
                "value": [{
                    "id": "combo-num", "src": "src",
                    "x": 0, "y": 10, "w": 10, "h": 20,
                    "divx": 10, "divy": 1, "digit": 4, "ref": 102
                }],
                "judge": [{
                    "id": "judge",
                    "images": [
                        { "id": "judgef-pg", "offset": 3, "dst": [
                            { "time": 0, "x": 10, "y": 20, "w": 30, "h": 10 },
                            { "time": 500 }
                        ]}
                    ],
                    "numbers": [
                        { "id": "combo-num", "offset": 3, "dst": [
                            { "time": 0, "x": 0, "y": 30, "w": 10, "h": 20 },
                            { "time": 500 }
                        ]}
                    ]
                }]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("src", 10.0, 10.0);

    fn render_y_positions(
        document: &SkinDocument,
        sources: &HashMap<String, SkinDocumentTexture>,
        lift_px: i32,
    ) -> (f32, f32) {
        let state = SkinDrawState { offset_lift_px: lift_px, ..SkinDrawState::default() };
        let items = document
            .judge_render_items_for_def(&document.judge[0], 0, 42, 0, sources, &state)
            .unwrap();
        let SkinRenderItem::Image { rect: image_rect, .. } = &items[0] else {
            panic!("first item should be image")
        };
        let SkinRenderItem::Image { rect: combo_rect, .. } = &items[1] else {
            panic!("second item should be first combo digit")
        };
        (image_rect.y + image_rect.height / 2.0, combo_rect.y + combo_rect.height / 2.0)
    }

    let (image_center_y_0, combo_center_y_0) = render_y_positions(&document, &sources, 0);
    let (image_center_y_lift, combo_center_y_lift) = render_y_positions(&document, &sources, 10);

    let image_shift = image_center_y_lift - image_center_y_0;
    let combo_shift = combo_center_y_lift - combo_center_y_0;
    assert!(
        approx_eq(image_shift, combo_shift),
        "image Y shift {image_shift} should match combo Y shift {combo_shift}"
    );
}

#[test]
fn judge_offset_alpha_applies_to_judge_image_and_combo() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 100, "h": 100,
                "source": [{ "id": "src", "path": "judge.png" }],
                "image": [{ "id": "judgef-pg", "src": "src", "x": 0, "y": 0, "w": 10, "h": 10 }],
                "value": [{
                    "id": "combo-num", "src": "src",
                    "x": 0, "y": 10, "w": 10, "h": 20,
                    "divx": 10, "divy": 1, "digit": 4, "ref": 102
                }],
                "judge": [{
                    "id": "judge",
                    "images": [
                        { "id": "judgef-pg", "offsets": [32], "dst": [
                            { "time": 0, "x": 10, "y": 20, "w": 30, "h": 10, "a": 200 },
                            { "time": 500 }
                        ]}
                    ],
                    "numbers": [
                        { "id": "combo-num", "offsets": [32], "dst": [
                            { "time": 0, "x": 0, "y": 30, "w": 10, "h": 20, "a": 200 },
                            { "time": 500 }
                        ]}
                    ]
                }]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("src", 10.0, 10.0);
    let mut offsets = SkinOffsetValues::default();
    offsets.set(
        OFFSET_JUDGE_1P,
        crate::skin_offset::SkinOffsetValue { x: 0, y: 0, w: 0, h: 0, r: 0, a: -80 },
    );

    let items =
        document.judge_render_items_with_offsets("PGREAT", 42, 0, &offsets, &sources).unwrap();

    let SkinRenderItem::Image { tint: judge_tint, .. } = &items[0] else { panic!() };
    let SkinRenderItem::Image { tint: combo_tint, .. } = &items[1] else { panic!() };
    let expected = (200.0 - 80.0) / 255.0;
    assert!(approx_eq(judge_tint.a, expected), "judge alpha {}", judge_tint.a);
    assert!(approx_eq(combo_tint.a, expected), "combo alpha {}", combo_tint.a);
}

#[test]
fn judge_offset_applies_to_judge_special_renderer() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 100, "h": 100,
                "source": [{ "id": "src", "path": "judge.png" }],
                "image": [{ "id": "judgef-pg", "src": "src", "x": 0, "y": 0, "w": 10, "h": 10 }],
                "judge": [{
                    "id": "judge",
                    "images": [
                        { "id": "judgef-pg", "offsets": [32], "dst": [{ "time": 0, "x": 10, "y": 20, "w": 30, "h": 10 }, { "time": 500 }] }
                    ]
                }]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("src", 10.0, 10.0);
    let mut offsets = SkinOffsetValues::default();
    offsets.set(
        OFFSET_JUDGE_1P,
        crate::skin_offset::SkinOffsetValue { x: 6, y: 0, w: 0, h: 0, r: 0, a: 0 },
    );

    let items =
        document.judge_render_items_with_offsets("PGREAT", 0, 0, &offsets, &sources).unwrap();

    let SkinRenderItem::Image { rect, .. } = &items[0] else { panic!() };
    assert!(approx_eq(rect.x, 0.16));
}

#[test]
fn skin_timer_maps_upper_scratchless_key_lanes() {
    let mut state = SkinDrawState::default();
    state.bomb_ms[Lane::Key8.index()] = Some(58);
    state.hold_ms[Lane::Key8.index()] = Some(78);
    state.keyon_ms[Lane::Key8.index()] = Some(108);
    state.keyoff_ms[Lane::Key8.index()] = Some(128);
    state.hcn_active_ms[Lane::Key8.index()] = Some(258);
    state.hcn_damage_ms[Lane::Key8.index()] = Some(278);

    assert_eq!(skin_timer_elapsed_ms(Some(58), &state), Some(58));
    assert_eq!(skin_timer_elapsed_ms(Some(78), &state), Some(78));
    assert_eq!(skin_timer_elapsed_ms(Some(108), &state), Some(108));
    assert_eq!(skin_timer_elapsed_ms(Some(128), &state), Some(128));
    assert_eq!(skin_timer_elapsed_ms(Some(258), &state), Some(258));
    assert_eq!(skin_timer_elapsed_ms(Some(278), &state), Some(278));
}

#[test]
fn runtime_event_toggles_flags_and_restarts_observe_timer() {
    let document: SkinDocument = serde_json::from_str(
        r#"{
                "runtimeFlag": [{ "id": -20001, "initial": false }],
                "runtimeEvent": [{ "id": -20002, "toggleFlags": [-20001] }],
                "dynamicTimer": [{ "id": 9000, "observe": "runtime_flag(-20001)" }]
            }"#,
    )
    .unwrap();
    let mut runtime = DynamicTimerRuntime::default();
    let mut state = SkinDrawState::default();

    runtime.advance(&document, &mut state, 100);
    assert_eq!(state.dynamic_timer_ms[0], None);
    assert!(eval_skin_draw_condition("not runtime_flag(-20001)", &state));

    assert!(runtime.dispatch_runtime_event(&document, -20_002));
    runtime.advance(&document, &mut state, 150);
    assert_eq!(state.dynamic_timer_ms[0], Some(0));
    assert!(eval_skin_draw_condition("runtime_flag(-20001)", &state));

    runtime.advance(&document, &mut state, 175);
    assert_eq!(state.dynamic_timer_ms[0], Some(25));
    assert!(runtime.dispatch_runtime_event(&document, -20_002));
    runtime.advance(&document, &mut state, 200);
    assert_eq!(state.dynamic_timer_ms[0], None);

    runtime.reset_for_document(Some(&document));
    runtime.advance(&document, &mut state, 250);
    assert_eq!(state.dynamic_timer_ms[0], None);
}

#[test]
fn runtime_lua_draw_is_evaluated_for_every_render_without_frame_cache() {
    let document: SkinDocument = serde_json::from_str(
        r#"{
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "panel.png" }],
                "image": [{ "id": "panel", "src": 1, "w": 10, "h": 10 }],
                "destination": [{
                    "id": "panel",
                    "draw": "bmz:lua_draw_callback:0",
                    "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }]
                }]
            }"#,
    )
    .unwrap();
    let runtime = Arc::new(AlternatingLuaDrawRuntime::default());
    let mut context = SkinContext::from_manifest_and_document(
        default_skin_manifest(),
        document,
        [SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(41),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        }],
    );
    context.set_lua_draw_runtime(Some(runtime.clone()));

    assert!(context.static_document_items().is_empty());
    assert_eq!(context.static_document_items().len(), 1);
    assert_eq!(runtime.calls.load(Ordering::Relaxed), 2);
}
