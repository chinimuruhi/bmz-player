use super::*;

fn test_ir_provider(provider: &str, base_url: &str) -> IrProviderConfig {
    IrProviderConfig {
        provider: provider.to_string(),
        provider_key: String::new(),
        base_url: base_url.to_string(),
        enabled: false,
        account_display_name: String::new(),
        account_id: String::new(),
        send_policy: IrSendPolicyConfig::default(),
        role: IrProviderRoleConfig::default(),
        last_login_at: None,
        last_success_at: None,
    }
}

#[test]
fn ir_provider_presets_recognize_official_and_legacy_urls() {
    assert_eq!(
        classify_ir_provider_preset(&test_ir_provider(
            "bmz-official",
            "https://bmz-player.hyrorre.workers.dev"
        )),
        IrProviderPreset::BmzIr
    );
    assert_eq!(
        classify_ir_provider_preset(&test_ir_provider("rianIR", "https://rianir.link/api/")),
        IrProviderPreset::RianIr
    );
    assert_eq!(
        classify_ir_provider_preset(&test_ir_provider("rian-ir", "http://localhost:8888/api/")),
        IrProviderPreset::Other
    );
}

#[test]
fn applying_ir_provider_presets_writes_canonical_values() {
    let mut provider = test_ir_provider("custom", "http://localhost:8888/");
    apply_ir_provider_preset(&mut provider, IrProviderPreset::BmzIr);
    assert_eq!(provider.provider, "bmz");
    assert_eq!(provider.base_url, "https://bmz-player.hyrorre.workers.dev/");

    apply_ir_provider_preset(&mut provider, IrProviderPreset::RianIr);
    assert_eq!(provider.provider, "rian-ir");
    assert_eq!(provider.base_url, "https://rianir.link/");

    apply_ir_provider_preset(&mut provider, IrProviderPreset::Other);
    assert_eq!(provider.provider, "rian-ir");
    assert_eq!(provider.base_url, "https://rianir.link/");
}

#[test]
fn cjk_font_definitions_keep_latin_first_and_preserve_face_indices() {
    use bmz_render::FontCoverage;
    use bmz_render::renderer::SystemFontData;

    let defaults = egui::FontDefinitions::default();
    let fonts = cjk_font_definitions(vec![
        (FontCoverage::Korean, SystemFontData { bytes: vec![1], font_index: 3 }),
        (FontCoverage::Japanese, SystemFontData { bytes: vec![2], font_index: 7 }),
    ]);

    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let default_chain = defaults.families.get(&family).expect("default family");
        let chain = fonts.families.get(&family).expect("CJK family");
        assert_eq!(&chain[..default_chain.len()], default_chain);
        assert_eq!(
            &chain[default_chain.len()..],
            &["bmz_cjk_korean".to_string(), "bmz_cjk_japanese".to_string()]
        );
    }
    assert_eq!(fonts.font_data["bmz_cjk_korean"].index, 3);
    assert_eq!(fonts.font_data["bmz_cjk_japanese"].index, 7);
}

#[test]
fn decide_and_play_restrict_settings_panels() {
    assert!(!scene_restricts_settings("Select"));
    assert!(scene_restricts_settings("Decide"));
    assert!(scene_restricts_settings("Play"));
    assert!(!scene_restricts_settings("Result"));
}

#[test]
fn hidden_play_egui_uses_idle_frame_until_an_overlay_needs_full_state() {
    assert!(!egui_frame_needs_full_state(false, false, false, "Play", false));
    assert!(egui_frame_needs_full_state(true, false, false, "Play", false));
    assert!(egui_frame_needs_full_state(false, true, false, "Play", false));
    assert!(egui_frame_needs_full_state(false, false, true, "Select", false));
    assert!(egui_frame_needs_full_state(false, false, true, "Play", true));
    assert!(!egui_frame_needs_full_state(false, false, true, "Play", false));
}

#[test]
fn difficulty_table_source_label_shows_fetched_table_name() {
    let tables = vec![DifficultyTableRecord {
        id: 1,
        source_url: "https://example.com/header.json".to_string(),
        name: "発狂BMS難易度表".to_string(),
        symbol: "★".to_string(),
        level_order: vec!["1".to_string()],
        fetched_at: 1_700_000_000,
    }];

    assert_eq!(
        difficulty_table_source_label("https://example.com/header.json", &tables),
        "発狂BMS難易度表 (https://example.com/header.json)"
    );
}

#[test]
fn difficulty_table_source_label_keeps_url_before_first_fetch() {
    assert_eq!(
        difficulty_table_source_label("https://example.com/header.json", &[]),
        "https://example.com/header.json"
    );
}

#[test]
fn debug_log_filter_keeps_selected_level_and_more_severe_entries() {
    assert!(!DebugLogFilter::Info.allows(TracingLogLevel::Debug));
    assert!(DebugLogFilter::Info.allows(TracingLogLevel::Info));
    assert!(DebugLogFilter::Info.allows(TracingLogLevel::Error));
    assert!(DebugLogFilter::All.allows(TracingLogLevel::Trace));
}

#[test]
fn debug_log_copy_text_includes_level_target_and_message() {
    let entry = LogEntry {
        level: TracingLogLevel::Warn,
        target: "bmz_player::test".to_string(),
        message: "slow frame".to_string(),
    };

    let text = Localizer::new(AppLocale::En);
    assert_eq!(format_log_entry(&entry, text), "[WARN] bmz_player::test slow frame");

    let empty = LogEntry { message: String::new(), ..entry };
    assert_eq!(format_log_entry(&empty, text), "[WARN] bmz_player::test (no message)");
}

#[test]
fn restricted_profile_settings_keep_only_realtime_categories() {
    let baseline = ProfileConfig::new_default("default", "Default", 1);
    let mut edited = baseline.clone();
    edited.display_name = "Changed".to_string();
    edited.play.rule_mode = RuleMode::Dx;
    edited.audio_mix.master_volume = 23;
    edited.judge.input_offset_us = 4_000;
    edited.lane.hispeed = 3.25;
    edited.input.analog_scratch_threshold = 321;
    edited.input.keyboard_release_bounce_ms = 4;
    edited.input.controller_release_bounce_ms = 7;

    restore_restricted_profile_settings(&mut edited, baseline.clone());

    assert_eq!(edited.display_name, baseline.display_name);
    assert_eq!(edited.play.rule_mode, baseline.play.rule_mode);
    assert_eq!(edited.audio_mix.master_volume, 23);
    assert_eq!(edited.judge.input_offset_us, 4_000);
    assert_eq!(edited.lane.hispeed, 3.25);
    assert_eq!(edited.input.analog_scratch_threshold, 321);
    assert_eq!(edited.input.keyboard_release_bounce_ms, 4);
    assert_eq!(edited.input.controller_release_bounce_ms, 7);
}
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_test_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{name}-{nanos}-{counter}"))
}

fn test_offset_def(name: &str, id: i32) -> SkinOffsetDef {
    SkinOffsetDef {
        category: "test".to_string(),
        name: name.to_string(),
        id,
        x: true,
        y: true,
        w: true,
        h: true,
        r: true,
        a: true,
    }
}

#[test]
fn sanitize_profile_id_input_keeps_portable_path_chars_only() {
    let mut value = "abc_日本語-_.012/\\: xyz".to_string();

    sanitize_profile_id_input(&mut value);

    assert_eq!(value, "abc_-_012xyz");
}

#[test]
fn sanitize_profile_id_input_truncates_to_profile_id_limit() {
    let mut value = "a".repeat(80);

    sanitize_profile_id_input(&mut value);

    assert_eq!(value.len(), 64);
}

#[test]
fn skin_candidate_display_hides_bundled_origin_label_when_requested() {
    let candidate = SkinCandidate {
        name: "Default".to_string(),
        path: "resource:skins/default/select.json".to_string(),
        origin: SkinCandidateOrigin::Bundled,
    };

    assert_eq!(
        skin_candidate_display(&candidate, true, Localizer::new(crate::i18n::AppLocale::Ja),),
        "[同梱] Default (resource:skins/default/select.json)"
    );
    assert_eq!(
        skin_candidate_display(&candidate, false, Localizer::new(crate::i18n::AppLocale::Ja),),
        "Default (resource:skins/default/select.json)"
    );
}

#[test]
fn skin_candidate_display_keeps_user_origin_label() {
    let candidate = SkinCandidate {
        name: "Custom".to_string(),
        path: "data:skins/custom/play7.luaskin".to_string(),
        origin: SkinCandidateOrigin::User,
    };

    assert_eq!(
        skin_candidate_display(&candidate, false, Localizer::new(crate::i18n::AppLocale::Ja),),
        "[ユーザー] Custom (data:skins/custom/play7.luaskin)"
    );
}

#[test]
fn bundled_skin_origin_is_hidden_for_development_or_portable_layout() {
    let app_paths = AppPaths::from_dirs(
        PathBuf::from("data"),
        PathBuf::from("data"),
        PathBuf::from("data/cache"),
        PathBuf::from("data/logs"),
    );
    let mut catalog = SkinCatalog::default();
    catalog.select.push(SkinCandidate {
        name: "Default".to_string(),
        path: "resource:skins/default/select.json".to_string(),
        origin: SkinCandidateOrigin::Bundled,
    });
    catalog.select.push(SkinCandidate {
        name: "Custom".to_string(),
        path: "data:skins/custom/select.luaskin".to_string(),
        origin: SkinCandidateOrigin::User,
    });

    assert!(!show_bundled_skin_origin(&app_paths, &catalog));
}

#[test]
fn bundled_skin_origin_is_shown_when_user_candidates_share_a_regular_layout() {
    let app_paths = AppPaths::from_dirs(
        PathBuf::from("resources"),
        PathBuf::from("profile-data"),
        PathBuf::from("profile-data/cache"),
        PathBuf::from("profile-data/logs"),
    );
    let mut catalog = SkinCatalog::default();
    catalog.select.push(SkinCandidate {
        name: "Default".to_string(),
        path: "resource:skins/default/select.json".to_string(),
        origin: SkinCandidateOrigin::Bundled,
    });
    catalog.select.push(SkinCandidate {
        name: "Custom".to_string(),
        path: "data:skins/custom/select.luaskin".to_string(),
        origin: SkinCandidateOrigin::User,
    });

    assert!(show_bundled_skin_origin(&app_paths, &catalog));
}

#[test]
fn bundled_skin_origin_is_hidden_when_catalog_has_no_user_candidates() {
    let app_paths = AppPaths::from_dirs(
        PathBuf::from("resources"),
        PathBuf::from("profile-data"),
        PathBuf::from("profile-data/cache"),
        PathBuf::from("profile-data/logs"),
    );
    let mut catalog = SkinCatalog::default();
    catalog.select.push(SkinCandidate {
        name: "Default".to_string(),
        path: "resource:skins/default/select.json".to_string(),
        origin: SkinCandidateOrigin::Bundled,
    });

    assert!(!show_bundled_skin_origin(&app_paths, &catalog));
}

#[test]
fn sync_ir_provider_roles_keeps_only_primary_role() {
    let mut ir_config = IrConfig {
        primary_provider: "bmz-dev".to_string(),
        providers: vec![
            IrProviderConfig {
                provider: "bmz".to_string(),
                provider_key: "bmz".to_string(),
                base_url: "https://bmz-player.hyrorre.workers.dev".to_string(),
                enabled: true,
                account_display_name: String::new(),
                account_id: String::new(),
                send_policy: IrSendPolicyConfig::default(),
                role: IrProviderRoleConfig::Primary,
                last_login_at: None,
                last_success_at: None,
            },
            IrProviderConfig {
                provider: "bmz".to_string(),
                provider_key: "bmz-dev".to_string(),
                base_url: "http://localhost:3000".to_string(),
                enabled: true,
                account_display_name: String::new(),
                account_id: String::new(),
                send_policy: IrSendPolicyConfig::default(),
                role: IrProviderRoleConfig::SubmitOnly,
                last_login_at: None,
                last_success_at: None,
            },
        ],
        ..IrConfig::default()
    };

    assert!(sync_ir_provider_roles(&mut ir_config));
    assert_eq!(ir_config.providers[0].role, IrProviderRoleConfig::SubmitOnly);
    assert_eq!(ir_config.providers[1].role, IrProviderRoleConfig::Primary);

    ir_config.primary_provider.clear();
    assert!(sync_ir_provider_roles(&mut ir_config));
    assert_eq!(ir_config.providers[0].role, IrProviderRoleConfig::SubmitOnly);
    assert_eq!(ir_config.providers[1].role, IrProviderRoleConfig::SubmitOnly);
}

#[test]
fn clamp_panel_layout_fits_high_dpi_1920x1080_logical_viewport() {
    // 1920x1080 物理ウィンドウ @ 2x → egui 論理 960x540 相当。
    let constrain = egui::Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(928.0, 508.0));
    // egui 0.34 既定 style 付近の chrome 高さ (frame + title bar)。
    let chrome = egui::vec2(12.0, 58.0);
    let (default_inner, max_inner, pos) =
        clamp_panel_layout(constrain, chrome, 440.0, 560.0, egui::pos2(16.0, 480.0));

    let outer = default_inner + chrome;
    assert!(outer.x <= constrain.width() + 0.01);
    assert!(outer.y <= constrain.height() + 0.01);
    assert!(pos.x + outer.x <= constrain.max.x + 0.01);
    assert!(pos.y + outer.y <= constrain.max.y + 0.01);
    assert_eq!(pos, egui::pos2(16.0, 16.0));
    assert!(default_inner.y < 560.0);
    assert_eq!(max_inner, egui::vec2(916.0, 450.0));
}

#[test]
fn clamp_panel_layout_keeps_preferred_size_on_large_viewport() {
    let constrain = egui::Rect::from_min_size(egui::pos2(16.0, 16.0), egui::vec2(1888.0, 1048.0));
    let chrome = egui::vec2(12.0, 58.0);
    let (default_inner, max_inner, pos) =
        clamp_panel_layout(constrain, chrome, 440.0, 560.0, egui::pos2(16.0, 480.0));

    assert_eq!(default_inner, egui::vec2(440.0, 560.0));
    assert_eq!(max_inner, egui::vec2(1876.0, 990.0));
    // outer 高さ 618 のため y=480 では下端がはみ出す → 446 へクランプ。
    assert_eq!(pos, egui::pos2(16.0, 446.0));
}

#[test]
fn apply_settings_list_action_moves_and_removes_entries() {
    let mut items = vec!["a", "b", "c"];

    apply_settings_list_action(&mut items, SettingsListAction::MoveDown(0));
    assert_eq!(items, vec!["b", "a", "c"]);

    apply_settings_list_action(&mut items, SettingsListAction::MoveUp(2));
    assert_eq!(items, vec!["b", "c", "a"]);

    apply_settings_list_action(&mut items, SettingsListAction::Remove(1));
    assert_eq!(items, vec!["b", "a"]);
}

#[test]
fn apply_settings_list_action_moves_entry_to_index() {
    let mut items = vec!["a", "b", "c", "d"];

    apply_settings_list_action(&mut items, SettingsListAction::MoveTo { from: 0, to: 2 });
    assert_eq!(items, vec!["b", "c", "a", "d"]);

    apply_settings_list_action(&mut items, SettingsListAction::MoveTo { from: 3, to: 1 });
    assert_eq!(items, vec!["b", "d", "c", "a"]);
}

#[test]
fn apply_settings_list_action_ignores_invalid_moves() {
    let mut items = vec!["a", "b"];

    apply_settings_list_action(&mut items, SettingsListAction::MoveUp(0));
    apply_settings_list_action(&mut items, SettingsListAction::MoveDown(1));
    apply_settings_list_action(&mut items, SettingsListAction::MoveTo { from: 0, to: 2 });
    apply_settings_list_action(&mut items, SettingsListAction::MoveTo { from: 2, to: 0 });
    apply_settings_list_action(&mut items, SettingsListAction::Remove(2));

    assert_eq!(items, vec!["a", "b"]);
}

#[test]
fn directory_open_targets_expose_only_app_path_roots() {
    let root = unique_test_dir("bmz-ui-directory-targets");
    let app_paths = AppPaths::from_dirs(
        root.join("resources"),
        root.join("data"),
        root.join("cache"),
        root.join("logs"),
    );

    let targets = directory_open_targets(&app_paths);
    let labels = targets.iter().map(|target| target.label).collect::<Vec<_>>();
    let paths = targets.iter().map(|target| target.path).collect::<Vec<_>>();

    assert_eq!(labels, vec!["resource_dir", "data_dir", "cache_dir", "logs_dir"]);
    assert_eq!(
        paths,
        vec![
            app_paths.resource_dir.as_path(),
            app_paths.data_dir.as_path(),
            app_paths.cache_dir.as_path(),
            app_paths.logs_dir.as_path(),
        ]
    );
}

#[test]
fn combined_license_notice_uses_packaged_notice_files() {
    let root = unique_test_dir("bmz-ui-license-packaged");
    let resource_dir = root.join("resources");
    let license_dir = resource_dir.join("licenses");
    fs::create_dir_all(&license_dir).unwrap();
    fs::write(license_dir.join("third-party-notices.txt"), "packaged third party").unwrap();
    fs::write(license_dir.join("rust-dependency-licenses.txt"), "packaged rust report").unwrap();
    let app_paths =
        AppPaths::from_dirs(resource_dir, root.join("data"), root.join("cache"), root.join("logs"));

    let notice = combined_license_notice_text_with_repo_root(&app_paths, &root);

    assert!(notice.contains("packaged third party"));
    assert!(notice.contains("packaged rust report"));
    assert!(!notice.contains("The generated Rust dependency license report was not found."));
}

#[test]
fn combined_license_notice_uses_local_rust_report_for_development() {
    let root = unique_test_dir("bmz-ui-license-local");
    let resource_dir = root.join("resources");
    let license_dir = resource_dir.join("licenses");
    fs::create_dir_all(&license_dir).unwrap();
    fs::write(license_dir.join("third-party-notices.txt"), "packaged third party").unwrap();
    fs::write(root.join("rust-dependency-licenses.txt"), "local rust report").unwrap();
    let app_paths =
        AppPaths::from_dirs(resource_dir, root.join("data"), root.join("cache"), root.join("logs"));

    let notice = combined_license_notice_text_with_repo_root(&app_paths, &root);

    assert!(notice.contains("packaged third party"));
    assert!(notice.contains("local rust report"));
    assert!(!notice.contains("The generated Rust dependency license report was not found."));
}

#[test]
fn combined_license_notice_explains_missing_rust_report() {
    let root = unique_test_dir("bmz-ui-license-missing");
    let app_paths = AppPaths::from_dirs(
        root.join("resources"),
        root.join("data"),
        root.join("cache"),
        root.join("logs"),
    );

    let notice = combined_license_notice_text_with_repo_root(&app_paths, &root);

    assert!(notice.contains("BMZ Player Third-Party Notices"));
    assert!(notice.contains("The generated Rust dependency license report was not found."));
    assert!(notice.contains("cargo-about generate --workspace --locked --fail"));
}

#[test]
fn glob_candidates_lists_files_matching_simple_pattern() {
    let root = unique_test_dir("bmz-ui-glob");
    fs::create_dir_all(root.join("parts")).unwrap();
    fs::write(root.join("parts/a.png"), []).unwrap();
    fs::write(root.join("parts/b.png"), []).unwrap();
    fs::write(root.join("parts/c.txt"), []).unwrap();

    let candidates = glob_candidates(&root, "parts/*.png");

    assert_eq!(candidates.len(), 2);
    assert!(candidates.contains(&"parts/a.png".to_string()));
    assert!(candidates.contains(&"parts/b.png".to_string()));
}

#[test]
fn glob_candidates_strips_beatoraja_filter_suffix() {
    let root = unique_test_dir("bmz-ui-glob");
    fs::create_dir_all(root.join("parts/lanecover_lift")).unwrap();
    fs::write(root.join("parts/lanecover_lift/default.png"), []).unwrap();
    fs::write(root.join("parts/lanecover_lift/TYPE-M.png"), []).unwrap();

    let candidates = glob_candidates(&root, "parts/lanecover_lift/*.png|lanecover|");

    assert_eq!(candidates.len(), 2);
    assert!(candidates.contains(&"parts/lanecover_lift/TYPE-M.png".to_string()));
    assert!(candidates.contains(&"parts/lanecover_lift/default.png".to_string()));
}

#[test]
fn normalize_filepath_selection_maps_legacy_basename_to_relative_candidate() {
    let candidates =
        vec!["parts/gauge/default.png".to_string(), "parts/gauge/blue.png".to_string()];

    assert_eq!(
        normalize_filepath_selection("blue.png", &candidates).as_deref(),
        Some("parts/gauge/blue.png")
    );
    assert_eq!(normalize_filepath_selection("old/blue.png", &candidates), None);
}

#[test]
fn property_default_uses_matching_def_name_or_first_item() {
    let prop = SkinPropertyDef {
        category: String::new(),
        name: "Notes".to_string(),
        item: vec![
            bmz_render::skin::SkinPropertyItemDef { name: "Light".to_string(), op: 1 },
            bmz_render::skin::SkinPropertyItemDef { name: "Dark".to_string(), op: 2 },
        ],
        def: "Dark".to_string(),
    };
    assert_eq!(property_default(&prop), "Dark");

    let prop = SkinPropertyDef { def: "Missing".to_string(), ..prop };
    assert_eq!(property_default(&prop), "Light");
}

#[test]
fn filepath_default_matches_def_with_or_without_extension_case_insensitive() {
    let filepath = SkinFilepathDef {
        category: String::new(),
        name: "Notes".to_string(),
        path: "notes/*.png".to_string(),
        def: "default".to_string(),
    };
    let candidates = vec!["aaa.png".to_string(), "Default.PNG".to_string()];

    assert_eq!(filepath_default(&filepath, &candidates).as_deref(), Some("Default.PNG"));

    let filepath = SkinFilepathDef { def: "missing".to_string(), ..filepath };
    assert_eq!(filepath_default(&filepath, &candidates).as_deref(), Some("aaa.png"));
}

#[test]
fn filepath_default_uses_random_sentinel_for_random_def() {
    // def="Random" は具体ファイルへ固定せず、ランダム番兵を既定にする。
    let filepath = SkinFilepathDef {
        category: String::new(),
        name: "BG".to_string(),
        path: "bg/*.mp4".to_string(),
        def: "Random".to_string(),
    };
    let candidates = vec!["bg/one.mp4".to_string(), "bg/two.mp4".to_string()];
    assert_eq!(filepath_default(&filepath, &candidates).as_deref(), Some(RANDOM_FILE_SELECTION));
}

#[test]
fn filepath_default_prefers_default_stem_when_def_missing() {
    let filepath = SkinFilepathDef {
        category: String::new(),
        name: "Note".to_string(),
        path: "notes/*.png".to_string(),
        def: String::new(),
    };
    let candidates = vec!["pastel.png".to_string(), "default.png".to_string()];

    assert_eq!(filepath_default(&filepath, &candidates).as_deref(), Some("default.png"));
}

#[test]
fn fill_missing_skin_defaults_keeps_saved_values_and_fills_new_items() {
    let root = unique_test_dir("bmz-ui-defaults");
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(root.join("notes/aaa.png"), []).unwrap();
    fs::write(root.join("notes/default.png"), []).unwrap();
    let defs = SceneSkinDefs {
        property: vec![
            SkinPropertyDef {
                category: String::new(),
                name: "Lane".to_string(),
                item: vec![
                    bmz_render::skin::SkinPropertyItemDef { name: "Off".to_string(), op: 0 },
                    bmz_render::skin::SkinPropertyItemDef { name: "On".to_string(), op: 1 },
                ],
                def: "On".to_string(),
            },
            SkinPropertyDef {
                category: String::new(),
                name: "Saved".to_string(),
                item: vec![
                    bmz_render::skin::SkinPropertyItemDef { name: "A".to_string(), op: 0 },
                    bmz_render::skin::SkinPropertyItemDef { name: "B".to_string(), op: 1 },
                ],
                def: "A".to_string(),
            },
        ],
        filepath: vec![SkinFilepathDef {
            category: String::new(),
            name: "Notes".to_string(),
            path: "notes/*.png".to_string(),
            def: "default".to_string(),
        }],
        offset: Vec::new(),
    };
    let mut options = BTreeMap::from([("Saved".to_string(), "B".to_string())]);
    let mut files = BTreeMap::new();

    assert!(fill_missing_skin_defaults(&defs, Some(&root), &mut options, &mut files));

    assert_eq!(options.get("Lane").map(String::as_str), Some("On"));
    assert_eq!(options.get("Saved").map(String::as_str), Some("B"));
    assert_eq!(files.get("Notes").map(String::as_str), Some("notes/default.png"));
}

#[test]
fn fill_missing_skin_defaults_replaces_stale_option_selection() {
    let defs = SceneSkinDefs {
        property: vec![SkinPropertyDef {
            category: String::new(),
            name: "Graph".to_string(),
            item: vec![
                bmz_render::skin::SkinPropertyItemDef { name: "AC".to_string(), op: 922 },
                bmz_render::skin::SkinPropertyItemDef { name: "TYPE-M".to_string(), op: 923 },
            ],
            def: "AC".to_string(),
        }],
        filepath: Vec::new(),
        offset: Vec::new(),
    };
    let mut options = BTreeMap::from([("Graph".to_string(), "999".to_string())]);
    let mut files = BTreeMap::new();

    assert!(fill_missing_skin_defaults(&defs, None, &mut options, &mut files));

    assert_eq!(options.get("Graph").map(String::as_str), Some("AC"));
}

#[test]
fn fill_missing_skin_defaults_keeps_stale_file_selection_like_beatoraja() {
    let root = unique_test_dir("bmz-ui-defaults-stale");
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(root.join("notes/aaa.png"), []).unwrap();
    fs::write(root.join("notes/default.png"), []).unwrap();
    let defs = SceneSkinDefs {
        property: Vec::new(),
        filepath: vec![SkinFilepathDef {
            category: String::new(),
            name: "Notes".to_string(),
            path: "notes/*.png".to_string(),
            def: "default".to_string(),
        }],
        offset: Vec::new(),
    };
    let mut options = BTreeMap::new();
    let mut files = BTreeMap::from([("Notes".to_string(), "../old/default.png".to_string())]);

    assert!(!fill_missing_skin_defaults(&defs, Some(&root), &mut options, &mut files));

    assert_eq!(files.get("Notes").map(String::as_str), Some("../old/default.png"));
}

#[test]
fn play_skin_defs_include_beatoraja_common_offsets() {
    let defs = SceneSkinDefs::from_play_document(None);

    let offsets: Vec<_> =
        defs.offset.iter().map(|offset| (offset.id, offset.name.as_str())).collect();
    assert!(offsets.contains(&(10, "All offset(%)")));
    assert!(offsets.contains(&(30, "Notes offset")));
    assert!(offsets.contains(&(32, "Judge offset")));
    assert!(offsets.contains(&(33, "Judge Detail offset")));
    assert!(offsets.contains(&(SKIN_OFFSET_BAR_LINE, "Bar Line offset")));
}

#[test]
fn play_skin_defs_append_beatoraja_common_offsets_after_same_id_custom_defs() {
    let mut defs = SceneSkinDefs::default();
    defs.offset.push(SkinOffsetDef {
        category: "custom".to_string(),
        name: "Custom all".to_string(),
        id: 10,
        x: true,
        y: true,
        w: false,
        h: false,
        r: false,
        a: false,
    });

    defs.append_play_common_offsets();

    assert_eq!(defs.offset.iter().filter(|offset| offset.id == 10).count(), 2);
    assert_eq!(defs.offset.len(), 6);
    assert_eq!(
        defs.offset.iter().rfind(|offset| offset.id == 10).map(|offset| offset.name.as_str()),
        Some("All offset(%)")
    );
}

#[test]
fn play_skin_defs_enable_bar_line_alpha_when_skin_def_disables_it() {
    let mut defs = SceneSkinDefs::default();
    defs.offset.push(SkinOffsetDef {
        category: "custom".to_string(),
        name: "Custom bar".to_string(),
        id: SKIN_OFFSET_BAR_LINE,
        x: false,
        y: false,
        w: false,
        h: true,
        r: false,
        a: false,
    });

    defs.append_play_common_offsets();

    let bar_line = defs
        .offset
        .iter()
        .find(|offset| offset.id == SKIN_OFFSET_BAR_LINE)
        .expect("bar line offset def");
    assert!(bar_line.a);
}

#[test]
fn skin_offset_sync_prefers_name_and_updates_changed_definition_id() {
    let defs = vec![test_offset_def("Antique lane", 80)];
    let mut offsets = vec![
        SkinOffsetConfig {
            name: Some("Antique lane".to_string()),
            id: 70,
            x: 12,
            ..Default::default()
        },
        SkinOffsetConfig { id: 80, x: 99, ..Default::default() },
    ];

    assert!(sync_skin_offsets_with_defs(&defs, &mut offsets));
    assert_eq!(
        offsets,
        vec![SkinOffsetConfig {
            name: Some("Antique lane".to_string()),
            id: 80,
            x: 12,
            ..Default::default()
        }]
    );
}

#[test]
fn skin_offset_sync_expands_legacy_duplicate_id_into_independent_names() {
    let defs = vec![test_offset_def("Lane A", 42), test_offset_def("Lane B", 42)];
    let mut offsets = vec![SkinOffsetConfig { id: 42, y: -8, ..Default::default() }];

    assert!(sync_skin_offsets_with_defs(&defs, &mut offsets));
    assert_eq!(offsets.len(), 2);
    assert_eq!(offsets[0].name.as_deref(), Some("Lane A"));
    assert_eq!(offsets[1].name.as_deref(), Some("Lane B"));
    assert_eq!(offsets[0].y, -8);
    assert_eq!(offsets[1].y, -8);

    let mut edited = offsets[0].clone();
    edited.y = 24;
    assert!(update_skin_offset_value(&mut offsets, &defs[0], edited));
    assert_eq!(offsets[0].y, 24);
    assert_eq!(offsets[1].y, -8);
}

#[test]
fn skin_offset_sync_shares_first_named_value_across_duplicate_name_ids() {
    let defs = vec![test_offset_def("Shared", 51), test_offset_def("Shared", 52)];
    let mut offsets = vec![
        SkinOffsetConfig { name: Some("Shared".to_string()), id: 51, a: 120, ..Default::default() },
        SkinOffsetConfig { name: Some("Shared".to_string()), id: 52, a: 240, ..Default::default() },
    ];

    assert!(sync_skin_offsets_with_defs(&defs, &mut offsets));
    assert_eq!(offsets.iter().map(|offset| offset.id).collect::<Vec<_>>(), vec![51, 52]);
    assert!(offsets.iter().all(|offset| offset.a == 120));

    let mut edited = offsets[1].clone();
    edited.a = 64;
    assert!(update_skin_offset_value(&mut offsets, &defs[1], edited));
    assert!(offsets.iter().all(|offset| offset.a == 64));
}

#[test]
fn reset_scene_skin_to_defaults_clears_saved_values_and_restores_factory_defaults() {
    let root = unique_test_dir("bmz-ui-reset-scene");
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(root.join("notes/aaa.png"), []).unwrap();
    fs::write(root.join("notes/default.png"), []).unwrap();
    let defs = SceneSkinDefs {
        property: vec![SkinPropertyDef {
            category: String::new(),
            name: "Lane".to_string(),
            item: vec![
                bmz_render::skin::SkinPropertyItemDef { name: "Off".to_string(), op: 0 },
                bmz_render::skin::SkinPropertyItemDef { name: "On".to_string(), op: 1 },
            ],
            def: "On".to_string(),
        }],
        filepath: vec![SkinFilepathDef {
            category: String::new(),
            name: "Notes".to_string(),
            path: "notes/*.png".to_string(),
            def: "default".to_string(),
        }],
        offset: vec![SkinOffsetDef {
            category: "test".to_string(),
            name: "Judge".to_string(),
            id: 32,
            x: true,
            y: true,
            w: false,
            h: false,
            r: false,
            a: false,
        }],
    };
    let mut options = BTreeMap::from([("Lane".to_string(), "Off".to_string())]);
    let mut files = BTreeMap::from([("Notes".to_string(), "aaa.png".to_string())]);
    let mut offsets = vec![SkinOffsetConfig { id: 32, x: 99, ..Default::default() }];

    assert!(reset_scene_skin_to_defaults(
        &defs,
        Some(&root),
        &mut options,
        &mut files,
        &mut offsets
    ));

    assert_eq!(options.get("Lane").map(String::as_str), Some("On"));
    assert_eq!(files.get("Notes").map(String::as_str), Some("notes/default.png"));
    assert!(offsets.is_empty());
}

#[test]
fn reset_scene_skin_to_defaults_removes_named_defs_without_same_id_name_collision() {
    let defs = SceneSkinDefs { offset: vec![test_offset_def("Current", 32)], ..Default::default() };
    let mut options = BTreeMap::new();
    let mut files = BTreeMap::new();
    let mut offsets = vec![
        SkinOffsetConfig { name: Some("Current".to_string()), id: 32, x: 10, ..Default::default() },
        SkinOffsetConfig { name: Some("Other".to_string()), id: 32, x: 20, ..Default::default() },
    ];

    assert!(reset_scene_skin_to_defaults(&defs, None, &mut options, &mut files, &mut offsets));
    assert_eq!(offsets.len(), 1);
    assert_eq!(offsets[0].name.as_deref(), Some("Other"));
    assert_eq!(offsets[0].x, 20);
}

#[test]
fn skin_slot_history_restores_options_files_and_offsets_by_path() {
    let mut skin = SkinConfig {
        play7: "data/skins/ECFN/play/play7.luaskin".to_string(),
        play7_offsets: vec![SkinOffsetConfig {
            name: Some("Judge offset".to_string()),
            id: 32,
            x: 12,
            ..Default::default()
        }],
        ..SkinConfig::default()
    };
    skin.play7_options.insert("Judge".to_string(), "On".to_string());
    skin.play7_files.insert("Notes".to_string(), "default.png".to_string());

    save_skin_slot_history(&mut skin, SkinSlot::Play7);
    skin.play7 = "data/skins/Starseeker/play/play7.luaskin".to_string();
    skin.play7_options.insert("Judge".to_string(), "Off".to_string());
    skin.play7_files.insert("Notes".to_string(), "other.png".to_string());
    skin.play7_offsets = vec![SkinOffsetConfig {
        name: Some("Judge offset".to_string()),
        id: 32,
        x: -4,
        ..Default::default()
    }];
    save_skin_slot_history(&mut skin, SkinSlot::Play7);

    skin.play7 = "data/skins/ECFN/play/play7.luaskin".to_string();
    restore_skin_slot_history(&mut skin, SkinSlot::Play7);

    assert_eq!(skin.play7_options.get("Judge").map(String::as_str), Some("On"));
    assert_eq!(skin.play7_files.get("Notes").map(String::as_str), Some("default.png"));
    assert_eq!(
        skin.play7_offsets,
        vec![SkinOffsetConfig {
            name: Some("Judge offset".to_string()),
            id: 32,
            x: 12,
            ..Default::default()
        }]
    );
}

#[test]
fn skin_slot_history_isolates_same_path_by_slot() {
    let shared_path = "data/skins/shared/play.luaskin".to_string();
    let mut skin = SkinConfig {
        play7: shared_path.clone(),
        play14: shared_path,
        play7_offsets: vec![SkinOffsetConfig { id: 30, h: 7, ..Default::default() }],
        play14_offsets: vec![SkinOffsetConfig { id: 30, h: 14, ..Default::default() }],
        ..SkinConfig::default()
    };

    save_skin_slot_history(&mut skin, SkinSlot::Play7);
    save_skin_slot_history(&mut skin, SkinSlot::Play14);
    skin.play7_offsets.clear();
    skin.play14_offsets.clear();
    restore_skin_slot_history(&mut skin, SkinSlot::Play7);
    restore_skin_slot_history(&mut skin, SkinSlot::Play14);

    assert_eq!(skin.play7_offsets[0].h, 7);
    assert_eq!(skin.play14_offsets[0].h, 14);
}

#[test]
fn skin_slot_history_restores_legacy_path_only_entry() {
    let path = "data/skins/legacy/play7.luaskin".to_string();
    let mut skin = SkinConfig { play7: path.clone(), ..SkinConfig::default() };
    skin.history.insert(
        path.clone(),
        SkinHistoryEntryConfig {
            offsets: vec![SkinOffsetConfig { id: 30, h: 12, ..Default::default() }],
            ..Default::default()
        },
    );

    restore_skin_slot_history(&mut skin, SkinSlot::Play7);

    assert_eq!(skin.play7_offsets[0].h, 12);
    assert!(skin.history.contains_key(&skin_slot_history_key(SkinSlot::Play7, &path)));
}

#[test]
fn skin_reload_diff_scopes_play_slot_without_select_reload() {
    let before = SkinConfig::default();
    let mut after = before.clone();
    after.play7_files.insert("Notes".to_string(), "blue.png".to_string());

    let request = skin_reload_request_from_diff(&before, &after);

    assert!(request.play7);
    assert!(!request.select);
    assert!(!request.play5);
    assert!(!request.result);
    assert!(request.any_reload());
}

#[test]
fn skin_reload_diff_separates_result_and_course_result_slots() {
    let before = SkinConfig::default();
    let mut after = before.clone();
    after.course_result = "data/skins/course/result.luaskin".to_string();
    after.course_result_options.insert("Layout".to_string(), "Course".to_string());

    let request = skin_reload_request_from_diff(&before, &after);

    assert!(request.course_result);
    assert!(!request.result);

    let mut after = before.clone();
    after.result_files.insert("Background".to_string(), "normal.png".to_string());

    let request = skin_reload_request_from_diff(&before, &after);

    assert!(request.result);
    assert!(!request.course_result);
}

#[test]
fn skin_reload_diff_marks_each_offset_slot_for_redecode() {
    type SkinReloadCase = (&'static str, fn(&mut SkinConfig), fn(SkinReloadRequest) -> bool);
    let cases: &[SkinReloadCase] = &[
        ("select", |skin| skin.select_offsets.push(Default::default()), |request| request.select),
        ("decide", |skin| skin.decide_offsets.push(Default::default()), |request| request.decide),
        ("play4", |skin| skin.play4_offsets.push(Default::default()), |request| request.play4),
        ("play5", |skin| skin.play5_offsets.push(Default::default()), |request| request.play5),
        ("play6", |skin| skin.play6_offsets.push(Default::default()), |request| request.play6),
        ("play7", |skin| skin.play7_offsets.push(Default::default()), |request| request.play7),
        ("play8", |skin| skin.play8_offsets.push(Default::default()), |request| request.play8),
        ("play9", |skin| skin.play9_offsets.push(Default::default()), |request| request.play9),
        ("play10", |skin| skin.play10_offsets.push(Default::default()), |request| request.play10),
        ("play14", |skin| skin.play14_offsets.push(Default::default()), |request| request.play14),
        ("result", |skin| skin.result_offsets.push(Default::default()), |request| request.result),
        (
            "course_result",
            |skin| skin.course_result_offsets.push(Default::default()),
            |request| request.course_result,
        ),
    ];

    for &(slot, change, slot_requested) in cases {
        let before = SkinConfig::default();
        let mut after = before.clone();
        change(&mut after);

        let request = skin_reload_request_from_diff(&before, &after);

        assert!(request.offsets, "{slot} offset did not mark runtime offset update");
        assert!(slot_requested(request), "{slot} offset did not mark scene re-decode");
        assert!(request.any_reload(), "{slot} offset did not request reload");
        assert!(request.any());
    }
}
