use super::*;

#[test]
fn build_render_snapshot_treats_replay_as_autoplay_off_for_skin_ops() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.play.auto_play = true;
    let replay = build_game_session(
        Arc::new(chart()),
        &profile,
        PlaySessionOptions {
            replay_player: Some(bmz_gameplay::replay::ReplayPlayer::default()),
            ..PlaySessionOptions::default()
        },
    );

    assert!(replay.autoplay.is_none());
    let snapshot = build_render_snapshot(&replay, TimeUs(0), &[], None);
    assert!(snapshot.replay_playback);
    assert!(!snapshot.autoplay);
}

#[test]
fn build_render_snapshot_passes_judge_rank() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut chart = chart();
    chart.metadata.judge_rank = Some(0);
    let session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(snapshot.judge_rank, Some(0));
}

#[test]
fn build_render_snapshot_passes_best_ex_score() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let session = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

    let with_best = build_render_snapshot(&session, TimeUs(0), &[], Some(42));
    let without_best = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(with_best.best_ex_score, Some(42));
    assert_eq!(without_best.best_ex_score, None);
}

#[test]
fn build_render_snapshot_passes_target_ex_score() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let session = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());

    let snapshot = build_render_snapshot_with_target_and_bga_frames(
        &session,
        TimeUs(0),
        &[],
        None,
        None,
        Some(1600),
        &BgaFrameCatalog::new(),
    );

    assert_eq!(snapshot.target_ex_score, Some(1600));
}

#[test]
fn build_render_snapshot_projects_best_score_from_ghost() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.score.past_notes = 3;

    let snapshot = build_render_snapshot_with_target_and_bga_frames(
        &session,
        TimeUs(0),
        &[],
        Some(8),
        Some(&[0, 1, 4, 0]),
        None,
        &BgaFrameCatalog::new(),
    );

    assert_eq!(snapshot.projected_best_ex_score, Some(3));
}

#[test]
fn build_render_snapshot_derives_judge_timing_offset_from_visual_offset() {
    use bmz_gameplay::session::PlayOffsets;

    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.offsets = PlayOffsets { input_offset_us: 3_000, visual_offset_us: 4_000 };

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(snapshot.judge_timing_offset_ms, 4);
}

#[test]
fn build_render_snapshot_copies_skin_offsets() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.skin_offsets.push(bmz_gameplay::session::PlaySkinOffset {
        id: 42,
        x: 1,
        y: 2,
        w: 3,
        h: 4,
        r: 5,
        a: -6,
    });

    let snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);

    assert_eq!(
        snapshot.skin_offsets.get(42),
        Some(SkinOffsetValue { x: 1, y: 2, w: 3, h: 4, r: 5, a: -6 })
    );
}

#[test]
fn build_render_snapshot_sets_scratch_angle_offset() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.skin_offsets.push(bmz_gameplay::session::PlaySkinOffset {
        id: SCRATCH_ANGLE_OFFSET_1P,
        x: 1,
        y: 2,
        w: 3,
        h: 4,
        r: 5,
        a: -6,
    });

    let snapshot = build_render_snapshot(&session, TimeUs(6_000_000), &[], None);

    assert_eq!(
        snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_1P),
        Some(SkinOffsetValue { x: 1, y: 2, w: 3, h: 4, r: 80, a: -6 })
    );
}

#[test]
fn refresh_play_skin_visuals_uses_play_elapsed_during_playstart() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let session = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    let mut snapshot = build_render_snapshot(&session, TimeUs(-1_000_000), &[], None);
    snapshot.play_elapsed_time = TimeUs(6_000_000);

    refresh_play_skin_visuals(&mut snapshot, &session);

    assert_eq!(
        snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_1P),
        Some(SkinOffsetValue { r: 80, ..SkinOffsetValue::default() })
    );
}

#[test]
fn refresh_play_skin_visuals_keeps_turntable_angle_after_chart_start() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let session = build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    let mut snapshot = build_render_snapshot(&session, TimeUs(0), &[], None);
    snapshot.play_elapsed_time = TimeUs(6_000_000);

    refresh_play_skin_visuals(&mut snapshot, &session);

    assert_eq!(
        snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_1P),
        Some(SkinOffsetValue { r: 80, ..SkinOffsetValue::default() })
    );
}

#[test]
fn refresh_play_skin_visuals_applies_accumulated_scratch_turntable_phase() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.lane_scratch_angle_delta_ms[Lane::Scratch.index()] = 2_000;
    let mut snapshot = build_render_snapshot(&session, TimeUs(1_000_000), &[], None);
    snapshot.play_elapsed_time = TimeUs(6_000_000);

    refresh_play_skin_visuals(&mut snapshot, &session);

    assert_eq!(
        snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_1P),
        Some(SkinOffsetValue { r: 53, ..SkinOffsetValue::default() })
    );
}

#[test]
fn refresh_play_skin_visuals_keeps_accumulated_scratch_phase_after_release() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.lane_scratch_angle_delta_ms[Lane::Scratch.index()] = -2_000;
    let mut snapshot = build_render_snapshot(&session, TimeUs(1_000_000), &[], None);
    snapshot.play_elapsed_time = TimeUs(6_000_000);

    refresh_play_skin_visuals(&mut snapshot, &session);

    assert_eq!(
        snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_1P),
        Some(SkinOffsetValue { r: 106, ..SkinOffsetValue::default() })
    );
}

#[test]
fn scratch_angle_offsets_match_beatoraja_1p_and_2p_values() {
    let first_frame = TimeUs(6_000_000);
    let next_frame = TimeUs(6_006_000);

    assert_eq!(scratch_angle_degrees(first_frame, 0, 0), 80);
    assert_eq!(scratch_angle_degrees(next_frame, 0, 0), 79);
    assert_eq!(scratch_angle_degrees(first_frame, 1, 0), 280);
    assert_eq!(scratch_angle_degrees(next_frame, 1, 0), 281);
}

#[test]
fn build_render_snapshot_sets_opposite_14k_turntable_offsets() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut chart = chart();
    chart.metadata.key_mode = KeyMode::K14;
    let session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());

    let snapshot = build_render_snapshot(&session, TimeUs(6_000_000), &[], None);

    assert_eq!(snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_1P).unwrap().r, 80);
    assert_eq!(snapshot.skin_offsets.get(SCRATCH_ANGLE_OFFSET_2P).unwrap().r, 280);
}

#[test]
fn scratch_offsets_render_with_beatoraja_rotation_after_skin_conversion() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut chart = chart();
    chart.metadata.key_mode = KeyMode::K14;
    let session = build_game_session(Arc::new(chart), &profile, PlaySessionOptions::default());
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 2,
                "w": 100,
                "h": 100,
                "source": [{ "id": "src", "path": "turntable.png" }],
                "image": [{ "id": "turntable", "src": "src", "w": 10, "h": 10 }],
                "destination": [
                    {
                        "id": "turntable",
                        "offset": 1,
                        "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }]
                    },
                    {
                        "id": "turntable",
                        "offset": 2,
                        "dst": [{ "x": 20, "y": 0, "w": 10, "h": 10 }]
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "src".to_string(),
        SkinDocumentTexture {
            source_id: "src".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        },
    )]);

    for (visual_time, expected_angles) in
        [(TimeUs(6_000_000), [-80, -280]), (TimeUs(6_006_000), [-79, -281])]
    {
        let snapshot = build_render_snapshot(&session, visual_time, &[], None);
        let state = SkinDrawState {
            key_mode: KeyMode::K14,
            skin_offsets: snapshot.skin_offsets,
            ..SkinDrawState::default()
        };
        let angles = document
            .static_image_render_items(&sources, &state)
            .iter()
            .map(|item| match item {
                SkinRenderItem::RotatedImage { angle_deg, .. } => *angle_deg as i32,
                _ => panic!("turntable should be rotated"),
            })
            .collect::<Vec<_>>();

        assert_eq!(angles, expected_angles);
    }
}

#[test]
fn refresh_play_skin_visuals_tracks_pre_ready_keybeam() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let mut session =
        build_game_session(Arc::new(chart()), &profile, PlaySessionOptions::default());
    session.lane_keyon_started_at[Lane::Key1.index()] = Some(TimeUs(1_000_000));
    let mut snapshot = build_render_snapshot(&session, TimeUs(-1_000_000), &[], None);
    snapshot.play_elapsed_time = TimeUs(1_050_000);

    refresh_play_skin_visuals(&mut snapshot, &session);

    assert_eq!(snapshot.keyon_ms[Lane::Key1.index()], Some(50));
}
