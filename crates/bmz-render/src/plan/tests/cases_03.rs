use super::*;

#[test]
fn play_skin_document_places_hit_timing_note_bottom_on_judge_line() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{"id": 1, "path": "note.png"}],
                "image": [{"id": "note", "src": 1, "x": 0, "y": 0, "w": 1, "h": 36}],
                "note": {
                    "note": ["note"],
                    "dst": [
                        { "x": 10, "y": 20, "w": 5, "h": 60 }
                    ]
                }
            }
            "#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let source_texture = crate::skin::SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: SkinTextureId(78),
        source_size: crate::skin::SkinImageSize { width: 1.0, height: 1.0 },
    };
    let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
    let mut snapshot = RenderSnapshot::default();
    snapshot.visible_notes[Lane::Key1.index()].push(VisibleNote {
        lane: Lane::Key1,
        time: TimeUs(1_000),
        y: 0.0,
        kind: NoteVisualKind::Tap,
        processed_judge: None,
    });

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, rect, .. }
            if *texture == TextureId(78)
                && approx_eq(rect.y + rect.height, 0.8)
                && approx_eq(rect.height, 0.36)
    )));
}

#[test]
fn skin_lane_height_uses_document_note_area_for_lane_cover_offsets() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 1920,
                "h": 1080,
                "note": {
                    "dst": [
                        { "x": 100, "y": 357, "w": 10, "h": 723 },
                        { "x": 110, "y": 357, "w": 10, "h": 723 },
                        { "x": 120, "y": 357, "w": 10, "h": 723 },
                        { "x": 130, "y": 357, "w": 10, "h": 723 },
                        { "x": 140, "y": 357, "w": 10, "h": 723 },
                        { "x": 150, "y": 357, "w": 10, "h": 723 },
                        { "x": 160, "y": 357, "w": 10, "h": 723 },
                        { "x": 170, "y": 357, "w": 10, "h": 723 }
                    ]
                }
            }
            "#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let skin = SkinContext::from_manifest_and_document(manifest, document, []);

    assert!(approx_eq(skin_lane_height_px(&skin, KeyMode::K7, 1080.0), 723.0));
}

#[test]
fn play_skin_lift_offsets_use_lane_height() {
    let lane_h = 723.0;

    assert_eq!(skin_lift_offset_px(0.3, lane_h), 217);
    assert_eq!(skin_lanecover_offset_px(0.5, 0.0, lane_h), -362);
    assert_eq!(skin_lanecover_offset_px(0.5, 0.25, lane_h), -362);
    assert!(approx_eq(lane_cover_bottom_progress(0.25, 0.0), 0.75));
    assert!(approx_eq(lane_cover_bottom_progress(0.25, 0.2), 0.6875));
    assert!(approx_eq(lane_cover_bottom_progress(0.9, 0.2), 0.0));
    assert_eq!(skin_hidden_cover_offset_px(0.3, 0.25, lane_h), 127);
}

#[test]
fn play_skin_ready_timer_starts_after_load_timers() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"{
                "type": 7,
                "w": 100,
                "h": 100,
                "loadstart": 500,
                "loadend": 3000,
                "source": [{"id": 1, "path": "panel.png"}],
                "image": [{"id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10}],
                "destination": [
                    {"id": "panel", "op": [80], "dst": [
                        {"time": 0, "x": 80, "y": 0, "w": 10, "h": 10}
                    ]},
                    {"id": "panel", "timer": 40, "dst": [
                        {"time": 0, "x": 0, "y": 0, "w": 10, "h": 10},
                        {"time": 1000, "x": 50, "y": 0, "w": 10, "h": 10}
                    ]}
                ]
            }"#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let source_texture = crate::skin::SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: SkinTextureId(99),
        source_size: crate::skin::SkinImageSize { width: 10.0, height: 10.0 },
    };
    let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
    let before_ready = RenderSnapshot {
        time: TimeUs(-1_000_000),
        play_elapsed_time: TimeUs(3_000_000),
        ready_elapsed_time: None,
        ..Default::default()
    };
    let after_ready = RenderSnapshot {
        time: TimeUs(-1_000_000),
        play_elapsed_time: TimeUs(4_000_000),
        ready_elapsed_time: Some(TimeUs(500_000)),
        resources_loaded: true,
        ..Default::default()
    };

    let mut dynamic_timers = crate::skin::DynamicTimerRuntime::default();
    let before_plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(before_ready),
        &skin,
        &mut dynamic_timers,
    );
    let after_plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(after_ready),
        &skin,
        &mut dynamic_timers,
    );

    assert!(before_plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, rect, .. }
            if *texture == TextureId(99) && approx_eq(rect.x, 0.8)
    )));
    assert!(after_plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, rect, .. }
            if *texture == TextureId(99) && approx_eq(rect.x, 0.25)
    )));
    assert!(!after_plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, rect, .. }
            if *texture == TextureId(99) && approx_eq(rect.x, 0.8)
    )));
}

#[test]
fn play_skin_stays_loading_after_load_delay_until_ready_timer_starts() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"{
                "type": 7,
                "w": 100,
                "h": 100,
                "loadstart": 500,
                "loadend": 3000,
                "source": [{"id": 1, "path": "panel.png"}],
                "image": [{"id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10}],
                "destination": [
                    {"id": "panel", "op": [80], "dst": [
                        {"time": 0, "x": 80, "y": 0, "w": 10, "h": 10}
                    ]},
                    {"id": "panel", "op": [81], "dst": [
                        {"time": 0, "x": 20, "y": 0, "w": 10, "h": 10}
                    ]}
                ]
            }"#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let source_texture = crate::skin::SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: SkinTextureId(99),
        source_size: crate::skin::SkinImageSize { width: 10.0, height: 10.0 },
    };
    let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
    let loaded_before_ready = RenderSnapshot {
        time: TimeUs(-1_000_000),
        play_elapsed_time: TimeUs(3_500_000),
        ready_elapsed_time: None,
        resources_loaded: true,
        ..Default::default()
    };

    let mut dynamic_timers = crate::skin::DynamicTimerRuntime::default();
    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(loaded_before_ready),
        &skin,
        &mut dynamic_timers,
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, rect, .. }
            if *texture == TextureId(99) && approx_eq(rect.x, 0.8)
    )));
    assert!(!plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, rect, .. }
            if *texture == TextureId(99) && approx_eq(rect.x, 0.2)
    )));
}

#[test]
fn play_skin_untimed_intro_uses_scene_elapsed_without_loadend_offset() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"{
                "type": 7,
                "w": 100,
                "h": 100,
                "loadend": 3000,
                "source": [{"id": 1, "path": "panel.png"}],
                "image": [{"id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10}],
                "destination": [{"id": "panel", "loop": 1600, "dst": [
                    {"time": 1400, "x": 0, "y": 0, "w": 10, "h": 10, "a": 0},
                    {"time": 1600, "a": 255}
                ]}]
            }"#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let source_texture = crate::skin::SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: SkinTextureId(99),
        source_size: crate::skin::SkinImageSize { width: 10.0, height: 10.0 },
    };
    let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
    let before_intro = RenderSnapshot {
        time: TimeUs(-1_000_000),
        play_elapsed_time: TimeUs(0),
        ..Default::default()
    };
    let during_intro = RenderSnapshot {
        time: TimeUs(-1_000_000),
        play_elapsed_time: TimeUs(1_500_000),
        ..Default::default()
    };

    let mut dynamic_timers = crate::skin::DynamicTimerRuntime::default();
    let before_plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(before_intro),
        &skin,
        &mut dynamic_timers,
    );
    let during_plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(during_intro),
        &skin,
        &mut dynamic_timers,
    );

    assert!(!before_plan.commands.iter().any(
        |command| matches!(command, DrawCommand::Image { texture, .. } if *texture == TextureId(99))
    ));
    assert!(during_plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, tint, .. }
            if *texture == TextureId(99) && approx_eq(tint.a, 128.0 / 255.0)
    )));
}

#[test]
fn play_skin_play_timer_is_inactive_before_chart_start() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"{
                "type": 7,
                "w": 100,
                "h": 100,
                "source": [{"id": 1, "path": "panel.png"}],
                "image": [{"id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10}],
                "destination": [{"id": "panel", "timer": 41, "dst": [
                    {"time": 0, "x": 0, "y": 0, "w": 10, "h": 10}
                ]}]
            }"#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let source_texture = crate::skin::SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: SkinTextureId(99),
        source_size: crate::skin::SkinImageSize { width: 10.0, height: 10.0 },
    };
    let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
    let before_start = RenderSnapshot {
        time: TimeUs(-1),
        play_elapsed_time: TimeUs(500_000),
        ..Default::default()
    };
    let after_start = RenderSnapshot {
        time: TimeUs(0),
        play_elapsed_time: TimeUs(500_000),
        ..Default::default()
    };

    let mut dynamic_timers = crate::skin::DynamicTimerRuntime::default();
    let before_plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(before_start),
        &skin,
        &mut dynamic_timers,
    );
    let after_plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(after_start),
        &skin,
        &mut dynamic_timers,
    );

    assert!(!before_plan.commands.iter().any(
        |command| matches!(command, DrawCommand::Image { texture, .. } if *texture == TextureId(99))
    ));
    assert!(after_plan.commands.iter().any(
        |command| matches!(command, DrawCommand::Image { texture, .. } if *texture == TextureId(99))
    ));
}

#[test]
fn play_plan_maps_normalized_note_y_to_distinct_screen_positions() {
    let mut snapshot = RenderSnapshot::default();
    snapshot.visible_notes[Lane::Key1.index()].push(VisibleNote {
        lane: Lane::Key1,
        time: TimeUs(1_000),
        y: 0.75,
        kind: NoteVisualKind::Tap,
        processed_judge: None,
    });
    snapshot.visible_notes[Lane::Key1.index()].push(VisibleNote {
        lane: Lane::Key1,
        time: TimeUs(2_000),
        y: 0.25,
        kind: NoteVisualKind::Tap,
        processed_judge: None,
    });

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));
    let note_ys: Vec<f32> = plan
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Image { rect, texture, .. } if *texture == DEFAULT_NOTE_TEXTURE => {
                Some(rect.y)
            }
            _ => None,
        })
        .collect();

    assert!(note_ys.iter().any(|y| approx_eq(*y, 0.2255)));
    assert!(note_ys.iter().any(|y| approx_eq(*y, 0.6125)));
}

#[test]
fn play_plan_places_hit_timing_note_on_judge_line() {
    let board = Rect { x: 0.18, y: 0.05, width: 0.64, height: 0.9 };

    assert!(approx_eq(note_rect_y(board, 0.0, 0.0) + NOTE_HEIGHT, judge_line_y(board, 0.0)));
}

#[test]
fn start_overlay_label_covers_opening_window() {
    assert_eq!(start_overlay_label(TimeUs(0)), Some("READY"));
    assert_eq!(start_overlay_label(TimeUs(999_999)), Some("READY"));
    assert_eq!(start_overlay_label(TimeUs(1_000_000)), Some("GO"));
    assert_eq!(start_overlay_label(TimeUs(1_599_999)), Some("GO"));
    assert_eq!(start_overlay_label(TimeUs(1_600_000)), None);
}

#[test]
fn play_plan_includes_ready_overlay_at_start() {
    let snapshot = RenderSnapshot { time: TimeUs(0), ..Default::default() };

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { style, .. } if style.color == Color::rgb(0.74, 0.88, 0.9)
    )));
}

#[test]
fn default_play_plan_includes_failed_overlay() {
    let snapshot = RenderSnapshot { failed_elapsed_ms: Some(500), ..Default::default() };

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "FAILED"
    )));
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Rect { color, .. } if color.a > 0.0
    )));
}

#[test]
fn select_plan_has_non_empty_commands() {
    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Select(Default::default()));

    assert!(!plan.commands.is_empty());
}

#[test]
fn decide_plan_activates_fadeout_timer_destinations() {
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
    let manifest: SkinManifest = SkinManifest::default();
    let skin = SkinContext::from_manifest_and_document(manifest, document, Vec::new());
    let mut dynamic_timers = crate::skin::DynamicTimerRuntime::default();

    let inactive = plan_decide(&RenderSnapshot::default(), &skin, &mut dynamic_timers);
    let active = plan_decide(
        &RenderSnapshot { fadeout_elapsed_ms: Some(100), ..RenderSnapshot::default() },
        &skin,
        &mut dynamic_timers,
    );

    assert!(!inactive.commands.iter().any(|command| {
        matches!(
            command,
            DrawCommand::Rect {
                rect: Rect { x, y, width, height },
                color: Color { r, g, b, a },
            } if approx_eq(*x, 0.0)
                && approx_eq(*y, 0.0)
                && approx_eq(*width, 1.0)
                && approx_eq(*height, 1.0)
                && approx_eq(*r, 0.0)
                && approx_eq(*g, 0.0)
                && approx_eq(*b, 0.0)
                && approx_eq(*a, 128.0 / 255.0)
        )
    }));
    assert!(active.commands.iter().any(|command| {
        matches!(
            command,
            DrawCommand::Rect {
                rect: Rect { x, y, width, height },
                color: Color { r, g, b, a },
            } if approx_eq(*x, 0.0)
                && approx_eq(*y, 0.0)
                && approx_eq(*width, 1.0)
                && approx_eq(*height, 1.0)
                && approx_eq(*r, 0.0)
                && approx_eq(*g, 0.0)
                && approx_eq(*b, 0.0)
                && approx_eq(*a, 128.0 / 255.0)
        )
    }));
}

#[test]
fn select_detail_panel_shows_gas_state() {
    let snapshot = crate::scene::SelectSnapshot {
        option_panel: 3,
        gauge_auto_shift: "BEST CLEAR".to_string(),
        ..Default::default()
    };

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Select(snapshot));

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "GAS      BEST CLEAR"
    )));
}

#[test]
fn custom_select_skin_does_not_force_stagefile_fullscreen_fallback() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "image": [{ "id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [{ "id": "panel", "dst": [{ "x": 10, "y": 10, "w": 10, "h": 10 }] }]
            }
            "#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let skin = SkinContext::from_manifest_and_document(
        manifest,
        document,
        [SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(1),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        }],
    );
    let mut dynamic_timers = crate::skin::DynamicTimerRuntime::default();
    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Select(crate::scene::SelectSnapshot {
            stage_background: true,
            stage_image_size: Some(SkinImageSize { width: 640.0, height: 480.0 }),
            ..Default::default()
        }),
        &skin,
        &mut dynamic_timers,
    );

    assert!(!plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, rect, .. }
            if *texture == SELECT_STAGE_TEXTURE
                && approx_eq(rect.x, 0.0)
                && approx_eq(rect.y, 0.0)
                && approx_eq(rect.width, 1.0)
                && approx_eq(rect.height, 1.0)
    )));
}
