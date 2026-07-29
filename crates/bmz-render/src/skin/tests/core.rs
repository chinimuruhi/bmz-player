use super::*;

#[test]
fn negative_image_destination_size_mirrors_texture_region() {
    let item = skin_image_item_for_frame(
        SkinTextureId(1),
        Rect { x: 0.2, y: 0.3, width: 0.4, height: 0.5 },
        TextureRegion { x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
        ResolvedSkinFrame { w: -101, h: -53, ..ResolvedSkinFrame::default() },
        0,
        BlendMode::Normal,
        None,
        false,
    );

    let SkinRenderItem::Image { rect, uv, .. } = item else { panic!() };
    assert!(approx_eq(rect.x, 0.2));
    assert!(approx_eq(rect.width, 0.4));
    assert!(approx_eq(uv.x, 0.4));
    assert!(approx_eq(uv.width, -0.3));
    assert!(approx_eq(uv.y, 0.6));
    assert!(approx_eq(uv.height, -0.4));
}

#[test]
fn positive_image_destination_size_keeps_texture_region_direction() {
    let item = skin_image_item_for_frame(
        SkinTextureId(1),
        Rect { x: 0.2, y: 0.3, width: 0.4, height: 0.5 },
        TextureRegion { x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
        ResolvedSkinFrame { w: 101, h: 53, ..ResolvedSkinFrame::default() },
        0,
        BlendMode::Normal,
        None,
        false,
    );

    let SkinRenderItem::Image { uv, .. } = item else { panic!() };
    assert!(approx_eq(uv.x, 0.1));
    assert!(approx_eq(uv.width, 0.3));
    assert!(approx_eq(uv.y, 0.2));
    assert!(approx_eq(uv.height, 0.4));
}

#[test]
fn number_object_resolves_to_padded_text() {
    let object = SkinObject {
        id: SkinObjectId(1),
        source: SkinSource::Number {
            slot: NumberSlot::ExScore,
            style: TextStyle {
                font_id: None,
                size: 0.04,
                bitmap_size: None,
                color: Color::rgb(1.0, 1.0, 1.0),
                layer: TextLayer::Skin,
                align: TextAlign::Left,
                max_width: 0.0,
                overflow: TextOverflow::Overflow,
                wrapping: false,
                outline: None,
                shadow: None,
            },
            digits: 4,
        },
        placements: vec![SkinPlacement {
            phase: SkinPhase::Result,
            time_ms: 0,
            rect: Rect { x: 0.1, y: 0.2, width: 0.2, height: 0.05 },
            alpha: 0.5,
            blend: BlendMode::Normal,
            animation: Animation::none(),
        }],
    };

    let items = object.resolve(SkinPhase::Result, 0, |_| String::new(), |_| 123);

    assert!(matches!(
        &items[0],
        SkinRenderItem::Text { text, style, .. }
            if text == "0123" && style.color.a == 0.5
    ));
}

#[test]
fn skin_definition_resolves_context_values() {
    let skin = SkinDefinition {
        objects: vec![SkinObject {
            id: SkinObjectId(1),
            source: SkinSource::Text {
                slot: TextSlot::Judge,
                style: TextStyle {
                    font_id: None,
                    size: 0.04,
                    bitmap_size: None,
                    color: Color::rgb(1.0, 1.0, 1.0),
                    layer: TextLayer::Skin,
                    align: TextAlign::Left,
                    max_width: 0.0,
                    overflow: TextOverflow::Overflow,
                    wrapping: false,
                    outline: None,
                    shadow: None,
                },
            },
            placements: vec![SkinPlacement {
                phase: SkinPhase::Play,
                time_ms: 0,
                rect: Rect { x: 0.3, y: 0.4, width: 0.2, height: 0.05 },
                alpha: 1.0,
                blend: BlendMode::Normal,
                animation: Animation::none(),
            }],
        }],
    };
    let context = SkinRenderContext {
        phase: SkinPhase::Play,
        elapsed_ms: 12,
        text: &[(TextSlot::Judge, "PGREAT FAST".to_string())],
        numbers: &[],
    };

    let items = skin.resolve(&context);

    assert!(matches!(&items[0], SkinRenderItem::Text { text, .. } if text == "PGREAT FAST"));
}

#[test]
fn append_skin_render_items_emits_image_commands() {
    let mut commands = Vec::new();
    append_skin_render_items(
        &mut commands,
        &[
            SkinRenderItem::Rect {
                rect: Rect { x: 0.0, y: 0.0, width: 0.1, height: 0.1 },
                color: Color::rgb(1.0, 1.0, 1.0),
                blend: BlendMode::Normal,
            },
            SkinRenderItem::Image {
                texture: SkinTextureId(1),
                rect: Rect { x: 0.0, y: 0.0, width: 0.1, height: 0.1 },
                uv: TextureRegion { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                tint: Color::rgb(1.0, 1.0, 1.0),
                blend: BlendMode::Add,
                scale: SkinImageScale::Stretch,
                border: None,
                source_size: None,
                linear_filter: false,
            },
        ],
    );

    assert_eq!(commands.len(), 2);
    assert!(matches!(
        commands[1],
        DrawCommand::Image { texture: TextureId(1), blend: BlendMode::Add, .. }
    ));
}

#[test]
fn append_skin_render_items_keeps_empty_text_with_caret() {
    let mut commands = Vec::new();
    append_skin_render_items(
        &mut commands,
        &[SkinRenderItem::Text {
            origin: Point { x: 0.25, y: 0.5 },
            text: String::new(),
            style: TextStyle {
                font_id: None,
                size: 0.04,
                bitmap_size: None,
                color: Color::rgb(1.0, 1.0, 1.0),
                layer: TextLayer::Skin,
                align: TextAlign::Left,
                max_width: 0.0,
                overflow: TextOverflow::Overflow,
                wrapping: false,
                outline: None,
                shadow: None,
            },
            caret: Some(TextCaret { byte_index: 0, color: Color::rgb(1.0, 1.0, 1.0) }),
            blend: BlendMode::Normal,
        }],
    );

    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0],
        DrawCommand::Text { text, caret: Some(TextCaret { byte_index: 0, .. }), .. }
            if text.is_empty()
    ));
}

#[test]
fn append_skin_render_items_expands_nine_slice_images() {
    let mut commands = Vec::new();
    append_skin_render_items(
        &mut commands,
        &[SkinRenderItem::Image {
            texture: SkinTextureId(10),
            rect: Rect { x: 0.1, y: 0.2, width: 0.6, height: 0.3 },
            uv: TextureRegion { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
            tint: Color::rgb(1.0, 1.0, 1.0),
            blend: BlendMode::Normal,
            scale: SkinImageScale::NineSlice,
            border: Some(SkinImageBorder {
                left: 0.1,
                right: 0.2,
                top: 0.25,
                bottom: 0.25,
                unit: SkinImageBorderUnit::Normalized,
            }),
            source_size: None,
            linear_filter: false,
        }],
    );

    assert_eq!(commands.len(), 9);
    assert!(matches!(
        commands[0],
        DrawCommand::Image {
            rect: Rect { x: 0.1, y: 0.2, width, height },
            uv: UvRect { x: 0.0, y: 0.0, width: uv_width, height: uv_height },
            texture: TextureId(10),
            ..
        } if approx_eq(width, 0.06)
            && approx_eq(height, 0.075)
            && approx_eq(uv_width, 0.1)
            && approx_eq(uv_height, 0.25)
    ));
    assert!(matches!(
        commands[4],
        DrawCommand::Image {
            rect: Rect { width, height, .. },
            uv: UvRect { width: uv_width, height: uv_height, .. },
            texture: TextureId(10),
            ..
        } if approx_eq(width, 0.42)
            && approx_eq(height, 0.15)
            && approx_eq(uv_width, 0.7)
            && approx_eq(uv_height, 0.5)
    ));
}

#[test]
fn append_skin_render_items_expands_pixel_based_nine_slice_images() {
    let mut commands = Vec::new();
    append_skin_render_items(
        &mut commands,
        &[SkinRenderItem::Image {
            texture: SkinTextureId(8),
            rect: Rect { x: 0.2, y: 0.1, width: 0.36, height: 0.48 },
            uv: TextureRegion { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
            tint: Color::rgb(1.0, 1.0, 1.0),
            blend: BlendMode::Normal,
            scale: SkinImageScale::NineSlice,
            border: Some(SkinImageBorder {
                left: 2.0,
                right: 2.0,
                top: 3.0,
                bottom: 3.0,
                unit: SkinImageBorderUnit::Pixels,
            }),
            source_size: Some(SkinImageSize { width: 12.0, height: 48.0 }),
            linear_filter: false,
        }],
    );

    assert_eq!(commands.len(), 9);
    assert!(matches!(
        commands[0],
        DrawCommand::Image {
            rect: Rect { width, height, .. },
            uv: UvRect { width: uv_width, height: uv_height, .. },
            ..
        } if approx_eq(width, 0.06)
            && approx_eq(height, 0.03)
            && approx_eq(uv_width, 2.0 / 12.0)
            && approx_eq(uv_height, 3.0 / 48.0)
    ));
}

#[test]
fn bundled_default_skin_manifest_resolves_relative_texture_paths() {
    let manifest = SkinManifest::bundled_default().with_texture_source_sizes(&default_skin_root());

    let textures = manifest.resolve_textures(Path::new("/skin/default"));

    assert_eq!(textures[0].id, TextureId(1));
    assert_eq!(textures[0].path, PathBuf::from("/skin/default/note.png"));
    assert_eq!(textures[1].id, TextureId(2));
    assert_eq!(textures[1].path, PathBuf::from("/skin/default/note-blue.png"));
    assert_eq!(textures[2].id, TextureId(3));
    assert_eq!(textures[2].path, PathBuf::from("/skin/default/note-red.png"));
    assert_eq!(textures[3].id, TextureId(4));
    assert_eq!(textures[3].path, PathBuf::from("/skin/default/receptor.png"));
    assert_eq!(textures[4].id, TextureId(5));
    assert_eq!(textures[4].path, PathBuf::from("/skin/default/receptor-blue.png"));
    assert_eq!(textures[5].id, TextureId(6));
    assert_eq!(textures[5].path, PathBuf::from("/skin/default/receptor-red.png"));
    assert_eq!(textures[6].id, TextureId(7));
    assert_eq!(textures[6].path, PathBuf::from("/skin/default/judge-line.png"));
    assert_eq!(textures[7].id, TextureId(8));
    assert_eq!(textures[7].path, PathBuf::from("/skin/default/gauge-frame.png"));
    assert_eq!(textures[8].id, TextureId(9));
    assert_eq!(textures[8].path, PathBuf::from("/skin/default/gauge-fill.png"));
    assert_eq!(textures[9].id, TextureId(10));
    assert_eq!(textures[9].path, PathBuf::from("/skin/default/combo-panel.png"));
    assert_eq!(textures[10].id, TextureId(11));
    assert_eq!(textures[10].path, PathBuf::from("/skin/default/combo-panel-inactive.png"));
    assert_eq!(textures[11].id, TextureId(12));
    assert_eq!(textures[11].path, PathBuf::from("/skin/default/note-mine.png"));
    assert_eq!(manifest.play_note_image().texture_for_lane(Lane::Key2), 2);
    assert_eq!(manifest.play_note_image().texture_for_lane(Lane::Scratch), 3);
    assert_eq!(manifest.play_receptor_image().texture_for_lane(Lane::Key2), 5);
    assert_eq!(manifest.play_receptor_image().texture_for_lane(Lane::Scratch), 6);
    assert_eq!(manifest.play_judge_line_image().texture, 7);
    assert_eq!(manifest.play_gauge_frame_image().texture, 8);
    assert_eq!(manifest.play_gauge_frame_image().scale, SkinImageScale::NineSlice);
    assert_eq!(
        manifest.play_gauge_frame_image().source_size,
        Some(SkinImageSize { width: 12.0, height: 48.0 })
    );
    assert_eq!(
        manifest.play_gauge_frame_image().border,
        Some(SkinImageBorder {
            left: 2.0,
            right: 2.0,
            top: 3.0,
            bottom: 3.0,
            unit: SkinImageBorderUnit::Pixels,
        })
    );
    assert_eq!(manifest.play_gauge_fill_image().texture, 9);
    assert_eq!(manifest.play_combo_panel_image(true).texture, 10);
    assert_eq!(manifest.play_combo_panel_image(true).scale, SkinImageScale::NineSlice);
    assert_eq!(manifest.play_combo_panel_image(false).texture, 11);
}

#[test]
fn difficulty_ops_reflect_chart_difficulty_code() {
    let unknown = SkinDrawState::default();
    let normal = SkinDrawState { difficulty: 2, ..SkinDrawState::default() };
    let insane = SkinDrawState { difficulty: 5, ..SkinDrawState::default() };

    assert!(test_skin_op(150, &[], &unknown));
    assert!(!test_skin_op(150, &[], &normal));
    assert!(test_skin_op(152, &[], &normal));
    assert!(!test_skin_op(153, &[], &normal));
    assert!(test_skin_op(155, &[], &insane));
}

#[test]
fn folded_constant_draw_condition_number_zero_is_true() {
    assert!(eval_skin_draw_condition("number(0) >= 0", &SkinDrawState::default()));
    assert!(!eval_skin_draw_condition("number(0) < 0", &SkinDrawState::default()));
}

#[test]
fn skin_state_number_maps_next_rank_diff() {
    let a_state = SkinDrawState {
        result_grade_diff_display: ResultGradeDiffDisplay::Next,
        select_ex_score: Some(1300),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };
    let aaa_state = SkinDrawState {
        result_grade_diff_display: ResultGradeDiffDisplay::Next,
        select_ex_score: Some(1800),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };
    let max_state = SkinDrawState {
        result_grade_diff_display: ResultGradeDiffDisplay::Next,
        select_ex_score: Some(2000),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(154, &a_state), Some(-34));
    assert_eq!(skin_state_number(154, &aaa_state), Some(-200));
    assert_eq!(skin_state_number(154, &max_state), Some(0));
    assert_eq!(skin_state_number(154, &SkinDrawState::default()), None);
    assert_eq!(next_rank_grade(&a_state), Some("AA"));
    assert_eq!(next_rank_grade(&aaa_state), Some("MAX"));
    let near_aaa_state = SkinDrawState {
        result_grade_diff_display: ResultGradeDiffDisplay::Next,
        select_ex_score: Some(1774),
        select_total_notes: 1000,
        select_play_count: 1,
        select_screen: true,
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(154, &near_aaa_state), Some(-4));
    assert_eq!(result_grade_diff_label(&near_aaa_state), Some("-4".to_string()));
    assert_eq!(next_rank_grade(&near_aaa_state), Some("AAA"));
    assert_eq!(grade_diff_rank_target_grade(&near_aaa_state, true), Some("AAA"));
    assert_eq!(
        next_rank_grade(&SkinDrawState {
            select_ex_score: Some(0),
            select_total_notes: 2253,
            ..SkinDrawState::default()
        }),
        Some("E")
    );

    let nearest = SkinDrawState {
        result_grade_diff_display: ResultGradeDiffDisplay::Nearest,
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };
    assert_eq!(
        result_grade_diff_label(&SkinDrawState { select_ex_score: Some(100), ..nearest.clone() }),
        Some("F+100".to_string())
    );
    assert_eq!(
        result_grade_diff_label(&SkinDrawState { select_ex_score: Some(300), ..nearest.clone() }),
        Some("E-145".to_string())
    );
    assert_eq!(
        skin_state_number(154, &SkinDrawState { select_ex_score: Some(300), ..nearest.clone() }),
        Some(-145)
    );
    assert_eq!(
        result_grade_diff_label(&SkinDrawState { select_ex_score: Some(500), ..nearest.clone() }),
        Some("E+55".to_string())
    );
    assert_eq!(
        result_grade_diff_label(&SkinDrawState { select_ex_score: Some(1900), ..nearest.clone() }),
        Some("MAX-100".to_string())
    );
    assert_eq!(
        result_grade_diff_label(&SkinDrawState { select_ex_score: Some(2000), ..nearest.clone() }),
        Some("MAX+0".to_string())
    );
    let screenshot_score = SkinDrawState {
        result_grade_diff_display: ResultGradeDiffDisplay::Nearest,
        ex_score: 1100,
        total_notes: 594,
        result_failed: Some(false),
        ..SkinDrawState::default()
    };
    assert_eq!(result_grade_diff_label(&screenshot_score), Some("AAA+44".to_string()));
    assert_eq!(skin_state_number(154, &screenshot_score), Some(44));
    assert_eq!(grade_diff_rank_target_grade(&screenshot_score, true), Some("AAA"));
    let next_screenshot_score = SkinDrawState {
        result_grade_diff_display: ResultGradeDiffDisplay::Next,
        ..screenshot_score
    };
    assert_eq!(result_grade_diff_label(&next_screenshot_score), Some("-88".to_string()));
    assert_eq!(skin_state_number(154, &next_screenshot_score), Some(-88));
    assert_eq!(grade_diff_rank_target_grade(&next_screenshot_score, true), Some("MAX"));
}

#[test]
fn skin_document_resolves_static_image_destinations() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 1280,
                "h": 720,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "panel", "src": 1, "x": 16, "y": 32, "w": 64, "h": 128 }],
                "destination": [
                    { "id": "panel", "blend": 2, "dst": [
                        { "x": 128, "y": 72, "w": 256, "h": 144, "a": 128, "r": 64 }
                    ]},
                    { "id": "panel", "timer": 1, "dst": [{ "x": 0, "y": 0, "w": 1, "h": 1 }] }
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
            source_size: SkinImageSize { width: 256.0, height: 512.0 },
        },
    )]);

    let items = document.static_image_render_items(&sources, &SkinDrawState::default());

    assert_eq!(items.len(), 1);
    assert!(matches!(
        items[0],
        SkinRenderItem::Image {
            texture: SkinTextureId(42),
            rect: Rect { x, y, width, height },
            uv: TextureRegion { x: u, y: v, width: uv_width, height: uv_height },
            tint: Color { r, a, .. },
            blend: BlendMode::Add,
            ..
        } if approx_eq(x, 0.1)
            && approx_eq(y, 0.7)
            && approx_eq(width, 0.2)
            && approx_eq(height, 0.2)
            && approx_eq(u, 16.0 / 256.0)
            && approx_eq(v, 32.0 / 512.0)
            && approx_eq(uv_width, 64.0 / 256.0)
            && approx_eq(uv_height, 128.0 / 512.0)
            && approx_eq(r, 64.0 / 255.0)
            && approx_eq(a, 128.0 / 255.0)
    ));
}

#[test]
fn skin_document_applies_destination_stretch_to_static_images() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "wide", "src": 1, "x": 0, "y": 0, "w": 200, "h": 100 }],
                "destination": [
                    { "id": "wide", "stretch": 1, "dst": [{ "x": 10, "y": 10, "w": 40, "h": 40 }] },
                    { "id": "wide", "stretch": 3, "dst": [{ "x": 10, "y": 60, "w": 40, "h": 40 }] },
                    { "id": "wide", "stretch": 9, "dst": [{ "x": 70, "y": 70, "w": 20, "h": 20 }] }
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
            source_size: SkinImageSize { width: 200.0, height: 100.0 },
        },
    )]);

    let items = document.static_image_render_items(&sources, &SkinDrawState::default());

    assert_eq!(items.len(), 3);
    assert!(matches!(
        items[0],
        SkinRenderItem::Image {
            rect: Rect { x, y, width, height },
            uv: TextureRegion { x: u, width: uv_width, .. },
            ..
        } if approx_eq(x, 0.1)
            && approx_eq(y, 0.6)
            && approx_eq(width, 0.4)
            && approx_eq(height, 0.2)
            && approx_eq(u, 0.0)
            && approx_eq(uv_width, 1.0)
    ));
    assert!(matches!(
        items[1],
        SkinRenderItem::Image {
            rect: Rect { x, y, width, height },
            uv: TextureRegion { x: u, width: uv_width, .. },
            ..
        } if approx_eq(x, 0.1)
            && approx_eq(y, 0.0)
            && approx_eq(width, 0.4)
            && approx_eq(height, 0.4)
            && approx_eq(u, 0.25)
            && approx_eq(uv_width, 0.5)
    ));
    assert!(matches!(
        items[2],
        SkinRenderItem::Image {
            rect: Rect { x, y, width, height },
            ..
        } if approx_eq(x, -0.2)
            && approx_eq(y, -0.3)
            && approx_eq(width, 2.0)
            && approx_eq(height, 1.0)
    ));
}

#[test]
fn skin_document_evaluates_number_draw_conditions() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "panel", "draw": "number(425) > 0", "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "panel", "draw": "number(425) == 0", "dst": [{ "x": 10, "y": 0, "w": 10, "h": 10 }] }
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

    let no_miss = document.static_image_render_items(&sources, &SkinDrawState::default());
    let miss = document.static_image_render_items(
        &sources,
        &SkinDrawState {
            judge_counts: DisplayJudgeCounts { bad: 1, poor: 2, ..Default::default() },
            ..SkinDrawState::default()
        },
    );

    assert!(
        matches!(no_miss[0], SkinRenderItem::Image { rect: Rect { x, .. }, .. } if approx_eq(x, 0.1))
    );
    assert!(
        matches!(miss[0], SkinRenderItem::Image { rect: Rect { x, .. }, .. } if approx_eq(x, 0.0))
    );
    assert!(eval_skin_draw_condition(
        "number(410) == number(411) or number(110) == number(410)",
        &SkinDrawState {
            judge_counts: DisplayJudgeCounts { pgreat: 300, ..Default::default() },
            fast_slow_counts: Some(crate::snapshot::FastSlowJudgeCounts {
                fast_pgreat: 300,
                slow_pgreat: 0,
                ..Default::default()
            }),
            ..Default::default()
        }
    ));
    assert!(eval_skin_draw_condition(
        "number(410) > number(411) and number(411) >= 1",
        &SkinDrawState {
            fast_slow_counts: Some(crate::snapshot::FastSlowJudgeCounts {
                fast_pgreat: 120,
                slow_pgreat: 20,
                ..Default::default()
            }),
            ..Default::default()
        }
    ));
}

#[test]
fn skin_document_evaluates_option_draw_conditions() {
    assert!(eval_skin_draw_condition(
        "option(197)",
        &SkinDrawState { select_replay_slots: [true, false, false, false], ..Default::default() }
    ));
    assert!(eval_skin_draw_condition("!option(197)", &SkinDrawState::default()));
    assert!(!eval_skin_draw_condition(
        "!option(197)",
        &SkinDrawState { select_replay_slots: [true, false, false, false], ..Default::default() }
    ));
}

#[test]
fn static_render_items_resolve_iidx_destination_with_base_image() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "frame.png" }],
                "image": [{ "id": "groove_frame", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "groove_frame_iidx", "timer": 9001, "dst": [{ "x": 1, "y": 2, "w": 10, "h": 10 }] }
                ],
                "dynamicTimer": [{ "id": 9001, "observe": "gauge_type() == 4 or gauge_type() == 5" }]
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
    let mut runtime = DynamicTimerRuntime::default();
    let mut state = SkinDrawState { gauge_type: 4, elapsed_ms: 100, ..Default::default() };
    runtime.advance(&document, &mut state, 100);
    let (behind, front, _) =
        document.static_render_items_split(&sources, &state, &SkinTextState::default());
    let items = behind.into_iter().chain(front).collect::<Vec<_>>();
    assert_eq!(items.len(), 1);
    assert!(matches!(items[0], SkinRenderItem::Image { .. }));
}

#[test]
fn skin_document_evaluates_destination_option_conditions() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "property": [
                    { "name": "Play Side", "item": [
                        { "name": "1P", "op": 920 },
                        { "name": "2P", "op": 921 }
                    ]},
                    { "name": "Score Graph", "def": "On", "item": [
                        { "name": "Off", "op": 900 },
                        { "name": "On", "op": 901 }
                    ]}
                ],
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "panel", "op": [920, 901], "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "panel", "op": [921], "dst": [{ "x": 10, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "panel", "op": [-901], "dst": [{ "x": 20, "y": 0, "w": 10, "h": 10 }] }
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

    let items = document.static_image_render_items(&sources, &SkinDrawState::default());

    assert_eq!(document.enabled_options(), [920, 901]);
    assert_eq!(items.len(), 1);
    assert!(
        matches!(items[0], SkinRenderItem::Image { rect: Rect { x, .. }, .. } if approx_eq(x, 0.0))
    );
}

#[test]
fn skin_document_applies_destination_acc_easing() {
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        },
    )]);

    for (acc, expected_x) in [(1, 0.25), (2, 0.75), (3, 0.0)] {
        let document: SkinDocument = serde_json::from_str(&format!(
            r#"
                {{
                    "type": 0,
                    "w": 100,
                    "h": 100,
                    "source": [{{ "id": 1, "path": "system.png" }}],
                    "image": [{{ "id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }}],
                    "destination": [
                        {{ "id": "panel", "dst": [
                            {{ "time": 0, "x": 0, "y": 0, "w": 10, "h": 10 }},
                            {{ "time": 100, "x": 100, "acc": {acc} }}
                        ]}}
                    ]
                }}
                "#
        ))
        .unwrap();

        let items = document.static_image_render_items(
            &sources,
            &SkinDrawState { elapsed_ms: 50, ..SkinDrawState::default() },
        );

        assert!(matches!(items[0], SkinRenderItem::Image { rect: Rect { x, .. }, .. }
                    if approx_eq(x, expected_x)));
    }
}

#[test]
fn skin_document_prefers_lnbody_active_for_pressed_long_body_in_new_format() {
    // 新形式 (lnbodyActive 定義あり): 押下中=lnbodyActive、非押下=lnbody。
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "source": [{ "id": 1, "path": "notes.png" }],
                "image": [
                    { "id": "body", "src": 1, "x": 20, "y": 0, "w": 20, "h": 1 },
                    { "id": "body-a", "src": 1, "x": 50, "y": 0, "w": 30, "h": 1 }
                ],
                "note": {
                    "id": "notes",
                    "note": ["body", "body", "body", "body", "body", "body", "body", "body"],
                    "lnbody": ["body", "body", "body", "body", "body", "body", "body", "body"],
                    "lnbodyActive": ["body-a", "body-a", "body-a", "body-a", "body-a", "body-a", "body-a", "body-a"]
                }
            }
            "#,
        )
        .unwrap();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 100.0, height: 50.0 },
        },
    )]);
    let rect = Rect { x: 0.0, y: 0.0, width: 0.1, height: 0.1 };

    let pressed = document
        .note_long_body_render_item(
            Lane::Scratch,
            KeyMode::K7,
            rect,
            LongNoteMode::Ln,
            LongBodyState::Processing,
            &SkinDrawState::default(),
            &sources,
        )
        .unwrap();
    let unpressed = document
        .note_long_body_render_item(
            Lane::Scratch,
            KeyMode::K7,
            rect,
            LongNoteMode::Ln,
            LongBodyState::Inactive,
            &SkinDrawState::default(),
            &sources,
        )
        .unwrap();

    // 押下中 → lnbodyActive (x=50/100)、非押下 → lnbody (x=20/100)
    assert!(matches!(
        pressed,
        SkinRenderItem::Image { uv: TextureRegion { x, .. }, .. } if approx_eq(x, 0.5)
    ));
    assert!(matches!(
        unpressed,
        SkinRenderItem::Image { uv: TextureRegion { x, .. }, .. } if approx_eq(x, 0.2)
    ));
}

#[test]
fn skin_document_animates_csv_ln_body_only_while_processing() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "source": [{ "id": 1, "path": "notes.png" }],
                "image": [
                    { "id": "active", "src": 1, "x": 0, "y": 0, "w": 20, "h": 10, "divx": 2, "cycle": 100, "timer": 70 },
                    { "id": "inactive", "src": 1, "x": 20, "y": 0, "w": 10, "h": 10 }
                ],
                "note": {
                    "id": "notes",
                    "lnbody": ["inactive", "inactive", "inactive", "inactive", "inactive", "inactive", "inactive", "inactive"],
                    "lnbodyActive": ["active", "active", "active", "active", "active", "active", "active", "active"]
                }
            }
            "#,
        )
        .unwrap();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 100.0, height: 10.0 },
        },
    )]);
    let rect = Rect { x: 0.0, y: 0.0, width: 0.1, height: 0.1 };
    let mut draw_state = SkinDrawState::default();
    draw_state.hold_ms[Lane::Scratch.index()] = Some(50);

    let pressed = document
        .note_long_body_render_item(
            Lane::Scratch,
            KeyMode::K7,
            rect,
            LongNoteMode::Ln,
            LongBodyState::Processing,
            &draw_state,
            &sources,
        )
        .unwrap();
    let unpressed = document
        .note_long_body_render_item(
            Lane::Scratch,
            KeyMode::K7,
            rect,
            LongNoteMode::Ln,
            LongBodyState::Inactive,
            &draw_state,
            &sources,
        )
        .unwrap();

    assert!(matches!(
        pressed,
        SkinRenderItem::Image { uv: TextureRegion { x, .. }, .. } if approx_eq(x, 0.1)
    ));
    assert!(matches!(
        unpressed,
        SkinRenderItem::Image { uv: TextureRegion { x, .. }, .. } if approx_eq(x, 0.2)
    ));
}

#[test]
fn panel_renders_fill_and_canvas_pixel_border() {
    let document: SkinDocument = serde_json::from_str(
        r##"
            {
                "w": 100,
                "h": 100,
                "panel": [{
                    "id": "option-panel",
                    "color": "#102030",
                    "borderColor": "#A0B0C0",
                    "borderWidth": 2
                }],
                "destination": [{
                    "id": "option-panel",
                    "dst": [{ "x": 10, "y": 20, "w": 30, "h": 40 }]
                }]
            }
            "##,
    )
    .unwrap();

    let items = document.static_image_render_items(&HashMap::new(), &SkinDrawState::default());

    assert_eq!(items.len(), 5);
    let SkinRenderItem::Rect { rect, color, blend } = items[0] else {
        panic!("expected panel fill");
    };
    assert_eq!(rect, Rect { x: 0.1, y: 0.4, width: 0.3, height: 0.4 });
    assert!(approx_eq(color.r, 16.0 / 255.0));
    assert!(approx_eq(color.g, 32.0 / 255.0));
    assert!(approx_eq(color.b, 48.0 / 255.0));
    assert_eq!(blend, BlendMode::Normal);
    assert!(matches!(
        items[1],
        SkinRenderItem::Rect {
            rect: Rect { x, y, width, height },
            ..
        } if approx_eq(x, 0.1)
            && approx_eq(y, 0.4)
            && approx_eq(width, 0.3)
            && approx_eq(height, 0.02)
    ));
}

#[test]
fn skin_document_resolves_static_value_destinations() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "number.png" }],
                "value": [
                    { "id": "combo", "src": 1, "x": 0, "y": 0, "w": 100, "h": 10, "divx": 10, "digit": 3, "ref": 104 },
                    { "id": "remain", "src": 1, "x": 0, "y": 0, "w": 100, "h": 10, "divx": 10, "digit": 3, "expr": "number(106) - number(110) - number(111)" },
                    { "id": "unknown", "src": 1, "x": 0, "y": 0, "w": 100, "h": 10, "divx": 10, "digit": 3, "ref": 9999 }
                ],
                "destination": [
                    { "id": "combo", "dst": [{ "x": 10, "y": 20, "w": 5, "h": 10 }] },
                    { "id": "remain", "dst": [{ "x": 10, "y": 30, "w": 5, "h": 10 }] },
                    { "id": "unknown", "dst": [{ "x": 10, "y": 40, "w": 5, "h": 10 }] }
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

    let items = document.static_image_render_items(
        &sources,
        &SkinDrawState {
            elapsed_ms: 0,
            combo: 45,
            total_notes: 100,
            judge_counts: DisplayJudgeCounts { pgreat: 30, great: 20, ..Default::default() },
            ..SkinDrawState::default()
        },
    );

    // combo=45 (2 digits), digit=3 → shiftbase=1, align=0 (right-aligned, default)
    // digit_step = 5/100 = 0.05, origin_x = 10/100 = 0.1
    // digit "4": x = 0.1 + 0.05 * (1+0) - 0 = 0.15
    // digit "5": x = 0.1 + 0.05 * (1+1) - 0 = 0.20
    assert_eq!(items.len(), 4);
    assert!(matches!(items[0], SkinRenderItem::Image {
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(x, 0.15) && approx_eq(y, 0.7) && approx_eq(u, 0.4)));
    assert!(matches!(items[1], SkinRenderItem::Image {
                rect: Rect { x, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(x, 0.20) && approx_eq(u, 0.5)));
    assert!(matches!(items[2], SkinRenderItem::Image {
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(x, 0.15) && approx_eq(y, 0.6) && approx_eq(u, 0.5)));
    assert!(matches!(items[3], SkinRenderItem::Image {
                rect: Rect { x, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(x, 0.20) && approx_eq(u, 0.0)));
}

#[test]
fn skin_document_resolves_static_text_destinations() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "text": [
                    { "id": "title", "font": "main", "size": 8, "align": 1, "wrapping": true, "outlineColor": "ff000080", "outlineWidth": 1, "shadowColor": "00000080", "shadowOffsetX": 2, "shadowOffsetY": 3, "ref": 12 },
                    { "id": "genre", "size": 6, "align": 2, "overflow": 1, "ref": 13 },
                    { "id": "constant", "size": 5, "constantText": "READY" },
                    { "id": "numeric-constant", "size": 5, "constantText": 1 }
                ],
                "destination": [
                    { "id": "title", "dst": [{ "x": 10, "y": 20, "w": 50, "h": 10, "r": 128, "g": 200, "b": 255 }] },
                    { "id": "genre", "dst": [{ "x": 10, "y": 40, "w": 40, "h": 6 }] },
                    { "id": "constant", "dst": [{ "x": 10, "y": 60, "h": 5, "a": 128 }] },
                    { "id": "numeric-constant", "dst": [{ "x": 10, "y": 70, "h": 5 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let items = document.static_render_items(
        &HashMap::new(),
        &SkinDrawState::default(),
        &SkinTextState {
            title: "Song",
            subtitle: "Another",
            genre: "Techno",
            ..SkinTextState::default()
        },
    );

    assert_eq!(items.len(), 4);
    assert!(matches!(&items[0], SkinRenderItem::Text {
                origin: Point { x, y },
                text,
                style,
                ..
            } if approx_eq(*x, -0.15)
                && approx_eq(*y, 0.7)
                && text == "Song Another"
                && style.font_id.as_deref() == Some("main")
                && approx_eq(style.size, 0.1)
                && style.align == TextAlign::Center
                && style.wrapping
                && matches!(style.outline, Some(TextOutline { color, width })
                    if color == Color::rgba(1.0, 0.0, 0.0, 128.0 / 255.0)
                        && approx_eq(width, 0.01))
                && matches!(style.shadow, Some(TextShadow { color, offset })
                    if color == Color::rgba(0.0, 0.0, 0.0, 128.0 / 255.0)
                        && approx_eq(offset.x, 0.02)
                        && approx_eq(offset.y, 0.03))
                && approx_eq(style.max_width, 0.5)
                && style.color == Color::rgba(128.0 / 255.0, 200.0 / 255.0, 1.0, 1.0)));
    assert!(matches!(&items[1], SkinRenderItem::Text { text, style, .. }
                if text == "Techno"
                    && style.align == TextAlign::Right
                    && style.overflow == TextOverflow::Shrink
                    && approx_eq(style.max_width, 0.4)));
    assert!(
        matches!(&items[2], SkinRenderItem::Text { text, style, .. } if text == "READY" && approx_eq(style.color.a, 128.0 / 255.0))
    );
    assert!(matches!(&items[3], SkinRenderItem::Text { text, .. } if text == "1"));
}

#[test]
fn skin_document_resolves_music_progress_slider() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "slider": [
                    { "id": "progress", "src": 1, "x": 10, "y": 20, "w": 5, "h": 6, "angle": 2, "range": 40, "type": 6 },
                    { "id": "lane-cover", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10, "angle": 2, "range": 20, "type": 4 },
                    { "id": "lane-cover-modern", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10, "angle": 2, "range": 20, "type": 5 },
                    { "id": "song-scroll", "src": 1, "x": 20, "y": 20, "w": 5, "h": 6, "angle": 2, "range": 40, "type": 1 },
                    { "id": "master", "src": 1, "x": 30, "y": 20, "w": 5, "h": 6, "angle": 1, "range": 40, "type": 17 },
                    { "id": "unknown", "src": 1, "x": 10, "y": 20, "w": 5, "h": 6, "angle": 0, "range": 40, "type": 999 }
                ],
                "destination": [
                    { "id": "progress", "blend": 2, "dst": [{ "x": 30, "y": 80, "w": 5, "h": 6 }] },
                    { "id": "lane-cover", "dst": [{ "x": 10, "y": 50, "w": 10, "h": 10 }] },
                    { "id": "lane-cover-modern", "dst": [{ "x": 20, "y": 50, "w": 10, "h": 10 }] },
                    { "id": "song-scroll", "dst": [{ "x": 30, "y": 80, "w": 5, "h": 6 }] },
                    { "id": "master", "dst": [{ "x": 30, "y": 80, "w": 5, "h": 6 }] },
                    { "id": "unknown", "dst": [{ "x": 30, "y": 80, "w": 5, "h": 6 }] }
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

    let items = document.static_image_render_items(
        &sources,
        &SkinDrawState {
            play_progress: 0.25,
            select_scroll_progress: 0.5,
            select_master_volume: 0.75,
            ..SkinDrawState::default()
        },
    );

    assert_eq!(items.len(), 3);
    assert!(matches!(items[0], SkinRenderItem::Image {
                rect: Rect { x, y, width, height },
                uv: TextureRegion { x: u, y: v, width: uw, height: uh },
                blend,
                ..
            } if approx_eq(x, 0.3)
                && approx_eq(y, 0.24)
                && approx_eq(width, 0.05)
                && approx_eq(height, 0.06)
                && approx_eq(u, 0.1)
                && approx_eq(v, 0.2)
                && approx_eq(uw, 0.05)
                && approx_eq(uh, 0.06)
                && blend == BlendMode::Add));
    assert!(matches!(
        items[1],
        SkinRenderItem::Image { rect: Rect { x, y, .. }, .. }
            if approx_eq(x, 0.3) && approx_eq(y, 0.34)
    ));
    assert!(matches!(
        items[2],
        SkinRenderItem::Image { rect: Rect { x, y, .. }, .. }
            if approx_eq(x, 0.6) && approx_eq(y, 0.14)
    ));

    let no_lane_cover = document.static_image_render_items(
        &sources,
        &SkinDrawState { lane_cover: 0.0, ..SkinDrawState::default() },
    );
    assert_eq!(no_lane_cover.len(), 3);

    let lane_cover = document.static_image_render_items(
        &sources,
        &SkinDrawState { lane_cover: 0.5, ..SkinDrawState::default() },
    );
    assert_eq!(lane_cover.len(), 5);
    assert!(matches!(
        lane_cover[1],
        SkinRenderItem::Image { rect: Rect { x, y, .. }, .. }
            if approx_eq(x, 0.1) && approx_eq(y, 0.5)
    ));
    assert!(matches!(
        lane_cover[2],
        SkinRenderItem::Image { rect: Rect { x, y, .. }, .. }
            if approx_eq(x, 0.2) && approx_eq(y, 0.5)
    ));
}

#[test]
fn skin_document_moves_sliders_in_beatoraja_directions() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "slider": [
                    { "id": "up", "src": 1, "x": 0, "y": 0, "w": 5, "h": 5, "angle": 0, "range": 20, "type": 17 },
                    { "id": "right", "src": 1, "x": 0, "y": 0, "w": 5, "h": 5, "angle": 1, "range": 20, "type": 17 },
                    { "id": "down", "src": 1, "x": 0, "y": 0, "w": 5, "h": 5, "angle": 2, "range": 20, "type": 17 },
                    { "id": "left", "src": 1, "x": 0, "y": 0, "w": 5, "h": 5, "angle": 3, "range": 20, "type": 17 }
                ],
                "destination": [
                    { "id": "up", "dst": [{ "x": 50, "y": 50, "w": 5, "h": 5 }] },
                    { "id": "right", "dst": [{ "x": 50, "y": 50, "w": 5, "h": 5 }] },
                    { "id": "down", "dst": [{ "x": 50, "y": 50, "w": 5, "h": 5 }] },
                    { "id": "left", "dst": [{ "x": 50, "y": 50, "w": 5, "h": 5 }] }
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

    let items = document.static_image_render_items(
        &sources,
        &SkinDrawState { select_master_volume: 0.5, ..SkinDrawState::default() },
    );

    assert_eq!(items.len(), 4);
    assert!(matches!(
        items[0],
        SkinRenderItem::Image { rect: Rect { x, y, .. }, .. }
            if approx_eq(x, 0.5) && approx_eq(y, 0.35)
    ));
    assert!(matches!(
        items[1],
        SkinRenderItem::Image { rect: Rect { x, y, .. }, .. }
            if approx_eq(x, 0.6) && approx_eq(y, 0.45)
    ));
    assert!(matches!(
        items[2],
        SkinRenderItem::Image { rect: Rect { x, y, .. }, .. }
            if approx_eq(x, 0.5) && approx_eq(y, 0.55)
    ));
    assert!(matches!(
        items[3],
        SkinRenderItem::Image { rect: Rect { x, y, .. }, .. }
            if approx_eq(x, 0.4) && approx_eq(y, 0.45)
    ));
}

#[test]
fn sudden_slider_progress_is_capped_by_lift() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "cover.png" }],
                "slider": [
                    { "id": "lanecover", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10, "angle": 2, "range": 100, "type": 4 }
                ],
                "destination": [
                    { "id": "lanecover", "dst": [{ "x": 0, "y": 100, "w": 10, "h": 10 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 100.0, 100.0);

    let items = document.static_image_render_items(
        &sources,
        &SkinDrawState { lane_cover: 0.9, lift: 0.2, ..SkinDrawState::default() },
    );

    let SkinRenderItem::Image { rect, .. } = &items[0] else { panic!() };
    assert!(approx_eq(rect.y, 0.7), "expected capped SUDDEN slider y, got {}", rect.y);
}

#[test]
fn skin_document_resolves_special_black_fade_rect() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 6,
                "w": 100,
                "h": 100,
                "destination": [
                    { "id": -110, "timer": 2, "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 100, "h": 100, "a": 0 },
                        { "time": 200, "a": 255 }
                    ] }
                ]
            }
            "#,
    )
    .unwrap();

    let mid = document.static_image_render_items(
        &HashMap::new(),
        &SkinDrawState { fadeout_ms: Some(100), ..SkinDrawState::default() },
    );

    assert_eq!(mid.len(), 1);
    assert!(matches!(mid[0], SkinRenderItem::Rect {
                rect: Rect { width, height, .. },
                color: Color { r, g, b, a },
                ..
            } if approx_eq(width, 1.0)
                && approx_eq(height, 1.0)
                && approx_eq(r, 0.0)
                && approx_eq(g, 0.0)
                && approx_eq(b, 0.0)
                && approx_eq(a, 128.0 / 255.0)));
}

#[test]
fn src_zero_image_uses_black_pixel_crop() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 1920,
                "h": 1080,
                "source": [{ "id": "system", "path": "system.png" }],
                "image": [
                    { "id": 7, "src": 0, "x": 0, "y": 0, "w": 8, "h": 8 },
                    { "id": "black", "src": "bg", "x": 391, "y": 1080, "w": 8, "h": 8 }
                ],
                "destination": [
                    { "id": 7, "timer": 3, "dst": [{ "x": 0, "y": 0, "w": 1920, "h": 1080, "a": 200 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let images = document.image_map();
    let image = images.get("7").unwrap();
    let black = images.get("black").unwrap();
    let rect = skin_image_pixel_rect(image, &images);
    assert_eq!(rect, (black.x, black.y, black.w, black.h));
}

#[test]
fn src_zero_with_explicit_crop_keeps_pixel_rect() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 1920,
                "h": 1080,
                "source": [{ "id": "system", "path": "system.png" }],
                "image": [
                    { "id": "black", "src": "bg", "x": 391, "y": 1080, "w": 8, "h": 8 },
                    { "id": 15, "src": 0, "x": 16, "y": 0, "w": 8, "h": 8 }
                ]
            }
            "#,
    )
    .unwrap();
    let images = document.image_map();
    let image = images.get("15").unwrap();
    let rect = skin_image_pixel_rect(image, &images);
    assert_eq!(rect, (16, 0, 8, 8));
}

#[test]
fn image_negative_crop_size_uses_remaining_source_extent() {
    let image = SkinImageDef {
        id: "frame".to_string(),
        src: "src".to_string(),
        x: 10,
        y: 20,
        w: -1,
        h: -1,
        divx: 1,
        divy: 1,
        timer: None,
        cycle: 0,
        len: 0,
        ref_id: 0,
        click: 0,
        act: None,
        clickable: None,
    };

    let uv = skin_image_texture_region(&image, SkinImageSize { width: 110.0, height: 220.0 }, 0);

    assert!(approx_eq(uv.x, 10.0 / 110.0));
    assert!(approx_eq(uv.y, 20.0 / 220.0));
    assert!(approx_eq(uv.width, 100.0 / 110.0));
    assert!(approx_eq(uv.height, 200.0 / 220.0));
}

#[test]
fn failed_close_black_fades_in_over_fullscreen() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 1920,
                "h": 1080,
                "source": [{ "id": "system", "path": "system.png" }],
                "image": [{ "id": "black", "src": "bg", "x": 391, "y": 1080, "w": 8, "h": 8 }],
                "destination": [
                    { "id": "black", "loop": 1000, "timer": 3, "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 1920, "h": 1080, "a": 0 },
                        { "time": 1000, "a": 255 }
                    ] }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("system", 1920.0, 1080.0);

    let inactive = document.static_image_render_items(
        &sources,
        &SkinDrawState { failed_ms: None, ..SkinDrawState::default() },
    );
    let mid = document.static_image_render_items(
        &sources,
        &SkinDrawState { failed_ms: Some(500), ..SkinDrawState::default() },
    );
    let (_, _, failed_overlay) = document.static_render_items_split(
        &sources,
        &SkinDrawState { failed_ms: Some(500), ..SkinDrawState::default() },
        &SkinTextState::default(),
    );

    assert!(inactive.is_empty());
    assert_eq!(mid.len(), 1);
    assert_eq!(failed_overlay.len(), 1);
    assert!(matches!(mid[0], SkinRenderItem::Image {
                rect: Rect { width, height, .. },
                tint: Color { a, .. },
                ..
            } if approx_eq(width, 1.0)
                && approx_eq(height, 1.0)
                && approx_eq(a, 128.0 / 255.0)));
}

#[test]
fn sudden_slider_draws_above_disappear_line() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 720,
                "h": 720,
                "source": [{ "id": 12, "path": "cover.png" }],
                "slider": [
                    { "id": "lanecover", "src": 12, "x": 0, "y": 0, "w": 431, "h": 723, "angle": 2, "range": 723, "type": 4 }
                ],
                "hiddenCover": [
                    { "id": "hiddencover", "src": 12, "x": 0, "y": 0, "w": 431, "h": 723, "disapearLine": 357 }
                ],
                "destination": [
                    { "id": "lanecover", "dst": [{ "x": 20, "y": 1080, "w": 431, "h": 723 }] }
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

    let items = document.static_image_render_items(
        &sources,
        &SkinDrawState { lane_cover: 1.0, ..SkinDrawState::default() },
    );
    let SkinRenderItem::Image { rect, uv, .. } = &items[0] else {
        panic!("expected sudden+ lane cover image");
    };
    assert!(approx_eq(rect.height, 723.0 / 720.0));
    assert!(approx_eq(uv.height, 1.0));
}

#[test]
fn skin_state_number_maps_operating_time_refs() {
    let state = SkinDrawState { operating_time_ms: 90_061_234, ..SkinDrawState::default() };

    assert_eq!(skin_state_number(27, &state), Some(25));
    assert_eq!(skin_state_number(28, &state), Some(1));
    assert_eq!(skin_state_number(29, &state), Some(1));
}

#[test]
fn skin_state_number_maps_beatoraja_point_score() {
    let state = SkinDrawState {
        key_mode: KeyMode::K7,
        max_combo: 45,
        total_notes: 100,
        judge_counts: DisplayJudgeCounts {
            pgreat: 30,
            great: 20,
            good: 10,
            bad: 4,
            poor: 3,
            empty_poor: 2,
        },
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(100, &state), Some(89_500));

    let five_key = SkinDrawState { key_mode: KeyMode::K5, ..state };
    assert_eq!(skin_state_number(100, &five_key), Some(55_000));
}

#[test]
fn skin_state_maps_level_failcount_and_float_properties() {
    let select = SkinDrawState {
        select_screen: true,
        select_play_level: 12,
        difficulty: 4,
        select_ex_score: Some(0),
        select_play_count: 9,
        select_clear_count: 4,
        ..SkinDrawState::default()
    };
    for ref_id in 45..=49 {
        assert_eq!(skin_state_number(ref_id, &select), Some(12));
    }
    assert_eq!(skin_state_number(79, &select), Some(5));
    assert!(approx_eq(skin_state_float_number(103, &select).unwrap(), 1.2));
    assert_eq!(skin_state_float_number(105, &select), Some(0.0));
    assert!(approx_eq(skin_state_float_number(108, &select).unwrap(), 1.2));
    assert_eq!(skin_state_float_number(109, &select), Some(0.0));

    let folder = SkinDrawState {
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        ..select.clone()
    };
    assert_eq!(skin_state_number(45, &folder), None);
    assert_eq!(skin_state_number(79, &folder), None);

    let state = SkinDrawState {
        current_fps: 237,
        play_timer_ms: Some(125_000),
        ex_score: 80,
        total_notes: 100,
        past_notes: 50,
        judge_counts: DisplayJudgeCounts {
            pgreat: 20,
            great: 15,
            good: 10,
            bad: 4,
            poor: 1,
            ..DisplayJudgeCounts::default()
        },
        best_ex_score: Some(120),
        target_ex_score: Some(150),
        hispeed: 1.75,
        gauge: 42.5,
        skin_loaded: false,
        resource_load_progress: 0.426,
        average_duration_us: Some(12_345),
        average_timing_ms: Some(-1.25),
        stddev_timing_ms: Some(4.5),
        select_chart_density: 8.25,
        select_chart_peak_density: 12.5,
        select_chart_end_density: 3.75,
        select_chart_total_gauge: 350.0,
        ..SkinDrawState::default()
    };
    assert!(approx_eq(skin_state_float_number(111, &state).unwrap(), 0.8));
    assert!(approx_eq(skin_state_float_number(113, &state).unwrap(), 0.6));
    assert_eq!(skin_state_float_number(101, &state), Some(0.0));
    assert!(approx_eq(skin_state_float_number(102, &state).unwrap(), 0.426));
    assert_eq!(skin_state_float_number(103, &state), Some(0.0));
    assert_eq!(skin_state_float_number(140, &state), Some(0.0));
    assert_eq!(skin_state_float_number(146, &state), None);
    assert_eq!(skin_state_float_number(1102, &state), None);
    assert_eq!(skin_state_float_number(372, &state), None);
    assert_eq!(skin_state_float_number(9_999, &state), None);
    assert_eq!(skin_state_number(161, &state), Some(2));
    assert_eq!(skin_state_number(162, &state), Some(5));
    assert_eq!(skin_state_number(20, &state), Some(237));
    assert_eq!(skin_state_number(368, &state), Some(350));
    assert_eq!(skin_state_number(165, &state), Some(42));
}

#[test]
fn skin_value_evaluates_default_chart_total_count_expr() {
    let state = SkinDrawState {
        select_screen: true,
        select_total_notes: 2_000,
        select_chart_total_gauge: 500.0,
        ..SkinDrawState::default()
    };
    let value = SkinValueDef {
        value_expr: SKIN_EXPR_DEFAULT_CHART_TOTAL_COUNT.to_string(),
        ..SkinValueDef::default()
    };
    let expected = 7.605_f32 * 2_000.0 / (0.01 * 2_000.0 + 6.5) - 500.0;
    assert!(
        (skin_value_number(&value, &state).unwrap() as f32 - expected).abs() < 0.5,
        "expected ~{expected}, got {:?}",
        skin_value_number(&value, &state)
    );
}

#[test]
fn skin_image_act_uses_event_index_for_button_frame_row() {
    let image = SkinImageDef {
        id: "auto-judge".to_string(),
        src: "1".to_string(),
        x: 0,
        y: 0,
        w: 68,
        h: 99,
        divx: 1,
        divy: 3,
        timer: None,
        cycle: 0,
        len: 0,
        ref_id: 0,
        click: 0,
        act: Some(75),
        clickable: None,
    };
    let source_size = SkinImageSize { width: 68.0, height: 99.0 };
    let off = skin_image_texture_region_for_state(
        &image,
        source_size,
        0,
        Some(&SkinDrawState::default()),
        (0, 0, 68, 99),
    );
    let on = skin_image_texture_region_for_state(
        &image,
        source_size,
        0,
        Some(&SkinDrawState { judge_timing_auto_adjust: true, ..SkinDrawState::default() }),
        (0, 0, 68, 99),
    );

    assert!(approx_eq(off.y, 0.0));
    assert!(approx_eq(on.y, 1.0 / 3.0));
    assert!(approx_eq(on.height, 1.0 / 3.0));
}

#[test]
fn filter_nonzero_destination_returns_linear_filter_item() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "panel", "filter": 1, "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 10, "h": 10 }
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
            texture: SkinTextureId(3),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        },
    )]);

    let items = document.static_image_render_items(&sources, &SkinDrawState::default());

    assert_eq!(items.len(), 1);
    assert!(matches!(items[0], SkinRenderItem::Image { linear_filter: true, .. }));
}

#[test]
fn destination_angle_and_center_emit_rotated_image() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 100, "h": 100,
                "source": [{ "id": "src", "path": "a.png" }],
                "image": [{ "id": "img", "src": "src", "w": 10, "h": 10 }],
                "destination": [
                    { "id": "img", "center": 1, "dst": [
                        { "time": 0, "x": 10, "y": 20, "w": 30, "h": 40, "angle": 90 }
                    ]}
                ]
            }
            "#,
    )
    .unwrap();

    let sources = mock_source("src", 10.0, 10.0);
    let items = document.static_image_render_items(&sources, &SkinDrawState::default());

    assert_eq!(items.len(), 1);
    assert!(matches!(
        items[0],
        SkinRenderItem::RotatedImage { angle_deg, center, .. }
            if approx_eq(angle_deg, -90.0) && approx_eq(center.x, 0.0) && approx_eq(center.y, 1.0)
    ));
}

#[test]
fn negative_static_image_width_matches_beatoraja_horizontal_mirroring() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 1920, "h": 1080,
                "source": [{ "id": "frame-src", "path": "frame.png" }],
                "image": [{
                    "id": "table-level-frame", "src": "frame-src",
                    "x": 0, "y": 0, "w": 101, "h": 53
                }],
                "destination": [{
                    "id": "table-level-frame",
                    "dst": [{ "x": 1193, "y": 100, "w": -101, "h": 53 }]
                }]
            }
            "#,
    )
    .unwrap();

    let sources = mock_source("frame-src", 101.0, 53.0);
    let items = document.static_image_render_items(&sources, &SkinDrawState::default());

    assert_eq!(items.len(), 1);
    let SkinRenderItem::Image { rect, uv, .. } = &items[0] else { panic!() };
    assert!(approx_eq(rect.x, (1193.0 - 101.0) / 1920.0));
    assert!(approx_eq(rect.width, 101.0 / 1920.0));
    assert!(approx_eq(uv.x, 1.0));
    assert!(approx_eq(uv.width, -1.0));
}

#[test]
fn value_number_right_aligns_by_default() {
    // 3-digit number "42" in a 5-digit area (align=0, default right-aligned)
    // shiftbase=3 → first digit at position 3, second at 4
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 1280, "h": 720,
                "source": [{ "id": "src", "path": "num.png" }],
                "value": [{ "id": "val", "src": "src", "x": 0, "y": 0, "w": 100, "h": 20, "divx": 10, "digit": 5, "ref": 104 }],
                "destination": [
                    { "id": "val", "dst": [{ "time": 0, "x": 0, "y": 0, "w": 20, "h": 20 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let sources = mock_source("src", 100.0, 20.0);
    // combo=42, total_notes=100 → ref 104 = combo = 42 → 2 digits
    let state =
        SkinDrawState { elapsed_ms: 0, combo: 42, total_notes: 100, ..SkinDrawState::default() };
    let items = document.static_image_render_items(&sources, &state);

    // 2 digits in a 5-digit space, right-aligned: shiftbase=3
    // digit_width = 20/1280, digit_step = digit_width (space=0)
    // digit 0 ("4"): x = 0 + step * (3 + 0) - 0 = 3 * step
    // digit 1 ("2"): x = 0 + step * (3 + 1) - 0 = 4 * step
    assert_eq!(items.len(), 2);
    let digit_width = 20.0 / 1280.0;
    let SkinRenderItem::Image { rect: r0, .. } = &items[0] else { panic!() };
    let SkinRenderItem::Image { rect: r1, .. } = &items[1] else { panic!() };
    assert!(
        approx_eq(r0.x, 3.0 * digit_width),
        "first digit x={} expected {}",
        r0.x,
        3.0 * digit_width
    );
    assert!(
        approx_eq(r1.x, 4.0 * digit_width),
        "second digit x={} expected {}",
        r1.x,
        4.0 * digit_width
    );
}

#[test]
fn volume_number_uses_blank_padding_and_digit_cell_width() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 1920, "h": 1080,
                "source": [{ "id": "src", "path": "num.png" }],
                "value": [{ "id": "volume", "src": "src", "x": 2401, "y": 510, "w": 242, "h": 15, "divx": 11, "digit": 3, "ref": 57 }],
                "destination": [
                    { "id": "volume", "dst": [{ "time": 0, "x": 1717, "y": 360, "w": 22, "h": 15 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let sources = mock_source("src", 3200.0, 3200.0);
    let items = document.static_image_render_items(
        &sources,
        &SkinDrawState { select_master_volume: 0.37, ..SkinDrawState::default() },
    );

    assert_eq!(items.len(), 3);
    let SkinRenderItem::Image { rect: r0, uv: uv0, .. } = &items[0] else { panic!() };
    let SkinRenderItem::Image { rect: r1, uv: uv1, .. } = &items[1] else { panic!() };
    let SkinRenderItem::Image { rect: r2, uv: uv2, .. } = &items[2] else { panic!() };
    let digit_width = 22.0 / 1920.0;
    assert!(approx_eq(r0.width, digit_width));
    assert!(approx_eq(r1.width, digit_width));
    assert!(approx_eq(r2.width, digit_width));
    assert!(approx_eq(r1.x - r0.x, digit_width));
    assert!(approx_eq(r2.x - r1.x, digit_width));
    assert!(approx_eq(uv0.width, 22.0 / 3200.0));
    assert!(approx_eq(uv1.width, 22.0 / 3200.0));
    assert!(approx_eq(uv2.width, 22.0 / 3200.0));
    assert!(approx_eq(uv0.x, (2401.0 + 10.0 * 22.0) / 3200.0));
    assert!(approx_eq(uv1.x, (2401.0 + 3.0 * 22.0) / 3200.0));
    assert!(approx_eq(uv2.x, (2401.0 + 7.0 * 22.0) / 3200.0));
    assert!(
        approx_eq(uv0.width, 242.0 / 11.0 / 3200.0),
        "value sprite must be sliced into 11 cells, got uv.width={}",
        uv0.width
    );
}

#[test]
fn value_number_slices_source_with_beatoraja_integer_division() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 1280, "h": 720,
                "source": [{ "id": "src", "path": "num.png" }],
                "value": [{ "id": "volume", "src": "src", "x": 3114, "y": 0, "w": 99, "h": 12, "divx": 10, "digit": 3, "ref": 57, "align": 2 }],
                "destination": [
                    { "id": "volume", "dst": [{ "time": 0, "x": 560, "y": 480, "w": 12, "h": 12 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let source_width = 3224.0;
    let sources = mock_source("src", source_width, 1024.0);
    let items = document.static_image_render_items(
        &sources,
        &SkinDrawState { select_master_volume: 0.37, ..SkinDrawState::default() },
    );

    assert_eq!(items.len(), 2);
    let SkinRenderItem::Image { uv: uv0, .. } = &items[0] else { panic!() };
    let SkinRenderItem::Image { uv: uv1, .. } = &items[1] else { panic!() };
    assert!(
        approx_eq(uv0.width, 9.0 / source_width),
        "beatoraja slices 99px / 10 as 9px cells, got {}",
        uv0.width * source_width
    );
    assert!(approx_eq(uv0.x, (3114.0 + 3.0 * 9.0) / source_width));
    assert!(approx_eq(uv1.x, (3114.0 + 7.0 * 9.0) / source_width));
}

#[test]
fn value_number_left_aligns_when_align_1() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 1280, "h": 720,
                "source": [{ "id": "src", "path": "num.png" }],
                "value": [{ "id": "val", "src": "src", "x": 0, "y": 0, "w": 100, "h": 20, "divx": 10, "digit": 5, "align": 1, "ref": 104 }],
                "destination": [
                    { "id": "val", "dst": [{ "time": 0, "x": 0, "y": 0, "w": 20, "h": 20 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let sources = mock_source("src", 100.0, 20.0);
    let state =
        SkinDrawState { elapsed_ms: 0, combo: 42, total_notes: 100, ..SkinDrawState::default() };
    let items = document.static_image_render_items(&sources, &state);

    // left-aligned: shift = 3 * step, digit 0 at 0, digit 1 at step
    assert_eq!(items.len(), 2);
    let digit_width = 20.0 / 1280.0;
    let SkinRenderItem::Image { rect: r0, .. } = &items[0] else { panic!() };
    let SkinRenderItem::Image { rect: r1, .. } = &items[1] else { panic!() };
    assert!(approx_eq(r0.x, 0.0), "first digit x={} expected 0", r0.x);
    assert!(approx_eq(r1.x, digit_width), "second digit x={} expected {}", r1.x, digit_width);
}

#[test]
fn skin_state_number_hispeed_and_timeleft() {
    let state = SkinDrawState { hispeed: 1.5, timeleft_ms: 90_500, ..SkinDrawState::default() };
    // NUMBER_HISPEED (310) = integer part = 1
    assert_eq!(skin_state_number(310, &state), Some(1));
    // NUMBER_HISPEED_AFTERDOT (311) = decimal part × 100 = 50
    assert_eq!(skin_state_number(311, &state), Some(50));
    // NUMBER_TIMELEFT_MINUTE (163) = 90500 / 60000 = 1
    assert_eq!(skin_state_number(163, &state), Some(1));
    // NUMBER_TIMELEFT_SECOND (164) = (90500 / 1000) % 60 = 90 % 60 = 30
    assert_eq!(skin_state_number(164, &state), Some(30));
    let result_state = SkinDrawState {
        result_failed: Some(false),
        total_duration_ms: 183_000,
        ..SkinDrawState::default()
    };
    // Starseeker 系の Result BMS DATA は選曲詳細の曲長 ref を流用する。
    assert_eq!(skin_state_number(1163, &result_state), Some(3));
    assert_eq!(skin_state_number(1164, &result_state), Some(3));
}

#[test]
fn skin_state_number_maps_bmz_hispeed_mode_refs() {
    let normal = SkinDrawState {
        hispeed_mode_index: 0,
        total_duration_ms: 500,
        duration_green_ms: Some(300),
        ..SkinDrawState::default()
    };
    let floating = SkinDrawState {
        hispeed_mode_index: 1,
        target_green_number: 280,
        total_duration_ms: 500,
        duration_green_ms: Some(300),
        ..SkinDrawState::default()
    };
    let clamped = SkinDrawState { hispeed_mode_index: 9, ..floating.clone() };
    let mode_text = SkinTextDef { ref_id: 1900, ..SkinTextDef::default() };

    assert_eq!(skin_state_number(1900, &normal), Some(0));
    assert_eq!(skin_state_number(1901, &normal), Some(0));
    assert_eq!(skin_state_number(1902, &normal), Some(300));
    assert_eq!(skin_state_event_index(1900, &normal), 0);
    assert!(!test_skin_op(1901, &[], &normal));
    assert_eq!(
        skin_state_text_with_draw_state(&mode_text, Some(&normal), &SkinTextState::default()),
        "NHS"
    );

    assert_eq!(skin_state_number(1900, &floating), Some(1));
    assert_eq!(skin_state_number(1901, &floating), Some(1));
    assert_eq!(skin_state_number(1902, &floating), Some(280));
    assert_eq!(skin_state_event_index(1900, &floating), 1);
    assert!(test_skin_op(1901, &[], &floating));
    assert_eq!(
        skin_state_text_with_draw_state(&mode_text, Some(&floating), &SkinTextState::default()),
        "FHS"
    );

    assert_eq!(skin_state_number(1900, &clamped), Some(1));
}

#[test]
fn skin_image_index_number_separates_colliding_value_refs() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_clear_count: 99,
        select_gauge_auto_shift_index: 2,
        select_sort_index: 5,
        select_option_panel: 3,
        judge_timing_offset_ms: 42,
        select_chart_normal_notes: 900,
        select_max_bpm: 180.0,
        judge_rank: Some(3),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_image_index_number(78, &state), Some(2));
    assert_eq!(skin_state_number(78, &state), Some(99));

    assert_eq!(skin_image_index_number(12, &state), Some(5));
    assert_eq!(skin_state_number(12, &state), Some(42));

    assert_eq!(skin_image_index_number(350, &state), Some(0));
    assert_eq!(skin_state_number(350, &state), Some(900));

    assert_eq!(skin_image_index_number(400, &state), Some(0));
    assert_eq!(skin_state_number(400, &state), Some(3));
}

#[test]
fn skin_value_number_evaluates_value_expr() {
    let state = SkinDrawState {
        total_duration_ms: 305_000,
        duration_green_ms: Some(183_000),
        ..SkinDrawState::default()
    };
    let value = SkinValueDef {
        id: "lanecover-green".to_string(),
        src: String::new(),
        value_expr: "0.6*number(312)".to_string(),
        ..Default::default()
    };
    assert_eq!(skin_value_number(&value, &state), Some(183_000));
}

#[test]
fn skin_value_number_for_destination_prefers_value_expr_over_ref_zero_fallback() {
    let state = SkinDrawState {
        play_level: 12,
        total_duration_ms: 500,
        duration_green_ms: Some(300),
        ..SkinDrawState::default()
    };
    let value = SkinValueDef {
        id: "lanecover-green".to_string(),
        src: String::new(),
        value_expr: "0.6*number(312)".to_string(),
        ..Default::default()
    };
    assert_eq!(skin_value_number_for_destination(&value, &state, false), Some(300));
}

#[test]
fn skin_value_number_evaluates_floor_division_value_expr() {
    let state = SkinDrawState {
        total_notes: 74,
        judge_counts: DisplayJudgeCounts { pgreat: 1, great: 1, good: 1, ..Default::default() },
        ..SkinDrawState::default()
    };
    let value = SkinValueDef {
        id: "pscore".to_string(),
        src: String::new(),
        value_expr: "floor((100000*number(110)+70000*number(111)+40000*number(112))/number(74))"
            .to_string(),
        ..Default::default()
    };

    assert_eq!(skin_value_number(&value, &state), Some(2837));
}

#[test]
fn skin_value_number_evaluates_remain_rate_scaled_after_division() {
    let state = SkinDrawState {
        total_notes: 100,
        judge_counts: DisplayJudgeCounts {
            pgreat: 30,
            great: 20,
            good: 5,
            bad: 3,
            poor: 2,
            ..Default::default()
        },
        ..SkinDrawState::default()
    };
    let value = SkinValueDef {
            id: "remain-rate-num".to_string(),
            src: String::new(),
            value_expr:
                "(number(106)-number(110)-number(111)-number(112)-number(113)-number(114))/number(106)*100"
                    .to_string(),
            ..Default::default()
        };
    let afterdot = SkinValueDef {
            id: "remain-rate-adot-num".to_string(),
            src: String::new(),
            value_expr:
                "(number(106)-number(110)-number(111)-number(112)-number(113)-number(114))/number(106)*10000"
                    .to_string(),
            ..Default::default()
        };

    assert_eq!(skin_value_number(&value, &state), Some(40));
    assert_eq!(skin_value_number(&afterdot, &state), Some(4000));
}

#[test]
fn skin_value_number_truncates_lua_value_expr_like_beatoraja_integer_property() {
    let state = SkinDrawState {
        total_notes: 2480,
        judge_counts: DisplayJudgeCounts { pgreat: 1, ..Default::default() },
        adjusted_rate: Some(0.6),
        adjusted_rate_adot: Some(60),
        ..SkinDrawState::default()
    };
    let remain_integer = SkinValueDef {
            id: "remain-rate-num".to_string(),
            src: String::new(),
            value_expr:
                "(number(106)-number(110)-number(111)-number(112)-number(113)-number(114))/number(106)*100"
                    .to_string(),
            ..Default::default()
        };
    let remain_afterdot = SkinValueDef {
            id: "remain-rate-adot-num".to_string(),
            src: String::new(),
            value_expr:
                "(number(106)-number(110)-number(111)-number(112)-number(113)-number(114))/number(106)*10000"
                    .to_string(),
            ..Default::default()
        };
    let adjusted_integer = SkinValueDef {
        id: "adjusted-rate-num".to_string(),
        src: String::new(),
        value_expr: SKIN_EXPR_ADJUSTED_RATE.to_string(),
        ..Default::default()
    };

    assert_eq!(skin_value_number(&remain_integer, &state), Some(99));
    assert_eq!(skin_value_number(&remain_afterdot, &state), Some(9995));
    assert_eq!(skin_value_number(&adjusted_integer, &state), Some(0));
}

#[test]
fn skin_state_float_expr_evaluates_option_weighted_terms() {
    let expr = "0.102*option(180)*number(350)+0.09*option(181)*number(350)";
    let very_hard = SkinDrawState {
        judge_rank: Some(0),
        select_screen: true,
        select_total_notes: 100,
        ..SkinDrawState::default()
    };
    let hard = SkinDrawState {
        judge_rank: Some(1),
        select_screen: true,
        select_total_notes: 100,
        ..SkinDrawState::default()
    };

    assert!((skin_state_float_expr(expr, &very_hard).unwrap() - 10.2).abs() < 0.001);
    assert!((skin_state_float_expr(expr, &hard).unwrap() - 9.0).abs() < 0.001);
}

#[test]
fn skin_state_text_maps_string_refs() {
    let ir_ranking = crate::scene::ResultIrSnapshot {
        state: crate::scene::ResultIrState::Loaded,
        provider_name: crate::scene::ResultIrRankingName::from_display_name("rianIR"),
        user_name: crate::scene::ResultIrRankingName::from_display_name("hyrorre"),
        entries: [
            crate::scene::ResultIrRankingEntrySnapshot {
                rank: Some(1),
                ex_score: Some(2000),
                clear_index: Some(8),
                player_name: crate::scene::ResultIrRankingName::from_display_name("Alice"),
            },
            crate::scene::ResultIrRankingEntrySnapshot::default(),
            crate::scene::ResultIrRankingEntrySnapshot::default(),
            crate::scene::ResultIrRankingEntrySnapshot::default(),
            crate::scene::ResultIrRankingEntrySnapshot::default(),
            crate::scene::ResultIrRankingEntrySnapshot::default(),
            crate::scene::ResultIrRankingEntrySnapshot::default(),
            crate::scene::ResultIrRankingEntrySnapshot::default(),
            crate::scene::ResultIrRankingEntrySnapshot::default(),
            crate::scene::ResultIrRankingEntrySnapshot::default(),
        ],
        ..Default::default()
    };
    let state = SkinTextState {
        player_name: "BMZ Player",
        title: "My Title",
        subtitle: "Sub",
        artist: "Artist Name",
        subartist: "Feat. X",
        genre: "TRANCE",
        target: "RANK_AAA",
        ir_ranking: &ir_ranking,
        course_titles: [
            "Stage 1", "Stage 2", "Stage 3", "Stage 4", "Stage 5", "Stage 6", "Stage 7", "Stage 8",
            "Stage 9", "Stage 10",
        ],
        ..SkinTextState::default()
    };

    let make_text = |ref_id: i32| SkinTextDef {
        id: "t".to_string(),
        ref_id,
        constant_text: String::new(),
        ..SkinTextDef::default()
    };

    // STRING_TITLE (10)
    assert_eq!(skin_state_text(&make_text(10), &state), "My Title");
    // STRING_SUBTITLE (11)
    assert_eq!(skin_state_text(&make_text(11), &state), "Sub");
    // STRING_FULLTITLE (12) = title + " " + subtitle
    assert_eq!(skin_state_text(&make_text(12), &state), "My Title Sub");
    // STRING_GENRE (13)
    assert_eq!(skin_state_text(&make_text(13), &state), "TRANCE");
    // STRING_ARTIST (14)
    assert_eq!(skin_state_text(&make_text(14), &state), "Artist Name");
    // STRING_SUBARTIST (15)
    assert_eq!(skin_state_text(&make_text(15), &state), "Feat. X");
    // STRING_FULLARTIST (16) = artist + " " + subartist
    assert_eq!(skin_state_text(&make_text(16), &state), "Artist Name Feat. X");
    // STRING_RIVAL (1) is also target score player name during play in beatoraja.
    assert_eq!(skin_state_text(&make_text(1), &state), "RANK AAA");
    assert_eq!(
        skin_state_text(&make_text(1), &SkinTextState { rival: "Rival A", ..state.clone() }),
        "Rival A"
    );
    // STRING_PLAYER (2)
    assert_eq!(skin_state_text(&make_text(2), &state), "BMZ Player");
    // STRING_TARGET (3)
    assert_eq!(skin_state_text(&make_text(3), &state), "RANK AAA");
    // STRING_TARGETNAME_P1/N1 (209/210)
    assert_eq!(skin_state_text(&make_text(209), &state), "RANK AAA-");
    assert_eq!(skin_state_text(&make_text(210), &state), "RANK MAX-");
    assert_eq!(select_target_name("RIVAL_2"), "RIVAL 2");
    assert_eq!(select_target_name("AAA"), "RANK AAA");
    // STRING_RANKINGNAME1..10
    assert_eq!(skin_state_text(&make_text(120), &state), "Alice");
    assert_eq!(skin_state_text(&make_text(121), &state), "");
    // STRING_COURSE1_TITLE..10_TITLE (150..159)
    assert_eq!(skin_state_text(&make_text(150), &state), "Stage 1");
    assert_eq!(skin_state_text(&make_text(159), &state), "Stage 10");
    // STRING_IR_NAME / STRING_IR_USERNAME
    assert_eq!(skin_state_text(&make_text(1020), &state), "rianIR");
    assert_eq!(skin_state_text(&make_text(1021), &state), "hyrorre");
    // Unknown ref → empty
    assert_eq!(skin_state_text(&make_text(99), &state), "");

    let m_select_bar_text =
        SkinTextDef { id: "default_songlist2_bartext".to_string(), ..SkinTextDef::default() };
    assert_eq!(
        skin_state_text(
            &m_select_bar_text,
            &SkinTextState { bar_text: "Song Title", ..SkinTextState::default() },
        ),
        "Song Title"
    );
}

#[test]
fn skin_state_text_formats_bmz_number_ref_extension() {
    let text = SkinTextDef {
        id: "gauge_text".to_string(),
        number_ref: Some(107),
        prefix: "GAUGE ".to_string(),
        suffix: "%".to_string(),
        ..SkinTextDef::default()
    };
    let draw_state = SkinDrawState { gauge: 78.6, ..SkinDrawState::default() };

    assert_eq!(
        skin_state_text_with_draw_state(&text, Some(&draw_state), &SkinTextState::default()),
        "GAUGE 78%"
    );
    assert_eq!(skin_state_text(&text, &SkinTextState::default()), "");
}

#[test]
fn text_render_item_applies_search_word_alpha_multiplier_for_ref_30() {
    let document: SkinDocument =
        serde_json::from_value(serde_json::json!({ "w": 1920, "h": 1080 })).unwrap();
    let text = SkinTextDef { id: "search".to_string(), ref_id: 30, ..SkinTextDef::default() };
    let frame = ResolvedSkinFrame { w: 100, h: 24, ..ResolvedSkinFrame::default() };
    let state =
        SkinTextState { search_word: "hello", search_word_alpha: 0.5, ..SkinTextState::default() };
    let item = document.text_render_item(&text, frame, &state).unwrap();
    match item {
        SkinRenderItem::Text { style, .. } => {
            // frame.a=255 (1.0) * 0.5 = 0.5
            assert!((style.color.a - 0.5).abs() < 1e-4, "got alpha {}", style.color.a);
        }
        other => panic!("expected SkinRenderItem::Text, got {other:?}"),
    }
}

#[test]
fn text_render_item_keeps_empty_search_word_with_caret() {
    let document: SkinDocument =
        serde_json::from_value(serde_json::json!({ "w": 1920, "h": 1080 })).unwrap();
    let text = SkinTextDef { id: "search".to_string(), ref_id: 30, ..SkinTextDef::default() };
    let frame = ResolvedSkinFrame { w: 100, h: 24, ..ResolvedSkinFrame::default() };
    let state = SkinTextState {
        search_word: "",
        search_caret_byte_index: Some(0),
        ..SkinTextState::default()
    };

    let item = document.text_render_item(&text, frame, &state).unwrap();

    assert!(matches!(
        item,
        SkinRenderItem::Text { text, caret: Some(TextCaret { byte_index: 0, .. }), .. }
            if text.is_empty()
    ));
}

#[test]
fn text_render_item_leaves_alpha_unchanged_for_other_refs() {
    let document: SkinDocument =
        serde_json::from_value(serde_json::json!({ "w": 1920, "h": 1080 })).unwrap();
    let text = SkinTextDef {
        id: "title".to_string(),
        ref_id: 10, // title, not search
        ..SkinTextDef::default()
    };
    let frame = ResolvedSkinFrame { w: 100, h: 24, ..ResolvedSkinFrame::default() };
    let state = SkinTextState {
        title: "song name",
        search_word_alpha: 0.1, // should be ignored for non-search refs
        ..SkinTextState::default()
    };
    let item = document.text_render_item(&text, frame, &state).unwrap();
    match item {
        SkinRenderItem::Text { style, .. } => {
            assert!((style.color.a - 1.0).abs() < 1e-4, "got alpha {}", style.color.a);
        }
        other => panic!("expected SkinRenderItem::Text, got {other:?}"),
    }
}

#[test]
fn text_render_item_separates_bitmap_font_size_from_destination_height() {
    let document: SkinDocument = serde_json::from_value(serde_json::json!({
        "w": 100,
        "h": 100,
        "font": [
            { "id": "bitmap", "path": "artist.fnt" },
            { "id": "vector", "path": "artist.ttf" }
        ]
    }))
    .unwrap();
    let frame = ResolvedSkinFrame { w: 80, h: 28, ..ResolvedSkinFrame::default() };
    let state = SkinTextState::default();
    let bitmap_text = SkinTextDef {
        id: "artist".to_string(),
        font: "result:bitmap".to_string(),
        size: 17,
        constant_text: "Aoi".to_string(),
        ..SkinTextDef::default()
    };
    let vector_text = SkinTextDef {
        id: "artist_vector".to_string(),
        font: "vector".to_string(),
        size: 17,
        constant_text: "Aoi".to_string(),
        ..SkinTextDef::default()
    };

    let bitmap_item = document.text_render_item(&bitmap_text, frame, &state).unwrap();
    let vector_item = document.text_render_item(&vector_text, frame, &state).unwrap();

    match bitmap_item {
        SkinRenderItem::Text { style, .. } => {
            assert!(approx_eq(style.size, 0.28), "got {}", style.size);
            assert_eq!(style.bitmap_size, Some(0.17));
        }
        other => panic!("expected SkinRenderItem::Text, got {other:?}"),
    }
    match vector_item {
        SkinRenderItem::Text { style, .. } => {
            assert!(approx_eq(style.size, 0.28), "got {}", style.size);
            assert_eq!(style.bitmap_size, None);
        }
        other => panic!("expected SkinRenderItem::Text, got {other:?}"),
    }
}

#[test]
fn skin_state_text_uses_constant_text_over_ref_id() {
    let state = SkinTextState { title: "Ignored", ..SkinTextState::default() };
    let text = SkinTextDef {
        id: "t".to_string(),
        ref_id: 10,
        constant_text: "Hardcoded".to_string(),
        ..SkinTextDef::default()
    };
    assert_eq!(skin_state_text(&text, &state), "Hardcoded");
}

#[test]
fn full_label_handles_empty_components() {
    // both empty
    assert_eq!(full_label("", ""), "");
    // only primary
    assert_eq!(full_label("Title", ""), "Title");
    // only secondary
    assert_eq!(full_label("", "Sub"), "Sub");
    // both present
    assert_eq!(full_label("Title", "Sub"), "Title Sub");
}

#[test]
fn loop_at_cycle_end_holds_final_frame() {
    // loop == cycle（終端へループバック）: 1回再生して最終フレームを保持する。
    // lane-bg(loop:1000,終端1000) や keybeam(loop:100,終端100) の挙動。
    assert_eq!(resolve_loop_elapsed(1000, 500, 1000), 500); // 再生中
    assert_eq!(resolve_loop_elapsed(1000, 1000, 1000), 1000); // 終端
    assert_eq!(resolve_loop_elapsed(1000, 5000, 1000), 1000); // 終端超過 → 保持
    // loop > cycle も終端で停止する。
    assert_eq!(resolve_loop_elapsed(300, 5000, 200), 200);
}

#[test]
fn loop_before_cycle_end_repeats_segment() {
    // loop < cycle: [loop, cycle) 区間を繰り返す。
    assert_eq!(resolve_loop_elapsed(0, 150, 200), 150); // 再生中はそのまま
    assert_eq!(resolve_loop_elapsed(0, 350, 200), 150); // 350 → 150 へループ
    assert_eq!(resolve_loop_elapsed(100, 350, 200), 150); // (350-100)%100+100
}

#[test]
fn negative_loop_destination_disappears_after_end() {
    // loop:-1 の destination はアニメーション終端を過ぎると描画されない（READY/ボム）。
    let destination: SkinDestinationDef = serde_json::from_str(
        r#"{ "id": "ready", "loop": -1, "dst": [
                { "time": 0, "x": 0, "y": 0, "w": 10, "h": 10, "a": 0 },
                { "time": 1000, "a": 255 }
            ]}"#,
    )
    .unwrap();
    assert!(resolve_destination_frame(&destination, 500, &[], &SkinDrawState::default()).is_some());
    assert!(
        resolve_destination_frame(&destination, 1000, &[], &SkinDrawState::default()).is_some()
    );
    assert!(
        resolve_destination_frame(&destination, 1001, &[], &SkinDrawState::default()).is_none()
    );
}

#[test]
fn single_frame_destination_preserves_start_and_loop_semantics() {
    let destination: SkinDestinationDef = serde_json::from_str(
        r#"{ "id": "flash", "dst": [{ "time": 1000, "x": 2, "y": 3, "w": 10, "h": 20 }] }"#,
    )
    .unwrap();

    assert!(resolve_destination_frame(&destination, 999, &[], &SkinDrawState::default()).is_none());
    let frame = resolve_destination_frame(&destination, 1000, &[], &SkinDrawState::default())
        .expect("single frame starts at its keyframe time");
    assert_eq!((frame.x, frame.y, frame.w, frame.h), (2, 3, 10, 20));

    let disappearing: SkinDestinationDef = serde_json::from_str(
            r#"{ "id": "flash", "loop": -1, "dst": [{ "time": 1000, "x": 2, "y": 3, "w": 10, "h": 20 }] }"#,
        )
        .unwrap();
    assert!(
        resolve_destination_frame(&disappearing, 1001, &[], &SkinDrawState::default()).is_none()
    );
}

#[test]
fn destination_frame_h_expr_resolves_fast_slow_breakdown_height() {
    let destination: SkinDestinationDef = serde_json::from_str(&format!(
        r#"{{
                "id": "graph_r",
                "dst": [
                    {{ "time": 0, "x": 0, "y": 0, "w": 10, "h": 0 }},
                    {{ "time": 1000, "h_expr": "{}(422)" }}
                ]
            }}"#,
        SKIN_EXPR_FAST_SLOW_BREAKDOWN_HEIGHT
    ))
    .unwrap();
    let state = SkinDrawState {
        fast_slow_counts: Some(crate::snapshot::FastSlowJudgeCounts {
            slow_empty_poor: 5,
            slow_poor: 10,
            ..crate::snapshot::FastSlowJudgeCounts::default()
        }),
        ..SkinDrawState::default()
    };

    let frame = resolve_destination_frame(&destination, 1000, &[], &state).unwrap();

    assert_eq!(frame.h, 50);
}

#[test]
fn text_destination_rect_for_ref_returns_normalized_first_frame() {
    let document: SkinDocument = serde_json::from_value(serde_json::json!({
        "w": 1280,
        "h": 720,
        "text": [
            { "id": "searchword", "ref": 30, "font": "f" },
            { "id": "title", "ref": 10, "font": "f" }
        ],
        "destination": [
            {
                "id": "title",
                "dst": [{ "x": 0, "y": 0, "w": 100, "h": 30 }]
            },
            {
                "id": "searchword",
                "dst": [{ "x": 640, "y": 360, "w": 320, "h": 36 }]
            }
        ]
    }))
    .unwrap();

    let rect = document.text_destination_rect_for_ref(30).unwrap();
    assert!(approx_eq(rect.0, 0.5));
    // skin y=360, h=36 → flipped: (720 - 396) / 720 = 0.45
    assert!(approx_eq(rect.1, 0.45));
    assert!(approx_eq(rect.2, 0.25));
    assert!(approx_eq(rect.3, 0.05));

    assert!(document.text_destination_rect_for_ref(999).is_none());
}
