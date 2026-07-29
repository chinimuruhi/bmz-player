use super::*;

#[test]
fn bundled_default_skin_manifest_defines_play_lane_images() {
    let manifest = default_skin_manifest();
    let note = manifest.play_note_image();
    let receptor = manifest.play_receptor_image();
    let judge_line = manifest.play_judge_line_image();
    let gauge_frame = manifest.play_gauge_frame_image();
    let gauge_fill = manifest.play_gauge_fill_image();
    let combo_panel = manifest.play_combo_panel_image(true);
    let combo_panel_inactive = manifest.play_combo_panel_image(false);

    assert_eq!(note.texture, 1);
    assert_eq!(note.texture_for_lane(Lane::Key1), 1);
    assert_eq!(note.texture_for_lane(Lane::Key2), 2);
    assert_eq!(note.texture_for_lane(Lane::Key4), 2);
    assert_eq!(note.texture_for_lane(Lane::Key6), 2);
    assert_eq!(note.texture_for_lane(Lane::Scratch), 3);
    assert_eq!(note.uv, TextureRegion::default());
    assert_eq!(receptor.texture, 4);
    assert_eq!(receptor.texture_for_lane(Lane::Key1), 4);
    assert_eq!(receptor.texture_for_lane(Lane::Key2), 5);
    assert_eq!(receptor.texture_for_lane(Lane::Key4), 5);
    assert_eq!(receptor.texture_for_lane(Lane::Key6), 5);
    assert_eq!(receptor.texture_for_lane(Lane::Scratch), 6);
    assert_eq!(receptor.uv, TextureRegion::default());
    assert_eq!(judge_line.texture, 7);
    assert_eq!(judge_line.uv, TextureRegion::default());
    assert_eq!(gauge_frame.texture, 8);
    assert_eq!(gauge_frame.scale, SkinImageScale::NineSlice);
    assert_eq!(gauge_frame.source_size, Some(SkinImageSize { width: 12.0, height: 48.0 }));
    assert!(matches!(
        gauge_frame.border,
        Some(SkinImageBorder { unit: SkinImageBorderUnit::Pixels, .. })
    ));
    assert_eq!(gauge_fill.texture, 9);
    assert_eq!(gauge_fill.source_size, Some(SkinImageSize { width: 8.0, height: 48.0 }));
    assert_eq!(combo_panel.texture, 10);
    assert_eq!(combo_panel.scale, SkinImageScale::NineSlice);
    assert_eq!(combo_panel.source_size, Some(SkinImageSize { width: 48.0, height: 16.0 }));
    assert!(matches!(
        combo_panel.border,
        Some(SkinImageBorder { unit: SkinImageBorderUnit::Pixels, .. })
    ));
    assert_eq!(combo_panel_inactive.texture, 11);
    assert_eq!(combo_panel_inactive.scale, SkinImageScale::NineSlice);
    assert_eq!(combo_panel_inactive.source_size, Some(SkinImageSize { width: 48.0, height: 16.0 }));
    assert!(matches!(
        combo_panel_inactive.border,
        Some(SkinImageBorder { unit: SkinImageBorderUnit::Pixels, .. })
    ));
}

#[test]
fn bga_destination_renders_placeholder_only_when_chart_has_bga() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "bga": { "id": "bga" },
                "destination": [
                    { "id": "bga", "dst": [{ "x": 10, "y": 20, "w": 30, "h": 40, "a": 128 }] }
                ]
            }
            "#,
    )
    .unwrap();

    let no_bga_items = document.static_render_items(
        &HashMap::new(),
        &SkinDrawState { has_bga: false, ..SkinDrawState::default() },
        &SkinTextState::default(),
    );
    let bga_items = document.static_render_items(
        &HashMap::new(),
        &SkinDrawState { has_bga: true, ..SkinDrawState::default() },
        &SkinTextState::default(),
    );

    assert!(no_bga_items.is_empty());
    assert!(matches!(
        bga_items.as_slice(),
        [SkinRenderItem::Rect {
            rect: Rect { x, y, width, height },
            color: Color { r: 0.0, g: 0.0, b: 0.0, a },
            ..
        }] if approx_eq(*x, 0.1)
            && approx_eq(*y, 0.4)
            && approx_eq(*width, 0.3)
            && approx_eq(*height, 0.4)
            && approx_eq(*a, 128.0 / 255.0)
    ));
}

#[test]
fn bga_destination_is_hidden_when_bga_is_disabled() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "bga": { "id": "bga" },
                "destination": [
                    { "id": "bga", "dst": [{ "x": 10, "y": 20, "w": 30, "h": 40 }] }
                ]
            }
            "#,
    )
    .unwrap();

    let items = document.static_render_items(
        &HashMap::new(),
        &SkinDrawState {
            has_bga: true,
            bga_enabled: false,
            bga_base: Some(SkinBgaFrame {
                texture: SkinTextureId(20000),
                source_size: SkinImageSize { width: 256.0, height: 256.0 },
                tint_r: 1.0,
                tint_g: 1.0,
                tint_b: 1.0,
                tint_a: 1.0,
                is_video: false,
            }),
            ..SkinDrawState::default()
        },
        &SkinTextState::default(),
    );

    assert!(items.is_empty());
}

#[test]
fn bga_option_conditions_still_reflect_song_bga_when_disabled() {
    let disabled = SkinDrawState { has_bga: true, bga_enabled: false, ..SkinDrawState::default() };

    assert!(test_skin_op(40, &[], &disabled));
    assert!(!test_skin_op(41, &[], &disabled));
    assert!(!test_skin_op(170, &[], &disabled));
    assert!(test_skin_op(171, &[], &disabled));

    let enabled_no_song_bga =
        SkinDrawState { has_bga: false, bga_enabled: true, ..SkinDrawState::default() };
    assert!(!test_skin_op(40, &[], &enabled_no_song_bga));
    assert!(test_skin_op(41, &[], &enabled_no_song_bga));
    assert!(test_skin_op(170, &[], &enabled_no_song_bga));
    assert!(!test_skin_op(171, &[], &enabled_no_song_bga));
}

#[test]
fn score_save_and_play_mode_ops_are_scene_scoped() {
    let select = SkinDrawState::default();
    let normal = SkinDrawState {
        play_screen: true,
        score_save_enabled: Some(true),
        ..SkinDrawState::default()
    };
    let replay = SkinDrawState {
        play_screen: true,
        replay_playback: true,
        score_save_enabled: Some(false),
        ..SkinDrawState::default()
    };
    let practice = SkinDrawState {
        play_screen: true,
        practice_mode: true,
        score_save_enabled: Some(false),
        ..SkinDrawState::default()
    };

    assert!(!test_skin_op(60, &[], &select));
    assert!(!test_skin_op(61, &[], &select));
    assert!(!test_skin_op(82, &[], &select));
    assert!(test_skin_op(61, &[], &normal));
    assert!(test_skin_op(82, &[], &normal));
    assert!(!test_skin_op(84, &[], &normal));
    assert!(test_skin_op(60, &[], &replay));
    assert!(!test_skin_op(82, &[], &replay));
    assert!(test_skin_op(84, &[], &replay));
    assert!(test_skin_op(60, &[], &practice));
    assert!(test_skin_op(82, &[], &practice));
    assert!(test_skin_op(1080, &[], &practice));
}

#[test]
fn play_asset_and_loading_ops_reflect_skin_state() {
    let unloaded = SkinDrawState { skin_loaded: false, ..SkinDrawState::default() };
    assert!(test_skin_op(80, &[], &unloaded));
    assert!(!test_skin_op(81, &[], &unloaded));

    let loaded = SkinDrawState::default();
    assert!(!test_skin_op(80, &[], &loaded));
    assert!(test_skin_op(81, &[], &loaded));
    assert!(test_skin_op(190, &[], &loaded));
    assert!(!test_skin_op(191, &[], &loaded));
    assert!(test_skin_op(194, &[], &loaded));
    assert!(!test_skin_op(195, &[], &loaded));

    let with_stagefile = SkinDrawState { has_stagefile: true, ..SkinDrawState::default() };
    assert!(!test_skin_op(190, &[], &with_stagefile));
    assert!(test_skin_op(191, &[], &with_stagefile));

    let with_backbmp = SkinDrawState { has_backbmp: true, ..SkinDrawState::default() };
    assert!(!test_skin_op(194, &[], &with_backbmp));
    assert!(test_skin_op(195, &[], &with_backbmp));
}

#[test]
fn lane_cover_changing_op_is_true_while_lane_cover_is_visible() {
    assert!(!test_skin_op(270, &[], &SkinDrawState::default()));
    assert!(!test_skin_op(
        270,
        &[],
        &SkinDrawState { lane_cover: 0.2, ..SkinDrawState::default() }
    ));
    assert!(test_skin_op(
        270,
        &[],
        &SkinDrawState { lane_cover_changing: true, ..SkinDrawState::default() }
    ));
    assert!(test_skin_op(
        271,
        &[],
        &SkinDrawState { lanecover_enabled: true, ..SkinDrawState::default() }
    ));
}

#[test]
fn play_key_mode_ops_use_play_key_mode() {
    let play_14k = SkinDrawState { key_mode: KeyMode::K14, ..SkinDrawState::default() };

    assert!(test_skin_op(162, &[], &play_14k));
    assert!(!test_skin_op(160, &[], &play_14k));

    let play_6k = SkinDrawState { key_mode: KeyMode::K6, ..SkinDrawState::default() };
    assert!(test_skin_op(SKIN_OPTION_BMZ_KEY_MODE_BASE + 2, &[], &play_6k));
    assert!(test_skin_op(SKIN_OPTION_BMZ_NO_SCRATCH, &[], &play_6k));
    assert_eq!(skin_state_number(SKIN_REF_BMZ_KEY_MODE, &play_6k), Some(6));
}

#[test]
fn play_rank_ops_reflect_current_ex_score() {
    let aa_state = SkinDrawState {
        ex_score: 1556,
        total_notes: 1000,
        past_notes: 1000,
        ..SkinDrawState::default()
    };
    let aaa_state = SkinDrawState {
        ex_score: 1800,
        total_notes: 1000,
        past_notes: 1000,
        ..SkinDrawState::default()
    };
    let current_aaa_state = SkinDrawState {
        ex_score: 90,
        total_notes: 1000,
        past_notes: 50,
        ..SkinDrawState::default()
    };
    let before_first_note_state = SkinDrawState { total_notes: 1000, ..SkinDrawState::default() };

    assert!(test_skin_op(201, &[], &aa_state));
    assert!(!test_skin_op(200, &[], &aa_state));
    assert!(test_skin_op(200, &[], &aaa_state));
    assert!(test_skin_op(200, &[], &current_aaa_state));
    assert!(test_skin_op(200, &[], &before_first_note_state));
}

#[test]
fn bga_destination_renders_current_bga_images() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "bga": { "id": "bga" },
                "destination": [
                    { "id": "bga", "stretch": 1, "dst": [{ "x": 10, "y": 20, "w": 30, "h": 40, "a": 128 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let items = document.static_render_items(
        &HashMap::new(),
        &SkinDrawState {
            has_bga: true,
            bga_base: Some(SkinBgaFrame {
                texture: SkinTextureId(20000),
                source_size: SkinImageSize { width: 256.0, height: 128.0 },
                tint_r: 1.0,
                tint_g: 1.0,
                tint_b: 1.0,
                tint_a: 1.0,
                is_video: false,
            }),
            bga_layer: Some(SkinBgaFrame {
                texture: SkinTextureId(20001),
                source_size: SkinImageSize { width: 256.0, height: 256.0 },
                tint_r: 1.0,
                tint_g: 1.0,
                tint_b: 1.0,
                tint_a: 1.0,
                is_video: false,
            }),
            ..SkinDrawState::default()
        },
        &SkinTextState::default(),
    );

    assert!(matches!(
        items.as_slice(),
        [
            SkinRenderItem::Image {
                texture: SkinTextureId(20000),
                rect: Rect { x, y, width, height },
                tint: Color { a, .. },
                ..
            },
            SkinRenderItem::Image { texture: SkinTextureId(20001), .. },
        ] if approx_eq(*x, 0.1)
            && approx_eq(*y, 0.525)
            && approx_eq(*width, 0.3)
            && approx_eq(*height, 0.15)
            && approx_eq(*a, 128.0 / 255.0)
    ));
}

#[test]
fn bga_destination_renders_poor_bga_instead_of_base_and_layer() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "bga": { "id": "bga" },
                "destination": [
                    { "id": "bga", "dst": [{ "x": 10, "y": 20, "w": 30, "h": 40 }] }
                ]
            }
            "#,
    )
    .unwrap();

    let items = document.static_render_items(
        &HashMap::new(),
        &SkinDrawState {
            has_bga: true,
            bga_base: Some(SkinBgaFrame {
                texture: SkinTextureId(20000),
                source_size: SkinImageSize { width: 256.0, height: 256.0 },
                tint_r: 1.0,
                tint_g: 1.0,
                tint_b: 1.0,
                tint_a: 1.0,
                is_video: false,
            }),
            bga_layer: Some(SkinBgaFrame {
                texture: SkinTextureId(20001),
                source_size: SkinImageSize { width: 256.0, height: 256.0 },
                tint_r: 1.0,
                tint_g: 1.0,
                tint_b: 1.0,
                tint_a: 1.0,
                is_video: false,
            }),
            bga_poor: Some(SkinBgaFrame {
                texture: SkinTextureId(20002),
                source_size: SkinImageSize { width: 256.0, height: 256.0 },
                tint_r: 1.0,
                tint_g: 1.0,
                tint_b: 1.0,
                tint_a: 1.0,
                is_video: false,
            }),
            ..SkinDrawState::default()
        },
        &SkinTextState::default(),
    );

    assert!(matches!(
        items.as_slice(),
        [SkinRenderItem::Image { texture: SkinTextureId(20002), .. }]
    ));
}

#[test]
fn bga_destination_uses_profile_stretch_when_destination_omits_stretch() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "bga": { "id": "bga" },
                "destination": [
                    { "id": "bga", "dst": [{ "x": 10, "y": 20, "w": 30, "h": 40 }] }
                ]
            }
            "#,
    )
    .unwrap();

    let items = document.static_render_items(
        &HashMap::new(),
        &SkinDrawState {
            has_bga: true,
            bga_base: Some(SkinBgaFrame {
                texture: SkinTextureId(20000),
                source_size: SkinImageSize { width: 256.0, height: 128.0 },
                tint_r: 1.0,
                tint_g: 1.0,
                tint_b: 1.0,
                tint_a: 1.0,
                is_video: false,
            }),
            bga_stretch: 1,
            ..SkinDrawState::default()
        },
        &SkinTextState::default(),
    );

    assert!(matches!(
        items.as_slice(),
        [SkinRenderItem::Image {
            texture: SkinTextureId(20000),
            rect: Rect { x, y, width, height },
            ..
        }] if approx_eq(*x, 0.1)
            && approx_eq(*y, 0.525)
            && approx_eq(*width, 0.3)
            && approx_eq(*height, 0.15)
    ));
}

#[test]
fn bga_destination_stretch_overrides_profile_stretch() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "bga": { "id": "bga" },
                "destination": [
                    { "id": "bga", "stretch": 0, "dst": [{ "x": 10, "y": 20, "w": 30, "h": 40 }] }
                ]
            }
            "#,
    )
    .unwrap();

    let items = document.static_render_items(
        &HashMap::new(),
        &SkinDrawState {
            has_bga: true,
            bga_base: Some(SkinBgaFrame {
                texture: SkinTextureId(20000),
                source_size: SkinImageSize { width: 256.0, height: 128.0 },
                tint_r: 1.0,
                tint_g: 1.0,
                tint_b: 1.0,
                tint_a: 1.0,
                is_video: false,
            }),
            bga_stretch: 1,
            ..SkinDrawState::default()
        },
        &SkinTextState::default(),
    );

    assert!(matches!(
        items.as_slice(),
        [SkinRenderItem::Image {
            texture: SkinTextureId(20000),
            rect: Rect { x, y, width, height },
            ..
        }] if approx_eq(*x, 0.1)
            && approx_eq(*y, 0.4)
            && approx_eq(*width, 0.3)
            && approx_eq(*height, 0.4)
    ));
}

#[test]
fn song_bga_options_are_evaluated_from_draw_state() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": "src", "path": "dummy.png" }],
                "image": [
                    { "id": "no-bga", "src": "src", "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "has-bga", "src": "src", "x": 0, "y": 0, "w": 10, "h": 10 }
                ],
                "destination": [
                    { "id": "no-bga", "op": [170], "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "has-bga", "op": [171], "dst": [{ "x": 20, "y": 0, "w": 10, "h": 10 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "src".to_string(),
        SkinDocumentTexture {
            source_id: "src".to_string(),
            texture: SkinTextureId(1),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        },
    )]);

    let no_bga_items = document.static_image_render_items(
        &sources,
        &SkinDrawState { has_bga: false, ..SkinDrawState::default() },
    );
    let bga_items = document.static_image_render_items(
        &sources,
        &SkinDrawState { has_bga: true, ..SkinDrawState::default() },
    );

    assert!(matches!(
        no_bga_items.as_slice(),
        [SkinRenderItem::Image { rect: Rect { x, .. }, .. }] if approx_eq(*x, 0.0)
    ));
    assert!(matches!(
        bga_items.as_slice(),
        [SkinRenderItem::Image { rect: Rect { x, .. }, .. }] if approx_eq(*x, 0.2)
    ));
}

#[test]
fn static_render_items_split_at_notes_marker() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [
                    { "id": "behind", "src": 1, "x": 0, "y": 0, "w": 8, "h": 8 },
                    { "id": "cover", "src": 1, "x": 0, "y": 0, "w": 8, "h": 8 },
                    { "id": "frame", "src": 1, "x": 0, "y": 0, "w": 8, "h": 8 }
                ],
                "destination": [
                    { "id": "behind", "dst": [{ "x": 0, "y": 0, "w": 100, "h": 100 }] },
                    { "id": "notes" },
                    { "id": "cover", "dst": [{ "x": 10, "y": 10, "w": 20, "h": 20 }] },
                    { "id": "frame", "dst": [{ "x": 5, "y": 5, "w": 90, "h": 90 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("1", 8.0, 8.0);

    let (behind, front, failed_overlay) = document.static_render_items_split(
        &sources,
        &SkinDrawState::default(),
        &SkinTextState::default(),
    );

    // `{"id":"notes"}` マーカーより前の destination は背面、後ろは前面に入る。
    assert_eq!(behind.len(), 1, "behind = destinations before the notes marker");
    assert_eq!(front.len(), 2, "front = destinations after the notes marker");
    assert!(failed_overlay.is_empty());
    // 結合版 static_render_items は behind→front→failed の順で全アイテムを返す。
    let all = document.static_render_items(
        &sources,
        &SkinDrawState::default(),
        &SkinTextState::default(),
    );
    assert_eq!(all.len(), 3);
}

#[test]
fn pre_notes_lift_line_at_note_origin_renders_in_front() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 720,
                "h": 720,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [
                    { "id": "backdrop", "src": 1, "x": 0, "y": 0, "w": 8, "h": 8 },
                    { "id": 15, "src": 1, "x": 16, "y": 0, "w": 8, "h": 8 },
                    { "id": "note", "src": 1, "x": 0, "y": 0, "w": 51, "h": 36 }
                ],
                "destination": [
                    { "id": "backdrop", "dst": [{ "x": 0, "y": 0, "w": 720, "h": 720 }] },
                    { "id": 15, "offset": 3, "dst": [{ "x": 76, "y": 357, "w": 431, "h": 8 }] },
                    { "id": "notes" }
                ],
                "note": {
                    "id": "notes",
                    "note": ["note"],
                    "dst": [{ "x": 168, "y": 345, "w": 51, "h": 723 }]
                }
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("1", 720.0, 720.0);

    let (behind, front, failed_overlay) = document.static_render_items_split(
        &sources,
        &SkinDrawState::default(),
        &SkinTextState::default(),
    );

    assert_eq!(behind.len(), 1, "ordinary pre-notes items stay behind notes");
    assert_eq!(front.len(), 1, "ECFN-style judge line is drawn in front of notes");
    assert!(failed_overlay.is_empty());
    assert!(matches!(
        front.first(),
        Some(SkinRenderItem::Image { rect, .. })
            if approx_eq(rect.y, 355.0 / 720.0)
                && approx_eq(rect.height, 8.0 / 720.0)
    ));
}

#[test]
fn skin_document_resolves_lane_note_images() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "source": [{ "id": 1, "path": "notes.png" }],
                "image": [
                    { "id": "note-w", "src": 1, "x": 0, "y": 0, "w": 20, "h": 10 },
                    { "id": "note-b", "src": 1, "x": 20, "y": 0, "w": 10, "h": 10 },
                    { "id": "note-s", "src": 1, "x": 30, "y": 0, "w": 30, "h": 10 }
                ],
                "note": {
                    "id": "notes",
                    "note": ["note-w", "note-b", "note-w", "note-b", "note-w", "note-b", "note-w", "note-s"]
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

    let key2 = document
        .note_image_render_item(
            Lane::Key2,
            KeyMode::K7,
            Rect { x: 0.0, y: 0.0, width: 0.1, height: 0.1 },
            &sources,
        )
        .unwrap();
    let scratch = document
        .note_image_render_item(
            Lane::Scratch,
            KeyMode::K7,
            Rect { x: 0.0, y: 0.0, width: 0.1, height: 0.1 },
            &sources,
        )
        .unwrap();

    assert!(matches!(
        key2,
        SkinRenderItem::Image {
            texture: SkinTextureId(42),
            uv: TextureRegion { x, width, .. },
            ..
        } if approx_eq(x, 0.2) && approx_eq(width, 0.1)
    ));
    assert!(matches!(
        scratch,
        SkinRenderItem::Image {
            texture: SkinTextureId(42),
            uv: TextureRegion { x, width, .. },
            ..
        } if approx_eq(x, 0.3) && approx_eq(width, 0.3)
    ));
}

#[test]
fn skin_document_uses_scratch_lnactive_for_unpressed_long_body() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "source": [{ "id": 1, "path": "notes.png" }],
                "image": [
                    { "id": "note-w", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "lnb-s", "src": 1, "x": 20, "y": 0, "w": 20, "h": 1 },
                    { "id": "lna-s", "src": 1, "x": 50, "y": 0, "w": 30, "h": 1 }
                ],
                "note": {
                    "id": "notes",
                    "note": ["note-w", "note-w", "note-w", "note-w", "note-w", "note-w", "note-w", "note-w"],
                    "lnbody": ["note-w", "note-w", "note-w", "note-w", "note-w", "note-w", "note-w", "lnb-s"],
                    "lnactive": ["note-w", "note-w", "note-w", "note-w", "note-w", "note-w", "note-w", "lna-s"]
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

    let scratch = document
        .note_long_body_render_item(
            Lane::Scratch,
            KeyMode::K7,
            Rect { x: 0.0, y: 0.0, width: 0.1, height: 0.1 },
            LongNoteMode::Ln,
            LongBodyState::Inactive,
            &SkinDrawState::default(),
            &sources,
        )
        .unwrap();

    assert!(matches!(
        scratch,
        SkinRenderItem::Image {
            texture: SkinTextureId(42),
            uv: TextureRegion { x, width, .. },
            ..
        } if approx_eq(x, 0.5) && approx_eq(width, 0.3)
    ));
}

#[test]
fn skin_document_resolves_judge_images_by_label() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "judge.png" }],
                "image": [
                    { "id": "judgef-pg", "src": 1, "x": 0, "y": 0, "w": 10, "h": 20, "divy": 2, "cycle": 100 },
                    { "id": "judgef-gr", "src": 1, "x": 10, "y": 0, "w": 10, "h": 10 },
                    { "id": "judgef-gd", "src": 1, "x": 20, "y": 0, "w": 10, "h": 10 },
                    { "id": "judgef-bd", "src": 1, "x": 30, "y": 0, "w": 10, "h": 10 },
                    { "id": "judgef-pr", "src": 1, "x": 40, "y": 0, "w": 10, "h": 10 },
                    { "id": "judgef-ms", "src": 1, "x": 50, "y": 0, "w": 10, "h": 10 }
                ],
                "judge": [{
                    "id": "judge",
                    "images": [
                        { "id": "judgef-pg", "dst": [{ "time": 0, "x": 0, "y": 10, "w": 20, "h": 10 }, { "time": 500 }] },
                        { "id": "judgef-gr", "dst": [{ "time": 0, "x": 0, "y": 10, "w": 20, "h": 10 }, { "time": 500 }] },
                        { "id": "judgef-gd", "dst": [{ "time": 0, "x": 0, "y": 10, "w": 20, "h": 10 }, { "time": 500 }] },
                        { "id": "judgef-bd", "dst": [{ "time": 0, "x": 0, "y": 10, "w": 20, "h": 10 }, { "time": 500 }] },
                        { "id": "judgef-pr", "dst": [{ "time": 0, "x": 0, "y": 10, "w": 20, "h": 10 }, { "time": 500 }] },
                        { "id": "judgef-ms", "dst": [{ "time": 0, "x": 0, "y": 10, "w": 20, "h": 10 }, { "time": 500 }] }
                    ]
                }]
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

    let pgreat = document.judge_image_render_item("PGREAT FAST", 175, &sources).unwrap();
    let poor = document.judge_image_render_item("POOR SLOW", 120, &sources).unwrap();
    let empty_poor = document.judge_image_render_item("EMPTY POOR SLOW", 120, &sources).unwrap();
    let expired = document.judge_image_render_item("PGREAT", 600, &sources);

    assert!(matches!(pgreat, SkinRenderItem::Image {
                uv: TextureRegion { x, y: u_y, height: u_height, .. },
                rect: Rect { y, width, .. },
                ..
            } if approx_eq(x, 0.0)
                && approx_eq(u_y, 0.1)
                && approx_eq(u_height, 0.1)
                && approx_eq(y, 0.8)
                && approx_eq(width, 0.2)));
    assert!(matches!(poor, SkinRenderItem::Image {
                uv: TextureRegion { x, .. },
                ..
            } if approx_eq(x, 0.4)));
    assert!(matches!(empty_poor, SkinRenderItem::Image {
                uv: TextureRegion { x, .. },
                ..
            } if approx_eq(x, 0.5)));
    assert!(expired.is_none());
}

#[test]
fn skin_document_resolves_judge_number_images() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "judge.png" }],
                "image": [
                    { "id": "judgef-pg", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }
                ],
                "value": [
                    { "id": "judgen-pg", "src": 1, "x": 0, "y": 20, "w": 100, "h": 10, "divx": 10, "digit": 3 }
                ],
                "judge": [{
                    "id": "judge",
                    "images": [
                        { "id": "judgef-pg", "dst": [{ "time": 0, "x": 10, "y": 10, "w": 20, "h": 10 }, { "time": 500 }] }
                    ],
                    "numbers": [
                        { "id": "judgen-pg", "dst": [{ "time": 0, "x": 20, "y": 5, "w": 5, "h": 10 }, { "time": 500 }] }
                    ]
                }]
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

    let items = document.judge_render_items("PGREAT", 123, 100, &sources).unwrap();

    assert_eq!(items.len(), 4);
    // judge number: dst x 20 - w*digit/2 = 13, align=2, base judge x=10 → digits at 0.23/0.28/0.33
    assert!(matches!(items[1], SkinRenderItem::Image {
                rect: Rect { x, y, width, height },
                uv: TextureRegion { x: u, y: v, width: uv_width, height: uv_height },
                ..
            } if approx_eq(x, 0.23)
                && approx_eq(y, 0.75)
                && approx_eq(width, 0.05)
                && approx_eq(height, 0.1)
                && approx_eq(u, 0.1)
                && approx_eq(v, 0.2)
                && approx_eq(uv_width, 0.1)
                && approx_eq(uv_height, 0.1)));
    assert!(matches!(items[2], SkinRenderItem::Image {
                rect: Rect { x, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(x, 0.28) && approx_eq(u, 0.2)));
    assert!(matches!(items[3], SkinRenderItem::Image {
                rect: Rect { x, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(x, 0.33) && approx_eq(u, 0.3)));
}

#[test]
fn skin_document_animates_judge_number_value_rows() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "judge.png" }],
                "image": [
                    { "id": "judgef-pg", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }
                ],
                "value": [
                    { "id": "judgen-pg", "src": 1, "x": 0, "y": 20, "w": 100, "h": 20, "divx": 10, "divy": 2, "digit": 1, "cycle": 100 }
                ],
                "judge": [{
                    "id": "judge",
                    "images": [
                        { "id": "judgef-pg", "dst": [{ "time": 0, "x": 10, "y": 10, "w": 20, "h": 10 }, { "time": 500 }] }
                    ],
                    "numbers": [
                        { "id": "judgen-pg", "dst": [{ "time": 0, "x": 20, "y": 5, "w": 5, "h": 10 }, { "time": 500 }] }
                    ]
                }]
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

    let early = document.judge_render_items("PGREAT", 7, 25, &sources).unwrap();
    let late = document.judge_render_items("PGREAT", 7, 75, &sources).unwrap();

    assert!(matches!(early[1], SkinRenderItem::Image {
                uv: TextureRegion { y, .. },
                ..
            } if approx_eq(y, 0.2)));
    assert!(matches!(late[1], SkinRenderItem::Image {
                uv: TextureRegion { y, .. },
                ..
            } if approx_eq(y, 0.3)));
}

#[test]
fn skin_document_renders_judge_destination_insert() {
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
                    ]}
                ],
                "source": [{ "id": 1, "path": "judge.png" }],
                "image": [
                    { "id": "judgef-pg", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }
                ],
                "value": [
                    { "id": "judgen-pg", "src": 1, "x": 0, "y": 20, "w": 100, "h": 10, "divx": 10, "digit": 3 }
                ],
                "judge": [{
                    "id": 2010,
                    "images": [
                        { "id": "judgef-pg", "loop": -1, "offset": 3, "dst": [
                            { "if": [920], "value": { "time": 0, "x": 10, "y": 20, "w": 20, "h": 10 } },
                            { "if": [921], "value": { "time": 0, "x": 70, "y": 20, "w": 20, "h": 10 } },
                            { "time": 500 }
                        ]}
                    ],
                    "numbers": [
                        { "id": "judgen-pg", "loop": -1, "dst": [
                            { "time": 0, "x": 20, "y": 5, "w": 5, "h": 10 },
                            { "time": 500 }
                        ]}
                    ]
                }],
                "destination": [
                    { "id": 2010 }
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

    let items = document.static_render_items(
        &sources,
        &SkinDrawState {
            judge_ms: judge_region_state(0, 100, 0).judge_ms,
            judge_index: judge_region_state(0, 100, 0).judge_index,
            judge_combo: {
                let mut combo = [0; MAX_JUDGE_REGIONS];
                combo[0] = 123;
                combo
            },
            offset_lift_px: 10,
            ..SkinDrawState::default()
        },
        &SkinTextState::default(),
    );

    assert_eq!(items.len(), 4);
    assert!(matches!(items[0], SkinRenderItem::Image {
                rect: Rect { x, y, width, height },
                ..
            } if approx_eq(x, 0.1)
                && approx_eq(y, 0.6)
                && approx_eq(width, 0.2)
                && approx_eq(height, 0.1)));
    assert!(matches!(items[1], SkinRenderItem::Image {
                rect: Rect { x, y, .. },
                ..
            } if approx_eq(x, 0.23) && approx_eq(y, 0.55)));
}

#[test]
fn build_judge_region_state_tracks_signed_timing_per_region() {
    use crate::snapshot::DisplayJudgement;
    let judgement = |lane, delta_us, suppressed| DisplayJudgement {
        lane,
        judge: bmz_core::judge::Judge::Great,
        side: Some(bmz_core::judge::TimingSide::Fast),
        text: String::new(),
        combo: 1,
        delta_us,
        time: TimeUs(1_000),
        is_miss: false,
        timing_ms_suppressed: suppressed,
    };
    // 1P 側 FAST 3ms、2P 側 SLOW 7ms。
    let judgements = [judgement(Lane::Key1, -3_000, false), judgement(Lane::Key8, 7_000, false)];
    let state = build_judge_region_state(&judgements, 2_000, 2);
    assert_eq!(state.judge_timing_ms[0], Some(-3));
    assert_eq!(state.judge_timing_ms[1], Some(7));
    assert_eq!(state.judge_timing_ms[2], None);

    // 閾値フィルタで抑制された判定は ±ms を領域ごと隠す。
    let suppressed = [judgement(Lane::Key1, -3_000, true)];
    let state = build_judge_region_state(&suppressed, 2_000, 2);
    assert_eq!(state.judge_timing_ms[0], None);
}

#[test]
fn lane_judge_region_maps_14k_sides() {
    assert_eq!(lane_judge_region(0, 16, 2), 0);
    assert_eq!(lane_judge_region(7, 16, 2), 0);
    assert_eq!(lane_judge_region(8, 16, 2), 1);
    assert_eq!(lane_judge_region(15, 16, 2), 1);
}

#[test]
fn dual_judge_regions_render_combo_at_separate_positions() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "judge.png" }],
                "image": [
                    { "id": "judgef-pg", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }
                ],
                "value": [
                    { "id": "judgen-pg", "src": 1, "x": 0, "y": 20, "w": 100, "h": 10, "divx": 10, "digit": 3 }
                ],
                "judge": [
                    {
                        "id": "judge",
                        "index": 0,
                        "images": [
                            { "id": "judgef-pg", "dst": [{ "time": 0, "x": 10, "y": 20, "w": 20, "h": 10 }, { "time": 500 }] }
                        ],
                        "numbers": [
                            { "id": "judgen-pg", "dst": [{ "time": 0, "x": 20, "y": 5, "w": 5, "h": 10 }, { "time": 500 }] }
                        ]
                    },
                    {
                        "id": "judge1",
                        "index": 1,
                        "images": [
                            { "id": "judgef-pg", "dst": [{ "time": 0, "x": 60, "y": 20, "w": 20, "h": 10 }, { "time": 500 }] }
                        ],
                        "numbers": [
                            { "id": "judgen-pg", "dst": [{ "time": 0, "x": 70, "y": 5, "w": 5, "h": 10 }, { "time": 500 }] }
                        ]
                    }
                ],
                "destination": [
                    { "id": "judge" },
                    { "id": "judge1" }
                ]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    assert_eq!(document.judge_region_count(), 2);
    let state = SkinDrawState {
        judge_ms: {
            let mut ms = [None; MAX_JUDGE_REGIONS];
            ms[0] = Some(100);
            ms[1] = Some(100);
            ms
        },
        judge_index: {
            let mut idx = [None; MAX_JUDGE_REGIONS];
            idx[0] = Some(0);
            idx[1] = Some(0);
            idx
        },
        judge_combo: {
            let mut combo = [0; MAX_JUDGE_REGIONS];
            combo[0] = 42;
            combo[1] = 42;
            combo
        },
        combo: 42,
        ..SkinDrawState::default()
    };
    let left = document
        .judge_render_items_for_def(&document.judge[0], 0, 42, 100, &sources, &state)
        .unwrap();
    let right = document
        .judge_render_items_for_def(&document.judge[1], 0, 42, 100, &sources, &state)
        .unwrap();
    let left_digit = match &left[1] {
        SkinRenderItem::Image { rect, .. } => rect.x,
        _ => panic!("expected digit image"),
    };
    let right_digit = match &right[1] {
        SkinRenderItem::Image { rect, .. } => rect.x,
        _ => panic!("expected digit image"),
    };
    assert!(
        right_digit > left_digit + 0.2,
        "right region digit x={right_digit} should be right of left x={left_digit}"
    );

    let static_items = document.static_render_items(&sources, &state, &SkinTextState::default());
    assert_eq!(static_items.len(), 6);
    let static_left = match &static_items[1] {
        SkinRenderItem::Image { rect, .. } => rect.x,
        _ => panic!(),
    };
    let static_right = match &static_items[4] {
        SkinRenderItem::Image { rect, .. } => rect.x,
        _ => panic!(),
    };
    assert!(static_right > static_left + 0.2);
}

#[test]
fn skin_document_hides_judge_combo_when_region_combo_is_zero() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "judge.png" }],
                "image": [
                    { "id": "judge-poor", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }
                ],
                "value": [
                    { "id": "combo", "src": 1, "x": 0, "y": 20, "w": 100, "h": 10, "divx": 10, "digit": 3 }
                ],
                "judge": [{
                    "id": "judge",
                    "images": [
                        { "id": "judge-poor", "dst": [{ "time": 0, "x": 10, "y": 20, "w": 20, "h": 10 }, { "time": 500 }] }
                    ],
                    "numbers": [
                        { "id": "combo", "dst": [{ "time": 0, "x": 20, "y": 5, "w": 5, "h": 10 }, { "time": 500 }] }
                    ]
                }],
                "destination": [{ "id": "judge" }]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    let state = SkinDrawState {
        combo: 123,
        judge_ms: judge_region_state(0, 100, 0).judge_ms,
        judge_index: judge_region_state(0, 100, 0).judge_index,
        judge_combo: [0; MAX_JUDGE_REGIONS],
        ..SkinDrawState::default()
    };

    let items = document.static_render_items(&sources, &state, &SkinTextState::default());

    assert_eq!(items.len(), 1);
}

#[test]
fn skin_draw_options_match_judge_fast_slow_regions() {
    let fast = SkinDrawState {
        judge_index: [Some(1), None, None],
        judge_timing_sign: [Some(1), None, None],
        ..SkinDrawState::default()
    };
    let slow = SkinDrawState {
        judge_index: [Some(1), None, None],
        judge_timing_sign: [Some(-1), None, None],
        ..SkinDrawState::default()
    };
    // Auto モード: PGREAT は apply_fast_slow_display_filter で side=None にされるため
    // judge_timing_sign=None となり、op 1242/1243 は false になる。
    let perfect_auto = SkinDrawState {
        judge_index: [Some(0), None, None],
        judge_timing_sign: [None, None, None],
        ..SkinDrawState::default()
    };
    // ThresholdMs モード(threshold=0): PGREAT も side=Some のまま渡るため
    // judge_timing_sign=Some(1) となり、op 1242 は true になる。
    let perfect_threshold = SkinDrawState {
        judge_index: [Some(0), None, None],
        judge_timing_sign: [Some(1), None, None],
        ..SkinDrawState::default()
    };

    assert!(test_skin_op(1242, &[], &fast));
    assert!(!test_skin_op(1243, &[], &fast));
    assert!(test_skin_op(1243, &[], &slow));
    assert!(!test_skin_op(1242, &[], &slow));
    assert!(test_skin_op(241, &[], &perfect_auto));
    assert!(!test_skin_op(1242, &[], &perfect_auto));
    assert!(test_skin_op(241, &[], &perfect_threshold));
    assert!(test_skin_op(1242, &[], &perfect_threshold));
}

#[test]
fn skin_document_shifts_judge_combo_numbers_beatoraja_style() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "judge.png" }],
                "image": [
                    { "id": "judgef-pg", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }
                ],
                "value": [
                    { "id": "judgen-pg", "src": 1, "x": 0, "y": 20, "w": 100, "h": 10, "divx": 10, "digit": 6 }
                ],
                "judge": [{
                    "id": 2010,
                    "shift": true,
                    "images": [
                        { "id": "judgef-pg", "dst": [{ "time": 0, "x": 30, "y": 20, "w": 20, "h": 10 }, { "time": 500 }] }
                    ],
                    "numbers": [
                        { "id": "judgen-pg", "dst": [{ "time": 0, "x": 20, "y": 5, "w": 5, "h": 10 }, { "time": 500 }] }
                    ]
                }],
                "destination": [
                    { "id": 2010 }
                ]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    let items = document.static_render_items(
        &sources,
        &SkinDrawState {
            judge_ms: judge_region_state(0, 100, 0).judge_ms,
            judge_index: judge_region_state(0, 100, 0).judge_index,
            judge_combo: {
                let mut combo = [0; MAX_JUDGE_REGIONS];
                combo[0] = 123;
                combo
            },
            ..Default::default()
        },
        &SkinTextState::default(),
    );

    assert_eq!(items.len(), 4);
    assert!(matches!(items[0], SkinRenderItem::Image {
                rect: Rect { x, .. },
                ..
            } if approx_eq(x, 0.23)));
    // dst x 20 - w*6/2 = 5, align=2, shiftbase=3, judge x 30 - length/2 = 23
    assert!(matches!(items[1], SkinRenderItem::Image {
                rect: Rect { x, .. },
                ..
            } if approx_eq(x, 0.43)));
    assert!(matches!(items[2], SkinRenderItem::Image {
                rect: Rect { x, .. },
                ..
            } if approx_eq(x, 0.48)));
    assert!(matches!(items[3], SkinRenderItem::Image {
                rect: Rect { x, .. },
                ..
            } if approx_eq(x, 0.53)));
}

#[test]
fn skin_document_resolves_lane_imageset_effects() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "effect.png" }],
                "image": [
                    { "id": "normal", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "pgreat", "src": 1, "x": 10, "y": 0, "w": 10, "h": 10 },
                    { "id": "good", "src": 1, "x": 20, "y": 0, "w": 10, "h": 10 }
                ],
                "imageset": [
                    { "id": "beam1", "ref": 501, "images": ["normal", "pgreat"] },
                    { "id": "bomb1", "ref": 501, "images": ["normal", "pgreat", "good"] },
                    { "id": "beam2", "ref": 502, "images": ["normal", "pgreat"] }
                ],
                "destination": [
                    { "id": "beam1", "timer": 51, "loop": -1, "dst": [{ "time": 0, "x": 10, "y": 20, "w": 20, "h": 10 }, { "time": 100 }] },
                    { "id": "bomb1", "timer": 51, "loop": -1, "dst": [{ "time": 0, "x": 30, "y": 20, "w": 20, "h": 10 }, { "time": 100 }] },
                    { "id": "beam2", "timer": 52, "loop": -1, "dst": [{ "time": 0, "x": 50, "y": 20, "w": 20, "h": 10 }, { "time": 100 }] }
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

    // Key1 (timer 51 = bomb_ms[1]) でボムタイマー進行中、直近判定 PGREAT
    let pgreat_state = SkinDrawState {
        bomb_ms: {
            let mut a = [None; LANE_COUNT];
            a[1] = Some(50);
            a
        },
        lane_judge: {
            let mut a = [None; LANE_COUNT];
            a[1] = Some(0);
            a
        },
        ..SkinDrawState::default()
    };
    let pgreat = document.static_render_items(&sources, &pgreat_state, &SkinTextState::default());
    // GOOD 判定
    let good_state = SkinDrawState {
        bomb_ms: {
            let mut a = [None; LANE_COUNT];
            a[1] = Some(50);
            a
        },
        lane_judge: {
            let mut a = [None; LANE_COUNT];
            a[1] = Some(2);
            a
        },
        ..SkinDrawState::default()
    };
    let good = document.static_render_items(&sources, &good_state, &SkinTextState::default());
    // タイマーがアニメーション終端を超過 → loop:-1 で非表示
    let expired_state = SkinDrawState {
        bomb_ms: {
            let mut a = [None; LANE_COUNT];
            a[1] = Some(150);
            a
        },
        lane_judge: {
            let mut a = [None; LANE_COUNT];
            a[1] = Some(0);
            a
        },
        ..SkinDrawState::default()
    };
    let expired = document.static_render_items(&sources, &expired_state, &SkinTextState::default());

    // beam1 と bomb1 のみ描画される (beam2 は timer 52 非アクティブ)
    assert_eq!(pgreat.len(), 2);
    // beam1: 2枚構成 + PGREAT → "pgreat" 画像 (u=0.1), rect x=0.1
    assert!(matches!(pgreat[0], SkinRenderItem::Image {
                rect: Rect { x, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(x, 0.1) && approx_eq(u, 0.1)));
    // beam1: 2枚構成 + GOOD → "normal" 画像 (u=0.0)
    assert!(matches!(good[0], SkinRenderItem::Image {
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(u, 0.0)));
    // bomb1: 3枚構成 + GOOD(index2) → "good" 画像 (u=0.2), rect x=0.3
    assert!(matches!(good[1], SkinRenderItem::Image {
                rect: Rect { x, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(x, 0.3) && approx_eq(u, 0.2)));
    assert!(expired.is_empty());
}

#[test]
fn judge_timing_value_omits_sign_when_numeric_digits_fill_all_cells() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "value": [{ "id": "judgetiming", "src": 1, "x": 0, "y": 0, "w": 120, "h": 20, "divx": 12, "divy": 2, "digit": 2, "ref": 12 }],
                "destination": [{ "id": "judgetiming", "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] }]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 120.0, 40.0);
    let state = SkinDrawState { judge_timing_offset_ms: 12, ..SkinDrawState::default() };

    let items = document.static_image_render_items(&sources, &state);
    let digit_uvs: Vec<f32> = items
        .iter()
        .filter_map(|item| match item {
            SkinRenderItem::Image { uv, .. } => Some(uv.x),
            _ => None,
        })
        .collect();

    assert_eq!(digit_uvs.len(), 2);
    assert!(approx_eq(digit_uvs[0], 1.0 / 12.0), "first cell should be tens digit");
    assert!(approx_eq(digit_uvs[1], 2.0 / 12.0), "second cell should be ones digit");
}

#[test]
fn lane_cover_numbers_render_before_ready_while_changing() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "number.png" }],
                "value": [
                    { "id": "white", "src": 1, "x": 0, "y": 0, "w": 100, "h": 10, "divx": 10, "digit": 3, "ref": 14 },
                    { "id": "green", "src": 1, "x": 0, "y": 0, "w": 100, "h": 10, "divx": 10, "digit": 3, "ref": 313 },
                    { "id": "combo", "src": 1, "x": 0, "y": 0, "w": 100, "h": 10, "divx": 10, "digit": 3, "ref": 104 }
                ],
                "destination": [
                    { "id": "white", "timer": 40, "op": [270], "dst": [{ "x": 10, "y": 20, "w": 5, "h": 10 }] },
                    { "id": "green", "timer": 40, "op": [270], "dst": [{ "x": 10, "y": 30, "w": 5, "h": 10 }] },
                    { "id": "combo", "timer": 40, "op": [270], "dst": [{ "x": 10, "y": 40, "w": 5, "h": 10 }] }
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
        &SkinDrawState { ready_timer_ms: None, ..SkinDrawState::default() },
    );
    assert!(inactive.is_empty());

    let active = document.static_image_render_items(
        &sources,
        &SkinDrawState {
            ready_timer_ms: None,
            lane_cover_changing: true,
            lane_cover: 0.25,
            total_duration_ms: 300,
            combo: 123,
            ..SkinDrawState::default()
        },
    );
    assert_eq!(active.len(), 6);
}

#[test]
fn skin_document_resolves_hidden_cover_destinations() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 12, "path": "cover.png" }],
                "hiddenCover": [
                    { "id": "hidden-cover", "src": 12, "x": 10, "y": 20, "w": 30, "h": 40 }
                ],
                "destination": [
                    { "id": "hidden-cover", "blend": 2, "dst": [{ "x": 20, "y": -40, "w": 30, "h": 40, "a": 128 }] }
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

    let hidden = document.static_image_render_items(&sources, &SkinDrawState::default());
    let items = document.static_image_render_items(
        &sources,
        &SkinDrawState { hidden_cover: 1.0, ..SkinDrawState::default() },
    );

    assert!(hidden.is_empty());
    assert_eq!(items.len(), 1);
    assert!(matches!(items[0], SkinRenderItem::Image {
                rect: Rect { x, y, width, height },
                uv: TextureRegion { x: u, y: v, width: uw, height: uh },
                tint: Color { a, .. },
                blend,
                ..
            } if approx_eq(x, 0.2)
                && approx_eq(y, 1.0)
                && approx_eq(width, 0.3)
                && approx_eq(height, 0.4)
                && approx_eq(u, 0.1)
                && approx_eq(v, 0.2)
                && approx_eq(uw, 0.3)
                && approx_eq(uh, 0.4)
                && approx_eq(a, 128.0 / 255.0)
                && blend == BlendMode::Add));
    assert_eq!(document.hidden_cover[0].disappear_line, 0);
    assert!(document.hidden_cover[0].is_disappear_line_link_lift);
}

#[test]
fn hidden_cover_clips_at_disappear_line() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 720,
                "h": 720,
                "source": [{ "id": 12, "path": "cover.png" }],
                "hiddenCover": [
                    { "id": "hidden-cover", "src": 12, "x": 0, "y": 0, "w": 390, "h": 580, "disapearLine": 140 }
                ],
                "destination": [
                    { "id": "hidden-cover", "dst": [{ "x": 20, "y": -440, "w": 390, "h": 580 }] }
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
            source_size: SkinImageSize { width: 390.0, height: 580.0 },
        },
    )]);

    let flush = document.static_image_render_items(
        &sources,
        &SkinDrawState { hidden_cover: 1.0, ..SkinDrawState::default() },
    );
    let SkinRenderItem::Image { rect: flush_rect, uv: flush_uv, .. } = &flush[0] else {
        panic!("expected image");
    };
    // オフセット無し: 上端 (skin y=140) が disappearLine
    assert!(approx_eq(flush_rect.y, 580.0 / 720.0));
    assert!(approx_eq(flush_rect.height, 580.0 / 720.0));

    let clipped = document.static_image_render_items(
        &sources,
        &SkinDrawState {
            hidden_cover: 1.0,
            offset_hidden_cover_px: 300,
            ..SkinDrawState::default()
        },
    );
    let SkinRenderItem::Image { rect: clipped_rect, uv: clipped_uv, .. } = &clipped[0] else {
        panic!("expected image");
    };
    // offset で上げた分、判定線より下を切り、上側 300px だけ残す
    assert!(approx_eq(clipped_rect.y, 280.0 / 720.0));
    assert!(approx_eq(clipped_rect.height, 300.0 / 720.0));
    assert!(approx_eq(flush_uv.height - clipped_uv.height, 280.0 / 580.0));
}

#[test]
fn lift_cover_hides_at_minimum_lift() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 720,
                "h": 720,
                "source": [{ "id": 12, "path": "lift.png" }],
                "image": [
                    { "id": "liftcover", "src": 12, "x": 0, "y": 0, "w": 431, "h": 723 }
                ],
                "hiddenCover": [
                    { "id": "hiddencover", "src": 12, "x": 0, "y": 0, "w": 431, "h": 723, "disapearLine": 357 }
                ],
                "destination": [
                    { "id": "liftcover", "offset": 3, "dst": [{ "x": 20, "y": -366, "w": 431, "h": 723 }] }
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
        &SkinDrawState { offset_lift_px: 0, ..SkinDrawState::default() },
    );
    assert!(items.is_empty());
}

#[test]
fn lift_cover_clips_at_disappear_line() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 720,
                "h": 720,
                "source": [{ "id": 12, "path": "lift.png" }],
                "image": [
                    { "id": "liftcover", "src": 12, "x": 0, "y": 0, "w": 431, "h": 723 }
                ],
                "hiddenCover": [
                    { "id": "hiddencover", "src": 12, "x": 0, "y": 0, "w": 431, "h": 723, "disapearLine": 357 }
                ],
                "destination": [
                    { "id": "liftcover", "offset": 3, "dst": [{ "x": 20, "y": -366, "w": 431, "h": 723 }] }
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

    let clipped = document.static_image_render_items(
        &sources,
        &SkinDrawState { offset_lift_px: 200, ..SkinDrawState::default() },
    );
    let SkinRenderItem::Image { rect, uv, .. } = &clipped[0] else {
        panic!("expected clipped lift cover image");
    };
    assert!(approx_eq(rect.height, 200.0 / 720.0));
    assert!(approx_eq(uv.height, 200.0 / 723.0));
}

#[test]
fn lift_hidden_cover_clips_with_its_own_disappear_line() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 720,
                "h": 720,
                "source": [{ "id": 12, "path": "lift.png" }],
                "hiddenCover": [
                    { "id": "lr2-liftcover", "src": 12, "x": 0, "y": 0, "w": 431, "h": 723, "disapearLine": 357, "isDisapearLineLinkLift": false }
                ],
                "destination": [
                    { "id": "lr2-liftcover", "offset": 3, "dst": [{ "x": 20, "y": -366, "w": 431, "h": 723 }] }
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

    let no_lift = document.static_image_render_items(
        &sources,
        &SkinDrawState { offset_lift_px: 0, ..SkinDrawState::default() },
    );
    assert!(no_lift.is_empty());

    let lifted = document.static_image_render_items(
        &sources,
        &SkinDrawState { offset_lift_px: 200, ..SkinDrawState::default() },
    );
    let SkinRenderItem::Image { rect, uv, tint, .. } = &lifted[0] else {
        panic!("expected clipped lift hidden cover image");
    };
    assert!(approx_eq(rect.height, 200.0 / 720.0));
    assert!(approx_eq(uv.height, 200.0 / 723.0));
    assert!(tint.a > 0.5);
}

#[test]
fn skin_state_number_maps_play_value_refs() {
    let state = SkinDrawState {
        combo: 12,
        max_combo: 45,
        ex_score: 167,
        total_notes: 100,
        past_notes: 100,
        judge_counts: DisplayJudgeCounts {
            pgreat: 30,
            great: 20,
            good: 10,
            bad: 4,
            poor: 3,
            empty_poor: 2,
        },
        gauge: 78.6,
        fast_slow_counts: Some(crate::snapshot::FastSlowJudgeCounts {
            fast_pgreat: 10,
            slow_pgreat: 11,
            fast_great: 12,
            slow_great: 13,
            fast_good: 14,
            slow_good: 15,
            fast_bad: 16,
            slow_bad: 17,
            fast_poor: 18,
            slow_poor: 19,
            fast_empty_poor: 20,
            slow_empty_poor: 21,
        }),
        best_ex_score: Some(123),
        target_ex_score: Some(145),
        judge_rank: Some(1),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(71, &state), Some(167));
    assert_eq!(skin_state_number(72, &state), Some(200));
    assert_eq!(skin_state_number(74, &state), Some(100));
    assert_eq!(skin_state_number(75, &state), Some(45));
    assert_eq!(skin_state_number(105, &state), Some(45));
    assert_eq!(skin_state_number(76, &state), Some(7));
    assert_eq!(skin_state_number(102, &state), Some(83));
    assert_eq!(skin_state_number(103, &state), Some(50));
    assert_eq!(skin_state_number(104, &state), Some(12));
    assert_eq!(skin_state_number(107, &state), Some(78));
    assert_eq!(skin_state_number(407, &state), Some(6));
    assert_eq!(skin_state_number(110, &state), Some(30));
    assert_eq!(skin_state_number(111, &state), Some(20));
    assert_eq!(skin_state_number(112, &state), Some(10));
    assert_eq!(skin_state_number(113, &state), Some(4));
    assert_eq!(skin_state_number(114, &state), Some(3));
    assert_eq!(skin_state_number(122, &state), Some(72));
    assert_eq!(skin_state_number(123, &state), Some(50));
    assert_eq!(skin_state_number(183, &state), Some(61));
    assert_eq!(skin_state_number(184, &state), Some(50));
    assert_eq!(skin_state_number(400, &state), Some(1));
    assert_eq!(skin_state_number(420, &state), Some(2));
    assert_eq!(skin_state_number(423, &state), Some(80));
    assert_eq!(skin_state_number(424, &state), Some(85));
    assert_eq!(skin_state_number(425, &state), Some(7));
    assert_eq!(skin_state_number(426, &state), Some(5));
    assert_eq!(skin_state_number(427, &state), Some(9));
    assert!(test_skin_op(181, &[], &state));
    assert!(!test_skin_op(182, &[], &state));
}

#[test]
fn autoplay_pgreat_fast_slow_refs_are_neutral() {
    let state = SkinDrawState {
        autoplay: true,
        judge_counts: DisplayJudgeCounts { pgreat: 30, ..DisplayJudgeCounts::default() },
        fast_slow_counts: Some(crate::snapshot::FastSlowJudgeCounts {
            fast_pgreat: 10,
            slow_pgreat: 11,
            fast_great: 12,
            slow_great: 13,
            ..crate::snapshot::FastSlowJudgeCounts::default()
        }),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(410, &state), Some(0));
    assert_eq!(skin_state_number(411, &state), Some(0));
    assert_eq!(skin_state_number(412, &state), Some(12));
    assert_eq!(skin_state_number(413, &state), Some(13));
    assert!(eval_skin_draw_condition(
        "number(110) > number(410) and number(110) > number(411)",
        &state
    ));
}

#[test]
fn display_number_digits_uses_absolute_value_like_beatoraja_skin_number() {
    assert_eq!(display_number_digits(-34, 2, NumberPadding::Zero), vec![3, 4]);
    assert_eq!(display_number_digits(-34, 4, NumberPadding::Blank), vec![10, 10, 3, 4]);
}

#[test]
fn skin_state_event_index_maps_lane_judge_values() {
    let mut lane_judge = [None; LANE_COUNT];
    lane_judge[Lane::Key1.index()] = Some(0);
    lane_judge[Lane::Key2.index()] = Some(1);
    lane_judge[Lane::Key3.index()] = Some(2);
    lane_judge[Lane::Key4.index()] = Some(3);
    lane_judge[Lane::Key5.index()] = Some(4);
    lane_judge[Lane::Key6.index()] = Some(5);
    lane_judge[Lane::Key8.index()] = Some(0);
    let state = SkinDrawState { lane_judge, ..SkinDrawState::default() };

    assert_eq!(skin_state_event_index(501, &state), 1);
    assert_eq!(skin_state_event_index(502, &state), 2);
    assert_eq!(skin_state_event_index(503, &state), 4);
    assert_eq!(skin_state_event_index(504, &state), 6);
    assert_eq!(skin_state_event_index(505, &state), 7);
    assert_eq!(skin_state_event_index(506, &state), 8);
    assert_eq!(skin_state_event_index(507, &state), 0);
    assert_eq!(skin_state_event_index(511, &state), 1);
}

#[test]
fn arrange_refs_use_each_sides_arrange_on_play_screen() {
    let state = SkinDrawState {
        select_arrange_index: 2,
        select_arrange_2p_index: 1,
        select_extended_arrange_index: 11,
        select_extended_arrange_2p_index: 10,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_imageset_index(42, &state), Some(2));
    assert_eq!(skin_state_imageset_index(43, &state), Some(1));
    assert_eq!(skin_state_number(42, &state), Some(2));
    assert_eq!(skin_state_number(43, &state), Some(1));
    assert_eq!(skin_state_event_index(42, &state), 2);
    assert_eq!(skin_state_event_index(43, &state), 1);
    assert_eq!(skin_state_imageset_index(344, &state), Some(11));
    assert_eq!(skin_state_imageset_index(345, &state), Some(10));
    assert_eq!(skin_state_number(344, &state), Some(11));
    assert_eq!(skin_state_number(345, &state), Some(10));
    assert_eq!(skin_state_event_index(344, &state), 11);
    assert_eq!(skin_state_event_index(345, &state), 10);
}

#[test]
fn random_lane_refs_map_beatoraja_pattern_numbers() {
    let mut pattern = (0..LANE_COUNT as u8).collect::<Vec<_>>();
    pattern[Lane::Key1.index()] = Lane::Key7.index() as u8;
    pattern[Lane::Key2.index()] = Lane::Key3.index() as u8;
    pattern[Lane::Key3.index()] = Lane::Key1.index() as u8;

    let refs = fixed_random_lane_refs(&pattern, KeyMode::K7, "RANDOM", "NORMAL");
    let state = SkinDrawState {
        result_arrange_index: 2,
        random_lane_refs: refs,
        result_failed: Some(false),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_event_index(42, &state), 2);
    assert_eq!(skin_state_imageset_index(450, &state), Some(7));
    assert_eq!(skin_state_imageset_index(451, &state), Some(3));
    assert_eq!(skin_state_imageset_index(452, &state), Some(1));
    assert_eq!(skin_state_imageset_index(457, &state), Some(0));
    assert_eq!(skin_state_imageset_index(459, &state), Some(0));
    assert_eq!(skin_state_event_index(450, &state), 7);
    assert_eq!(skin_state_event_index(451, &state), 3);
    assert_eq!(skin_state_event_index(452, &state), 1);
    assert_eq!(skin_state_event_index(457, &state), 0);
    assert_eq!(skin_state_event_index(459, &state), 0);
    assert_eq!(skin_state_number(450, &state), Some(7));
    assert_eq!(skin_state_number(466, &state), Some(0));
    assert_eq!(skin_state_number(467, &state), None);
    assert_eq!(skin_state_number(468, &state), None);
    assert_eq!(skin_state_event_index(467, &state), 0);
    assert_eq!(skin_state_event_index(468, &state), 0);
}

#[test]
fn random_lane_refs_hide_for_non_fixed_random() {
    let refs = fixed_random_lane_refs(
        &(0..LANE_COUNT as u8).collect::<Vec<_>>(),
        KeyMode::K7,
        "S-RANDOM",
        "NORMAL",
    );
    let state = SkinDrawState {
        result_arrange_index: 4,
        random_lane_refs: refs,
        result_failed: Some(false),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_event_index(42, &state), 4);
    assert_eq!(skin_state_imageset_index(450, &state), Some(0));
}

#[test]
fn random_lane_refs_use_each_sides_arrange() {
    let mut pattern = (0..LANE_COUNT as u8).collect::<Vec<_>>();
    pattern[Lane::Key1.index()] = Lane::Key7.index() as u8;
    pattern[Lane::Key8.index()] = Lane::Key10.index() as u8;
    let refs = fixed_random_lane_refs(&pattern, KeyMode::K14, "NORMAL", "RANDOM");
    let p2_random = SkinDrawState {
        result_arrange_index: 0,
        result_arrange_2p_index: 2,
        random_lane_refs: refs,
        result_failed: Some(false),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_imageset_index(450, &p2_random), Some(0));
    assert_eq!(skin_state_imageset_index(460, &p2_random), Some(3));

    let p1_random = SkinDrawState {
        result_arrange_index: 2,
        result_arrange_2p_index: 0,
        random_lane_refs: fixed_random_lane_refs(&pattern, KeyMode::K14, "RANDOM", "NORMAL"),
        ..p2_random
    };
    assert_eq!(skin_state_imageset_index(450, &p1_random), Some(7));
    assert_eq!(skin_state_imageset_index(460, &p1_random), Some(0));
}

#[test]
fn play_target_image_index_matches_beatoraja_default_target_list() {
    assert_eq!(play_target_image_index("RANK_A"), 1);
    assert_eq!(play_target_image_index("RANK_AA-"), 3);
    assert_eq!(play_target_image_index("RANK_AA"), 4);
    assert_eq!(play_target_image_index("RANK_AAA-"), 6);
    assert_eq!(play_target_image_index("RANK_AAA"), 7);
    assert_eq!(play_target_image_index("RANK_MAX-"), 9);
    assert_eq!(play_target_image_index("MAX"), 10);
    assert_eq!(play_target_image_index("IR_TOP"), 0);
}

#[test]
fn bundled_beatoraja_default_play7_json_loads_when_available() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.local/beatoraja/skin/default/play7.json");
    if !path.is_file() {
        return;
    }

    let document = SkinDocument::load_beatoraja_json(&path).unwrap();

    assert_eq!(document.name, "beatoraja default");
    assert_eq!(document.w, 1280);
    assert_eq!(document.h, 720);
    assert!(document.source_map().contains_key("7"));
    assert!(document.image_map().contains_key("note-w"));
    assert_eq!(document.note.as_ref().unwrap().id, "notes");
    assert!(!document.destination.is_empty());
}

#[test]
fn local_ecfn_converted_play7_json_loads_when_available() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/ECFN/play/play7-1p.json");
    if !path.is_file() {
        return;
    }

    let document = SkinDocument::load_beatoraja_json(&path).unwrap();

    assert!(!document.destination.is_empty());
}

#[test]
fn stretch_applied_to_judge_destination() {
    // stretch=9 (resize_about_center) should resize the image to its source dimensions
    // centered on the destination rect.
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "effect.png" }],
                "image": [{ "id": "judge-pg", "src": 1, "x": 0, "y": 0, "w": 50, "h": 20 }],
                "judge": [{
                    "id": "judge-1p",
                    "index": 0,
                    "images": [
                        { "id": "judge-pg", "stretch": 9, "dst": [
                            { "time": 0, "x": 0, "y": 0, "w": 100, "h": 100 }
                        ]}
                    ]
                }]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(5),
            source_size: SkinImageSize { width: 50.0, height: 20.0 },
        },
    )]);

    let items = document.judge_render_items("PGREAT", 0, 0, &sources).unwrap();

    // stretch=9: resize_about_center places the 50x20 source centered in 100x100 destination.
    // In normalized coords (canvas 100x100):
    //   dest rect: x=0/100=0, y=0/100=0, w=100/100=1, h=100/100=1
    //   source size: 50x20 pixels → w=50/100=0.5, h=20/100=0.2
    //   centered: x = 0 + (1 - 0.5)*0.5 = 0.25, y = 0 + (1 - 0.2)*0.5 = 0.4
    assert!(matches!(
        items[0],
        SkinRenderItem::Image {
            rect: Rect { x, y, width, height },
            ..
        } if approx_eq(x, 0.25)
            && approx_eq(y, 0.4)
            && approx_eq(width, 0.5)
            && approx_eq(height, 0.2)
    ));
}

#[test]
fn lr2_2p_bomb_destination_uses_play_key_mode_op() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "bomb.png" }],
                "image": [{ "id": "bomb-img", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10, "divx": 16, "cycle": 251 }],
                "destination": [
                    { "id": "bomb-img", "timer": 61, "op": [162], "loop": -1, "dst": [
                        { "time": 0, "x": 10, "y": 10, "w": 10, "h": 10 },
                        { "time": 250, "x": 10, "y": 10, "w": 10, "h": 10 }
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
            source_size: SkinImageSize { width: 160.0, height: 10.0 },
        },
    )]);
    let bomb_ms = {
        let mut a = [None; LANE_COUNT];
        a[Lane::Key8.index()] = Some(0);
        a
    };

    let active_14k = SkinDrawState { key_mode: KeyMode::K14, bomb_ms, ..Default::default() };
    let inactive_7k = SkinDrawState { key_mode: KeyMode::K7, bomb_ms, ..Default::default() };

    assert_eq!(document.static_image_render_items(&sources, &active_14k).len(), 1);
    assert!(document.static_image_render_items(&sources, &inactive_7k).is_empty());
}

#[test]
fn note_rect_for_progress_shifts_with_lift() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 720, "h": 720,
                "image": [
                    { "id": "n1", "src": 1, "x": 0, "y": 0, "w": 50, "h": 12 }
                ],
                "note": {
                    "id": "notes",
                    "note": ["n1"],
                    "dst": [{ "time": 0, "x": 10, "y": 140, "w": 50, "h": 580 }]
                }
            }
            "#,
    )
    .unwrap();
    let skin = SkinContext::from_manifest_and_document(default_skin_manifest(), document, []);
    let note_height = 12.0 / 720.0;
    let state_no_lift = SkinDrawState { offset_lift_px: 0, ..SkinDrawState::default() };
    let state_lifted = SkinDrawState { offset_lift_px: 72, ..SkinDrawState::default() };

    let rect_no_lift = skin
        .note_rect_for_progress(Lane::Key1, KeyMode::K7, 0.0, note_height, &state_no_lift)
        .unwrap();
    let rect_lifted = skin
        .note_rect_for_progress(Lane::Key1, KeyMode::K7, 0.0, note_height, &state_lifted)
        .unwrap();

    let judge_no_lift = 580.0 / 720.0;
    let judge_lifted = judge_no_lift - 72.0 / 720.0;
    assert!(approx_eq(rect_no_lift.y + note_height, judge_no_lift));
    assert!(approx_eq(rect_lifted.y + note_height, judge_lifted));
    assert!(
        rect_lifted.y < rect_no_lift.y,
        "expected lifted note higher on screen, got no_lift={} lifted={}",
        rect_no_lift.y,
        rect_lifted.y
    );
}

#[test]
fn pms_note_expansion_uses_quarter_note_elapsed_time() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 100, "h": 100,
                "note": {
                    "id": "notes",
                    "note": ["n1"],
                    "expansionrate": [150, 80],
                    "dst": [{ "time": 0, "x": 10, "y": 20, "w": 30, "h": 60 }]
                }
            }
            "#,
    )
    .unwrap();
    let skin = SkinContext::from_manifest_and_document(default_skin_manifest(), document, []);

    let peak = skin.document_note_expansion_scale(&SkinDrawState {
        quarter_note_elapsed_ms: Some(9),
        ..SkinDrawState::default()
    });
    let finished = skin.document_note_expansion_scale(&SkinDrawState {
        quarter_note_elapsed_ms: Some(159),
        ..SkinDrawState::default()
    });

    assert!(approx_eq(peak.0, 1.5));
    assert!(approx_eq(peak.1, 0.8));
    assert!(approx_eq(finished.0, 1.0));
    assert!(approx_eq(finished.1, 1.0));
}

#[test]
fn pms_missed_note_falls_toward_dst2() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 100, "h": 100,
                "note": {
                    "id": "notes",
                    "note": ["n1"],
                    "size": [10],
                    "dst2": 90,
                    "dst": [{ "time": 0, "x": 10, "y": 20, "w": 30, "h": 60 }]
                }
            }
            "#,
    )
    .unwrap();
    let skin = SkinContext::from_manifest_and_document(default_skin_manifest(), document, []);
    let state = SkinDrawState::default();

    let start = skin.missed_note_rect_for_fall(Lane::Key1, KeyMode::K9, 0.0, 0.1, &state).unwrap();
    let end = skin.missed_note_rect_for_fall(Lane::Key1, KeyMode::K9, 1.0, 0.1, &state).unwrap();

    assert!(approx_eq(start.y + start.height, 0.8));
    assert!(approx_eq(end.y + end.height, 0.1));
}

#[test]
fn note_body_rect_shifts_with_lift() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 720, "h": 720,
                "image": [
                    { "id": "n1", "src": 1, "x": 0, "y": 0, "w": 50, "h": 12 }
                ],
                "note": {
                    "id": "notes",
                    "note": ["n1"],
                    "dst": [{ "time": 0, "x": 10, "y": 140, "w": 50, "h": 580 }]
                }
            }
            "#,
    )
    .unwrap();
    let skin = SkinContext::from_manifest_and_document(default_skin_manifest(), document, []);
    let state_no_lift = SkinDrawState { offset_lift_px: 0, ..SkinDrawState::default() };
    let state_lifted = SkinDrawState { offset_lift_px: 72, ..SkinDrawState::default() };

    let rect_no_lift =
        skin.note_body_rect(Lane::Key1, KeyMode::K7, 0.0, 0.5, &state_no_lift).unwrap();
    let rect_lifted =
        skin.note_body_rect(Lane::Key1, KeyMode::K7, 0.0, 0.5, &state_lifted).unwrap();

    // beatoraja 座標系（y-up）での body 位置:
    //   body.y      = tail_bottom = area.height * (1 - tail_y) = 580/720 * 0.5 = 290/720
    //   body.height = head_top - tail_bottom = (head_bottom - note_height) - tail_bottom
    //               = (580/720 - 12/720) - 290/720 = 278/720
    assert!(approx_eq(rect_no_lift.y, (580.0 * 0.5) / 720.0));
    assert!(approx_eq(rect_no_lift.height, (580.0 * 0.5 - 12.0) / 720.0));
    assert!(
        rect_lifted.y < rect_no_lift.y,
        "expected lifted long body higher on screen, got no_lift={} lifted={}",
        rect_no_lift.y,
        rect_lifted.y
    );
    assert!(rect_lifted.height <= rect_no_lift.height + 0.0001);
}

#[test]
fn skin_state_number_bpm_lanecover_duration_timing() {
    let state = SkinDrawState {
        now_bpm: 148.7,
        min_bpm: 80.0,
        max_bpm: 200.3,
        lane_cover: 0.25,
        total_duration_ms: 305_000,
        duration_green_ms: Some(183_000),
        judge_timing_ms: [Some(-3), Some(7), None],
        ..SkinDrawState::default()
    };
    // NUMBER_NOWBPM (160) = round(148.7) = 149
    assert_eq!(skin_state_number(160, &state), Some(149));
    // NUMBER_MINBPM (91) = round(80.0) = 80
    assert_eq!(skin_state_number(91, &state), Some(80));
    // NUMBER_MAXBPM (90) = round(200.3) = 200
    assert_eq!(skin_state_number(90, &state), Some(200));
    // NUMBER_LANECOVER1 (14) = round(0.25 * 1000) = 250
    assert_eq!(skin_state_number(14, &state), Some(250));
    // NUMBER_LIFT1 (314) = round(0.42 * 1000) = 420
    let lifted = SkinDrawState { lift: 0.42, ..state.clone() };
    assert_eq!(skin_state_number(314, &lifted), Some(420));
    let capped_cover = SkinDrawState { lane_cover: 0.9, lift: 0.2, ..state.clone() };
    assert_eq!(skin_state_number(14, &capped_cover), Some(800));
    // float_number(113) tracks BARGRAPH_BESTSCORERATE
    let best_rate =
        SkinDrawState { total_notes: 100, best_ex_score: Some(150), ..SkinDrawState::default() };
    assert!((skin_state_float_number(113, &best_rate).unwrap() - 0.75).abs() < 0.001);
    assert!(!eval_skin_draw_condition("float_number(113) == 0", &best_rate));
    assert!(eval_skin_draw_condition(
        "float_number(113) == 0",
        &SkinDrawState { total_notes: 100, best_ex_score: Some(0), ..SkinDrawState::default() }
    ));
    // BMZ keeps the green number in SkinDrawState and exposes beatoraja's duration as green*5/3.
    assert_eq!(skin_state_number(312, &state), Some(305_000));
    // NUMBER_DURATION_GREEN (313) = green number.
    assert_eq!(skin_state_number(313, &state), Some(183_000));
    assert_eq!(
        skin_state_number(
            313,
            &SkinDrawState { duration_green_ms: Some(183_001), ..state.clone() }
        ),
        Some(183_001)
    );
    let duration_state = SkinDrawState {
        now_bpm: 100.0,
        main_bpm: 100.0,
        min_bpm: 50.0,
        max_bpm: 200.0,
        hispeed: 2.0,
        lane_cover: 0.25,
        total_duration_ms: 900,
        duration_green_ms: Some(540),
        ..SkinDrawState::default()
    };
    // 1312..=1327 are lane-cover duration variants:
    // current/main/min/max BPM x cover on/off x normal/green.
    // Current-BPM variants use SkinDrawState's real note display duration; main/min/max variants
    // are theoretical values derived from their BPM.
    assert_eq!(skin_state_number(1312, &duration_state), Some(900));
    assert_eq!(skin_state_number(1313, &duration_state), Some(540));
    assert_eq!(skin_state_number(1314, &duration_state), Some(1_200));
    assert_eq!(skin_state_number(1315, &duration_state), Some(720));
    assert_eq!(skin_state_number(1317, &duration_state), Some(540));
    assert_eq!(skin_state_number(1321, &duration_state), Some(1_080));
    assert_eq!(skin_state_number(1325, &duration_state), Some(270));
    let changed_now_bpm = SkinDrawState {
        now_bpm: 150.0,
        duration_green_ms: Some(777),
        total_duration_ms: 1_295,
        ..duration_state.clone()
    };
    // WMII uses the main/min/max variants.  They should stay stable across BPM changes and
    // current-duration rounding; current-BPM variants follow the runtime display duration.
    assert_eq!(skin_state_number(1312, &changed_now_bpm), Some(1_295));
    assert_eq!(skin_state_number(1313, &changed_now_bpm), Some(777));
    assert_eq!(skin_state_number(1317, &changed_now_bpm), Some(540));
    assert_eq!(skin_state_number(1321, &changed_now_bpm), Some(1_080));
    assert_eq!(skin_state_number(1325, &changed_now_bpm), Some(270));
    let faster = SkinDrawState { hispeed: 3.0, ..duration_state.clone() };
    assert_eq!(skin_state_number(1317, &faster), Some(360));
    let lower_cover = SkinDrawState { lane_cover: 0.5, ..duration_state.clone() };
    assert_eq!(skin_state_number(1317, &lower_cover), Some(360));
    let lifted_cover = SkinDrawState {
        lift: 0.2,
        total_duration_ms: 660,
        duration_green_ms: Some(396),
        ..duration_state.clone()
    };
    assert_eq!(skin_state_number(1312, &lifted_cover), Some(660));
    assert_eq!(skin_state_number(1313, &lifted_cover), Some(396));
    assert_eq!(skin_state_number(1314, &lifted_cover), Some(960));
    // VALUE_JUDGE_1P_DURATION (525) = -(-3) = 3 (FAST 3ms は beatoraja 規約で正)
    assert_eq!(skin_state_number(525, &state), Some(3));
    // VALUE_JUDGE_2P_DURATION (526): SLOW 7ms (delta=+7) は beatoraja 規約で負
    assert_eq!(skin_state_number(526, &state), Some(-7));
    // VALUE_JUDGE_3P_DURATION (527): 領域に判定が無ければ None
    assert_eq!(skin_state_number(527, &state), None);
    // SLOW 5ms (delta=+5) は beatoraja 規約で負
    let slow = SkinDrawState { judge_timing_ms: [Some(5), None, None], ..state.clone() };
    assert_eq!(skin_state_number(525, &slow), Some(-5));
    // When no recent judgement, 525 returns None
    let no_judge = SkinDrawState { judge_timing_ms: [None; MAX_JUDGE_REGIONS], ..state.clone() };
    assert_eq!(skin_state_number(525, &no_judge), None);
}

#[test]
fn skin_image_index_number_maps_replay_slot_rules() {
    let state = SkinDrawState {
        select_replay_slot_rule_indices: [10, 1, 3, 0],
        ..SkinDrawState::default()
    };
    assert_eq!(skin_image_index_number(321, &state), Some(10));
    assert_eq!(skin_image_index_number(322, &state), Some(1));
    assert_eq!(skin_image_index_number(323, &state), Some(3));
    assert_eq!(skin_image_index_number(324, &state), Some(0));
}

#[test]
fn timing_judge_areas_follow_beatoraja_mode_windows() {
    let areas = beatoraja_timing_judge_areas(&SkinDrawState {
        key_mode: KeyMode::K7,
        judge_rank: None,
        ..SkinDrawState::default()
    });

    assert_eq!(areas[0], TimingJudgeArea { late_ms: -20.0, early_ms: 20.0 });
    assert_eq!(areas[1], TimingJudgeArea { late_ms: -60.0, early_ms: 60.0 });
    assert_eq!(areas[2], TimingJudgeArea { late_ms: -150.0, early_ms: 150.0 });
    assert_eq!(areas[3], TimingJudgeArea { late_ms: -220.0, early_ms: 280.0 });
    assert_eq!(areas[4], TimingJudgeArea { late_ms: -500.0, early_ms: 150.0 });
}

#[test]
fn timing_judge_areas_apply_pms_rank_rule() {
    let areas = beatoraja_timing_judge_areas(&SkinDrawState {
        key_mode: KeyMode::K9,
        judge_rank: Some(0),
        ..SkinDrawState::default()
    });

    assert_eq!(areas[0], TimingJudgeArea { late_ms: -20.0, early_ms: 20.0 });
    assert_eq!(areas[1], TimingJudgeArea { late_ms: -20.0, early_ms: 20.0 });
    assert_eq!(areas[2], TimingJudgeArea { late_ms: -38.61, early_ms: 38.61 });
    assert_eq!(areas[3], TimingJudgeArea { late_ms: -183.0, early_ms: 183.0 });
    assert_eq!(areas[4], TimingJudgeArea { late_ms: -500.0, early_ms: 175.0 });
}

#[test]
fn skin_state_text_formats_bmz_judge_region_extension() {
    let text = SkinTextDef {
        id: "judge_text".to_string(),
        judge_region: Some(0),
        ..SkinTextDef::default()
    };
    let state = SkinDrawState {
        judge_ms: [Some(120), None, None],
        judge_index: [Some(0), None, None],
        judge_timing_sign: [Some(1), None, None],
        ..SkinDrawState::default()
    };

    assert_eq!(
        skin_state_text_with_draw_state(&text, Some(&state), &SkinTextState::default()),
        "PGREAT"
    );

    let expired = SkinDrawState {
        judge_ms: [None, None, None],
        judge_index: [Some(1), None, None],
        ..SkinDrawState::default()
    };
    assert_eq!(
        skin_state_text_with_draw_state(&text, Some(&expired), &SkinTextState::default()),
        ""
    );
}

#[test]
fn skin_state_text_formats_bmz_judge_timing_region_extension() {
    let text = SkinTextDef {
        id: "judge_timing".to_string(),
        judge_timing_region: Some(0),
        ..SkinTextDef::default()
    };
    let fast = SkinDrawState {
        judge_ms: [Some(120), None, None],
        judge_timing_sign: [Some(1), None, None],
        ..SkinDrawState::default()
    };
    let slow = SkinDrawState {
        judge_ms: [Some(120), None, None],
        judge_timing_sign: [Some(-1), None, None],
        ..SkinDrawState::default()
    };
    let just = SkinDrawState {
        judge_ms: [Some(120), None, None],
        judge_timing_sign: [None, None, None],
        ..SkinDrawState::default()
    };

    assert_eq!(
        skin_state_text_with_draw_state(&text, Some(&fast), &SkinTextState::default()),
        "FAST"
    );
    assert_eq!(
        skin_state_text_with_draw_state(&text, Some(&slow), &SkinTextState::default()),
        "SLOW"
    );
    assert_eq!(skin_state_text_with_draw_state(&text, Some(&just), &SkinTextState::default()), "");
}

#[test]
fn text_render_item_colors_bmz_judge_region_by_category() {
    let document: SkinDocument =
        serde_json::from_value(serde_json::json!({ "w": 1920, "h": 1080 })).unwrap();
    let text = SkinTextDef {
        id: "judge".to_string(),
        judge_region: Some(0),
        judge_color: true,
        ..SkinTextDef::default()
    };
    let frame = ResolvedSkinFrame {
        w: 100,
        h: 24,
        a: 128,
        r: 255,
        g: 255,
        b: 255,
        ..ResolvedSkinFrame::default()
    };
    let color_for = |index| {
        let draw_state = SkinDrawState {
            judge_ms: [Some(100), None, None],
            judge_index: [Some(index), None, None],
            ..SkinDrawState::default()
        };
        match document
            .text_render_item_with_draw_state(
                &text,
                frame,
                Some(&draw_state),
                &SkinTextState::default(),
            )
            .unwrap()
        {
            SkinRenderItem::Text { style, .. } => style.color,
            other => panic!("expected SkinRenderItem::Text, got {other:?}"),
        }
    };

    let pgreat = color_for(0);
    assert!(approx_eq(pgreat.r, 112.0 / 255.0));
    assert!(approx_eq(pgreat.g, 224.0 / 255.0));
    assert!(approx_eq(pgreat.b, 1.0));
    assert!(approx_eq(pgreat.a, 128.0 / 255.0));

    let good = color_for(2);
    assert!(approx_eq(good.r, 1.0));
    assert!(approx_eq(good.g, 224.0 / 255.0));
    assert!(approx_eq(good.b, 80.0 / 255.0));

    let poor = color_for(4);
    assert!(approx_eq(poor.r, 1.0));
    assert!(approx_eq(poor.g, 88.0 / 255.0));
    assert!(approx_eq(poor.b, 82.0 / 255.0));
}

#[test]
fn text_render_item_colors_bmz_judge_timing_region_by_side() {
    let document: SkinDocument =
        serde_json::from_value(serde_json::json!({ "w": 1920, "h": 1080 })).unwrap();
    let text = SkinTextDef {
        id: "judge_timing".to_string(),
        judge_timing_region: Some(0),
        judge_timing_color: true,
        ..SkinTextDef::default()
    };
    let frame = ResolvedSkinFrame {
        w: 100,
        h: 24,
        a: 128,
        r: 255,
        g: 255,
        b: 255,
        ..ResolvedSkinFrame::default()
    };
    let color_for = |sign| {
        let draw_state = SkinDrawState {
            judge_ms: [Some(100), None, None],
            judge_timing_sign: [Some(sign), None, None],
            ..SkinDrawState::default()
        };
        match document
            .text_render_item_with_draw_state(
                &text,
                frame,
                Some(&draw_state),
                &SkinTextState::default(),
            )
            .unwrap()
        {
            SkinRenderItem::Text { style, .. } => style.color,
            other => panic!("expected SkinRenderItem::Text, got {other:?}"),
        }
    };

    let fast = color_for(1);
    assert!(approx_eq(fast.r, 72.0 / 255.0));
    assert!(approx_eq(fast.g, 176.0 / 255.0));
    assert!(approx_eq(fast.b, 1.0));
    assert!(approx_eq(fast.a, 128.0 / 255.0));

    let slow = color_for(-1);
    assert!(approx_eq(slow.r, 1.0));
    assert!(approx_eq(slow.g, 88.0 / 255.0));
    assert!(approx_eq(slow.b, 82.0 / 255.0));
}

#[test]
fn note_lane_area_resolves_flat_frame_dst_after_expansion() {
    // load_beatoraja_json が expand_json_skin_value で条件ブロックを展開すると
    // note.dst はレーン順の Frame エントリ列になる。全レーンが正しく解決されること。
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 1280, "h": 720,
                "note": {
                    "dst": [
                        {"x": 90, "y": 140, "w": 50, "h": 580},
                        {"x": 140, "y": 140, "w": 40, "h": 580},
                        {"x": 180, "y": 140, "w": 50, "h": 580},
                        {"x": 230, "y": 140, "w": 40, "h": 580},
                        {"x": 270, "y": 140, "w": 50, "h": 580},
                        {"x": 320, "y": 140, "w": 40, "h": 580},
                        {"x": 360, "y": 140, "w": 50, "h": 580},
                        {"x": 20, "y": 140, "w": 70, "h": 580}
                    ]
                }
            }
            "#,
    )
    .unwrap();

    let enabled: Vec<i32> = vec![];
    // Key1 is index 0 → first Frame
    let area = document.note_lane_area(Lane::Key1, KeyMode::K7, &enabled).unwrap();
    assert!(approx_eq(area.x, 90.0 / 1280.0));
    assert!(approx_eq(area.y, 0.0));
    assert!(approx_eq(area.width, 50.0 / 1280.0));
    assert!(approx_eq(area.height, 580.0 / 720.0));
    // Key2 is index 1 → second Frame
    let area2 = document.note_lane_area(Lane::Key2, KeyMode::K7, &enabled).unwrap();
    assert!(approx_eq(area2.x, 140.0 / 1280.0));
    assert!(approx_eq(area2.width, 40.0 / 1280.0));
    // Scratch is index 7 → eighth Frame
    let scratch = document.note_lane_area(Lane::Scratch, KeyMode::K7, &enabled).unwrap();
    assert!(approx_eq(scratch.x, 20.0 / 1280.0));
    assert!(approx_eq(scratch.width, 70.0 / 1280.0));
}

#[test]
fn note_lane_area_resolves_conditional_dst_for_enabled_option() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "w": 1280, "h": 720,
                "note": {
                    "dst": [
                        {
                            "if": [920],
                            "values": [
                                {"x": 90, "y": 140, "w": 50, "h": 580},
                                {"x": 140, "y": 140, "w": 40, "h": 580},
                                {"x": 180, "y": 140, "w": 50, "h": 580},
                                {"x": 230, "y": 140, "w": 40, "h": 580},
                                {"x": 270, "y": 140, "w": 50, "h": 580},
                                {"x": 320, "y": 140, "w": 40, "h": 580},
                                {"x": 360, "y": 140, "w": 50, "h": 580},
                                {"x": 20, "y": 140, "w": 70, "h": 580}
                            ]
                        }
                    ]
                }
            }
            "#,
    )
    .unwrap();

    let enabled = vec![920];
    // Key1 is index 0
    let area = document.note_lane_area(Lane::Key1, KeyMode::K7, &enabled).unwrap();
    assert!(approx_eq(area.x, 90.0 / 1280.0));
    assert!(approx_eq(area.y, 0.0));
    assert!(approx_eq(area.width, 50.0 / 1280.0));
    assert!(approx_eq(area.height, 580.0 / 720.0));

    // Scratch is index 7
    let scratch_area = document.note_lane_area(Lane::Scratch, KeyMode::K7, &enabled).unwrap();
    assert!(approx_eq(scratch_area.x, 20.0 / 1280.0));
    assert!(approx_eq(scratch_area.width, 70.0 / 1280.0));

    // Without the required option, returns None
    assert!(document.note_lane_area(Lane::Key1, KeyMode::K7, &[]).is_none());
}

#[test]
fn beatoraja_note_index_maps_6k_lanes_without_scratch() {
    assert_eq!(beatoraja_note_index(Lane::Key1, KeyMode::K6), 0);
    assert_eq!(beatoraja_note_index(Lane::Key2, KeyMode::K6), 1);
    assert_eq!(beatoraja_note_index(Lane::Key3, KeyMode::K6), 2);
    assert_eq!(beatoraja_note_index(Lane::Key4, KeyMode::K6), 3);
    assert_eq!(beatoraja_note_index(Lane::Key5, KeyMode::K6), 4);
    assert_eq!(beatoraja_note_index(Lane::Key6, KeyMode::K6), 5);
    assert_eq!(beatoraja_note_index(Lane::Scratch, KeyMode::K6), 5);
}

#[test]
fn beatoraja_note_index_maps_4k_lanes_without_scratch() {
    assert_eq!(beatoraja_note_index(Lane::Key1, KeyMode::K4), 0);
    assert_eq!(beatoraja_note_index(Lane::Key2, KeyMode::K4), 1);
    assert_eq!(beatoraja_note_index(Lane::Key3, KeyMode::K4), 2);
    assert_eq!(beatoraja_note_index(Lane::Key4, KeyMode::K4), 3);
    assert_eq!(beatoraja_note_index(Lane::Scratch, KeyMode::K4), 3);
}

#[test]
fn beatoraja_note_index_maps_8k_lanes_without_scratch() {
    assert_eq!(beatoraja_note_index(Lane::Key1, KeyMode::K8), 0);
    assert_eq!(beatoraja_note_index(Lane::Key2, KeyMode::K8), 1);
    assert_eq!(beatoraja_note_index(Lane::Key3, KeyMode::K8), 2);
    assert_eq!(beatoraja_note_index(Lane::Key4, KeyMode::K8), 3);
    assert_eq!(beatoraja_note_index(Lane::Key5, KeyMode::K8), 4);
    assert_eq!(beatoraja_note_index(Lane::Key6, KeyMode::K8), 5);
    assert_eq!(beatoraja_note_index(Lane::Key7, KeyMode::K8), 6);
    assert_eq!(beatoraja_note_index(Lane::Key8, KeyMode::K8), 7);
    assert_eq!(beatoraja_note_index(Lane::Scratch, KeyMode::K8), 0);
}
