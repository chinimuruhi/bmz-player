use super::*;

#[test]
fn wmii_fhd_lr2skin_moves_judge_line_with_lift_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let judge_line_ids = decoded
        .document
        .image
        .iter()
        .filter(|image| image.src == "1" && image.x == 1231 && image.y == 0)
        .map(|image| image.id.as_str())
        .collect::<Vec<_>>();
    assert!(!judge_line_ids.is_empty(), "expected WMII judge line source image");

    assert!(
        decoded
            .document
            .all_destinations(&decoded.document.enabled_options())
            .iter()
            .any(|destination| judge_line_ids.contains(&destination.id.as_str())
                && destination.offsets.contains(&3)),
        "expected WMII DST_JUDGELINE to include beatoraja default OFFSET_LIFT"
    );
}

#[test]
fn wmii_fhd_lr2skin_renders_score_graph_bars_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin_with_options(
        &skin_path,
        SkinKind::Play,
        &BTreeMap::from([("Score Graph".to_string(), "On".to_string())]),
        &BTreeMap::new(),
    )
    .unwrap();
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        total_notes: 1_000,
        past_notes: 500,
        ex_score: 1_000,
        best_ex_score: Some(1_300),
        projected_best_ex_score: Some(650),
        target_ex_score: Some(1_500),
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );

    assert!(
        items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                if (rect.x - 546.0 / 1920.0).abs() < 0.01
                    && (rect.width - 277.0 / 1920.0).abs() < 0.01
                    && (rect.height - 798.0 / 1080.0).abs() < 0.01
                    && tint.a > 0.5
        )),
        "expected WMII score graph frame/background to render on the left side"
    );
    assert!(
        items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { rect, .. }
                if (rect.x - 670.0 / 1920.0).abs() < 0.01
                    && rect.width > 0.0
                    && rect.height > 0.05
        )),
        "expected WMII score graph bars to render in the graph area"
    );
}

#[test]
fn wmii_fhd_lr2skin_uses_autoplay_layout_and_hides_score_graph_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([
        ("BGA Size".to_string(), "Extend".to_string()),
        ("Score Graph".to_string(), "On".to_string()),
    ]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        ready_timer_ms: Some(2_000),
        has_bga: true,
        bga_enabled: true,
        bga_base: Some(bmz_render::skin::SkinBgaFrame::opaque(
            bmz_render::skin::SkinTextureId(90_000),
            SkinImageSize { width: 640.0, height: 480.0 },
        )),
        autoplay: true,
        skin_loaded: true,
        total_notes: 1_000,
        past_notes: 500,
        ex_score: 1_000,
        best_ex_score: Some(1_300),
        target_ex_score: Some(1_500),
        ..Default::default()
    };

    let text_state = bmz_render::skin::SkinTextState {
        title: "AUTOPLAY UNIQUE TITLE",
        artist: "AUTOPLAY UNIQUE ARTIST",
        genre: "AUTOPLAY UNIQUE GENRE",
        ..Default::default()
    };
    let items = decoded.document.static_render_items(&sources, &state, &text_state);

    let bga_rects = items
        .iter()
        .filter_map(|item| match item {
            bmz_render::skin::SkinRenderItem::Image { texture, rect, .. }
                if *texture == bmz_render::skin::SkinTextureId(90_000) =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        bga_rects.len(),
        1,
        "autoplay and Score Graph layouts must not render duplicate BGA destinations: {bga_rects:?}"
    );
    assert!((bga_rects[0].x - 732.0 / 1920.0).abs() < 0.01);
    assert!((bga_rects[0].width - 1015.0 / 1920.0).abs() < 0.01);

    let title_origins = items
        .iter()
        .filter_map(|item| match item {
            bmz_render::skin::SkinRenderItem::Text { origin, text, .. }
                if text == "AUTOPLAY UNIQUE TITLE" =>
            {
                Some(*origin)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        title_origins.len(),
        1,
        "expected one title without a duplicated graph-layout title: {title_origins:?}"
    );

    assert!(
        items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                if (rect.x - 726.0 / 1920.0).abs() < 0.01
                    && (rect.width - 1027.0 / 1920.0).abs() < 0.01
                    && tint.a > 0.5
        )),
        "expected WMII autoplay BGA frame to render; got {items:?}"
    );
    assert!(
        !items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                if (rect.x - 546.0 / 1920.0).abs() < 0.01
                    && (rect.width - 277.0 / 1920.0).abs() < 0.01
                    && (rect.height - 798.0 / 1080.0).abs() < 0.01
                    && tint.a > 0.5
        )),
        "WMII score graph frame must honor its autoplay-off condition"
    );
    assert!(
        !items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { rect, tint, .. }
                if (rect.x - 551.0 / 1920.0).abs() < 0.01
                    && (rect.width - 267.0 / 1920.0).abs() < 0.01
                    && tint.a > 0.5
        )),
        "WMII score graph labels must honor their autoplay-off condition"
    );
}

#[test]
fn wmii_fhd_lr2skin_renders_lane_cover_and_lift_numbers_when_adjusting() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let source1 = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "1")
        .expect("WMII number source should decode");
    let number_uv_y = 883.0 / source1.size.height;
    let number_uv_h = 20.0 / source1.size.height;
    let sources = decoded
        .sources
        .iter()
        .map(|source| {
            (
                source.source_id.clone(),
                SkinDocumentTexture {
                    source_id: source.source_id.clone(),
                    texture: source.texture,
                    source_size: SkinImageSize {
                        width: source.size.width,
                        height: source.size.height,
                    },
                },
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        lane_cover: 0.290,
        lift: 0.222,
        total_duration_ms: 517,
        offset_lift_px: (0.222_f32 * 723.0).round() as i32,
        offset_lanecover_px: -(723.0_f32 * 0.290).round() as i32,
        lane_cover_changing: true,
        lanecover_enabled: true,
        lift_enabled: true,
        now_bpm: 88.0,
        main_bpm: 88.0,
        min_bpm: 38.0,
        max_bpm: 156.0,
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );

    let number_digits = items
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, uv, .. }
                    if *texture == source1.texture
                        && (uv.y - number_uv_y).abs() < 0.001
                        && (uv.height - number_uv_h).abs() < 0.001
            )
        })
        .collect::<Vec<_>>();
    let white_digits = number_digits
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { tint, .. }
                    if tint.r > 0.95 && tint.g > 0.95 && tint.b > 0.95 && tint.a > 0.5
            )
        })
        .count();
    let green_digits = number_digits
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { tint, .. }
                    if tint.r < 0.4 && tint.g > 0.75 && tint.b < 0.5 && tint.a > 0.5
            )
        })
        .count();
    let green_bpm_cover_digits = number_digits
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { tint, rect, .. }
                    if tint.r < 0.4
                        && tint.g > 0.75
                        && tint.b < 0.5
                        && tint.a > 0.5
                        && (rect.y * 1080.0 - 165.0).abs() < 2.0
            )
        })
        .count();
    let green_bpm_no_cover_digits = number_digits
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { tint, rect, .. }
                    if tint.r < 0.4
                        && tint.g > 0.75
                        && tint.b < 0.5
                        && tint.a > 0.5
                        && (rect.y * 1080.0 - 203.0).abs() < 2.0
            )
        })
        .count();
    let green_digit_ys = number_digits
        .iter()
        .filter_map(|item| {
            if let bmz_render::skin::SkinRenderItem::Image { tint, rect, .. } = item
                && tint.r < 0.4
                && tint.g > 0.75
                && tint.b < 0.5
                && tint.a > 0.5
            {
                Some((rect.y * 1080.0).round() as i32)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    assert!(
        white_digits >= 6,
        "expected WMII SUDDEN and LIFT white number digits to render; got {white_digits}"
    );
    assert!(
        green_digits >= 6,
        "expected WMII upper and lower green number digits to render; got {green_digits}"
    );
    assert!(
        green_bpm_cover_digits >= 9,
        "expected WMII BPM green digits to use lanecover-on layout; got {green_bpm_cover_digits}; green ys {green_digit_ys:?}"
    );
    assert_eq!(
        green_bpm_no_cover_digits, 0,
        "expected WMII BPM green digits not to use lanecover-off layout when op271 is active"
    );

    let zero_lift_state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        lane_cover: 0.290,
        lift: 0.0,
        total_duration_ms: 517,
        offset_lift_px: 0,
        offset_lanecover_px: -(723.0_f32 * 0.290).round() as i32,
        lane_cover_changing: true,
        lanecover_enabled: true,
        lift_enabled: true,
        now_bpm: 88.0,
        main_bpm: 88.0,
        min_bpm: 38.0,
        max_bpm: 156.0,
        ..Default::default()
    };
    let zero_lift_items = decoded.document.static_render_items(
        &sources,
        &zero_lift_state,
        &bmz_render::skin::SkinTextState::default(),
    );
    let zero_lift_digits = zero_lift_items
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, uv, rect, .. }
                    if *texture == source1.texture
                        && (uv.y - number_uv_y).abs() < 0.001
                        && (uv.height - number_uv_h).abs() < 0.001
                        && (rect.y * 1080.0 - 724.0).abs() < 2.0
            )
        })
        .collect::<Vec<_>>();
    let zero_lift_white_digits = zero_lift_digits
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { tint, .. }
                    if tint.r > 0.95 && tint.g > 0.95 && tint.b > 0.95 && tint.a > 0.5
            )
        })
        .count();
    let zero_lift_green_digits = zero_lift_digits
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { tint, .. }
                    if tint.r < 0.4 && tint.g > 0.75 && tint.b < 0.5 && tint.a > 0.5
            )
        })
        .count();
    assert!(
        zero_lift_white_digits > 0,
        "expected WMII LIFT white digits to render even when LIFT is zero"
    );
    assert!(
        zero_lift_green_digits > 0,
        "expected WMII LIFT green digits to render even when LIFT is zero"
    );
}
