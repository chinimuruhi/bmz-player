use super::*;

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
fn skin_draw_options_match_live_score_ranks() {
    let base = SkinDrawState { total_notes: 100, ..SkinDrawState::default() };
    let aaa = SkinDrawState { ex_score: 178, ..base.clone() };
    let aa = SkinDrawState { ex_score: 156, ..base.clone() };
    let a = SkinDrawState { ex_score: 134, ..base.clone() };
    let f = SkinDrawState { ex_score: 0, ..base };

    assert!(test_skin_op(220, &[], &aaa));
    assert!(test_skin_op(221, &[], &aaa));
    assert!(!test_skin_op(220, &[], &aa));
    assert!(test_skin_op(221, &[], &aa));
    assert!(test_skin_op(222, &[], &a));
    assert!(test_skin_op(223, &[], &a));
    assert!(test_skin_op(227, &[], &f));
    assert!(!test_skin_op(226, &[], &f));
    assert!(!test_skin_op(220, &[], &SkinDrawState::default()));
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
        &SkinDrawState { hidden_enabled: true, ..SkinDrawState::default() },
    );

    assert!(matches!(hidden.as_slice(), [SkinRenderItem::Image { tint, .. }] if tint.a == 0.0));
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
