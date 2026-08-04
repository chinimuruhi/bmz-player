use super::*;

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
    assert!(
        resolve_destination_frame(&destination, 1000, &[], &SkinDrawState::default()).is_none()
    );

    let held: SkinDestinationDef = serde_json::from_str(
        r#"{ "id": "flash", "loop": 1000, "dst": [{ "time": 1000, "x": 2, "y": 3, "w": 10, "h": 20 }] }"#,
    )
    .unwrap();
    let frame = resolve_destination_frame(&held, 1000, &[], &SkinDrawState::default())
        .expect("single frame starts at its keyframe time");
    assert_eq!((frame.x, frame.y, frame.w, frame.h), (2, 3, 10, 20));
    assert!(resolve_destination_frame(&held, 1001, &[], &SkinDrawState::default()).is_some());

    let disappearing: SkinDestinationDef = serde_json::from_str(
            r#"{ "id": "flash", "loop": -1, "dst": [{ "time": 1000, "x": 2, "y": 3, "w": 10, "h": 20 }] }"#,
        )
        .unwrap();
    assert!(
        resolve_destination_frame(&disappearing, 1001, &[], &SkinDrawState::default()).is_none()
    );
}

#[test]
fn omitted_loop_restarts_destination_animation_from_zero() {
    let destination: SkinDestinationDef = serde_json::from_str(
        r#"{ "id": "rhythm", "timer": 140, "dst": [
                { "time": 0, "x": 0, "y": 0, "w": 10, "h": 10 },
                { "time": 1000, "x": 100 }
            ]}"#,
    )
    .unwrap();

    let before = resolve_destination_frame(&destination, 999, &[], &SkinDrawState::default())
        .expect("animation should reach its final keyframe");
    let restarted = resolve_destination_frame(&destination, 1000, &[], &SkinDrawState::default())
        .expect("animation should restart at its first keyframe");

    assert_eq!(before.x, 100);
    assert_eq!(restarted.x, 0);
}

#[test]
fn destination_frame_h_expr_resolves_fast_slow_breakdown_height() {
    let destination: SkinDestinationDef = serde_json::from_str(&format!(
        r#"{{
                "id": "graph_r",
                "loop": 1000,
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
