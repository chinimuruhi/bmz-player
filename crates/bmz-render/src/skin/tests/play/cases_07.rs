use super::*;

fn pm_chara_document() -> SkinDocument {
    let mut document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "pmchara": [
                    { "id": "pm", "src": "pm-source", "color": 1, "type": 0, "side": 1 }
                ],
                "destination": [
                    { "id": "pm", "dst": [{ "x": 10, "y": 20, "w": 40, "h": 60 }] }
                ]
            }
        "#,
    )
    .unwrap();
    document.pmchara[0].runtime = Some(SkinPmCharaRuntimeDef {
        canvas_width: 20,
        canvas_height: 30,
        motions: vec![
            SkinPmCharaMotionLayerDef {
                motion: 1,
                source_id: "pm-bmp".to_string(),
                frame_ms: 100,
                loop_start: 0,
                frames: vec![
                    SkinPmCharaFrameDef {
                        source_x: 0,
                        source_y: 0,
                        source_w: 20,
                        source_h: 30,
                        destination_x: 0,
                        destination_y: 0,
                        destination_w: 20,
                        destination_h: 30,
                        alpha: 255,
                        angle: 0,
                    },
                    SkinPmCharaFrameDef {
                        source_x: 20,
                        source_y: 0,
                        source_w: 20,
                        source_h: 30,
                        destination_x: 5,
                        destination_y: 0,
                        destination_w: 10,
                        destination_h: 30,
                        alpha: 128,
                        angle: 0,
                    },
                ],
            },
            SkinPmCharaMotionLayerDef {
                motion: 7,
                source_id: "pm-bmp".to_string(),
                frame_ms: 200,
                loop_start: 0,
                frames: vec![SkinPmCharaFrameDef {
                    source_x: 20,
                    source_y: 0,
                    source_w: 20,
                    source_h: 30,
                    destination_x: 0,
                    destination_y: 0,
                    destination_w: 20,
                    destination_h: 30,
                    alpha: 255,
                    angle: 0,
                }],
            },
        ],
    });
    document
}

fn pm_chara_sources() -> HashMap<String, SkinDocumentTexture> {
    HashMap::from([(
        "pm-bmp".to_string(),
        SkinDocumentTexture {
            source_id: "pm-bmp".to_string(),
            texture: SkinTextureId(77),
            source_size: SkinImageSize { width: 40.0, height: 30.0 },
        },
    )])
}

#[test]
fn pm_chara_renders_neutral_animation_frames_and_inner_geometry() {
    let document = pm_chara_document();
    let sources = pm_chara_sources();

    let first = document.static_image_render_items(
        &sources,
        &SkinDrawState { elapsed_ms: 0, play_timer_ms: Some(0), ..Default::default() },
    );
    let second = document.static_image_render_items(
        &sources,
        &SkinDrawState { elapsed_ms: 100, play_timer_ms: Some(100), ..Default::default() },
    );

    assert!(matches!(
        first.as_slice(),
        [SkinRenderItem::Image {
            texture: SkinTextureId(77),
            rect: Rect { x, y, width, height },
            uv: TextureRegion { x: uv_x, width: uv_width, .. },
            ..
        }] if approx_eq(*x, 0.1)
            && approx_eq(*y, 0.2)
            && approx_eq(*width, 0.4)
            && approx_eq(*height, 0.6)
            && approx_eq(*uv_x, 0.0)
            && approx_eq(*uv_width, 0.5)
    ));
    assert!(matches!(
        second.as_slice(),
        [SkinRenderItem::Image {
            rect: Rect { x, width, .. },
            uv: TextureRegion { x: uv_x, .. },
            tint: Color { a, .. },
            ..
        }] if approx_eq(*x, 0.2)
            && approx_eq(*width, 0.2)
            && approx_eq(*uv_x, 0.5)
            && approx_eq(*a, 128.0 / 255.0)
    ));
}

#[test]
fn pm_chara_uses_recent_great_reaction_motion() {
    let document = pm_chara_document();
    let sources = pm_chara_sources();
    let mut state = SkinDrawState { elapsed_ms: 1_000, gauge: 50.0, ..Default::default() };
    state.judge_ms[0] = Some(0);
    state.judge_index[0] = Some(1);

    let items = document.static_image_render_items(&sources, &state);

    assert!(matches!(
        items.as_slice(),
        [SkinRenderItem::Image { uv: TextureRegion { x, .. }, .. }] if approx_eq(*x, 0.5)
    ));
}
