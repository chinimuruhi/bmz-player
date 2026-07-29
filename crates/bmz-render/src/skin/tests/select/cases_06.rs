use super::*;

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
