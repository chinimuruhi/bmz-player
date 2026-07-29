use super::*;

#[test]
fn lua_skin_converts_customs_boolean_toggle_to_runtime_event() {
    let root = unique_test_dir("bmz-skin-lua-runtime-toggle");
    fs::create_dir_all(&root).unwrap();
    fs::write(
            root.join("result.luaskin"),
            r#"
            local timer_util = require("timer_util")
            CUSTOMS = {
                show_primary = true,
                show_secondary = false,
            }
            CUSTOMS.toggle = function()
                CUSTOMS.show_primary = not CUSTOMS.show_primary
                CUSTOMS.show_secondary = not CUSTOMS.show_secondary
            end
            local primary_timer = timer_util.timer_observe_boolean(function()
                return CUSTOMS.show_primary
            end)
            local secondary_timer = timer_util.timer_observe_boolean(function()
                return CUSTOMS.show_secondary
            end)
            return {
                type = 7,
                image = {
                    {
                        id = "switch",
                        src = 1,
                        x = 0,
                        y = 0,
                        w = 10,
                        h = 10,
                        act = function() return CUSTOMS.toggle() end,
                    },
                },
                destination = {
                    { id = "switch", timer = primary_timer, dst = {{ x = 0, y = 0, w = 10, h = 10 }} },
                    { id = "switch", timer = secondary_timer, dst = {{ x = 0, y = 0, w = 10, h = 10 }} },
                },
            }
            "#,
        )
        .unwrap();

    let loaded =
        load_lua_skin_value(&root.join("result.luaskin"), &BTreeMap::new(), &BTreeMap::new())
            .unwrap();

    assert_eq!(
        loaded.value["runtimeFlag"],
        serde_json::json!([
            { "id": 0, "initial": true },
            { "id": 1, "initial": false }
        ])
    );
    assert_eq!(
        loaded.value["dynamicTimer"],
        serde_json::json!([
            { "id": 9000, "observe": "runtime_flag(0)" },
            { "id": 9001, "observe": "runtime_flag(1)" }
        ])
    );
    assert_eq!(
        loaded.value["runtimeEvent"],
        serde_json::json!([{ "id": -20000, "toggleFlags": [0, 1] }])
    );
    assert_eq!(loaded.value["image"][0]["act"], serde_json::json!(-20000));
    assert!(
        !loaded.warnings.iter().any(|warning| {
            warning.message.contains("runtime Lua state changes are unsupported")
        })
    );
}

#[test]
fn milliondollar_result_runtime_toggles_convert_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/MILLIONDOLLAR/result.luaskin");
    if !skin_path.is_file() {
        return;
    }

    let loaded = load_lua_skin_value(&skin_path, &BTreeMap::new(), &BTreeMap::new()).unwrap();
    assert_eq!(loaded.value["runtimeFlag"].as_array().map(Vec::len), Some(4));
    assert_eq!(loaded.value["runtimeEvent"].as_array().map(Vec::len), Some(2));
    assert!(loaded.value["dynamicTimer"].as_array().is_some_and(|timers| {
        !timers.is_empty()
            && timers.iter().all(|timer| {
                timer["observe"].as_str().is_some_and(|observe| observe.contains("runtime_flag("))
            })
    }));
    assert!(
        !loaded.warnings.iter().any(|warning| {
            warning.message.contains("runtime Lua state changes are unsupported")
        })
    );
}

#[test]
fn milliondollar_result_audio_events_convert_when_available() {
    let skin_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/skins/MILLIONDOLLAR/result.luaskin");
    if !skin_path.is_file() {
        return;
    }
    let options = BTreeMap::from([("BGM".to_string(), "使用する".to_string())]);
    let loaded = load_lua_skin(&skin_path, SkinKind::Result, &options, &BTreeMap::new()).unwrap();

    assert!(loaded.document.value.iter().any(|value| value.id == "Number_Todayplayednotes"));
    assert!(loaded.document.scene_audio.iter().any(|action| {
        action.action == bmz_skin_document::SkinAudioActionKind::Loop
            && action.path == "RESULT/parts/BGM.ogg"
    }));
    let event = loaded
        .document
        .custom_events
        .iter()
        .find(|event| event.id == 1001)
        .expect("timer 2 audio event");
    assert_eq!(event.timer, 2);
    assert!(event.once);
    assert_eq!(event.audio_actions.len(), 2);
    assert!(!loaded.warnings.iter().any(|warning| {
        warning.message.contains("customEvents") || warning.message.contains("audio_")
    }));
}

#[test]
fn lua_skin_config_get_path_prefers_filepath_default() {
    let root = unique_test_dir("bmz-skin-lua");
    fs::create_dir_all(root.join("parts")).unwrap();
    fs::write(root.join("parts/aaa.png"), []).unwrap();
    fs::write(root.join("parts/default.png"), []).unwrap();
    fs::write(
        root.join("play7.luaskin"),
        r#"
            local image_path = "parts/*.png"
            if skin_config then
                image_path = skin_config.get_path(image_path)
            end
            return {
                type = 0,
                filepath = {
                    { name = "Notes", path = "parts/*.png", def = "default" }
                },
                source = {
                    { id = "notes", path = image_path }
                }
            }
            "#,
    )
    .unwrap();

    let loaded =
        load_lua_skin_value(&root.join("play7.luaskin"), &BTreeMap::new(), &BTreeMap::new())
            .unwrap();

    assert_eq!(
        loaded.value["source"][0]["path"].as_str().and_then(|path| {
            std::path::Path::new(path).file_name().and_then(|name| name.to_str())
        }),
        Some("default.png")
    );
}

#[test]
fn lua_skin_config_get_path_falls_back_when_selection_missing() {
    let root = unique_test_dir("bmz-skin-lua");
    fs::create_dir_all(root.join("parts")).unwrap();
    fs::write(root.join("parts/a.png"), []).unwrap();
    fs::write(root.join("parts/z.png"), []).unwrap();
    fs::write(
        root.join("play7.luaskin"),
        r#"
            local cover_path = "parts/*.png"
            if skin_config then
                cover_path = skin_config.get_path(cover_path)
            end
            return {
                type = 0,
                filepath = {
                    { name = "Cover", path = "parts/*.png", def = "a" }
                },
                source = {
                    { id = "cover", path = cover_path }
                }
            }
            "#,
    )
    .unwrap();

    // 存在しないファイルを選択 → beatoraja と同じく列挙候補へフォールバック。
    let files = BTreeMap::from([("Cover".to_string(), "parts/missing.png".to_string())]);
    let loaded =
        load_lua_skin_value(&root.join("play7.luaskin"), &BTreeMap::new(), &files).unwrap();

    let filename = loaded.value["source"][0]["path"]
        .as_str()
        .and_then(|path| std::path::Path::new(path).file_name().and_then(|name| name.to_str()));
    assert!(matches!(filename, Some("a.png" | "z.png")));
}

#[test]
fn lua_skin_dofile_resolves_get_path_joined_with_forward_slash() {
    // Regression: `skin_config.get_path` returns an absolute path and skins
    // build the dofile target by concatenating `"/sub.lua"`. On Windows the
    // skin root must not be a `\\?\` verbatim path, or the mixed-separator
    // path fails to canonicalize and the dofile is silently lost.
    let root = unique_test_dir("bmz-skin-lua");
    fs::create_dir_all(root.join("parts/frame")).unwrap();
    fs::write(
        root.join("parts/frame/mod.lua"),
        r#"return { destination = { { id = "x", dst = {{ x = 1, y = 2, w = 3, h = 4 }} } } }"#,
    )
    .unwrap();
    fs::write(
        root.join("play7.luaskin"),
        r#"
            if skin_config then
                local dir = skin_config.get_path("parts/*")
                local sub = dofile(dir .. "/mod.lua")
                return { type = 0, destination = sub.destination }
            else
                return { type = 0 }
            end
            "#,
    )
    .unwrap();

    let loaded = load_lua_skin(
        &root.join("play7.luaskin"),
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();

    assert_eq!(loaded.document.destination.len(), 1);
}

#[test]
fn lua_skin_timer_util_supports_observe_boolean_for_dofile_parts() {
    let root = unique_test_dir("bmz-skin-lua");
    fs::create_dir_all(root.join("parts/frame")).unwrap();
    fs::write(
        root.join("parts/frame/mod.lua"),
        r#"
            local timer_util = require("timer_util")
            return {
                destination = {
                    {
                        id = "frame-panel",
                        timer = timer_util.timer_observe_boolean(function()
                            return true
                        end),
                        dst = { { x = 1, y = 2, w = 3, h = 4 } },
                    },
                },
            }
            "#,
    )
    .unwrap();
    fs::write(
        root.join("play7.luaskin"),
        r#"
            if skin_config then
                local dir = skin_config.get_path("parts/*")
                local sub = dofile(dir .. "/mod.lua")
                return { type = 0, destination = sub.destination }
            else
                return { type = 0 }
            end
            "#,
    )
    .unwrap();

    let loaded = load_lua_skin(
        &root.join("play7.luaskin"),
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();

    assert_eq!(loaded.document.destination.len(), 1);
    let bmz_skin_document::DestinationListEntry::Single(destination) =
        &loaded.document.destination[0]
    else {
        panic!("destination should be single");
    };
    assert_eq!(destination.id, "frame-panel");
    assert_eq!(destination.timer, Some(bmz_skin_document::SKIN_DYNAMIC_TIMER_BASE));
    assert_eq!(loaded.document.dynamic_timers.len(), 1);
    assert_eq!(loaded.document.dynamic_timers[0].observe, "number(0) >= 0");
}

#[test]
fn lua_skin_timer_observe_reuses_id_for_same_runtime_predicate() {
    let root = unique_test_dir("bmz-skin-lua-shared-observe-timer");
    fs::create_dir_all(&root).unwrap();
    fs::write(
            root.join("result.luaskin"),
            r#"
            local main_state = require("main_state")
            local timer_util = require("timer_util")
            local function visible_timer()
                return timer_util.timer_observe_boolean(function()
                    return main_state.number(300) > 0
                end)
            end
            return {
                type = 7,
                destination = {
                    { id = "segment-a", timer = visible_timer(), dst = {{ x = 0, y = 0, w = 1, h = 1 }} },
                    { id = "segment-b", timer = visible_timer(), dst = {{ x = 1, y = 0, w = 1, h = 1 }} },
                },
            }
            "#,
        )
        .unwrap();

    let loaded = load_lua_skin(
        &root.join("result.luaskin"),
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();

    assert_eq!(loaded.document.dynamic_timers.len(), 1);
    assert_eq!(loaded.document.dynamic_timers[0].observe, "number(300) > 0");
    let timers = loaded
        .document
        .destination
        .iter()
        .filter_map(|entry| match entry {
            bmz_skin_document::DestinationListEntry::Single(destination) => destination.timer,
            bmz_skin_document::DestinationListEntry::Conditional { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(timers, vec![bmz_skin_document::SKIN_DYNAMIC_TIMER_BASE; 2]);
}

#[test]
fn lua_skin_timer_observe_infers_is_gauge_iidx_global() {
    let root = unique_test_dir("bmz-skin-lua-iidx");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("play7.luaskin"),
        r#"
            local timer_util = require("timer_util")
            return {
                type = 0,
                destination = {
                    {
                        id = "groove_frame",
                        timer = timer_util.timer_observe_boolean(function()
                            return not is_gauge_iidx
                        end),
                        dst = { { x = 0, y = 0, w = 1, h = 1 } },
                    },
                    {
                        id = "groove_frame_iidx",
                        timer = timer_util.timer_observe_boolean(function()
                            return is_gauge_iidx
                        end),
                        dst = { { x = 0, y = 0, w = 1, h = 1 } },
                    },
                },
            }
            "#,
    )
    .unwrap();

    let loaded = load_lua_skin(
        &root.join("play7.luaskin"),
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();

    assert_eq!(loaded.document.dynamic_timers.len(), 2);
    assert_eq!(
        loaded.document.dynamic_timers[0].observe,
        "gauge_type() != 4 and gauge_type() != 5"
    );
    assert_eq!(loaded.document.dynamic_timers[1].observe, "gauge_type() == 4 or gauge_type() == 5");
}

#[test]
fn lua_skin_timer_observe_infers_starseeker_default_gauge_iidx_global_as_constant() {
    let root = unique_test_dir("bmz-skin-lua-iidx-gauge-default");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("play7.luaskin"),
        r#"
            local timer_util = require("timer_util")
            return {
                type = 0,
                property = {
                    {
                        name = "グルーヴゲージ表示",
                        def = "default",
                        item = {
                            { name = "default", op = 930 },
                            { name = "gauge_off", op = 931 },
                            { name = "all_off", op = 932 },
                        },
                    },
                },
                destination = {
                    {
                        id = "groove_frame",
                        timer = timer_util.timer_observe_boolean(function()
                            return not is_gauge_iidx
                        end),
                        dst = { { x = 0, y = 0, w = 1, h = 1 } },
                    },
                    {
                        id = "groove_frame_iidx",
                        timer = timer_util.timer_observe_boolean(function()
                            return is_gauge_iidx
                        end),
                        dst = { { x = 0, y = 0, w = 1, h = 1 } },
                    },
                },
            }
            "#,
    )
    .unwrap();

    let loaded = load_lua_skin(
        &root.join("play7.luaskin"),
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();

    assert_eq!(loaded.document.dynamic_timers.len(), 2);
    assert_eq!(loaded.document.dynamic_timers[0].observe, "number(0) >= 0");
    assert_eq!(loaded.document.dynamic_timers[1].observe, "number(0) < 0");
}

#[test]
fn lua_skin_infers_gauge_type_class_predicate_covers_ids_6_7_8() {
    // 段位ゲージ用 skin が `gauge_type() >= 6` のような draw 条件を書いたとき、
    // probe は 6 / 7 / 8 すべてを検出して or 連結する必要がある。
    let root = unique_test_dir("bmz-skin-lua-class-gauge");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("play7.luaskin"),
        r#"
            local main_state = require("main_state")
            return {
                type = 0,
                destination = {
                    {
                        id = "class_gauge_overlay",
                        draw = function() return main_state.gauge_type() >= 6 end,
                        dst = {{ x = 0, y = 0, w = 1, h = 1 }},
                    },
                },
            }
            "#,
    )
    .unwrap();

    let loaded = load_lua_skin(
        &root.join("play7.luaskin"),
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();

    let bmz_skin_document::DestinationListEntry::Single(destination) =
        &loaded.document.destination[0]
    else {
        panic!("destination should be single");
    };
    assert_eq!(destination.draw, "gauge_type() == 6 or gauge_type() == 7 or gauge_type() == 8");
}

#[test]
fn lua_skin_infers_or_draw_and_division_graph_value() {
    let root = unique_test_dir("bmz-skin-lua");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("play7.luaskin"),
        r#"
            local main_state = require("main_state")
            return {
                type = 0,
                graph = {
                    {
                        id = "ratio",
                        src = 1,
                        x = 0,
                        y = 0,
                        w = 10,
                        h = 10,
                        value = function()
                            local fast = main_state.number(410)
                            local slow = main_state.number(411)
                            local total = fast + slow
                            if total == 0 then return 0 end
                            return fast / total
                        end,
                    },
                },
                destination = {
                    {
                        id = "panel",
                        draw = function()
                            return main_state.number(77) > 0 or main_state.number(150) > 0
                        end,
                        dst = {{ x = 1, y = 2, w = 3, h = 4 }},
                    },
                },
            }
            "#,
    )
    .unwrap();

    let loaded = load_lua_skin(
        &root.join("play7.luaskin"),
        SkinKind::Play,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();

    assert!(
        loaded.warnings.is_empty(),
        "warnings: {:?}",
        loaded.warnings.iter().map(|warning| warning.message.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(loaded.document.graph[0].value_expr, "(number(410))/(number(410)+number(411))");
    let bmz_skin_document::DestinationListEntry::Single(destination) =
        &loaded.document.destination[0]
    else {
        panic!("destination should be single");
    };
    assert_eq!(destination.draw, "number(77) > 0 or number(150) > 0");
}

#[test]
fn lua_skin_infers_option_weighted_graph_value() {
    let root = unique_test_dir("bmz-skin-lua-option-weighted");
    fs::create_dir_all(&root).unwrap();
    fs::write(
            root.join("select.luaskin"),
            r#"
            local main_state = require("main_state")
            return {
                type = 0,
                graph = {
                    {
                        id = "difficulty",
                        src = 1,
                        x = 0,
                        y = 0,
                        w = 10,
                        h = 10,
                        value = function()
                            local rank
                            if main_state.option(180) then
                                rank = 1.7
                            elseif main_state.option(181) then
                                rank = 1.5
                            elseif main_state.option(182) then
                                rank = 1.3
                            end
                            if rank < 0 then rank = 0 end
                            return (main_state.number(350) / 25 + main_state.number(351) / 8.3) * rank * 1.5
                        end,
                    },
                },
            }
            "#,
        )
        .unwrap();

    let loaded = load_lua_skin(
        &root.join("select.luaskin"),
        SkinKind::Select,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();

    assert!(
        loaded.warnings.is_empty(),
        "warnings: {:?}",
        loaded.warnings.iter().map(|warning| warning.message.as_str()).collect::<Vec<_>>()
    );
    let expr = &loaded.document.graph[0].value_expr;
    assert!(expr.contains("*option(180)*number(350)"));
    assert!(expr.contains("*option(181)*number(351)"));
    assert!(expr.contains("*option(182)*number(350)"));
}

#[test]
fn lua_skin_infers_or_eq_zero_and_lt_zero_draw() {
    let root = unique_test_dir("bmz-skin-lua-or-zero");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("result.luaskin"),
        r#"
            local main_state = require("main_state")
            return {
                type = 0,
                destination = {
                    {
                        id = "miss-f",
                        draw = function()
                            return main_state.number(71) == 0 or main_state.number(150) == 0
                        end,
                        dst = {{ x = 0, y = 0, w = 1, h = 1 }},
                    },
                    {
                        id = "zero-mask",
                        draw = function()
                            return main_state.number(77) < 0 or main_state.number(150) < 0
                        end,
                        dst = {{ x = 0, y = 0, w = 1, h = 1 }},
                    },
                },
            }
            "#,
    )
    .unwrap();

    let loaded = load_lua_skin(
        &root.join("result.luaskin"),
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(
        loaded.warnings.is_empty(),
        "warnings: {:?}",
        loaded.warnings.iter().map(|w| w.message.as_str()).collect::<Vec<_>>()
    );
    let bmz_skin_document::DestinationListEntry::Single(miss) = &loaded.document.destination[0]
    else {
        panic!("expected single destination");
    };
    let bmz_skin_document::DestinationListEntry::Single(mask) = &loaded.document.destination[1]
    else {
        panic!("expected single destination");
    };
    assert_eq!(miss.draw, "number(71) == 0 or number(150) == 0");
    assert_eq!(mask.draw, "number(77) < 0 or number(150) < 0");
}

#[test]
fn lua_skin_infers_result_average_timing_sign_draw() {
    let root = unique_test_dir("bmz-skin-lua-average-timing-sign");
    fs::create_dir_all(&root).unwrap();
    fs::write(
            root.join("result.luaskin"),
            r#"
            local main_state = require("main_state")
            return {
                type = 7,
                image = {
                    { id = "judge_adv_f", src = "src", x = 0, y = 0, w = 52, h = 12 },
                    { id = "judge_adv_s", src = "src", x = 0, y = 12, w = 52, h = 12 },
                    { id = "judge_adv_non_negative", src = "src", x = 0, y = 24, w = 52, h = 12 },
                },
                destination = {
                    {
                        id = "judge_adv_s",
                        draw = function()
                            local ave_timing = main_state.number(374) + (main_state.number(375) * 0.01)
                            return ave_timing < 0
                        end,
                        dst = {{ x = 424, y = 132, w = 52, h = 12 }},
                    },
                    {
                        id = "judge_adv_f",
                        draw = function()
                            local ave_timing = main_state.number(374) + (main_state.number(375) * 0.01)
                            return 0 < ave_timing
                        end,
                        dst = {{ x = 424, y = 132, w = 52, h = 12 }},
                    },
                    {
                        id = "judge_adv_non_negative",
                        draw = function()
                            local ave_timing = main_state.number(374) + (main_state.number(375) * 0.01)
                            return ave_timing >= 0
                        end,
                        dst = {{ x = 424, y = 132, w = 52, h = 12 }},
                    },
                },
            }
            "#,
        )
        .unwrap();

    let loaded = load_lua_skin(
        &root.join("result.luaskin"),
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(
        loaded.warnings.is_empty(),
        "warnings: {:?}",
        loaded.warnings.iter().map(|w| w.message.as_str()).collect::<Vec<_>>()
    );
    let bmz_skin_document::DestinationListEntry::Single(slow) = &loaded.document.destination[0]
    else {
        panic!("expected slow destination");
    };
    let bmz_skin_document::DestinationListEntry::Single(fast) = &loaded.document.destination[1]
    else {
        panic!("expected fast destination");
    };
    let bmz_skin_document::DestinationListEntry::Single(non_negative) =
        &loaded.document.destination[2]
    else {
        panic!("expected non-negative destination");
    };
    assert_eq!(slow.draw, "number(374) < 0 or number(375) < 0");
    assert_eq!(fast.draw, "number(374) > 0 or number(375) > 0");
    assert_eq!(non_negative.draw, "number(374) >= 0 and number(375) >= 0");
}

#[test]
fn lua_skin_infers_all_terminal_timers_off_draw() {
    let root = unique_test_dir("bmz-skin-lua-all-timers-off");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("result.luaskin"),
        r#"
            local main_state = require("main_state")
            return {
                type = 7,
                image = {
                    { id = "irWait", src = "src", x = 0, y = 0, w = 10, h = 10 },
                },
                destination = {
                    {
                        id = "irWait",
                        timer = 172,
                        draw = function()
                            return main_state.timer(173) == main_state.timer_off_value
                                and main_state.timer(174) == main_state.timer_off_value
                        end,
                        dst = {{ x = 0, y = 0, w = 10, h = 10 }},
                    },
                },
            }
            "#,
    )
    .unwrap();

    let loaded = load_lua_skin(
        &root.join("result.luaskin"),
        SkinKind::Result,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
    .unwrap();
    let bmz_skin_document::DestinationListEntry::Single(wait) = &loaded.document.destination[0]
    else {
        panic!("expected wait destination");
    };
    assert_eq!(wait.draw, "timer(173) == timer_off and timer(174) == timer_off");
}
