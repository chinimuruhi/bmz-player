use super::*;

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
