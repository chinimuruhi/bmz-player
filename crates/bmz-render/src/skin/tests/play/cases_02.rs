use super::*;

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
