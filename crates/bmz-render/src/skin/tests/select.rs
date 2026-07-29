use super::*;

#[test]
fn select_document_options_follow_selected_song_text_presence() {
    let no_document = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_has_document: false,
        ..SkinDrawState::default()
    };
    let with_document = SkinDrawState { select_has_document: true, ..no_document.clone() };
    let folder = SkinDrawState { select_row_kind: SelectRowKind::Folder, ..with_document.clone() };

    assert!(test_skin_op(174, &[], &no_document));
    assert!(!test_skin_op(175, &[], &no_document));
    assert!(!test_skin_op(174, &[], &with_document));
    assert!(test_skin_op(175, &[], &with_document));
    assert!(!test_skin_op(174, &[], &folder));
    assert!(!test_skin_op(175, &[], &folder));
}

#[test]
fn select_row_bar_slots_follow_beatoraja_bar_types() {
    let cases = [
        (SelectRowKind::Song, true, 0, 2),
        (SelectRowKind::Song, false, 4, 8),
        (SelectRowKind::Folder, true, 1, 4),
        (SelectRowKind::TableFolder, true, 2, 6),
        (SelectRowKind::SearchFolder, true, 6, 10),
        (SelectRowKind::Course, true, 3, 7),
        (SelectRowKind::Course, false, 4, 8),
        (SelectRowKind::Executable, true, 2, 6),
        (SelectRowKind::RandomCourse, true, 2, 6),
        (SelectRowKind::RandomCourse, false, 4, 8),
        (SelectRowKind::Command, true, 5, 9),
        (SelectRowKind::Container, true, 5, 9),
        (SelectRowKind::NoSong, false, 4, 8),
        (SelectRowKind::SettingsRoot, true, 8, 11),
        (SelectRowKind::SettingsFolder, true, 8, 11),
        (SelectRowKind::SettingsBack, true, 9, 12),
        (SelectRowKind::SettingsClose, true, 10, 13),
        (SelectRowKind::Config, true, 0, 2),
    ];

    for (kind, in_library, image_index, text_index) in cases {
        let row = SelectRowSnapshot {
            kind,
            in_library,
            is_folder: matches!(
                kind,
                SelectRowKind::Folder
                    | SelectRowKind::TableFolder
                    | SelectRowKind::SearchFolder
                    | SelectRowKind::Command
                    | SelectRowKind::Container
                    | SelectRowKind::SettingsRoot
                    | SelectRowKind::SettingsFolder
                    | SelectRowKind::SettingsBack
                    | SelectRowKind::SettingsClose
            ),
            ..SelectRowSnapshot::default()
        };
        assert_eq!(select_row_bar_image_index(&row), image_index, "image index for {kind:?}");
        assert_eq!(select_row_bar_text_index(&row), text_index, "text index for {kind:?}");
    }
}

#[test]
fn select_settings_rows_use_dedicated_slots_with_legacy_fallbacks() {
    let search = SelectRowSnapshot {
        kind: SelectRowKind::SearchFolder,
        is_folder: true,
        ..SelectRowSnapshot::default()
    };
    let settings_root = SelectRowSnapshot {
        kind: SelectRowKind::SettingsRoot,
        is_folder: true,
        ..SelectRowSnapshot::default()
    };
    let settings_folder = SelectRowSnapshot {
        kind: SelectRowKind::SettingsFolder,
        is_folder: true,
        ..SelectRowSnapshot::default()
    };
    let settings_back = SelectRowSnapshot {
        kind: SelectRowKind::SettingsBack,
        is_folder: true,
        ..SelectRowSnapshot::default()
    };
    let settings_close = SelectRowSnapshot {
        kind: SelectRowKind::SettingsClose,
        is_folder: true,
        ..SelectRowSnapshot::default()
    };

    assert_eq!(select_row_bar_image_index(&search), 6);
    assert_eq!(select_row_bar_text_index(&search), 10);
    assert_eq!(select_row_bar_image_fallback_indices(&search), &[1]);
    assert_eq!(select_row_bar_text_fallback_indices(&search), &[4]);
    let cases: [(&SelectRowSnapshot, usize, usize, &[usize], &[usize]); 4] = [
        (&settings_root, 8, 11, &[6, 1], &[10, 4]),
        (&settings_folder, 8, 11, &[1], &[4]),
        (&settings_back, 9, 12, &[6, 1], &[10, 4]),
        (&settings_close, 10, 13, &[6, 1], &[10, 4]),
    ];
    for (row, image, text, fallback_images, fallback_texts) in cases {
        assert_eq!(select_row_bar_image_index(row), image);
        assert_eq!(select_row_bar_text_index(row), text);
        assert_eq!(select_row_bar_image_fallback_indices(row), fallback_images);
        assert_eq!(select_row_bar_text_fallback_indices(row), fallback_texts);
    }
}

#[test]
fn select_settings_image_slots_follow_legacy_search_and_folder_fallbacks() {
    let dedicated_slots: Vec<_> = (0..11).collect();
    for primary in [8, 9, 10] {
        assert_eq!(
            select_row_slot_with_fallbacks(&dedicated_slots, primary, &[6, 1]).copied(),
            Some(primary)
        );
    }

    // beatoraja skin may define index 7 as a legacy no-song bar. BMZ settings
    // slots start after all eight existing entries, so ECFN-style arrays
    // continue to use the search bar at index 6.
    let beatoraja_slots: Vec<_> = (0..8).collect();
    for primary in [8, 9, 10] {
        assert_eq!(
            select_row_slot_with_fallbacks(&beatoraja_slots, primary, &[6, 1]).copied(),
            Some(6)
        );
    }

    let folder_slots: Vec<_> = (0..2).collect();
    assert_eq!(select_row_slot_with_fallbacks(&folder_slots, 8, &[6, 1]).copied(), Some(1));

    assert_eq!(select_row_slot_with_fallbacks(&[0], 8, &[6, 1]).copied(), Some(0));
    assert_eq!(select_row_slot_with_fallbacks::<usize>(&[], 8, &[6, 1]), None);
}

#[test]
fn select_bar_type_ops_match_song_folder_and_course_rows() {
    let song = SkinDrawState {
        select_row_kind: SelectRowKind::Song,
        select_is_folder: false,
        ..SkinDrawState::default()
    };
    let folder = SkinDrawState {
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let table_folder = SkinDrawState {
        select_row_kind: SelectRowKind::TableFolder,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let search_folder = SkinDrawState {
        select_row_kind: SelectRowKind::SearchFolder,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let settings_folder = SkinDrawState {
        select_row_kind: SelectRowKind::SettingsFolder,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let settings_root = SkinDrawState {
        select_row_kind: SelectRowKind::SettingsRoot,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let settings_back = SkinDrawState {
        select_row_kind: SelectRowKind::SettingsBack,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let settings_close = SkinDrawState {
        select_row_kind: SelectRowKind::SettingsClose,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let command = SkinDrawState {
        select_row_kind: SelectRowKind::Command,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let container = SkinDrawState {
        select_row_kind: SelectRowKind::Container,
        select_is_folder: true,
        ..SkinDrawState::default()
    };
    let executable = SkinDrawState {
        select_row_kind: SelectRowKind::Executable,
        select_is_folder: false,
        ..SkinDrawState::default()
    };
    let random_course = SkinDrawState {
        select_row_kind: SelectRowKind::RandomCourse,
        select_is_folder: false,
        ..SkinDrawState::default()
    };
    let course = SkinDrawState {
        select_row_kind: SelectRowKind::Course,
        select_is_folder: false,
        ..SkinDrawState::default()
    };
    let unowned_song = SkinDrawState {
        select_row_kind: SelectRowKind::Song,
        select_is_folder: false,
        select_in_library: false,
        ..SkinDrawState::default()
    };

    assert!(test_skin_op(2, &[], &song));
    assert!(test_skin_op(2, &[], &unowned_song));
    assert!(!test_skin_op(1, &[], &song));
    assert!(!test_skin_op(3, &[], &song));
    assert!(test_skin_op(1, &[], &folder));
    assert!(test_skin_op(1, &[], &table_folder));
    assert!(test_skin_op(1, &[], &search_folder));
    assert!(test_skin_op(1, &[], &settings_root));
    assert!(test_skin_op(1, &[], &settings_folder));
    assert!(test_skin_op(1, &[], &settings_back));
    assert!(test_skin_op(1, &[], &settings_close));
    assert!(test_skin_op(SKIN_OPTION_BMZ_SETTINGS_FOLDER, &[], &settings_root));
    assert!(test_skin_op(SKIN_OPTION_BMZ_SETTINGS_FOLDER, &[], &settings_folder));
    assert!(!test_skin_op(SKIN_OPTION_BMZ_SETTINGS_FOLDER, &[], &settings_back));
    assert!(test_skin_op(SKIN_OPTION_BMZ_SETTINGS_BACK, &[], &settings_back));
    assert!(test_skin_op(SKIN_OPTION_BMZ_SETTINGS_CLOSE, &[], &settings_close));
    assert!(test_skin_op(1, &[], &command));
    assert!(test_skin_op(1, &[], &container));
    assert!(!test_skin_op(2, &[], &folder));
    assert!(test_skin_op(3, &[], &course));
    assert!(!test_skin_op(2, &[], &course));
    assert!(test_skin_op(1030, &[], &executable));
    assert!(!test_skin_op(1030, &[], &random_course));
    assert!(test_skin_op(1031, &[], &random_course));
    assert!(!test_skin_op(1031, &[], &course));
}

#[test]
fn select_settings_row_ref_distinguishes_folder_back_and_close() {
    let cases = [
        (SelectRowKind::Song, 0),
        (SelectRowKind::SettingsRoot, 1),
        (SelectRowKind::SettingsFolder, 1),
        (SelectRowKind::SettingsBack, 2),
        (SelectRowKind::SettingsClose, 3),
    ];

    for (kind, expected) in cases {
        let state = SkinDrawState {
            select_screen: true,
            select_row_kind: kind,
            ..SkinDrawState::default()
        };
        assert_eq!(skin_state_event_index(SKIN_REF_BMZ_SELECT_SETTINGS_ROW_KIND, &state), expected);
        assert_eq!(
            skin_state_number(SKIN_REF_BMZ_SELECT_SETTINGS_ROW_KIND, &state),
            Some(i64::from(expected))
        );
    }
}

#[test]
fn table_song_op_matches_table_context() {
    let table_song = SkinDrawState { table_song: true, ..SkinDrawState::default() };
    let non_table_song = SkinDrawState::default();

    assert!(test_skin_op(1008, &[], &table_song));
    assert!(test_skin_op(-1008, &[], &non_table_song));
    assert!(!test_skin_op(1008, &[], &non_table_song));
}

#[test]
fn select_row_trophy_index_prefers_achieved_course_trophy_names() {
    let row = SelectRowSnapshot {
        kind: SelectRowKind::Course,
        achieved_trophy_names: vec!["bronzemedal".to_string(), "goldmedal".to_string()],
        ex_score: Some(0),
        total_notes: 100,
        ..SelectRowSnapshot::default()
    };
    assert_eq!(select_row_trophy_index(&row), Some(2));

    let silver = SelectRowSnapshot {
        kind: SelectRowKind::Course,
        achieved_trophy_names: vec!["silvermedal".to_string()],
        ..SelectRowSnapshot::default()
    };
    assert_eq!(select_row_trophy_index(&silver), Some(1));

    let high_score_without_trophy = SelectRowSnapshot {
        kind: SelectRowKind::Course,
        total_notes: 100,
        ex_score: Some(200),
        ..SelectRowSnapshot::default()
    };
    assert_eq!(select_row_trophy_index(&high_score_without_trophy), None);
}

#[test]
fn playable_bar_op_matches_library_presence() {
    let owned_song = SkinDrawState {
        select_row_kind: SelectRowKind::Song,
        select_is_folder: false,
        select_in_library: true,
        ..SkinDrawState::default()
    };
    let unowned_song = SkinDrawState {
        select_row_kind: SelectRowKind::Song,
        select_is_folder: false,
        select_in_library: false,
        ..SkinDrawState::default()
    };
    let owned_course = SkinDrawState {
        select_row_kind: SelectRowKind::Course,
        select_is_folder: false,
        select_in_library: true,
        ..SkinDrawState::default()
    };
    let unowned_course = SkinDrawState {
        select_row_kind: SelectRowKind::Course,
        select_is_folder: false,
        select_in_library: false,
        ..SkinDrawState::default()
    };
    let owned_random_course = SkinDrawState {
        select_row_kind: SelectRowKind::RandomCourse,
        select_is_folder: false,
        select_in_library: true,
        ..SkinDrawState::default()
    };
    let executable = SkinDrawState {
        select_row_kind: SelectRowKind::Executable,
        select_is_folder: false,
        select_in_library: false,
        ..SkinDrawState::default()
    };
    let folder = SkinDrawState {
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        select_in_library: true,
        ..SkinDrawState::default()
    };

    assert!(test_skin_op(5, &[], &owned_song));
    assert!(!test_skin_op(5, &[], &unowned_song));
    assert!(test_skin_op(5, &[], &owned_course));
    assert!(!test_skin_op(5, &[], &unowned_course));
    assert!(test_skin_op(5, &[], &owned_random_course));
    assert!(test_skin_op(5, &[], &executable));
    assert!(!test_skin_op(5, &[], &folder));
    assert!(!test_skin_op(-5, &[], &owned_song));
    assert!(test_skin_op(-5, &[], &unowned_song));
    assert!(test_skin_op(-5, &[], &folder));
}

#[test]
fn select_banner_ops_follow_selected_banner_presence() {
    let no_banner =
        SkinDrawState { select_screen: true, select_has_banner: false, ..SkinDrawState::default() };
    let with_banner =
        SkinDrawState { select_screen: true, select_has_banner: true, ..SkinDrawState::default() };
    let play_screen =
        SkinDrawState { select_screen: false, select_has_banner: true, ..SkinDrawState::default() };

    assert!(test_skin_op(192, &[], &no_banner));
    assert!(!test_skin_op(193, &[], &no_banner));
    assert!(!test_skin_op(192, &[], &with_banner));
    assert!(test_skin_op(193, &[], &with_banner));
    assert!(!test_skin_op(192, &[], &play_screen));
    assert!(!test_skin_op(193, &[], &play_screen));

    assert!(test_skin_ops(&[2, 192], &[], &no_banner));
    assert!(!test_skin_ops(&[2, 193], &[], &no_banner));
    assert!(!test_skin_ops(&[2, 192], &[], &with_banner));
    assert!(test_skin_ops(&[2, 193], &[], &with_banner));
}

#[test]
fn result_panel_draw_condition_uses_runtime_selection() {
    let ir = SkinDrawState { result_panel: Some(1), ..Default::default() };
    let graph = SkinDrawState { result_panel: Some(2), ..Default::default() };
    assert!(eval_skin_draw_condition("result_panel(1)", &ir));
    assert!(!eval_skin_draw_condition("result_panel(2)", &ir));
    assert!(eval_skin_draw_condition("result_panel(0) or result_panel(2)", &graph,));
}

#[test]
fn select_score_available_requires_an_actual_score_record() {
    let folder = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        select_in_library: true,
        select_ex_score: Some(1234),
        ..SkinDrawState::default()
    };
    let zero_score = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_ex_score: Some(0),
        ..SkinDrawState::default()
    };
    let out_of_library = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: false,
        select_ex_score: Some(1234),
        ..SkinDrawState::default()
    };

    assert!(!eval_skin_draw_condition("select_score_available()", &folder));
    assert!(eval_skin_draw_condition("select_score_available()", &zero_score));
    assert!(!eval_skin_draw_condition("select_score_available()", &out_of_library));
}

#[test]
fn select_rank_ops_reflect_selected_ex_score() {
    let aa_state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_ex_score: Some(1556),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };
    let max_state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_ex_score: Some(2000),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };
    let f_state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_ex_score: Some(300),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };

    assert!(test_skin_op(201, &[], &aa_state));
    assert!(test_skin_op(302, &[], &aa_state));
    assert!(!test_skin_op(200, &[], &aa_state));
    assert!(test_skin_op(-200, &[], &aa_state));
    assert!(test_skin_op(200, &[], &max_state));
    assert!(test_skin_op(300, &[], &max_state));
    assert!(test_skin_op(207, &[], &f_state));
    assert!(!test_skin_op(307, &[], &f_state));
    assert!(!test_skin_op(200, &[], &SkinDrawState::default()));
}

#[test]
fn select_rank_ops_are_false_for_folder_rows() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        select_in_library: true,
        select_ex_score: Some(1556),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };

    assert!(!test_skin_op(201, &[], &state));
    assert!(!test_skin_op(302, &[], &state));
}

#[test]
fn select_key_mode_op_160_requires_song_row_key_mode() {
    let config_row = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Config,
        in_settings: true,
        ..SkinDrawState::default()
    };
    assert!(!test_skin_op(160, &[], &config_row));

    let song_7k = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_chart_key_mode: Some(KeyMode::K7),
        ..SkinDrawState::default()
    };
    assert!(test_skin_op(160, &[], &song_7k));
    assert!(!test_skin_op(161, &[], &song_7k));
    assert!(test_skin_op(SKIN_OPTION_BMZ_KEY_MODE_BASE + 3, &[], &song_7k));
    assert!(test_skin_op(SKIN_OPTION_BMZ_SINGLE_PLAY, &[], &song_7k));
    assert!(!test_skin_op(SKIN_OPTION_BMZ_NO_SCRATCH, &[], &song_7k));
    assert_eq!(skin_state_number(SKIN_REF_BMZ_KEY_MODE, &song_7k), Some(7));
    assert_eq!(skin_state_number(SKIN_REF_BMZ_ACTIVE_LANE_COUNT, &song_7k), Some(8));

    let folder = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Folder,
        select_chart_key_mode: Some(KeyMode::K7),
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(SKIN_REF_BMZ_KEY_MODE, &folder), None);
    assert!(!test_skin_op(SKIN_OPTION_BMZ_KEY_MODE_BASE + 3, &[], &folder));
}

#[test]
fn select_settings_screen_hides_bpm_numbers() {
    let state = SkinDrawState {
        select_screen: true,
        in_settings: true,
        select_max_bpm: 180.0,
        select_min_bpm: 120.0,
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(90, &state), None);
    assert_eq!(skin_state_number(91, &state), None);
}

#[test]
fn select_settings_screen_volume_numbers_match_beatoraja_refs() {
    let state = SkinDrawState {
        select_screen: true,
        in_settings: true,
        select_master_volume: 0.42,
        select_key_volume: 0.73,
        select_bgm_volume: 0.18,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(57, &state), Some(42));
    assert_eq!(skin_state_number(58, &state), Some(73));
    assert_eq!(skin_state_number(59, &state), Some(18));
}

#[test]
fn select_rank_and_judge_ops_are_hidden_in_settings() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Config,
        select_in_library: true,
        select_ex_score: Some(1556),
        select_total_notes: 1000,
        judge_rank: Some(2),
        in_settings: true,
        ..SkinDrawState::default()
    };

    assert!(!test_skin_op(200, &[], &state));
    assert!(!test_skin_op(201, &[], &state));
    assert!(!test_skin_op(302, &[], &state));
    assert!(!test_skin_op(180, &[], &state));
}

#[test]
fn select_detail_artist_shows_config_value_in_settings() {
    let snapshot = SelectSnapshot {
        in_settings: true,
        settings_editing: true,
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            title: "MASTER".to_string(),
            artist: "25".to_string(),
            kind: SelectRowKind::Config,
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };
    let row = &snapshot.rows[0];
    assert_eq!(select_detail_artist(&snapshot, Some(row)), "25");
    assert_eq!(select_detail_subtitle(&snapshot, Some(row)), "[編集中]");
    assert_eq!(
        skin_state_text(
            &SkinTextDef { id: "t".to_string(), ref_id: 3, ..SkinTextDef::default() },
            &SkinTextState { target: "", ..SkinTextState::default() },
        ),
        ""
    );
}

#[test]
fn nearest_select_diff_number_renders_e_minus_when_f_rank_destination_is_missing() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [
                    {"id": "rank", "path": "rank.png"}
                ],
                "image": [
                    {"id": "RANK_s_E", "src": "rank", "x": 0, "y": 0, "w": 45, "h": 19}
                ],
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
                        "digit": 4,
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
    let sources = HashMap::from([
        (
            "num".to_string(),
            SkinDocumentTexture {
                source_id: "num".to_string(),
                texture: SkinTextureId(42),
                source_size: SkinImageSize { width: 120.0, height: 40.0 },
            },
        ),
        (
            "rank".to_string(),
            SkinDocumentTexture {
                source_id: "rank".to_string(),
                texture: SkinTextureId(7),
                source_size: SkinImageSize { width: 45.0, height: 19.0 },
            },
        ),
    ]);
    let snapshot = SelectSnapshot {
        rows: vec![SelectRowSnapshot {
            index: 0,
            ex_score: Some(100),
            total_notes: 1000,
            in_library: true,
            ..SelectRowSnapshot::default()
        }],
        chart_count: 1,
        grade_diff_display: ResultGradeDiffDisplay::Nearest,
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);
    let first_digit_uv = items.iter().find_map(|item| match item {
        SkinRenderItem::Image { texture: SkinTextureId(42), uv, .. } => Some(*uv),
        _ => None,
    });

    assert_eq!(first_digit_uv.map(|uv| uv.y), Some(0.0));
    assert!(
        items
            .iter()
            .any(|item| matches!(item, SkinRenderItem::Image { texture: SkinTextureId(7), .. }))
    );
}

#[test]
fn next_select_diff_number_renders_next_rank_label() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [
                    {"id": "rank", "path": "rank.png"}
                ],
                "image": [
                    {"id": "RANK_s_E", "src": "rank", "x": 0, "y": 0, "w": 45, "h": 19}
                ],
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
                        "digit": 4,
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
    let sources = HashMap::from([
        (
            "num".to_string(),
            SkinDocumentTexture {
                source_id: "num".to_string(),
                texture: SkinTextureId(42),
                source_size: SkinImageSize { width: 120.0, height: 40.0 },
            },
        ),
        (
            "rank".to_string(),
            SkinDocumentTexture {
                source_id: "rank".to_string(),
                texture: SkinTextureId(7),
                source_size: SkinImageSize { width: 45.0, height: 19.0 },
            },
        ),
    ]);
    let snapshot = SelectSnapshot {
        rows: vec![SelectRowSnapshot {
            index: 0,
            ex_score: Some(0),
            play_count: 1,
            total_notes: 2253,
            in_library: true,
            ..SelectRowSnapshot::default()
        }],
        chart_count: 1,
        grade_diff_display: ResultGradeDiffDisplay::Next,
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);
    let first_digit_uv = items.iter().find_map(|item| match item {
        SkinRenderItem::Image { texture: SkinTextureId(42), uv, .. } => Some(*uv),
        _ => None,
    });

    let (state, _) = document.select_draw_state(&snapshot, None);
    assert_eq!(skin_state_number(154, &state), Some(-501));
    assert_eq!(first_digit_uv.map(|uv| uv.y), Some(0.0));
    assert!(
        items
            .iter()
            .any(|item| matches!(item, SkinRenderItem::Image { texture: SkinTextureId(7), .. }))
    );

    let no_play_snapshot = SelectSnapshot {
        rows: vec![SelectRowSnapshot {
            index: 0,
            ex_score: None,
            play_count: 0,
            total_notes: 2253,
            in_library: true,
            ..SelectRowSnapshot::default()
        }],
        chart_count: 1,
        grade_diff_display: ResultGradeDiffDisplay::Next,
        ..SelectSnapshot::default()
    };
    let no_play_items = document.select_render_items(&sources, &no_play_snapshot);
    let (no_play_state, _) = document.select_draw_state(&no_play_snapshot, None);
    assert_eq!(skin_state_number(154, &no_play_state), None);
    assert!(!no_play_items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { texture: SkinTextureId(7) | SkinTextureId(42), .. }
    )));

    let no_play_zero_snapshot = SelectSnapshot {
        rows: vec![SelectRowSnapshot {
            index: 0,
            ex_score: Some(0),
            play_count: 0,
            total_notes: 2253,
            in_library: true,
            ..SelectRowSnapshot::default()
        }],
        chart_count: 1,
        grade_diff_display: ResultGradeDiffDisplay::Next,
        ..SelectSnapshot::default()
    };
    let no_play_zero_items = document.select_render_items(&sources, &no_play_zero_snapshot);
    let (no_play_zero_state, _) = document.select_draw_state(&no_play_zero_snapshot, None);
    assert_eq!(skin_state_number(154, &no_play_zero_state), None);
    assert!(!no_play_zero_items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { texture: SkinTextureId(7) | SkinTextureId(42), .. }
    )));
}

#[test]
fn select_diff_number_renders_max_zero_as_positive_row() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
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
                        "digit": 4,
                        "ref": 154,
                        "zeropadding": 2
                    }
                ],
                "destination": [
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
    let snapshot = SelectSnapshot {
        rows: vec![SelectRowSnapshot {
            index: 0,
            ex_score: Some(2000),
            total_notes: 1000,
            in_library: true,
            ..SelectRowSnapshot::default()
        }],
        chart_count: 1,
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);
    let first_digit_uv = items.iter().find_map(|item| match item {
        SkinRenderItem::Image { texture: SkinTextureId(42), uv, .. } => Some(*uv),
        _ => None,
    });

    let (state, _) = document.select_draw_state(&snapshot, None);
    assert_eq!(skin_state_number(154, &state), Some(0));
    assert_eq!(first_digit_uv.map(|uv| uv.y), Some(0.5));
}

#[test]
fn select_replay_ops_reflect_replay_slots_and_selection() {
    let no_replay = SkinDrawState::default();
    let first_replay = SkinDrawState {
        select_replay_slots: [true, false, false, false],
        select_replay_index: Some(0),
        ..SkinDrawState::default()
    };
    let second_replay = SkinDrawState {
        select_replay_slots: [false, true, false, false],
        select_replay_index: Some(1),
        ..SkinDrawState::default()
    };

    assert!(test_skin_op(196, &[], &no_replay));
    assert!(!test_skin_op(197, &[], &no_replay));
    assert!(!test_skin_op(1205, &[], &no_replay));
    assert!(test_skin_op(197, &[], &first_replay));
    assert!(!test_skin_op(196, &[], &first_replay));
    assert!(test_skin_op(1205, &[], &first_replay));
    assert!(test_skin_op(-1205, &[], &no_replay));
    assert!(test_skin_op(1197, &[], &second_replay));
    assert!(test_skin_op(1206, &[], &second_replay));
    assert!(!test_skin_op(1205, &[], &second_replay));
    assert!(!test_skin_op(198, &[], &first_replay));
}

#[test]
fn select_row_snapshot_carries_achieved_trophy_names() {
    // SelectRowSnapshot is the carrier — SkinDrawState intentionally does
    // not duplicate this field (it must stay Copy).  This test simply
    // pins down that course rows preserve the data and song rows default
    // to empty, so future skin ops have a stable contract to consume.
    use crate::scene::{SelectRowKind, SelectRowSnapshot};
    let course = SelectRowSnapshot {
        kind: SelectRowKind::Course,
        achieved_trophy_names: vec!["gold".to_string(), "silver".to_string()],
        ..SelectRowSnapshot::default()
    };
    let song = SelectRowSnapshot { kind: SelectRowKind::Song, ..SelectRowSnapshot::default() };

    assert_eq!(course.achieved_trophy_names, vec!["gold".to_string(), "silver".to_string()]);
    assert!(song.achieved_trophy_names.is_empty());
}

#[test]
fn select_row_replay_index_is_row_kind_agnostic() {
    // Regression: course rows must surface their replay slot indicators
    // exactly like song rows.  `select_row_replay_index` looks only at
    // `row.replay_slots`, so swapping row.kind must not change the
    // result.  This locks the invariant for future refactors.
    use crate::scene::{SelectRowKind, SelectRowSnapshot};
    let song = SelectRowSnapshot {
        kind: SelectRowKind::Song,
        replay_slots: [false, true, false, true],
        ..SelectRowSnapshot::default()
    };
    let course = SelectRowSnapshot {
        kind: SelectRowKind::Course,
        replay_slots: [false, true, false, true],
        ..SelectRowSnapshot::default()
    };

    assert_eq!(select_row_replay_index(&song), Some(1));
    assert_eq!(select_row_replay_index(&course), Some(1));
}

#[test]
fn peaceful_gauge_value_overlay_selects_exactly_one_integer_width() {
    for (state, mode, expected_digits) in [
        (SkinDrawState { gauge: 7.5, gauge_max: 120.0, ..Default::default() }, "percent", 1),
        (SkinDrawState { gauge: 78.75, gauge_max: 120.0, ..Default::default() }, "percent", 2),
        (SkinDrawState { gauge: 120.0, gauge_max: 120.0, ..Default::default() }, "percent", 3),
        (SkinDrawState { gauge: 7.5, gauge_max: 120.0, ..Default::default() }, "amount", 1),
        (SkinDrawState { gauge: 78.75, gauge_max: 120.0, ..Default::default() }, "amount", 2),
        (SkinDrawState { gauge: 120.0, gauge_max: 120.0, ..Default::default() }, "amount", 3),
    ] {
        let visible = (1..=3)
            .filter(|digits| {
                eval_skin_draw_condition(&format!("gauge_value_digits({mode},{digits})"), &state)
            })
            .collect::<Vec<_>>();
        assert_eq!(visible, vec![expected_digits]);
    }
}

#[test]
fn skin_context_updates_user_selected_options() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "property": [
                    { "name": "Side", "def": "1P", "item": [
                        { "name": "1P", "op": 920 },
                        { "name": "2P", "op": 921 }
                    ]}
                ]
            }
            "#,
    )
    .unwrap();
    let mut context =
        SkinContext::from_manifest_and_document(default_skin_manifest(), document, []);

    assert_eq!(context.document().unwrap().enabled_options(), [920]);
    assert!(context.set_user_selected_options(vec![921]));
    assert_eq!(context.document().unwrap().enabled_options(), [921]);
}

#[test]
fn skin_document_selects_hcn_body_by_state() {
    // 旧形式 HCN: [6]=hcnbody(processing) [7]=hcnactive(inactive)
    // [8]=hcndamage(回復中) [9]=hcnreactive(減衰中)
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "source": [{ "id": 1, "path": "notes.png" }],
                "image": [
                    { "id": "hb", "src": 1, "x": 10, "y": 0, "w": 10, "h": 1 },
                    { "id": "ha", "src": 1, "x": 20, "y": 0, "w": 10, "h": 1 },
                    { "id": "hd", "src": 1, "x": 30, "y": 0, "w": 10, "h": 1 },
                    { "id": "hr", "src": 1, "x": 40, "y": 0, "w": 10, "h": 1 }
                ],
                "note": {
                    "id": "notes",
                    "note": ["hb", "hb", "hb", "hb", "hb", "hb", "hb", "hb"],
                    "hcnbody": ["hb", "hb", "hb", "hb", "hb", "hb", "hb", "hb"],
                    "hcnactive": ["ha", "ha", "ha", "ha", "ha", "ha", "ha", "ha"],
                    "hcndamage": ["hd", "hd", "hd", "hd", "hd", "hd", "hd", "hd"],
                    "hcnreactive": ["hr", "hr", "hr", "hr", "hr", "hr", "hr", "hr"]
                }
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 100.0, height: 50.0 },
        },
    )]);
    let rect = Rect { x: 0.0, y: 0.0, width: 0.1, height: 0.1 };
    let render_x = |state: LongBodyState| {
        let item = document
            .note_long_body_render_item(
                Lane::Scratch,
                KeyMode::K7,
                rect,
                LongNoteMode::Hcn,
                state,
                &SkinDrawState::default(),
                &sources,
            )
            .unwrap();
        match item {
            SkinRenderItem::Image { uv: TextureRegion { x, .. }, .. } => x,
            _ => panic!("expected image item"),
        }
    };

    assert!(approx_eq(render_x(LongBodyState::Processing), 0.1)); // hcnbody
    assert!(approx_eq(render_x(LongBodyState::Inactive), 0.2)); // hcnactive
    assert!(approx_eq(render_x(LongBodyState::HcnActive), 0.3)); // hcndamage
    assert!(approx_eq(render_x(LongBodyState::HcnDamage), 0.4)); // hcnreactive
}

#[test]
fn skin_gauge_sprite_selects_exhard_nodes_and_tip_frame() {
    let mut document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "gauge.png" }],
                "image": [],
                "gauge": { "id": "gauge", "nodes": [], "parts": 4, "type": 3, "cycle": 33 },
                "destination": [
                    { "id": "gauge", "dst": [{ "x": 0, "y": 0, "w": 40, "h": 10 }] }
                ]
            }
            "#,
    )
    .unwrap();
    document.gauge.as_mut().unwrap().nodes = (0..36).map(|index| format!("node-{index}")).collect();
    document.image = (0..36)
        .map(|index| SkinImageDef {
            id: format!("node-{index}"),
            src: "1".to_string(),
            x: index,
            y: 0,
            w: 1,
            h: 1,
            divx: 1,
            divy: 1,
            timer: None,
            cycle: 0,
            len: 0,
            ref_id: 0,
            click: 0,
            act: None,
            clickable: None,
        })
        .collect();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 36.0, height: 1.0 },
        },
    )]);
    let items = document
        .static_image_render_items(
            &sources,
            &SkinDrawState {
                elapsed_ms: 1_000,
                gauge: 75.0,
                gauge_max: 100.0,
                gauge_border: 1.0,
                gauge_type: 4,
                ..Default::default()
            },
        )
        .into_iter()
        .filter_map(|item| match item {
            SkinRenderItem::Image { .. } => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(items.len(), 5, "4 parts + flickering tip overlay");
    let tip_flicker = items.iter().find_map(|item| match item {
        SkinRenderItem::Image { uv, blend: BlendMode::Normal, .. } if uv.x > 0.7 => Some(uv.x),
        _ => None,
    });
    assert!(
        tip_flicker.is_some(),
        "EX-HARD flickering tip should use node index 28+ (normal blend overlay)"
    );
}

#[test]
fn select_skin_document_renders_songlist_rows() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [
                    { "id": 1, "path": "bar.png" },
                    { "id": 2, "path": "num.png" },
                    { "id": 3, "path": "lamp.png" },
                    { "id": 4, "path": "graph.png" }
                ],
                "image": [
                    { "id": "bar-song", "src": 1, "x": 0, "y": 0, "w": 40, "h": 10 },
                    { "id": "bar-folder", "src": 1, "x": 0, "y": 10, "w": 40, "h": 10 },
                    { "id": "bar-table", "src": 1, "x": 0, "y": 30, "w": 40, "h": 10 },
                    { "id": "song-op-marker", "src": 1, "x": 0, "y": 20, "w": 4, "h": 4 },
                    { "id": "folder-op-marker", "src": 1, "x": 4, "y": 20, "w": 4, "h": 4 },
                    { "id": "trophy-bronze", "src": 3, "x": 0, "y": 0, "w": 4, "h": 4 },
                    { "id": "trophy-silver", "src": 3, "x": 4, "y": 0, "w": 4, "h": 4 },
                    { "id": "trophy-gold", "src": 3, "x": 8, "y": 0, "w": 4, "h": 4 },
                    { "id": "lamp-none", "src": 3, "x": 0, "y": 0, "w": 4, "h": 4 },
                    { "id": "lamp-failed", "src": 3, "x": 4, "y": 0, "w": 4, "h": 4 },
                    { "id": "lamp-assist", "src": 3, "x": 8, "y": 0, "w": 4, "h": 4 },
                    { "id": "lamp-light-assist", "src": 3, "x": 12, "y": 0, "w": 4, "h": 4 },
                    { "id": "lamp-easy", "src": 3, "x": 16, "y": 0, "w": 4, "h": 4 },
                    { "id": "lamp-normal", "src": 3, "x": 20, "y": 0, "w": 4, "h": 4 },
                    { "id": "label-ln", "src": 1, "x": 0, "y": 40, "w": 4, "h": 4 },
                    { "id": "label-random", "src": 1, "x": 4, "y": 40, "w": 4, "h": 4 },
                    { "id": "label-mine", "src": 1, "x": 8, "y": 40, "w": 4, "h": 4 }
                ],
                "imageset": [{ "id": "bar", "images": ["bar-song", "bar-folder", "bar-table"] }],
                "text": [
                    { "id": "bartext", "font": "main", "size": 10 },
                    { "id": "bartext1", "font": "folder", "size": 10 },
                    { "id": "bartext2", "font": "table", "size": 10 },
                    { "id": "bartext3", "font": "main", "size": 10 },
                    { "id": "bartext4", "font": "folder", "size": 10 }
                ],
                "value": [
                    { "id": "level-other", "src": 2, "x": 0, "y": 0, "w": 100, "h": 10, "divx": 10, "digit": 2 },
                    { "id": "level-beginner", "src": 2, "x": 0, "y": 10, "w": 100, "h": 10, "divx": 10, "digit": 2 },
                    { "id": "level-normal", "src": 2, "x": 0, "y": 20, "w": 100, "h": 10, "divx": 10, "digit": 2 }
                ],
                "graph": [{ "id": "graph-lamp", "src": 4, "x": 0, "y": 0, "w": 44, "h": 4, "divx": 11, "angle": 0, "type": -1 }],
                "songlist": {
                    "id": "songlist",
                    "center": 1,
                    "listoff": [
                        { "id": "bar", "dst": [{ "x": 10, "y": 70, "w": 40, "h": 10 }] },
                        { "id": "bar", "dst": [{ "x": 10, "y": 50, "w": 40, "h": 10 }] },
                        { "id": "bar", "dst": [{ "x": 10, "y": 30, "w": 40, "h": 10 }] }
                    ],
                    "liston": [
                        { "id": "bar", "dst": [{ "x": 12, "y": 70, "w": 40, "h": 10 }] },
                        { "id": "bar", "dst": [{ "x": 12, "y": 50, "w": 40, "h": 10 }] },
                        { "id": "bar", "dst": [{ "x": 12, "y": 30, "w": 40, "h": 10 }] }
                    ],
                    "text": [
                        { "id": "bartext", "dst": [{ "x": 1, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext", "dst": [{ "x": 2, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext", "dst": [{ "x": 5, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext", "dst": [{ "x": 6, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext4", "dst": [{ "x": 7, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext4", "dst": [{ "x": 8, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext2", "dst": [{ "x": 9, "y": 2, "w": 20, "h": 8 }] }
                    ],
                    "judgegraph": [
                        { "id": "song-op-marker", "op": [2], "dst": [{ "x": 8, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "folder-op-marker", "op": [1], "dst": [{ "x": 12, "y": 1, "w": 4, "h": 4 }] }
                    ],
                    "level": [
                        { "id": "level-other", "dst": [{ "x": 30, "y": 2, "w": 5, "h": 8 }] },
                        { "id": "level-beginner", "dst": [{ "x": 30, "y": 2, "w": 5, "h": 8 }] },
                        { "id": "level-normal", "dst": [{ "x": 30, "y": 2, "w": 5, "h": 8 }] }
                    ],
                    "trophy": [
                        { "id": "trophy-bronze", "dst": [{ "x": 35, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "trophy-silver", "dst": [{ "x": 35, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "trophy-gold", "dst": [{ "x": 35, "y": 1, "w": 4, "h": 4 }] }
                    ],
                    "label": [
                        { "id": "label-ln", "dst": [{ "x": 40, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "label-random", "dst": [{ "x": 44, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "label-mine", "dst": [{ "x": 48, "y": 1, "w": 4, "h": 4 }] }
                    ],
                    "graph": { "id": "graph-lamp", "dst": [{ "x": 5, "y": 1, "w": 20, "h": 2 }] },
                    "lamp": [
                        { "id": "lamp-none", "dst": [{ "x": 1, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-failed", "dst": [{ "x": 1, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-assist", "dst": [{ "x": 1, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-light-assist", "dst": [{ "x": 1, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-easy", "dst": [{ "x": 1, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-normal", "dst": [{ "x": 1, "y": 1, "w": 4, "h": 4 }] }
                    ],
                    "playerlamp": [
                        { "id": "lamp-none", "dst": [{ "x": 60, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-failed", "dst": [{ "x": 60, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-assist", "dst": [{ "x": 60, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-light-assist", "dst": [{ "x": 60, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-easy", "dst": [{ "x": 60, "y": 1, "w": 4, "h": 4 }] },
                        { "id": "lamp-normal", "dst": [{ "x": 60, "y": 1, "w": 4, "h": 4 }] }
                    ]
                },
                "destination": [{ "id": "songlist" }]
            }
            "#,
        )
        .unwrap();
    let mut sources = mock_source("1", 100.0, 100.0);
    sources.extend(mock_source("2", 100.0, 100.0));
    sources.extend(mock_source("3", 24.0, 4.0));
    sources.extend(mock_source("4", 44.0, 4.0));
    let snapshot = SelectSnapshot {
        selected_index: 2,
        rows: vec![
            SelectRowSnapshot {
                index: 1,
                title: "Folder".to_string(),
                play_level: "0".to_string(),
                clear_type: "Normal".to_string(),
                folder_lamp_counts: {
                    let mut counts = [0; 11];
                    counts[5] = 1;
                    counts[6] = 1;
                    counts
                },
                is_folder: true,
                kind: SelectRowKind::Folder,
                ..SelectRowSnapshot::default()
            },
            SelectRowSnapshot {
                index: 2,
                title: "Song".to_string(),
                difficulty_name: "2".to_string(),
                play_level: "12".to_string(),
                clear_type: "Normal".to_string(),
                total_notes: 100,
                ex_score: Some(180),
                has_long_notes: true,
                has_mines: true,
                ..SelectRowSnapshot::default()
            },
            SelectRowSnapshot {
                index: 3,
                title: "Table".to_string(),
                play_level: "0".to_string(),
                is_folder: true,
                kind: SelectRowKind::TableFolder,
                ..SelectRowSnapshot::default()
            },
        ],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);

    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image { .. })));
    assert!(
        items
            .iter()
            .any(|item| matches!(item, SkinRenderItem::Text { text, .. } if text == "Song"))
    );
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Text {
                origin: Point { x, y },
                text,
                style,
                ..
            } if text == "Folder"
                && style.font_id.as_deref() == Some("folder")
                && approx_eq(*x, 0.17)
                && approx_eq(*y, 0.2))));
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item, SkinRenderItem::Text { text, .. } if text == "Folder"))
            .count(),
        1
    );
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Text {
                text,
                style,
                ..
            } if text == "Table"
                && style.font_id.as_deref() == Some("table"))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                uv: TextureRegion { y: v, .. },
                ..
            } if approx_eq(*v, 30.0 / 100.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, height },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(*x, 0.13)
                && approx_eq(*y, 0.45)
                && approx_eq(*width, 0.04)
                && approx_eq(*height, 0.04)
                && approx_eq(*u, 20.0 / 24.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, height },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(*x, 0.11)
                && approx_eq(*y, 0.25)
                && approx_eq(*width, 0.04)
                && approx_eq(*height, 0.04)
                && approx_eq(*u, 20.0 / 24.0))));
    assert!(!items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, height },
                ..
            } if approx_eq(*x, 0.72)
                && approx_eq(*y, 0.45)
                && approx_eq(*width, 0.04)
                && approx_eq(*height, 0.04))));
    assert!(!items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(*x, 0.47)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 8.0 / 24.0))));
    let course_snapshot = SelectSnapshot {
        selected_index: 4,
        rows: vec![SelectRowSnapshot {
            index: 4,
            title: "Course".to_string(),
            kind: SelectRowKind::Course,
            difficulty_name: "2".to_string(),
            play_level: "12".to_string(),
            total_notes: 100,
            ex_score: Some(200),
            achieved_trophy_names: vec!["goldmedal".to_string()],
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };
    let course_items = document.select_render_items(&sources, &course_snapshot);
    assert!(course_items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(*x, 0.47)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 8.0 / 24.0))));
    assert!(!course_items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.2)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 0.0)
                && approx_eq(*v, 20.0 / 100.0))));
    assert!(!items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, .. },
                uv: TextureRegion { width: u_width, .. },
                ..
            } if approx_eq(*x, 0.17)
                && approx_eq(*y, 0.47)
                && approx_eq(*width, 0.1)
                && approx_eq(*u_width, 0.5))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, .. },
                uv: TextureRegion { x: u, width: u_width, .. },
                ..
            } if approx_eq(*x, 0.15)
                && approx_eq(*y, 0.27)
                && approx_eq(*width, 0.1)
                && approx_eq(*u, 24.0 / 44.0)
                && approx_eq(*u_width, 4.0 / 44.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, .. },
                uv: TextureRegion { x: u, width: u_width, .. },
                ..
            } if approx_eq(*x, 0.25)
                && approx_eq(*y, 0.27)
                && approx_eq(*width, 0.1)
                && approx_eq(*u, 20.0 / 44.0)
                && approx_eq(*u_width, 4.0 / 44.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { y: u, .. },
                ..
            } if approx_eq(*x, 0.47)
                && approx_eq(*y, 0.4)
                && approx_eq(*u, 0.2))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.2)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 0.0)
                && approx_eq(*v, 20.0 / 100.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.52)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 0.0)
                && approx_eq(*v, 40.0 / 100.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.60)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 8.0 / 100.0)
                && approx_eq(*v, 40.0 / 100.0))));
    let scrolling_snapshot =
        SelectSnapshot { bar_scroll_direction: 1, bar_scroll_progress: 0.5, ..snapshot.clone() };
    let scrolling_items = document.select_render_items(&sources, &scrolling_snapshot);
    assert!(scrolling_items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, width, height },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.11)
                && approx_eq(*y, 0.5)
                && approx_eq(*width, 0.4)
                && approx_eq(*height, 0.1)
                && approx_eq(*u, 0.0)
                && approx_eq(*v, 0.0))));
    assert!(!items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.22)
                && approx_eq(*y, 0.45)
                && approx_eq(*u, 4.0 / 100.0)
                && approx_eq(*v, 20.0 / 100.0))));

    let folder_selected = SelectSnapshot { selected_index: 1, ..snapshot };
    let items = document.select_render_items(&sources, &folder_selected);
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.18)
                && approx_eq(*y, 0.65)
                && approx_eq(*u, 0.0)
                && approx_eq(*v, 20.0 / 100.0))));
    assert!(!items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                rect: Rect { x, y, .. },
                uv: TextureRegion { x: u, y: v, .. },
                ..
            } if approx_eq(*x, 0.22)
                && approx_eq(*y, 0.65)
                && approx_eq(*u, 4.0 / 100.0)
                && approx_eq(*v, 20.0 / 100.0))));

    let wrapped_snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![
            SelectRowSnapshot {
                index: 2,
                title: "Last".to_string(),
                play_level: "2".to_string(),
                ..SelectRowSnapshot::default()
            },
            SelectRowSnapshot {
                index: 0,
                title: "First".to_string(),
                play_level: "1".to_string(),
                ..SelectRowSnapshot::default()
            },
            SelectRowSnapshot {
                index: 1,
                title: "Second".to_string(),
                play_level: "2".to_string(),
                ..SelectRowSnapshot::default()
            },
        ],
        ..SelectSnapshot::default()
    };
    let items = document.select_render_items(&sources, &wrapped_snapshot);
    assert!(
        items
            .iter()
            .any(|item| matches!(item, SkinRenderItem::Text { text, .. } if text == "Last"))
    );
    assert!(
        items
            .iter()
            .any(|item| matches!(item, SkinRenderItem::Text { text, .. } if text == "First"))
    );
    assert!(
        items
            .iter()
            .any(|item| matches!(item, SkinRenderItem::Text { text, .. } if text == "Second"))
    );
}

#[test]
fn select_folder_distribution_graph_uses_cycle_animation_row() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "graph.png" }],
                "graph": [
                    { "id": "graph-lamp", "src": 1, "x": 0, "y": 0, "w": 44, "h": 8, "divx": 11, "divy": 2, "cycle": 100, "type": -1 }
                ],
                "songlist": {
                    "id": "songlist",
                    "center": 0,
                    "liston": [{ "id": "row", "dst": [{ "x": 10, "y": 40, "w": 80, "h": 20 }] }],
                    "graph": { "id": "graph-lamp", "dst": [{ "x": 0, "y": 0, "w": 44, "h": 4 }] }
                },
                "destination": [{ "id": "songlist" }]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 44.0, 8.0);
    let snapshot = SelectSnapshot {
        time: TimeUs(50_000),
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            is_folder: true,
            kind: SelectRowKind::Folder,
            folder_lamp_counts: {
                let mut counts = [0; 11];
                counts[5] = 1;
                counts[6] = 1;
                counts
            },
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);
    let graph_items: Vec<&SkinRenderItem> = items
        .iter()
        .filter(|item| {
            matches!(
                item,
                SkinRenderItem::Image {
                    texture: SkinTextureId(9999),
                    rect: Rect { y, height, .. },
                    ..
                } if approx_eq(*y, 0.56) && approx_eq(*height, 0.04)
            )
        })
        .collect();

    assert_eq!(graph_items.len(), 2);
    assert!(graph_items.iter().all(|item| matches!(
        item,
        SkinRenderItem::Image {
            uv: TextureRegion { y, height, .. },
            ..
        } if approx_eq(*y, 0.5) && approx_eq(*height, 0.5)
    )));
    assert!(matches!(
        graph_items[0],
        SkinRenderItem::Image {
            rect: Rect { x, width, .. },
            uv: TextureRegion { x: uv_x, .. },
            ..
        } if approx_eq(*x, 0.10) && approx_eq(*width, 0.22) && approx_eq(*uv_x, 24.0 / 44.0)
    ));
    assert!(matches!(
        graph_items[1],
        SkinRenderItem::Image {
            rect: Rect { x, width, .. },
            uv: TextureRegion { x: uv_x, .. },
            ..
        } if approx_eq(*x, 0.32) && approx_eq(*width, 0.22) && approx_eq(*uv_x, 20.0 / 44.0)
    ));
}

#[test]
fn select_songlist_judgegraph_renders_chart_distribution() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "judgegraph": [{ "id": "density", "delay": 0, "noGap": 1, "noGapX": 1 }],
                "songlist": {
                    "id": "songlist",
                    "center": 0,
                    "liston": [{ "id": "row", "dst": [{ "x": 10, "y": 40, "w": 80, "h": 20 }] }],
                    "listoff": [{ "id": "row", "dst": [{ "x": 10, "y": 40, "w": 80, "h": 20 }] }],
                    "judgegraph": [{ "id": "density", "dst": [{ "x": 0, "y": 0, "w": 40, "h": 10 }] }]
                },
                "destination": [{ "id": "songlist" }]
            }
            "#,
        )
        .unwrap();
    let snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            kind: SelectRowKind::Song,
            in_library: true,
            chart_distribution: vec![
                crate::scene::SelectChartDistributionSecond {
                    key_taps: 4,
                    mines: 1,
                    ..Default::default()
                },
                crate::scene::SelectChartDistributionSecond {
                    scratch_taps: 2,
                    key_long_bodies: 3,
                    ..Default::default()
                },
            ],
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let sources = HashMap::new();
    let items = document.select_render_items(&sources, &snapshot);
    let rect_count =
        items.iter().filter(|item| matches!(item, SkinRenderItem::Rect { .. })).count();

    assert_eq!(rect_count, 7);
}

#[test]
fn select_destination_judgegraph_renders_selected_chart_distribution() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "judgegraph": [{ "id": "density", "delay": 0, "backTexOff": 1, "noGap": 1, "noGapX": 1 }],
                "destination": [{ "id": "density", "dst": [{ "x": 0, "y": 0, "w": 40, "h": 10 }] }]
            }
            "#,
        )
        .unwrap();
    let snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            kind: SelectRowKind::Song,
            in_library: true,
            chart_distribution: vec![
                crate::scene::SelectChartDistributionSecond { key_taps: 4, ..Default::default() },
                crate::scene::SelectChartDistributionSecond {
                    scratch_taps: 2,
                    ..Default::default()
                },
            ],
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&HashMap::new(), &snapshot);

    assert_eq!(items.iter().filter(|item| matches!(item, SkinRenderItem::Rect { .. })).count(), 2);
}

#[test]
fn select_destination_bpmgraph_renders_selected_chart_segments() {
    let document: SkinDocument = serde_json::from_str(
            r##"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "bpmgraph": [{ "id": "bpm", "lineWidth": 2, "mainBPMColor": "#ff0000", "otherBPMColor": "#00ff00" }],
                "destination": [{ "id": "bpm", "dst": [{ "x": 0, "y": 0, "w": 40, "h": 20 }] }]
            }
            "##,
        )
        .unwrap();
    let snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            kind: SelectRowKind::Song,
            in_library: true,
            min_bpm: 100.0,
            max_bpm: 200.0,
            chart_main_bpm: 100.0,
            chart_bpm_graph_segments: vec![
                crate::chart_graph::BpmGraphSegment {
                    start_ratio: 0.0,
                    end_ratio: 0.5,
                    bpm: 100.0,
                    is_stop: false,
                },
                crate::chart_graph::BpmGraphSegment {
                    start_ratio: 0.5,
                    end_ratio: 1.0,
                    bpm: 200.0,
                    is_stop: false,
                },
            ],
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&HashMap::new(), &snapshot);

    // 横線2本 + BPM変化縦線1本 = 3
    assert_eq!(items.iter().filter(|item| matches!(item, SkinRenderItem::Rect { .. })).count(), 3);
}

#[test]
fn select_songlist_bpmgraph_renders_row_segments() {
    let document: SkinDocument = serde_json::from_str(
            r##"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "bpmgraph": [{ "id": "bpm", "lineWidth": 2, "mainBPMColor": "#ff0000", "otherBPMColor": "#00ff00" }],
                "songlist": {
                    "id": "list",
                    "center": 0,
                    "liston": [{ "id": "row", "dst": [{ "x": 0, "y": 0, "w": 100, "h": 100 }] }],
                    "listoff": [{ "id": "row", "dst": [{ "x": 0, "y": 0, "w": 100, "h": 100 }] }],
                    "bpmgraph": [{ "id": "bpm", "dst": [{ "x": 0, "y": 0, "w": 40, "h": 20 }] }]
                },
                "destination": [{ "id": "list" }]
            }
            "##,
        )
        .unwrap();
    let snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            kind: SelectRowKind::Song,
            in_library: true,
            min_bpm: 100.0,
            max_bpm: 200.0,
            chart_main_bpm: 100.0,
            chart_bpm_graph_segments: vec![
                crate::chart_graph::BpmGraphSegment {
                    start_ratio: 0.0,
                    end_ratio: 0.5,
                    bpm: 100.0,
                    is_stop: false,
                },
                crate::chart_graph::BpmGraphSegment {
                    start_ratio: 0.5,
                    end_ratio: 1.0,
                    bpm: 200.0,
                    is_stop: false,
                },
            ],
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&HashMap::new(), &snapshot);

    // 横線2本 + BPM変化縦線1本 = 3
    assert_eq!(items.iter().filter(|item| matches!(item, SkinRenderItem::Rect { .. })).count(), 3);
}

#[test]
fn select_option_panel_three_renders_judge_timing_value() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "value": [{ "id": "judgetiming", "src": 1, "x": 0, "y": 0, "w": 120, "h": 20, "divx": 12, "divy": 2, "digit": 3, "ref": 12 }],
                "destination": [{ "id": "judgetiming", "timer": 23, "op": [23], "dst": [{ "x": 40, "y": 0, "w": 10, "h": 10 }] }]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 120.0, 40.0);
    let snapshot = SelectSnapshot {
        option_panel: 3,
        option_panel_time: TimeUs(100_000),
        judge_timing_offset_ms: -12,
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);

    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.4)
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, uv, .. }
            if approx_eq(rect.x, 0.4) && approx_eq(uv.x, 11.0 / 12.0) && uv.y > 0.0
    )));
}

#[test]
fn select_option_panel_text_uses_snapshot_option_strings() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "text": [
                    { "id": "bmz_select_gauge", "size": 10 },
                    { "id": "bmz_select_target", "size": 10 },
                    { "id": "bmz_select_judge_timing_auto_adjust", "size": 10 }
                ],
                "destination": [
                    { "id": "bmz_select_gauge", "op": [23], "dst": [{ "x": 0, "y": 0, "w": 50, "h": 10 }] },
                    { "id": "bmz_select_target", "op": [23], "dst": [{ "x": 0, "y": 10, "w": 50, "h": 10 }] },
                    { "id": "bmz_select_judge_timing_auto_adjust", "op": [23], "dst": [{ "x": 0, "y": 20, "w": 50, "h": 10 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let snapshot = SelectSnapshot {
        option_panel: 3,
        gauge: "HARD".to_string(),
        target: "AAA".to_string(),
        judge_timing_auto_adjust: true,
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&HashMap::new(), &snapshot);

    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Text {
            text, ..
        } if text == "HARD")));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Text {
            text, ..
        } if text == "RANK AAA")));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Text {
            text, ..
        } if text == "ON")));
}

#[test]
fn select_draw_state_uses_select_judge_timing_offset() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 5 }"#).unwrap();
    let snapshot = SelectSnapshot {
        option_panel: 3,
        judge_timing_offset_ms: -12,
        ..SelectSnapshot::default()
    };

    let (state, _) = document.select_draw_state(&snapshot, None);

    assert_eq!(skin_state_number(12, &state), Some(-12));
}

#[test]
fn select_snapshot_custom_offset_adjusts_destination_geometry_and_alpha() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
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
    let sources = mock_source("src", 10.0, 10.0);
    let mut skin_offsets = SkinOffsetValues::default();
    skin_offsets
        .set(42, crate::skin_offset::SkinOffsetValue { x: 6, y: 8, w: 10, h: 12, r: 0, a: -50 });

    let items = document.select_render_items(
        &sources,
        &SelectSnapshot { skin_offsets, ..SelectSnapshot::default() },
    );

    assert_eq!(items.len(), 1);
    let SkinRenderItem::Image { rect, tint, .. } = &items[0] else { panic!() };
    assert!(approx_eq(rect.x, 0.11));
    assert!(approx_eq(rect.y, 0.26));
    assert!(approx_eq(rect.width, 0.4));
    assert!(approx_eq(rect.height, 0.52));
    assert!(approx_eq(tint.a, 150.0 / 255.0));
}

#[test]
fn select_draw_state_uses_application_operating_time() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 5 }"#).unwrap();
    let snapshot = SelectSnapshot { operating_time_ms: 90_061_234, ..SelectSnapshot::default() };

    let (state, _) = document.select_draw_state(&snapshot, None);

    assert_eq!(skin_state_number(27, &state), Some(25));
    assert_eq!(skin_state_number(28, &state), Some(1));
    assert_eq!(skin_state_number(29, &state), Some(1));
}

#[test]
fn select_draw_state_maps_hispeed_and_green_number() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 5 }"#).unwrap();
    let snapshot = SelectSnapshot {
        hispeed: 3.25,
        note_display_duration_ms: Some(280),
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            kind: SelectRowKind::Song,
            in_library: true,
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let (state, _) = document.select_draw_state(&snapshot, None);

    assert_eq!(skin_state_number(310, &state), Some(3));
    assert_eq!(skin_state_number(311, &state), Some(25));
    assert_eq!(skin_state_number(312, &state), Some(467));
    assert_eq!(skin_state_number(313, &state), Some(280));
}

#[test]
fn select_draw_state_maps_extended_option_refs() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 5 }"#).unwrap();
    let snapshot = SelectSnapshot {
        arrange: "RANDOM".to_string(),
        arrange_2p: "SPIRAL".to_string(),
        double_option: "BATTLE AS".to_string(),
        hs_fix: "MAIN BPM".to_string(),
        hispeed_auto_adjust: true,
        ..SelectSnapshot::default()
    };

    let (state, _) = document.select_draw_state(&snapshot, None);

    assert_eq!(skin_state_number(42, &state), Some(2));
    assert_eq!(skin_state_number(43, &state), Some(5));
    assert_eq!(skin_state_number(54, &state), Some(3));
    assert_eq!(skin_state_number(55, &state), Some(3));
    assert_eq!(skin_state_number(342, &state), Some(1));
}

#[test]
fn select_draw_state_exposes_planned_random_lane_pattern() {
    let document: SkinDocument = serde_json::from_str(r#"{ "type": 5 }"#).unwrap();
    let mut pattern = (0..LANE_COUNT as u8).collect::<Vec<_>>();
    pattern[Lane::Key1.index()] = Lane::Key7.index() as u8;
    let snapshot = SelectSnapshot {
        arrange: "NORMAL".to_string(),
        lane_shuffle_pattern: pattern,
        rows: vec![SelectRowSnapshot {
            index: 0,
            kind: SelectRowKind::Song,
            chart_key_mode: Some(KeyMode::K7),
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let (state, _) = document.select_draw_state(&snapshot, None);

    assert_eq!(skin_state_number(42, &state), Some(0));
    assert_eq!(skin_state_number(450, &state), Some(7));
}

#[test]
fn select_songlist_judgegraph_honors_delay_backtexoff_and_type() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "judgegraph": [
                    { "id": "density", "type": 0, "delay": 1000, "backTexOff": 1, "noGap": 1, "noGapX": 1 },
                    { "id": "judge", "type": 1, "delay": 0 }
                ],
                "songlist": {
                    "id": "songlist",
                    "center": 0,
                    "liston": [{ "id": "row", "dst": [{ "x": 0, "y": 0, "w": 100, "h": 20 }] }],
                    "listoff": [{ "id": "row", "dst": [{ "x": 0, "y": 0, "w": 100, "h": 20 }] }],
                    "judgegraph": [
                        { "id": "density", "dst": [{ "x": 0, "y": 0, "w": 100, "h": 20 }] },
                        { "id": "judge", "dst": [{ "x": 0, "y": 20, "w": 100, "h": 20 }] }
                    ]
                },
                "destination": [{ "id": "songlist" }]
            }
            "#,
        )
        .unwrap();
    let row = SelectRowSnapshot {
        index: 0,
        kind: SelectRowKind::Song,
        in_library: true,
        chart_distribution: vec![
            crate::scene::SelectChartDistributionSecond { key_taps: 4, ..Default::default() },
            crate::scene::SelectChartDistributionSecond { key_taps: 4, ..Default::default() },
        ],
        ..SelectRowSnapshot::default()
    };
    let snapshot = SelectSnapshot {
        time: TimeUs(500_000),
        selected_index: 0,
        rows: vec![row],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&HashMap::new(), &snapshot);

    assert_eq!(items.iter().filter(|item| matches!(item, SkinRenderItem::Rect { .. })).count(), 1);
    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Rect { rect, .. } if approx_eq(rect.x, 0.0) && approx_eq(rect.width, 0.5)
    )));
}

#[test]
fn select_context_exposes_chart_image_sources_to_skin_document() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "image": [
                    { "id": "stage", "src": 100, "x": 0, "y": 0, "w": 40, "h": 20 },
                    { "id": "back", "src": 101, "x": 0, "y": 0, "w": 20, "h": 10 },
                    { "id": "banner", "src": 102, "x": 0, "y": 0, "w": 30, "h": 12 }
                ],
                "destination": [
                    { "id": "stage", "op": [191], "dst": [{ "x": 0, "y": 0, "w": 40, "h": 20 }] },
                    { "id": "back", "op": [195], "dst": [{ "x": 40, "y": 0, "w": 20, "h": 10 }] },
                    { "id": "banner", "op": [193], "dst": [{ "x": 60, "y": 0, "w": 30, "h": 12 }] }
                ]
            }
            "#,
    )
    .unwrap();
    let context = SkinContext::from_manifest_and_document(default_skin_manifest(), document, []);
    let snapshot = SelectSnapshot {
        stage_background: true,
        stage_image_size: Some(SkinImageSize { width: 400.0, height: 200.0 }),
        backbmp_image: true,
        backbmp_image_size: Some(SkinImageSize { width: 200.0, height: 100.0 }),
        banner_image: true,
        banner_image_size: Some(SkinImageSize { width: 300.0, height: 120.0 }),
        ..SelectSnapshot::default()
    };

    let items = context.select_document_items(&snapshot);

    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { texture, .. } if *texture == SkinTextureId(SELECT_STAGE_TEXTURE.0)
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { texture, .. } if *texture == SkinTextureId(PLAY_BACKBMP_TEXTURE.0)
    )));
    assert!(items.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { texture, .. } if *texture == SkinTextureId(SELECT_BANNER_TEXTURE.0)
    )));
}

#[test]
fn select_destination_negative_image_id_renders_runtime_stagefile_source() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
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
    let snapshot = SelectSnapshot {
        stage_background: true,
        stage_image_size: Some(SkinImageSize { width: 400.0, height: 200.0 }),
        ..SelectSnapshot::default()
    };

    let items = context.select_document_items(&snapshot);

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
fn select_chart_image_ops_follow_loaded_runtime_images() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "image": [
                    { "id": "no_stage", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "stage", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "no_back", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "back", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }
                ],
                "destination": [
                    { "id": "no_stage", "op": [190], "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "stage", "op": [191], "dst": [{ "x": 10, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "no_back", "op": [194], "dst": [{ "x": 20, "y": 0, "w": 10, "h": 10 }] },
                    { "id": "back", "op": [195], "dst": [{ "x": 30, "y": 0, "w": 10, "h": 10 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let context = SkinContext::from_manifest_and_document(
        default_skin_manifest(),
        document,
        [SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(1),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        }],
    );

    let missing = context.select_document_items(&SelectSnapshot::default());
    assert!(missing.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.0)
    )));
    assert!(missing.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.2)
    )));
    assert!(!missing.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.1) || approx_eq(rect.x, 0.3)
    )));

    let loaded = context.select_document_items(&SelectSnapshot {
        stage_background: true,
        backbmp_image: true,
        ..SelectSnapshot::default()
    });
    assert!(loaded.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.1)
    )));
    assert!(loaded.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.3)
    )));
    assert!(!loaded.iter().any(|item| matches!(
        item,
        SkinRenderItem::Image { rect, .. } if approx_eq(rect.x, 0.0) || approx_eq(rect.x, 0.2)
    )));
}

#[test]
fn select_click_hit_resolves_destination_act_for_dynamic_text() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "text": [
                    { "id": "bmz_select_arrange", "font": "default", "size": 18 },
                    { "id": "disabled", "font": "default", "size": 18, "constantText": "OFF" }
                ],
                "destination": [
                    {
                        "id": "bmz_select_arrange",
                        "act": 42,
                        "click": 2,
                        "dst": [{ "x": 10, "y": 20, "w": 30, "h": 10 }]
                    },
                    {
                        "id": "disabled",
                        "act": 43,
                        "clickable": false,
                        "dst": [{ "x": 50, "y": 20, "w": 30, "h": 10 }]
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let snapshot = SelectSnapshot { arrange: "MF-RANDOM".to_string(), ..SelectSnapshot::default() };

    assert!(document.select_render_items(&HashMap::new(), &snapshot).iter().any(|item| matches!(
        item,
        SkinRenderItem::Text { text, .. } if text == "MF-RANDOM"
    )));
    let hit = document
        .select_click_hit(
            &HashMap::new(),
            &snapshot,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            0.2,
            0.75,
        )
        .unwrap();

    assert_eq!(hit.target, SkinClickTarget::Event { event_id: 42, click: 2 });
    assert_eq!(hit.rect, Rect { x: 0.1, y: 0.7, width: 0.3, height: 0.1 });
    assert!(
        document
            .select_click_hit(
                &HashMap::new(),
                &snapshot,
                &crate::select_settings_dest::SelectSettingsDestIndex::default(),
                0.6,
                0.75,
            )
            .is_none()
    );
}

#[test]
fn select_click_hit_resolves_image_act_event() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "button.png" }],
                "image": [
                    { "id": "button_play", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10, "act": 15, "click": 2 }
                ],
                "destination": [
                    { "id": "button_play", "dst": [{ "x": 10, "y": 20, "w": 30, "h": 10 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    let snapshot = match crate::sample::sample_select_scene() {
        crate::scene::AppSceneSnapshot::Select(snapshot) => snapshot,
        _ => unreachable!(),
    };

    let hit = document
        .select_click_hit(
            &sources,
            &snapshot,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            0.2,
            0.75,
        )
        .unwrap();

    assert_eq!(hit.target, SkinClickTarget::Event { event_id: 15, click: 2 });
    assert_eq!(hit.rect, Rect { x: 0.1, y: 0.7, width: 0.3, height: 0.1 });
}

#[test]
fn select_mouse_rect_gates_render_and_click_hits() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "button.png" }],
                "image": [
                    { "id": "button", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10, "act": 15 }
                ],
                "destination": [
                    {
                        "id": "button",
                        "dst": [{ "x": 10, "y": 20, "w": 30, "h": 10 }],
                        "mouseRect": { "x": 5, "y": 2, "w": 10, "h": 4 }
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    let inside = SelectSnapshot { mouse_position: Some((0.16, 0.75)), ..SelectSnapshot::default() };
    let outside =
        SelectSnapshot { mouse_position: Some((0.01, 0.01)), ..SelectSnapshot::default() };

    assert!(document.select_render_items(&sources, &inside).iter().any(|item| {
        matches!(item, SkinRenderItem::Image { texture: SkinTextureId(9999), .. })
    }));
    assert!(!document.select_render_items(&sources, &outside).iter().any(|item| {
        matches!(item, SkinRenderItem::Image { texture: SkinTextureId(9999), .. })
    }));

    assert!(
        document
            .select_click_hit(
                &sources,
                &inside,
                &crate::select_settings_dest::SelectSettingsDestIndex::default(),
                0.2,
                0.75,
            )
            .is_some()
    );
    assert!(
        document
            .select_click_hit(
                &sources,
                &outside,
                &crate::select_settings_dest::SelectSettingsDestIndex::default(),
                0.2,
                0.75,
            )
            .is_none()
    );
}

#[test]
fn select_slider_hit_resolves_changeable_volume_slider() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "slider": [
                    { "id": "master", "src": 1, "x": 0, "y": 0, "w": 10, "h": 5, "angle": 1, "range": 50, "type": 17 }
                ],
                "destination": [
                    { "id": "master", "dst": [{ "x": 10, "y": 20, "w": 10, "h": 5 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let snapshot = SelectSnapshot::default();

    // angle=1 destination x=10 range=50 → value 0.5 at skin x=35 (norm x=0.35)
    let hit = document
        .select_slider_hit(
            &snapshot,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            0.35,
            0.775,
        )
        .unwrap();

    assert_eq!(hit.slider_type, 17);
    assert!(approx_eq(hit.value, 0.5));
    assert!(
        document
            .select_slider_hit(
                &snapshot,
                &crate::select_settings_dest::SelectSettingsDestIndex::default(),
                0.70,
                0.775,
            )
            .is_none()
    );
}

#[test]
fn select_slider_hit_resolves_song_scroll_slider() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "slider": [
                    { "id": "song-scroll", "src": 1, "x": 0, "y": 0, "w": 10, "h": 5, "angle": 2, "range": 50, "type": 1 }
                ],
                "destination": [
                    { "id": "song-scroll", "dst": [{ "x": 10, "y": 70, "w": 10, "h": 5 }] }
                ]
            }
            "#,
        )
        .unwrap();
    let snapshot = SelectSnapshot::default();

    // beatoraja: value=(region.y - mouse_y)/range. Mid = skin y 45 → norm 0.55.
    let hit = document
        .select_slider_hit(
            &snapshot,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            0.15,
            0.55,
        )
        .unwrap();

    assert_eq!(hit.slider_type, 1);
    assert!(approx_eq(hit.value, 0.5));
    // Top of track (value 0) is destination y itself → skin y 70 → norm 0.30.
    let top_hit = document
        .select_slider_hit(
            &snapshot,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            0.15,
            0.30,
        )
        .unwrap();
    assert_eq!(top_hit.slider_type, 1);
    assert!(approx_eq(top_hit.value, 0.0));
}

#[test]
fn select_slider_hit_matches_mz_select_songlist_scroll_collision() {
    // mz-select default_songlistscroll2 collision:
    // parts_position=(1888,270), dst x=1864 y=790 w=64 h=64, angle=2 range=500
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 1920,
                "h": 1080,
                "slider": [
                    {
                        "id": "default_songlistscroll2_collision",
                        "src": 1,
                        "x": 80,
                        "y": 0,
                        "w": 64,
                        "h": 64,
                        "angle": 2,
                        "range": 500,
                        "type": 1
                    }
                ],
                "destination": [
                    {
                        "id": "default_songlistscroll2_collision",
                        "dst": [{ "x": 1864, "y": 790, "w": 64, "h": 64 }]
                    }
                ]
            }
            "#,
    )
    .unwrap();
    let snapshot = SelectSnapshot::default();
    let settings = crate::select_settings_dest::SelectSettingsDestIndex::default();
    let x = (1864.0 + 32.0) / 1920.0;

    let top = document.select_slider_hit(&snapshot, &settings, x, 1.0 - 790.0 / 1080.0).unwrap();
    assert_eq!(top.slider_type, 1);
    assert!(approx_eq(top.value, 0.0));

    let mid = document.select_slider_hit(&snapshot, &settings, x, 1.0 - 540.0 / 1080.0).unwrap();
    assert_eq!(mid.slider_type, 1);
    assert!(approx_eq(mid.value, 0.5));

    let bottom = document.select_slider_hit(&snapshot, &settings, x, 1.0 - 290.0 / 1080.0).unwrap();
    assert_eq!(bottom.slider_type, 1);
    assert!(approx_eq(bottom.value, 1.0));

    // Clicks above destination y must miss (beatoraja uses region.y as the upper edge).
    assert!(document.select_slider_hit(&snapshot, &settings, x, 1.0 - 822.0 / 1080.0).is_none());
}

#[test]
fn select_click_hit_resolves_clickable_songlist_row() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "songlist": {
                    "id": "songlist",
                    "center": 0,
                    "clickable": [0],
                    "liston": [
                        { "id": "bar", "dst": [{ "x": 0, "y": 0, "w": 50, "h": 10 }] }
                    ],
                    "listoff": [
                        { "id": "bar", "dst": [{ "x": 50, "y": 0, "w": 50, "h": 10 }] }
                    ]
                },
                "destination": [{ "id": "songlist" }]
            }
            "#,
    )
    .unwrap();
    let snapshot = match crate::sample::sample_select_scene() {
        crate::scene::AppSceneSnapshot::Select(snapshot) => snapshot,
        _ => unreachable!(),
    };

    let hit = document
        .select_click_hit(
            &HashMap::new(),
            &snapshot,
            &crate::select_settings_dest::SelectSettingsDestIndex::default(),
            0.25,
            0.95,
        )
        .unwrap();

    assert_eq!(hit.target, SkinClickTarget::SelectRow { row_index: 0 });
    assert_eq!(hit.rect, Rect { x: 0.0, y: 0.9, width: 0.5, height: 0.1 });
    assert!(
        document
            .select_click_hit(
                &HashMap::new(),
                &snapshot,
                &crate::select_settings_dest::SelectSettingsDestIndex::default(),
                0.75,
                0.95,
            )
            .is_none()
    );
}

#[test]
fn select_skin_document_advances_dynamic_timers() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "marker.png" }],
                "image": [{ "id": "marker", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "marker", "timer": 9001, "dst": [{ "x": 10, "y": 10, "w": 10, "h": 10 }] }
                ],
                "dynamicTimer": [{ "id": 9001, "observe": "number(300) > 0" }]
            }
            "#,
        )
        .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    let snapshot = SelectSnapshot {
        time: TimeUs(100_000),
        chart_count: 1,
        rows: vec![SelectRowSnapshot {
            index: 0,
            is_folder: true,
            kind: SelectRowKind::Folder,
            folder_lamp_counts: [1; 11],
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    assert!(document.select_render_items(&sources, &snapshot).is_empty());

    let mut runtime = DynamicTimerRuntime::default();
    let items = document.select_render_items_with_dynamic_timers(
        &sources,
        &snapshot,
        Some(&mut runtime),
        &crate::select_settings_dest::SelectSettingsDestIndex::default(),
        None,
    );

    assert_eq!(items.len(), 1);
    assert!(matches!(items[0], SkinRenderItem::Image { .. }));
}

#[test]
fn select_skin_document_renders_unowned_song_with_nograde_bar() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "bar.png" }],
                "image": [
                    { "id": "bar-song", "src": 1, "x": 0, "y": 0, "w": 40, "h": 10 },
                    { "id": "bar-nograde", "src": 1, "x": 0, "y": 40, "w": 40, "h": 10 }
                ],
                "imageset": [{
                    "id": "bar",
                    "images": ["bar-song", "bar-song", "bar-song", "bar-song", "bar-nograde"]
                }],
                "text": [
                    { "id": "bartext-owned", "font": "main", "size": 10 },
                    { "id": "bartext-owned2", "font": "main", "size": 10 },
                    { "id": "bartext-owned3", "font": "main", "size": 10 },
                    { "id": "bartext-owned4", "font": "main", "size": 10 },
                    { "id": "bartext-owned5", "font": "main", "size": 10 },
                    { "id": "bartext-owned6", "font": "main", "size": 10 },
                    { "id": "bartext-owned7", "font": "main", "size": 10 },
                    { "id": "bartext-owned8", "font": "main", "size": 10 },
                    { "id": "bartext-unowned", "font": "unowned", "size": 10 }
                ],
                "songlist": {
                    "id": "songlist",
                    "center": 0,
                    "listoff": [{ "id": "bar", "dst": [{ "x": 10, "y": 50, "w": 40, "h": 10 }] }],
                    "liston": [{ "id": "bar", "dst": [{ "x": 12, "y": 50, "w": 40, "h": 10 }] }],
                    "text": [
                        { "id": "bartext-owned", "dst": [{ "x": 1, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned2", "dst": [{ "x": 2, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned3", "dst": [{ "x": 3, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned4", "dst": [{ "x": 4, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned5", "dst": [{ "x": 5, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned6", "dst": [{ "x": 6, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned7", "dst": [{ "x": 7, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-owned8", "dst": [{ "x": 8, "y": 2, "w": 20, "h": 8 }] },
                        { "id": "bartext-unowned", "dst": [{ "x": 9, "y": 2, "w": 20, "h": 8 }] }
                    ]
                },
                "destination": [{ "id": "songlist" }]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    let snapshot = SelectSnapshot {
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            title: "Missing Song".to_string(),
            in_library: false,
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);

    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                texture: SkinTextureId(9999),
                uv: TextureRegion { y: v, .. },
                ..
            } if approx_eq(*v, 40.0 / 100.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Text {
                text,
                style,
                ..
            } if text == "Missing Song" && style.font_id.as_deref() == Some("unowned"))));
}

#[test]
fn select_skin_uses_snapshot_time_and_bar_type_ops() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "panel.png" }],
                "image": [
                    { "id": "song-panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "folder-panel", "src": 1, "x": 10, "y": 0, "w": 10, "h": 10 }
                ],
                "destination": [
                    { "id": "song-panel", "timer": 11, "loop": 200, "op": [2], "dst": [
                        { "time": 0, "x": 0, "y": 0, "w": 10, "h": 10 },
                        { "time": 200, "x": 20 }
                    ] },
                    { "id": "folder-panel", "op": [1], "dst": [
                        { "x": 50, "y": 0, "w": 10, "h": 10 }
                    ] },
                    { "id": "song-panel", "timer": 21, "op": [21], "dst": [
                        { "time": 0, "x": 30, "y": 0, "w": 10, "h": 10 },
                        { "time": 200, "x": 50 }
                    ] }
                ]
            }
            "#,
    )
    .unwrap();
    let sources = mock_source("1", 100.0, 100.0);
    let snapshot = SelectSnapshot {
        time: bmz_core::time::TimeUs(100_000),
        selection_time: bmz_core::time::TimeUs(100_000),
        option_panel_time: bmz_core::time::TimeUs(100_000),
        option_panel: 1,
        selected_index: 0,
        rows: vec![SelectRowSnapshot {
            index: 0,
            title: "Song".to_string(),
            is_folder: false,
            ..SelectRowSnapshot::default()
        }],
        ..SelectSnapshot::default()
    };

    let items = document.select_render_items(&sources, &snapshot);

    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                rect: Rect { x, .. },
                uv: TextureRegion { x: u, .. },
                ..
            } if approx_eq(*x, 0.1) && approx_eq(*u, 0.0))));
    assert!(items.iter().any(|item| matches!(item, SkinRenderItem::Image {
                rect: Rect { x, .. },
                ..
            } if approx_eq(*x, 0.4))));
}

#[test]
fn select_folder_hides_song_score_numbers() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        select_in_library: true,
        ex_score: 1234,
        total_notes: 1000,
        select_total_notes: 1000,
        select_play_count: 7,
        select_clear_count: 3,
        select_bp: Some(12),
        select_cb: Some(8),
        judge_counts: DisplayJudgeCounts {
            pgreat: 20,
            great: 30,
            good: 10,
            bad: 5,
            poor: 2,
            empty_poor: 1,
        },
        fast_slow_counts: Some(crate::snapshot::FastSlowJudgeCounts {
            fast_pgreat: 7,
            slow_empty_poor: 2,
            ..Default::default()
        }),
        ..SkinDrawState::default()
    };

    for ref_id in [71, 74, 76, 77, 78, 80, 85, 102, 110, 154, 410, 420, 426] {
        assert_eq!(skin_state_number(ref_id, &state), None, "ref {ref_id}");
    }
    assert_eq!(skin_state_number(30, &state), Some(0));
    assert_eq!(skin_state_number(33, &state), Some(0));
}

#[test]
fn select_course_exposes_score_numbers() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Course,
        select_in_library: true,
        ex_score: 1234,
        max_combo: 345,
        total_notes: 1000,
        select_total_notes: 1000,
        select_play_count: 42,
        select_clear_count: 31,
        select_bp: Some(12),
        select_cb: Some(8),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(71, &state), Some(1234));
    assert_eq!(skin_state_number(74, &state), Some(1000));
    assert_eq!(skin_state_number(75, &state), Some(345));
    assert_eq!(skin_state_number(76, &state), Some(12));
    assert_eq!(skin_state_number(77, &state), Some(42));
    assert_eq!(skin_state_number(78, &state), Some(31));
    assert_eq!(skin_state_number(425, &state), Some(8));
    assert_eq!(skin_state_number(427, &state), Some(8));
}

#[test]
fn select_panel_on_and_off_timers_follow_each_panel_state() {
    let state = SkinDrawState {
        select_option_panel: 2,
        select_option_panel_elapsed_ms: 75,
        select_option_panel_off_elapsed_ms: [Some(120), None, Some(340), None, None, None],
        ..SkinDrawState::default()
    };

    assert_eq!(skin_timer_elapsed_ms(Some(21), &state), None);
    assert_eq!(skin_timer_elapsed_ms(Some(22), &state), Some(75));
    assert_eq!(skin_timer_elapsed_ms(Some(23), &state), None);
    assert_eq!(skin_timer_elapsed_ms(Some(31), &state), Some(120));
    assert_eq!(skin_timer_elapsed_ms(Some(32), &state), None);
    assert_eq!(skin_timer_elapsed_ms(Some(33), &state), Some(340));
}

#[test]
fn rival_skin_properties_map_select_rival_best() {
    let state = SkinDrawState {
        rival_ex_score: Some(1500),
        rival_max_combo: Some(700),
        rival_bp: Some(12),
        rival_judge_counts: Some([900, 50, 7, 3, 3]),
        select_total_notes: 1000,
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(271, &state), Some(1500));
    assert_eq!(skin_state_number(275, &state), Some(700));
    assert_eq!(skin_state_number(276, &state), Some(12));
    assert_eq!(skin_state_number(280, &state), Some(900));
    assert_eq!(skin_state_number(281, &state), Some(50));
    assert_eq!(skin_state_number(282, &state), Some(7));
    assert_eq!(skin_state_number(283, &state), Some(3));
    assert_eq!(skin_state_number(284, &state), Some(3));
    assert_eq!(skin_state_number(285, &state), Some(90));
    assert_eq!(skin_state_number(286, &state), Some(5));
    assert_eq!(skin_state_number(287, &state), Some(0));
    assert!((skin_state_float_number(285, &state).unwrap() - 0.9).abs() < f32::EPSILON);
    assert!((skin_state_float_number(286, &state).unwrap() - 0.05).abs() < f32::EPSILON);
    assert!(!test_skin_op(624, &[], &state));
    assert!(test_skin_op(625, &[], &state));

    let no_rival = SkinDrawState::default();
    assert_eq!(skin_state_number(271, &no_rival), None);
    assert_eq!(skin_state_number(280, &no_rival), None);
    assert_eq!(skin_state_number(285, &no_rival), None);
    assert_eq!(skin_state_float_number(285, &no_rival), None);
    assert!(test_skin_op(624, &[], &no_rival));
    assert!(!test_skin_op(625, &[], &no_rival));
}

#[test]
fn skin_state_number_maps_select_refs() {
    let state = SkinDrawState {
        select_folder_song_count: Some(42),
        select_screen: true,
        select_play_level: 12,
        select_clear_index: 5,
        select_total_notes: 1200,
        select_bpm: 148.0,
        select_chart_normal_notes: 900,
        select_chart_long_notes: 180,
        select_chart_scratch_notes: 100,
        select_chart_long_scratch_notes: 20,
        select_chart_density: 4.56,
        select_chart_peak_density: 12.34,
        select_chart_end_density: 7.89,
        select_chart_total_gauge: 200.0,
        select_chart_main_bpm: 150.0,
        select_min_bpm: 120.0,
        select_max_bpm: 180.0,
        select_length_ms: 183_000,
        hispeed: 2.75,
        total_duration_ms: 500,
        duration_green_ms: Some(300),
        select_master_volume: 0.575,
        select_key_volume: 0.59,
        select_bgm_volume: 0.28,
        select_mode_index: 4,
        select_sort_index: 6,
        select_ln_mode_index: 2,
        select_bp: Some(12),
        select_cb: Some(8),
        ex_score: 1234,
        max_combo: 345,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(11, &state), Some(4));
    assert_eq!(skin_state_number(12, &state), Some(6));
    assert_eq!(skin_state_number(300, &state), Some(42));
    assert_eq!(skin_state_number(96, &state), Some(12));
    assert_eq!(
        skin_state_number(
            96,
            &SkinDrawState { select_play_level: 12, play_level: 9, ..SkinDrawState::default() }
        ),
        Some(9)
    );
    assert_eq!(skin_state_number(370, &state), Some(5));
    assert_eq!(skin_state_number(74, &state), Some(1200));
    assert_eq!(skin_state_number(75, &state), Some(345));
    assert_eq!(skin_state_number(105, &state), Some(345));
    assert_eq!(skin_state_number(76, &state), Some(12));
    assert_eq!(skin_state_number(425, &state), Some(8));
    assert_eq!(skin_state_number(90, &state), Some(180));
    assert_eq!(skin_state_number(91, &state), Some(120));
    assert_eq!(skin_state_number(92, &state), Some(150));
    assert_eq!(skin_state_number(160, &state), Some(148));
    assert_eq!(skin_state_number(350, &state), Some(900));
    assert_eq!(skin_state_number(351, &state), Some(180));
    assert_eq!(skin_state_number(352, &state), Some(100));
    assert_eq!(skin_state_number(353, &state), Some(20));
    assert_eq!(skin_state_number(360, &state), Some(12));
    assert_eq!(skin_state_number(361, &state), Some(34));
    assert_eq!(skin_state_number(362, &state), Some(7));
    assert_eq!(skin_state_number(363, &state), Some(89));
    assert_eq!(skin_state_number(364, &state), Some(4));
    assert_eq!(skin_state_number(365, &state), Some(56));
    assert_eq!(skin_state_number(368, &state), Some(200));
    assert_eq!(skin_state_number(71, &state), Some(1234));
    assert_eq!(skin_state_number(1163, &state), Some(3));
    assert_eq!(skin_state_number(1164, &state), Some(3));
    assert_eq!(skin_state_number(310, &state), Some(2));
    assert_eq!(skin_state_number(311, &state), Some(75));
    assert_eq!(skin_state_number(312, &state), Some(500));
    assert_eq!(skin_state_number(313, &state), Some(300));
    assert_eq!(skin_state_number(57, &state), Some(57));
    assert_eq!(skin_state_number(58, &state), Some(59));
    assert_eq!(skin_state_number(59, &state), Some(28));
    assert_eq!(skin_state_number(308, &state), Some(2));

    assert!(skin_state_number(21, &state).is_some_and(|value| value >= 2026));
    assert!(skin_state_number(22, &state).is_some_and(|value| (1..=12).contains(&value)));
    assert!(skin_state_number(23, &state).is_some_and(|value| (1..=31).contains(&value)));
    assert!(skin_state_number(24, &state).is_some_and(|value| (0..=23).contains(&value)));
    assert!(skin_state_number(25, &state).is_some_and(|value| (0..=59).contains(&value)));
    assert!(skin_state_number(26, &state).is_some_and(|value| (0..=59).contains(&value)));
}

#[test]
fn select_mode_index_matches_beatoraja_skin_ref_order() {
    let cases = [
        ("ALL", 0),
        ("5K", 1),
        ("7K", 2),
        ("10K", 3),
        ("14K", 4),
        ("9K", 5),
        ("24K", 6),
        ("24K_DOUBLE", 7),
        ("unknown", 0),
    ];

    for (mode, expected) in cases {
        assert_eq!(select_mode_index(mode), expected, "select mode {mode}");
    }
}

#[test]
fn select_folder_hides_chart_bpm_and_judge_rank() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        select_in_library: true,
        select_bpm: 0.0,
        select_min_bpm: 0.0,
        select_max_bpm: 0.0,
        judge_rank: None,
        total_duration_ms: 500,
        duration_green_ms: Some(300),
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(90, &state), None);
    assert_eq!(skin_state_number(91, &state), None);
    assert_eq!(skin_state_number(92, &state), None);
    assert_eq!(skin_state_number(160, &state), None);
    for ref_id in [350, 351, 352, 353, 360, 362, 364, 368, 1163, 1164] {
        assert_eq!(skin_state_number(ref_id, &state), None, "chart detail ref {ref_id}");
    }
    assert_eq!(skin_state_number(312, &state), Some(500));
    assert_eq!(skin_state_number(313, &state), Some(300));
    for op in 180..=184 {
        assert!(!test_skin_op(op, &[], &state), "judge rank option {op}");
    }
}

#[test]
fn select_course_keeps_score_totals_but_hides_per_chart_details() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Course,
        select_in_library: true,
        select_total_notes: 10_718,
        total_notes: 10_718,
        select_chart_normal_notes: 10_718,
        select_chart_total_gauge: 224.0,
        select_length_ms: 180_000,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_number(74, &state), Some(10_718));
    for ref_id in [90, 91, 92, 160, 350, 351, 352, 353, 360, 362, 364, 368, 1163, 1164] {
        assert_eq!(skin_state_number(ref_id, &state), None, "chart detail ref {ref_id}");
    }
}

#[test]
fn skin_state_imageset_index_maps_select_options() {
    let state = SkinDrawState {
        select_screen: true,
        select_arrange_index: 2,
        select_arrange_2p_index: 5,
        select_double_option_index: 3,
        select_hs_fix_index: 4,
        select_gauge_index: 4,
        select_target_index: 3,
        select_bga_index: 1,
        judge_timing_auto_adjust: true,
        select_judge_algorithm_index: 2,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_state_imageset_index(42, &state), Some(2));
    assert_eq!(skin_state_imageset_index(43, &state), Some(5));
    assert_eq!(skin_state_imageset_index(54, &state), Some(3));
    assert_eq!(skin_state_imageset_index(55, &state), Some(4));
    assert_eq!(skin_state_imageset_index(40, &state), Some(4));
    assert_eq!(skin_state_imageset_index(41, &state), Some(3));
    assert_eq!(skin_state_imageset_index(75, &state), Some(1));
    assert_eq!(skin_state_imageset_index(72, &state), Some(1));
    assert_eq!(skin_state_imageset_index(340, &state), Some(2));
    assert_eq!(skin_state_imageset_index(301, &state), Some(0));
    assert_eq!(skin_state_imageset_index(500, &state), None);
}

#[test]
fn select_arrange_index_maps_beatoraja_random_options() {
    assert_eq!(select_arrange_index("NORMAL"), 0);
    assert_eq!(select_arrange_index("MIRROR"), 1);
    assert_eq!(select_arrange_index("RANDOM"), 2);
    assert_eq!(select_arrange_index("R-RANDOM"), 3);
    assert_eq!(select_arrange_index("S-RANDOM"), 4);
    assert_eq!(select_arrange_index("SPIRAL"), 5);
    assert_eq!(select_arrange_index("H-RANDOM"), 6);
    assert_eq!(select_arrange_index("ALL-SCR"), 7);
    assert_eq!(select_arrange_index("RANDOM-EX"), 8);
    assert_eq!(select_arrange_index("S-RANDOM-EX"), 9);
    assert_eq!(select_arrange_index("F-RANDOM"), 2);
    assert_eq!(select_arrange_index("MF-RANDOM"), 2);
    assert_eq!(extended_arrange_index("F-RANDOM"), 10);
    assert_eq!(extended_arrange_index("MF-RANDOM"), 11);
    assert_eq!(select_arrange_index("unknown"), 0);
}

#[test]
fn select_judge_algorithm_index_maps_beatoraja_order() {
    assert_eq!(select_judge_algorithm_index("Combo"), 0);
    assert_eq!(select_judge_algorithm_index("Duration"), 1);
    assert_eq!(select_judge_algorithm_index("Lowest"), 2);
    assert_eq!(select_judge_algorithm_index("unknown"), 0);
}

#[test]
fn select_hs_fix_index_maps_beatoraja_order() {
    assert_eq!(select_hs_fix_index("OFF"), 0);
    assert_eq!(select_hs_fix_index("START BPM"), 1);
    assert_eq!(select_hs_fix_index("MAX BPM"), 2);
    assert_eq!(select_hs_fix_index("MAIN BPM"), 3);
    assert_eq!(select_hs_fix_index("MIN BPM"), 4);
    assert_eq!(select_hs_fix_index("unknown"), 0);
}

#[test]
fn skin_image_ref_number_maps_extended_select_arrange() {
    let state = SkinDrawState {
        select_screen: true,
        select_arrange_index: 9,
        select_arrange_2p_index: 6,
        select_extended_arrange_index: 11,
        select_extended_arrange_2p_index: 10,
        select_gauge_index: 4,
        select_target_index: 10,
        select_double_option_index: 2,
        select_hs_fix_index: 3,
        select_bga_index: 2,
        select_assist_index: 1,
        judge_timing_auto_adjust: true,
        select_gauge_auto_shift_index: 3,
        select_ln_mode_index: 2,
        select_judge_algorithm_index: 3,
        select_bottom_shiftable_gauge_index: 2,
        ..SkinDrawState::default()
    };

    assert_eq!(skin_image_ref_number(40, &state), Some(4));
    assert_eq!(skin_image_ref_number(41, &state), Some(10));
    assert_eq!(skin_image_ref_number(42, &state), Some(9));
    assert_eq!(skin_image_ref_number(43, &state), Some(6));
    assert_eq!(skin_image_ref_number(344, &state), Some(11));
    assert_eq!(skin_image_ref_number(345, &state), Some(10));
    assert_eq!(skin_image_ref_number(54, &state), Some(2));
    assert_eq!(skin_image_ref_number(55, &state), Some(3));
    assert_eq!(skin_image_ref_number(72, &state), Some(2));
    assert_eq!(skin_image_ref_number(75, &state), Some(1));
    assert_eq!(skin_image_ref_number(78, &state), Some(3));
    assert_eq!(skin_image_ref_number(308, &state), Some(2));
    assert_eq!(skin_image_ref_number(340, &state), Some(3));
    assert_eq!(skin_image_ref_number(341, &state), Some(2));
    assert_eq!(skin_state_number(42, &state), Some(9));
    assert_eq!(skin_state_number(43, &state), Some(6));
    assert_eq!(skin_state_number(344, &state), Some(11));
    assert_eq!(skin_state_number(345, &state), Some(10));
    assert_eq!(skin_state_number(54, &state), Some(2));
    assert_eq!(skin_state_number(55, &state), Some(3));
    assert_eq!(skin_state_number(340, &state), Some(3));
    assert_eq!(skin_state_event_index(40, &state), 4);
    assert_eq!(skin_state_event_index(41, &state), 10);
    assert_eq!(skin_state_event_index(42, &state), 9);
    assert_eq!(skin_state_event_index(43, &state), 6);
    assert_eq!(skin_state_event_index(344, &state), 11);
    assert_eq!(skin_state_event_index(345, &state), 10);
    assert_eq!(skin_state_event_index(54, &state), 2);
    assert_eq!(skin_state_event_index(55, &state), 3);
    assert_eq!(skin_state_event_index(72, &state), 2);
    assert_eq!(skin_state_event_index(73, &state), 1);
    assert_eq!(skin_state_event_index(75, &state), 1);
    assert_eq!(skin_state_event_index(78, &state), 3);
    assert_eq!(skin_state_event_index(308, &state), 2);
    assert_eq!(skin_state_event_index(340, &state), 3);
    assert_eq!(skin_state_event_index(341, &state), 2);
}

#[test]
fn select_skin_imageset_uses_extended_arrange_index() {
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 5,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "arrange.png" }],
                "image": [
                    { "id": "normal", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 },
                    { "id": "mirror", "src": 1, "x": 10, "y": 0, "w": 10, "h": 10 },
                    { "id": "random", "src": 1, "x": 20, "y": 0, "w": 10, "h": 10 },
                    { "id": "r-random", "src": 1, "x": 30, "y": 0, "w": 10, "h": 10 },
                    { "id": "s-random", "src": 1, "x": 40, "y": 0, "w": 10, "h": 10 },
                    { "id": "spiral", "src": 1, "x": 50, "y": 0, "w": 10, "h": 10 },
                    { "id": "h-random", "src": 1, "x": 60, "y": 0, "w": 10, "h": 10 },
                    { "id": "all-scr", "src": 1, "x": 70, "y": 0, "w": 10, "h": 10 },
                    { "id": "random-ex", "src": 1, "x": 80, "y": 0, "w": 10, "h": 10 },
                    { "id": "s-random-ex", "src": 1, "x": 90, "y": 0, "w": 10, "h": 10 }
                ],
                "imageset": [{
                    "id": "option-random",
                    "ref": 42,
                    "images": [
                        "normal", "mirror", "random", "r-random", "s-random",
                        "spiral", "h-random", "all-scr", "random-ex", "s-random-ex"
                    ]
                }],
                "destination": [{
                    "id": "option-random",
                    "dst": [{ "x": 10, "y": 20, "w": 20, "h": 10 }]
                }]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(42),
            source_size: SkinImageSize { width: 100.0, height: 10.0 },
        },
    )]);
    let items = document.select_render_items(
        &sources,
        &crate::scene::SelectSnapshot { arrange: "S-RANDOM-EX".to_string(), ..Default::default() },
    );

    assert!(matches!(
        items.as_slice(),
        [SkinRenderItem::Image {
            texture: SkinTextureId(42),
            uv: TextureRegion { x, .. },
            ..
        }] if approx_eq(*x, 0.9)
    ));
}

#[test]
fn select_target_index_maps_fixed_targets() {
    let index = |target| select_target_index_for_name(target).unwrap_or(0);
    assert_eq!(index("NONE"), 0);
    assert_eq!(index("RANK_A"), 1);
    assert_eq!(index("RANK_AA-"), 2);
    assert_eq!(index("RANK_AA"), 3);
    assert_eq!(index("RANK_AAA-"), 4);
    assert_eq!(index("RANK_AAA"), 5);
    assert_eq!(index("RANK_MAX-"), 6);
    assert_eq!(index("MAX"), 7);
    assert_eq!(index("RANK_NEXT"), 8);
    assert_eq!(index("IR_TOP"), 9);
    assert_eq!(index("IR_NEXT"), 10);
    assert_eq!(index("RIVAL TOP"), 11);
    assert_eq!(index("RIVAL NEXT"), 12);
    assert_eq!(index("RIVAL"), 11);
    assert_eq!(index("AAA"), 5);
    assert_eq!(index("AA"), 3);
    assert_eq!(index("A"), 1);
    assert_eq!(index("B"), 1);
}

#[test]
fn bundled_beatoraja_default_select_json_loads_when_available() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.local/beatoraja/skin/default/select.json");
    if !path.is_file() {
        return;
    }

    let document = SkinDocument::load_beatoraja_json(&path).unwrap();

    assert_eq!(document.name, "beatoraja default");
    assert_eq!(document.skin_type, 5);
    assert!(document.songlist.is_some());
    assert!(!document.destination.is_empty());
}

#[test]
fn local_ecfn_converted_select_json_loads_when_available() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/skins/ECFN/select/select-converted.json");
    if !path.is_file() {
        return;
    }

    let document = SkinDocument::load_beatoraja_json(&path).unwrap();

    assert_eq!(document.skin_type, 5);
    assert!(document.songlist.is_some());
    assert!(!document.destination.is_empty());
}

#[test]
fn judge_timer_elapsed_ms_selects_animation_frame() {
    // timer=46 → TIMER_JUDGE_1P; two dst frames at time=0 and time=200
    let document: SkinDocument = serde_json::from_str(
        r#"
            {
                "type": 0,
                "w": 100,
                "h": 100,
                "source": [{ "id": 1, "path": "system.png" }],
                "image": [{ "id": "panel", "src": 1, "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [
                    { "id": "panel", "timer": 46, "dst": [
                        { "time": 0,   "x": 0,   "y": 0, "w": 10, "h": 10 },
                        { "time": 200, "x": 50,  "y": 0, "w": 10, "h": 10 }
                    ]}
                ]
            }
            "#,
    )
    .unwrap();
    let sources = HashMap::from([(
        "1".to_string(),
        SkinDocumentTexture {
            source_id: "1".to_string(),
            texture: SkinTextureId(2),
            source_size: SkinImageSize { width: 10.0, height: 10.0 },
        },
    )]);

    // judge_ms=Some(100) → between frame 0 and frame 200 → x should be 0.25 (interpolated)
    let state_early = SkinDrawState {
        judge_ms: judge_region_state(0, 100, 0).judge_ms,
        ..SkinDrawState::default()
    };
    let items_early = document.static_image_render_items(&sources, &state_early);
    assert_eq!(items_early.len(), 1);
    assert!(
        matches!(items_early[0], SkinRenderItem::Image { rect: Rect { x, .. }, .. }
            if approx_eq(x, 0.25)),
        "at judge_ms=100, x should interpolate to 0.25 (halfway between 0 and 0.5)"
    );

    // judge_ms=Some(300) → past last frame → last frame x=0.5
    let state_late = SkinDrawState {
        judge_ms: judge_region_state(0, 300, 0).judge_ms,
        ..SkinDrawState::default()
    };
    let items_late = document.static_image_render_items(&sources, &state_late);
    assert_eq!(items_late.len(), 1);
    assert!(
        matches!(items_late[0], SkinRenderItem::Image { rect: Rect { x, .. }, .. }
            if approx_eq(x, 0.5)),
        "at judge_ms=300 (past last frame), x should be at last frame x=0.5"
    );

    // judge_ms=None → no items (timer inactive)
    let state_inactive =
        SkinDrawState { judge_ms: [None; MAX_JUDGE_REGIONS], ..SkinDrawState::default() };
    let items_inactive = document.static_image_render_items(&sources, &state_inactive);
    assert_eq!(items_inactive.len(), 0, "judge_ms=None should produce no items");
}

#[test]
fn dst_if_value_selects_frame_by_enabled_option() {
    // property: option 920 enabled (1P)
    // destination dst has two conditional frames: one for 920, one for 921
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "w": 1280, "h": 720,
                "property": [
                    { "name": "Side", "def": "1P", "item": [
                        { "name": "1P", "op": 920 },
                        { "name": "2P", "op": 921 }
                    ]}
                ],
                "source": [{ "id": "src", "path": "a.png" }],
                "image": [{ "id": "img", "src": "src", "w": 10, "h": 10 }],
                "destination": [
                    { "id": "img", "dst": [
                        { "if": [920], "value": { "time": 0, "x": 100, "y": 200, "w": 50, "h": 50 } },
                        { "if": [921], "value": { "time": 0, "x": 900, "y": 200, "w": 50, "h": 50 } },
                        { "time": 500 }
                    ]}
                ]
            }
            "#,
        )
        .unwrap();

    let sources = mock_source("src", 10.0, 10.0);
    let state = SkinDrawState::default();
    let items = document.static_image_render_items(&sources, &state);

    // With option 920 (1P) enabled, x should be 100/1280
    assert_eq!(items.len(), 1);
    let SkinRenderItem::Image { rect, .. } = &items[0] else { panic!() };
    assert!(approx_eq(rect.x, 100.0 / 1280.0), "expected 1P x position, got {}", rect.x);
}

#[test]
fn skin_state_number_maps_folder_lamp_counts_on_folder_rows() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Folder,
        select_is_folder: true,
        select_folder_lamp_counts: [12, 3, 0, 1, 4, 5, 6, 7, 8, 9, 10],
        ..SkinDrawState::default()
    };
    assert_eq!(skin_state_number(320, &state), Some(12));
    assert_eq!(skin_state_number(321, &state), Some(3));
    assert_eq!(skin_state_number(324, &state), Some(4));
    assert_eq!(skin_state_number(330, &state), Some(10));

    let song = SkinDrawState {
        select_row_kind: SelectRowKind::Song,
        select_is_folder: false,
        select_in_library: true,
        select_chart_normal_notes: 900,
        ..state
    };
    assert_eq!(skin_state_number(320, &song), None);
    assert_eq!(skin_state_number(300, &song), None);
    assert_eq!(skin_state_number(350, &song), Some(900));
}

#[test]
fn select_folder_song_count_uses_cursor_folder_row() {
    let row = SelectRowSnapshot {
        is_folder: true,
        kind: SelectRowKind::Folder,
        folder_lamp_counts: [2, 3, 0, 1, 0, 0, 0, 0, 0, 0, 0],
        ..SelectRowSnapshot::default()
    };
    assert_eq!(select_row_folder_song_count(&row), Some(6));

    let song = SelectRowSnapshot { kind: SelectRowKind::Song, ..row };
    assert_eq!(select_row_folder_song_count(&song), None);
}

#[test]
fn skin_image_index_number_select_favorite_refs() {
    let state = SkinDrawState {
        select_screen: true,
        select_row_kind: SelectRowKind::Song,
        select_in_library: true,
        select_favorite_song: true,
        select_favorite_chart: false,
        select_max_bpm: 200.0,
        ..SkinDrawState::default()
    };
    assert_eq!(skin_image_index_number(89, &state), Some(1));
    assert_eq!(skin_image_index_number(90, &state), Some(0));
    assert_eq!(skin_state_number(90, &state), Some(200));

    let chart_favorite = SkinDrawState { select_favorite_chart: true, ..state };
    assert_eq!(skin_image_index_number(90, &chart_favorite), Some(1));
    assert_eq!(skin_state_number(90, &chart_favorite), Some(200));
}

#[test]
fn graph_value_select_rate_exscore_uses_selected_total_notes() {
    // ECFN select uses BARGRAPH_RATE_EXSCORE (147) for the score rate bar.
    // Select has no play-progress past_notes, so it should use the selected chart total.
    let state = SkinDrawState {
        select_screen: true,
        ex_score: 418,
        total_notes: 594,
        select_total_notes: 594,
        past_notes: 0,
        ..SkinDrawState::default()
    };
    let v = graph_value(147, &state);
    assert!((v - (418.0 / 1188.0)).abs() < 1e-5, "select ex rate: got {v}");
}

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
