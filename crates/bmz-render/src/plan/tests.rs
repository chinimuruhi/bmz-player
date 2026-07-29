use bmz_core::judge::{Judge, TimingSide};
use bmz_core::lane::Lane;
use bmz_core::time::TimeUs;

use crate::skin::{SkinDocument, SkinDocumentTexture, SkinImageSize, SkinTextureId};
use crate::snapshot::{
    DisplayInput, DisplayJudgeCounts, DisplayJudgement, LongBodyState, NoteVisualKind,
    RenderSnapshot, VisibleBarLine, VisibleLongNote, VisibleNote,
};

use super::*;

#[test]
fn skin_level_number_extracts_digits_without_requiring_numeric_label() {
    assert_eq!(skin_level_number("12"), 12);
    assert_eq!(skin_level_number("LV 10+"), 10);
    assert_eq!(skin_level_number("no level"), 0);
}

#[test]
fn skin_difficulty_code_matches_numeric_and_case_insensitive_names() {
    assert_eq!(skin_difficulty_code("1"), 1);
    assert_eq!(skin_difficulty_code(" hyper "), 3);
    assert_eq!(skin_difficulty_code("ANOTHER"), 4);
    assert_eq!(skin_difficulty_code("unknown"), 0);
}

#[test]
fn lr2_judgetimer_limits_bomb_judgements() {
    assert!(judge_starts_bomb(Some(0), 1));
    assert!(judge_starts_bomb(Some(1), 1));
    assert!(!judge_starts_bomb(Some(2), 1));
    assert!(judge_starts_bomb(Some(3), 3));
    assert!(!judge_starts_bomb(None, 3));
}

#[test]
fn play_plan_renders_long_note_body() {
    let mut snapshot = RenderSnapshot::default();
    snapshot.visible_long_notes.push(VisibleLongNote {
        lane: Lane::Key4,
        mode: bmz_chart::model::LongNoteMode::Ln,
        head_y: 0.1,
        tail_y: 0.7,
        body_state: LongBodyState::Inactive,
    });

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    // 胴体は LONG_NOTE_BODY_COLOR の Rect。終端(tail)が始端(head)より画面上方にある。
    let body = plan.commands.iter().find_map(|command| match command {
        DrawCommand::Rect { rect, color } if *color == LONG_NOTE_BODY_COLOR => Some(*rect),
        _ => None,
    });
    let body = body.expect("long note body rect should be present");
    assert!(body.height > NOTE_HEIGHT, "body should be taller than a tap note");
    let board = Rect { x: 0.18, y: 0.05, width: 0.64, height: 0.9 };
    assert!(approx_eq(body.y, play_object_y(board, 0.0, 0.7)));
    assert!(approx_eq(body.y + body.height, play_object_y(board, 0.0, 0.1)));
}

#[test]
fn play_plan_colors_long_note_body_by_mode() {
    let mut snapshot = RenderSnapshot::default();
    snapshot.visible_long_notes.push(VisibleLongNote {
        lane: Lane::Key4,
        mode: LongNoteMode::Cn,
        head_y: 0.1,
        tail_y: 0.7,
        body_state: LongBodyState::Inactive,
    });
    snapshot.visible_long_notes.push(VisibleLongNote {
        lane: Lane::Key6,
        mode: LongNoteMode::Hcn,
        head_y: 0.1,
        tail_y: 0.7,
        body_state: LongBodyState::Inactive,
    });

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    assert!(plan.commands.iter().any(
        |command| matches!(command, DrawCommand::Rect { color, .. } if *color == CN_BODY_COLOR)
    ));
    assert!(plan.commands.iter().any(
        |command| matches!(command, DrawCommand::Rect { color, .. } if *color == HCN_BODY_COLOR)
    ));
}

#[test]
fn play_plan_includes_lanes_notes_and_bar_lines() {
    let mut snapshot = RenderSnapshot::default();
    snapshot.visible_notes[Lane::Key1.index()].push(VisibleNote {
        lane: Lane::Key1,
        time: TimeUs(1_000),
        y: 0.5,
        kind: NoteVisualKind::Tap,
        processed_judge: None,
    });
    snapshot.bar_lines.push(VisibleBarLine { time: TimeUs(900), y: 0.25 });

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    assert!(plan.commands.len() >= LANE_COUNT + 3);
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, tint, .. }
            if *texture == DEFAULT_NOTE_TEXTURE && *tint == skin_image_tint(Lane::Key1)
    )));
}

#[test]
fn play_plan_uses_note_textures_by_lane() {
    let mut snapshot = RenderSnapshot::default();
    snapshot.visible_notes[Lane::Scratch.index()].push(VisibleNote {
        lane: Lane::Scratch,
        time: TimeUs(1_000),
        y: 0.5,
        kind: NoteVisualKind::Tap,
        processed_judge: None,
    });
    snapshot.visible_notes[Lane::Key1.index()].push(VisibleNote {
        lane: Lane::Key1,
        time: TimeUs(1_000),
        y: 0.5,
        kind: NoteVisualKind::Tap,
        processed_judge: None,
    });
    snapshot.visible_notes[Lane::Key2.index()].push(VisibleNote {
        lane: Lane::Key2,
        time: TimeUs(1_000),
        y: 0.5,
        kind: NoteVisualKind::Tap,
        processed_judge: None,
    });

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, .. } if *texture == DEFAULT_SCRATCH_NOTE_TEXTURE
    )));
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, .. } if *texture == DEFAULT_NOTE_TEXTURE
    )));
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, .. } if *texture == DEFAULT_KEY_EVEN_NOTE_TEXTURE
    )));
}

#[test]
fn result_plan_uses_skin_document_for_result_and_course_result_types() {
    use crate::scene::ResultSnapshot;
    use crate::skin::SkinTextureId;
    use crate::snapshot::{FastSlowJudgeCounts, ResultGaugeGraphPoint, ResultGraphSnapshot};
    use bmz_core::clear::ClearType;

    for skin_type in [7, 15] {
        let json = r#"{
                "type": __TYPE__,
                "name": "test",
                "w": 100,
                "h": 100,
                "source": [{"id": 1, "path": "x.png"}],
                "image": [{"id": "logo", "src": 1, "x": 0, "y": 0, "w": 8, "h": 8}],
                "gaugegraph": [{"id": "graph"}],
                "destination": [
                    {"id": "logo", "dst": [{"x": 0, "y": 0, "w": 8, "h": 8}]},
                    {"id": "graph", "dst": [{"x": 0, "y": 8, "w": 100, "h": 80}]}
                ]
            }"#
        .replace("__TYPE__", &skin_type.to_string());
        let document: crate::skin::SkinDocument = serde_json::from_str(&json).unwrap();
        let manifest: SkinManifest = SkinManifest::default();
        let source_texture = crate::skin::SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(99),
            source_size: crate::skin::SkinImageSize { width: 64.0, height: 64.0 },
        };
        let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
        let snapshot = ResultSnapshot {
            player_name: String::new(),
            target_name: String::new(),
            current_fps: 0,
            skin_input: Default::default(),
            skin_offsets: Default::default(),
            hispeed_auto_adjust: false,
            clear_type: ClearType::Normal,
            result_failed: false,
            arrange: "NORMAL".to_string(),
            arrange_2p: "NORMAL".to_string(),
            double_option: "OFF".to_string(),
            lane_shuffle_pattern: Vec::new(),
            ex_score: 100,
            ex_score_rate: 0.5,
            max_combo: 50,
            bp: 0,
            cb: 0,
            gauge_value: 80.0,
            gauge_type: bmz_core::clear::GaugeType::Normal as i32,
            total_notes: 100,
            grade_diff_display: crate::scene::ResultGradeDiffDisplay::default(),
            duration_ms: 0,
            note_display_duration_ms: None,
            initial_bpm: 0.0,
            min_bpm: 0.0,
            max_bpm: 0.0,
            main_bpm: 0.0,
            total_gauge: 0.0,
            judge_rank: None,
            key_mode: bmz_core::lane::KeyMode::default(),
            has_long_notes: false,
            ln_mode_index: 0,
            result_gauge_graph_type: bmz_core::clear::GaugeType::Normal as i32,
            result_panel: 0,
            favorite_chart: false,
            judge_counts: DisplayJudgeCounts::default(),
            fast_slow_counts: FastSlowJudgeCounts::default(),
            score_save_enabled: false,
            score_history_id: 0,
            replay_saved: false,
            replay_slots: [false; 4],
            saved_replay_slots: [false; 4],
            best_ex_score: None,
            best_clear_type: None,
            target_ex_score: None,
            best_max_combo: None,
            target_max_combo: None,
            best_bp: None,
            target_bp: None,
            previous_best_ex_score: None,
            previous_best_clear_type: None,
            previous_best_max_combo: None,
            previous_best_bp: None,
            target_clear_type: None,
            elapsed_time: TimeUs(1_500_000),
            fadeout_elapsed: None,
            title: String::new(),
            subtitle: String::new(),
            artist: String::new(),
            subartist: String::new(),
            genre: String::new(),
            difficulty_name: String::new(),
            play_level: String::new(),
            table_text_primary: String::new(),
            table_text_secondary: String::new(),
            table_text_fallback: String::new(),
            stagefile_background: false,
            stagefile_image_size: None,
            course_titles: Default::default(),
            course_result: Default::default(),
            graph: std::sync::Arc::new(ResultGraphSnapshot {
                gauge_points: vec![
                    ResultGaugeGraphPoint {
                        value: 20.0,
                        max: 100.0,
                        border: 80.0,
                        gauge_type: bmz_core::clear::GaugeType::Normal as i32,
                        ..Default::default()
                    },
                    ResultGaugeGraphPoint {
                        value: 80.0,
                        max: 100.0,
                        border: 80.0,
                        gauge_type: bmz_core::clear::GaugeType::Normal as i32,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            overlay: crate::snapshot::OverlaySnapshot::default(),
            ir: crate::scene::ResultIrSnapshot::default(),
            player_stats: crate::scene::PlayerStatsSnapshot::default(),
        };

        let plan = DrawPlan::from_scene_with_skin(
            &AppSceneSnapshot::Result(snapshot),
            &skin,
            &mut crate::skin::DynamicTimerRuntime::default(),
        );

        assert!(plan.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Image { texture, .. } if *texture == TextureId(99)
        )));
        assert!(
            plan.commands
                .iter()
                .any(|command| matches!(command, DrawCommand::RectBatch { cache: Some(_), .. }))
        );
    }
}

#[test]
fn result_snapshot_custom_offset_adjusts_destination_geometry_and_alpha() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 7,
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
    let skin = SkinContext::from_manifest_and_document(
        SkinManifest::default(),
        document,
        [SkinDocumentTexture {
            source_id: "src".to_string(),
            texture: SkinTextureId(99),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        }],
    );
    let AppSceneSnapshot::Result(mut snapshot) = crate::sample::sample_result_scene() else {
        panic!()
    };
    snapshot
        .skin_offsets
        .set(42, crate::skin_offset::SkinOffsetValue { x: 6, y: 8, w: 10, h: 12, r: 0, a: -50 });

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Result(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    let command = plan
            .commands
            .iter()
            .find(|command| {
                matches!(command, DrawCommand::Image { texture, .. } if *texture == TextureId(99))
            })
            .expect("custom result destination should render");
    let DrawCommand::Image { rect, tint, .. } = command else { unreachable!() };
    assert!((rect.x - 0.11).abs() < 0.0001);
    assert!((rect.y - 0.26).abs() < 0.0001);
    assert!((rect.width - 0.4).abs() < 0.0001);
    assert!((rect.height - 0.52).abs() < 0.0001);
    assert!((tint.a - 150.0 / 255.0).abs() < 0.0001);
}

#[test]
fn course_result_plan_supplies_course_titles_to_skin_document() {
    let document: SkinDocument = serde_json::from_str(
        r#"{
                "type": 15,
                "name": "test",
                "w": 100,
                "h": 100,
                "text": [{"id": "course1", "size": 10, "ref": 150}],
                "destination": [
                    {"id": "course1", "dst": [{"x": 0, "y": 0, "w": 100, "h": 10}]}
                ]
            }"#,
    )
    .unwrap();
    let skin = SkinContext::from_manifest_and_document(
        SkinManifest::default(),
        document,
        std::iter::empty(),
    );
    let AppSceneSnapshot::Result(mut snapshot) = crate::sample::sample_result_scene() else {
        panic!("sample result scene");
    };
    snapshot.course_titles[0] = "Stage One".to_string();

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Result(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(
        plan.commands.iter().any(
            |command| matches!(command, DrawCommand::Text { text, .. } if text == "Stage One")
        )
    );
}

#[test]
fn result_plan_supplies_result_judge_graph_data_to_skin_document() {
    use crate::scene::ResultSnapshot;
    use crate::snapshot::{FastSlowJudgeCounts, ResultGraphSnapshot, ResultJudgeGraphBucket};
    use bmz_core::clear::ClearType;

    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"{
                "type": 7,
                "name": "test",
                "w": 100,
                "h": 100,
                "judgegraph": [{"id": "jg", "type": 1, "backTexOff": 1, "noGap": 1}],
                "destination": [
                    {"id": "jg", "dst": [{"x": 0, "y": 0, "w": 100, "h": 50}]}
                ]
            }"#,
    )
    .unwrap();
    let skin = SkinContext::from_manifest_and_document(
        SkinManifest::default(),
        document,
        std::iter::empty(),
    );
    let snapshot = ResultSnapshot {
        player_name: String::new(),
        target_name: String::new(),
        current_fps: 0,
        skin_input: Default::default(),
        skin_offsets: Default::default(),
        hispeed_auto_adjust: false,
        clear_type: ClearType::Normal,
        result_failed: false,
        arrange: "NORMAL".to_string(),
        arrange_2p: "NORMAL".to_string(),
        double_option: "OFF".to_string(),
        lane_shuffle_pattern: Vec::new(),
        ex_score: 100,
        ex_score_rate: 0.5,
        max_combo: 50,
        bp: 0,
        cb: 0,
        gauge_value: 80.0,
        gauge_type: bmz_core::clear::GaugeType::Normal as i32,
        total_notes: 100,
        grade_diff_display: crate::scene::ResultGradeDiffDisplay::default(),
        duration_ms: 0,
        note_display_duration_ms: None,
        initial_bpm: 0.0,
        min_bpm: 0.0,
        max_bpm: 0.0,
        main_bpm: 0.0,
        total_gauge: 0.0,
        judge_rank: None,
        key_mode: bmz_core::lane::KeyMode::default(),
        has_long_notes: false,
        ln_mode_index: 0,
        result_gauge_graph_type: bmz_core::clear::GaugeType::Normal as i32,
        result_panel: 0,
        favorite_chart: false,
        judge_counts: DisplayJudgeCounts::default(),
        fast_slow_counts: FastSlowJudgeCounts::default(),
        score_save_enabled: false,
        score_history_id: 0,
        replay_saved: false,
        replay_slots: [false; 4],
        saved_replay_slots: [false; 4],
        best_ex_score: None,
        best_clear_type: None,
        target_ex_score: None,
        best_max_combo: None,
        target_max_combo: None,
        best_bp: None,
        target_bp: None,
        previous_best_ex_score: None,
        previous_best_clear_type: None,
        previous_best_max_combo: None,
        previous_best_bp: None,
        target_clear_type: None,
        elapsed_time: TimeUs(0),
        fadeout_elapsed: None,
        title: String::new(),
        subtitle: String::new(),
        artist: String::new(),
        subartist: String::new(),
        genre: String::new(),
        difficulty_name: String::new(),
        play_level: String::new(),
        table_text_primary: String::new(),
        table_text_secondary: String::new(),
        table_text_fallback: String::new(),
        stagefile_background: false,
        stagefile_image_size: None,
        course_titles: Default::default(),
        course_result: Default::default(),
        graph: std::sync::Arc::new(ResultGraphSnapshot {
            judge_graph_density: vec![1, 3, 2],
            judge_graph_buckets: vec![ResultJudgeGraphBucket { values: [0, 0, 1, 0, 0, 0] }],
            ..ResultGraphSnapshot::default()
        }),
        overlay: crate::snapshot::OverlaySnapshot::default(),
        ir: crate::scene::ResultIrSnapshot::default(),
        player_stats: crate::scene::PlayerStatsSnapshot::default(),
    };

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Result(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| {
        draw_command_has_rect_color(command, |Color { r, g, b, .. }| {
            (*r - 0.0).abs() < 0.01 && (*g - 1.0).abs() < 0.01 && (*b - 0.53).abs() < 0.01
        })
    }));
}

#[test]
fn result_skin_state_sets_beatoraja_result_timers() {
    let AppSceneSnapshot::Result(mut snapshot) = crate::sample::sample_result_scene() else {
        panic!("sample result scene");
    };
    snapshot.elapsed_time = TimeUs(500_000);

    let pending = build_result_skin_draw_state(&snapshot, 1000);
    assert_eq!(pending.result_graph_begin_ms, Some(500));
    assert_eq!(pending.result_graph_end_ms, Some(500));
    assert_eq!(pending.result_update_score_ms, None);

    snapshot.elapsed_time = TimeUs(1_500_000);
    let active = build_result_skin_draw_state(&snapshot, 1000);
    assert_eq!(active.result_update_score_ms, Some(500));

    let immediate = build_result_skin_draw_state(&snapshot, 0);
    assert_eq!(immediate.result_update_score_ms, Some(1500));
}

#[test]
fn result_skin_state_maps_arrange_option() {
    let AppSceneSnapshot::Result(mut snapshot) = crate::sample::sample_result_scene() else {
        panic!("sample result scene");
    };
    snapshot.arrange = "S-RANDOM-EX".to_string();
    snapshot.arrange_2p = "MF-RANDOM".to_string();
    snapshot.double_option = "FLIP".to_string();

    let state = build_result_skin_draw_state(&snapshot, 0);

    assert_eq!(state.select_arrange_index, 9);
    assert_eq!(state.select_arrange_2p_index, 2);
    assert_eq!(state.select_double_option_index, 1);
    assert_eq!(state.result_arrange_index, 9);
    assert_eq!(state.result_arrange_2p_index, 2);
    assert_eq!(state.select_extended_arrange_index, 9);
    assert_eq!(state.select_extended_arrange_2p_index, 11);
    assert_eq!(state.result_extended_arrange_index, 9);
    assert_eq!(state.result_extended_arrange_2p_index, 11);
}

#[test]
fn result_skin_state_maps_favorite_chart_as_two_state_index() {
    let AppSceneSnapshot::Result(mut snapshot) = crate::sample::sample_result_scene() else {
        panic!("sample result scene");
    };

    let not_favorite = build_result_skin_draw_state(&snapshot, 0);
    assert_eq!(not_favorite.result_favorite_chart, Some(false));

    snapshot.favorite_chart = true;
    let favorite = build_result_skin_draw_state(&snapshot, 0);
    assert_eq!(favorite.result_favorite_chart, Some(true));
}

#[test]
fn result_skin_state_maps_random_lane_pattern() {
    let AppSceneSnapshot::Result(mut snapshot) = crate::sample::sample_result_scene() else {
        panic!("sample result scene");
    };
    let mut pattern = (0..bmz_core::lane::LANE_COUNT as u8).collect::<Vec<_>>();
    pattern[bmz_core::lane::Lane::Key1.index()] = bmz_core::lane::Lane::Key7.index() as u8;
    snapshot.arrange = "RANDOM".to_string();
    snapshot.lane_shuffle_pattern = pattern;

    let state = build_result_skin_draw_state(&snapshot, 0);

    assert_eq!(state.result_arrange_index, 2);
    assert_eq!(state.random_lane_refs[0], 7);
}

#[test]
fn result_skin_state_maps_effective_long_note_state() {
    let AppSceneSnapshot::Result(mut snapshot) = crate::sample::sample_result_scene() else {
        panic!("sample result scene");
    };
    snapshot.has_long_notes = true;
    snapshot.ln_mode_index = 2;

    let state = build_result_skin_draw_state(&snapshot, 0);

    assert_eq!(state.result_has_long_notes, Some(true));
    assert_eq!(state.result_ln_mode_index, Some(2));
}

#[test]
fn result_skin_state_keeps_clear_failed_flag_separate_from_clear_type() {
    let AppSceneSnapshot::Result(mut snapshot) = crate::sample::sample_result_scene() else {
        panic!("sample result scene");
    };
    snapshot.clear_type = bmz_core::clear::ClearType::NoPlay;
    snapshot.result_failed = false;

    let state = build_result_skin_draw_state(&snapshot, 0);

    assert_eq!(state.select_clear_index, bmz_core::clear::ClearType::NoPlay as i64);
    assert_eq!(state.result_failed, Some(false));

    snapshot.result_failed = true;
    let failed_state = build_result_skin_draw_state(&snapshot, 0);

    assert_eq!(failed_state.select_clear_index, bmz_core::clear::ClearType::NoPlay as i64);
    assert_eq!(failed_state.result_failed, Some(true));
}

#[test]
fn result_skin_state_falls_back_to_timing_points_for_average_timing() {
    let AppSceneSnapshot::Result(mut snapshot) = crate::sample::sample_result_scene() else {
        panic!("sample result scene");
    };
    std::sync::Arc::make_mut(&mut snapshot.graph).timing_points = vec![
        crate::snapshot::ResultTimingPoint {
            time_ms: 0,
            delta_us: -12_000,
            judge: bmz_core::judge::Judge::Great,
        },
        crate::snapshot::ResultTimingPoint {
            time_ms: 1000,
            delta_us: 20_000,
            judge: bmz_core::judge::Judge::PGreat,
        },
    ];

    let state = build_result_skin_draw_state(&snapshot, 0);

    assert_eq!(state.average_timing_ms, Some(4.0));
    assert_eq!(state.average_duration_us, Some(998_032));
    assert_eq!(state.stddev_timing_ms, Some(16.0));
}

#[test]
fn result_average_duration_uses_absolute_deltas_and_unjudged_penalty() {
    let points = [
        crate::snapshot::ResultTimingPoint {
            time_ms: 0,
            delta_us: -10_000,
            judge: bmz_core::judge::Judge::Great,
        },
        crate::snapshot::ResultTimingPoint {
            time_ms: 1000,
            delta_us: 20_000,
            judge: bmz_core::judge::Judge::PGreat,
        },
    ];

    assert_eq!(result_average_duration_us(&points, 4), Some(507_500));
    assert_eq!(result_average_duration_us(&points, 0), None);
}

#[test]
fn result_display_gauge_uses_selected_graph_history_tail() {
    use crate::snapshot::ResultGaugeGraphPoint;
    use bmz_core::clear::GaugeType;

    let points = [
        ResultGaugeGraphPoint {
            time_ms: 0,
            value: 20.0,
            max: 100.0,
            border: 0.0,
            gauge_type: GaugeType::ExHard as i32,
        },
        ResultGaugeGraphPoint {
            time_ms: 1_000,
            value: 80.0,
            max: 100.0,
            border: 80.0,
            gauge_type: GaugeType::Normal as i32,
        },
        ResultGaugeGraphPoint {
            time_ms: 1_000,
            value: 42.0,
            max: 100.0,
            border: 0.0,
            gauge_type: GaugeType::ExHard as i32,
        },
    ];

    assert_eq!(
        result_display_gauge(&points, GaugeType::ExHard as i32, 80.0, GaugeType::Normal as i32,),
        (42.0, GaugeType::ExHard as i32, 100.0, 0.0)
    );
}

#[test]
fn result_display_gauge_falls_back_when_selected_history_is_missing() {
    use bmz_core::clear::GaugeType;

    assert_eq!(
        result_display_gauge(&[], GaugeType::ExHard as i32, 80.0, GaugeType::Normal as i32,),
        (80.0, GaugeType::Normal as i32, 100.0, 80.0)
    );
}

#[test]
fn result_plan_renders_gaugegraph_from_result_graph_data() {
    use crate::scene::ResultSnapshot;
    use crate::snapshot::{FastSlowJudgeCounts, ResultGaugeGraphPoint, ResultGraphSnapshot};
    use bmz_core::clear::ClearType;

    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"{
                "type": 7,
                "name": "test",
                "w": 100,
                "h": 100,
                "gaugegraph": [{
                    "id": "gg",
                    "color": [
                        "010101", "ff0000", "00ff00", "0000ff",
                        "010101", "010101", "010101", "010101",
                        "010101", "010101", "010101", "010101",
                        "010101", "010101", "010101", "010101",
                        "010101", "010101", "010101", "010101",
                        "010101", "010101", "010101", "010101"
                    ]
                }],
                "destination": [
                    {"id": "gg", "dst": [{"x": 0, "y": 0, "w": 100, "h": 100}]}
                ]
            }"#,
    )
    .unwrap();
    let skin = SkinContext::from_manifest_and_document(
        SkinManifest::default(),
        document,
        std::iter::empty(),
    );
    let snapshot = ResultSnapshot {
        player_name: String::new(),
        target_name: String::new(),
        current_fps: 0,
        skin_input: Default::default(),
        skin_offsets: Default::default(),
        hispeed_auto_adjust: false,
        clear_type: ClearType::Normal,
        result_failed: false,
        arrange: "NORMAL".to_string(),
        arrange_2p: "NORMAL".to_string(),
        double_option: "OFF".to_string(),
        lane_shuffle_pattern: Vec::new(),
        ex_score: 100,
        ex_score_rate: 0.5,
        max_combo: 50,
        bp: 0,
        cb: 0,
        gauge_value: 80.0,
        gauge_type: bmz_core::clear::GaugeType::Normal as i32,
        total_notes: 100,
        grade_diff_display: crate::scene::ResultGradeDiffDisplay::default(),
        duration_ms: 0,
        note_display_duration_ms: None,
        initial_bpm: 0.0,
        min_bpm: 0.0,
        max_bpm: 0.0,
        main_bpm: 0.0,
        total_gauge: 0.0,
        judge_rank: None,
        key_mode: bmz_core::lane::KeyMode::default(),
        has_long_notes: false,
        ln_mode_index: 0,
        result_gauge_graph_type: bmz_core::clear::GaugeType::AssistEasy as i32,
        result_panel: 0,
        favorite_chart: false,
        judge_counts: DisplayJudgeCounts::default(),
        fast_slow_counts: FastSlowJudgeCounts::default(),
        score_save_enabled: false,
        score_history_id: 0,
        replay_saved: false,
        replay_slots: [false; 4],
        saved_replay_slots: [false; 4],
        best_ex_score: None,
        best_clear_type: None,
        target_ex_score: None,
        best_max_combo: None,
        target_max_combo: None,
        best_bp: None,
        target_bp: None,
        previous_best_ex_score: None,
        previous_best_clear_type: None,
        previous_best_max_combo: None,
        previous_best_bp: None,
        target_clear_type: None,
        elapsed_time: TimeUs(2_000_000),
        fadeout_elapsed: None,
        title: String::new(),
        subtitle: String::new(),
        artist: String::new(),
        subartist: String::new(),
        genre: String::new(),
        difficulty_name: String::new(),
        play_level: String::new(),
        table_text_primary: String::new(),
        table_text_secondary: String::new(),
        table_text_fallback: String::new(),
        stagefile_background: false,
        stagefile_image_size: None,
        course_titles: Default::default(),
        course_result: Default::default(),
        graph: std::sync::Arc::new(ResultGraphSnapshot {
            gauge_points: vec![
                ResultGaugeGraphPoint {
                    time_ms: 0,
                    value: 20.0,
                    max: 100.0,
                    border: 60.0,
                    gauge_type: bmz_core::clear::GaugeType::AssistEasy as i32,
                },
                ResultGaugeGraphPoint {
                    time_ms: 1_000,
                    value: 90.0,
                    max: 100.0,
                    border: 60.0,
                    gauge_type: bmz_core::clear::GaugeType::AssistEasy as i32,
                },
            ],
            ..ResultGraphSnapshot::default()
        }),
        overlay: crate::snapshot::OverlaySnapshot::default(),
        ir: crate::scene::ResultIrSnapshot::default(),
        player_stats: crate::scene::PlayerStatsSnapshot::default(),
    };

    let draw_state = result_skin_draw_state(&snapshot, 0);
    assert_eq!(draw_state.gauge, 90.0);
    assert_eq!(draw_state.gauge_type, bmz_core::clear::GaugeType::AssistEasy as i32);
    assert_eq!(draw_state.gauge_max, 100.0);
    assert_eq!(draw_state.gauge_border, 60.0);

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Result(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| {
        draw_command_has_rect(command, |_, Color { r, g, b, .. }| {
            (*r - 0.0).abs() < 0.01 && (*g - 1.0).abs() < 0.01 && (*b - 0.0).abs() < 0.01
        })
    }));
    assert!(plan.commands.iter().any(|command| {
        draw_command_has_rect(command, |rect, Color { r, g, b, .. }| {
            (*r - 1.0).abs() < 0.01 && *g < 0.01 && *b < 0.01 && (rect.height - 0.4).abs() < 0.01
        })
    }));
}

#[test]
fn result_plan_renders_timing_distribution_from_result_graph_data() {
    use crate::scene::ResultSnapshot;
    use crate::snapshot::{
        FastSlowJudgeCounts, ResultGraphSnapshot, ResultTimingDistribution, ResultTimingPoint,
    };
    use bmz_core::clear::ClearType;
    use bmz_core::judge::Judge;

    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"{
                "type": 7,
                "name": "test",
                "w": 100,
                "h": 100,
                "timingdistributiongraph": [{"id": "td", "graphColor": "00FF00FF"}],
                "destination": [
                    {"id": "td", "dst": [{"x": 0, "y": 0, "w": 100, "h": 50}]}
                ]
            }"#,
    )
    .unwrap();
    let skin = SkinContext::from_manifest_and_document(
        SkinManifest::default(),
        document,
        std::iter::empty(),
    );
    let mut timing_distribution = ResultTimingDistribution::default();
    timing_distribution.add(-12);
    timing_distribution.add(8);
    let snapshot = ResultSnapshot {
        player_name: String::new(),
        target_name: String::new(),
        current_fps: 0,
        skin_input: Default::default(),
        skin_offsets: Default::default(),
        hispeed_auto_adjust: false,
        clear_type: ClearType::Normal,
        result_failed: false,
        arrange: "NORMAL".to_string(),
        arrange_2p: "NORMAL".to_string(),
        double_option: "OFF".to_string(),
        lane_shuffle_pattern: Vec::new(),
        ex_score: 100,
        ex_score_rate: 0.5,
        max_combo: 50,
        bp: 0,
        cb: 0,
        gauge_value: 80.0,
        gauge_type: bmz_core::clear::GaugeType::Normal as i32,
        total_notes: 100,
        grade_diff_display: crate::scene::ResultGradeDiffDisplay::default(),
        duration_ms: 0,
        note_display_duration_ms: None,
        initial_bpm: 0.0,
        min_bpm: 0.0,
        max_bpm: 0.0,
        main_bpm: 0.0,
        total_gauge: 0.0,
        judge_rank: None,
        key_mode: bmz_core::lane::KeyMode::default(),
        has_long_notes: false,
        ln_mode_index: 0,
        result_gauge_graph_type: bmz_core::clear::GaugeType::Normal as i32,
        result_panel: 0,
        favorite_chart: false,
        judge_counts: DisplayJudgeCounts::default(),
        fast_slow_counts: FastSlowJudgeCounts::default(),
        score_save_enabled: false,
        score_history_id: 0,
        replay_saved: false,
        replay_slots: [false; 4],
        saved_replay_slots: [false; 4],
        best_ex_score: None,
        best_clear_type: None,
        target_ex_score: None,
        best_max_combo: None,
        target_max_combo: None,
        best_bp: None,
        target_bp: None,
        previous_best_ex_score: None,
        previous_best_clear_type: None,
        previous_best_max_combo: None,
        previous_best_bp: None,
        target_clear_type: None,
        elapsed_time: TimeUs(0),
        fadeout_elapsed: None,
        title: String::new(),
        subtitle: String::new(),
        artist: String::new(),
        subartist: String::new(),
        genre: String::new(),
        difficulty_name: String::new(),
        play_level: String::new(),
        table_text_primary: String::new(),
        table_text_secondary: String::new(),
        table_text_fallback: String::new(),
        stagefile_background: false,
        stagefile_image_size: None,
        course_titles: Default::default(),
        course_result: Default::default(),
        graph: std::sync::Arc::new(ResultGraphSnapshot {
            timing_distribution,
            timing_points: vec![
                ResultTimingPoint { time_ms: 0, delta_us: -12_000, judge: Judge::Great },
                ResultTimingPoint { time_ms: 100, delta_us: 8_000, judge: Judge::PGreat },
            ],
            ..ResultGraphSnapshot::default()
        }),
        overlay: crate::snapshot::OverlaySnapshot::default(),
        ir: crate::scene::ResultIrSnapshot::default(),
        player_stats: crate::scene::PlayerStatsSnapshot::default(),
    };

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Result(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Rect { color: Color { r, g, b, .. }, .. }
            if (*r - 0.0).abs() < 0.01 && (*g - 1.0).abs() < 0.01 && (*b - 0.0).abs() < 0.01
    )));
}

#[test]
fn play_plan_uses_supplied_skin_context() {
    let manifest = SkinManifest {
        play: crate::skin::SkinPlayManifest {
            note: Some(crate::skin::SkinImageManifest {
                texture: 42,
                key_even_texture: None,
                scratch_texture: None,
                source_size: None,
                uv: crate::skin::TextureRegion::default(),
                scale: crate::skin::SkinImageScale::Stretch,
                border: None,
            }),
            ..crate::skin::SkinPlayManifest::default()
        },
        ..SkinManifest::default()
    };
    let skin = SkinContext::from_manifest(manifest);
    let mut snapshot = RenderSnapshot::default();
    snapshot.visible_notes[Lane::Key1.index()].push(VisibleNote {
        lane: Lane::Key1,
        time: TimeUs(1_000),
        y: 0.5,
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
        DrawCommand::Image { texture, .. } if *texture == TextureId(42)
    )));
}

#[test]
fn play_skin_document_receives_target_text() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "text": [{ "id": "target", "size": 12, "ref": 1 }],
                "destination": [
                    { "id": "target", "dst": [{ "x": 10, "y": 20, "w": 60, "h": 12 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let skin = SkinContext::from_manifest_and_document(SkinManifest::default(), document, []);
    let snapshot = RenderSnapshot { target: "IR_TOP".to_string(), ..RenderSnapshot::default() };

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "IR TOP"
    )));
}

#[test]
fn play_skin_document_renders_bar_lines_in_note_area() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{"id": 1, "path": "line.png"}],
                "image": [{"id": "section-line", "src": 1, "x": 0, "y": 0, "w": 1, "h": 1}],
                "note": {
                    "dst": [
                        { "x": 10, "y": 20, "w": 5, "h": 60 },
                        { "x": 15, "y": 20, "w": 5, "h": 60 },
                        { "x": 20, "y": 20, "w": 5, "h": 60 },
                        { "x": 25, "y": 20, "w": 5, "h": 60 },
                        { "x": 30, "y": 20, "w": 5, "h": 60 },
                        { "x": 35, "y": 20, "w": 5, "h": 60 },
                        { "x": 40, "y": 20, "w": 5, "h": 60 },
                        { "x": 45, "y": 20, "w": 5, "h": 60 }
                    ],
                    "group": [
                        {
                            "id": "section-line",
                            "dst": [
                                { "x": 10, "y": 25, "w": 40, "h": 2, "r": 64, "g": 128, "b": 255, "a": 200 }
                            ]
                        }
                    ]
                }
            }
            "#,
        )
        .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let source_texture = crate::skin::SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: SkinTextureId(77),
        source_size: crate::skin::SkinImageSize { width: 1.0, height: 1.0 },
    };
    let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
    let mut snapshot = RenderSnapshot::default();
    snapshot.bar_lines.push(VisibleBarLine { time: TimeUs(1_000), y: 0.5 });

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, rect, tint, .. }
            if *texture == TextureId(77)
                && approx_eq(rect.x, 0.1)
                && approx_eq(rect.y + rect.height, 0.45)
                && approx_eq(rect.width, 0.4)
                && approx_eq(rect.height, 0.02)
                && approx_eq(tint.r, 64.0 / 255.0)
                && approx_eq(tint.g, 128.0 / 255.0)
                && approx_eq(tint.b, 1.0)
                && approx_eq(tint.a, 200.0 / 255.0)
    )));
}

#[test]
fn play_skin_document_moves_bar_lines_in_same_direction_as_notes() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{"id": 1, "path": "line.png"}],
                "image": [
                    {"id": "note", "src": 1, "x": 0, "y": 0, "w": 1, "h": 1},
                    {"id": "section-line", "src": 1, "x": 0, "y": 0, "w": 1, "h": 1}
                ],
                "note": {
                    "id": "notes",
                    "note": ["note", "note", "note", "note", "note", "note", "note", "note"],
                    "dst": [{ "x": 10, "y": 20, "w": 40, "h": 60 }],
                    "group": [{
                        "id": "section-line",
                        "dst": [{ "x": 10, "y": 20, "w": 40, "h": 2 }]
                    }]
                }
            }
            "#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let source_texture = crate::skin::SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: SkinTextureId(77),
        source_size: crate::skin::SkinImageSize { width: 1.0, height: 1.0 },
    };
    let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
    let note_height = skin.document_note_height(Lane::Key1, KeyMode::K7).unwrap();
    let state = crate::skin::SkinDrawState::default();
    let early_note =
        skin.note_rect_for_progress(Lane::Key1, KeyMode::K7, 0.5, note_height, &state).unwrap();
    let later_note =
        skin.note_rect_for_progress(Lane::Key1, KeyMode::K7, 0.25, note_height, &state).unwrap();

    let bar_y = |progress| {
        let items = skin.document_bar_line_items(progress, KeyMode::K7, &state);
        let Some(SkinRenderItem::Image { rect, .. }) = items.first() else { panic!() };
        rect.y
    };
    let early_bar_y = bar_y(0.5);
    let later_bar_y = bar_y(0.25);

    assert!(later_note.y > early_note.y);
    assert!(later_bar_y > early_bar_y);
}

#[test]
fn play_skin_document_applies_bar_line_offset_height_and_alpha() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{"id": 1, "path": "line.png"}],
                "image": [{"id": "section-line", "src": 1, "x": 0, "y": 0, "w": 1, "h": 1}],
                "note": {
                    "dst": [{ "x": 10, "y": 20, "w": 5, "h": 60 }],
                    "group": [{
                        "id": "section-line",
                        "dst": [{ "x": 10, "y": 20, "w": 40, "h": 2, "a": 200 }]
                    }]
                }
            }
            "#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let source_texture = crate::skin::SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: SkinTextureId(77),
        source_size: crate::skin::SkinImageSize { width: 1.0, height: 1.0 },
    };
    let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
    let mut snapshot = RenderSnapshot::default();
    snapshot.skin_offsets.set(
        SKIN_OFFSET_BAR_LINE,
        crate::skin_offset::SkinOffsetValue { h: 3, a: -50, ..Default::default() },
    );
    snapshot.bar_lines.push(VisibleBarLine { time: TimeUs(1_000), y: 0.5 });

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, rect, tint, .. }
            if *texture == TextureId(77)
                && approx_eq(rect.height, 0.05)
                && approx_eq(tint.a, 150.0 / 255.0)
    )));
}

#[test]
fn default_play_bar_line_applies_height_and_alpha_offset() {
    let mut snapshot = RenderSnapshot::default();
    snapshot.skin_offsets.set(
        SKIN_OFFSET_BAR_LINE,
        crate::skin_offset::SkinOffsetValue { h: 4, a: -128, ..Default::default() },
    );
    snapshot.bar_lines.push(VisibleBarLine { time: TimeUs(1_000), y: 0.5 });

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Rect { rect, color }
            if approx_eq(rect.height, 0.004 + 4.0 / 1080.0)
                && approx_eq(color.a, 127.0 / 255.0)
    )));
}

#[test]
fn play_skin_document_ignores_notes_offset_on_bar_lines() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{"id": 1, "path": "line.png"}],
                "image": [{"id": "section-line", "src": 1, "x": 0, "y": 0, "w": 1, "h": 1}],
                "note": {
                    "dst": [{ "x": 10, "y": 20, "w": 5, "h": 60 }],
                    "group": [{
                        "id": "section-line",
                        "offset": 30,
                        "dst": [{ "x": 10, "y": 20, "w": 40, "h": 2, "a": 200 }]
                    }]
                }
            }
            "#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let source_texture = crate::skin::SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: SkinTextureId(77),
        source_size: crate::skin::SkinImageSize { width: 1.0, height: 1.0 },
    };
    let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
    let mut snapshot = RenderSnapshot::default();
    snapshot
        .skin_offsets
        .set(30, crate::skin_offset::SkinOffsetValue { h: 20, ..Default::default() });
    snapshot.skin_offsets.set(
        SKIN_OFFSET_BAR_LINE,
        crate::skin_offset::SkinOffsetValue { h: 5, a: -50, ..Default::default() },
    );
    snapshot.bar_lines.push(VisibleBarLine { time: TimeUs(1_000), y: 0.5 });

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, rect, tint, .. }
            if *texture == TextureId(77)
                && approx_eq(rect.height, 0.07)
                && approx_eq(tint.a, 150.0 / 255.0)
    )));
}

#[test]
fn play_skin_document_without_group_does_not_fallback_to_bar_line_rect() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "note": {
                    "dst": [{ "x": 10, "y": 20, "w": 5, "h": 60 }]
                }
            }
            "#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let skin = SkinContext::from_manifest_and_document(manifest, document, []);
    let mut snapshot = RenderSnapshot::default();
    snapshot.skin_offsets.set(
        SKIN_OFFSET_BAR_LINE,
        crate::skin_offset::SkinOffsetValue { h: 4, a: -128, ..Default::default() },
    );
    snapshot.bar_lines.push(VisibleBarLine { time: TimeUs(1_000), y: 0.5 });

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(
        !plan.commands.iter().any(|command| matches!(command, DrawCommand::Rect { .. })),
        "skin documents without note.group should not receive default bar line fallback"
    );
}

#[test]
fn play_skin_document_applies_bar_line_alpha_after_global_offset() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{"id": 1, "path": "line.png"}],
                "image": [{"id": "section-line", "src": 1, "x": 0, "y": 0, "w": 1, "h": 1}],
                "note": {
                    "dst": [{ "x": 10, "y": 20, "w": 5, "h": 60 }],
                    "group": [{
                        "id": "section-line",
                        "dst": [{ "x": 10, "y": 20, "w": 40, "h": 2, "a": 255 }]
                    }]
                }
            }
            "#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let source_texture = crate::skin::SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: SkinTextureId(77),
        source_size: crate::skin::SkinImageSize { width: 1.0, height: 1.0 },
    };
    let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
    let mut snapshot = RenderSnapshot::default();
    snapshot
        .skin_offsets
        .set(10, crate::skin_offset::SkinOffsetValue { w: 20, ..Default::default() });
    snapshot.skin_offsets.set(
        SKIN_OFFSET_BAR_LINE,
        crate::skin_offset::SkinOffsetValue { a: -64, ..Default::default() },
    );
    snapshot.bar_lines.push(VisibleBarLine { time: TimeUs(1_000), y: 0.5 });

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, tint, .. }
            if *texture == TextureId(77) && approx_eq(tint.a, 191.0 / 255.0)
    )));
}

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

#[test]
fn play_plan_passes_runtime_stagefile_to_skin_document() {
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
    let skin = SkinContext::from_manifest_and_document(SkinManifest::default(), document, []);
    let AppSceneSnapshot::Play(mut snapshot) = crate::sample::sample_play_scene() else {
        panic!("sample play scene");
    };
    snapshot.stagefile_background = true;
    snapshot.stagefile_image_size = Some(SkinImageSize { width: 400.0, height: 200.0 });

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, .. } if *texture == SELECT_STAGE_TEXTURE
    )));
}

#[test]
fn result_plan_passes_runtime_stagefile_to_skin_document() {
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
    let skin = SkinContext::from_manifest_and_document(SkinManifest::default(), document, []);
    let AppSceneSnapshot::Result(mut snapshot) = crate::sample::sample_result_scene() else {
        panic!("sample result scene");
    };
    snapshot.stagefile_background = true;
    snapshot.stagefile_image_size = Some(SkinImageSize { width: 400.0, height: 200.0 });

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Result(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, .. } if *texture == SELECT_STAGE_TEXTURE
    )));
}

#[test]
fn select_plan_renders_all_snapshot_rows() {
    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Select(crate::scene::SelectSnapshot {
        chart_count: 20,
        rows: select_rows(20),
        ..Default::default()
    }));

    let selected_row_color = Color::rgb(0.22, 0.28, 0.31);
    let row_color = Color::rgb(0.075, 0.09, 0.1);
    let row_count = plan
            .commands
            .iter()
            .filter(|command| matches!(
                command,
                DrawCommand::Rect { color, .. } if *color == selected_row_color || *color == row_color
            ))
            .count();
    assert_eq!(row_count, 20);
    assert!(plan.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Text { text, .. } if text.contains("DIFFICULTY NORMAL") && text.contains("LEVEL 0")
        )));
}

#[test]
fn select_plan_renders_empty_row_when_no_rows_are_available() {
    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Select(Default::default()));

    let selected_row_color = Color::rgb(0.22, 0.28, 0.31);
    let row_count = plan
            .commands
            .iter()
            .filter(|command| {
                matches!(command, DrawCommand::Rect { color, .. } if *color == selected_row_color)
            })
            .count();
    assert_eq!(row_count, 1);
}

#[test]
fn result_plan_clamps_ex_score_bar() {
    let judge_counts = DisplayJudgeCounts::default();
    let fast_slow_counts = FastSlowJudgeCounts::default();
    let graph = ResultGraphSnapshot::default();
    let plan = plan_result_fallback(ResultFallbackSummary {
        clear_type: "Normal",
        ex_score: 0,
        ex_score_rate: 1.5,
        max_combo: 0,
        gauge_value: 0.0,
        total_notes: 100,
        judge_counts: &judge_counts,
        fast_slow_counts: &fast_slow_counts,
        graph: &graph,
        score_history_id: 1,
        replay_saved: true,
        difficulty_name: "",
        play_level: "",
        grade_diff: String::new(),
        ir: &crate::scene::ResultIrSnapshot::default(),
    });

    assert!(plan.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Rect { rect, color } if rect.width == 0.72 && *color == Color::rgb(0.55, 0.78, 0.86)
        )));
}

#[test]
fn result_plan_includes_extended_summary_text() {
    let judge_counts = DisplayJudgeCounts::default();
    let fast_slow_counts = FastSlowJudgeCounts::default();
    let graph = ResultGraphSnapshot::default();
    let plan = plan_result_fallback(ResultFallbackSummary {
        clear_type: "Normal",
        ex_score: 1500,
        ex_score_rate: 0.75,
        max_combo: 500,
        gauge_value: 82.0,
        total_notes: 1000,
        judge_counts: &judge_counts,
        fast_slow_counts: &fast_slow_counts,
        graph: &graph,
        score_history_id: 42,
        replay_saved: true,
        difficulty_name: "HYPER",
        play_level: "10",
        grade_diff: "AA+56".to_string(),
        ir: &crate::scene::ResultIrSnapshot {
            state: crate::scene::ResultIrState::Loaded,
            rank: Some(3),
            total_player: Some(42),
            clear_rate: None,
            previous_rank: None,
            ..Default::default()
        },
    });

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { style, .. } if style.color == Color::rgb(0.72, 0.84, 0.86)
    )));
    assert!(plan.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Text { text, .. } if text.contains("DIFFICULTY HYPER") && text.contains("LEVEL 10")
        )));
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text.contains("GRADE AA+56")
    )));
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text.contains("IR RANK 3/42")
    )));
    assert_eq!(format_percent(0.754), "75%");
}

#[test]
fn result_plan_includes_stat_detail_panels() {
    let judge_counts =
        DisplayJudgeCounts { pgreat: 12, great: 8, good: 4, bad: 2, poor: 1, empty_poor: 3 };
    let fast_slow_counts = FastSlowJudgeCounts {
        fast_pgreat: 7,
        slow_pgreat: 5,
        fast_great: 3,
        slow_great: 5,
        fast_good: 1,
        slow_good: 3,
        fast_bad: 1,
        slow_bad: 1,
        fast_poor: 0,
        slow_poor: 1,
        fast_empty_poor: 2,
        slow_empty_poor: 1,
    };
    let graph = ResultGraphSnapshot {
        timing_points: vec![
            ResultTimingPoint {
                time_ms: 100,
                delta_us: -12_000,
                judge: bmz_core::judge::Judge::Great,
            },
            ResultTimingPoint {
                time_ms: 200,
                delta_us: 8_000,
                judge: bmz_core::judge::Judge::PGreat,
            },
        ],
        judge_graph_density: vec![1, 3, 2],
        ..ResultGraphSnapshot::default()
    };

    let plan = plan_result_fallback(ResultFallbackSummary {
        clear_type: "Normal",
        ex_score: 1500,
        ex_score_rate: 0.75,
        max_combo: 500,
        gauge_value: 82.0,
        total_notes: 1000,
        judge_counts: &judge_counts,
        fast_slow_counts: &fast_slow_counts,
        graph: &graph,
        score_history_id: 42,
        replay_saved: true,
        difficulty_name: "HYPER",
        play_level: "10",
        grade_diff: "AA+56".to_string(),
        ir: &crate::scene::ResultIrSnapshot::default(),
    });

    for label in ["JUDGE DETAILS", "FAST/SLOW DETAILS", "TIMING DETAILS"] {
        assert!(plan.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Text { text, .. } if text == label
        )));
    }
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text.starts_with("AVG ")
    )));
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "F 7  S 11"
    )));
}

#[test]
fn play_plan_includes_judge_line_gauge_and_combo_panel() {
    let snapshot = RenderSnapshot {
        combo: 1234,
        max_combo: 1234,
        ex_score: 2000,
        total_notes: 1200,
        past_notes: 900,
        gauge: 82.0,
        difficulty_name: "ANOTHER".to_string(),
        play_level: "12".to_string(),
        ..Default::default()
    };

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, tint, .. }
            if *texture == DEFAULT_JUDGE_LINE_TEXTURE && *tint == skin_image_tint(Lane::Key1)
    )));
    // デフォルトスキンではグルーブゲージを描画しない。
    assert!(!plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, .. } if *texture == DEFAULT_GAUGE_FRAME_TEXTURE
    )));
    assert!(!plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, .. } if *texture == DEFAULT_GAUGE_FILL_TEXTURE
    )));
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, tint, .. }
            if *texture == DEFAULT_COMBO_PANEL_TEXTURE && *tint == Color::rgb(1.0, 1.0, 1.0)
    )));
    assert_eq!(
        plan.commands
            .iter()
            .filter(|command| matches!(
                command,
                DrawCommand::Image { texture, .. } if *texture == DEFAULT_COMBO_PANEL_TEXTURE
            ))
            .count(),
        9
    );
    assert!(plan.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Rect { rect, color } if rect.x == 0.05 && rect.width == 0.11 && *color == Color::rgb(0.035, 0.04, 0.044)
        )));
    assert!(plan.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Rect { rect, color } if rect.x == 0.05 && rect.y == 0.36 && *color == Color::rgb(0.032, 0.036, 0.04)
        )));
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, tint, .. }
            if *texture == DEFAULT_KEY_EVEN_RECEPTOR_TEXTURE && *tint == skin_image_tint(Lane::Key2)
    )));
    assert!(plan.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Image { texture, tint, .. }
                if *texture == DEFAULT_SCRATCH_RECEPTOR_TEXTURE && *tint == skin_image_tint(Lane::Scratch)
        )));
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, style, .. }
            if text == "1234" && style.layer == TextLayer::Skin
    )));
    assert!(plan.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Text { text, .. } if text.contains("DIFFICULTY ANOTHER") && text.contains("LEVEL 12")
        )));
}

#[test]
fn play_plan_uses_snapshot_2p_arrange_for_skin_imageset() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "arrange.png" }],
                "image": [
                    { "id": "normal", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "mirror", "src": 1, "x": 10, "y": 0, "w": 10, "h": 10 },
                    { "id": "random", "src": 1, "x": 20, "y": 0, "w": 10, "h": 10 }
                ],
                "imageset": [
                    { "id": "arrange", "ref": 43, "images": ["normal", "mirror", "random"] }
                ],
                "destination": [
                    { "id": "arrange", "dst": [{ "time": 0, "x": 10, "y": 20, "w": 20, "h": 10 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let source_texture = crate::skin::SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: crate::skin::SkinTextureId(77),
        source_size: crate::skin::SkinImageSize { width: 30.0, height: 10.0 },
    };
    let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
    let snapshot = RenderSnapshot {
        arrange: "NORMAL".to_string(),
        arrange_2p: "RANDOM".to_string(),
        ..Default::default()
    };

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, uv, .. }
            if *texture == TextureId(77) && (uv.x - 20.0 / 30.0).abs() < 0.001
    )));
}

#[test]
fn play_plan_uses_beatoraja_target_list_index_for_skin_imageset() {
    for ref_id in [41, 77] {
        let document_json =
                r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "target.png" }],
                "image": [
                    { "id": "target", "src": 1, "x": 0, "y": 0, "w": 10, "h": 110, "divy": 11, "len": 11, "ref": REF_ID }
                ],
                "destination": [
                    { "id": "target", "dst": [{ "time": 0, "x": 10, "y": 20, "w": 20, "h": 10 }] }
                ]
            }
            "#
                .replace("REF_ID", &ref_id.to_string());
        let document: crate::skin::SkinDocument = serde_json::from_str(&document_json).unwrap();
        let source_texture = crate::skin::SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: crate::skin::SkinTextureId(78),
            source_size: crate::skin::SkinImageSize { width: 10.0, height: 110.0 },
        };
        let skin = SkinContext::from_manifest_and_document(
            SkinManifest::default(),
            document,
            [source_texture],
        );
        let snapshot = RenderSnapshot { target: "RANK_AAA".to_string(), ..Default::default() };

        let plan = DrawPlan::from_scene_with_skin(
            &AppSceneSnapshot::Play(snapshot),
            &skin,
            &mut crate::skin::DynamicTimerRuntime::default(),
        );

        // beatoraja の11段階では AAA は 7 番目。BMZ の選択肢に A+/AA+/AAA+
        // がなくても、その分を詰めずに元の画像行を選ぶ。
        assert!(
            plan.commands.iter().any(|command| matches!(
                command,
                DrawCommand::Image { texture, uv, .. }
                    if *texture == TextureId(78) && (uv.y - 70.0 / 110.0).abs() < 0.001
            )),
            "target ref {ref_id} must select the AAA row"
        );
    }
}

#[test]
fn play_plan_uses_snapshot_extended_2p_arrange_for_ref_image() {
    let document: crate::skin::SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "arrange.png" }],
                "image": [
                    { "id": "arrange", "src": 1, "x": 0, "y": 0, "w": 10, "h": 120, "divy": 12, "len": 12, "ref": 345 }
                ],
                "destination": [
                    { "id": "arrange", "dst": [{ "time": 0, "x": 10, "y": 20, "w": 20, "h": 10 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let manifest: SkinManifest = SkinManifest::default();
    let source_texture = crate::skin::SkinDocumentTexture {
        source_id: "1".to_string(),
        texture: crate::skin::SkinTextureId(77),
        source_size: crate::skin::SkinImageSize { width: 10.0, height: 120.0 },
    };
    let skin = SkinContext::from_manifest_and_document(manifest, document, [source_texture]);
    let snapshot = RenderSnapshot {
        arrange: "NORMAL".to_string(),
        arrange_2p: "MF-RANDOM".to_string(),
        ..Default::default()
    };

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(snapshot),
        &skin,
        &mut crate::skin::DynamicTimerRuntime::default(),
    );

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Image { texture, uv, .. }
            if *texture == TextureId(77) && (uv.y - 110.0 / 120.0).abs() < 0.001
    )));
}

#[test]
fn play_plan_routes_recent_judge_text_through_default_skin() {
    let snapshot = RenderSnapshot {
        time: TimeUs(1_000_000),
        recent_judgements: vec![DisplayJudgement {
            lane: Lane::Key2,
            judge: Judge::PGreat,
            side: Some(TimingSide::Fast),
            text: "PGREAT FAST".to_string(),
            combo: 1,
            delta_us: -3_000,
            time: TimeUs(920_000),
            is_miss: false,
            timing_ms_suppressed: false,
        }],
        ..Default::default()
    };

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, style, .. }
            if text == "PGREAT FAST" && style.layer == TextLayer::Skin
    )));
}

#[test]
fn play_plan_includes_judge_count_text() {
    let snapshot = RenderSnapshot {
        judge_counts: DisplayJudgeCounts {
            pgreat: 2,
            great: 1,
            good: 1,
            bad: 1,
            poor: 1,
            empty_poor: 3,
        },
        ..Default::default()
    };

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { style, .. } if style.color == Color::rgb(0.66, 0.92, 0.98)
    )));
    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { style, .. } if style.color == Color::rgb(0.96, 0.4, 0.44)
    )));
}

#[test]
fn play_plan_flashes_recent_judgement_lane() {
    let snapshot = RenderSnapshot {
        time: TimeUs(1_000_000),
        recent_judgements: vec![DisplayJudgement {
            lane: Lane::Key2,
            judge: Judge::PGreat,
            side: Some(TimingSide::Fast),
            text: "PGREAT FAST".to_string(),
            combo: 1,
            delta_us: -3_000,
            time: TimeUs(920_000),
            is_miss: false,
            timing_ms_suppressed: false,
        }],
        ..Default::default()
    };

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Rect { color, .. } if *color == judge_flash_color("PGREAT FAST", 0.35)
    )));
}

#[test]
fn play_plan_includes_recent_judgement_history_panel() {
    let snapshot = RenderSnapshot {
        time: TimeUs(1_000_000),
        recent_judgements: vec![DisplayJudgement {
            lane: Lane::Key2,
            judge: Judge::EmptyPoor,
            side: Some(TimingSide::Slow),
            text: "EMPTY POOR SLOW".to_string(),
            combo: 0,
            delta_us: 50_000,
            time: TimeUs(980_000),
            is_miss: false,
            timing_ms_suppressed: false,
        }],
        ..Default::default()
    };

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    assert!(plan.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Rect { rect, color } if rect.x == 0.885 && rect.y == 0.17 && *color == Color::rgb(0.03, 0.035, 0.038)
        )));
}

#[test]
fn lane_flash_expires_old_judgements() {
    let snapshot = RenderSnapshot {
        time: TimeUs(1_000_000),
        recent_judgements: vec![DisplayJudgement {
            lane: Lane::Key2,
            judge: Judge::Bad,
            side: Some(TimingSide::Slow),
            text: "BAD SLOW".to_string(),
            combo: 0,
            delta_us: 88_000,
            time: TimeUs(700_000),
            is_miss: false,
            timing_ms_suppressed: false,
        }],
        ..Default::default()
    };

    assert_eq!(lane_flash_color(&snapshot, Lane::Key2), None);
}

#[test]
fn play_plan_flashes_recent_input_lane_without_judgement() {
    let snapshot = RenderSnapshot {
        time: TimeUs(1_000_000),
        recent_inputs: vec![DisplayInput { lane: Lane::Key4, time: TimeUs(930_000) }],
        ..Default::default()
    };

    let plan = DrawPlan::from_scene(&AppSceneSnapshot::Play(snapshot));

    assert!(plan.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Rect { color, .. } if *color == Color::rgba(0.95, 0.98, 1.0, 0.16)
    )));
}

#[test]
fn input_lane_flash_expires_old_inputs() {
    let snapshot = RenderSnapshot {
        time: TimeUs(1_000_000),
        recent_inputs: vec![DisplayInput { lane: Lane::Key4, time: TimeUs(800_000) }],
        ..Default::default()
    };

    assert_eq!(input_lane_flash_color(&snapshot, Lane::Key4), None);
}

#[test]
fn lane_text_labels_match_default_bindings() {
    assert_eq!(lane_label(Lane::Scratch), "SC");
    assert_eq!(lane_label(Lane::Key7), "7");
    assert_eq!(lane_key_label(Lane::Scratch), "LS");
    assert_eq!(lane_key_label(Lane::Key1), "Z");
    assert_eq!(lane_key_label(Lane::Key7), "V");
}

#[test]
fn display_title_falls_back_and_sanitizes_non_ascii() {
    assert_eq!(display_title(""), "NO TITLE");
    assert_eq!(display_title("AあB"), "A?B");
}

#[test]
fn display_label_sanitizes_and_truncates_text() {
    assert_eq!(display_label("FullCombo!!", 8), "FullComb");
    assert_eq!(display_label("A_B", 8), "A?B");
}

#[test]
fn play_text_formats_delta_and_time() {
    assert_eq!(format_delta_ms(-12_345), "-12MS");
    assert_eq!(format_delta_ms(8_999), "+8MS");
    assert_eq!(format_time(TimeUs(65_000_000)), "01:05");
}

#[test]
fn judge_flash_color_reflects_judge_family() {
    assert_eq!(judge_flash_color("GREAT SLOW", 0.5), Color::rgba(0.55, 0.9, 1.0, 0.5));
    assert_eq!(judge_flash_color("GOOD FAST", 0.5), Color::rgba(0.85, 0.9, 0.45, 0.5));
    assert_eq!(judge_flash_color("POOR SLOW", 0.5), Color::rgba(1.0, 0.28, 0.32, 0.5));
}

#[test]
fn judgement_history_label_abbreviates_judges_and_sides() {
    assert_eq!(history_label("PGREAT FAST"), "PG F");
    assert_eq!(history_label("GREAT SLOW"), "GR S");
    assert_eq!(history_label("GOOD FAST"), "GD F");
    assert_eq!(history_label("BAD SLOW"), "BD S");
    assert_eq!(history_label("POOR FAST"), "PR F");
    assert_eq!(history_label("EMPTY POOR SLOW"), "EP S");
}

#[test]
fn clear_type_label_abbreviates_long_names() {
    assert_eq!(clear_type_label("Normal"), "NORMAL");
    assert_eq!(clear_type_label("LightAssistEasy"), "LAEASY");
    assert_eq!(clear_type_label("FullCombo"), "FC");
    assert_eq!(clear_type_label(""), "");
}

#[test]
fn row_status_label_shows_not_owned_for_unregistered_songs() {
    let unowned = SelectRowSnapshot {
        in_library: false,
        table_level: "12".to_string(),
        ..SelectRowSnapshot::default()
    };
    assert_eq!(row_status_label(Some(&unowned)), "NOT OWNED");
}

fn select_rows(count: u32) -> Vec<crate::scene::SelectRowSnapshot> {
    (0..count)
        .map(|index| crate::scene::SelectRowSnapshot {
            index,
            title: format!("Title {index}"),
            artist: format!("Artist {index}"),
            difficulty_name: "NORMAL".to_string(),
            play_level: index.to_string(),
            table_level: String::new(),
            total_notes: 1000 + index,
            initial_bpm: 128.0,
            min_bpm: 128.0,
            max_bpm: 128.0,
            length_ms: 90_000,
            clear_type: if index == 0 { "Normal".to_string() } else { String::new() },
            ex_score: (index == 0).then_some(1234),
            max_combo: (index == 0).then_some(777),
            gauge_value: (index == 0).then_some(80.0),
            replay_slots: [false; 4],
            is_folder: false,
            kind: Default::default(),
            ..Default::default()
        })
        .collect()
}

fn history_label(text: &str) -> String {
    judgement_history_label(&DisplayJudgement {
        lane: Lane::Key1,
        judge: Judge::PGreat,
        side: Some(TimingSide::Fast),
        text: text.to_string(),
        combo: 0,
        delta_us: 0,
        time: TimeUs(0),
        is_miss: false,
        timing_ms_suppressed: false,
    })
}

#[test]
fn fallback_bga_uses_normal_blend_for_video_layer_textures() {
    // 動画 BGA Layer は beatoraja の `ffmpeg.frag` 相当で黒クロマキー
    // をかけないため、`is_video` が立っているときは Normal を選ぶ。
    use crate::snapshot::DisplayBgaFrame;
    let snapshot = RenderSnapshot {
        has_bga: true,
        bga_enabled: true,
        bga_stretch: 1,
        bga_base: Some(DisplayBgaFrame::opaque(100, 256.0, 256.0)),
        bga_layer: Some(DisplayBgaFrame::opaque_video(201, 640.0, 360.0)),
        bga_layer2: Some(DisplayBgaFrame::opaque(102, 256.0, 256.0)),
        ..Default::default()
    };
    let mut commands = Vec::new();
    push_fallback_bga_background(&mut commands, &snapshot);
    let blends: Vec<(u32, BlendMode)> = commands
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCommand::Image { texture, blend, .. } => Some((texture.0, *blend)),
            _ => None,
        })
        .collect();
    assert_eq!(
        blends,
        vec![(100, BlendMode::Normal), (201, BlendMode::Normal), (102, BlendMode::LayerMask),]
    );
}

#[test]
fn fallback_bga_uses_layer_mask_blend_for_layer_textures() {
    // BGA Layer / Layer2 は beatoraja の `layer.frag` 相当の黒クロマキー
    // (`BlendMode::LayerMask`) を使うことを担保する。
    // bl.jpg のような黒画像 Layer が Base を完全に覆い隠さないために必要。
    use crate::snapshot::DisplayBgaFrame;
    let snapshot = RenderSnapshot {
        has_bga: true,
        bga_enabled: true,
        bga_stretch: 1,
        bga_base: Some(DisplayBgaFrame::opaque(100, 256.0, 256.0)),
        bga_layer: Some(DisplayBgaFrame::opaque(101, 256.0, 256.0)),
        bga_layer2: Some(DisplayBgaFrame::opaque(102, 256.0, 256.0)),
        ..Default::default()
    };
    let mut commands = Vec::new();
    push_fallback_bga_background(&mut commands, &snapshot);
    let blends: Vec<(u32, BlendMode)> = commands
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCommand::Image { texture, blend, .. } => Some((texture.0, *blend)),
            _ => None,
        })
        .collect();
    assert_eq!(
        blends,
        vec![(100, BlendMode::Normal), (101, BlendMode::LayerMask), (102, BlendMode::LayerMask),]
    );
}

#[test]
fn bga_fullscreen_geometry_letterbox_preserves_aspect() {
    let (rect, uv) = bga_fullscreen_geometry(1920.0, 1080.0, 1);
    assert!((rect.width - 1.0).abs() < f32::EPSILON);
    assert!((rect.height - (1080.0 / 1920.0)).abs() < 0.001);
    assert!((uv.width - 1.0).abs() < f32::EPSILON);
}

#[test]
fn miss_poor_does_not_flash_lane() {
    let snapshot = RenderSnapshot {
        time: TimeUs(1_000_000),
        recent_judgements: vec![DisplayJudgement {
            lane: Lane::Key3,
            judge: Judge::Poor,
            side: Some(TimingSide::Slow),
            text: "POOR SLOW".to_string(),
            combo: 0,
            delta_us: 50_000,
            time: TimeUs(950_000),
            is_miss: true,
            timing_ms_suppressed: false,
        }],
        ..Default::default()
    };

    // 見逃しPOORでは判定ラインフラッシュを出さない
    assert_eq!(judgement_lane_flash_color(&snapshot, Lane::Key3), None);
    // 打鍵判定（is_miss=false）では通常通りフラッシュが出る
    let mut with_hit = snapshot.clone();
    with_hit.recent_judgements[0].is_miss = false;
    assert!(judgement_lane_flash_color(&with_hit, Lane::Key3).is_some());
}

fn approx_eq(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 0.0001
}

fn draw_command_has_rect_color(command: &DrawCommand, predicate: impl Fn(&Color) -> bool) -> bool {
    match command {
        DrawCommand::Rect { color, .. } => predicate(color),
        DrawCommand::RectBatch { rects, .. } => rects.iter().any(|rect| predicate(&rect.color)),
        _ => false,
    }
}

fn draw_command_has_rect(command: &DrawCommand, predicate: impl Fn(&Rect, &Color) -> bool) -> bool {
    match command {
        DrawCommand::Rect { rect, color } => predicate(rect, color),
        DrawCommand::RectBatch { rects, .. } => {
            rects.iter().any(|command| predicate(&command.rect, &command.color))
        }
        _ => false,
    }
}
