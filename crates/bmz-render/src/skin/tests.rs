use bmz_core::ids::NoteId;
use bmz_core::input::{InputDeviceKind, InputEvent, InputKind, InputSource};
use bmz_core::time::TimeUs;

use crate::plan::TextLayer;

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

fn judge_region_state(region: usize, ms: i32, image_index: usize) -> JudgeRegionState {
    let mut judge_ms = [None; MAX_JUDGE_REGIONS];
    let mut judge_index = [None; MAX_JUDGE_REGIONS];
    let mut judge_combo = [0; MAX_JUDGE_REGIONS];
    let mut judge_timing_sign = [None; MAX_JUDGE_REGIONS];
    if region < MAX_JUDGE_REGIONS {
        judge_ms[region] = Some(ms);
        judge_index[region] = Some(image_index);
        judge_combo[region] = 42;
        judge_timing_sign[region] = Some(1);
    }
    JudgeRegionState {
        judge_ms,
        judge_index,
        judge_combo,
        judge_timing_sign,
        judge_timing_ms: [None; MAX_JUDGE_REGIONS],
    }
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
fn select_document_options_follow_selected_song_text_presence() {
    let no_document = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_has_document: false,
        ..SkinDrawState::default()
    };
    let with_document = SkinDrawState { select_has_document: true, ..no_document.clone() };
    let folder = SkinDrawState { select_row_kind: SelectRowKind::Folder, ..with_document.clone() };

    assert!(test_skin_op(174, &[], &no_document));
    assert!(!test_skin_op(175, &[], &no_document));
    assert!(!test_skin_op(174, &[], &with_document));
    assert!(test_skin_op(175, &[], &with_document));
    assert!(!test_skin_op(174, &[], &folder));
    assert!(!test_skin_op(175, &[], &folder));
}

#[test]
fn result_long_note_options_and_index_use_effective_chart_state() {
    let no_ln = SkinDrawState {
        result_has_long_notes: Some(false),
        result_ln_mode_index: Some(0),
        ..SkinDrawState::default()
    };
    assert!(test_skin_op(172, &[], &no_ln));
    assert!(!test_skin_op(173, &[], &no_ln));

    for (index, expected) in [(0, 0), (1, 1), (2, 2)] {
        let with_ln = SkinDrawState {
            result_has_long_notes: Some(true),
            result_ln_mode_index: Some(index),
            ..SkinDrawState::default()
        };
        assert!(!test_skin_op(172, &[], &with_ln));
        assert!(test_skin_op(173, &[], &with_ln));
        assert_eq!(skin_image_index_number(308, &with_ln), Some(expected));
        assert_eq!(skin_state_event_index(308, &with_ln), expected as i32);
    }
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
fn select_row_bar_slots_follow_beatoraja_bar_types() {
    let cases = [
        (SelectRowKind::Song, true, 0, 2),
        (SelectRowKind::Song, false, 4, 8),
        (SelectRowKind::Folder, true, 1, 4),
        (SelectRowKind::TableFolder, true, 2, 6),
        (SelectRowKind::SearchFolder, true, 6, 10),
        (SelectRowKind::Course, true, 3, 7),
        (SelectRowKind::Course, false, 4, 8),
        (SelectRowKind::Executable, true, 2, 6),
        (SelectRowKind::RandomCourse, true, 2, 6),
        (SelectRowKind::RandomCourse, false, 4, 8),
        (SelectRowKind::Command, true, 5, 9),
        (SelectRowKind::Container, true, 5, 9),
        (SelectRowKind::NoSong, false, 4, 8),
        (SelectRowKind::SettingsRoot, true, 8, 11),
        (SelectRowKind::SettingsFolder, true, 8, 11),
        (SelectRowKind::SettingsBack, true, 9, 12),
        (SelectRowKind::SettingsClose, true, 10, 13),
        (SelectRowKind::Config, true, 0, 2),
    ];

    for (kind, in_library, image_index, text_index) in cases {
        let row = SelectRowSnapshot {
            kind,
            in_library,
            is_folder: matches!(
                kind,
                SelectRowKind::Folder
                    | SelectRowKind::TableFolder
                    | SelectRowKind::SearchFolder
                    | SelectRowKind::Command
                    | SelectRowKind::Container
                    | SelectRowKind::SettingsRoot
                    | SelectRowKind::SettingsFolder
                    | SelectRowKind::SettingsBack
                    | SelectRowKind::SettingsClose
            ),
            ..SelectRowSnapshot::default()
        };
        assert_eq!(select_row_bar_image_index(&row), image_index, "image index for {kind:?}");
        assert_eq!(select_row_bar_text_index(&row), text_index, "text index for {kind:?}");
    }
}

#[test]
fn select_settings_rows_use_dedicated_slots_with_legacy_fallbacks() {
    let search = SelectRowSnapshot {
        kind: SelectRowKind::SearchFolder,
        is_folder: true,
        ..SelectRowSnapshot::default()
    };
    let settings_root = SelectRowSnapshot {
        kind: SelectRowKind::SettingsRoot,
        is_folder: true,
        ..SelectRowSnapshot::default()
    };
    let settings_folder = SelectRowSnapshot {
        kind: SelectRowKind::SettingsFolder,
        is_folder: true,
        ..SelectRowSnapshot::default()
    };
    let settings_back = SelectRowSnapshot {
        kind: SelectRowKind::SettingsBack,
        is_folder: true,
        ..SelectRowSnapshot::default()
    };
    let settings_close = SelectRowSnapshot {
        kind: SelectRowKind::SettingsClose,
        is_folder: true,
        ..SelectRowSnapshot::default()
    };

    assert_eq!(select_row_bar_image_index(&search), 6);
    assert_eq!(select_row_bar_text_index(&search), 10);
    assert_eq!(select_row_bar_image_fallback_indices(&search), &[1]);
    assert_eq!(select_row_bar_text_fallback_indices(&search), &[4]);
    let cases: [(&SelectRowSnapshot, usize, usize, &[usize], &[usize]); 4] = [
        (&settings_root, 8, 11, &[6, 1], &[10, 4]),
        (&settings_folder, 8, 11, &[1], &[4]),
        (&settings_back, 9, 12, &[6, 1], &[10, 4]),
        (&settings_close, 10, 13, &[6, 1], &[10, 4]),
    ];
    for (row, image, text, fallback_images, fallback_texts) in cases {
        assert_eq!(select_row_bar_image_index(row), image);
        assert_eq!(select_row_bar_text_index(row), text);
        assert_eq!(select_row_bar_image_fallback_indices(row), fallback_images);
        assert_eq!(select_row_bar_text_fallback_indices(row), fallback_texts);
    }
}

#[test]
fn select_settings_image_slots_follow_legacy_search_and_folder_fallbacks() {
    let dedicated_slots: Vec<_> = (0..11).collect();
    for primary in [8, 9, 10] {
        assert_eq!(
            select_row_slot_with_fallbacks(&dedicated_slots, primary, &[6, 1]).copied(),
            Some(primary)
        );
    }

    // beatoraja skin may define index 7 as a legacy no-song bar. BMZ settings
    // slots start after all eight existing entries, so ECFN-style arrays
    // continue to use the search bar at index 6.
    let beatoraja_slots: Vec<_> = (0..8).collect();
    for primary in [8, 9, 10] {
        assert_eq!(
            select_row_slot_with_fallbacks(&beatoraja_slots, primary, &[6, 1]).copied(),
            Some(6)
        );
    }

    let folder_slots: Vec<_> = (0..2).collect();
    assert_eq!(select_row_slot_with_fallbacks(&folder_slots, 8, &[6, 1]).copied(), Some(1));

    assert_eq!(select_row_slot_with_fallbacks(&[0], 8, &[6, 1]).copied(), Some(0));
    assert_eq!(select_row_slot_with_fallbacks::<usize>(&[], 8, &[6, 1]), None);
}

#[test]
fn select_bar_type_ops_match_song_folder_and_course_rows() {
    let song = SkinDrawState {
        select_row_kind: SelectRowKind::Song,
        select_is_folder: false,
        ..SkinDrawState::default()
    };
    let folder = SkinDrawState {
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let table_folder = SkinDrawState {
        select_row_kind: SelectRowKind::TableFolder,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let search_folder = SkinDrawState {
        select_row_kind: SelectRowKind::SearchFolder,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let settings_folder = SkinDrawState {
        select_row_kind: SelectRowKind::SettingsFolder,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let settings_root = SkinDrawState {
        select_row_kind: SelectRowKind::SettingsRoot,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let settings_back = SkinDrawState {
        select_row_kind: SelectRowKind::SettingsBack,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let settings_close = SkinDrawState {
        select_row_kind: SelectRowKind::SettingsClose,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let command = SkinDrawState {
        select_row_kind: SelectRowKind::Command,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let container = SkinDrawState {
        select_row_kind: SelectRowKind::Container,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let executable = SkinDrawState {
        select_row_kind: SelectRowKind::Executable,
        select_is_folder: false,
        ..SkinDrawState::default()
    };
    let random_course = SkinDrawState {
        select_row_kind: SelectRowKind::RandomCourse,
        select_is_folder: false,
        ..SkinDrawState::default()
    };
    let course = SkinDrawState {
        select_row_kind: SelectRowKind::Course,
        select_is_folder: false,
        ..SkinDrawState::default()
    };
    let unowned_song = SkinDrawState {
        select_row_kind: SelectRowKind::Song,
        select_is_folder: false,
        select_in_library: false,
        ..SkinDrawState::default()
    };

    assert!(test_skin_op(2, &[], &song));
    assert!(test_skin_op(2, &[], &unowned_song));
    assert!(!test_skin_op(1, &[], &song));
    assert!(!test_skin_op(3, &[], &song));
    assert!(test_skin_op(1, &[], &folder));
    assert!(test_skin_op(1, &[], &table_folder));
    assert!(test_skin_op(1, &[], &search_folder));
    assert!(test_skin_op(1, &[], &settings_root));
    assert!(test_skin_op(1, &[], &settings_folder));
    assert!(test_skin_op(1, &[], &settings_back));
    assert!(test_skin_op(1, &[], &settings_close));
    assert!(test_skin_op(SKIN_OPTION_BMZ_SETTINGS_FOLDER, &[], &settings_root));
    assert!(test_skin_op(SKIN_OPTION_BMZ_SETTINGS_FOLDER, &[], &settings_folder));
    assert!(!test_skin_op(SKIN_OPTION_BMZ_SETTINGS_FOLDER, &[], &settings_back));
    assert!(test_skin_op(SKIN_OPTION_BMZ_SETTINGS_BACK, &[], &settings_back));
    assert!(test_skin_op(SKIN_OPTION_BMZ_SETTINGS_CLOSE, &[], &settings_close));
    assert!(test_skin_op(1, &[], &command));
    assert!(test_skin_op(1, &[], &container));
    assert!(!test_skin_op(2, &[], &folder));
    assert!(test_skin_op(3, &[], &course));
    assert!(!test_skin_op(2, &[], &course));
    assert!(test_skin_op(1030, &[], &executable));
    assert!(!test_skin_op(1030, &[], &random_course));
    assert!(test_skin_op(1031, &[], &random_course));
    assert!(!test_skin_op(1031, &[], &course));
}

#[test]
fn select_settings_row_ref_distinguishes_folder_back_and_close() {
    let cases = [
        (SelectRowKind::Song, 0),
        (SelectRowKind::SettingsRoot, 1),
        (SelectRowKind::SettingsFolder, 1),
        (SelectRowKind::SettingsBack, 2),
        (SelectRowKind::SettingsClose, 3),
    ];

    for (kind, expected) in cases {
        let state = SkinDrawState {
            select_screen: true,
            select_row_kind: kind,
            ..SkinDrawState::default()
        };
        assert_eq!(skin_state_event_index(SKIN_REF_BMZ_SELECT_SETTINGS_ROW_KIND, &state), expected);
        assert_eq!(
            skin_state_number(SKIN_REF_BMZ_SELECT_SETTINGS_ROW_KIND, &state),
            Some(i64::from(expected))
        );
    }
}

#[test]
fn gradebar_constraint_ops_match_course_constraint_flags() {
    let course = SkinDrawState {
        select_row_kind: SelectRowKind::Course,
        select_course_constraints: CourseConstraintFlags {
            mirror: true,
            no_speed: true,
            no_great: true,
            gauge_7k: true,
            hcn: true,
            ..CourseConstraintFlags::default()
        },
        ..SkinDrawState::default()
    };
    let song = SkinDrawState {
        select_row_kind: SelectRowKind::Song,
        select_course_constraints: course.select_course_constraints,
        ..SkinDrawState::default()
    };

    assert!(test_skin_op(1003, &[], &course));
    assert!(test_skin_op(1005, &[], &course));
    assert!(test_skin_op(1007, &[], &course));
    assert!(test_skin_op(1012, &[], &course));
    assert!(test_skin_op(1017, &[], &course));
    assert!(!test_skin_op(1002, &[], &course));
    assert!(!test_skin_op(1016, &[], &course));
    assert!(!test_skin_op(1003, &[], &song));
    assert!(test_skin_op(-1003, &[], &song));
}

#[test]
fn table_song_op_matches_table_context() {
    let table_song = SkinDrawState { table_song: true, ..SkinDrawState::default() };
    let non_table_song = SkinDrawState::default();

    assert!(test_skin_op(1008, &[], &table_song));
    assert!(test_skin_op(-1008, &[], &non_table_song));
    assert!(!test_skin_op(1008, &[], &non_table_song));
}

#[test]
fn select_row_trophy_index_prefers_achieved_course_trophy_names() {
    let row = SelectRowSnapshot {
        kind: SelectRowKind::Course,
        achieved_trophy_names: vec!["bronzemedal".to_string(), "goldmedal".to_string()],
        ex_score: Some(0),
        total_notes: 100,
        ..SelectRowSnapshot::default()
    };
    assert_eq!(select_row_trophy_index(&row), Some(2));

    let silver = SelectRowSnapshot {
        kind: SelectRowKind::Course,
        achieved_trophy_names: vec!["silvermedal".to_string()],
        ..SelectRowSnapshot::default()
    };
    assert_eq!(select_row_trophy_index(&silver), Some(1));

    let high_score_without_trophy = SelectRowSnapshot {
        kind: SelectRowKind::Course,
        total_notes: 100,
        ex_score: Some(200),
        ..SelectRowSnapshot::default()
    };
    assert_eq!(select_row_trophy_index(&high_score_without_trophy), None);
}

#[test]
fn playable_bar_op_matches_library_presence() {
    let owned_song = SkinDrawState {
        select_row_kind: SelectRowKind::Song,
        select_is_folder: false,
        select_in_library: true,
        ..SkinDrawState::default()
    };
    let unowned_song = SkinDrawState {
        select_row_kind: SelectRowKind::Song,
        select_is_folder: false,
        select_in_library: false,
        ..SkinDrawState::default()
    };
    let owned_course = SkinDrawState {
        select_row_kind: SelectRowKind::Course,
        select_is_folder: false,
        select_in_library: true,
        ..SkinDrawState::default()
    };
    let unowned_course = SkinDrawState {
        select_row_kind: SelectRowKind::Course,
        select_is_folder: false,
        select_in_library: false,
        ..SkinDrawState::default()
    };
    let owned_random_course = SkinDrawState {
        select_row_kind: SelectRowKind::RandomCourse,
        select_is_folder: false,
        select_in_library: true,
        ..SkinDrawState::default()
    };
    let executable = SkinDrawState {
        select_row_kind: SelectRowKind::Executable,
        select_is_folder: false,
        select_in_library: false,
        ..SkinDrawState::default()
    };
    let folder = SkinDrawState {
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        select_in_library: true,
        ..SkinDrawState::default()
    };

    assert!(test_skin_op(5, &[], &owned_song));
    assert!(!test_skin_op(5, &[], &unowned_song));
    assert!(test_skin_op(5, &[], &owned_course));
    assert!(!test_skin_op(5, &[], &unowned_course));
    assert!(test_skin_op(5, &[], &owned_random_course));
    assert!(test_skin_op(5, &[], &executable));
    assert!(!test_skin_op(5, &[], &folder));
    assert!(!test_skin_op(-5, &[], &owned_song));
    assert!(test_skin_op(-5, &[], &unowned_song));
    assert!(test_skin_op(-5, &[], &folder));
}

#[test]
fn select_banner_ops_follow_selected_banner_presence() {
    let no_banner =
        SkinDrawState { select_screen: true, select_has_banner: false, ..SkinDrawState::default() };
    let with_banner =
        SkinDrawState { select_screen: true, select_has_banner: true, ..SkinDrawState::default() };
    let play_screen =
        SkinDrawState { select_screen: false, select_has_banner: true, ..SkinDrawState::default() };

    assert!(test_skin_op(192, &[], &no_banner));
    assert!(!test_skin_op(193, &[], &no_banner));
    assert!(!test_skin_op(192, &[], &with_banner));
    assert!(test_skin_op(193, &[], &with_banner));
    assert!(!test_skin_op(192, &[], &play_screen));
    assert!(!test_skin_op(193, &[], &play_screen));

    assert!(test_skin_ops(&[2, 192], &[], &no_banner));
    assert!(!test_skin_ops(&[2, 193], &[], &no_banner));
    assert!(!test_skin_ops(&[2, 192], &[], &with_banner));
    assert!(test_skin_ops(&[2, 193], &[], &with_banner));
}

#[test]
fn play_mode_option_ops_reflect_autoplay_and_course_stage() {
    let normal_play = SkinDrawState::default();
    let autoplay = SkinDrawState { autoplay: true, ..SkinDrawState::default() };
    let course_stage1 =
        SkinDrawState { course_stage: Some(CourseStageMarker::Stage1), ..SkinDrawState::default() };
    let course_final =
        SkinDrawState { course_stage: Some(CourseStageMarker::Final), ..SkinDrawState::default() };

    // Starseeker freestage: op = {32, -290}
    assert!(test_skin_op(32, &[], &normal_play));
    assert!(!test_skin_op(290, &[], &normal_play));
    assert!(test_skin_ops(&[32, -290], &[], &normal_play));

    // Starseeker auto_play: op = {33}
    assert!(!test_skin_op(33, &[], &normal_play));
    assert!(test_skin_op(33, &[], &autoplay));

    // Course stage labels
    assert!(test_skin_ops(&[32, 290, 280], &[], &course_stage1));
    assert!(!test_skin_ops(&[32, 290, 280], &[], &course_final));
    assert!(test_skin_ops(&[32, 290, 289], &[], &course_final));

    // beatoraja currently leaves these defined constants without BooleanProperty handlers.
    for op in 291..=293 {
        assert!(
            !test_skin_op(op, &[op], &course_stage1),
            "{op} must not fall back to property defaults"
        );
        assert!(test_skin_op(-op, &[op], &course_stage1), "negative {op} should invert false");
    }
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
fn folded_constant_draw_condition_number_zero_is_true() {
    assert!(eval_skin_draw_condition("number(0) >= 0", &SkinDrawState::default()));
    assert!(!eval_skin_draw_condition("number(0) < 0", &SkinDrawState::default()));
}

#[test]
fn result_panel_draw_condition_uses_runtime_selection() {
    let ir = SkinDrawState { result_panel: Some(1), ..Default::default() };
    let graph = SkinDrawState { result_panel: Some(2), ..Default::default() };
    assert!(eval_skin_draw_condition("result_panel(1)", &ir));
    assert!(!eval_skin_draw_condition("result_panel(2)", &ir));
    assert!(eval_skin_draw_condition("result_panel(0) or result_panel(2)", &graph,));
}

#[test]
fn wmii_result_draw_predicates_use_runtime_score_and_nearest_rank() {
    let near_aa = SkinDrawState { ex_score: 155, total_notes: 100, ..Default::default() };
    assert!(eval_skin_draw_condition("score_rate_band(6,7)", &near_aa));
    assert!(!eval_skin_draw_condition("score_rate_band(7,8)", &near_aa));
    assert!(eval_skin_draw_condition("nearest_rank(AA,minus)", &near_aa));
    assert!(eval_skin_draw_condition("nearest_rank_sign(minus)", &near_aa));
    assert!(!eval_skin_draw_condition("nearest_rank(A,plus)", &near_aa));

    let max = SkinDrawState { ex_score: 200, total_notes: 100, ..Default::default() };
    assert!(eval_skin_draw_condition("score_rate_band(9,10)", &max));
    assert!(eval_skin_draw_condition("nearest_rank(MAX,plus)", &max));
}

#[test]
fn select_score_available_requires_an_actual_score_record() {
    let folder = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        select_in_library: true,
        select_ex_score: Some(1234),
        ..SkinDrawState::default()
    };
    let zero_score = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_ex_score: Some(0),
        ..SkinDrawState::default()
    };
    let out_of_library = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: false,
        select_ex_score: Some(1234),
        ..SkinDrawState::default()
    };

    assert!(!eval_skin_draw_condition("select_score_available()", &folder));
    assert!(eval_skin_draw_condition("select_score_available()", &zero_score));
    assert!(!eval_skin_draw_condition("select_score_available()", &out_of_library));
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
fn select_rank_ops_reflect_selected_ex_score() {
    let aa_state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_ex_score: Some(1556),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };
    let max_state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_ex_score: Some(2000),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };
    let f_state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_ex_score: Some(300),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };

    assert!(test_skin_op(201, &[], &aa_state));
    assert!(test_skin_op(302, &[], &aa_state));
    assert!(!test_skin_op(200, &[], &aa_state));
    assert!(test_skin_op(-200, &[], &aa_state));
    assert!(test_skin_op(200, &[], &max_state));
    assert!(test_skin_op(300, &[], &max_state));
    assert!(test_skin_op(207, &[], &f_state));
    assert!(!test_skin_op(307, &[], &f_state));
    assert!(!test_skin_op(200, &[], &SkinDrawState::default()));
}

#[test]
fn select_rank_ops_are_false_for_folder_rows() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        select_in_library: true,
        select_ex_score: Some(1556),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };

    assert!(!test_skin_op(201, &[], &state));
    assert!(!test_skin_op(302, &[], &state));
}

#[test]
fn select_key_mode_op_160_requires_song_row_key_mode() {
    let config_row = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Config,
        in_settings: true,
        ..SkinDrawState::default()
    };
    assert!(!test_skin_op(160, &[], &config_row));

    let song_7k = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_chart_key_mode: Some(KeyMode::K7),
        ..SkinDrawState::default()
    };
    assert!(test_skin_op(160, &[], &song_7k));
    assert!(!test_skin_op(161, &[], &song_7k));
    assert!(test_skin_op(SKIN_OPTION_BMZ_KEY_MODE_BASE + 3, &[], &song_7k));
    assert!(test_skin_op(SKIN_OPTION_BMZ_SINGLE_PLAY, &[], &song_7k));
    assert!(!test_skin_op(SKIN_OPTION_BMZ_NO_SCRATCH, &[], &song_7k));
    assert_eq!(skin_state_number(SKIN_REF_BMZ_KEY_MODE, &song_7k), Some(7));
    assert_eq!(skin_state_number(SKIN_REF_BMZ_ACTIVE_LANE_COUNT, &song_7k), Some(8));

    let folder = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Folder,
        select_chart_key_mode: Some(KeyMode::K7),
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(SKIN_REF_BMZ_KEY_MODE, &folder), None);
    assert!(!test_skin_op(SKIN_OPTION_BMZ_KEY_MODE_BASE + 3, &[], &folder));
}

#[test]
fn result_key_mode_ops_use_result_key_mode() {
    let result_5k = SkinDrawState {
        result_failed: Some(false),
        key_mode: KeyMode::K5,
        ..SkinDrawState::default()
    };
    assert!(test_skin_op(161, &[], &result_5k));
    assert!(!test_skin_op(160, &[], &result_5k));

    let result_14k = SkinDrawState { key_mode: KeyMode::K14, ..result_5k };
    assert!(test_skin_op(162, &[], &result_14k));
    assert!(test_skin_op(SKIN_OPTION_BMZ_KEY_MODE_LAST, &[], &result_14k));
    assert!(test_skin_op(SKIN_OPTION_BMZ_DOUBLE_PLAY, &[], &result_14k));
    assert_eq!(skin_state_event_index(SKIN_REF_BMZ_KEY_MODE, &result_14k), 14);
    assert_eq!(skin_state_number(SKIN_REF_BMZ_ACTIVE_LANE_COUNT, &result_14k), Some(16));
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
fn select_settings_screen_hides_bpm_numbers() {
    let state = SkinDrawState {
        select_screen: true,
        in_settings: true,
        select_max_bpm: 180.0,
        select_min_bpm: 120.0,
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(90, &state), None);
    assert_eq!(skin_state_number(91, &state), None);
}

#[test]
fn select_settings_screen_volume_numbers_match_beatoraja_refs() {
    let state = SkinDrawState {
        select_screen: true,
        in_settings: true,
        select_master_volume: 0.42,
        select_key_volume: 0.73,
        select_bgm_volume: 0.18,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(57, &state), Some(42));
    assert_eq!(skin_state_number(58, &state), Some(73));
    assert_eq!(skin_state_number(59, &state), Some(18));
}

#[test]
fn select_rank_and_judge_ops_are_hidden_in_settings() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Config,
        select_in_library: true,
        select_ex_score: Some(1556),
        select_total_notes: 1000,
        judge_rank: Some(2),
        in_settings: true,
        ..SkinDrawState::default()
    };

    assert!(!test_skin_op(200, &[], &state));
    assert!(!test_skin_op(201, &[], &state));
    assert!(!test_skin_op(302, &[], &state));
    assert!(!test_skin_op(180, &[], &state));
}

#[test]
fn select_detail_artist_shows_config_value_in_settings() {
    let snapshot = SelectSnapshot {
        in_settings: true,
        settings_editing: true,
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            title: "MASTER".to_string(),
            artist: "25".to_string(),
            kind: SelectRowKind::Config,
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };
    let row = &snapshot.rows[0];
    assert_eq!(select_detail_artist(&snapshot, Some(row)), "25");
    assert_eq!(select_detail_subtitle(&snapshot, Some(row)), "[編集中]");
    assert_eq!(
        skin_state_text(
            &SkinTextDef { id: "t".to_string(), ref_id: 3, ..SkinTextDef::default() },
            &SkinTextState { target: "", ..SkinTextState::default() },
        ),
        ""
    );
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
fn nearest_result_diff_rank_destinations_use_target_grade() {
    fn destination(id: &str, op: i32) -> SkinDestinationDef {
        SkinDestinationDef {
            id: id.to_string(),
            blend: 0,
            filter: 0,
            timer: None,
            timer_expr: String::new(),
            loop_time: None,
            center: 0,
            offset: 0,
            offsets: Vec::new(),
            stretch: default_stretch(),
            op: vec![op],
            draw: String::new(),
            act: None,
            click: 0,
            clickable: None,
            dst: Vec::new(),
            mouse_rect: None,
        }
    }
    fn grade_diff_value() -> SkinValueDef {
        SkinValueDef {
            id: "RANK_Diff_Exscore".to_string(),
            src: "num".to_string(),
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            divx: default_grid_division(),
            divy: default_grid_division(),
            timer: None,
            cycle: 0,
            align: 0,
            judge_align: None,
            digit: 0,
            padding: 0,
            zeropadding: 0,
            space: 0,
            ref_id: 154,
            expr: String::new(),
            value_expr: String::new(),
            offset: Vec::new(),
        }
    }

    let max_minus = SkinDrawState {
        ex_score: 1900,
        total_notes: 1000,
        result_failed: Some(false),
        result_grade_diff_display: ResultGradeDiffDisplay::Nearest,
        ..SkinDrawState::default()
    };
    assert!(destination_ops_match(&destination("RANK_s_MAX", 300), &[], &max_minus, false));
    assert!(!destination_ops_match(&destination("RANK_s_AAA", 301), &[], &max_minus, false));
    assert!(destination_ops_match(&destination("RANK_m_AAA", 300), &[], &max_minus, false));

    let aaa_plus = SkinDrawState {
        ex_score: 1100,
        total_notes: 594,
        result_failed: Some(false),
        result_grade_diff_display: ResultGradeDiffDisplay::Nearest,
        ..SkinDrawState::default()
    };
    assert!(destination_ops_match(&destination("RANK_s_AAA", 301), &[], &aaa_plus, false));
    assert!(!destination_ops_match(&destination("RANK_s_MAX", 300), &[], &aaa_plus, false));

    let nearest_e_minus = SkinDrawState {
        select_ex_score: Some(0),
        select_total_notes: 2253,
        select_play_count: 1,
        select_screen: true,
        ..SkinDrawState::default()
    };
    assert!(destination_ops_match(&destination("RANK_s_E", 307), &[], &nearest_e_minus, false));
    assert!(!destination_ops_match(&destination("RANK_s_D", 306), &[], &nearest_e_minus, false));

    let nearest_aaa_minus = SkinDrawState {
        select_ex_score: Some(1774),
        select_total_notes: 1000,
        select_play_count: 1,
        select_screen: true,
        ..SkinDrawState::default()
    };
    assert!(destination_ops_match(&destination("RANK_s_AAA", 301), &[], &nearest_aaa_minus, false));
    assert!(!destination_ops_match(
        &destination("RANK_s_MAX", 300),
        &[],
        &nearest_aaa_minus,
        false
    ));

    let f_plus = SkinDrawState {
        ex_score: 100,
        total_notes: 1000,
        result_failed: Some(false),
        result_grade_diff_display: ResultGradeDiffDisplay::Nearest,
        ..SkinDrawState::default()
    };
    assert!(destination_ops_match(&destination("RANK_s_E", 307), &[], &f_plus, false));
    assert!(!destination_ops_match(&destination("RANK_s_F", 307), &[], &f_plus, false));
    assert_eq!(skin_value_number_for_destination(&grade_diff_value(), &f_plus, false), Some(-345));
    assert_eq!(
        skin_state_number(
            154,
            &SkinDrawState { result_grade_diff_f_fallback_to_e: true, ..f_plus.clone() }
        ),
        Some(-345)
    );

    assert!(destination_ops_match(&destination("RANK_s_F", 307), &[], &f_plus, true));
    assert!(!destination_ops_match(&destination("RANK_s_E", 307), &[], &f_plus, true));
    assert_eq!(skin_value_number_for_destination(&grade_diff_value(), &f_plus, true), Some(100));
    assert!(destination_ops_match(&destination("RANK_m_F", 307), &[], &f_plus, false));
}

#[test]
fn nearest_result_diff_number_renders_negative_when_f_rank_destination_is_missing() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 7,
                "w": 100,
                "h": 100,
                "value": [
                    {
                        "id": "RANK_Diff_Exscore",
                        "src": "num",
                        "x": 0,
                        "y": 0,
                        "w": 120,
                        "h": 40,
                        "divx": 12,
                        "divy": 2,
                        "digit": 5,
                        "ref": 154,
                        "zeropadding": 2
                    }
                ],
                "destination": [
                    {
                        "id": "RANK_s_E",
                        "op": [307],
                        "dst": [{"x": 0, "y": 20, "w": 10, "h": 10}]
                    },
                    {
                        "id": "RANK_Diff_Exscore",
                        "dst": [{"x": 10, "y": 20, "w": 10, "h": 10}]
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "num".to_string(),
        SkinDocumentTexture {
            source_id: "num".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 120.0, height: 40.0 },
        },
    )]);
    let state = SkinDrawState {
        ex_score: 100,
        total_notes: 1000,
        result_failed: Some(false),
        result_grade_diff_display: ResultGradeDiffDisplay::Nearest,
        ..SkinDrawState::default()
    };

    let items = document.static_render_items(&sources, &state, &SkinTextState::default());
    let first_digit_uv = items.iter().find_map(|item| match item {
        SkinRenderItem::Image { texture: SkinTextureId(42), uv, .. } => Some(*uv),
        _ => None,
    });

    assert_eq!(first_digit_uv.map(|uv| uv.y), Some(0.5));
}

#[test]
fn nearest_select_diff_number_renders_e_minus_when_f_rank_destination_is_missing() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [
                    {"id": "rank", "path": "rank.png"}
                ],
                "image": [
                    {"id": "RANK_s_E", "src": "rank", "x": 0, "y": 0, "w": 45, "h": 19}
                ],
                "value": [
                    {
                        "id": "RANK_Diff_Exscore",
                        "src": "num",
                        "x": 0,
                        "y": 0,
                        "w": 120,
                        "h": 40,
                        "divx": 12,
                        "divy": 2,
                        "digit": 4,
                        "ref": 154,
                        "zeropadding": 2
                    }
                ],
                "destination": [
                    {
                        "id": "RANK_s_E",
                        "op": [307],
                        "dst": [{"x": 0, "y": 20, "w": 10, "h": 10}]
                    },
                    {
                        "id": "RANK_Diff_Exscore",
                        "dst": [{"x": 10, "y": 20, "w": 10, "h": 10}]
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([
        (
            "num".to_string(),
            SkinDocumentTexture {
                source_id: "num".to_string(),
                texture: SkinTextureId(42),
                source_size: SkinImageSize { width: 120.0, height: 40.0 },
            },
        ),
        (
            "rank".to_string(),
            SkinDocumentTexture {
                source_id: "rank".to_string(),
                texture: SkinTextureId(7),
                source_size: SkinImageSize { width: 45.0, height: 19.0 },
            },
        ),
    ]);
    let snapshot = SelectSnapshot {
        rows: vec![SelectRowSnapshot {
            index: 0,
            ex_score: Some(100),
            total_notes: 1000,
            in_library: true,
            ..SelectRowSnapshot::default()
        }],
        chart_count: 1,
        grade_diff_display: ResultGradeDiffDisplay::Nearest,
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);
    let first_digit_uv = items.iter().find_map(|item| match item {
        SkinRenderItem::Image { texture: SkinTextureId(42), uv, .. } => Some(*uv),
        _ => None,
    });

    assert_eq!(first_digit_uv.map(|uv| uv.y), Some(0.0));
    assert!(
        items
            .iter()
            .any(|item| matches!(item, SkinRenderItem::Image { texture: SkinTextureId(7), .. }))
    );
}

#[test]
fn next_select_diff_number_renders_next_rank_label() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [
                    {"id": "rank", "path": "rank.png"}
                ],
                "image": [
                    {"id": "RANK_s_E", "src": "rank", "x": 0, "y": 0, "w": 45, "h": 19}
                ],
                "value": [
                    {
                        "id": "RANK_Diff_Exscore",
                        "src": "num",
                        "x": 0,
                        "y": 0,
                        "w": 120,
                        "h": 40,
                        "divx": 12,
                        "divy": 2,
                        "digit": 4,
                        "ref": 154,
                        "zeropadding": 2
                    }
                ],
                "destination": [
                    {
                        "id": "RANK_s_E",
                        "op": [307],
                        "dst": [{"x": 0, "y": 20, "w": 10, "h": 10}]
                    },
                    {
                        "id": "RANK_Diff_Exscore",
                        "dst": [{"x": 10, "y": 20, "w": 10, "h": 10}]
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([
        (
            "num".to_string(),
            SkinDocumentTexture {
                source_id: "num".to_string(),
                texture: SkinTextureId(42),
                source_size: SkinImageSize { width: 120.0, height: 40.0 },
            },
        ),
        (
            "rank".to_string(),
            SkinDocumentTexture {
                source_id: "rank".to_string(),
                texture: SkinTextureId(7),
                source_size: SkinImageSize { width: 45.0, height: 19.0 },
            },
        ),
    ]);
    let snapshot = SelectSnapshot {
        rows: vec![SelectRowSnapshot {
            index: 0,
            ex_score: Some(0),
            play_count: 1,
            total_notes: 2253,
            in_library: true,
            ..SelectRowSnapshot::default()
        }],
        chart_count: 1,
        grade_diff_display: ResultGradeDiffDisplay::Next,
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);
    let first_digit_uv = items.iter().find_map(|item| match item {
        SkinRenderItem::Image { texture: SkinTextureId(42), uv, .. } => Some(*uv),
        _ => None,
    });

    let (state, _) = document.select_draw_state(&snapshot, None);
    assert_eq!(skin_state_number(154, &state), Some(-501));
    assert_eq!(first_digit_uv.map(|uv| uv.y), Some(0.0));
    assert!(
        items
            .iter()
            .any(|item| matches!(item, SkinRenderItem::Image { texture: SkinTextureId(7), .. }))
    );

    let no_play_snapshot = SelectSnapshot {
        rows: vec![SelectRowSnapshot {
            index: 0,
            ex_score: None,
            play_count: 0,
            total_notes: 2253,
            in_library: true,
            ..SelectRowSnapshot::default()
        }],
        chart_count: 1,
        grade_diff_display: ResultGradeDiffDisplay::Next,
        ..SelectSnapshot::default()
    };
    let no_play_items = document.select_render_items(&sources, &no_play_snapshot);
    let (no_play_state, _) = document.select_draw_state(&no_play_snapshot, None);
    assert_eq!(skin_state_number(154, &no_play_state), None);
    assert!(!no_play_items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { texture: SkinTextureId(7) | SkinTextureId(42), .. }
    )));

    let no_play_zero_snapshot = SelectSnapshot {
        rows: vec![SelectRowSnapshot {
            index: 0,
            ex_score: Some(0),
            play_count: 0,
            total_notes: 2253,
            in_library: true,
            ..SelectRowSnapshot::default()
        }],
        chart_count: 1,
        grade_diff_display: ResultGradeDiffDisplay::Next,
        ..SelectSnapshot::default()
    };
    let no_play_zero_items = document.select_render_items(&sources, &no_play_zero_snapshot);
    let (no_play_zero_state, _) = document.select_draw_state(&no_play_zero_snapshot, None);
    assert_eq!(skin_state_number(154, &no_play_zero_state), None);
    assert!(!no_play_zero_items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { texture: SkinTextureId(7) | SkinTextureId(42), .. }
    )));
}

#[test]
fn select_diff_number_renders_max_zero_as_positive_row() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "value": [
                    {
                        "id": "RANK_Diff_Exscore",
                        "src": "num",
                        "x": 0,
                        "y": 0,
                        "w": 120,
                        "h": 40,
                        "divx": 12,
                        "divy": 2,
                        "digit": 4,
                        "ref": 154,
                        "zeropadding": 2
                    }
                ],
                "destination": [
                    {
                        "id": "RANK_Diff_Exscore",
                        "dst": [{"x": 10, "y": 20, "w": 10, "h": 10}]
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "num".to_string(),
        SkinDocumentTexture {
            source_id: "num".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 120.0, height: 40.0 },
        },
    )]);
    let snapshot = SelectSnapshot {
        rows: vec![SelectRowSnapshot {
            index: 0,
            ex_score: Some(2000),
            total_notes: 1000,
            in_library: true,
            ..SelectRowSnapshot::default()
        }],
        chart_count: 1,
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);
    let first_digit_uv = items.iter().find_map(|item| match item {
        SkinRenderItem::Image { texture: SkinTextureId(42), uv, .. } => Some(*uv),
        _ => None,
    });

    let (state, _) = document.select_draw_state(&snapshot, None);
    assert_eq!(skin_state_number(154, &state), Some(0));
    assert_eq!(first_digit_uv.map(|uv| uv.y), Some(0.5));
}

#[test]
fn select_replay_ops_reflect_replay_slots_and_selection() {
    let no_replay = SkinDrawState::default();
    let first_replay = SkinDrawState {
        select_replay_slots: [true, false, false, false],
        select_replay_index: Some(0),
        ..SkinDrawState::default()
    };
    let second_replay = SkinDrawState {
        select_replay_slots: [false, true, false, false],
        select_replay_index: Some(1),
        ..SkinDrawState::default()
    };

    assert!(test_skin_op(196, &[], &no_replay));
    assert!(!test_skin_op(197, &[], &no_replay));
    assert!(!test_skin_op(1205, &[], &no_replay));
    assert!(test_skin_op(197, &[], &first_replay));
    assert!(!test_skin_op(196, &[], &first_replay));
    assert!(test_skin_op(1205, &[], &first_replay));
    assert!(test_skin_op(-1205, &[], &no_replay));
    assert!(test_skin_op(1197, &[], &second_replay));
    assert!(test_skin_op(1206, &[], &second_replay));
    assert!(!test_skin_op(1205, &[], &second_replay));
    assert!(!test_skin_op(198, &[], &first_replay));
}

#[test]
fn result_replay_ops_reflect_result_replay_slots() {
    let no_replay = SkinDrawState { result_failed: Some(false), ..SkinDrawState::default() };
    let existing = SkinDrawState {
        result_failed: Some(false),
        result_replay_slots: [true, false, false, false],
        ..SkinDrawState::default()
    };
    let saved = SkinDrawState {
        result_failed: Some(false),
        result_replay_slots: [true, true, false, false],
        result_saved_replay_slots: [true, false, false, false],
        ..SkinDrawState::default()
    };

    assert!(test_skin_op(196, &[], &no_replay));
    assert!(!test_skin_op(197, &[], &no_replay));
    assert!(!test_skin_op(198, &[], &no_replay));
    assert!(test_skin_op(197, &[], &existing));
    assert!(!test_skin_op(196, &[], &existing));
    assert!(!test_skin_op(198, &[], &existing));
    assert!(test_skin_op(198, &[], &saved));
    assert!(!test_skin_op(197, &[], &saved));
    assert!(test_skin_op(1197, &[], &saved));
    assert!(!test_skin_op(1198, &[], &saved));
}

#[test]
fn select_row_snapshot_carries_achieved_trophy_names() {
    // SelectRowSnapshot is the carrier — SkinDrawState intentionally does
    // not duplicate this field (it must stay Copy).  This test simply
    // pins down that course rows preserve the data and song rows default
    // to empty, so future skin ops have a stable contract to consume.
    use crate::scene::{SelectRowKind, SelectRowSnapshot};
    let course = SelectRowSnapshot {
        kind: SelectRowKind::Course,
        achieved_trophy_names: vec!["gold".to_string(), "silver".to_string()],
        ..SelectRowSnapshot::default()
    };
    let song = SelectRowSnapshot { kind: SelectRowKind::Song, ..SelectRowSnapshot::default() };

    assert_eq!(course.achieved_trophy_names, vec!["gold".to_string(), "silver".to_string()]);
    assert!(song.achieved_trophy_names.is_empty());
}

#[test]
fn select_row_replay_index_is_row_kind_agnostic() {
    // Regression: course rows must surface their replay slot indicators
    // exactly like song rows.  `select_row_replay_index` looks only at
    // `row.replay_slots`, so swapping row.kind must not change the
    // result.  This locks the invariant for future refactors.
    use crate::scene::{SelectRowKind, SelectRowSnapshot};
    let song = SelectRowSnapshot {
        kind: SelectRowKind::Song,
        replay_slots: [false, true, false, true],
        ..SelectRowSnapshot::default()
    };
    let course = SelectRowSnapshot {
        kind: SelectRowKind::Course,
        replay_slots: [false, true, false, true],
        ..SelectRowSnapshot::default()
    };

    assert_eq!(select_row_replay_index(&song), Some(1));
    assert_eq!(select_row_replay_index(&course), Some(1));
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
fn peaceful_gauge_value_overlay_selects_exactly_one_integer_width() {
    for (state, mode, expected_digits) in [
        (SkinDrawState { gauge: 7.5, gauge_max: 120.0, ..Default::default() }, "percent", 1),
        (SkinDrawState { gauge: 78.75, gauge_max: 120.0, ..Default::default() }, "percent", 2),
        (SkinDrawState { gauge: 120.0, gauge_max: 120.0, ..Default::default() }, "percent", 3),
        (SkinDrawState { gauge: 7.5, gauge_max: 120.0, ..Default::default() }, "amount", 1),
        (SkinDrawState { gauge: 78.75, gauge_max: 120.0, ..Default::default() }, "amount", 2),
        (SkinDrawState { gauge: 120.0, gauge_max: 120.0, ..Default::default() }, "amount", 3),
    ] {
        let visible = (1..=3)
            .filter(|digits| {
                eval_skin_draw_condition(&format!("gauge_value_digits({mode},{digits})"), &state)
            })
            .collect::<Vec<_>>();
        assert_eq!(visible, vec![expected_digits]);
    }
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
fn skin_context_updates_user_selected_options() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "property": [
                    { "name": "Side", "def": "1P", "item": [
                        { "name": "1P", "op": 920 },
                        { "name": "2P", "op": 921 }
                    ]}
                ]
            }
            "#,
    )
    .unwrap();
    let mut context =
        SkinContext::from_manifest_and_document(default_skin_manifest(), document, []);

    assert_eq!(context.document().unwrap().enabled_options(), [920]);
    assert!(context.set_user_selected_options(vec![921]));
    assert_eq!(context.document().unwrap().enabled_options(), [921]);
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
fn skin_document_selects_hcn_body_by_state() {
    // 旧形式 HCN: [6]=hcnbody(processing) [7]=hcnactive(inactive)
    // [8]=hcndamage(回復中) [9]=hcnreactive(減衰中)
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "source": [{ "id": 1, "path": "notes.png" }],
                "image": [
                    { "id": "hb", "src": 1, "x": 10, "y": 0, "w": 10, "h": 1 },
                    { "id": "ha", "src": 1, "x": 20, "y": 0, "w": 10, "h": 1 },
                    { "id": "hd", "src": 1, "x": 30, "y": 0, "w": 10, "h": 1 },
                    { "id": "hr", "src": 1, "x": 40, "y": 0, "w": 10, "h": 1 }
                ],
                "note": {
                    "id": "notes",
                    "note": ["hb", "hb", "hb", "hb", "hb", "hb", "hb", "hb"],
                    "hcnbody": ["hb", "hb", "hb", "hb", "hb", "hb", "hb", "hb"],
                    "hcnactive": ["ha", "ha", "ha", "ha", "ha", "ha", "ha", "ha"],
                    "hcndamage": ["hd", "hd", "hd", "hd", "hd", "hd", "hd", "hd"],
                    "hcnreactive": ["hr", "hr", "hr", "hr", "hr", "hr", "hr", "hr"]
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
    let render_x = |state: LongBodyState| {
        let item = document
            .note_long_body_render_item(
                Lane::Scratch,
                KeyMode::K7,
                rect,
                LongNoteMode::Hcn,
                state,
                &SkinDrawState::default(),
                &sources,
            )
            .unwrap();
        match item {
            SkinRenderItem::Image { uv: TextureRegion { x, .. }, .. } => x,
            _ => panic!("expected image item"),
        }
    };

    assert!(approx_eq(render_x(LongBodyState::Processing), 0.1)); // hcnbody
    assert!(approx_eq(render_x(LongBodyState::Inactive), 0.2)); // hcnactive
    assert!(approx_eq(render_x(LongBodyState::HcnActive), 0.3)); // hcndamage
    assert!(approx_eq(render_x(LongBodyState::HcnDamage), 0.4)); // hcnreactive
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
fn skin_gauge_sprite_selects_exhard_nodes_and_tip_frame() {
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
                elapsed_ms: 1_000,
                gauge: 75.0,
                gauge_max: 100.0,
                gauge_border: 1.0,
                gauge_type: 4,
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
    let tip_flicker = items.iter().find_map(|item| match item {
        SkinRenderItem::Image { uv, blend: BlendMode::Normal, .. } if uv.x > 0.7 => Some(uv.x),
        _ => None,
    });
    assert!(
        tip_flicker.is_some(),
        "EX-HARD flickering tip should use node index 28+ (normal blend overlay)"
    );
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
fn select_skin_document_renders_songlist_rows() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [
                    { "id": 1, "path": "bar.png" },
                    { "id": 2, "path": "num.png" },
                    { "id": 3, "path": "lamp.png" },
                    { "id": 4, "path": "graph.png" }
                ],
                "image": [
                    { "id": "bar-song", "src": 1, "x": 0, "y": 0, "w": 40, "h": 10 },
                    { "id": "bar-folder", "src": 1, "x": 0, "y": 10, "w": 40, "h": 10 },
                    { "id": "bar-table", "src": 1, "x": 0, "y": 30, "w": 40, "h": 10 },
                    { "id": "song-op-marker", "src": 1, "x": 0, "y": 20, "w": 4, "h": 4 },
                    { "id": "folder-op-marker", "src": 1, "x": 4, "y": 20, "w": 4, "h": 4 },
                    { "id": "trophy-bronze", "src": 3, "x": 0, "y": 0, "w": 4, "h": 4 },
                    { "id": "trophy-silver", "src": 3, "x": 4, "y": 0, "w": 4, "h": 4 },
                    { "id": "trophy-gold", "src": 3, "x": 8, "y": 0, "w": 4, "h": 4 },
                    { "id": "lamp-none", "src": 3, "x": 0, "y": 0, "w": 4, "h": 4 },
                    { "id": "lamp-failed", "src": 3, "x": 4, "y": 0, "w": 4, "h": 4 },
                    { "id": "lamp-assist", "src": 3, "x": 8, "y": 0, "w": 4, "h": 4 },
                    { "id": "lamp-light-assist", "src": 3, "x": 12, "y": 0, "w": 4, "h": 4 },
                    { "id": "lamp-easy", "src": 3, "x": 16, "y": 0, "w": 4, "h": 4 },
                    { "id": "lamp-normal", "src": 3, "x": 20, "y": 0, "w": 4, "h": 4 },
                    { "id": "label-ln", "src": 1, "x": 0, "y": 40, "w": 4, "h": 4 },
                    { "id": "label-random", "src": 1, "x": 4, "y": 40, "w": 4, "h": 4 },
                    { "id": "label-mine", "src": 1, "x": 8, "y": 40, "w": 4, "h": 4 }
                ],
                "imageset": [{ "id": "bar", "images": ["bar-song", "bar-folder", "bar-table"] }],
                "text": [
                    { "id": "bartext", "font": "main", "size": 10 },
                    { "id": "bartext1", "font": "folder", "size": 10 },
                    { "id": "bartext2", "font": "table", "size": 10 },
                    { "id": "bartext3", "font": "main", "size": 10 },
                    { "id": "bartext4", "font": "folder", "size": 10 }
                ],
                "value": [
                    { "id": "level-other", "src": 2, "x": 0, "y": 0, "w": 100, "h": 10, "divx": 10, "digit": 2 },
                    { "id": "level-beginner", "src": 2, "x": 0, "y": 10, "w": 100, "h": 10, "divx": 10, "digit": 2 },
                    { "id": "level-normal", "src": 2, "x": 0, "y": 20, "w": 100, "h": 10, "divx": 10, "digit": 2 }
                ],
                "graph": [{ "id": "graph-lamp", "src": 4, "x": 0, "y": 0, "w": 44, "h": 4, "divx": 11, "angle": 0, "type": -1 }],
                "songlist": {
                    "id": "songlist",
                    "center": 1,
                    "listoff": [
                        { "id": "bar", "dst": [{ "x": 10, "y": 70, "w": 40, "h": 10 }] },
                        { "id": "bar", "dst": [{ "x": 10, "y": 50, "w": 40, "h": 10 }] },
                        { "id": "bar", "dst": [{ "x": 10, "y": 30, "w": 40, "h": 10 }] }
                    ],
                    "liston": [
                        { "id": "bar", "dst": [{ "x": 12, "y": 70, "w": 40, "h": 10 }] },
                        { "id": "bar", "dst": [{ "x": 12, "y": 50, "w": 40, "h": 10 }] },
                        { "id": "bar", "dst": [{ "x": 12, "y": 30, "w": 40, "h": 10 }] }
                    ],
                    "text": [
                        { "id": "bartext", "dst": [{ "x": 1, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext", "dst": [{ "x": 2, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext", "dst": [{ "x": 5, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext", "dst": [{ "x": 6, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext4", "dst": [{ "x": 7, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext4", "dst": [{ "x": 8, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext2", "dst": [{ "x": 9, "y": 2, "w": 20, "h": 8 }] }
                    ],
                    "judgegraph": [
                        { "id": "song-op-marker", "op": [2], "dst": [{ "x": 8, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "folder-op-marker", "op": [1], "dst": [{ "x": 12, "y": 1, "w": 4, "h": 4 }] }
                    ],
                    "level": [
                        { "id": "level-other", "dst": [{ "x": 30, "y": 2, "w": 5, "h": 8 }] },
                        { "id": "level-beginner", "dst": [{ "x": 30, "y": 2, "w": 5, "h": 8 }] },
                        { "id": "level-normal", "dst": [{ "x": 30, "y": 2, "w": 5, "h": 8 }] }
                    ],
                    "trophy": [
                        { "id": "trophy-bronze", "dst": [{ "x": 35, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "trophy-silver", "dst": [{ "x": 35, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "trophy-gold", "dst": [{ "x": 35, "y": 1, "w": 4, "h": 4 }] }
                    ],
                    "label": [
                        { "id": "label-ln", "dst": [{ "x": 40, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "label-random", "dst": [{ "x": 44, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "label-mine", "dst": [{ "x": 48, "y": 1, "w": 4, "h": 4 }] }
                    ],
                    "graph": { "id": "graph-lamp", "dst": [{ "x": 5, "y": 1, "w": 20, "h": 2 }] },
                    "lamp": [
                        { "id": "lamp-none", "dst": [{ "x": 1, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-failed", "dst": [{ "x": 1, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-assist", "dst": [{ "x": 1, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-light-assist", "dst": [{ "x": 1, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-easy", "dst": [{ "x": 1, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-normal", "dst": [{ "x": 1, "y": 1, "w": 4, "h": 4 }] }
                    ],
                    "playerlamp": [
                        { "id": "lamp-none", "dst": [{ "x": 60, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-failed", "dst": [{ "x": 60, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-assist", "dst": [{ "x": 60, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-light-assist", "dst": [{ "x": 60, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-easy", "dst": [{ "x": 60, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-normal", "dst": [{ "x": 60, "y": 1, "w": 4, "h": 4 }] }
                    ]
                },
                "destination": [{ "id": "songlist" }]
            }
            "#,
        )
        .unwrap();
    let mut sources = mock_source("1", 100.0, 100.0);
    sources.extend(mock_source("2", 100.0, 100.0));
    sources.extend(mock_source("3", 24.0, 4.0));
    sources.extend(mock_source("4", 44.0, 4.0));
    let snapshot = SelectSnapshot {
        selected_index: 2,
        rows: vec![
            SelectRowSnapshot {
                index: 1,
                title: "Folder".to_string(),
                play_level: "0".to_string(),
                clear_type: "Normal".to_string(),
                folder_lamp_counts: {
                    let mut counts = [0; 11];
                    counts[5] = 1;
                    counts[6] = 1;
                    counts
                },
                is_folder: true,
                kind: SelectRowKind::Folder,
                ..SelectRowSnapshot::default()
            },
            SelectRowSnapshot {
                index: 2,
                title: "Song".to_string(),
                difficulty_name: "2".to_string(),
                play_level: "12".to_string(),
                clear_type: "Normal".to_string(),
                total_notes: 100,
                ex_score: Some(180),
                has_long_notes: true,
                has_mines: true,
                ..SelectRowSnapshot::default()
            },
            SelectRowSnapshot {
                index: 3,
                title: "Table".to_string(),
                play_level: "0".to_string(),
                is_folder: true,
                kind: SelectRowKind::TableFolder,
                ..SelectRowSnapshot::default()
            },
        ],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);

    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image { .. })));
    assert!(
        items
            .iter()
            .any(|item| matches!(item, SkinRenderItem::Text { text, .. } if text == "Song"))
    );
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Text {
                origin: Point { x, y },
                text,
                style,
                ..
            } if text == "Folder"
                && style.font_id.as_deref() == Some("folder")
                && approx_eq(*x, 0.17)
                && approx_eq(*y, 0.2))));
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item, SkinRenderItem::Text { text, .. } if text == "Folder"))
            .count(),
        1
    );
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Text {
                text,
                style,
                ..
            } if text == "Table"
                && style.font_id.as_deref() == Some("table"))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                uv: TextureRegion { y: v, .. },
                ..
            } if approx_eq(*v, 30.0 / 100.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, height },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(*x, 0.13)
                && approx_eq(*y, 0.45)
                && approx_eq(*width, 0.04)
                && approx_eq(*height, 0.04)
                && approx_eq(*u, 20.0 / 24.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, height },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(*x, 0.11)
                && approx_eq(*y, 0.25)
                && approx_eq(*width, 0.04)
                && approx_eq(*height, 0.04)
                && approx_eq(*u, 20.0 / 24.0))));
    assert!(!items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, height },
                ..
            } if approx_eq(*x, 0.72)
                && approx_eq(*y, 0.45)
                && approx_eq(*width, 0.04)
                && approx_eq(*height, 0.04))));
    assert!(!items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(*x, 0.47)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 8.0 / 24.0))));
    let course_snapshot = SelectSnapshot {
        selected_index: 4,
        rows: vec![SelectRowSnapshot {
            index: 4,
            title: "Course".to_string(),
            kind: SelectRowKind::Course,
            difficulty_name: "2".to_string(),
            play_level: "12".to_string(),
            total_notes: 100,
            ex_score: Some(200),
            achieved_trophy_names: vec!["goldmedal".to_string()],
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };
    let course_items = document.select_render_items(&sources, &course_snapshot);
    assert!(course_items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(*x, 0.47)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 8.0 / 24.0))));
    assert!(!course_items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.2)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 0.0)
                && approx_eq(*v, 20.0 / 100.0))));
    assert!(!items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, .. },
                uv: TextureRegion { width: u_width, .. },
                ..
            } if approx_eq(*x, 0.17)
                && approx_eq(*y, 0.47)
                && approx_eq(*width, 0.1)
                && approx_eq(*u_width, 0.5))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, .. },
                uv: TextureRegion { x: u, width: u_width, .. },
                ..
            } if approx_eq(*x, 0.15)
                && approx_eq(*y, 0.27)
                && approx_eq(*width, 0.1)
                && approx_eq(*u, 24.0 / 44.0)
                && approx_eq(*u_width, 4.0 / 44.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, .. },
                uv: TextureRegion { x: u, width: u_width, .. },
                ..
            } if approx_eq(*x, 0.25)
                && approx_eq(*y, 0.27)
                && approx_eq(*width, 0.1)
                && approx_eq(*u, 20.0 / 44.0)
                && approx_eq(*u_width, 4.0 / 44.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { y: u, .. },
                ..
            } if approx_eq(*x, 0.47)
                && approx_eq(*y, 0.4)
                && approx_eq(*u, 0.2))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.2)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 0.0)
                && approx_eq(*v, 20.0 / 100.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.52)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 0.0)
                && approx_eq(*v, 40.0 / 100.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.60)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 8.0 / 100.0)
                && approx_eq(*v, 40.0 / 100.0))));
    let scrolling_snapshot =
        SelectSnapshot { bar_scroll_direction: 1, bar_scroll_progress: 0.5, ..snapshot.clone() };
    let scrolling_items = document.select_render_items(&sources, &scrolling_snapshot);
    assert!(scrolling_items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, height },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.11)
                && approx_eq(*y, 0.5)
                && approx_eq(*width, 0.4)
                && approx_eq(*height, 0.1)
                && approx_eq(*u, 0.0)
                && approx_eq(*v, 0.0))));
    assert!(!items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.22)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 4.0 / 100.0)
                && approx_eq(*v, 20.0 / 100.0))));

    let folder_selected = SelectSnapshot { selected_index: 1, ..snapshot };
    let items = document.select_render_items(&sources, &folder_selected);
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.18)
                && approx_eq(*y, 0.65)
                && approx_eq(*u, 0.0)
                && approx_eq(*v, 20.0 / 100.0))));
    assert!(!items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.22)
                && approx_eq(*y, 0.65)
                && approx_eq(*u, 4.0 / 100.0)
                && approx_eq(*v, 20.0 / 100.0))));

    let wrapped_snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![
            SelectRowSnapshot {
                index: 2,
                title: "Last".to_string(),
                play_level: "2".to_string(),
                ..SelectRowSnapshot::default()
            },
            SelectRowSnapshot {
                index: 0,
                title: "First".to_string(),
                play_level: "1".to_string(),
                ..SelectRowSnapshot::default()
            },
            SelectRowSnapshot {
                index: 1,
                title: "Second".to_string(),
                play_level: "2".to_string(),
                ..SelectRowSnapshot::default()
            },
        ],
        ..SelectSnapshot::default()
    };
    let items = document.select_render_items(&sources, &wrapped_snapshot);
    assert!(
        items
            .iter()
            .any(|item| matches!(item, SkinRenderItem::Text { text, .. } if text == "Last"))
    );
    assert!(
        items
            .iter()
            .any(|item| matches!(item, SkinRenderItem::Text { text, .. } if text == "First"))
    );
    assert!(
        items
            .iter()
            .any(|item| matches!(item, SkinRenderItem::Text { text, .. } if text == "Second"))
    );
}

#[test]
fn select_folder_distribution_graph_uses_cycle_animation_row() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "graph.png" }],
                "graph": [
                    { "id": "graph-lamp", "src": 1, "x": 0, "y": 0, "w": 44, "h": 8, "divx": 11, "divy": 2, "cycle": 100, "type": -1 }
                ],
                "songlist": {
                    "id": "songlist",
                    "center": 0,
                    "liston": [{ "id": "row", "dst": [{ "x": 10, "y": 40, "w": 80, "h": 20 }] }],
                    "graph": { "id": "graph-lamp", "dst": [{ "x": 0, "y": 0, "w": 44, "h": 4 }] }
                },
                "destination": [{ "id": "songlist" }]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 44.0, 8.0);
    let snapshot = SelectSnapshot {
        time: TimeUs(50_000),
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            is_folder: true,
            kind: SelectRowKind::Folder,
            folder_lamp_counts: {
                let mut counts = [0; 11];
                counts[5] = 1;
                counts[6] = 1;
                counts
            },
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);
    let graph_items: Vec<&SkinRenderItem> = items
        .iter()
        .filter(|item| {
            matches!(
                item,
                SkinRenderItem::Image {
                    texture: SkinTextureId(9999),
                    rect: Rect { y, height, .. },
                    ..
                } if approx_eq(*y, 0.56) && approx_eq(*height, 0.04)
            )
        })
        .collect();

    assert_eq!(graph_items.len(), 2);
    assert!(graph_items.iter().all(|item| matches!(
        item,
        SkinRenderItem::Image {
            uv: TextureRegion { y, height, .. },
            ..
        } if approx_eq(*y, 0.5) && approx_eq(*height, 0.5)
    )));
    assert!(matches!(
        graph_items[0],
        SkinRenderItem::Image {
            rect: Rect { x, width, .. },
            uv: TextureRegion { x: uv_x, .. },
            ..
        } if approx_eq(*x, 0.10) && approx_eq(*width, 0.22) && approx_eq(*uv_x, 24.0 / 44.0)
    ));
    assert!(matches!(
        graph_items[1],
        SkinRenderItem::Image {
            rect: Rect { x, width, .. },
            uv: TextureRegion { x: uv_x, .. },
            ..
        } if approx_eq(*x, 0.32) && approx_eq(*width, 0.22) && approx_eq(*uv_x, 20.0 / 44.0)
    ));
}

#[test]
fn select_songlist_judgegraph_renders_chart_distribution() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "judgegraph": [{ "id": "density", "delay": 0, "noGap": 1, "noGapX": 1 }],
                "songlist": {
                    "id": "songlist",
                    "center": 0,
                    "liston": [{ "id": "row", "dst": [{ "x": 10, "y": 40, "w": 80, "h": 20 }] }],
                    "listoff": [{ "id": "row", "dst": [{ "x": 10, "y": 40, "w": 80, "h": 20 }] }],
                    "judgegraph": [{ "id": "density", "dst": [{ "x": 0, "y": 0, "w": 40, "h": 10 }] }]
                },
                "destination": [{ "id": "songlist" }]
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
                crate::scene::SelectChartDistributionSecond {
                    key_taps: 4,
                    mines: 1,
                    ..Default::default()
                },
                crate::scene::SelectChartDistributionSecond {
                    scratch_taps: 2,
                    key_long_bodies: 3,
                    ..Default::default()
                },
            ],
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let sources = HashMap::new();
    let items = document.select_render_items(&sources, &snapshot);
    let rect_count =
        items.iter().filter(|item| matches!(item, SkinRenderItem::Rect { .. })).count();

    assert_eq!(rect_count, 7);
}

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
fn select_draw_state_uses_select_judge_timing_offset() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 5 }"#).unwrap();
    let snapshot = SelectSnapshot {
        option_panel: 3,
        judge_timing_offset_ms: -12,
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
fn result_destination_negative_image_id_renders_runtime_stagefile_source() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 7,
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
        result_failed: Some(false),
        ..SkinDrawState::default()
    };

    let items = context.static_document_items_for_result_state_and_text(
        &Arc::new(crate::snapshot::ResultGraphSnapshot::default()),
        &state,
        &SkinTextState::default(),
    );

    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image {
            texture,
            source_size: Some(SkinImageSize { width: 400.0, height: 200.0 }),
            ..
        } if *texture == SkinTextureId(SELECT_STAGE_TEXTURE.0)
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
fn result_judgegraphs_render_beatoraja_judge_and_early_late_series() {
    let mut document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 7,
                "w": 100,
                "h": 100,
                "judgegraph": [
                    { "id": "judge", "type": 1, "backTexOff": 1, "noGap": 1, "noGapX": 1 },
                    { "id": "fs", "type": 2, "backTexOff": 1, "noGap": 1, "noGapX": 1 }
                ],
                "destination": [
                    { "id": "judge", "dst": [{ "x": 0, "y": 0, "w": 50, "h": 20, "a": 255 }] },
                    { "id": "fs", "dst": [{ "x": 0, "y": 20, "w": 50, "h": 20, "a": 255 }] }
                ]
            }
            "#,
    )
    .unwrap();
    document.result_judge_graph_buckets =
        vec![crate::snapshot::ResultJudgeGraphBucket { values: [0, 0, 1, 0, 0, 0] }];
    document.result_early_late_graph_buckets = vec![crate::snapshot::ResultEarlyLateGraphBucket {
        values: [0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
    }];

    let items = document.static_image_render_items(&HashMap::new(), &SkinDrawState::default());

    assert!(items.iter().any(|item| {
        skin_render_item_has_rect_color(item, |Color { r, g, b, .. }| {
            approx_eq(*r, 0.0) && approx_eq(*g, 1.0) && approx_eq(*b, 0.53)
        })
    }));
    assert!(items.iter().any(|item| {
        skin_render_item_has_rect_color(item, |Color { r, g, b, .. }| {
            approx_eq(*r, 1.0) && approx_eq(*g, 0.53) && approx_eq(*b, 0.0)
        })
    }));
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

#[test]
fn select_click_hit_resolves_image_act_event() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "button.png" }],
                "image": [
                    { "id": "button_play", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10, "act": 15, "click": 2 }
                ],
                "destination": [
                    { "id": "button_play", "dst": [{ "x": 10, "y": 20, "w": 30, "h": 10 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    let snapshot = match crate::sample::sample_select_scene() {
        crate::scene::AppSceneSnapshot::Select(snapshot) => snapshot,
        _ => unreachable!(),
    };

    let hit = document
        .select_click_hit(
            &sources,
            &snapshot,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            0.2,
            0.75,
        )
        .unwrap();

    assert_eq!(hit.target, SkinClickTarget::Event { event_id: 15, click: 2 });
    assert_eq!(hit.rect, Rect { x: 0.1, y: 0.7, width: 0.3, height: 0.1 });
}

#[test]
fn result_click_hit_uses_runtime_panel_visibility() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 7,
                "w": 100,
                "h": 100,
                "image": [
                    { "id": "graph", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10, "act": -10002 },
                    { "id": "ir", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10, "act": -10001 },
                    { "id": "favorite", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10, "divy": 3, "ref": 90, "act": 90 }
                ],
                "destination": [
                    { "id": "graph", "draw": "result_panel(1)", "dst": [{ "x": 10, "y": 20, "w": 30, "h": 10 }] },
                    { "id": "ir", "draw": "result_panel(2)", "dst": [{ "x": 50, "y": 20, "w": 30, "h": 10 }] },
                    { "id": "favorite", "dst": [{ "x": 10, "y": 40, "w": 30, "h": 10 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let ir_panel = SkinDrawState { result_panel: Some(1), ..SkinDrawState::default() };
    let graph_hit = document.result_click_hit(&ir_panel, 0.2, 0.75).unwrap();
    assert_eq!(
        graph_hit.target,
        SkinClickTarget::Event { event_id: SKIN_EVENT_RESULT_PANEL_GRAPH, click: 0 }
    );
    assert!(document.result_click_hit(&ir_panel, 0.65, 0.75).is_none());

    let graph_panel = SkinDrawState {
        result_panel: Some(2),
        result_favorite_chart: Some(false),
        ..SkinDrawState::default()
    };
    let ir_hit = document.result_click_hit(&graph_panel, 0.65, 0.75).unwrap();
    assert_eq!(
        ir_hit.target,
        SkinClickTarget::Event { event_id: SKIN_EVENT_RESULT_PANEL_IR, click: 0 }
    );
    assert!(document.result_click_hit(&graph_panel, 0.2, 0.75).is_none());

    let favorite_hit = document.result_click_hit(&graph_panel, 0.2, 0.55).unwrap();
    assert_eq!(favorite_hit.target, SkinClickTarget::Event { event_id: 90, click: 0 });
}

#[test]
fn select_mouse_rect_gates_render_and_click_hits() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "button.png" }],
                "image": [
                    { "id": "button", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10, "act": 15 }
                ],
                "destination": [
                    {
                        "id": "button",
                        "dst": [{ "x": 10, "y": 20, "w": 30, "h": 10 }],
                        "mouseRect": { "x": 5, "y": 2, "w": 10, "h": 4 }
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    let inside = SelectSnapshot { mouse_position: Some((0.16, 0.75)), ..SelectSnapshot::default() };
    let outside =
        SelectSnapshot { mouse_position: Some((0.01, 0.01)), ..SelectSnapshot::default() };

    assert!(document.select_render_items(&sources, &inside).iter().any(|item| {
        matches!(item, SkinRenderItem::Image { texture: SkinTextureId(9999), .. })
    }));
    assert!(!document.select_render_items(&sources, &outside).iter().any(|item| {
        matches!(item, SkinRenderItem::Image { texture: SkinTextureId(9999), .. })
    }));

    assert!(
        document
            .select_click_hit(
                &sources,
                &inside,
                &crate::select_settings_dest::SelectSettingsDestIndex::default(),
                0.2,
                0.75,
            )
            .is_some()
    );
    assert!(
        document
            .select_click_hit(
                &sources,
                &outside,
                &crate::select_settings_dest::SelectSettingsDestIndex::default(),
                0.2,
                0.75,
            )
            .is_none()
    );
}

#[test]
fn select_slider_hit_resolves_changeable_volume_slider() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "slider": [
                    { "id": "master", "src": 1, "x": 0, "y": 0, "w": 10, "h": 5, "angle": 1, "range": 50, "type": 17 }
                ],
                "destination": [
                    { "id": "master", "dst": [{ "x": 10, "y": 20, "w": 10, "h": 5 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let snapshot = SelectSnapshot::default();

    // angle=1 destination x=10 range=50 → value 0.5 at skin x=35 (norm x=0.35)
    let hit = document
        .select_slider_hit(
            &snapshot,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            0.35,
            0.775,
        )
        .unwrap();

    assert_eq!(hit.slider_type, 17);
    assert!(approx_eq(hit.value, 0.5));
    assert!(
        document
            .select_slider_hit(
                &snapshot,
                &crate::select_settings_dest::SelectSettingsDestIndex::default(),
                0.70,
                0.775,
            )
            .is_none()
    );
}

#[test]
fn result_ir_slider_hit_and_rate_use_ranking_scroll_position() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 7,
                "w": 100,
                "h": 100,
                "slider": [
                    { "id": "ir-scroll", "src": 1, "x": 0, "y": 0, "w": 10, "h": 5, "angle": 2, "range": 50, "type": 8, "changeable": true }
                ],
                "destination": [
                    { "id": "ir-scroll", "draw": "result_panel(1)", "dst": [{ "x": 10, "y": 70, "w": 10, "h": 5 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let state = SkinDrawState {
        result_panel: Some(1),
        ir_ranking: crate::scene::ResultIrSnapshot {
            scroll_offset: 5,
            scroll_max: 10,
            ..Default::default()
        },
        ..Default::default()
    };

    assert!(approx_eq(skin_state_float_number(8, &state).unwrap(), 0.5));
    assert!(approx_eq(skin_slider_progress_by_type(8, &state).unwrap(), 0.5));
    // angle=2 destination y=70 range=50 → value 0.5 at skin y=45 (norm y=0.55)
    let hit = document.result_slider_hit(&state, 0.15, 0.55).unwrap();
    assert_eq!(hit.slider_type, 8);
    assert!(approx_eq(hit.value, 0.5));
}

#[test]
fn select_slider_hit_resolves_song_scroll_slider() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "slider": [
                    { "id": "song-scroll", "src": 1, "x": 0, "y": 0, "w": 10, "h": 5, "angle": 2, "range": 50, "type": 1 }
                ],
                "destination": [
                    { "id": "song-scroll", "dst": [{ "x": 10, "y": 70, "w": 10, "h": 5 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let snapshot = SelectSnapshot::default();

    // beatoraja: value=(region.y - mouse_y)/range. Mid = skin y 45 → norm 0.55.
    let hit = document
        .select_slider_hit(
            &snapshot,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            0.15,
            0.55,
        )
        .unwrap();

    assert_eq!(hit.slider_type, 1);
    assert!(approx_eq(hit.value, 0.5));
    // Top of track (value 0) is destination y itself → skin y 70 → norm 0.30.
    let top_hit = document
        .select_slider_hit(
            &snapshot,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            0.15,
            0.30,
        )
        .unwrap();
    assert_eq!(top_hit.slider_type, 1);
    assert!(approx_eq(top_hit.value, 0.0));
}

#[test]
fn select_slider_hit_matches_mz_select_songlist_scroll_collision() {
    // mz-select default_songlistscroll2 collision:
    // parts_position=(1888,270), dst x=1864 y=790 w=64 h=64, angle=2 range=500
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 1920,
                "h": 1080,
                "slider": [
                    {
                        "id": "default_songlistscroll2_collision",
                        "src": 1,
                        "x": 80,
                        "y": 0,
                        "w": 64,
                        "h": 64,
                        "angle": 2,
                        "range": 500,
                        "type": 1
                    }
                ],
                "destination": [
                    {
                        "id": "default_songlistscroll2_collision",
                        "dst": [{ "x": 1864, "y": 790, "w": 64, "h": 64 }]
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let snapshot = SelectSnapshot::default();
    let settings = crate::select_settings_dest::SelectSettingsDestIndex::default();
    let x = (1864.0 + 32.0) / 1920.0;

    let top = document.select_slider_hit(&snapshot, &settings, x, 1.0 - 790.0 / 1080.0).unwrap();
    assert_eq!(top.slider_type, 1);
    assert!(approx_eq(top.value, 0.0));

    let mid = document.select_slider_hit(&snapshot, &settings, x, 1.0 - 540.0 / 1080.0).unwrap();
    assert_eq!(mid.slider_type, 1);
    assert!(approx_eq(mid.value, 0.5));

    let bottom = document.select_slider_hit(&snapshot, &settings, x, 1.0 - 290.0 / 1080.0).unwrap();
    assert_eq!(bottom.slider_type, 1);
    assert!(approx_eq(bottom.value, 1.0));

    // Clicks above destination y must miss (beatoraja uses region.y as the upper edge).
    assert!(document.select_slider_hit(&snapshot, &settings, x, 1.0 - 822.0 / 1080.0).is_none());
}

#[test]
fn select_click_hit_resolves_clickable_songlist_row() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "songlist": {
                    "id": "songlist",
                    "center": 0,
                    "clickable": [0],
                    "liston": [
                        { "id": "bar", "dst": [{ "x": 0, "y": 0, "w": 50, "h": 10 }] }
                    ],
                    "listoff": [
                        { "id": "bar", "dst": [{ "x": 50, "y": 0, "w": 50, "h": 10 }] }
                    ]
                },
                "destination": [{ "id": "songlist" }]
            }
            "#,
    )
    .unwrap();
    let snapshot = match crate::sample::sample_select_scene() {
        crate::scene::AppSceneSnapshot::Select(snapshot) => snapshot,
        _ => unreachable!(),
    };

    let hit = document
        .select_click_hit(
            &HashMap::new(),
            &snapshot,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            0.25,
            0.95,
        )
        .unwrap();

    assert_eq!(hit.target, SkinClickTarget::SelectRow { row_index: 0 });
    assert_eq!(hit.rect, Rect { x: 0.0, y: 0.9, width: 0.5, height: 0.1 });
    assert!(
        document
            .select_click_hit(
                &HashMap::new(),
                &snapshot,
                &crate::select_settings_dest::SelectSettingsDestIndex::default(),
                0.75,
                0.95,
            )
            .is_none()
    );
}

#[test]
fn select_skin_document_advances_dynamic_timers() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "marker.png" }],
                "image": [{ "id": "marker", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "marker", "timer": 9001, "dst": [{ "x": 10, "y": 10, "w": 10, "h": 10 }] }
                ],
                "dynamicTimer": [{ "id": 9001, "observe": "number(300) > 0" }]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    let snapshot = SelectSnapshot {
        time: TimeUs(100_000),
        chart_count: 1,
        rows: vec![SelectRowSnapshot {
            index: 0,
            is_folder: true,
            kind: SelectRowKind::Folder,
            folder_lamp_counts: [1; 11],
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    assert!(document.select_render_items(&sources, &snapshot).is_empty());

    let mut runtime = DynamicTimerRuntime::default();
    let items = document.select_render_items_with_dynamic_timers(
        &sources,
        &snapshot,
        Some(&mut runtime),
        &crate::select_settings_dest::SelectSettingsDestIndex::default(),
        None,
    );

    assert_eq!(items.len(), 1);
    assert!(matches!(items[0], SkinRenderItem::Image { .. }));
}

#[test]
fn select_skin_document_renders_unowned_song_with_nograde_bar() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "bar.png" }],
                "image": [
                    { "id": "bar-song", "src": 1, "x": 0, "y": 0, "w": 40, "h": 10 },
                    { "id": "bar-nograde", "src": 1, "x": 0, "y": 40, "w": 40, "h": 10 }
                ],
                "imageset": [{
                    "id": "bar",
                    "images": ["bar-song", "bar-song", "bar-song", "bar-song", "bar-nograde"]
                }],
                "text": [
                    { "id": "bartext-owned", "font": "main", "size": 10 },
                    { "id": "bartext-owned2", "font": "main", "size": 10 },
                    { "id": "bartext-owned3", "font": "main", "size": 10 },
                    { "id": "bartext-owned4", "font": "main", "size": 10 },
                    { "id": "bartext-owned5", "font": "main", "size": 10 },
                    { "id": "bartext-owned6", "font": "main", "size": 10 },
                    { "id": "bartext-owned7", "font": "main", "size": 10 },
                    { "id": "bartext-owned8", "font": "main", "size": 10 },
                    { "id": "bartext-unowned", "font": "unowned", "size": 10 }
                ],
                "songlist": {
                    "id": "songlist",
                    "center": 0,
                    "listoff": [{ "id": "bar", "dst": [{ "x": 10, "y": 50, "w": 40, "h": 10 }] }],
                    "liston": [{ "id": "bar", "dst": [{ "x": 12, "y": 50, "w": 40, "h": 10 }] }],
                    "text": [
                        { "id": "bartext-owned", "dst": [{ "x": 1, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned2", "dst": [{ "x": 2, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned3", "dst": [{ "x": 3, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned4", "dst": [{ "x": 4, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned5", "dst": [{ "x": 5, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned6", "dst": [{ "x": 6, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned7", "dst": [{ "x": 7, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned8", "dst": [{ "x": 8, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-unowned", "dst": [{ "x": 9, "y": 2, "w": 20, "h": 8 }] }
                    ]
                },
                "destination": [{ "id": "songlist" }]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    let snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            title: "Missing Song".to_string(),
            in_library: false,
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);

    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                uv: TextureRegion { y: v, .. },
                ..
            } if approx_eq(*v, 40.0 / 100.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Text {
                text,
                style,
                ..
            } if text == "Missing Song" && style.font_id.as_deref() == Some("unowned"))));
}

#[test]
fn select_skin_uses_snapshot_time_and_bar_type_ops() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "panel.png" }],
                "image": [
                    { "id": "song-panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "folder-panel", "src": 1, "x": 10, "y": 0, "w": 10, "h": 10 }
                ],
                "destination": [
                    { "id": "song-panel", "timer": 11, "loop": 200, "op": [2], "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 10, "h": 10 },
                        { "time": 200, "x": 20 }
                    ] },
                    { "id": "folder-panel", "op": [1], "dst": [
                        { "x": 50, "y": 0, "w": 10, "h": 10 }
                    ] },
                    { "id": "song-panel", "timer": 21, "op": [21], "dst": [
                        { "time": 0, "x": 30, "y": 0, "w": 10, "h": 10 },
                        { "time": 200, "x": 50 }
                    ] }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    let snapshot = SelectSnapshot {
        time: bmz_core::time::TimeUs(100_000),
        selection_time: bmz_core::time::TimeUs(100_000),
        option_panel_time: bmz_core::time::TimeUs(100_000),
        option_panel: 1,
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            title: "Song".to_string(),
            is_folder: false,
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);

    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                rect: Rect { x, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(*x, 0.1) && approx_eq(*u, 0.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                rect: Rect { x, .. },
                ..
            } if approx_eq(*x, 0.4))));
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

/// Starseeker 閉店の `black` 相当: `src = "bg"` を `system` に解決し、timer 3 で暗転フェード。
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
fn skin_state_number_maps_operating_time_refs() {
    let state = SkinDrawState { operating_time_ms: 90_061_234, ..SkinDrawState::default() };

    assert_eq!(skin_state_number(27, &state), Some(25));
    assert_eq!(skin_state_number(28, &state), Some(1));
    assert_eq!(skin_state_number(29, &state), Some(1));
}

#[test]
fn skin_state_number_maps_player_statistics_refs() {
    let state = SkinDrawState {
        total_notes: 99,
        select_total_notes: 100,
        select_screen: true,
        select_play_count: 42,
        select_clear_count: 31,
        player_stats: PlayerStatsSnapshot {
            play_count: 10,
            clear_count: 7,
            playtime_seconds: 3_661,
            max_combo: 999,
            fast_pgreat: 2,
            slow_pgreat: 3,
            fast_great: 4,
            slow_great: 5,
            fast_good: 6,
            slow_good: 7,
            fast_bad: 8,
            slow_bad: 9,
            fast_poor: 10,
            slow_poor: 11,
            fast_empty_poor: 12,
            slow_empty_poor: 13,
            daily: Default::default(),
        },
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(17, &state), Some(1));
    assert_eq!(skin_state_number(18, &state), Some(1));
    assert_eq!(skin_state_number(19, &state), Some(1));
    assert_eq!(skin_state_number(30, &state), Some(10));
    assert_eq!(skin_state_number(31, &state), Some(7));
    assert_eq!(skin_state_number(32, &state), Some(3));
    assert_eq!(skin_state_number(33, &state), Some(5));
    assert_eq!(skin_state_number(34, &state), Some(9));
    assert_eq!(skin_state_number(35, &state), Some(13));
    assert_eq!(skin_state_number(36, &state), Some(17));
    assert_eq!(skin_state_number(37, &state), Some(21));
    assert_eq!(skin_state_number(333, &state), Some(44));
    assert_eq!(skin_state_number(77, &state), Some(42));
    assert_eq!(skin_state_number(78, &state), Some(31));
}

#[test]
fn compatible_daily_statistics_use_skin_specific_note_definitions() {
    let daily = DailyPlayerStatsSnapshot {
        play_count: 5,
        clear_count: 4,
        pgreat: 50,
        great: 25,
        good: 10,
        bad: 5,
        poor: 3,
        empty_poor: 2,
        score_update_count: 3,
        clear_update_count: 2,
        miss_count_update_count: 1,
        recent_titles: std::array::from_fn(|index| format!("Recent {}", index + 1)),
    };
    let state = SkinDrawState {
        player_stats: PlayerStatsSnapshot { daily, ..PlayerStatsSnapshot::default() },
        ..SkinDrawState::default()
    };

    let million_value = SkinValueDef {
        id: "Number_Todayplayednotes".to_string(),
        value_expr: "1".to_string(),
        ..SkinValueDef::default()
    };
    assert_eq!(skin_value_number(&million_value, &state), Some(95));
    assert_eq!(skin_state_number(1930, &state), Some(5));
    assert_eq!(skin_state_number(1938, &state), Some(90));
    assert_eq!(skin_state_number(1939, &state), Some(95));
    assert_eq!(skin_state_number(1940, &state), Some(125));
    assert_eq!(skin_state_number(1941, &state), Some(180));
    assert_eq!(skin_state_number(1942, &state), Some(6944));
    assert_eq!(skin_state_number(1943, &state), Some(2));
    assert_eq!(skin_state_number(1944, &state), Some(3));
    assert_eq!(skin_state_number(1945, &state), Some(2));
    assert_eq!(skin_state_number(1946, &state), Some(1));

    let text = |id: &str| SkinTextDef { id: id.to_string(), ..SkinTextDef::default() };
    let text_state = SkinTextState::default();
    assert_eq!(
        skin_state_text_with_draw_state(
            &text("defaultNotesProcessingCounter_notes"),
            Some(&state),
            &text_state,
        ),
        "90"
    );
    assert_eq!(
        skin_state_text_with_draw_state(
            &text("defaultNotesProcessingCounter_pg"),
            Some(&state),
            &text_state,
        ),
        "50  (55.56%)"
    );
    assert_eq!(
        skin_state_text_with_draw_state(
            &text("defaultNotesProcessingCounter_cp"),
            Some(&state),
            &text_state,
        ),
        "4/5"
    );
    assert_eq!(
        skin_state_text_with_draw_state(
            &text("defaultNotesProcessingCounter_rank"),
            Some(&state),
            &text_state,
        ),
        "A"
    );
    assert_eq!(
        skin_state_text_with_draw_state(
            &text("defaultNotesProcessingCounter_rate"),
            Some(&state),
            &text_state,
        ),
        "69.44"
    );
    let generic_rank = SkinTextDef { ref_id: 1943, ..Default::default() };
    let generic_recent = SkinTextDef { ref_id: 1950, ..Default::default() };
    assert_eq!(skin_state_text_with_draw_state(&generic_rank, Some(&state), &text_state), "A");
    assert_eq!(
        skin_state_text_with_draw_state(&generic_recent, Some(&state), &text_state),
        "Recent 1"
    );
}

#[test]
fn course_stage_result_refs_map_fixed_slots() {
    let mut course_result = CourseResultSkinSnapshot { stage_count: 2, ..Default::default() };
    course_result.stages[1] = crate::scene::CourseStageResultSkinSnapshot {
        ex_score: 200,
        gauge: 42.9,
        bp: 17,
        rate_basis_points: 8_333,
    };
    let state = SkinDrawState { course_result, ..Default::default() };

    assert_eq!(skin_state_number(SKIN_REF_BMZ_COURSE_STAGE_COUNT, &state), Some(2));
    assert_eq!(skin_state_number(SKIN_REF_BMZ_COURSE_STAGE_EX_BASE + 1, &state), Some(200));
    assert_eq!(skin_state_number(SKIN_REF_BMZ_COURSE_STAGE_GAUGE_BASE + 1, &state), Some(42));
    assert_eq!(skin_state_number(SKIN_REF_BMZ_COURSE_STAGE_BP_BASE + 1, &state), Some(17));
    assert_eq!(skin_state_number(SKIN_REF_BMZ_COURSE_STAGE_RATE_BASE + 1, &state), Some(8_333));
}

#[test]
fn select_folder_hides_song_score_numbers() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        select_in_library: true,
        ex_score: 1234,
        total_notes: 1000,
        select_total_notes: 1000,
        select_play_count: 7,
        select_clear_count: 3,
        select_bp: Some(12),
        select_cb: Some(8),
        judge_counts: DisplayJudgeCounts {
            pgreat: 20,
            great: 30,
            good: 10,
            bad: 5,
            poor: 2,
            empty_poor: 1,
        },
        fast_slow_counts: Some(crate::snapshot::FastSlowJudgeCounts {
            fast_pgreat: 7,
            slow_empty_poor: 2,
            ..Default::default()
        }),
        ..SkinDrawState::default()
    };

    for ref_id in [71, 74, 76, 77, 78, 80, 85, 102, 110, 154, 410, 420, 426] {
        assert_eq!(skin_state_number(ref_id, &state), None, "ref {ref_id}");
    }
    assert_eq!(skin_state_number(30, &state), Some(0));
    assert_eq!(skin_state_number(33, &state), Some(0));
}

#[test]
fn select_course_exposes_score_numbers() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Course,
        select_in_library: true,
        ex_score: 1234,
        max_combo: 345,
        total_notes: 1000,
        select_total_notes: 1000,
        select_play_count: 42,
        select_clear_count: 31,
        select_bp: Some(12),
        select_cb: Some(8),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(71, &state), Some(1234));
    assert_eq!(skin_state_number(74, &state), Some(1000));
    assert_eq!(skin_state_number(75, &state), Some(345));
    assert_eq!(skin_state_number(76, &state), Some(12));
    assert_eq!(skin_state_number(77, &state), Some(42));
    assert_eq!(skin_state_number(78, &state), Some(31));
    assert_eq!(skin_state_number(425, &state), Some(8));
    assert_eq!(skin_state_number(427, &state), Some(8));
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
fn display_number_digits_uses_absolute_value_like_beatoraja_skin_number() {
    assert_eq!(display_number_digits(-34, 2, NumberPadding::Zero), vec![3, 4]);
    assert_eq!(display_number_digits(-34, 4, NumberPadding::Blank), vec![10, 10, 3, 4]);
}

#[test]
fn skin_state_number_maps_result_value_refs() {
    let fast_slow = crate::snapshot::FastSlowJudgeCounts {
        fast_pgreat: 350,
        slow_pgreat: 427,
        fast_great: 180,
        slow_great: 154,
        fast_good: 12,
        slow_good: 10,
        fast_bad: 2,
        slow_bad: 1,
        fast_poor: 3,
        slow_poor: 2,
        fast_empty_poor: 5,
        slow_empty_poor: 4,
    };
    let state = SkinDrawState {
        ex_score: 1888,
        max_combo: 777,
        total_notes: 1000,
        past_notes: 1000,
        judge_counts: DisplayJudgeCounts {
            pgreat: 777,
            great: 334,
            good: 22,
            bad: 3,
            poor: 5,
            empty_poor: 9,
        },
        fast_slow_counts: Some(fast_slow),
        best_ex_score: Some(1700),
        best_clear_index: Some(6),
        target_ex_score: Some(1900),
        best_max_combo: Some(800),
        target_max_combo: Some(1000),
        best_bp: Some(20),
        previous_best_ex_score: Some(1800),
        previous_best_clear_index: Some(4),
        previous_best_max_combo: Some(700),
        previous_best_bp: Some(10),
        target_bp: Some(0),
        target_clear_index: Some(8),
        select_clear_index: 5,
        result_failed: Some(false),
        result_arrange_index: 9,
        result_arrange_2p_index: 1,
        average_timing_ms: Some(-12.34),
        average_duration_us: Some(345_670),
        stddev_timing_ms: Some(56.78),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(42, &state), Some(9));
    assert_eq!(skin_state_number(43, &state), Some(1));
    // 符号付き差分
    assert_eq!(skin_state_number(170, &state), Some(1800));
    assert_eq!(skin_state_number(121, &state), Some(1900));
    assert_eq!(skin_state_number(151, &state), Some(1900));
    assert_eq!(skin_state_number(122, &state), Some(95));
    assert_eq!(skin_state_number(123, &state), Some(0));
    assert_eq!(skin_state_number(135, &state), Some(95));
    assert_eq!(skin_state_number(136, &state), Some(0));
    assert_eq!(skin_state_number(157, &state), Some(95));
    assert_eq!(skin_state_number(158, &state), Some(0));
    assert_eq!(skin_state_number(183, &state), Some(90));
    assert_eq!(skin_state_number(184, &state), Some(0));
    assert_eq!(skin_state_number(152, &state), Some(1888 - 1800));
    assert_eq!(skin_state_number(172, &state), Some(1888 - 1800));
    assert_eq!(skin_state_number(153, &state), Some(1888 - 1900));
    assert_eq!(skin_state_number(173, &state), Some(700));
    assert_eq!(skin_state_number(175, &state), Some(777 - 700));
    assert_eq!(skin_state_number(176, &state), Some(10));
    assert_eq!(skin_state_number(177, &state), Some(8));
    // 現在 bp = bad+poor = 8、MYBEST = 更新前の 10 → diff = -2
    assert_eq!(skin_state_number(178, &state), Some(-2));
    assert_eq!(skin_state_number(370, &state), Some(5));
    assert_eq!(skin_state_number(371, &state), Some(4));
    assert_eq!(skin_state_number(372, &state), Some(345));
    assert_eq!(skin_state_number(373, &state), Some(67));
    assert_eq!(skin_state_number(374, &state), Some(-12));
    assert_eq!(skin_state_number(375, &state), Some(-34));
    assert_eq!(skin_image_index_number(370, &state), Some(5));
    assert_eq!(skin_image_index_number(371, &state), Some(4));
    assert!(test_skin_op(320, &[], &state));
    assert!(!test_skin_op(321, &[], &state));
    assert!(test_skin_op(330, &[], &state));
    assert!(!test_skin_op(1330, &[], &state));
    assert!(test_skin_op(331, &[], &state));
    assert!(!test_skin_op(1331, &[], &state));
    assert!(test_skin_op(332, &[], &state));
    assert!(!test_skin_op(1332, &[], &state));
    assert!(test_skin_op(335, &[], &state));
    assert!(!test_skin_op(1335, &[], &state));
    assert!(test_skin_op(300, &[], &state));
    assert!(test_skin_op(310, &[], &state));
    assert!(!test_skin_op(301, &[], &state));
    assert!(!test_skin_op(308, &[], &state));
    assert!(test_skin_op(350, &[], &state));
    assert!(!test_skin_op(351, &[], &state));
    assert!(!test_skin_op(352, &[], &state));
    assert!(test_skin_op(353, &[], &state));
    assert!(!test_skin_op(354, &[], &state));

    let draw_state = SkinDrawState {
        ex_score: 1800,
        max_combo: 700,
        total_notes: 1000,
        judge_counts: DisplayJudgeCounts { bad: 5, poor: 5, ..DisplayJudgeCounts::default() },
        previous_best_ex_score: Some(1800),
        previous_best_max_combo: Some(700),
        previous_best_bp: Some(10),
        target_ex_score: Some(1800),
        result_failed: Some(false),
        ..SkinDrawState::default()
    };
    assert!(test_skin_op(1330, &[], &draw_state));
    assert!(test_skin_op(1331, &[], &draw_state));
    assert!(test_skin_op(1332, &[], &draw_state));
    assert!(test_skin_op(1335, &[], &draw_state));
    assert!(test_skin_op(354, &[], &draw_state));

    let failed_record_bp_state = SkinDrawState {
        judge_counts: DisplayJudgeCounts { bad: 1, poor: 2, ..DisplayJudgeCounts::default() },
        previous_best_bp: Some(10),
        result_bp: Some(100),
        result_cb: Some(80),
        result_failed: Some(true),
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(76, &failed_record_bp_state), Some(100));
    assert_eq!(skin_state_number(177, &failed_record_bp_state), Some(100));
    assert_eq!(skin_state_number(178, &failed_record_bp_state), Some(90));
    assert_eq!(skin_state_number(425, &failed_record_bp_state), Some(80));
    assert_eq!(skin_state_number(427, &failed_record_bp_state), Some(80));
    assert!(!test_skin_op(332, &[], &failed_record_bp_state));
    assert!(!test_skin_op(1332, &[], &failed_record_bp_state));

    let updated_result_state = SkinDrawState {
        ex_score: 1900,
        total_notes: 1000,
        past_notes: 1000,
        best_ex_score: Some(1900),
        previous_best_ex_score: Some(1700),
        result_failed: Some(false),
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(150, &updated_result_state), Some(1700));
    assert_eq!(skin_state_number(170, &updated_result_state), Some(1700));
    assert_eq!(skin_state_number(152, &updated_result_state), Some(200));
    assert_eq!(skin_state_number(183, &updated_result_state), Some(85));
    assert!(test_skin_op(321, &[], &updated_result_state));
    assert!(!test_skin_op(320, &[], &updated_result_state));
    assert!((graph_value(113, &updated_result_state) - 0.85).abs() < 1e-5);

    let first_play_result_state = SkinDrawState {
        ex_score: 1888,
        max_combo: 777,
        total_notes: 1000,
        past_notes: 1000,
        judge_counts: DisplayJudgeCounts { bad: 3, poor: 5, ..DisplayJudgeCounts::default() },
        best_ex_score: Some(1888),
        best_clear_index: Some(6),
        best_bp: Some(8),
        previous_best_ex_score: None,
        previous_best_clear_index: None,
        previous_best_bp: None,
        result_failed: Some(false),
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(150, &first_play_result_state), Some(0));
    assert_eq!(skin_state_number(170, &first_play_result_state), Some(0));
    assert_eq!(skin_state_number(152, &first_play_result_state), Some(1888));
    assert_eq!(skin_state_number(176, &first_play_result_state), None);
    assert_eq!(skin_state_number(178, &first_play_result_state), None);
    assert!(!test_skin_op(332, &[], &first_play_result_state));
    assert!(!test_skin_op(1332, &[], &first_play_result_state));
    assert_eq!(skin_state_number(183, &first_play_result_state), Some(0));
    assert_eq!(skin_state_number(184, &first_play_result_state), Some(0));
    assert_eq!(skin_state_number(371, &first_play_result_state), Some(0));
    assert_eq!(graph_value(113, &first_play_result_state), 0.0);
    assert!(!test_skin_op(320, &[], &first_play_result_state));

    let zero_rank_state = SkinDrawState {
        ex_score: 0,
        total_notes: 1000,
        result_failed: Some(true),
        ..SkinDrawState::default()
    };
    assert!(test_skin_op(308, &[], &zero_rank_state));
    assert!(test_skin_op(318, &[], &zero_rank_state));

    // Fast/Slow 内訳
    assert_eq!(skin_state_number(410, &state), Some(350));
    assert_eq!(skin_state_number(411, &state), Some(427));
    assert_eq!(skin_state_number(412, &state), Some(180));
    assert_eq!(skin_state_number(413, &state), Some(154));
    assert_eq!(skin_state_number(414, &state), Some(12));
    assert_eq!(skin_state_number(415, &state), Some(10));
    assert_eq!(skin_state_number(416, &state), Some(2));
    assert_eq!(skin_state_number(417, &state), Some(1));
    assert_eq!(skin_state_number(418, &state), Some(3));
    assert_eq!(skin_state_number(419, &state), Some(2));
    assert_eq!(skin_state_number(421, &state), Some(5));
    assert_eq!(skin_state_number(422, &state), Some(4));
    // TOTAL_EARLY = fast 合計 (PGREAT 除外) = 180+12+2+3+5 = 202
    assert_eq!(skin_state_number(423, &state), Some(202));
    // TOTAL_LATE = slow 合計 (PGREAT 除外) = 154+10+1+2+4 = 171
    assert_eq!(skin_state_number(424, &state), Some(171));

    // Result timing distribution
    assert_eq!(skin_state_number(374, &state), Some(-12));
    assert_eq!(skin_state_number(375, &state), Some(-34));
    assert_eq!(skin_state_number(376, &state), Some(56));
    assert_eq!(skin_state_number(377, &state), Some(78));

    // best/target が None のとき None を返す
    let bare = SkinDrawState::default();
    assert_eq!(skin_state_number(152, &bare), None);
    assert_eq!(skin_state_number(173, &bare), None);
    assert_eq!(skin_state_number(410, &bare), None);
    assert_eq!(skin_state_number(374, &bare), None);
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
fn skin_ops_map_gauge_ranges_and_result_judge_existence() {
    let play = SkinDrawState {
        play_screen: true,
        gauge: 45.0,
        gauge_max: 100.0,
        ..SkinDrawState::default()
    };
    assert!(test_skin_op(234, &[], &play));
    assert!(!test_skin_op(233, &[], &play));
    assert!(test_skin_op(240, &[], &SkinDrawState { gauge: 100.0, ..play.clone() }));
    assert!(test_skin_op(
        234,
        &[],
        &SkinDrawState { ready_timer_ms: None, play_timer_ms: None, ..play.clone() }
    ));
    assert!(!test_skin_op(234, &[], &SkinDrawState { play_screen: false, ..play.clone() }));

    let result = SkinDrawState {
        result_failed: Some(false),
        judge_counts: DisplayJudgeCounts {
            pgreat: 1,
            good: 2,
            poor: 3,
            ..DisplayJudgeCounts::default()
        },
        ..SkinDrawState::default()
    };
    assert!(test_skin_op(2241, &[], &result));
    assert!(!test_skin_op(2242, &[], &result));
    assert!(test_skin_op(2243, &[], &result));
    assert!(!test_skin_op(2244, &[], &result));
    assert!(test_skin_op(2245, &[], &result));
    assert!(!test_skin_op(2246, &[], &result));
    assert!(!test_skin_op(2241, &[], &SkinDrawState::default()));
}

#[test]
fn skin_state_number_maps_result_chart_detail_refs() {
    let state = SkinDrawState {
        result_failed: Some(false),
        now_bpm: 128.0,
        min_bpm: 100.0,
        max_bpm: 180.0,
        main_bpm: 150.0,
        total_duration_ms: 200_000,
        duration_green_ms: Some(120_000),
        select_chart_total_gauge: 200.0,
        judge_rank: Some(2),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(160, &state), Some(128));
    assert_eq!(skin_state_number(91, &state), Some(100));
    assert_eq!(skin_state_number(90, &state), Some(180));
    assert_eq!(skin_state_number(92, &state), Some(150));
    assert_eq!(skin_state_number(312, &state), Some(200_000));
    assert_eq!(skin_state_number(313, &state), Some(120_000));
    assert_eq!(skin_state_number(368, &state), Some(200));
    assert_eq!(skin_state_number(400, &state), Some(2));
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
fn result_skin_state_maps_arrange_ops() {
    let cases = [
        (0, 126),
        (1, 127),
        (2, 128),
        (3, 1128),
        (4, 129),
        (5, 1129),
        (6, 130),
        (7, 131),
        (8, 1130),
        (9, 1131),
    ];
    for (index, op) in cases {
        let state = SkinDrawState {
            result_failed: Some(false),
            result_arrange_index: index,
            ..SkinDrawState::default()
        };
        assert!(test_skin_op(op, &[], &state), "op {op} should match index {index}");
        for (_, other_op) in cases {
            if other_op != op {
                assert!(
                    !test_skin_op(other_op, &[], &state),
                    "op {other_op} should not match index {index}"
                );
            }
        }
    }

    assert!(!test_skin_op(
        1131,
        &[],
        &SkinDrawState { result_arrange_index: 9, ..SkinDrawState::default() }
    ));
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
fn result_timers_follow_result_state() {
    let inactive = SkinDrawState::default();
    assert_eq!(skin_timer_elapsed_ms(Some(150), &inactive), None);
    assert_eq!(skin_timer_elapsed_ms(Some(151), &inactive), None);
    assert_eq!(skin_timer_elapsed_ms(Some(152), &inactive), None);
    assert_eq!(skin_timer_elapsed_ms(Some(172), &inactive), None);
    assert_eq!(skin_timer_elapsed_ms(Some(173), &inactive), None);
    assert_eq!(skin_timer_elapsed_ms(Some(174), &inactive), None);

    let active = SkinDrawState {
        result_graph_begin_ms: Some(120),
        result_graph_end_ms: Some(120),
        result_update_score_ms: Some(40),
        ir_ranking: crate::scene::ResultIrSnapshot {
            connect_begin_ms: Some(180),
            connect_success_ms: Some(90),
            connect_fail_ms: Some(30),
            ..Default::default()
        },
        ..SkinDrawState::default()
    };
    assert_eq!(skin_timer_elapsed_ms(Some(150), &active), Some(120));
    assert_eq!(skin_timer_elapsed_ms(Some(151), &active), Some(120));
    assert_eq!(skin_timer_elapsed_ms(Some(152), &active), Some(40));
    assert_eq!(skin_timer_elapsed_ms(Some(172), &active), Some(180));
    assert_eq!(skin_timer_elapsed_ms(Some(173), &active), Some(90));
    assert_eq!(skin_timer_elapsed_ms(Some(174), &active), Some(30));
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
fn select_panel_on_and_off_timers_follow_each_panel_state() {
    let state = SkinDrawState {
        select_option_panel: 2,
        select_option_panel_elapsed_ms: 75,
        select_option_panel_off_elapsed_ms: [Some(120), None, Some(340), None, None, None],
        ..SkinDrawState::default()
    };

    assert_eq!(skin_timer_elapsed_ms(Some(21), &state), None);
    assert_eq!(skin_timer_elapsed_ms(Some(22), &state), Some(75));
    assert_eq!(skin_timer_elapsed_ms(Some(23), &state), None);
    assert_eq!(skin_timer_elapsed_ms(Some(31), &state), Some(120));
    assert_eq!(skin_timer_elapsed_ms(Some(32), &state), None);
    assert_eq!(skin_timer_elapsed_ms(Some(33), &state), Some(340));
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
fn ir_skin_properties_map_loaded_ranking() {
    let loaded = SkinDrawState {
        ir_ranking: crate::scene::ResultIrSnapshot {
            state: crate::scene::ResultIrState::Loaded,
            rank: Some(3),
            total_player: Some(42),
            clear_rate: Some(85),
            previous_rank: None,
            entries: [
                crate::scene::ResultIrRankingEntrySnapshot {
                    rank: Some(1),
                    ex_score: Some(2000),
                    clear_index: Some(8),
                    player_name: crate::scene::ResultIrRankingName::from_display_name("Alice"),
                },
                crate::scene::ResultIrRankingEntrySnapshot {
                    rank: Some(2),
                    ex_score: Some(1900),
                    clear_index: Some(6),
                    player_name: crate::scene::ResultIrRankingName::from_display_name("Bob"),
                },
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
        },
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(179, &loaded), Some(3));
    assert_eq!(skin_state_number(180, &loaded), Some(42));
    assert_eq!(skin_state_number(200, &loaded), Some(42));
    assert_eq!(skin_state_number(181, &loaded), Some(85));
    assert_eq!(skin_state_number(182, &loaded), None);
    assert_eq!(skin_state_number(226, &loaded), Some(36));
    assert_eq!(skin_state_number(227, &loaded), Some(85));
    assert_eq!(skin_state_number(241, &loaded), Some(0));
    assert_eq!(skin_state_number(380, &loaded), Some(2000));
    assert_eq!(skin_state_number(381, &loaded), Some(1900));
    assert_eq!(skin_state_number(390, &loaded), Some(1));
    assert_eq!(skin_state_number(391, &loaded), Some(2));
    assert_eq!(skin_image_index_number(390, &loaded), Some(8));
    assert_eq!(skin_image_index_number(391, &loaded), Some(6));
    assert_eq!(skin_state_number(382, &loaded), None);
    assert!(!test_skin_op(601, &[], &loaded));
    assert!(test_skin_op(602, &[], &loaded));
    assert!(!test_skin_op(603, &[], &loaded));
    assert!(!test_skin_op(604, &[], &loaded));

    let loading = SkinDrawState {
        ir_ranking: crate::scene::ResultIrSnapshot {
            state: crate::scene::ResultIrState::Loading,
            ..Default::default()
        },
        ..SkinDrawState::default()
    };
    assert!(test_skin_op(601, &[], &loading));
    assert!(!test_skin_op(602, &[], &loading));
    assert!(!test_skin_op(606, &[], &loading));

    let waiting = SkinDrawState {
        ir_ranking: crate::scene::ResultIrSnapshot {
            state: crate::scene::ResultIrState::Waiting,
            ..Default::default()
        },
        ..SkinDrawState::default()
    };
    assert!(test_skin_op(606, &[], &waiting));
    assert!(!test_skin_op(601, &[], &waiting));

    let failed = SkinDrawState {
        ir_ranking: crate::scene::ResultIrSnapshot {
            state: crate::scene::ResultIrState::Failed,
            ..Default::default()
        },
        ..SkinDrawState::default()
    };
    assert!(test_skin_op(604, &[], &failed));
    assert!(test_skin_op(608, &[], &failed));

    let no_player = SkinDrawState {
        ir_ranking: crate::scene::ResultIrSnapshot {
            state: crate::scene::ResultIrState::Loaded,
            total_player: Some(0),
            ..Default::default()
        },
        ..SkinDrawState::default()
    };
    assert!(test_skin_op(603, &[], &no_player));
}

#[test]
fn bmz_result_ir_scope_refs_and_options_follow_snapshot() {
    let state = SkinDrawState {
        ir_ranking: crate::scene::ResultIrSnapshot {
            scope: crate::scene::ResultIrScope::Rival,
            global_scope_supported: true,
            rival_scope_supported: true,
            total_player: Some(7),
            ..Default::default()
        },
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(SKIN_REF_BMZ_RESULT_IR_SCOPE, &state), Some(1));
    assert_eq!(skin_state_number(SKIN_REF_BMZ_RESULT_IR_SCOPE_TOTAL, &state), Some(7));
    assert!(!test_skin_op(SKIN_OPTION_BMZ_RESULT_IR_SCOPE_GLOBAL, &[], &state));
    assert!(test_skin_op(SKIN_OPTION_BMZ_RESULT_IR_SCOPE_RIVAL, &[], &state));
    assert!(test_skin_op(SKIN_OPTION_BMZ_RESULT_IR_SCOPE_GLOBAL_SUPPORTED, &[], &state));
    assert!(test_skin_op(SKIN_OPTION_BMZ_RESULT_IR_SCOPE_RIVAL_SUPPORTED, &[], &state));

    let text_state = SkinTextState::default();
    assert_eq!(
        skin_main_state_text(SKIN_REF_BMZ_RESULT_IR_SCOPE, Some(&state), &text_state),
        "RIVAL"
    );
}

#[test]
fn wmii_ir_score_graph_and_user_highlight_use_ranking_snapshot() {
    let state = SkinDrawState {
        total_notes: 100,
        ir_ranking: crate::scene::ResultIrSnapshot {
            state: crate::scene::ResultIrState::Loaded,
            user_name: crate::scene::ResultIrRankingName::from_display_name("Alice"),
            entries: [
                crate::scene::ResultIrRankingEntrySnapshot {
                    rank: Some(1),
                    ex_score: Some(155),
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
        },
        ..SkinDrawState::default()
    };

    assert_eq!(skin_builtin_value_f32("bmz:ir_score_rate:1", &state), Some(0.775));
    assert_eq!(skin_builtin_value_i64("bmz:ir_score_rate_integer:1", &state), Some(77));
    assert_eq!(skin_builtin_value_i64("bmz:ir_score_rate_fraction:1", &state), Some(50));
    assert!(eval_skin_draw_condition("ir_score_rate_band(1,6,7)", &state));
    assert!(!eval_skin_draw_condition("ir_score_rate_band(1,7,8)", &state));
    assert!(eval_skin_draw_condition("option(51) and ir_score_rate_range(1,666,777)", &state));
    assert!(!eval_skin_draw_condition("option(51) and ir_score_rate_range(1,777,888)", &state));
    assert!(eval_skin_draw_condition("ir_ranking_user(1)", &state));
    assert!(!eval_skin_draw_condition("ir_ranking_user(2)", &state));
}

#[test]
fn wmii_ir_score_diff_uses_best_of_old_and_current_score() {
    let mut entries =
        std::array::from_fn(|_| crate::scene::ResultIrRankingEntrySnapshot::default());
    entries[0] = crate::scene::ResultIrRankingEntrySnapshot {
        rank: Some(1),
        ex_score: Some(2293),
        clear_index: Some(9),
        player_name: crate::scene::ResultIrRankingName::from_display_name("Alice"),
    };
    let mut state = SkinDrawState {
        ex_score: 2284,
        total_notes: 1155,
        past_notes: 1155,
        previous_best_ex_score: Some(2293),
        result_failed: Some(false),
        ir_ranking: crate::scene::ResultIrSnapshot {
            state: crate::scene::ResultIrState::Loaded,
            entries,
            ..Default::default()
        },
        ..SkinDrawState::default()
    };

    assert_eq!(skin_builtin_value_i64("bmz:ir_score_diff:1", &state), Some(0));

    state.previous_best_ex_score = Some(2200);
    assert_eq!(skin_builtin_value_i64("bmz:ir_score_diff:1", &state), Some(-9));

    state.previous_best_ex_score = Some(2293);
    state.ir_ranking.entries[0].ex_score = Some(2300);
    assert_eq!(skin_builtin_value_i64("bmz:ir_score_diff:1", &state), Some(-7));
}

#[test]
fn rival_skin_properties_map_select_rival_best() {
    let state = SkinDrawState {
        rival_ex_score: Some(1500),
        rival_max_combo: Some(700),
        rival_bp: Some(12),
        rival_judge_counts: Some([900, 50, 7, 3, 3]),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(271, &state), Some(1500));
    assert_eq!(skin_state_number(275, &state), Some(700));
    assert_eq!(skin_state_number(276, &state), Some(12));
    assert_eq!(skin_state_number(280, &state), Some(900));
    assert_eq!(skin_state_number(281, &state), Some(50));
    assert_eq!(skin_state_number(282, &state), Some(7));
    assert_eq!(skin_state_number(283, &state), Some(3));
    assert_eq!(skin_state_number(284, &state), Some(3));
    assert_eq!(skin_state_number(285, &state), Some(90));
    assert_eq!(skin_state_number(286, &state), Some(5));
    assert_eq!(skin_state_number(287, &state), Some(0));
    assert!((skin_state_float_number(285, &state).unwrap() - 0.9).abs() < f32::EPSILON);
    assert!((skin_state_float_number(286, &state).unwrap() - 0.05).abs() < f32::EPSILON);
    assert!(!test_skin_op(624, &[], &state));
    assert!(test_skin_op(625, &[], &state));

    let no_rival = SkinDrawState::default();
    assert_eq!(skin_state_number(271, &no_rival), None);
    assert_eq!(skin_state_number(280, &no_rival), None);
    assert_eq!(skin_state_number(285, &no_rival), None);
    assert_eq!(skin_state_float_number(285, &no_rival), None);
    assert!(test_skin_op(624, &[], &no_rival));
    assert!(!test_skin_op(625, &[], &no_rival));
}

#[test]
fn ir_skin_properties_use_offline_defaults() {
    let state = SkinDrawState::default();

    assert!(test_skin_op(50, &[], &state));
    assert!(!test_skin_op(51, &[], &state));
    for op in 601..=608 {
        assert!(!test_skin_op(op, &[], &state), "IR option {op} should be false offline");
    }

    for ref_id in [179, 180, 181, 182, 200, 201, 202, 220, 226, 227, 241, 242, 380, 390] {
        assert_eq!(skin_state_number(ref_id, &state), None, "IR number {ref_id}");
    }
}

#[test]
fn ir_online_property_enables_result_submission_destinations() {
    let state = SkinDrawState {
        ir_ranking: crate::scene::ResultIrSnapshot {
            state: crate::scene::ResultIrState::Loading,
            ..Default::default()
        },
        ..SkinDrawState::default()
    };

    assert!(!test_skin_op(50, &[], &state));
    assert!(test_skin_op(51, &[], &state));
}

#[test]
fn skin_state_number_maps_select_refs() {
    let state = SkinDrawState {
        select_folder_song_count: Some(42),
        select_screen: true,
        select_play_level: 12,
        select_clear_index: 5,
        select_total_notes: 1200,
        select_bpm: 148.0,
        select_chart_normal_notes: 900,
        select_chart_long_notes: 180,
        select_chart_scratch_notes: 100,
        select_chart_long_scratch_notes: 20,
        select_chart_density: 4.56,
        select_chart_peak_density: 12.34,
        select_chart_end_density: 7.89,
        select_chart_total_gauge: 200.0,
        select_chart_main_bpm: 150.0,
        select_min_bpm: 120.0,
        select_max_bpm: 180.0,
        select_length_ms: 183_000,
        hispeed: 2.75,
        total_duration_ms: 500,
        duration_green_ms: Some(300),
        select_master_volume: 0.575,
        select_key_volume: 0.59,
        select_bgm_volume: 0.28,
        select_mode_index: 4,
        select_sort_index: 6,
        select_ln_mode_index: 2,
        select_bp: Some(12),
        select_cb: Some(8),
        ex_score: 1234,
        max_combo: 345,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(11, &state), Some(4));
    assert_eq!(skin_state_number(12, &state), Some(6));
    assert_eq!(skin_state_number(300, &state), Some(42));
    assert_eq!(skin_state_number(96, &state), Some(12));
    assert_eq!(
        skin_state_number(
            96,
            &SkinDrawState { select_play_level: 12, play_level: 9, ..SkinDrawState::default() }
        ),
        Some(9)
    );
    assert_eq!(skin_state_number(370, &state), Some(5));
    assert_eq!(skin_state_number(74, &state), Some(1200));
    assert_eq!(skin_state_number(75, &state), Some(345));
    assert_eq!(skin_state_number(105, &state), Some(345));
    assert_eq!(skin_state_number(76, &state), Some(12));
    assert_eq!(skin_state_number(425, &state), Some(8));
    assert_eq!(skin_state_number(90, &state), Some(180));
    assert_eq!(skin_state_number(91, &state), Some(120));
    assert_eq!(skin_state_number(92, &state), Some(150));
    assert_eq!(skin_state_number(160, &state), Some(148));
    assert_eq!(skin_state_number(350, &state), Some(900));
    assert_eq!(skin_state_number(351, &state), Some(180));
    assert_eq!(skin_state_number(352, &state), Some(100));
    assert_eq!(skin_state_number(353, &state), Some(20));
    assert_eq!(skin_state_number(360, &state), Some(12));
    assert_eq!(skin_state_number(361, &state), Some(34));
    assert_eq!(skin_state_number(362, &state), Some(7));
    assert_eq!(skin_state_number(363, &state), Some(89));
    assert_eq!(skin_state_number(364, &state), Some(4));
    assert_eq!(skin_state_number(365, &state), Some(56));
    assert_eq!(skin_state_number(368, &state), Some(200));
    assert_eq!(skin_state_number(71, &state), Some(1234));
    assert_eq!(skin_state_number(1163, &state), Some(3));
    assert_eq!(skin_state_number(1164, &state), Some(3));
    assert_eq!(skin_state_number(310, &state), Some(2));
    assert_eq!(skin_state_number(311, &state), Some(75));
    assert_eq!(skin_state_number(312, &state), Some(500));
    assert_eq!(skin_state_number(313, &state), Some(300));
    assert_eq!(skin_state_number(57, &state), Some(57));
    assert_eq!(skin_state_number(58, &state), Some(59));
    assert_eq!(skin_state_number(59, &state), Some(28));
    assert_eq!(skin_state_number(308, &state), Some(2));

    assert!(skin_state_number(21, &state).is_some_and(|value| value >= 2026));
    assert!(skin_state_number(22, &state).is_some_and(|value| (1..=12).contains(&value)));
    assert!(skin_state_number(23, &state).is_some_and(|value| (1..=31).contains(&value)));
    assert!(skin_state_number(24, &state).is_some_and(|value| (0..=23).contains(&value)));
    assert!(skin_state_number(25, &state).is_some_and(|value| (0..=59).contains(&value)));
    assert!(skin_state_number(26, &state).is_some_and(|value| (0..=59).contains(&value)));
}

#[test]
fn select_mode_index_matches_beatoraja_skin_ref_order() {
    let cases = [
        ("ALL", 0),
        ("5K", 1),
        ("7K", 2),
        ("10K", 3),
        ("14K", 4),
        ("9K", 5),
        ("24K", 6),
        ("24K_DOUBLE", 7),
        ("unknown", 0),
    ];

    for (mode, expected) in cases {
        assert_eq!(select_mode_index(mode), expected, "select mode {mode}");
    }
}

#[test]
fn select_folder_hides_chart_bpm_and_judge_rank() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        select_in_library: true,
        select_bpm: 0.0,
        select_min_bpm: 0.0,
        select_max_bpm: 0.0,
        judge_rank: None,
        total_duration_ms: 500,
        duration_green_ms: Some(300),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(90, &state), None);
    assert_eq!(skin_state_number(91, &state), None);
    assert_eq!(skin_state_number(92, &state), None);
    assert_eq!(skin_state_number(160, &state), None);
    for ref_id in [350, 351, 352, 353, 360, 362, 364, 368, 1163, 1164] {
        assert_eq!(skin_state_number(ref_id, &state), None, "chart detail ref {ref_id}");
    }
    assert_eq!(skin_state_number(312, &state), Some(500));
    assert_eq!(skin_state_number(313, &state), Some(300));
    for op in 180..=184 {
        assert!(!test_skin_op(op, &[], &state), "judge rank option {op}");
    }
}

#[test]
fn select_course_keeps_score_totals_but_hides_per_chart_details() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Course,
        select_in_library: true,
        select_total_notes: 10_718,
        total_notes: 10_718,
        select_chart_normal_notes: 10_718,
        select_chart_total_gauge: 224.0,
        select_length_ms: 180_000,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(74, &state), Some(10_718));
    for ref_id in [90, 91, 92, 160, 350, 351, 352, 353, 360, 362, 364, 368, 1163, 1164] {
        assert_eq!(skin_state_number(ref_id, &state), None, "chart detail ref {ref_id}");
    }
}

#[test]
fn skin_state_imageset_index_maps_select_options() {
    let state = SkinDrawState {
        select_screen: true,
        select_arrange_index: 2,
        select_arrange_2p_index: 5,
        select_double_option_index: 3,
        select_hs_fix_index: 4,
        select_gauge_index: 4,
        select_target_index: 3,
        select_bga_index: 1,
        judge_timing_auto_adjust: true,
        select_judge_algorithm_index: 2,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_imageset_index(42, &state), Some(2));
    assert_eq!(skin_state_imageset_index(43, &state), Some(5));
    assert_eq!(skin_state_imageset_index(54, &state), Some(3));
    assert_eq!(skin_state_imageset_index(55, &state), Some(4));
    assert_eq!(skin_state_imageset_index(40, &state), Some(4));
    assert_eq!(skin_state_imageset_index(41, &state), Some(3));
    assert_eq!(skin_state_imageset_index(75, &state), Some(1));
    assert_eq!(skin_state_imageset_index(72, &state), Some(1));
    assert_eq!(skin_state_imageset_index(340, &state), Some(2));
    assert_eq!(skin_state_imageset_index(301, &state), Some(0));
    assert_eq!(skin_state_imageset_index(500, &state), None);
}

#[test]
fn result_gauge_type_image_index_uses_applied_gauge() {
    let state = SkinDrawState {
        select_screen: false,
        select_gauge_index: bmz_core::clear::GaugeType::Normal as usize,
        gauge_type: bmz_core::clear::GaugeType::ExHard as i32,
        result_failed: Some(false),
        ..SkinDrawState::default()
    };

    assert_eq!(
        skin_state_imageset_index(40, &state),
        Some(bmz_core::clear::GaugeType::ExHard as usize)
    );
    assert_eq!(skin_image_ref_number(40, &state), Some(bmz_core::clear::GaugeType::ExHard as i64));
}

#[test]
fn select_arrange_index_maps_beatoraja_random_options() {
    assert_eq!(select_arrange_index("NORMAL"), 0);
    assert_eq!(select_arrange_index("MIRROR"), 1);
    assert_eq!(select_arrange_index("RANDOM"), 2);
    assert_eq!(select_arrange_index("R-RANDOM"), 3);
    assert_eq!(select_arrange_index("S-RANDOM"), 4);
    assert_eq!(select_arrange_index("SPIRAL"), 5);
    assert_eq!(select_arrange_index("H-RANDOM"), 6);
    assert_eq!(select_arrange_index("ALL-SCR"), 7);
    assert_eq!(select_arrange_index("RANDOM-EX"), 8);
    assert_eq!(select_arrange_index("S-RANDOM-EX"), 9);
    assert_eq!(select_arrange_index("F-RANDOM"), 2);
    assert_eq!(select_arrange_index("MF-RANDOM"), 2);
    assert_eq!(extended_arrange_index("F-RANDOM"), 10);
    assert_eq!(extended_arrange_index("MF-RANDOM"), 11);
    assert_eq!(select_arrange_index("unknown"), 0);
}

#[test]
fn select_judge_algorithm_index_maps_beatoraja_order() {
    assert_eq!(select_judge_algorithm_index("Combo"), 0);
    assert_eq!(select_judge_algorithm_index("Duration"), 1);
    assert_eq!(select_judge_algorithm_index("Lowest"), 2);
    assert_eq!(select_judge_algorithm_index("unknown"), 0);
}

#[test]
fn select_hs_fix_index_maps_beatoraja_order() {
    assert_eq!(select_hs_fix_index("OFF"), 0);
    assert_eq!(select_hs_fix_index("START BPM"), 1);
    assert_eq!(select_hs_fix_index("MAX BPM"), 2);
    assert_eq!(select_hs_fix_index("MAIN BPM"), 3);
    assert_eq!(select_hs_fix_index("MIN BPM"), 4);
    assert_eq!(select_hs_fix_index("unknown"), 0);
}

#[test]
fn skin_image_ref_number_maps_extended_select_arrange() {
    let state = SkinDrawState {
        select_screen: true,
        select_arrange_index: 9,
        select_arrange_2p_index: 6,
        select_extended_arrange_index: 11,
        select_extended_arrange_2p_index: 10,
        select_gauge_index: 4,
        select_target_index: 10,
        select_double_option_index: 2,
        select_hs_fix_index: 3,
        select_bga_index: 2,
        select_assist_index: 1,
        judge_timing_auto_adjust: true,
        select_gauge_auto_shift_index: 3,
        select_ln_mode_index: 2,
        select_judge_algorithm_index: 3,
        select_bottom_shiftable_gauge_index: 2,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_image_ref_number(40, &state), Some(4));
    assert_eq!(skin_image_ref_number(41, &state), Some(10));
    assert_eq!(skin_image_ref_number(42, &state), Some(9));
    assert_eq!(skin_image_ref_number(43, &state), Some(6));
    assert_eq!(skin_image_ref_number(344, &state), Some(11));
    assert_eq!(skin_image_ref_number(345, &state), Some(10));
    assert_eq!(skin_image_ref_number(54, &state), Some(2));
    assert_eq!(skin_image_ref_number(55, &state), Some(3));
    assert_eq!(skin_image_ref_number(72, &state), Some(2));
    assert_eq!(skin_image_ref_number(75, &state), Some(1));
    assert_eq!(skin_image_ref_number(78, &state), Some(3));
    assert_eq!(skin_image_ref_number(308, &state), Some(2));
    assert_eq!(skin_image_ref_number(340, &state), Some(3));
    assert_eq!(skin_image_ref_number(341, &state), Some(2));
    assert_eq!(skin_state_number(42, &state), Some(9));
    assert_eq!(skin_state_number(43, &state), Some(6));
    assert_eq!(skin_state_number(344, &state), Some(11));
    assert_eq!(skin_state_number(345, &state), Some(10));
    assert_eq!(skin_state_number(54, &state), Some(2));
    assert_eq!(skin_state_number(55, &state), Some(3));
    assert_eq!(skin_state_number(340, &state), Some(3));
    assert_eq!(skin_state_event_index(40, &state), 4);
    assert_eq!(skin_state_event_index(41, &state), 10);
    assert_eq!(skin_state_event_index(42, &state), 9);
    assert_eq!(skin_state_event_index(43, &state), 6);
    assert_eq!(skin_state_event_index(344, &state), 11);
    assert_eq!(skin_state_event_index(345, &state), 10);
    assert_eq!(skin_state_event_index(54, &state), 2);
    assert_eq!(skin_state_event_index(55, &state), 3);
    assert_eq!(skin_state_event_index(72, &state), 2);
    assert_eq!(skin_state_event_index(73, &state), 1);
    assert_eq!(skin_state_event_index(75, &state), 1);
    assert_eq!(skin_state_event_index(78, &state), 3);
    assert_eq!(skin_state_event_index(308, &state), 2);
    assert_eq!(skin_state_event_index(340, &state), 3);
    assert_eq!(skin_state_event_index(341, &state), 2);
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
fn arrange_ref_uses_result_arrange_on_result_screen() {
    let state = SkinDrawState {
        select_arrange_index: 2,
        select_arrange_2p_index: 3,
        result_arrange_index: 8,
        result_arrange_2p_index: 1,
        result_extended_arrange_index: 11,
        result_extended_arrange_2p_index: 10,
        result_failed: Some(false),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_imageset_index(42, &state), Some(8));
    assert_eq!(skin_state_imageset_index(43, &state), Some(1));
    assert_eq!(skin_image_ref_number(42, &state), Some(8));
    assert_eq!(skin_image_ref_number(43, &state), Some(1));
    assert_eq!(skin_state_number(42, &state), Some(8));
    assert_eq!(skin_state_number(43, &state), Some(1));
    assert_eq!(skin_state_event_index(42, &state), 8);
    assert_eq!(skin_state_event_index(43, &state), 1);
    assert_eq!(skin_state_imageset_index(344, &state), Some(11));
    assert_eq!(skin_state_imageset_index(345, &state), Some(10));
    assert_eq!(skin_state_number(344, &state), Some(11));
    assert_eq!(skin_state_number(345, &state), Some(10));
    assert_eq!(skin_state_event_index(344, &state), 11);
    assert_eq!(skin_state_event_index(345, &state), 10);
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
fn random_lane_refs_are_available_outside_result_screen() {
    let mut refs = [0; SKIN_RANDOM_LANE_REF_COUNT];
    refs[0] = 7;
    let state = SkinDrawState { random_lane_refs: refs, ..SkinDrawState::default() };

    assert_eq!(skin_state_imageset_index(450, &state), Some(7));
    assert_eq!(skin_state_event_index(450, &state), 7);
    assert_eq!(skin_state_number(450, &state), Some(7));
}

#[test]
fn select_skin_imageset_uses_extended_arrange_index() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "arrange.png" }],
                "image": [
                    { "id": "normal", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "mirror", "src": 1, "x": 10, "y": 0, "w": 10, "h": 10 },
                    { "id": "random", "src": 1, "x": 20, "y": 0, "w": 10, "h": 10 },
                    { "id": "r-random", "src": 1, "x": 30, "y": 0, "w": 10, "h": 10 },
                    { "id": "s-random", "src": 1, "x": 40, "y": 0, "w": 10, "h": 10 },
                    { "id": "spiral", "src": 1, "x": 50, "y": 0, "w": 10, "h": 10 },
                    { "id": "h-random", "src": 1, "x": 60, "y": 0, "w": 10, "h": 10 },
                    { "id": "all-scr", "src": 1, "x": 70, "y": 0, "w": 10, "h": 10 },
                    { "id": "random-ex", "src": 1, "x": 80, "y": 0, "w": 10, "h": 10 },
                    { "id": "s-random-ex", "src": 1, "x": 90, "y": 0, "w": 10, "h": 10 }
                ],
                "imageset": [{
                    "id": "option-random",
                    "ref": 42,
                    "images": [
                        "normal", "mirror", "random", "r-random", "s-random",
                        "spiral", "h-random", "all-scr", "random-ex", "s-random-ex"
                    ]
                }],
                "destination": [{
                    "id": "option-random",
                    "dst": [{ "x": 10, "y": 20, "w": 20, "h": 10 }]
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
            source_size: SkinImageSize { width: 100.0, height: 10.0 },
        },
    )]);
    let items = document.select_render_items(
        &sources,
        &crate::scene::SelectSnapshot { arrange: "S-RANDOM-EX".to_string(), ..Default::default() },
    );

    assert!(matches!(
        items.as_slice(),
        [SkinRenderItem::Image {
            texture: SkinTextureId(42),
            uv: TextureRegion { x, .. },
            ..
        }] if approx_eq(*x, 0.9)
    ));
}

#[test]
fn select_target_index_maps_fixed_targets() {
    let index = |target| select_target_index_for_name(target).unwrap_or(0);
    assert_eq!(index("NONE"), 0);
    assert_eq!(index("RANK_A"), 1);
    assert_eq!(index("RANK_AA-"), 2);
    assert_eq!(index("RANK_AA"), 3);
    assert_eq!(index("RANK_AAA-"), 4);
    assert_eq!(index("RANK_AAA"), 5);
    assert_eq!(index("RANK_MAX-"), 6);
    assert_eq!(index("MAX"), 7);
    assert_eq!(index("RANK_NEXT"), 8);
    assert_eq!(index("IR_TOP"), 9);
    assert_eq!(index("IR_NEXT"), 10);
    assert_eq!(index("RIVAL TOP"), 11);
    assert_eq!(index("RIVAL NEXT"), 12);
    assert_eq!(index("RIVAL"), 11);
    assert_eq!(index("AAA"), 5);
    assert_eq!(index("AA"), 3);
    assert_eq!(index("A"), 1);
    assert_eq!(index("B"), 1);
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
fn bundled_beatoraja_default_select_json_loads_when_available() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.local/beatoraja/skin/default/select.json");
    if !path.is_file() {
        return;
    }

    let document = SkinDocument::load_beatoraja_json(&path).unwrap();

    assert_eq!(document.name, "beatoraja default");
    assert_eq!(document.skin_type, 5);
    assert!(document.songlist.is_some());
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
fn local_ecfn_converted_select_json_loads_when_available() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/ECFN/select/select-converted.json");
    if !path.is_file() {
        return;
    }

    let document = SkinDocument::load_beatoraja_json(&path).unwrap();

    assert_eq!(document.skin_type, 5);
    assert!(document.songlist.is_some());
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
fn judge_timer_elapsed_ms_selects_animation_frame() {
    // timer=46 → TIMER_JUDGE_1P; two dst frames at time=0 and time=200
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "panel", "timer": 46, "dst": [
                        { "time": 0,   "x": 0,   "y": 0, "w": 10, "h": 10 },
                        { "time": 200, "x": 50,  "y": 0, "w": 10, "h": 10 }
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
            texture: SkinTextureId(2),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        },
    )]);

    // judge_ms=Some(100) → between frame 0 and frame 200 → x should be 0.25 (interpolated)
    let state_early = SkinDrawState {
        judge_ms: judge_region_state(0, 100, 0).judge_ms,
        ..SkinDrawState::default()
    };
    let items_early = document.static_image_render_items(&sources, &state_early);
    assert_eq!(items_early.len(), 1);
    assert!(
        matches!(items_early[0], SkinRenderItem::Image { rect: Rect { x, .. }, .. }
            if approx_eq(x, 0.25)),
        "at judge_ms=100, x should interpolate to 0.25 (halfway between 0 and 0.5)"
    );

    // judge_ms=Some(300) → past last frame → last frame x=0.5
    let state_late = SkinDrawState {
        judge_ms: judge_region_state(0, 300, 0).judge_ms,
        ..SkinDrawState::default()
    };
    let items_late = document.static_image_render_items(&sources, &state_late);
    assert_eq!(items_late.len(), 1);
    assert!(
        matches!(items_late[0], SkinRenderItem::Image { rect: Rect { x, .. }, .. }
            if approx_eq(x, 0.5)),
        "at judge_ms=300 (past last frame), x should be at last frame x=0.5"
    );

    // judge_ms=None → no items (timer inactive)
    let state_inactive =
        SkinDrawState { judge_ms: [None; MAX_JUDGE_REGIONS], ..SkinDrawState::default() };
    let items_inactive = document.static_image_render_items(&sources, &state_inactive);
    assert_eq!(items_inactive.len(), 0, "judge_ms=None should produce no items");
}

#[test]
fn dst_if_value_selects_frame_by_enabled_option() {
    // property: option 920 enabled (1P)
    // destination dst has two conditional frames: one for 920, one for 921
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 1280, "h": 720,
                "property": [
                    { "name": "Side", "def": "1P", "item": [
                        { "name": "1P", "op": 920 },
                        { "name": "2P", "op": 921 }
                    ]}
                ],
                "source": [{ "id": "src", "path": "a.png" }],
                "image": [{ "id": "img", "src": "src", "w": 10, "h": 10 }],
                "destination": [
                    { "id": "img", "dst": [
                        { "if": [920], "value": { "time": 0, "x": 100, "y": 200, "w": 50, "h": 50 } },
                        { "if": [921], "value": { "time": 0, "x": 900, "y": 200, "w": 50, "h": 50 } },
                        { "time": 500 }
                    ]}
                ]
            }
            "#,
        )
        .unwrap();

    let sources = mock_source("src", 10.0, 10.0);
    let state = SkinDrawState::default();
    let items = document.static_image_render_items(&sources, &state);

    // With option 920 (1P) enabled, x should be 100/1280
    assert_eq!(items.len(), 1);
    let SkinRenderItem::Image { rect, .. } = &items[0] else { panic!() };
    assert!(approx_eq(rect.x, 100.0 / 1280.0), "expected 1P x position, got {}", rect.x);
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
fn result_judge_pie_segments_use_runtime_judge_counts() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 200, "h": 200,
                "source": [{ "id": "src", "path": "jud_detail.png" }],
                "image": [
                    { "id": "judge_graph", "src": "src", "x": 574, "y": 1, "w": 140, "h": 8 }
                ],
                "destination": [
                    { "id": "judge_graph", "dst": [{ "x": 41, "y": 241, "w": 140, "h": 8, "r": 8, "g": 179, "b": 239, "angle": 91 }] },
                    { "id": "judge_graph", "dst": [{ "x": 41, "y": 241, "w": 140, "h": 8, "r": 8, "g": 179, "b": 239, "angle": 100 }] },
                    { "id": "judge_graph", "dst": [{ "x": 41, "y": 241, "w": 140, "h": 8, "r": 8, "g": 179, "b": 239, "angle": 120 }] },
                    { "id": "judge_graph", "dst": [{ "x": 41, "y": 241, "w": 140, "h": 8, "r": 8, "g": 179, "b": 239, "angle": 150 }] },
                    { "id": "judge_graph", "dst": [{ "x": 41, "y": 241, "w": 140, "h": 8, "r": 8, "g": 179, "b": 239, "angle": 290 }] }
                ]
            }
            "#,
        )
        .unwrap();

    let sources = mock_source("src", 800.0, 800.0);
    let state = SkinDrawState {
        result_failed: Some(false),
        judge_counts: DisplayJudgeCounts {
            pgreat: 70,
            great: 20,
            good: 5,
            bad: 3,
            poor: 2,
            empty_poor: 0,
        },
        ..SkinDrawState::default()
    };
    let items = document.static_image_render_items(&sources, &state);

    let segments = items
        .iter()
        .map(|item| match item {
            SkinRenderItem::RotatedImage { tint, angle_deg, .. } => (
                (
                    (tint.r * 255.0).round() as i32,
                    (tint.g * 255.0).round() as i32,
                    (tint.b * 255.0).round() as i32,
                ),
                *angle_deg as i32,
            ),
            _ => panic!("expected rotated judge pie segment"),
        })
        .collect::<Vec<_>>();
    let colors = segments.iter().map(|(color, _)| *color).collect::<Vec<_>>();
    assert_eq!(
        colors,
        vec![(217, 68, 35), (226, 135, 42), (240, 190, 15), (240, 239, 10), (8, 179, 239),]
    );
    let angles = segments.iter().map(|(_, angle)| *angle).collect::<Vec<_>>();
    assert_eq!(angles, vec![-91, -100, -120, -150, -290]);
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
fn skin_state_number_maps_folder_lamp_counts_on_folder_rows() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        select_folder_lamp_counts: [12, 3, 0, 1, 4, 5, 6, 7, 8, 9, 10],
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(320, &state), Some(12));
    assert_eq!(skin_state_number(321, &state), Some(3));
    assert_eq!(skin_state_number(324, &state), Some(4));
    assert_eq!(skin_state_number(330, &state), Some(10));

    let song = SkinDrawState {
        select_row_kind: SelectRowKind::Song,
        select_is_folder: false,
        select_in_library: true,
        select_chart_normal_notes: 900,
        ..state
    };
    assert_eq!(skin_state_number(320, &song), None);
    assert_eq!(skin_state_number(300, &song), None);
    assert_eq!(skin_state_number(350, &song), Some(900));
}

#[test]
fn select_folder_song_count_uses_cursor_folder_row() {
    let row = SelectRowSnapshot {
        is_folder: true,
        kind: SelectRowKind::Folder,
        folder_lamp_counts: [2, 3, 0, 1, 0, 0, 0, 0, 0, 0, 0],
        ..SelectRowSnapshot::default()
    };
    assert_eq!(select_row_folder_song_count(&row), Some(6));

    let song = SelectRowSnapshot { kind: SelectRowKind::Song, ..row };
    assert_eq!(select_row_folder_song_count(&song), None);
}

#[test]
fn skin_image_index_number_select_favorite_refs() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_favorite_song: true,
        select_favorite_chart: false,
        select_max_bpm: 200.0,
        ..SkinDrawState::default()
    };
    assert_eq!(skin_image_index_number(89, &state), Some(1));
    assert_eq!(skin_image_index_number(90, &state), Some(0));
    assert_eq!(skin_state_number(90, &state), Some(200));

    let chart_favorite = SkinDrawState { select_favorite_chart: true, ..state };
    assert_eq!(skin_image_index_number(90, &chart_favorite), Some(1));
    assert_eq!(skin_state_number(90, &chart_favorite), Some(200));
}

#[test]
fn skin_image_index_number_result_favorite_ref_has_only_two_states() {
    let not_favorite =
        SkinDrawState { result_favorite_chart: Some(false), ..SkinDrawState::default() };
    assert_eq!(skin_image_index_number(90, &not_favorite), Some(0));

    let favorite = SkinDrawState { result_favorite_chart: Some(true), ..SkinDrawState::default() };
    assert_eq!(skin_image_index_number(90, &favorite), Some(1));
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
fn wmii_nearest_rank_diff_value_uses_absolute_runtime_difference() {
    let state = SkinDrawState { ex_score: 155, total_notes: 100, ..Default::default() };
    let value =
        SkinValueDef { value_expr: "bmz:nearest_rank_diff_abs".to_string(), ..Default::default() };
    assert_eq!(skin_value_number(&value, &state), Some(1));
}

#[test]
fn luxe_flat_nearest_rank_uses_runtime_result_score() {
    let state = SkinDrawState {
        ex_score: 2246,
        total_notes: 1261,
        result_failed: Some(false),
        result_grade_diff_display: ResultGradeDiffDisplay::Nearest,
        ..Default::default()
    };
    let value =
        SkinValueDef { value_expr: "bmz:nearest_rank_diff_abs".to_string(), ..Default::default() };

    assert_eq!(skin_value_number(&value, &state), Some(4));
    assert_eq!(result_grade_diff_label(&state), Some("AAA+4".to_string()));
    assert!(eval_skin_draw_condition("nearest_rank(AAA,plus)", &state));
    assert!(!eval_skin_draw_condition("nearest_rank(MAX,minus)", &state));
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
fn wmii_course_clear_rate_uses_course_progress_and_aggregate_judges() {
    let completed = SkinDrawState {
        total_notes: 7_085,
        judge_counts: DisplayJudgeCounts {
            pgreat: 6_407,
            great: 631,
            good: 24,
            bad: 9,
            poor: 14,
            empty_poor: 32,
        },
        ..SkinDrawState::default()
    };
    let partial = SkinDrawState {
        total_notes: 100,
        judge_counts: DisplayJudgeCounts { pgreat: 50, ..DisplayJudgeCounts::default() },
        ..SkinDrawState::default()
    };
    let value = SkinValueDef {
        value_expr: SKIN_EXPR_COURSE_CLEAR_RATE.to_string(),
        ..SkinValueDef::default()
    };

    assert_eq!(skin_value_number(&value, &completed), Some(100));
    assert!((course_clear_rate_value(&partial) - 70.0).abs() < 0.001);
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
fn result_gaugegraph_sample_ratio_matches_beatoraja_history_spacing() {
    assert_eq!(gaugegraph_sample_ratio(0, 3), 0.0);
    assert!((gaugegraph_sample_ratio(1, 3) - (1.0 / 3.0)).abs() < 1e-6);
    assert!((gaugegraph_sample_ratio(2, 3) - (2.0 / 3.0)).abs() < 1e-6);
    assert_eq!(gaugegraph_sample_ratio(0, 0), 0.0);
}

#[test]
fn result_gaugegraph_multiplies_color_alpha_by_destination_alpha() {
    let graph: SkinGaugeGraphDef = serde_json::from_str(
        r#"{
                "id":"graph",
                "color":["11223380","445566","77889940","AABBCC"]
            }"#,
    )
    .unwrap();
    let frame_alpha = 200.0 / 255.0;

    let colors = gaugegraph_colors(&graph, 0, frame_alpha);

    assert!((colors.border_line.a - (128.0 / 255.0) * frame_alpha).abs() < 1e-6);
    assert!((colors.border_bg.a - frame_alpha).abs() < 1e-6);
    assert!((colors.graph_line.a - (64.0 / 255.0) * frame_alpha).abs() < 1e-6);
    assert!((colors.graph_bg.a - frame_alpha).abs() < 1e-6);
}

#[test]
fn result_gaugegraph_caches_only_completed_graph_per_type_and_graph_arc() {
    use crate::snapshot::{ResultGaugeGraphPoint, ResultGraphSnapshot};

    let document: SkinDocument = serde_json::from_str(r#"{"w":1280,"h":720}"#).unwrap();
    let graph_def: SkinGaugeGraphDef = serde_json::from_str(r#"{"id":"graph"}"#).unwrap();
    let destination: SkinDestinationDef =
        serde_json::from_str(r#"{"id":"graph","dst":[]}"#).unwrap();
    let frame = ResolvedSkinFrame { w: 640, h: 360, a: 255, ..Default::default() };
    let graph = Arc::new(ResultGraphSnapshot {
        gauge_points: vec![
            ResultGaugeGraphPoint {
                value: 20.0,
                max: 100.0,
                border: 80.0,
                gauge_type: 2,
                ..Default::default()
            },
            ResultGaugeGraphPoint {
                value: 70.0,
                max: 100.0,
                border: 80.0,
                gauge_type: 3,
                ..Default::default()
            },
            ResultGaugeGraphPoint {
                value: 40.0,
                max: 100.0,
                border: 80.0,
                gauge_type: 2,
                ..Default::default()
            },
            ResultGaugeGraphPoint {
                value: 90.0,
                max: 100.0,
                border: 80.0,
                gauge_type: 3,
                ..Default::default()
            },
        ],
        ..Default::default()
    });
    let render = |cache: &mut ResultRenderCache, elapsed_ms, gauge_type| {
        document.gaugegraph_render_items(
            7,
            &graph_def,
            &destination,
            frame,
            &SkinDrawState {
                elapsed_ms,
                result_gauge_graph_type: Some(gauge_type),
                ..Default::default()
            },
            &graph.gauge_points,
            Some(cache),
        )
    };

    let mut cache = ResultRenderCache::default();
    cache.prepare_gauge_graph(&graph);
    let reveal = render(&mut cache, 1499, 2);
    assert!(matches!(reveal.as_slice(), [SkinRenderItem::RectBatch { cache: None, .. }]));

    let normal = render(&mut cache, 1500, 2);
    let [SkinRenderItem::RectBatch { rects: normal_rects, cache: Some(normal_key) }] =
        normal.as_slice()
    else {
        panic!("completed gauge graph must use the offscreen batch cache");
    };
    let normal_again = render(&mut cache, 2500, 2);
    let [SkinRenderItem::RectBatch { rects: normal_again_rects, cache: Some(normal_again_key) }] =
        normal_again.as_slice()
    else {
        panic!("completed gauge graph cache must stay reusable");
    };
    assert!(Arc::ptr_eq(normal_rects, normal_again_rects));
    assert_eq!(normal_key, normal_again_key);

    let hard = render(&mut cache, 2500, 3);
    let [SkinRenderItem::RectBatch { cache: Some(hard_key), .. }] = hard.as_slice() else {
        panic!("switched gauge graph must also use its own batch cache");
    };
    assert_ne!(normal_key.key, hard_key.key);
    let normal_after_switch = render(&mut cache, 2500, 2);
    let [
        SkinRenderItem::RectBatch {
            rects: normal_after_switch_rects,
            cache: Some(normal_after_switch_key),
        },
    ] = normal_after_switch.as_slice()
    else {
        panic!("switching back must reuse the original gauge batch");
    };
    assert!(Arc::ptr_eq(normal_rects, normal_after_switch_rects));
    assert_eq!(normal_key, normal_after_switch_key);

    let changed_graph = Arc::new(ResultGraphSnapshot {
        gauge_points: graph
            .gauge_points
            .iter()
            .copied()
            .map(|mut point| {
                point.value += 1.0;
                point
            })
            .collect(),
        ..Default::default()
    });
    cache.prepare_gauge_graph(&changed_graph);
    let changed = document.gaugegraph_render_items(
        7,
        &graph_def,
        &destination,
        frame,
        &SkinDrawState { elapsed_ms: 1500, result_gauge_graph_type: Some(2), ..Default::default() },
        &changed_graph.gauge_points,
        Some(&mut cache),
    );
    let [SkinRenderItem::RectBatch { cache: Some(changed_key), .. }] = changed.as_slice() else {
        panic!("changed graph must produce a completed batch");
    };
    assert_ne!(normal_key.key, changed_key.key);

    let mut other_context_cache = ResultRenderCache::default();
    other_context_cache.prepare_gauge_graph(&graph);
    let other_context = render(&mut other_context_cache, 1500, 2);
    let [SkinRenderItem::RectBatch { cache: Some(other_context_key), .. }] =
        other_context.as_slice()
    else {
        panic!("another skin context must produce a completed batch");
    };
    assert_ne!(normal_key.key, other_context_key.key);
}

#[test]
fn result_gaugegraph_batch_skips_additive_black_backgrounds() {
    use crate::snapshot::ResultGaugeGraphPoint;

    let document: SkinDocument = serde_json::from_str(r#"{"w":1280,"h":720}"#).unwrap();
    let graph_def: SkinGaugeGraphDef = serde_json::from_str(
        r#"{
                "id":"graph",
                "borderlineColor":"00FF00",
                "borderColor":"000000",
                "grooveFailLineColor":"FF0000",
                "grooveFailBGColor":"000000"
            }"#,
    )
    .unwrap();
    let destination: SkinDestinationDef =
        serde_json::from_str(r#"{"id":"graph","blend":2,"dst":[]}"#).unwrap();
    let frame = ResolvedSkinFrame { w: 640, h: 360, a: 255, ..Default::default() };
    let points = [
        ResultGaugeGraphPoint {
            value: 20.0,
            max: 100.0,
            border: 80.0,
            gauge_type: 2,
            ..Default::default()
        },
        ResultGaugeGraphPoint {
            value: 40.0,
            max: 100.0,
            border: 80.0,
            gauge_type: 2,
            ..Default::default()
        },
    ];

    let items = document.gaugegraph_render_items(
        7,
        &graph_def,
        &destination,
        frame,
        &SkinDrawState { elapsed_ms: 1500, result_gauge_graph_type: Some(2), ..Default::default() },
        &points,
        None,
    );
    let [SkinRenderItem::RectBatch { rects, .. }] = items.as_slice() else {
        panic!("gauge graph must render as a rectangle batch");
    };
    assert!(!rects.is_empty());
    assert!(rects.iter().all(|command| !is_additive_black(command.color)));
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
fn graph_value_select_rate_exscore_uses_selected_total_notes() {
    // ECFN select uses BARGRAPH_RATE_EXSCORE (147) for the score rate bar.
    // Select has no play-progress past_notes, so it should use the selected chart total.
    let state = SkinDrawState {
        select_screen: true,
        ex_score: 418,
        total_notes: 594,
        select_total_notes: 594,
        past_notes: 0,
        ..SkinDrawState::default()
    };
    let v = graph_value(147, &state);
    assert!((v - (418.0 / 1188.0)).abs() < 1e-5, "select ex rate: got {v}");
}

#[test]
fn select_state_exposes_best_judge_detail_counts() {
    let document: SkinDocument = serde_json::from_str(r#"{ "w": 1280, "h": 720 }"#).unwrap();
    let row = SelectRowSnapshot {
        index: 0,
        total_notes: 100,
        judge_counts: crate::snapshot::DisplayJudgeCounts {
            pgreat: 20,
            great: 30,
            good: 10,
            bad: 5,
            poor: 2,
            empty_poor: 1,
        },
        fast_slow_counts: Some(crate::snapshot::FastSlowJudgeCounts {
            fast_pgreat: 2,
            slow_pgreat: 3,
            fast_great: 7,
            slow_good: 4,
            fast_bad: 3,
            slow_empty_poor: 2,
            ..Default::default()
        }),
        ..SelectRowSnapshot::default()
    };
    let snapshot =
        SelectSnapshot { selected_index: 0, rows: vec![row], ..SelectSnapshot::default() };

    let (state, _) = document.select_draw_state(&snapshot, None);

    assert_eq!(skin_state_number(110, &state), Some(20));
    assert_eq!(skin_state_number(111, &state), Some(30));
    assert_eq!(skin_state_number(112, &state), Some(10));
    assert_eq!(skin_state_number(113, &state), Some(5));
    assert_eq!(skin_state_number(426, &state), Some(3));
    assert_eq!(skin_state_number(412, &state), Some(7));
    assert_eq!(skin_state_number(422, &state), Some(2));
    assert!((graph_value(140, &state) - 0.2).abs() < 1e-5);
    assert!((graph_value(141, &state) - 0.3).abs() < 1e-5);
    assert!((graph_value(148, &state) - (12.0 / 21.0)).abs() < 1e-5);
    assert!((graph_value(149, &state) - (9.0 / 21.0)).abs() < 1e-5);
}

#[test]
fn select_state_starts_input_timer_after_document_delay() {
    let document: SkinDocument =
        serde_json::from_str(r#"{ "w": 1280, "h": 720, "input": 500 }"#).unwrap();

    let (waiting, _) = document.select_draw_state(
        &SelectSnapshot { time: TimeUs(500_000), ..SelectSnapshot::default() },
        None,
    );
    let (active, _) = document.select_draw_state(
        &SelectSnapshot { time: TimeUs(725_000), ..SelectSnapshot::default() },
        None,
    );

    assert_eq!(waiting.start_input_ms, None);
    assert_eq!(active.start_input_ms, Some(225));
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

#[test]
fn select_render_items_passes_selected_row_genre_to_string_ref_13() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "text": [{ "id": "genre", "size": 6, "ref": 13 }],
                "destination": [{ "id": "genre", "dst": [{ "x": 10, "y": 40, "w": 40, "h": 6 }] }]
            }
            "#,
    )
    .unwrap();
    let snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            genre: "Techno".to_string(),
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&HashMap::new(), &snapshot);

    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Text { text, .. } if text == "Techno"
    )));
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
fn select_course_rows_only_expose_course_stage_title_refs() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "text": [
                    { "id": "title", "size": 6, "ref": 12 },
                    { "id": "genre", "size": 6, "ref": 13 },
                    { "id": "artist", "size": 6, "ref": 16 },
                    { "id": "stage", "size": 6, "ref": 150 }
                ],
                "destination": [
                    { "id": "title", "dst": [{ "x": 10, "y": 70, "w": 40, "h": 6 }] },
                    { "id": "genre", "dst": [{ "x": 10, "y": 60, "w": 40, "h": 6 }] },
                    { "id": "artist", "dst": [{ "x": 10, "y": 50, "w": 40, "h": 6 }] },
                    { "id": "stage", "dst": [{ "x": 10, "y": 40, "w": 40, "h": 6 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            title: "Course title".to_string(),
            genre: "Course genre".to_string(),
            artist: "Course artist".to_string(),
            kind: SelectRowKind::Course,
            course_titles: std::array::from_fn(|index| {
                if index == 0 { "Stage title".to_string() } else { String::new() }
            }),
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&HashMap::new(), &snapshot);
    let texts = items
        .iter()
        .filter_map(|item| match item {
            SkinRenderItem::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(texts, vec!["Stage title"]);
}

#[test]
fn skin_state_text_formats_result_table_title_expr() {
    let text = SkinTextDef {
        value_expr: SKIN_EXPR_RESULT_TABLE_TITLE.to_string(),
        ..SkinTextDef::default()
    };
    let state = SkinTextState {
        title: "Song",
        subtitle: "Another",
        table_text_primary: "Insane",
        table_level: "★12",
        ..SkinTextState::default()
    };

    assert_eq!(skin_state_text(&text, &state), "★12 Insane Song Another");
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
fn skin_state_text_formats_select_option_fields() {
    let state = SkinTextState {
        target: "AAA",
        select_arrange: "RANDOM",
        select_arrange_2p: "MIRROR",
        select_gauge: "HARD",
        select_gauge_auto_shift: "BEST CLEAR",
        select_bottom_shiftable_gauge: "NORMAL",
        select_double_option: "FLIP",
        select_hs_fix: "MAIN BPM",
        select_assist: "AUTOPLAY",
        select_mode: "7K",
        select_sort: "LEVEL",
        select_ln_mode: "AUTO(LN)",
        select_bga: "AUTO",
        select_judge_timing_auto_adjust: "ON",
        ..SkinTextState::default()
    };
    let make_text = |id: &str| SkinTextDef { id: id.to_string(), ..SkinTextDef::default() };

    assert_eq!(skin_state_text(&make_text("bmz_select_target"), &state), "RANK AAA");
    assert_eq!(skin_state_text(&make_text("bmz_select_arrange"), &state), "RANDOM");
    assert_eq!(skin_state_text(&make_text("bmz_select_arrange_2p"), &state), "MIRROR");
    assert_eq!(skin_state_text(&make_text("bmz_select_gauge"), &state), "HARD");
    assert_eq!(skin_state_text(&make_text("bmz_select_gauge_auto_shift"), &state), "BEST CLEAR");
    assert_eq!(skin_state_text(&make_text("bmz_select_bottom_shiftable_gauge"), &state), "NORMAL");
    assert_eq!(skin_state_text(&make_text("bmz_select_double_option"), &state), "FLIP");
    assert_eq!(skin_state_text(&make_text("bmz_select_hs_fix"), &state), "MAIN BPM");
    assert_eq!(skin_state_text(&make_text("bmz_select_assist"), &state), "AUTOPLAY");
    assert_eq!(skin_state_text(&make_text("bmz_select_mode"), &state), "7K");
    assert_eq!(skin_state_text(&make_text("bmz_select_sort"), &state), "LEVEL");
    assert_eq!(skin_state_text(&make_text("bmz_select_ln_mode"), &state), "AUTO(LN)");
    assert_eq!(skin_state_text(&make_text("bmz_select_bga"), &state), "AUTO");
    assert_eq!(skin_state_text(&make_text("bmz_select_judge_timing_auto_adjust"), &state), "ON");
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
fn format_rm_skin_course_table_text_matches_lua_branches() {
    use crate::snapshot::CourseStageMarker;

    assert_eq!(
        format_rm_skin_course_table_text(Some(CourseStageMarker::Final), "", "", ""),
        "COURSE : STAGE FINAL"
    );
    assert_eq!(
        format_rm_skin_course_table_text(
            Some(CourseStageMarker::Stage2),
            "Insane",
            "★12",
            "★12Insane"
        ),
        "COURSE : STAGE 2"
    );
    assert_eq!(
        format_rm_skin_course_table_text(None, "Insane", "★12", "★12Insane"),
        "Insane > ★12"
    );
    assert_eq!(format_rm_skin_course_table_text(None, "", "★12", "★12Insane"), " > ★12");
    assert_eq!(format_rm_skin_course_table_text(None, "Insane", "", "★12Insane"), "★12Insane");
    assert_eq!(format_rm_skin_course_table_text(None, "", "", ""), "# No-Table");
}

#[test]
fn skin_state_text_course_table_uses_value_expr_and_table_id() {
    use crate::snapshot::CourseStageMarker;

    let state = SkinTextState {
        table_level: "★12",
        table_text_primary: "Insane",
        table_text_secondary: "★12",
        table_text_fallback: "★12Insane",
        course_stage: None,
        ..SkinTextState::default()
    };
    let by_expr = SkinTextDef {
        id: "table".to_string(),
        value_expr: SKIN_EXPR_COURSE_TABLE_TEXT.to_string(),
        ..SkinTextDef::default()
    };
    assert_eq!(skin_state_text(&by_expr, &state), "Insane > ★12");

    let by_id = SkinTextDef { id: "table".to_string(), ..SkinTextDef::default() };
    assert_eq!(skin_state_text(&by_id, &state), "Insane > ★12");

    let course_state =
        SkinTextState { course_stage: Some(CourseStageMarker::Stage1), ..state.clone() };
    assert_eq!(skin_state_text(&by_id, &course_state), "COURSE : STAGE 1");

    let by_ref = |ref_id| SkinTextDef { ref_id, ..SkinTextDef::default() };
    assert_eq!(skin_state_text(&by_ref(1001), &state), "Insane");
    assert_eq!(skin_state_text(&by_ref(1002), &state), "★12");
    assert_eq!(skin_state_text(&by_ref(1003), &state), "★12Insane");
    assert_eq!(
        skin_state_text(&by_ref(1010), &state),
        format!("bmz-player {}", env!("CARGO_PKG_VERSION"))
    );

    let concatenated =
        SkinTextDef { value_expr: "bmz:text_concat:1001:1002".to_string(), ..Default::default() };
    assert_eq!(skin_state_text(&concatenated, &state), "Insane ★12");
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

fn mock_source(id: &str, width: f32, height: f32) -> HashMap<String, SkinDocumentTexture> {
    let mut map = HashMap::new();
    map.insert(
        id.to_string(),
        SkinDocumentTexture {
            source_id: id.to_string(),
            texture: SkinTextureId(9999),
            source_size: SkinImageSize { width, height },
        },
    );
    map
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

fn approx_eq(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 0.0001
}

fn skin_render_item_has_rect_color(
    item: &SkinRenderItem,
    predicate: impl Fn(&Color) -> bool,
) -> bool {
    match item {
        SkinRenderItem::Rect { color, .. } => predicate(color),
        SkinRenderItem::RectBatch { rects, .. } => rects.iter().any(|rect| predicate(&rect.color)),
        _ => false,
    }
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

#[derive(Debug, Default)]
struct AlternatingLuaDrawRuntime {
    calls: std::sync::atomic::AtomicUsize,
}

impl SkinLuaDrawRuntime for AlternatingLuaDrawRuntime {
    fn evaluate_draw(
        &self,
        callback_id: usize,
        _state: &SkinDrawState,
        _enabled_options: &[i32],
        _text_values: &BTreeMap<i32, String>,
    ) -> bool {
        assert_eq!(callback_id, 0);
        (self.calls.fetch_add(1, Ordering::Relaxed) + 1).is_multiple_of(2)
    }
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
