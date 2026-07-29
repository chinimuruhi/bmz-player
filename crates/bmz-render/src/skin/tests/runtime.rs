use super::*;

#[test]
fn keylogger_runtime_consumes_sequences_and_builds_nps_and_lane_counts() {
    let input = SkinRuntimeEvent {
        sequence: 10,
        kind: SkinRuntimeEventKind::Input(InputEvent {
            lane: Lane::Key1,
            kind: InputKind::Press,
            time: TimeUs(500_000),
            source: InputSource::Human,
            device_kind: InputDeviceKind::Keyboard,
            scratch_direction: None,
        }),
    };
    let judgement = SkinRuntimeEvent {
        sequence: 11,
        kind: SkinRuntimeEventKind::Judgement(bmz_gameplay::judge::model::JudgementEvent {
            note_id: Some(NoteId(1)),
            lane: Lane::Key1,
            judge: Judge::Great,
            side: TimingSide::Fast,
            delta: TimeUs(-1_000),
            time: TimeUs(500_000),
            affects_score: true,
        }),
    };
    let mut runtime = KeyLoggerRuntime::default();
    runtime.ingest(&[input.clone(), judgement.clone()], KeyMode::K9, 500_000);
    runtime.ingest(&[input, judgement], KeyMode::K9, 500_000);
    let mut state = SkinDrawState::default();
    runtime.write_state(&mut state, 500);

    assert_eq!(state.keylogger_nps, 1);
    assert_eq!(state.keylogger_judge_counts[0], [0, 1, 0, 0]);
    assert_eq!(state.keylogger_fast_slow_counts[0], [0, 1, 0]);
    assert_eq!(state.keylogger_event_ms[0][0], Some(0));
    assert!(eval_skin_draw_condition("keylogger_judge(1,1,great)", &state));
    assert!(eval_skin_draw_condition("keylogger_fastslow(1,1,fast)", &state));
    assert!(!eval_skin_draw_condition("keylogger_judge(1,1,bad)", &state));
    let destination: SkinDestinationDef = serde_json::from_str(
        r#"{"id":"keylogger-note-1","timer_expr":"bmz:keylogger_event:1:1","dst":[]}"#,
    )
    .unwrap();
    assert_eq!(destination_timer_elapsed_ms(&destination, &state), Some(0));
    assert!(
        (keylogger_graph_value("bmz:keylogger_graph:judge:1:great", &state).unwrap() - 1.0).abs()
            < f32::EPSILON
    );

    runtime.ingest(&[], KeyMode::K9, 1_500_001);
    runtime.write_state(&mut state, 1_500);
    assert_eq!(state.keylogger_nps, 0);

    let next_session_input = SkinRuntimeEvent {
        sequence: 0,
        kind: SkinRuntimeEventKind::Input(InputEvent {
            lane: Lane::Key2,
            kind: InputKind::Press,
            time: TimeUs(0),
            source: InputSource::Human,
            device_kind: InputDeviceKind::Keyboard,
            scratch_direction: None,
        }),
    };
    runtime.ingest(&[next_session_input], KeyMode::K9, 0);
    runtime.write_state(&mut state, 0);

    assert_eq!(state.keylogger_nps, 1);
    assert_eq!(state.keylogger_judge_counts, [[0; 4]; LANE_COUNT]);
    assert_eq!(state.keylogger_event_ms[0], [None; 16]);
    assert_eq!(state.keylogger_event_ms[1][0], Some(0));
}

#[test]
fn placement_uses_latest_animation_keyframe() {
    let placement = SkinPlacement {
        phase: SkinPhase::Play,
        time_ms: 0,
        rect: Rect { x: 0.0, y: 0.0, width: 0.1, height: 0.1 },
        alpha: 1.0,
        blend: BlendMode::Normal,
        animation: Animation {
            keyframes: vec![
                Keyframe {
                    time_ms: 0,
                    rect: Rect { x: 0.1, y: 0.0, width: 0.1, height: 0.1 },
                    alpha: 1.0,
                },
                Keyframe {
                    time_ms: 100,
                    rect: Rect { x: 0.2, y: 0.0, width: 0.1, height: 0.1 },
                    alpha: 0.8,
                },
            ],
        },
    };

    assert_eq!(placement.resolve(120).rect.x, 0.2);
}

#[test]
fn judge_line_with_lift_offset_still_renders_at_minimum_lift() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 720,
                "h": 720,
                "source": [{ "id": 12, "path": "line.png" }],
                "image": [{ "id": "judge_line", "src": 12, "w": 431, "h": 8 }],
                "destination": [
                    { "id": "judge_line", "offset": 3, "dst": [{ "time": 0, "x": 20, "y": 357, "w": 431, "h": 8, "a": 255 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let sources = HashMap::from([(
        "12".to_string(),
        SkinDocumentTexture {
            source_id: "12".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 431.0, height: 8.0 },
        },
    )]);

    let items = document.static_image_render_items(
        &sources,
        &SkinDrawState { offset_lift_px: 0, ..SkinDrawState::default() },
    );
    assert_eq!(items.len(), 1, "judge_line must not be skipped with liftcover skip logic");
}

#[test]
fn skin_document_evaluates_timer_draw_conditions() {
    assert!(eval_skin_draw_condition("timer(46) == timer_off", &SkinDrawState::default()));
    assert!(eval_skin_draw_condition(
        "timer(46) != timer_off",
        &SkinDrawState { judge_ms: judge_region_state(0, 120, 0).judge_ms, ..Default::default() }
    ));
    assert!(eval_skin_draw_condition(
        "timer(46) > 0 and option(197)",
        &SkinDrawState {
            judge_ms: judge_region_state(0, 120, 0).judge_ms,
            select_replay_slots: [true, false, false, false],
            ..Default::default()
        }
    ));
    let eon_shadow_draw = "timer(143) == timer_off and number(106)-number(110)-number(111)-number(112)-number(113)-number(114) == 0";
    assert!(eval_skin_draw_condition(
        eon_shadow_draw,
        &SkinDrawState {
            total_notes: 5,
            judge_counts: DisplayJudgeCounts { pgreat: 5, ..Default::default() },
            ..Default::default()
        }
    ));
    assert!(!eval_skin_draw_condition(
        eon_shadow_draw,
        &SkinDrawState {
            total_notes: 5,
            judge_counts: DisplayJudgeCounts { pgreat: 5, ..Default::default() },
            end_of_note_ms: Some(0),
            ..Default::default()
        }
    ));
    let ir_wait_draw = "timer(173) == timer_off and timer(174) == timer_off";
    assert!(eval_skin_draw_condition(ir_wait_draw, &SkinDrawState::default()));
    assert!(!eval_skin_draw_condition(
        ir_wait_draw,
        &SkinDrawState {
            ir_ranking: crate::scene::ResultIrSnapshot {
                connect_begin_ms: Some(500),
                connect_success_ms: Some(100),
                ..Default::default()
            },
            ..Default::default()
        }
    ));
}

#[test]
fn skin_document_applies_declared_14k_turntable_offsets_with_beatoraja_rotation() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 2,
                "w": 100,
                "h": 100,
                "source": [{ "id": "src", "path": "a.png" }],
                "image": [{ "id": "turntable", "src": "src", "w": 10, "h": 10 }],
                "destination": [
                    {
                        "id": "turntable",
                        "offset": 1,
                        "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }]
                    },
                    {
                        "id": "turntable",
                        "offset": 1,
                        "dst": [{ "x": 20, "y": 0, "w": 10, "h": 10 }]
                    },
                    {
                        "id": "turntable",
                        "offset": 2,
                        "dst": [{ "x": 40, "y": 0, "w": 10, "h": 10 }]
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let mut skin_offsets = SkinOffsetValues::default();
    skin_offsets.set(1, crate::skin_offset::SkinOffsetValue { r: 30, ..Default::default() });
    skin_offsets.set(2, crate::skin_offset::SkinOffsetValue { r: 70, ..Default::default() });
    let state = SkinDrawState { key_mode: KeyMode::K14, skin_offsets, ..SkinDrawState::default() };

    let angles = document
        .static_image_render_items(&mock_source("src", 10.0, 10.0), &state)
        .iter()
        .map(|item| match item {
            SkinRenderItem::RotatedImage { angle_deg, .. } => *angle_deg as i32,
            _ => panic!("turntable should be rotated"),
        })
        .collect::<Vec<_>>();

    assert_eq!(angles, vec![-30, -30, -70]);
}

#[test]
fn skin_document_samples_destination_keyframes_by_elapsed_time() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "panel", "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 10, "h": 10 },
                        { "time": 100, "x": 30, "a": 128 },
                        { "time": 200, "x": 60, "w": 20 }
                    ]}
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

    let early = document.static_image_render_items(
        &sources,
        &SkinDrawState { elapsed_ms: 50, ..SkinDrawState::default() },
    );
    let middle = document.static_image_render_items(
        &sources,
        &SkinDrawState { elapsed_ms: 150, ..SkinDrawState::default() },
    );
    let late = document.static_image_render_items(
        &sources,
        &SkinDrawState { elapsed_ms: 250, ..SkinDrawState::default() },
    );

    assert!(
        matches!(early[0], SkinRenderItem::Image { rect: Rect { x, width, .. }, tint: Color { a, .. }, .. }
                if approx_eq(x, 0.15) && approx_eq(width, 0.1) && approx_eq(a, 192.0 / 255.0))
    );
    assert!(
        matches!(middle[0], SkinRenderItem::Image { rect: Rect { x, width, .. }, tint: Color { a, .. }, .. }
                if approx_eq(x, 0.45) && approx_eq(width, 0.15) && approx_eq(a, 128.0 / 255.0))
    );
    assert!(
        matches!(late[0], SkinRenderItem::Image { rect: Rect { x, width, .. }, tint: Color { a, .. }, .. }
                if approx_eq(x, 0.6) && approx_eq(width, 0.2) && approx_eq(a, 128.0 / 255.0))
    );
}

#[test]
fn skin_document_loops_destination_keyframes() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "panel", "loop": 100, "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 10, "h": 10 },
                        { "time": 100, "x": 30 },
                        { "time": 200, "x": 60 }
                    ]}
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

    // loop=100, 終端=200。elapsed=350 は終端超過なので [100, 200) 区間へループバック:
    // (350 - 100) % (200 - 100) + 100 = 150 → time 150 は keyframe 100(x=30)/200(x=60) の中間
    // x = 45 → 正規化 0.45
    let wrapped = document.static_image_render_items(
        &sources,
        &SkinDrawState { elapsed_ms: 350, ..SkinDrawState::default() },
    );

    assert!(matches!(wrapped[0], SkinRenderItem::Image { rect: Rect { x, .. }, .. }
                if approx_eq(x, 0.45)));
}

#[test]
fn play_destination_negative_image_id_renders_runtime_stagefile_source() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
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
    let state = SkinDrawState {
        has_stagefile: true,
        stagefile_image_size: Some(SkinImageSize { width: 400.0, height: 200.0 }),
        ..SkinDrawState::default()
    };

    let (behind, front, overlay) = context.static_document_play_items_split_for_state_and_text(
        &state,
        &SkinTextState::default(),
        &[],
        &[],
    );

    assert!(behind.iter().chain(&front).chain(&overlay).any(|item| matches!(
        item,
        SkinRenderItem::Image {
            texture,
            source_size: Some(SkinImageSize { width: 400.0, height: 200.0 }),
            ..
        } if *texture == SkinTextureId(SELECT_STAGE_TEXTURE.0)
    )));
}

#[test]
fn skin_document_resolves_end_of_note_timer_destinations() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "marker", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "marker", "timer": 143, "dst": [{ "x": 10, "y": 20, "w": 5, "h": 6 }] }
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

    let hidden = document.static_image_render_items(
        &sources,
        &SkinDrawState { end_of_note: false, ..SkinDrawState::default() },
    );
    let visible = document.static_image_render_items(
        &sources,
        &SkinDrawState { end_of_note: true, end_of_note_ms: Some(0), ..SkinDrawState::default() },
    );

    assert!(hidden.is_empty());
    assert_eq!(visible.len(), 1);
    assert!(matches!(visible[0], SkinRenderItem::Image {
                rect: Rect { x, y, width, height },
                ..
            } if approx_eq(x, 0.1)
                && approx_eq(y, 0.74)
                && approx_eq(width, 0.05)
                && approx_eq(height, 0.06)));
}

#[test]
fn skin_document_resolves_full_combo_timer_destinations() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "fc", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "fc", "timer": 48, "loop": -1, "dst": [
                        { "time": 0, "x": 10, "y": 20, "w": 5, "h": 6, "a": 255 },
                        { "time": 1000, "a": 0 }
                    ] }
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

    let hidden = document.static_image_render_items(
        &sources,
        &SkinDrawState { full_combo_ms: None, ..SkinDrawState::default() },
    );
    let visible = document.static_image_render_items(
        &sources,
        &SkinDrawState { full_combo_ms: Some(500), ..SkinDrawState::default() },
    );

    assert!(hidden.is_empty());
    assert_eq!(visible.len(), 1);
    assert!(matches!(visible[0], SkinRenderItem::Image {
                tint: Color { a, .. },
                ..
            } if approx_eq(a, 128.0 / 255.0)));
}

#[test]
fn skin_context_reports_timer_animation_duration() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "fc", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "fc", "timer": 48, "loop": -1, "dst": [
                        { "time": 0, "x": 10, "y": 20, "w": 5, "h": 6 },
                        { "time": 1966, "a": 0 }
                    ] },
                    { "id": "other", "timer": 2, "dst": [{ "time": 3000 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let context =
        SkinContext::from_manifest_and_document(default_skin_manifest(), document, Vec::new());

    assert_eq!(context.timer_animation_duration_ms(48), 1966);
    assert_eq!(context.timer_animation_duration_ms(49), 0);
}

#[test]
fn skin_document_resolves_fadeout_timer_destinations() {
    // timer=2 (TIMER_FADEOUT) はシーン終了アニメーション用。
    // fadeout_ms=None なら非アクティブで描画されず、Some なら経過 ms で
    // keyframe アニメーションが進行する。
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 7,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "curtain", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "curtain", "timer": 2, "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 100, "h": 0 },
                        { "time": 200, "x": 0, "y": 0, "w": 100, "h": 100 }
                    ] }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(7),
            source_size: SkinImageSize { width: 100.0, height: 100.0 },
        },
    )]);

    let inactive = document.static_image_render_items(
        &sources,
        &SkinDrawState { fadeout_ms: None, ..SkinDrawState::default() },
    );
    let mid = document.static_image_render_items(
        &sources,
        &SkinDrawState { fadeout_ms: Some(100), ..SkinDrawState::default() },
    );

    assert!(inactive.is_empty(), "fadeout timer is inactive when fadeout_ms is None");
    assert_eq!(mid.len(), 1);
    assert!(matches!(mid[0], SkinRenderItem::Image {
                rect: Rect { height, .. },
                ..
            } if approx_eq(height, 0.5)));
}

#[test]
fn skin_document_resolves_failed_timer_destinations() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "destination": [
                    { "id": -111, "timer": 3, "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 100, "h": 100, "a": 0 },
                        { "time": 100, "a": 255 }
                    ] }
                ]
            }
            "#,
    )
    .unwrap();

    let inactive = document.static_image_render_items(
        &HashMap::new(),
        &SkinDrawState { failed_ms: None, ..SkinDrawState::default() },
    );
    let active = document.static_image_render_items(
        &HashMap::new(),
        &SkinDrawState { failed_ms: Some(50), ..SkinDrawState::default() },
    );

    assert!(inactive.is_empty());
    assert_eq!(active.len(), 1);
    assert!(matches!(active[0], SkinRenderItem::Rect {
                color: Color { r, g, b, a },
                ..
            } if approx_eq(r, 1.0)
                && approx_eq(g, 1.0)
                && approx_eq(b, 1.0)
                && approx_eq(a, 128.0 / 255.0)));
}

#[test]
fn lift_cover_schema_applies_lift_offset_once() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 720,
                "h": 720,
                "source": [{ "id": 12, "path": "lift.png" }],
                "liftCover": [
                    { "id": "lift", "src": 12, "x": 0, "y": 0, "w": 431, "h": 723, "disapearLine": 357 }
                ],
                "destination": [
                    { "id": "lift", "dst": [{ "x": 20, "y": -366, "w": 431, "h": 723 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let sources = HashMap::from([(
        "12".to_string(),
        SkinDocumentTexture {
            source_id: "12".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 431.0, height: 723.0 },
        },
    )]);

    let hidden = document.static_image_render_items(
        &sources,
        &SkinDrawState { offset_lift_px: 0, ..SkinDrawState::default() },
    );
    assert!(hidden.is_empty());

    let lifted = document.static_image_render_items(
        &sources,
        &SkinDrawState { offset_lift_px: 200, ..SkinDrawState::default() },
    );
    let SkinRenderItem::Image { rect, uv, .. } = &lifted[0] else {
        panic!("expected lift cover image");
    };
    assert!(approx_eq(rect.height, 200.0 / 720.0));
    assert!(approx_eq(uv.height, 200.0 / 723.0));
}

#[test]
fn hidden_cover_destination_applies_lift_and_hidden_offsets() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 12, "path": "cover.png" }],
                "hiddenCover": [
                    { "id": "hidden-cover", "src": 12, "x": 0, "y": 0, "w": 10, "h": 10 }
                ],
                "destination": [
                    { "id": "hidden-cover", "dst": [{ "x": 20, "y": -40, "w": 30, "h": 40 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "12".to_string(),
        SkinDocumentTexture {
            source_id: "12".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 100.0, height: 100.0 },
        },
    )]);

    let items = document.static_image_render_items(
        &sources,
        &SkinDrawState {
            hidden_cover: 0.5,
            offset_lift_px: 10,
            offset_hidden_cover_px: 20,
            ..SkinDrawState::default()
        },
    );

    assert_eq!(items.len(), 1);
    let SkinRenderItem::Image { rect, .. } = &items[0] else { panic!() };
    assert!(
        approx_eq(rect.y, (100 - (-40 + 10 + 20) - 40) as f32 / 100.0),
        "expected hidden cover to use automatic lift and hidden offsets, got {}",
        rect.y
    );
}

#[test]
fn display_signed_number_digits_uses_sign_cell_and_row_offset() {
    // divx=12, divy=2 のレイアウト想定
    // beatoraja の digit=5 は符号を含むため、数値部分は4枠。
    let positive = display_signed_number_digits(12, 5, NumberPadding::Zero, 12);
    assert_eq!(positive, vec![11, 0, 0, 1, 2]);
    assert!(positive.iter().all(|&d| d < 12), "positive digits should be in row 0");

    // 負数 -12 (max_digits=5): row 1 (offset=12)
    let negative = display_signed_number_digits(-12, 5, NumberPadding::Zero, 12);
    assert_eq!(negative, vec![23, 12, 12, 13, 14]);
    assert!(negative.iter().all(|&d| d >= 12), "negative digits should be in row 1");

    // 0 は正側
    let zero = display_signed_number_digits(0, 5, NumberPadding::Zero, 12);
    assert_eq!(zero, vec![11, 0, 0, 0, 0]);
    assert!(zero.iter().all(|&d| d < 12));

    // WMII IR差分: digit=5, zeropadding=4 は符号1枠 + 数値4枠。
    assert_eq!(
        display_signed_number_digits(2284, 5, NumberPadding::Zero, 12),
        vec![11, 2, 2, 8, 4]
    );
    assert_eq!(
        display_signed_number_digits(-9, 5, NumberPadding::Zero, 12),
        vec![23, 12, 12, 12, 21]
    );

    // LR2の符号付き数値は省略されたzeropaddingを2として扱い、符号を先頭に固定する。
    assert_eq!(display_signed_number_digits(9, 3, NumberPadding::Blank, 12), vec![11, 10, 9]);
    assert_eq!(display_signed_number_digits(-9, 3, NumberPadding::Blank, 12), vec![23, 22, 21]);
    assert_eq!(display_signed_number_digits(13, 3, NumberPadding::Blank, 12), vec![11, 1, 3]);
    assert_eq!(display_signed_number_digits(-13, 3, NumberPadding::Blank, 12), vec![23, 13, 15]);

    // ゼロ埋めなしで全枠が数値に埋まる場合、beatoraja同様に符号枠は出ない。
    assert_eq!(
        display_signed_number_digits(12_345, 5, NumberPadding::None, 12),
        vec![1, 2, 3, 4, 5]
    );

    // NUMBER_DIFF_NEXTRANK (154) も同じ符号セル付き mimage レイアウトを使う。
    assert_eq!(display_signed_number_digits(-34, 4, NumberPadding::None, 12), vec![23, 15, 16]);
    assert!(ref_id_is_signed(154));
    assert_eq!(display_signed_number_digits(34, 4, NumberPadding::None, 12), vec![11, 3, 4]);
    assert_eq!(display_signed_number_digits(0, 4, NumberPadding::None, 12), vec![11, 0]);
    assert_eq!(
        display_signed_number_digits_with_row_order(
            -34,
            4,
            NumberPadding::None,
            12,
            SignedNumberRowOrder::NegativeFirst
        ),
        vec![11, 3, 4]
    );
    assert_eq!(
        display_signed_number_digits_with_row_order(
            0,
            4,
            NumberPadding::None,
            12,
            SignedNumberRowOrder::NegativeFirst
        ),
        vec![23, 12]
    );

    let score_diff_value = SkinValueDef {
        id: "score_diff_mybest".to_string(),
        src: "num".to_string(),
        x: 0,
        y: 0,
        w: 0,
        h: 0,
        divx: 12,
        divy: 2,
        timer: None,
        cycle: 0,
        align: 0,
        judge_align: None,
        digit: 5,
        padding: 0,
        zeropadding: 1,
        space: 0,
        ref_id: 152,
        expr: String::new(),
        value_expr: String::new(),
        offset: Vec::new(),
    };
    let score_diff_padding = number_padding(&score_diff_value);
    assert!(score_diff_padding.is_zero_padding());
    assert_eq!(signed_value_padding(&score_diff_value, score_diff_padding), NumberPadding::None);
    assert_eq!(display_signed_number_digits(16, 5, NumberPadding::None, 12), vec![11, 1, 6]);

    let select_detail =
        SkinDrawState { select_screen: true, select_option_panel: 3, ..Default::default() };
    let select_normal = SkinDrawState { select_screen: true, ..Default::default() };
    assert!(value_ref_is_signed_for_state(12, &select_detail));
    assert!(!value_ref_is_signed_for_state(12, &select_normal));
}

#[test]
fn logical_input_press_edges_drive_options_timers_and_runtime_events() {
    let document: SkinDocument = serde_json::from_str(
        r#"{
                "type": 0, "w": 1, "h": 1, "destination": [],
                "runtimeFlag": [{ "id": 1, "initial": false }],
                "runtimeEvent": [{
                    "id": -20001,
                    "toggleFlags": [1],
                    "triggerAction": "e1_press"
                }]
            }"#,
    )
    .unwrap();
    let mut runtime = DynamicTimerRuntime::default();
    let mut state = SkinDrawState::default();

    // A held input on scene entry is synchronized without inventing a press edge.
    state.logical_input_held[0] = true;
    runtime.advance(&document, &mut state, 100);
    assert_eq!(state.runtime_flags.get(&1), Some(&false));
    assert_eq!(skin_timer_elapsed_ms(Some(SKIN_TIMER_BMZ_INPUT_BASE), &state), None);
    assert!(test_skin_op(SKIN_OPTION_BMZ_INPUT_BASE, &[], &state));

    state.logical_input_held[0] = false;
    runtime.advance(&document, &mut state, 110);
    state.logical_input_held[0] = true;
    runtime.advance(&document, &mut state, 120);
    assert_eq!(state.runtime_flags.get(&1), Some(&true));
    assert_eq!(skin_timer_elapsed_ms(Some(SKIN_TIMER_BMZ_INPUT_BASE), &state), Some(0));

    runtime.advance(&document, &mut state, 150);
    assert_eq!(state.runtime_flags.get(&1), Some(&true));
    assert_eq!(skin_timer_elapsed_ms(Some(SKIN_TIMER_BMZ_INPUT_BASE), &state), Some(30));
}

#[test]
fn end_of_note_timers_use_elapsed_since_end_of_note() {
    let inactive =
        SkinDrawState { elapsed_ms: 5_000, end_of_note_ms: None, ..SkinDrawState::default() };
    assert_eq!(skin_timer_elapsed_ms(Some(143), &inactive), None);
    assert_eq!(skin_timer_elapsed_ms(Some(144), &inactive), None);

    let active = SkinDrawState {
        elapsed_ms: 5_000,
        end_of_note: true,
        end_of_note_ms: Some(250),
        end_of_note_2p_ms: Some(325),
        ..SkinDrawState::default()
    };
    assert_eq!(skin_timer_elapsed_ms(Some(143), &active), Some(250));
    assert_eq!(skin_timer_elapsed_ms(Some(144), &active), Some(325));
}

#[test]
fn fixed_delay_timer_starts_after_source_delay() {
    let document: SkinDocument = serde_json::from_str(
        r#"{
                "type": 0, "w": 1, "h": 1, "destination": [],
                "fixedDelayTimer": [{ "id": 11900, "sourceTimer": 143, "delayMs": 1000 }]
            }"#,
    )
    .unwrap();
    let mut runtime = DynamicTimerRuntime::default();
    let mut state = SkinDrawState { end_of_note_ms: Some(999), ..SkinDrawState::default() };

    runtime.advance(&document, &mut state, 5_000);
    assert_eq!(skin_timer_elapsed_ms(Some(11900), &state), None);

    state.end_of_note_ms = Some(1_250);
    runtime.advance(&document, &mut state, 5_251);
    assert_eq!(skin_timer_elapsed_ms(Some(11900), &state), Some(250));

    state.end_of_note_ms = None;
    runtime.advance(&document, &mut state, 5_252);
    assert_eq!(skin_timer_elapsed_ms(Some(11900), &state), None);
}

#[test]
fn zero_delay_timer_alias_follows_source_timer() {
    let document: SkinDocument = serde_json::from_str(
        r#"{
                "type": 0, "w": 1, "h": 1, "destination": [],
                "fixedDelayTimer": [{ "id": 11901, "sourceTimer": 143, "delayMs": 0 }]
            }"#,
    )
    .unwrap();
    let mut runtime = DynamicTimerRuntime::default();
    let mut state = SkinDrawState { end_of_note_ms: Some(1_250), ..SkinDrawState::default() };

    runtime.advance(&document, &mut state, 5_000);
    assert_eq!(skin_timer_elapsed_ms(Some(11901), &state), Some(1_250));

    state.end_of_note_ms = None;
    runtime.advance(&document, &mut state, 5_001);
    assert_eq!(skin_timer_elapsed_ms(Some(11901), &state), None);
}

#[test]
fn timer_zero_uses_scene_elapsed_time() {
    let state = SkinDrawState { elapsed_ms: 1_800, ..SkinDrawState::default() };

    assert_eq!(skin_timer_elapsed_ms(Some(0), &state), Some(1_800));
}

#[test]
fn start_input_timer_activates_strictly_after_skin_input_delay() {
    assert_eq!(skin_start_input_elapsed_ms(499, 500), None);
    assert_eq!(skin_start_input_elapsed_ms(500, 500), None);
    assert_eq!(skin_start_input_elapsed_ms(501, 500), Some(1));

    let state = SkinDrawState { start_input_ms: Some(275), ..SkinDrawState::default() };
    assert_eq!(skin_timer_elapsed_ms(Some(1), &state), Some(275));
}

#[test]
fn rhythm_timer_uses_bpm_normalized_snapshot_time() {
    let inactive = SkinDrawState::default();
    assert_eq!(skin_timer_elapsed_ms(Some(140), &inactive), None);

    let active = SkinDrawState { rhythm_timer_ms: Some(2_750), ..SkinDrawState::default() };
    assert_eq!(skin_timer_elapsed_ms(Some(140), &active), Some(2_750));
}

#[test]
fn keybeam_runtime_suppresses_ln_press_and_its_release_fade() {
    let document: SkinDocument =
        serde_json::from_str(r#"{ "type": 0, "w": 1, "h": 1, "destination": [] }"#).unwrap();
    let mut runtime = DynamicTimerRuntime::default();
    let mut state = SkinDrawState::default();
    let lane = Lane::Key1.index();

    state.keyon_ms[lane] = Some(0);
    runtime.advance(&document, &mut state, 100);
    assert!(state.keybeam_hold_active[lane]);

    state.keyon_ms[lane] = Some(10);
    state.hold_ms[lane] = Some(0);
    runtime.advance(&document, &mut state, 110);
    assert!(!state.keybeam_hold_active[lane]);

    state.keyon_ms[lane] = None;
    state.hold_ms[lane] = None;
    state.keyoff_ms[lane] = Some(0);
    runtime.advance(&document, &mut state, 120);
    assert!(!state.keybeam_fade_active[lane]);

    state.keyoff_ms[lane] = None;
    state.keyon_ms[lane] = Some(0);
    runtime.advance(&document, &mut state, 200);
    state.keyon_ms[lane] = None;
    state.keyoff_ms[lane] = Some(0);
    runtime.advance(&document, &mut state, 210);
    assert!(state.keybeam_fade_active[lane]);
    assert!(eval_skin_draw_condition("keybeam_fade(121) != 0", &state));
}

#[test]
fn keybeam_timer_lane_mapping_matches_skin_timer_mapping() {
    assert_eq!(keybeam_lane_for_keyon_timer(108), Some(Lane::Key8.index()));
    assert_eq!(keybeam_lane_for_keyon_timer(109), Some(Lane::Key9.index()));
    assert_eq!(keybeam_lane_for_keyon_timer(110), Some(Lane::Scratch2.index()));
    assert_eq!(keybeam_lane_for_keyoff_timer(128), Some(Lane::Key8.index()));
    assert_eq!(keybeam_lane_for_keyoff_timer(129), Some(Lane::Key9.index()));
    assert_eq!(keybeam_lane_for_keyoff_timer(130), Some(Lane::Scratch2.index()));
}

#[test]
fn bomb_timer_activates_only_for_active_lane() {
    // timer=51 maps to bomb Key1 (TIMER_BOMB_1P_KEY1 = 50 + Lane::Key1.index() = 51)
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "bomb.png" }],
                "image": [{ "id": "bomb-img", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "bomb-img", "timer": 51, "dst": [
                        { "time": 0, "x": 10, "y": 10, "w": 10, "h": 10 }
                    ]}
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(9),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        },
    )]);

    // All lanes inactive → no items
    let inactive_state = SkinDrawState::default();
    let items_inactive = document.static_image_render_items(&sources, &inactive_state);
    assert_eq!(items_inactive.len(), 0, "should be empty when all bomb timers are None");

    // Key1 (index=1) active → items returned
    let active_state = SkinDrawState {
        bomb_ms: {
            let mut a = [None; LANE_COUNT];
            a[1] = Some(0);
            a
        },
        ..SkinDrawState::default()
    };
    let items_active = document.static_image_render_items(&sources, &active_state);
    assert_eq!(items_active.len(), 1, "should have one item when Key1 bomb timer is active");
}

#[test]
fn dst_if_value_uses_default_when_option_disabled() {
    // No property → no enabled options → conditional frame skipped, only end frame {time:500}.
    // 最初のキーフレーム時刻 (500) より前は描画されず、500ms 以降に既定位置 (0,0) で描画される。
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 1280, "h": 720,
                "source": [{ "id": "src", "path": "a.png" }],
                "image": [{ "id": "img", "src": "src", "w": 10, "h": 10 }],
                "destination": [
                    { "id": "img", "dst": [
                        { "if": [920], "value": { "time": 0, "x": 100, "y": 200, "w": 50, "h": 50 } },
                        { "time": 500 }
                    ]}
                ]
            }
            "#,
        )
        .unwrap();

    let sources = mock_source("src", 10.0, 10.0);

    // elapsed=0: 最初のキーフレーム時刻 (500) より前なので描画しない。
    let before = document.static_image_render_items(
        &sources,
        &SkinDrawState { elapsed_ms: 0, ..SkinDrawState::default() },
    );
    assert!(before.is_empty(), "destination is not drawn before its first keyframe time");

    // elapsed=500: 条件フレームが skip され、{time:500} の既定位置 (0,0) で描画される。
    let items = document.static_image_render_items(
        &sources,
        &SkinDrawState { elapsed_ms: 500, ..SkinDrawState::default() },
    );
    assert_eq!(items.len(), 1);
    let SkinRenderItem::Image { rect, .. } = &items[0] else { panic!() };
    assert!(approx_eq(rect.x, 0.0), "expected default x=0, got {}", rect.x);
    assert!(approx_eq(rect.y, 1.0), "expected default y=1, got {}", rect.y);
}

#[test]
fn offset_lift_shifts_destination_y() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 1280, "h": 720,
                "source": [{ "id": "src", "path": "a.png" }],
                "image": [{ "id": "img", "src": "src", "w": 10, "h": 10 }],
                "destination": [
                    { "id": "img", "offset": 3, "dst": [
                        { "time": 0, "x": 100, "y": 200, "w": 50, "h": 50 }
                    ]}
                ]
            }
            "#,
    )
    .unwrap();

    let sources = mock_source("src", 10.0, 10.0);
    let state_no_lift = SkinDrawState { offset_lift_px: 0, ..SkinDrawState::default() };
    let state_lifted = SkinDrawState { offset_lift_px: 72, ..SkinDrawState::default() };

    let items_no_lift = document.static_image_render_items(&sources, &state_no_lift);
    let items_lifted = document.static_image_render_items(&sources, &state_lifted);

    assert_eq!(items_no_lift.len(), 1);
    assert_eq!(items_lifted.len(), 1);

    let SkinRenderItem::Image { rect: rect_no_lift, .. } = &items_no_lift[0] else { panic!() };
    let SkinRenderItem::Image { rect: rect_lifted, .. } = &items_lifted[0] else { panic!() };

    // With lift=72px on a 720h canvas, beatoraja y shifts upward in bottom-origin space.
    assert!(approx_eq(rect_no_lift.y, (720 - 200 - 50) as f32 / 720.0));
    assert!(
        approx_eq(rect_lifted.y, (720 - (200 + 72) - 50) as f32 / 720.0),
        "expected y shifted by lift, got {}",
        rect_lifted.y
    );
}

#[test]
fn offset_lanecover_shifts_destination_y() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 1280, "h": 720,
                "source": [{ "id": "src", "path": "a.png" }],
                "image": [{ "id": "img", "src": "src", "w": 10, "h": 10 }],
                "destination": [
                    { "id": "img", "offset": 4, "dst": [
                        { "time": 0, "x": 0, "y": 720, "w": 50, "h": 50 }
                    ]}
                ]
            }
            "#,
    )
    .unwrap();

    let sources = mock_source("src", 10.0, 10.0);
    // lanecover=0.5, lift=0 → offset_lanecover_px = (0-1)*720*0.5 = -360
    let state = SkinDrawState { offset_lanecover_px: -360, ..SkinDrawState::default() };
    let items = document.static_image_render_items(&sources, &state);

    assert_eq!(items.len(), 1);
    let SkinRenderItem::Image { rect, .. } = &items[0] else { panic!() };
    // y=720 shifted by -360 in bottom-origin space: top = 720 - (720 - 360 + 50).
    assert!(
        approx_eq(rect.y, (720 - (720 - 360 + 50)) as f32 / 720.0),
        "expected shifted y, got {}",
        rect.y
    );
}

#[test]
fn custom_offset_adjusts_destination_geometry_and_alpha() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
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
    let mut offsets = SkinOffsetValues::default();
    offsets.set(42, crate::skin_offset::SkinOffsetValue { x: 6, y: 8, w: 10, h: 12, r: 0, a: -50 });
    let items = document.static_image_render_items(
        &sources,
        &SkinDrawState { skin_offsets: offsets, ..SkinDrawState::default() },
    );

    assert_eq!(items.len(), 1);
    let SkinRenderItem::Image { rect, tint, .. } = &items[0] else { panic!() };
    assert!(approx_eq(rect.x, (10 + 6 - 10 / 2) as f32 / 100.0));
    assert!(approx_eq(rect.y, (100 - (20 + 8 - 12 / 2) - (40 + 12)) as f32 / 100.0));
    assert!(approx_eq(rect.width, 40.0 / 100.0));
    assert!(approx_eq(rect.height, 52.0 / 100.0));
    assert!(approx_eq(tint.a, 150.0 / 255.0));
}

#[test]
fn all_offset_transforms_play_skin_render_item() {
    let mut offsets = SkinOffsetValues::default();
    offsets.set(
        OFFSET_ALL,
        crate::skin_offset::SkinOffsetValue { x: 10, y: 20, w: 50, h: -50, r: 0, a: 0 },
    );
    let item = SkinRenderItem::Image {
        texture: SkinTextureId(1),
        rect: Rect { x: 0.2, y: 0.4, width: 0.1, height: 0.2 },
        uv: TextureRegion::default(),
        tint: Color::rgb(1.0, 1.0, 1.0),
        blend: BlendMode::Normal,
        scale: SkinImageScale::Stretch,
        border: None,
        source_size: None,
        linear_filter: false,
    };

    let item = apply_all_offset_to_render_item(
        item,
        &SkinDrawState { skin_offsets: offsets, ..SkinDrawState::default() },
    );

    let SkinRenderItem::Image { rect, .. } = item else { panic!() };
    assert!(approx_eq(rect.x, 0.4));
    assert!(approx_eq(rect.y, 0.0));
    assert!(approx_eq(rect.width, 0.15));
    assert!(approx_eq(rect.height, 0.1));
}

#[test]
fn notes_offset_adjusts_note_rect() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 100, "h": 100,
                "note": {
                    "id": "notes",
                    "note": ["n1"],
                    "dst": [{ "time": 0, "x": 10, "y": 20, "w": 30, "h": 40 }]
                }
            }
            "#,
    )
    .unwrap();
    let mut offsets = SkinOffsetValues::default();
    offsets.set(
        OFFSET_NOTES_1P,
        crate::skin_offset::SkinOffsetValue { x: 0, y: 0, w: 0, h: 20, r: 0, a: 0 },
    );

    let area = document.note_lane_area(Lane::Key1, KeyMode::K7, &[]).unwrap();
    let center_y = area.y + area.height * 0.5;
    let rect = document.apply_notes_offset_to_rect(
        Rect { x: area.x, y: center_y - 0.05, width: area.width, height: 0.1 },
        &SkinDrawState { skin_offsets: offsets, ..SkinDrawState::default() },
    );

    assert!(approx_eq(rect.y, 0.45));
    assert!(approx_eq(rect.height, 0.3));
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
