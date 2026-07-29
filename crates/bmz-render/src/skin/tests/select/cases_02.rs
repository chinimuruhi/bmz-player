use super::*;

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
