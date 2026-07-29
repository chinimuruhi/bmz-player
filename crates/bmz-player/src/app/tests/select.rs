use super::*;

#[test]
fn operating_time_is_applied_to_select_snapshot() {
    let mut scene = AppSceneSnapshot::Select(SelectSnapshot::default());

    apply_operating_time_ms_to_scene(&mut scene, 90_061_234);

    let AppSceneSnapshot::Select(snapshot) = scene else {
        panic!("expected select snapshot");
    };
    assert_eq!(snapshot.operating_time_ms, 90_061_234);
}

#[test]
fn chart_snapshot_metadata_preserves_selected_chart_best_score() {
    let mut row = select_chart_row(7);
    row.best_score = Some(best_score_with_replay(456, "best.json"));
    let items = vec![SelectItem::Chart(row)];

    let (chart, best_ex_score) = chart_snapshot_metadata_for_chart(&items, 7, |_| {
        panic!("selected chart metadata should take priority")
    })
    .expect("selected chart metadata");

    assert_eq!(chart.title, "Title 7");
    assert_eq!(best_ex_score, Some(456));
}

#[test]
fn table_breadcrumb_uses_table_name_without_symbol_prefix() {
    let breadcrumb = table_breadcrumb_from_record(&DifficultyTableRecord {
        id: 1,
        source_url: "https://example.com/insane/".to_string(),
        name: "通常難易度表".to_string(),
        symbol: "★".to_string(),
        level_order: vec!["1".to_string()],
        fetched_at: 0,
    });

    assert_eq!(breadcrumb.name, "通常難易度表");
    assert_eq!(breadcrumb.symbol, "★");
}

#[test]
fn initial_folder_stack_starts_at_select_root_even_with_single_enabled_root() {
    let mut config = AppConfig::default();
    config.songs.roots =
        vec![PathEntry { path: "/music/bms".to_string(), enabled: true, recursive: true }];
    assert!(initial_folder_stack(&config).is_empty());
}

#[test]
fn skin_catalog_loads_mz_select_lua_header_when_available() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let skin_root = repo_root.join("data/skins");
    let path = skin_root.join("mz-select/music_select.luaskin");
    if !path.is_file() {
        return;
    }

    let (skin_type, candidate) =
        load_skin_candidate(&skin_root, &path, SkinCandidateOrigin::Bundled)
            .expect("load mz-select catalog candidate");

    assert_eq!(skin_type, 5);
    assert_eq!(candidate.path, "resource:skins/mz-select/music_select.luaskin");
    assert_eq!(candidate.origin, SkinCandidateOrigin::Bundled);
    assert!(candidate.name.contains("m-select"), "candidate name: {}", candidate.name);
}

#[test]
fn skin_catalog_loads_luxez_flat_select_lua_header_when_available() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let skin_root = repo_root.join("data/skins");
    let path = skin_root.join("Luxez-Flat/music_select.luaskin");
    if !path.is_file() {
        return;
    }

    let (skin_type, candidate) =
        load_skin_candidate(&skin_root, &path, SkinCandidateOrigin::Bundled)
            .expect("load Luxez-Flat catalog candidate");

    assert_eq!(skin_type, 5);
    assert_eq!(candidate.path, "resource:skins/Luxez-Flat/music_select.luaskin");
    assert_eq!(candidate.origin, SkinCandidateOrigin::Bundled);
    assert!(!candidate.name.trim().is_empty(), "candidate name should not be empty");
}

#[test]
fn select_action_maps_start_and_vertical_movement() {
    let keys = default_select_keys();
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::Enter), ElementState::Pressed, false, &keys),
        Some(SelectAction::EnterOrPlay)
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::ArrowUp), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::Previous))
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::ArrowDown), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::Next))
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::ShiftLeft), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::Previous))
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::ControlLeft), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::Next))
    );
    assert_eq!(
        select_action(
            PhysicalKey::Code(KeyCode::ControlRight),
            ElementState::Pressed,
            false,
            &keys
        ),
        Some(SelectAction::Move(SelectMove::Next))
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::ShiftRight), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::Previous))
    );
}

#[test]
fn select_option_gamepad_lane_distinguishes_same_buttons_by_device() {
    let profile = ProfileConfig::new_default("default", "Default", 0);
    let control = "Button1";

    assert_eq!(
        select_option_lane_for_gamepad(
            &profile.input,
            crate::input::gamepad::GamepadSlotMap::from_slot_ids([Some(0), Some(1)]),
            DeviceId(16),
            control,
        ),
        Some(Lane::Key1)
    );
    assert_eq!(
        select_option_lane_for_gamepad(
            &profile.input,
            crate::input::gamepad::GamepadSlotMap::from_slot_ids([Some(0), Some(1)]),
            DeviceId(17),
            control,
        ),
        Some(Lane::Key8)
    );
    assert_eq!(
        select_option_lane_for_gamepad(
            &profile.input,
            crate::input::gamepad::GamepadSlotMap::from_slot_ids([Some(1), Some(0)]),
            DeviceId(16),
            control,
        ),
        Some(Lane::Key8)
    );
}

#[test]
fn select_row_click_enters_only_when_row_is_already_selected() {
    assert_eq!(
        select_row_click_action(2, MouseButton::Left, 0, 4, false),
        Some(SelectRowClickAction::Select(2))
    );
    assert_eq!(
        select_row_click_action(2, MouseButton::Left, 2, 4, false),
        Some(SelectRowClickAction::EnterOrPlay)
    );
    assert_eq!(select_row_click_action(4, MouseButton::Left, 2, 4, false), None);
    assert_eq!(
        select_row_click_action(2, MouseButton::Right, 2, 4, false),
        Some(SelectRowClickAction::ExitFolder)
    );
    assert_eq!(
        select_row_click_action(2, MouseButton::Right, 2, 4, true),
        Some(SelectRowClickAction::CancelSettingsEdit)
    );
    assert_eq!(select_row_click_action(2, MouseButton::Middle, 2, 4, false), None);
}

#[test]
fn select_key_bindings_identify_e_action_controls() {
    let keys = default_select_keys();

    assert_eq!(keys.e_action_for_control("Q"), Some(InputActionConfig::E1));
    assert_eq!(keys.e_action_for_control("W"), Some(InputActionConfig::E2));
    assert_eq!(keys.e_action_for_control("E"), Some(InputActionConfig::E3));
    assert_eq!(keys.e_action_for_control("R"), Some(InputActionConfig::E4));
    assert_eq!(keys.e_action_for_control("Slash"), None);
}

#[test]
fn select_scroll_slider_value_maps_to_nearest_row() {
    assert_eq!(select_scroll_slider_index(0.0, 0), None);
    assert_eq!(select_scroll_slider_index(0.5, 1), Some(0));
    assert_eq!(select_scroll_slider_index(-1.0, 10), Some(0));
    assert_eq!(select_scroll_slider_index(0.0, 10), Some(0));
    assert_eq!(select_scroll_slider_index(0.49, 10), Some(4));
    assert_eq!(select_scroll_slider_index(0.50, 10), Some(5));
    assert_eq!(select_scroll_slider_index(1.0, 10), Some(9));
    assert_eq!(select_scroll_slider_index(2.0, 10), Some(9));
}

#[test]
fn skin_video_source_fast_path_updates_selected_options() {
    let document: SkinDocument = serde_json::from_str(
            r#"
            {
                "type": 5,
                "property": [
                    {
                        "name": "動画を使用する",
                        "def": "ON",
                        "item": [
                            { "name": "ON", "op": 920 },
                            { "name": "OFF", "op": 921 }
                        ]
                    }
                ],
                "source": [{ "id": "mv", "path": "mv/default.mp4" }],
                "image": [{ "id": "mv", "src": "mv", "x": 0, "y": 0, "w": 10, "h": 10 }],
                "destination": [{ "id": "mv", "op": [920], "dst": [{ "x": 0, "y": 0, "w": 10, "h": 10 }] }]
            }
            "#,
        )
        .unwrap();
    let gating = skin_video_source_gating(&document, "mv");
    let mut sources = vec![ActiveSkinVideoSource {
        texture: SkinTextureId(0),
        path: PathBuf::new(),
        decoder: None,
        last_pts: None,
        loop_start_us: 0,
        active: gating.active,
        gating_op_sets: gating.op_sets,
        enabled_options: document.enabled_options(),
        result_ranktime_ms: document.ranktime,
        failed: false,
    }];

    apply_skin_video_source_enabled_options(
        &mut sources,
        &[921],
        &skin_document_property_ops(&document),
    );

    assert_eq!(sources[0].enabled_options, vec![921]);
    assert!(!sources[0].active);
}

#[test]
fn select_action_maps_page_and_edge_movement() {
    let keys = default_select_keys();
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::PageUp), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::PagePrevious))
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::PageDown), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::PageNext))
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::Home), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::First))
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::End), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::Last))
    );
}

#[test]
fn select_action_maps_configured_lane_keys() {
    let keys = default_select_keys();
    // Key1(Z), Key3(X), Key5(C), Key7(V) → EnterOrPlay
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyZ), ElementState::Pressed, false, &keys),
        Some(SelectAction::EnterOrPlay)
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyV), ElementState::Pressed, false, &keys),
        Some(SelectAction::EnterOrPlay)
    );
    // Key2(S), Key4(D), Key6(F) → ExitFolder
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyS), ElementState::Pressed, false, &keys),
        Some(SelectAction::ExitFolder)
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyD), ElementState::Pressed, false, &keys),
        Some(SelectAction::ExitFolder)
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyF), ElementState::Pressed, false, &keys),
        Some(SelectAction::ExitFolder)
    );
    // E2(W) is also mapped to ExitFolder for direct lookup paths.
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyW), ElementState::Pressed, false, &keys),
        Some(SelectAction::ExitFolder)
    );
}

#[test]
fn select_action_maps_collection_keys() {
    let keys = default_select_keys();
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::F8), ElementState::Pressed, false, &keys),
        Some(SelectAction::FavoriteSong)
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::F9), ElementState::Pressed, false, &keys),
        Some(SelectAction::FavoriteChart)
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::Numpad8), ElementState::Pressed, false, &keys),
        Some(SelectAction::SameFolder)
    );
}

#[test]
fn select_control_action_uses_key2_binding_for_controller_back() {
    let input = crate::config::play_input::default_profile_input();
    let keys = SelectKeyBindings::from_profile(&input);

    assert!(keys.is_back("Button2"));
    assert_eq!(select_control_action("Button2", &keys), Some(SelectAction::ExitFolder));
    assert_eq!(select_control_action("Button1", &keys), Some(SelectAction::EnterOrPlay));
}

#[test]
fn select_control_action_does_not_hardcode_button2_as_back() {
    let mut input = crate::config::play_input::default_profile_input();
    let play7 = input.play.get_mut(KeyMode::K7.play_map_key()).expect("7K bindings");
    for entry in &mut play7.bindings {
        if entry.device == "gamepad" && entry.control == "Button2" {
            entry.lane = Some(LaneConfig::Key3);
        }
    }
    let keys = SelectKeyBindings::from_profile(&input);

    assert!(keys.is_enter("Button2"));
    assert_eq!(select_control_action("Button2", &keys), Some(SelectAction::EnterOrPlay));
    assert_eq!(select_control_action("Button1", &keys), Some(SelectAction::EnterOrPlay));
}

#[test]
fn key9_select_input_maps_configured_lane_keys() {
    let keys = select_keys_9k();

    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyF), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::Next))
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyD), ElementState::Pressed, false, &keys),
        Some(SelectAction::Move(SelectMove::Previous))
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyC), ElementState::Pressed, false, &keys),
        Some(SelectAction::EnterOrPlay)
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyV), ElementState::Pressed, false, &keys),
        Some(SelectAction::EnterOrPlay)
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyX), ElementState::Pressed, false, &keys),
        Some(SelectAction::ExitFolder)
    );
    assert_eq!(target_cycle_from_control("G", &keys), Some(TargetCycle::Next));
    assert_eq!(target_cycle_from_control("B", &keys), Some(TargetCycle::Previous));
}

#[test]
fn select_action_rejects_releases_repeats_and_other_keys() {
    let keys = default_select_keys();
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::ArrowDown), ElementState::Released, false, &keys),
        None
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::ArrowDown), ElementState::Pressed, true, &keys),
        None
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyA), ElementState::Pressed, false, &keys),
        None
    );
}

#[test]
fn select_wheel_move_maps_vertical_scroll_to_selection_movement() {
    assert_eq!(
        select_wheel_move(MouseScrollDelta::LineDelta(0.0, 1.0)),
        Some(SelectMove::Previous)
    );
    assert_eq!(select_wheel_move(MouseScrollDelta::LineDelta(0.0, -1.0)), Some(SelectMove::Next));
    assert_eq!(select_wheel_move(MouseScrollDelta::LineDelta(3.0, 0.0)), None);
}

#[test]
fn select_wheel_move_supports_pixel_delta() {
    assert_eq!(
        select_wheel_move(MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(
            0.0, 12.0
        ))),
        Some(SelectMove::Previous)
    );
    assert_eq!(
        select_wheel_move(MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(
            0.0, -12.0
        ))),
        Some(SelectMove::Next)
    );
}

#[test]
fn lane_cover_wheel_change_maps_vertical_scroll() {
    assert_eq!(
        lane_cover_wheel_change(MouseScrollDelta::LineDelta(0.0, 1.0)),
        Some(LaneCoverChange::Up)
    );
    assert_eq!(
        lane_cover_wheel_change(MouseScrollDelta::LineDelta(0.0, -1.0)),
        Some(LaneCoverChange::Down)
    );
    assert_eq!(lane_cover_wheel_change(MouseScrollDelta::LineDelta(1.0, 0.0)), None);
}

#[test]
fn select_click_event_arg_matches_beatoraja_click_types() {
    let rect = Rect { x: 0.2, y: 0.3, width: 0.4, height: 0.2 };
    assert_eq!(select_click_event_arg(0, MouseButton::Left, rect, 0.3, 0.4), Some(1));
    assert_eq!(select_click_event_arg(0, MouseButton::Right, rect, 0.3, 0.4), Some(-1));
    assert_eq!(select_click_event_arg(1, MouseButton::Right, rect, 0.3, 0.4), Some(1));
    assert_eq!(select_click_event_arg(2, MouseButton::Left, rect, 0.39, 0.4), Some(-1));
    assert_eq!(select_click_event_arg(2, MouseButton::Left, rect, 0.41, 0.4), Some(1));
    assert_eq!(select_click_event_arg(3, MouseButton::Left, rect, 0.3, 0.39), Some(1));
    assert_eq!(select_click_event_arg(3, MouseButton::Left, rect, 0.3, 0.41), Some(-1));
    assert_eq!(select_click_event_arg(4, MouseButton::Left, rect, 0.3, 0.4), None);
}

#[test]
fn select_key_bindings_builds_correct_hints() {
    let keys = default_select_keys();
    assert!(keys.key_hint().contains("Z/X/C/V"), "enter keys in hint: {}", keys.key_hint());
    assert!(keys.key_hint().contains("/S/D/F:BACK"), "back keys in hint: {}", keys.key_hint());
    assert!(keys.key_hint().contains(" Q"), "start key in hint: {}", keys.key_hint());
    assert!(keys.option_hint().contains("F1 MENU"), "menu in hint: {}", keys.option_hint());
    assert!(keys.option_hint().contains("F5 RELOAD"), "reload in hint: {}", keys.option_hint());
    assert!(
        keys.option_hint().contains("Q+K1/K2:1P ARR"),
        "1P arrange in hint: {}",
        keys.option_hint()
    );
    assert!(
        keys.option_hint().contains("Q+2P K1/K2:2P ARR"),
        "2P arrange in hint: {}",
        keys.option_hint()
    );
    assert!(keys.option_hint().contains("Q+K5:HS-FIX"), "HS-FIX in hint: {}", keys.option_hint());
    assert!(
        keys.option_hint().contains("Q+K6:DP OPT"),
        "DP option in hint: {}",
        keys.option_hint()
    );
    assert!(
        keys.option_hint().contains("Q+UP/DOWN:TARGET"),
        "target in hint: {}",
        keys.option_hint()
    );
}

#[test]
fn select_option_panel_maps_start_and_select_holds() {
    assert_eq!(select_option_panel_for_holds(false, false), 0);
    assert_eq!(select_option_panel_for_holds(true, false), 1);
    assert_eq!(select_option_panel_for_holds(false, true), 2);
    assert_eq!(select_option_panel_for_holds(true, true), 3);
}

#[test]
fn select_option_panel_transition_plays_open_and_close_sounds() {
    use crate::system_sound::SoundType;

    assert_eq!(select_option_panel_sound_for_transition(0, 1), Some(SoundType::OptionOpen));
    assert_eq!(select_option_panel_sound_for_transition(3, 0), Some(SoundType::OptionClose));
    assert_eq!(select_option_panel_sound_for_transition(1, 2), None);
    assert_eq!(select_option_panel_sound_for_transition(2, 3), None);
    assert_eq!(select_option_panel_sound_for_transition(0, 0), None);
}

#[test]
fn select_option_panel_transition_tracks_independent_off_timers() {
    let base = Instant::now();
    let mut current = 1;
    let mut on_started_at = base;
    let mut off_started_at = [None; 6];

    assert!(transition_select_option_panel(
        &mut current,
        &mut on_started_at,
        &mut off_started_at,
        2,
        base + Duration::from_millis(100),
    ));
    assert_eq!(current, 2);
    assert_eq!(off_started_at[0], Some(base + Duration::from_millis(100)));
    assert_eq!(off_started_at[1], None);

    assert!(transition_select_option_panel(
        &mut current,
        &mut on_started_at,
        &mut off_started_at,
        0,
        base + Duration::from_millis(200),
    ));
    assert_eq!(off_started_at[0], Some(base + Duration::from_millis(100)));
    assert_eq!(off_started_at[1], Some(base + Duration::from_millis(200)));

    assert!(transition_select_option_panel(
        &mut current,
        &mut on_started_at,
        &mut off_started_at,
        1,
        base + Duration::from_millis(300),
    ));
    assert_eq!(off_started_at[0], None);
    assert_eq!(off_started_at[1], Some(base + Duration::from_millis(200)));
    assert!(!transition_select_option_panel(
        &mut current,
        &mut on_started_at,
        &mut off_started_at,
        1,
        base + Duration::from_millis(400),
    ));
}

#[test]
fn select_hold_state_rebuilds_from_pressed_controls() {
    let keys = default_select_keys();
    let pressed = HashSet::from(["Q".to_string(), "W".to_string()]);

    let (start_held, select_held, e_action_holds) =
        select_hold_state_from_pressed_controls(&pressed, &keys);

    assert!(start_held);
    assert!(select_held);
    assert!(e_action_holds.contains(&InputActionConfig::E1));
    assert!(e_action_holds.contains(&InputActionConfig::E2));

    let pressed = HashSet::from(["W".to_string()]);
    let (start_held, select_held, e_action_holds) =
        select_hold_state_from_pressed_controls(&pressed, &keys);

    assert!(!start_held);
    assert!(select_held);
    assert!(!e_action_holds.contains(&InputActionConfig::E1));
    assert!(e_action_holds.contains(&InputActionConfig::E2));
}

#[test]
fn select_analog_scroll_delta_maps_scratch_bindings() {
    let gamepad_keys =
        SelectKeyBindings::from_profile(&ProfileConfig::new_default("default", "Default", 1).input);
    // Axis1+ = scratch up (Previous = 負), Axis1- = scratch down (Next = 正)
    assert_eq!(select_analog_scroll_delta("Axis1", 4, &gamepad_keys), Some(-4));
    assert_eq!(select_analog_scroll_delta("Axis1", -4, &gamepad_keys), Some(4));
    assert_eq!(select_analog_scroll_delta("Axis2", -4, &gamepad_keys), None);
    assert_eq!(select_analog_scroll_delta("Axis1", 0, &gamepad_keys), None);
    assert_eq!(select_analog_scroll_delta("Axis3", 4, &gamepad_keys), None);
}

#[test]
fn settings_edit_analog_scroll_uses_scratch_direction() {
    assert_eq!(settings_edit_direction_from_analog_scroll(3), 1);
    assert_eq!(settings_edit_direction_from_analog_scroll(-2), -1);
    assert_eq!(settings_edit_direction_from_analog_scroll(0), 0);
}

#[test]
fn settings_edit_mouse_wheel_uses_scroll_direction() {
    assert_eq!(settings_edit_direction_from_mouse_wheel(MouseScrollDelta::LineDelta(0.0, 1.0)), 1);
    assert_eq!(
        settings_edit_direction_from_mouse_wheel(MouseScrollDelta::PixelDelta(
            winit::dpi::PhysicalPosition::new(0.0, -12.0)
        )),
        -1
    );
}

#[test]
fn update_analog_scroll_buffer_suppresses_until_idle() {
    let mut buffer = 0;
    let mut suppress = true;
    // 回転継続中 (idle=false) は捨て続ける
    update_analog_scroll_buffer(&mut buffer, &mut suppress, false, 5);
    assert_eq!(buffer, 0);
    assert!(suppress);
    // 一度止まった後の tick から蓄積再開
    update_analog_scroll_buffer(&mut buffer, &mut suppress, true, 2);
    assert_eq!(buffer, 2);
    assert!(!suppress);
    update_analog_scroll_buffer(&mut buffer, &mut suppress, false, 3);
    assert_eq!(buffer, 5);
    // 通常時も idle で端数を破棄
    update_analog_scroll_buffer(&mut buffer, &mut suppress, true, 1);
    assert_eq!(buffer, 1);
}

#[test]
fn take_analog_scroll_steps_keeps_remainder() {
    let mut buffer = 7;
    assert_eq!(take_analog_scroll_steps(&mut buffer, 3), 2);
    assert_eq!(buffer, 1);

    let mut buffer = -7;
    assert_eq!(take_analog_scroll_steps(&mut buffer, 3), -2);
    assert_eq!(buffer, -1);

    let mut buffer = 2;
    assert_eq!(take_analog_scroll_steps(&mut buffer, 3), 0);
    assert_eq!(buffer, 2);
}

#[test]
fn select_modifier_keys_are_handled_before_folder_back() {
    let keys = default_select_keys();
    assert!(!is_select_modifier_key(PhysicalKey::Code(KeyCode::ArrowLeft), &keys));
    assert!(is_select_modifier_key(PhysicalKey::Code(KeyCode::KeyW), &keys));
    assert!(!is_select_modifier_key(PhysicalKey::Code(KeyCode::KeyS), &keys));
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::ArrowLeft), ElementState::Pressed, false, &keys),
        Some(SelectAction::ExitFolder)
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyW), ElementState::Pressed, false, &keys),
        Some(SelectAction::ExitFolder)
    );
    assert_eq!(
        select_action(PhysicalKey::Code(KeyCode::KeyS), ElementState::Pressed, false, &keys),
        Some(SelectAction::ExitFolder)
    );
}

#[test]
fn select_start_key_uses_profile_start_binding() {
    let keys = default_select_keys();
    assert!(is_select_start_key(PhysicalKey::Code(KeyCode::KeyQ), &keys));
    assert!(!is_select_start_key(PhysicalKey::Code(KeyCode::KeyW), &keys));
    assert!(!is_select_start_key(PhysicalKey::Code(KeyCode::KeyS), &keys));
}

#[test]
fn select_key_bindings_map_e1_plus_key7_to_autoplay_option() {
    let keys = default_select_keys();

    assert!(keys.is_start("Q"));
    assert!(keys.is_ui_key7("V"));
    assert!(keys.is_enter("V"));
}

#[test]
fn select_key_bindings_include_e3_action() {
    let keys = default_select_keys();

    assert!(keys.is_e3_action("E"));
}

#[test]
fn select_key_bindings_expose_key2_for_gas_toggle() {
    let keys = default_select_keys();

    assert!(keys.is_start("Q"));
    assert!(keys.is_back("W"));
    assert!(keys.is_back("S"));
    assert!(keys.is_back("D"));
    assert!(keys.is_back("F"));
    assert!(keys.is_key2("S"));
}

#[test]
fn select_key_bindings_expose_2p_keys_for_random2() {
    let keys = default_select_keys();

    assert!(keys.is_key8("M"));
    assert!(keys.is_key9("K"));
    assert!(keys.is_key10("Comma"));
    assert!(keys.is_key11("L"));
    assert!(keys.is_key12("Period"));
    assert!(keys.is_key13("Semicolon"));
    assert!(keys.is_key14("Slash"));
}

#[test]
fn select_key_bindings_treat_2p_keys_as_ui_equivalents() {
    let keys = select_keys_with_full_2p_bindings();

    for control in ["M", "Comma", "Period", "Slash", "P2K7"] {
        assert!(keys.is_enter(control), "{control} should decide like odd 1P keys");
    }
    for control in ["K", "L", "Semicolon", "P2K6"] {
        assert!(keys.is_back(control), "{control} should go back like even 1P keys");
    }
    assert_eq!(keys.ui_lane_for_control("M"), Some(Lane::Key1));
    assert_eq!(keys.ui_lane_for_control("K"), Some(Lane::Key2));
    assert_eq!(keys.ui_lane_for_control("Comma"), Some(Lane::Key3));
    assert_eq!(keys.ui_lane_for_control("L"), Some(Lane::Key4));
    assert_eq!(keys.ui_lane_for_control("Period"), Some(Lane::Key5));
    assert_eq!(keys.ui_lane_for_control("Semicolon"), Some(Lane::Key6));
    assert_eq!(keys.ui_lane_for_control("Slash"), Some(Lane::Key7));
    assert_eq!(keys.ui_lane_for_control("P2K6"), Some(Lane::Key6));
    assert_eq!(keys.ui_lane_for_control("P2K7"), Some(Lane::Key7));
}

#[test]
fn select_gauge_auto_shift_toggle_requires_start_then_key2() {
    let keys = default_select_keys();

    assert!(should_toggle_select_gauge_auto_shift("S", true, true, &keys));
    assert!(should_toggle_select_gauge_auto_shift("K", true, true, &keys));
    assert!(!should_toggle_select_gauge_auto_shift("Q", false, true, &keys));
    assert!(!should_toggle_select_gauge_auto_shift("Q", true, true, &keys));
    assert!(!should_toggle_select_gauge_auto_shift("W", true, false, &keys));
}

#[test]
fn select_judge_auto_adjust_toggle_requires_start_then_key3() {
    let keys = default_select_keys();

    assert!(should_toggle_select_judge_auto_adjust("X", true, true, &keys));
    assert!(should_toggle_select_judge_auto_adjust("Comma", true, true, &keys));
    assert!(!should_toggle_select_judge_auto_adjust("X", false, true, &keys));
    assert!(!should_toggle_select_judge_auto_adjust("S", true, true, &keys));
    assert!(!should_toggle_select_judge_auto_adjust("W", true, false, &keys));
}

#[test]
fn select_skin_cover_events_toggle_sudden_and_hidden_independently() {
    assert_eq!(toggled_select_sudden(LaneEffectConfig::Off), LaneEffectConfig::Sudden);
    assert_eq!(toggled_select_sudden(LaneEffectConfig::Hidden), LaneEffectConfig::HiddenSudden);
    assert_eq!(toggled_select_sudden(LaneEffectConfig::HiddenSudden), LaneEffectConfig::Hidden);

    assert_eq!(toggled_select_hidden(LaneEffectConfig::Off), LaneEffectConfig::Hidden);
    assert_eq!(toggled_select_hidden(LaneEffectConfig::Sudden), LaneEffectConfig::HiddenSudden);
    assert_eq!(toggled_select_hidden(LaneEffectConfig::HiddenSudden), LaneEffectConfig::Sudden);
}

#[test]
fn result_panel_direct_selection_matches_tab_availability() {
    assert_eq!(selected_result_panel(1, 2, true, true), Some(2));
    assert_eq!(selected_result_panel(2, 1, true, true), Some(1));
    assert_eq!(selected_result_panel(2, 1, true, false), None);
    assert_eq!(selected_result_panel(1, 2, true, false), Some(2));
    assert_eq!(selected_result_panel(2, 2, true, true), None);
    assert_eq!(selected_result_panel(1, 2, false, true), None);
}

#[test]
fn play_option_control_uses_chart_mode_instead_of_select_input_mode() {
    let input = crate::config::play_input::default_profile_input();
    assert_eq!(input.select_input_mode, SelectInputModeConfig::Key7Key14);
    let keys = SelectKeyBindings::from_profile(&input);
    let play_input = play_option_input_for(&input, KeyMode::K9);

    assert_eq!(
        keyboard_play_option("B", true, false, &keys, &play_input, &input),
        Some(PlayOptionControl::Hispeed(HispeedChange::Down))
    );
    assert_eq!(
        keyboard_play_option("G", true, false, &keys, &play_input, &input),
        Some(PlayOptionControl::Hispeed(HispeedChange::Up))
    );
}

#[test]
fn select_skin_green_number_uses_profile_target_green_for_nhs() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.lane.hispeed = 2.0;
    profile.lane.hispeed_mode = HispeedModeConfig::Normal;
    profile.lane.target_green_number = 300;

    assert_eq!(WinitApp::select_note_display_duration_ms_for_skin(&profile), 300);
}

#[test]
fn select_skin_green_number_uses_target_green_for_fhs() {
    let mut profile = ProfileConfig::new_default("default", "Default", 1);
    profile.lane.hispeed_mode = HispeedModeConfig::Floating;
    profile.lane.target_green_number = 280;

    assert_eq!(WinitApp::select_note_display_duration_ms_for_skin(&profile), 280);
}

#[test]
fn profile_play_option_changes_sync_all_select_runtime_options() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let before = profile.play.clone();
    let current = select_play_options_from_profile(&before);
    let mut after = before.clone();
    after.gauge = GaugeTypeConfig::AutoShift;
    after.gauge_auto_shift = GaugeAutoShiftConfig::Continue;
    after.bottom_shiftable_gauge = BottomShiftableGaugeConfig::Normal;
    after.random = RandomOptionConfig::SRandom;
    after.random2 = RandomOptionConfig::RRandom;
    after.double_option = DoubleOptionConfig::Flip;
    after.hs_fix = HsFixConfig::MainBpm;
    after.target = TargetOptionConfig::RankAaa;
    after.auto_play = true;

    let synced = merge_changed_select_play_options_from_profile(current, &before, &after);

    assert_eq!(synced.gauge, GaugeTypeConfig::ExHard);
    assert_eq!(synced.gauge_auto_shift, GaugeAutoShiftConfig::BestClear);
    assert_eq!(synced.bottom_shiftable_gauge, BottomShiftableGaugeConfig::Normal);
    assert_eq!(synced.arrange, ArrangeOption::SRandom);
    assert_eq!(synced.arrange_2p, ArrangeOption::RRandom);
    assert_eq!(synced.double_option, DoubleOption::Flip);
    assert_eq!(synced.hs_fix, HsFixOption::MainBpm);
    assert_eq!(synced.target, TargetOption::RankAaa);
    assert_eq!(synced.session_mode, SessionMode::Autoplay);
}

#[test]
fn select_score_context_changes_only_for_rule_or_ln_mode() {
    let profile = ProfileConfig::new_default("default", "Default", 1);
    let before = SelectScoreContext::from_profile(&profile);

    let mut random_changed = profile.clone();
    random_changed.play.random = RandomOptionConfig::Mirror;
    assert_eq!(before, SelectScoreContext::from_profile(&random_changed));

    let mut rule_changed = profile.clone();
    rule_changed.play.rule_mode = RuleMode::Dx;
    assert_ne!(before, SelectScoreContext::from_profile(&rule_changed));

    let mut ln_changed = profile;
    ln_changed.play.ln_mode_policy = LnPolicySetting::ForceCn;
    assert_ne!(before, SelectScoreContext::from_profile(&ln_changed));
}

#[test]
fn select_bgm_is_skipped_when_preview_is_already_playing() {
    assert!(should_play_select_bgm_on_enter(false));
    assert!(!should_play_select_bgm_on_enter(true));
}

#[test]
fn select_preview_fade_factor_ramps_in_and_out() {
    let started_at = Instant::now();
    let half = started_at + SELECT_PREVIEW_FADE_DURATION / 2;
    let done = started_at + SELECT_PREVIEW_FADE_DURATION;

    assert_eq!(
        select_preview_fade_factor(SelectPreviewFade::FadingIn { started_at }, started_at),
        0.0
    );
    assert!(
        (select_preview_fade_factor(SelectPreviewFade::FadingIn { started_at }, half) - 0.5).abs()
            < 0.001
    );
    assert_eq!(select_preview_fade_factor(SelectPreviewFade::FadingIn { started_at }, done), 1.0);
    assert!(
        (select_preview_fade_factor(SelectPreviewFade::FadingOut { started_at }, half) - 0.5).abs()
            < 0.001
    );
    assert_eq!(select_preview_fade_factor(SelectPreviewFade::FadingOut { started_at }, done), 0.0);
}

#[test]
fn select_preview_normalization_gain_follows_chart_normalization_setting() {
    assert_eq!(select_preview_normalization_gain(true, 0.25), 0.25);
    assert_eq!(select_preview_normalization_gain(false, 0.25), 1.0);
    assert_eq!(select_preview_normalization_gain(true, f32::NAN), 1.0);
    assert_eq!(select_preview_normalization_gain(true, 1.5), 1.0);
}

#[test]
fn prepare_select_preview_keeps_sample_with_analyzed_gain() {
    let sample = DecodedSample { channels: 2, sample_rate: 48_000, frames: vec![1.0; 480] };

    let prepared = prepare_select_preview(sample.clone());

    assert_eq!(prepared.sample.frames, sample.frames);
    assert!(prepared.normalization_gain > 0.0);
    assert!(prepared.normalization_gain < 1.0);
}

#[test]
fn select_preview_key_waits_for_beatoraja_start_delay() {
    let key = Some("folder|preview.ogg".to_string());

    assert_eq!(
        select_preview_key_after_delay(
            key.clone(),
            SELECT_PREVIEW_START_DELAY - Duration::from_millis(1),
            SELECT_PREVIEW_START_DELAY,
        ),
        None
    );
    assert_eq!(
        select_preview_key_after_delay(
            key.clone(),
            SELECT_PREVIEW_START_DELAY,
            SELECT_PREVIEW_START_DELAY,
        ),
        key
    );
}

#[test]
fn select_preview_load_queue_keeps_only_latest_pending_request() {
    let mut queue = SelectPreviewLoadQueue::default();

    assert_eq!(queue.request("first".to_string()), Some("first".to_string()));
    assert_eq!(queue.request("second".to_string()), None);
    assert_eq!(queue.request("latest".to_string()), None);
    assert_eq!(queue.finish(), Some("latest".to_string()));
    assert_eq!(queue.finish(), None);
    assert_eq!(queue.request("after-idle".to_string()), Some("after-idle".to_string()));
}

#[test]
fn select_preview_uses_generated_fallback_after_explicit_preview_fails() {
    assert!(should_use_generated_preview("", false));
    assert!(should_use_generated_preview("missing-preview.ogg", true));
    assert!(!should_use_generated_preview("preview.ogg", false));
}

#[test]
fn audio_diagnostic_marks_generated_preview_callback_pressure() {
    assert_eq!(
        classify_audio_output_issue(0, 0, 0, 0, 0, 0, true, 0, true),
        AudioOutputIssueCause::GeneratedPreviewCpuPressure
    );
    assert_eq!(
        classify_audio_output_issue(0, 0, 1, 0, 0, 0, true, 0, true),
        AudioOutputIssueCause::CallbackLockContention
    );
    assert_eq!(
        classify_audio_output_issue(0, 0, 0, 0, 0, 0, false, 1, true),
        AudioOutputIssueCause::MixClipping
    );
    assert_eq!(
        classify_audio_output_issue(0, 0, 0, 0, 1, 0, false, 0, false),
        AudioOutputIssueCause::Unknown
    );
}

#[test]
fn select_snapshot_rows_centers_selection_and_copies_score_summary() {
    let rows: Vec<SelectItem> = (0..10)
        .map(|index| {
            let mut row = select_chart_row(index);
            if index == 5 {
                if let Some(analysis) = &mut row.chart_analysis {
                    analysis.speed_changes = vec![
                        crate::storage::library_db::ChartSpeedChange { speed: 100.0, time_ms: 0 },
                        crate::storage::library_db::ChartSpeedChange {
                            speed: 200.0,
                            time_ms: 45_000,
                        },
                    ];
                }
                let mut best_score = best_score_with_replay(1234, "replay/test.toml");
                best_score.bp = 12;
                best_score.cb = 8;
                best_score.max_combo = 345;
                row.best_score = Some(best_score);
                row.replay_slots = [true, false, false, false];
                row.table_text =
                    DifficultyTableText::from_parts("Test Table".to_string(), "T", "5");
                row.table_level = row.table_text.table_level.clone();
            }
            SelectItem::Chart(row)
        })
        .collect();

    let profile = ProfileConfig::new_default("default", "Default", 0);
    let mut chart_distributions = HashMap::new();
    chart_distributions.insert(
        5,
        vec![crate::storage::library_db::ChartDistributionSecond {
            key_taps: 2,
            key_long_heads: 1,
            ..Default::default()
        }],
    );
    let snapshot_rows = select_snapshot_rows(&rows, 5, 7, &profile, None, &chart_distributions);

    assert_eq!(snapshot_rows.len(), 7);
    assert_eq!(snapshot_rows[0].index, 2);
    assert_eq!(snapshot_rows[3].index, 5);
    assert_eq!(snapshot_rows[3].title, "Title 5");
    assert_eq!(snapshot_rows[3].clear_type, "Normal");
    assert_eq!(snapshot_rows[3].ex_score, Some(1234));
    assert_eq!(snapshot_rows[3].bp, Some(12));
    assert_eq!(snapshot_rows[3].cb, Some(8));
    assert_eq!(snapshot_rows[3].max_combo, Some(345));
    assert_eq!(snapshot_rows[3].judge_rank, Some(1));
    assert_eq!(snapshot_rows[3].play_count, 42);
    assert_eq!(snapshot_rows[3].clear_count, 31);
    assert_eq!(snapshot_rows[3].replay_slots, [true, false, false, false]);
    assert_eq!(snapshot_rows[3].chart_normal_notes, 45);
    assert_eq!(snapshot_rows[3].chart_long_notes, 6);
    assert_eq!(snapshot_rows[3].chart_peak_density, 12.5);
    assert_eq!(snapshot_rows[3].chart_distribution.len(), 1);
    assert_eq!(snapshot_rows[3].chart_distribution[0].key_taps, 2);
    assert_eq!(snapshot_rows[3].chart_bpm_graph_segments.len(), 2);
    assert_eq!(snapshot_rows[3].chart_bpm_graph_segments[0].start_ratio, 0.0);
    assert_eq!(snapshot_rows[3].chart_bpm_graph_segments[0].end_ratio, 0.5);
    assert_eq!(snapshot_rows[3].chart_bpm_graph_segments[1].start_ratio, 0.5);
    assert_eq!(snapshot_rows[3].chart_bpm_graph_segments[1].end_ratio, 1.0);
    assert_eq!(snapshot_rows[3].table_text_primary, "Test Table");
    assert_eq!(snapshot_rows[3].table_text_secondary, "T5");
    assert_eq!(snapshot_rows[3].table_text_fallback, "T5Test Table");
}

#[test]
fn select_snapshot_rows_preserves_settings_action_kinds() {
    let rows = vec![SelectItem::SettingsBack, SelectItem::SettingsClose];
    let profile = ProfileConfig::new_default("default", "Default", 0);

    let snapshot_rows = select_snapshot_rows(&rows, 0, 2, &profile, None, &HashMap::new());

    let back = snapshot_rows
        .iter()
        .find(|row| row.kind == bmz_render::scene::SelectRowKind::SettingsBack)
        .unwrap();
    let close = snapshot_rows
        .iter()
        .find(|row| row.kind == bmz_render::scene::SelectRowKind::SettingsClose)
        .unwrap();
    assert_eq!(back.title, "戻る");
    assert_eq!(close.title, "閉じる");
    assert!(back.is_folder);
    assert!(close.is_folder);
}

#[test]
fn select_snapshot_rows_uses_policy_scored_note_count() {
    let mut row = select_chart_row(0);
    let chart = row.chart.as_mut().unwrap();
    chart.total_notes = 100;
    chart.bms_total = 0.0;
    chart.ln_profile =
        crate::ln_policy::ChartLnProfile { has_defined_cn: true, ..Default::default() };
    chart.ln_counts = crate::ln_policy::ChartLnCounts { defined_cn_pairs: 2, ..Default::default() };
    let rows = vec![SelectItem::Chart(row)];
    let profile = ProfileConfig::new_default("default", "Default", 0);

    let snapshot = select_snapshot_rows(&rows, 0, 1, &profile, None, &HashMap::new());

    assert_eq!(snapshot[0].total_notes, 102);
    assert_eq!(snapshot[0].chart_total_gauge, bmz_gameplay::gauge::default_gauge_total(102) as f32);
}

#[test]
fn select_snapshot_rows_copies_course_best_score_summary() {
    let mut row = select_course_row(2, 2);
    row.best_score = Some(crate::storage::score_db::CourseBestScore {
        course_score_id: 99,
        course_hash: "course-hash".to_string(),
        rule_mode: bmz_gameplay::rule::RuleMode::Beatoraja,
        ex_score: 1234,
        max_ex_score: 2000,
        clear_type: "Hard".to_string(),
        gauge_type: "Class".to_string(),
        gauge_value: 80.0,
        max_combo: 345,
        bp: 12,
        cb: 8,
        judge_counts: DisplayJudgeCounts {
            pgreat: 500,
            great: 100,
            good: 20,
            bad: 10,
            poor: 5,
            empty_poor: 3,
        },
        fast_slow_counts: bmz_render::snapshot::FastSlowJudgeCounts {
            fast_pgreat: 300,
            slow_pgreat: 200,
            ..Default::default()
        },
        course_failed: false,
        course_clear: true,
        play_count: 42,
        clear_count: 31,
        played_at: 1,
    });
    row.replay_slots = [true, false, true, false];
    let rows = vec![SelectItem::Course(row)];

    let profile = ProfileConfig::new_default("default", "Default", 0);
    let snapshot_rows = select_snapshot_rows(&rows, 0, 1, &profile, None, &HashMap::new());

    assert_eq!(snapshot_rows.len(), 1);
    assert_eq!(snapshot_rows[0].kind, bmz_render::scene::SelectRowKind::Course);
    assert!(snapshot_rows[0].play_level.is_empty());
    assert_eq!(snapshot_rows[0].clear_type, "Hard");
    assert_eq!(snapshot_rows[0].ex_score, Some(1234));
    assert_eq!(snapshot_rows[0].bp, Some(12));
    assert_eq!(snapshot_rows[0].cb, Some(8));
    assert_eq!(snapshot_rows[0].max_combo, Some(345));
    assert_eq!(snapshot_rows[0].judge_counts.pgreat, 500);
    assert_eq!(snapshot_rows[0].judge_counts.empty_poor, 3);
    assert_eq!(snapshot_rows[0].fast_slow_counts.unwrap().fast_pgreat, 300);
    assert_eq!(snapshot_rows[0].play_count, 42);
    assert_eq!(snapshot_rows[0].clear_count, 31);
    assert_eq!(snapshot_rows[0].replay_slots, [true, false, true, false]);
}

#[test]
fn select_snapshot_rows_wraps_near_edges() {
    let rows: Vec<SelectItem> = (0..4).map(|i| SelectItem::Chart(select_chart_row(i))).collect();

    let profile = ProfileConfig::new_default("default", "Default", 0);
    let snapshot_rows = select_snapshot_rows(&rows, 0, 7, &profile, None, &HashMap::new());

    assert_eq!(snapshot_rows.len(), 7);
    assert_eq!(
        snapshot_rows.iter().map(|row| row.index).collect::<Vec<_>>(),
        vec![1, 2, 3, 0, 1, 2, 3]
    );
}

#[test]
fn select_snapshot_rows_keeps_twelve_rows_around_selection() {
    let rows: Vec<SelectItem> = (0..30).map(|i| SelectItem::Chart(select_chart_row(i))).collect();

    let profile = ProfileConfig::new_default("default", "Default", 0);
    let snapshot_rows = select_snapshot_rows(&rows, 2, 25, &profile, None, &HashMap::new());

    assert_eq!(snapshot_rows.len(), 25);
    assert_eq!(snapshot_rows[0].index, 20);
    assert_eq!(snapshot_rows[12].index, 2);
    assert_eq!(snapshot_rows[24].index, 14);
}

#[test]
fn moved_select_index_moves_by_single_page_and_wraps_edges() {
    assert_eq!(moved_select_index(4, 10, SelectMove::Previous), 3);
    assert_eq!(moved_select_index(4, 10, SelectMove::Next), 5);
    assert_eq!(moved_select_index(9, 10, SelectMove::Next), 0);
    assert_eq!(moved_select_index(0, 10, SelectMove::Previous), 9);
    assert_eq!(moved_select_index(8, 10, SelectMove::PagePrevious), 1);
    assert_eq!(moved_select_index(4, 10, SelectMove::PagePrevious), 7);
    assert_eq!(moved_select_index(7, 10, SelectMove::PageNext), 4);
    assert_eq!(moved_select_index(0, 10, SelectMove::Last), 9);
    assert_eq!(moved_select_index(9, 10, SelectMove::First), 0);
}

#[test]
fn moved_select_index_handles_empty_rows() {
    assert_eq!(moved_select_index(9, 0, SelectMove::Last), 0);
}

#[test]
fn select_scroll_duration_config_uses_beatoraja_bounds() {
    let mut config = AppConfig::default();
    config.select.scroll_duration_low_ms = 0;
    config.select.scroll_duration_high_ms = 0;
    assert_eq!(select_scroll_duration_low_ms(&config), 2);
    assert_eq!(select_scroll_duration_high_ms(&config), 1);

    config.select.scroll_duration_low_ms = 5_000;
    config.select.scroll_duration_high_ms = 5_000;
    assert_eq!(select_scroll_duration_low_ms(&config), 1000);
    assert_eq!(select_scroll_duration_high_ms(&config), 1000);
}

#[test]
fn select_move_scroll_direction_matches_row_movement() {
    assert_eq!(select_move_scroll_direction(SelectMove::Previous), -1);
    assert_eq!(select_move_scroll_direction(SelectMove::Next), 1);
    assert_eq!(select_move_scroll_direction(SelectMove::PagePrevious), -1);
    assert_eq!(select_move_scroll_direction(SelectMove::PageNext), 1);
    assert_eq!(select_move_scroll_direction(SelectMove::First), 0);
    assert_eq!(select_move_scroll_direction(SelectMove::Last), 0);
}

#[test]
fn select_skin_event_state_cycles_supported_mode_filters() {
    assert_eq!(SelectModeFilter::All.next(), SelectModeFilter::K7);
    assert_eq!(SelectModeFilter::All.previous(), SelectModeFilter::K10);
    assert_eq!(SelectSort::Title.next(), SelectSort::Artist);
    assert_eq!(SelectSort::Title.previous(), SelectSort::Bp);
    assert_eq!(
        crate::ln_policy::LnPolicySetting::AutoLn.next(),
        crate::ln_policy::LnPolicySetting::AutoCn
    );
    assert_eq!(
        crate::ln_policy::LnPolicySetting::AutoLn.previous(),
        crate::ln_policy::LnPolicySetting::ForceHcn
    );
    assert_eq!(crate::ln_policy::LnPolicySetting::ForceHcn.display_label(), "FORCE(HCN)");
    assert_eq!(
        cycle_gauge_option_with_direction(GaugeTypeConfig::Normal, 1),
        GaugeTypeConfig::Hard
    );
    assert_eq!(
        cycle_gauge_option_with_direction(GaugeTypeConfig::Normal, -1),
        GaugeTypeConfig::Easy
    );
    assert_eq!(
        cycle_arrange_option_with_direction(ArrangeOption::Normal, -1),
        ArrangeOption::MFRandom
    );
    assert_eq!(
        cycle_double_option_with_direction(DoubleOption::Off, -1),
        DoubleOption::BattleAutoScratch
    );
    assert_eq!(cycle_hs_fix_option_with_direction(HsFixOption::Off, 1), HsFixOption::StartBpm);
    assert_eq!(cycle_hs_fix_option_with_direction(HsFixOption::StartBpm, 1), HsFixOption::MaxBpm);
    assert_eq!(cycle_hs_fix_option_with_direction(HsFixOption::MaxBpm, 1), HsFixOption::MainBpm);
    assert_eq!(cycle_hs_fix_option_with_direction(HsFixOption::MainBpm, 1), HsFixOption::MinBpm);
    assert_eq!(cycle_hs_fix_option_with_direction(HsFixOption::Off, -1), HsFixOption::MinBpm);
    assert_eq!(cycle_bga_option_with_direction(BgaModeConfig::On, -1), BgaModeConfig::Off);
    assert_eq!(
        cycle_bga_expand_with_direction(BgaExpandConfig::KeepAspect, 1),
        BgaExpandConfig::Full
    );
    assert_eq!(
        cycle_gauge_auto_shift_option_with_direction(GaugeAutoShiftConfig::Off, -1),
        GaugeAutoShiftConfig::SelectToUnder
    );
    assert_eq!(
        cycle_judge_algorithm_with_direction(JudgeAlgorithmConfig::Combo, 1),
        JudgeAlgorithmConfig::Duration
    );
    assert_eq!(
        cycle_judge_algorithm_with_direction(JudgeAlgorithmConfig::Combo, -1),
        JudgeAlgorithmConfig::Lowest
    );
}

#[test]
fn select_ir_context_separates_source_resolved_score_keys() {
    let auto_ln = select_ir_cache_context(
        crate::ln_policy::LnPolicySetting::AutoLn,
        crate::ln_policy::LnScorePolicy::AutoLn,
        crate::select_options::DoubleOptionScoreBucket::Off,
        bmz_gameplay::rule::RuleMode::Beatoraja,
    );
    let auto_cn = select_ir_cache_context(
        crate::ln_policy::LnPolicySetting::AutoLn,
        crate::ln_policy::LnScorePolicy::AutoCn,
        crate::select_options::DoubleOptionScoreBucket::Off,
        bmz_gameplay::rule::RuleMode::Beatoraja,
    );

    assert_ne!(auto_ln, auto_cn);
}

#[test]
fn select_mode_filter_keeps_matching_chart_rows() {
    let mut k7 = select_chart_row(1);
    k7.chart.as_mut().unwrap().mode = "7K".to_string();
    let mut k14 = select_chart_row(2);
    k14.chart.as_mut().unwrap().mode = "14K".to_string();
    let mut items = vec![
        SelectItem::Folder {
            path: "folder".to_string(),
            name: "folder".to_string(),
            kind: SelectRowKind::Folder,
            summary: None,
        },
        SelectItem::Chart(k7),
        SelectItem::Chart(k14),
    ];

    apply_select_mode_filter(&mut items, SelectModeFilter::K14);

    assert_eq!(items.len(), 2);
    assert!(matches!(items[0], SelectItem::Folder { .. }));
    assert_eq!(items[1].display_name(), "Title 2");
}

#[test]
fn resolve_mode_filter_keeps_mode_with_matching_charts() {
    let items = vec![chart_row_with_mode(1, "7K"), chart_row_with_mode(2, "5K")];
    // 7K のチャートがあるので据え置く。
    assert_eq!(resolve_non_empty_mode_filter(&items, SelectModeFilter::K7), SelectModeFilter::K7);
}

#[test]
fn resolve_mode_filter_advances_when_all_charts_mismatch() {
    // 5K しか無いフォルダで 7K フィルターを掛けると全消えになるため、
    // beatoraja 同様に前方向 (K7 -> K14 -> K9 -> K5) へ送って K5 で止まる。
    let items = vec![chart_row_with_mode(1, "5K"), chart_row_with_mode(2, "5K")];
    assert_eq!(resolve_non_empty_mode_filter(&items, SelectModeFilter::K7), SelectModeFilter::K5);
}

#[test]
fn resolve_mode_filter_does_not_advance_when_folder_remains() {
    // フォルダ行が残るなら全消えにはならないので据え置く（beatoraja 準拠）。
    let items = vec![
        SelectItem::Folder {
            path: "folder".to_string(),
            name: "folder".to_string(),
            kind: SelectRowKind::Folder,
            summary: None,
        },
        chart_row_with_mode(1, "5K"),
    ];
    assert_eq!(resolve_non_empty_mode_filter(&items, SelectModeFilter::K7), SelectModeFilter::K7);
}

#[test]
fn resolve_mode_filter_keeps_all_filter() {
    let items = vec![chart_row_with_mode(1, "5K")];
    assert_eq!(resolve_non_empty_mode_filter(&items, SelectModeFilter::All), SelectModeFilter::All);
}

#[test]
fn select_mode_filter_roundtrips_through_str() {
    for mode in SelectModeFilter::ORDER {
        assert_eq!(SelectModeFilter::from_str_or_default(mode.as_str()), mode);
    }
    assert_eq!(SelectModeFilter::from_str_or_default("24K"), SelectModeFilter::All);
    assert_eq!(SelectModeFilter::from_str_or_default("24K_DOUBLE"), SelectModeFilter::All);
    assert_eq!(SelectModeFilter::from_str_or_default("unknown"), SelectModeFilter::All);
}

#[test]
fn select_sort_roundtrips_through_str() {
    for sort in SelectSort::ORDER {
        assert_eq!(SelectSort::from_str_or_default(sort.as_str()), sort);
    }
    assert_eq!(SelectSort::from_str_or_default("unknown"), SelectSort::Title);
}

#[test]
fn select_sort_orders_chart_rows_without_moving_folders() {
    let mut slow = select_chart_row(1);
    slow.chart.as_mut().unwrap().title = "Slow".to_string();
    slow.chart.as_mut().unwrap().initial_bpm = 100.0;
    let mut fast = select_chart_row(2);
    fast.chart.as_mut().unwrap().title = "Fast".to_string();
    fast.chart.as_mut().unwrap().initial_bpm = 200.0;
    let mut items = vec![
        SelectItem::Folder {
            path: "folder".to_string(),
            name: "folder".to_string(),
            kind: SelectRowKind::Folder,
            summary: None,
        },
        SelectItem::Chart(fast),
        SelectItem::Chart(slow),
    ];

    apply_select_sort(&mut items, SelectSort::Bpm);

    assert!(matches!(items[0], SelectItem::Folder { .. }));
    assert_eq!(items[1].display_name(), "Slow");
    assert_eq!(items[2].display_name(), "Fast");
}

#[test]
fn restored_select_index_keeps_chart_when_clear_sort_moves_after_score_update() {
    let mut played = select_chart_row(1);
    played.chart.as_mut().unwrap().title = "Played".to_string();
    let mut other = select_chart_row(2);
    other.chart.as_mut().unwrap().title = "Other".to_string();
    let old_items = [SelectItem::Chart(played.clone()), SelectItem::Chart(other.clone())];
    let selected_key = select_item_key(&old_items[0]);

    played.best_score = Some(BestScoreSummary {
        clear_type: "Hard".to_string(),
        ..best_score_with_replay(100, "played.json")
    });
    let mut new_items = vec![SelectItem::Chart(played), SelectItem::Chart(other)];
    apply_select_sort(&mut new_items, SelectSort::Clear);

    assert_eq!(restored_select_index(&new_items, Some(&selected_key), 0), 1);
    assert_eq!(new_items[1].display_name(), "Played");
}

#[test]
fn select_item_key_uses_typed_settings_identity() {
    let config = SelectItem::Config(crate::screens::settings_model::ConfigSelectRow {
        entry_id: SettingsEntryId::MasterVolume,
    });
    assert_eq!(select_item_key(&config), SelectItemKey::Config(SettingsEntryId::MasterVolume));

    let binding = SelectItem::KeyBinding(crate::screens::settings_model::KeyBindingSelectRow {
        key_mode: KeyMode::K7,
        target: KeyBindingTarget::Action {
            action: InputActionConfig::E1,
            slot: KeyBindingSlot::KeyboardPrimary,
        },
    });
    assert_eq!(
        select_item_key(&binding),
        SelectItemKey::KeyBinding {
            key_mode: KeyMode::K7,
            target: KeyBindingTarget::Action {
                action: InputActionConfig::E1,
                slot: KeyBindingSlot::KeyboardPrimary,
            },
        }
    );
}
