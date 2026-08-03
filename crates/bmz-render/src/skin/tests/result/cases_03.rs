use super::*;

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
fn skin_state_text_course_table_requires_value_expr() {
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
    assert_eq!(skin_state_text(&by_id, &state), "");

    let course_state =
        SkinTextState { course_stage: Some(CourseStageMarker::Stage1), ..state.clone() };
    assert_eq!(skin_state_text(&by_id, &course_state), "");

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
