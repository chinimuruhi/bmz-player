use super::*;

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
fn random_lane_refs_are_available_outside_result_screen() {
    let mut refs = [0; SKIN_RANDOM_LANE_REF_COUNT];
    refs[0] = 7;
    let state = SkinDrawState { random_lane_refs: refs, ..SkinDrawState::default() };

    assert_eq!(skin_state_imageset_index(450, &state), Some(7));
    assert_eq!(skin_state_event_index(450, &state), 7);
    assert_eq!(skin_state_number(450, &state), Some(7));
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
fn skin_image_index_number_result_favorite_ref_has_only_two_states() {
    let not_favorite =
        SkinDrawState { result_favorite_chart: Some(false), ..SkinDrawState::default() };
    assert_eq!(skin_image_index_number(90, &not_favorite), Some(0));

    let favorite = SkinDrawState { result_favorite_chart: Some(true), ..SkinDrawState::default() };
    assert_eq!(skin_image_index_number(90, &favorite), Some(1));
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
