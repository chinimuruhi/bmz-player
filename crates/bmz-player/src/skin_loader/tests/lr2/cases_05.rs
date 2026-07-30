use super::*;

#[test]
fn wmii_fhd_lr2skin_renders_runtime_difficulty_badge_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
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
        difficulty: 4,
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
                if (rect.x - 617.0 / 1920.0).abs() < 0.01
                    && (rect.width - 187.0 / 1920.0).abs() < 0.01
                    && tint.a > 0.1
        )),
        "expected WMII ANOTHER difficulty badge to render for difficulty op154"
    );
}

#[test]
fn wmii_fhd_lr2skin_renders_judge_and_combo_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("Displayjudge".to_string(), "ON".to_string())]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();
    let judge_texture = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "13")
        .expect("WMII judge source should load")
        .texture;
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
    let mut judge_ms = [None; bmz_render::skin::MAX_JUDGE_REGIONS];
    judge_ms[0] = Some(100);
    let mut judge_index = [None; bmz_render::skin::MAX_JUDGE_REGIONS];
    judge_index[0] = Some(0);
    let mut judge_combo = [0; bmz_render::skin::MAX_JUDGE_REGIONS];
    judge_combo[0] = 123;
    let state = bmz_render::skin::SkinDrawState {
        elapsed_ms: 2_000,
        play_timer_ms: Some(2_000),
        judge_ms,
        judge_index,
        judge_combo,
        ..Default::default()
    };

    let items = decoded.document.static_render_items(
        &sources,
        &state,
        &bmz_render::skin::SkinTextState::default(),
    );
    let judge_items = items
        .iter()
        .filter(|item| {
            matches!(
                item,
                bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                    if *texture == judge_texture
                        && rect.height > 0.01
                        && tint.a > 0.5
            )
        })
        .count();

    assert!(
        judge_items >= 2,
        "expected WMII judge text and combo digits from source 13; got {items:?}"
    );
    assert!(
        items.iter().any(|item| matches!(
            item,
            bmz_render::skin::SkinRenderItem::Image { texture, rect, uv, tint, .. }
                if *texture == judge_texture
                    && rect.height > 0.05
                    && uv.y < 0.001
                    && tint.a > 0.5
        )),
        "expected PGREAT judge image to use the top WMII judge source row; got {items:?}"
    );

    for (judge_index, label) in ["PGREAT", "GREAT", "GOOD", "BAD", "POOR"].iter().enumerate() {
        let mut judge_ms = [None; bmz_render::skin::MAX_JUDGE_REGIONS];
        judge_ms[0] = Some(100);
        let mut judge_indices = [None; bmz_render::skin::MAX_JUDGE_REGIONS];
        judge_indices[0] = Some(judge_index);
        let mut judge_combo = [0; bmz_render::skin::MAX_JUDGE_REGIONS];
        judge_combo[0] = 123;
        let state = bmz_render::skin::SkinDrawState {
            elapsed_ms: 2_000,
            play_timer_ms: Some(2_000),
            judge_ms,
            judge_index: judge_indices,
            judge_combo,
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
                bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                    if *texture == judge_texture
                        && rect.height > 0.05
                        && tint.a > 0.5
            )),
            "expected WMII {label} judge image to render; got {items:?}"
        );
    }
}

#[test]
fn wmii_fhd_lr2skin_dp_renders_judge_detail_panel_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC_DP.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([
        ("Displayjudge".to_string(), "ON".to_string()),
        ("GRAPH SIDE".to_string(), "RIGHT".to_string()),
        ("Score Graph".to_string(), "On".to_string()),
    ]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();

    assert!(
        decoded.document.enabled_options().contains(&983),
        "expected WMII DP judge detail panel op983 to stay enabled"
    );

    let frame_texture = decoded
        .sources
        .iter()
        .find(|source| source.source_id == "1")
        .expect("WMII frame source should load")
        .texture;
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
        key_mode: bmz_core::lane::KeyMode::K14,
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
            bmz_render::skin::SkinRenderItem::Image { texture, rect, tint, .. }
                if *texture == frame_texture
                    && (rect.x - 71.0 / 1920.0).abs() < 0.01
                    && (rect.width - 247.0 / 1920.0).abs() < 0.02
                    && rect.height > 0.1
                    && tint.a > 0.1
        )),
        "expected WMII DP judge detail panel body to render; got {items:?}"
    );
}

#[test]
fn wmii_fhd_lr2skin_renders_fast_slow_during_replay_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("Display FAST/SLOW".to_string(), "ON-A".to_string())]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();
    let sources = decoded.sources.iter().map(|source| SkinDocumentTexture {
        source_id: source.source_id.clone(),
        texture: source.texture,
        source_size: SkinImageSize { width: source.size.width, height: source.size.height },
    });
    let skin = SkinContext::from_manifest_and_document(
        SkinManifest::default(),
        decoded.document.clone(),
        sources,
    );
    let replay_snapshot = bmz_render::snapshot::RenderSnapshot {
        time: TimeUs(100_000),
        play_elapsed_time: TimeUs(100_000),
        replay_playback: true,
        key_mode: bmz_core::lane::KeyMode::K7,
        recent_judgements: vec![bmz_render::snapshot::DisplayJudgement {
            lane: bmz_core::lane::Lane::Key1,
            judge: bmz_core::judge::Judge::PGreat,
            side: Some(bmz_core::judge::TimingSide::Fast),
            text: "PGREAT FAST".to_string(),
            combo: 1,
            delta_us: -2_000,
            time: TimeUs(0),
            is_miss: false,
            timing_ms_suppressed: false,
        }],
        ..Default::default()
    };
    let has_wmii_fast_slow_image = |plan: &DrawPlan| {
        plan.commands.iter().any(|command| {
            matches!(
                command,
                DrawCommand::Image { rect, tint, .. }
                    if ((rect.x - 292.0 / 1920.0).abs() < 0.01
                        || (rect.x - 246.0 / 1920.0).abs() < 0.01)
                        && (rect.y - 502.0 / 1080.0).abs() < 0.01
                        && (rect.width - 82.0 / 1920.0).abs() < 0.01
                        && tint.a > 0.5
            )
        })
    };

    let mut snapshot = replay_snapshot.clone();
    crate::screens::play_snapshot::apply_fast_slow_display_filter(
        &mut snapshot,
        0,
        crate::config::profile_config::FastSlowDisplayScope::ThresholdMs,
    );

    let plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(snapshot),
        &skin,
        &mut DynamicTimerRuntime::default(),
    );

    assert!(
        has_wmii_fast_slow_image(&plan),
        "expected WMII replay PGREAT FAST/SLOW image to render; got {:?}",
        plan.commands
    );

    let mut auto_snapshot = replay_snapshot;
    crate::screens::play_snapshot::apply_fast_slow_display_filter(
        &mut auto_snapshot,
        0,
        crate::config::profile_config::FastSlowDisplayScope::Auto,
    );
    let auto_plan = DrawPlan::from_scene_with_skin(
        &AppSceneSnapshot::Play(auto_snapshot),
        &skin,
        &mut DynamicTimerRuntime::default(),
    );

    assert!(
        !has_wmii_fast_slow_image(&auto_plan),
        "expected WMII Auto scope to hide replay PGREAT FAST/SLOW; got {:?}",
        auto_plan.commands
    );
}

#[test]
fn wmii_fhd_lr2skin_applies_play_timing_headers_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

    assert_eq!(decoded.document.loadstart, 0);
    assert_eq!(decoded.document.loadend, 3000);
    assert_eq!(decoded.document.playstart, 1500);
    assert_eq!(decoded.document.fadeout, 500);
    assert_eq!(decoded.document.close, 2500);
}

#[test]
fn wmii_fhd_lr2skin_uses_lr2_bitmap_fonts_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

    assert!(
        decoded.document.font.iter().any(|font| {
            font.id.starts_with("lr2font-")
                && font.path.replace('\\', "/").ends_with("../font/songTitle/font.fnt")
        }),
        "expected LR2FONT font.lr2font to resolve to bundled font.fnt; got {:?}",
        decoded.document.font
    );
    assert!(
        decoded.document.text.iter().any(|text| {
            text.ref_id == 12 && text.font.starts_with("play:lr2font-") && text.size == 0
        }),
        "expected full-title text to keep its LR2 bitmap font id; got {:?}",
        decoded.document.text
    );
    assert!(
        decoded.document.text.iter().any(|text| {
            text.ref_id == 10 && text.font.starts_with("play:lr2font-") && text.size == 0
        }),
        "expected READY title text to use LR2 bitmap font index 0; got {:?}",
        decoded.document.text
    );
    assert!(
        decoded.document.text.iter().any(|text| {
            text.ref_id == 14 && text.font.starts_with("play:lr2font-") && text.size == 0
        }),
        "expected artist text to keep its LR2 bitmap font id; got {:?}",
        decoded.document.text
    );
    assert!(
        decoded.fonts.iter().any(|font| {
            font.stored_id.starts_with("play:lr2font-")
                && matches!(font.data.as_ref(), Some(DecodedFontData::Bitmap(_)))
        }),
        "expected decoded LR2 bitmap font to be loaded"
    );
}

#[test]
fn wmii_fhd_lr2skin_uses_dst_text_size_for_lr2_bitmap_fonts_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let title_id = decoded
        .document
        .text
        .iter()
        .find(|text| text.ref_id == 12)
        .map(|text| text.id.as_str())
        .expect("WMII full-title text should exist");
    let has_frame_height = |id: &str, height: i32| {
        decoded.document.destination.iter().any(|entry| match entry {
            bmz_render::skin::DestinationListEntry::Single(destination) => {
                destination.id == id
                    && destination.dst.iter().any(|frame| match frame {
                        bmz_render::skin::SkinDstEntry::Frame(frame) => frame.h == Some(height),
                        bmz_render::skin::SkinDstEntry::Conditional { frames, .. } => {
                            frames.iter().any(|frame| frame.h == Some(height))
                        }
                    })
            }
            bmz_render::skin::DestinationListEntry::Conditional { destinations, .. } => {
                destinations.iter().any(|destination| {
                    destination.id == id
                        && destination.dst.iter().any(|frame| match frame {
                            bmz_render::skin::SkinDstEntry::Frame(frame) => frame.h == Some(height),
                            bmz_render::skin::SkinDstEntry::Conditional { frames, .. } => {
                                frames.iter().any(|frame| frame.h == Some(height))
                            }
                        })
                })
            }
        })
    };

    assert!(
        has_frame_height(title_id, 41),
        "expected WMII full-title bitmap font size to come from DST_TEXT h=41"
    );
    assert!(
        decoded.document.text.iter().any(|text| {
            text.ref_id == 14
                && text.font.starts_with("play:lr2font-")
                && has_frame_height(&text.id, 29)
        }),
        "expected WMII artist bitmap font size to come from DST_TEXT h=29"
    );
}

#[test]
fn wmii_fhd_lr2skin_uses_lr2_bitmap_font_for_table_level_when_enabled() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("Display Table Level".to_string(), "ON".to_string())]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();

    assert!(
        decoded.document.text.iter().any(|text| {
            text.ref_id == 1002 && text.font.starts_with("play:lr2font-") && text.size == 0
        }),
        "expected difficulty-table text to keep its LR2 bitmap font id; got {:?}",
        decoded.document.text
    );
}

#[test]
fn wmii_fhd_lr2skin_preserves_green_number_digit_width_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();
    let green_numbers = decoded
        .document
        .value
        .iter()
        .filter(|value| matches!(value.ref_id, 313 | 1317 | 1321 | 1325))
        .collect::<Vec<_>>();

    assert!(!green_numbers.is_empty(), "expected WMII green-number value sprites");
    assert!(
        green_numbers.iter().all(|value| value.digit == 3),
        "LR2 keta field should remain 3 digits for WMII green numbers; got {green_numbers:?}"
    );

    assert!(
        decoded.document.value.iter().any(|value| value.ref_id == 310 && value.digit == 1),
        "expected WMII white high-speed integer digit to use LR2 keta=1"
    );
    assert!(
        decoded.document.value.iter().any(|value| value.ref_id == 311 && value.digit == 2),
        "expected WMII white high-speed decimal digits to use LR2 keta=2"
    );
}

#[test]
fn wmii_fhd_lr2skin_keeps_runtime_difficulty_option_destinations_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

    for op in 150..=155 {
        assert!(
            decoded.document.destination.iter().any(|entry| match entry {
                bmz_render::skin::DestinationListEntry::Single(destination) =>
                    destination.op.contains(&op),
                bmz_render::skin::DestinationListEntry::Conditional { destinations, .. } =>
                    destinations.iter().any(|destination| destination.op.contains(&op)),
            }),
            "expected runtime difficulty op {op} to survive LR2 #IF conversion"
        );
    }
}

#[test]
fn wmii_fhd_lr2skin_uses_relative_combo_destination_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let options = BTreeMap::from([("Displayjudge".to_string(), "ON".to_string())]);
    let decoded =
        decode_beatoraja_skin_with_options(&skin_path, SkinKind::Play, &options, &BTreeMap::new())
            .unwrap();

    assert!(
        decoded.document.judge.iter().flat_map(|judge| &judge.numbers).any(|number| {
            number.dst.iter().any(|entry| match entry {
                bmz_render::skin::SkinDstEntry::Frame(frame) => {
                    frame.x == Some(242) && frame.y == Some(0) && frame.h == Some(124)
                }
                bmz_render::skin::SkinDstEntry::Conditional { frames, .. } => {
                    frames.iter().any(|frame| {
                        frame.x == Some(242) && frame.y == Some(0) && frame.h == Some(124)
                    })
                }
            })
        }),
        "expected WMII NOWCOMBO destination to stay relative to judge image"
    );
    assert!(
        decoded
            .document
            .judge
            .iter()
            .flat_map(|judge| &judge.images)
            .any(|image| { image.offsets.contains(&3) && image.offsets.contains(&32) }),
        "expected WMII NOWJUDGE destinations to include beatoraja LR2 judge and lift offsets"
    );
    assert!(
        decoded
            .document
            .judge
            .iter()
            .flat_map(|judge| &judge.numbers)
            .any(|number| { number.offsets.contains(&3) && number.offsets.contains(&32) }),
        "expected WMII NOWCOMBO destinations to include beatoraja LR2 judge and lift offsets"
    );
}

#[test]
fn wmii_fhd_lr2skin_defaults_score_graph_to_off_when_available() {
    let skin_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/WMII_FHD/play/FHDPLAY_AC.lr2skin");
    if !skin_path.is_file() {
        return;
    }

    let decoded = decode_beatoraja_skin(&skin_path, SkinKind::Play).unwrap();

    assert!(decoded.document.graph.iter().all(|graph| !matches!(graph.graph_type, 110..=115)));
    assert!(
        decoded
            .document
            .property
            .iter()
            .any(|property| property.name == "Score Graph" && property.def == "Off"),
        "expected beatoraja's built-in Score Graph option to default to Off"
    );
}
